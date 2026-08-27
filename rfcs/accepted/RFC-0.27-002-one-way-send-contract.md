# RFC-0.27-002: `try_send` does not try — the one-way send contract

**Status:** Accepted — by the owner (nabbisen), 2026-08-28; implementation may begin (RFC 000)
**Milestone:** 0.27
**Tracks.** The userspace contract for one-way IPC send, and whether a genuinely
non-blocking send should exist.
**Touches.** `crates/fjell-syscall`, `tests/abi/snapshot.json`, and any caller
the rename reaches. Possibly `crates/fjell-kernel` and `fjell-abi` — **only if
the open question in §4 is answered "yes"**, which is an ABI addition and
escalates.
**Relates to:** closes **E-022**; RFC-0.26-004 (which uncovered it);
**E-011** (the same shape: a doc-comment describing authority that does not
execute).

## Summary

`sys_ipc_try_send` is named as though it tries, documented as though it queues
and returns, and **blocks the caller**.

The kernel is not at fault. `SendResult`'s own definition says so:

```rust
/// Message delivered directly to a waiting receiver.
Delivered { receiver_tid: u16 },
/// No receiver; sender enqueued — must block.
Queued,
```

**"must block."** It is symmetric with `RecvResult::Queued` (*"No sender;
receiver enqueued — must block"*), and `cap/syscall.rs:605` wakes the sender the
moment a receiver takes the message, under the comment *"One-way send: sender
can proceed immediately."*

This is coherent **rendezvous IPC**, implemented deliberately and consistently.
`sendq` and `recvq` are waiter queues, not message buffers.

**The divergence is entirely in the userspace wrapper**, which describes a
buffering, fire-and-forget primitive that this kernel does not have.

## Motivation

### What the wrapper claims

`crates/fjell-syscall/src/lib.rs:278`:

> *"One-way IPC send (no reply expected). If no receiver is waiting the message
> is queued. Returns `WouldBlock` if the endpoint sendq is full."*

Three claims, three problems:

| Claim | Reality |
|---|---|
| named `try_send` | it does not try; it blocks |
| *"the message is queued"* | the **sender** is queued, and suspended |
| *"`WouldBlock` if the sendq is full"* | that is the **waiter** queue overflowing — too many blocked senders, not a full buffer |

### What it cost

RFC-0.26-004 established the invariant *a service's endpoint has exactly one
receiver*, and removing `init` as an accidental co-receiver removed the cover
this defect had been hiding under. `semantic-stream` and `proxy-text` each
announced readiness into **their own** endpoint — under rendezvous semantics, a
task waiting for itself. Previously `init` happened to be waiting, so the send
found a receiver and returned `Delivered`. Once it did not, the send returned
`Queued` and the sender blocked forever.

The workaround was to delete the two call sites. **The trap remains laid for any
future one-way send**, and it is not discoverable from the wrapper's name or its
documentation — which is what makes it worth a line rather than a comment.

### Why this is E-011's family

E-011 records `sys_cap_install`'s doc-comment claiming a rights check that never
executes. This is the same shape at a different layer: **the wrapper's
documentation describes a primitive the kernel does not implement**, and a
caller who reads the doc and believes it writes a deadlock.

## The settled part

**The kernel is correct and is not to be changed to match the wrapper.**
Rendezvous send is a coherent design, implemented symmetrically, with a
deliberate wake path and its own type documentation stating the contract.
Making `Queued` return without blocking would turn the waiter queue into an
unbounded message buffer and change lease-revocation semantics
(`wake_or_cancel_blocked_ipc_for_lease` exists precisely because senders block).

**R1 — the wrapper is corrected to describe what the kernel does.** Rename to
`sys_ipc_send`; rewrite the doc-comment to state that the call blocks until a
receiver takes the message, and that `WouldBlock` means the *waiter* queue is
full.

## The open question — §4, and the actual deliverable

**Should a genuinely non-blocking one-way send exist?**

Not rhetorical. Two services announcing readiness into their own endpoints is a
reasonable thing to want to write, and under rendezvous it is unwritable. The
answer shapes whether this line ends at a rename.

Three shapes, none pre-selected:

1. **No.** Rendezvous is the only send this kernel offers; announcing into your
   own endpoint is a design error and the corrected name makes it obvious.
   Cheapest, and defensible — the readiness use case was itself replaced by
   RFC-0.26-004's invariant.
2. **Yes, as a flag or a new syscall** returning `WouldBlock` instead of
   blocking when no receiver waits. This is an **ABI addition** — Gate 12's
   `syscall-surface` moves — and it interacts with the standing disposition of
   the six declared-but-undispatched syscalls. **Escalate before writing it.**
3. **Yes, but out of this line.** Record the requirement, close E-022 on the
   rename, and let a separate RFC decide the primitive.

**Answer it in writing before implementing**, per the pattern of RFC-0.26-001
and RFC-0.26-003.

## Scope

| # | Requirement |
|---|---|
| **R1** | Rename `sys_ipc_try_send` → `sys_ipc_send`; doc-comment states blocking rendezvous and what `WouldBlock` actually means |
| **R2** | Every caller updated; **no caller left announcing into an endpoint only it receives on** |
| **R3** | Answer §4 in writing, committed; escalate before any ABI addition |
| **R4** | ABI snapshot reconciled — see D2 |
| **R5** | **E-022 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the same commit |

### Non-goals

- **Changing the kernel's send semantics.** Settled above.
- E-019 / RFC-0.26-003, and the four `unscheduled` errata.
- Reworking RFC 058's readiness protocol.
- `syscall-surface` stays **35/29/6** unless §4 answers (2) *and* that is
  escalated and accepted.

## Design decisions

### D1 — The kernel is right; do not "fix" it

Stated because it is the tempting shortcut: making `Queued` non-blocking would
make the old call sites work and silently convert a rendezvous primitive into a
buffered one, changing revocation semantics nobody asked to change.

### D2 — The rename is an ABI-snapshot event, and must be reconciled

`fjell-syscall` is in `STABLE_CRATES`. Renaming a public function is a
**removal plus an addition**, and Gate 4 will report it as breaking — correctly.

**Regenerate only after showing the diff is exactly that:** one item removed,
one added, nothing else. The RFC-0.24-003 discipline applies in full — a
regeneration that absorbs an unexplained change is how a real break disappears.

`fjell-abi` is published to crates.io, but `SyscallNumber::IpcSend` is
unchanged, so the published surface does not move. `fjell-syscall` is not
published.

### D3 — Answer before implementing

§4 is a design decision, not a coding one.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The kernel is changed to match the wrapper | Medium | **Critical** | D1. It is a non-goal and the reason is stated |
| R2 | The ABI regeneration absorbs more than the rename | Medium | **High** | D2 — show the diff is one removal and one addition before regenerating |
| R3 | §4 is answered implicitly by adding a syscall | Medium | High | **Escalate before writing it.** Six declared-but-undispatched syscalls already await disposition |
| R4 | A caller is left announcing into its own endpoint | Medium | Medium | R2 — audit the call sites, do not assume RFC-0.26-004 removed them all |
| R5 | Not host-testable (E-013) | **Certain** | Medium | QEMU evidence, cited by log |

## Acceptance criteria

- [ ] **§4 answered in writing and committed**, with the rejected shapes.
- [ ] `sys_ipc_try_send` gone; the doc-comment describes blocking rendezvous and
      the real meaning of `WouldBlock`.
- [ ] **A demonstration that the corrected contract holds** — a send with no
      receiver blocks, and is woken when one arrives. QEMU, cited by log.
- [ ] No caller announces into an endpoint only it receives on; the audit is
      shown, not asserted.
- [ ] **ABI diff shown to be exactly one removal and one addition** before the
      baseline is regenerated.
- [ ] **E-022 `CLOSED`**, both documents in the same commit.
- [ ] `release-rehearsal` green; `test-all` 21/21; `syscall-surface` **35/29/6**.
- [ ] `cargo fmt --all --check` clean — run, not predicted.

## What this is really about

A name and a doc-comment described a primitive the kernel never offered, and the
only reason nobody deadlocked for nineteen releases is that an unrelated bug
kept a receiver accidentally waiting.

The kernel has been right the whole time. What was wrong was the sentence
describing it — which is the fifth instance this project has found of a label
narrower or wider than the thing it names, and the first in the syscall surface
itself.
