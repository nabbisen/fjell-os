# Developer Handoff — RFC-0.26-003

**Governing RFC:** [RFC-0.26-003](../../proposed/RFC-0.26-003-ipc-blocked-recv-rendezvous.md)
**Milestone:** 0.26
**Status:** inherited from the governing RFC (Proposed — awaiting owner acceptance)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. There is no known fix. That is the point.

Its sibling RFC-0.26-002 has an easy answer: a `READY` signal exists and is
simply not used. **This one has no signal, and none can be trivially built.**

`neg-test` needs `sample-service` to be **blocked in `sys_ipc_recv`** before it
revokes — that is what the test verifies. But a task cannot atomically announce
"I am about to block" and then block. Anything sent beforehand can be observed
before the block happens, so the announcement moves the race rather than
removing it.

**The deliverable is a written answer to that question**, committed, before
anything is built. RFC-0.26-001 established that order for the same reason: an
implementation that goes green without settling the question is a timing
assumption wearing a fix's clothing.

## 0.1 Three shapes, none obviously right

1. **Restructure so the test does not need the observation.** Note the trap:
   distinguishing "woke correctly" from "never blocked" is the same observation
   problem in different clothes. If you choose this, show how it is not.
2. **Make it observable from the kernel** — a task-state query, or a
   well-defined revoke against a not-yet-blocked receiver. This is an **ABI
   addition**. **Escalate before writing it** (RFC D4): it interacts with the
   standing disposition of the nine declared-but-undispatched syscalls, and
   adding a tenth to fix a test is a poor trade unless the primitive is
   independently warranted.
3. **Conclude it is not testable from userspace** and cover the kernel
   behaviour another way — which means a kernel unit test, which **E-013 says
   cannot run**.

**If you land on 3, say so loudly.** It is a legitimate outcome, and it makes
E-013 materially more expensive than four releases of disclosure have suggested.
Do not quietly work around it.

## 0.2 Design decisions settled — do not re-open

1. **Answer before implementing.**
2. **A defensive yield is not a fix.** No yield counts, retries, sleeps, or
   raised bounds. Step 3 of `test_ipc_blocked_recv` already does exactly that,
   and it is what broke.
3. **Do not soften RFC-0.26-001.**
4. **E-020 / RFC-0.26-002 is not yours.**

---

## 1. Why this matters more than "a test is red"

**E-010.** These exact `NEG:IPC:*` markers false-passed from v0.1 through v0.19
by accidentally binding `LeaseId(0)`, a revoked lease, and failing instantly
rather than exercising the protocol. Every word-carrying IPC protocol was
broken underneath them and nobody noticed for nineteen releases.

A permanently red guard and a permanently green useless one detect the same
amount. This one is merely honest about it.

That is why E-019 stays `ACCEPTED` rather than blocking releases — precedent
(E-013) governs — but it is also why this line should not drift.

## 2. Required evidence

1. **The written answer**, committed to the tree: which shape, and why the
   other two were rejected.
2. `ipc` green — **or** a committed statement of why it cannot be, and what
   covers the kernel's blocked-recv revocation behaviour instead.
3. **E-019 dispositioned** — `CLOSED` if fixed; re-argued with evidence if not.
   `ERRATA.md` and `v1-limitations.md` in the same commit either way.
4. `cargo xtask test-all` — 21/21 if it went green; otherwise the remaining
   failure named and justified.
5. `cargo xtask release-rehearsal`; `syscall-surface` still **35/29/6** unless
   D4 was escalated *and accepted*, which would change it deliberately.
6. `cargo fmt --all --check` — run it.

## 3. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The answer itself**, and which parts are observation versus inference.
- Any point where you were tempted by a yield or a bound.
- If you reached option 2 or 3, that alone — both change something larger than
  this RFC.
