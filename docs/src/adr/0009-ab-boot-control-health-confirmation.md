# ADR-0009 — A/B Boot Control and Health Confirmation

**Status:** Accepted  
**Date:** 2026-05-12  
**Milestone:** M6/M7

---

## Context

Safe over-the-air updates require an A/B slot model: the currently running system
occupies one slot; a staged update occupies the other.  If the new slot fails to boot
or fails health checks, the system must be able to roll back to the last confirmed slot
without operator intervention.

---

## Decision

### Slot model

Two slots (A and B) are defined.  Each slot has a `SlotInfo` record:

```rust
pub struct SlotInfo {
    pub state:             SlotState,   // Empty | Staging | Staged | Bootable | Unbootable
    pub image_generation:  u64,
    pub remaining_tries:   u8,
    pub confirmed:         u8,          // 1 = health-confirmed
}
```

On first boot, slot A is `Bootable` (with `remaining_tries = 3` and `confirmed = 1`);
slot B is `Empty` (RFC 002 fix).

### BootControlBlock

Stored at two mirrored locations on disk (LBA 1 and LBA 33) so that a write failure to
one mirror does not destroy boot-control state.  Fields:

- `active_slot`: which slot is currently running
- `last_confirmed_slot`: last slot that passed health checks
- `candidate_slot`: `0xFF` = none, `0` = A, `1` = B
- `slot_a`, `slot_b`: `SlotInfo` records
- `crc32`: ISO 3309 CRC over all other fields (RFC 008)

Mirror selection policy: on read, choose the mirror with the higher `generation` field
that also passes `is_valid()` (magic + CRC).  This is **defined** but not yet exercised
in M7: both mirrors are written identically.

### Upgrade state machine

```
Created → Verified → Staging → Staged → CandidateSet → CandidateBoot
  → HealthCheck → Confirmed  (success path)
                → Rollback   (failure path)
```

- **Verified:** `SignedObject::verify_dev()` must return `Ok` before staging proceeds.
- **CandidateSet:** `BootControlBlock.candidate_slot` is updated and written to disk.
- **CandidateBoot:** In M7 this is simulated inline in `fjell-init` (not a real reboot).
- **HealthCheck:** `health_ok` is currently a fixed `true` constant.  A real health
  check compares running services against a `HealthTarget` struct (defined in
  `fjell-upgrade-format` but not yet connected to a runtime evaluator).
- **Rollback:** If `health_ok` is false (or `remaining_tries` reaches 0), the system
  restores `active_slot = last_confirmed_slot` and marks the candidate unbootable.

**Known limitation (RB-07):** The candidate boot is simulated; no real reboot occurs.
`health_ok = true` is hardcoded.  The mirror selection algorithm is defined but
untested.  Active-slot write rejection is defined but not enforced by `upgraded`.

### upgraded service

`fjell-upgraded` stages the new release image to the inactive slot.  In M7, inactive
slot validation (preventing writes to the active slot) is defined in the type contract
but not enforced at the kernel level; `upgraded` is a stub.

---

## Consequences

- The A/B model is structurally in place; the smoke test exercises the state transitions
  with simulated health checks.
- Real reboot-and-confirm is blocked on the preemptive scheduler (M8 prerequisite).
- Active-slot write protection will be enforced when `upgraded` is an IPC service
  and the kernel provides a write-permission capability for the inactive slot only.

> **Correction, 2026-09-15 (E-044).** Several statements above no longer
> describe the tree. Mirror selection **is** tested (four tests in
> `fjell-upgrade-format`). `health_ok` is not a hardcoded constant:
> `fjell-bootctl-model` models health failure and last-known-good fallback —
> but no crate uses that model, so there is still no health check at runtime.
> Candidate boot is no longer simulated in `fjell-init`, or anywhere. The smoke
> test does not exercise the state transitions: `bootctl` is spawned and waits
> for four messages that no component sends. M8 shipped without timer
> preemption, and no reboot syscall is dispatched, so the rollback path — if
> anything ever reached it — would discard the reboot error and spin. The
> decision stands; the runtime it describes does not exist yet.

> **Correction, 2026-09-22 (RFC-0.33-001, E-044).** The correction above is
> itself wrong in places, and the ADR's text was wrong in ways it did not
> notice. This one is true of the tree at the commit that adds it; the ADR's
> *decision* still stands.
>
> **What the ADR got wrong from the start.**
>
> * `SlotState` is `Empty | Bootable | Candidate | Confirmed | Failed`, not
>   `Empty | Staging | Staged | Bootable | Unbootable`, and `SlotInfo` also
>   carries `tries_allowed`.
> * There is no `HealthTarget` type. It is not "defined in
>   `fjell-upgrade-format` but not yet connected": no struct of that name exists
>   in any crate. (The semantic toolkit has `HealthTargetReachedArgs` and
>   `HealthTargetFailedArgs`, generated event-argument types; nothing in a
>   service or the kernel uses them.)
> * "Real reboot-and-confirm is blocked on the preemptive scheduler" was never
>   established. A reset needs no scheduler: the reset device is one 32-bit
>   store, and a probe program that does nothing else resets the machine
>   (measured; the answer to RFC-0.33-001 §D). This kernel does not preempt in any
>   case — its timer interrupt is never enabled (`csr::enable_interrupts` has no
>   caller). What a reset needed was a reset device and a syscall that reaches it,
>   and neither was there.
>
> **What the correction above got wrong.**
>
> * "Candidate boot is no longer simulated in `fjell-init`, or anywhere" is
>   false. `fjell-init` still has `let health_ok = true` and still prints
>   `M6: boot confirmation simulated`, `M7: candidate boot simulated`,
>   `M7: health target passed`, `M7: slot confirmed after health` and
>   `M7: health failure rollback simulated`, unconditionally. No later step
>   depends on any of them, so they are *markers of a simulation*, not of a
>   boot. `fjell-init` also writes a fresh `BootControlBlock` to both mirrors on
>   every boot, and nothing reads one back.
>
> **What exists now.**
>
> * `bootctl` owns a real `BootControlBlock` and drives the ADR's transitions on
>   it (`fjell_upgrade_format::boot_state`: `set_candidate`, `begin_boot`,
>   `confirm`, `fail_health`, `reboot`, and `apply_health`, which decides what a
>   health report means). `fjell-bootctl-model` is no longer unused: its
>   `tests/refines.rs` runs the same operations through the model and the block
>   and compares them after every step. That comparison found three defects in
>   the model (a freshly staged slot began unhealthy; rollback left the failed
>   candidate staged, so the next reboot re-selected it; a staged image inherited
>   the outgoing slot's boot count) and one in the block (a slot that had never
>   run could be confirmed). All are fixed.
> * **The ADR is silent on one case and the state machine had to choose.** A
>   health failure on the *last confirmed* slot has nowhere to roll back to. The
>   block does not mark it unbootable — it is the only image the system can fall
>   back to — and reports `NothingToRollBackTo`; `bootctl` does not reset on it,
>   because a reset that changes nothing repeats.
> * One protocol: `tags::BOOT_*`. The second (`fjell_service_api::bootctl`,
>   RFC 019) and the two commands that had the sender decide the outcome
>   (`BOOT_CONFIRM`, `BOOT_ROLLBACK`) are removed; a health *report*
>   (`BOOT_HEALTH_REPORT`) replaces them, accepted only from `service-manager`.
>
> **What does not exist, so that no line above is read as more.**
>
> * **Nothing sends the report.** `service-manager` evaluates no health; nothing
>   in the tree emits `BOOT_HEALTH_REPORT`, and `bootctl` waits for a message no
>   component sends.
> * **No boot selects an image.** There is one kernel image. "Roll back to slot
>   A" changes what the block says and nothing else; a reset boots the same image
>   with a fresh block. This line does not claim otherwise (RFC-0.33-001 D7).
> * **The block is not durable.** It lives in `bootctl`'s memory. Reading a block
>   back from disk needs a store client, which does not exist: `storaged`
>   accepts a sector write, but its read reply is a placeholder that returns no
>   data (RFC-0.33-001 §A).
> * **No reboot is dispatched.** `PlatformReboot` (18) and its duplicate
>   `Reboot` (120) are both undispatched; `bootctl`'s rollback arm still calls
>   the former, is refused, and spins.
>
> The rest of E-044 is not closed by this note.

> **Correction, 2026-09-23.** Everything the 2026-09-22 correction listed as
> "does not exist" now does, except one thing this note explains was never
> reachable to begin with — and the earlier tag name in that correction was
> itself wrong.
>
> **What now exists, measured, not assumed.**
>
> * **The report is sent, and read.** `service-manager` tracks whichever
>   registered task it was told is required (`init`, via
>   `tags::SM_REGISTER_REQUIRED`, sent right after `sys_task_spawn` — before
>   the task has run, so there is no race with its own `SERVICE_READY`) and
>   reports one verdict to `bootctl`: `tags::BOOT_HEALTH_OK` once the existing
>   readiness threshold is met with nothing required yet faulted, or
>   `tags::BOOT_HEALTH_FAILED` the first time a required entry's fault is
>   observed — whichever comes first, and never both. *(The earlier
>   correction named this `BOOT_HEALTH_REPORT` carrying the verdict in a data
>   word; that never worked — a one-way `sys_ipc_send` cannot carry one, only
>   a reply can — and is two tags now.)*
> * **`bootctl` confirms and fails for real.** `service-manager: started` used
>   to be the last line `bootctl` ever printed. It now prints
>   `bootctl: health passed; active slot confirmed` on every boot's happy path
>   — confirmed live in all 19 tracked QEMU artifacts — and, on the
>   console-triggered failure path (`tests/qemu/profiles/health-fail.toml`),
>   `bootctl: health FAILED on the last confirmed slot ... not resetting`.
>   `fjell-init`'s own simulated confirm/rollback markers (`health_ok = true`,
>   `health_fail = true`, unconditional) are removed: two sources for one
>   fact, one of them always claiming success, was how a confirm path stopped
>   being evidence.
> * **`PlatformReboot` is dispatched.** Capability-checked, one MMIO write, and
>   `Reboot` (120) is retired from the enum. `bootctl` holds the `Reboot`
>   capability it calls with; the reset device is mapped kernel-only into
>   every task. Not yet exercised by anything real (see below).
>
> **What is still true, restated so it is not read as more than it is.**
>
> * **No reboot has actually happened.** Console-triggering `svc-fault`
>   produces `HealthVerdict::NoFallback`, not `MustRollBack` — see next.
> * **The block is still not durable**, for the reason §A already gives.
>
> **What was found reaching this point, structural, not a bug to fix here.**
> `BootControlBlock::new` starts slot A both active *and* last-confirmed — the
> factory image is its own fallback until something better is confirmed — and
> this line never stages a second candidate slot (D7: no slot switching, one
> kernel image). So the active slot **is** the last-confirmed slot here,
> always: `fail_health()` correctly refuses to mark the system's only fallback
> unbootable, `apply_health(false)` returns `NothingToRollBackTo`, and
> `bootctl` does not reset — matching "fail on positive evidence only" and the
> 2026-09-22 correction's own warning that a reset with nothing durable to
> bound it would loop, by making the reset itself unreachable rather than by
> bounding it. **`MustRollBack` — the verdict an actual reset follows from —
> cannot happen in this deployment without `bootctl` staging a nominal
> candidate slot at boot**, which is a further decision about the block's
> operational semantics that RFC-0.33-001's mid-line ruling did not make.
> Escalated, not decided here.
>
> The rest of E-044 is still not closed by this note.

## Security Boundary Impact

Boot-control write must require a capability (v0.2: RFC 031 + RFC 038). At v0.1.x the barrier is policy-level only.

## Deferred Work

- Capability-enforced bootctl IPC (v0.2: RFC 031 + 038).
- Remote rollback notification (v0.4).

## Related RFCs

- RFC 002, RFC 008, RFC 023
- RFC 038 (Service Plane Separation, v0.2)
