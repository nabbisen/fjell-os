# Developer Handoff — RFC-0.48-001

**Governing RFC:** [RFC-0.48-001](../../done/RFC-0.48-001-a-bootstrap-capability-can-be-taken-back.md)
**Milestone:** 0.48 · first line
**Status:** inherited from the governing RFC (Implemented, 0.48.0)
**Audience:** implementation model

---

## 1. The premise the architect got wrong, corrected before anything was built

**The 0.48 handoff's motive was *"an operator who decides a service must lose its MMIO window"*, and no
option in the line could deliver it.** `MmioUnmap` is declared in the ABI and has **no dispatch arm**
— one of the three syscalls v1.0 limitations row 7 records as declared and never dispatched — so the
unmap primitive is not a reachable call. The implementer found this before building and said so.

**And the generalisation that replaced it also over-reached**: revocation was said not to reclaim
authority that became a resource, *"(MMIO mapping, DMA frame)"*. **DMA is reclaimable.**
`sys_dma_revoke` is dispatched, requires `DmaRegion` + `DMA_REVOKE` through the funnel, and unmaps the
owner's page **verified**. The asymmetry is **MMIO versus DMA**, which is the more useful finding: the
project already knows how to do this for one resource and has not for the other.

## 2. What the line does

**Every task's boot capabilities are bound to a boot lease its spawner owns.** `spawn()` creates the
lease **before any frame**, so a full table fails the spawn with nothing to undo, and **one pass binds
every capability the task holds** — no `lease: None` literal was edited, and a new grant cannot
forget. The lease lives in the subject's TCB and is revoked when the subject exits or faults.

**It is deliberately not ended by its owner's death**, because `init` exiting must not strip every
service it started.

## 3. The arithmetic, and the architect's recommendation that did not survive it

The handoff said one lease per task against `MAX_LEASES = 32`. The implementer measured a real boot:
**31 holders**, not the 55 source literals; runtime leases share the table (peak live 17); and
**`MAX_LEASES_PER_OWNER` = 16 kills any single-owner design against 31 holders**. `MAX_LEASES` is
**128**, in `fjell-abi` so `neg-test` derives its bounds from it rather than a second literal, with a
compile-time assert that the table holds one lease per task plus a delegator's whole quota.
**0 Verus obligations changed** — the lease model has no capacity token.

## 4. What this does not buy, and the guard that keeps the claim honest

Revocation withdraws authority exercised **per call**. A revoked `MmioRegion` stops further mapping;
**pages already mapped stay.** Rather than a boot scenario needing a third fixture exemption,
`bootstrap-leases` **fails the day `MmioUnmap` is dispatched** — guarding the premise, so the claim
must be restated when it changes rather than rotting quietly.

## 5. Two defects the line found in its own neighbourhood

**`sys_lease_inspect` wrote the epoch into `a0`, the status register, and never wrote `a1`** — which
still held the lease id the caller passed in. So it returned that id, as the epoch, for every lease,
always, and **half of two revocation claims could not fail** (E-132). Found by chasing
`before=27 after=27` against a kernel line printing `revoke -> Ok(2)`.

**`sys_task_spawn`'s wrapper used `ecall2`**, which declares `a2` as an input that survives the call —
false the moment the kernel returns the boot lease there. Caught in the same line that introduced it.

## 6. The orphan question, and the premise it rests on

A boot lease outliving its owner can be revoked by **a `LEASE_REVOKE_ANY` holder only; otherwise
nobody until the subject exits or faults.** That is safe **because a lease's owner is a task-table
index and no code returns a slot to reuse** — `TaskTable::remove` has no callers. `bootstrap-leases`
premise 6 guards that; the named successor, **not built**, is a generation-aware owner.
