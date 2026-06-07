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
| v0.18.0 | Selected targets may become Release-required. |

## Promotion criteria

A target becomes release-required only when all hold:

- proof passed in CI across at least two releases/milestone tags,
- Rust conformance test exists and passes,
- a proof review record exists,
- maintenance cost is acceptable,
- no hidden unsound assumptions (all assumptions written in the proof file).

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
capability, lease, boot-control — all Experimental, all release_required=false.
Conformance tests pass in ordinary `cargo test`. Proofs written, pending
toolchain pin (`verification/verus/TOOLCHAIN.md`).
