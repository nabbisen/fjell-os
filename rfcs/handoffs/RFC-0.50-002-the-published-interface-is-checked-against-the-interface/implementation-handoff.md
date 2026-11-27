# Developer Handoff — RFC-0.50-002

**Governing RFC:** [RFC-0.50-002](../../done/RFC-0.50-002-the-published-interface-is-checked-against-the-interface.md)
**Milestone:** 0.50 · second line
**Status:** inherited from the governing RFC (Implemented, 0.50.0)
**Audience:** implementation model

## Subject

**The published interface is checked against the interface.**

Correct the six (then seven) defects in the published syscall surface and build `syscall-doc`, which compares both
published tables against `SyscallNumber`. The red run is captured before any document is touched.

## Where this handoff came from

The milestone's instructions were given as one document, `.git-exclude/handoff/handoff-0.50.md`
(scope: `.git-exclude/handoff/scope-0.50.md`), covering all three lines and the four small items
together — the project's convention since 0.46. This file is the per-RFC record the lifecycle
requires, and it carries the governing link and the inherited status that `handoff-status` and
`rfc promote` read.

## The architect's errors on this line, named

Recorded here because the convention requires each handoff to carry them, and because three of
them were found by the implementer rather than by me:

* **RFC-0.50-002**: see the RFC's *"Where the handoff and the tree differ"* and *"As built"* sections,
  and the three stage records under `.git-exclude/reviewed/review-record-0.50-stage-*.md`.
* Across the milestone: the two stale `MmioUnmap` comments I ruled swept and had not; E-135's
  headline figure, which added two sets together; the criterion-9 gap at the 0.49.0 cut, which is
  **E-137**; a `services.md` miscount that compared rows to programs; a lease-seam grep that
  filtered for a syntax and missed a function call; and a criterion-9 check I specified
  document-scoped, which passes the very record it was for.
