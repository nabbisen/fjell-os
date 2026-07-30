# Fjell OS — External Design (Architect Handoff)

*Compact design handoff. Version: v0.21.1.*

## 1. Design goal

The design delivers a minimal trusted kernel that enforces capability and
memory isolation while pushing all policy into user-space services. The main
design problem is keeping the trusted computing base small and reasoning-friendly
while still supporting a useful fleet-node service plane. The primary consumers
are user-space service authors (against `fjell-sdk`) and fleet operators. The
constraints inherited from requirements are: no ambient authority,
`forbid(unsafe_code)` outside audited boundaries, reproducible builds, and a
verifiable capability/lease core. Non-goals that prevent scope creep: no POSIX
surface, no kernel heap, no multi-hart in v1.0, no in-kernel policy decisions.

## 2. External design

**Syscall surface (the kernel's public API).** `fjell-abi` declares 35
syscall numbers; **26 are dispatched** in eight groups: IPC
(call/recv/reply/send/try-recv, synchronous rendezvous), capability
management (copy/mint/delete/revoke/drop/inspect/bind-lease), lease lifecycle
(create/revoke/inspect), task management (spawn/start/status), hardware access
(mmio-map, dma-alloc/revoke — each gated by a capability naming the resource),
platform (info-get), audit (drain), and scheduler (yield, exit). The
remaining 9 (`cap_install`, `irq_bind`/`irq_wait`/`irq_ack`, `reboot`,
`task_kill`, `mmio_unmap`, `dma_share`, and a second reboot number) are
declared but not dispatched — see
[Kernel](../../external-design/kernel.md) §2. The register-level contract is
normative in `docs/src/abi/ipc-register-layout.md` (a0 status/handle, a1 packed tag,
a2–a5 message words, a6 kernel-attested sender identity, a7 syscall number).

**Major components.** The kernel (`fjell-kernel`) holds the capability system,
lease table, IPC state machine, scheduler, MM, and trap/syscall dispatch. The
service plane is 29 user-space RISC-V programs (`crates/services/`): the `*d`
daemons (storaged, auditd, measuredd, verifyd, recoveryd, etc.) plus bootstrap
and test programs. Data schemas are 22 `*-format` crates (`crates/formats/`),
pure definitions depended on by 47 crates. Architecture abstraction is the
`arch/` group (trait + riscv64 impl + arm64 stub). Drivers are `drivers/`
(virtio-blk, virtio-net). Library/infra crates (cap, ipc, syscall, abi, crypto,
sdk, tools) stay flat under `crates/`.

**Data flow / lifecycle.** Boot → kernel init (frame allocator, page tables,
capability bootstrap) → spawn `init` → init spawns the service plane via the
cap-broker → services rendezvous over IPC endpoints. Authority flows by minting
capabilities with strictly-subset rights and binding them to leases; revoking a
lease advances its epoch and cancels all in-flight IPC bound to it.

**Error model.** Typed `SysError` returned in a0; `CapError` mapped through
`to_sys_error()` (canonical) — `WrongKind → WrongType` unified at v0.20.1.
Default-deny everywhere: absence of a capability yields `PermissionDenied` with
no observation of the resource.

**Compatibility.** ABI stability is enforced by a snapshot gate over `fjell-abi`
(401 tracked items at v0.21.1); removals fail the gate. Changes require an RFC
with an architect decision record.

## 3. Requirements coverage

| Requirement | Design response | Evidence / source |
|---|---|---|
| Capability authority never amplified | Mint path enforces `is_subset_of`; Verus-proved predicate | `crates/fjell-cap/src/rights.rs`; Verus capability target |
| Revocation bounded and atomic | Lease epoch monotonic; reply-edge cancellation on revoke | `crates/fjell-kernel/src/lease/`; Verus lease target |
| No ambient authority | Every syscall capability-gated; default deny | `crates/fjell-kernel/src/trap/syscall.rs` |
| Signed bundles | Ed25519 verify before execution | `fjell-sig-ed25519`, `fjell-bundle-format` |
| Audited unsafe only | `forbid(unsafe_code)` + classified boundaries gate | `docs/src/verification/unsafe-charter.md` |
| Reproducible build | Two-build digest comparison | `tests/repro/`, `fjell-repro-check` |

## 4. Key tradeoffs and decisions

| ID | Decision | Alternatives considered | Rationale | Risk / consequence |
|---|---|---|---|---|
| TR-01 | Selective Verus (cap + lease predicates only) | Verify whole kernel; verify nothing | Prove the security-critical pure logic; test the plumbing | IPC/service-manager bugs are not proof-caught (v0.20 IPC defect is the cautionary example) |
| TR-02 | User-space service plane, micro-kernel | Monolithic kernel | Small TCB; policy out of kernel | More IPC; more boot orchestration |
| TR-03 | Crate subdirectory grouping | Keep 80 flat crates | Navigability | Relative path deps; one-time migration |
| TR-04 | Pure-Rust disk-image creation | Depend on `qemu-img` | Portability (Arch ships qemu-img separately) | None observed |
| TR-05 | No kernel heap; fixed-capacity tables | General allocator | Reasoning + future verification | Capacity limits are compile-time constants |

## 5. Rust design notes

- **Workspace/crate boundary:** explicit member list (no globs); four role
  subdirectories (`arch`, `drivers`, `formats`, `services`) plus flat
  library/infra crates. Crate names are independent of path.
- **Module style:** Rust 2024, no `mod.rs` (a `foo.rs` + `foo/` subdir
  coexist). Test modules are separated: `src/x.rs` ↔ `src/x/tests.rs`.
- **Error strategy:** typed enums (`SysError`, `CapError`, per-format errors);
  no stringly-typed errors across boundaries.
- **Unsafe policy:** `#![forbid(unsafe_code)]` workspace-wide except kernel and
  arch crates, where each `unsafe` site carries a categorised `// SAFETY:`
  comment enforced by the unsafe-audit gate. MMIO writes additionally carry an
  `MMIO-ORDER:` annotation.
- **Async/runtime:** none — bare-metal `no_std`, cooperative scheduling.

## 6. Known design gaps

| Gap | Why it remains | Impact | Proposed owner / follow-up |
|---|---|---|---|
| DMA user-VA unmap deferred | Page-table corruption under v0.8.x not yet root-caused | Stale PTE post-revoke; mitigated by zeroize-before-reuse | Implementer, post-v1.0 |
| Store/upgrade negative emitters absent | Late-stage v1 scope control | No runtime evidence for two paths | Implementer, v1.1 |
| svc READY negative pair timing-sensitive | Rendezvous timing not yet deterministic in test harness | Partial lifecycle coverage (2/4) | QA, v1.1 |
| End-to-end provision/sign/verify not gated | Signing-side coupling is operational, not enforced | Operator must match keys manually | Architect, v1.1 |
| arm64 arch crate is a stub | Single-target focus for v1.0 | No ARM build | Deferred post-v1.0 |
