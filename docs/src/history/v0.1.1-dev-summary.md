# Fjell OS v0.1.1 — Developer Summary

**Date:** 2026-05-17  
**Theme:** v0.1.x stabilization — release freeze + CI foundation.  
**Preceded by:** v0.1.0 (M8 complete).  
**Followed by:** v0.1.2 (RFC 026 negative-test bodies + RFC 027 threat
model body + RFC 028 ABI inventory).

---

## What this release does

v0.1.1 is the first release in the **v0.1.x stabilization line**. It
adds **no new OS functionality**. The point of the release is to:

1. Freeze v0.1.0 and document what it *is* and *is not*.
2. Lay down the CI + negative-test infrastructure that v0.2 will
   depend on.
3. File the full v0.2 RFC set so v0.2 implementation can begin from
   a concrete plan rather than from improvisation.

The repository at the v0.1.1 tag should be byte-identical in *runtime
behavior* to v0.1.0: every smoke marker (`TEST:M1:PASS` …
`TEST:M8:PASS`) is observed, no syscall changed semantics, no
service was added or removed.

## What changed concretely

### RFCs (24 new files in `rfcs/`)

| Range | Purpose | Status |
|---|---|---|
| 024–025 | v0.1.x freeze + CI | Accepted |
| 026–030, 044–047 | v0.1.x stabilization audits and gates | Proposed |
| 031–043 | v0.2 *Security Boundary Closure* design | Proposed |

Each file carries both a filesystem RFC number (`NNN`) and an "Also
known as" identifier (`RFC-v0.1.x-NNN` or `RFC-v0.2-NNN`) so external
references stay stable.

### Documentation (`docs/src/`)

New files:

- `releases/v0.1.0-scope.md` — 13-item inclusion list.
- `releases/v0.1.0-limitations.md` — 10-item exclusion list (no
  production secure boot, no remote attestation, no networking, no
  POSIX, …).
- `security/v0.1.0-known-non-goals.md` — project-level + scope-
  discipline non-goals.
- `security/v0.1.0-threat-model.md` — skeleton; full body lands in
  v0.1.2 with RFC 027.
- `roadmap/v0.1.x-stabilization.md` — release sequence v0.1.1 → v0.1.5.

Updated files: `SUMMARY.md` (top-level sections added),
`README.md` (version stamp + limitation warning),
`ROADMAP.md` (v0.2 declared as Security Boundary Closure; full v0.1.x
through v1.0 progression).

### `fjell-tools` (`crates/fjell-tools/src/`)

| File | Change |
|---|---|
| `qemu_log_check.rs` | new — generic substring matcher |
| `qemu_run.rs` | new — profile-driven runner + minimal TOML reader |
| `negative.rs` | new — `qemu-negative <category>` dispatcher |
| `smoke.rs` | refactored to use `Profile::smoke` + `run_profile` |
| `main.rs` | dispatches `qemu-test`, `qemu-negative`, `qemu-log-check`, `qemu-run` |
| `qemu.rs` | unchanged |

Every QEMU run now writes artefacts to
`tests/qemu/artifacts/<run-id>/` (`serial.log`, `qemu-command.txt`,
`expected-markers.txt`, `result-summary.txt`).

### Test profiles

`tests/qemu/profiles/` now contains a placeholder TOML for each
v0.1.x negative-test category: `capability`, `ipc`, `mmio`, `dma`,
`store`, `upgrade`. Each placeholder asserts no markers — the
infrastructure runs in CI, real cases land per v0.2 RFC.

### CI (`.github/workflows/ci.yml`)

Five jobs:

| Job | Purpose |
|---|---|
| `ci-format` | `cargo fmt --check` |
| `ci-check` | `cargo check --workspace --exclude fjell-kernel` |
| `ci-test-host` | host unit tests on every format/api/tools crate |
| `ci-qemu-smoke` | matrix m1..m8, artefact upload |
| `ci-qemu-negative` | matrix capability/ipc/mmio/dma/store/upgrade, artefact upload |

## How to verify v0.1.1

```bash
# Workspace check (host-buildable subset)
cargo check \
    -p fjell-tools \
    -p fjell-attestation-format -p fjell-audit-format \
    -p fjell-block-format       -p fjell-config-format \
    -p fjell-device-format      -p fjell-measure-format \
    -p fjell-recovery-format    -p fjell-rootfs-format \
    -p fjell-semantic-format    -p fjell-service-api \
    -p fjell-snapshot-format    -p fjell-store-format \
    -p fjell-upgrade-format     -p fjell-verify-format

# Host unit tests on the format crates (37 tests at v0.1.1)
cargo test --lib --bins \
    -p fjell-tools \
    -p fjell-attestation-format -p fjell-audit-format \
    -p fjell-block-format       -p fjell-config-format \
    -p fjell-device-format      -p fjell-measure-format \
    -p fjell-recovery-format    -p fjell-rootfs-format \
    -p fjell-semantic-format    -p fjell-service-api \
    -p fjell-snapshot-format    -p fjell-store-format \
    -p fjell-upgrade-format     -p fjell-verify-format

# Smoke (requires qemu-system-riscv64 + llvm-objcopy)
cargo xtask qemu-test m8

# Negative test infrastructure (placeholder)
cargo xtask qemu-negative capability
```

## What v0.1.1 does **not** do

- Does not change any syscall ABI.
- Does not add any new syscall.
- Does not weaken or strengthen any security check.
- Does not change the boot sequence.
- Does not change service inventory.

If any of those things appear to change at the v0.1.1 tag, it is a
regression and must be reverted.

## Next steps (v0.1.2)

- RFC 026 case bodies: turn each placeholder profile into a real
  negative test exercising a v0.1.0 boundary that should reject
  invalid use.
- RFC 027 threat-model body: replace the v0.1.1 skeleton.
- RFC 028 ABI inventory: enumerate every syscall and IPC protocol
  shipping in v0.1.0.

Beyond v0.1.2: v0.1.3 audits, v0.1.4 ADR sync + release checklist,
v0.1.5 v0.2 backlog. v0.2.0 begins after v0.1.5.
