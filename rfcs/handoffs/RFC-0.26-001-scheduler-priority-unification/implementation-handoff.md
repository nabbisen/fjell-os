# Developer Handoff — RFC-0.26-001

**Governing RFC:** [RFC-0.26-001](../../done/RFC-0.26-001-scheduler-priority-unification.md)
**Milestone:** 0.26
**Status:** inherited from the governing RFC (Implemented, 0.26.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The deliverable is an explanation, not a patch

You already know the two-line fix. You tried it during RFC-0.25-001 and **it
hung M6 boot**, and you reverted rather than chasing it — which was the right
call then and is the reason this line exists now.

**The M6 hang is the subject.** Something already shipped depends on `init`
preempting its own services. Until that is written down, unifying the constant
only moves the hang out of sight.

An RFC that lands green with the cause unknown is the same failure this project
has spent two milestones on, relocated: a check passing without the thing it
checks being understood.

## 0.1 Order — this one is not negotiable

**R1 → R2 → R3 → R4 → R5 → R6.**

**Reproduce and explain before you fix anything.** Point `spawn.rs` at
`scheduler::PRIORITY_USER`, capture the hang, and find out which task is
waiting on what. Only then decide what the fix is.

If you unify first and the hang disappears because of something you changed
along the way, the finding is gone and cannot be recovered — the same shape as
correcting the `asm-instruction` tag before capturing Gate 2's failure.

## 0.2 The mechanism, already established — do not re-derive it

| Site | Value | Bucket |
|---|---|---|
| `task/scheduler.rs:17` — the real `pub const` | 32 | **1** |
| `task/spawn.rs:26` — a local `const` shadowing it | 2 | **0** |
| `trap/syscall.rs:431` — a hardcoded literal | 2 | **0** |

`priority_to_bucket(p) = p * MAX_PRIORITY_LEVELS / 256` with
`MAX_PRIORITY_LEVELS = 8`, and `dequeue_next` takes the **highest** non-empty
bucket (`7 - non_empty_mask.leading_zeros()`).

`init` is built by a hand-rolled path in `main.rs` at 32. Everything spawned
through `spawn()` gets 2. **Bucket 1 always drains before bucket 0.**

Why it was invisible: every prior `init` wait (`wait_service_ready`,
`wait_storaged_ready`, `wait_ready_exact`) is a **blocking** recv, which takes
`init` out of the ready queue so it never contends.

## 0.3 Design decisions settled — do not re-open

1. **Investigation before patch** (§0.1).
2. **One value survives.** Either everything shares one, or `init` is genuinely
   privileged and gets a **named `PRIORITY_INIT`** with a written reason. A
   local `const` shadowing a public one with a different value is not a policy.
3. **The stopgap comes out.** RFC-0.25-001's `image_id`-keyed special case in
   `sys_task_start` is temporary and says so at the site. **Its removal is the
   acceptance test for the investigation** — if `uart-rx` cannot pass without
   it, you have not finished.
4. **`MAX_PRIORITY_LEVELS` is not in scope.** Eight buckets over a `u8` gives a
   three-bit effective priority space, and the two values in play straddle a
   boundary nearly by accident. Note it. Do not act on it.

---

## 1. What "explained" means for R2

Not "M6 hangs because of priorities." Specifically:

- **Which task** fails to make progress.
- **What it is waiting on** — a marker, a READY message, a lease, a reply.
- **Why that wait completes today** — what `init` does, while running ahead,
  that satisfies it.

If the answer is *"service X is never signalled ready, and today `init` reaches
the signalling code before X ever needs it"*, that is a **missing
synchronisation** and the fix is that, not the constant.

If the answer is *"ordering only, nothing semantic depends on it"*, say so and
show the evidence.

Commit the explanation as a document. A review request paragraph is not enough
— the next person to touch the scheduler needs it in the tree.

## 2. Expect collateral, and do not absorb it

Unifying priorities changes timing across all 21 tiers. **A different latent
defect may surface.** That is not a regression this line caused — it is another
thing that was invisible for the same reason.

Record it separately. Do not fold it into this RFC to keep the tier count
green, and do not chase it. Same rule as `driver-virtio-net` in RFC-0.25-001.

## 3. Prohibited shortcuts

- Do not unify the constant before the hang is explained and committed.
- Do not leave the stopgap in "just in case".
- Do not touch `MAX_PRIORITY_LEVELS` or the bucket function.
- Do not enable timer interrupts or touch RFC 037's dormant quantum path — the
  `enable_interrupts()` left alone in RFC-0.25-001 stays alone.
- Do not claim host test coverage. `fjell-kernel` has no `[lib]` (E-013); cite
  the QEMU log for every claim.
- Do not run `cargo fmt --all --check` in your head.

## 4. Required evidence

1. **The M6 hang, reproduced deliberately** — QEMU log, and the task and wait
   identified.
2. **The written explanation**, committed to the tree.
3. Exactly one `PRIORITY_USER` value, or a named `PRIORITY_INIT` with its
   reason. No shadowing local `const`.
4. **The stopgap removed**, and `uart-rx` passing without it.
5. `cargo xtask test-all` — 21 tiers, with anything newly surfaced recorded
   separately.
6. `cargo xtask release-rehearsal` green; `syscall-surface` **35/29/6**.
7. `cargo fmt --all --check` — run it.
8. **E-018 → `CLOSED`**, with `ERRATA.md` and `v1-limitations.md` edited in the
   same commit. Splitting them is what produced the divergence the 0.24 audit
   found, and `errata-limitations` matches only the ID, so it would not catch a
   second one either.

## 5. Review request

Standard format, in `.git-exclude/review-request/`. One request.

Flag for focused review:

- **The explanation itself.** That is what I will read hardest, and the part
  where a plausible-but-unverified mechanism would be easiest to write. If any
  step of it is inference rather than observation, say which.
- Anything that changed behaviour in a tier you did not expect to touch.
- Whether removing the stopgap needed anything beyond the unification — if it
  did, that is a second finding.
