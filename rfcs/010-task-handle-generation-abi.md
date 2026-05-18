# RFC 010: Include generation in task handle ABI

**RFC ID:** 010  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/trap/syscall.rs`,
             `crates/fjell-syscall/src/lib.rs`,
             `crates/fjell-init/src/main.rs`

## Problem (H-06)

`sys_task_spawn` returns only `tid.index` as the task handle.
`sys_task_start` / `sys_task_status` use `TaskId::new(handle, 0)` — generation=0.
When a task slot is reused (task exits and a new task is spawned), the old index
still works as a valid handle for the new task.

## Proposed fix

Encode `(index, generation)` into a single u32 handle:

```rust
// encode: handle = (index as u32) | ((generation as u32) << 16)
// decode: index = handle & 0xFFFF;  generation = (handle >> 16) as u16;
pub fn encode_task_handle(index: u16, generation: u16) -> u32 {
    (index as u32) | ((generation as u32) << 16)
}
```

In `sys_task_spawn`:
```rust
let handle = encode_task_handle(tid.index, tid.gen);
tf.gpr[REG_A1] = handle as usize;
```

In `sys_task_start` / `sys_task_status`:
```rust
let raw    = tf.gpr[REG_A0] as u32;
let index  = (raw & 0xFFFF) as u16;
let gen    = (raw >> 16) as u16;
let tid    = TaskId::new(index, gen);
```

In `fjell-syscall`:
```rust
pub fn sys_task_spawn(image_id: ImageId) -> Result<(u32, usize), SysError> {
    // returns (task_handle: u32, _)
}
pub fn sys_task_start(task_handle: u32, entry: usize, stack: usize) -> Result<(), SysError> { ... }
```

## Impact

| Crate | Change |
|---|---|
| `fjell-kernel/src/trap/syscall.rs` | encode/decode handle in task syscalls |
| `fjell-syscall/src/lib.rs` | update sys_task_spawn/start/status signatures |
| `fjell-init/src/main.rs` | update spawn() helper to pass u32 handle |

## Test plan

1. Spawn two tasks sequentially (second reuses slot of first after first exits).
2. Attempt to start the second task using the first task's handle → InvalidCap.
3. `cargo xtask qemu-test m7` passes.
