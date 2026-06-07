# Fjell Verus Proofs

Selective formal verification for small, stable, security-critical logic.
Fjell stays Rust-first; see `docs/verification/verus/proof-gate-policy.md`.

## Layout

```
verus-targets.toml                 target registry (read by xtask)
TOOLCHAIN.md                       version pin + install
capability/rights_lattice.rs       CAP-RIGHTS non-amplification
lease/lease_epoch.rs               LEASE-VERUS epoch revocation
boot-control/mirror_selection.rs   BCB-VERUS deterministic selection
```

## Each proof maps to shipped Rust

| Proof | Shipped code | Conformance test |
|-------|--------------|------------------|
| capability | `CapRights::is_subset_of` | `fjell-cap/tests/verus_conformance.rs` |
| lease | kernel lease table + `fjell_abi::lease` | `fjell-cap/tests/lease_conformance.rs` |
| boot-control | `select_bcb_mirror` | `fjell-upgrade-format/tests/mirror_conformance.rs` |

## Status

Conformance tests run in ordinary `cargo test` today (19 cases, all pass).
The Verus proofs are written and map 1:1 to the shipped predicates; they are
machine-checked once the toolchain in `TOOLCHAIN.md` is installed. No proof
is a release blocker at v0.17.0 (Stage A).
