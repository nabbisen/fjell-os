# Syscall ABI

Defined in `crates/fjell-abi/src/lib.rs`.  Calling convention:

| Register | Role |
|---|---|
| `a7` | Syscall number |
| `a0`–`a5` | Arguments (in order) |
| `a0` (return) | `SysError` code (`0` = OK, negative = error) |
| `a1`–`a3` (return) | Optional return values on success |

`sepc` is advanced by 4 after every `ecall` before `sret`.

## M2 syscalls

| Number | Name | Arguments | Returns |
|---|---|---|---|
| 0 | `sys_yield` | — | OK |
| 1 | `sys_exit` | `a0`: exit code | never |
| 2 | `sys_debug_write` | `a0`: ptr, `a1`: len | OK or InvalidArg |

`sys_debug_write` is a smoke-test aid.  It will be removed or
capability-protected in the production ABI (M3+).

## Error codes

| Value | Name | Meaning |
|---|---|---|
| 0 | `Ok` | Success |
| -1 | `UnknownSyscall` | No such syscall number |
| -2 | `InvalidArg` | Argument out of range or malformed |
| -3 | `PermissionDenied` | Caller lacks required capability |
| -4 | `BadState` | Object is in wrong state for this operation |
| -5 | `InternalError` | Kernel-internal error |
