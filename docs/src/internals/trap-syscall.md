# Trap and Syscall

> Implemented in **M2**.  This page describes the target design.

## Trap path (three layers)

```
1. Assembly trap entry  (stvec, direct mode)
   - CSRRW sscratch ↔ scratch register for working space
   - Save all 32 GPRs + CSRs into TrapFrame on kernel stack
   - Switch to kernel stack
   - Call Rust trap_dispatch(tf: &mut TrapFrame)

2. Rust trap dispatch
   - Read scause
   - Branch: UserEcall | SupervisorTimer | PageFault | IllegalInsn | Other

3. Handler
   - Syscall handler  →  sys_yield / sys_exit / sys_debug_write
   - Timer handler    →  scheduler.tick()
   - Fault handler    →  mark_task_faulted(), schedule_next()
```

## Syscall calling convention

| Register | Role |
|---|---|
| `a7` | Syscall number |
| `a0`–`a5` | Arguments |
| `a0` (return) | Status (`0` = OK, negative = `SysError`) |
| `a1`–`a3` (return) | Optional return values on success |

After `ecall` dispatch, `sepc` is advanced by 4 before `sret`.

## M2 syscall numbers

| Number | Name | Description |
|---|---|---|
| 0 | `sys_yield` | Voluntarily relinquish the CPU |
| 1 | `sys_exit` | Terminate the calling task |
| 2 | `sys_debug_write` | Write bytes to UART (smoke-test only, removed in production ABI) |

IPC and capability syscalls are added in M3.

## Fault containment

User faults do **not** panic the kernel:

```
user page fault / illegal instruction / unknown syscall
    → FaultInfo { cause, sepc, stval }
    → task.state = TaskState::Faulted(info)
    → audit_ring.append(AuditKind::TaskFault, …)
    → schedule_next()          // switch to another task or idle
```

Kernel-mode faults are treated as unrecoverable and panic.

## Invariants

| ID | Invariant |
|---|---|
| TRAP-001 | `sepc` is incremented by 4 after every `ecall`. |
| TRAP-002 | Unknown syscall number → `SysError::UnknownSyscall`; no panic. |
| TRAP-003 | User page fault → `TaskState::Faulted`; no panic. |
| TRAP-004 | Kernel page fault → panic (legitimate). |
| TRAP-005 | `sepc` on `sret` is verified to be within canonical user VA range. |
| TRAP-006 | `sstatus.SPP` is `User` before every `sret` to user mode. |
