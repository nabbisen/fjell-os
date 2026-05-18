# Fjell OS — Development Summary (v0.0.3 final)

**Date:** 2026-05-11  
**Scope:** M3: IPC and Capability — complete and QEMU-verified  
**Continuation from:** dev-summary-v0.0.3.md (pre-QEMU state)

---

## Status

`cargo xtask qemu-test m3` passes. `TEST:M3:PASS` confirmed in QEMU 8.2.2 output.

---

## Bugs Fixed in This Session

### Compile errors (Rust 2024 edition)

| File | Error | Fix |
|---|---|---|
| `fjell-cap/src/cspace.rs` | `gen` is a reserved keyword in 2024 | Renamed to `slot_gen` |
| `fjell-ipc/src/endpoint.rs` | `IPC_WORDS`/`IPC_CAPS` duplicate `pub use` (E0252) | Removed `pub use` from endpoint.rs; re-export directly from `message` in lib.rs |
| `fjell-ipc/src/endpoint.rs` | `SendResult`/`RecvResult` missing `Debug` for `unwrap_err` | Added `#[derive(Debug)]` |
| `fjell-cap/src/slot.rs` | `Capability` missing `PartialEq` for `assert_eq!` | Added `#[derive(PartialEq)]` |
| `task/user_image.rs` | Stray `}` after `USER_TASK_A` slice literal | Removed |
| `cap/table.rs` | Unused imports | Cleaned up |

### Runtime bugs (QEMU)

| Bug | Symptom | Root cause | Fix |
|---|---|---|---|
| PMP not configured | S-mode instruction access fault immediately after mret | RISC-V PMP denies all S/U-mode access by default | Added `write_pmpaddr0(usize::MAX)` + `write_pmpcfg0(0x1F)` in m_mode_setup |
| DTB reservation panic | `rsv dtb: AlreadyReserved` on boot | DTB overlaps kernel image on QEMU virt | Changed to `let _ = fa!().reserve_range(...)` |
| Kernel stack not mapped | store_page_fault at 0x8040_xxxx | Identity map only covered to BSS+2MiB; stack is at BSS+4MiB | Extended map loop to `__stack_top` |
| sscratch corrupted after trap | Second trap: `ld sp, 0(t6)` with t6=0 | `csrrw t6, sscratch, t6` leaves sscratch=user_t6; never restored | Added `write_sscratch` in `schedule_next` |
| clone_kernel_half wrong range | InstructionPageFault when user task traps | Copied entries 256..511 (empty); kernel is at entry 2 (0x80000000) | Fixed to copy entry 2 only |
| UART not in user page table | load_page_fault in trap handler's kprintln | Trap handler runs with user satp; UART not mapped | Added UART map_page to each user's AddressSpace |
| double-dequeue in scheduler | Tasks silently skipped/lost | `block()` called `on_exit()` which calls `choose_next()` and discards result; schedule_next also calls `choose_next()` | Added `suspend_current()`; changed `on_exit/on_fault/on_yield` to not call `choose_next` |
| premature sender wake in ipc_recv | Client woken before server replies | `wake(sender)` called unconditionally in Delivered path | For `is_call=true`, sender is NOT woken; only woken by explicit `ipc_reply` |
| zombie task resurrection in ipc_reply | Exited task set back to Runnable | No state check before setting caller to Runnable | Added `matches!(caller.state, Blocked(_))` guard |

---

## QEMU Output (verified)

```
Fjell OS kernel started.
mode: S
platform: qemu-virt
memory: detected (128 MiB)
mm: boot allocator ready
mm: frame allocator ready  (32210 free frames)
vm: sv39 enabled
trap: stvec installed
M3: capability table initialized
M3: endpoint table initialized
M3: endpoint created (id=0)
M3: client task created
M3: client call cap granted
audit: capability.granted
M3: server task created
M3: server receive cap granted
audit: capability.granted
M3: denied task created
sched: started
client: call sent
audit: ipc.call
server: request received
denied: ipc denied as expected
audit: capability.denied
server: reply sent
client: reply received
audit: ipc.reply
denied: exit(0)
client: exit(0)
server: exit(0)
sched: idle
TEST:M3:PASS
```

---

## Known limitations (carried forward to M4)

All limitations from the original dev-summary-v0.0.3.md remain:

- **Address space isolation**: user tasks now have correct per-task page tables
  (satp switching implemented), but kernel code/stack is shared via root entry 2
  (R|W|X; user tasks can read kernel memory). Full isolation deferred to M4.
- **Single kernel stack**: trap entry uses the boot stack for all tasks. Per-task
  kernel stacks are allocated but not wired into trap_entry sscratch[0].
- **Timer interrupt disabled**: `enable_interrupts()` is never called; only
  cooperative scheduling via `sys_yield` works.
- **DTB not parsed**: physical memory and MMIO are hardcoded for QEMU virt.
- **ASID fixed at 0**: TLB is flushed globally on every satp switch.

---

## What the Next Developer Should Do (M4)

1. Implement a static ELF loader so init/service-manager/sample-service are
   real Rust binaries rather than hand-assembled bytes.
2. Fix address space isolation: remove kernel R|W|X from user page tables;
   use a proper kernel/user VA split.
3. Wire per-task kernel stacks into trap_entry (scratch[0] per task, not shared
   boot stack).
4. Enable timer interrupts (`arch::riscv64::csr::enable_interrupts()` in kmain).
5. Implement `fjell-init`, `fjell-service-manager`, `fjell-sample-service`.
6. Define init→service-manager IPC protocol in `fjell-abi`.
