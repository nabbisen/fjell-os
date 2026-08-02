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
