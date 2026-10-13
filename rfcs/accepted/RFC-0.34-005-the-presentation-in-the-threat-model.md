# RFC-0.34-005: The presentation in the threat model

**Status:** Accepted — by the owner (nabbisen), 2026-10-13; implementation may begin (RFC 000)
**Milestone:** 0.34
**Tracks.** **E-060** — `docs/src/security/threat-model-v1.md` is authoritative for
v1.0.0 and contains no proxy and no presentation, so the component that receives
every operator-facing byte, that an archetype now depends on, and that can stall the
node has never been analysed as a boundary.
**Kind:** amendment to an authoritative document — the model's own governing RFC
(RFC-v0.15-002) requires one for any change, which is why this is an RFC and not an
edit.
**Touches** *(indicative)*: `docs/src/security/threat-model-v1.md`,
`docs/src/releasing/v1-limitations.md`, `docs/src/compliance/standards-mapping.md`,
`docs/src/adr/ADR-v0.5-005-proxy-text-no-input.md`.
**Relates to:** **T17**, whose residual already names the class this amendment must
cover; RFC-0.34-004, which fixes the live instance of it while this line puts the
class in the model; RFC-0.33-002 (the archetype that depends on a proxy);
RFC-0.34-001 (E-058: a presentation's absence or crash as a failure mode).

## Summary

Re-derived 2026-10-13 at `33182e0`.

### Finding 1 — the component is absent from the document that is authoritative about it

The model is 230 lines, 20 threats **T1–T20**, each with adversary, defence and
residual, and its header says *"Governed by RFC-v0.15-002. Authoritative for
v1.0.0."* Searched with a control: **`proxy` 0, `presentation` 0, `braille` 0**, and
`T17` found. The component that renders every operator-facing byte appears nowhere.

### Finding 2 — T17's residual already describes the gap

T17 is *IPC sender forgery*; its defence is RFC 055's kernel-attested sender, and its
residual reads: *"None within the current architecture, for forgery. **Adjacent, and
not forgery:** a correctly identified sender whose payload is malformed — by a bug or
by compromise. The kernel attests who sent the bytes, never that…"*

**That is the class the proxy sits in**, and RFC-0.34-004's E-059 is a live instance:
a correctly identified presentation whose payload the receiver trusted. The model
names the class in a residual and then has no threat for the component it applies to.

### Finding 3 — what exists now that did not when the model was written

Two presentations, output-only, driven by one stream; an absence and a mid-run crash
that are both tiers; a stream that reports a presentation that has stopped asking;
an archetype in `v1-direction.md` that **depends on** a proxy; and 0.33.0 shipping
all of it. The model was written when `proxy-text` was a demonstration of a
rendering, not a component anything relied on.

## The settled part

**D1 — the amendment covers the presentation in all three directions:** as a
**sender** (what it can send the stream, and what the stream must not believe), as a
**receiver** (what it can be given, and what a compromised one learns), and as a
component whose **absence or crash** is a failure mode (E-058's class, now with a
mechanism and two tiers).

**D2 — the separate-device case is named or explicitly excluded.** RFC-0.33-002's
archetype describes a person using an assistive presentation; whether that
presentation may run on a different machine changes the boundary completely. The
model must say which configurations it is authoritative for.

**D3 — each new threat keeps the model's existing shape** — adversary, defence,
residual — and **T1–T20 are not rewritten.** An amendment adds; it does not re-open
settled analysis.

**D4 — T17 is the hinge, and the amendment says so.** Each new threat states whether
it is an instance of T17's adjacent class, and T17's residual gains a pointer to the
threats that now carry it, so the two cannot drift apart.

**D5 — a threat whose defence does not exist yet gets a residual that says so**, by
name: the rights word (E-059 / RFC-0.34-004, if that line has not landed), the
160-byte console bound, a presentation that is **alive but silent** and is never
reported, and the fact that nothing restarts a stuck presentation.

**D6 — the amendment is read by exit criterion 8 from the next cut onward.** If a row
in `standards-mapping.md` or a bullet in `v1-limitations.md` becomes true or false
because of it, that change lands in the same commit.

## The open questions

**§A — new threats, or extensions to existing ones?** *Lean: new threats, T21
onward*, because the component is new to the model and folding it into T17 would bury
a boundary inside a residual. Say how many and what each is, before writing prose.

**§B — is a presentation inside the TCB?** The model must answer this, and the answer
may differ by configuration: a `proxy-text` on the node's serial console is not the
same boundary as an assistive presentation on a person's own device. *Lean: name both
configurations, and make the TCB answer explicit for each* — this is the question
D2 exists to force.

**§C — is "the node continues with no presentation" a threat, a defence, or a
limitation?** E-058's fix made the publishers independent of a presentation, which is
a **defence**; what survives — alive but silent, nothing restarts it — is a
**residual**. *Lean: one threat, with that defence and those residuals*, rather than
leaving the whole area in `v1-limitations.md` where no adversary is named.

**§D — who is the adversary?** The model's existing classes (e.g. `C-NODE-EXEC`) may
not fit a compromised presentation on a separate device. *Lean: reuse an existing
class if one honestly fits and define one if none does* — and say which, because an
invented adversary class is a claim about the model's completeness.

**§E — what does this amendment make checkable?** A threat model is prose, and this
project's rule is that a claim needs a demonstrated mechanism. *Lean: every defence
cited must name the tier, test or erratum that shows it holds*, so the amendment is
not a page of assertions. Where nothing demonstrates a defence, D5 applies.

**Answer all five in writing before writing the amendment.**

## Settled at the review, 2026-10-13

**D7 — accepted. The defence-to-evidence table is what makes this an amendment rather
than a page of prose**, and the row that proves the discipline worked is **T23:
defence "None", demonstrated by "—"**. A threat model that can write that about itself
is worth more than one that cannot.

**§E held.** T21's defence cites the `semantic-liar` tier, T22's cites
`semantic-absent` and `semantic-crash`, and where there is nothing, the table says
nothing. Doing this line **after** RFC-0.34-004 rather than before it is why T21 can
cite a closed erratum instead of a conditional.

**D8 — §B's answer is right, and the escalation was correctly not taken.** A
presentation is a same-node task in the TCB exactly as every other service is, which
is T17's own point rather than a new property; and the separate-device configuration
is **named and explicitly excluded** rather than analysed thinly. *"No assistive
presentation on a separate device may be called safe under this model until its own
amendment exists"* is the sentence that makes the exclusion a commitment instead of a
silence.

**D9 — T23 needs no erratum number of its own (your question 1).** E-060's closure
already names it — *"a compromised presentation showing false state to the attending
operator, for which no defence"* — and the model entry carries the analysis. A second
number would duplicate one record in two places, which is how they drift. **But the
register entry is the one that must stay**: Gate 7 and every cut read the register, not
the threat model.

**D10 — your question 2: yes, cite T22 at `CRA-I-2h`, and I have done it** — that row
is exit criterion 8's territory, not yours. The row reads *"no dedicated in-kernel
resource-exhaustion or DoS mitigation instrument was found"*, and T22's defence **is**
a real mechanism with two tiers for a specific availability failure. A conservative
row that omits a mechanism that exists understates the evidence, and criterion 8 reads
these rows for truth in **both** directions. The status stays **partial**, because a
no-stall guarantee for the narration path is not availability in general; the citation
now reads `(T13, T22 — …)` with the tiers named. Leaving it uncited and asking was
right: the judgement is mine.

**Accepted as delivered:** T21–T23 in the model's existing shape; T17's residual
gaining exactly one sentence, a pointer to T21 by name, with T1–T20 otherwise
untouched (the whole diff of the amendment commit removes **one** line, that
residual's); the fifth T22 residual found at R1 and added to both documents in the
commit that found it; `CRA-I-2f` gaining T21 as a precise fit to an existing row
rather than a new one; and `errata-limitations` dropping from 8 ACCEPTED to 7 by a
real closure.

## Requirements

**R1 — Re-derive** Finding 1 with its control (the three absences and `T17` present),
and Finding 3: what the presentation line actually shipped, read from the tree and
from `releases/0.33.0.md`, not from memory.

**R2 — §A–§E answered in writing.**

**R3 — D1/D2/§A/§B:** the new threats, in the model's shape, with the configurations
named.

**R4 — D4:** T17's residual pointing at them, and each new threat saying whether it
is an instance of its class.

**R5 — D5/§E:** every defence citing what demonstrates it; every absent defence a
named residual with its erratum or limitation.

**R6 — D6:** whatever `v1-limitations.md` or `standards-mapping.md` must change,
in the same commit, with the rows named for the architect's criterion-8 reading.

**R7 — E-060 CLOSED** or survivors named, register and limitations together.

**R8 — The gates**, each by its own exit status, and `consistency-check --all` —
`standards-mapping` and `evidence` both resolve citations, so a new citation must
resolve from the site as well as on disk (RFC-0.33-004 D4's rule).

### Non-goals

- Rewriting T1–T20, or re-opening any settled residual (D3).
- Building any defence this amendment names as missing — naming it is the work.
- An input path (the owner ruled *no v1 yet* at 0.34's scoping).
- Hardware attestation of a separate device (E-004's territory).

## Risks

**A threat model is the easiest document in this project to fill with sentences that
cannot be wrong.** §E is the guard: a defence that names no tier, test or erratum is
an assertion, and this project has an erratum class for exactly that.

**The separate-device configuration can quietly expand the TCB.** If §B's answer is
*"the presentation may run on another machine"*, then a claim about what the node
guarantees becomes a claim about a channel, and that is a larger analysis than this
amendment — if it reaches that, **stop and escalate** rather than write it small.
