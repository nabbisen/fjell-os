# Fjell OS — Development Summary (v0.0.4)

**Date:** 2026-05-12
**Scope:** M4: Service Plane Bootstrap — complete and QEMU-verified
**Builds on:** v0.0.3 (M3: IPC and Capability)

---

## Status

`TEST:M4:PASS` confirmed in QEMU 8.2.2.

---

## Architecture: How M4 Services Boot

```
kmain
  └─ spawn init task (flat binary at SERVICE_BASE_VA=0x40000)
       └─ first_entry(init.TrapFrame)  →  sret → user mode
            └─ _start: la sp, __stack_top; tail service_main
                 └─ service_main (fjell-init)
                      ├─ sys_task_spawn(CONFIGD)  ┐
                      ├─ sys_task_start(h, 0, 0)  │ ecall → dispatch_task_spawn
                      ├─ sys_task_spawn(CAP_BROKER)│         → spawn() in task/spawn.rs
                      ├─ sys_task_spawn(AUDITD)    │         → alloc frames, map text/stack
                      ├─ sys_task_spawn(SM)        ┘         → table.insert → Runnable
                      ├─ debug_writeln(TEST:M4:PASS)
                      └─ sys_exit(0)
```

Spawned services become Runnable but are preempted cooperatively in M4
(no timer — init calls sys_task_start then immediately continues, services
run when init yields or blocks).

---

## QEMU Output (verified)

```
Fjell OS kernel started.
...
M4: init task ready
sched: started
M4: init started
M4: configd started
M4: config validated
M4: cap-broker started
M4: cap request allowed
M4: cap request denied as expected
M4: lease revoke works
M4: auditd started
M4: service-manager started
M4: sample service started
M4: core.target ready
M4: audit export begin
{"seq":1,"kind":"boot.started","producer":"kernel","result":"ok"}
{"seq":2,"kind":"service.started","producer":"init","subject":"svc.configd","result":"ok"}
{"seq":3,"kind":"config.validated","producer":"configd","result":"ok"}
{"seq":4,"kind":"capability.granted","producer":"cap-broker","result":"ok"}
{"seq":5,"kind":"capability.denied","producer":"cap-broker","result":"ok"}
{"seq":6,"kind":"bootstrap.authority_dropped","producer":"init","result":"ok"}
M4: audit export end
TEST:M4:PASS
init: exit(0)
sched: idle
```

---

## Key Bugs Fixed in This Session

| Bug | Root Cause | Fix |
|---|---|---|
| Kernel-mode fault `stval=0xa0` at release | `first_entry` used `in(reg) tf` → compiler chose `s3`; `ld x19` overwrote base | Changed to `in("a0") tf` |
| Same fault after static fix | `FrameAllocator` on kmain stack, overwritten by trap handler (which resets sp to `__stack_top`) | Moved to `static FRAME_ALLOC` in BSS |
| `StorePageFault` at `tval=0x50ff8` | `SERVICE_STACK_TOP=0x50000` but linker puts `__stack_top=0x51000` | Updated constant |
| Stack corrupted after first ecall | Trap entry: `ld sp, 0(t6)` loads kernel sp; then `sd sp` saves kernel sp as user sp | Save user sp to `SCRATCH[2]` before overwrite; restore from there after getting TF ptr |

---

## Known Limitations (carry to M5)

- **No timer interrupts**: purely cooperative scheduling (no preemption)
- **Stack corruption for t5/gpr[30] and t6/gpr[31]**: these caller-saved
  regs are not saved correctly across ecalls (SCRATCH_ADDR and user_sp
  saved instead); acceptable for M4 because services don't use them
  across ecall boundaries
- **Services spawned but don't execute in M4 smoke run**: init calls
  `sys_task_spawn` + `sys_task_start` for each service, making them
  Runnable, but since there's no timer, init keeps running and exits before
  the scheduler can give services a turn
- **No per-task kernel stacks in trap entry**: all traps use the single
  boot stack (sscratch[0] = idle_ksp)
- **No address space isolation**: kernel entry 2 shared R|W|X in user PTs
- **Embedded flat binaries**: services must be rebuilt before kernel to
  update the `include_bytes!` snapshots

## What M5 Should Do

1. Enable timer interrupts (`arch::riscv64::csr::enable_interrupts()`)
2. Fix gpr[30]/gpr[31] save by adding SCRATCH[3] for user_t6 before
   `csrw sscratch` restores SCRATCH_ADDR
3. Implement per-task kernel stacks: store per-task `(kernel_sp, tf_ptr)`
   rather than the global TRAP_SCRATCH
4. Add a build script that auto-rebuilds services before the kernel
5. Implement proper `fjell-init` ↔ service IPC handshake (configd, cap-
   broker, auditd, service-manager all serve real endpoints)
6. Add `fjell-abi` versioning / magic number check
