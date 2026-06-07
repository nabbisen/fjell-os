# RFC-v0.18-001: Verus Target Promotion to Release-Required

**Status:** Proposed
**Milestone:** v0.18.0
**Depends on:** RFC-v0.17-002…006 (Verus selective adoption, Stage A)

## Summary

Promote the two **tier-3** Verus pilot targets — `capability` (rights
non-amplification) and `lease` (epoch revocation) — from Experimental to
**Release-required**. `boot-control` (tier 2) remains pilot-required. This is
the v0.18.0 step scheduled by the RFC-v0.17-005 staging table.

## Selection rule (not ad hoc)

`verus-targets.toml` already tiers the targets: **tier 3 = release-critical**,
**tier 2 = pilot-required**. Promotion follows that existing classification —
the tier-3 proofs become release-required; the tier-2 proof does not. No new
judgement is introduced.

| Target | Tier | v0.17.x | v0.18.0 |
|--------|------|---------|---------|
| capability | 3 | Experimental | **Release-required** |
| lease | 3 | Experimental | **Release-required** |
| boot-control | 2 | Experimental | Experimental (pilot-required) |

## Promotion criteria check (proof-gate-policy)

For both promoted targets, all criteria hold:

- **Proof passed in CI across ≥2 milestone tags** — v0.17.1 (first recorded
  PASS) and v0.18.0 (this tag). See the promotion ledger.
- **Rust conformance test exists and passes** — `verus_conformance`,
  `lease_conformance` (12 cases) plus 13 property tests.
- **Proof review record exists** — `review-records/v0.17-pilot-targets.md`.
- **Maintenance cost acceptable** — proofs are small, pure-logic, stable since
  v0.17.0; no hardware effects (guardrail-compliant).
- **No hidden unsound assumptions** — assumptions are written in the proof
  files; `subset_is_transitive`, `zero_is_subset`, `equal_rights_allowed` use
  explicit `by(bit_vector)`.

### Honest caveat on the two-milestone rule

The criterion's intent is to see a proof survive real intervening development.
Here the two recorded PASS tags (v0.17.1, v0.18.0) land close together rather
than across a long development gap. The additional stability evidence —
conformance + property tests, and the proofs' independence from the surrounding
code's churn — is what justifies promotion now. The **demotion path**
(proof-gate-policy) remains the safety valve if either proof later proves
fragile or obstructive.

## What Release-required means (the teeth)

Promotion is only meaningful if "the prover did not run" cannot pass as
success. Therefore, for a release-required target, the release gate
(`verus-check --release-required`, wired into `release-rehearsal` as Gate 10)
treats **anything other than a real Verus PASS as blocking — including
CONFORMANCE-ONLY**. A release cannot be certified for these targets without an
installed, passing Verus toolchain (pin: `TOOLCHAIN.lock`).

This does **not** change the Stage A guarantees that still hold:

- Verus is still **not** a build dependency (`cargo build`/`test` never need it).
- Normal CI (`ci-verus`) stays `continue-on-error` — non-blocking on push.
- Most contributors still never need Verus; the gate bites only at *release*.

## Demotion

Unchanged from proof-gate-policy: a release-required target may be demoted
(retaining its conformance test) with architect approval if the toolchain
breaks repeatedly, the proof blocks important fixes without safety value, or the
proof no longer matches the implementation.

## Scope

This RFC promotes exactly two targets and adds the release-required teeth to the
gate. It introduces no new proof targets and no new shipped-code behaviour.
