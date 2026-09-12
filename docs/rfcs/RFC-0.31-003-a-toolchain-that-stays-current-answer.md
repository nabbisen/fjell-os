# RFC-0.31-003 §7 — What stops the next ten-month drift?

**Governing RFC:** [rfcs/accepted/RFC-0.31-003-a-toolchain-that-stays-current.md](../../rfcs/accepted/RFC-0.31-003-a-toolchain-that-stays-current.md)

Answered before the pin, per the handoff's order — the pin is what creates
the problem this question is about, so deciding it afterwards would be
deciding it under pressure to ship.

---

## Answer: shape 4, with teeth — not shape 3, and shape 2 rejected on a structural ground rather than on discomfort

**Exit criterion 10**: the cut records the pinned version, current stable,
and the gap between them. **Beyond three minor versions the tag is blocked
unless the owner writes an accepted-risk statement** — exit criterion 9's
shape exactly, applied to the toolchain instead of to CI.

## First, the question the RFC asks about its own lean

> *Say plainly whether that makes my lean the cheap option chosen twice.*

**Shape 4 alone, as written, is the cheap option.** Shape 4 *with teeth* is
not, and the difference is the whole answer.

"The cut records how far behind the pin is" is a reporting step. This
project has just spent a milestone establishing that **a report nobody must
act on changes nothing** — that is E-041 in one sentence, and the badge on
`README.md` was red for four months while eleven documents said CI was
passing. Exit criterion 9 does not work because it records the CI run. It
works because *"a red job blocks the tag or takes an accepted-risk
statement"*. The teeth are the mechanism; the recording is the occasion.

So the honest form of the lean is: record it **and** give it a threshold
that stops the cut. Without the threshold, criterion 10 is a number in a
document, and this project has a demonstrated ten-month record of numbers in
documents not being read.

## Shape 3 is refused, and the RFC's own paragraph is why

> *a signal nobody is obliged to read is E-041's entire subject*

That is correct and it is decisive. **Staleness was never an information
problem.** `rustup check` would have printed the gap on any day of the ten
months. Nothing was hidden; nothing needed discovering. What was missing was
a *moment at which somebody had to answer for it*.

A scheduled job adds information to a project that already had all of it.
Worse, it adds the *appearance* of coverage: a green-ish nightly nobody
reads is how a project convinces itself the question is handled. Given this
milestone's subject, adding an instrument that reports into the void would
be the single most on-the-nose mistake available.

**Refused, and not as a companion either.** The RFC offers 3 alongside 4; I
think 3 subtracts from 4 by supplying a plausible answer to "who is watching
this?" that is not a person.

## Shape 2 — argued on its merits, and rejected for a reason that is not the network

Shape 2 is the strongest *forcing* function on offer: a gate that fails
cannot be walked past, and it fires between cuts rather than only at one.
The RFC is right that it deserves better than inherited discomfort.

The network is the weaker objection. The real one is this:

**Every gate in this project is a pure function of the committed tree, and
the release process depends on that property.**

- `release-handoff.md` §1 step 14 clones the committed tree into a scratch
  directory and re-runs `consistency-check --all` there, expecting the same
  verdict. That check exists because it is what caught **E-038**. A gate
  whose answer depends on today's date and on upstream's current release
  makes "re-run the gates on this tree" a question with no stable answer.
- Release records cite gate output as evidence. **Evidence that changes
  meaning over time is not evidence.** `records/0.29.0.md` says
  `consistency-check` passed; under shape 2 that sentence would quietly
  become false six months later, with nothing in the tree having changed.
- Every historical commit would eventually go red. "Was this tree green when
  it shipped?" stops being answerable retroactively — and that question is
  the reason the records exist.

A gate that fails on a clock is also a gate that fails on work unrelated to
the change in front of you. That trains people to route around it, which
recreates E-041's dynamic in a worse form: not an instrument nobody reads,
but an instrument everybody has learned to ignore.

**The hybrid I considered and rejected.** Shape 2 can be made offline: a
scheduled job commits a file recording "current stable as of date X", and
the gate compares the pin against that committed file — a pure function of
the tree again. It is genuinely elegant, and it collapses. If the job stops
running or nobody merges its output, the file goes stale and the gate passes
on stale data: fail-open, *invisible indistinguishable from absent*, the
exact family this milestone has spent four RFCs removing. Making the gate
fail when the *file* is old restores the time-dependence it was built to
avoid. There is no version of this that is both offline and not fail-open.

## Why shape 4 is stronger here than it looks, and the evidence for it

The RFC's stated weakness of shape 4 is that it fires *"only as often as we
cut"*. Measured, from `docs/release/records/`:

| | |
|---|---|
| Releases cut `0.21.3` → `0.30.0` | **10** |
| Elapsed | 2026-07-30 → 2026-09-10, **six weeks** |
| Median interval | **~3 days** |

This project cuts roughly every three days. "Only as often as we cut" is
more often than any scheduled weekly job would fire, and the ten-month
toolchain gap spanned **every one of those ten cuts**. Criterion 10 is not a
rare-event instrument here; it is close to continuous.

That frequency introduces the one real risk: a step that says "0 behind"
every three days becomes furniture, and the one time it says "7 behind" it
gets waved through. **That is precisely what the threshold prevents** — at
four or more minor versions behind, there is nothing to wave; the tag stops
until the owner writes down a reason. The step being boring nine times out
of ten is fine, because the tenth time it is not a step, it is a wall.

## Why three minor versions

Rust ships a stable release every six weeks. Three minors is about four and
a half months — long enough that skipping a release or two while something
else is urgent costs nothing, short enough that no drift can reach ten
months without a person having explicitly signed for it at least once.

It is a starting value, not a derived constant, and the criterion says so.
The number that matters is that one exists.

## What this does and does not claim

It does not claim the toolchain will stay current. It claims **it cannot go
stale without a dated, signed decision in a release record** — which is the
same thing E-039's closure achieved for the cut itself, and the same thing
exit criterion 9 achieved for CI. This is the third application of one
pattern: *the gates stay offline and deterministic; the outside world is
observed at a moment, recorded where someone is already reading, and given
teeth.*

That the pattern repeats is not evidence of choosing cheaply. It is the
pattern that has closed E-039 and E-041 in this project, and shape 2's
alternative would cost the property that makes every one of those records
re-checkable.
