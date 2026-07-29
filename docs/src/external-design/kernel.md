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

The kernel's boundary is the **syscall surface**: 38 syscalls dispatched from
`trap/syscall.rs` via the RISC-V `ecall` convention. The register-level contract
is normative in [IPC Register Layout](../abi/ipc-register-layout.md); the
syscall catalog is in [Syscall Reference](../api/syscalls.md).

Groups (as-built at v0.21.2):

| Group | Syscalls |
|---|---|
| Scheduler / lifecycle | `yield`, `exit` |
| IPC | `ipc_call`, `ipc_call_words`, `ipc_recv`, `ipc_recv_msg`, `ipc_reply`, `ipc_try_send`, `ipc_try_recv` |
| Capability | `cap_copy`, `cap_mint`, `cap_revoke`, `cap_drop`, `cap_install`, `cap_install_with_rights`, `cap_inspect`, `cap_bind_lease` |
| Lease | `lease_create`, `lease_revoke`, `lease_inspect` |
| Task | `task_spawn`, `task_start`, `task_status` |
| Hardware | `mmio_map`, `dma_alloc`, `dma_revoke`, `irq_bind`, `irq_wait`, `irq_ack` |
| Platform | `reboot`, `platform_info_get`, `platform_region_resolve` |
| Audit | `audit_drain` |
| Debug (dev only) | `debug_write`, `debug_writeln`, `debug_write_byte` |

Every syscall except the debug helpers is capability-gated. The result is
returned in `a0` as a typed `SysError` (or `Ok`).

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
- **Interrupt syscalls** (`irq_bind/wait/ack`) exist; full driver interrupt
  routing is exercised by virtio drivers but not all device classes.

## 7. Known gaps

| Gap | Impact | Follow-up |
|---|---|---|
| Power-state coupled scheduling (FR-KRN-006) is telemetry-only | No active suspend/resume scheduling | post-v1.0 |
| DMA user-VA unmap bypassed in `revoke_by_pa` | Stale PTE, mitigated by zeroize-before-reuse | post-v1.0 |
| Multi-hart | No SMP | post-v1.0 milestone |
