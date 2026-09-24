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

## Settled at the second mid-line ruling, 2026-09-23

**D14 — F7 is declined: `bootctl` does not stage a nominal candidate slot.**
The block would then record `active = B, candidate = B, last_confirmed = A`
while **one image exists and B holds nothing**. That is a state machine
asserting a fact about the world that is not true — the same class as the six
simulated markers D11 removed one commit earlier, and it would be worse for
being inside the structure the whole line exists to make trustworthy. The
`health-fail` profile stands as delivered, with `expect_shutdown` deliberately
absent; **that judgement was right.**

**D15 — the reset mechanism is demonstrated on its own, and honestly.** D5's
purpose was that a reset be observed rather than claimed. With one image the
*decision* cannot reach a reset (D14), but the *mechanism* — dispatch arm,
capability check, MMIO write, and R6's harness — must not ship unexercised.
So: a negative-test profile in which **`neg-test` calls `sys_reboot`**:

- **with** the `Reboot` right → the machine resets, observed through QMP's
  `SHUTDOWN{guest:true, reason:"guest-reset"}`, never through a marker;
- **without** it → refused, with the error observed.

**A fifth kernel touch is approved for this and nothing else**: granting
`Reboot` to `NEG_TEST`, exactly as its `TaskControl` grant already is. Disclosed
in `v1-limitations.md` beside D10's console trigger — `neg-test` is already an
intentional test target in the shipped image, so the precedent is the
disclosure, not the capability.

**The two halves are named as two halves.** The record says: the health
*decision* is demonstrated end to end (`health-fail`); the *reset* is
demonstrated end to end (`reboot`); **the two are not joined in this
deployment**, and joining them needs slot switching (D7) and a durable block
(§A). Neither profile may imply otherwise.

**D16 — R9's survivor list**, at closing: no reset from a health decision here;
the four remaining undispatched syscalls; and the durable-counter risk — for
which **the implementer's sentence is adopted over my D13 text**, because
theirs describes what was measured (`NoFallback`, not a loop) and mine
predicted a loop that this structure cannot reach. I write it into
`v1-limitations.md`; they do not edit that file.

## Settled at the third mid-line ruling, 2026-09-24 (D15's review)

**D17 — the reset trigger is refused, and a sixth kernel touch is approved to
replace it.** `neg-test` resets the machine when it finds a virtio **entropy**
device among the MMIO slots, and the profile supplies one. Three facts decide
this:

1. **`neg-test` is spawned unconditionally** — `init` line 469, no condition — so
   the scenario ships in every boot.
2. **The trigger survives the reset.** The device is still attached afterwards, so
   a machine that has one resets, boots, resets again: **a reboot loop**, on
   hardware whose only fault is having an entropy source.
3. **The tier passes only because `-no-reboot` is set.** QEMU exits instead of
   rebooting, so the loop is invisible exactly where it would be observed.

An entropy device is ordinary hardware. A node that power-cycles because one is
attached is not a test affordance; it is a trap, and the loop cannot be broken
from inside because nothing is durable (D13, F6).

**The rule this establishes: a trigger for a reset must not survive the reset.**
The console byte does not — it is injected once, by an external agent, and is
absent on the next boot. So: **`neg-test` gets its own endpoint object and `init`
a send capability to it** — the sixth kernel touch, mirroring `bootctl`'s from
D8 — and the reboot scenario runs on an injected byte, as D10's does. The shared
object 0 they rightly rejected is what the new endpoint removes.

*Everything else in D15 stands and is accepted*: the grant confined to one slot,
the `PermissionDenied` case with its marker, the harness judging QEMU's own
`SHUTDOWN{guest-reset}` with a live control that a reset which did not happen
fails, the forbidden `REBOOT_RETURNED` marker, and the two halves named as two
halves.

**D18 — E-044 is not closed yet, and this is why.** Its closure would record a
demonstrated reset armed by a trigger I have just refused. Close it in the commit
that lands D17's trigger, with these survivors — my text, for the register, since
`v1-limitations.md` is mine and already carries them:

- **no reset follows a health decision in this deployment** — the active slot is
  also the last confirmed one, so `fail_health` correctly refuses to strand the
  system; joining the decision to the reset needs slot switching (D7) and a
  durable block (§A);
- **four syscalls remain undispatched** (`CapInstall`, `TaskKill`, `MmioUnmap`,
  `DmaShare`);
- **nothing durable bounds a reset**, so an organic health failure would loop —
  which is why the failure path is reachable only through a deliberate trigger.

**D19 — the four new evidence tiers leave nothing committed behind, and the
harness says so on every run.** `health-fail`, `reboot`, `semantic-absent` and
`semantic-braille` are the only profiles with no committed
`tests/qemu/artifacts/<name>/expected-markers.txt`; the other fourteen have one,
and the runner prints *"if `reboot` is meant to be a gated profile, commit its
expected-markers.txt deliberately"* each time. The **specification** is safe —
it is `expected_markers` in the committed profile — so this is not a fail-open;
what is missing is the **evidence artefact** a reader or a citation can reach.
Commit all four deliberately, in the commits that land D17's trigger and
RFC-0.34-001's crash tier.

*Two things checked here that turned out to be sound, recorded so they are not
re-checked:* the `reboot` tier finishing in under a second is **real** — the
guest resets and QEMU exits, while the 60-second tiers are waiting out their
timeout (controlled against `dma`: 60s). And your profile comment is right that
the entropy device keeps the reset out of the other eighteen profiles. D17
refuses the trigger on the machine it would meet outside QEMU, not on
cross-profile leakage — and that comment changes with the trigger.

## Settled at the fourth ruling, 2026-09-24 (D17–D19's review)

**D20 — `ce37be9` is ratified, and the class it belongs to is named.** Clearing
`satp` at entry is right, minimal and in the right place: after the hart check and
**before** the first store, which is what the BSS fill is. Finding it by running
the reset without `-no-reboot` is the better half of the work — it is the failure
case run, not the absence observed, and it turned my D17 prediction (a reboot
loop) into the truth (a reset into a hung machine). That correction stands in the
record.

**But `satp` is not the only supervisor CSR a reset leaves behind, and the fix
must say why the others need nothing.** Read out of the tree, not assumed:
`m_mode_setup` writes `mstatus = 1 << 11` **wholesale**, which clears `MIE` and
`MPIE`, so `mret` leaves S-mode interrupts disabled and no stale `sie` or `stvec`
can be reached through an interrupt before the kernel installs its own; PMP is
rewritten every boot. What survives unexamined is `sie`, `stvec`, `sscratch`,
`sepc`, `scause` — and `stimecmp` if SSTC is ever used. They are harmless only
while **no exception** occurs between `mret` and the kernel's own `stvec` write,
which holds today precisely because `satp` is now zero and PMP is permissive.
**Put that paragraph beside the `satp` clear** — one comment naming each CSR and
why it needs no instruction. Not more instructions: clearing a register blindly is
not better than knowing why it does not need clearing, and the next person to add
an early store deserves the reasoning, not the list.

**D21 — yes: "the machine boots again after its own reset" becomes a gate.** Your
question answers itself. The defect lived exactly where no gate looked, and a
script run by hand is the instrument class this project refuses to count — the
same shape as a skipped job read as a pass (E-041) and a green job that fuzzed
nothing (E-043). Two further reasons: E-044's survivor list now carries *"no tier
boots the machine a second time"*, and a survivor that a day's work can retire
should not be carried into v1; and the trigger is already there — the `R` byte —
so this needs **no new affordance**.

Shape, so it is not larger than it must be: a profile field (`expect_boots = 2`
or equivalent) that makes the harness run **without** `-no-reboot`, inject once,
count the boot banner and judge the count. Two controls, both required in the
evidence: **no byte → one boot**, and **`ce37be9` reverted → the tier red**. The
second is the one that matters; a gate that has never been seen failing is not
yet a gate.

**D20.1 — the satp claim is reproduced here, and D21's failing case is already
run.** Three runs of your own script, at your tip, from the repository root:

| kernel | command | result |
|---|---|---|
| tip (`ce37be9` in) | `reset_boots_once.py 45 R` | `injected=True` **boots=2** |
| tip (control, no byte) | `reset_boots_once.py 25 -` | `injected=False` **boots=1** |
| **`ce37be9` reverted**, scratch worktree, kernel rebuilt | `reset_boots_once.py 45 R` | `injected=True` **boots=1** |

The third is the one that matters: same command, same byte, and the machine does
not come back. Its log ends exactly where you said — `Fjell OS kernel started` …
`mm: frame allocator ready`, then 158 bytes and silence for the rest of the run.
**So the gate D21 asks for is already known to be capable of failing**; what
remains is wiring the harness mode, not discovering whether the check has teeth.
The scratch worktree was removed and the main tree is unchanged.

**D22 — E-064, filed at this review, belongs to this line's boot path.** The BSS
zero-fill overwrites `a1` — the DTB pointer firmware passes — three lines above
the comment saying it does not, so `kmain` receives `__bss_end` (`0x8007ccb8`
here). Nothing has a symptom because `platform::detect` ignores the tree, and the
reserve that exists to keep firmware's device tree out of the free pool fails on
its first frame and has its error discarded — so **the real DTB page is
allocatable**. Same file, same twenty instructions, same class as the comment that
was true until the machine could reset itself. Fix it with D20's comment, and show
the received value first.

## Settled at the fifth ruling, 2026-09-24 (D20–D22's review)

**D23 — the gate is accepted, and the tier claims more than it asserts.**
`expect_boots` with an exact count is a real assertion, and `sched: started` is
the right banner for the reason you give: counting the first line would have
counted the hung second boot as a boot. The loader's three refusals, the recorded
`boots.txt` before judging, and both controls — run, with the reverted `satp`
clear turning the tier red — are what D21 asked for.

**But the profile's header says** *"every boot must find and reserve firmware's
tree, and the reset boot receives the pointer again"*, **and nothing enforces it.**
`expected_markers` match anywhere in the log, so all four are satisfied by the
first boot alone; a second boot that printed `sched: started` and then died would
still pass. As it happens the claim is true — counted in the tier's own log:
`mm: device tree reserved` **2**, `driver-uart: ready` **2**, `sched: started`
**2** (and `M6: storaged ready` **4**, which is E-063's two writers, twice) — but
true and unenforced is the shape this line exists to correct.

**Make the count generic**: a marker that must appear once per boot carries a
required count, and at least one of them is a **service**-level marker.
`driver-uart: ready` is the natural choice — it is already the injection hook — so
that *came back* means the machine reached the readiness the first boot did, not
merely the end of the kernel's own boot. Correcting the header instead would be
the weaker half of the fix.

**That also answers your second question.** No "stop after N boots" mode: a hang
after the second banner is exactly what the per-boot service marker catches, and
it costs less than a new way to end a run. The 60-second wait is what its
neighbours do; leave it.

**D24 — the dependency, not the testability, is what I refuse.** Choosing a
host-tested function over an inline check with no test was right. But `fdt_extent`
is **14 lines** and uses **neither** of `fjell-dtb-validate`'s two dependencies
(`fjell-platform-format`, `fjell-measure-format`) — so the most privileged
component in the tree gained a 645-line crate and two format crates to read an
8-byte header. Split those 14 lines, their constants and their tests into a leaf
with no dependencies (or into a crate the kernel already has), and leave the
boot-handoff validator outside the kernel until E-048 wires it deliberately. The
kernel's dependency list is its trust surface, and it is easier to keep short than
to shorten later.

**D25 — E-064's closure verified, live.** The failing case was shown first as the
ruling asked (`dtb_pa = 0x8007ccb8 = __bss_end`). At this tip: the tree is found at
`0x87e00000`, 5,044 bytes, its **real extent** reserved, the free-frame count
`32131 → 32129` — the two pages that were allocatable before — and **both boots**
print all three lines. The reserve's failure is printed instead of discarded, and
the header is checked before anything is stored or reserved. What survives is
correctly named: only the header is read, and no path here has run on a real
board's tree.

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
