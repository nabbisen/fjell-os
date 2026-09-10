# RFC-0.30-003 §7 — What is the single source of truth?

**Governing RFC:** [rfcs/accepted/RFC-0.30-003-a-toolchain-that-records-itself.md](../../rfcs/accepted/RFC-0.30-003-a-toolchain-that-records-itself.md)

Answered after D1/D2 (the recording) were built, per the handoff's required
order — the recording does not depend on how this lands.

---

## Answer: shape 3 — a drift gate over the 21 live declaration sites

A new `toolchain-declarations` subcheck (`tools/fjell-consistency-check`)
parses `rust-toolchain.toml`'s `channel` as the anchor and compares it
against every other **live** declaration: the 17 `ci.yml` install blocks,
`docs/src/internals/local-development.md` (its table row and its
`rustup toolchain install` line, checked independently), `docs/src/
tutorials/quick-start.md`, and `docs/release/release-checklist.md`'s
check. 21 of the 22 sites participate; `Cargo.toml`'s `rust-version` does
not (see "What is deliberately excluded" below).

## The lean was right, but not for either reason the RFC gave for it

The RFC leaned to shape 3 because D1 has a downstream dependency and a
drift gate makes later consolidation safe. Both are true and neither is
the strongest argument. **The strongest argument is that shape 1 would
make the exact incident that motivated this RFC worse, not better.**

Finding 6 — CI installs from apt, bypassing `rust-toolchain.toml`
entirely — reads like a gap. Measured against what actually happened on
2026-09-09, it is not: `rust-toolchain.toml` was removed, and **local**
builds silently drifted to `1.98.1`. CI did not, because CI never reads
that file. CI's independence from `rust-toolchain.toml` is precisely
what kept a stable reference point through the incident — the thing the
drift was eventually measured against.

**Shape 1 deletes that independence.** Making CI install via `rustup`
(which honours `rust-toolchain.toml` by construction, the entire appeal
of shape 1) means the next time that file is removed or misconfigured,
`rustup` does not fail — it falls back to whatever the ambient default
toolchain is, and CI goes green on it, silently, the same way the local
builds did. Shape 1 does not fix the single-point-of-failure this
project already has one incident from; it would extend that single point
of failure onto CI, which is the one thing that caught the last drift by
*not* depending on the file that broke.

**This is not the "instrument able to see, not the thing being measured"
pattern the RFC worried about being on its fourth repetition.** That
pattern applies when detection is chosen because prevention is expensive.
Here, the "prevention" on offer is not safer than detection — it is
detection with a new failure mode of its own, one this project has
already paid for once. Shape 3 is not the cheaper option chosen out of
avoidance; it is the option that does not reintroduce the exact defect
this RFC exists to close.

## What is deliberately excluded, and why

**`Cargo.toml`'s `rust-version` is not compared against the channel.**
It is a floor (the oldest toolchain the crate claims to build under), not
a mirror of the exact pinned version — this project's own Non-goals say
a bump should generally not move it. Requiring exact equality would be
correct today (both happen to read `1.91`) and wrong the first time they
legitimately diverge, which is exactly D3's warning applied to a second
axis: not every two things that currently say the same number are the
same *kind* of declaration.

**The Verus (`1.95.0-x86_64-unknown-linux-gnu`) and `nightly`
(`cargo-fuzz`) toolchains are named, not consolidated** (Finding 4,
Non-goals). Two additional live mentions of "Fjell's 1.91" were found
while re-deriving the counts — `verification/verus/TOOLCHAIN.md:25` and
`docs/src/verification/verus-setup.md:18`, both cross-references *inside*
Verus-toolchain documentation, comparing Verus's pin against Fjell's in
passing. Neither is one of the 22: they describe the Fjell toolchain
rather than declaring it, and they live inside the explicitly-out-of-scope
Verus documentation. Not wired into the gate; named here so they are not
silently forgotten either.

## Does E-037 close?

**No — `ACCEPTED`, with two survivors named, not `CLOSED`.** E-037 set its
own closure bar: *"one declaration, an exact pin, and a record of which
toolchain produced each artefact."* Shape 3 delivers the third clause in
full (D1/D2) and turns the first clause from a hazard into a checked
invariant — but it is still 22 places to edit at a bump, and the channel
remains floating. Two of three, honestly said rather than a `CLOSED` the
next reader has to notice is wrong.

## Should the channel be pinned to an exact patch? (D5 — argued, not done)

**Probably, eventually — and deliberately not decided here.** The
project has already been bitten twice by a version move changing
digests (`-C metadata`, cited in this milestone's own earlier lines, and
the `1.91→1.98.1` incident this RFC is about). An exact pin
(`channel = "1.91.1"`) would remove one more axis of non-determinism and
make the "same-machine reproducibility" RFC-0.30-001 measured slightly
stronger against time (a floating channel can still drift *within* one
machine, between two widely-spaced builds, if `rustup` re-resolves it).

Against: floating within a minor version is normal Rust practice, and an
exact pin means every upstream patch release — including security
fixes — requires a deliberate `rust-toolchain.toml` edit rather than
arriving for free. That cost is real and this line has not weighed it
against the benefit with the care the decision deserves. **D5's
instruction stands: this is a separate decision, argued on its own
merits in its own line, not smuggled in here because the file is already
open.**

## What changed

- `tools/fjell-consistency-check/src/toolchain_declarations.rs` (new):
  the drift gate described above, wired into `ALL_SUBCHECKS`.
- `docs/src/tutorials/quick-start.md`: `rust-1.91-src` → `rust-src`,
  matching the package name every CI job actually installs (Finding 1's
  "second, quieter disagreement" — a real bug, a new user's copy-pasted
  command would fail on that package name).
