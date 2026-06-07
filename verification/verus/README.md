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

Machine-checked: **20 obligations verified, 0 errors** (v0.18.1; lease gained the C6 bounded-domain lemma) under verus
`release/0.2026.05.24.ecee80a` (see `TOOLCHAIN.lock`). Conformance tests also
run in ordinary `cargo test` (23 cases, incl. the C6 retire-before-wrap boundary tests) plus 14 property tests — all pass.
Each proof maps 1:1 to the shipped predicate. No proof is a release blocker
at v0.17.x (Stage A, `release_required=false`).
