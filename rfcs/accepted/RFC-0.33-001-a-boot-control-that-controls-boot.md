# RFC-0.33-001: A boot control that controls boot

**Status:** Accepted — by the owner (nabbisen), 2026-09-22; implementation may begin (RFC 000)
**Milestone:** 0.33
**Tracks.** **E-044** — the A/B boot-control state machine has no runtime
client, and the reboot it depends on is not dispatched.
**Touches** *(indicative, not exhaustive)*: `crates/services/fjell-bootctl/`,
`crates/services/fjell-service-manager/`, `crates/fjell-kernel/` (one syscall
and one MMIO write), `crates/fjell-abi/` (a retired syscall number),
`crates/formats/fjell-upgrade-format/`, `crates/fjell-bootctl-model/`,
`tests/qemu/profiles/`, ADR-0009.
**Relates to:** E-046's line, which reworked the `BootControlBlock`'s checksum;
E-034 (syscall helpers taking words the kernel never carries); RFC-0.29-001 D5
(a profile that fails honestly rather than placeholder-passing).

## Summary

Re-derived 2026-09-22. The erratum said the mechanism exists "only as parts".
Three of those parts are further apart than it recorded.

### Finding 1 — the running service holds a three-state enum, not a boot-control block

`fjell-bootctl` is **74 lines**. Its entire state is:

```rust
enum BootState { Pending, Confirmed, Rollback }
```

It never constructs, reads or writes a `BootControlBlock`. *(Corrected at the
mid-line ruling: **`fjell-init` does** — it constructs, seals and writes one to
both mirrors every boot, `LBA_BOOT_CTL_A/B_START`, and sends `storaged` six
sector writes. Nothing **reads** one back. The claim below is true of reading,
not of writing, and the difference is why persistence is out of scope: a
persisted block would be clobbered at the next boot.)* Of reading:
the type, its mirrors, `active_slot`, `last_confirmed_slot`, `candidate_slot`,
`remaining_tries` and `confirmed` exist in `fjell-upgrade-format` and are
touched at runtime by nothing. So the A/B model has no runtime representation
at all — not merely no client.

### Finding 2 — there are two boot-control protocols, and the service implements the other one

`fjell_service_api::bootctl` declares `READY`, **`READ_BCB`**, **`WRITE_BCB`**,
`READ_OK`, `WRITE_OK`, `ERR` — a protocol for moving a boot-control block.
**Nothing implements or sends any of it.** The service instead answers
`tags::BOOT_PENDING_QUERY`, `BOOT_CONFIRM`, `BOOT_ROLLBACK`, `BOOT_SHUTDOWN`,
which nothing sends. Two protocols, zero users, one service.

### Finding 3 — the kernel cannot reset the machine

`syscall-surface`: **35 declared, 29 dispatched, 6 undispatched.** Two of the
six are `PlatformReboot` (18) and `Reboot` (120) — the second declared as an
alias of the first, and both dead. `sys_reboot` exists and issues `ecall 18`,
which no dispatch arm handles.

**And there is no reset path to dispatch to.** The kernel contains no SBI call,
no `syscon` write, no SiFive test-device write — searched for all three. On
QEMU `virt` with `-bios none` there is no firmware to ask, so a real reset means
the kernel writing the board's reset device itself, which is a new MMIO site
under the MMIO-ordering gate and a device the `BoardProfile` does not list.

The rollback arm therefore ends: `let _ = sys_reboot(...); loop {}` — an error
discarded, then a spin.

### Finding 4 — the verified part is the part nothing runs

`fjell-bootctl-model` models health failure and last-known-good fallback, with
tests, and **no crate depends on it**. Mirror selection has four tests in the
format crate *and* a Verus proof (`verification/verus/boot-control/`). The
best-verified component in this area is a model of a runtime that does not
exist; the service that does run is the least verified thing in it.

### Finding 5 — health already has a source, and no evaluator

ADR-0009 says a real health check "compares running services against a
`HealthTarget`". *(Corrected at the mid-line ruling: **no `HealthTarget` type
exists** — re-checked with a word boundary and a control. And
`fjell-service-manager` tracks **readiness only**: entries are created on
`READY` with `task_handle: 0`, so its fault branch is unreachable.)* The signal
that does exist is readiness; faults need `init` to register task handles, which
is D9.

### Finding 6 — a QEMU tier is already waiting for this

`tests/qemu/profiles/upgrade.toml` expects four markers, one of which is
`NEG:UPGRADE:HEALTH_FAILURE_NOT_CONFIRMED:PASS`. Nothing emits any of the four,
so the tier fails honestly and is not release-gated (RFC-0.29-001 D5). The
other three belong to `verifyd`/`upgraded`, not here.

## The settled part

**D1 — `bootctl` owns a real `BootControlBlock`.** The service's state is the
format crate's type — slots, generation, `remaining_tries`, `confirmed`,
`candidate_slot` — not an ad-hoc enum. The transitions it performs are the
ADR's, and `fjell-bootctl-model` is the model it is checked against, so the
model gains the dependent it has never had.

**D2 — One protocol.** Either `BOOT_*` or `bootctl::{READ,WRITE}_BCB` survives;
the other is deleted, not left for a reader to guess at. Whichever survives
carries the block.

**D3 — Health is evaluated where the evidence already is.**
`fjell-service-manager` decides, from readiness and faults it already tracks,
and says so to `bootctl`. No new health-reporting plane.

**D4 — The kernel dispatches one reboot syscall, capability-checked**, and the
duplicate is retired through the ABI process (snapshot re-recorded, not
silently dropped). A `Reboot` capability without the right fails closed.

**D5 — A real reset happens in QEMU.** Not a marker claiming one: the machine
resets or powers off, and the tier observes it.

**D6 — Demonstrated failing on the health path**, in a profile of this line's
own: health fails, the candidate is not confirmed, and the rollback path runs
to the reset. Emitting `NEG:UPGRADE:HEALTH_FAILURE_NOT_CONFIRMED:PASS` in
`upgrade.toml` does **not** make that tier gated — its other three markers
belong to other lines, and a tier is gated when all of its markers are real.

**D7 — What this line does not claim.** It does not boot a different image.
There is one kernel image in this system, so "roll back to the last confirmed
slot" can set state, mark the candidate unbootable and reset — it cannot select
a different slot at the next boot, because nothing chooses images. **ADR-0009
is corrected to say which half exists.**

## Settled at the mid-line ruling, 2026-09-22

The implementation delivered D1, D2 and R8, stopped where the handoff said to
stop, and asked six questions the RFC did not anticipate. **Four of my own
figures were wrong**, corrected in the findings above. The rulings:

**D8 — F1: all four kernel touches are approved**, one commit each, `test-all`
green before any is pushed. My handoff said "one dispatch arm and one reset
site"; that was written without knowing the other three were prerequisites, not
extras:

| Touch | Why it is not optional |
|---|---|
| the `PlatformReboot` dispatch arm | the syscall the line exists to make real |
| map the reset page kernel-only into task address spaces | a store to an unmapped `0x100000` from the syscall handler faults **in the kernel** — a hang, which is the outcome D5 exists to rule out. Mirrors `plic::MAPPED_PAGES` |
| install a `Reboot` capability in `bootctl`'s CSpace | **no `CapKind::Reboot` is granted anywhere**, and `CapInstall` is undispatched, so there is no user-space path to do it |
| a dedicated `bootctl` endpoint | it receives on shared object 0, which other services race for — RFC-0.28-001 fixed exactly this for three services and left this one |

**The boundary still holds otherwise:** nothing else in the kernel.

**D9 — F2: readiness *and* faults**, with `init` registering task handles.
`service-manager` tracks readiness only — entries are created on `READY` with
`task_handle: 0`, and the fault check requires `task_handle != 0`, so that branch
is unreachable. Verified at review. A health evaluator that cannot see a crash is
not a health evaluator: a service that never sent `READY` and one that died look
identical to it. **`HealthTarget` is retired** — it does not exist.

**D10 — F3: the console-injected trigger is approved, with one hard constraint.**
**The trigger's only power is "spawn this fault service".** It must never request
a reset, or the profile would be testing the console rather than the system's
decision. The reset must still be `bootctl`'s own conclusion from the health
report. The affordance is named in `v1-limitations.md` as a test hook present in
the shipped image — `svc-fault` and `svc-timeout` already are, so the precedent
is the disclosure, not the hook.

**D11 — F4: `service-manager` reports healthy, `bootctl` confirms and prints.**
And **`init`'s simulated `M6:`/`M7:` markers go in this line**, once the real
confirm path prints — not left beside it. Two sources for one fact, one of them
hard-coded `true`, is how a confirm path stops being evidence. If removing them
breaks a tier, file it rather than keeping both.

**D12 — F5: do not reopen E-046.** It shipped, and it fixed the receive path it
named; a shipped erratum stays closed (the register's own property 2). The four
write-side sites are **E-055**, tracked 0.33, to be done with **E-045**'s schema
work because both change on-disk bytes and neither should do it twice.

**D13 — F6: failure on positive evidence only**, approved — and the deeper
consequence stated: **with nothing durable, a reset cannot be bounded from
inside.** A genuine health failure on a real node would reset, lose the
in-memory decision, and fail again. So until the block is durable the failure
condition must be reachable **only** through D10's trigger, and
`v1-limitations.md` must say that a real health failure would loop. That is an
**E-044 survivor**, not a detail.

**§A (persistence) is confirmed out of scope**, on their four blockers — and the
fourth is one of mine: `init` writes a `BootControlBlock` to both mirrors every
boot, so a persisted block would be clobbered at the next boot. The RFC said no
crate writes one. It does.

## The open questions

**§A — Does the block survive the reset?** Persisting it means a store client:
`storaged` has a sector protocol, and no crate sends it a write today. Without
persistence, a reset loses the decision, and the rollback marker is emitted
before a reset that forgets it.
**My lean: persist, if the store path can be shown working first** — a
reset that forgets is a demonstration of a reset, not of boot control. If the
store path cannot carry it, say so plainly and scope this line to the decision
plus the reset, with persistence named as the follow-up, rather than pretending
the state is durable.

**§B — Which protocol survives** (D2)? `BOOT_*` is what the service implements
and what `service-manager` would speak; `READ_BCB`/`WRITE_BCB` is what a block
carrier looks like. **My lean: keep `BOOT_*`, extend it to carry the block, and
delete the unimplemented pair** — but argue it, because the deleted names are
in the ABI's protocol surface and someone chose them.

**§C — `PlatformReboot` (18) or `Reboot` (120)?** One is dispatched and the
other deleted. **Lean: keep 18**, whose name says what it does, and retire 120
with an ABI snapshot entry. Say what `syscall-surface` reports afterwards — it
will be 34 declared, 30 dispatched, 4 undispatched.

**§D — How does the kernel reset the board?** QEMU `virt` exposes a reset
device; the `BoardProfile` does not list one, and the MMIO-ordering gate covers
every MMIO site. Propose: the device, where its address comes from (profile
entry, not a literal), and how the write is audited. **If you find the address
can only be a literal today, say so** rather than adding a profile field that
nothing fills.

**§E — What does `service-manager` do when health fails?** Send
`BOOT_ROLLBACK`, or report and let `bootctl` decide? **Lean: report the
observation, let `bootctl` decide** — the state machine is the ADR's, and a
health reporter that also decides is two mechanisms in one place.

**Answer all five in writing before implementing.**

## Requirements

**R1 — Re-derive** each finding, with a positive control for every absence:
the 74-line service, the two protocols, the undispatched pair, the absent reset
path, the model's lack of dependents, and the tier's markers. Report
disagreements.

**R2 — D1 and D2:** the block as the service's state; one protocol; the
model as the checked model, with the tests that tie them.

**R3 — D3 and §E:** health evaluated from readiness and faults, with the
`HealthTarget` type either used or explicitly retired.

**R4 — D4 and §C:** one reboot syscall dispatched with a capability check;
the other retired; the ABI snapshot re-recorded; `syscall-surface`'s new
numbers stated.

**R5 — §D:** the reset implemented, the MMIO site audited, and the address
sourced as §D answers.

**R6 — D5 and D6:** a QEMU profile of this line's own in which health fails,
the candidate is not confirmed, and the machine resets — plus the confirm path
observed in a smoke tier. Both by marker, and the reset by the machine's
behaviour, not by a marker asserting it.

**R7 — §A answered**, and if persistence is in scope, the block read back
after the reset.

**R8 — ADR-0009 corrected** (D7): what exists, what does not, and that
"blocked on the preemptive scheduler" was never established — no reboot syscall
was dispatched, which is a different and smaller obstacle.

**R9 — E-044 CLOSED**, or its survivors named — the undispatched syscalls this
line does not reach are survivors, not silence; register and `v1-limitations.md`
in the same commit.

**R10 — The gates**, each by its own exit status, `test-all` with the new
profile, and a CI run id.

### Non-goals

- **Booting a different image** (D7). No bootloader, no second slot on disk.
- `upgraded`'s staging, and active-slot write protection.
- The other three `upgrade.toml` markers.
- Timer preemption.
- The four remaining undispatched syscalls (`TaskKill`, `MmioUnmap`,
  `DmaShare`, `CapInstall`'s unsafe handler) — named in E-044, not fixed here.

## Risks

**This is the first line this milestone that can break a boot.** Every other
0.32 line was instruments, documents or host-side code. A wrong MMIO write or a
mis-ordered reset leaves a machine that does not come back, and the QEMU tiers
are the only place that shows.

**A reset in a test tier can hide a hang**: a machine that resets looks, to a
marker-matching harness, much like a machine that stopped. The profile must
distinguish them — the markers before the reset, and the harness's own view of
why the run ended.

**Persistence (§A) can quietly become the whole line.** If the store path needs
work to carry a block, that is a second line, and this RFC would rather be
narrow and true than broad and mostly deferred.
