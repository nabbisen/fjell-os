# Instrument Audit Register

**Governing RFC:** [RFC-0.24-001](../../rfcs/done/RFC-0.24-001-instrument-audit.md)
**Handoff:** [implementation-handoff.md](../../rfcs/handoffs/RFC-0.24-001-instrument-audit/implementation-handoff.md)
**Close-out and disposition:** [instrument-audit-closeout.md](./instrument-audit-closeout.md)
— this file is the authoritative row-level record; the close-out disposes of
what it found (repairs, errata E-013/E-014/E-015/E-016, and 0.25 candidates).

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

### Gate 1 — Host test suite (0 failures) — **sound** (RFC-0.24-002 Slice 1)

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
- **Repaired (RFC-0.24-002 Slice 1):** `release_rehearsal.rs` gained
  `sh_status()`, a sibling of `sh()` that also returns the real process
  exit status; Gate 1 now consumes it instead of matching a substring.
  Re-ran the identical demonstration after the fix:
  ```
  $ cargo xtask release-rehearsal   # fjell-store-model still broken
  [FAIL] Gate 1  Host test suite (0 failures)     host lib tests
  ```
  then reverted the break and confirmed `[PASS]` on a clean tree. Gates
  3–8's predicates are unchanged — this slice touched Gates 1 and 2 only.
- **A second, larger finding surfaced investigating this one (still open,
  not this slice's to fix):** `fjell-kernel`
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

### Gate 4 — ABI snapshot verify — **sound** (RFC-0.24-003, R1–R6; citing Demonstration 1's signature-mismatch pathway, not the tool's unit suite)

- **Claim:** the committed ABI snapshot matches the current signatures of the
  ABI-stable crates.
- **Actual:** runs `abi-snapshot --verify`, checks for `Result: PASS` /
  `Result         : PASS` in the output.
- **Modes considered in Pass 1:** 4 — the tool's own tests
  (`realigned_whitespace_produces_no_signature_change`,
  `differently_indented_wrapped_declaration_produces_no_signature_change`)
  show it deliberately normalizes formatting-only diffs rather than being
  fooled by them in the *unsafe* direction; `genuine_signature_change_still_detected`
  confirms a real change is still caught.
- **Pass 1 "demonstration" (RFC-v0.22-001, re-verified then):** `cargo test -p
  fjell-abi-snapshot` — 8/8 pass, including the genuine-change-detected case.

> **Reversed in the RFC-0.24-002 review (2026-08-03) — identity-key collapse.**
> `verify()` keys both maps on `(crate_name, kind, name)`. **`module` is not in
> the key**, and both are built with `.collect()` into a `BTreeMap`, so
> **duplicate keys silently overwrite — last one wins.**
>
> ```
> items in baseline            : 423
> distinct keys after collapse : 378
> items never compared         :  45      ← 10.6% of the declared stable ABI
> ```
>
> Worst groups: `fjell-service-api const READY` ×10, `fjell-abi const fn` ×8,
> `fjell-service-api const ERR` ×7. **Seventeen collisions span different
> modules** — genuinely distinct items, e.g. `fjell-cap const fn` across
> `cspace`, `handle`, and `slot`.
>
> **Demonstration.** Corrupted the signature of a *shadowed* baseline entry —
> the first of ten `READY` rows — leaving everything else untouched:
>
> ```
> $ cargo run -p fjell-abi-snapshot -- --verify --snapshot <corrupted>
>   Baseline items : 423
>   Changed sig    : 0
>   Result         : PASS
> exit=0
> ```
>
> A corrupted signature inside the file the gate exists to protect, and the gate
> reports `PASS`. The removal case follows from the same collapse — `removed` is
> computed over collapsed keys, so a deleted shadowed item's key survives via a
> sibling — but only the signature case was demonstrated.
>
> **Modes:** 1 (scope blindness — 45 items are outside what it examines) and 4
> (weak predicate — the key does not identify an item).
>
> **Why Pass 1 missed it.** The row's "demonstration" was the tool's own unit
> suite passing. That is not the gate observed failing on a broken repository
> state; it is **mode 2, proxy attestation**, and it is precisely why the
> collapse stayed invisible — the unit tests use synthetic items that do not
> collide. This row is the architect's, approved in the Pass 1 review, and it is
> the **first of the 15 `sound` verdicts re-derived under the Pass 4 close-out
> item** — which found something on the first attempt.
>
> **Two adjacent defects, recorded, not repaired here:** 15 entries carry
> `kind:"const", name:"fn"` — extractor misparses capturing the literal token
> `fn` as a name; and 237 items carry `module:""`, so adding `module` to the key
> would not separate the nine empty-module `READY`s. Both go to the close-out.
>
> **Disposition — RFC-0.24-003**, which blocks the 0.24 cut (owner decision,
> 2026-08-03). Originally proposed as an eighth slice of RFC-0.24-002 on the
> architect's estimate that it was "a duplicate-key check plus one field in a
> tuple." **That estimate was wrong**, and sizing it found three further
> scanner defects that are what *make* the duplicates:
>
> | | Defect | Items |
> |---|---|---|
> | B | `pub const fn` parsed as a `const` named `fn` | **15** |
> | C | Inline `mod` blocks untracked — items take the file's path | **162** |
> | D | `storaged.rs` has no `mod` declaration anywhere; scanned anyway | **17 phantom** |
>
> Adding `module` to the key alone takes shadowing from 45 to 27 and leaves nine
> duplicate groups, so the gate would stay red. D is the inverse failure: the
> gate asserts ABI stability over code not compiled into the crate — and its
> cause is that `scan_dir` walks the *filesystem* as a proxy for the *module
> tree*, **mode 2 again**, in the same instrument.
>
> Row stayed `finding` until RFC-0.24-003 landed — see below.

> **Repaired — RFC-0.24-003, R2/R3/R4 first, then R1, then R6 (added in
> review of R1; see the two new rows below for how R6 was found).**
>
> - **R2** — `strip_fn_modifiers()` replaces enumerated prefix strings with
>   recognition of the general *modifiers-then-`fn`* shape (`const`,
>   `async`, `unsafe`, `extern "ABI"`, any combination). Fixes B (15 items)
>   and, as a byproduct, a second gap in the same class — see the
>   `pub unsafe fn` row below.
> - **R3** — inline `mod name { … }` blocks tracked via a whole-file
>   comment/string/char-literal stripper (braces inside any of them,
>   including a multi-line block comment, verified not to perturb depth)
>   plus a brace-depth walk; items take the qualified `parent::child` path.
>   Fixes C.
> - **R4** — the scanner starts at `lib.rs` and follows only the `mod`
>   declarations it actually contains (`mod NAME;` resolved to a sibling
>   file; inline blocks recursed into directly), replacing the directory
>   walk. Fixes D: `storaged.rs` — reachable by no `mod` declaration
>   anywhere — is confirmed never visited.
> - **R1** — `module` joins the identity key; `build_identity_map()`
>   returns every duplicate instead of a `BTreeMap` silently keeping the
>   last. Run first against the untouched-by-R1-alone tree, **as the
>   handoff specified, it did not pass**: two duplicates survived, neither
>   explained by B, C, or D — see the impl-scope row below for what that
>   found.
> - **R6** — `impl_type` (the impl block's self type, generics stripped;
>   the type after `for` in a trait impl; `""` for free items) joins the
>   key as its own field, not appended to `module`. Fixes the two R1
>   survivors and the wider class they were an instance of.
>
> **R5 — reconciliation** (required before regenerating; the RFC's own
> words: *"the single most dangerous action in this RFC"*). `git diff`
> could not serve as evidence — R6 added a field, so every line changed.
> Semantic comparison instead, parsing old (423 items) and new (408)
> baselines directly:
>
> | Cause | Expected | Found | Note |
> |---|---|---|---|
> | B — `const fn` | 15 | **15** | exact |
> | B′ — `pub unsafe fn` | +2 | **+2** | exact — `sys_audit_drain_ptr`, `sys_audit_drain_raw` present |
> | C — inline `mod` | ~162 | **159** | 3-item gap explained: `SvcLifecycle`, `ServiceManifestEntry`, `ServiceManifestEntry::new` were already genuinely top-level in the *old* scan too — the 162 estimate counted all `module:""` items in the crate, not only the ones that actually move |
> | D — orphaned file | −17 | **−17** | exact — `StoreResult`, `store_read`, `store_append` confirmed absent |
> | E — impl scope | ~60 | **129** (60 `fn` + 69 `const`) | the `fn`-only subset matches the architect's independent spot-check exactly (60 across 22 self types); the remaining 69 are associated `const`s the spot-check's script didn't enumerate by design |
> | C ∩ E overlap | — | **0** | no inline-mod-reattributed item is also inside an impl block in this codebase |
> | unexplained | 0 | **0** | — |
>
> Net: 423 − 17 + 2 = **408**, matching the pre-registered prediction exactly.
>
> **Demonstration 1, re-run against the reconciled baseline** (the RFC
> required this fail by the signature-mismatch pathway specifically, not
> the duplicate-key check — a `FAIL` via the wrong pathway does not count):
> ```
> $ cargo run -p fjell-abi-snapshot -- --verify   # attestd's READY corrupted
>   Changed sig    : 1
>   ~ fjell-service-api::attestd const READY (was sig=CORRUPTE, now sig=4183035c)
> Result: FAIL
> ```
> Reverted; `--verify` on the clean, regenerated `tests/abi/snapshot.json`
> now reports `PASS` (408/408, 0 removed, 0 changed) — this **is** the
> demonstration this row is `sound` on, not the tool's own unit suite (37
> tests, including dedicated brace-depth and impl-scope cases, all pass and
> are necessary but not sufficient — Pass 1's mistake, corrected).

### fjell-abi-snapshot — `pub unsafe fn` matched no pattern, absent from the surface — **sound** (found and repaired within RFC-0.24-003, R2; new row per review)

- **Claim (implicit in R2):** the scanner recognizes function declarations
  regardless of modifier combination.
- **Actual, before repair:** the old code enumerated specific prefix
  strings (`pub fn `, `pub async fn `) rather than the general shape. `pub
  unsafe fn` matched none of them and fell through silently — not
  misparsed, **absent**.
- **Modes:** 1 (scope blindness), and named explicitly in review as the
  same defect class as E-014's literal matching, one level down — the
  third instance in this tool alone (line-oriented parser, collapsed key,
  now enumerated prefixes).
- **Consequence:** two real functions in `fjell-syscall` — the crate
  carrying the kernel/user-space syscall ABI — never appeared in any
  generated snapshot: `sys_audit_drain_ptr`, `sys_audit_drain_raw`
  (`crates/fjell-syscall/src/lib.rs:349`, `:518`). The gate guarding the
  syscall ABI had never known they exist and would not have noticed either
  being removed or re-signed.
- **Repaired** by the same `strip_fn_modifiers()` generalisation as B (R2).
  Confirmed present in the regenerated baseline; unit test
  `unsafe_fn_is_scanned_at_all` covers it directly, plus zero-count checks
  for `pub async fn` / `pub extern "C" fn` / `pub const unsafe fn` (none
  found in the eight stable crates today; the scanner is verified correct
  for them regardless, via synthetic-content tests).

### fjell-abi-snapshot — identity lacked impl scope — **sound** (found and repaired within RFC-0.24-003, R6; new row per review)

- **Claim (implicit in R1):** `(crate, module, kind, name)` uniquely
  identifies a stable-surface item.
- **Actual, before repair:** it does not — two distinct types in the same
  crate and module with a same-named method collide, because the identity
  has no notion of which type's `impl` block a method belongs to.
- **How it was found — worth recording, per the review:** this is the
  first defect in the whole 0.24 line found by **an instrument's own
  guard**, not a person. R1's duplicate-key check, run for the first time,
  reported two survivors on real committed input:
  ```
  fjell-audit-format::         fn kind    (AuditRecordBin::kind, AuditPersistRecord::kind)
  fjell-semantic-v1::catalog   fn new     (CatalogOwner::new,    CatalogRangeOwner::new)
  ```
  Per the handoff's explicit instruction ("if it does not [pass], a defect
  remains unfound — report the surviving keys and escalate"), this was
  reported rather than resolved in code — deciding "the scanner should also
  understand `impl` blocks" was correctly treated as outside R1–R4's named
  scope, not the implementer's to decide by extension.
- **Modes:** 4 (weak predicate) and 2 (proxy attestation, one level
  removed) — `CatalogOwner::new` and `CatalogRangeOwner::new` are distinct
  ABI items; a gate that cannot tell them apart cannot do its job regardless
  of how correctly `module` is computed.
- **Ruling (design authority, in review):** a distinct `impl_type` field,
  **not** appended to `module` — a field named `module` holding a type name
  would itself be the "name that lies" pattern this whole line exists to
  correct. Generics stripped (`impl<T> Foo<T>` → `Foo`); trait impls take
  the type after `for`, not the trait name.
- **Repaired** as R6. `build_identity_map()` keys on five fields; zero
  duplicates in the regenerated 408-item baseline. Unit tests cover a
  simple impl, a generic impl, a trait impl, a generic trait impl, impl
  scope not affecting module path, and the exact real collision shape (two
  types, one method name each, confirmed distinct post-repair).

### fjell-unsafe-audit category extractor — **finding** (new, RFC-0.24-002 review)

- **Claim:** a `// SAFETY: category=X` comment tags the site with category `X`.
- **Actual:** the extractor splits on whitespace and commas only. A valid
  category followed by ordinary punctuation — `category=csr-asm; <explanation>`
  — yields the token `"csr-asm;"`, which matches no valid category and falls to
  `_ => Self::Unknown`.
- **Modes:** 4 (weak predicate).
- **How it was found:** by the implementer during RFC-0.24-002 Slice 3, writing
  the corrected tag for `fjell-hello` and having it silently not take. All 283
  pre-existing sites happen to use `category=X <explanation>` with a space and
  no semicolon. **Nothing enforces that convention.**
- **Not repaired in Slice 3** — hardening the extractor was correctly declined
  as scope creep. Carried to the close-out with the extractor work.

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
- **Repaired (RFC-0.24-002 Slice 2):** the `_` catch-all arm is now a hard
  error naming the unrecognised milestone. Re-ran the identical command
  after the fix:
  ```
  $ cargo xtask qemu-test m8-typo
  [xtask] qemu-test: unknown milestone `m8-typo`
  [xtask] known: m1, m2, m3, m4, m5, m6, m7, m8, v0.4-net, v0.5-platform, v0.6-verification, v0.7-sync
  ```
  exit 1. A bare `qemu-test` (no argument) still defaults to `m8` — kept
  deliberately as "current milestone" shorthand; only an unrecognised
  *name* is now an error. **This one sub-finding is sound; the other two
  below are unchanged and remain open**, so the row's overall status
  stays `finding` until they're dispositioned.
- **Secondary finding, same shared code, smaller, still open:** the `FORBIDDEN` literal
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
  once here and cross-referenced. **Also mode 3, and now repaired
  (RFC-0.24-002 Slice 4):** `negative.rs`'s placeholder fallback — a
  category listed in `KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` with no
  profile file (`lease`, `evidence`) ran a zero-marker placeholder and
  passed. The placeholder path is removed entirely (not merely bypassed;
  `Profile::negative_placeholder` deleted as dead code once its only call
  site was gone). Demonstrated:
  ```
  $ cargo xtask qemu-negative lease
  [xtask] qemu-negative: `lease` is a known category with no profile at tests/qemu/profiles/lease.toml — write one before running it.
  ```
  exit 1. Confirmed `test-all`'s ten real categories (all have profiles)
  are unaffected — spot-checked `capability` still delegates to the real
  profile path. **This sub-finding is sound; the shared TOML/FORBIDDEN
  gaps above remain open**, so the row's overall status stays `finding`.
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

---

## Pass 3 — the eight committed state-asserting artifacts

The question per the handoff (§4) is different from Passes 1–2: not "does
this instrument correctly check its input" but **"what makes this artifact
go stale, and would anything notice?"** Three of the eight were already
examined from the reader's side in Pass 1 (Gates 5/6/7 read
`v1-readiness.md`/`trust-report.txt`/`ERRATA.md`); this pass looks from the
artifact's side, plus covers the five Pass 1 didn't touch directly.

### trust-report.txt — **finding** (carried from Pass 1, Gate 6)

What makes it stale: any value inside a section can drift from reality while
the section headers — unconditional string literals — stay present. What
would notice: nothing, per Pass 1's demonstration (regeneration failure
falls back silently to the stale committed file, which still has all six
headers). No new demonstration; see Gate 6's entry above for the evidence.

### v1-readiness.md — **finding** (carried from Pass 1, Gate 5)

What makes it stale: a row using any status text other than the four exact
literals (`**DONE**`, `**IN PROGRESS**`/`IN_PROGRESS`, `**DEFERRED**`,
`**OPEN**`) is invisible to every counter — not flagged, not miscounted,
simply absent from the totals. What would notice: nothing, per Pass 1's
demonstration. No new demonstration; see Gate 5's entry above.

### ERRATA.md — **finding** (carried from Pass 1, Gate 7, plus a live current instance found this pass)

What makes it stale: the summary table can disagree with the authoritative
per-entry prose above it, or use annotated status text the exact-literal
grep doesn't match. Pass 1 demonstrated the second. This pass found the
first **already true in this repository, right now, not constructed**:

`v1-limitations.md`'s E-013 note (lines 19–27) still describes only
`fjell-kernel` — the pre-widening version. `ERRATA.md`'s E-013 entry was
widened during Pass 1's own review (commit `b82d949`) to ten crates and 166
tests, eight of them the gate tools themselves. Nothing surfaced the gap
between the two, including Gate 12's own `errata-limitations` sub-check,
because that check only asks whether the literal string `E-013` appears
*anywhere* in `v1-limitations.md` — confirmed by reading
`errata_limitations.rs::run_check`: `!limitations_src.contains(id.as_str())`.
The old note still contains the string `E-013`, so the check is satisfied
regardless of what the note actually says. Re-ran the exact command to
confirm:
```
$ cargo run -q -p fjell-tools -- consistency-check --all
errata-limitations: PASS (4 ACCEPTED errata, all referenced in docs/release/v1-limitations.md)
```
This is not a constructed demonstration — it is the artifact's present,
uncorrected state, produced as a side effect of this RFC's own Pass 1
review, not fixed by that review because the ruling addressed `ERRATA.md`
specifically and did not extend to its paired note. Not fixed here either,
per the non-goal; reported for disposition like every other finding, not
edited unilaterally just because it would be easy to.

### v1-limitations.md — **finding** (same live instance as ERRATA.md, above)

This is the other half of the same pair — recorded as its own row because
it is its own artifact per the RFC's list of eight, not because it is a
second, independent defect. What makes it stale: nothing re-derives its
content from `ERRATA.md` when the latter changes; the linkage is a
one-directional "does the ID string appear" check with no content
comparison. See the ERRATA.md row above for the live instance and evidence.

### abi/snapshot.json — **sound** (RFC-0.24-002 Slice 5; distinct from Gate 4's already-sound signature-diffing)

- **What makes it stale:** the file's *parseability*, not just its content.
  `load_snapshot()` is a hand-rolled reader with a hard assumption: exactly
  one JSON object per line (`tools/fjell-abi-snapshot/src/main.rs`,
  `load_snapshot`). It does not use a real JSON parser.
- **What would notice:** nothing, and the failure mode is silent rather than
  a parse error. Demonstrated live: reformatted the committed
  `tests/abi/snapshot.json` into valid, semantically-identical, minified
  single-line JSON (the kind of change a well-meaning formatter or editor
  auto-save could produce) and ran the exact Gate 4 command:
  ```
  $ cargo run -q -p fjell-tools -- abi-snapshot --verify
  Baseline items : 0
  Current items  : 423
  Added          : 378 (additive — OK)
  Removed        : 0
  Changed sig    : 0
  Result         : PASS
  ```
  The parser silently read zero baseline items from perfectly valid JSON.
  With an empty baseline, every real item in the current scan counts as
  "Added — OK", nothing counts as removed or changed, and the gate most
  responsible for catching ABI drift passes having compared against
  nothing. Reverted; `git diff --stat tests/abi/snapshot.json` clean.
- This is a sharper, format-level version of the same class Gate 4 was
  otherwise found sound against in Pass 1 (real signature changes are
  caught; formatting-only *signature line* changes are correctly ignored).
  The gap is one level up: the *snapshot file's own* formatting is
  load-bearing and unvalidated.
- **Repaired (RFC-0.24-002 Slice 5):** the snapshot now carries a declared
  `{"count":N}` header as its first element; `--verify` requires
  `parsed items == declared count` before trusting either, catching both
  failure shapes — not just the total case above. `tests/abi/snapshot.json`
  regenerated in this slice (also picked up legitimate, pre-existing
  additive API growth unrelated to this fix — `ACTION_RESULT`,
  `CHUNK_BYTES`, `DISPATCH_ACTION` and others from RFC-v0.23-001 — since
  `--generate` necessarily reflects current source truth; verified
  additive-only, nothing removed or changed). Both shapes re-demonstrated
  after the fix:
  ```
  $ jq -c . tests/abi/snapshot.json > /tmp/minified.json   # total: reformatted
  fjell-abi-snapshot: tests/abi/snapshot.json has no `count` header — malformed or reformatted snapshot, cannot trust its contents

  $ head -200 tests/abi/snapshot.json > /tmp/truncated.json; echo ']' >> /tmp/truncated.json   # partial
  fjell-abi-snapshot: tests/abi/snapshot.json declares 423 items but 198 were parsed — file is truncated or malformed
  ```
  Both exit 1. Restored the regenerated file; `--verify` passes clean
  (423/423, 0 removed, 0 changed). Four new unit tests added
  (`load_snapshot_reports_no_count_header_when_reformatted_to_one_line`,
  `load_snapshot_declared_count_disagrees_with_truncated_items`,
  `load_snapshot_agrees_when_file_is_intact`, `extract_json_usize_works`);
  all 12 of the tool's own tests pass.

### repro/baseline-digests.txt — **sound**

- **What makes it stale:** a prebuilt binary rebuilt without re-recording
  the baseline (the actual, real incident this project already had —
  RFC-v0.23-002's review corrections re-recorded exactly this).
- **What would notice:** the digest-mismatch check, already re-verified
  fail-closed-on-missing-baseline in Pass 2 (Tier 3b). Additionally checked
  for this pass: (a) format fragility — `load_digests` explicitly rejects a
  non-64-hex-char digest with a named error ("legacy FNV-1a baseline"),
  not a silent empty-parse, unlike `abi/snapshot.json`; (b) scope
  completeness — `DEFAULT_TARGETS` covers `crates/fjell-kernel/prebuilt/`
  and the kernel ELF; confirmed by search that no other `prebuilt/`
  directory or committed `.bin` exists in the repository outside that scope,
  so the target list is not currently under-scoped.

### syscall/expected.toml — **sound** (cross-referenced from Pass 1, Gate 12)

- **What makes it stale:** a syscall added to the ABI enum without a
  matching entry, or an entry naming a syscall no longer in source, in
  either direction.
- **What would notice:** `syscall_surface.rs`'s check, already demonstrated
  both directions failing correctly in Pass 1 (`new_declared_syscall_not_in_expected_fails`,
  `stale_expected_entry_no_longer_in_source_fails`). Not re-demonstrated;
  same reasoning as Tier 1's cross-reference in Pass 2 — re-running an
  already-established fact isn't new evidence.

### rfcs/README.md — **finding** (new this pass; zero coverage found)

- **Claim:** the file states outright, in its second line, "Folder is the
  source of truth for state" and presents itself as an index: file counts
  per lifecycle folder, and a full table of links into `rfcs/done/`.
- **What makes it stale:** a renamed or moved RFC file breaking a relative
  link; the folder's actual file count diverging from the header's stated
  count. Exactly the RFC's own motivation example #10 ("13 broken relative
  links in tracked docs"), which happened to this exact file's class of
  content before.
- **What would notice:** nothing. Searched the full instrument set audited
  in Passes 1–2 for any markdown-link-checker or count-verifier —
  none exists. `rfc_status_folder.rs` (Gate 12's own sub-check that this
  file's introductory text explicitly references) reads `rfcs/proposed/`
  and `rfcs/done/` **directly from disk** and never opens `rfcs/README.md`
  at all — confirmed by reading its `check()` function.
- **Demonstration:** broke the very first link in the file (the one in its
  own second line, pointing readers to the RFC lifecycle policy) and
  changed the stated count from 159 to 200, then ran the full gate matrix:
  ```
  $ cargo xtask release-rehearsal
  ... [PASS] Gate 12 Consistency check ...
  --- consistency-check: rfc-status-folder ---
  rfc-status-folder: PASS (158 RFCs checked)
  RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS
  ```
  All twelve gates passed, unaffected, with a dangling link and a wrong
  count sitting in the one document that calls itself the source of truth.
  Reverted; `git diff --stat rfcs/README.md` clean.
- The file's *current* content happens to be accurate (verified separately:
  159 files in `rfcs/done/`, 159 links in the README, all 159 resolving,
  0 broken) — this is a finding about the complete absence of anything that
  would notice the next time it isn't, not a claim that it is wrong today.

### Pass 3 summary

- **Audited:** 8 / 8.
- **Sound:** 2 — `repro/baseline-digests.txt`, `syscall/expected.toml`.
- **Findings:** 6 — `trust-report.txt`, `v1-readiness.md`, `ERRATA.md`,
  `v1-limitations.md`, `abi/snapshot.json`, `rfcs/README.md`.
- **UNAUDITED:** 0.
- **Notable:** one finding (the ERRATA.md / v1-limitations.md pair) is not
  a constructed demonstration but the artifact's actual, current,
  uncorrected state — produced as a side effect of this RFC's own Pass 1
  review and still present as of this pass. Not fixed here, per the
  non-goal; flagged explicitly as live rather than hypothetical.

Required evidence: `cargo xtask release-rehearsal` re-run clean and Gate 12
still 35/26/9 after all Pass 3 demonstrations were reverted — captured in
the review request for this pass. One incidental `trust-report.txt`
regeneration (a byproduct of re-running the rehearsal during the
`rfcs/README.md` demonstration) was reverted, not committed.

---

## Pass 4 — the sixteen CI jobs

Source: `.github/workflows/ci.yml`. Sixteen `jobs:` entries (excluding the
`on:` trigger keys): `ci-format`, `ci-check`, `ci-cross-check`,
`ci-test-host`, `ci-test-services`, `ci-docs`, `ci-qemu-smoke`,
`ci-qemu-negative`, `ci-test-v07-formats`, `ci-proptest`,
`ci-unsafe-audit`, `ci-arm64-check`, `ci-schema-gate`, `ci-qemu-v07`,
`ci-fuzz-nightly`, `ci-verus`.

Cannot trigger GitHub Actions from here. Where a job's `run:` step is a
command reproducible locally, ran it directly (several jobs run the exact
mechanisms already audited in Passes 1–3, so those are cross-referenced
rather than re-demonstrated). Where it genuinely cannot be reproduced
locally (a scheduled nightly fuzz run, the GitHub-hosted step environment
itself), recorded `UNAUDITED` with the reason, per handoff §0.1 — not
manufactured.

### A cross-cutting finding first: 21 workspace crates are never named in `ci.yml` at all

Every host `cargo check`/`cargo test` job in this workflow (`ci-check`,
`ci-cross-check`, `ci-test-host`, `ci-test-services`,
`ci-test-v07-formats`) lists its packages **explicitly**, by name — the
same shape as `abi-snapshot`'s `STABLE_CRATES` before its RFC-v0.22-001
fix, and exactly the RFC's own concern about explicit-list staleness.
Extracted every `-p <crate>` argument across the whole file and diffed
against all workspace package names:

```
comm -23 <workspace-crate-names> <every -p argument in ci.yml>
```

**19 of 89 crates never appear:** `fjell-abi-snapshot`, `fjell-benchmarks`,
`fjell-bundle-format`, `fjell-cap-manifest`, `fjell-config-sync`,
`fjell-consistency-check`, `fjell-dev-harness`, `fjell-dtb-validate`,
`fjell-fleet-sync`, `fjell-mmio-audit`, `fjell-readiness-check`,
`fjell-replay-cache`, `fjell-repro-check`, `fjell-sdk`,
`fjell-semantic-toolkit`, `fjell-sig-ed25519`, `fjell-summary-check`,
`fjell-svc-fault`, `fjell-svc-timeout`.

> **Corrected in Pass 4 review (2026-08-03).** Originally recorded as *21 of
> 91*, and `fjell-fuzz` / `fjell-hello` were in the list. `cargo metadata
> --no-deps` reports **89** workspace packages; the 92 manifests on disk are
> 1 root + 89 members + those two, which are **not workspace members** and so
> cannot be named with `-p` in a workspace command — they were never in the
> population. A denominator including things outside the population is mode 1
> arriving in this audit's own measurement. Both sub-findings below are
> unaffected: all nine named crates are genuine members.

Two sub-findings inside that list matter more than the count:

- **Six are the gate tools** (`fjell-abi-snapshot`, `fjell-consistency-check`,
  `fjell-mmio-audit`, `fjell-readiness-check`, `fjell-repro-check`,
  `fjell-summary-check`). This compounds E-013 for CI specifically: their
  unit tests are already unreachable via `test-all` tier 1 (`--lib` skip);
  they are *also* never named in any CI job, so nothing in ordinary CI ever
  runs them either, by any mechanism.
- **Three back Gate 8's validation drills** (`fjell-sig-ed25519`,
  `fjell-fleet-sync`, `fjell-config-sync`). Gate 8's five markers — the ones
  Pass 1 found sound — run only at `release-rehearsal` time. Ordinary CI, on
  every push and PR, never executes them. That may be an intentional
  design choice (drills reserved for release checks, not every commit) —
  recorded as a finding because nothing in the workflow says so; it reads
  as an omission rather than a decision.

Not all 21 are necessarily meaningful gaps — `fjell-benchmarks`,
`fjell-hello`, `fjell-fuzz`, `fjell-dev-harness` may be intentionally
excluded example/scratch crates. Did not individually adjudicate all 21
(timebox, per handoff §0.2.3) — the two sub-findings above are the ones with
a concrete, checkable consequence; the full list is reported for whoever
dispositions this.

### ci-format — **sound**

- **Claim:** the workspace is `rustfmt`-clean.
- **Actual:** `cargo fmt --all --check`, a real Cargo subcommand with a real
  exit code.
- **Demonstration:** appended deliberately malformatted lines to
  `crates/fjell-abi/src/lib.rs` and ran the exact command: exit 1, diff
  printed. Reverted; clean.

### ci-check, ci-cross-check, ci-test-host, ci-test-services, ci-test-v07-formats — **finding** (the explicit-list gap above)

- **Claim:** each checks/tests its named set of crates for the target it
  specifies (host, RISC-V bare-metal host-check, host-pure lib tests,
  RISC-V service cross-check, v0.4–v0.7 format crates).
- **Actual:** each is internally consistent — every crate it lists really
  does get checked/tested by that job. The finding is not that any one job
  is wrong; it's that the five lists, even unioned, don't cover the
  workspace, and nothing diffs them against the crate set the way this
  audit just did.
- **Modes:** 1 (scope blindness), same shape as the historical
  `STABLE_CRATES` incident this project already fixed once, in the tool
  that fixed it.
- Not independently re-demonstrated per job (five jobs, one mechanism,
  same reasoning as Pass 2's fourteen QEMU tiers); the cross-cutting
  section above is the demonstration; run once, applies to all five.

### ci-docs — **UNAUDITED**

- **Claim:** the mdBook documentation builds without error.
- **Reason UNAUDITED:** ran `cd docs && mdbook build` locally — it succeeds,
  which confirms the job's literal claim (the book builds), but auditing
  *what mdbook itself does and doesn't catch* (e.g. broken internal links
  within the book, vs. the separate `rfcs/README.md` finding from Pass 3
  which is outside the book entirely) is a question about a third-party
  tool's own behavior, not this repository's instrument. Recording
  `UNAUDITED` rather than asserting soundness I didn't verify.

### ci-qemu-smoke, ci-qemu-v07 — **finding** (inherits Pass 2's shared-harness findings, plus a new gap)

- Both matrix jobs call `cargo xtask qemu-test <name>` — the exact mechanism
  Pass 2 already found has a silent catch-all to `m8` for any unrecognized
  name (`smoke.rs`'s `_ => ("m8", "TEST:M8:PASS")`). Not re-demonstrated
  (same mechanism, same fact).
- **New for this pass:** `smoke.rs` supports a `"v0.6-verification"`
  milestone (`TEST:V0.6-VERIFY:PASS`), but it appears in **neither**
  matrix (`ci-qemu-smoke`: `m1..m8`; `ci-qemu-v07`:
  `v0.4-net, v0.5-platform, v0.7-sync`) — nor in `test_all.rs`'s own
  `SMOKE_PROFILES` constant. Confirmed by reading all three lists directly.
  This specific profile is defined in code and never invoked by any
  instrument, local or CI — likely vestigial from a milestone-naming
  transition, not a live risk, but exactly the "instrument exists on paper,
  never runs" shape this RFC catalogues.

### ci-qemu-negative — **finding**

- **Claim:** runs every negative-test category.
- **Actual:** matrix lists `[capability, ipc, mmio, dma, user-copy, audit,
  policy, svc, harness]` — **nine** categories.
- **Modes:** 5 (stale assertion). `test_all.rs`'s `NEG_CATEGORIES` lists
  **ten** — the same nine plus `"semantic"` (RFC-v0.23-001's addition,
  confirmed passing under `test-all` in Passes 1–3 of this very audit).
  Confirmed by direct comparison of the two lists. The `semantic` negative
  category has never been added to this CI matrix — it runs locally under
  `test-all` and never in ordinary CI, on any push or PR, since the RFC
  that added it.

### ci-test-v07-formats — covered above

`ci-test-v07-formats` is one of the five explicit-list jobs (see above).

### ci-proptest — **sound** (RFC-0.24-002 Slice 6; reversed from `sound` in Pass 4 review, now genuinely sound)

- **Claim:** the job is named "Property tests"; it runs the workspace's
  property-based tests on every push and PR.
- **Actual:** `cargo test -p fjell-proptest --lib` (plus `fjell-store-model`
  and `fjell-bootctl-model`). `fjell-proptest` has **no `proptest!`
  invocation in `src/`** — all 24 live in `tests/` (`harness.rs` 10,
  `verus_lemma_properties.rs` 14), which `--lib` excludes. The step runs
  **zero** of them:
  ```
  $ cargo test -p fjell-proptest --lib
  running 0 tests
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
- **Modes:** 1 (scope blindness) — the same `--lib` narrowing as Tier 1 /
  E-013, this pass's own worked example.
- **Where they do run:** only `test-all` tier 2
  (`cargo test -p fjell-proptest --release`, no `--lib`), which is manual.
  Gate 1 and Tier 1 `--exclude fjell-proptest` explicitly, and
  `release-rehearsal` has **no proptest gate at all** (gates 1–8, 10, 11,
  12). So on every push and PR the "Property tests" job is green having run
  nothing — including the 14 `verus_lemma_properties` cases that
  cross-check the Verus proofs behind capability 8/8 and lease 5/5.

> **Reversed in Pass 4 review (2026-08-03).** Originally recorded `sound`,
> reasoned as "a small, complete, explicit list — no gap." The *list* is
> complete; all three property-test-bearing crates are named. The
> **predicate** was not examined. A complete list of crates is not an answer
> to *what would make this report success without having checked* — and per
> handoff §0.1, no demonstration was produced, so `UNAUDITED` was the floor
> here, never `sound`. Fix (drop `--lib`) is in the pre-cut group.

- **Repaired (RFC-0.24-002 Slice 6):** dropped `--lib` for the
  `fjell-proptest` invocation only; `fjell-store-model` and
  `fjell-bootctl-model` keep `--lib` (their `proptest!` blocks are
  correctly in `src/`). Before/after:
  ```
  $ cargo test -p fjell-proptest --lib      # before
  running 0 tests

  $ cargo test -p fjell-proptest            # after
  running 10 tests   (harness.rs)  ... ok
  running 14 tests   (verus_lemma_properties.rs)  ... ok
  ```
  24 of 24, matching the claim. Then broke `p1_no_generation_alias` with
  `prop_assert!(false, ...)` and reran: `test result: FAILED. 9 passed; 1
  failed`, exit 101 — the job now genuinely fails on a broken property.
  Reverted the break and removed the `.regressions` artifact it generated;
  `git diff --stat` clean.

### ci-unsafe-audit — **sound** (RFC-0.24-002 Slice 3)

- **Claim:** every `unsafe` site in the workspace has a `SAFETY:` comment
  (the same claim as Gate 2 / Tier 3).
- **Actual:** `cargo run -p fjell-unsafe-audit -- --root crates --check` —
  scoped to `crates/` only, unlike Gate 2 and Tier 3's `--workspace .`.
- **Modes:** 1 (scope blindness) — confirmed live, not hypothetical. A real
  `unsafe` block already exists outside `crates/`
  (`examples/three-node-fleet/fjell-hello/src/main.rs:43`, currently
  correctly commented). Removed its `SAFETY:` comment and ran both scopes:
  ```
  --root crates      → missing comment: 0, exit 0 (PASS)
  --workspace .       → missing comment: 1, exit 1 (FAIL)
  ```
  CI's exact command does not see the violation the local gate and tier
  both catch. Reverted; `git diff --stat` clean.

> **Extended in Pass 4 review (2026-08-03) — a second, live finding on the
> same tool.** Run on the **untouched** tree, `--workspace .` reports
> `MISSING/UNKNOWN category: 1` **and exits 0 anyway**. The site is the same
> file, one line up (`…/fjell-hello/src/main.rs:46`), tagged
> `category=asm-instruction`, which is not a valid category — `from_str`'s
> `_ => Self::Unknown` catch-all swallows it, the same silent-catch-all shape
> as `smoke.rs`'s `_ => ("m8", …)`. `main.rs:363` reads
> `if check && missing > 0 { process::exit(1); }`: `missing_cats` is
> computed, printed, and **never enforced**. Three consumers pass over it —
> CI, whose step is *named* "Unsafe audit (category= check)";
> release-rehearsal Gate 2, whose predicate is
> `out.contains("missing comment    : 0")` and never reads the category
> line; and `test-all` Tier 3. Mode 4 (weak predicate) on top of the mode 1
> above. **This is the only live, present-tense false green in the entire
> 55-instrument audit** — every other finding required constructing a break.
> **Do not correct the `asm-instruction` tag:** it is the pre-cut RFC's
> built-in demonstration on real committed input.

- **Repaired (RFC-0.24-002 Slice 3, done first — the tree was red before
  anything else was touched):**
  1. Captured the untouched-tree failure exactly as above: `exit=0` despite
     `MISSING/UNKNOWN category: 1`.
  2. Made `--check` exit non-zero on `missing_cats > 0` too (not just
     `missing > 0`), and added `category_valid` to the `--json` output
     (previously category validity was invisible to JSON consumers
     entirely). Re-ran on the **still-uncorrected** tree: `exit=1` — the
     tree went red on real committed input, no construction needed.
  3. Verified the chain to Gate 2 explicitly, per the handoff's required
     ordering: with the tool now enforcing but Gate 1/2's exit-status
     consumption (Slice 1) not yet implemented, `release-rehearsal` Gate 2
     still reported `[PASS]` — proof Slice 1 is load-bearing, not
     cosmetic. After Slice 1, re-ran the same still-uncorrected tree: Gate
     2 correctly reported `[FAIL]`.
  4. *Only then* corrected the tag — to `csr-asm`, matching the codebase's
     own comment convention (`category=X <explanation>`, no semicolon; an
     initial `category=csr-asm;` attempt tripped the same whitespace/comma
     tokenizer that caused the original defect, `Unknown`-ing the corrected
     tag until the punctuation was fixed to match every other of the 283
     valid sites). Confirmed clean: `284/284` valid, `exit=0`.
  5. CI's invocation changed from `--root crates` to `--workspace .`,
     matching Gate 2 and Tier 3. The scope gap (finding 1, above) is closed
     by construction — there is no longer a narrower CI scope to diverge
     from the local instruments.
  All 10 of the tool's own tests still pass unmodified.

### ci-arm64-check — **sound**

- **Claim:** `fjell-arch-arm64` type-checks for the `aarch64-unknown-none`
  target.
- **Actual:** narrowly scoped by design (one crate, one cross-compilation
  boundary) — not claiming workspace-wide coverage, so the narrowness isn't
  a gap the way the explicit-list jobs' is. No finding.

### ci-schema-gate — **sound** (RFC-0.24-002 Slice 7; naming and presence only — see repair note)

- **Claim:** the job step is literally named "Verify frozen schemas have
  not drifted."
- **Actual:** the script checks only that every `*.frozen` file exists and
  is non-empty (`[ -s "$f" ]`). It does not read, parse, or compare against
  any current schema definition. Its own comment says why: *"Full
  schema-dump tooling lands with the fjell-tools schema subcommand"* —
  confirmed no `schema` subcommand exists yet in `fjell-tools/src/main.rs`.
- **Modes:** 4 (weak predicate) — the starkest instance found in this whole
  audit. The check's name promises content comparison; the implementation
  performs none.
- **Demonstration:** replaced
  `crates/fjell-semantic-v1/schema/intent-v1.frozen`'s entire content with
  an unrelated sentence (still non-empty) and ran the exact CI script:
  ```
  schema-gate: all *.frozen files present and non-empty
  ```
  exit 0. A schema file containing garbage passes a job whose name claims
  to verify it hasn't drifted. Reverted; `git diff --stat` clean.

> **Extended in Pass 4 review (2026-08-03) — the deletion case is worse.**
> Run the same script against a tree with the `*.frozen` files **deleted**:
> `find` returns nothing, the loop body never executes, and
> `schema-gate: all *.frozen files present and non-empty` prints with exit 0.
> The step's own echo says *"checking `*.frozen` files are committed"* — and
> deleting every one of them passes. **Mode 3 (fail-open on absence)**
> underneath the mode 4 above. Two further notes: the empty-file branch
> *does* work — `exit 1` inside a `find | while` subshell propagates under
> GitHub Actions' default `bash -e`, and that one line is the only thing the
> step enforces; all 11 `.frozen` files are currently under `crates/`, so the
> `find crates` scope is adequate today. And the step carries a **comment
> describing two behaviours it does not have** (BREAKING-SCHEMA marker
> scanning on PRs, frozen-counterpart matching on push to main): the name is
> a false claim and the comment is two more.

- **Repaired (RFC-0.24-002 Slice 7) — presence and non-emptiness only; the
  actual drift check remains explicitly out of scope, a v0.25 candidate:**
  1. Renamed the step to "Frozen schema files present and non-empty" —
     what it actually does, nothing more.
  2. Deleted the two comment lines describing BREAKING-SCHEMA scanning and
     frozen-counterpart matching; replaced with a pointer to the
     RFC-0.24-001 close-out where the real check is a v0.25 candidate.
  3. Replaced the `find crates -name "*.frozen"` loop source with a fixed,
     enumerated list of the 11 expected paths, so a *missing* file is
     checked against (and fails) rather than never appearing in the loop
     at all.
  - Demonstrated: deleted `crates/fjell-semantic-v1/schema/intent-v1.frozen`
    and ran the corrected script — `ERROR: missing or empty frozen schema:
    ...intent-v1.frozen`, exit 1 (previously exit 0). Restored the file;
    confirmed green (`all 11 expected *.frozen files present and
    non-empty`). Re-confirmed the corrupted-but-non-empty-content case
    still passes — correctly, since the renamed step no longer claims to
    catch that.

### ci-fuzz-nightly — **UNAUDITED**

- **Claim:** eight fuzz targets each run without crashing for 300s.
- **Reason UNAUDITED:** `if: github.event_name == 'schedule'` — does not
  run on push or PR at all, only a weekly cron. Requires a nightly
  toolchain, `cargo-fuzz`, and genuine multi-minute fuzzing runs per target;
  not practical to reproduce meaningfully within this audit's timebox, and
  a manufactured "did it crash in 10 seconds" run would not honestly answer
  the claim. Recording `UNAUDITED` rather than a weak substitute.

### ci-verus — **sound (by explicit design)**

- **Claim:** records Verus machine-check markers; explicitly does **not**
  gate the pipeline.
- **Actual:** `continue-on-error: true`, and the job's own comment states
  the real blocking check is `release-rehearsal` Gate 10 (audited sound in
  Pass 1, with its own vacuous-empty-target finding recorded there). This
  job succeeding or failing has no effect on CI's outcome by design — asked
  whether that design is itself honestly represented, and it is: nothing
  here claims to block, and nothing does.

### Pass 4 summary

- **Audited:** 16 / 16.
- **Sound:** ~~4~~ **3** — `ci-format`, `ci-arm64-check`, `ci-verus`.
- **Findings:** ~~10~~ **11** — the five explicit-list jobs (one shared
  cross-cutting finding), `ci-qemu-smoke`, `ci-qemu-v07`, `ci-qemu-negative`,
  `ci-unsafe-audit`, `ci-schema-gate`, **`ci-proptest`** (reversed from
  `sound` in review — runs zero tests).
- **UNAUDITED:** 2 — `ci-docs` (third-party tool behavior out of this
  repository's instruments), `ci-fuzz-nightly` (schedule-only, not
  practically reproducible in this session).

This closes the audit's four passes: 55 instruments attempted, 0 skipped
outright — every one answered with either a demonstration or an honest
`UNAUDITED`.

**Audit totals, as corrected in review:**

| | Pass 1 | Pass 2 | Pass 3 | Pass 4 | Total |
|---|---|---|---|---|---|
| Sound | 6 | 4 | 2 | 3 | **15** |
| Findings | 5 | 15 | 6 | 11 | **37** |
| `UNAUDITED` | 1 | 0 | 0 | 2 | **3** |
| | 12 | 19 | 8 | 16 | **55** |

Before this line, all 55 were reporting green.

**Carried to close-out (Pass 4 review ruling):** the `ci-proptest` reversal
means a `sound` verdict was reached by checking a list's completeness rather
than the predicate. All **15** `sound` rows are to be re-derived against one
question — *was a demonstration actually produced, or was a completeness
check mistaken for one?*

Required evidence: `cargo xtask release-rehearsal` re-run clean and Gate 12
still 35/26/9 after all Pass 4 demonstrations were reverted — captured in
the review request for this pass.

---

## RFC-0.24-002 — the seven repairs, and the tallies after them

Seven slices, each a small change to an existing instrument, each
demonstrated failing before the fix and passing after (per-instrument rows
above cite each demonstration). Nothing outside the seven was touched; 30
findings remain open by design (RFC-0.24-002 §Non-goals).

**Five rows moved `finding` → `sound` in full:**

| Row | Slice |
|---|---|
| Gate 1 — Host test suite | 1 |
| `abi/snapshot.json` | 5 |
| `ci-unsafe-audit` | 3 |
| `ci-proptest` | 6 |
| `ci-schema-gate` | 7 |

**Two rows repaired in part — status stays `finding`, one sub-issue of
several now sound:**

| Row | Slice | Sub-issue repaired | Still open |
|---|---|---|---|
| Tiers 4–7 (QEMU smoke) | 2 | Silent milestone catch-all | `FORBIDDEN`'s `"TEST:FAIL"` miss; TOML bracket truncation |
| Tiers 8–17 (QEMU negative) | 4 | `lease`/`evidence` placebo pass | Same two, shared code |

Gate 2 (Pass 1) was already `sound`; Slice 1 additionally wired it to
`sh_status()` so Slice 3's enforcement reaches it — recorded as a repair
note on Gate 1's row rather than a second status flip, since Gate 2's own
verdict didn't change.

**Updated totals** — as submitted, then reconciled in review:

| | Pass 1 | Pass 2 | Pass 3 | Pass 4 | Total |
|---|---|---|---|---|---|
| Sound | 6 → 7 → **6** | 4 | 2 → **3** | 3 → **5** | 15 → 20 → **18** |
| Findings | 5 → 4 → **5** | 15 | 6 → **5** | 11 → **8** | 37 → 32 → **33** |
| `UNAUDITED` | 1 | 0 | 0 | 2 | **3** |
| | 12 | 19 | 8 | 16 | 55 → **56** |

Three review adjustments to the submitted figures:

1. **Gate 4 reverts** `sound` → `finding` (identity-key collapse, above). Pass 1
   loses the slice-1 gain it made, netting back to 6 sound / 5 findings.
2. **A new instrument row** — the `fjell-unsafe-audit` category extractor — takes
   the population from 55 to **56**. It was never one of the enumerated 55; it is
   a sub-component surfaced by repairing the tool that contains it.
3. Everything else stands exactly as submitted.

**33 findings remain**, all dispositioned — the deferred literal-matching family,
the audit close-out, or (for Gate 4) proposed Slice 8. None forgotten.

This RFC made the instruments that block a 0.24 cut honest — with one exception
that the cut now waits on. It did not make every instrument honest; that was
never its goal.

---

## RFC-0.24-003 — Gate 4's exception closes

Slice 8 (above) became its own RFC when sizing it found three further scanner
defects (B, C, D) that were *what made the duplicates* — see
[RFC-0.24-003](../../rfcs/done/RFC-0.24-003-abi-snapshot-identity.md). R1's
own duplicate-key check, run for the first time against a corrected scanner,
found a fourth (impl scope, repaired as R6) — the first defect in this entire
milestone caught by an instrument's own guard rather than a person.

**Gate 4's row moves `finding` → `sound`**, this time citing a live
signature-mismatch demonstration against a semantically-reconciled baseline,
not the tool's own unit suite — the substitution that made the row wrong the
first time. Full repair narrative and reconciliation table on Gate 4's row,
above.

**Two new rows**, both `sound` — found and repaired within the same RFC, so
recorded here rather than filed as errata: the `pub unsafe fn` gap (found
during R2, a real absence in the syscall-ABI crate) and the impl-scope gap
(found by R1's own check, repaired as R6).

Delta from this RFC: Gate 4 `finding` → `sound`; +2 new `sound` rows
(population 56 → 58).

### Totals, corrected

The implementer noted that the pre-RFC-0.24-003 figures (18 sound / 33
findings / 3 `UNAUDITED` / 56) sum to 54, and correctly left them alone as not
theirs to have written. They are the architect's, and the gap was **two**
errors, not one:

1. **Pass 4's `sound` cell read 5; it should have been 6.** RFC-0.24-002 moved
   three Pass-4 rows to `sound` (`ci-unsafe-audit`, `ci-proptest`,
   `ci-schema-gate`) from a base of 3. That submission's *total* of 20 required
   6 — cell and total disagreed, and the architect recomputed the total **from
   the cells**, propagating 5 into an 18 that should have been 19.
2. **The 56th instrument's finding was never added to any total.** The
   `fjell-unsafe-audit` category extractor was correctly recorded as taking the
   population from 55 to 56, but it belongs to no pass, so the per-pass column
   sums silently omitted it. Findings should have read 34, not 33.

**Corrected, with rows outside the four passes given their own column so the
same omission cannot recur:**

| | Pass 1 | Pass 2 | Pass 3 | Pass 4 | Outside | Total |
|---|---|---|---|---|---|---|
| Sound | 7 | 4 | 3 | 6 | 2 | **22** |
| Findings | 4 | 15 | 5 | 8 | 1 | **33** |
| `UNAUDITED` | 1 | 0 | 0 | 2 | 0 | **3** |
| | 12 | 19 | 8 | 16 | 3 | **58** |

22 + 33 + 3 = 58, and every column sums to its own instrument count.

**33 findings remain open**, all previously dispositioned (deferred
literal-matching family or audit close-out) and unaffected by this RFC.

**Why this happened, recorded rather than left as an anecdote.** This table was
maintained as prose arithmetic across four passes and three RFCs, by two
parties, with **no instrument checking it**. That is **E-016**'s shape — *no
instrument verifies any document link, index, or count* — occurring inside the
audit's own record, and it is filed there as a concrete instance.

**A note on the population figure.** "55 instruments" was always an enumeration,
not a measurement: it counted gates, tiers, jobs, and artifacts at the
granularity someone chose when scoping the audit. Item 2 above is the first time
that boundary moved, and it moved because repairing a tool exposed a component
inside it that makes its own claim. Expect the number to keep moving. A fixed
denominator would be the more comfortable record and the less honest one.
