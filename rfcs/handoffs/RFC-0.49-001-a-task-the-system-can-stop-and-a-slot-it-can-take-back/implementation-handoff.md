# Developer Handoff — RFC-0.49-001

**Governing RFC:** [RFC-0.49-001](../../done/RFC-0.49-001-a-task-the-system-can-stop-and-a-slot-it-can-take-back.md)
**Milestone:** 0.49 · first line
**Status:** inherited from the governing RFC (Implemented, 0.49.0)
**Audience:** implementation model

---

## 1. Two premises the architect got wrong, both corrected before anything was built

**"`TASK_KILL` exists and is granted by nothing today" was false.** `TASK_KILL` is bit 15 and
`ALL_NON_META` is `(1 << 26) - 1`, so bit 15 is **inside** it — and all three `TaskControl` grants
(`init`, `service-manager`, `neg-test`) carry `rights: ALL_NON_META, scope: Any`. **Dispatching
`TaskKill` on the handoff's instruction would have armed a test fixture to destroy `init`.** The
architect had asserted an absence from a by-name search that a blanket constant defeated — the fourth
such error in three milestones.

**"Decide the kill holder as RFC-0.46-003 decided the lease revoker" was wrong for a measurable
reason.** `init` ends with `sys_exit(0)` (`crates/services/fjell-init/src/main.rs:1347`), so an
owner-only rule would let no one kill any service in a normal boot. The same fact makes **every** boot
lease 0.48 creates owned by a dead task in any boot reaching M7 — the architect's 0.48 review had
called that a corner case; it is the normal case.

## 2. What the measurements decided, against what the scope assumed

* A boot occupies **32 of 40 slots, 8 spare**. A spawn costs **24–46 frames** (median 24, mean 25.4).
* **Reclaiming slots without frames would move the ceiling from 8 to ≈ 655–1,257 spawns and turn
  `TaskTableFull` into `NoMemory`** — a clean, specific refusal replaced by allocator exhaustion that
  starves every other caller. That is why D4 frees frames rather than only slots.
* **E-110's question ("is recycling enough?") answered by measurement, not analogy:** there is no
  restart loop to exhaust anything — `service-manager` observes and never spawns, and `TaskCreate` is
  held by two tasks. **No quota**, with the reopening condition written into the RFC.

## 3. What was built

`TaskKill` (43) dispatched and meaning **destroy**: a live task gets the death path, its
endpoint/reply/audit/IRQ/ready-queue entries are purged, its CSpace is reset with every slot
generation bumped, its user frames and kernel stack are freed, and the slot's generation advances; an
already-dead task has only its slot released, so its verdict stays readable until a `TASK_KILL`
holder destroys it. `service-manager` is the destroyer and reads the verdict **before** destroying.
Real generations throughout: **20 production `TaskId::new(_, 0)` → 0**, five remaining in four
classified functions. `TASK_KILL` left the blanket grants and is held by name by two tasks.

## 4. Two rulings that changed the design mid-line

* **The line must include a destroyer.** A kill syscall nobody calls leaves a normal boot exhausting
  its slots exactly as before and ships a mechanism whose existence implies a property the system has
  not got — the E-014 shape raised from an instrument to the architecture.
* **The harness's dependency on corpses is removed by a latch, not a carve-out.** The first design
  proposed a test asserting the milestone images are never destroyed; a per-image "exited cleanly"
  latch set on the exit path costs nothing and removes the dependency instead of fencing it off.

## 5. Bounds this line did not cross, each with its reason in the RFC

Page-table frames stay allocated (≈ 6 per task, **measured** 18 of 24 freed): freeing them needs a
walker over a live structure whose upper half is shared with the kernel, and the tree's own history
records page-table corruption on that path. **0.49 delivers reclamation, not restart** —
`service-manager` holds no `TaskCreate`, nothing refills a slot, and a third `TaskCreate` holder is
the condition that reopens the quota question. `ReadyQueue::remove` hung the machine during this work
and was found by booting, because the kernel crate has no runnable unit tests (**E-134**).
