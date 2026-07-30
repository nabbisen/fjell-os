# External Design — Kernel

*Subsystem 1 of 9. Anchored to FR-KRN-001…007 and the `fjell-kernel` crate at
v0.21.2.*

## 1. Responsibility

The kernel is the only component that runs in RISC-V S-mode. Its responsibility
is deliberately narrow (FR-KRN-001): CPU initialization, minimal memory
management, address-space separation, task management, scheduling, IPC,
capability management, interrupt management, minimal device abstraction, and
starting the initial services at boot. Everything else — drivers, file systems,
network, audit, configuration — is a user-space service.

Non-goals inherited from the requirements: no in-kernel file system, network
stack, GUI stack, font rendering, dynamic plugins, or app-compatibility layer
(NFR-SEC-002). No kernel heap.

## 2. External surface

The kernel's boundary is the **syscall surface**. `fjell-abi` declares 35
syscall numbers; `trap/syscall.rs` dispatches **26** of them via the RISC-V
`ecall` convention. The register-level contract is normative in
[IPC Register Layout](../abi/ipc-register-layout.md); the syscall catalog is
in [Syscall Reference](../api/syscalls.md).

Dispatched groups (as-built at v0.21.2, 26 syscalls):

| Group | Syscalls |
|---|---|
| Scheduler / lifecycle | `yield`, `exit` |
| IPC | `ipc_call` (also reached via `ipc_call_words`), `ipc_recv` (also reached via `ipc_recv_msg`), `ipc_reply`, `ipc_send` (also reached via `ipc_try_send`), `ipc_try_recv` |
| Capability | `cap_copy`, `cap_mint`, `cap_delete`, `cap_revoke`, `cap_drop`, `cap_inspect`, `cap_bind_lease` |
| Lease | `lease_create`, `lease_revoke`, `lease_inspect` |
| Task | `task_spawn`, `task_start`, `task_status` |
| Hardware | `mmio_map`, `dma_alloc`, `dma_revoke` |
| Platform | `platform_info_get` |
| Audit | `audit_drain` |
| Debug (dev only) | `debug_write` |

Every syscall except the debug helper is capability-gated. The result is
returned in `a0` as a typed `SysError` (or `Ok`).

### Declared, not dispatched (9 of the 35)

These `SyscallNumber` variants exist in `fjell-abi` and have user-space
wrapper functions in `fjell-syscall`, but `trap/syscall.rs` has no dispatch
arm for them — the fallthrough (`Some(_) | None => SysError::UnknownSyscall`,
`syscall.rs:52`) rejects every one. Calling any of them from user space
returns `UnknownSyscall`, not the behaviour their wrapper's doc-comment may
suggest:

| Syscall number | Wrapper(s) that issue it |
|---|---|
| `CapInstall` (17) | `sys_cap_install`, `sys_cap_install_with_rights` |
| `PlatformReboot` (18) | `sys_reboot` |
| `TaskKill` (43) | — (no wrapper) |
| `MmioUnmap` (91) | — (no wrapper) |
| `IrqBind` (100) | `sys_irq_bind` |
| `IrqAck` (101) | `sys_irq_ack` |
| `IrqWait` (102) | `sys_irq_wait` |
| `DmaShare` (111) | — (no wrapper) |
| `Reboot` (120) | — (no wrapper; distinct from `PlatformReboot`) |

`sys_platform_region_resolve` is a separate case: it is an explicit
host-side stub that returns `UnknownSyscall` without issuing an `ecall` at
all, so it never reaches the kernel.

The disposition of these 9 — implement, remove from the ABI, or keep
permanently reserved — is not decided; see
`rfcs/proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`
§Deferred.

## 3. Internal module boundaries (as-built)

`crates/fjell-kernel/src/`:

| Module | Responsibility |
|---|---|
| `mm/` | Boot allocator, frame allocator, Sv39 page tables, user-copy |
| `trap/` | Trap entry, syscall dispatch, fault handling |
| `task/` | TCB table, scheduler, task lifecycle |
| `cap/` | In-kernel capability space (CSpace) management |
| `lease/` | Lease table, epoch revocation, reply-edge cancellation |
| `audit/` | Kernel audit ring buffer |
| `arch/` | RISC-V CSR access, `sfence`, trap vectors (delegated to `fjell-arch-riscv64`) |
| `platform/` | QEMU `virt` platform parameters, MMIO map |
| `boot.rs` | Boot info, capability bootstrap slots |
| `console.rs`, `uart.rs` | Kernel console (single-hart spinlock) |

## 4. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-KRN-001 Minimal microkernel | Kernel limited to the ten listed duties; services in user space | `fjell-kernel` module list above |
| FR-KRN-002 Address-space separation | Per-task Sv39 page table; cross-task memory requires a capability + IPC | `mm/page_table.rs`, `task/tcb.rs` (`satp_root_pfn`) |
| FR-KRN-003 Capability management | Unforgeable typed capabilities; see [Capability & Lease](./capability-lease.md) | `cap/`, `fjell-cap` |
| FR-KRN-004 IPC | Synchronous rendezvous, capability-checked; see [IPC](./ipc.md) | `trap/syscall.rs`, `fjell-ipc` |
| FR-KRN-005 Scheduling | Cooperative single-hart scheduler with task state | `task/` scheduler |
| FR-KRN-006 Power-state coupling | Power telemetry surfaced via `powerd`; kernel idle path | `platform/`, `fjell-powerd` (skeleton) |
| FR-KRN-007 Hardware protection | Sv39 memory protection; PMP/IOMMU integration point | `arch/`, `mm/` |
| NFR-VER-003 Simple state transitions | Capability/lease/IPC/boot transitions kept explicit and verifiable | Verus targets (capability, lease) |
| NFR-PERF-003 Lightweight boot | Minimal components loaded at boot | `boot.rs`, minimal init set |

## 5. Error model

All syscalls return a typed `SysError` in `a0`. Capability errors are mapped
through the canonical `to_sys_error()` in `fjell-cap/src/rights.rs`
(`WrongKind → WrongType` unified at v0.20.1). Default-deny is universal: a task
lacking the relevant capability receives `PermissionDenied` and cannot observe
that the resource exists (NFR-SEC-001).

## 6. As-built scope limits (v1.0 QEMU profile)

- **Single-hart only.** SMP scheduling, per-hart locking, and IPIs are deferred
  (requirements limitation item 2). The console spinlock's single-hart invariant
  is noted in `console.rs`.
- **Scheduling is cooperative**, not the full power-state-coupled scheduler of
  FR-KRN-005/FR-KRN-006; `powerd` provides telemetry but not active power-state
  scheduling yet.
- **Interrupt syscalls are not dispatched.** `irq_bind`/`irq_ack`/`irq_wait`
  are declared in `fjell-abi` and have user-space wrappers, but
  `trap/syscall.rs` has no dispatch arm for any of them (see §2, "Declared,
  not dispatched"); calling one returns `UnknownSyscall`. `driver-virtio-net`
  calls `sys_irq_bind`, but it is a documented early-exit stub
  (`docs/release/v1-limitations.md`), so this is not exercised as a live
  interrupt path at v1.0.

## 7. Known gaps

| Gap | Impact | Follow-up |
|---|---|---|
| Power-state coupled scheduling (FR-KRN-006) is telemetry-only | No active suspend/resume scheduling | post-v1.0 |
| DMA user-VA unmap bypassed in `revoke_by_pa` | Stale PTE, mitigated by zeroize-before-reuse | post-v1.0 |
| Multi-hart | No SMP | post-v1.0 milestone |
