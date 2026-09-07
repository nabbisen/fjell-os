# RFC-0.28-003: The blocked-recv rendezvous exists — the test just cannot name the task

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.28
**Supersedes:** **RFC-0.26-003**, whose central premise is false (premise
correction recorded on that RFC, 2026-09-07).
**Tracks.** **E-019** — `tests/qemu/profiles/ipc.toml` passes and nothing holds
it there.
**Touches.** `crates/services/fjell-neg-test`,
`crates/services/fjell-sample-service`. **Possibly `crates/fjell-kernel` — only
if §4 is answered that way, which is an ABI change and escalates.**
**Relates to:** **E-010** (these markers false-passed for nineteen releases);
RFC-0.26-001 (which removed the accident the test relied on); RFC 055 (attested
sender identity).

## Summary

`fjell-neg-test::test_ipc_blocked_recv` needs `sample-service` to be **blocked
in `sys_ipc_recv`** before it revokes the lease — that is the whole point of the
test, which verifies the kernel wakes a blocked receiver on revocation.

Today, step 3 is this:

```rust
// 3. At this point sample-service has replied and is running.
//    By the cooperative-scheduling contract, sample-service immediately
//    calls sys_ipc_recv(SLOT_LEASED_EP) and blocks before the scheduler
//    returns to neg-test.  One defensive yield is included for safety.
sys_yield();
```

**RFC-0.26-001 removed the priority asymmetry that made that "contract" true.**
The profile is green anyway — RFC-0.26-004 fixed something upstream — which
means the guard is green **by a mechanism nobody designed**, for the second time
in its history. E-010 records these same markers false-passing for nineteen
releases.

RFC-0.26-003 concluded that nothing could be done, on the grounds that *"there
is no signal to wait on, and none can be trivially built."* **That is false, and
this RFC exists because it is false.**

## What is actually available

| | |
|---|---|
| `SyscallNumber::TaskStatus = 42` | **dispatched** |
| `trap/syscall.rs:474` | `TaskState::Blocked(_)` → `TaskLifecycle::Blocked` |
| `fjell-syscall:249` | `sys_task_status(cap_handle, task_handle)` wrapper exists |
| `fjell-neg-test` | **already imports it and already calls it** (`main.rs:683`, `:719`) |
| `neg-test`'s `TaskControl` cap, slot 6 | `scope: ObjectScope::Any` — **covers every task, no new grant needed** |

The kernel is the authority on whether a task is blocked, because the kernel is
what sets the state. Polling it is **stronger** evidence than any announcement
protocol, not weaker — RFC-0.26-003 reasoned only about the blocking task
announcing its own state and never considered asking the kernel.

## The real gap, which is much smaller

`sys_task_status` takes a **`TaskId`** — `TaskId::new(raw & 0xFFFF, raw >> 16)`,
a global identifier. `neg-test` knows the TaskIds of tasks **it spawned itself**
(that is what `main.rs:683` uses). **It did not spawn `sample-service`, and has
no way to name it.**

And it cannot learn it from the exchange it already performs. `neg-test` calls
`sample-service`; the **caller** does not receive attested identity. The reply
path (`cap/syscall.rs:712-726`) writes `a0`, `a1` and `a2..a5` into the caller's
frame and **does not write `a6`** — only `deliver()` does that, on the receive
side.

So the problem is not "no signal." It is **"the signal exists and the test
cannot address it."**

## The open question — §4

**How does `neg-test` learn `sample-service`'s `TaskId`?**

1. **`sample-service` returns its own TaskId in the reply words.** `a2..a5` are
   already copied through. Smallest change, entirely inside test infrastructure.
   Cost: **self-reported, not kernel-attested** — a lying service could name
   another task. For a test whose subject is kernel revocation behaviour, and
   where `sample-service` is trusted scaffolding, that may be acceptable, but it
   must be **stated as a limitation, not glossed**.
2. **Extend the reply path to carry `a6`**, as `deliver()` already does for
   sends and calls. Attested, and it removes a genuine asymmetry in the IPC
   contract — a caller learns nothing about who replied, while a receiver always
   learns who sent. **This is a kernel and ABI change: escalate before writing
   it.**
3. **`sample-service` performs a one-way `send` to `neg-test`** at bind time,
   whose delivery *does* carry attested `a6`. No kernel change, attested
   identity — at the cost of an extra message in the handshake.

**Answer in writing before implementing.** I am not stating an inclination: 1 is
smallest, 3 is attested without kernel change, and 2 is the one that fixes
something real beyond this test. That last point is worth weighing rather than
dismissing for being the largest.

## The settled part

**D1 — The rendezvous is a poll of `sys_task_status`, not an announcement.**
Whatever §4 decides about addressing, the wait is: poll until the task reads
`Blocked`, then revoke. Do not build an announcement protocol; RFC-0.26-003 was
right that a task cannot announce its own blocking atomically, and that is why
the observer asks the kernel instead.

**D2 — `TaskLifecycle::Blocked` collapses every blocking reason into one
value.** A poller cannot distinguish *blocked in `recv` on my endpoint* from
*blocked on anything else*. For `sample-service` in this test that is probably
immaterial — **check that it is, and record the reasoning either way.** It is
the kind of "probably fine" this project has repeatedly found not to be.

**D3 — The poll is bounded.** An unbounded poll turns a failing test into a
hang, and a hang is the failure mode this project is worst at diagnosing. Bound
it, and on exhaustion **fail with a marker**, never fall through to the revoke.

**D4 — The demonstration is the deliverable.** Remove the rendezvous and the
test must fail. A guard that passes both with and without the thing it guards is
what E-010 and RFC-0.26-003 are both about.

## Scope

`fjell-neg-test`'s `test_ipc_blocked_recv`; `fjell-sample-service`'s side of the
handshake; `tests/qemu/profiles/ipc.toml` if its markers change; **E-019** →
`CLOSED`; **RFC-0.26-003** → `archive/` as superseded.

### Non-goals

- Building an announcement protocol for blocked-ness (D1).
- Changing the kernel — unless §4 chooses shape 2, which escalates first.
- Widening `TaskLifecycle` to carry a blocking reason. If D2's check shows the
  collapse matters, that is a **finding and an erratum**, not this line's work.
- E-013, E-025, E-027, E-028, E-029, E-030, E-032.

## Risks

**`neg-test` and `sample-service` are the instruments here.** A test that passes
after this change proves nothing unless D4's inverse demonstration was captured
first. Capture it before the fix, not after — RFC-0.26-001's investigation is
the precedent, and RFC-0.28-001 followed it correctly.

**The green profile is load-bearing on something nobody has identified.**
RFC-0.26-004 made it green and no one knows exactly which part. Changing the
handshake may make it red for a *third* reason. That would be a good outcome
honestly reported, and a bad one absorbed.
