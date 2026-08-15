# RFC-0.24-002: Instrument Repairs — the seven that cannot wait for the cut

**Status:** Accepted — by the owner (nabbisen), 2026-08-03; implementation may begin (RFC 000)
**Milestone:** 0.24 — **blocks the cut.**
**Tracks.** Repair of instruments the 0.24 audit demonstrated do not check what
they claim.
**Touches.** `crates/fjell-tools` (release_rehearsal, smoke, negative),
`tools/fjell-unsafe-audit`, `tools/fjell-abi-snapshot`, `.github/workflows/ci.yml`.
No kernel, ABI, capability, lease, IPC, or crypto behaviour.
**Relates to:** RFC-0.24-001 (the audit that found these — its non-goal was
fixing, and this is where the fixing happens), RFC-v0.22-001 (whose governing
principle every slice here must satisfy), ERRATA E-013.

## Summary

RFC-0.24-001 audited 55 instruments across four passes and raised **37
findings** against **15** sound. Most are dispositioned to a deferred family or
to the audit's close-out.

**Seven are not deferrable.** Each is a small change to an existing instrument,
each is contingent on nothing, and each already has a demonstration in hand from
the audit. Until they land, the 0.24 release would be cut using gates the same
release line just proved do not check what they say.

One of them — the `unsafe-audit` category miss — is **live in the committed tree
today**, not a constructed break.

## Motivation

### Why these seven and not the other thirty

The line drawn across four passes was **contingency, not severity alone**: a
finding belongs here if fixing it depends on nothing the audit had not yet
examined. The deferred family — Gates 5/6/7, `FORBIDDEN`'s literal miss, the
TOML bracket truncation, the stale category lists, the CI crate-list and matrix
gaps — are all literal-string-matching or list-completeness problems that want a
single considered answer, not seven separate patches.

There is a second, sharper distinction worth stating because it decided two
close calls:

> These seven are instruments that **report success without checking**.
> The deferred ones are mostly checks that **do not run**, or that check
> narrowly and say so.

A missing CI matrix entry (`semantic`) is a gap in coverage. A gate that prints
`PASS` over a non-compiling workspace is a false statement. Only the second kind
blocks a cut.

### What "blocks the cut" means concretely

At the 0.23.0 cut, the release was signed against `release-rehearsal`'s twelve
gates. We now know Gate 1 passes on a workspace that does not compile, Gate 2
ignores its own tool's category verdict, and the ABI gate reads a reformatted
baseline as empty. Cutting 0.24 without repairing those means the release record
would cite evidence this same milestone documented as unreliable.

## Scope — seven slices

Each slice is independently reviewable and independently revertible.

### Slice 1 — exit status where it was discarded

**Finding (Pass 1, Pass 4).** `release_rehearsal.rs`'s `sh()` returns combined
stdout+stderr text and **never reads the process exit status**. Gate 1's entire
verdict is:

```rust
let out = sh(&["cargo","test","--workspace","--lib","--exclude","fjell-proptest"]);
let failed = out.contains("FAILED") || out.contains("test result: FAILED");
(!failed, "host lib tests".into())
```

A compile error never reaches "running tests", so it prints neither substring
and the first mechanical gate reports `PASS`. Demonstrated in Pass 1: underlying
`cargo test` exit 101, gate `PASS`.

**The correct pattern already exists one file away.** `test_all.rs`'s
`capture_command()` returns `(o.status.success(), combined)`. That contrast is
why several tiers are sound while their gate counterparts are not.

**Required.** `sh()` — or a sibling — surfaces exit status, and **Gate 1 and
Gate 2** consume it. Gate 2 is included because Slice 3 makes `unsafe-audit`
exit non-zero on a condition Gate 2's current predicate
(`out.contains("missing comment    : 0")`) cannot see; without this, Slice 3's
fix would not reach the gate.

**Out of scope:** Gates 3–8's predicates. They are the deferred
literal-matching family and are not made worse by this slice.

**Demonstration.** Reintroduce the Pass 1 break (syntax error in
`fjell-store-model`, chosen because `fjell-tools` does not depend on it, so the
rehearsal binary still builds). Gate 1 must report `FAIL`.

### Slice 2 — the silent milestone catch-all

**Finding (Pass 2).** `smoke.rs:32`:

```rust
_ => ("m8", "TEST:M8:PASS"), // default = current milestone
```

A typo'd or unknown milestone runs `m8` and reports its marker. The instrument
runs *something*, succeeds, and never confirms it was the thing asked for — the
same shape as Gate 1, which is why the audit grouped them.

**Required.** An unrecognised milestone is an error, not a default.

**Correction to the Pass 4 review record.** That record folded
`fjell-unsafe-audit`'s `from_str` catch-all (`_ => Self::Unknown`) into this
slice as "same shape, same fix." On closer reading it is the same *shape* but
**not** the same fix: `Unknown` is a legitimate value to compute, and the defect
is that nothing enforces it. It moves to Slice 3, where enforcement lives. The
arm itself stays.

**Demonstration.** `cargo xtask qemu-test m8-typo` must fail, naming the unknown
milestone, rather than running `m8`.

**Note.** `smoke.rs:29` supports a `"v0.6-verification"` milestone that appears
in no CI matrix and not in `SMOKE_PROFILES` — invoked by nothing, anywhere.
Likely vestigial. **Leave it**: removing it is a judgement about intent, not a
repair, and it belongs to the close-out.

### Slice 3 — `unsafe-audit` enforces the check its callers are named for

**Finding (Pass 4) — live in the committed tree.** On the untouched tree:

```
$ cargo run -p fjell-unsafe-audit -- --workspace . --check
  total unsafe sites : 284
  with valid category tag: 283
  MISSING/UNKNOWN category: 1
  missing comment    : 0
exit=0
```

`main.rs:363` is `if check && missing > 0 { process::exit(1); }`. `missing_cats`
is computed, printed, and never enforced. The offending site is
`examples/three-node-fleet/fjell-hello/src/main.rs:46`, tagged
`category=asm-instruction`, which is not one of the seven valid categories
(`csr-asm` is the near one).

Three consumers pass over it: CI, whose step is **named** *"Unsafe audit
(category= check)"*; `release-rehearsal` Gate 2; and `test-all` Tier 3.

**Second finding, same job.** CI runs `--root crates`, while Gate 2 and Tier 3
run `--workspace .`. A real `unsafe` site exists outside `crates/` — the same
file — so CI's scope misses violations both local instruments catch.

**Required.**

1. `--check` exits non-zero when `missing_cats > 0`.
2. `MISSING/UNKNOWN category` is reported in the `--json` branch too; today it
   prints only in the human branch, so JSON consumers cannot see it at all.
3. CI's invocation becomes `--workspace .`, matching Gate 2 and Tier 3.

**Sequencing — read this before starting.** Implement (1) first and run it. The
tree goes **red** on real committed input. **That red is the demonstration**, and
it satisfies RFC-v0.22-001's requirement that every strengthened gate be observed
failing — without constructing anything. Capture it, *then* correct the tag to
`csr-asm`, then confirm green.

**Do not correct the tag first.** Doing so would throw away the only
naturally-occurring demonstration in the entire audit.

### Slice 4 — a listed category with no profile must not pass

**Finding (Pass 2).** `negative.rs`: when no profile file exists, a category
still listed in `KNOWN_V01X_CATEGORIES` / `KNOWN_V02_CATEGORIES` falls to
`Profile::negative_placeholder(category)` — `timeout_secs: 1`,
`expected_markers: vec![]`. `lease` and `evidence` are both listed and neither
has a profile, so `cargo xtask qemu-negative lease` **passes against an empty
expectation set**: the RFC 025 placeholder behaviour v0.19 was supposed to have
retired.

Not reachable from `test-all` (which runs the ten categories that do have
profiles). Reachable by anyone invoking those two directly. Mode 3, fail-open on
absence.

**Required.** A category with no profile is an error. The placeholder path is
removed, not merely bypassed.

**Out of scope.** Whether `lease` and `evidence` *should* have profiles, and
whether the category lists are stale — that is the deferred stale-list item.
This slice makes their absence loud.

**Demonstration.** `cargo xtask qemu-negative lease` must fail naming the
missing profile. Verify `test-all`'s ten categories are unaffected.

### Slice 5 — the ABI baseline must not parse to nothing

**Finding (Pass 3).** `load_snapshot()` is line-oriented, not a parser:

```rust
// Minimal JSON parser: each line is one item object
for line in content.lines() {
    let line = line.trim().trim_end_matches(',');
    if !line.starts_with('{') { continue; }
```

`tests/abi/snapshot.json` holds 404 items, one per line, by convention only.
Reformatting it — pretty-printing, a merge, an editor writing single-line JSON —
yields `Baseline items : 0`, every current item reads as "Added — OK", and
**removals and signature changes become undetectable**. Total silent bypass of
the gate most responsible for ABI drift.

**Required — stated as a property, not an implementation.** *A snapshot file
that does not parse completely must fail the gate, never read as empty or
partial.* Two failure shapes must both be caught:

- **Total:** a non-empty file yielding zero items.
- **Partial:** a file yielding *some* items but not all of them — the more
  insidious case, and the one a zero-check alone would miss.

The obvious route is for the snapshot to carry its own item count and for
`--verify` to require parsed == declared, but the implementer chooses. If the
chosen route changes the committed artifact's format, regenerate it in the same
slice and say so.

**Demonstration.** Both shapes, separately:
`jq -c . tests/abi/snapshot.json` (single-line) → `--verify` must FAIL; a file
truncated mid-array → `--verify` must FAIL.

### Slice 6 — `ci-proptest` must actually run the property tests

**Finding (Pass 4).** `cargo test -p fjell-proptest --lib` → **`running 0
tests`**. `fjell-proptest` has no `proptest!` invocation in `src/`; all 24 live
in `tests/` (`harness.rs` 10, `verus_lemma_properties.rs` 14), which `--lib`
excludes.

Gate 1 and Tier 1 `--exclude fjell-proptest` explicitly, and
`release-rehearsal` has **no proptest gate at all**. So the only automated path
running them is a manually-invoked `test-all` tier 2 (`--release`, no `--lib`).
On every push and PR, the job named "Property tests" is green having run
nothing — including the 14 `verus_lemma_properties` cases that cross-check the
Verus proofs behind capability 8/8 and lease 5/5.

**Required.** Drop `--lib` for `fjell-proptest` in `ci-proptest`. The other two
crates in that job (`fjell-store-model`, `fjell-bootctl-model`) hold their
`proptest!` blocks in `src/` and are correctly covered; leave them.

**Out of scope.** Whether `release-rehearsal` should have a proptest gate at
all. It is a real question — 24 property tests including the Verus cross-checks
run in no release-time instrument — but adding a gate is **adding an
instrument**, and that is RFC-0.24-001's non-goal and this RFC's. It goes to the
close-out as a 0.25 candidate.

**Demonstration.** Run the corrected command; it must report 24 tests, not 0.
Then break one property and confirm the job fails.

### Slice 7 — `ci-schema-gate` stops claiming what it does not do

**Finding (Pass 4), two layers.**

The step is named *"Verify frozen schemas have not drifted."* Its script checks
only `[ -s "$f" ]` — exists and non-empty. Replacing `intent-v1.frozen`'s entire
contents with an unrelated sentence passes, exit 0.

Worse, and found in review: with the `.frozen` files **deleted**, `find` returns
nothing, the loop body never runs, and `schema-gate: all *.frozen files present
and non-empty` prints with exit 0 — while the step's own echo says it is
*"checking `*.frozen` files are committed"*. Mode 3 beneath mode 4.

The step also carries a comment describing **two behaviours it does not have**:
BREAKING-SCHEMA marker scanning on PRs, and frozen-counterpart matching on push
to main. Neither is implemented. The name is a false claim and the comment is
two more.

**Required.**

1. Rename the step to what it does — presence and non-emptiness.
2. Delete the two comment lines describing unimplemented behaviour. A pointer to
   the close-out item may replace them.
3. Fail closed on absence: check an expected set of paths rather than iterating
   whatever `find` happens to return.

**Explicitly out of scope: writing the drift check.** That is a new instrument —
the comment itself says the real tooling "lands with the `fjell-tools` schema
subcommand," which does not exist. Building it here would breach this line's own
boundary while repairing a finding about false boundaries. It is a 0.25
candidate. **This slice removes a false claim; it does not add a capability.**

**Demonstration.** Delete a `.frozen` file → the script must FAIL. Then confirm
the corrupted-content case still passes and that the new step name does not claim
otherwise.

## Non-goals

- **The deferred family.** Gates 5/6/7, `FORBIDDEN`'s `"TEST:FAIL"` miss, the
  TOML bracket truncation, the stale category lists, the 19-crate CI gap, the
  `semantic` matrix gap, `v0.6-verification`. All go to the close-out.
- **Adding any instrument.** Not the schema drift check, not a
  `release-rehearsal` proptest gate, not a link-and-count checker for
  `rfcs/README.md`. All are 0.25 candidates, and the boundary matters more than
  any one of them.
- **E-013's fix.** Architectural, still its own item.
- **Rewriting the gate or tier harness.** Slice 1 adds exit-status handling to
  two gates; it does not restructure `sh()`'s callers.
- Any kernel, ABI, capability, lease, IPC, or crypto behaviour.
- Gate 12 `syscall-surface` must stay **35/26/9**.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Fixing Gate 1's exit code turns the tree red for unrelated pre-existing reasons | Medium | Medium | That is the gate working. If it fires on something real, **stop and escalate** — do not weaken the predicate to get green. |
| R2 | Scope creep: 37 findings are one edit away and each looks small | **High** | **High** | Seven slices, enumerated. The non-goals are explicit. RFC-0.24-001's R2 is the precedent: the audit nearly went unbounded twice. |
| R3 | Slice 3's red tree is "fixed" by correcting the tag before capturing the failure | Medium | Medium | Called out in the slice. It is the only naturally-occurring demonstration in the audit; losing it costs the evidence, not just the anecdote. |
| R4 | Slice 5's format change invalidates the committed baseline | Medium | Medium | Regenerate in the same slice, state it in the review request, and re-run Gate 4 both before and after. |
| R5 | A slice is demonstrated with a test that passes for the wrong reason | Medium | High | Every demonstration must be observed **failing first**, per RFC-v0.22-001. A demonstration only ever seen green is not a demonstration. |

## Acceptance criteria

- [ ] All seven slices implemented, each independently revertible.
- [ ] **Every slice observed failing on a deliberately broken input, before the
      fix and after** — the RFC-v0.22-001 standard, applied to each.
- [ ] Slice 3's live failure captured on the **untouched** tree before the
      `asm-instruction` tag is corrected.
- [ ] `cargo xtask release-rehearsal` green, Gate 12 still **35/26/9**.
- [ ] `cargo xtask test-all` — all 19 tiers, with tier 2 unaffected and the ten
      negative categories unaffected.
- [ ] `cargo fmt --all --check` clean. *(Named explicitly: omitting it from
      RFC-v0.23-002's evidence list surfaced a fmt failure at release prep.)*
- [ ] `docs/verification/instrument-audit.md` updated — each repaired
      instrument's row moves from `finding` to `sound`, citing the
      demonstration.
- [ ] No instrument added. No finding outside these seven touched.

## What this does not resolve

After this lands, **30 findings remain open** and the instruments carrying them
are still green. That is the intended state: they are dispositioned, recorded,
and scheduled, not forgotten. The close-out (mine, separate) assigns each to an
erratum or a 0.25 candidate.

Worth stating plainly in the release record when 0.24 is cut: this milestone
made the instruments *more* honest, not honest.
