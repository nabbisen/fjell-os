# Developer Handoff — RFC-0.27-002

**Governing RFC:** [RFC-0.27-002](../../done/RFC-0.27-002-one-way-send-contract.md)
**Milestone:** 0.27
**Status:** inherited from the governing RFC (Implemented, 0.27.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The kernel is right. Do not fix it.

This is the shortcut, and it would work: make `SendResult::Queued` return
without blocking, and every old call site starts working again.

**It is a non-goal, for a reason worth reading before you are tempted.** The
kernel implements rendezvous IPC deliberately and symmetrically — `SendResult`'s
own definition says *"No receiver; sender enqueued — must block"*, `RecvResult`
says the mirror image, and `cap/syscall.rs:605` wakes the sender when a receiver
takes the message under an explicit comment. `sendq` and `recvq` are **waiter**
queues, not message buffers.

Making `Queued` non-blocking converts a waiter queue into an unbounded message
buffer and changes lease-revocation semantics —
`wake_or_cancel_blocked_ipc_for_lease` exists precisely because senders block.

**What is wrong is the sentence describing the kernel, not the kernel.**

## 0.1 What you are actually fixing

`crates/fjell-syscall/src/lib.rs:278`:

> *"One-way IPC send (no reply expected). If no receiver is waiting the message
> is queued. Returns `WouldBlock` if the endpoint sendq is full."*

Named `try_send`, so a reader expects it to try. It blocks. *"The message is
queued"* — the **sender** is queued and suspended. *"`WouldBlock` if the sendq
is full"* — that is the **waiter** queue overflowing, meaning too many blocked
senders, not a full buffer.

## 0.2 Design decisions settled — do not re-open

1. **The kernel keeps rendezvous semantics** (§0).
2. **Answer §4 before implementing anything beyond the rename.** Whether a
   genuinely non-blocking send should exist is a design decision.
3. **If the answer is "add a syscall or a flag" — stop and escalate.** That is
   an ABI addition, `syscall-surface` moves, and six declared-but-undispatched
   syscalls already await disposition. Do not write it and report it.
4. **Regenerating the ABI baseline requires showing the diff first** (§2).

---

## 1. §4 is the deliverable, not the rename

**Should a genuinely non-blocking one-way send exist?**

Two services announcing readiness into their own endpoints was a reasonable
thing to want to write, and under rendezvous it is unwritable. That use case was
removed by RFC-0.26-004's invariant — but the question of whether the primitive
should exist is not thereby answered, only deferred.

Three shapes in the RFC. Say which, and why the other two were rejected. *"No"*
is a perfectly good answer if argued.

## 2. The rename is an ABI event — the trap in this line

`fjell-syscall` is in `STABLE_CRATES`. Renaming a public function is a
**removal plus an addition**, and Gate 4 will report it as breaking. That is the
gate working.

**Show the diff before regenerating:** exactly one item removed
(`sys_ipc_try_send`), exactly one added (`sys_ipc_send`), nothing else. Then
regenerate.

RFC-0.24-003's discipline applies in full — a regeneration that absorbs an
unexplained change is how a real break disappears and the gate reports `PASS`
forever after. **The reconciliation is the evidence, not the green gate.**

`fjell-abi` is published to crates.io now, but `SyscallNumber::IpcSend` does not
change, so the published surface does not move. `fjell-syscall` is not published.

## 3. R2 — audit the callers, do not assume

RFC-0.26-004 deleted the two `send_ready()` call sites that were deadlocking.
**Do not assume that was all of them.** Find every caller and check none is
announcing into an endpoint only it receives on.

Show the audit. "I grepped and there were none" with the grep shown is
evidence; "the callers are fine" is not.

## 4. Prohibited shortcuts

- Do not make `SendResult::Queued` non-blocking.
- Do not add a syscall or a flag without escalating first.
- Do not regenerate the ABI baseline before showing the diff is one removal and
  one addition.
- Do not assume RFC-0.26-004 removed every problematic caller.
- Do not claim host coverage — `fjell-kernel` has no `[lib]` (E-013). Cite the
  QEMU log.
- Do not run `cargo fmt --all --check` in your head.

## 5. Required evidence

1. **§4 answered in writing, committed**, with the rejected shapes.
2. The rename, and a doc-comment that describes blocking rendezvous and what
   `WouldBlock` actually means.
3. **A demonstration the corrected contract holds** — a send with no receiver
   blocks; a receiver arriving wakes it. QEMU, cited by log.
4. The caller audit, shown.
5. **The ABI diff, shown to be one removal and one addition**, before
   regeneration.
6. **E-022 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the **same
   commit**.
7. `release-rehearsal` green; `test-all` 21/21; `syscall-surface` **35/29/6**.
8. `cargo fmt --all --check` — run it.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your answer to §4**, and which parts of it are observation versus judgement.
- The ABI diff, and anything in it you did not expect.
- Any caller you found that was announcing into its own endpoint — that is a
  second finding, not a tidy-up.
- Anything you found while reading that is not in this RFC. The last four lines
  each turned up at least one, and three of those were the reason the next line
  existed.
