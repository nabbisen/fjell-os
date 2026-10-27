# RFC-0.39-003: Find out why a live page reads as zeros

**Status:** Accepted (0.39) — accepted 2026-10-27
**Milestone:** 0.39
**Tracks.** **E-097** — the `semantic` tier fails intermittently because a live service's
text page reads back as 46 NUL bytes, cause unknown — and **E-098**, the `revoke_by_pa`
workaround that is E-097's strongest lead and a set of verified defects in its own right.
**Touches** *(indicative)*: `crates/fjell-kernel/src/main.rs`,
`crates/fjell-kernel/src/mm/frame_alloc.rs`, `crates/fjell-kernel/src/mm/`,
`tests/qemu/`, `rfcs/ERRATA.md`. **Possibly very little**: see §"This line may not
close E-097".
**Relates to:** **RFC-0.39-001**, which found it and is not its cause;
**RFC-0.39-002**, which is independent and proceeds in parallel; **E-014**, the
discarded-error family E-098's third defect belongs to.

## Why this is its own line, inside 0.39

E-097 is **OPEN**, so Gate 7 refuses and 0.39.0 cannot cut. Work that must precede a cut
cannot be numbered after it — the same reasoning that put the 0.38 kernel defects in 0.38
rather than 0.39. So this is **0.39-003**, not 0.40-anything, and the owner has held the
milestone for it.

It is a separate line from RFC-0.39-002 because it is **kernel memory management**, not
measurement, and because folding an investigation with an unknown outcome into a feature
line would make both unreviewable.

## What is actually known

Verified, by the implementer of RFC-0.39-001 and re-verified by the architect:

- **The symptom.** `clean-console` refuses a NUL at serial-log offset 7210; the log holds
  exactly one NUL run, of **46** bytes; `upgraded: update index fetch returned no index` —
  46 characters — is absent. `upgraded`'s own text, at offset 4290 of its text page, reads
  as zeros. Strings earlier on the same page print correctly.
- **It reproduces.** 5 of 10 runs at RFC-0.39-001's tip; about 1 in 10 at that line's base
  commit; and about **0 in 40** in a scratch worktree with byte-identical inputs. Nobody
  can explain the last one.
- **It is placement-sensitive.** Bimodal in `upgraded`'s physical address — `0x80451000`
  passes, `0x80481000` fails.
- **RFC-0.39-001 did not cause it.** `upgraded` is untouched, `fjell-upgraded.bin` is
  byte-identical across that line, and **no kernel source changed at all**. Its only
  channel to the outcome is placement.
- **Ruled out by experiment** (8-run blocks): the new console prints; the new
  `fjell-measure-format → fjell-abi` edge; binary size as such; `measuredd`'s logic;
  `init`'s change; a path leaking into the kernel; the disk image; the host tool; and
  DMA-zeroed frames overlapping `upgraded`'s text pages — a kernel instrument printed
  every DMA zero PA against every text PA over ten runs and they **never overlapped**.
- **E-098's defects**, which are verified independently of whether they explain anything:
  `revoke_by_pa` skips the user-VA unmap over a corruption its own comment says *"was
  never isolated"* and describes as *"clobbering the task's text/rodata PTEs"*; it calls
  the stale mapping *"harmless because the frame is zeroed before reuse"*, which is
  backwards, because zeroing a frame a live task still maps is the damage and not the
  safety; it discards the frame free's result with `let _ =`; and its deferral reads *"to
  v0.3"* while 0.38.0 has shipped.

**What is not known is the cause.** Nothing below assumes E-098 is it.

### One thing that is *not* a lead, so nobody spends a day on it

The architect first called the failure run (2–6 failing, 7–10 passing) "notably clustered"
and a lead about cross-run state. **That was an over-reading and is retracted in E-097.**
A run of five or more identical outcomes occurs in **10.9%** of ten fair trials, which is
the fair test because the failure count was not fixed in advance. Do not chase cross-run
state on the strength of it.

## Requirements

**R1 — A measured reproducer, before any theory.** State a procedure and the **failure
rate it produces**, over enough runs that the rate means something — 40 or more, not ten.
Report the rate for: the current tip, the main tree; and the same tree in a scratch
worktree, because the ~0/40 worktree result is the most confusing fact available and
either it survives a larger sample or it does not.

**This is the requirement everything else depends on.** An intermittent defect without a
measured baseline cannot be shown fixed, and a fix for it cannot be distinguished from a
change in placement. If R1 ends with "I cannot make it reproduce reliably", **stop and
escalate** — that is a legitimate and important outcome, not a failure.

**R2 — Make the allocator able to answer "who owns this frame".** The previous
investigation stalled partly because the allocator's owner map read `None` in an
instrumented build, so nobody could see who owned a frame at the moment it was zeroed. A
frame allocator that cannot name a frame's owner cannot be debugged.

Whether this ships or stays a scratch instrument is your call — **say which and why.** If
it ships, it is kernel code and needs its own justification; if it is scratch, prove the
revert the way 0.37/0.38's R5 demonstrations did.

**R3 — Fix E-098's verified defects, regardless of what R1 and R2 find.** These are true
independently of the cause:

1. **The free's result is checked**, not discarded. A failed or refused `free_frame` must
   be visible. State what the kernel does when it fails — and note that "nothing, because
   it cannot happen" is the claim the discarded result currently prevents anyone from
   testing.
2. **The backwards justification goes.** Whatever the code ends up doing with the stale
   PTE, the comment must not say a frame zeroed under a live mapping is harmless because
   it was zeroed. If the PTE stays, say what actually makes that safe; if nothing does,
   say that.
3. **The deferral is re-dated or withdrawn.** "Deferred to v0.3" at 0.38.0 is not a
   deferral, it is a lapsed one.

**R3 does not require closing E-098** if the stale-PTE decision turns out to need a design
change beyond this line — but it does require that nothing in the tree still claims the
unsafe thing is safe.

**R4 — The fixes that are forbidden.** Do not raise a timeout, retry the tier, relax
`clean-console`, mark `semantic` `continue-on-error`, or pad a binary to move the
placement. **Each of those converts a visible intermittent failure into an invisible
one**, and the last one would look like a fix while changing nothing. E-097 names this
refusal and this requirement repeats it because it will be tempting at the point where the
investigation is expensive and the cut is held.

**R5 — If you find the cause: fix it, and demonstrate the fix against R1's rate.** The
measure is the rate going to zero over a sample at least as large as R1's baseline, in
**both** the main tree and a worktree. A single green run is not a fix for a defect that
passes half the time.

**R6 — If you do not find the cause: say so, and E-097 stays OPEN.** See below. Report
what you ruled out, with the method and the sample size for each, so the next attempt
starts where this one stopped rather than repeating it.

**R7 — Gates by exit status.** `release-rehearsal` (expect Gate 7 red while E-097 is
OPEN — that is not this line failing), `consistency-check --all`, `readiness-check`,
`abi-snapshot --verify`, `fjell-repro-check`, `test-all`, `callsite-audit`,
`cargo fmt --check`. Kernel changes move the prebuilts: rebuild **before** re-recording
the repro baseline and commit the binaries with it. One `test-all` at a time.

### This line may not close E-097, and that is an accepted outcome

Every other line in this project has had a definite remedy. This one has a symptom and a
lead. **It is explicitly acceptable for this line to end with E-097 still OPEN**, provided
R1, R3 and R6 are done: a measured reproducer, E-098's verified defects fixed, and an
honest record of what was ruled out.

That is stated here because the alternative is worse. An RFC that required closing E-097
would create pressure to declare a cause on insufficient evidence, or to apply one of R4's
forbidden quietenings and call the tier green. A held milestone is a smaller cost than a
false closure, and the owner has already accepted holding it.

### Non-goals

- **The v0.8.x unmap path.** The comment says its root cause was never isolated. Isolating
  it is in scope only if R1's reproducer leads there; reinstating the unmap is **not** in
  scope on any other ground.
- **Reclaiming task memory in general** — E-093's territory, and a design question.
- **RFC-0.39-002's work.** Independent; it proceeds in parallel.
- **Any addition to the kernel's ABI surface.** Escalate.

## Risks

- **Chasing E-098 because it is the only lead.** The non-overlap instrumentation argues
  against that specific path. If the evidence stops supporting it, say so and leave R3's
  fixes standing on their own merits — they do not depend on E-098 explaining anything.
- **Declaring victory on a quiet sample.** The defect passes half the time; ten green runs
  are what it looks like when nothing has been fixed. R5 sets the bar deliberately high.
- **The worktree result.** ~0/40 with byte-identical inputs is either a measurement
  artefact or the most informative fact in the file. R1 asks which; do not resolve it by
  assuming the first.
- **Scope drift into kernel memory management at large.** The frame allocator, DMA
  lifetimes and task teardown are all adjacent and all out of scope. Report what you see,
  fix what R3 names, and escalate the rest.
