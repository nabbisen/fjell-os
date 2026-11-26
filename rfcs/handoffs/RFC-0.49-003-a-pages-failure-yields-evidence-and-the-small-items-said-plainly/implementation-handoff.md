# Developer Handoff — RFC-0.49-003

**Governing RFC:** [RFC-0.49-003](../../done/RFC-0.49-003-a-pages-failure-yields-evidence-and-the-small-items-said-plainly.md)
**Milestone:** 0.49 · the small items
**Status:** inherited from the governing RFC (Implemented, 0.49.0)
**Audience:** implementation model

---

## 1. A Pages failure now yields evidence, and no third theory was offered

E-133 records **two diagnoses by the architect, each refuted by the next run**: a transient
artifact-store race, then a queued-second-deploy theory. The symptom is `upload-pages-artifact`
finalising an artifact with an id and a byte count, then `deploy-pages` reporting **`Found 0
artifact(s)`** in its own run — five observations, intermittent, in both event types.

`.github/scripts/pages-artifact-probe.sh` lists the run's artifacts and the repository's newest
`github-pages` artifacts as the API sees them, immediately before the deploy and again after it
(`if: always()`), with `expired`, size, both timestamps and the attributed run — the fields that
separate a store race from a mis-association from an expiry. It is **read-only and incapable of
failing the job**: `set +e`, timeouts, `exit 0` whatever happens, `continue-on-error`. **No fix was
attempted.** The cost is one permission, `actions: read`, on the job that deploys the site, stated in
the RFC rather than left in a diff.

Tested against a stub `gh` (documented JSON, HTTP 403, `gh` absent). **Not demonstrated: a live run** —
which is the nature of the instrument, since it exists so that the *next* failure produces data.
E-133 remains closed with **no mechanism claimed**.

## 2. The clippy gate was dropped, and the measurement is why

The architect's instruction was "gate `--lib` for the tool crates". Measured: **six of the nine tool
crates are bin-only**, so that gate covers three crates and is, in the implementer's words, "green and
near-meaningless"; a gate over default targets is red at **44 locations**. **Neither shape was what
was asked for, so nothing was built and the table was written down instead.** This is the third time
measuring a clippy instruction has overturned it.

## 3. The readiness figure remains the owner's

`verus-count` reports release-required prover-verified **19**, all targets **26**, release-required
named ids **12**, all targets **17**. The readiness row promises **20**, matching none. The architect's
recommendation is **19** — prover-verified items in release-required targets, derived — and the
decision has now stood open for six milestones. **Not resolved by implementation, deliberately.**

## 4. What this line recorded rather than fixed

Two errata were filed at the cut from findings in this milestone rather than from its lines:
**E-134**, the kernel crate's twenty-seven `#[test]` functions that cannot compile, found by a
scheduler bug that only booting could catch; and **E-135**, a newly gated QEMU profile being unable to
record its own marker artefacts, found while trying to cite one as evidence for RFC-0.49-001. Both are
ACCEPTED with their bounds stated, and both are deliberately unscheduled to 0.49: each needs code, and
code added after a milestone's verification run does not belong in a release commit.
