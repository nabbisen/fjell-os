# RFC-0.35-003 R1–R8: a build that --help no longer starts, and a summary nothing compared

**Governing RFC:** [../done/RFC-0.35-003-instruments-that-answer-the-question-you-asked.md](../done/RFC-0.35-003-instruments-that-answer-the-question-you-asked.md)
**Handoff:** [../handoffs/RFC-0.35-003-instruments-that-answer-the-question-you-asked/implementation-handoff.md](../handoffs/RFC-0.35-003-instruments-that-answer-the-question-you-asked/implementation-handoff.md)

Written in the order the handoff requires: R1 with controls first, R3's
before-transcript taken in a scratch worktree before R2 landed, R4's sweep
done with the same discipline R1 used, R5 shown failing on a reconstructed
fixture before passing on the tree, everything after ordinary.

---

## R1 — Re-derivation, with controls, and what Gate 5 actually checks

**`fjell-repro-check`'s flag handling, confirmed unchanged from the RFC's
own derivation:** `args.iter().any(|a| a == "--skip-build")` — any other
argument, `--help` included, falls through to `two_build_check`. No
`--help` arm exists. Control: `--artefacts <dir>` is documented in the
module's own usage comment and has **zero** implementation anywhere in the
file — a second, smaller instance of "claims more than the code does,"
found while reading for the first.

**What Gate 5 (`fjell-readiness-check`) actually checks, named rather than
implied:** it parses `v1-readiness.md`, counts `|`-prefixed rows by status,
prints all four counts, and **exits on `open == 0` alone**. Read in full —
there is no comparison anywhere in the file between what it just counted
and what the document's own `## Summary at …` table or prose claims. The
counts it prints are informational; only `open` decides the exit code.
Confirmed live: `readiness-check` reports `DONE 56 / IN_PROGRESS 2 /
DEFERRED 3 / OPEN 0`, matching the document's own summary table exactly
*today* — the gap is not that they currently disagree, it is that nothing
would notice if they did, and twice they did.

## R3 (first half) — the failing case, shown before R2 removed it

Scratch worktree (`git worktree add --detach`, unmodified tip), repro-check
built unmodified:

```
$ cargo run -p fjell-repro-check -- --help
fjell-repro-check: performing two-build reproducibility check
[... full build output ...]
fjell-repro-check: build 1 took 7.83s, build 2 took 1.93s …
fjell-repro-check: PASS (33 artefacts identical)
```

Asking for help ran two complete, independent builds and reported a
reproducibility verdict — never printing a usage line. Exit 0, having done
the one thing `--help` must never do. Full transcript (scratch, cited by
path): `target/scratch/rfc-0.35-003-evidence/unknown-flag-starts-build-before-fix.log`.

## R2 — `fjell-repro-check` refuses what it does not understand

`first_unrecognized` walks the argument list against `KNOWN_FLAGS`
(`--skip-build`, `--record-baseline`, `--help`, `-h`) and
`KNOWN_VALUE_FLAGS` (`--build-cmd`, `--baseline`), consuming a value flag's
own value without checking it against `KNOWN_FLAGS` (so `--build-cmd
--skip-build` does not misread the value as a second flag — a regression
test reproduces exactly this). `--help`/`-h` is checked **before**
validation, so it answers regardless of what else is on the line. An
unrecognised argument prints `USAGE` to stderr and exits non-zero; `--help`
prints it to stdout and exits 0. `--artefacts` is removed from the usage
text rather than carried forward (R1's own finding).

```
$ cargo run -p fjell-repro-check -- --help
Usage:
  fjell-repro-check [--build-cmd <cmd>]
  fjell-repro-check --skip-build [--baseline <path>] [--record-baseline]
  fjell-repro-check --help | -h
…
[exit 0]

$ cargo run -p fjell-repro-check -- --bogus-flag
fjell-repro-check: unrecognised argument: --bogus-flag
Usage: …
[exit 1]
```

Both pasted from a real run, not written then assumed (handoff trap 1).
13 unit tests, including the removed `--artefacts` flag's new rejection.

## R3 (second half) — the same flag, no longer starting a build

Same binary, rebuilt with the fix, same invocation:

```
$ cargo run -p fjell-repro-check -- --help
Usage:
  fjell-repro-check [--build-cmd <cmd>]
  …
[exit 0, 22 lines, no build output]
```

`target/scratch/rfc-0.35-003-evidence/unknown-flag-no-longer-starts-build-after-fix.log`.

## R4 — The sweep, what it found, what was the same defect, what was filed

Swept `crates/fjell-tools/src/` and `tools/` for `args.iter().any(|a| a ==
…)`-style flag detection (18 matches) and read each `main()` fully. **The
defect this entry names has two necessary ingredients — no `--help`, and
an unrecognised argument silently accepted — not merely "uses `.any()`
somewhere."**

| Tool | Has `--help`? | Rejects unrecognised? | Verdict |
|---|---|---|---|
| `fjell-abi-snapshot`, `fjell-consistency-check`, `fjell-tools` (xtask dispatcher) | — (subcommand dispatch) | **yes** — unmatched subcommand already prints usage + `FAILURE` | not the same defect |
| `fjell-summary-check` | **yes**, already | no (defaults silently) — but every path runs the same one check; no wrong-mode risk | a different, lower-severity shape; not fixed here |
| `crates/fjell-tools/src/bench.rs` | no | no — a typo'd `--baseline`/`--check` silently falls to "just print," neither writing nor checking what was asked | **different defect** (silent intent-loss, not a wrong expensive mode) — **filed**, not fixed: **E-077** |
| `fjell-mmio-audit` | no | no | **same defect — fixed** |
| `fjell-unsafe-audit` | no | no (`_ => {}`) | **same defect — fixed** |

**Control for "nothing else":** every remaining `.any()` match in the swept
directories (`dev_modes.rs`, `provision.rs`, `sign_bundle.rs`,
`trust_report.rs`, `test_all.rs`) toggles an optional sub-behaviour under a
subcommand the top-level dispatcher has *already* validated by name
(`dev run`, `provision-dev`, `sign-bundle`, …) — getting the toggle flag
wrong does not select a different, costly mode; the already-explicitly-
requested action runs either way. `fjell-readiness-check` has neither
`--help` nor rejection either, but no caller passes it anything but its own
`--matrix <path>` (checked, every call site) and its one behaviour is a
cheap read — not fixed, named here so the next sweep does not have to
re-discover it.

**Fixed the same way as `fjell-repro-check`:** `fjell-unsafe-audit` and
`fjell-mmio-audit` each gained a `USAGE` constant, `--help`/`-h`, and
rejection of anything else, with unit tests for the new `first_unrecognized`
logic (`fjell-mmio-audit`) or the loop's new default arm
(`fjell-unsafe-audit`).

### The sweep found a live bug, not just the pattern

Fixing `fjell-unsafe-audit` to reject unrecognised arguments broke its own
callers: **four of them** — `release-rehearsal` Gate 2, `test-all` tier 3,
`trust_report.rs`'s §5, and CI's own `ci-unsafe-audit` job — pass
`--workspace .`, which is **`fjell-mmio-audit`'s flag name**, not this
tool's own (`--root`). Before this line, `--workspace` matched nothing in
`fjell-unsafe-audit`'s `_ => {}` arm and was silently discarded; `root`
stayed at its default (`"."`), which is what every caller's value also
was — so the audit ran correctly **by coincidence**, not because the flag
worked. Confirmed live:

```
$ cargo run -q -p fjell-unsafe-audit -- --workspace . --check
fjell-unsafe-audit: unrecognised argument: --workspace
[exit 2]
```

All four call sites, plus three documentation pages giving the same wrong
command (`unsafe-charter.md`, `unsafe-gate.md`, `release-checklist.md`)
corrected to `--root`; the fifth and sixth mentions
(`rfcs/ERRATA.md`'s historical entry, `instrument-audit.md`'s own audit
record) are **left untouched** — both are historical narrative describing
what was invoked *at the time*, not a live instruction, the same exclusion
`version-currency`'s own design note already makes for narrative documents.
Every corrected command run for real:

```
$ cargo run -q -p fjell-unsafe-audit -- --root . --check
fjell-unsafe-audit  root=.
  total unsafe sites : 282
  with SAFETY comment: 282
  with valid category tag: 282
  missing comment    : 0
[exit 0]
```

## R5 — `readiness-summary`, Gate 12's 20th subcheck

**Design.** `tools/fjell-consistency-check/src/readiness_summary.rs`, a
pure text-scanning checker. `count_dimension_rows` duplicates
`fjell-readiness-check::check_matrix`'s row-classification deliberately
(the established convention — `capability_topology`'s own module doc
states the same reasoning) and stops before `## Summary at`, so the summary
table's own `|`-prefixed rows are never double-counted. `summary_block`
isolates the heading, table and first prose paragraph, stopping at the
first `\n>` — so a correction blockquote's own historical figures (which
legitimately *quote* an old, wrong number as part of explaining it) are
never read as a live claim. `word_before` extracts the number-word
immediately preceding `"OPEN cell"` / `"IN PROGRESS"` in the **prose only**
(scoped past the table, which also contains the literal string `"IN
PROGRESS"` as a row label — found by a failing test before being trusted).

**Three independent comparisons**, each reported separately:
1. The table's four counts vs. `count_dimension_rows`'s own.
2. The prose's number-words vs. the table's own counts (not the row
   count — R5's own wording: *"must not state a figure the table
   contradicts"*).
3. (R6) The heading's version vs. the workspace version.

**Failing on the fixture, passing on the tree.** 8 unit tests, including
`the_2026_10_12_block_fails` — the real historical disagreement
reconstructed (headed `v0.9.4`, table claiming `DONE 42`/`IN PROGRESS 0`
against real rows counting 56/2, prose claiming "three") — and
`a_correction_blockquote_s_own_figures_are_not_read_as_a_live_claim`,
proving the blockquote boundary holds. Against the real tree:

```
$ cargo run -p fjell-consistency-check -- readiness-summary
readiness-summary: PASS (summary agrees with docs/src/releasing/v1-readiness.md's own rows)
```

## R6 — The heading is enforced, against the workspace version

**Enforced**, not merely noted. `## Summary at <X>` is compared against
`Cargo.toml`'s `[workspace.package] version` (`version_currency`'s own
parser, made `pub(crate)` and reused — one source for "what version is
this right now," not two). **Why this is never a false alarm mid-milestone:**
the workspace version changes only at a cut, and the summary's own counts
change only when a matrix row does; an RFC line like this one, touching no
row and no `Cargo.toml`, leaves both sides of the comparison exactly where
they were. Both prior incidents happened **at** a cut boundary — the
heading not moving with the version, or the version moving and the heading
not following — which is precisely the boundary this check now covers.

## R7 — E-071 CLOSED, with both halves recorded

**E-071 CLOSED** in `rfcs/ERRATA.md`, with a dated blockquote covering both
the flag-handling fix and the sweep's own finding (the four-site caller
bug), and a parallel paragraph in `v1-limitations.md` in the same commit.
**The readiness-summary gap has no erratum number of its own** — the
governing RFC's own words, since it was "found and corrected by hand twice"
with no erratum ever filed — recorded instead in
`readiness_summary.rs`'s own module doc (a reader meeting the gate meets
the explanation) and folded into E-071's own `v1-limitations.md` paragraph,
since the RFC itself treats both as one defect. `v1-readiness.md`'s own
correction note — which promised *"0.34 carries a line for it"* and then
had to admit 0.34 did not — now has a third amendment naming this line as
the one that does.

**Also filed, not fixed (R4):** **E-077** — `cargo xtask bench`'s
`--baseline`/`--check` flags fail open to "just print" on a typo, silently
not recording or checking what was asked. A different defect from E-071's
own shape (intent-loss on a typo, not a wrong expensive mode triggered by
one) — named per R4's own instruction, not repaired here.

## R8 — The gates, each by its own exit status

| | |
|---|---|
| `cargo fmt --all --check` | **exit 0** |
| `abi-snapshot --verify` | **PASS**, exit 0 — 453/453, unaffected (no ABI-tracked surface touched) |
| `release-rehearsal` | **exit 0 — ALL MECHANICAL GATES PASS**; Gate 12 now **20 subchecks** |
| `consistency-check --all` | **exit 0 — PASS, 20 subchecks**; `readiness-summary: PASS`; `errata-limitations: PASS (10 ACCEPTED)` |
| `fjell-consistency-check` host tests | **207 passed, 0 failed**, including `readiness_summary`'s 8 |
| `fjell-repro-check`, `fjell-unsafe-audit`, `fjell-mmio-audit` host tests | **13 + 12 + 10 passed, 0 failed** |
| `test-all` | *(below, filled after the background run completes)* |
| repro-check | `--skip-build` **PASS, 32 artefacts identical** — no guest code touched this line, no rebuild, no baseline re-record needed |
| CI run id | none — nothing pushed |

## Decisions I am making

1. **`readiness-summary` duplicates row-counting rather than depending on
   `fjell-readiness-check` as a library.** Matches this tool family's own
   stated convention; the alternative (a `[lib]` target for a tool that has
   never needed one) is more plumbing for the same dozen lines.
2. **The prose check compares against the table, not the re-derived rows.**
   R5's own wording draws this line — "a figure the table contradicts," not
   "a figure the rows contradict" — and it is the simpler, more mechanical
   check: a prose sentence can only ever be read against the number
   sitting two lines above it in the same block, not against a count the
   reader of the prose cannot see.
3. **`fjell-readiness-check` itself is not touched.** No caller passes it a
   mismatched flag (checked), and its one behaviour is a cheap, read-only
   scan — outside R4's own bound ("do not widen").

## Afterword — what survived contact

R4's sweep was supposed to be the small, bounded part of this line, and it
is where the real finding was: not a second instance of E-071's own shape
(though there were two), but a caller bug the first tool's own silence had
been hiding the whole time — four call sites and three documentation pages,
none of them wrong by intent, all of them wrong by a flag name that was
never checked. Tightening `fjell-unsafe-audit` is what made that visible;
nothing short of actually re-running the gates after the fix would have
caught it before it broke `release-rehearsal`, `test-all`, the trust
report, and CI in one commit.
