# RFC-0.34-004 R1, §A–§E, and the live failing case

**Governing RFC:** [../accepted/RFC-0.34-004-a-word-that-claims-an-authority.md](../accepted/RFC-0.34-004-a-word-that-claims-an-authority.md)
**Handoff:** [../handoffs/RFC-0.34-004-a-word-that-claims-an-authority/implementation-handoff.md](../handoffs/RFC-0.34-004-a-word-that-claims-an-authority/implementation-handoff.md)

Written after R1 and the live failing transcript, before the mechanism. Every
absence is probed with `/usr/bin/grep -a` and a control. This document
contains **measurements** (marked as such) and **design** (marked as such).
An Afterword, added after implementation, says what survived contact.

---

## R1 — Findings 1–4, re-derived at `cc658e1`

| Finding | RFC's claim | What the tree says |
|---|---|---|
| 1 | Comment at `main.rs:60`–`:61` claims kernel-verified, not self-asserted | **Confirmed**, `crates/services/fjell-semantic-stream/src/main.rs:60`. `granted_rights = w2 as u32` at `:441`; `allowed = required & !granted_rights == 0` at `:78`. Control: `sys_cap_inspect` appears **once** in this file (inside the comment's own prose) — `grep -c` confirms it is never called here, against a control count of **4** call sites in `fjell-proxy-text/src/main.rs`, where the methodology does find real calls. |
| 2 | The stream already holds the kernel-attested sender and does not use it for this decision | **Confirmed.** `presentation_of` (`:269`–`:270`) resolves `ipc_sender_image_id(sender)` and keys the fan-out engine by it; `dispatch_action_checked` (`:62`) is never given `sender`, only the payload word. |
| 3 | A permitted action executes nothing | **Confirmed.** `dispatch_action_checked`'s body issues zero syscalls (grep over its 30-line span finds none); it only builds an `ActionResult` message. |
| 4 (E-034) | Four helpers take `w0` and never send it | **Confirmed**, all four signatures unchanged: `fjell-attestd::sxt_send(tag: u16, w0: usize)` (`:309`), `fjell-diagnosticsd::send_tag(ep, tag: usize, w0: usize)` (`:55`), `fjell-secure-transportd::send_tag(ep, tag: u16, w0: usize)` (`:63`), `fjell-upgraded::send_sxt(tag: u16, w0: usize)` (`:159`). Kernel's `build_msg` (`cap/syscall.rs:321`) reads the word count from `(raw >> 16) & 0xFF`; a bare `sys_ipc_send(ep, tag)` (`fjell-syscall/src/lib.rs:345`) packs no count, so the copy loop runs zero times. Control: `sys_ipc_send_words` (`fjell-syscall/src/lib.rs:366`) **does** pack a count and **does** carry two words — confirming the methodology would find a carrying wrapper if one were used here, and none of the four use it. |

## The live failing transcript (D0.1, taken before any change)

Scratch worktree `target/scratch/rfc-0.34-004-live-fail` (`git worktree add
--detach`, `cc658e1`), one edit: `fjell-proxy-text`'s `dispatch_action`
replaced its `sys_cap_inspect` read with a hardcoded `0xFFFF_FFFF` — a claim
of every right there is, where the slot's real rights are `SEND | REPLY |
INSPECT` only (`spawn.rs:849`–`:851`). Built, run against the **unmodified**
`semantic-stream` (`cargo xtask qemu-run --profile semantic`):

| | honest (control, same tip) | forged |
|---|---|---|
| `action accepted` | 5 | **6** |
| `action DENIED (capability not held)` | 1 | **0** |

Sample-service's action 2 requires `MMIO_MAP` (`fjell-sample-service/src/main.rs:70`),
a right `DEMO_CAP_SLOT` does not and cannot hold under honest inspection —
yet with the forged word, **every** call to `dispatch_action_checked` for it
is accepted. **This is D1's test, answered "yes" today**: a presentation
can obtain a permission it does not hold by sending different bytes. The
honest run (5/1) is the control, taken on the same tip immediately
afterward, confirming the methodology — the forgery, not drift, produced the
6/0 result. Full transcript:
`target/scratch/rfc-0.34-004-live-fail-evidence/before-fix-forged-rights-accepted.log`
(not committed — scratch, discarded after this document and the review
request cite it; the "refused" half of D5 is a committed tier instead, below).

---

## §B — Can a receiver ask the kernel about a sender's rights at all today? **No.**

Verified with a control. `sys_cap_inspect`'s kernel side
(`crates/fjell-kernel/src/cap/syscall.rs:185`–`:222`) is dispatched with
`tidx = sched.current()` (`trap/syscall.rs:383`–`:384`) — **the caller's own
task index, never one the caller supplies** — and resolves `ct.cspace(tidx)`
with a local `CapHandle` slot number, nothing that names another task.
`CSpace` is strictly per-task (`cap/table.rs:63`–`:73`), indexed nowhere else
by anything but the current task. The only two syscalls that take a
cross-task reference at all: `sys_cap_install` **writes** a fresh capability
into another task's CSpace (it does not read an existing one), and
`sys_task_status` returns only a `TaskLifecycle` enum (Created / Runnable /
Running / Blocked / Exited / Faulted) — no rights data exists on that return
path. `ipc_sender_image_id` is identity, not rights: it decodes a word the
kernel wrote at delivery time from the sender's own task table entry
(`cap/syscall.rs:341`–`:347`), telling a receiver *who* sent a message, never
*what it holds*. **Control:** the same reading of `sys_cap_inspect` confirms
it resolves only the caller's own slot — the self-only case the RFC itself
describes — so the search methodology is finding what it should.

**Consequence:** §A's capability route would need a new syscall (cross-task
rights query), which is an ABI change, Gate 4, and its own line per the
handoff's own instruction. Not pursued here. §A is therefore the policy
route, not the capability route — not a preference, a constraint.

## §A — Attested identity, plus a policy derived from the presentation's own compiled row

The stream already compiles `PRESENTATIONS: [PresentationSlot; 3]` — each row
carrying `image_id` and `kind`. I add one field, **`dispatch_rights: u32`**,
to that same row, populated at the same place image_id and kind are:

- `proxy-text`'s row: `fjell_cap::CapRights::PROXY_TEXT_DEMO.0` — **a new
  named constant in `fjell-cap`**, `SEND | REPLY | INSPECT`, the exact bits
  `spawn.rs` installs at CSpace slot 2 (`spawn.rs:849`–`:851`). `spawn.rs`'s
  own `install_raw` call is changed to reference the same constant, so there
  is **one** declaration of the bit pattern, not two independently
  hand-maintained ones that could drift — the kernel's grant and the
  stream's policy read the same symbol. This is what "derive it from what is
  already there" means concretely: the row the stream already compiles in,
  extended by one field, pointing at the one place the kernel actually
  installed the rights it is now used to check against.
- `proxy-braille`'s and `fault-test`'s rows: **`0`**. Neither has any
  capability installed beyond the stream endpoint (`spawn.rs:792`–`:808`
  installs only slot 1, the `CALL` cap, for both); neither has ever called
  `DISPATCH_ACTION`. `0` is the honest value: a presentation holding no
  capability for this can satisfy no non-empty requirement, and any action
  whose `required_capability` is `None` (`required = 0`) still passes for
  every presentation, exactly as today — this change does not newly deny
  anything a real capability would have allowed.

`dispatch_action_checked` is given `granted_rights` from
`PRESENTATIONS[presentation_of(sender)].dispatch_rights`, never from `w2`.
**`w2` is no longer read anywhere on this path.** This is D1 directly: the
sender cannot influence the decision by sending different bytes, because
nothing it sends is consulted — the decision is a lookup keyed by the
kernel-attested sender identity already in `presentation_of`, against a
policy the stream owns and shares its one source with the kernel's own
grant.

**Why this is not E-014's family (a second hand-maintained table).** The
alternative — a bitmask literal hand-copied into `PRESENTATIONS` and another
hand-copied into `spawn.rs` — would be exactly that risk. Routing both
through `CapRights::PROXY_TEXT_DEMO` removes it: editing the constant changes
what the kernel installs and what the stream checks in the same place, in
the same commit, by construction. The coupling is named in a comment at both
use sites.

## §C — E-034: drop the parameter — and two findings that change why

Investigated with a control (two independent passes: one over the four
helpers and every call site, one specifically asking whether any *receiver*
reads the dropped word): **most call sites pass values that look meaningful
in isolation** (`channel_id`, `record_seq`, `MAX_QUERY_RECORDS`, `SxtError`
variants) — contradicting a first guess that `w0` is always a dummy. Checked
against receivers, not assumed:

- **Three of the four categories are genuinely inert.** `diagnosticsd`'s
  `AUDIT_QUERY`/`max_count`: `fjell-auditd` never implements this protocol
  over IPC at all — it drains the audit ring unconditionally on a bare
  `sys_ipc_recv`, ignoring tag and words. `attestd`'s `record_seq`
  (`SxtTag::AttestPush`): `secure-transportd`'s dispatch has no arm for it.
  `SxtError` payloads: every receiver branches on the reply **tag**, never a
  payload word — `SXT_FAULTED` is `#[allow(dead_code)]` in two of the three.
  Dropping `w0` for these loses nothing a receiver was ever going to read.
- **One is not: `upgraded`'s `channel_id` for `UpdateMetadataFetch`/`Close`.**
  `main.rs:181`/`:206` genuinely reads `w0 as u32` and looks it up in
  `ChannelTable`; because it always arrives `0` (never carried), every fetch
  and close silently fails closed (`Faulted`/`ChannelClosed` instead of the
  real reply) and a `Close` on channel `0` matches nothing, leaking the
  slot. **This is a live, independent defect**, already true before this
  line and unaffected by what this line does to the signature — a payload
  this caller needs delivered, per §C's own test. **Per §C's instruction,
  carrying it is an ABI change to the tag packing and is not done here.**
  Filed as its own finding, below, not fixed.
- **Found in passing, named, not touched:** `secure-transportd`'s receive
  loop polls only `CAP_UPGRAD_EP`, never `CAP_DIAG_EP`/`CAP_ATTEST_EP`
  despite the module's own doc claiming round-robin polling of three
  endpoints — so `attestd`'s and `diagnosticsd`'s `OpenChannel`/`DiagPush`/
  `AttestPush` are unreachable by any receiver today, independent of `w0`.
  Far outside this RFC's *Touches* list; named for escalation, not fixed.

**Decision:** drop `w0` from all four signatures and every call site.
Dropping it changes no runtime behaviour — the word was never delivered —
it only stops four signatures from promising something none of them do.
The `upgraded` channel-lookup defect and the secure-transportd polling gap
are **new findings**, not covered by E-034's own claim ("nothing is broken
today" — one of the four categories now shown to be false), and are named in
the register as survivors rather than repaired here, per the handoff's own
scope line.

## §D — The refusal is reported on the stream's own output, asserted by a tier

Today only `proxy-text` prints its own local outcome
(`"proxy-text: action accepted"` / `"... DENIED ..."`) — the stream itself
says nothing about a dispatch decision. I add one line to
`dispatch_action_checked`'s call site in `service_main`, on the stream's own
output: `"semantic-stream: dispatch <presentation> action <id> -> accepted" `
/ `"... refused"`, naming the presentation by its row's `name` (the same
field `say()` already prints for fan-out reports) — so the decision is
observable at the point that makes it, not only inferred from what the
caller chose to print about its own call. The permanent tier is a new
test-only presentation, `svc-presentation-liar`, below (R6).

## §E — Order: E-059 first, then E-034

As the RFC leans: the mechanism is the line; E-034 is independent, smaller,
and does not block it. R3→R4 (E-059) before R5 (E-034).

---

## Order of work

R1 + live failing transcript (above, done) → §A–§E (above) → R3 (the
mechanism: `dispatch_rights` field, `CapRights::PROXY_TEXT_DEMO`, `spawn.rs`
referencing the same constant) → R4 (the comment, and the sweep) → R6 (the
refused transcript: `svc-presentation-liar`, its tier) → R5 (E-034: drop the
parameter, four crates) → R7 (errata, `v1-limitations.md`) → evidence.

## Decisions I am making that nothing above the line specified

1. The field name `dispatch_rights` and its placement directly on
   `PresentationSlot` (not a parallel map).
2. The constant's name and home: `CapRights::PROXY_TEXT_DEMO` in `fjell-cap`
   (the crate that already owns every other `CapRights` constant; both the
   kernel and `fjell-semantic-stream` already sit downstream of it through
   existing dependencies).
3. The stream's new output line's exact wording (not specified by the
   handoff beyond "reported").
4. `svc-presentation-liar`'s shape: a new test-only service mirroring
   `svc-presentation-fault`'s minimalism, started by its own profile, not a
   branch added to an existing shipped presentation.

I will not touch `fjell-proxy-text`'s or `fjell-proxy-braille`'s source
(the mechanism change is entirely receiver-side), `fjell-auditd`'s missing
`AUDIT_QUERY` implementation, or `secure-transportd`'s endpoint-polling gap.

---
