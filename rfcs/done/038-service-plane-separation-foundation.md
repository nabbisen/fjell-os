# RFC 038: Service plane separation foundation

**RFC ID:** 038  
**Also known as:** RFC-v0.2-008  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 5 — Cooperative Service Separation  
**Related epics:** D (Service Plane Realization)

## Problem

At v0.1.0, `fjell-init` contains the inline implementations of
`storaged`, `bootctl`, `verifyd`, `upgraded`, `rootfsd`, and
`snapshotd` (ADR-0010).  This was a deliberate M7-era workaround:
without blocking IPC rendezvous and reliable service handshakes,
services could not be spawned as separate tasks.

RFC 037 provides the missing primitive (`sys_ipc_try_recv` +
preemptive fail-safe).  This RFC defines the **separation
foundation** the services build on:

- a service-ready protocol so `service-manager` can detect failed
  starts,
- a fault/timeout model so a hung service does not freeze the
  system,
- the order in which the inline services are extracted.

## Proposed fix

### Service-ready protocol

Every separated service, on start:

1. Performs minimum initialisation.
2. Sends a `READY` message on its private endpoint.
3. Enters its cooperative service loop (RFC 037 shape).

`service-manager` watches:

```
- READY message within start_timeout → service is up
- timeout without READY              → service start failed (audit)
- fault propagated from kernel       → service-manager records and
                                       may restart per policy
```

### Required initial separation order

```
1. storaged
2. bootctl
3. verifyd
4. upgraded
5. rootfsd
6. snapshotd
```

The order matches data-dependency: `storaged` is the foundation,
`bootctl` reads it, and the rest depend on those.

### Service-manager responsibilities

- Owns the `start_timeout` per service.
- Owns the restart policy (initially: no auto-restart in v0.2;
  service fault is fatal so the failure is visible).
- Watches `TaskQuantumExceeded` (RFC 037) audit events; repeated
  violations within a window mark the service Faulted.

### Service manifest

The TOML service manifest (M6) gains new fields:

```toml
[service.storaged]
image        = "fjell-storaged"
start_timeout_ms = 1000
ready_endpoint   = 7
needs_caps   = ["StoragedRead", "StoragedWrite"]
```

`needs_caps` lists the *kinds* the service requires; the
`cap-broker` (RFC 044) resolves these against policy before granting.

### Cap-broker handoff

Before this RFC ships, `cap-broker` must be operational (RFC 044
covers bootstrap).  This RFC therefore depends on RFC 044 landing
first or in lockstep; the v0.2 phase sequence in
`fjell-os-v0.2-overview-design.md` reflects this — service
separation is Phase 5, `cap-broker` is Phase 7, and service
separation can begin before Phase 7 because v0.1.x `cap-broker`
already exists in a non-default-deny shape sufficient for separated
services.

## Rationale

The fixed extraction order avoids a class of cyclic-dependency
bugs.  `storaged` first means every other service has a place to
write durable state from day one of separation; trying to extract
`verifyd` before `storaged` would force a stub-storage fallback that
hides bugs.

Marking a service Faulted on quantum violation rather than
auto-restarting matches the v0.2 philosophy: surface failure rather
than hide it.  Auto-restart can land in v0.3 once the failure
patterns are well understood.

## Impact

- Crates: `fjell-init` (drop inline service code, replace with
  service-manager driven spawn), `fjell-service-manager` (READY
  protocol, timeout, fault), `fjell-storaged`, `fjell-bootctl`,
  `fjell-verifyd`, `fjell-upgraded`, `fjell-rootfsd`,
  `fjell-snapshotd` (each becomes a real service binary, not
  inlined).
- Backward compatibility: changes the boot sequence; `TEST:M4:PASS`
  → `TEST:M7:PASS` smoke chains must be re-verified.
- Configuration: new service manifest fields.

## Test plan

### QEMU negative tests
- `NEG:SVC:START_TIMEOUT_DETECTED:PASS` — a service that never
  sends READY is marked Failed.
- `NEG:SVC:READY_MISSING_REJECTED:PASS`
- `NEG:SVC:FAULT_DETECTED:PASS`
- `NEG:SVC:BOOTCTL_UNAVAILABLE_PREVENTS_CONFIRM:PASS`
- `NEG:SVC:STORAGED_UNAVAILABLE_PREVENTS_DURABLE_APPEND:PASS`

### Acceptance gates
- `storaged` and `bootctl` are no longer inline `init` logic.
- `service-manager` observes service ready states.
- Service loops do not deadlock.
- A spinning service cannot monopolise the hart indefinitely.

## Implementation notes

- Out of scope: priority scheduling, auto-restart policy,
  cross-service dependency graph beyond the fixed order, init
  declarativeness beyond the manifest extension.
- The `service-manager` must use `try_recv` itself; otherwise a
  faulted service could hang the supervisor.
- ADR-0010 should be updated to *Superseded by RFC 038* once
  separation lands.
