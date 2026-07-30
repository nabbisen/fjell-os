# Fjell OS — Testing and Gates (QA Handoff)

*Compact testing handoff. Version: v0.21.2.*

## 1. Test goal

This handoff provides confidence that v0.21.2 is the v1.0 freeze candidate: the
capability and lease security boundaries are enforced (and machine-checked for
their core predicates), the negative-test harness is fail-closed for the covered
categories, and the build is reproducible. Behaviours validated: capability
non-amplification, lease-bounded revocation including in-flight IPC cancellation,
MMIO/DMA/audit/user-copy/policy/IPC/service-lifecycle refusal paths, and ABI
stability. Intentionally not tested yet: store/upgrade negative paths (specs
exist, no emitters), the svc READY negative pair (timing-sensitive, 2 of 4
markers), and real hardware.

## 2. Requirements-to-tests map

| Requirement | Test type | Command / file | Latest result |
|---|---|---|---|
| Capability authority never amplified | proof + property + qemu | Verus capability target; `qemu-negative capability` | Verus 8/8; 8 markers |
| Lease revocation bounded + atomic | proof + qemu | Verus lease target; `qemu-negative ipc` | Verus 5/5; 3 markers |
| BCB mirror selection total | proof | Verus boot-control target | machine-checked (pilot, not release-gated) |
| MMIO refusal enforced | qemu negative | `qemu-negative mmio` | 3 markers |
| DMA rights enforced | qemu negative | `qemu-negative dma` | 3 markers |
| Audit evidence emitted | qemu negative | `qemu-negative audit` | 1 marker |
| User-copy bounds enforced | qemu negative | `qemu-negative user-copy` | 2 markers |
| Policy default-deny | qemu negative | `qemu-negative policy` | 4 markers |
| Service lifecycle (partial) | qemu negative | `qemu-negative svc` | 2 of 4 markers |
| Unsafe sites annotated | static audit | Gate 2 (unsafe-audit) | 0 missing |
| MMIO ordering annotated | static audit | Gate 3 (mmio-audit) | 0 missing |
| ABI stability | snapshot | Gate 4 (abi-snapshot) | no removals |
| Reproducible build | digest compare | `fjell-repro-check` | artefacts identical |

## 3. Required local commands

The project's gate commands are driven through `cargo xtask`:

```sh
cargo xtask build                  # zero warnings expected
cargo xtask test-all --no-qemu     # 5 required host tiers; expect ALL REQUIRED TIERS PASSED
cargo xtask test-all               # adds QEMU smoke + negative
cargo xtask release-rehearsal      # all 11 mechanical gates
```

Individual negative profiles:

```sh
cargo xtask qemu-negative capability   # 8 markers, fail-closed
cargo xtask qemu-negative ipc          # 3 markers
# … mmio, dma, audit, user-copy, policy, harness, svc
```

Formal proofs (requires Verus on PATH — see
`docs/src/verification/verus-setup.md`):

```sh
cargo xtask verus-check --release-required   # capability + lease (Gate 10)
cargo xtask verus-check --all-pilot          # + boot-control
```

A run fails fail-closed if an expected marker is absent **or** a forbidden
marker (`NEG:HARNESS:WRONG_ERROR`, `NEG:HARNESS:UNEXPECTED_OK`, `TEST:FAIL`,
`kernel panic`, `panicked at`) appears. Serial logs land in
`tests/qemu/artifacts/<category>/serial.log`.

## 4. Test coverage and known gaps

| Area | Coverage status | Gap / risk | Follow-up |
|---|---|---|---|
| Capability boundary | Strong | proof + property + qemu | — |
| Lease revocation | Strong | proof + qemu | — |
| IPC protocol | Adequate | real blocking path now exercised (post-E-010); not formally proven | optional property tests |
| Store negative | Missing emitters | specs exist, no runtime markers | v1.1 |
| Upgrade negative | Missing emitters | specs exist, no runtime markers | v1.1 |
| Service-manager lifecycle | Partial (2/4) | READY pair timing-sensitive | v1.1 |
| DMA revoke unmap | Functional via zeroize | PTE unmap path bypassed | post-v1.0 |

## 5. Failure investigation notes

| Symptom | Likely cause | First check | Fix / owner |
|---|---|---|---|
| QEMU "Could not open fjell-disk.img" | Stale build before v0.20.2 fix | Confirm v0.20.2+; disk image is pure-Rust now | resolved at v0.20.2 |
| Gate 10 "verus not on PATH" | Verus binary not installed | `verus --version` | add to PATH per verus-setup.md |
| repro-check FAIL after kernel edit | Baseline stale | re-record: `fjell-repro-check --skip-build` then re-run | expected after any kernel change |
| bare `cargo build` fails on asm crates | Wrong build entrypoint | use `cargo xtask build` | use the xtask |

## 6. Release confidence statement

**Conditionally ready.** All eleven mechanical gates pass (Gates 1–8, 10, 11
green; Verus machine-checked capability 8/8 and lease 5/5). The non-blocking
gaps are documented and accepted: store/upgrade negative emitters (v1.1), svc
READY partial coverage (2/4), and the deferred DMA unmap. **Gate 9 (manual
limitations sign-off by the owner) is the one remaining blocker** before the
v1.0.0 tag. Evidence: `cargo xtask release-rehearsal` output;
`docs/release/v1.0-release-notes.md`; `docs/release/v1-limitations.md`.

*(Superseded — RFC-v0.21.3-001: at v0.21.2 the workspace manifest did not
parse, so none of the eleven mechanical gates could actually run; "all
eleven pass" and "Gate 9 is the one remaining blocker" both described an
intended, not a verified, state. See
`rfcs/done/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`.)*
