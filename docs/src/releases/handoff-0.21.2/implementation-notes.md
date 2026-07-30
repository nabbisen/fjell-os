# Fjell OS — Implementation Notes (Implementer Handoff)

*Compact implementation handoff. Version: v0.21.2.*

## 1. Implementation goal

v0.21.2 is the v1.0 freeze candidate. Implemented: the full capability/lease
kernel, synchronous IPC with kernel-attested identity, the 29-program user-space
service plane, signed-bundle verification, append-only audit, reproducible
build, and the eleven-gate release-rehearsal harness. Postponed (non-goals for
v1.0): real-hardware bring-up, multi-hart, POSIX, store/upgrade negative
emitters, end-to-end trust-anchor provisioning. Compatibility the implementation
must preserve: the `fjell-abi` snapshot surface (no removals without an RFC) and
the IPC register layout in `docs/src/abi/ipc-register-layout.md`.

**Correction (RFC-v0.22-001):** the release-rehearsal harness gained a
twelfth gate (consistency-check) in v0.22. "Eleven-gate" was accurate as of
v0.21.2 and is left as originally written, per the frozen-bundle convention.

## 2. Repository map

```text
fjell-os/
├── Cargo.toml                     # workspace root (explicit member list)
├── .cargo/config.toml             # xtask alias + RISC-V linker settings
├── crates/
│   ├── arch/                      # architecture trait + impls (3)
│   ├── drivers/                   # virtio-blk, virtio-net (2)
│   ├── formats/                   # *-format data schemas (22)
│   ├── services/                  # *d daemons + bootstrap/test programs (29)
│   ├── fjell-kernel/              # the kernel
│   ├── fjell-abi/                 # stable ABI surface
│   ├── fjell-cap/                 # capability model (host-testable)
│   ├── fjell-ipc/                 # IPC state machine (host-testable)
│   ├── fjell-syscall/             # userspace syscall wrappers
│   ├── fjell-tools/               # cargo xtask runner
│   └── … (crypto, sdk, semantic, dtb, etc.)
├── docs/                          # mdBook (docs/src/) + operational docs
├── rfcs/                          # RFC register (done/, README, lifecycle policy)
├── tests/                         # qemu/ profiles, repro/ baselines, abi/ snapshot
└── verification/verus/            # Verus targets + TOOLCHAIN.lock
```

Inspect first:

| Path | Purpose | Notes |
|---|---|---|
| `crates/fjell-kernel/src/trap/syscall.rs` | Syscall dispatch + cap-error mapping | The kernel's behavioural core |
| `crates/fjell-kernel/src/lease/mod.rs` | Lease table + reply-edge cancellation | Revocation atomicity lives here |
| `crates/fjell-cap/src/rights.rs` | Rights lattice + `to_sys_error()` | Canonical cap-error mapping; Verus-proved |
| `crates/fjell-tools/src/qemu.rs` | `SERVICES` build list + objcopy pipeline | Add new services to `SERVICES` |
| `crates/fjell-tools/src/release_rehearsal.rs` | The 11 gates | Release gate definitions |
| `docs/src/abi/ipc-register-layout.md` | Normative IPC ABI | Change requires an RFC |

**Correction (RFC-v0.22-001):** `release_rehearsal.rs` now defines twelve
gates (Gate 12, consistency-check, added in v0.22). Row left as originally
written, per the frozen-bundle convention.

## 3. Setup and build commands

Toolchain: Rust 1.91, target `riscv64gc-unknown-none-elf`, `ld.lld`, QEMU
`qemu-system-riscv64`. No `qemu-img` needed (disk image is created in pure
Rust). Full setup, including per-distro packages and Verus, is in
`docs/src/internals/local-development.md`.

```sh
rustup toolchain install 1.91
rustup target add riscv64gc-unknown-none-elf
# Ubuntu/Debian: sudo apt-get install qemu-system-misc lld
# Arch:          sudo pacman -S qemu-system-riscv lld

cargo xtask build                 # services (RISC-V) + kernel; expect zero warnings
cargo xtask qemu-test m8          # smoke boot
cargo xtask test-all --no-qemu    # host tiers (fast)
```

Note: this project drives builds through `cargo xtask`, not bare
`cargo build`, because the kernel and services need explicit
`--target`/`--package` selection and `build-std`. Running bare `cargo build`
over the whole workspace will try to compile RISC-V-asm crates (e.g.
`fjell-syscall`) for the host and fail — use `cargo xtask build`.

## 4. Key implementation details

- **Capability state model:** a per-task capability space of typed slots; rights
  are a bitset; minting enforces `new_rights.is_subset_of(source.rights)`.
- **Lease revocation:** `sys_lease_revoke` advances the epoch, walks the endpoint
  send/recv queues cancelling lease-bound waiters with `LeaseRevoked`, and calls
  `cancel_replies_for_lease` so blocked callers awaiting a reply are also woken.
- **IPC ABI:** word count is packed into tag bits 16–23 by `sys_ipc_call_words`;
  `deliver()` writes words to a2..a5 and the kernel-attested identity to a6. The
  badge is not delivered. (This is the v0.20.0 E-010 fix.)
- **Service bootstrap pattern:** each private-endpoint service needs `et.alloc()`
  in `main.rs`, a `spawn.rs` match arm, a `cs.install_raw` cap, and a
  `wait_service_ready` call. `static mut` is forbidden in services (BSS-write
  page faults in no_std) — use loop-local variables.
- **DMA revoke:** `revoke_by_pa` currently zeroizes and frees the frame but skips
  the user-VA PTE unmap (`unmap_user_va_for`) pending root-cause of a v0.8.x
  page-table corruption; the stale PTE is harmless because the frame is zeroed
  before reuse. Full analysis is in the `revoke_by_pa` comment block.

## 5. Important decisions and constraints

| ID | Implementation decision | Reason | Where enforced |
|---|---|---|---|
| IMP-01 | `forbid(unsafe_code)` outside kernel/arch; every unsafe site categorised | Small auditable TCB | unsafe-audit gate (Gate 2) |
| IMP-02 | Every `write_volatile` carries `MMIO-ORDER:` | Ordering correctness | mmio-audit gate (Gate 3) |
| IMP-03 | No `static mut` in services | BSS-write page faults in no_std RISC-V | Code review; service pattern |
| IMP-04 | Disk image via `File::create + set_len` | No `qemu-img` dependency | `crates/fjell-tools/src/qemu_run.rs` |
| IMP-05 | ABI surface frozen; snapshot forbids removals | Downstream stability | abi-snapshot gate (Gate 4) |
| IMP-06 | Release archive unpacks to `fjell-os-v{version}/`, no nesting | Clean extraction | `cargo xtask package-release` |

**Correction (RFC-v0.21.3-002):** "no nesting" means no *double* nesting, not
"no parent directory" — the archive intentionally does have one
(`fjell-os-v{version}/`). See
[`docs/src/release/v0-release-cycle.md`](../../release/v0-release-cycle.md)
§Release archive convention for the authoritative, single-source statement
(owner-accepted, Decision request 1). Row left as originally written, per
the frozen-bundle convention.

## 6. Known issues and maintenance notes

| Type | Item | Impact | Suggested fix | Owner |
|---|---|---|---|---|
| Debt | DMA user-VA unmap bypassed in `revoke_by_pa` | Stale PTE (mitigated) | Root-cause v0.8.x corruption, re-enable unmap | post-v1.0 |
| Follow-up | Store/upgrade negative emitters absent | No runtime evidence | Add `NEG:STORE:*` / `NEG:UPGRADE:*` | v1.1 |
| Follow-up | svc READY pair (2/4 markers) | Partial coverage | Make rendezvous timing deterministic | v1.1 |
| Debt | arm64 arch crate is a stub | No ARM build | Implement when ARM target is scheduled | post-v1.0 |

Evidence for the latest successful gates: `cargo xtask build` produced zero
warnings at v0.21.2; `cargo xtask test-all --no-qemu` passed all five required
host tiers; `cargo xtask release-rehearsal` passed Gates 1–8, 10, 11 with Verus
machine-checked (capability 8/8, lease 5/5). Re-run to regenerate logs.

**Repro baseline, corrected (RFC-v0.21.3-001 §M4):** `fjell-repro-check` has
two modes. The default (two-build) mode needs no stored baseline — it builds
twice and compares. `--skip-build` mode (test-all tier 5, used in CI for
speed) hashes the committed `crates/fjell-kernel/prebuilt/*.bin` against the
committed `tests/repro/baseline-digests.txt` and **fails closed** if that
baseline is absent. The earlier statement here — "the repro baseline is
re-recorded per run by design" — described a fail-open bug (a missing
baseline silently recorded itself and reported PASS), not a design. Recording
a new baseline now requires the explicit `--record-baseline` flag and is
never a side effect of a check run.
