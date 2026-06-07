# Verus Proof Gate Policy

Fjell stays Rust-first. Verus is used only for small, stable,
security-critical logic where proof value exceeds maintenance cost.
This policy governs how proofs enter CI without slowing ordinary work.

## Gate levels

| Level | Meaning |
|-------|---------|
| **Experimental** | Optional CI; failure never blocks a release or unrelated PR. |
| **Pilot-required** | Required for PRs that touch the target's module. |
| **Release-required** | Failure blocks the release tag. |

## Staging (RFC-v0.17-005)

| Version | Status |
|---------|--------|
| v0.17.0 | All pilot targets **Experimental** (conformance-only in CI until the Verus toolchain is pinned). |
| v0.17.1 | Pilot-required for touched target modules. |
| v0.18.0 | Tier-3 targets (capability, lease) promoted to **Release-required** (RFC-v0.18-001). boot-control stays pilot-required. |

## Promotion criteria

A target becomes release-required only when all hold:

- proof passed in CI across at least two releases/milestone tags,
- Rust conformance test exists and passes,
- a proof review record exists,
- maintenance cost is acceptable,
- no hidden unsound assumptions (all assumptions written in the proof file).

**R-V1 (architect rule):** a target whose Verus machine-check FAILS stays
Experimental — or, if already promoted, blocks the release — even if every
Rust test passes. Property and conformance tests are evidence, never proof;
machine-check is the promotion precondition (C5) and a bare `PASS` is never
reported by the conformance fallback (C7: `CONFORMANCE-ONLY` /
`CONFORMANCE-FAIL`, JSON `machine_check = not_run`).

### Promotion artifact checklist (per promoted target)

1. Pinned Verus release (`TOOLCHAIN.lock [verus]`)
2. Pinned solver version (`TOOLCHAIN.lock [z3]`)
3. Exact check command (`TOOLCHAIN.lock [run] command`)
4. Machine-check logs / CI marker record (`ci-verus` artifact)
5. Updated proof review record (review-records/)
6. Failure policy documented (R-V1 above + demotion criteria below)
7. Conformance + property tests passing at the promoting tag
8. Assumptions enumerated in the proof file header
9. *(lease only)* epoch wrap modeled: retire-before-wrap (architect C6;
   `RevokeOutcome::MustRetire` at `u32::MAX`, LEASE-VERUS-005)

## Demotion criteria

A target may be demoted (retaining conformance tests) if the Verus toolchain
breaks repeatedly, the proof blocks important fixes without safety value, the
proof no longer matches the implementation, or the logic is being redesigned.
Demotion of a release-required target requires architect approval.

## Non-negotiables

- Verus is never a kernel build dependency.
- Every proof maps to shipped Rust via a conformance test (no proof-theater).
- Most contributors never need Verus knowledge.

## Current state

Three pilot targets configured (`verification/verus/verus-targets.toml`):
capability and lease are **Release-required** (tier 3, promoted v0.18.0);
boot-control is Experimental (tier 2, pilot-required). All are machine-checked
(20 verified, 0 errors as of v0.18.1) under the pinned toolchain
(`verification/verus/TOOLCHAIN.lock`) and recorded in CI by the non-blocking
`ci-verus` job; the release gate (`release-rehearsal` Gate 10) enforces the
two release-required proofs.

## Promotion ledger

The first promotion criterion — "proof passed in CI across at least two
releases/milestone tags" — is tracked here. Promotion to
`release_required = true` is a deliberate, recorded decision, not automatic.

| Milestone | Verus markers | Recorded by |
|-----------|---------------|-------------|
| v0.17.0   | CONFORMANCE-ONLY (toolchain absent) | — (does not count toward promotion) |
| v0.17.1   | capability / lease / boot-control = PASS (19 verified, 0 errors) | first CI-recorded PASS (`ci-verus`) |
| v0.18.0   | capability / lease / boot-control = PASS | **second PASS — two-milestone criterion met** |
| v0.18.1   | capability / lease / boot-control = MACHINE-CHECKED-PASS (20 obligations; markers renamed per C7) | architect conditions C4–C8 landed |

**Promotion executed at v0.18.0 (RFC-v0.18-001):** the tier-3 targets
`capability` and `lease` are now `release_required = true`. `boot-control`
(tier 2) remains Experimental. For a release-required target the gate is
strict — `CONFORMANCE-ONLY` (prover absent) blocks `--release-required`, so a
release cannot be cut for these targets without a passing Verus run
(`TOOLCHAIN.lock`). Demotion remains available with architect sign-off.
