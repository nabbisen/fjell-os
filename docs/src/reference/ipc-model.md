# IPC Model

> Implemented in **M3**.

Synchronous rendezvous endpoints (L4/seL4 style).

## Operations

| Syscall | Blocks caller? |
|---|---|
| `ipc_send` | Yes (until receiver ready) |
| `ipc_recv` | Yes (until sender ready) |
| `ipc_call` | Yes (until reply) |
| `ipc_reply` | No |

See ADR-0010 (added in M3) for design rationale.