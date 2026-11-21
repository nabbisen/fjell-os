# RFC-0.46-001: Lease slots are recycled, and the proof says why that is safe

**Status:** Accepted — accepted 2026-11-21
**Milestone:** 0.46
**Tracks.** **E-110** — `MAX_LEASES = 32` slots, never recycled; 0.45 made the limit reachable from a
production path. Does **not** by itself close the reachable denial of service (see §"A premise this RFC
corrects"); asks for two further bounds that do.
**Touches** *(indicative)*: `crates/fjell-kernel/src/lease/mod.rs`, `crates/fjell-abi/src/lease.rs`,
`verification/verus/lease/lease_epoch.rs`, `crates/fjell-cap/tests/lease_conformance.rs`,
`verification/verus/verus-targets.toml` (unchanged shape), `crates/services/fjell-cap-broker/src/main.rs`,
`crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/{policy,ipc}.toml`, prebuilts and
`tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.46-003** (do it first: recycling plus a revoke you do not own is worse than either
alone), **E-093** (recursive revocation — kept out on purpose), **LEASE-001**, **LEASE-VERUS-005**
(retire-before-wrap), `TaskTable` (explicitly *not* touched, §Non-goals).

---

## What is true now

Re-measured at `3f141a9`:

- `MAX_LEASES = 32` (`lease/mod.rs:23`). `LeaseTable::create` allocates **only** over `LeaseState::Empty`.
  `revoke` moves a slot to `Revoked` and **never increments the generation**; the code says so itself and
  records that a false claim to the contrary was removed on 2026-10-21.
- `LeaseObject` already carries a `generation: u16`; `LeaseId` carries one in its high half;
  `get`, `revoke` and the rest **already check it**. It has never had anything to discriminate.
- The cap-broker creates one lease per grant request (0.45-002 R5).

### A premise this RFC corrects: recycling `Revoked` slots does not close the reachable defect

The 0.45 measurement the handoff cites — *a requester looping request-then-drop is granted 26 times, after
which every other task's `sys_lease_create` fails* — **dropped the capability and never sent `CAP_REVOKE`**.
Dropping a capability does not touch its lease: the lease stays `Active` until someone revokes it, and only
the broker (on a `CAP_REVOKE` from the grantee or `init`) or the owner's exit does. So in that loop **every
lease is `Active`, not `Revoked`**, and a recycler that reclaims only revoked slots reclaims none of them.

Recycling fixes the *well-behaved* cycle — grant, use, revoke, grant again — which today exhausts the table
after 26 rounds and never recovers. It does **not** fix a requester that asks and abandons. That defect is
about *who may hold how many leases*, not about whether a slot comes back. This RFC therefore asks for
three things, in order of how directly they answer the measured denial of service:

| # | Bound | Where | Answers |
|---|---|---|---|
| **R-Q** | A task owns at most `MAX_LEASES_PER_OWNER` `Active` leases | kernel, `LeaseTable::create` | one owner exhausting the table **for everyone else** |
| **R-G** | The broker grants a requester at most `MAX_GRANTS_PER_REQUESTER` outstanding grants | `cap-broker` | one requester exhausting **the broker's** share for the other requesters |
| **R-R** | A `Revoked` slot is reclaimed, its generation bumped | kernel, `LeaseTable::create` | the grant-use-revoke cycle running forever |

R-Q is the one that restores the property the handoff wants — *a requester the policy admits can no longer
exhaust a kernel resource for every other task* — and it is a **kernel** bound, in this milestone's theme.
R-G is fairness among the broker's own clients, and is service policy (the kernel bound limits its blast
radius). R-R is the handoff's line.

## Requirements — the recycling (R-R)

**R1 — when a slot is reclaimed: on allocation pressure, not on revoke.** `create` prefers an `Empty` slot;
if none exists it reclaims a `Revoked` one. *Not* in `revoke`. Reason, from the tree: a capability bound to a
lease that has just been revoked must still answer `LeaseRevoked` (the 0.45 `BROKER-004` demonstration and
five `neg-test` scenarios assert exactly that error). Eager reclamation would turn that into `InvalidCap`
(stale generation) the moment the slot is reusable, and would make the answer depend on a race with the next
`create`. Lazy reclamation keeps the old answer for as long as the slot is not needed. The cost is that
revoked slots occupy the table until pressure; at 32 slots that is the point.

**R2 — which `Revoked` slot: round-robin from a cursor**, not lowest-index. The longest-revoked slot is
reclaimed first, which maximises the window in which a stale binding still reads `LeaseRevoked` rather than
the generation mismatch. One `u8` of state in the table.

**R3 — the new incarnation.** `generation += 1`, `state = Active`, a new `owner`. **The epoch continues; it
is not reset to 1.** (A binding from the old incarnation then differs by epoch *and* by generation.) The
proof (below) is written so it holds either way.

**R4 — retire-before-wrap, for the generation.** A slot whose generation is `u16::MAX`, or whose epoch is
`u32::MAX`, is **never reclaimed** — the same rule LEASE-VERUS-005 already applies to the epoch. 65 535
reuses of one slot is practically unreachable and the rule costs one comparison; without it, reuse number
65 536 would resurrect every binding ever issued against index *i*.

**R5 — a stale `LeaseId` is `LeaseRevoked` on *use*, `InvalidCap` on *address*.** `check_active` and
`active_epoch` (what `require_cap` step 7 and `sys_cap_bind_lease` call) map "the slot's generation is newer
than the id's" to `LeaseRevoked`: that capability was bound to a lease that no longer exists, and the answer
is the same as the one it gave a moment before the slot was reclaimed. `revoke`, `owner` and `inspect` by a
stale id return `InvalidCap` (it names nothing). An id with a *newer* generation than the slot, or an index
out of range, is `InvalidCap` in both — it was never issued.

**R6 — `revoke_owned_by` and the IPC cancellation are unchanged.** They are keyed by `(id, epoch)` at
revocation time, which happens before any reclamation of that slot; a `ReplyEdge` bound to the lease was
cancelled when it was revoked, so none can name a reclaimed slot.

## Requirements — the bounds (R-Q, R-G)

**R7 (R-Q) — `MAX_LEASES_PER_OWNER`.** `create` refuses (`NoMemory`) when the owner already owns that many
`Active` leases. Proposed value **16** of 32 — the architect's call, and *not* the number the register
refuses to move: this does not raise `MAX_LEASES`, it makes one task unable to take all of it. With the
broker as the sole owner of grant leases, the broker is bounded to half the table and every other task
keeps the other half. (The bound counts `Active` only, so R-R is what makes it sustainable.)

**R8 (R-G) — `MAX_GRANTS_PER_REQUESTER`** in the broker, counted from its own `DelegationRecord`s:
at the limit a request is `CAP_DENIED` with a console line saying why, and a `CAP_REVOKE` by the grantee
(already authorised, R7 of 0.45) frees a slot. Proposed **4**. Service policy; shown on a boot.

## The proof (the constraint that outranks the feature)

`verification/verus/lease/lease_epoch.rs` models `revoke`, `check_active` and retire-before-wrap on a slot
that is **never reused**; its safety argument is *"epochs are monotone within a slot that is never
reused"*, and recycling deletes the second half of that sentence. This line **extends the model**; it does
not re-verify the old one.

**Prototyped before this RFC was written, so the feasibility is a fact and not a hope** (scratch file under
`.git-exclude/tmp/verus/`, not committed; the pinned prover `release/0.2026.11.02.8bf68ee`):

- `Lease { active, epoch, gen }` and `Binding { epoch_at_issue, gen_at_issue }`; `usable` additionally
  requires `l.gen == b.gen_at_issue`; `can_recycle(l) = !l.active && l.gen < u16::MAX`; `recycle` advances
  the generation and takes **an arbitrary new epoch**.
- Six obligations: a binding usable before `revoke` is not usable after `revoke` + `recycle`, **for any
  new epoch**; the same when the new epoch is chosen to *collide* with the binding's; the generation is
  strictly monotone and stays in `u16`; an `Active` slot cannot be recycled; the old
  `revoked_binding_not_usable` survives; **no ABA across two reuses**. **Result: `6 verified, 0 errors`.**
- **The proof is not vacuous.** With the generation clause removed from `usable`, the same file gives
  **`3 verified, 3 errors`** — the recycling obligations fail without the generation, which is exactly the
  claim *"the epoch alone cannot carry this once a slot restarts"*. The current model alone is
  `5 verified, 0 errors`.

Gate 10 for `lease` reads 5 verified today. The line must leave it green **on the new obligations**, not on
the old five re-verified: the existing five are restated over the extended `Lease`/`Binding` types (they
cannot stay literally unchanged, `Binding` gains a field) and the recycling obligations are added. The
final count is reported from the run, not predicted here; the prototype's six are a floor. `fjell_abi::lease` gains the host
mirror of `can_recycle`/`recycle` and `lease_conformance.rs` drives the same cases — the arrangement the
current proof already uses ("the kernel is no_std and not host-testable, so the pure decision predicate is
mirrored host-side").

### Not in this line, and said plainly

- **The task table's generation is untouched.** `TaskTable::remove` is the only site that would bump a
  task slot's generation and **it has no callers**, so task generations are always 0, and **26 sites across
  7 files** construct `TaskId::new(x, 0)` correctly *because* of that. This RFC bumps a generation in a
  **neighbouring** table. A reader must not take "generations are live now" as true of both: it is true of
  leases and false of tasks.
- **`MAX_LEASES` is not raised.** Moving a number is not fixing a family (E-025's own words).
- **E-093** (recursive revocation) is not touched; pairing two lease-model changes is how a proof
  obligation gets rushed.

## Demonstrations (each shown red by mutation, on a real boot where the tier can reach it)

1. **R-R:** a task cycles create → revoke 100 times (more than `MAX_LEASES`) and the 100th succeeds; with
   reclamation removed the 33rd fails `NoMemory`.
2. **R5:** a capability bound to a lease, the lease revoked and its slot **reclaimed by another create**, is
   used → still `LeaseRevoked` (not `InvalidCap`, not success). The mutation that drops the generation check
   makes that use *succeed* against the new incarnation — the ABA this exists to prevent — and is red.
3. **R4:** the host test drives one slot to generation `u16::MAX` and shows it is never reclaimed.
4. **R-Q:** a task holding `LeaseAdmin` creates leases until refused at `MAX_LEASES_PER_OWNER`, and a
   *different* task's `sys_lease_create` still succeeds (the broker-DoS property, from the outside).
5. **R-G:** the 0.45 request-and-abandon loop is refused at `MAX_GRANTS_PER_REQUESTER` with the broker's
   own console line, and **other tasks' `sys_lease_create` still succeeds** afterward — the measured defect,
   re-run, now closed.
6. The `ipc`, `policy`, `audit` and `capability` tiers stay green: the `LeaseRevoked` answers they assert are
   R1/R5's reason for existing.

## Risks

- **Lazy reclamation changes `create`'s worst case** from O(32) to O(32) with a second pass: no change in
  order, one more loop. Stated because LEASE-001 is about `revoke` being O(1), which it remains.
- **`LeaseId` values change meaning**: today an id equals its slot index (`0..32`); after reuse it does not.
  `neg-test` prints lease ids into its audit lines (`arg0=…`) and the broker packs `lease & 0xFF` into its
  reply; both stay correct while ids stay below 256, and the broker's packing is **wrong for a reused slot**
  (generation in the high half): it must pack the index, or the field must widen. The RFC requires the
  reply to carry the `LeaseId` value unchanged in a field wide enough for it (today the lease field is 8 bits at bit 48, sharing the top 16 bits with the requester's task
  index; the reply layout needs reworking, which is the broker's and this line's to do).
- **The formal model is a mirror, not the kernel.** The proof covers the *pure decision*; it does not prove
  the table code. The conformance test and the demonstrations above are what tie them.
- **The quota is a new way for a request to fail** (`NoMemory` from `sys_lease_create`). Every existing
  caller already handles that error; none loops on it.

## What this needs from the architect

1. **Is R-Q (a kernel per-owner quota) in scope?** It is not in the handoff's "what to build", but the
   handoff's own measured defect cannot be closed without it or R-G. If you would rather keep this line to
   recycling, say so and I will file the premise correction as a finding and leave the DoS open.
2. `MAX_LEASES_PER_OWNER` (16) and `MAX_GRANTS_PER_REQUESTER` (4).
3. **Lazy** reclamation (R1) rather than reap-on-revoke, and the epoch **continuing** (R3).
4. Whether E-110 closes on this line (it would, with R-Q and R-G) or stays open for the abandon case.

## Rulings — architect, 2026-11-21 (`.git-exclude/reviewed/review-record-0.46-stage-1.md`)

**Accepted. The decisions this RFC asked for are settled here, and the requirements below are added to it; reasoning is in the review record.**

* **R-Q is in scope, and is now the line's point.** Recycling closes grant-use-revoke-grant; R-Q restores
  the property wanted: *a requester the policy admits can no longer exhaust a kernel resource for every other
  task*. R-G is accepted as service policy and defence in depth, never the only bound.
* **The numbers, 16 and 4, are accepted, but the RFC states the property, not the number:** *no single owner
  can deny the table to everyone else.* Any defensible number satisfies it.
* **R7 must not refuse with `NoMemory`.** `SysError::TaskTableFull = -35` exists with the comment
  "distinct from `NoMemory`" (RFC-0.34-002 D1); the lease table never received that treatment. This line
  owes **two** distinct answers: the table is genuinely full (a `LeaseTableFull`-shaped error) and the owner
  is at quota (its own error). It is an ABI addition: enumerate it where the knowledge is and regenerate the
  snapshot **in the same line**.
* **Lazy reclamation on allocation pressure, round-robin, and a continuing epoch: accepted** (R1–R6).
* **E-110 closes on this line only if R-Q and R-R both land.** With recycling alone it does not. The
  erratum's closing condition names both. If R-Q is dropped, E-110 stays ACCEPTED and the RFC says so.
* The Verus prototype is the feasibility evidence; Gate 10 must come back green on the new obligations.
