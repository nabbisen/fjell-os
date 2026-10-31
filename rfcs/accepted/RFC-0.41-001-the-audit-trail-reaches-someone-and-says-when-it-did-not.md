# RFC-0.41-001: The audit trail reaches someone, and says when it did not

**Status:** Accepted — accepted 2026-10-30
**Milestone:** 0.41
**Tracks.** **E-102** — `auditd` drains the kernel ring once at start-up and then only on a
message nothing sends, so the audit trail stops at boot. Also from the same entry, found while
ruling on its direction: the per-drain **drop count is discarded** by the only consumer that
renders the trail, and the drain **copies at most 64 of up to 256** pending records, so one drain
per wake cannot empty the ring.
**Touches** *(indicative)*: `crates/fjell-abi/src/syscall.rs`,
`crates/fjell-kernel/src/task/tcb.rs`, `crates/fjell-kernel/src/trap/syscall.rs`,
`crates/fjell-kernel/src/trap/dispatch.rs`, `crates/fjell-kernel/src/audit/ring.rs`,
`crates/services/fjell-auditd/src/main.rs`, `tests/abi/snapshot.json`,
`tests/qemu/profiles/audit.toml`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.41-002**, which takes the shared-endpoint half of E-102 and **must land
after this one**; **E-068**, which retracted a *query sub-protocol* and does **not** decide this —
see §Non-goals; **RFC-0.40-002**, which delivered a correctly coded revocation that this line
makes visible; **RFC-0.25-001**, whose `sys_irq_wait` is the pattern this line follows.

## What is wrong

The kernel records the trail, the published decoder names it, and **the one consumer that renders
it sees nothing after boot.**

- `crates/services/fjell-auditd/src/main.rs:101`–`108`: one drain at start-up, then
  `loop { sys_ipc_recv(0u32); drain_once(); }`, above a comment reading *"Other services (or init)
  send a message to endpoint 0 to trigger a drain."*
- **Nothing sends to object 0.** `AUDITD` is not among the `ep_obj` match's 15 dedicated arms
  (`crates/fjell-kernel/src/task/spawn.rs:272`), so it falls to `_ => 0`, and the one declared
  capability that could have carried a trigger was deleted when E-068 closed by retraction.
- So a lease revocation after boot — correctly coded and correctly decodable since RFC-0.40-002 —
  **is never printed.** `auditd`'s whole output in a normal boot is the early-boot records.

A comment that names a sender which does not exist is not a stale comment; it is why three
separate lines (RFC-0.28-001, RFC-0.33-001, RFC-0.40-002) each routed around this gap instead of
hitting it. The code reads as though the mechanism is present.

Two further defects sit in the same path and must be fixed with it, or the fix is not a fix:

**The loss count is received and thrown away.** The kernel counts dropped records twice —
`dropped_count` (lifetime) and `drops_since_advance` (per drain) — documents the per-drain figure
at `crates/fjell-kernel/src/trap/syscall.rs:687` and returns it in `a2` (`:797`). `auditd`'s
`drain_once` binds it to `_`:

```rust
Ok((n_records, _n_dropped)) => {            // src/main.rs:76
```

An operator reading the console therefore sees a trail with **silent holes**.

**The drain under-drains by construction.** `DRAIN_BUF_RECORDS` is **64** (`src/main.rs:24`);
`AUDIT_RING_CAPACITY` is **256** (`crates/fjell-kernel/src/audit/ring.rs:12`). One `drain_once`
copies at most 64 records and the loop then blocks again, leaving up to 192 behind. **Any** design
that drains once per wake is wrong on its own terms, a working message trigger included.

And the loss is not hypothetical. The ring drops the **newest** record when full
(`audit/ring.rs:116`–`117`), and during service start-up `TaskSwitch` records arrive at roughly 32
per drain. A high-rate kind therefore **evicts the bounded kinds that matter** — a flood of
scheduling telemetry destroys the revocation that follows it, and today that destruction is
invisible.

### The direction was decided, and one alternative was withdrawn

The owner delegated E-102's direction to the architect on 2026-10-30 with the standing design bar
as the instrument. The ruling, recorded in E-102:

**Drain on a kernel-side wake — not on a message, and not on a timer.**

A message trigger needs a sender; every candidate re-opens the authority question E-068 declined,
and the only object `auditd` can receive on is the contested shared object 0, so the trigger would
be built *on* the hazard in the same entry.

A periodic drain — **the architect's own earlier recommendation, now withdrawn** — has no
mechanism to stand on. `BlockReason` has a `Sleep` variant that **nothing sets**; the only setters
in the kernel are `ReservedForIpc` (`cap/syscall.rs:924`) and `Irq` (`trap/syscall.rs:1261`), and
`Yield` is unset too. There is no sleep syscall and no userspace timer facility, so a periodic
drain means inventing one — more permanent structure than one syscall following an existing
pattern — and it is less correct, being stale by up to one period by construction while the ring
overflows *between* ticks.

What does exist is the pattern to copy. `IrqWait = 102` parks a task on a kernel-side event, and
`crates/fjell-kernel/src/trap/dispatch.rs:158`–`180` wakes it, **including a lost-wakeup guard**:
if the bound task has not yet called `sys_irq_wait`, the event is recorded so the next wait
returns immediately instead of blocking on something already past. An audit wait has exactly that
hazard.

## Requirements

**R1 — `AuditWait`, a syscall of its own.** Add `AuditWait = 61` to `SyscallNumber`
(`crates/fjell-abi/src/syscall.rs`; 61 is free, 60 is `AuditDrain`). It takes the same
`AuditDrain` capability handle the caller already holds, and blocks until the ring is non-empty.

**Not** by making `AuditDrain` block when empty. A syscall named `Drain` that parks its caller
indefinitely is a name that implies less than it does, and the ABI already establishes the idiom
in which waiting is its own number: `IrqBind` / `IrqAck` / `IrqWait`. This is an ABI **addition**,
declared here — see R8.

**R2 — a real block reason.** Add a `BlockReason` variant for the audit ring
(`crates/fjell-kernel/src/task/tcb.rs:112`). It carries no payload: there is one ring, so there is
nothing to discriminate. Do **not** reuse `Sleep` — see R9.

**R3 — the wake, on the ring's append path.** When a record is appended to an empty-and-waited-on
ring, wake the parked task: set `Runnable` and enqueue it, exactly as the IRQ path does. The wake
belongs on append, not on a timer tick and not on the scheduler's path.

**R4 — the lost-wakeup guard, demonstrated and not assumed.** A record appended between "a drain
returned empty" and "the task called `AuditWait`" must make the next `AuditWait` return
immediately. Model it on the IRQ path's guard. **A tier must exercise the race**, not merely the
happy path: produce a record in that window and assert it is still delivered. An absence probe
that passes because the window was never hit is not evidence.

**R5 — drain until empty.** `auditd`'s loop becomes: drain repeatedly until the kernel reports the
ring empty, then `AuditWait`. The 64-record buffer stays; the loop, not the buffer, is what must
change. After this, `auditd` **receives on no endpoint at all** — delete the `sys_ipc_recv(0)` and
the comment that describes a sender which does not exist.

**R6 — the loss is reported.** When a drain reports `n_dropped > 0`, `auditd` emits a line saying
how many records were lost at that point in the trail. **It must not be mistakable for an audited
event**: a reader must be able to tell "the kernel dropped 4 records here" from "the kernel
recorded an event". Shape it deliberately — this is operator surface, and a line a reader
misreads is a defect, not polish.

**R7 — `TaskSwitch` is filterable at the append site, and defaults to off.** A record kind that
can arrive unboundedly must not be able to evict kinds that cannot. Filter where the record is
appended, not where it is rendered: a record dropped at the console still consumed a ring slot.

*The default is a design position, the architect's, and the owner may reverse it. The correctness
half does not depend on it: an unbounded kind can evict a bounded one, and until R6 that eviction
is invisible. If the default is reversed, R6 is what keeps the result honest.*

**R8 — the ABI surface moves by declaration.** The stable surface is 498 items today. R1 adds to
it, so update `tests/abi/snapshot.json` in the same commit as the syscall. `ci-abi-check` must
report the addition because the snapshot was updated deliberately — not discover it.

**R9 — do not fold in the dead block reasons.** `BlockReason::Sleep` and `BlockReason::Yield` are
set by nobody. That is real, and it is **not this line's work**; it is scheduled separately. Add
the new variant beside them and leave them alone. Folding a state-machine cleanup into an
audit-trail fix is how a line grows until nothing in it can be reviewed.

**R10 — a tier asserts what an operator sees.** The property is not "the syscall returns". It is
that a revocation raised **after boot** appears in `auditd`'s own output. RFC-0.40-002 had to
assert its property inside `neg-test` because this consumer could not see anything; that
workaround should no longer be necessary, and the tier should read the trail where a person would.

### Non-goals

* **Who may read the audit trail.** E-068's question, deliberately not needed here: a wake on the
  ring grants authority over nothing, and `auditd` already holds `AuditDrain`. Do not introduce a
  reader-authority model in this line.
* **A query sub-protocol.** That is what E-068 retracted. This is a trigger for `auditd`'s own
  loop.
* **The endpoint topology.** `auditd` leaving object 0 is a consequence of R5, not a fix to the
  shared-object hazard. RFC-0.41-002 owns that and lands after this.
* **The record format.** No audit record changes shape. `SCHEMA_VERSION` is untouched.
* **Ring capacity.** 256 stays. R7 addresses pressure at the source; a bigger ring would only
  move the overflow later.

## Risks

**A wake that is lost is worse than no wake**, because the trail would then be silently
incomplete in a *new* way while appearing to work. R4 is the mitigation and it must be a real
race test, not an assertion that the happy path works.

**A wake on the append path runs in whatever context appends a record** — including kernel paths
that hold state. Keep the wake to the same two operations the IRQ path performs (mark runnable,
enqueue) and add nothing that can fail or block there.

**R7 changes what the trail contains by default.** That is a reduction in recorded events, and it
must be stated in the release notes as such, with the reason: the ring drops the newest record, so
an unbounded kind was already costing the trail its bounded ones. A reader who sees fewer
`TaskSwitch` lines should be able to find out why.

**Prebuilts will move** for `auditd` and the kernel. Per standing practice the repro baseline is
re-recorded in the same commit as the rebuild.

## Amendments — 2026-10-31, architect, after review

**R3 is corrected. The implementation was right and this requirement's wording was wrong.** R3 said
the wake "belongs on append, not on a timer tick and not on the scheduler's path". The wake **cannot**
run at the append site: `lock_free_append` is called from inside `schedule_next`
(`trap/dispatch.rs:306`, `:338`), which already holds `&mut` to the task table and the scheduler, and
the wake needs both — doing it there requires `unsafe` re-entry into state the caller holds. The
delivered design is correct: the append **claims** the waiter (a `Cell` write that cannot fail), and
the two IRQ-path operations run at the tail of the same trap in
`trap::dispatch::deliver_audit_wake`, called at `:367` before `choose_next` at `:370`, so the woken
task can be chosen in the trap that appended the record. What the phrase "not on the scheduler's path"
was ruling out was a **polling** drain; as written it forbade the only implementable design.

**R4 is corrected in the same spirit.** It said to model the guard on the IRQ path's latch. The
delivered guard is a **level, not a latch** — `park()` returns immediately when the ring is non-empty
— because the condition a waiter waits for is a *state*, not an *edge*. That closes the window
structurally, with no latch to fail to set, and is stronger than what this requirement asked for.

**R11 — new requirement, to be done in 0.41 before the cut.** `sys_audit_wait` reports in `a1`
whether it parked the caller or returned at once, and the `audit` profile requires at least one run of
the lost-wakeup sweep in which it did **not** park.

*Why.* The implementer's own residual-risk finding: the race window's position depends on how many
traps `auditd` spends per record, so if that drifts the window leaves the sweep and **the test passes
without touching the guard, with nothing detecting it**. *Why now rather than 0.42:* `sys_audit_wait`
has never been released, so a meaningful `a1` today is a free ABI addition; after 0.41.0 ships it is a
semantic change to a published ABI item.
