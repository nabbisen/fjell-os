# RFC 057: bootctl service extraction

**RFC ID:** 057
**Also known as:** RFC-v0.2-023
**Status:** Proposed
**Target version:** v0.2.12
**Phase:** Service separation + release-gate close
**Closes review item:** RB-12 (bootctl half)
**Depends on:** RFC 038 (Service plane separation foundation), RFC 056 (cap-install)

## Problem

`crates/fjell-bootctl/src/main.rs:1-6` is:

```rust
pub extern "C" fn service_main() -> ! { sys_exit(0) }
```

A stub.  Boot-control decision logic (the v0.1 design directive that
boot confirmation is a separate authority domain from init) is currently
inline in `fjell-init` or implicit in storaged's `BOOT_STATE` slot
handling.

RFC 038 required boot-control to run as a separated service, observable
by service-manager, holding its own caps for storaged's boot-state slot
and for invoking reboot/rollback.  The release-gate cannot grant
`TEST:V02:PASS` while this is a stub.

## Proposed fix

### Service responsibilities

`fjell-bootctl` is a small user-space service that:

1. Holds the storage `BOOT_STATE` slot endpoint cap (delegated by
   cap-broker via RFC 056 — or installed at spawn for v0.2.12 with a
   `TODO(v0.3)` to move to broker delegation).
2. Holds a `RebootCap` (new — see below) that authorizes
   `sys_reboot(mode)`.
3. Responds to four message tags from init (or service-manager):
   - `BOOT_PENDING_QUERY` → returns current state (Pending / Confirmed /
     Rollback)
   - `BOOT_CONFIRM` → writes Confirmed to BOOT_STATE; replies Ok
   - `BOOT_ROLLBACK` → writes Rollback to BOOT_STATE; replies Ok; then
     calls `sys_reboot(Cold)`
   - `BOOT_SHUTDOWN` → replies Ok then exits

4. Sends `READY` (RFC 058) on successful startup with its endpoint cap
   handle (so service-manager can route confirmation requests to it).

### New protocol tags

`fjell-service-api::tags`:

```rust
pub const BOOT_PENDING_QUERY: usize = 0x070;
pub const BOOT_CONFIRM:       usize = 0x071;
pub const BOOT_ROLLBACK:      usize = 0x072;
pub const BOOT_STATE_REPLY:   usize = 0x073;  // reply tag; w0 = state byte
pub const BOOT_SHUTDOWN:      usize = 0x07F;
```

### `RebootCap` and `sys_reboot`

A new minimal kernel primitive:

```
sys_reboot(reboot_cap_handle, mode: u32) -> Result<!, SysError>
```

Validation: `require_cap_on(handle, CapKind::Reboot, CapRights::REBOOT)`.
On success, the kernel logs a `Reboot` audit event and triggers a
platform reset.  Mode = 0 means cold, 1 means warm (current platform
treats both identically).

This cap is granted at spawn time only to `fjell-bootctl`.

### init flow change

`crates/fjell-init/src/main.rs` currently performs boot-state decisions
inline.  After RFC 057:

```rust
// 1. Spawn bootctl, wait for READY.
let bootctl = spawn(ImageId::BOOTCTL, "bootctl");
wait_for_ready(bootctl);  // RFC 058

// 2. Query boot state via IPC instead of direct storage read.
let (_, state, _, _) = sys_ipc_call_words(SLOT_BOOTCTL_EP, BOOT_PENDING_QUERY, 0, 0, 0)?;

// 3. Continue boot per state.
match state {
    0 => /* Pending */ run_self_check_and_confirm(),
    1 => /* Confirmed */ continue_normal_boot(),
    2 => /* Rollback in progress */ trigger_recovery(),
    _ => panic!("invalid boot state"),
}
```

`SLOT_BOOTCTL_EP` is installed by cap-broker at init's request, or
pre-granted at spawn for v0.2.12.

### What removes from init

| Currently in init | Moves to |
|-------------------|----------|
| Direct storage write to BOOT_STATE slot | bootctl |
| `sys_reboot` call (if any) | bootctl |
| Boot-state validation | bootctl |
| Self-check decision logic | stays in init (uses bootctl only to commit state) |

## Rationale

**Why a separate service?**  The boot-control authority is sensitive
(decides whether to roll back, when to reboot).  Keeping it in a
separated service with its own minimal caps means:

- Compromise of init does not grant immediate authority to flip boot
  state.
- Auditing is scoped: `bootctl` is the only origin of `BOOT_*` audit
  records.
- service-manager can restart bootctl on fault without restarting init.

**Why a new `RebootCap` instead of granting `sys_reboot` to bootctl
unconditionally?**  Consistent with the capability-everywhere model.
The kernel has no business knowing which service is "the boot
controller" — only that the caller of `sys_reboot` holds reboot
authority.

**Why not delegate via cap-broker now?**  RFC 056's cap-broker
installation primitive lands in v0.2.12 — same release.  Either order
works; this RFC pre-grants at spawn time for simplicity, and a follow-up
moves grants to broker.

**Why protocol tags 0x070-0x07F?**  Free range in the existing tag
space.  RFC 038's general layout reserves 0x060-0x06F for IPC test, so
boot-control gets the next block.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-bootctl` | Real implementation replacing stub |
| `fjell-init` | Replace inline boot-state logic with IPC to bootctl |
| `fjell-cap` | `CapKind::Reboot`, `CapRights::REBOOT` |
| `fjell-kernel/trap/syscall.rs` | `sys_reboot` syscall |
| `fjell-kernel/task/spawn.rs` | Install RebootCap and BOOT_STATE storage cap into bootctl |
| `fjell-service-api` | New tags |
| `fjell-syscall` | `sys_reboot` wrapper |

### Backward compatibility

init's behavior is observably identical to a working v0.2.x.  The
internal path changes — boot decisions now require bootctl to be
running.

### Audit trail

New audit kinds:
- `BootConfirm` — bootctl wrote Confirmed
- `BootRollback` — bootctl wrote Rollback
- `Reboot` — bootctl invoked `sys_reboot`

These appear in the audit log and in snapshot digests (RFC 041's
evidence path).

## Test plan

### Host

1. `RebootCap` round-trips through serialization
2. Protocol tag values don't collide

### QEMU

3. `xtask qemu-test m8` continues to pass — bootctl is in the boot path.
4. **NEW** `NEG:BOOTCTL:UNAUTHORIZED_REBOOT_REJECTED:PASS` — neg-test
   directly calls `sys_reboot(0, 0)` (no RebootCap) → `PermissionDenied`.
5. **NEW** `NEG:BOOTCTL:CONFIRM_WITHOUT_PENDING_REJECTED:PASS` — bootctl
   rejects `BOOT_CONFIRM` if current state is not Pending; replies with
   error tag.
6. **NEW** `NEG:SVC:BOOTCTL_DOWN_BLOCKS_CONFIRMATION:PASS` — kill
   bootctl, init's `BOOT_PENDING_QUERY` times out → init detects via
   service-manager's status (RFC 058) → emits marker.

Marker #6 closes the review's "test bootctl unavailable prevents
confirmation" requirement.

## Implementation notes

- `sys_reboot` should be marked `!` (diverging) — it never returns
  normally.  If the reboot platform call fails, the kernel panics
  rather than returning to the caller (boot consistency invariant).
- bootctl's storaged endpoint slot needs to be wired in.  v0.2.12
  pre-grants it at spawn time; a follow-up (v0.3) moves to cap-broker
  delegation.
- Audit records for boot decisions should include the `image_id` of the
  caller (always bootctl) in `arg0`.  Useful for forensics if a
  compromised service somehow obtains a RebootCap copy.

## Open questions

- Should bootctl re-validate the storage-side BOOT_STATE slot
  cryptographically (e.g. against a measurement)?  Recommendation: not
  for v0.2.12; aligns with v0.3 attestation work.
- Should `BOOT_CONFIRM` require a fresh capability per call (one-shot)?
  Reduces replay surface.  Recommendation: defer; init's policy is
  to call `BOOT_CONFIRM` at most once per boot anyway.
