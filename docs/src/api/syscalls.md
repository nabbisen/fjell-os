# Syscall Reference

*Authoritative as-built catalog at v0.21.3. See `cargo doc --no-deps -p
fjell-syscall` for full wrapper signatures and doc-comments.*

`fjell-abi` declares 35 `SyscallNumber` values. `trap/syscall.rs` dispatches
**26** of them. The remaining 9 are declared and have user-space wrappers but
are not dispatched — calling one returns `UnknownSyscall`; see
[Kernel](../external-design/kernel.md) §2 "Declared, not dispatched" for the
full list and disposition. This page lists only the 26 that are live.

| # | Syscall number | `fjell-syscall` wrapper(s) | Group |
|---|---|---|---|
| 0 | `Yield` | `sys_yield` | Scheduler / lifecycle |
| 1 | `Exit` | `sys_exit` | Scheduler / lifecycle |
| 2 | `DebugWrite` | `sys_debug_write_byte` (also reached via `sys_debug_write`, `sys_debug_writeln`) | Debug (dev only) |
| 10 | `CapCopy` | `sys_cap_copy` | Capability |
| 11 | `CapMint` | `sys_cap_mint` | Capability |
| 12 | `CapDelete` | — (dispatched; no `fjell-syscall` wrapper at v0.21.3) | Capability |
| 13 | `CapRevoke` | `sys_cap_revoke` | Capability |
| 14 | `CapInspect` | `sys_cap_inspect` | Capability |
| 15 | `CapDrop` | `sys_cap_drop` | Capability |
| 16 | `CapBindLease` | `sys_cap_bind_lease` | Capability |
| 20 | `IpcSend` | `sys_ipc_send` | IPC |
| 21 | `IpcRecv` | `sys_ipc_recv` (also reached via `sys_ipc_recv_msg`) | IPC |
| 22 | `IpcCall` | `sys_ipc_call` (also reached via `sys_ipc_call_words`) | IPC |
| 23 | `IpcReply` | `sys_ipc_reply` | IPC |
| 24 | `IpcTryRecv` | `sys_ipc_try_recv` | IPC |
| 40 | `TaskSpawn` | `sys_task_spawn` | Task |
| 41 | `TaskStart` | `sys_task_start` | Task |
| 42 | `TaskStatus` | `sys_task_status` | Task |
| 50 | `LeaseCreate` | `sys_lease_create` | Lease |
| 51 | `LeaseRevoke` | `sys_lease_revoke` | Lease |
| 52 | `LeaseInspect` | `sys_lease_inspect` | Lease |
| 60 | `AuditDrain` | `sys_audit_drain` | Audit |
| 80 | `PlatformInfoGet` | `sys_platform_info_get` | Platform |
| 90 | `MmioMap` | `sys_mmio_map` | Hardware |
| 110 | `DmaAlloc` | `sys_dma_alloc` | Hardware |
| 112 | `DmaRevoke` | `sys_dma_revoke` | Hardware |

Notes:

- `CapDelete` (12) is a live dispatch arm in `trap/syscall.rs` (routed through
  `dispatch_m3`), but `fjell-syscall` does not currently expose a
  `sys_cap_delete` wrapper; a caller would need to issue the raw `ecall`.
- `IpcSend` (20) is one-way **rendezvous** send: it blocks the caller until
  a receiver takes the message (RFC-0.27-002, closes E-022 — `sys_ipc_send`
  was named `sys_ipc_try_send` and documented as non-blocking until then,
  which the kernel never implemented).
- Every syscall except `DebugWrite` is capability-gated; the result is
  returned in `a0` as a typed `SysError` (or `Ok`). See
  [IPC Register Layout](../abi/ipc-register-layout.md) for the full
  register-level ABI on the IPC syscalls specifically.
