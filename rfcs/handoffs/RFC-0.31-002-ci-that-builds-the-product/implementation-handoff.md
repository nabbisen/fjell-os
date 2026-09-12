# Developer Handoff — RFC-0.31-002

**Governing RFC:** [RFC-0.31-002](../../done/RFC-0.31-002-ci-that-builds-the-product.md)
**Milestone:** 0.31
**Status:** inherited from the governing RFC (Implemented, 0.31.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The one rule this line is about

**Nothing about CI is true until you have read the run.** Not the YAML, not
the diff, not the commit message, not the badge — the run, opened with
`gh run view`, per job. Every false sentence E-041 lists was written by
someone who had read `ci.yml` carefully and no run at all. The last RFC's
submission did it too, while quoting the rule against it.

Your review request will be judged first on whether every claim in it about
CI cites a run id.

## 0.1 Read the runs before you read the workflow

```
gh run list --workflow ci.yml --limit 5 --json databaseId,headSha,conclusion
rid=<latest>
gh run view "$rid" --json jobs --jq '.jobs[] | "\(.conclusion)\t\(.name)"' | sort
gh run view --job <job-id> --log | grep -E 'error|FAILED|Process completed' | head
```

Expect **28 failures, three causes** (RFC Finding 1). Re-derive the table:
which jobs, which cause, from the log line. If your count differs from 25/1/1,
report it — E-041's first draft said "one cause" on the strength of eight logs
and was wrong the same day.

## 0.2 Settled — do not re-open

1. **rustup, from `rust-toolchain.toml`, via one composite action** (D1).
   There is no apt package that ships `library/Cargo.lock`; do not look for
   one.
2. **The action fails closed** (D2): file present, `rustc -vV` `release`
   matches the channel, *before* any build step. RFC-0.30-003's objection to
   rustup is answered by this check, not by avoiding rustup.
3. **`toolchain-declarations` inverts for CI** (D3): zero versioned mentions
   in `ci.yml`, every building job on the action. Four doc sites unchanged.
4. **Green is observed** (D4). The closure condition is a run id.
5. **`m1`–`m6` leave the matrix here** (D5), proven by `m7`/`m8` going green
   in the same run.
6. **Causes 2 and 3 are fixed, not re-named** (D6).
7. **The cycle reads CI** (D7): a step, a record shape, a handoff step.
8. **Do not pin the channel.** This line removes the obstacle; the decision is
   E-037's, argued separately.

---

## 1. Order

**Runs read (§0.1) → R1 measure one job → §6 answered → composite action →
D2 check → convert every building job → D5 matrix → D6 causes 2 and 3 →
D3 gate inverted → D4 observe green → D7 cycle step → D8 demonstrations →
R8 errata → evidence.**

Two things about this order are not negotiable:

- **R1 before §6, and one job before thirty.** Convert `ci-repro-check` (a
  single, non-matrix, build-std job) first. Push. Watch it. If it is not
  green, nothing else is converted until it is — a broken change and a
  working one are indistinguishable on a red CI, which is the entire history
  of this workflow.
- **D3 after conversion, not before.** The gate as it stands fails on zero
  apt mentions; invert it in the same commit that removes the last block, or
  Gate 12 goes red on a tree that is correct.

## 2. R1 — measure before §6, and write the number down first

`ci-repro-check`, converted, uncached: the "Install toolchain" step's
duration from `gh run view --job`, against the apt step's duration in the run
before. Then §6. If uncached rustup is under a minute per job, say so and lean
on shape 1 until it isn't; RFC-0.30-001's "doubles CI time" was 4–7 seconds
once someone timed it. If it is minutes, shape 2 with a key that **includes
`rust-toolchain.toml`'s hash** — a key that omits it serves the old
toolchain to a new declaration, silently, which is E-037 wearing a cache.

Shape 3 (a thinner component set in CI than the file declares) needs a
better argument than cost. It re-creates a second declaration.

## 3. D2 — the check is the whole reason shape 1 is safe now

The composite action, before any build:

```
test -f rust-toolchain.toml || { echo "rust-toolchain.toml missing"; exit 1; }
want=$(grep -oP 'channel\s*=\s*"\K[^"]+' rust-toolchain.toml)
have=$(rustc -vV | grep -oP '^release: \K.*')
case "$have" in "$want"*) ;; *) echo "toolchain $have != declared $want"; exit 1;; esac
```

(Illustrative; the shape matters, not the text.) Prefix-match today because
the channel floats within `1.91.x`; note in the action that an exact pin
makes this `=`. **This is the local `toolchain-declarations` failure on a
missing file, reproduced on CI.** Without it, rustup on a missing file
resolves to the runner's default `stable` and goes green — RFC-0.30-003's
exact objection, and it was right.

## 4. D3 — inverting my gate

`toolchain_declarations.rs:92` fails when `ci.yml` has no versioned
`rustc-`/`cargo-` mention. After this line that is the *correct* state.
Invert: any such mention is a failure naming the line (the apt block came
back), and additionally every job that runs `cargo xtask build`,
`qemu-test`, `qemu-negative`, `two-build-check`, or checks the RISC-V target
must reference the composite action. Derive the "building job" set from the
`run:` lines, not from a hand list of job names — the hand list is E-014's
family and this crate has removed three of them this month.

The four documentation sites (`local-development.md` ×2, `quick-start.md`,
`release-checklist.md`) stay compared against the channel; they are still
live declarations for a human.

## 5. D6 — causes 2 and 3

- **`test-v07-formats`**: RFC-0.29-001 wrote the fix and did not apply it —
  `--features fjell-sxt-crypto/crypto-profile-development`, the same flag
  `ci-host-bins` already passes. Apply it. Green or explain.
- **`proptest`**: `error[E0405]: cannot find trait Strategy` ×10. Find out
  what the job invokes and why it does not compile when `cargo xtask
  test-all`'s tier "passes". Either it compiles and runs real property tests
  (say how many), or the job is deleted with the reason in the commit and
  the erratum — a job that has never compiled is not coverage, and leaving
  it red-but-ignored is what this whole erratum is.

## 6. D4 and D7 — the two scopes of the same rule

**D4, this line**: every service-building job green, seen with `gh run
view` on a run of a commit in this line. Paste the per-job table with the
run id into the review request. If any job is red, the line is not done;
say which and why.

**D7, every cut after**: before the tag, the release commit's run — id, per-job
conclusion — goes in the release record. A red job blocks the tag or takes an
accepted-risk statement under the existing rule. Add the step to
`release-handoff.md` §1 (after the record's own consistency re-run, before the
clean-clone check) and to `v0-release-cycle.md`, and **rewrite line 345** —
*"It adds no CI enforcement"* — to say what the cycle now does with CI and
what it still does not (the gates stay local; CI is recorded, not deferred
to).

## 7. D8 — demonstrated failing, both on real runs

1. **D2**: a branch or commit with `rust-toolchain.toml` moved aside. CI red,
   the action's own message naming the file, **before any build step ran**.
   Revert. Cite the run id.
2. **D3**: one apt block reintroduced locally; `toolchain-declarations` red
   naming the line. Revert; `git status` clean.

## 8. R8 — the errata, argued

- **E-041 → CLOSED** on D4's run id.
- **E-037**: its consolidation survivor closes with D1 (twenty-two places →
  five; say the new count and where). Its pinning survivor is **re-stated**:
  the apt-naming collision RFC-0.30-003's review recorded no longer exists,
  because rustup accepts `channel = "1.91.1"`. Then **argue whether E-037
  closes entirely.** It probably does not — the channel still floats and
  that was E-037's own closure bar — but say so in the entry's own words
  rather than leaving it for the reader.
- `v1-limitations.md` in the same commit, for every erratum touched.

## 9. Prohibited shortcuts

- **Do not write "runs in CI" anywhere without a run id next to it.**
- Do not convert thirty jobs before one is green.
- Do not `continue-on-error` your way to a green badge.
- Do not delete `proptest` without finding out what it was supposed to run.
- Do not hand-list the building jobs in the D3 gate.
- Do not pin the channel; do not touch the Verus or nightly toolchains.
- Do not touch the kernel, the ABI surface, or any service — the feature flag
  in §5 is a job's command line, not a crate.
- Do not run `cargo fmt --all --check` in your head.

## 10. Required evidence

1. The 28-job cause table, re-derived from logs, with the run id.
2. R1's number, and §6 answered in writing from it.
3. `.github/actions/<name>/action.yml` (or equivalent), and `ci.yml` with
   zero `rustc-<v>`/`cargo-<v>` mentions.
4. **D4: the per-job table of a green run, with its id.** The closure
   condition.
5. D8's two demonstrations, the first with a run id.
6. D6: `test-v07-formats` green; `proptest` green with a test count, or
   removed with the reason.
7. D7: the cycle step, the handoff step, and `v0-release-cycle.md:345`
   rewritten.
8. `toolchain-declarations` inverted and green; its demonstration.
9. E-041 CLOSED; E-037 argued; `v1-limitations.md` updated.
10. `test-all` all tiers locally (unchanged — the gates stay local);
    `release-rehearsal` green; `consistency-check --all` 11/11.
11. `cargo fmt --all --check`.

## 11. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The run id of the green run**, first line.
- **R1's number** and whether §6 followed from it or from preference.
- What `proptest` turned out to be.
- **Whether E-037 closes**, in your own words.
- Anything in `ci.yml` that turned out never to have worked, beyond the 28.
