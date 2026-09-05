# RFC-0.27-004 §7 — Does an unresolvable historical citation block a release?

**Governing RFC:** [rfcs/done/RFC-0.27-004-evidence-that-survives.md](../../rfcs/done/RFC-0.27-004-evidence-that-survives.md)

The handoff states no inclination this time and asks for an argued answer.
This document gives one, states the two rejected shapes, and — because the
rejection of shape 1 turns on a specific worry (permanent exemption) — shows
how that worry is addressed without building a new instrument.

## The three shapes

**Shape 1 — never blocks, full stop.** Every unresolvable citation, past or
future, is pure disclosure, like `v1-limitations.md`. Nothing about
evidence citations is ever release-gated.

**Shape 2 — the parallel to RFC-0.27-003's answer.** An *annotated*
historical gap never blocks (it is disclosure, not a live defect). A
citation into `tests/evidence/` that is not annotated — new or old — must
stay valid: exists, carries provenance, and that provenance's commit sha is
a real ancestor of `HEAD`. If any of that breaks, the cut fails.

**Shape 3 — a sunset.** Annotated citations are tolerated only until some
milestone, after which each must be resolved (promoted properly) or the
underlying claim withdrawn. Unlike shapes 1 and 2, this needs somewhere to
record *which* annotations are still owed and *by when* — a second
tracked fact, which is exactly the kind of thing this project's own errata
register has repeatedly found going stale when nobody gave it a mechanism
(E-014's family, and RFC-0.27-003's own reason for deferring its analogous
shape 2).

## Answer: shape 2, mechanically — with shape 3's concern addressed by reusing an instrument that already exists

**A citation into `tests/evidence/` must always resolve to an existing file
with valid provenance; an annotated historical gap never blocks.** This is
what the `evidence` subcheck (R4) implements, and it costs nothing beyond
R4 itself — the same observation RFC-0.27-003 made about its own shape 3.

**Shape 1 is rejected for the reason RFC-0.27-003 rejected its own shape
1**: a document read by outsiders needs a mechanical floor, not just a
human noticing at review time that a `tests/evidence/` citation quietly
stopped resolving.

**Shape 3's objection is real and is not dismissed — it is answered without
building shape 3's machinery.** A citation "tolerated forever" is exactly
what E-026 exists to end, and shape 2 alone would let R6's unresolvable
citations sit unaddressed indefinitely with nothing tracking that they
should not. But this project already has an instrument for "a known gap,
disclosed, with a real commitment to when it gets addressed, checked so the
commitment can't silently go stale": **the errata register's tracking
column**, verified by the `errata-tracking` subcheck (RFC-0.27-001) to be
an RFC id, a bare milestone, or `unscheduled` — never unenforceable prose.
Rather than build a second tracking mechanism scoped to evidence citations
alone, **R6's unresolvable citations are filed as an erratum with a real
milestone tracking value**, below. That milestone is the sunset. The
existing subcheck already refuses to let it drift to a shipped release
silently, the same way it already catches every other errata entry that
tries to.

**This is a deliberate act of reuse, not a loophole.** R6's own resolution
path for an unresolvable citation is not "delete the claim" — the
underlying architectural facts most of them assert (the readiness wait
executes; the capability refusal fires) are still true today, because the
code paths are unchanged. The honest resolution, when the milestone is
reached, is to **re-run the current code** (a real, fresh, honestly-dated
run — not a re-manufacture of the original, which D4 forbids) and promote
*that* log properly, superseding the annotation. Nothing about D4 forbids
producing new evidence for a claim that remains true; D4 forbids presenting
new evidence as if it were the old evidence.

## Observation versus judgement, named explicitly per the handoff's request

- **Observation:** shape 2 costs nothing beyond R4 (verified — R4 is built
  regardless of this answer, since the RFC requires it either way).
- **Observation:** the errata register's tracking column is already
  mechanically checked (RFC-0.27-001, `errata-tracking`), independent of
  this RFC.
- **Judgement:** that reusing it here is the right way to answer shape 3's
  objection rather than building a second instrument — the same
  cost-of-a-second-artifact argument RFC-0.27-003 made for deferring its
  own shape 2, applied here to reach the opposite conclusion (reuse
  instead of defer) because, unlike RFC-0.27-003's case, a suitable
  instrument already exists to reuse.
- **Judgement:** the milestone chosen for R6's erratum (`0.28`, see the
  erratum itself) — a call about how soon this is worth doing, not a fact
  derivable from the tree.
