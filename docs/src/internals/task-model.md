# Task Model

> Implemented in **M2**.  This page describes the target design.

## Task Control Block (TCB)

Each task holds two register save areas:

- **`TrapFrame`** — all 32 GPRs plus `sstatus`, `sepc`, `scause`, `stval`.
  Saved/restored on every kernel entry and `sret`.
- **`KernelContext`** — callee-saved registers (`s0–s11`, `ra`, `sp`) used
  only for kernel-to-kernel context switches.

This separation means the trap path only touches `TrapFrame` (keeping it
short), while context switches only touch `KernelContext`.

## Task states

```
Empty → Created → Runnable ⇄ Running → Exited(i32)
                                     → Blocked(reason)
                                     → Faulted(FaultInfo)
```

| Transition | Trigger |
|---|---|
| `Created → Runnable` | Task fully initialised and enqueued |
| `Runnable → Running` | Scheduler picks this task |
| `Running → Runnable` | `sys_yield` or timer interrupt |
| `Running → Blocked` | IPC wait (M3+) |
| `Running → Faulted` | User page fault, illegal instruction, unknown syscall |
| `Running → Exited` | `sys_exit` |
| `Blocked → Runnable` | IPC delivered (M3+) |

`Faulted` and `Exited` are terminal.  The scheduler never re-enqueues them.

## Scheduler (M2)

Single-hart fixed-priority round-robin.

- 8 priority buckets; within a bucket, tasks run round-robin.
- Default user priority: `PRIORITY_USER = 32`.
- Idle task priority: `PRIORITY_IDLE = 0`; runs `wfi` in a loop.
- Preemption via CLINT timer interrupt (optional in early M2 bring-up;
  `sys_yield` is always required).

## Invariants

| ID | Invariant |
|---|---|
| TASK-003 | At most one task is in `Running` state at any time. |
| TASK-004 | Only `Runnable` tasks appear in the ready queue. |
| TASK-005 | `Faulted` and `Exited` tasks are never re-enqueued. |
| TASK-006 | Each task owns exactly one `AddressSpace`. |
| SCHED-001 | The same `TaskId` never appears twice in the ready queue. |
| SCHED-005 | The idle task never reaches `Exited`. |
