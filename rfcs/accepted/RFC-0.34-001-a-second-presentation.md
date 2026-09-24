# RFC-0.34-001: A second presentation

**Status:** Accepted — by the owner (nabbisen), 2026-09-24; implementation may begin (RFC 000)
**Milestone:** 0.34
**Delivers.** The first of RFC-0.33-002 **D9**'s three v1.0 release criteria — *a
second presentation modality, end to end* — and with it the first test of the
claim the architecture has rested on since v0.5: **"a screen, a screen reader and
an assistive personal device run the same core; only the proxy differs."**
**Touches** *(indicative)*: a new service crate, `fjell-abi`'s image ids,
`fjell-kernel`'s spawn table (an image, an endpoint, capabilities),
`fjell-semantic-stream` (fan-out), `tests/qemu/profiles/`, the prebuilt set and
the repro baseline, `docs/src/external-design/abdd-semantic.md`.
**Relates to:** ADR-0005 (semantic-stream-first), ADR-v0.5-005 (the proxy is
output-only — **unchanged here**), E-049 (which removes the CI package lists this
line would otherwise have to extend).

## Summary

### Finding 1 — the claim has never been tested, and it is load-bearing

`fjell-proxy-text` is the only proxy: **211 lines in `main.rs`, 1,069 in the
crate** (`renderer.rs` is 540 of them — *figure corrected at RFC-0.33-002's
review, which re-derived it; the point stands and the renderer is larger than the
service*), rendering to the serial console and dispatching the actions an intent
offers. Every statement the project
makes about inclusion — in the requirements, in ADR-0005, in
`external-design/abdd-semantic.md` — rests on *one* renderer of the intent
stream. **One implementation of an interface is a design intention, not a
demonstrated boundary.**

### Finding 2 — there is real structure to render, not just text

An `IntentNode` carries `kind` (Information … ActionRequest), `title` and
`description` as `TextToken`s (an id plus fallback text), **`severity`**,
`actions: FixedVec<ActionSpec, MAX_ACTIONS>` — each with the capability it would
require — and `consequences`. A second modality is therefore not a font change:
it must decide **order, emphasis, and what to omit**, from the same bytes.

### Finding 3 — what the platform can and cannot show

QEMU `virt` has **no audio device** and no braille display. So a *speech* proxy
could not speak, and a braille proxy cannot drive hardware cells. What either can
do honestly is **emit the stream a synthesiser or a display driver would
consume**, to the serial console, where a tier can read it byte for byte.
**This RFC will not call that "speech" or "braille output" without qualification**
— the deliverable is a presentation, and the hardware is E-004's business.

### Finding 4 — the cost is structural, and mostly not the renderer

A second proxy needs: a workspace member; an `ImageId` (the last assigned is
`0x1D`); a spawn-table entry with a CSpace size; an endpoint object **allocated
in the kernel's table** — the omission that cost RFC-0.33-001 a live defect; a
send capability for `semantic-stream`; a prebuilt binary (29 → 30) and a
re-recorded repro baseline; and, until E-049 lands, an entry in CI's hand-written
package lists.

## The settled part

**D1 — One decoder, two renderers.** The new proxy consumes the same wire format
through the same `fjell_semantic_format::wire::decode`. **If any decoding logic is
duplicated, the line has failed its own point.**

**D2 — The stream does not know how many proxies exist.** `semantic-stream`
forwards to each proxy it holds a capability for, and adding the second changes
no decoding, no validation, and no envelope content.

**D3 — The same envelope, two renderings, in one QEMU run**, both asserted by
markers, so the demonstration is that *one* message became two presentations —
not that a second service exists.

**D4 — The renderer is a pure function of the envelope**, testable on the host
without QEMU: envelope in, bytes out, with table-driven vectors. A modality whose
output can only be checked by eye is not checkable.

**D5 — `proxy-text` is not modified beyond what fan-out requires.** Its markers
are load-bearing in three tiers.

**D8 — No presentation may stall a publisher** (**E-058**, filed at
RFC-0.33-002's review). `semantic-stream` forwards to the proxy with a blocking
call *before* replying to the publisher, so today an absent or faulting
presentation stops the emitter — measured at 313 → 125 output lines with
`proxy-text` never started. **A second proxy multiplies it**, which is why the
line that adds one removes the coupling: a publisher's reply may not wait on any
presentation, and an unavailable or faulting proxy must be observable without
stalling anything. **This is a requirement of this line, not a bonus** — and its
demonstration is a tier that runs with one presentation deliberately absent and
shows the node's narration continuing.

**D6 — ADR-v0.5-005 stands: output only.** The input path is RFC-0.33-002 §C's
open question and is **not** opened here.

**D7 — The deliverable is named for what it is.** A stream a braille display or a
synthesiser would consume — never "Fjell supports braille" or "Fjell speaks".
The limitations section says which hardware does not exist.

## Settled at the review, 2026-09-24

**D9 — the relay goes, and `proxy-text` speaks the ask protocol. D5 was the wrong
instruction and this corrects it.** `proxy-relay` exists to hold the blocking call
`proxy-text` has always answered, away from the stream. That is a faithful reading
of D5 — *"`proxy-text` is not modified beyond what fan-out requires"* — and D5
bought a permanent shim, an extra image, an extra endpoint and a thirty-first
prebuilt to avoid roughly ten lines in one service. **A workaround kept in the
architecture is not what "clean" means.** Convert `proxy-text` to ask, delete the
relay, and let the tiers that assert its markers be the check that its behaviour
did not change. *(The parked-to-`recv` window survives in whichever task asks —
removing the relay does not fix it, and it stays a named survivor.)*

**D10 — the absence switch stays, on severity grounds, and the contrast with the
reset trigger is the rule.** A balloon device makes `init` skip `proxy-text`, and
`init` **says so on the console** — *"presentation-absent test configuration;
proxy-text not started"*. Worst case on a real node: the text presentation does
not start, the line says why, and the stream now reports the absence. Visible,
recoverable, self-explaining. Contrast D17's refused trigger, whose worst case is
an unbreakable reboot loop and which is silent by construction. **Severity and
recurrence decide whether a hardware-presence switch is acceptable, not
convenience.**

**D11 — the mid-run crash tier is required.** It is the case that found the
counting defect in your own absence report, and it is the case you trust least;
leaving it to a scratch build leaves the weakest evidence uncommitted.
**Constraint: no new device-presence trigger.** Use the console-byte channel; a
small test-only service that registers as a presentation and then faults is
acceptable, as `svc-fault` already is.

**D12 — `MAX_TASKS` 40 is accepted**, with its measured `.bss` cost, and the
debug-buffer fix that had to go with it. But **a full table reporting `NoMemory`
is a defect in its own right**: three different allocation failures in `spawn.rs`
return the same value, so `init` cannot say which limit it hit, and the symptom
was a bare `init: spawn error`. Filed as **E-061**. The table's size is not worth
its own decision; the error that hides the reason is.

**D13 — the readiness row is amended, not re-graded.** DONE is right: a second
modality is driven end to end from the same stream and asserted by content. The
row must say **what** was observed — braille cells on a serial console, read by no
braille reader and driven on no braille device — because a reader of that row is
being told a v1.0 criterion is met.

**D14 — the garbled line you reported is real, and it is worse than a cosmetic
one. Located, mechanism named, filed.** It is in
`tests/qemu/artifacts/*/serial.log` in **every** profile, and in archived runs
back to 2026-09-02: eight non-printing bytes (`90 90 90 90 90 90 90 92`)
immediately after `devmgr: profiles verified`. The kernel's per-task console
buffer flushes on a newline or at 160 bytes and **never when a task leaves**, so
a task that exits mid-line has its bytes emitted in front of the next line from
that slot — filed as **E-062**, together with the silent 160-byte split, which
matters here because a braille line is already 136 bytes at the sizes you tested.
And the line that follows it, `M6: storaged ready`, is printed by **`storaged`
and `init` both** — so the tiers asserting it pass on `init`'s line alone. Filed
as **E-063**. You were right to report it rather than patch it; closing E-062
needs the failing case shown, a task exiting mid-line and the leftover appearing
on the next task's, not the bytes merely going away.

**Accepted as delivered:** the queue-and-credit policy with its bound argued from
`MAX_WIRE_BYTES`, the logical clock that replaced the offer count, both endpoint
tests (the boot-time assertion is the real one; the text scan is a cheap early
warning — keep both), the three kernel touches you flagged for veto, the task
labels by `ImageId`, and every figure you re-derived against mine.

## Settled at the second review, 2026-09-24

**D15 — the dormant row is the right mechanism and the wrong encoding.** Keep it.
It records a real distinction that the alternative erases: a presentation the
image **expects** must accumulate and be reported when it never starts (that is
E-058's whole point), while one only a test starts must cost the other profiles
nothing. Your alternative — a queue on first ask for everybody — would make a
late-starting production presentation lose what was published before it, which
D2 is there to prevent. Nothing about the flag is awkward; it is the fact.

**What is refused is `const DORMANT: [bool; PRESENTATIONS.len()] = [false, false,
true]`** — a hand-maintained array parallel to `PRESENTATIONS`, where inserting a
row silently shifts every flag onto the wrong presentation. That is E-014's family
in a data structure. **Put the flag on the row**, named for which kind of
presentation it is rather than for what the engine does with it, and derive
dormancy from it; then add the assertion that the production rows are not marked.
One field, one constructor change, and the two facts can no longer drift apart.

**D16 — the readiness row is right as updated.** The run id now names the run at
the final code and nothing else changed. No further edit.

**D17 — E-061 to E-064 need a line, and it is not this one.** All four are
tracked 0.34 and none belongs to a presentation: a spawn failure that cannot say
which limit it hit; a console buffer that prefixes a dead task's bytes onto a live
one's line and splits long lines silently; a marker two tasks print; and a boot
shim that destroys the DTB pointer. They are one subject — **what the machine
tells a person, and whether it is true** — and they deserve one RFC rather than
being appended to a presentation line. Scoped, not started: the owner decides.

**Accepted at this review:** the relay gone with `proxy-text` asking and the
`semantic` counts identical either side of the change; `semantic-crash` as a
committed tier with its control and its hand-derived braille line; the committed
evidence artefacts for all five new profiles (`fleet-demo` remains the only
profile with none, and it is not a gated tier); and the `(ends by machine reset)`
label, which removes the misreading I made myself.

## The open questions

**§A — Which modality?** Candidates:

1. **Braille cells** (grade 1, uncontracted — Unicode braille patterns or BRF),
   with line width and a rule for severity and actions.
2. **An announcement stream** — what a screen reader would say, in order, with
   severity first and actions enumerated: the input a synthesiser consumes.
3. Both, which is two lines.

**My lean: 1, braille.** It is deterministic (a table plus width rules), needs no
device, and is byte-for-byte checkable — and it differs from `proxy-text` in
*order and omission*, which is the part of "only the proxy differs" worth
testing. Argue 2 if you think an announcement stream is the more honest first
modality for a blind operator; it is a defensible answer, and cheaper to get
subtly wrong.

**§B — How does the second proxy get its envelope?** `semantic-stream` fan-out
(D2) needs a second send capability, installed by the kernel's spawn table
because `CapInstall` is undispatched. Alternative: the new proxy receives on a
shared endpoint — **rejected in advance**: RFC-0.28-001 removed shared-endpoint
races, and RFC-0.33-001 met one again.

**§C — What does the tier assert?** Both proxies' markers from one envelope, at
minimum. Should it also assert the **rendering content** (a specific braille
cell sequence), or only that a rendering happened? **Lean: content, from a
committed vector** — a marker that only says "rendered" is the shape this project
keeps filing errata about.

**§D — Where does the renderer live?** In the service, or in a host-testable
crate the service wraps (like `fjell-bootctl-model`'s relationship to the block)?
**Lean: a crate**, so D4's vectors run in Gate 1 and the service stays a thin
adapter.

**§E — What does the second modality do about `TextToken`?** It carries an id and
a fallback string. A real assistive presentation would look the id up in a
catalogue; today only the fallback exists. **Say whether the renderer uses the id
at all**, and if not, what that means for the claim that meaning — not text —
crosses the boundary.

**Answer all five in writing before implementing.**

## Requirements

**R1 — Re-derive** Findings 1–4, with `/usr/bin/grep -a` and a control per
absence. Report every disagreement, including the next free `ImageId` and the
current prebuilt count.

**R2 — §A–§E answered in writing.**

**R3 — D4:** the renderer as a pure function with committed vectors, running on
the host.

**R4 — D1/D2/§B:** the service, its image, its endpoint **allocated**, its
capability, and `semantic-stream`'s fan-out — with the endpoint allocation
checked by a test or an assertion, because that omission is now a known trap.

**R5 — D3/§C:** the QEMU tier, asserting both renderings of one envelope.

**R6 — D7:** `external-design/abdd-semantic.md` and
`docs/src/releasing/v1-limitations.md` updated to say what now exists, what the
stream is *for*, and which hardware still does not exist.

**R7 — RFC-0.33-002's readiness row** for a second modality moves from
`**IN PROGRESS** → v1.x` to done, with the run id that shows it.

**R8 — The prebuilt set and the repro baseline** re-recorded in the same commit
as the rebuild — the rule E-0.33's line learned twice.

**R9 — D8:** the coupling removed, with a tier that starts one presentation and
not the other and shows the publisher unaffected. **E-058 CLOSED**, or its
survivor named.

**R10 — The gates**, each by its own exit status, `test-all` including the new
tiers, the ABI snapshot re-recorded for the new items, and a CI run id.

### Non-goals

- **Audio output, or driving a braille display.** No device exists (E-004).
- **An input path** (D6, ADR-v0.5-005).
- Grade-2 contracted braille, or any language beyond the fallback text's
  character set.
- The adaptive Personal Proxy — a *Could* in the founding requirements, and
  unscheduled.
- A third modality.

## Risks

**A renderer is content work, and content work has no natural end.** §A's lean to
grade-1 braille is partly a scope decision: a table, a width, and a rule for
severity. If the answer becomes "implement a braille standard", the line has
escaped.

**Two proxies can drift into two decoders.** D1 exists because the cheapest way
to make a second renderer work is to copy the first, and that would disprove the
very claim this line tests while appearing to confirm it.

**The demonstration can be satisfied dishonestly.** A marker saying
`proxy-braille: rendered` proves nothing. §C's lean — assert the bytes — is what
makes the tier evidence.
