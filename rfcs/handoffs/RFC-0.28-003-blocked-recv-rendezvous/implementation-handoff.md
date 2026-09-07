# Developer Handoff — RFC-0.28-003

**Governing RFC:** [RFC-0.28-003](../../proposed/RFC-0.28-003-blocked-recv-rendezvous.md)
**Milestone:** 0.28
**Status:** inherited from the governing RFC (Proposed — awaiting owner acceptance)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Capture the failure first, and it is not obvious how

D4 says: remove the rendezvous and the test must fail. **The problem is that
today, with no rendezvous at all, the test passes.** That is the entire reason
this line exists.

So "make it fail" is not "delete the poll and watch it go red." The profile is
green by a mechanism nobody has identified, inherited from RFC-0.26-004.
**Before you write the fix, find out what currently makes step 4's revoke land
while `sample-service` is blocked**, and capture that. If you cannot make the
test fail on demand, you cannot prove the rendezvous does anything, and this
line will have replaced one unexplained green with another.

That is the deliverable. The poll is the easy part.

## 0.1 Design decisions settled — do not re-open

1. **Poll the kernel; do not build an announcement protocol** (D1).
   RFC-0.26-003 was right that a task cannot atomically announce its own
   blocking. It was wrong that this meant nothing could be done — the observer
   asks the kernel instead.
2. **The `Blocked` collapse must be checked, not assumed** (D2).
3. **Bound the poll; fail with a marker on exhaustion** (D3). Never fall through
   to the revoke.
4. **The inverse demonstration is the deliverable** (D4).

---

## 1. Order

**§4 answered → capture the current pass mechanism → inverse demonstration →
the poll → markers → E-019, supersede RFC-0.26-003.**

## 2. §4 is a real choice and I am not steering it

**How does `neg-test` learn `sample-service`'s `TaskId`?** Three shapes in the
RFC. Shape 1 is smallest and self-reported. Shape 3 is attested with no kernel
change but adds a message. **Shape 2 — extend the reply path to carry `a6` —
fixes something real beyond this test**: a receiver always learns who sent, a
caller learns nothing about who replied, and that asymmetry is in the IPC
contract rather than in this test.

Do not pick shape 1 because it is smallest, and do not pick shape 2 because it
is interesting. **Shape 2 is a kernel and ABI change and escalates before a line
of it is written.**

## 3. What you already have — verify it, then use it

I checked these; re-derive them:

- `SyscallNumber::TaskStatus = 42` is **dispatched**.
- `trap/syscall.rs:474` maps `TaskState::Blocked(_)` → `TaskLifecycle::Blocked`.
- `neg-test` **already imports and calls** `sys_task_status` — `main.rs:683`,
  `:719`.
- Its `TaskControl` cap at slot 6 has **`scope: ObjectScope::Any`**, so it needs
  **no new capability** to query any task.
- The reply path (`cap/syscall.rs:712-726`) writes `a0`, `a1`, `a2..a5` and
  **not `a6`** — which is why addressing is the gap and not the signal.

If any of that is wrong, **say so**. Every line this milestone has corrected at
least one thing I asserted, and those corrections have been worth more than the
assertions.

## 4. D2 — the collapse, checked

`TaskLifecycle::Blocked` is one value for every `TaskState::Blocked(_)`. Confirm
that between the reply and the revoke, `sample-service` cannot be blocked on
anything other than the `recv` this test is about. **Write the reasoning down
either way.** If it can, that is an erratum and a finding — not something to
work around with a longer poll.

## 5. Prohibited shortcuts

- Do not build an announcement protocol for blocked-ness.
- Do not poll unbounded — a hang is the failure mode this project diagnoses
  worst.
- Do not fall through to the revoke on poll exhaustion; emit a failure marker.
- Do not touch the kernel unless §4 chose shape 2 **and** you escalated first.
- Do not widen `TaskLifecycle` — that is a finding, not this line's work.
- Do not report a green profile as proof. It was green before you started.
- Do not claim host coverage — E-013; cite a committed log.
- Do not run `cargo fmt --all --check` in your head. **It was missing from your
  RFC-0.28-001 evidence list and it was failing.**

## 6. Required evidence

1. **§4 answered in writing**, with the two rejected shapes and why.
2. **What makes the test pass today**, identified and written down.
3. **The inverse demonstration**: the test failing with the rendezvous removed,
   captured — this is the point of the line.
4. The bounded poll, with its exhaustion marker.
5. D2's reasoning about the `Blocked` collapse.
6. A promoted evidence log with provenance (RFC-0.27-004).
7. **E-019 → `CLOSED`**, and its **tracking field retracked** from
   `RFC-0.26-003` to this RFC; **RFC-0.26-003** moved to `rfcs/archive/` as
   superseded, with its premise correction intact — do not delete it, the false
   reasoning is part of the record.

   **Do this deliberately, not by watching the gate.** E-019's tracking was
   left pointing at `RFC-0.26-003` while this RFC was `proposed/`, because two
   live RFCs cannot both claim one erratum and `errata-tracking` correctly
   refuses it. It passes right now only because the subcheck's "claims to
   close" predicate is a literal match that does not recognise this RFC's
   phrasing — the E-014 family, in the instrument that guards this very field.
   So the green result is not confirmation; retrack it because it is right.
8. `release-rehearsal` green; `test-all` **21/21**; `syscall-surface`
   **35/29/6** unless §4 chose shape 2, in which case it moves and that is an
   escalation you already had approved.
9. `cargo fmt --all --check` — run it, and put it in the evidence list.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The inverse demonstration.** I will read this hardest. A test that passes
  after the change and cannot be shown to fail without it has not been fixed.
- What was making it pass before, and whether that is a finding of its own.
- Your §4 answer, and specifically whether you considered shape 2 on its merits
  rather than dismissing it for size.
- Anything I asserted in §3 that turned out to be wrong.
