# Developer Handoff — RFC-0.39-003

**Governing RFC:** [RFC-0.39-003](../../done/RFC-0.39-003-find-out-why-a-live-page-reads-as-zeros.md)
**Milestone:** 0.39
**Status:** inherited from the governing RFC (Implemented, 0.39.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Do we know why a live service's text page reads as zeros?** Today we have a symptom
that reproduces about half the time, a placement correlation, and one lead that the
evidence partly argues against.

**This is the first line in this project whose success does not require closing its
erratum.** If you end with a measured reproducer, E-098's verified defects fixed, and an
honest account of what you ruled out — and E-097 still OPEN — **that is a completed
line.** Read §"This line may not close E-097" in the RFC before you start, because it
changes what you are optimising for.

## 0.1 You are not starting from zero

The implementer of RFC-0.39-001 did a great deal of work on this before handing it over,
and the RFC lists what they ruled out with their methods. **Read that list before running
anything** — nine things are already eliminated, each with an 8-run block behind it. The
most useful facts they established:

- The failure is **bimodal in `upgraded`'s physical address**, so it tracks placement.
- **No kernel source and no `upgraded` byte changed** in the line that surfaced it.
- **DMA-zeroed PAs never overlapped `upgraded`'s text pages** across ten instrumented
  runs — which is the fact that argues against the obvious lead.
- The **worktree anomaly**: ~0/40 with byte-identical kernel ELF, prebuilts, QEMU command
  line and disk image. They reported it rather than using it to claim innocence.

## 0.2 Two things the architect got wrong, so you do not inherit them

1. **The "clustering" is not a lead.** I called the failure run (2–6 failing, 7–10
   passing) notably clustered and suggested cross-run state. A run of five or more
   identical outcomes happens in **10.9%** of ten fair trials. It is a one-in-nine
   coincidence. Retracted in E-097; do not spend a day on cross-run state because of it.
2. **E-098 is a lead, not the cause**, and I filed it as a separate entry specifically so
   that this line is not steered into proving it. If the evidence stops supporting it, say
   so — R3's fixes stand on their own merits either way.

## 1. Order

1. **R1 first, and do not skip it.** A measured rate over 40+ runs, for the main tree and
   a worktree. Everything else is unfalsifiable without it — including any fix you might
   be tempted to call done. If you cannot make it reproduce reliably, **escalate**; that
   is a real outcome.
2. **R2**, allocator ownership visibility. The previous attempt stalled on exactly this.
3. **R3**, E-098's three verified defects. These are independent of the investigation and
   should land whatever R1 and R2 turn up — do them while the reproducer runs rather than
   last.
4. **Then investigate**, with a reproducer and an instrument.
5. **R5 if you find it, R6 if you do not.** Both are complete endings.
6. **R7**, the gates. Gate 7 will be red the whole time; that is E-097 being OPEN, not
   this line failing.

## 2. Traps specific to this line

1. **The forbidden quietenings (R4).** Raising a timeout, retrying the tier, relaxing
   `clean-console`, marking `semantic` `continue-on-error`, or padding a binary to move the
   placement. The last is the dangerous one: it would look like a fix, pass the tier, and
   change nothing about the defect. They will all be tempting around the point where the
   investigation is expensive and the milestone is visibly held. **If you reach that point,
   escalate instead** — a held milestone was already accepted by the owner and is cheaper
   than a false closure.
2. **Ten green runs are what "not fixed" looks like.** The defect passes about half the
   time. R5 sets the bar at R1's sample size in both tree locations for exactly this
   reason.
3. **Do not reinstate the v0.8.x unmap** on the theory that it was fine. Its comment says
   the root cause was never isolated; reinstating it is in scope only if your reproducer
   leads there.
4. **Scope drift.** The frame allocator, DMA lifetimes and task teardown are adjacent and
   all out of scope. E-093 already owns the general reclamation question. Report what you
   see, fix what R3 names, escalate the rest.
5. **`/usr/bin/grep -a`, not `grep`** — plain `grep` skips NUL-containing files here, and
   this investigation is **about a file full of NULs**. This trap has never been more
   directly relevant: the serial log is the evidence and plain `grep` may silently decline
   to read it.
6. **Rebuild before re-recording the repro baseline** and commit the `.bin`s with it. Note
   that any kernel change moves every prebuilt's placement, so expect your own R1 rate to
   shift when you change the kernel — measure it again rather than comparing across
   builds.

## 3. What to put in the review request

- **R1's rate**, with the procedure, the sample size, and the two locations. If the
  worktree anomaly survived, say so plainly; it is the most interesting thing you could
  report.
- **R2's decision** — shipped instrument or scratch — and why; with the revert proved if
  scratch.
- **R3's three fixes**, and specifically what the kernel now does when a free fails.
- **Either R5's before/after rates, or R6's list of what you ruled out**, each with its
  method and sample size, written so the next attempt starts where you stopped.
- **Whether the evidence still supports E-098 as the lead.** A clear "no" is as useful as
  a yes.
- **Exact commands and exit statuses** for every gate in R7, with Gate 7's red noted as
  expected.
- **Any place my RFC is wrong.** I have already had to retract one claim in it (§0.2) and
  this is the line where I am least able to predict what you will find. The tree wins, and
  here it wins harder than usual.
