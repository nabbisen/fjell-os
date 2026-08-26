# RFC-0.26-003: The blocked-recv test needs a rendezvous it cannot currently have

**Status:** Accepted — by the owner (nabbisen), 2026-08-27; implementation may begin (RFC 000)
**Milestone:** 0.26
**Tracks.** The `ipc` negative profile's dependency on an unobservable task state.
**Touches.** `crates/services/fjell-neg-test`, `crates/services/fjell-sample-service`.
Possibly the kernel — **see the open question**, which is the point of this RFC.
**Relates to:** ERRATA **E-019** (this RFC closes it), **E-010** (why IPC
negative coverage going dark is not a small thing), RFC-0.26-001 (which removed
the accident it relied on), RFC-0.26-002 (the sibling with an easier answer).

## Summary

`tests/qemu/profiles/ipc.toml` is red and covers nothing.

`fjell-neg-test::test_ipc_blocked_recv` needs `sample-service` to be **blocked
in `sys_ipc_recv`** before it revokes the lease — that is the entire point of
the test, which verifies the kernel wakes a blocked receiver on revocation. Its
step 3 says so:

> *"By the cooperative-scheduling contract, sample-service immediately calls
> `sys_ipc_recv(SLOT_LEASED_EP)` and blocks before the scheduler returns to
> neg-test. One defensive yield is included for safety."*

RFC-0.26-001 removed the priority asymmetry that made the "contract" true.

**Unlike its sibling RFC-0.26-002, there is no signal to wait on, and none can
be trivially built.** A task cannot atomically announce "I am about to block"
and then block — anything it sends before blocking can be observed before it has
blocked. That is not an implementation gap; it is the shape of the problem.

**This RFC exists to decide what to do about that, not to apply a known fix.**

## Motivation

### Why this is not RFC-0.26-002 with different filenames

| | Waits for | Signal available? |
|---|---|---|
| RFC-0.26-002 | peers **ready** | **Yes** — `send_ready()` already exists on both peers |
| **This RFC** | a peer **blocked in `recv`** | **No** |

Readiness is a state you can announce before entering. Blocked-ness is not.
`sample-service` sending "I am about to block" and then being preempted before
it blocks leaves `neg-test` revoking against a task that is still runnable — the
same race, moved.

### Why letting it stay dark is worse than it looks

**E-010 is on the record.** The `NEG:IPC:*` markers *false-passed for an entire
release line* — v0.1 through v0.19 — by accidentally binding `LeaseId(0)`, a
previously revoked lease, and failing instantly rather than exercising the
protocol. Every word-carrying IPC protocol was silently broken underneath them.

So this coverage has already been useless once, for nineteen releases, without
anyone noticing. It is now useless again, loudly this time.

**A permanently red guard and a permanently green useless one detect exactly the
same amount.** The difference is only that this one is honest about it.

## Why E-019 stays ACCEPTED while its sibling E-020 is OPEN

Asked and re-examined, because the two look alike and the answer decides whether
releases are blocked.

The register's axis is not severity — it is **are we shipping with this**.
E-020 is a shipped feature that no longer executes; we are not shipping with it,
so it is OPEN and Gate 7 is red.

E-019 is **lost test coverage**, and the governing precedent is **E-013**:
"the gate tools' own tests run under no mechanism", ACCEPTED, tracked, and
carried across four releases. Lost coverage has consistently been ACCEPTED in
this register when it is disclosed and has a tracking line. Reclassifying E-019
would make E-013 look misclassified by the same argument, and consistency in the
register is worth more than one instinct about this case.

**The counter-argument, recorded because it is not weak:** E-010 says these
exact `NEG:IPC:*` markers false-passed for nineteen releases without anyone
noticing. IPC negative coverage has already been silently worthless once. That
is a reason to hold this RFC to its schedule — **not** a reason to block every
release behind a question that may not have a cheap answer.

If RFC-0.26-003 concludes the behaviour cannot be covered from userspace at all
(option 3), E-019's classification is **re-argued with that evidence**, and
E-013 becomes materially more expensive than currently disclosed.

## The open question — the deliverable

**How should a test observe that another task has blocked?**

Three shapes, none obviously right:

1. **Restructure the test so it does not need to.** Revoke unconditionally and
   assert on the marker `sample-service` already emits when it wakes. If the
   revoke lands before the block, the marker never appears — so the test would
   need to distinguish "woke correctly" from "never blocked", which is the same
   observation problem wearing a different hat.
2. **Have the kernel make it observable.** A task-state query, or a revoke that
   is well-defined against a not-yet-blocked receiver. This is an **ABI
   addition** and would need its own justification — the project has nine
   declared-but-undispatched syscalls already and adding a tenth to fix a test
   is a poor trade unless the primitive is independently warranted.
3. **Accept that this specific kernel behaviour is not testable from userspace**
   and cover it another way — a kernel unit test, which **E-013 says cannot run**
   (`fjell-kernel` has no `[lib]`), which is its own finding.

**Option 3's dead end is worth noticing:** the honest answer may be that this
needs a kernel-side test, and the reason we cannot write one is a limitation
this project has carried and disclosed for four releases. If the investigation
lands there, that is a real result and E-013 becomes materially more expensive
than "some tests do not run."

## Design decisions

### D1 — Decide before implementing

Same rule as RFC-0.26-001, and for the same reason: the deliverable is **a
written answer to the open question**, committed, before any of the three shapes
is built.

An implementation that makes the profile green without settling which shape it
is will be a timing assumption wearing a fix's clothing.

### D2 — A "defensive yield" is not a fix

Any answer of the form *yield more*, *retry N times*, *sleep*, or *raise a
bound* is rejected in advance. That is what step 3 already does, and it is what
broke.

### D3 — Do not soften RFC-0.26-001

The priority fix is correct. The assumption is the defect. Restoring the
asymmetry to make the test pass would trade a known bug for a hidden one.

### D4 — Escalate before adding a syscall

If the investigation concludes option 2, **stop and escalate.** An ABI addition
is a design decision above this RFC, and it interacts with the standing
disposition of the nine undispatched syscalls.

## Scope

| # | Requirement |
|---|---|
| **R1** | Answer the open question in writing, committed — which shape, and why the other two were rejected |
| **R2** | Implement the chosen shape, or escalate if it is option 2 (D4) |
| **R3** | `ipc` profile green, **or** a reasoned statement that it cannot be and what replaces it |
| **R4** | **E-019 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the same commit — or its classification revised with evidence |

### Non-goals

- **E-020 / RFC-0.26-002.** Separate line, separate RFC.
- A general service-synchronisation framework.
- E-013's fix — but if option 3 makes it a prerequisite, **say so loudly**.
- Gate 12 `syscall-surface` must stay **35/29/6** unless D4 is escalated and
  accepted, which would change it deliberately.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | A timing assumption ships as a fix | **High** | **High** | D2. The review asks what the test *waits on*; "reliably enough" is not an answer |
| R2 | The answer is "add a syscall" and it lands without a design decision | Medium | **High** | D4 — escalate |
| R3 | The answer is "needs a kernel test" and E-013 blocks it | Medium | High | That is a legitimate outcome. Report it; do not work around it quietly |
| R4 | This line outgrows a test fix and becomes an IPC-semantics RFC | Medium | Medium | Then say so and stop. Discovering the question is larger than scoped is a result |

## Acceptance criteria

- [ ] **The open question answered in writing and committed**, with the rejected
      shapes and why.
- [ ] No yield-count, retry-count, sleep, or raised bound anywhere in the fix.
- [ ] `ipc` green — **or** a committed statement of why it cannot be, and what
      covers the kernel's blocked-recv revocation behaviour instead.
- [ ] **E-019 dispositioned**: `CLOSED` if fixed; re-argued with evidence if not.
- [ ] `cargo xtask test-all` — 21/21 if R3 succeeded; otherwise the exact
      remaining failure named and justified.
- [ ] `cargo fmt --all --check` clean — run, not predicted.

## A note on why this is its own RFC

It would have been tidier to fold this into RFC-0.26-002. Both were caused by
the same scheduler fix, both are assertions about ordering, and both profiles
went red in the same run.

But one has a signal waiting to be used and the other has no signal at all, and
bundling them would put a bounded fix behind an open question. **The tidier
scoping would have been the slower one.**
