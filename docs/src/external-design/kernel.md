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

The kernel's boundary is the **syscall surface**. `fjell-abi` declares the syscall numbers and
`trap/syscall.rs` dispatches every one of them via the RISC-V `ecall` convention. The register-level
contract is normative in [IPC Register Layout](../abi/ipc-register-layout.md); the syscall catalog
is in [Syscall Reference](../api/syscalls.md). This section states no count: the one it carried
(*"dispatches 26 of them"*) was stale for twenty milestones, and the table below is kept equal to
`SyscallNumber` by the `syscall-doc` subcheck instead (RFC-0.50-002).

Dispatched groups (every declared syscall, by wrapper name; kept equal to `SyscallNumber`):

| Group | Syscalls |
|---|---|
| Scheduler / lifecycle | `yield`, `exit` |
| IPC | `ipc_call` (also reached via `ipc_call_words`), `ipc_recv` (also reached via `ipc_recv_msg`), `ipc_reply`, `ipc_send`, `ipc_try_recv` |
| Capability | `cap_copy`, `cap_mint`, `cap_delete`, `cap_revoke`, `cap_drop`, `cap_inspect`, `cap_bind_lease`, `cap_install` |
| Lease | `lease_create`, `lease_revoke`, `lease_inspect` |
| Task | `task_spawn`, `task_start`, `task_status`, `task_kill` |
| Hardware | `mmio_map`, `mmio_unmap`, `dma_alloc`, `dma_revoke`, `irq_bind`, `irq_ack`, `irq_wait` |
| Platform | `platform_info_get`, `platform_reboot`, `clock_now` |
| Audit | `audit_drain`, `audit_wait` |
| Debug (dev only) | `debug_write` |

Every syscall except the debug helper is capability-gated. The result is
returned in `a0` as a typed `SysError` (or `Ok`).

### Declared, not dispatched (none)

**The rule since RFC-0.49-002: every declared syscall is dispatched, or it is retired.**
`cargo run -p fjell-consistency-check -- syscall-surface` **fails** on a `SyscallNumber`
variant with no dispatch arm in `trap/syscall.rs` (the fallthrough `Some(_) | None =>
SysError::UnknownSyscall` is what such a call would reach). The one exception the gate can
express is a *held* variant -- named in `tests/syscall/expected.toml` with a reason and an
expiry, failing once the workspace version is past it. Nothing is held today.

Three syscalls were declared and undispatched until 0.49, and each was resolved the way the rule
allows:

| Syscall | Number | How it ended |
|---|---|---|
| `TaskKill` | 43 | **dispatched** (`fjell_syscall::sys_task_kill`, RFC-0.49-001) |
| `MmioUnmap` | 91 | **dispatched** (`fjell_syscall::sys_mmio_unmap`, RFC-0.49-002) |
| `DmaShare` | 111 | **retired**: the variant is gone and the number is not reused, as `Reboot` (120) was not (RFC-0.49-002 D6) |

`DmaShare` was mis-specified rather than merely unfinished: it routed *inter-service messaging*
through *device DMA memory*, and a `SemanticEnvelope` between two services has nothing to do with
DMA. Retiring it removed an advertised mechanism that did not exist; it did **not** remove the
requirement it stood for -- bulk transfer between services still has no designed transport (the
chunked path is a demonstration-scale workaround, costing up to 147 round trips per transfer;
see [IPC](./ipc.md) §7).

`CapInstall` (17) was the fourth until RFC-0.45-002: it is dispatched now, as
`sys_cap_install` (see [Capability and lease](./capability-lease.md)).

`sys_platform_region_resolve` is a separate case: it is an explicit
host-side stub that returns `UnknownSyscall` without issuing an `ecall` at
all, so it never reaches the kernel.

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
- **Interrupt syscalls are dispatched and in use.** `irq_bind`/`irq_ack`/`irq_wait` have dispatch arms in
  `trap/syscall.rs`, and `driver-uart` binds, waits and acks in a live loop (its console RX path); the
  `uart-rx` and `uart-rx-unbound` negative categories exercise exactly that path. `driver-virtio-net`
  calls none of them: it is a one-shot transport scan that maps the virtio-mmio region, reports, and exits
  (`docs/src/releasing/v1-limitations.md`). *(Corrected at 0.50: this bullet read "Interrupt syscalls are
  not dispatched", cited §2 as its authority, and named `driver-virtio-net` as the caller of `sys_irq_bind`.
  The arms were added by RFC-0.25-001; the paragraph was false from that milestone, and false in all three
  clauses.)*

## 7. Known gaps

| Gap | Impact | Follow-up |
|---|---|---|
| Power-state coupled scheduling (FR-KRN-006) is telemetry-only | No active suspend/resume scheduling | post-v1.0 |
| ~~DMA user-VA unmap bypassed in `revoke_by_pa`~~ **closed 2026-10-28 (RFC-0.39-004, E-099)**: the unmap is performed and verified before the frame is zeroed or freed. *This row said "Stale PTE, mitigated by zeroize-before-reuse" — zeroing did not mitigate it: a task kept `R\|W\|U` access to a frame the allocator then gave to another task, from v0.8.x through 0.38.0.* | none — see `docs/src/internals/memory-model.md` MM-PHY-008 | — |
| Multi-hart | No SMP | post-v1.0 milestone |
