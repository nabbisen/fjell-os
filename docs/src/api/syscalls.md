# Syscall Reference

*A catalogue **kept equal to `SyscallNumber`** by the `syscall-doc` subcheck (`cargo run -p fjell-consistency-check -- syscall-doc`):
every declared syscall has exactly one row below, with the number the enum gives it, and every wrapper a row names exists in
`fjell-syscall`. See `cargo doc --no-deps -p fjell-syscall` for full wrapper signatures and doc-comments.*

Every declared syscall is dispatched, or it is retired (RFC-0.49-002); `syscall-surface` keeps that true of the kernel's dispatch
table, and the table below is the catalogue of what is declared. This page states no count of its own: the one it used to carry
(*"dispatches 26 of them … the remaining 9 are not dispatched"*) was true when written, was never read by anything that checks, and was
wrong about its own contents for twenty milestones (RFC-0.50-002).

| # | Syscall number | `fjell-syscall` wrapper(s) | Group |
|---|---|---|---|
| 0 | `Yield` | `sys_yield` | Scheduler / lifecycle |
| 1 | `Exit` | `sys_exit` | Scheduler / lifecycle |
| 2 | `DebugWrite` | `sys_debug_write_byte` (also reached via `sys_debug_write`, `sys_debug_writeln`) | Debug (dev only) |
| 10 | `CapCopy` | `sys_cap_copy` | Capability |
| 11 | `CapMint` | `sys_cap_mint` | Capability |
| 12 | `CapDelete` | `sys_cap_delete` (a wrapper since 0.47) | Capability |
| 13 | `CapRevoke` | `sys_cap_revoke` | Capability |
| 14 | `CapInspect` | `sys_cap_inspect` | Capability |
| 15 | `CapDrop` | `sys_cap_drop` | Capability |
| 16 | `CapBindLease` | `sys_cap_bind_lease` (needs an `Active` `LeaseAdmin` cap with `LEASE_CREATE`, and an `Active` lease) | Capability |
| 17 | `CapInstall` | `sys_cap_install` (installs `Endpoint` capabilities only, RFC-0.45-002) | Capability |
| 18 | `PlatformReboot` | `sys_reboot` (needs a `Reboot` capability carrying `REBOOT`) | Platform |
| 20 | `IpcSend` | `sys_ipc_send` | IPC |
| 21 | `IpcRecv` | `sys_ipc_recv` (also reached via `sys_ipc_recv_msg`) | IPC |
| 22 | `IpcCall` | `sys_ipc_call` (also reached via `sys_ipc_call_words`) | IPC |
| 23 | `IpcReply` | `sys_ipc_reply` | IPC |
| 24 | `IpcTryRecv` | `sys_ipc_try_recv` | IPC |
| 40 | `TaskSpawn` | `sys_task_spawn` | Task |
| 41 | `TaskStart` | `sys_task_start` | Task |
| 42 | `TaskStatus` | `sys_task_status` | Task |
| 43 | `TaskKill` | `sys_task_kill` (needs a `TaskControl` capability carrying `TASK_KILL`; destroys the task and frees its slot, RFC-0.49-001) | Task |
| 50 | `LeaseCreate` | `sys_lease_create` | Lease |
| 51 | `LeaseRevoke` | `sys_lease_revoke` | Lease |
| 52 | `LeaseInspect` | `sys_lease_inspect` | Lease |
| 60 | `AuditDrain` | `sys_audit_drain` | Audit |
| 61 | `AuditWait` | `sys_audit_wait` | Audit |
| 80 | `PlatformInfoGet` | `sys_platform_info_get` | Platform |
| 90 | `MmioMap` | `sys_mmio_map` | Hardware |
| 91 | `MmioUnmap` | `sys_mmio_unmap` (the holder's own act; whole mapping, verified, RFC-0.49-002) | Hardware |
| 100 | `IrqBind` | `sys_irq_bind` (takes an `Interrupt` capability) | Hardware |
| 101 | `IrqAck` | `sys_irq_ack` | Hardware |
| 102 | `IrqWait` | `sys_irq_wait` | Hardware |
| 110 | `DmaAlloc` | `sys_dma_alloc` | Hardware |
| 112 | `DmaRevoke` | `sys_dma_revoke` | Hardware |
| 113 | `ClockNow` | `sys_clock_now_ns` (needs a `Clock` cap with `CLOCK_READ`) | Platform |

Notes:

- `CapDelete` (12) is routed through `dispatch_m3` in `trap/syscall.rs`; `fjell-syscall` has exposed `sys_cap_delete` since 0.47 (until
  then a caller had to issue the raw `ecall`, which is what this page said at v0.21.3 and went on saying).
- `IpcSend` (20) is one-way **rendezvous** send: it blocks the caller until
  a receiver takes the message (RFC-0.27-002, closes E-022 — `sys_ipc_send`
  was named `sys_ipc_try_send` and documented as non-blocking until then,
  which the kernel never implemented).
- `AuditWait` (61) returns `Ok` in `a0` and, in `a1`, `1` if it parked the caller or `0`
  if it returned at once because the audit ring was already non-empty (RFC-0.41-001 R11).
  `a1` is a plain word, meaningful only when `a0` is `Ok`.
- `ClockNow` (113) returns `Ok` in `a0` and, in `a1`, a monotone count of **nanoseconds**
  (RFC-0.42-002). The origin is not defined by the ABI: only the difference between two
  readings is meaningful. The resolution is the platform counter's tick (100 ns on QEMU `virt`).
  It is a clock to read -- there is no sleep or wait-until, since blocking on time needs the
  timer interrupt, which stays masked. `a1` is a plain word, meaningful only when `a0` is `Ok`.
- Every syscall except `DebugWrite` is capability-gated; the result is
  returned in `a0` as a typed `SysError` (or `Ok`). See
  [IPC Register Layout](../abi/ipc-register-layout.md) for the full
  register-level ABI on the IPC syscalls specifically.
