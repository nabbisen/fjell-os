# RFC-0.26-001: The scheduler priority defect — and what depends on it

**Status:** Proposed — awaiting owner acceptance
**Milestone:** 0.26
**Tracks.** `task::scheduler` priority assignment; the boot-ordering dependency
that a correct fix exposes.
**Touches.** `crates/fjell-kernel/src/task/{scheduler,spawn}.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`. No ABI, capability, lease, IPC, or
crypto change.
**Relates to:** ERRATA **E-018** (this RFC closes it), RFC-0.25-001 (which
found it and shipped the stopgap), RFC 037 (the cooperative loop this ordering
sits under).

## Summary

`PRIORITY_USER` exists three times with two values, and the scheduler's bucket
arithmetic turns that into a real preemption asymmetry:

| Site | Value | Bucket |
|---|---|---|
| `task/scheduler.rs:17` — the real constant | **32** | **1** |
| `task/spawn.rs:26` — a *local* `const` shadowing it | **2** | **0** |
| `trap/syscall.rs:431` — a hardcoded literal | **2** | **0** |

`priority_to_bucket(p) = p * 8 / 256`, and `dequeue_next` selects the **highest
non-empty bucket** (`7 - leading_zeros`). `init` is constructed by a
hand-rolled path in `main.rs` at the real `32`; every service spawned through
`spawn()` gets `2`.

**So `init` preempts every other spawned task whenever both are ready.**

**This RFC's subject is not the constant.** Correcting it was attempted during
RFC-0.25-001 and **hung the M6 boot sequence**. Something already shipped
depends on the current ordering. Finding out what — that is the line.

## Motivation

### Why it stayed invisible

Every `init` code path that waits on a service uses a **blocking**
`sys_ipc_recv` — `wait_service_ready`, `wait_storaged_ready`,
`wait_ready_exact`. A blocking receive removes `init` from the ready queue
entirely, so it is not contending, so the asymmetry never expresses itself.

Nothing in this kernel had ever yield-looped from `init` while another service
was still starting.

RFC-0.25-001 needed exactly that: a **non-blocking** poll for the uart-rx byte,
because a blocking wait would hang every QEMU profile in which nobody types
anything. That poll starved `fjell-driver-uart` completely — `init`'s poll
budget exhausted and it moved on **before `driver-uart` ran a single
instruction.**

This is the same shape as `ci-proptest` running zero tests: a defect that is
latent rather than absent, invisible because every existing caller happens to
take a path that avoids it, and surfaced the first time something new does not.

### Why the obvious fix is not the deliverable

Pointing `spawn.rs` at the real constant is a two-line change. It was tried. It
**hung M6 boot.**

That result is the finding. A boot sequence that depends on `init` preempting
its own services is either relying on an ordering nobody designed, or masking a
missing synchronisation somewhere — and both possibilities are worth more than
the constant is.

**An RFC that "unifies the constant" and reports green would be the same
mistake in a different place:** the hang would be gone from the tree and its
cause would still be unknown.

## Design decisions

### D1 — This is an investigation first, a patch second

The deliverable is **an explanation of the M6 hang**, in writing, before any
unification lands. Which task, waiting on what, that today only completes
because `init` runs ahead of it.

If the explanation turns out to be "a real missing synchronisation", that
becomes its own fix and this RFC narrows to it. If it turns out to be
"accidental ordering nothing depends on semantically", unification lands with
the evidence recorded.

**Do not unify first and investigate the fallout.** That inverts the risk.

### D2 — Whatever the outcome, one value survives

There is no defensible end state with two `PRIORITY_USER`s. Either `init` is
genuinely privileged — in which case say so with a distinct, named constant
(`PRIORITY_INIT`) and a stated reason — or it is not, and everything shares one.

**A local `const` shadowing a public one with a different value is not a
priority policy; it is a bug that reads as a policy.**

### D3 — The stopgap comes out

RFC-0.25-001's `image_id`-keyed special case for `driver-uart` in
`sys_task_start` is explicitly temporary and documented as such at the site. It
is removed by this RFC. If it cannot be removed, the investigation is not
finished.

### D4 — Three buckets of headroom is a separate question, and stays separate

`MAX_PRIORITY_LEVELS = 8` over a `u8`, so `p * 8 / 256` gives every priority
0–31 bucket 0 and 32–63 bucket 1. The effective priority space is **three
bits**, and the two values in play straddle a boundary almost by coincidence.

Whether eight buckets is the right granularity is a real design question and is
**not** in this RFC. Note it; do not act on it.

## Scope

| # | Requirement |
|---|---|
| **R1** | Reproduce the M6 hang deliberately: point `spawn.rs` at `scheduler::PRIORITY_USER`, capture the hang, identify the exact task and wait it hangs on |
| **R2** | Explain it in writing — what completes today only because `init` runs first |
| **R3** | Fix what R2 finds. If a missing synchronisation, fix that; if accidental ordering, record the evidence |
| **R4** | One value survives — unify, or introduce a named `PRIORITY_INIT` with a stated reason (D2) |
| **R5** | Remove the `driver-uart` stopgap in `sys_task_start` and the local `const` in `spawn.rs` |
| **R6** | Close **E-018**: `CLOSED` in `ERRATA.md`, and the paired entry removed from `v1-limitations.md` **in the same edit** |

### Non-goals

- **Changing `MAX_PRIORITY_LEVELS` or the bucket function** (D4).
- Preemption policy, time slicing, or RFC 037's quantum path. The dormant
  `enable_interrupts()` noted in RFC-0.25-001 §5 stays dormant.
- Any ABI, capability, lease, IPC, or crypto behaviour.
- Gate 12 `syscall-surface` must stay **35/29/6**.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The hang is fixed by an unrelated-looking change and the cause is never established | **High** | **High** | D1 — the written explanation is the deliverable and gates the patch. A green M6 with no explanation does not satisfy R2 |
| R2 | The dependency is real and larger than this RFC (e.g. a service-manager readiness gap) | Medium | High | Then narrow to it and say so. Discovering the line is bigger than scoped is a result, not a failure — **escalate** |
| R3 | Unification changes timing across all 21 tiers, and a *different* latent defect surfaces | Medium | Medium | Expect it. A newly-visible defect is not a regression this RFC caused; record it separately rather than folding it in |
| R4 | **None of this is host-testable** — `fjell-kernel` has no `[lib]` (E-013) | **Certain** | High | Every claim rests on a QEMU log, as in RFC-0.25-001. Say where, not "tested" |
| R5 | The stopgap is left in "just in case" | Medium | Medium | D3 — its removal is the acceptance test for the investigation |

## Acceptance criteria

- [ ] **The M6 hang reproduced deliberately and explained in writing** — the
      task, the wait, and what completes today only because `init` runs ahead.
- [ ] The explanation is committed, not just described in a review request.
- [ ] Exactly one `PRIORITY_USER` value in the tree, or a distinct
      `PRIORITY_INIT` with a stated reason. **No local `const` shadowing a
      public one.**
- [ ] The `image_id` stopgap in `sys_task_start` **removed**, and `uart-rx`
      still passes without it.
- [ ] `cargo xtask test-all` — all 21 tiers, with any newly-surfaced defect
      recorded separately rather than absorbed.
- [ ] `cargo xtask release-rehearsal` green; `syscall-surface` still **35/29/6**.
- [ ] `cargo fmt --all --check` clean — **run, not predicted.**
- [ ] **E-018 moved to `CLOSED`**, with `ERRATA.md` and `v1-limitations.md`
      edited together.

## What this is really about

A constant was duplicated, the copies drifted, and the drift became load-bearing
without anyone deciding it should be. The system now boots *because of* a bug.

That is worth more attention than a two-line correction, and it is the reason
this is its own line rather than a cleanup folded into the next functional RFC.
