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
