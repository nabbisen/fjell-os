# RFC-0.28-003 §4 — How does `neg-test` learn `sample-service`'s `TaskId`?

**Governing RFC:** [rfcs/accepted/RFC-0.28-003-blocked-recv-rendezvous.md](../../rfcs/accepted/RFC-0.28-003-blocked-recv-rendezvous.md)

Per the handoff's required order, this is written before any poll is built,
and before capturing what makes the test pass today — building the
addressing this section settles turned out to be a *prerequisite* for that
capture, not a separate step after it (see "the current mechanism" below).

---

## Two gaps the RFC's own three shapes do not mention

Checked before choosing, not assumed from the RFC's framing — both matter
for which shape is actually buildable without touching the kernel.

**Shape 1's gap: nothing lets a task learn its own `TaskId`.** Searched
`fjell-abi`/`fjell-syscall` exhaustively (every `SyscallNumber` variant,
every wrapper) — there is no `sys_task_self` or equivalent. `sample-service`
cannot "return its own `TaskId` in the reply words" because it has no way to
*obtain* that value in the first place. Self-loopback (send to its own
endpoint, then receive) cannot substitute: a single-threaded task cannot be
its own rendezvous partner — the sender blocks until *someone else*
receives, and there is no one else. This is a real, previously-unstated
prerequisite gap, not a preference against shape 1.

**Shape 3's naive form has a collision, of the exact kind this project has
already been bitten by.** `neg-test`'s *only* receive-capable endpoint —
`SLOT_OWN_EP`, object 0 — is the same shared object `fjell-auditd` and
`fjell-bootctl` block-`recv` on in a tight loop for the entire run. Routing
a new identity-announcement there risks it being silently absorbed by
whichever of the three has been parked in that `recvq` longest — almost
certainly `auditd`, spawned earliest. Confirmed by reading both loops:
`fjell-auditd::service_main`'s `loop { sys_ipc_recv(0u32); drain_once(...); }`
and `fjell-bootctl`'s equivalent — both persistent for the run's duration,
not just at boot. This is RFC-0.28-001's object-0 finding, one honest
`grep` away from recurring in new test code.

## The fix to shape 3 that avoids both gaps: use the channel that already exists for exactly this

`neg-test` already holds **`SLOT_SAMPLE_EP` (object 6) with both `CALL` and
`RECV` rights** — proven by its own existing code: `test_ipc_blocked_call_
and_late_reply` already calls `sys_ipc_recv(SLOT_SAMPLE_EP)` to receive
`sample-service`'s callback on this exact object. Object 6 is dedicated
specifically so this two-party protocol "cannot be stolen by other
shared-endpoint receivers" (`spawn.rs`'s own comment) — no third service
holds any capability to it at all. **This is shape 3, unmodified in spirit,
routed through the channel the codebase already treats as private to this
pair, instead of the one that isn't.**

Sequencing removes the only remaining hazard (could `sample-service`
receive its own announcement?): a one-way `sys_ipc_send` blocks the sender
until *someone else* receives — `sample-service` cannot simultaneously be
blocked sending and be a receiver, so it structurally cannot consume its
own message. It sends, then blocks; `neg-test`, once unblocked from its own
call, calls `sys_ipc_recv_msg(SLOT_SAMPLE_EP)` and is the only other party
that could ever receive it.

**No kernel change. No new capability grant** (the rights already exist).
One new one-way message, in the direction `sample-service` → `neg-test`,
carrying no payload — the value is the delivery's kernel-attested sender
identity (`sys_ipc_recv_msg`'s 6th return element), not anything
self-reported.

## Shape 2 — considered on its merits, not dismissed for size, and not chosen

Extending the reply path to carry `a6` fixes a real asymmetry: a receiver
always learns who sent; a caller learns nothing about who replied. That
asymmetry is real and independent of this test. But it is a kernel and ABI
change to `sys_ipc_reply`'s completion contract — exactly the class of
change this project's own discipline (`STABLE_CRATES`, the ABI snapshot,
`syscall-surface`) requires showing to the owner before a line is written,
per the handoff's explicit instruction. The shape-3 fix above closes this
line's actual gap — a test that cannot address its own subject — without
that change. **Recommending it as a separate, future escalation**: the
asymmetry shape 2 would fix is real and worth the owner's attention on its
own merits, independent of whether this test needs it.

## D2 — the `Blocked` collapse, checked

`TaskLifecycle::Blocked` collapses every `TaskState::Blocked(_)` reason into
one value. Between `sample-service`'s reply and the revoke, could it be
`Blocked` for a reason *other than* the leased `sys_ipc_recv` this test is
about? Traced its exact instruction sequence in that window: reply (does
not block the replier) → the identity-announcement one-way send (§4) →
`sys_ipc_recv(leased_h.0)`. The identity-send **could** block
(`BlockReason::ReservedForIpc`, indistinguishable from the leased recv's
own block reason at the `TaskLifecycle` level) if `neg-test` has not yet
called `sys_ipc_recv_msg` to receive it — but `neg-test`'s own code
receives that message, unconditionally, *before* the poll loop starts.
By the time the poll's first iteration runs, the identity-send has
already been drained (drained ⟺ delivered ⟺ `sample-service` already woken
and past that line), so the only blocking point left in
`sample-service`'s path is the leased recv. **Checked, not assumed**: the
collapse is real in general (confirmed by reading `TaskState`'s
definition) but immaterial to this specific poll, because the ordering is
structural (the poll cannot start before the drain completes), not timed.

## Generation, checked rather than assumed

`sys_task_status`'s `task_handle` packs `(index | generation << 16)`
(`trap/syscall.rs`'s `sys_task_status`); the attested sender identity from
IPC delivery packs `(sender_tid | sender_image_id << 16)` — **not the same
encoding**. `sample-service` is spawned once during boot and never
removed, faulted, or respawned for the duration of any QEMU negative-test
profile (confirmed: no code path calls `sys_task_kill` or otherwise removes
it; `TaskTable::remove` is the only place a slot's generation increments,
per `crates/fjell-kernel/src/task/tcb.rs`). Its generation is therefore `0`
for the whole run, and `TaskId::new(attested_tid, 0)` is correct — stated
here as a checked fact about this specific scenario, not a general
assumption about attested identities encoding generation (they do not).

---

## What currently makes the test pass — captured, not assumed

Building the addressing above (a prerequisite, per the order note at the
top) made this directly observable rather than theoretical.

**Without any wait at all** (identity received, `sys_task_status` checked
exactly once, no `sys_yield`): the status reads `Runnable` (`1`), not
`Blocked` (`3`) — `sample-service` has not yet reached its own
`sys_ipc_recv(leased_h.0)` call. This was captured live (`cargo xtask
qemu-negative ipc`, 3 consecutive runs, identical result each time) by
temporarily replacing the poll body with a single immediate check —
**the inverse demonstration D4 requires**: with the wait removed, the new
`NEG:HARNESS:BLOCKED_RECV_POLL_EXHAUSTED` marker fires and
`cargo xtask qemu-negative ipc` correctly reports **FAIL**
(`[xtask] FORBIDDEN marker ... present`), not a silent pass.

**With the real bounded poll**, `sample-service` reaches `Blocked` after
**exactly 2 iterations** (one intervening `sys_yield()`) — measured across
6 consecutive runs, identical every time on this build. **This precisely
characterizes what the old single-`sys_yield()` comment was relying on**:
one yield was, in fact, enough, today, on this exact code — but "enough"
was never checked, only assumed, and the zero-yield measurement above
proves there was no margin: one fewer yield and the revoke lands on a
`Runnable` task, not a `Blocked` one. The old code's own `sys_ipc_recv`
call still returns `LeaseRevoked` in that case (`check_right`'s lease
check rejects the syscall before it ever queues), so the same `PASS`
marker still fires — the "instant lease-check rejection" failure mode
`tests/qemu/profiles/ipc.toml`'s own comment already documents from this
test's history, not a new hazard, but the exact one this line closes.

**Reconciling with the RFC's `RFC-0.26-001` framing**: that RFC removed a
priority asymmetry that used to make `sample-service` provably reach its
`recv` before any other task could run. The measurement above shows the
schedule today still happens to leave `sample-service` two yields away —
a property of the current, specific instruction sequence between the
reply and the `recv` call, not a contract anything guarantees. Confirmed
mechanism: after `sample-service`'s reply, `trap_dispatch`'s
`schedule_next` re-enqueues the replier like any other completed syscall
(the reply itself does not hand off control), so `sample-service`
continues immediately — but its *own* subsequent one-way identity-send
(§4) is itself a syscall boundary at which `neg-test`, freshly woken by
the reply, gets a turn; the exact interleaving from there on is a
property of instruction counts, not a documented invariant, which is
D1's whole argument for polling instead of continuing to assume it.

---

## Evidence (D4)

A post-commit, clean-tree serial log of the final implementation (real
bounded poll, `sample-service` reaching `Blocked` at iteration 2) is
promoted at
[`tests/evidence/RFC-0.28-003/blocked-recv-poll-real.log`](../../tests/evidence/RFC-0.28-003/blocked-recv-poll-real.log)
(provenance:
[`tests/evidence/RFC-0.28-003/blocked-recv-poll-real.provenance.txt`](../../tests/evidence/RFC-0.28-003/blocked-recv-poll-real.provenance.txt)).
The zero-yield inverse demonstration was captured against temporarily
modified code that was reverted before commit, per D4's own point — it is
described above rather than promoted, since promoting a log from code that
no longer exists in the tree would be evidence of nothing checkable
against `HEAD`.
