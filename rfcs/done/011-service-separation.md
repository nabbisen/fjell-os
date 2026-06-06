# RFC 011: Service plane separation — remove fjell-init inline logic

**RFC ID:** 011  
**Status:** Implemented (v0.1.0)
**Affects:** All M6/M7 service stub crates + fjell-init

## Problem (RB-05)

M6/M7 logic (virtio I/O, storaged, bootctl, upgraded, verifyd, rootfsd, snapshotd)
is implemented inline in `fjell-init`.  Service binaries are `sys_exit(0)` stubs.

## Root cause

No synchronous IPC rendezvous: a spawned task runs to completion before init resumes.
Without a blocking `ipc_call` + `ipc_recv` round-trip, services cannot signal readiness
back to init.

## Proposed fix

### Phase 1: IPC rendezvous (M8 prerequisite)
- Implement `sys_task_yield` + blocking scheduler tick for S-mode timer interrupt.
- Alternatively: implement `sys_notify_wait` as a lightweight synchronisation primitive.

### Phase 2: Service protocol (M8)
- Define `ServiceRequest` / `ServiceResponse` in `fjell-service-api` for each M6/M7 service.
- Implement service binaries using IPC request/response loop.
- fjell-init calls services via IPC; removes inline virtio/store/bootctl/verify logic.

## Defer condition

M8 — cannot implement without timer/preemption or IPC blocking primitive.
