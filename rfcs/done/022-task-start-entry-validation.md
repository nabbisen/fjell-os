# RFC 022: sys_task_start entry_pc / stack_top user range validation

**RFC ID:** 022  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/src/trap/syscall.rs`

## Problem (H-03 from v0.0.10 review)

`sys_task_start(handle, entry_pc, stack_top)` installs user-supplied `entry_pc`
and `stack_top` directly into the task trap frame without validation.

An attacker could supply `entry_pc = 0x8000_0000` to cause the task to execute at
kernel text, or `stack_top = 0x0` to fault immediately.

## Proposed fix

Add range checks before writing sepc and sp:

```rust
const USER_ADDR_MAX: usize = crate::platform::qemu_virt::RAM_BASE; // 0x8000_0000

if entry != 0 {
    if entry >= USER_ADDR_MAX || entry < 0x1000 {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }
}
if stack != 0 {
    if stack >= USER_ADDR_MAX || stack < 0x2000 {
        tf.gpr[REG_A0] = SysError::InvalidCap as isize as usize;
        return;
    }
}
```

Additionally: in `first_entry()` / `schedule_next()`, assert `sepc < RAM_BASE` and
`sstatus.SPP == 0` before sret.

## Note

This is a defence-in-depth check.  Full validation (entry_pc in an executable user
mapping) requires page table walk at syscall time and is deferred to M8.
