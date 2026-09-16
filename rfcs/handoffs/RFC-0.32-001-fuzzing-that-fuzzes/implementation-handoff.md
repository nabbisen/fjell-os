# Developer Handoff — RFC-0.32-001

**Governing RFC:** [RFC-0.32-001](../../accepted/RFC-0.32-001-fuzzing-that-fuzzes.md)
**Milestone:** 0.32
**Status:** inherited from the governing RFC (Accepted, 2026-09-15)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line is decoders exercised, not target files

The harness this line replaces was recorded as done with eight targets. One
of them fuzzed anything. **Eight became the number everyone read, and the
number was the defect.** `v1-readiness.md`'s criterion was "≥ 4 targets", and
eight files satisfied it while five had never compiled.

So nothing you report in this line is a count of targets. It is a table of
decoders, each with where its input comes from and whether a real fuzz run —
with a run id — has exercised it. **If this line ends with fewer target files
than it started with, that is the expected shape**, not a regression to
explain away (RFC Risks).

## 0.1 Re-derive before touching anything (R1)

The RFC's findings came from a scratch clone and a single-line grep. Both are
weaker than they look. Reproduce:

```
cargo build --manifest-path fuzz/Cargo.toml 2>&1 | head -3                     # defect 1: "believes it's in a workspace"
git log --oneline --diff-filter=A -- 'crates/formats/*/Cargo.toml' | tail -1   # defect 2: expect a5b5167
git show e63d19f:fuzz/fuzz_targets/attestation_v2_parse.rs | grep -n 'fjell_'  # defect 3 at creation
```

All three were run before this handoff was committed and return what the
comments say. **A fourth did not**: `--diff-filter=R` on the same pathspec
returns nothing, and so does grepping `git show --stat` for `formats/` —
git abbreviates the rename as `crates/{ => formats}/…`, so the literal is
never printed. An empty result from either is the rename display, not
evidence the move did not happen. That is the kind of absence §0.1's
positive-control rule exists for.

Repair each defect **in a scratch clone** under `.git-exclude/tmp/` to expose
the next, as the RFC did. Then check each of the five "never existed" claims
at `e63d19f` yourself — `git grep` at that commit for each function — and
report any that did exist.

**The decoder inventory is the real work of R1, and my first one is
incomplete by construction** (RFC Finding 3). It matched public functions
with one-line signatures. Re-derive it properly: multi-line signatures,
`TryFrom<&[u8]>`, `from_bytes`/`decode`/`parse`/`read` in any visibility,
`u32::from_le_bytes`-style field walks inside functions with other names, and
raw reinterpretations (`read_unaligned`, `from_raw_parts`, `transmute`). For
each: **where does the input come from** (another service, the kernel,
firmware, disk, the network, only the same crate), and **can a fuzz crate on
the host reach it** (public, host-buildable, no RISC-V `asm!`).

**Every probe gets a positive control.** A grep that finds nothing must be
shown finding something it should — run it against `fjell_semantic_v1::decode`,
which you know exists, before believing any absence it reports.

## 0.2 Settled — do not re-open

1. **The harness builds** outside the workspace, with correct paths (D1).
2. **A target exercises a real decoder of bytes from outside its component**,
   or it is retired (D2). No target for a format with no decoder; **no decoder
   written so a target can exist.**
3. **The seed corpora are passed to the run** (D3).
4. **Green is a real fuzz run, per target, with a run id** (D4). A green build
   is not a green fuzz run.
5. **`workflow_dispatch` is admitted by the job's `if:`** (D5). The schedule
   stays.
6. **Exit criterion 9 also reads the latest scheduled run** (D6).
7. **`reassemble` is not fuzzed here** (D7). It is undefined behaviour by
   construction until E-046's line fixes it.
8. **Demonstrated failing on a real run** (D8).
9. **The README badge stays unfiltered** (D9).

---

## 1. Order

**R1 (re-derive, inventory) → §7 and §8 answered in writing → R2 (builds) →
R3 (target table) → R4 (the job, dispatch, crash upload) → push, dispatch,
observe → R5 (demonstrations) → R6 (green per target) → R7 (criterion 9) →
R8 (claims) → R9 (errata) → evidence.**

Two things about this order are not negotiable:

- **§7 and §8 before R4.** The shape of the job depends on both answers.
- **R3 is decided from R1's inventory, not from `fuzz/fuzz_targets/`.** The
  existing file list is the thing under suspicion. Start from the decoders and
  ask which get a target; do not start from the eight and ask which to fix.

## 2. §7 — and the argument against my own lean

**I lean to shape 2**: build every target on push, run them on schedule and
dispatch. The RFC names the hazard, and I wrote both: **a green build job is
the easiest thing in this project to read as "fuzzing works."** E-041 read a
*skipped* job that way, and this erratum exists because of it.

Say whether shape 2's protection is real. It catches a target that stops
compiling. It does not catch a decoder that starts panicking — only a run does
that, and runs happen weekly. **Is a weekly run, now visible through D6 at
each cut, enough for the decoders R1 finds?** If R1 finds a decoder on a real
trust boundary — service-to-service, firmware — argue shape 3 for that one
decoder on its merits, not from the cost of all of them.

If shape 2 is built, **the job's name, its step names and its record entry
say `build`**. A job named `fuzz` that only builds is the defect this line is
correcting.

## 3. §8 — pin or float the nightly

A floating nightly can turn the job red with no change to the tree. That is
E-037's drift on the one job that gates only through D6. A dated nightly
needs:

- a single declaration (where?),
- a currency step like exit criterion 10, and
- a decision about `toolchain-declarations`, which **deliberately exempts**
  any job that runs `cargo +<name>`
  (`tools/fjell-consistency-check/src/toolchain_declarations.rs`, around the
  `body.contains("cargo +")` test, and a fixture using
  `cargo +nightly fuzz run parse`).

**Argue whether the pin is worth that machinery.** A reasonable answer is
"float, and D6 plus a named failure is the currency step," but then a red job
from nightly drift and a red job from a real crash look the same on the run
summary — say how a reader tells them apart. Whichever you choose, the job
must also stop running `apt-get install rustup` and then an unpinned
`cargo install cargo-fuzz` without saying why those are acceptable. Either
lock `cargo-fuzz` (`--locked --version`), or say why not.

## 4. R4 — the job, and a defect the RFC does not name

**The job's last step claims to upload crash artifacts and does not.** It
uploads `fuzz/corpus/${{ matrix.target }}/`. cargo-fuzz writes a crashing input
to `fuzz/artifacts/<target>/`, which nothing uploads — so the first real crash
on CI would turn the job red and **discard the one input that explains it**.
Recorded in E-043 today. Upload the artifacts directory; D8's demonstration
must show the crashing input **downloaded from the run** and reproducing
locally, not only a red job.

Also in R4:

- **D3's corpus wiring**: seeds are in `fuzz/corpora/<name>/`, while cargo-fuzz's
  default corpus directory is `fuzz/corpus/<target>/`. Either pass the seed
  directory explicitly, or rename, but **`fuzz/corpora/`'s names do not match
  the target names** (`semantic_record` vs `semantic_record_parse`). Decide,
  and say what happens to the grown corpus after a run (D3's "stated choice").
- **The matrix is derived from the targets, not hand-listed**, or a check
  proves the two agree. The hand list is how a target retired in `Cargo.toml`
  stays in CI, or the reverse — E-014's family again.
- `actions/upload-artifact@v4` beside `actions/checkout@v7` — confirm v4 is
  current rather than assume the mismatch is deliberate.

## 5. R5 and R6 — what counts as observed

**D8, on real runs, each with a run id:**

1. **Dispatch works**: a `workflow_dispatch` run that runs the fuzz job — not
   one where it is skipped. Show the per-job conclusion, not the run's.
2. **A crash turns it red**: a temporary target (or a temporary `panic!` on a
   byte pattern seeded in the corpus) that fails the run. The crashing input
   uploaded, downloaded, and reproduced with `cargo fuzz run <target> <file>`.
   Revert.
3. **Shape 2 if chosen**: a target broken so it does not compile, on a push
   run, turns the build job red. Revert.

**R6 — E-043's closure condition:** one run, per kept target, conclusion
`success`, having actually executed for the configured time. **Read the log
for libFuzzer's `Done N runs in M second(s)` line** and paste it with N and M
per target. A job can be green because the step ran zero iterations — a
missing corpus directory, an empty target, a `-runs=0` — and "the job
passed" is not evidence it fuzzed.

## 6. R7 — criterion 9 reads scheduled runs

In both `docs/src/release/v0-release-cycle.md` (the "Criterion 9 is read from
the run" paragraph) and `docs/src/release/release-handoff.md` (the step that
records CI): the latest `schedule` run's id, its date, and its per-job table
beside the release commit's push run.

State the rule for **no scheduled run since the last cut**, and for a
scheduled run **older than the release commit** — which is the normal case,
and exactly the gap D5's dispatch closes: a cut can dispatch the job against
the release commit rather than read last Monday's run of an older tree. Say
whether a cut should, and write it as a step, not a suggestion.

## 7. R8 — claims

Every claim E-043 lists: ADR-v0.6-003's three sentences, `v1-readiness.md`'s
row (replace the `≥ 4` count, RFC R8), `what-is-fjell.md` and `overview.md`
(fuzzing was removed from their test tiers on 2026-09-15; restore it only as
far as R6 supports), and RFC-v0.6-003's `Implemented-with-Errata` status line
if E-043 closes.

## 8. Prohibited shortcuts

- **Do not count targets anywhere as evidence.** Count decoders exercised.
- Do not keep a target "for later". Retire it with its reason in R3's table.
- Do not write a decoder, or make a private one public, so a target can exist.
  An unreachable decoder is recorded, not restructured (Non-goals).
- Do not fuzz `reassemble`, the boot-control block or the store superblock.
- **If a real run finds a real crash**: record it as an erratum. Fix it here
  only if the fix is inside the decoder and small; otherwise stop and escalate.
  Do not narrow the target or the corpus until the crash goes away.
- Do not write "fuzzed in CI" without a run id and a `Done N runs` line beside
  it.
- Do not filter the badge. Do not `continue-on-error`.
- Do not touch kernel, ABI or service source.
- Do not run `cargo fmt --all --check` in your head.

## 9. Required evidence

1. R1: the three defects reproduced; each "never existed" claim checked at
   `e63d19f`; **the full decoder inventory** with source, reachability and the
   control that shows the inventory probe can find a known decoder. Every
   disagreement with the RFC's tables.
2. §7 and §8 answered in writing, engaging the arguments in §2 and §3.
3. R3's table — kept, retired, added — each with its reason.
4. The job: dispatch admitted, corpus wired, crash artifacts uploaded, matrix
   derived or checked.
5. D8's demonstrations, each with a run id; the crash input reproduced from a
   downloaded artifact.
6. **R6: per kept target, a run id and its `Done N runs in M second(s)`
   line.**
7. R7 in both documents, with the stale-scheduled-run rule.
8. R8's claims corrected; `v1-readiness.md`'s row without a count.
9. E-043 CLOSED or its survivors named; `v1-limitations.md` in the same commit.
10. `release-rehearsal` green; `consistency-check --all` 11/11 **with its exit
    status**, not a grep of its output; `cargo fmt --all --check`.

## 10. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The run id of R6's green run, and the `Done N runs` lines**, first.
- **The decoder inventory**, and any decoder you think should be fuzzed that
  this line cannot reach.
- **Your §7 and §8 answers**, and whether shape 2 is the cheap option.
- **Any crash a real run found**, and what you did with it.
- Anything in the fuzz job beyond the crash upload that turned out never to
  have worked.
