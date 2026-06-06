# RFC 019: sys_ipc_try_recv + cooperative service loop + service separation

**RFC ID:** 019  
**Status:** Implemented (v0.1.0)
**Affects:** kernel syscall, fjell-service-api, all M6/M7 service stubs

## Problem (RB-06, architect recommendation 5.4)

Services are inline in fjell-init because spawned services cannot signal readiness
without blocking on recv (which deadlocks without preemption).

The architect recommends: `sys_ipc_try_recv` (non-blocking) + cooperative service
loop as an interim solution before M8 preemptive scheduler.

## Proposed fix

### sys_ipc_try_recv

```
sys_ipc_try_recv(a0=endpoint_handle) -> a0=status
```

Returns:
- `SysError::Ok` with message in a1–a5 if a message is waiting
- `SysError::WouldBlock` (new error code) if no message pending
- `SysError::InvalidCap` if handle invalid

Implementation: check `EndpointTable::try_recv()` without blocking.

### sys_yield

```
sys_yield() — voluntarily give up the CPU to the next runnable task
```

### Cooperative service loop pattern

```rust
// Each M6/M7 service:
loop {
    match sys_ipc_try_recv(ep) {
        Ok(msg)         => handle_request(msg),
        WouldBlock      => sys_yield(),
        InvalidCap | _  => break,
    }
}
```

### Startup protocol

1. Service calls `sys_ipc_send(ready_ep, READY_MSG)` when initialized.
2. init polls `try_recv(ready_ep)` after each spawn.
3. Only advances to next spawn when ready signal received.

## Defer condition

Requires `sys_yield` syscall + EndpointTable non-blocking path.
This is the prerequisite for RFC 011 service separation.
