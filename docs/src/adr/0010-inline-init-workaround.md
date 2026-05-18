# ADR 0010: Inline init smoke workaround and service separation deprecation plan

**Status:** Accepted  
**Date:** 2026-05-12  
**Milestone:** M6/M7 (workaround); M8 (deprecation)

---

## Context

The Fjell OS architecture places all device drivers, store management, boot control,
upgrade, verification, rootfs, and snapshot logic in user-space services that
communicate via IPC.  However, this requires services to be able to signal readiness
back to their orchestrator and to block waiting for requests.

In M5 and earlier, services were independent IPC loops.  In M6, the following
constraint was discovered:

> Without a preemptive scheduler (timer interrupts in S-mode), a spawned task runs
> to completion before the spawning task resumes.  An IPC service that blocks on
> `sys_ipc_recv` will never be scheduled again unless something sends to it.

The `fjell-init` task spawns services sequentially.  If service B blocks on `recv`,
init never resumes to send to it — deadlock.

---

## Decision

### Workaround: inline logic in fjell-init

For M6 and M7, the following services are **stub binaries** that call `sys_exit(0)`
immediately:

- `fjell-storaged`
- `fjell-bootctl`
- `fjell-upgraded`
- `fjell-verifyd`
- `fjell-rootfsd`
- `fjell-snapshotd`

Their logic is implemented **inline in `fjell-init`** as sequential function calls.
Each service binary exists in the workspace to:

1. Reserve the `ImageId` for future IPC service use.
2. Provide a type-safe namespace for format types
   (`fjell-upgrade-format`, `fjell-store-format`, etc.).
3. Make the spawn sequence structurally correct (the service is spawned and started;
   it just exits immediately).

Comments in each stub binary document this explicitly:

```rust
// STUB: logic runs inline in fjell-init (see ADR 0010).
// Will become a real IPC service when the preemptive scheduler is available (M8).
fn main() { sys_exit(0); }
```

### Why not a cooperative scheduler?

A cooperative `yield` primitive was considered.  This would allow init to yield after
spawning each service, letting the service run until it calls `recv`, then init
resumes.  However:

- Services that call `recv` and never get a message would be stuck unless init
  proactively sends to them.
- Services with initialisation dependencies (storaged before bootctl, bootctl before
  upgraded, upgraded before verifyd) would require a fragile startup protocol.
- The smoke test scenario is inherently sequential; cooperative yield would not change
  the observable result, only the code structure.

The inline approach is structurally simpler for the smoke milestone and defers
IPC complexity to when it is genuinely needed (M8).

### Deprecation plan

M8 introduces a timer interrupt in S-mode, enabling a preemptive scheduler.  Once
preemption is available, the deprecation sequence is:

1. Implement `sys_notify_wait` (lightweight notification primitive).
2. Implement the `ServiceReadyProtocol`: each service signals init via a one-shot
   notification after initialisation.
3. Move `storaged` logic out of `fjell-init` into `fjell-storaged`.
4. Repeat for `bootctl`, `upgraded`, `verifyd`, `rootfsd`, `snapshotd`.
5. Remove inline logic from `fjell-init`; it becomes a pure orchestrator.
6. Delete this ADR's workaround status and mark as Superseded.

RFC 011 specifies the IPC protocol and service API changes needed.

---

## Consequences

- M6/M7 smoke tests pass with inline logic; the architectural intent is preserved
  as service stubs and format crates.
- `fjell-init` is a monolith during M6/M7 (intentional, documented, time-bounded).
- The inline code is **not tested in isolation**; unit tests for service logic require
  refactoring to pure functions not tied to `fjell-init`'s call flow.
- Any M8 work that depends on services being contactable via IPC will require the
  deprecation plan to be executed first.
