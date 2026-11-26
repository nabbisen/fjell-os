# RFC-0.49-001: A task the system can stop, and a slot it can take back — measured first, designed second

**Status:** Proposed
**Milestone:** 0.49
**Tracks.** Handoff line 1 (`.git-exclude/handoff/handoff-0.49.md`): *a task cannot be stopped by anything but itself, and
every task that stops costs a slot that never returns.* Closes the first of the three syscalls `v1-limitations.md` row 7
records as declared and never dispatched (**`TaskKill`**), and answers the recovery promise in `crates/fjell-os/README.md`
(*"every documented failure mode has a tested playbook"*) for the one failure mode a supervisor exists to handle. **No
erratum is filed for it**; the owner's pending question (D1) decides whether this RFC proposes one.
**Touches** *(indicative, depends on D1–D4)*: `crates/fjell-kernel/src/task/{tcb,spawn}.rs`,
`crates/fjell-kernel/src/trap/{syscall,dispatch}.rs`, `crates/fjell-kernel/src/cap/{table,syscall}.rs`,
`crates/fjell-kernel/src/lease/mod.rs`, `crates/fjell-kernel/src/irq.rs`, `crates/fjell-cap/src/{cspace,rights}.rs`,
`crates/fjell-syscall/src/lib.rs`, the three `TaskControl` grants in `spawn.rs`/`main.rs`,
`crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/svc.toml`, `tests/syscall/expected.toml`,
`tests/abi/snapshot.json`, `tools/fjell-consistency-check/src/{bootstrap_leases,capability_topology}.rs`, prebuilts and
`tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.48-001** (boot leases, and premise 6 — the guard this RFC retires), **RFC-0.46-001 / E-110**
(lease recycling and the quota that closed it), **RFC-0.46-003** (who may revoke), **RFC-0.47-001** (one funnel).
**Not in this line:** `DELEGATE_ONLY`; freeing the dead task's **page-table** frames (D4, deferred with its reason);
`MmioUnmap` and `DmaShare` (RFC-0.49-002).

*Every figure below was re-measured at `b2c17b7`, not copied from the handoff. Measurements marked **boot** were taken on a
real QEMU `m8` boot in a scratch worktree carrying a print-only patch that was never committed. Where the tree and the
handoff differ, "Where the handoff and the tree differ" says so — **there are five places, and two of them change the
design.***

---

## What is true now

* **A task slot is taken once and never returned.** `TaskTable::insert` takes only a slot where `slot.task.is_none()`
  (`task/tcb.rs:228`–`:236`); an exiting or faulting task keeps `slot.task = Some(task)` and only `state` changes
  (`trap/dispatch.rs:313`, `:342`); the **only** clear of `slot.task` is inside `TaskTable::remove` (`tcb.rs:263`), which
  has **no caller** (probe below, with its control). `bootstrap-leases` premise 6 guards exactly that.
* **The only termination path is `sys_exit`.** No `fn kill`, no `terminate`, anywhere in the kernel. `TaskKill = 43` is
  declared (`fjell-abi/src/syscall.rs:78`) and has no dispatch arm; the ABI inventory row says *"TaskControl + TASK_KILL"*.
* **`MAX_TASKS` = 40.** A boot (**boot**) creates **30 tasks through `spawn()`, in slots 2–31**, beside the idle task (slot 0)
  and `init` (slot 1): **32 occupied, 8 spare for the whole life of the node.** `SysError::TaskTableFull` is returned by
  `spawn` (`spawn.rs:110`, `:292`).
* **Death does reclaim DMA and leases** (handoff correction, confirmed): both death arms run
  `crate::dma_table().release_task(id)`, `lt.revoke_owned_by(id)` and, since 0.48, `task.boot_lease.take()` →
  `lt.revoke(bl)`. **Frames, the CSpace, endpoint queues, the IRQ binding and the slot are not reclaimed.**

```
/usr/bin/grep -an '\.remove(' crates/fjell-kernel/src -r ; /usr/bin/grep -an 'pub fn remove' crates/fjell-kernel/src/task/tcb.rs
/usr/bin/grep -an 'task = None\|task\.take()' crates/fjell-kernel/src -r
```
*(Control: the first command finds `pub fn remove` and nothing else; the second finds only `slot.task.take()` inside `remove`.)*

### Measured — what a spawn costs, and what that does to "just recycle the slot" (boot)

`fa.free_count()` before and after each `spawn_with_lease`, 30 spawns on the `m8` image:

| Figure | Value |
|---|---|
| Free frames when the allocator became ready | **31,041** (≈ 121 MiB) |
| Frames taken per spawn | **min 24, median 24, mean 25.4, max 46** (19 of 30 took 24; the 46 is `image 6`; `semantic-stream`'s stack is 32 pages) |
| Frames taken by the 30 spawns | **761** (free: 30,171 after the last) |
| By owner tag, whole machine, after the 30th spawn | `UserText` 55 · `UserStack` 496 (= 31 × 16: 30 tasks + `semantic-stream`'s extra 16) · `KernelPageTable` 263 · `KernelStack` 1,055 (= 31 task stacks + 1,024, which I infer to be the boot stack: it is a round number, not a measured tag) |
| **Derived** page-table frames per task | 761 − 55 − 496 − 30 = **180 → 6.0 per task (23.7 %)** *(by subtraction from three measured totals, assuming one kernel-stack frame each; not a direct count)* |

**The arithmetic the design turns on.** Spare slots today: **8**. If slots were reclaimed and **frames were not**, memory — not
the table — becomes the ceiling: 30,171 free ÷ 24 = **≈ 1,257** further spawns of the commonest image, **≈ 655** of the
largest. That moves the ceiling ~80–157× and **converts a clean `TaskTableFull` into `NoMemory` that also starves every
other allocator caller** (a driver's `sys_mmio_map`, a new page table). If everything except page-table frames is freed:
30,171 ÷ 6.0 ≈ **5,000** destroy-and-respawn cycles. If all is freed: unbounded.

### Is recycling enough? — the E-110 question, measured before answered

E-110's lesson was that recycling did not close it: the denial was a request-and-abandon loop whose leases were never
revoked, and a **quota** closed it. The same question here, with what the tree says:

* **Nothing restarts a service.** `service-manager` *observes* (`sys_task_status`, `main.rs`) and reports health to `bootctl`;
  it never calls `sys_task_spawn`. `sys_task_spawn` is called by `init` (30 spawns, once), `neg-test` (2: `svc-timeout`,
  `svc-fault`), and — in comments only — `semantic-stream`. **There is no restart loop in the system to exhaust anything.**
* **`TaskCreate` is held by exactly two tasks:** `init` (`main.rs:1386`) and `neg-test` (`spawn.rs:932`), both `ObjectScope::Any`,
  `ALL_NON_META`. `cap-broker`'s `TASK_MGMT` mask names `TASK_CREATE`, but a delegator can delegate only what it holds, and
  it holds none.
* So the E-110 shape needs a **spawn-and-hold** holder (fill the table and never exit), which recycling does not touch and
  which today is reachable only by those two trusted tasks. **I am recommending no quota (D6)** and no number; the RFC states
  the condition under which one becomes necessary (a third `TaskCreate` holder, or a delegable one).

---

## Where the handoff and the tree differ

1. **`TASK_KILL` is not "granted by nothing" — three blanket grants already carry it.** `CapRights::ALL_NON_META` is
   `(1 << 26) - 1` (`rights.rs:142`); `TASK_KILL` is bit 15. The three `TaskControl` grants — `init` slot 29
   (`main.rs:1402`), `service-manager` slot 29 (`spawn.rs:875`), `neg-test` slot 6 (`spawn.rs:947`) — are all
   `rights: CapRights::ALL_NON_META`, `scope: Any`. **The right exists; the operation does not. Dispatching `TaskKill` would
   arm three holders, including a test fixture, with the power to destroy any task including each other and `init`,
   with no new grant.** This is the hazard `LEASE_BIND`'s doc comment records ("widening an aggregate would hand a new
   authority to every holder of a blanket grant") — in the other direction: the right is *already inside* the aggregate, and
   moving the bit would be an ABI break.
2. **`init` exits in every boot, with its 30 children running.** `init`'s last statement is `sys_exit(0)`
   (`fjell-init/src/main.rs:1347`); the kernel's own `TEST:M7:PASS` is printed *because* it exited cleanly. **Every
   boot lease 0.48 created is therefore owned by a dead task** in any boot that reaches M7, and the *only* task that can
   revoke any of them is the `LEASE_REVOKE_ANY` holder — **`neg-test`, a test fixture** (`capability-topology`: *"granted at 1
   site, all to NEG_TEST"*). 0.48's "owner = spawner" gave the operator a revocation path that is **unavailable in
   production by the time anyone would use it.** The 0.48 review accepted the orphan answer ("only a `LEASE_REVOKE_ANY` holder,
   otherwise nobody until the subject dies") as a **defensible corner case**; **it is the normal case.** *(This is not a
   defect in 0.48-001's mechanism — the design held and its scenarios pass — it is a fact about what the mechanism buys. It
   is also why an owner-only rule for kill (D2) would be useless.)*
3. **The kernel itself reads dead tasks.** `check_smoke_pass` (`dispatch.rs:539`) prints the `TEST:*:PASS` markers from
   `exited_ok_by_image(table, ImageId::{DEVMGR, SYNCD, NETD, UPGRADED, INIT})` and `done_by_image(table, INIT)`
   (`dispatch.rs:525`, `:536`) — it requires the exited task to *still be in the table*. `done_by_image` is **vacuously true**
   when no task carries the image, so an auto-reclaimed `init` would print `TEST:M7:FAIL (init did not exit cleanly)`. Neither
   reader is an ABI observer, so the handoff's constraint 2 is real and is **load-bearing for the test harness**.
4. **A reused CSpace would hand the new task the old one's capabilities.** `CSpace` has no reset (its `pub fn` list has none),
   and `install_raw` **refuses a slot that is already `Active`** (`cspace.rs:153`–`:161`) while every grant discards the
   result (`let _ = cs.install_raw(...)`). A task spawned into a reused slot would hold the **predecessor's** capability in every
   slot both images use, and its own grant would fail silently. **A reuse must reset the CSpace; this is the first
   thing a stage-2 negative case must show** (D4, negative case 4).
5. **The `TaskId::new(x, 0)` idiom is wrong the moment a generation bumps.** There are **20** production sites that build a
   task id from an index with generation 0 — `cap/syscall.rs` 9, `trap/syscall.rs` 7, `lease/mod.rs` 3, `task/spawn.rs` 1 — and
   `spawn_with_lease` builds `tid = TaskId::new(tid_index, 0)` for the TCB and every `FrameOwner` tag **before**
   `table.insert` assigns the slot's real generation. Reuse with `remove`'s generation bump makes `table.get(TaskId::new(i, 0))`
   return `None` for the new occupant: **a wake or a reply to it silently does not happen.** `lease/mod.rs:64` records that
   the idiom is correct *because generation is always 0*.

```
/usr/bin/grep -an 'pub const ALL_NON_META' crates/fjell-cap/src/rights.rs
for f in crates/fjell-kernel/src/task/spawn.rs crates/fjell-kernel/src/main.rs; do /usr/bin/grep -an -A2 'kind: CapKind::TaskControl' $f | /usr/bin/grep -a 'rights:'; done
/usr/bin/grep -an 'sys_exit(0)' crates/services/fjell-init/src/main.rs
/usr/bin/grep -an 'exited_ok_by_image(table, ImageId::' crates/fjell-kernel/src/trap/dispatch.rs | wc -l
```

### What a reuse must scrub — the index-keyed state (inventory)

Everything below is keyed by task **index** and survives a death today. A destroy has to leave **none** of it naming the slot:

| State | Where | Today at death |
|---|---|---|
| CSpace (capabilities, per-slot generation) | `cap/table.rs` `cspaces[MAX_TASKS]` | **kept, all slots** (finding 4) |
| One-shot reply edge | `cap/table.rs` `replies[MAX_TASKS]` (indexed by server; holds the caller index) | kept |
| Endpoint receive/send queues | `fjell-ipc` `Endpoint` `recvq`/`sendq`, `QUEUE_DEPTH` 32, `MAX_ENDPOINTS` 32; entries carry a `u16` index | kept (a *running* task is never in one; a *killed blocked* one is) |
| Audit-ring waiter | `audit/ring.rs:148` `waiter: Cell<Option<TaskId>>` | kept |
| IRQ binding | `irq.rs:18` `bound: [Option<TaskId>; MAX_IRQ]`; **no unbind exists** | kept: a dead driver owns its line for ever |
| Ready queue | `scheduler.rs` `[Option<TaskId>; MAX_TASKS]` per priority | a dead task is never re-enqueued; a killed `Runnable` one **is** in it |
| Lease ownership | `LeaseObject.owner` | non-boot revoked (`revoke_owned_by`); **boot leases it spawned survive, owned by its index** |
| DMA table | `DmaRegionEntry.owner` | **released** (RFC 017) |
| Frames | `FrameOwner::{UserText,UserStack}{task}`; `KernelStack`/`KernelPageTable` carry **no** task | kept (761 frames per 30 tasks) |
| Device mappings | none recorded (RFC-0.49-002) | kept in the dead task's page table |

Not a hazard, checked: **ASID is always 0** (`vspace.rs:73`) and `satp` writes are followed by `sfence.vma` (`satp.rs`), so a
reused address-space id cannot alias through a stale TLB entry.

---

## The decisions this RFC asks for

**D1 — where a dead task's status lives. This is the owner's pending question; I recommend (B) and say what (A) would need.**
The handoff's three candidates, against the measurements:

* **(A) reclaim at death, status in a small exit-record ring** (plus a per-image "exited cleanly" latch for
  `check_smoke_pass`). Needs: the ring (size is a *new* tunable that decides whether `service-manager`'s fault poll can lose a
  verdict — a faulted *required* service whose record is overwritten before the next poll is a `BOOT_HEALTH_FAILED` that is never
  sent); latches for the five milestone images; a `Gone` outcome for `sys_task_status`. Every existing reader changes.
* **(B) a dead task stays observable until a `TASK_KILL` holder destroys it; `TaskKill` means *destroy*.** On a live task: stop
  it, run the death path, release what D4 lists, free the slot. **On an already-dead task: release the slot** (its resources are
  gone already). One verb, one meaning — *afterwards the id names nothing* — and a status read before the call is the caller's
  choice. **Nothing that reads a dead task today changes** (`service-manager`, `check_smoke_pass`, `sys_task_status`), provided
  the milestone images are never destroyed (stated, not gated). The cost, stated plainly: **a task that exits or faults still holds
  its slot until something destroys it**, so statement 1 of the handoff ("every exit permanently costs a slot") is closed *for
  a system with a destroyer*, and the destroyer is `service-manager`, which today does not destroy.
* **(C) generation bump only, no record** — the stale `TaskId` reports `InvalidCap`. Loses the fault verdict outright.

**If the owner rules that a dead task need not remain observable, (A) becomes the simpler design and I will rewrite D1.**

**D2 — who may kill whom.** *(Finding 2 decides this: an owner-only rule — 0.46-003's shape — would mean no one can kill any
service in a normal boot, because the spawner is dead.)* Recommended:
1. **`TASK_KILL` moves out of the blanket grants and is granted by name** to **`service-manager`** (the component that already
   watches `sys_task_status`) and **`neg-test`** (the fixture, so the line can be demonstrated). `init` loses it: it exits.
   Mechanically: the three `TaskControl` grants carry `ALL_NON_META` *minus* `TASK_KILL` except the two named; no bit moves.
2. **`capability-topology` counts holders of `TASK_KILL` — including through `ALL_NON_META`**, which the by-name counter
   (`by_name_right_holders`) cannot see. A second counting mode is needed and is part of this line; allowed set
   `{SERVICE_MANAGER, NEG_TEST}`, the second an explicit named exception exactly as `LEASE_REVOKE_ANY` is.
3. **Refused targets:** the caller itself (use `sys_exit`) and the idle task (slot 0). *Not* refused: `init`, `service-manager`
   — a `TASK_KILL` holder is trusted with them, and a kill right that cannot be used on the supervisor is a rule that will
   be broken to be useful.
4. **Scope stays `Any`.** `ObjectScope::Task(id)` exists and the check already uses it, but a service's id is not known when
   the grant is written; a runtime-narrowed kill capability is a different line.

**D3 — identity across a reuse; generation-aware ownership, as the handoff makes a prerequisite.** Two designs, both
measured against the 20 sites:
* **(a) a real `TaskId` everywhere it is compared.** `LeaseObject.owner` already *stores* a `TaskId`; the comparison is the
  weak half (`TaskId::new(tidx, 0)`). Add `TaskTable::current_id(index)` and use it at the 20 sites; give `spawn_with_lease`
  the slot's real generation (`next_free_index` → `next_free() -> (index, generation)`). **Then a new occupant of slot *n*
  has generation *k+1* and cannot equal an owner stored with *k*: the orphan answer cannot be inherited, without any
  disowning step.** IPC queue entries stay `u16`: they are purged at destroy (D5), not made generation-aware.
* **(b) index identity inside the kernel, disown at destroy** — `LeaseObject.owner := NOBODY` for leases its index owns, and
  `get_by_index` at the wake/reply sites. Fewer widths change, but it leaves **two** identity notions in the kernel and the
  safety of every future `TaskId::new(i, 0)` rests on a reviewer remembering.

**Recommended: (a).** The 20 sites must change under *either*; (a) is the one that is true by comparison instead of by
procedure. **Premise 6 retires in the same change** — it guards "`remove` has no callers", which this RFC falsifies — and
its **successor premise** is: *no production `TaskId::new(_, 0)` outside an allow-list of the idle task, `init`'s bootstrap and
the DMA sentinel*, measured today at **20 outside / 3 allowed**. **What the orphan answer becomes:** unchanged in effect
(a `LEASE_REVOKE_ANY` holder, otherwise the subject's death) and now **provably not inheritable**; the RFC does not extend
who can revoke an orphan, because that is D2's lesson applied to leases and wants its own ruling — see "What I did not do".

**D4 — what `destroy` frees, and the page-table frames I propose to defer.**
Frees: the user pages (owner-tagged; `free_frame` exists), the kernel stack (`task.kernel_stack_top`), the CSpace (a new
`CSpace::reset` that bumps each slot's generation, so a stale handle cannot name a new capability), the reply edge, the IRQ
binding (a new `IrqTable::release_task`), the endpoint and audit-ring entries (D5), the ready-queue entry, and — through RFC-0.49-002 — device
mappings. **Defers the page-table frames: ~6.0 per task, 23.7 %, measured by subtraction.** Reason: freeing them needs a
**walker over a live structure** whose upper half is shared with the kernel (`clone_kernel_half`) — freeing a shared
sub-table is a kernel-wide corruption — and the tree's own history is a warning: `DmaRegionEntry`'s doc still says its deferred unmap path is "bypassed due to
page-table corruption under v0.8.x", and `unmap_dma_region_verified` (RFC-0.39-004) carries a comment that the cause was never isolated. **What the deferral costs: ≈ 6 frames per destroy,
a ceiling of ≈ 5,000 destroy-and-respawn cycles per boot** (arithmetic above), recorded as a bound rather than left implied. If the
architect would rather have the walker in this line, it is its own requirement with its own negative cases.

**D5 — killing a task that is not running.** A killer is running, so the target is `Created`, `Runnable`, or `Blocked(_)`; `Runnable`
and `Blocked(_)` hold references in structures the task does not own. Destroy must purge, and each must be shown: (1) endpoint
`recvq`/`sendq` entries by index (a walk over 32 endpoints — `lease/mod.rs`'s revoke already walks "every endpoint by ID and
cancels matching waiters"; the template exists); (2) the reply edge, both as server and as caller; (3) the audit-ring waiter;
(4) the IRQ binding; (5) the ready queue. A blocked caller that is killed also owes its **server** nothing but silence: the
server's later `reply` finds no blocked caller and is refused by the existing guard (`cap/syscall.rs:890`).

**D6 — a quota (E-110's remedy). Recommended: none in this line**, for the measured reason above (two trusted `TaskCreate`
holders, no restart loop). **The condition that reopens it is written into the RFC:** a third `TaskCreate` holder, or a
delegable one, makes spawn-and-hold a denial of service that destroy-on-demand does not bound, and the answer would be a
per-spawner live-task count — a number I will not choose without a second holder to size it against.

**D7 — ABI.** No new `TaskLifecycle` value and no new right: `TaskKill` returns `Ok` and the id then names nothing
(`sys_task_status` → `InvalidCap`, as for any stale id). `sys_task_kill(cap, handle)` joins `fjell-syscall`; the inventory row
(`TaskControl + TASK_KILL`, partial) is updated; `abi-snapshot` moves by the wrapper. *If (A) is ruled in D1, `Gone` is a new
`TaskLifecycle` value and this changes.*

---

## Requirements

**R1 — `sys_task_kill`** through the funnel (`TaskControl`, `TASK_KILL`, scope `Task(id)`; the refused targets of D2.3).
**R2 — destroy**, one function, run by `TaskKill` on a live or dead task: the death path's existing steps, then D4's, then the
slot's generation bump and `remove`. **R3 — real generations** (D3a): `next_free() -> (index, generation)`, `spawn_with_lease` uses
it for the TCB and every `FrameOwner` tag, and the 20 sites use it. **R4 — the grants**: `TASK_KILL` out of the blanket grants, by
name to two holders (D2.1). **R5 — gates:** `capability-topology` counts `TASK_KILL` including through `ALL_NON_META` (D2.2);
`bootstrap-leases` premise 6 retires and its successor (D3) lands **in the same commit**. **R6 — the death-path readers**
(`check_smoke_pass`, `service-manager`) are unchanged and a test says the milestone images are not destroyed by any scenario.
**R7 — an inventory test** for the table in "What a reuse must scrub": a destroy followed by a spawn into the slot finds none
of it.

## Negative cases — what I will show

1. **The hung task, stopped, its slot reused.** `neg-test` spawns `svc-timeout` (never READYs; a hung service by design),
   confirms `Runnable`/`Blocked`, **kills it**, spawns again into the **same index**, and the old handle is refused.
2. **Refused:** a kill without `TASK_KILL` (`PermissionDenied`); with a capability whose lease was revoked (`LeaseRevoked`, the
   funnel's own path); of itself; of the idle task.
3. **Exhaustion, before and after.** Spawn until `TaskTableFull` (the 8 spare slots); destroy them all; spawn again to the same
   count, **repeatedly (≥ 3 × `MAX_TASKS` cycles)**. **Mutation: disable the slot release and the second phase must redden by
   name.** (The "before" is the mutation, because the suite cannot run the exhaustion on the current kernel without leaving the
   later scenarios no slot.)
4. **A new occupant holds exactly its own grants.** After a destroy-and-respawn into the slot, the CSpace's occupied slots are
   precisely the new image's grant set (finding 4). **Mutation: skip `CSpace::reset` and it must redden.**
5. **A stale `TaskId` after reuse does not reach the new occupant** — status, start, kill, and a lease owned by the previous
   occupant (D3's own test, and the reason premise 6 existed).
6. **A killed blocked task leaves no queue entry:** kill a task blocked in `recv`; a later `send` to that endpoint is queued, not
   "delivered" to a dead receiver.
7. **Leak accounting.** Free frames before and after *N* spawn-and-destroy cycles differ by exactly *N* × (the page-table frames of
   one task, measured) — **the deferred leak is a number the test states**, so it cannot grow unannounced.
8. **Gates:** `capability-topology` red when `TASK_KILL` is added to a blanket grant; premise 6's successor red when a production
   `TaskId::new(x, 0)` is added; each shown red on the real files, restored sha-exact. A compile error is INVALID; a RED needs a
   named failure.

## What this RFC does not do, and why

* **It does not free page-table frames** (D4) — a measured 23.7 % of a task, deferred with the reason and the resulting bound.
* **It does not add a quota** (D6) — nothing in the system can create the shape that needs one.
* **It does not make any service restart anything.** `service-manager` gains the *authority* to destroy; the policy of when to
  — a restart budget, a cooldown — is a service-manager design that has no consumer yet.
* **It does not change who may revoke an orphaned boot lease.** Finding 2 shows the answer is mostly "only `neg-test`", which
  this RFC reports and does not repair: widening `LEASE_REVOKE_ANY` is exactly the decision 0.48 declined to make, and it
  deserves its own ruling with the *owner's* view of whether boot-time authority is meant to be revocable by the operator at all (0.48
  D6, still open).
* **It does not touch `DELEGATE_ONLY`.** Task identity gaining a generation changes delegation's identity story (scope-0.49 says
  so); no delegation path changes here, and that is checked, not assumed — the broker's grants are lease-bound runtime installs.

## Reproduce

```
git worktree add --detach <scratch> HEAD        # then, in the scratch tree only:
#   spawn.rs: print fa.free_count() before/after spawn_with_lease; once, tally frames by FrameOwner at slot 31
cargo run -p fjell-tools -- qemu-test m8        # serial: tests/qemu/artifacts/smoke-m8/serial.log
/usr/bin/grep -a 'probe' tests/qemu/artifacts/smoke-m8/serial.log
```
The patch was never committed; the worktree was removed. The arithmetic: `30171 / 24 = 1257`, `30171 / 46 = 655`,
`30171 / (180/30) = 5028`.
