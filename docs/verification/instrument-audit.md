# Instrument Audit Register

**Governing RFC:** [RFC-v0.24-001](../../rfcs/proposed/RFC-v0.24-001-instrument-audit.md)
**Handoff:** [implementation-handoff.md](../../rfcs/handoffs/RFC-v0.24-001-instrument-audit/implementation-handoff.md)

One register, appended to as each pass lands (handoff §0.2.1). One row per
instrument: Claim, Actual, Modes it could exhibit (of the RFC's five —
1 scope blindness, 2 proxy attestation, 3 fail-open on absence, 4 weak
predicate, 5 stale assertion), and a status:

- **sound** — a demonstration was run and the instrument correctly failed on
  a deliberately broken input.
- **finding** — a demonstration was run and the instrument did **not** fail
  when it should have. Findings are reported here, not fixed (RFC §Non-goals).
- **UNAUDITED** — no demonstration was attempted or possible; reason given.
  Never conflated with `sound`.

All demonstrations were run against a temporarily broken working tree and
reverted immediately after capturing the result; `git diff --stat` confirmed
clean before moving to the next instrument.

---

## Pass 1 — the twelve release-rehearsal gates

Source: `crates/fjell-tools/src/release_rehearsal.rs`. Per handoff §2, gates
2/3/4/11/12 were touched by RFC-v0.22-001 and already carry demonstrations
(unit tests in the tools that implement them) — these were re-run to confirm
they still hold, not re-derived from scratch.

### Gate 1 — Host test suite (0 failures) — **finding**

- **Claim:** the host test suite has zero failures.
- **Actual:** runs `cargo test --workspace --lib --exclude fjell-proptest`,
  captures combined stdout+stderr, and passes iff neither `"FAILED"` nor
  `"test result: FAILED"` appears anywhere in that text. **The process exit
  code is never inspected.**
- **Modes:** 1 (scope blindness — a compile error never reaches "running
  tests", so it never prints either substring) and 3 (fail-open on absence —
  no test output at all reads as success).
- **Demonstration:** introduced a syntax error into `fjell-store-model`
  (chosen because it is not a dependency of `fjell-tools`, so the rehearsal
  binary itself still builds) and ran the real instrument:
  ```
  $ cargo xtask release-rehearsal
  [PASS] Gate 1  Host test suite (0 failures)     host lib tests
  ```
  Exit code of the underlying `cargo test` was 101 (compile failure); the
  gate reported PASS regardless. Reverted; `git diff --stat` clean.
- **A second, larger finding surfaced investigating this one:** `fjell-kernel`
  is not the only crate whose tests tier 1 silently skips (E-013). Of 91
  workspace packages, **10 declare `[[bin]]` with no `[lib]` and contain real
  `#[test]` functions that tier 1's `--lib` never reaches** — 166 tests total:
  `fjell-kernel` (30, already E-013), `fjell-tools` (68, including
  `callsite_audit`'s 18 — Gate 11's own demonstrations), `fjell-consistency-check`
  (26 — Gate 12's own demonstrations), `fjell-unsafe-audit` (10 — Gate 2's),
  `fjell-abi-snapshot` (8 — Gate 4's), `fjell-repro-check` (6),
  `fjell-readiness-check` (5 — Gate 5's), `fjell-ci-coverage` (4),
  `fjell-summary-check` (2), `fjell-mmio-audit` (7 — Gate 3's). Confirmed for
  `fjell-consistency-check`: `cargo test -p fjell-consistency-check --lib` →
  `error: no library targets found`; plain `cargo test -p fjell-consistency-check`
  (no `--lib`) runs and passes all 26. **Unlike `fjell-kernel`, these nine are
  ordinary host `std` binaries with no architectural obstacle** — the gap is
  the `--lib` flag alone, not a `no_std`/bare-metal constraint. Not fixed
  here (explicit non-goal); noted because it changes E-013's severity
  assessment and because it means gates 2/3/4/5/11/12's own regression tests
  — verified sound below — are *themselves* invisible to tier 1 for the same
  reason as the kernel.

### Gate 2 — Unsafe audit (0 missing) — **sound**

- **Claim:** every `unsafe` site in the workspace has a preceding `SAFETY:`
  comment.
- **Actual:** runs `fjell-unsafe-audit --workspace . --check`, greps its
  output for the literal line `missing comment    : 0`.
- **Modes considered:** 4 (weak predicate, if the comment-detection window
  were satisfiable by unrelated text) — checked, not present: the tool's own
  suite includes `unsafe_inside_string_literal_not_counted` and
  `unsafe_inside_raw_string_not_counted`, ruling out the obvious
  string-content false-negative.
- **Demonstration (RFC-v0.22-001, re-verified now):**
  ```
  $ cargo test -p fjell-unsafe-audit
  test tests::detects_unsafe_block_missing_safety ... ok
  test tests::detects_unsafe_fn ... ok
  ... (10 passed)
  ```
  Re-run today, all 10 pass, including the two that directly assert
  detection of a missing/absent safety comment.

### Gate 3 — MMIO audit (0 missing) — **sound**

- **Claim:** every MMIO access site carries a recognized ordering annotation.
- **Actual:** runs `fjell-mmio-audit --workspace . --check`, greps for
  `missing annotation : 0`.
- **Modes considered:** 4 — checked via `strip_does_not_flag_string_patterns`
  (a string literal containing annotation-like text is not miscounted as a
  real annotation).
- **Demonstration (RFC-v0.22-001, re-verified now):** `cargo test -p
  fjell-mmio-audit` — 7/7 pass, including `missing_annotation_returns_none`.

### Gate 4 — ABI snapshot verify — **sound**

- **Claim:** the committed ABI snapshot matches the current signatures of the
  ABI-stable crates.
- **Actual:** runs `abi-snapshot --verify`, checks for `Result: PASS` /
  `Result         : PASS` in the output.
- **Modes considered:** 4 — the tool's own tests
  (`realigned_whitespace_produces_no_signature_change`,
  `differently_indented_wrapped_declaration_produces_no_signature_change`)
  show it deliberately normalizes formatting-only diffs rather than being
  fooled by them in the *unsafe* direction; `genuine_signature_change_still_detected`
  confirms a real change is still caught.
- **Demonstration (RFC-v0.22-001, re-verified now):** `cargo test -p
  fjell-abi-snapshot` — 8/8 pass, including the genuine-change-detected case.

### Gate 5 — Readiness matrix (0 OPEN) — **finding**

- **Claim:** the v1.0 readiness matrix has zero items blocking release.
- **Actual:** parses `docs/release/v1-readiness.md`, counts table rows
  containing the exact literal `**OPEN**`; passes iff that count is zero.
  Only four status literals are recognized at all: `**DONE**`,
  `**IN PROGRESS**`/`IN_PROGRESS`, `**DEFERRED**`, `**OPEN**`.
- **Modes:** 1 (scope blindness) / 4 (weak predicate) — any other status
  text is invisible to the counter, not merely miscounted.
- **Demonstration:** inserted a row with status `**BLOCKED** — semantically
  open, not literally "OPEN"` and ran the real instrument:
  ```
  $ cargo run -q -p fjell-readiness-check
  DONE     : 55
  IN_PROGRESS: 0
  DEFERRED : 3
  OPEN     : 0
  Result   : PASS — zero OPEN cells
  ```
  The inserted row is counted in none of the four buckets — not flagged,
  not silently miscounted as DONE, simply absent from every total. A reader
  checking only "OPEN : 0 → PASS" would never notice the row exists.
  Reverted; `git diff --stat` clean.

### Gate 6 — Trust report (6 sections) — **finding**

- **Claim:** the trust report is complete (RFC 061 §6's six sections).
- **Actual:** runs `trust-report` to regenerate the file, discards the
  regeneration's own success/failure (`let _ = sh(...)`), then reads
  `docs/release/trust-report.txt` from disk and counts how many of the
  literal strings `§1`..`§6` appear; passes iff all six do.
- **Modes:** 3 (fail-open on absence) and 4 (weak predicate) — compounding.
  The six section headers are unconditional string literals the generator
  always writes (`trust_report.rs`, one `push_str` per section, no
  conditional path that omits one) whenever it runs at all, so the count
  check can only ever fail if generation crashes *and* leaves no prior file.
  If generation fails but a prior file exists, the gate reads the stale file
  and cannot tell the difference from a fresh, correct one.
- **Demonstration:** introduced a syntax error into `trust_report.rs` (the
  generator itself, so only this gate's regeneration path is affected) and
  ran the gate's exact two-step logic:
  ```
  $ cargo run -q -p fjell-tools -- trust-report
  error: could not compile `fjell-tools` ...
  $ cat docs/release/trust-report.txt   # unregenerated, pre-existing content
  Version   : 0.23.0
  # §1..§6 all present — 6 of 6
  ```
  Regeneration failed completely; the gate's check would still report PASS
  (6/6 sections) from the untouched, unrevalidated file. This is the same
  shape as the RFC's own founding example — `trust-report.txt` sat at
  `Version: 1.0.0` for months — this gate would not have caught that either,
  since the drift was in the *values*, not the *headers*. Reverted (source
  and the one incidentally-regenerated timestamp in the committed report);
  `git diff --stat` clean.

### Gate 7 — ERRATA register (0 OPEN) — **finding**

- **Claim:** the errata register has zero unresolved (OPEN) entries.
- **Actual:** `grep -c "| OPEN |" docs/rfcs/ERRATA.md` — an exact literal
  match against the three-column summary table's status cell.
- **Modes:** 1 / 4 — same shape as Gate 5. Additionally, the check only
  examines the **summary table**, never the per-entry prose sections above
  it (each entry's authoritative `**Resolution:**` line) — the two could
  disagree and nothing would notice.
- **Demonstration:** appended a summary-table row with status
  `OPEN (needs owner decision)` and ran the exact gate command:
  ```
  $ grep -c "| OPEN |" docs/rfcs/ERRATA.md
  0
  ```
  An annotated OPEN entry is invisible to the count precisely because it is
  not a bare `OPEN`, which is the realistic case — every existing OPEN-like
  entry in this file's history (e.g. E-012 before reclassification) carried
  qualifying text. Reverted; `git diff --stat` clean.

### Gate 8 — Validation drills (markers) — **sound**

- **Claim:** five specific validation drills (2 Ed25519 TV1 vectors, 2 fleet
  partition-drill markers, 2 SDK config-sync markers — five distinct
  positive checks, not five files) all pass.
- **Actual:** runs each test/drill individually and checks for **positive**
  evidence (`"1 passed"`, or the literal `DRILL:*:PASS` marker) rather than
  the absence of a failure keyword — a materially different, and better,
  shape than Gate 1.
- **Modes considered:** 2 (proxy attestation) — checked directly: both
  `partition_drill.rs` markers and both `runtime_trial.rs` markers are
  `println!`'d as the *last statement* of their test function, after every
  `assert!`/`assert_eq!` in that function, so a failed assertion panics
  before the marker prints and the test itself reports `FAILED` — not a
  marker that could fire independent of the property it names.
- **Demonstration:** corrupted `TV1_SECRET`'s first byte (reproducing
  E-001's exact original defect shape) and ran the exact gate command:
  ```
  $ cargo test -p fjell-sig-ed25519 --features sign from_seed_matches_tv1_public
  test result: FAILED. 0 passed; 1 failed; ...
  ```
  `"1 passed"` correctly absent; gate 8 would correctly report this marker
  missing. Reverted; `git diff --stat` clean.

### Gate 9 — Release-notes limitations — **UNAUDITED**

- **Claim:** `docs/release/v1-limitations.md` covers hardware, multi-hart,
  POSIX, kernel-IPC, ZeroizeOnDrop, and trust-anchor provisioning.
- **Actual:** printed as a manual reminder; not auto-checked by design
  (`release_rehearsal.rs` prints `[ -- ]`, never `[PASS]`/`[FAIL]`, for this
  line).
- **Reason UNAUDITED:** it is explicitly a human checklist item, per the
  handoff (§2): "Gate 9 is manual. It will be `UNAUDITED` with that as the
  reason, which is a legitimate outcome, not a gap." No demonstration is
  applicable — there is no automated check to break.

### Gate 10 — Verus release-required proofs — **finding**

- **Claim:** every release-required Verus proof target is
  machine-checked-proved.
- **Actual:** runs `verus-check --release-required`, which filters the
  target list to `release_required == true` and checks the exit status
  (a real improvement over Gate 1 — no substring matching here).
  **However:** if that filter selects zero targets, the check loop runs zero
  iterations, `any_blocking_fail` never becomes `true`, and the command
  exits `SUCCESS` — a release with **no proof targets at all** would report
  identically to a release where every target is proved.
- **Modes:** 3 (fail-open on absence) — the same shape as `repro-check
  --skip-build`'s missing-baseline auto-pass (already fixed for that
  instrument in RFC-v0.21.3-001), here in a sibling tool.
- **Demonstration:** the two current release-required targets (`capability`,
  `lease`) were temporarily flipped to `release_required = false` in
  `verification/verus/verus-targets.toml`, and the real gate was run:
  ```
  $ cargo run -q -p fjell-tools -- release-rehearsal
  [PASS] Gate 10 Verus release-required proofs  every release-required target MACHINE-CHECKED-PASS
  ```
  No `VERUS:TARGET:*` line was printed at all — zero targets were examined —
  yet the gate reported PASS with the same message it uses when both real
  targets are genuinely proved. Reverted; `git diff --stat` clean.
- Note: this gate's other fail-closed guards (pinned-version identity check,
  `no [[target]] entries found` hard error) are well-built and were not
  bypassed by this demonstration — the gap is specifically the
  zero-selected-targets path after a valid, non-empty target list exists.

### Gate 11 — Callsite conformance — **sound**

- **Claim:** every LEASE/CAP capability-check call site's static shape
  matches the proved predicate (Gate 11's original RFC-v0.22-001 subject —
  the substring-in-a-comment defect).
- **Actual:** runs `callsite-audit`, checks exit status.
- **Modes considered:** 4 (weak predicate — the original defect) and 1.
  Checked directly against the regression suite for the exact original
  failure: `cap_check_fails_when_is_subset_of_only_in_comment` and
  `lease_check_ignores_wrapping_add_mentioned_only_in_a_comment`.
- **Demonstration (RFC-v0.22-001, re-verified now):** `cargo test -p
  fjell-tools callsite_audit` — 18/18 pass, including both comment-only
  false-positive regression tests and
  `lease_check_fails_on_real_wrapping_add_in_revoke` (real-code detection).

### Gate 12 — Consistency check — **sound**

- **Claim:** declared repository state matches actual state across four
  sub-checks: syscall surface, errata/limitations binding, RFC status/folder
  agreement, handoff status inheritance.
- **Actual:** runs `consistency-check --all`, checks exit status.
- **Modes considered:** all four sub-checks have dedicated regression tests
  for their own historical defect (E-013's own filing exercised this gate
  directly, for instance — `errata_limitations.rs`'s tests assert an
  ACCEPTED erratum absent from `v1-limitations.md` fails).
- **Demonstration (RFC-v0.22-001, re-verified now):** `cargo test -p
  fjell-consistency-check` (26/26 pass) — includes
  `new_declared_syscall_not_in_expected_fails` and
  `stale_expected_entry_no_longer_in_source_fails` (syscall_surface),
  errata/limitations binding failures, and RFC-status/handoff-status
  mismatches, each with its own "fails on bad input" test.

### Pass 1 summary

- **Audited:** 12 / 12.
- **Sound (demonstration confirmed correct):** 6 — Gates 2, 3, 4, 8, 11, 12.
- **Findings (demonstration showed it does NOT fail when it should):** 5 —
  Gates 1, 5, 6, 7, 10.
- **UNAUDITED:** 1 — Gate 9 (manual by design).
- **Cross-cutting finding surfaced by Gate 1's investigation:** 166 `#[test]`
  functions across 10 `[[bin]]`-only crates — including the tools that
  implement Gates 2, 3, 4, 5, 11, and 12 — are silently invisible to
  `test-all` tier 1, for the same structural reason as E-013. Reported here;
  disposition deferred to whoever owns E-013's follow-up RFC, since it
  changes that RFC's scope estimate rather than this one's.

Required evidence per handoff §8, item 4 (`cargo xtask release-rehearsal`
still green and Gate 12 still 35/26/9 syscall-surface) captured in the
review request for this pass, not duplicated here.

---

## Pass 2 — the nineteen `test-all` tiers

Source: `crates/fjell-tools/src/test_all.rs`, plus the shared QEMU harness
(`qemu_run.rs`, `smoke.rs`, `negative.rs`) that fourteen of the nineteen
tiers delegate to. Per the ruling on Pass 1, tier 1 gets its own row here
rather than a cross-reference to Gate 1's, per the architect's note that the
register should stand alone.

A structural point that applies across this whole pass: `test_all.rs`'s own
`capture_command()` checks the real process **exit status**
(`o.status.success()`), not a substring of the output. This is a materially
different, and better, design than `release_rehearsal.rs`'s `sh()` +
string-match pattern audited in Pass 1 — most of this pass's tiers are sound
*because of* that difference, even where the underlying tool being invoked is
the same one Pass 1 already looked at.

### Tier 1 — Host library tests — **finding**

- **Claim:** the workspace's host-side unit tests all pass.
- **Actual:** `cargo test --workspace --lib --exclude fjell-proptest`,
  checked via real exit status (sound on that axis — see the pass note
  above; this is *not* Gate 1's flaw, which never inspected exit status at
  all).
- **Modes:** 1 (scope blindness) — inherited whole from E-013. `--lib`
  silently omits any package with no library target. 40 of 89 non-proptest
  packages have none; 10 of those carry 166 real `#[test]` functions
  (figures per the corrected count in E-013's widened entry).
- **Demonstration:** not re-run here (identical mechanism to Gate 1's
  already-demonstrated case; re-breaking `fjell-store-model` would reproduce
  the same skip, not a new fact). Recorded as its own row per instruction,
  cross-referencing E-013 for the underlying figures rather than duplicating
  the count derivation.

### Tier 2 — Property tests (proptest) — **sound**

- **Claim:** the fourteen `proptest`-based properties over capability rights
  and lease epochs all hold.
- **Actual:** `cargo test -p fjell-proptest --release`, exit status checked.
- **Demonstration:** temporarily forced `prop_zero_is_subset` to
  `prop_assert!(false, ...)` and ran the exact tier command:
  ```
  test result: FAILED. 0 passed; 1 failed; ...
  ```
  real exit code 101, correctly surfaced. Reverted; also removed the
  `.proptest-regressions` seed file the failing run generated (an artefact
  of the deliberate break, not a real regression worth keeping).

### Tier 3 — Unsafe site audit — **sound**

- **Claim:** every `unsafe` site has a preceding `SAFETY:` comment.
- **Actual:** `fjell-unsafe-audit --workspace . --check`, exit status
  checked. Read the tool's own `main()`: `if check && missing > 0 {
  process::exit(1); }` — a real, direct exit-code contract, not a string
  the caller has to parse (contrast Gate 2 in Pass 1, which greps this same
  tool's text output instead of reading its exit code — sound today, per
  Pass 1, but structurally more fragile than tier 3's own check of the
  identical tool).
- **Demonstration:** covered by Gate 2's demonstration in Pass 1 (same
  tool); not repeated.

### Tier 3c — MMIO ordering audit — **sound**

- **Claim:** every MMIO access carries a recognized ordering annotation.
- **Actual:** `fjell-mmio-audit --workspace . --check`, exit status checked
  (`ExitCode::FAILURE` on any missing annotation, read directly from source).
- **Demonstration:** covered by Gate 3's demonstration in Pass 1; not
  repeated.

### Tier 3b — Reproducible build (skip-build) — **sound**

- **Claim:** the committed `prebuilt/*.bin` artefacts match their recorded
  baseline digests.
- **Actual:** `fjell-repro-check --skip-build`, exit status checked.
- **Modes considered:** 3 (fail-open on absence) — this is the RFC's own
  worked example ("Tier 5 auto-recorded a missing baseline and passed, until
  v0.21.3"). Re-verified rather than assumed.
- **Demonstration:** moved `tests/repro/baseline-digests.txt` aside and ran
  the exact tier command:
  ```
  fjell-repro-check: FAIL — no baseline at tests/repro/baseline-digests.txt
  (this is not a passing state).
  ```
  exit 1. The RFC-v0.21.3-001 fail-closed fix still holds. Restored the
  file; `git diff --stat` clean.

### Tiers 4–7 — QEMU smoke: m8, v0.4-net, v0.5-platform, v0.7-sync — **finding** (shared mechanism)

- **Claim:** each names a milestone/subsystem and its serial-log marker
  (`TEST:M8:PASS`, `TEST:V0.4-NET:PASS`, etc.) attests that milestone
  completed — the *marker-identity* question RFC-v0.23-002 already settled.
  This pass's question is different: does the **runner** correctly map a
  requested profile name to the right marker at all, and correctly fail
  closed on a bad log?
- **Actual:** `smoke.rs` maps a milestone string to `(profile_id, marker)`
  via a `match`; `qemu_run.rs::run_profile` boots QEMU, then requires every
  expected marker present **and** none of a `FORBIDDEN` list present
  (`NEG:HARNESS:WRONG_ERROR`, `NEG:HARNESS:UNEXPECTED_OK`, `TEST:FAIL`,
  `kernel panic`, `panicked at`).
- **Modes:** 1 (scope blindness) — `smoke.rs`'s match has a catch-all:
  `_ => ("m8", "TEST:M8:PASS")`. Any unrecognized milestone name — a typo,
  a name for a milestone not yet wired up — silently runs the **m8**
  profile instead of erroring.
- **Demonstration:**
  ```
  $ cargo xtask qemu-test totally-bogus-milestone-xyz
  [xtask] running profile `smoke-m8` (timeout 60s)
  [xtask] profile `smoke-m8` PASS (1 marker(s) matched) ✓
  ```
  exit 0. A caller requesting a milestone that does not exist gets a clean
  PASS for a *different* milestone, with nothing in the output naming the
  substitution as anything other than the profile that was asked for having
  simply run.
- **Secondary finding, same shared code, smaller:** the `FORBIDDEN` literal
  `"TEST:FAIL"` does not match the one real fail-marker the kernel emits,
  `"TEST:M7:FAIL (init did not exit cleanly)"` — confirmed by direct
  substring check (`"TEST:FAIL" in "TEST:M7:FAIL (init did not exit
  cleanly)"` → `False`, the `:M7` breaks the match). In practice an M7
  failure is very likely also caught by a missing expected-marker check
  further down the same dependency chain (M8 cannot pass if M7 failed), so
  this looks currently masked rather than actively exploited — but the
  forbidden-marker guard specifically does not do the job its own comment
  says it does for the one message it was seemingly written to catch.
- **Third finding, same shared code, confirmed relevant to this pass:** the
  TOML array parser truncates `expected_markers` silently at the first
  embedded `]` (RFC-v0.23-001's already-reported, still-unfixed parser bug —
  re-demonstrated here with a disposable fixture profile rather than
  assumed still present):
  ```
  parsed 1 markers: ["[INTENT][Normal] first marker"]
  ```
  against a 3-marker fixture array. None of the four smoke profiles
  currently use bracket-containing marker text (the RFC-v0.23-001 workaround
  was exactly to avoid this), so this is not live for tiers 4–7 today, but
  the runner they share with tiers 8–17 has no such workaround available if
  a future marker needs bracket characters.
- Reverted: temporary test module and fixture file both removed;
  `git diff --stat crates/fjell-tools/src/qemu_run.rs` clean.

### Tiers 8–17 — QEMU negative: capability, mmio, dma, user-copy, audit, policy, ipc, svc, harness, semantic — **finding** (shared mechanism)

- **Claim:** each category's negative-test scenarios exercise a real
  fail-path and the run only passes if every expected `NEG:*:PASS` marker
  is present and no forbidden marker fired.
- **Actual:** `negative.rs::cmd_qemu_negative` delegates to the identical
  `run_profile`/`load_profile` machinery as tiers 4–7 when
  `tests/qemu/profiles/<category>.toml` exists (true for all ten today).
- **Modes:** 3 (fail-open on absence) — same TOML-truncation and
  `FORBIDDEN` gap as tiers 4–7, since it is the same code. Not re-run
  per-category (would reproduce the identical mechanism ten times); recorded
  once here and cross-referenced.
- **Additional, minor, mode-1 observation specific to this entry point:**
  `cmd_qemu_negative`'s `KNOWN_V01X_CATEGORIES` / `KNOWN_V02_CATEGORIES`
  lists — used only for the *placeholder* fallback path when no profile file
  exists yet — do not include `"harness"` or `"semantic"`, two of the ten
  categories `test-all` actually runs. Not currently live (both have real
  profile files, so the `Path::exists()` check short-circuits before the
  known-category list is ever consulted), but the list no longer describes
  the real category set, which is exactly the shape of a stale assertion
  (mode 5) waiting for the day one of those profile files is temporarily
  absent (e.g. a bad rebase) — at which point the category would be
  rejected as "unknown" rather than falling through to a placeholder.

### Pass 2 summary

- **Audited:** 19 / 19.
- **Sound:** 4 — Tiers 2 (proptest), 3 (unsafe audit), 3c (MMIO audit), 3b
  (repro-check).
- **Findings:** 15 — Tier 1 (E-013's `--lib` gap, its own row) and Tiers
  4–17 (fourteen tiers sharing the smoke/negative QEMU harness's three
  findings: silent milestone fallback to m8, a forbidden-marker string that
  doesn't match the one real message it names, and the still-unfixed
  TOML-array bracket truncation).
- **UNAUDITED:** 0.

The fourteen QEMU-tier findings are one underlying set of code defects, not
fourteen independent discoveries — recorded per-tier because each tier is
its own instrument per the RFC's framing, but the disposition question is
almost certainly "fix `qemu_run.rs`/`smoke.rs` once," not fourteen separate
items.

Required evidence: `cargo xtask release-rehearsal` re-run clean and Gate 12
still 35/26/9 after all Pass 2 demonstrations were reverted — captured in
the review request for this pass.
