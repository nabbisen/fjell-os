# RFC-0.30-001: The reproducibility check compares a file to itself, and the kernel is not in the baseline

**Status:** Accepted — by the owner (nabbisen), 2026-09-09; implementation may begin (RFC 000)
**Milestone:** 0.30
**Tracks.** **E-036**, widened on scoping from *"the two-build check is never
run"* to *"it is never run, it could not fail if it were, and neither mode
covers the kernel."*
**Touches.** `tools/fjell-repro-check`, `tests/repro/baseline-digests.txt`,
`docs/security/threat-model-v1.md` (**T20**), the release cycle. **Does not touch
the kernel source, the ABI, or any service.**
**Relates to:** **E-037** (the toolchain is recorded with no artefact — the
reason cross-machine reproducibility is untestable today); RFC-0.24-002 (whose
`ci-proptest` finding is this defect's exact shape); RFC-v0.22-001.

## Summary

`docs/security/threat-model-v1.md` **T20** — *Reproducibility-failure-as-
substitution* — states its defence as:

> *"RFC-v0.10-003 (reproducible build gate). **Two-build SHA-256 digest
> comparison** (hardened from FNV-1a in RFC-v0.16-005, H-04)."*

Three things are wrong with that sentence, and each was found by running the
thing rather than reading about it.

### 1. The two-build check cannot fail

`two_build_check` runs `cargo xtask build`, hashes, runs `cargo xtask build`
again, hashes, and compares. **There is no clean between the builds and no
separate target directory** — so the second build is an incremental no-op, the
files are never rewritten, and the comparison is between a file and itself.

Run on 2026-09-09, possibly for the first time:

```
fjell-repro-check: build 1 / 2 …
    Finished `release` profile [optimized] target(s) in 0.41s
fjell-repro-check: build 2 / 2 …
    Finished `release` profile [optimized] target(s) in 0.40s
fjell-repro-check: PASS (30 artefacts identical)
```

**0.41s and 0.40s.** Neither build compiled anything. The working tree was
unchanged afterwards.

This is **`ci-proptest`'s shape** — a job named for a check that ran zero of
them (RFC-0.24-002 Slice 6). Here it is a gate named for a comparison that
compares nothing.

### 2. It is never run anyway

Every invocation in the tree and in every release record passes `--skip-build`.
CI has no repro job at all. So the tautology has never even been executed as
part of anything.

### 3. Neither mode covers the kernel

`DEFAULT_TARGETS` is the kernel ELF **plus** the `prebuilt/` directory, so the
two-build path collects **30** artefacts. `tests/repro/baseline-digests.txt`
holds **29** — every one a `prebuilt/*.bin`. **The kernel binary is in no
baseline.**

So `--skip-build`, the only mode that runs, verifies that 29 committed service
binaries still hash to digests recorded from those same files, and says nothing
about the kernel at all.

## What `--skip-build` is actually worth

It is not nothing, and the RFC should not pretend otherwise: it catches a
committed prebuilt that was rebuilt without re-recording, which is a real
incident this project has had. It is a **staleness check on committed
artefacts**. It is not a reproducibility check, and T20 claims the latter.

## The settled part

**D1 — T20's sentence is corrected in this line, whatever else happens.** A
threat model naming a defence that cannot fail is worse than one naming none.
**Correcting the claim is a legitimate outcome on its own** — if the mechanism
turns out to be expensive or infeasible, the honest move is to describe what is
actually done and record the gap, not to keep the sentence.

**D2 — A real two-build check needs two real builds.** Separate target
directories, or a clean between, or a content-addressed comparison of freshly
produced outputs. **Whatever is chosen, demonstrate it failing** on a
deliberately non-reproducible input (an embedded timestamp, `__DATE__`-style
build metadata, or a hash of the build path) — per RFC-v0.22-001. A check that
has only ever been seen passing is what this RFC is about.

**D3 — The kernel goes in the baseline, or its absence is stated.** Either
`fjell-kernel`'s digest is recorded and checked, or the documents say the
reproducibility evidence covers services and not the kernel. The present
situation — collected by one code path, absent from the other, mentioned by
neither — is the worst of the three.

**D4 — Do not conflate this with E-037.** Cross-machine reproducibility is not
testable while the toolchain is unrecorded; **this line establishes
same-machine, two-build reproducibility**, which is a real and weaker property.
Say which one the corrected T20 claims.

## The open question — §5

**Where should the real check run?**

1. **In CI, every push.** Strongest signal, and it doubles CI build time —
   two full builds of 29 services plus the kernel.
2. **At the release cut only**, as an exit criterion. Cheap in aggregate, and
   catches a regression only at the moment it is most expensive to fix.
3. **Nightly / scheduled**, off the critical path, with failures reported rather
   than blocking.

**Answer in writing before implementing**, with the measured cost of a genuine
two-build run. **I do not know that number** — nobody does, because it has never
been done — and it is the first thing this line should produce.

## Requirements

**R1 — Measure a real two-build run** and report the wall-clock cost. This
precedes the design.
**R2 — `two_build_check` performs two genuinely independent builds**, and is
**demonstrated failing** on a deliberately non-reproducible input.
**R3 — The kernel's coverage settled** per D3.
**R4 — T20 corrected** to describe what is actually verified, whichever way R2
lands.
**R5 — §5 answered**, and the check placed where the answer says.
**R6 — E-036 → `CLOSED`.** If the two-build check turns out infeasible at
acceptable cost, **E-036 still closes** — with T20 corrected and a new erratum
filed for the missing capability. What must not survive is the false sentence.

### Non-goals

- **E-037.** The toolchain remains unrecorded; this line does not fix
  cross-machine reproducibility and must not claim to.
- Changing what is built, or any kernel/ABI/service source.
- Making `--skip-build` do more than it does. It is a staleness check; leave it
  one and name it honestly.

## Risks

**A real two-build check may fail the first time it runs**, and that would be
the finding of the milestone rather than an obstacle. Embedded paths, timestamps
and `-C metadata` are the usual culprits. **Do not chase a green result** — a
reproducible build that is documented as not-yet-reproducible is worth more than
a check quietly narrowed until it passes.

**The cheap fix is to keep the tautology and reword T20.** That is D1's fallback
and it is legitimate — but it should be chosen after R1's measurement, not
instead of it.
