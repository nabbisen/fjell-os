# RFC-0.48-001: A bootstrap capability can be taken back — measured first, designed second

**Status:** Implemented (0.48.0) — accepted 2026-11-25
**Evidence:** marker:svc:NEG:BOOTLEASE:OWNER_REVOKE_OK:PASS, marker:svc:NEG:BOOTLEASE:ENDS_WITH_SUBJECT_FAULT:PASS, marker:svc:NEG:BOOTLEASE:CHILD_SEND_REFUSED_LEASE_REVOKED:PASS, marker:policy:NEG:LEASE:INSPECT_REPORTS_EPOCH:PASS, marker:policy:NEG:LEASE:SLOTS_RECYCLED:PASS, file:tools/fjell-consistency-check/src/bootstrap_leases.rs, test:tools/fjell-consistency-check/src/bootstrap_leases.rs::an_unbound_literal_in_a_new_function_is_red_and_named
**Milestone:** 0.48
**Tracks.** **E-093** — `RFC-v0.7.4-003` removed kernel-local recursive revocation on the premise that *"every
meaningful cap MUST have a lease"*. The premise is false for **boot-time grants**, and this RFC measures how
false, what it would cost to make true, and what making it true would **not** do. E-093's disposition (and
`v1-limitations.md` row 12) are the architect's and the owner's; this RFC is evidence for them.
**Touches** *(indicative, depends on the ruling in D1)*: `crates/fjell-kernel/src/task/spawn.rs`,
`crates/fjell-kernel/src/main.rs`, `crates/fjell-kernel/src/lease/mod.rs`, `crates/fjell-cap/src/slot.rs`,
`crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/capability.toml`,
`tools/fjell-consistency-check/src/` (one subcheck), prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.46-001** (lease recycling and quotas — this RFC spends the capacity it created),
**RFC-0.46-003** (who may revoke a lease), **RFC-0.46-002** (runtime installs are lease-bound),
**RFC-0.47-001** (one funnel — what makes a lease check *total*).
**Not in this line:** `DELEGATE_ONLY`; any change to what `cap_revoke` does to the capability it names; line 2's
sweep (which runs after this lands, as the handoff requires).

*Every figure was re-measured at `587778d`, not copied from the handoff. Where the tree and the handoff differ, the
section "Where the handoff and the tree differ" says so. Measurements marked **boot** were taken on a real QEMU
boot of the `m8` image in a scratch worktree carrying a print-only patch that was never committed.*

---

## What is true now

**There is no involuntary revocation of a boot-time capability.** The handoff's statement holds, verified:

* `Capability::check_lease` returns `Ok(())` when there is no binding (`fjell-cap/src/slot.rs:51`–`:56`). The
  funnel's lease step (RFC-0.47-001, step 7) therefore passes an unbound capability **forever**.
* **55** `Capability { … lease: None }` literals at the boot-time grant sites — **40** in `task/spawn.rs`, **15**
  in `main.rs`, none under `#[cfg(test)]` (re-run: the handoff's command, then a scan for `#[cfg(test)]`, which
  neither file contains). By kind: **33 `Endpoint`**, and **22 non-endpoint** — `LeaseAdmin` 5, `TaskControl` 3,
  `TaskCreate` 2, `Reboot` 2, `MmioRegion` 2, `Interrupt` 2, `Clock` 2, `CapInstall` 2, `DmaRegion` 1,
  `AuditDrain` 1.
* `sys_cap_revoke` operates on the caller's own CSpace and deletes **descendants**, not the capability it names
  (`CSpace::revoke`: *"The capability at `h` itself is NOT deleted"*). Parent links exist for **mint** and are
  severed for **copy** (`new_cap.parent = None`, deliberate, `cspace.rs:171`–`:182`).
* Runtime installs are always lease-bound (BROKER-004; `LeaseFact::Zero` is refused). **The hole is boot-time
  grants only**, as the handoff corrected E-093's text to say.

### What the boot actually grants — the figure the arithmetic needs (boot)

The 55 literals are *source sites*; a site inside a per-image `match` arm runs for every image that matches.
Measured on the `m8` boot, by counting each task's occupied slots when `spawn()` returned:

| | Figure |
|---|---|
| Tasks created by `spawn()` that receive at least one capability | **30** |
| Capabilities installed by `spawn()` across those 30 | **101**, all `lease: None` |
| Most any one task holds | **22** (`neg-test`); the next are 6, 5, 5, 5, 5 |
| Plus `init`, bootstrapped directly in `main.rs` | **1** task, 15 literal sites |
| `MAX_TASKS` | 40 |

**The number that governs the design is therefore 31 tasks, not 55 grants.**

### The capacity arithmetic — and my recommendation's problem, confirmed

| Constant | Value | Where |
|---|---|---|
| `MAX_LEASES` | **32** | `lease/mod.rs:23` |
| `MAX_LEASES_PER_OWNER` | **16** | `lease/mod.rs:30` |
| `MAX_TASKS` | 40 | `task/tcb.rs:197` |
| tasks holding boot grants | **31** | measured (boot) |

* **One lease per grant** needs ≥ 55 (101 at runtime) of 32 — impossible, as the handoff said.
* **One lease per task needs 31 of 32 — it nominally fits and cannot work**, because the broker's runtime
  delegation draws on the same table. Measured (boot) lease creations, by owning task: **`neg-test` 104,
  `cap-broker` 7, `sample-service` 2**; peak live **17**. Outside the test churn the steady state is **2–3 live
  leases**. The broker alone may hold `MAX_LEASES_PER_OWNER` = 16 (R-Q) and each requester 4 (R-G). **31 boot
  leases + 16 broker + a handful ≈ 50 > 32**: the table that 0.46-001 made recyclable would be exhausted by boot
  before the first delegation.
* **`MAX_LEASES_PER_OWNER` = 16 breaks any design that makes one task own the boot leases** — 31 > 16 — unless
  boot leases are exempt from the quota (D3).
* **Enlarging the table is cheap and proof-free, verified rather than assumed.** `verification/verus/lease/
  lease_epoch.rs` contains no capacity token (no `MAX_LEASES`, no table length; the only `32`s are `u32`/`u16`
  widths). `fjell_abi::lease::lease_alloc` takes a `&[SlotView]` slice, so it is length-agnostic. A
  `LeaseObject` is 12 bytes (`state`, `generation: u16`, `epoch: u32`, `owner: TaskId`), so **128 slots is
  1.5 KiB**; `cursor` is a `u8`, which holds 128. The *only* code that names the capacity besides the table
  itself is **`neg-test`'s recycling scenario** — which cycles *"more than the 32 slots"* — and would stop
  exercising reclamation at 128 (D5).

### What revoking a lease takes back — **the axis is MMIO versus DMA, not per-call versus resource** *(corrected at the stage-1 review)*

The handoff's example is *"an operator who decides a service must lose its MMIO window"*. **Lease-binding the
`MmioRegion` capability does not deliver that**, and the mechanism is sharper than *"nothing ever unmaps it"*:

* **`MmioUnmap = 91` is declared in the ABI (`fjell-abi/src/syscall.rs`) and has no dispatch arm at all** — one of the
  three syscalls `v1-limitations.md` row 7 records as declared and never dispatched. **The unmap primitive does not
  exist as a reachable call.** The only `unmap_page` in the syscall layer is the failed-map rollback
  (`trap/syscall.rs`, *"Rollback: unmap all pages that were successfully mapped"*). A task that has mapped its window
  **keeps it after the lease is revoked**; revocation takes away only the right to map again.
* **DMA is different, and my first draft over-generalised.** `DmaRevoke = 112` **is** dispatched, requires
  `CapKind::DmaRegion` + `CapRights::DMA_REVOKE` through the funnel, and unmaps the owner's page **verified**
  (`unmap_dma_region_verified`, RFC-0.39-004: the frame is zeroed and read back before it is returned, and a read
  through the revoked address faults — the `dma` profile's `NEG:DMA:REVOKED_VA_READ_ATTEMPTED` plus the kernel's own
  `fault(LoadPageFault)` line is the standing positive case). **The project already knows how to reclaim a resource
  through a capability-checked primitive, and has done it for DMA and not for MMIO.** (Revoking the *lease* does not
  invoke it either way — `sys_dma_revoke` is called by the holder — but the primitive and its checks exist.)
* Per-call authority is unaffected by any of this: `sys_irq_*`, `sys_platform_reboot`, `sys_clock_now`, `sys_task_*`,
  `sys_lease_*`, `sys_cap_*`, `sys_audit_*` and every IPC syscall consult the funnel **on every invocation**, so a
  revocation is immediate for them, and blocked IPC is woken with `LeaseRevoked` (RFC 034/050).

**So the honest statement for any of (a)–(c) is:** *a boot-time capability's authority can be taken back for every
future use; an MMIO mapping already obtained is not reclaimed, because the unmap call does not exist.* Closing that is
a **separate line** — dispatch `MmioUnmap` and track a mapping to the capability that created it — and the RFC does not
scope it in.

### Where the handoff and the tree differ

* The handoff's **55** counts **source literals**; the number of capabilities installed on a boot is **101 (+ init's)**,
  and the number that matters for leases is **31 tasks**.
* The handoff's *"lose its MMIO window"* is not delivered by lease revocation (above) — because the unmap syscall is
  declared and never dispatched, not because revocation is shallow in general: DMA has the primitive.
* The handoff's **"one lease per task needs up to `MAX_TASKS` = 40"** overstates: the tree has 31 holders. The
  conclusion stands for a different reason — the broker's runtime draw.

---

## The decisions this RFC asks for

**D1 — which remedy. RULED: (a), one lease per task, `MAX_LEASES` 128.** *The measurement decided it (31 holders; the
broker's draw; `MAX_LEASES_PER_OWNER` = 16 against 31), not a preference.* **Two conditions, both taken:**
(i) **`neg-test`'s recycling scenario must still drive the table to exhaustion** — it names 32, and raising the table must
not silently retire the only negative test that exercises reclamation; its bound is **derived from the capacity, not a
second literal**, and I will show it red against a table it can no longer fill. (ii) **What 1.5 KiB buys, said plainly:**
it buys **coverage** — lease revocation reaches what boot granted — and **does not** buy recursive revocation for copies
(option (b), rejected) **or** reclamation of an MMIO mapping (above).

The handoff's four, assessed against the measurements. **My recommendation is (a), with
one lease per task, a larger table, and a stated scope (D2)** — not because the handoff recommended it but
because it is the only option that makes the *existing* primitive total, and because the capacity objection that
made me doubt it is a number, not a proof obligation.

| | What it does | Cost, measured | What it does not do |
|---|---|---|---|
| **(a) lease per task** | every boot capability of a task is revocable at once, per-call authority immediately | `MAX_LEASES` 32 → 128 (1.5 KiB, **0 proof obligations**); 31 `lease_create`s at spawn; the `check_lease` path is already on every capability | does not reclaim a mapping or frame; revokes a task's authority **coarsely** (all or nothing) |
| (b) stop severing `parent` in copy | `cap_revoke` cascades to copies | contradicts `cspace.rs`'s documented decision; **no kernel growth** | **closes half the hole**: still per-CSpace; copies *are* how a task hands authority to itself under another slot, so it also changes what "independent" means |
| (c) cross-CSpace revoke | an authority holder deletes a named capability in another task | needs an authority model and a **second revocation primitive** beside leases; RFC-0.46-003's ownership rule would need restating for capabilities | the most direct, and the largest |
| (d) narrow the claim | document permanence | zero code | **fails the design bar** unless the owner rules permanence intended (scope question 2) |

*What 0.46-003's ownership rule becomes under (c):* it would not survive unchanged — a lease is revoked by its
owner, but a capability in another task has no owner but the task, so (c) needs either "the installer owns what
it installed" (true only for runtime installs) or a new right. That asymmetry is the argument that (c) is a
second model rather than an extension of the first.

**D2 — scope of the lease binding.** *All* 31 tasks, or only the tasks holding non-endpoint (privileged) kinds?
Measured: **22 of the 55 literal sites are non-endpoint**; a lease per *privileged-holding* task would be fewer
leases (fewer than 31) but leaves an operator unable to revoke an endpoint — to cut a service off from storaged.
**Recommendation: every task that receives a boot grant gets one lease, `init` excepted** (D4). *Ruled: accepted.* Uniformity is the
point; a privileged-only rule is a second, hand-curated list (the E-125 family) deciding which kinds count.

**D3 — who owns a boot lease, and whether a second `LEASE_REVOKE_ANY` holder is needed. RULED on ownership; the
architect asked me to check whether owner-based revocation suffices, and it does — on three conditions that are the real
content of the answer.**

*What I found when I traced who creates a task.* **Every one of the 30 spawned tasks is created by the `sys_task_spawn`
syscall** — `init` spawns the services (`fjell-init/src/main.rs`), `neg-test` spawns two fixtures; `init` itself is the
only task built directly in `main.rs`. So a boot task always has a **spawner**, and the spawner is the natural owner:
0.46-003 made revocation ownership-based, so **the spawner may revoke its child's boot lease with no exception right**,
and `capability-topology`'s rule for `LEASE_REVOKE_ANY` stays at **exactly one holder (`neg-test`)**. My first draft
proposed widening it to `init`; **that was unnecessary and is withdrawn.**

The three conditions that make owner = spawner work, each found by tracing rather than assumed:

1. **The per-owner quota must not govern boot leases.** `init` would own **30** leases against `MAX_LEASES_PER_OWNER` =
   **16**. The quota bounds *what a task asks for* (`sys_lease_create`; R-Q, the abandon-loop defence); a lease the
   **kernel** creates on a spawn is not a request. The kernel creates them through a separate `create_boot`, **bounded by
   table capacity (128) and `MAX_TASKS` (40), not by the quota** — a second path into the table, stated and tested, not
   a loosened quota.
2. **The lease must die with its subject, or the table leaks.** A boot lease owned by `init` is *not* revoked when the
   *child* exits (`revoke_owned_by(child)` finds nothing: the child owns no lease). Without more, **every service restart
   would consume a slot for ever** — the exhaustion 0.46-001 closed. So the TCB carries `boot_lease: Option<LeaseId>`, and
   the two places that already call `revoke_owned_by` on exit and on fault (`trap/dispatch.rs`) also revoke it.
   **Capacity then tracks live tasks (≤ 31), not boot history.**
3. **The owner must be able to *name* the lease.** `sys_task_spawn` returns the handle in `a1`; it additionally returns
   the **lease id in `a2`** (additive; the existing wrapper is unchanged, and a new `sys_task_spawn_leased` returns
   both). `init` holds, from the moment it spawns a service, the one thing it needs to take that service's authority back.

**What owner = the task itself would have needed instead** (my first recommendation): a `LEASE_REVOKE_ANY` holder that is
not a test service, because no one but the task could revoke. **Spawner-owner makes that unnecessary** and keeps the
narrowest grant in the system narrow in the milestone that makes leases universal. *Do not widen it to save an argument.*
If a case appears where the party that must revoke is not the spawner (a supervisor distinct from `init`), **that** case
justifies widening, and I will name it then.

**Who can revoke a boot lease once its owner is dead** *(asked at the stage-2 review; the answer is read from the code).*
A boot lease is **not** ended by its owner's death (`revoke_owned_by` skips it): `init` or a supervisor exiting must not
strip every service it started, which would be a new failure mode. The consequence, stated plainly:

* **The authority to revoke it is then the `LEASE_REVOKE_ANY` holder's — `neg-test` — and no one else's.** The owner is
  gone, and `sys_lease_revoke` admits only the owner or a `LEASE_REVOKE_ANY` holder (RFC-0.46-003).
* **Otherwise the lease ends when its *subject* exits or faults** (`Task.boot_lease`, revoked on both paths). For an
  orphaned boot lease no `LEASE_REVOKE_ANY` holder chooses to revoke, the answer is "nobody, until the subject dies". It
  is defensible: the authority was granted by a supervisor that no longer exists, and nothing in the system has been
  *given* the right to take it back.
* **That answer rests on a fact this line does not own: a task slot is never reused.** Lease ownership is the slot
  *index* (`sys_lease_revoke` compares the owner with `TaskId::new(tidx, 0)` — generation 0), and nothing in the kernel
  calls `TaskTable::remove`: an exited or faulted task stays in its slot as `Exited`/`Faulted`, so an orphaned lease's
  owner index names a dead task for ever. **If a future change reuses task slots, the new occupant of that index would
  become the "owner" of every boot lease its predecessor spawned**; that change must give a lease owner a generation
  first. Recorded here so it is not discovered by someone who needs it.
* **Guarded, because a property true by a function being dead fails silently** (ruled at the stage-2 follow-up review).
  `bootstrap-leases` (premise 6) is red on any `.remove(` / `::remove(` call in the kernel, or any hand-written clearing of
  a slot's `.task` outside `TaskTable::remove`, and its message names this paragraph. It was shown red on a real tree by
  adding a caller. **It is a stopgap, and the named successor is a generation-aware owner:** store and compare the full
  `TaskId` (index *and* generation) in `LeaseObject.owner`, `sys_lease_revoke`, and the lifecycle revoke, so a new
  occupant of slot *n* does not inherit the previous occupant's leases. **Not built here.** Whoever wants slot reuse
  finds that design question, not a hazard; when it is built the guard is retired in the same change.

**D4 — `init`. RULED: excepted.** `init` is the root that creates the leases, and **a lease whose owner is also its
subject revokes nothing** — `init` could revoke only its own authority, and revoking `init`'s authority from outside would
leave the system supervisor-less. It is excepted by name, with that reason, in the gate's classified list, not as an
unexplained exception. (The demonstration says *every task except `init`*.)

**D5 — capacity and what it does to existing tests.** `MAX_LEASES` = **128**. `neg-test`'s recycling scenario
(`RECYCLE_CYCLES` = 40, tuned against the audit ring) must exceed the capacity to exercise reclamation; at 128 that
is 4× the audit pressure. **Recommendation: change the scenario to fill the table first** (create until
`LeaseTableFull`, revoke, then create again) rather than cycle past the capacity — its property is *reclamation
under pressure*, which does not need the pressure to come from cycling. `v1-limitations.md` row 21 states *"at
most 32 live leases"*; its text is the architect's.

**D6 — the delegation statement (d) needs from the owner.** If the owner rules that boot-time authority is
**permanent for a task's lifetime** (scope question 2), (a) is unnecessary and this RFC becomes: *document it, gate
the claim, correct `RFC-v0.7.4-003`'s text*. The measurements above are the same either way. **I am not asking
which; I am noting that the choice is the owner's and that D1–D5 are void if the answer is (d).**

---

## Requirements (D1 = (a), as ruled)

**R1 — a lease per spawned task, owned by its spawner.** `spawn()` takes the spawner's `TaskId`; after the capabilities
are installed it calls `LeaseTable::create_boot(spawner)` and **binds every capability the new task holds** (one pass
over the CSpace, so no `lease: None` literal is edited and **a grant added tomorrow cannot forget the bind**). The lease
id is stored in the TCB as `boot_lease`. `init` (built directly in `main.rs`) is excepted (D4).
**R2 — `create_boot`**: bounded by table capacity, **not** by `MAX_LEASES_PER_OWNER`; a full table returns `LeaseTableFull`
and **the spawn fails cleanly** (the task is not left half-granted).
**R3 — `MAX_LEASES` 128**, with a compile-time assert that the table holds every live task's lease plus one delegator's
per-owner quota; Verus and `lease_conformance.rs` re-run to show **0 changed obligations**.
**R4 — the lease dies with its subject:** exit and fault revoke `boot_lease`.
**R5 — `sys_task_spawn` returns the lease id in `a2`**; `fjell-syscall::sys_task_spawn_leased` added (the ABI snapshot
moves by additive items).
**R6 — the gate: `bootstrap-leases`.** Derives its population — every `Capability { … }` literal under
`crates/fjell-kernel/src` outside `#[cfg(test)]` — and fails on a `lease: None` literal unless it is in `main.rs`'s
`init` bootstrap (classified, with the D4 reason) or in the spawn path that binds it. Evaluator, not a name match; a
function it cannot delimit is a failure; a stale classified entry fails.
**R7 — the recycling scenario:** its cycle bound derives from the table's capacity.
**R8 — the statement, written down.** `slot.rs`'s *"Bootstrap capabilities carry `lease: None`"* becomes true;
`v1-limitations.md` rows 12 and 21 and E-093's entry are the architect's.

## Negative cases — what I will show

1. **The statement, demonstrated on a real boot — and the case that shows owner-based revocation suffices.** `neg-test`
   spawns `svc-timeout` (a fixture) and so **owns its boot lease**; `svc-timeout` `try_recv`s its endpoint each iteration
   and prints a marker when it first sees `LeaseRevoked`. `neg-test` revokes the lease it was handed **through slot 4, the
   `LeaseAdmin` that has `LEASE_REVOKE` and no `LEASE_REVOKE_ANY`**, and the marker appears. The fixture stays alive and
   runnable (the existing start-timeout test is unaffected).
2. **A non-owner cannot.** `neg-test` attempts the same on a boot lease **`init` owns**, through slot 4: refused
   `PermissionDenied`, audited `lease.revoke_rejected`, and the lease stays Active.
3. **A mint chain cascading while a copy survives**, side by side.
4. **`cap_revoke` on a capability with no descendants deletes nothing.**
5. **What revocation does not take back — MMIO. *(Changed at the stage-2 review: guarded, not scenario-tested.)***
   A boot scenario needs an *unbound* `MmioRegion` capability, i.e. a third fixture exemption, and the exemption list is
   the honesty metric of this line. Instead **`bootstrap-leases` fails if `MmioUnmap` ever becomes dispatched**, which says
   *restate the claim* on the day its premise changes (a scenario only fails when someone re-runs it). **The positive case
   beside it is DMA:** the existing `dma` profile's revoked-address read faulting.
6. **Capacity and lifetime.** With every boot lease live the broker can still grant to its quota; a service that exits
   frees its lease slot (spawn → exit → spawn again leaves the live-lease count flat).
7. **Mutations, each a named failure** (a compile error is INVALID): spawn skips the bind for one image →
   `bootstrap-leases` red naming the literal; `MAX_LEASES` back to 32 → capacity case red; exit does not revoke
   `boot_lease` → lifetime case red; `create_boot` charged to the per-owner quota → the spawn of the 17th service fails;
   the recycling scenario's bound left at a literal → red against the larger table.

## Not demonstrated by any boot scenario — said here so it is not also unmentioned

* **A boot lease ending on the subject's normal exit.** The fault path has a scenario (`NEG:BOOTLEASE:ENDS_WITH_SUBJECT_FAULT`);
  nothing in the suite exits normally. The exit path is the same `take()` + `revoke` as the fault path.
* **A boot lease surviving its owner's death** — the `boot` flag in `revoke_owned_by`, the design choice above. No boot
  test kills a supervisor whose children are watched, and the kernel crate has no host tests, so removing the skip turns
  nothing red. It is held by the flag's comment (`lease/mod.rs`) and by this paragraph.
* **`create_boot` failing mid-spawn** (the lease is revoked, no slot leaks). Evidenced by mutation M4 only.
* **MMIO is not reclaimed** — held by the `bootstrap-leases` guard, not by a boot scenario.

## What this RFC does not do, and why

* **It does not reclaim an MMIO mapping or a DMA frame** on revocation. That is a separate mechanism with its own
  tracking cost, and the RFC says what the claim is without it.
* **It does not change `cap_revoke`'s treatment of the capability it names** (a separate ABI question) **or
  `CSpace::copy`'s parent severing** (option (b) is rejected, not deferred).
* **It does not decide D6.** If the owner rules permanence, this RFC is the documentation line.
* **It does not touch `DELEGATE_ONLY`.** A bound boot grant *can* be delegated with its lease — the interaction the
  scope names — and that is a reason `DELEGATE_ONLY` should follow this line, not accompany it.

## Reproduce

```
git ls-files -z '*.rs' | xargs -0 /usr/bin/grep -an 'lease: None' | cut -d: -f1 | sort | uniq -c
cat crates/fjell-kernel/src/task/spawn.rs crates/fjell-kernel/src/main.rs | /usr/bin/grep -o 'kind: CapKind::[A-Za-z]*' | sort | uniq -c
/usr/bin/grep -n 'pub const MAX_LEASES\|pub const MAX_LEASES_PER_OWNER' crates/fjell-kernel/src/lease/mod.rs
/usr/bin/grep -nE 'MAX|capacity|table' verification/verus/lease/lease_epoch.rs
```

The **boot** figures (30 tasks, 101 capabilities, 104 / 7 / 2 lease creations, peak live 17) come from a print-only patch
to `spawn()` and `LeaseTable::create` in a scratch worktree, run under `cargo xtask qemu-test m8`; the patch was
never committed, and the implementation commits the measurement as a host test over the same counts rather than as
one-off output.
