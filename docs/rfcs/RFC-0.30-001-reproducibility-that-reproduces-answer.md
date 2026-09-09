# RFC-0.30-001 §5 — Where should the real check run?

**Governing RFC:** [rfcs/accepted/RFC-0.30-001-reproducibility-that-reproduces.md](../../rfcs/accepted/RFC-0.30-001-reproducibility-that-reproduces.md)

Answered after R1's measurement, per the handoff's required order — the
handoff's own words: "that question cannot be answered without the number."

---

## R1 — the number

Measured directly, twice, on 2026-09-09 (this machine: 32 cores, `cargo
xtask build` at `CARGO_BUILD_JOBS=8`):

```
clean (scoped, --release --target riscv64gc-unknown-none-elf): 27–39ms
build 1: 1.94–2.10s
build 2: 1.93–2.32s
```

Re-measured at `CARGO_BUILD_JOBS=2` (a conservative proxy for a standard
GitHub-hosted runner, which this project's other CI jobs already assume):

```
build (jobs=2): 3.37s
```

**Total two-build wall-clock cost: on the order of 4–7 seconds**, clean
included. This is a workspace of small, `opt-level = "s"` embedded crates —
one kernel and 29 services, none of them large — and the number reflects
that. **The RFC's own risk section assumed this would be expensive enough to
"double CI build time"; it is not.** Nobody had measured it before because,
per E-036, nobody had ever run a genuine two-build check at all.

Full methodology and raw output: `.git-exclude/review-request/` for this
line (per the standing workflow, review requests are not committed to the
tree).

## §5 — where it runs

**Answer: option 1 — every CI push**, as a new job (`ci-repro-check`,
`.github/workflows/ci.yml`), invoking a new `cargo xtask two-build-check`
subcommand that runs `fjell-repro-check` without `--skip-build`.

**Why not option 2 (release cut only):** the RFC's own words — "catches a
regression only at the moment it is most expensive to fix" — are correct,
and there is no longer a cost argument for accepting that trade. A ~4–7s
addition to a CI run that already builds the same kernel and services in
`ci-test-services`/`ci-qemu-smoke` is not a reason to defer the signal to
the one moment it is least useful.

**Why not option 3 (nightly, off the critical path):** the same reasoning —
nightly exists to defer *expensive* signal. This signal is cheap. Deferring
it would trade timeliness for nothing.

**Rejected for a different reason — folding it into `test-all` instead of a
dedicated CI job:** `test-all`'s tier 3b already runs `fjell-repro-check
--skip-build` and RFC-0.30-001 §3/the handoff are explicit that this stays a
staleness check and is left alone. Adding a *second*, real two-build tier to
`test-all` was considered and rejected for the same reason RFC-0.29-001 kept
`ci-host-bins` as its own CI job rather than invoking `test-all` from CI at
all: `test-all` is a local dev aggregator, re-run at each developer's
discretion, while CI needs one job per named property so a failure's cause
is unambiguous in the job list. `two-build-check` gets a job the same way
`host-bin-tests` did — same command locally and in CI, so the two never
drift apart, but named and reported separately from the fast tier.

## Placed

`.github/workflows/ci.yml`: new `ci-repro-check` job, `needs: [ci-check]`,
running `cargo xtask two-build-check` on every push and pull request (the
workflow's existing triggers — no new trigger added).
`crates/fjell-tools/src/main.rs`: new `two-build-check` subcommand,
distinct from the existing `repro-check` (which keeps forcing
`--skip-build` — unchanged, per D2/§3).
