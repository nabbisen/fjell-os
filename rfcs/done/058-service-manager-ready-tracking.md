# RFC 058: service-manager READY tracking with cooperative timeouts

**RFC ID:** 058
**Also known as:** RFC-v0.2-024
**Status:** Implemented
**Target version:** v0.2.12
**Phase:** Service separation + release-gate close
**Closes review items:** RB-12 (service-manager half), H-04
**Depends on:** RFC 038 (Service plane separation foundation), RFC 055 (sender identity)

## Problem

### RB-12: service-manager is still a stub

`crates/fjell-service-manager/src/main.rs:1-18` is a stub that exits.
The v0.2 plan required service-manager to:

- Spawn services with the READY protocol
- Track which services have sent READY within a timeout window
- Observe faults via `sys_task_status`
- Restart or escalate on failure

None of this is implemented.

### H-04: cap-broker uses blocking recv despite `IpcTryRecv` availability

`crates/fjell-cap-broker/src/main.rs:363-366` reads:

```rust
// blocking recv (try_recv not available in current fjell-syscall;
// replace when IpcTryRecv is exposed)
```

`sys_ipc_try_recv` exists in the kernel (RFC 037).  The userspace wrapper
is not exposed; services that should cooperatively poll instead block.

### v0.2.8 SVC test scaffolding is creative but non-production

`fjell-neg-test::test_svc_start_timeout` and `test_svc_fault_detected`
implement timeout/fault detection by directly spawning svc-timeout and
svc-fault, yielding N times, and calling `sys_task_status`.  This works
for marker emission but is not the production path — service-manager
should be the source of truth for these events.

## Proposed fix

### service-manager responsibilities

`fjell-service-manager` becomes a real service:

1. Receives a manifest of services to manage (from a static table or
   from init via IPC).
2. Spawns each service using `sys_task_spawn` + `sys_task_start`.
3. For each service, sets a "READY deadline" = current_tick +
   START_TIMEOUT_TICKS.
4. Cooperatively polls its IPC endpoint via `sys_ipc_try_recv`.
5. Handles `SERVICE_READY` messages (tag 0x101) — records the service as
   ready.
6. On each poll cycle, checks all services for:
   - READY deadline passed and not yet ready → emit
     `NEG:SVC:START_TIMEOUT_DETECTED:PASS` and (in production)
     mark service for restart
   - `sys_task_status` returns Faulted → emit `NEG:SVC:FAULT_DETECTED:PASS`
     and (in production) mark service for restart
7. Responds to status queries from init: "is service X ready?"

### `SERVICE_READY` protocol

`fjell-service-api::tags`:

```rust
pub const SERVICE_READY: usize = 0x101;     // already exists per RFC 038
```

`fjell-service-api::types`:

```rust
pub struct ServiceReadyMsg {
    pub image_id:    u16,    // image_id of the sender (kernel-attested via RFC 055)
    pub endpoint_id: u32,    // service's primary endpoint object id
    pub version:     u16,    // service ABI version
}
```

Sent by every service after spawn-time initialization, before the main
work loop:

```rust
sys_ipc_send_words(SLOT_SERVICE_MANAGER_EP, SERVICE_READY,
                   own_endpoint_id, version, 0);
```

Service-manager receives, validates `image_id` (against the spawned set —
RFC 055 attestation prevents spoofing), records ready state.

### Cooperative timeout

Without a timer-based callback in user space, service-manager uses tick
count via `sys_clock_now` (RFC 037 timer fail-safe exposes monotonic ticks
to userspace):

```rust
const START_TIMEOUT_TICKS: u64 = 50;  // ~ 1-2 seconds at current tick rate

let now = sys_clock_now()?;
for entry in &mut self.services {
    if !entry.ready && now > entry.deadline {
        sys_debug_writeln(M::SVC_START_TIMEOUT);
        entry.timeout_emitted = true;
    }
}
```

The cooperative scheduler ensures service-manager runs frequently enough
to detect timeouts within a tick or two.

### Fault polling

```rust
for entry in &self.services {
    if let Ok(lc) = sys_task_status(entry.task_handle) {
        if lc == TaskLifecycle::Faulted as u8 && !entry.fault_emitted {
            sys_debug_writeln(M::SVC_FAULT);
            entry.fault_emitted = true;
        }
    }
}
```

### Replacing neg-test's creative implementation

After RFC 058 lands, `fjell-neg-test::test_svc_*` no longer emits the
markers directly.  Instead:

- neg-test still spawns svc-timeout and svc-fault (it has TaskCreate)
- service-manager observes them via the production path and emits markers
- neg-test waits long enough for service-manager to do so (yields N
  times)

This means the `svc` profile becomes a true end-to-end test of
service-manager.

### Closing H-04

Add `sys_ipc_try_recv` to fjell-syscall:

```rust
pub fn sys_ipc_try_recv(ep_handle: u32)
    -> Result<Option<(usize, usize, usize, usize, usize, SenderIdentity)>, SysError>
{
    // Calls SyscallNumber::IpcTryRecv = 23 (already defined).
    // Returns Ok(None) on WouldBlock, Ok(Some(...)) on message available.
}
```

cap-broker, service-manager, and other cooperative services use this
instead of `sys_ipc_recv`.

## Rationale

**Why cooperative timeouts via tick counts?**  A timer callback into
user space would require a kernel-to-user signal/upcall mechanism,
which is significantly larger than v0.2 scope.  Tick polling is
adequate when the scheduler cycles fast enough — typical service
turnaround is sub-millisecond.

**Why a separate service rather than embedding in init?**  init's job
is to spawn the initial set and hand control off.  Service-lifecycle
management is a continuing responsibility; combining them couples init
to the runtime in a way that defeats the v0.2 separation goal.

**Why don't services send READY via cap-broker?**  cap-broker handles
*authority*; service-manager handles *liveness*.  Conflating them
overloads cap-broker's responsibilities.  Each service holds an
endpoint cap to both — but cap-broker requests go to one, READY/status
goes to the other.

**What's the relationship to neg-test's current SVC tests?**  RFC 058
*replaces* neg-test's emit-from-inside-the-test pattern with
emit-from-service-manager.  The end-state markers are the same; the
emitter changes.  This is a strict improvement: v0.2.8's tests
incidentally proved that `sys_task_status` works; RFC 058 tests that
service-manager *uses* `sys_task_status` correctly.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-service-manager` | Full implementation replacing stub |
| `fjell-service-api` | `ServiceReadyMsg` struct |
| `fjell-syscall` | `sys_ipc_try_recv` wrapper; `sys_clock_now` wrapper if not present |
| `fjell-init` | Spawns service-manager early; delegates further spawning to it (or keeps current spawn list and just sends manifest) |
| `fjell-cap-broker` | Switches to `sys_ipc_try_recv` + cooperative loop |
| All services | Send `SERVICE_READY` after initialization |
| `fjell-neg-test` | SVC tests delegate to service-manager (waits and verifies markers emitted) |

### Backward compatibility

All services need a one-line addition (send READY).  Failure to send
READY makes the service appear hung — service-manager will emit the
timeout marker.

### Audit trail

New audit kinds:
- `ServiceReady` — service-manager received READY (arg0 = image_id)
- `ServiceTimeout` — deadline missed (arg0 = image_id, arg1 = task_id)
- `ServiceFault` — task_status returned Faulted (arg0 = image_id)

## Test plan

### Host

1. `ServiceReadyMsg` round-trips through serialization
2. `sys_ipc_try_recv` returns `WouldBlock` on empty endpoint

### QEMU

3. `NEG:SVC:START_TIMEOUT_DETECTED:PASS` — recharacterized: emitted by
   service-manager when svc-timeout misses its READY deadline.
4. `NEG:SVC:FAULT_DETECTED:PASS` — recharacterized: emitted by
   service-manager when svc-fault is observed Faulted via `sys_task_status`.
5. **NEW** `NEG:SVC:READY_ACCEPTED:PASS` — every normally-starting service
   sends READY and service-manager records it.  Service-manager emits this
   marker after the first 10 services successfully report ready.
6. **NEW** `NEG:SVC:UNAUTHORIZED_READY_REJECTED:PASS` — neg-test
   sends a `SERVICE_READY(image_id=STORAGED)` claim (spoofing).  RFC 055
   attestation kicks in: service-manager reads sender's true image_id
   (NEG_TEST) — does not match claimed STORAGED → rejects, emits marker.

## Implementation notes

- service-manager runs at a priority that ensures it polls frequently.
  In the current cooperative scheduler, all user tasks share the same
  priority; service-manager's polling rate equals the schedule cycle.
- The `SERVICE_READY` payload uses kernel-attested `image_id` (RFC 055)
  for the **identity check**.  The payload's `image_id` field is for
  audit clarity; the attestation supersedes any payload claim.
- service-manager's manifest could be:
  - Static (compiled-in table) — simplest for v0.2.12
  - Dynamic (init sends a list at startup) — defer to v0.3
- `sys_clock_now` exposes monotonic ticks.  The exact tick rate depends
  on platform; document in service-api.

## Open questions

- Should service-manager auto-restart faulted services?  v0.2.12 scope:
  no — only detect and report.  Restart logic is v0.3.
- Should `SERVICE_READY` be a *call* (expects reply) or a *send* (no
  reply)?  Recommendation: send.  A call would block the service on
  service-manager's responsiveness, which inverts the dependency.
- Tick rate for `START_TIMEOUT_TICKS = 50`: tuned empirically.  Should
  be a per-service value in the manifest.  Defer; one global value
  suffices for v0.2.12.
