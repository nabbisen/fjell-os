# RFC-v0.17-006: Selective Verus Adoption (umbrella)

**Status:** Proposed
**Milestone:** v0.17

## Summary

Adopt Verus formal verification for a small set of stable, security-critical
pilot targets, while keeping Fjell Rust-first. Proofs are additive and never
a build or (initially) release dependency.

## Principle

```
Verus is used only for small, stable, security-critical logic where
proof value exceeds maintenance cost.
```

## Pilot targets

1. capability rights non-amplification (RFC-v0.17-002)
2. lease epoch revocation (RFC-v0.17-003)
3. boot-control mirror selection (RFC-v0.17-004)

CI and proof-gate policy: RFC-v0.17-005.

## Guardrails (from the adoption handoff pack)

- No proof-theater: every proof maps to shipped Rust via a conformance test.
- No hardware-effect targets first (MMIO, DMA, trap, page tables excluded).
- Verus never a kernel build dependency.
- Demotion path exists if proof cost outweighs value.

## Scope

This RFC ratifies the program. The per-target proof scopes and the CI policy
are in RFC-v0.17-002…005. Implementation foundation (proof modules,
conformance tests, `cargo xtask verus-check`, policy docs) lands in v0.17.0
as Experimental.

> Note: trust-anchor provisioning (deferred from RFC-v0.16-005, H-02) is
> tracked separately and is **not** part of the Verus program.
