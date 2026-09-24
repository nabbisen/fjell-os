# RFC-0.34-001 R1, §A–§E, and what D8 needs that the kernel does not offer

**Governing RFC:** [../accepted/RFC-0.34-001-a-second-presentation.md](../accepted/RFC-0.34-001-a-second-presentation.md)
**Handoff:** [../handoffs/RFC-0.34-001-a-second-presentation/implementation-handoff.md](../handoffs/RFC-0.34-001-a-second-presentation/implementation-handoff.md)

Written after R1 and before any code, in the handoff's order. Every absence is
probed with `/usr/bin/grep -a` and a control on the file the claim is about. This
document contains **measurements** (marked as such) and **design** (marked as
such). Nothing below the "Design" headings has been run yet; the review request
will say which of it survived contact.

---

## R1 — the figures, re-derived

| Claim (RFC or handoff) | What the tree says | |
|---|---|---|
| `proxy-text`: 211 lines in `main.rs`, 1,069 in the crate | `main.rs` **211**, `lib.rs` 305, `renderer.rs` 540, `rt.rs` 13 = **1,069** (`wc -l`) | agrees |
| The last assigned `ImageId` is `0x1D` | **`0x1E`** — `DRIVER_UART = ImageId(0x1E)`, `crates/fjell-abi/src/service.rs:140`. `SYNCD` is `0x1D`. The next free id is **`0x1F`**. Control: the same grep finds `SYNCD` at `0x1D`, so the pattern matches this file. | **differs** |
| 29 prebuilt binaries | `ls crates/fjell-kernel/prebuilt/*.bin \| wc -l` → **29** | agrees |
| Adding a proxy takes the set 29 → 30 | **29 → 31 under my design** (a presentation and a relay, below). The reason is D8, not the renderer. | flagged, not a disagreement yet |
| `semantic-stream` forwards with a blocking call before it replies | `PUBLISH_COMMIT` arm, `crates/services/fjell-semantic-stream/src/main.rs:204-243`: `forward_to_proxy_text(&out[..n])` at line 225, `reply(PUBLISH_OK…)` at line 233. `forward_to_proxy_text` is `chunked::send`, which is one blocking `ipc_call` per message (`crates/fjell-service-api/src/lib.rs`, `ipc_call4`). | agrees |
| E-049 removes the CI package lists this line would extend | **Not landed.** E-049 is `ACCEPTED`, tracked to RFC-0.33-004. `.github/workflows/ci.yml:169` lists `-p fjell-proxy-text` by hand, and a cross-check list beside it names the service binaries. New crates need entries in both. | differs from the RFC's framing |
| `semantic` profile output: 313 lines | **314** on this tree (`wc -l` of the run's `serial.log`) | differs by one |
| … with `proxy-text` never started: 125 lines | **124** | differs by one |
| … with `proxy-text` faulting mid-run: 229 lines | **229**, `[STATE]` 3, `[EVENT]` 1, `[INTENT]` 1, `TEST:M7:PASS` 1, `driver-uart: ready` 0 | agrees |

### E-058, reproduced before any change (measured)

In a scratch `git worktree` under `target/scratch/` (removed at the end), on
`17ebb0c`, `semantic` profile, 60 s:

| | lines | `[STATE]` | `[EVENT]` | `[INTENT]` | `TEST:M7:PASS` | `driver-uart: ready` | `action accepted` |
|---|---|---|---|---|---|---|---|
| normal | 314 | 8 | 3 | 3 | 2 | 1 | 5 |
| **V1** `init` does not spawn `proxy-text` | **124** | 0 | 0 | 0 | 0 | 0 | 0 |
| **V2** `proxy-text` faults on its 40th received call | **229** | 3 | 1 | 1 | 1 | 0 | 1 |

V1's log ends `M7: post-confirmation snapshot created`, then nothing until the
timeout kills QEMU: `init` is stopped at its first publish and never reaches
`driver-uart`. V2 shows `[task#9 proxy-text]: fault(LoadPageFault)` at line 224
and `init` stops at its next publish. The register's account is right, in both
variants.

**One process failure to record, because it touched the working tree.** I ran
V2's two edits in the *main* checkout instead of the scratch worktree (a `cd`
from an earlier call had not carried over). Nothing was committed;
`git status` showed exactly the two touched files (`fjell-proxy-text/src/main.rs`
and its rebuilt prebuilt), both wholly the experiment; I restored those two
paths, moved the misplaced logs into `target/scratch/`, rebuilt, and the tree
was clean again (`git status --short` empty, prebuilts regenerated
byte-identically). The V2 numbers above come from that run, which is valid — it
is the same commit plus the same one-line fault.

---

## What D8 needs, and what the kernel gives (measured, from source)

D8 says a publisher's reply may not wait on any presentation. The obvious
change — reply first, forward after — only moves the stall by one publish: the
stream is then blocked in the forward, and the *next* call to the stream queues
behind it. So the requirement is that **the stream never blocks on a
presentation at all**. What the kernel offers for that:

1. **There is no non-blocking send.** `sys_ipc_send` and `sys_ipc_call` both, when
   the endpoint has no receiver waiting, queue the message and `block(tasks,
   sched, cur_id)` the *caller* (`crates/fjell-kernel/src/cap/syscall.rs:540-541`
   and `:667-670`). The one-way form blocks too; it only skips the wait for a
   *reply*. `sys_ipc_try_recv` exists (RFC 019); there is no `try_send`, and no
   timeout on anything (the timer interrupt has never been enabled, E-044).
2. **A server can hold one caller's reply, not several.** The reply edge is
   `replies: [ReplySlot; MAX_TASKS]`, one `Option<ReplyEdge>` **per server
   task** (`crates/fjell-kernel/src/cap/table.rs:70-113`), and `set_reply`
   overwrites it. A stream that parked a presentation's call and then received a
   publisher's would lose the first caller for good. So "the presentation calls
   and waits for the stream to reply when there is something" cannot work.
3. **A dead receiver never releases a blocked sender.** A task that faults leaves
   callers blocked on it (E-058 V2, measured). Only lease revocation wakes blocked
   IPC (`cancel_blocked_ipc_for_lease`).

So a stream that *pushes* to a presentation with `send` or `call` is one fault
away from a stall, whatever order it does things in. **A change to the kernel
(a non-blocking send, or a send timeout) is the other way to satisfy D8; I have
not made one, because a new syscall is not a spawn-table entry and the RFC's
"touches" list does not name it.** If you would rather have that, it replaces
the mechanism below and everything else in this document stands.

### Design — the stream never initiates a blocking IPC to a presentation

The only IPC the kernel never blocks the sender on is a **reply**. So the stream
only ever *replies* to a presentation, and a presentation *asks*:

- A presentation runs `loop { m = ipc_call(STREAM, PRESENT_NEXT); … }`. The stream
  **replies immediately** to every `PRESENT_NEXT`, in one of two ways:
  - with the **next message** of the oldest queued envelope — a `RENDER_BEGIN`,
    `RENDER_CHUNK` or `RENDER_COMMIT` (the tags `proxy-text` already speaks), whose
    reply words are the same four words a chunk carries today. `sys_ipc_reply`
    copies four words unconditionally (`syscall.rs`, the loop over `12 + i`), which is
    exactly a chunk;
  - or with `PRESENT_EMPTY`, meaning "nothing queued; you are now parked".
- A parked presentation blocks in `recv` on **its own endpoint**. When an envelope
  arrives for a parked presentation, the stream sends it one **wake** (a tag, no
  payload) with a one-way send. This is the single place the stream can block on a
  presentation, and the window is **the presentation's own two-instruction glue**
  between the reply that told it "parked" and its `recv`: a send to a task that is
  alive and about to receive completes as soon as it does. It is a residual, not
  a cure (below).
- **Who is asking is not asserted.** The stream keys its per-presentation state
  by the kernel-attested sender identity (`ipc_sender_image_id`) of the
  `PRESENT_NEXT` call, the identity `sys_ipc_recv_msg` already returns and this
  loop already discards. It does not trust a "which presentation am I" word
  (E-059 is exactly that mistake in another leg).
- The relay/presentation **never calls the stream while the stream calls it**,
  because the stream never calls it. That also removes a latent deadlock I found
  reading the loop: today the stream can `forward` to `proxy-text` while
  `proxy-text` is blocked in its own `DISPATCH_ACTION` call to the stream, and
  neither can proceed. It needs two publishers in flight to occur and I have not
  triggered it; the new topology cannot have it.

**The relay.** `proxy-text` must stay unmodified (D5), and its protocol is
blocking (it replies to each message, and renders *before* replying to
`COMMIT`), so it cannot be the party that asks. A small **`fjell-proxy-relay`**
service asks on its behalf: `PRESENT_NEXT`, and for each message a **blocking
`ipc_call` to `proxy-text`** with the same tag and words, then asks again. If
`proxy-text` is absent or dead the relay blocks forever — and the stream, which
sees only "this presentation stopped asking", is unaffected. The blocking has
moved into a task nobody else waits on. The new braille presentation is its own
asker, natively.

The cost is one more image, endpoint and prebuilt than the RFC counted (29 → 31,
not 30), and `proxy-text`'s **source** is untouched — which is stronger D5
evidence than "changed a little": `git diff` over `crates/services/fjell-proxy-text/`
must be empty at the end. (Its prebuilt bytes may still shift when a shared crate
it depends on changes; that is a build-input effect, documented since 0.33, not a
source change.)

### Design — the undeliverable-envelope policy (§2 of the handoff), both halves

**What the publisher is told.** `PUBLISH_OK` when the envelope decoded and
validated — meaning *"published"*. It is **not** told whether any presentation
rendered it, and cannot be: rendering is not the publisher's business (the
handoff's first half). `PUBLISH_ERR` remains for an envelope that did not decode.

**What happens to an envelope a presentation cannot take right now: queued with a
bound, then dropped.** Per presentation, a **byte ring of 8 KiB** holds
length-prefixed encoded envelopes in arrival order. If a new envelope does not
fit, **it is dropped** (drop-newest: it keeps order, never disturbs an envelope
partly sent, and needs no second pass) and counted. Why a queue at all, when the
handoff says dropping a rendering is acceptable: a presentation is busy, not
gone, for most of its life — it prints and dispatches actions between its
questions, while publishers keep publishing — so dropping whenever it is busy
would lose renderings in **normal** operation and break the three tiers whose
markers depend on them. The queue is what makes "busy" and "gone" look
different. Why **8 KiB**: the widest envelope the wire format can produce is
`MAX_WIRE_BYTES` — **4,624 bytes**, as the crate's test measured it; I first wrote
"a little over 4 KiB" and the test's first run, which asserted room for *two*, is
what corrected me — so 8 KiB holds one worst case with room to spare but **not two**,
and typically dozens of ordinary ones (the sample intent encodes in a few hundred
bytes — to be confirmed by the tier); two rings cost 16 KiB of the service's 64 KiB stack (16 pages,
`spawn.rs`), which I will measure rather than assume. A queue with no bound is
the handoff's "third failure mode"; this one has exactly one number, in one
constant, with a test that overflows it.

**Observable absence.** The stream prints, on its own output and in a fixed
shape: `semantic-stream: presentation <name> not taking envelopes; N dropped` at
the **first** drop and at each **power of two** after (1, 2, 4, 8 …), and
`semantic-stream: presentation <name> resumed; N dropped in total` when it asks
again after having dropped. A presentation that has *never* asked is reported
the same way from its first drop, with `never asked` in place of the resumed
form. Bounded output for an unbounded fault: about `log2(N)` lines.

**What this does not cure (the survivors D8/R9 asks me to name).**

- **A presentation that is alive but never asks again** keeps its queue full and
  drops forever. That is correct behaviour and it is observable; it is not
  detected as "dead" (there is no timer to say how long is too long).
- **The wake window.** A presentation that faults in the two instructions
  between being told "parked" and its `recv` would still block the stream on the
  next wake. The relay and the braille service are the code in that window and
  are written to have nothing in it that can fault; but it is a window, not a
  proof.
- **Head-of-line.** An envelope half-sent to a presentation that then stops
  asking stays at the head of *its own* queue. Other presentations have their own.

---

## §A — Which modality? **Grade-1 braille, as Unicode braille patterns.**

I take the RFC's lean, and for the reason it gives that matters most: it differs
from `proxy-text` in **order and omission**, which is the part of "only the
proxy differs" worth testing, and it is a table plus a width rule, so its output
is checkable byte for byte with no device. An announcement stream for a
synthesiser is defensible, and the RFC is right that it is easier to get subtly
wrong: there is no ground truth for "what a screen reader would say", so a test
could only compare a string I made up with a string I made up.

**What makes the braille output checkable** (the handoff asks): (1) the renderer
is a pure function, bytes out; (2) its expected outputs are **committed vectors
whose cells I derive by hand from the published dot patterns of the
letters**, not by running the renderer and pasting what it printed — otherwise
the test proves the code agrees with itself; (3) properties that need no
vector (every output cell lies in U+2800–U+28FF; no line exceeds the width;
output is a function of the envelope alone); (4) the QEMU tier asserts a
**committed line** from the real run, not a marker that says "rendered".

**The rules — deliberately small, and not a braille standard.** This is an
**uncontracted, grade-1-style rendering with documented simplifications**; it is
not a conforming implementation of UEB or any national code, and no page may say
it is (D7). The RFC's risk — *"if the answer becomes 'implement a braille
standard', the line has escaped"* — is why the table is this short.

- **Cells:** Unicode braille patterns, six-dot cells only (U+2800–U+283F); a
  space is the blank cell U+2800, so widths are cell counts.
- **Letters** `a–z` by the standard patterns; a **capital** is the dot-6 cell
  (U+2820) before the letter, one per capital (a simplification: UEB uses a
  capitals-word indicator for runs).
- **Digits:** the number sign (dots 3456, U+283C) once at the start of a digit
  run, then `a–j` patterns for `1–9,0`; a `.` or `,` **between digits** stays in
  the run.
- **Punctuation** — a short fixed set: `. , ; : ! ? ' - / ( )` and a few
  others, each a fixed pattern.
- **Anything else** (any byte outside that set, including all non-ASCII) is the
  **eight-dot full cell U+28FF**, which no six-dot rule produces — so an
  unrepresentable character is visible and cannot be mistaken for a real cell.
- **Width:** 40 cells, wrapped at spaces; a word longer than a line is broken
  at the width. The width is a parameter of the pure function, 40 is the
  service's choice, and no claim is made about any device's width.

**The structure — where it differs from `proxy-text`.**

- **Severity comes first**, spelled as a word and a colon (`critical:`,
  `important:`, `normal:`, `low:`) before the title; `proxy-text` prints its
  bracketed tags first and the severity in the middle of them. For states, the
  status word comes first the same way; for events, severity then result.
- **Actions are numbered and listed after the description**, one per line, as the
  number then the label; `proxy-text` prints them in the same order but with
  its own bracket layout.
- **Omitted, on purpose, and to be stated in the documents:** each action's
  required capability, reversibility and confirmation policy; `expires_at_tick`;
  a state's facts beyond their count; an event's audit sequence and subject. A
  presentation for a person deciding whether to act would want reversibility
  and confirmation — **there is no input path (D6), so nothing can be acted on
  through this presentation, and the omission is recorded as a limitation, not
  filled in.**
- **Where it stops:** a fixed cap on lines per envelope is not needed — the
  widest envelope is bounded by the format — but the renderer reports whether
  it emitted everything (`Complete`) or ran out of room in the caller's buffer
  (`Truncated`), and the service says so on its output rather than silently
  showing a prefix.

**What may be claimed** (D7): the deliverable is *the stream a braille display
driver would consume*, written to the serial console. QEMU `virt` has no braille
display, and none was driven (E-004). No page, comment, marker or commit message
will say Fjell speaks, supports braille, or is accessible.

## §B — How does the second proxy get its envelope? **Its own endpoint; nothing shared.**

`semantic-stream` never pushes; each presentation **asks the stream** and is
answered by reply, and is **woken** through an endpoint that is its own. The
relay and the braille service each get: an image id (`0x1F`, `0x20`), their own
endpoint object (`13`, `14`, after `BOOTCTL_EP_OBJECT = 12`), a `CALL` capability
to the stream's endpoint (7), and — on the stream's side — a `SEND` capability
to each of those two endpoints (the wake). `CapInstall` is undispatched, so all
of it is installed by `spawn.rs`. Shared-endpoint receiving is refused in advance
(RFC-0.28-001).

**The trap, and how it is closed rather than warned about.** Endpoint objects are
allocated by `et.alloc()` calls whose returned ids are thrown away
(`let _ = x_ep_id; // id=12`), so the constant a spawn table names and the
allocation that makes it exist are related **by a comment and the order of
statements**. RFC-0.33-001 lost a day to a missing call with the warning two
lines above it. I will (1) bind each allocation to its named constant with an
`assert_eq!` so a mismatch or omission stops boot in every tier, and (2) add a
host test that counts `et.alloc()` calls in `main.rs` against a single
`ENDPOINT_OBJECT_COUNT` in `fjell-abi` and that every `*_EP_OBJECT` constant is
below it. Whether a test that reads `main.rs` as text is too clever is a
judgement I would like the architect to see; the assertion in (1) is the part
that cannot be argued with, because it runs at every boot.

## §C — What does the tier assert? **Content, from committed vectors.**

Two things, in one QEMU run, from **one** envelope: the sample service's demo
intent (`title = "sample-service demo intent"`, `severity = Normal`, two actions).
`proxy-text` prints `[INTENT][Normal] sample-service demo intent` (the existing
marker, unchanged); the braille presentation prints its committed braille line
for the same title with the severity word first. Both are `expected_markers`
in the profile. The braille line is the **committed vector** from the renderer
crate's test, character for character — so the same file is the source of the
host test and of the QEMU assertion, and drift between them fails one or both.
The markers contain no `,` or `]` (E-057), which the braille cells and the
chosen words do not.

## §D — Where does the renderer live? **In a crate the service wraps.**

`crates/fjell-braille` — `no_std`, no syscalls, no allocation, in `members` and
`default-members` beside `fjell-bootctl-model`, so its vectors run in Gate 1.
The service (`fjell-proxy-braille`) is the adapter: receive, reassemble with
the existing `chunked::Reassembler`, decode with the existing
`wire::decode_exact` (D1 — there is no second decoder and there will not be),
call the crate, print each line. The **queue and credit engine** is the same
shape for the same reason: `crates/fjell-semantic-fanout`, pure, host-tested,
wrapped by the stream. `proxy-text`'s own renderer writes through syscalls and
cannot be tested on the host; the new crates must not repeat that.

## §E — What does the second modality do about `TextToken`? **It uses the fallback text only, and says so.**

A `TextToken` is an id plus fallback text. No catalogue exists, so the id has no
referent: **the braille renderer ignores it.** What this means for the claim
that *meaning, not text, crosses the boundary*: it is true of everything
**around** the text — kind, severity, action set, required capability — which the
renderer really does re-order and re-present, and **false of the text itself**,
which crosses as English fallback strings in both modalities. A presentation
cannot localise, simplify or substitute by id today, and a person who needs
simplified language gets the producer's fallback sentence in braille cells.
That is a stronger limit on "only the proxy differs" than the abdd page states,
and `abdd-semantic.md` and `v1-limitations.md` will say it.

---

## Order of work

`R1` (this document) → **D8**: the fan-out engine crate with its host tests, the
stream and relay, the spawn/endpoint additions with the allocation assertion,
the machine-configuration switch that makes a presentation absent, and a tier
that runs with `proxy-text` absent and shows the node's narration continuing →
**D4**: the braille crate and vectors, Gate 1 → the braille service and the tier
asserting both renderings of one envelope → R6/R7 documents → R9 (E-058, register
and `v1-limitations.md` in one commit) → evidence and the review request.

## Decisions I am making that nothing above the line specified

1. **The D8 mechanism** (ask-and-wake; no kernel change) and the **relay**, above.
2. **8 KiB byte ring, drop-newest, counted, printed at powers of two.**
3. **The "absent presentation" switch** for the tier is a test affordance in the
   shipped image, the same class as D10's console byte and D15's `Reboot`
   capability, and it will be disclosed in `v1-limitations.md` beside them. I
   propose: `init` reads the machine's virtio device list (it already holds the
   MMIO region capabilities, slots 31–34, and uses none) and does not spawn
   `proxy-text` when a device the profile adds, and no other profile adds, is
   present. That needs **no new kernel grant**. It is the same trigger shape as
   D15's, and it puts one more `read_volatile` site with its `MMIO-ORDER` tag in
   `init`. Alternatives I weighed and rejected are in the review request.
4. **The braille rules** as written — including the simplifications listed.

I will not touch `proxy-text`'s source, ADR-v0.5-005, `BoardProfile`, any audio
or display device, or `v1-limitations.md` beyond what R6/R9 direct.
