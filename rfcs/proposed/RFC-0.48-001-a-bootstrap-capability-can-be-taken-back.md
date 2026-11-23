# RFC-0.48-001: A bootstrap capability can be taken back — measured first, designed second

**Status:** Proposed
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

### What revoking a lease takes back — **the finding that changes the statement the line exists for (boot / source)**

The handoff's example is *"an operator who decides a service must lose its MMIO window"*. **Lease-binding the
`MmioRegion` capability does not deliver that**, and the RFC says so before anyone builds toward it:

* The funnel is consulted **at each use**. `sys_irq_wait`, `sys_irq_ack`, `sys_platform_reboot`, `sys_clock_now`,
  `sys_task_*`, `sys_lease_*`, `sys_cap_*` and every IPC syscall call it per invocation, so for **authority
  exercised per call** a revocation is immediate — and blocked IPC is woken with `LeaseRevoked`
  (`wake_or_cancel_blocked_ipc_for_lease`, RFC 034/050).
* **`sys_mmio_map` consults it once, to create a page mapping, and nothing ever removes the mapping.** The only
  `unmap_page` calls in the syscall layer are the failed-map rollback. A task that has mapped its window **keeps
  it after the lease is revoked**; revocation takes away only the right to map again. (`sys_dma_revoke` does
  reclaim a DMA frame — RFC-0.39-004 — but only when *the holder* calls it.)
* **Three classes, then:** (1) authority exercised **per call** — revocable by a lease today: endpoints,
  `LeaseAdmin`, `TaskControl`, `TaskCreate`, `Reboot`, `Interrupt`, `Clock`, `AuditDrain`, `CapInstall`;
  (2) authority that **becomes a resource** — `MmioRegion` (a mapping) and `DmaRegion` (a frame): a lease
  revocation stops *new* acquisition and leaves the resource; (3) the task itself, which the existing
  `TaskControl` kill takes back whole.

**So the honest statement for any of (a)–(c) is:** *a boot-time capability's authority can be taken back for every
future use; an MMIO mapping or DMA frame already obtained is not reclaimed by revocation*. Reclaiming a mapping
is a separate mechanism (track the mapping to its capability, unmap on revoke) and a larger line than this one.

### Where the handoff and the tree differ

* The handoff's **55** counts **source literals**; the number of capabilities installed on a boot is **101 (+ init's)**,
  and the number that matters for leases is **31 tasks**.
* The handoff's *"lose its MMIO window"* is not delivered by lease revocation (above).
* The handoff's **"one lease per task needs up to `MAX_TASKS` = 40"** overstates: the tree has 31 holders. The
  conclusion stands for a different reason — the broker's runtime draw.

---

## The decisions this RFC asks for

**D1 — which remedy.** The handoff's four, assessed against the measurements. **My recommendation is (a), with
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
**Recommendation: every task that receives a boot grant gets one lease, `init` excepted** (D4). Uniformity is the
point; a privileged-only rule is a second, hand-curated list (the E-125 family) deciding which kinds count.

**D3 — who owns a boot lease, and how it relates to the quota.** Three candidates, with what each breaks:

* **The task itself.** Its lease dies with it (`revoke_owned_by` on exit/fault, `dispatch.rs:305 :330`), so the
  table recycles as tasks exit — **capacity tracks live tasks, not boot history**. The quota is 1 per task: fine.
  But **the task can revoke its own authority** (voluntary, harmless: `cap_delete` already does) and **no one else
  can**, because 0.46-003 bounds revocation to the owner or `LEASE_REVOKE_ANY`. Involuntary revocation then needs
  a `LEASE_REVOKE_ANY` holder — and `capability-topology` currently permits **exactly one**, `neg-test`.
* **`init`.** The natural supervisor, and it already holds a `LeaseAdmin` (`main.rs`, slot 30). **31 > 16
  (`MAX_LEASES_PER_OWNER`)**, so it needs the quota lifted for boot leases — a second class of lease, which
  I do not recommend.
* **A kernel pseudo-owner.** Exempt from the quota by construction, outlives every task (so the table does *not*
  recycle with exits), and needs a `TaskId` value that names no task.

**Recommendation: the task itself, plus one `LEASE_REVOKE_ANY` holder that is not a test service** (`init`; it is the
supervisor, it already holds `LeaseAdmin`, and it is the only task whose revocation of another is *the point*).
That changes `capability-topology`'s allowed-holder rule from *only `NEG_TEST`* to *only `NEG_TEST` and `INIT`*,
which is the architect's to rule — it is the one place this line widens a deliberately narrow right.

**D4 — `init`.** It is the root of authority. A lease on `init`'s own capabilities that `init` could revoke is
harmless and one that nothing else can revoke is meaningless; revoking `init`'s authority from outside would
leave the system supervisor-less. **Recommendation: `init` is not bound**, and the RFC says so rather than leaving
it an unexplained exception. (The corresponding statement in the demonstration: *every task except `init`*.)

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

## Requirements (if D1 is (a))

**R1 — a lease per boot task.** In `spawn()`, after the capabilities are installed, create one lease owned by the
new task and bind every capability the task was granted to it (all `lease: None` literals in the per-image arms
become one bind step, so no literal needs editing and a new grant cannot forget it). `init` excepted (D4).
**R2 — `MAX_LEASES` 128**, with `lease_alloc`/`create` unchanged and the Verus model and `lease_conformance.rs`
re-run to show **0 changed obligations**.
**R3 — the revoker.** One `LEASE_REVOKE_ANY` holder besides `neg-test` (D3), and `capability-topology`'s count
updated to say which.
**R4 — the gate: `bootstrap-leases`.** A subcheck that **derives its population** — every `Capability { … }`
literal under `crates/fjell-kernel/src` outside `#[cfg(test)]` — and fails on one that is `lease: None` and is not
inside the spawn path that binds it, or is not on a **classified list with a reason** (`init`'s grants; the
`Capability` literals in the host-side tests). Evaluator, not a name match; a function it cannot delimit is a
failure.
**R5 — the statement, written down.** `slot.rs`'s *"Bootstrap capabilities carry `lease: None`"* becomes true or
is removed; `v1-limitations.md` row 12 is the architect's.

## Negative cases — what I will show

1. **The statement, demonstrated on a real boot:** `neg-test` holds a capability of a *peer*'s kind (it cannot hold a
   peer's capability — CSpaces are local), so the demonstration is the other direction: **a task's boot capability
   stops working when the `LEASE_REVOKE_ANY` holder revokes that task's boot lease, through every funnel route** —
   `ipc_send`/`ipc_call`/`ipc_try_recv`/`cap_inspect`/`cap_copy` each refused `LeaseRevoked`, with a marker naming
   the route. **And** the same revocation attempted by a task that is *not* the owner and lacks the right: refused,
   `PermissionDenied`, audited `0x0606` (already in the tree).
2. **A mint chain cascading while a copy survives**, side by side (the handoff's asymmetry, pinned): minted
   child dies with `cap_revoke` of its parent; the copy does not.
3. **`cap_revoke` on a capability with no descendants deletes nothing** — the documented, surprising behaviour.
4. **What revocation does not take back:** a task that has mapped its MMIO window keeps the mapping after its boot
   lease is revoked, and a *second* `sys_mmio_map` through the revoked capability is refused. **This is a test of
   the stated limit, so the limit cannot silently become a claim.**
5. **Capacity:** with every boot lease live, the broker can still grant up to its quota (the 31-and-16 interaction
   that kills the 32-slot version), and a revoked boot lease is reclaimed when its task exits.
6. **Mutations, each a named failure** (a compile error is INVALID): spawn skips the bind for one image → the
   `bootstrap-leases` gate red naming the literal; `MAX_LEASES` back to 32 → capacity case red; the bind covers all
   but one capability → case 1 red for that route.

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
