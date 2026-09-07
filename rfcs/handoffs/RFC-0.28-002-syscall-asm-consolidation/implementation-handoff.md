# Developer Handoff — RFC-0.28-002

**Governing RFC:** [RFC-0.28-002](../../accepted/RFC-0.28-002-syscall-asm-consolidation.md)
**Milestone:** 0.28
**Status:** inherited from the governing RFC (Accepted, 2026-09-07)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. You already know how this one fails

You spent RFC-0.28-001's bring-up on exactly this bug: a missing `a6`, a
permanent hang, six identical reproductions, and debug prints that made it go
away. **You are about to do the same edit thirty-five times, across every
service in the tree, and the failure mode is silence.**

Two things follow.

**Run the QEMU tiers more than once.** You needed 13 consecutive clean runs
before believing a two-block fix. This is thirty-five blocks in eleven crates.
A single green `test-all` is not evidence here, and saying so is not pedantry —
it is the specific lesson from the last line, applied.

**A hang after a swap is a finding about that site, not a reason to revert to
asm.** Register-level code can depend on what it does *not* clobber. If a site
misbehaves once it stops lying to the compiler, the site was relying on the lie.
Record it; do not restore the block.

## 0.1 Design decisions settled — do not re-open

1. **Delete the blocks; call the wrapper** (D1). Do not fix `a6`/`a0` in
   thirty-five places. The wrapper is a correct superset for the recv shape and
   every affected crate already depends on `fjell-syscall`.
2. **Audit each site** (D2). I verified one and read the wrapper signature.
   That is not thirty-five verifications. Where no wrapper fits, **escalate** —
   do not write a local variant, which is precisely how thirty-five happened.
3. **Add the guard** (D3), shape decided by §6.
4. **The `unsafe` reduction is a result, not a target** (D4). Do not delete a
   block to move the number, and do not headline the count.
5. **Evidence is a committed log** (D5), per RFC-0.27-004.

---

## 1. Order

**§6 answered → per-site audit → swaps, in small batches → guard → guard
demonstrated failing → evidence.**

Answer §6 first: whether the guard forbids raw syscall asm outright or enforces
clobbers changes what "done" means for a site the wrapper cannot serve.

**Swap in batches, not in one commit.** Eleven crates at once, with a hang as
the failure mode, gives you no bisection. One crate per step, tiers between.

## 2. §6 is a real question and I have no answer for you

Shape 1 (refuse raw syscall asm outside `fjell-syscall`) is strongest and
simplest. Shape 3 (refuse where a wrapper exists, enforce clobbers where it does
not) is the compromise with the most moving parts. **I am not stating an
inclination** — 1 and 3 both look defensible and I would rather read your
argument than have you check mine.

Say what happens under your chosen shape when someone needs a syscall that has
no wrapper. That is the case the rule has to survive.

## 3. The counts, to re-derive rather than trust

I claim: 37 raw syscall `asm!` blocks, 2 in `fjell-syscall`, **35** in services;
12 missing `a6` (all `IpcRecv`); **18** with `a0` as a plain `in` (`IpcRecv` and
`IpcReply`); five distinct syscalls, all five already wrapped.

**Re-derive all of it.** My regex looked for `core::arch::asm!` with `li a7, N`
inside a 900-character window — a block longer than that, or one that builds its
syscall number differently, is invisible to it. If the real numbers differ,
**report the difference**; every line this milestone has corrected one of my
counts and each correction was worth more than the count.

## 4. The guard must be demonstrated failing

RFC-v0.22-001. At minimum:

| # | Broken input | Must |
|---|---|---|
| 1 | a new raw syscall block added to a service | FAIL, naming file and line |
| 2 | (shape 2/3 only) a block missing `a6` on an `IpcRecv` | FAIL, naming the register |
| 3 | the tree as shipped by this RFC | **PASS** |

Demonstration 3 is not a formality: a guard that passes because it matches
nothing is the `ci-proptest` defect (RFC-0.24-002) — a job named for a check
that ran zero of them. **Show the guard sees the sites it is meant to see**, by
pointing it at the pre-change tree and getting 35 hits.

## 5. Prohibited shortcuts

- Do not annotate the thirty-five in place.
- Do not swap all eleven crates in one commit.
- Do not restore an `asm!` block because the wrapper changed behaviour.
- Do not add a wrapper without escalating first.
- Do not touch the kernel, `fjell-abi`, or `fjell-syscall`'s own two blocks.
- Do not report the `unsafe` reduction as the achievement.
- Do not claim host coverage — E-013; cite a committed log.
- Do not run `cargo fmt --all --check` in your head. **It was missing from your
  last submission's evidence list and it was failing.**

## 6. Required evidence

1. **§6 answered in writing**, with the rejected shapes.
2. The per-site audit — all sites, what each became, and any escalation.
3. The guard, with unit tests.
4. **All three demonstrations captured**, including the 35-hit pre-change run.
5. **Repeated QEMU runs**, not one — say how many and cite the logs.
6. A promoted evidence log with provenance (D5).
7. **E-032 widened to name bug B, then `CLOSED`** — register and
   `v1-limitations.md` in the same commit.
8. `release-rehearsal` green; `test-all` **21/21**; `syscall-surface`
   **35/29/6**, unchanged.
9. `cargo fmt --all --check` — run it, and put it in the evidence list.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Any site where the wrapper was not a clean swap**, and what you did.
- **Any behaviour that changed after a swap** — that is the most valuable output
  this line can produce.
- Your §6 answer, and what happens to an unwrapped syscall under it.
- Any count of mine you re-derived and found different.
- The number of QEMU runs you were willing to call sufficient, and why.
