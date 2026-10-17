# RFC-0.36-002: A benchmark flag that does what it says

**Status:** Implemented (0.36.0) — accepted 2026-10-15
**Milestone:** 0.36
**Tracks.** **E-077** — `cargo xtask bench`'s `--baseline` and `--check` are read
with `args.iter().any(…)`, so a typo matches neither and `cmd_bench` falls silently
to its default arm: run the benches, print the results, exit success. A caller who
meant to **write** a baseline believes they recorded one; a caller who meant to
**check** tolerance believes they checked it.
**Touches** *(indicative)*: `crates/fjell-tools/src/bench.rs`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **E-071** and **RFC-0.35-003**, which closed the identical shape in
`fjell-repro-check`, `fjell-unsafe-audit` and `fjell-mmio-audit` — including the
exit-code contract that review had to correct afterwards. This line is the fourth
application of a remedy that now has three precedents and one known mistake to
avoid.

## Why this is its own line and not a footnote

E-071's unknown flag triggered the **more expensive** of two modes, which is a cost.
This one triggers the **cheapest of three**, which is a **wrong answer**: the
command succeeds, prints plausible output, and silently drops the caller's intent.
`cargo xtask bench --basline` reports success and records nothing, and a benchmark
baseline nobody noticed was not written is a measurement everybody trusts later.

It is small. It is here because 0.35's sweep found it, judged it a different defect,
and filed rather than fixed it on that line's own instruction — and a filed finding
with a known remedy and three precedents should not sit for another milestone.

## Requirements

**R1 — Re-derive** the flag reads and the default arm at your tip, with a control,
and confirm the three flags are `--baseline`, `--check`, `--fast` (`--fast` is also
implied by `--check`). Name any fourth you find.

**R2 — An unrecognised argument exits non-zero with the usage; `--help`/`-h` prints
it and exits zero.** Neither runs a benchmark.

**R3 — The exit code is 2, and that must survive `xtask`.** `1` already means
*"a metric exceeded its tolerance"* and *"the baseline write failed"* — the loudest
things this command says. **`cmd_bench` returns an `ExitCode` to a dispatcher**, so
unlike 0.35's three standalone binaries, returning `ExitCode::from(2)` is not
enough on its own: **show that `cargo xtask bench --nonsense` actually exits 2 at
the shell**, not that the function returned it. If the dispatcher flattens it, say
so — a contract that the wrapper discards is the claim-vs-mechanism defect this
family of fixes keeps producing.

**R4 — The failing case, demonstrated.** A transcript of a typo'd flag running the
benches and exiting success **before** the change, and refusing **after**. This is
the closing condition the register already states for this entry; an absence probe
is not it.

**R5 — `--help`'s real output, pasted from a real run**, and every flag it names
checked to exist. `fjell-repro-check`'s own `--help` was found documenting
`--artefacts <dir>`, which had never been implemented.

**R6 — E-077 CLOSED**, `v1-limitations.md` in the same commit.

**R7 — The gates**, each by its own exit status, as the required set. No guest code
changes here, so no prebuilt should move; if one does, find out why before
re-recording anything.

### Non-goals

- Changing what any mode of `bench` does, or what it measures.
- A shared argument parser for `xtask`'s subcommands. If the same shape exists in
  other subcommands, **report it; do not fix it here** — RFC-0.35-003's sweep
  already judged the remaining `.any()` sites safe because the toggle only narrows
  an already-chosen action, and that judgement stands until something disproves it.

## Risks

- **A `--help` nobody ran.** R5 exists because this line's own subject is a flag
  that did not do what it said.
- **`ExitCode::from(2)` through a dispatcher** is the one thing genuinely new here
  versus the three precedents. R3 is the whole of it.
