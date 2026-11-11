# Developer Handoff — RFC-0.42-002

**Governing RFC:** [RFC-0.42-002](../../done/RFC-0.42-002-user-space-can-read-a-clock.md)
**Milestone:** 0.42 · independent of 0.42-001 (done) and 0.42-003 (done)
**Status:** inherited from the governing RFC (Implemented, 0.42.0)
**Audience:** implementation model

---

## 1. Read the RFC first

R1–R6 and the non-goals are there. **The one thing to hold onto:** the kernel already has
`read_mtime`, `set_mtimecmp`, `schedule_next_tick`, `handle_timer` and a preemption flag, all
written and all dormant because **`enable_interrupts()` — the only function that sets `STIE` — has
zero callers**. You are exposing a readable counter, not building a clock.

## 2. The line in the sand

**`STIE` stays masked.** Do not call `enable_interrupts()`, do not set bit 5 of `sie`, do not wire
`schedule_next_tick`. `csr.rs:150` records why: turning it on *"would silently activate RFC 037's
dormant timer-preemption path kernel-wide, on every existing QEMU profile."* Reading `mtime` does
not need it — the interrupt enable is about delivery, not observation.

**If you find you need the interrupt to read the counter, stop and escalate.** That would mean the
RFC's premise is wrong, which is worth more to me than an implementation.

## 3. Two decisions that are yours to argue, not mine to dictate

- **R2's unit.** Nanoseconds, or ticks plus an exposed frequency. Pick one and say why. A counter
  with an undocumented unit recreates E-106 at one remove — a number that reads like a duration and
  is not one.
- **R4's grant set.** Which services get the right, and why each needs it. Grant it **explicitly**,
  not through `ALL_NON_META` or `ALL_DEFINED` — the 0.42-001 precedent, and for the same reason: a
  blanket grant should not silently confer a new authority.

## 4. Mechanics and traps

- **Syscall number 113** (next free). **Not 120** — retired `Reboot`, and reviving it would make old
  and new ABIs disagree silently.
- **The ABI snapshot will move.** `fjell-abi`, `fjell-cap` and `fjell-syscall` are all in its crate
  list. Regenerate in this line's own commit and **say you predicted it**.
- **`storaged`'s prebuilt and the baseline move** (R5). `build-services` then `build`, then
  `rm tests/repro/baseline-digests.txt` and
  `cargo run -p fjell-repro-check -- --skip-build --record-baseline`, same commit, every moved line
  accounted for.
- **Do not regenerate `releases/trust-report.txt`.**
- One `test-all` at a time. `/usr/bin/grep -a` in a `.bin`. Stage by explicit path and check with
  `git diff --cached --name-only`. The shell starts in the repository.

## 5. Evidence the review request must carry

1. **R6's line from a real boot**, quoted: the `io incomplete` report showing **both** the round
   count and a duration.
2. **Proof the give-up condition did not change** — the budget is still 500,000 rounds and the
   line still fires at the same point. The duration is an added field.
3. **`sie` is unchanged**: show that nothing sets bit 5, and that `enable_interrupts()` still has no
   callers.
4. **The ABI diff**, with the movement stated as predicted.
5. **The baseline `--numstat`** with every moved line named.
6. `build-services && build` with **0 warning lines**; full `test-all` with tier counts;
   `consistency-check --all` (21 subchecks); `fmt`, mmio-audit, unsafe-audit, `abi-snapshot --verify`.

**If a tier goes red, report which and do not re-run to green.** `storaged: io incomplete` is
**E-104** — closed on a cause, disclosed as limitations row 19, a known host-latency intermittent.

## 6. Stop and escalate if

- **Reading the counter appears to need `STIE`** (§2).
- **The counter reads zero or does not advance.** That is a platform finding, not a reason to reach
  for the interrupt.
- **Any tier's timing shifts** once the clock is readable. Nothing should change, since nothing new
  is delivered — if something does, it is the most interesting result available.

## 7. Not yours

`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`, `standards-mapping.md`, `do_io`'s budget
and give-up condition, the `io incomplete` line's existing fields, RFC 037's preemption path, and
any tag. **Criterion 8 is not yours**: if a row looks false, say so in the review request.
