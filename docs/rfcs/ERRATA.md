# RFC Errata Register

This file records every case where an RFC's normative text claims more
than the merged implementation delivered. Established by RFC-v0.16-004
in response to architect review RB-05.

Each entry names the RFC, the over-claim, what actually shipped, the
resolution status, and the tracking RFC that closes it.

Status legend: **OPEN** (drift live) · **CLOSED** (reconciled) ·
**ACCEPTED** (drift is a documented, deliberate v1.0 limitation).

---

## E-001 — RFC-v0.11-002 §4: Ed25519 test vectors

- **Claim:** all RFC 8032 §7.1 TV1 tests pass.
- **Shipped (v0.11–v0.15):** two tests removed; seed→pubkey and sign
  paths unverified due to a corrupted test-vector seed constant.
- **Resolution:** **CLOSED** by RFC-v0.16-001. Seed corrected;
  both tests restored and passing; cross-verified against OpenSSL and
  libsodium. Root cause was a transcription error, not a crypto defect.

## E-002 — RFC-v0.11-003 §5: key encryption at rest

- **Claim:** signing keys encrypted at rest with an Argon2id-derived key.
- **Shipped:** keys written as plaintext with magic `FJKY`.
- **Resolution:** **CLOSED** by RFC-v0.16-006 — Argon2id encryption
  implemented; plaintext path retained only behind an explicit
  `--insecure-plaintext` flag for CI fixtures.

## E-003 — RFC-v0.11-004 §3: revocation record wire length

- **Claim:** `WIRE_LEN` = 106 bytes.
- **Shipped:** actual layout is 116 bytes (4+2+16+4+2+8+16+64).
- **Resolution:** **CLOSED** in v0.15.x — constant corrected to 116;
  RFC text updated. No external consumer existed at correction time.

## E-004 — RFC-v0.12-002: real-board target selection

- **Claim:** StarFive VisionFive 2 selected as a validated "Path A"
  real-world deployment target.
- **Shipped:** board profile, DTB validator, MMIO audit, deployment
  guide — but no hardware was booted.
- **Resolution:** **ACCEPTED** as a v1.0 limitation per RFC-v0.16-005.
  v1.0 scope is narrowed to "QEMU `virt` supported profile; VisionFive 2
  profile is provisional and unvalidated on silicon." Hardware bring-up
  tracked for v1.1.

## E-005 — RFC-v0.13-005 §6: disaster-recovery drill attestation

- **Claim:** recovery procedures rehearsed; drill attestation committed.
- **Shipped:** recovery guide written; no drill run; no attestation.
- **Resolution:** **CLOSED** by RFC-v0.16-003 — a QEMU recovery drill
  is executed and its attestation committed under
  `docs/operations/recovery-drills/`.

## E-006 — RFC-v0.14-002 §5: catalog intent tags

- **Claim:** `cap-manifest.toml` intent tags 0x0501–0x0503 exist in the
  catalog.
- **Shipped:** the tags were referenced before the catalog generation
  step was run for them.
- **Resolution:** **CLOSED** by RFC-v0.16-007 — the runtime SDK trial
  regenerates the catalog and confirms the tags resolve.

## E-007 — RFC-v0.15-002 §5.8: threat-model adversarial review

- **Claim:** threat model passed an adversarial review.
- **Shipped:** threat model written; no adversarial review recorded.
- **Resolution:** **CLOSED** by RFC-v0.16-005 — a recorded adversarial
  review pass is committed; findings folded into the threat model.

## E-008 — RFC-v0.15-004 §3: recovery guide follow-test

- **Claim:** recovery guide validated by a non-author follow-test.
- **Shipped:** guide written; no follow-test.
- **Resolution:** **CLOSED** by RFC-v0.16-003 (same drill as E-005).

## E-009 — RFC-v0.15-005 §3: non-goals adversarial review

- **Claim:** non-goals list passed an adversarial review.
- **Shipped:** list written; no review recorded.
- **Resolution:** **CLOSED** by RFC-v0.16-005 — review recorded together
  with the threat-model review.


## E-010 — RFC 034 / RFC 042: IPC payload word delivery

- **Claim:** `sys_ipc_call_words` transfers w0..w2 to the receiver's trap
  frame, accessible via `sys_ipc_recv_msg` as `(label, w0, w1, w2, ...)`.
- **Shipped (v0.1–v0.19):** two independent defects silently dropped every
  payload word: (a) the `sys_ipc_call_words` wrapper sent the raw label
  without packing the word count into tag bits 16–23, so the kernel's
  `build_msg` read `tag.words = 0` and copied nothing; (b) `deliver()` wrote
  the sender badge to `a2` and the words to `a3..a6`, while userspace
  `sys_ipc_recv_msg` read `w0` from `a2` (the badge, always 0). Every
  word-carrying protocol failed silently; label-only protocols were unaffected
  and masked the breakage. The neg-test IPC profiles false-passed by
  accidentally binding `LeaseId(0)` (a previously-revoked lease) and failing
  instantly rather than exercising the real protocol.
- **Resolution:** **CLOSED** in v0.20.0. `sys_ipc_call_words` packs
  `tag | (word_count << 16)`; `deliver()` writes w0..w3 to a2..a5, identity
  to a6, badge removed (no user-space consumer existed). Covered by the
  three new real IPC negative markers now passing for the first time.

## E-011 — RFC-v0.7.4-003: `cap_install` rights validation

- **Claim:** `sys_cap_install`'s doc-comment states "the kernel validates that
  `rights` ⊆ installer authority"; `sys_cap_install_with_rights`'s doc-comment
  states it "[a]llows cap-broker to install caps with a narrower right set
  than `ALL_NON_META`."
- **Shipped:** neither claim executes. `sys_cap_install_with_rights`
  (`crates/fjell-syscall/src/lib.rs:639`) discards its `rights_bits` argument
  (`let _ = rights_bits;`) and falls back to `sys_cap_install`. More
  fundamentally, `CapInstall` (17) has no dispatch arm in
  `crates/fjell-kernel/src/trap/syscall.rs` at all (RFC-v0.21.3-001 §M2) — both
  wrappers issue a syscall number the kernel rejects with `UnknownSyscall`.
  No rights check of any kind currently executes for this path, because the
  path itself is unreachable.
- **Resolution:** **ACCEPTED** pending RFC-v0.21.3-001. Deferred to v0.22: the
  durable disposition of `CapInstall` and the other 8 declared-but-undispatched
  syscalls (implement, remove from the ABI, or keep permanently reserved) is
  an open roadmap item, not decided by RFC-v0.21.3-001 itself. Not a live
  security hole — the syscall fails closed (`UnknownSyscall`) rather than
  installing with excess rights — but the doc-comments must not be read as
  describing shipped behaviour until v0.22 resolves it.

## E-012 — RFC-v0.15-003: v1.0 release checklist Step 9 bundle path

- **Claim:** `docs/release/release-checklist.md` Step 9 ("Sign all bundles")
  iterates `target/release-bundles/*.bundle` and signs each one.
- **Shipped:** `cargo xtask package-release`
  (`crates/fjell-tools/src/package_release.rs`) produces a single
  `fjell-os-v{version}.tar.gz` archive at the repository root. No code under
  `crates/` or `tools/` writes to `target/release-bundles/`, and no
  `.bundle` file is produced anywhere in the toolchain — Step 9's glob would
  match nothing.
- **Resolution:** **ACCEPTED** (architect, 2026-07-31; reclassified from the
  initial recording as OPEN). Recorded per RFC-v0.22-001 §Scope item 5.
  Declining to investigate E-012 was a deliberate owner decision
  (2026-07-30 — cutting the v1.0 checklist audit because v1.0 is not in
  view), which is ACCEPTED semantics under this register's own legend
  (a documented, deliberate limitation), on the same grounds as E-004.
  Not investigated or fixed; must be resolved before v1.0 preparation
  begins. See `docs/release/v1-limitations.md`.

## E-013 — `crates/fjell-tools/src/test_all.rs` tier 1: "Host library tests" claim

> **Scope widened 2026-08-02 (RFC-0.24-001 Pass 1).** This entry originally
> described `fjell-kernel` alone. Measured across the workspace: **40 of 89
> manifests have no lib target**, and **10 of those carry 166 `#[test]`
> functions that `--lib` never reaches.**
>
> The composition is the point. Eight of the ten are the **gate tools
> themselves**: `fjell-tools` (68, including `callsite_audit`'s — Gate 11's own
> demonstrations), `fjell-consistency-check` (26 — Gate 12's),
> `fjell-unsafe-audit` (10 — Gate 2's), `fjell-abi-snapshot` (8 — Gate 4's),
> `fjell-mmio-audit` (7 — Gate 3's), `fjell-readiness-check` (5 — Gate 5's),
> plus `fjell-repro-check` (6), `fjell-ci-coverage` (4), `fjell-summary-check`
> (2), and `fjell-kernel` (30).
>
> So the demonstrations that establish five gates as sound are themselves never
> run by the tier that claims to run the test suite. They pass when invoked
> directly; nothing in `test-all` or `release-rehearsal` would catch a
> regression in them.
>
> This is not "kernel unit tests do not run" but **"the verification tooling's
> own tests do not run under the tier that claims to run the test suite."**
>
> The follow-up RFC therefore has two separable halves: the **nine host
> binaries**, ordinary `std` crates where the gap is the bare `--lib` flag and
> the fix is trivial; and **`fjell-kernel`**, where it is architectural
> (a `[lib]` target, or splitting out a host-testable subset).

- **Claim:** tier 1 of `cargo xtask test-all` ("Host library tests",
  `cargo test --workspace --lib --exclude fjell-proptest`) verifies the
  workspace's host-side unit tests.
- **Shipped:** `crates/fjell-kernel/Cargo.toml` declares only a `[[bin]]`
  target, no `[lib]`. `cargo test --workspace --lib` silently skips any
  package with no library target — no error, no warning — so tier 1 has
  never once executed fjell-kernel's own `#[cfg(test)]` modules:
  `mm/frame_alloc.rs`, `mm/user_ptr.rs`, `task/scheduler.rs`,
  `trap/dispatch.rs` (including the RFC-v0.23-002 milestone-marker tests
  added under that RFC's Slice 1), and **`lease/mod.rs`** — the kernel-side
  lease table, one half of a Verus release-required target. The proof
  covers the predicate; these tests cover the table that invokes it, and
  neither has executed. The real target, `riscv64gc-unknown-none-elf`, is
  bare-metal with no OS and no libtest harness, so no alternate `cargo
  test` invocation reaches them either.
- **Resolution:** **CLOSED** by RFC-0.29-001. ~~ACCEPTED (architect,
  2026-08-01)~~ → fixed for the host-binary half (see the 2026-09-09
  addendum above); `fjell-kernel`'s host-testability remains a distinct,
  unaddressed architectural question — explicitly this line's non-goal,
  and, per this entry's own words, never its core claim ("this is not
  'kernel unit tests do not run'") — tracked separately rather than
  reopening this entry. Originally found during RFC-v0.23-002 Slice 1
  while writing the two-demonstration unit tests that RFC requires — they
  could not be proven to run under tier 1 or any other `cargo test`
  invocation. See `docs/release/v1-limitations.md`.

> **Second confirmation 2026-08-03 (RFC-0.24-001 Pass 4).** The six gate-tool
> crates — `fjell-abi-snapshot`, `fjell-consistency-check`, `fjell-mmio-audit`,
> `fjell-readiness-check`, `fjell-repro-check`, `fjell-summary-check` — are
> **also never named in any job in `.github/workflows/ci.yml`**, which lists its
> packages explicitly by name. So nothing runs their tests anywhere, by any
> mechanism, in ordinary operation: not `test-all` tier 1 (`--lib`, no lib
> target), and not CI (never enumerated).
>
> Not a new erratum — the same one, reached by a second independent mechanism.
> Recorded because the disclosure understates the reach without it.
>
> The three crates backing Gate 8's validation drills (`fjell-sig-ed25519`,
> `fjell-fleet-sync`, `fjell-config-sync`) are **deliberately not folded in**:
> their tests exist and are reachable, and CI simply never invokes them. That is
> **E-015**, and filing it here would blur an erratum that is currently precise.

> **Re-derived 2026-09-08, scoping RFC-0.29-001.** The figure has grown from
> **166** to **305** unreachable `#[test]` functions: **285** in crates with no
> lib target, plus **20** in integration `tests/` directories (excluding
> `fjell-proptest`'s 24, which tier 2 runs). The two largest are now
> `fjell-consistency-check` (**98** — Gate 12's ten subchecks) and `fjell-tools`
> (**86** — Gate 11's five callsite checks, including the 36 behind
> `SYSCALL-CALLSITE-001`/`-002` that 0.28 added). **Every instrument this
> project adds lands inside this erratum**, which is why it is the one of the
> four that has been actively worsening. `--bins` reaches the 285; `--tests`
> reaches the 20.

> **Fixed by RFC-0.29-001 R1, 2026-09-09 — and both figures above had
> already moved by the time they were used.** Re-derived rather than
> reproduced: **41** no-lib crates (not 40), **288** unit tests in them
> (258 host + 30 `fjell-kernel`, not 285 — `fjell-tools`'s own share grew
> to 89 while this line was being built), **27** integration-test-dir
> tests excluding proptest (not 20 — `fjell-cap` 16, `fjell-upgrade-format`
> 7, `fjell-config-sync` 2, `fjell-fleet-sync` 2). `--bins`/`--tests`
> cannot be added `--workspace`-wide as this entry's own phrasing implied:
> `crates/fjell-kernel`, every `crates/services/*`, and every
> `crates/drivers/*` crate are `#![no_std]` binaries with their own
> `panic_impl`, and either flag tries to build all 31 of them for the host
> — a compile error, not a test failure, demonstrated live. Fixed by
> deriving the exclude set from `cargo metadata` (`crates/fjell-tools/src/
> cargo_metadata.rs`, new) rather than reaching for the flag without
> checking what it broke — the same mistake this erratum was filed to
> correct in `--lib`. **`fjell-kernel`'s 30 remain unreachable** —
> unchanged, and, per the RFC's own text above, never this erratum's core
> claim ("this is not 'kernel unit tests do not run'"). See the answer
> document for the full count reconciliation and the `ci-host-bins` CI job
> that now runs the same command in ordinary CI, not only `test-all`.

## E-014 — Verification instruments that decide by matching a fixed string

- **Claim:** several instruments assert a semantic property —
  RFC-v0.22-001 Slice 4 (`errata-limitations`: *"every ACCEPTED erratum appears
  in `v1-limitations.md`"*), Gates 5/6/7, RFC 026's negative harness
  (`FORBIDDEN` markers), and `fjell-unsafe-audit`'s category tagging.
- **Shipped:** each decides by matching a fixed literal, satisfiable without the
  property holding, and unsatisfiable when the property holds in a form the
  literal does not anticipate. Found by RFC-0.24-001, Passes 1–4:
  - **Gate 5** counts rows containing `**OPEN**`. A row marked `**BLOCKED**` is
    counted in none of the four buckets — not miscounted, absent. Demonstrated.
  - **Gate 6** counts the literals `§1`..`§6` in `trust-report.txt` and discards
    the regeneration's own exit status (`let _ = sh(...)`).
  - **Gate 7** counts `OPEN` in this register.
  - **`FORBIDDEN`** matches `"TEST:FAIL"`, which is not a substring of the real
    message `TEST:M7:FAIL (init did not exit cleanly)`.
  - **`errata-limitations`** requires only that an erratum's *ID string* appear
    in `v1-limitations.md`. It passed over a live divergence in which the
    architect widened E-013 here and not there — the content disagreed while the
    ID matched.
  - **`fjell-unsafe-audit`'s category extractor** splits on whitespace and commas
    only, so `category=csr-asm; <explanation>` yields the token `"csr-asm;"` and
    falls to `Unknown`. All 283 pre-existing sites happen to use the convention
    that works; nothing enforces it.
  - **The shared TOML array parser** closes an array at a `]` inside a string
    literal, loading 2 of 4 markers silently.
- **Resolution:** **ACCEPTED**, tracked **RFC-0.29-002** — not `CLOSED`.
  ~~ACCEPTED (architect, 2026-08-03), 0.25 candidate~~ → six of the seven
  instances above are fixed by RFC-0.29-002 (2026-09-09), each demonstrated
  on the exact input the old literal missed (D1: parse the structure,
  don't match the surface — see that RFC's answer document). **The shared
  TOML array parser survives, unfixed, and is named rather than swept
  in**: `qemu_run.rs`'s multi-line-array joiner still closes an array at
  the first line *containing* `]`, not the first unquoted one, so a
  marker string with a literal `]` (e.g. `"[INTENT] ..."`) still truncates
  the array early — confirmed still live
  (`multiline_array_still_closes_early_on_a_bracket_inside_a_marker_string`).
  Not this RFC's R3 to fix (five specific instruments were named; this was
  not one of them) — left open rather than closed around, per R6's
  instruction for a surviving instance.

  **Two more instances found inside `errata-tracking` itself** while
  retracking E-015 below, in the same instrument that guards the
  register's tracking column:
  - **Fixed:** `header_claims_close`'s clause-scoped "clos" stem search
    misattributed a claim to the file being scanned when the clause's real
    subject was a *different*, explicitly-cited RFC — live and not
    hypothetical: RFC-0.29-001's own `**Relates to:**` clause reads *"RFC-
    0.28-005 (which closed one E-015 instance and established the
    template)"*, background about RFC-0.28-005's action, and the check
    flagged RFC-0.29-001 as claiming to close E-015 the moment E-015's
    tracking field moved to RFC-0.29-002 below. Fixed by comparing the
    clause's own cited RFC id (`own_rfc_id`/`find_rfc_id_in`) against the
    file actually being scanned.
  - **Found, not fixed:** the same search still does not recognise this
    project's own `**Tracks.**` field convention (RFC-0.28-003's header:
    `**Tracks.** **E-019** — ...`, no "clos" stem nearby) — confirmed as a
    real historical incident, not hypothetical: E-019's tracking field sat
    stale at `RFC-0.26-003` after RFC-0.28-003 shipped, undetected, until
    manually retracked. A fix was built (treating every `**Tracks.**`
    mention as an exclusive current claim) and reverted after it broke on
    a legitimate case in testing: this project tracks some errata across a
    *sequence* of RFCs as work continues (E-015 itself: RFC-0.29-001, then
    RFC-0.29-002), and an earlier, already-`Implemented` RFC's historical
    `**Tracks.**` mention is not a wrong claim, just a true statement about
    an earlier point in the erratum's life. Left open rather than shipped
    with a demonstrated false-positive.

  See `docs/verification/instrument-audit-closeout.md` §3.1 and
  `docs/release/v1-limitations.md`.

## E-015 — Hand-enumerated instrument scopes that no longer match reality

- **Claim:** RFC 025 (CI/QEMU automation foundation) and RFC 026 (negative test
  harness) present CI and the negative-test harness as covering the workspace
  and its negative categories.
- **Shipped:** both enumerate their subjects by hand, and the lists have drifted.
  Found by RFC-0.24-001 Passes 2 and 4:
  - **21 of 91 workspace crates are never named in any `ci.yml` job.** *(Measured 2026-08-27. This figure read **19 of 89** until today: it went stale when `fjell-driver-uart` was added in `0.25.0` and again when `fjell-os` was added, both times unnoticed. RFC-0.27-001's S5 exists for exactly this.)* Six are
    the gate tools (see E-013); three back Gate 8's validation drills
    (`fjell-sig-ed25519`, `fjell-fleet-sync`, `fjell-config-sync`), whose five
    markers therefore run only at `release-rehearsal` time and never on a push
    or PR. Possibly intentional; nothing in the workflow says so.
  - **`ci-qemu-negative`'s matrix lists nine categories; `test_all.rs` runs
    ten.** The `semantic` category, added by RFC-v0.23-001, has never run in
    ordinary CI since the RFC that introduced it.
  - **`KNOWN_V01X_CATEGORIES` / `KNOWN_V02_CATEGORIES`** no longer describe the
    profiles on disk.
  - **`smoke.rs`'s `v0.6-verification` milestone** appears in neither CI matrix
    nor `SMOKE_PROFILES` — defined in code, invoked by nothing, anywhere.
    Vestigial from a naming transition.
- **Resolution:** **CLOSED** by RFC-0.29-002. ~~ACCEPTED, tracked
  RFC-0.29-001~~ — three of the four bullets above were fixed by
  RFC-0.29-001 (2026-09-09), leaving `smoke.rs`'s vestigial
  `v0.6-verification` milestone as the one named, surviving instance.
  **Retracked from RFC-0.29-001 to RFC-0.29-002 deliberately**, not because
  a gate prompted it: two live RFCs cannot both claim the same erratum,
  and `errata-tracking` (this same subcheck) correctly refused the
  alternative while RFC-0.29-002 was still being written to fix it.
  RFC-0.29-002 R5 deletes the dead match arm and its usage-string mention
  (no kernel/service code has ever emitted `TEST:V0.6-VERIFY:PASS`, so
  "wire it up" was not an available choice) — `cargo xtask qemu-test
  v0.6-verification` now correctly reports `unknown milestone` rather than
  silently accepting a name nothing backs. Distinct in kind from E-014:
  these were checks that **did not run**, or ran over an incomplete set —
  not checks that report success without checking.

> **Re-derived 2026-09-08, scoping RFC-0.29-001.** The negative-test categories
> are enumerated in **five** places and no two agree: `test_all.rs`'s
> doc-comment says *"× 9 categories"*; `NEG_CATEGORIES` three lines below holds
> **12**; `ci.yml`'s matrix lists **9**; there are **15** profiles on disk; and
> `KNOWN_V01X_CATEGORIES` + `KNOWN_V02_CATEGORIES` hold 13 entries including an
> alias. Concretely: **`semantic`, `uart-rx` and `uart-rx-unbound` run in
> `test-all` and have never run in ordinary CI.** The crate figure is now
> **23 of 93** never named in `ci.yml` — it read 21 of 91 and went stale exactly
> as this entry describes.

> **Partially fixed by RFC-0.29-001, 2026-09-09 — three of four bullets
> above, not all.** The crate figure was re-derived again rather than
> trusted: **21 of 91** today, not 23 of 93 — the same number the
> 2026-08-27 measurement found, reported as measured rather than
> reconciled against either prior figure. `NEG_CATEGORIES`, `ci.yml`'s
> hardcoded matrix, and `KNOWN_V01X_CATEGORIES`/`KNOWN_V02_CATEGORIES` are
> all **removed** — `qemu_run::discover_negative_categories` (derived from
> `tests/qemu/profiles/*.toml`) is the one answer all three call sites
> use now, including `ci.yml` via a new `ci-negative-matrix` job and
> `cargo xtask list-negative-categories`. `fjell-sig-ed25519`,
> `fjell-fleet-sync`, `fjell-config-sync`'s tests now also run in ordinary
> CI (`ci-host-bins`, R1's own fix) — closing this bullet's "possibly
> intentional; nothing in the workflow says so" ambiguity: it was not
> intentional, and now it runs. **`smoke.rs`'s `v0.6-verification`
> milestone bullet is untouched** — `smoke.rs` is outside this line's
> `Touches`, and the same defect shape recurring there is a finding to
> report, not a scope to fold in. Full reconciliation and the
> `fjell-driver-uart`/`fjell-svc-fault`/`fjell-svc-timeout` cross-check
> gap (a `cargo check` gap, not a test gap — all three have zero
> `#[test]`s) are in the answer document.

> **Closed by RFC-0.29-002 R5, 2026-09-09.** The one surviving instance —
> `smoke.rs`'s `v0.6-verification` match arm, mapping to a marker
> (`TEST:V0.6-VERIFY:PASS`) no kernel or service code has ever emitted,
> and to a profile (`v0.6-verify`) neither `SMOKE_PROFILES` nor any
> `ci.yml` job has ever run — is deleted, along with its mention in the
> usage string. Demonstrated: `cargo xtask qemu-test v0.6-verification`
> now reports `unknown milestone` (the same fail-closed path RFC-0.24-002
> Slice 2 already built for a typo), rather than accepting a name with
> nothing behind it. No instance survives.

## E-016 — No instrument verifies any document link, index, or count

- **Claim:** RFC 000 (RFC Lifecycle Policy) makes `rfcs/README.md` the
  repository's RFC index, and the documentation set is presented as
  cross-navigable.
- **Shipped:** nothing checks any of it. Found by RFC-0.24-001 Pass 3 and the
  0.24 review cycle:
  - **`rfcs/README.md` has zero instrument coverage.** The only trace of it
    anywhere in the instrument set is a **doc comment in
    `rfc_status_folder.rs` that mentions the file without opening it.** A search
    for coverage found a sentence claiming coverage.
  - **13 broken relative links** in tracked documentation.
  - **The audit's own totals table drifted** — maintained as prose arithmetic
    across four passes and three RFCs, by two parties, with nothing checking
    it; it stated a population of 56 while summing to 54. Found by the
    implementer during RFC-0.24-003 and corrected in review. This instance is
    inside the verification record itself.
  - **The index's "Shipped" column names a release for roughly 150 rows as
    `v0.3.0`, `v0.22.0`, and so on — tags that do not exist under those names.**
    Release tags have never carried a `v` prefix. Its section headers do the
    same. The 0.24 series was renamed to match (2026-08-03); historical rows
    were left, because renaming ~150 files to apply a convention retroactively
    would break the links that commits and release records point at.
- **Resolution:** **ACCEPTED** (architect, 2026-08-03). Recorded, not fixed.
  One link-and-count integrity instrument closes all three, and **adding an
  instrument was RFC-0.24-001's explicit non-goal** — which is why this waits
  for 0.25 rather than being fixed quietly by the person who would then write
  the checker. The drift and the reason nobody noticed it are the same finding.
  See `docs/verification/instrument-audit-closeout.md` §3.3.

## E-017 — RFC-0.24-001: "every instrument claimed as sound has a committed demonstration"

- **Claim:** RFC-0.24-001's acceptance criteria require that *"every instrument
  claimed as sound has a **committed demonstration of it failing**"*, and its
  handoff §0.1 states that *"an instrument with no demonstration is recorded
  `UNAUDITED`, never `sound`."*
- **Shipped:** the criterion held for most rows and demonstrably failed for two,
  both caught only in review and on opposite sides of the review boundary:
  - **`ci-proptest`** was certified `sound` on the completeness of its crate
    list. The list was correct; the predicate (`--lib`) was never examined, and
    the job ran **zero** tests.
  - **`Gate 4 — ABI snapshot verify`** was certified `sound`, by the architect
    in Pass 1, because the tool's own unit suite passed. A tool's unit tests
    passing is not the gate observed failing on a broken repository state — it
    is **mode 2, proxy attestation**, the taxonomy's own second entry, and it is
    why a 45-item identity collapse stayed invisible.

  Both were repaired (RFC-0.24-002 Slice 6; RFC-0.24-003). **The re-derivation
  of the remaining `sound` rows against the same question — *was a demonstration
  produced, or was something else mistaken for one?* — is incomplete.** Gate 4
  was the first re-derived and it fell immediately, so the base rate is not
  known to be low.
- **Resolution:** **CLOSED** by RFC-0.29-002. ~~ACCEPTED (architect,
  2026-08-03), 0.25 candidate~~ — the 22 `sound` verdicts were provisional
  pending re-derivation; all are now re-derived or corrected (see the
  2026-09-09 addendum below), and no instance survives. This is why
  RFC-0.24-001 shipped `Implemented-with-Errata` rather than `Implemented`:
  its normative text claimed more than the merged work verified at the
  time. See `docs/verification/instrument-audit-closeout.md` §4.1 and
  `docs/release/v1-limitations.md`.

> **Counted 2026-09-09, at the owner's request, before scheduling.** E-017 was
> read as *"two rows verified, twenty assumed."* Going through the register row
> by row, the remainder is **smaller than that and differently shaped**, and a
> large part of closing it is updating the register rather than producing new
> demonstrations.
>
> **The register holds 21 `sound` rows. Its own summary table (line 1418) says
> 22.** The per-pass cells sum to 22; the `###` headings number 21. One of the
> two is wrong, and this is the same table whose arithmetic was corrected once
> before, in the RFC-0.24-002 review.
>
> Classified by the *basis* each row actually states — not by whether the word
> "demonstration" appears, which would be E-014's predicate applied to E-014's
> own audit:
>
> | Basis | Rows | Which |
> |---|---|---|
> | **First-hand demonstration** — broke an input, ran the instrument, saw it fail | **8** | Gate 1, Gate 8, Tier 2, Tier 3b, `abi/snapshot.json`, `ci-format`, `ci-unsafe-audit`, `ci-schema-gate` |
> | **The tool's own unit suite**, cited as the demonstration | **4** | Gate 2, Gate 11, Gate 12, `syscall/expected.toml` (inherits Gate 12's) |
> | **Inherited** — "covered by another row's demonstration; not repeated" | **2** | Tier 3 (from Gate 2), Tier 3c (from Gate 3) |
> | **No demonstration at all** | **4** | `fjell-abi-snapshot` ×2 (repaired inside RFC-0.24-003), `repro/baseline-digests.txt` (sound by citing Tier 3b), `ci-arm64-check` (reasoned: *"narrowly scoped by design… No finding"*) |
> | Cited to a prior RFC / repaired since | 3 | Gate 3, Gate 4, `ci-proptest` |
>
> **The four in row 2 are the exact defect this erratum names.** E-017 records
> that Gate 4 was certified `sound` because *"the tool's own unit suite
> passed"*, and calls that **mode 2, proxy attestation**. Gate 2, Gate 11 and
> Gate 12 cite `cargo test -p <tool>` in precisely the same way. **Gate 4 was
> re-derived by RFC-0.24-003; the other three never were.**
>
> **But the conclusion mostly holds anyway, and that is the useful part.** Real
> demonstrations for those instruments exist now — Gate 2 on a live category
> violation (0.24.0 release record), Gate 11's `SYSCALL-CALLSITE-001`/`-002`
> (RFC-0.28-002, RFC-0.28-004), Gate 12's ten subchecks (RFC-0.27-001,
> RFC-0.27-003, RFC-0.27-004). **The register's stated basis is weaker than the
> evidence that now exists, and the register cannot tell you which rows those
> are.**
>
> **So E-017 is roughly: 4 rows needing a demonstration produced, 4 rows needing
> their basis corrected to cite work already done, 2 inheritances to accept or
> re-derive, and one summary-table count to reconcile.** That is a slice, not a
> milestone — and it is smaller than the architect's own 2026-09-08 brief
> implied when it guessed E-017 "may already be satisfied by RFC-v0.22-001." It
> is not satisfied; it is just less work than "twenty assumed" suggests.

> **Closed by RFC-0.29-002 R4, 2026-09-09 — and the "21 vs 22" reconciliation
> above found the wrong number wrong.** Recounting `### ... — **sound**`
> headings directly: **22**, not 21. The 2026-09-09 recount above missed
> `ci-verus`, whose heading reads `**sound (by explicit design)**` — a
> differently-bolded phrase a literal `"**sound**"` boundary check does not
> match. **The summary table's 22 was correct the whole time; the recount
> checking it was itself defeated by inconsistent formatting** — the exact
> defect family E-014/this RFC exists to fix, found live inside the audit
> record correcting for it. `ci-verus`'s row (Pass 4) verifies its own claim
> directly against the real `ci.yml` config (`continue-on-error: true`,
> matching the job's own comment) — a sixth basis, distinct from the five
> above, needing no work:
>
> | Basis | Rows | Which |
> |---|---|---|
> | First-hand demonstration | 8 | (unchanged) |
> | Tool's own unit suite (the defect) | 4 | (unchanged) |
> | Inherited | 2 | (unchanged) |
> | No demonstration at all | 4 | (unchanged) |
> | Cited to a prior RFC / repaired since | 3 | (unchanged) |
> | **Verified directly against real config ("sound by design")** | **1** | `ci-verus` |
>
> 8+4+2+4+3+1 = **22**, matching the summary table exactly. `docs/verification/
> instrument-audit.md`'s totals table needed no edit; this erratum's own prior
> addendum did, and does now.
>
> **All required actions complete:**
> - **4 demonstrations produced** (not corrected citations — new, real runs):
>   `fjell-abi-snapshot` ×2 (`strip_fn_modifiers` reverted to the pre-repair
>   enumerated-prefix shape, `sys_audit_drain_ptr`/`_raw` confirmed absent
>   from the real `fjell-syscall` scan; the five-field identity key checked
>   for zero collisions across the real, current 420-item snapshot),
>   `repro/baseline-digests.txt` (one real digest byte corrupted, caught as
>   `DIGEST DIFFERS`), `ci-arm64-check` (a real type error appended to
>   `fjell-arch-arm64`, caught by the exact CI command). All reverted;
>   `git status` clean after each.
> - **4 citations corrected** (not new demonstrations — pointed at evidence
>   that already existed): Gate 2 → the 0.24.0 release record's live
>   category-violation catch; Gate 11 → `SYSCALL-CALLSITE-001`/`-002`'s own
>   regression tests (RFC-0.28-002/-004); Gate 12 → one named "fails on the
>   input it exists to catch" test per subcheck, all ten, verified passing;
>   `syscall/expected.toml` checked and found to already cite its own
>   specific tests, not a proxy — no correction needed, only confirmation.
> - **2 inheritances** (Tier 3 from Gate 2, Tier 3c from Gate 3) accepted:
>   both source rows now carry real, non-proxy demonstrations, so the
>   inheritance is no longer resting on anything weak.
> - **Count reconciled**: 22, corrected above.

## E-018 — `task::scheduler::PRIORITY_USER` has three disconnected copies, two values

- **Claim:** every spawned user task, including `init`, runs at the same
  ready-queue priority bucket, so the round-robin scheduler gives each a fair
  turn. `task::scheduler::PRIORITY_USER = 32` is the one real constant this
  implies.
- **Shipped:** two other, disconnected copies exist with a different value:
  `task/spawn.rs`'s local `const PRIORITY_USER: u8 = 2`, used for every task
  spawned through `spawn()` (i.e. every service except `init`, which is
  constructed by a separate hand-rolled path in `main.rs` using the real
  `32`), and a third hardcoded literal `2` in `trap/syscall.rs`'s
  `sys_task_start`, which ignores `Task.priority` entirely at enqueue time.
  `priority_to_bucket` places 2 and 32 in different buckets
  (`(p as usize) * 8 / 256`), and `Scheduler::choose_next` always drains the
  higher bucket first — so **`init` preempts every other spawned task
  whenever both are ready.**

  Invisible until RFC-0.25-001: every existing `init` code path that waits on
  another service uses a *blocking* `sys_ipc_recv` (`wait_service_ready`,
  `wait_storaged_ready`, `wait_ready_exact`), which removes `init` from the
  ready queue entirely and sidesteps the bug by construction — nothing before
  RFC-0.25-001 ever yield-looped from `init` while another service was still
  starting. RFC-0.25-001's `init` needed a *non-blocking* poll for its uart-rx
  byte (a blocking wait would hang every QEMU profile that never types
  anything), and that poll starved `crates/drivers/fjell-driver-uart`
  completely — `init`'s poll budget exhausted and moved on before
  `driver-uart`'s own spawn code ever ran a single instruction.

  Fixing the constant directly was attempted and reverted: it hung the M6
  boot sequence, meaning some already-shipped code path depends on today's
  (broken) ordering in a way not yet understood. RFC-0.25-001 shipped a
  narrow, `image_id`-keyed stopgap instead — `crates/drivers/
  fjell-driver-uart` alone spawned at `init`'s own priority bucket — rather
  than correcting the general constant.
- **Resolution:** **CLOSED** by RFC-0.26-001. The M6 hang was investigated
  before anything was unified (D1): `svc-timeout`
  (`crates/services/fjell-svc-timeout`) is RFC 042's negative-test service
  and *by design* never exits, looping `sys_yield()` forever. Its first
  enqueue used `sys_task_start`'s hardcoded `2` (bucket 0); every enqueue
  after its first yield used `task.priority`, set by `spawn.rs`. Changing
  only `spawn.rs`'s constant to `32` moved `svc-timeout`'s *ongoing*
  re-enqueues to bucket 1 while leaving newly spawned M6 services
  (`devmgr`, `driver-virtio-blk`, `storaged`) enqueuing into the still-`2`
  bucket 0 via `sys_task_start` — bucket 1, permanently occupied by a task
  that never blocks or exits, was drained on every scheduling decision, and
  bucket 0 was never reached again. Full explanation, with log evidence:
  `docs/rfcs/RFC-0.26-001-scheduler-priority-unification-investigation.md`.

  The fix unifies both enqueue paths to the same value —
  `task/spawn.rs` now imports `task::scheduler::PRIORITY_USER` directly
  (no local shadow), and `trap/syscall.rs::sys_task_start`'s initial enqueue
  reads `task.priority` instead of a disconnected literal — so the two
  paths can no longer disagree. The `driver-uart` stopgap is removed
  entirely; `uart-rx`/`uart-rx-unbound` both pass without it. No
  `PRIORITY_INIT` was introduced — nothing in the investigation showed
  `init` needs to be genuinely privileged.

## E-019 — The `ipc` negative profile assumes an unsynchronised scheduling order

- **Claim:** `tests/qemu/profiles/ipc.toml` and `tests/qemu/profiles/
  semantic.toml` are fail-closed, permanent regression coverage —
  `test-all`'s own framing for every profile in `NEG_CATEGORIES`.
- **Shipped:** both reproducibly fail after RFC-0.26-001 unified the
  scheduler priority (`cargo xtask qemu-run --profile ipc` /
  `--profile semantic`, deterministic across repeated runs — QEMU TCG is
  fully deterministic given the same binary and inputs, so this is not
  flakiness).

  - **`ipc`:** `fjell-neg-test` reaches `NEG:SVC:FAULT_DETECTED:PASS` (the
    scenario immediately before the IPC block) and then never reaches any
    of `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE`,
    `NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE`, or `NEG:IPC:LATE_REPLY_REJECTED`.
    `test_ipc_blocked_recv` (`crates/services/fjell-neg-test/src/main.rs:435-439`)
    documents its own assumption in a comment: *"By the cooperative-
    scheduling contract, sample-service immediately calls
    `sys_ipc_recv(SLOT_LEASED_EP)` and blocks before the scheduler returns
    to neg-test. One defensive yield is included for safety."* That
    contract no longer holds exactly as assumed once every task shares one
    priority bucket rather than `init`-adjacent tasks preempting freely.
  - **`semantic`:** `sample-service`'s `emit_sample_intent()`
    (`crates/services/fjell-sample-service/src/main.rs:141-144`) is called
    once at startup on the documented assumption that *"semantic-stream and
    proxy-text are already spawned and ready by this point"* — asserted,
    not synchronised on. `M5: semantic-stream started` / `M5: proxy-text
    started` / `M5: semantic policy loaded` each now print **twice** (two
    services' boot lines happening to share identical text, not a double
    spawn — the same coincidence RFC-0.26-001's investigation document
    records for `storaged`'s and `init`'s identical "M6: storaged ready"
    lines), and `sample-service demo intent` /
    `proxy-text: action DENIED (capability not held)` never appear.

  Both are the same root cause as the M6 hang this RFC investigated and
  fixed (docs/rfcs/RFC-0.26-001-scheduler-priority-unification-
  investigation.md) — code that assumes a specific relative scheduling
  order between concurrently-running tasks rather than synchronising on it
  explicitly — surfacing in a different shape (a silently-skipped assertion
  rather than a total hang) because the assumption here is about *relative
  arrival order* between two already-running peers, not about one bucket
  permanently starving another.
- **Resolution:** **ACCEPTED** (implementer, pending review, RFC-0.26-001).
  Per the governing RFC's explicit instruction (§2, "Expect collateral, and
  do not absorb it... do not chase it"): reproduced and characterised, not
  fixed. Fixing either requires the affected service to synchronise
  explicitly (a real `READY`/rendezvous exchange) rather than assuming
  ordering — that is out of RFC-0.26-001's scope (it unifies the scheduler
  constant; it does not audit every service for ordering assumptions) and
  is real design work for its own line. `cargo xtask test-all` is 19/21
  with these two tiers failing; every other tier, including the two new
  RFC-0.25-001 uart-rx profiles, passes. See `docs/release/v1-limitations.md`.
- **Correction and closure (RFC-0.28-003, 2026-09-08).** This entry's scope
  narrowed to the `ipc` profile only once E-020 was filed separately for the
  `semantic` profile's distinct consequence (a shipped feature not
  executing, vs. this entry's lost test coverage). **The tracking field was
  left pointing at RFC-0.26-003 while this RFC's own predecessor
  (RFC-0.26-003, since superseded) held it, and while the superseding RFC
  was `proposed/` — two live RFCs cannot claim one erratum, and
  `errata-tracking` correctly refuses that; the field was retracked to
  RFC-0.28-003 deliberately, once accepted, not by watching a gate that
  could not have caught the wrong pointer anyway** (its "claims to close"
  predicate is a literal string match, blind to the phrasing difference —
  the same E-014 instrument-fragility family, in the field that tracks it).
  RFC-0.26-003's own conclusion — "there is no signal to wait on, and none
  can be trivially built" — was itself false; it reasoned only about a
  blocking task announcing its own state and never considered polling the
  kernel, which is the authority on the state in question. **CLOSED** by
  RFC-0.28-003: `fjell-neg-test::test_ipc_blocked_recv` now polls
  `sys_task_status` on `fjell-sample-service`'s kernel-attested `TaskId`
  (learned via a one-way identity exchange on their already-dedicated,
  uncontested endpoint — RFC 042's object 6 — not the shared object 0),
  bounded, failing closed on exhaustion rather than falling through to the
  revoke. Demonstrated live: with the wait removed, `sys_task_status` reads
  `Runnable`, not `Blocked`, and the profile now correctly reports FAIL.

## E-020 — RFC-v0.23-001: the ABDD live path no longer runs

- **Claim:** RFC-v0.23-001 (shipped `0.23.0`) made this project's
  distinguishing architectural bet actually execute — `sample-service` emits an
  intent, `semantic-stream` routes it, and a *separate* `proxy-text` task
  renders it, with the capability-checked refusal demonstrated alongside the
  accept. `tests/qemu/profiles/semantic.toml` was created **in that same RFC as
  a fail-closed guard so the path could not rot.**
- **Shipped (from RFC-0.26-001 onward):** the path does not run at all.
  `crates/services/fjell-sample-service/src/main.rs` calls
  `emit_sample_intent()` once from `service_main()` under the comment
  *"semantic-stream and proxy-text are already spawned and ready by this point
  (Slice 1)"* — an **assertion about scheduling order, not a synchronisation**.
  RFC-0.26-001 removed the priority asymmetry that assertion silently depended
  on. Measured on the current tree: **zero occurrences** of
  `sample-service demo intent` or `proxy-text: action` in the profile's serial
  log. The guard is now permanently red and therefore detects nothing.
- **Why this is OPEN and not ACCEPTED.** It was first filed as part of E-019,
  ACCEPTED, on the reading that both failing profiles were *"negative-test
  coverage gaps, not production-path defects — only two coordination-timing
  assertions in test harnesses."* That holds for `ipc`, whose assertion lives in
  `fjell-neg-test`, a harness. It does **not** hold here:
  `fjell-sample-service` is a service under `crates/services/`, the race is in
  its `service_main()`, and the consequence is not lost coverage but **a shipped
  feature that no longer executes**. ACCEPTED means a documented, deliberate
  limitation; this is live drift in a released capability, which is the
  register's own definition of OPEN.
- **Consequence, deliberately.** Gate 7 (`ERRATA register (0 OPEN)`) now fails,
  so `release-rehearsal` is red and **no release can be cut until this is
  fixed.** That is the gate doing its job. Classifying it ACCEPTED was the
  choice that would have kept the gate green while the ABDD path was dead.
- **Resolution:** **CLOSED** by RFC-0.26-004. `sample-service` now
  synchronises rather than asserts: `emit_sample_intent()`'s underlying
  transport (`fjell_service_api::chunked::send`) is a blocking `sys_ipc_call`,
  which queues and blocks the caller until `semantic-stream` actually reaches
  its receive loop and replies (`SendResult::Queued`) — a real wait, not a
  timing assumption. This is safe under RFC-0.26-004's established invariant
  (see E-021's resolution below): `semantic-stream`'s and `proxy-text`'s
  endpoints each have exactly one receiver (the service itself), so the call
  can never be delivered to, and dropped by, anyone else. Confirmed live: all
  four `semantic.toml` markers pass, and the serial log shows the causal
  order — `semantic-stream` validating and forwarding the intent precedes
  `sample-service`'s own `"intent emitted"` print, and `proxy-text`'s
  accept/deny both fire from the forwarded envelope — not merely both present
  in isolation.

## E-021 — `init::wait_ready_exact` silently consumes and drops other tasks' IPC

- **Claim:** RFC 058's readiness protocol, and `fjell-init`'s use of it, treat a
  service's dedicated endpoint as the channel on which that service announces
  itself ready. `wait_ready_exact(ep, expected)` is documented as waiting *"for
  exactly one expected READY tag"*.
- **Shipped:** `crates/services/fjell-init/src/main.rs:147` loops on a blocking
  `IpcRecv` and, when the tag does not match, **discards the message with no
  `else` branch** — not replied to, not re-queued, not logged.

  `init` holds receive-capable capabilities to endpoint objects **7** and **8**
  (slots 6 and 7), which are also `semantic-stream`'s and `proxy-text`'s *own*
  endpoints, and which `sample-service` and `semantic-stream` hold send-capable
  capabilities to for ordinary protocol traffic. **Two tasks receive on one
  queue with nothing arbitrating between them.**

  A blocking `sys_ipc_call` consumed by `init` therefore **leaves its caller
  blocked forever**, because the task that received the message does not know it
  owed a reply. Observed live during RFC-0.26-002: `init` consumed
  `semantic_stream::PUBLISH_BEGIN` (`0x501`), the first word of
  `sample-service`'s `chunked::send`, and looped back to `recv`.

  Not a probabilistic race that used to get luckier: it is unsafe on any
  endpoint another task can call into. RFC-0.26-001's scheduling change only
  altered where it lands.
- **Resolution:** **CLOSED** by RFC-0.26-004, cleanly, with no residual hazard.
  `wait_ready_exact` — the function with the missing `else` — is **removed
  entirely**, not patched: `init` no longer receives on endpoint objects 7 or
  8 at all. Its capability to object 7 (slot 6) is narrowed from
  `ALL_NON_META` to `CALL` only (its one remaining use is `emit_envelope`'s
  outbound `ipc_call`, checked against `CapRights::CALL`, not `SEND`/`RECV` —
  even a future `sys_ipc_recv` added there would fail the rights check, not
  silently reintroduce the hazard); its capability to object 8 (the old slot
  7) is removed outright, since `init` never sent to `proxy-text` directly.
  **Invariant established: a service's endpoint has exactly one receiver —
  the service itself.** No other task holds a receive-capable capability to
  either endpoint. See
  `docs/rfcs/RFC-0.26-004-readiness-channel-answer.md` for the full design
  answer and the rejected alternatives.

## E-022 — `sys_ipc_send`'s one-way path blocks the sender against its own documented contract

- **Claim:** `sys_ipc_try_send`'s doc-comment (`crates/fjell-syscall/src/
  lib.rs:278-279`) states *"One-way IPC send (no reply expected). If no
  receiver is waiting the message is queued."* — describing a non-blocking
  fire-and-forget contract: the call returns, the message waits.
  RFC-0.26-004's own handoff (§0.1) independently asserted the same reading
  of `sys_ipc_send`'s `SendResult::Queued`: *"the READY message is not
  dropped — it queues,"* framing the remaining race as *"who dequeues
  first,"* not whether the sender itself proceeds.
- **Shipped:** `sys_ipc_send`'s `Ok(SendResult::Queued)` arm
  (`crates/fjell-kernel/src/cap/syscall.rs:540-544`) calls `block(tasks,
  sched, cur_id)` — it suspends the **calling** task, exactly like a
  two-way `sys_ipc_call` would, whenever no receiver is currently waiting.
  Nothing sets up a reply edge for this case (unlike `sys_ipc_call`'s
  `Queued` arm, which the later `sys_ipc_recv`/`sys_ipc_reply` path
  correctly wires up), so wake-up depends entirely on some other task later
  calling `sys_ipc_recv` on the same endpoint and dequeuing the message —
  the recv-side handler does then call `wake()` on the original sender for
  a one-way message, but only *if* some other task ever reaches that
  `recv`.

  **This self-deadlocks a task that announces into an endpoint only it will
  ever receive on.** Found live while implementing RFC-0.26-004: with
  `init` correctly removed as a co-receiver of `semantic-stream`'s and
  `proxy-text`'s endpoints (E-021's resolution, establishing "exactly one
  receiver"), both services' pre-existing `send_ready()` call — a one-way
  `sys_ipc_send` into their *own* endpoint, issued before either task first
  reaches its own `recv_call()` — queued against a queue only that same
  task could ever drain, blocking it permanently. Confirmed by instrumented
  diagnostic (`sys_debug_writeln` immediately before/after `send_ready()`
  and around the loop's `recv_call()`, added and removed during
  investigation): the task's own post-`send_ready()` debug lines never
  printed, in a build otherwise confirmed alive and scheduled.

  Previously masked, not previously safe: under the design E-021 replaces,
  `init` also held a receive capability to these same endpoints and reached
  its own blocking `wait_ready_exact` receive almost immediately after
  spawning each service — so by the time `send_ready()` ran, a receiver was
  already waiting, hitting `SendResult::Delivered` rather than `Queued`, and
  the sender never blocked. Removing that accidental, unsynchronised
  co-receiver (correctly, per E-021) removed the cover along with it.
- **Resolution:** **CLOSED** by RFC-0.27-002. The prior recording
  classified this as a kernel defect outside RFC-0.26-004's authorised
  `Touches`; RFC-0.27-002 investigated further and found **the kernel is
  correct** — `sendq`/`recvq` are waiter queues implementing coherent
  rendezvous IPC deliberately and symmetrically with `RecvResult`, not a
  message buffer with a bug. The defect was entirely in the userspace
  wrapper: `sys_ipc_try_send` was named as though it tries and documented
  as though it queues and returns, when the kernel has never offered a
  non-blocking one-way send. **Fixed by renaming** `sys_ipc_try_send` →
  `sys_ipc_send` and rewriting its doc-comment
  (`crates/fjell-syscall/src/lib.rs`) to state the actual contract: it
  blocks the caller until a receiver takes the message, and `WouldBlock`
  means the *waiter* queue is full, not a message buffer. The three
  normative docs describing the old, incorrect contract
  (`docs/src/abi/ipc-register-layout.md`, `docs/src/api/syscalls.md`,
  `docs/src/external-design/ipc.md`) are corrected in the same commit.

  **Correction to this entry's own prior claim.** It previously stated *"no
  other one-way `sys_ipc_send`/`sys_ipc_try_send` call site in the current
  tree is known to send into an endpoint the sender itself exclusively
  receives on."* RFC-0.27-002's R2 audit found this incomplete: five more
  services share the identical self-targeting shape, each currently
  surviving it because the sender genuinely **blocks** — `block(tasks,
  sched, cur_id)` at `cap/syscall.rs:540` — and is later woken by the
  matching `wake()` at the receive side (`cap/syscall.rs:605`) once `init`'s
  own wait reaches that endpoint; confirmed live via the kernel's own audit
  ring, not inferred from timing (`fjell-attestd`'s `send_ready()`: `arg1 ==
  0`, i.e. `Queued`, immediately before `init`'s matching wait resolves it).
  **Filed as its own erratum, E-024**, rather than restated here: describing
  a live defect only inside the text of an erratum this RFC closes would
  leave it tracked by nothing once E-022 closes. See E-024 for the full
  finding, and
  `docs/rfcs/RFC-0.27-002-one-way-send-contract-answer.md` for the audit and
  the design-answer document naming this a real, six-instance, unmet
  primitive need — not decided here. May be relevant to **E-019 /
  RFC-0.26-003**'s `ipc` investigation — flagged, not absorbed. See
  `docs/release/v1-limitations.md`.

## E-023 — RFC-v0.7.1-001: the release tool's `RELEASE.md` and consistency checks were never built

- **Claim:** RFC-v0.7.1-001 (`Implemented (v0.7.1)`) specifies that each release
  tarball carries a root `RELEASE.md` containing *"exact git commit (or
  'unreleased') and tag, exact Rust channel and toolchain components used, the
  precise cargo invocations that produced the headline counts, SHA-256 of every
  prebuilt `.bin`… known-broken items quoted from CHANGELOG"*, and that the
  release tool **generates** it. Its §Implementation lists five behaviours for
  that tool.
- **Shipped:** `crates/fjell-tools/src/package_release.rs` is 121 lines and
  contains no digest, manifest, generation, or grep logic. Of the five specified
  behaviours, **one shipped**:

  | Specified | Shipped |
  |---|---|
  | reads `Cargo.toml` version | **yes** |
  | greps for stale version mentions outside `CHANGELOG.md` | **no** |
  | generates `RELEASE.md` with the headline command and counts | **no** |
  | generates a file-digest manifest of `crates/fjell-kernel/prebuilt/` | **no** |
  | exits non-zero on any inconsistency | **no** |

  `package-release` tars the repository root with exclusions, so whatever
  `RELEASE.md` sat at the root was what shipped. That file was a ten-line
  signpost carrying three links and **none of the five specified contents**. It
  was removed on 2026-08-27 as serving no purpose; its removal turns a
  misleadingly-present artefact into a cleanly absent one, and does not change
  whether the RFC's claim is met.

  **The second row is the one that matters.** A check that grepped for stale
  version mentions outside `CHANGELOG.md` is exactly what would have caught
  `README.md` sitting at `0.21.3` — with five wrong counts — through five
  releases, found only when the owner asked. The instrument that would have
  caught it was specified, marked `Implemented`, and never written.
- **Resolution:** **ACCEPTED** (architect, 2026-08-27). Recorded, not fixed.
  Same family as **E-016** (nothing verifies a document's claims) and the same
  shape as **E-012** (a checklist step referencing a build output that does not
  exist). Building it is a real instrument, which E-016's own disposition
  already carries as a 0.27 candidate — this entry gives that candidate a
  concrete, already-specified starting point rather than a blank page. See
  `docs/release/v1-limitations.md`.

---

## E-024 — `init` co-receives on four services' own endpoints; RFC-0.26-004's one-receiver invariant is narrower in the tree than in its text

- **Claim:** RFC-0.26-004 established the invariant *a service's endpoint has
  exactly one receiver*, and removed `init` as a co-receiver of
  `semantic-stream`'s and `proxy-text`'s endpoints (objects 7 and 8) to satisfy
  it.
- **Tree:** the invariant holds for objects 7 and 8 and **is violated on
  objects 1–4**. `crates/fjell-kernel/src/task/spawn.rs:204` gives `storaged`,
  `measuredd`, `attestd` and `recoveryd` their own endpoint objects; each
  service both announces into that object (raw `core::arch::asm!("li a7, 20")`,
  bypassing the wrapper) and later receives protocol traffic on it, while
  `init`'s `wait_service_ready` / `wait_storaged_ready`
  (`crates/services/fjell-init/src/main.rs:118`) hold full receive rights on the
  same four objects. That co-receive is not incidental — **it is what keeps the
  announcement from deadlocking.** RFC-0.27-002's R2 audit found this; it was
  not known when RFC-0.26-004 wrote the invariant.
- **Second defect, same site.** `wait_service_ready` loops on a blocking recv
  and `break`s only on `MREADY | AREADY | RREADY`; a message carrying any other
  tag is **consumed and discarded**, and if it was a `call`, its sender is never
  replied to. That is the identical missing-`else` defect `wait_ready_exact`
  had, recorded as **E-021** and closed only for the two objects RFC-0.26-004
  touched. It remains live on four more.
- **Why this is filed separately.** RFC-0.27-002 disclosed all of the above,
  accurately and in detail — **inside E-022's entry, which that RFC closes.**
  A live defect described only in the text of a CLOSED erratum is tracked by
  nothing: Gate 7 counts `OPEN`, and `errata-tracking` derives from the
  erratum↔RFC link, so once E-022 closed, this would have left the register
  with no row and become archaeology. The disclosure was right; the placement
  made it invisible.
- **Root cause found 2026-09-06, while scoping RFC-0.28-001** — and it is one
  line, in a table nobody associated with readiness. Every service announces to
  capability **slot 0**; `crates/fjell-kernel/src/task/spawn.rs:204` decides what
  slot 0 *means*, giving **9 of 14** images a dedicated endpoint object and
  leaving 5 at object 0. `fjell-service-manager` receives `SERVICE_READY` on
  object 0. So the nine announce into their own endpoint and the five reach the
  tracker. **The slot number never changed; its meaning changed underneath the
  services** when each was given a dedicated object, for reasons the table
  records as being about test routing and ABDD paths, with no mention of
  readiness. The scope in this entry (four services) understates it: it is nine.
- **Correction to this entry's own count (RFC-0.28-001, 2026-09-07).** "Nine"
  overstated it, in the direction opposite the usual pattern this project has
  found — not four services worse than claimed, but five better. Of the nine
  `ep_obj` table entries, only **four** (`storaged`, `measuredd`, `attestd`,
  `recoveryd`) both send readiness *and* would deadlock without `init`'s
  co-receive: `semantic-stream` and `proxy-text`'s sends were already deleted
  as dead code by RFC-0.26-004 itself (the exact self-deadlock this entry
  describes, found and removed for those two first); `cap-broker` and
  `driver-uart` never send a readiness signal at all; `sample-service` has a
  dedicated endpoint but was separately patched around it. A deeper,
  previously unrecorded problem was found checking this: those four also send
  a **different tag** (their own private `0x2xx`/`0x3xx` value, not
  `tags::SERVICE_READY`) to their own object — fixing routing alone would not
  have made service-manager recognise them either. And `service-manager`'s own
  receiving object (0) was not exclusively its own: `auditd` and `bootctl`
  independently default to the same object for their own protocols, and
  live-verified, silently won two of the three `SERVICE_READY` messages that
  should have reached service-manager under the old topology. See
  `docs/rfcs/RFC-0.28-001-readiness-topology-answer.md` §3 for the full
  re-derivation and how each was checked.
- **Resolution:** **CLOSED** by RFC-0.28-001. `init` no longer holds any
  receive-capable capability on objects 1-4 (narrowed to `CALL`, the same
  narrowing RFC-0.26-004 applied to objects 7-8); the invariant "a service's
  endpoint has exactly one receiver" now holds structurally on all six
  objects, not merely two. **RFC-0.26-004's invariant text corrected** with a
  dated note rather than silently rewritten. Related: **E-019** /
  RFC-0.26-003, whose `ipc` investigation touches the same objects.

## E-025 — `trust-report`'s cap-manifest scan walks untracked scratch trees

- **Claim:** `docs/release/trust-report.txt`'s capability inventory reports the
  cap-manifests in this repository.
- **Tree:** `crates/fjell-tools/src/trust_report.rs:122` skips exactly
  `["target", ".git", "tests/runs"]` — a hand-maintained literal list that does
  **not** include `.git-exclude/`, the directory this project uses for
  scratch work and temporary checkouts by standing instruction. Any checkout
  under `.git-exclude/tmp/` contributes its own
  `examples/three-node-fleet/fjell-hello/cap-manifest.toml`, and the inventory
  reports "2 cap-manifest(s) found" instead of 1. Found by the implementation
  model during RFC-0.27-002 and correctly flagged rather than fixed inside an
  unrelated RFC. The interfering checkout on that occasion was the architect's
  own clean-clone verification of `0785c96`.
- **Demonstrated live, and it is larger than the manifest count** (architect,
  2026-08-28). Regenerating the report with a clean-clone checkout present under
  `.git-exclude/tmp/` produced, against the committed report:

  ```
  -  1 cap-manifest(s) found:
  +  2 cap-manifest(s) found:
  +  ┌ ./.git-exclude/tmp/verify-clean-0785/examples/.../cap-manifest.toml
  -  total unsafe sites : 311
  -  with SAFETY comment: 311
  +  total unsafe sites : 622
  +  with SAFETY comment: 622
  ```

  **The unsafe-site inventory doubles**: every `unsafe` site in the repository is
  counted twice, once from the tree and once from the checkout. The manifest
  count was the visible symptom; the trust report's principal safety number is
  the one that actually moves. `311/311` and `622/622` are both internally
  consistent, so nothing in the report signals that it is wrong — it simply
  reports a different repository than the one it is committed to.
- **A second walker, found while scoping RFC-0.28-005.** The doubled unsafe
  count does not come from `trust_report` at all — §5 shells out to
  `fjell-unsafe-audit --workspace .`, whose own `walk`
  (`tools/fjell-unsafe-audit/src/main.rs:280`) skips `target`, `.git` and
  `node_modules`, and not `.git-exclude`. **So two tools have the defect,
  with two different hand-written lists**, and a third
  (`consistency-check`'s `evidence`) has a third list that is the only
  correct one — added unprompted by the implementation model after watching
  this erratum happen. Three tools, three answers to what counts as this
  repository, pairwise disagreeing: **E-015's family, caught live.**
- **Two defects, not one.** The inventory can be inflated by anything sitting in
  a scratch directory — the report is a function of the developer's working
  tree, not of the repository. And the skip list is an **explicit enumeration**
  that drifts from reality, the same family as **E-015**; adding `.git-exclude`
  to it fixes today's instance and leaves the family intact. The scan should be
  bounded by what git tracks.
- **Resolution:** **CLOSED** by RFC-0.28-005. ~~ACCEPTED (architect, 2026-08-28), scheduled **0.27**~~ → rescheduled 0.28 (architect, 2026-09-05) → fixed.
- **Slipped once, and recorded rather than quietly re-dated.** It was scheduled
  for 0.27 and 0.27 shipped without it. `errata-tracking` — RFC-0.27-001's own
  subcheck — refused the cut: *"tracking names milestone 0.27, which has
  already shipped, but status is ACCEPTED (not CLOSED)"*. That is the
  subcheck doing precisely what it was built for, against the architect who
  made the commitment. The fix was not attempted during the cut because
  extending an instrument carries RFC-v0.22-001's demonstration requirement
  and a release cut is the wrong place for it — the same reasoning applied to
  **E-030** in the same cut.
- **Fixed, per §6 of RFC-0.28-005's answer document:** both walkers now derive
  their scope from `git` instead of an enumerated skip list. `fjell-unsafe-audit`
  (which must still see a developer's uncommitted work) uses
  `git ls-files --cached --others --exclude-standard`; `trust_report`'s
  cap-manifest scan (which reports on the repository as shipped) uses
  `git ls-files` alone, tracked-only. Neither hand-lists `.git-exclude` or
  any other scratch-directory name — a scratch checkout is excluded because
  `.gitignore` already says so.
- **Demonstrated against a live checkout (2026-09-08), not the original
  numbers.** The cited `622/622` → `311/311` had already drifted: the
  unmodified tool, re-run on today's clean tree, reports `284/284` (most
  plausibly RFC-0.28-002's syscall-asm consolidation reduced real unsafe
  sites since 2026-08-28) — an unrelated finding, checked and reported
  rather than carried forward silently. Against a fresh `git clone --depth
  1` checkout under `.git-exclude/tmp/`: cap-manifest count **2 → 1**,
  unsafe inventory **568/568 → 284/284**. Against the clean tree with the
  checkout removed: **284/284 before and after** — the required invariant
  that the fix does not exclude anything real.

## E-026 — no QEMU evidence this project cites has ever been committed with the document citing it

- **Claim:** kernel-side behaviour in this repository is evidenced by QEMU serial
  logs. **E-013** makes that the *only* available evidence — `fjell-kernel` has
  no `[lib]`, so nothing kernel-side is host-testable — and every architect
  handoff since 0.24 has instructed "cite the QEMU log" on those grounds.
- **Tree:** `.gitignore:28` is `*.log`. **No QEMU serial log has ever been
  committed**, and the two places one can live are each insufficient on their
  own:

  | Location | Overwritten? | Committed? |
  |---|---|---|
  | `tests/qemu/artifacts/<profile>/serial.log` | **yes**, by the next run | no |
  | `tests/runs/<timestamp>/` | no | **no** — also `*.log` |

  And `tests/runs/<id>/04-qemu-smoke-*.log` does not contain a serial
  transcript at all: `test-all`'s per-tier logs capture the xtask/build stdout
  only (`grep -c "Fjell OS kernel started"` → `0`). Found by the implementation
  model during RFC-0.27-002's resubmission, after the architect's review
  proposed `tests/runs/` as the fix — it solves the overwriting half and not the
  committed half, which the implementer checked rather than accepted.
- **How it surfaced.** RFC-0.27-002's first submission cited
  `tests/qemu/artifacts/smoke-m8/serial.log` for a trace that file no longer
  contained; the only surviving copy of the quoted lines was the citing document
  itself. That is not a mistake peculiar to that submission — it is the
  guaranteed end state of the instruction, for every kernel claim this project
  has made.
- **Scope.** Every QEMU-evidenced claim in `docs/`, `rfcs/` and the release
  records. The claims are not thereby wrong; they are **unverifiable from the
  tree**, which is a different and lesser thing, and is the thing this register
  exists to say out loud.
- **Shapes offered, not decided** (RFC-0.27-002's answer document,
  §"Persisting this evidence"): a narrow `.gitignore` exception for an evidence
  directory that only hand-copied logs enter; or a committed transcript of the
  cited lines alongside the document, with the raw log referenced by run id.
- **Resolution:** **CLOSED** by RFC-0.27-004. `tests/evidence/` plus a
  narrow `!tests/evidence/**` `.gitignore` exception (shape 1 of the two
  offered); `cargo xtask evidence promote` requires D2/D3 provenance
  (run id, commit sha, command, and an explicit `instrumented` answer —
  never defaulted) for every promoted log; `qemu_run` retains a per-run
  copy so the next tier's run of the same profile no longer destroys the
  only one worth promoting; a Gate 12 `evidence` subcheck verifies both
  directions (every citation resolves with valid, ancestor-checked
  provenance; every promoted file is cited by something), demonstrated
  failing on all four required broken inputs against the real CLI.
  **R6's reconciliation count:** of the citations this project had already
  made, **1 of 3 was resolvable** — RFC-0.27-002's own citation, promoted
  to `tests/evidence/RFC-0.27-002/m8-attestd-storaged-audit-ring.log` with
  full provenance (the log was produced by an instrumented build; the
  provenance says so explicitly rather than letting `commit_sha` imply a
  reproducible build it is not). **2 of 3 were not** — the specific runs
  `RFC-0.26-004-readiness-channel-answer.md` and the archived
  `RFC-0.26-002-abdd-path-synchronisation.md` cited no longer exist to
  promote, and were **not re-run to stand in for the original** (D4); the
  first is annotated in place, the second (already `Superseded`, already
  framing its citation as a past measurement rather than a live one) was
  left as the point-in-time record it already was. That 2-of-3 figure —
  not the RFC's own estimate of three tracked documents plus this one — is
  the honest count after checking rather than assuming; see the review
  request for the search method. Tracked to resolution as **E-029**, below.

## E-027 — the "threat-model gate" named in the v0.9–v0.15 handoff was never built

- **Claim:** `docs/src/releases/handoff-v0.9-v0.15.md` §4.2 stated that each of
  the threat model's 20 in-scope threats *"references an existing merged RFC
  (the threat-model gate fails otherwise)"* — asserting an instrument that
  enforces the property.
- **Tree:** there is no such gate, and there never was. `grep -rani threat`
  across every tracked `.rs`, `.yml`, `.toml` and `.sh` returns nothing outside
  documentation, and `git log -S "threat" --all` over those file types returns
  no commit on any branch. It is not a gate that was built and later removed;
  it was never written.
- **The property itself holds**, checked by hand on 2026-08-31:
  `docs/security/threat-model-v1.md` carries 20 `### Tn` sections, every one
  citing an RFC, and 8 `OSn` rows — matching the handoff's counts exactly. So
  the sentence was wrong about the mechanism while being right about the
  outcome, which is the shape this project keeps finding: **a claim that reads
  as enforcement, standing in for a check nobody wrote.** Compare **E-023**,
  where four of five specified release-tool behaviours were never built while
  the RFC read `Implemented`.
- **What is actually at risk:** nothing today, and nothing detectable tomorrow.
  If a threat's RFC reference were deleted, or a 21st threat added without one,
  no instrument would report it — and the handoff told a reader one would.
- **Found:** while fixing an unrelated mdBook render warning
  (`T<n>` parsed as an unclosed HTML tag) raised in passing during the logo
  work, and pursued rather than noted because a wrong statement is to be
  corrected, not flagged.
- **Resolution:** **ACCEPTED** (architect, 2026-08-31), `unscheduled`. The
  handoff paragraph is corrected in place with a dated note rather than
  silently rewritten — it is a published record, and erasing the false sentence
  would leave no trace that it had been believed. Building the gate is a
  natural fit for whatever line takes **E-026** and the **E-014** literal-
  predicate family; if built, it carries RFC-v0.22-001's demonstration
  requirement.

## E-028 — `RFC-v0.7.3-002`'s specified deliverable docs do not exist in the tree

- **Claim:** `rfcs/done/RFC-v0.7.3-002-crypto-profile-documentation.md`
  (Status: Implemented, v0.7.1) specifies creating
  `docs/src/security/crypto-profile.md` (current state, risks, threat
  model of the development crypto profile) and
  `docs/src/security/crypto-roadmap.md` (migration plan to v0.9), and lists
  `docs/src/security/crypto-roadmap.md exists` among its own acceptance
  criteria.
- **Tree:** neither file exists anywhere under `docs/`; `docs/src/security/`
  contains only `threat-model-v0.1.md` and `v0.1.0-known-non-goals.md`, both
  superseded. `crates/fjell-sxt-crypto/src/lib.rs`'s live doc-comment still
  points a reader at both nonexistent paths, twice, as the place to read
  about the crate's non-production status and migration plan.
- **Found:** building the CRA/IEC standards mapping (RFC-0.27-003), tracing
  the confidentiality clauses' evidence into `fjell-sxt-crypto` per the
  handoff's instruction to open every artifact before citing it.
- **What is actually at risk:** a reader following the crate's own pointer
  for the crypto profile's risk description and migration plan finds
  nothing at either path. The underlying disclosure — this crate is
  development-only, must not be used in production, and has a documented
  cache-timing leak — is not lost; it is stated directly in the crate's own
  doc-comment. What is missing is the dedicated write-up RFC-v0.7.3-002
  promised and marked done.
- **Same shape as E-023**: an RFC marked `Implemented` while a named,
  checkable piece of its own specified deliverable was never built (or was
  later deleted; `git log` was not searched to distinguish the two, and the
  distinction does not change the resolution).
- **Resolution:** **ACCEPTED**, `unscheduled`. Not fixed here — building
  documentation content is outside RFC-0.27-003's non-goals (no mechanism
  gets built inside a documentation RFC), and the standards mapping's own
  row (CRA-I-2e, IEC-4-2-FR4) already discloses the underlying gap this
  missing documentation would have described.

## E-029 — two historical QEMU-log citations remain unresolvable, with a sunset

- **Claim:** `RFC-0.26-004-readiness-channel-answer.md`'s "Evidence the wait
  executed" section, and the archived
  `RFC-0.26-002-abdd-path-synchronisation.md`'s "zero occurrences" measurement,
  each cite a specific `tests/qemu/artifacts/semantic/serial.log`.
- **Tree:** that path has been overwritten many times since either was
  written — including by later `test-all` runs of the same profile as a
  QEMU negative-test category — and RFC-0.27-004's own R3 fix (retaining
  per-run copies) postdates both citations, so no run-id-keyed copy exists
  to promote. **Not re-run to stand in for the original** (D4): the
  underlying architectural facts (the readiness wait blocks and is woken;
  the capability-checked refusal fires) rest on code paths unchanged since
  RFC-0.26-004 shipped, so a fresh run would demonstrate the same claim
  truthfully — but presenting it as the 2026-era original would not be.
- **Resolution:** **CLOSED** by RFC-0.28-005 R3. ~~ACCEPTED, tracked to
  `0.28`~~ (not `unscheduled` — see RFC-0.27-004's §7 answer document for
  why a real milestone was chosen here rather than deferred indefinitely:
  reusing the `errata-tracking` subcheck's own already-verified tracking
  column as the sunset RFC-0.27-004 §7 argues for, rather than building a
  second, purpose-specific instrument to track it) → fixed. The `semantic`
  QEMU profile was re-run fresh (commit `6582d04`,
  `tree_dirty_at_run_time = false`) and promoted via
  `cargo xtask evidence promote` with real provenance
  (`tests/evidence/RFC-0.28-005/semantic-fresh-2026-09-08.log`,
  `instrumented = none`); `RFC-0.26-004-readiness-channel-answer.md` now
  cites it **alongside** the historical annotation, which stays as the
  honest record of what happened between 0.26 and 0.28 rather than being
  deleted once superseded. The same causal argument (the readiness wait
  blocks and is genuinely woken, not merely scheduled-lucky) reproduces at
  different line numbers in the fresh run, checked rather than assumed.
  The archived `RFC-0.26-002` citation is left as-is: it is a `Superseded`
  RFC's own point-in-time measurement, already framed as a past
  observation rather than a live claim, consistent with this project's
  practice of not rewriting archived records (RFC-0.27-002's precedent).

## E-030 — nothing checks that the two version strings agree

- **Claim:** `version-currency` (RFC-0.27-001 S4) exists to stop the project's
  stated version drifting from the workspace version. It checks `README.md`.
- **Tree:** the version is written in **two** places that must agree, and the
  subcheck sees neither of them as a pair:

  | Site | What it is |
  |---|---|
  | `Cargo.toml` `[workspace.package] version` | the release version |
  | `crates/fjell-os/Cargo.toml` `fjell-abi = { path = "...", version = "..." }` | a publishability requirement that must match it |

  A path dependency needs an explicit `version` to be publishable, and Cargo
  cannot inherit the workspace *package* version into a dependency requirement,
  so the second string is hand-maintained. **When the two disagree the workspace
  does not resolve** — `cargo metadata` fails, and with it every gate, tier and
  build, because none of them can run.
- **How it surfaced:** the 0.27.0 cut. Bumping `[workspace.package]` to `0.27.0`
  left `fjell-os` requiring `fjell-abi 0.26.0` from a workspace containing
  `0.27.0`; the first command of the cycle after the bump failed. Recorded
  rather than silently fixed because *the cut caught it by breaking* — which is
  luck, not a check, and is the same discovery process RFC-0.24-001 was written
  to replace.
- **Why it was ACCEPTED rather than fixed at the cut:** extending
  `version-currency` is an instrument change and carries RFC-v0.22-001's
  demonstration requirement — show it failing on a deliberately mismatched
  pair before trusting it. That did not belong inside a release cut. The
  procedure step was written down at the time
  (`docs/src/release/v0-release-cycle.md`, "Before criterion 1"), the same
  interim treatment the repro-baseline step got at 0.24.0 before anything
  enforced it.
- **Fail-closed, at least.** This defect cannot produce a silent wrong result:
  a mismatch stops the workspace resolving. It cost time, not correctness —
  which is why it was `0.28` rather than urgent.
- **Fixed by RFC-0.28-005 R2.** `version-currency` now also parses
  `crates/fjell-os/Cargo.toml`'s `fjell-abi` version pin and compares it
  against `[workspace.package] version`, demonstrated failing on the real
  CLI against the exact mismatch the 0.27.0 cut hit (`0.26.0` pin vs.
  `0.27.0` workspace version — necessarily run via the already-built binary
  directly, since the mismatch itself stops `cargo run` from resolving the
  workspace to reach the check at all). The check's own failure message
  states the fail-closed caveat: catching this here buys the time of a
  named failure instead of a `cargo metadata` stack trace at a cut — it
  does not add correctness a passing run didn't already have.
- **Resolution:** **CLOSED** by RFC-0.28-005. ~~ACCEPTED (architect,
  2026-09-05), tracked 0.28~~ → fixed.

## E-031 — RFC 058's readiness tracking has never completed, and the test was narrowed to match

- **Claim:** RFC 058 (`Implemented`, v0.2.12) specifies that service-manager
  receives `SERVICE_READY` from every service, records ready state, and emits
  `NEG:SVC:READY_ACCEPTED:PASS` once the startup set has reported.
- **Tree:** `crates/fjell-service-manager/src/main.rs:85` emits that marker at
  `n_ready >= 10`. Per **E-024**'s root cause, **at most 5 of the 14 images can
  ever report** — the other nine announce into their own endpoint. **The
  threshold is unreachable by construction**, and the marker appears in no log
  this project has produced.
- **The test was narrowed to fit.** `fjell-service-api` defines **four** SVC
  markers (`SVC_START_TIMEOUT`, `SVC_READY_ACCEPTED`, `SVC_UNAUTHORIZED_READY`,
  `SVC_FAULT`); `tests/qemu/artifacts/svc/expected-markers.txt` expects **two**.
  The absent pair is exactly the two requiring service-manager to receive a READY
  message. A profile that expects only what already passes detects nothing about
  the rest.
- **And the recorded cause was wrong.** `docs/release/v1-limitations.md` read
  *"svc 2/4 — READY pair pending a startup-timing fix."* It is not timing; it is
  topology. **A wrong diagnosis on the record is why nobody looked again** —
  corrected 2026-09-06.
- **Family.** This is **E-023**'s shape (an RFC reading `Implemented` with
  specified behaviour never built) compounded by **E-014**'s (an expectation
  reduced until the instrument agreed with the defect).
- **Correction to "at most 5" (RFC-0.28-001, 2026-09-07).** Re-derived per the
  handoff's instruction, not trusted: under the old topology the real number
  able to reach service-manager reliably was **3** (`sample-service`,
  `verifyd`, `neg-test`) — worse than claimed, not better, because
  `service-manager`'s own receiving object (0) was also `auditd`'s and
  `bootctl`'s default and raced them for the same messages (found live;
  see **E-024**'s correction and
  `docs/rfcs/RFC-0.28-001-readiness-topology-answer.md` §3). `10` was
  unreachable by an even wider margin than this entry stated.
- **Resolution:** **CLOSED** by RFC-0.28-001. All four SVC markers restored to
  `tests/qemu/profiles/svc.toml` and confirmed firing over repeated runs.
  `service-manager` now has its own dedicated, uncontested endpoint; every
  image that sends `tags::SERVICE_READY` reaches it (8 of them, confirmed
  live), and the threshold is re-derived to **8** — not lowered to make the
  marker fire, raised to the number of services actually capable of firing
  it, per D3's requirement to state the reason in writing (see
  `crates/services/fjell-service-manager/src/main.rs`'s
  `READY_ACCEPTED_THRESHOLD`).

## E-032 — 12 of 15 raw `IpcRecv` asm blocks omit the `a6` clobber

- **Claim:** userspace inline-asm syscall blocks declare every register the
  kernel may write, so the compiler does not keep live values in them across an
  `ecall`.
- **Tree:** `crates/fjell-kernel/src/cap/syscall.rs:497` writes the
  kernel-attested sender identity into **`a6`** (`tf.gpr[16]`) on **every**
  successful IPC delivery, one-way sends included (RFC 055). Of the 15 raw
  `core::arch::asm!` blocks issuing `li a7, 21` in `crates/`, **12 do not
  declare `a6`**:

  | Crate | Sites |
  |---|---|
  | `fjell-attestd` | 2 |
  | `fjell-diagnosticsd`, `fjell-measuredd`, `fjell-netd`, `fjell-proxy-text`, `fjell-recoveryd`, `fjell-secure-transportd`, `fjell-semantic-stream`, `fjell-storaged`, `fjell-upgraded`, `fjell-verifyd` | 1 each |

  `fjell-syscall`'s `sys_ipc_recv_msg` wrapper declares it correctly, and
  RFC-0.28-001 corrected the two blocks it had itself introduced.
- **How it surfaced:** RFC-0.28-001's bring-up. Two newly-written blocks omitted
  `a6`; the compiler kept loop-carried state there, the kernel overwrote it, and
  a three-way relay wait exited after two of three arrivals — a permanent hang,
  reproducible across 6 consecutive builds and **made to disappear by adding
  debug prints**, which the implementer correctly identified as a timing-shaped
  mask rather than a fix. Root cause found by reading the kernel's `deliver()`.
- **Why the other 12 have never been seen:** the bug fires only when the
  compiler happens to allocate a live value to `a6` across the `ecall`. It is
  latent, non-deterministic, and its symptom is a hang rather than an error —
  the worst combination this project has for a defect to have.
- **Not fixed in RFC-0.28-001**, correctly: repairing eleven unrelated services
  is outside that line's scope, and the same judgement was applied to **E-025**.
  The extent was recorded in that RFC's answer document during review, because
  the submission scoped the finding to the two blocks it had repaired.
- **A fix should be mechanical, not manual.** Twelve hand-edits invite a
  thirteenth omission. The candidates are a shared `recv` wrapper the raw sites
  call instead, or a callsite-conformance check in the Gate 11 family asserting
  that any block containing `li a7, 21` names `a6`. Whichever is chosen carries
  RFC-v0.22-001's demonstration requirement.
- **Widened 2026-09-07, while scoping RFC-0.28-002.** The `a6` omission is one
  of **two** register-contract violations in this code, and the surface is
  larger than twelve sites:

  | | Count |
  |---|---|
  | Raw syscall `asm!` blocks in `crates/` | **37** |
  | …in `fjell-syscall`, where they belong | 2 |
  | …hand-rolled inside services | **35** |
  | **Bug A** — `a6` omitted (`IpcRecv`) | 12 |
  | **Bug B** — `a0` declared a plain `in` where the kernel writes status (`IpcRecv`, `IpcReply`) | **18** |

  **Bug B was found independently by the implementation model during
  RFC-0.28-001**, in its own new code, fixed there, and correctly reported as
  real-but-not-the-cause of that line's hang. It was recorded nowhere else and is
  live in eighteen places. A and B overlap; neither contains the other.

  All five syscalls issued from raw asm (13, 20, 21, 22, 23) **already have a
  wrapper in `fjell-syscall`**, so every one of the 35 duplicates something
  audited and correct. That makes the root cause plainer than "a missing
  register": **thirty-five places are each independently responsible for knowing
  what the kernel writes.**
- **Correction to the counts, and to "eleven services" (RFC-0.28-002,
  2026-09-07).** Re-derived rather than trusted:

  | | Widening claimed | Actual |
  |---|---|---|
  | Raw syscall `asm!` blocks in `crates/` | 37 | **41** |
  | …in `fjell-syscall` | 2 | **6** — four set `a7` via a register (`in("a7") nr`) rather than the literal `"li a7, N"` the earlier count searched for |
  | …hand-rolled elsewhere | 35 | 35 (same total) |
  | Crates containing the 35 | 11 named | **14** — `fjell-init` (3), `fjell-service-api` (1), and `fjell-driver-virtio-net` (1) also had hand-rolled blocks, exactly the "sibling the audit turns up" RFC-0.28-002's own scope text anticipated |
  | Distinct syscalls issued by the 35 | 13, 20, 21, 22, 23 | **20, 21, 22, 23 only** — 13 (`CapInspect`) appears solely inside `fjell-syscall`'s own wrapper, never in a service |

  **A third register-contract bug, found auditing the two originally
  claimed:** `IpcCall`'s reply path (`sys_ipc_reply` copies the replier's
  `a2`-`a5` into the caller's frame unconditionally on completion) has the
  same shape as Bug B, on the call side. Three sites needed it
  (`fjell-init::ipc_call`, `fjell-service-api::chunked::ipc_call4`,
  `fjell-proxy-text::ipc_call_action` — the last already documented a live
  incident from exactly this bug, in its own `a2`, fixed there but not in
  `a3`-`a5`). Fixed in all three as part of RFC-0.28-002's per-site pass,
  since these three sites are kept (no wrapper covers 4-word `IpcCall`),
  not deleted.
- **Resolution:** **CLOSED** by **RFC-0.28-002**, which deleted 28 of the 35
  hand-rolled blocks (calling the audited `fjell-syscall` wrapper instead)
  and fixed the register contract of the 7 kept — three 4-word `IpcCall`
  sites and four worded `IpcReply` sites, neither shape covered by an
  existing wrapper. Closed structurally, not case-by-case: `SYSCALL-
  CALLSITE-001` (Gate 11's fourth check) refuses any raw syscall-issuing
  `asm!` block outside `fjell-syscall` unless it is on an explicit,
  guard-owned allowlist naming exactly those 7 sites, and an allowlisted
  site must still declare every register the kernel writes as a correct
  clobber — a 36th block, or a weakened one of the 7, requires editing the
  guard's own source, not adding a comment next to new code.

## E-033 — `fjell-syscall::sys_ipc_recv` has Bug A's shape, live, in five services

- **Claim:** a syscall wrapper in `fjell-syscall` declares every register
  the kernel writes for that syscall, so no caller can be exposed to E-032's
  Bug A (an undeclared clobber silently corrupting compiler-assumed-live
  state).
- **Tree:** `sys_ipc_recv(ep) -> Result<usize, SysError>`
  (`crates/fjell-syscall/src/lib.rs`) is implemented via the generic
  `ecall2(nr, a0, a1, a2, a3)` helper, passing `0, 0` for `a2`/`a3` as
  though they were real inputs. For `IpcRecv` they are not inputs at all —
  the kernel writes `w0`/`w1` into them on delivery — and `ecall2` never
  mentions `a4`, `a5`, or `a6` in its asm operand list, so none of `w2`,
  `w3`, or the RFC-055 sender identity are declared as clobbered, despite
  the kernel writing all three on every successful delivery
  (`crates/fjell-kernel/src/cap/syscall.rs`'s `deliver()`). This is the
  *distinct* wrapper from `sys_ipc_recv_msg`, which declares all of these
  correctly and is what E-032's fix routes every service through instead.
- **Who is exposed:** `sys_ipc_recv` is called live today by `fjell-auditd`,
  `fjell-bootctl`, `fjell-configd`, and three sites across
  `fjell-neg-test`/`fjell-sample-service` — not a theoretical risk; the
  exact shape of bug that produced RFC-0.28-001's permanent hang, inside
  code this project calls a "wrapper" and therefore trusts without the
  per-site audit E-032's services just received.
- **How it surfaced:** RFC-0.28-002's own per-site audit of `fjell-syscall`,
  done to confirm the wrapper being routed to was actually correct rather
  than assumed so — one verified `IpcRecv` shape (`sys_ipc_recv_msg`) and
  one read of `ecall2`'s signature surfaced the gap in the other.
- **Not fixed here.** `fjell-syscall` is RFC-0.28-002's own explicit
  non-goal (D1/D2 are about *service*-side blocks calling *into* the
  wrapper crate, not about auditing the wrapper crate's own correctness),
  and fixing a public wrapper's contract is a decision about that contract,
  not a mechanical swap — recorded here rather than resolved unilaterally.
- **Widened 2026-09-08, while scoping RFC-0.28-004.** `ecall2` is worse than
  "Bug A's shape": it declares `a2`/`a3` as **plain inputs** as well as omitting
  `a4`–`a6`, so `sys_ipc_recv` carries **both** E-032 bug classes. And it is not
  the only casualty. **`sys_cap_inspect` (`lib.rs:671-695`) issues its syscall
  twice** — once through `ecall2`, then again in raw `asm!` to read the
  `rights`/`badge` the helper could not return — **and never checks the second
  call's status.** Since `schedule_next` runs after every trap (RFC-0.28-001),
  another task runs between the two `ecall`s as a matter of course; a revocation
  in that window leaves the function returning `Ok((kind, garbage, garbage))` to
  a caller asking a security question. Live at `fjell-proxy-text:53`.
- **Bounded scope, verified:** 27 call sites use `ecall2`; **exactly two** issue
  a syscall the kernel writes past `a1` for. The other 25 are correct today.
- **The shape of it.** Not two bugs — a helper layer whose contract is narrower
  than the ABI it fronts, which the crate routed around twice instead of
  widening. The comment at `lib.rs:674` is the tell: the author knew `ecall2`
  was insufficient and reached for a second `ecall`.
- **Why RFC-0.28-002 did not catch it:** `SYSCALL-CALLSITE-001` exempts
  `fjell-syscall` entirely, so the one place the register contract must be right
  is the one place nothing checks it.
- **Correction to the mechanism (RFC-0.28-004, 2026-09-08): it is not a race
  window, it is a wrong syscall number.** `SyscallNumber::CapInspect = 14`
  (`crates/fjell-abi/src/syscall.rs:32`); the second raw block issued
  `"li a7, 13"` — **`CapRevoke`**, not `CapInspect`, most likely a literal left
  stale when `CapRevoke` was inserted ahead of `CapInspect`'s current slot.
  **Demonstrated live, not reasoned about:** instrumented the one real caller
  (`fjell-proxy-text`) and ran `cargo xtask qemu-test m8` — `granted_rights`
  read `0x448` on every call, the *correct* `SEND|REPLY|INSPECT` value, with
  both accept and deny outcomes observed in the same run. It "works" because
  `DEMO_CAP_SLOT` correctly lacks `REVOKE`, so the mis-numbered call fails
  closed (`PermissionDenied`, which touches only `a0`) without disturbing
  `a2`/`a3` — leaving the *first* (correctly-numbered) call's real
  `rights`/`badge`, already written into those physical registers by
  `cap/syscall.rs:217-218`, sitting there to be read back by coincidence.
  **The scheduling-race mechanism this entry originally proposed does not
  apply as described:** a task's own register state is preserved in its own
  saved trap frame across a context switch, so another task running between
  the two `ecall`s does not, by itself, disturb this task's pending
  `a2`/`a3`. The actual defect is simpler and does not depend on scheduling
  at all — and worse in the direction that matters: a capability that *does*
  hold `REVOKE` would not get today's coincidence; it would be revoked, as a
  side effect of being inspected. No such caller exists today (checked
  `fjell-neg-test`'s two other call sites: one only checks `Ok`/`Err` on
  empty slots, the other deliberately tests a cap missing `INSPECT`, so the
  *first* call fails and the second's identity never matters).
- **The counts, re-derived once more:** "27 call sites" is a literal
  `grep -c "ecall2("` count that includes `ecall2`'s own definition (26 real
  calls) and does not separate `ecall0`/`ecall1`'s internal delegation (2,
  independently verified safe — the four syscalls reaching `ecall2` this way,
  `Yield`/`Exit`/`DebugWrite`/`CapDrop`, each write only `a0`) from the 24
  named `sys_*` functions. Of those 24, **2** write past `a1` — the same
  total, more precisely derived. A **third** instance of E-032's exact
  register-contract defect was found extending `SYSCALL-CALLSITE-001` into
  this crate (D3): `sys_ipc_call_words` never declared `a5` at all (not
  merely as a plain `in`), since `sys_ipc_reply` copies all four of `a2`-`a5`
  regardless of the original call's declared word count. Its one caller
  (`fjell-init`'s `BOOTSTRAP_COMPLETE`) discards the result entirely, so this
  was latent, not confirmed-firing. Fixed alongside the other two.
- **The audit-record count does not move — contrary to this entry's own
  prediction.** `CapInspect` has no `AuditKindInternal` variant and is never
  audited at all; the phantom second call's only audit line
  (`AuditKindInternal::CapRevoke`, `cap/syscall.rs:182`) sits *after* the
  `PermissionDenied` early return the mis-numbered call always took, so it
  was never reached either. Zero audit records before this fix, zero after —
  checked, not assumed, per the governing RFC's own explicit instruction not
  to adjust a moved count without reporting it as a finding.
- **Resolution:** **CLOSED** by **RFC-0.28-004**. `sys_cap_inspect` now
  issues one syscall, correctly numbered via the `SyscallNumber` constant
  rather than a literal; `sys_ipc_recv` now declares `a2`-`a6`; `SYSCALL-
  CALLSITE-002` (Gate 11's fifth check) enforces both inside `fjell-syscall`
  going forward. `sys_ipc_recv`'s long-term future (fix vs. remove) is
  escalated, not decided — see the governing RFC's answer document §5(c):
  a recommendation to migrate its three callers to `sys_ipc_recv_msg` and
  remove it, not a ruling.

## E-034 — four `send` helpers take a payload word the kernel has never carried

- **Claim:** `fjell-attestd::sxt_send(tag, w0)`,
  `fjell-diagnosticsd::send_tag(ep, tag, w0)`,
  `fjell-secure-transportd::send_tag(ep, tag, w0)` and
  `fjell-upgraded::send_sxt(tag, w0)` each take a data word and, by their
  signatures, send it.
- **Tree:** none of them ever has. The kernel's `build_msg`
  (`cap/syscall.rs:321`) reads the word count from `(raw >> 16) & 0xFF` of the
  tag; all four pass a bare tag with no count packed, so **zero words are
  copied** and the payload never reaches the receiver. The parameter is
  accepted, named, and dropped.
- **Not caused by RFC-0.28-002.** The defect predates it. What that line changed
  is that the word used to be placed in `a2` — where the kernel ignored it —
  and now is discarded explicitly, because the wrapper it swapped to cannot
  carry a word either. **The swap is bit-for-bit behaviour-preserving**, which
  the submission stated accurately and demonstrated over 16 clean runs.
- **What made this an erratum rather than a note.** The swap left `w0`
  genuinely unused, and `let _ = w0;` was added to each site to silence the
  compiler. That is defensible — a build carrying four permanent warnings is
  not a better record — but it removed **the only mechanical signal** that
  these four functions promise something they do not deliver, and
  `cargo xtask build` reports 0 warnings. The finding was documented honestly
  in RFC-0.28-002's answer document; an answer document is not derivable, and
  the same reasoning produced **E-024** when a live defect was disclosed only
  inside a closing erratum's text.
- **Nothing is broken today.** No reachable receiver reads these words. The
  hazard is the next person who writes one, reads the signature, and is
  silently given nothing.
- **Family:** **E-011** — a declaration describing behaviour that does not
  execute.
- **Resolution:** **ACCEPTED** (architect, 2026-09-08), `unscheduled`. Each
  discard now carries a comment naming this erratum, added in review. The real
  fix is to pack a word count and give these flows a receiver, or to delete the
  parameter — whichever, it is a behaviour decision, not a cleanup.

## E-035 — the ABI baseline is never re-recorded, so additive drift accumulates silently

- **Claim:** `tests/abi/snapshot.json` is the record of the surface this project
  promises not to break, and Gate 4 verifies the tree against it.
- **Tree:** the baseline holds **413** items; the tree has **418**. Gate 4
  reports `Added: 5 (additive — OK)` and **PASSes**, correctly — additions do
  not break anyone. The five are `fjell-abi::service` consts added by
  RFC-0.28-001 and never baselined.
- **Nothing ever re-records it.** `docs/src/release/v0-release-cycle.md` does not
  mention the ABI snapshot at any point; the `0.27.0` cut did not touch it; the
  last regeneration was `40ea59b` (RFC-0.27-002), and that was a
  removal-plus-addition reconciliation, not a release step.
- **Why this matters even though the gate is green.** Removals and signature
  changes are still caught, so nothing is currently at risk. But the baseline
  stops describing any shipped release, `Added: N` grows monotonically into
  noise nobody reads, and **whenever someone finally regenerates, every
  accumulated addition is absorbed in a single unreviewed step** — which is
  exactly the hazard RFC-0.24-003 exists to prevent, arriving by patience
  instead of by mistake.
- **Found:** during the RFC-0.28-004 review. The implementation model reported
  the `Added: 5` while verifying its own ABI diff was clean, and correctly left
  it alone as out of scope.
- **Interim fix applied:** the release cycle now carries a step to re-record the
  baseline at each cut with the additions enumerated and justified before
  regeneration (`docs/src/release/v0-release-cycle.md`). **Nothing enforces
  that step** — which is why this is an erratum and not just a procedure edit.
  A `Gate 4`-adjacent check could assert `Added == 0` at a cut; that is an
  instrument change and carries RFC-v0.22-001's demonstration requirement.
- **Slipped 0.29 → 0.30** (architect, 2026-09-09), recorded rather than
  re-dated. `errata-tracking` refused the 0.29.0 cut on it. The *procedure* step
  was written at the 0.28.0 cut and has now been exercised twice — the 0.29.0
  cut reported `Added: 0, Removed: 0, Changed sig: 0` and regenerated nothing,
  because nothing had drifted. **What has not happened is the enforcement**, and
  that is what this erratum tracks: the step is still a paragraph in a document,
  not a check.
- **Resolution:** ~~**ACCEPTED** (architect, 2026-09-08), tracked **0.29**.~~ →
  **CLOSED** by **RFC-0.30-002**.

  > **Closed by RFC-0.30-002, 2026-09-10.** §5 answered shape 1: `Gate
  > 4`/`fjell-abi-snapshot --verify` now fails whenever `Added != 0`, not
  > only on `Removed`/`Changed sig`. Cost argued, not assumed:
  > `tests/abi/snapshot.json` has been touched in 8 commits across this
  > project's entire 177-RFC history — the stable surface changes rarely,
  > so the gate is red only on the line that made the addition, closed by
  > the one command (`--generate`) that same line already needed. Full
  > argument, including why shapes 2 (cut-only) and 3 (version-stamped
  > baseline) were not built, in
  > `docs/rfcs/RFC-0.30-002-checks-that-name-themselves-answer.md`.
  >
  > **Demonstrated failing** (D5/R4) on a deliberately un-regenerated
  > baseline — a temp copy of the real, current `snapshot.json` with 3 real
  > current items removed, fed via `--snapshot`, no tracked file touched:
  > `Added: 3`, previously `Result: PASS`, now `Result: FAIL` naming all
  > three added items. Confirmed the old code passes the identical input
  > (`git stash` comparison).
  >
  > `docs/src/release/v0-release-cycle.md`'s "before criterion 6" step
  > rewritten from a cut-time task to a cut-time confirmation — the
  > enumeration now happens where an addition is made, not deferred to
  > whoever runs the cut.

## E-036 — T20's two-build reproducibility check has never been run

- **Claim:** `docs/security/threat-model-v1.md` §T20
  (*Reproducibility-failure-as-substitution*) states its defence as
  *"RFC-v0.10-003 (reproducible build gate). **Two-build SHA-256 digest
  comparison** (hardened from FNV-1a in RFC-v0.16-005, H-04)."*
- **Tree:** `tools/fjell-repro-check` has exactly two modes.
  `two_build_check` builds twice and compares — **it is invoked nowhere**
  (`git grep two_build_check` returns only its definition and its one internal
  call site inside `main`, reached only when `--skip-build` is absent).
  Every invocation in the repository passes `--skip-build`:

  | Caller | Mode |
  |---|---|
  | `crates/fjell-tools/src/main.rs:125` | `--skip-build` |
  | `crates/fjell-tools/src/test_all.rs:151` (tier 3b) | `--skip-build` |
  | release records `0.23.0`, `0.25.0`, `0.26.0`, `0.27.0` | `--skip-build` |
  | CI | **no repro job at all** — `grep repro .github/workflows/` is empty |

- **What `--skip-build` actually verifies**, and it is not nothing: the
  committed `prebuilt/*.bin` still hash to `tests/repro/baseline-digests.txt`.
  That catches a corrupted or stale committed binary. **It does not build
  anything**, so it cannot detect a build that fails to reproduce — which is
  the property T20 names and the word "reproducible" means.
- **The tier's label is honest** — `"Reproducible build (skip-build)"` — and the
  threat model's is not. The gap is in T20's defence line, not in the tier.
- **How it surfaced:** the owner asked whether `rust-toolchain.toml` was stale.
  It is not; checking what depends on it led here.
- **Family:** **E-023**/**E-027** — a specified mechanism that exists in code
  and is never invoked, with documentation asserting it runs.
- **Widened 2026-09-09, while scoping RFC-0.30-001, by running the check
  instead of reading about it.** This entry said the two-build comparison is
  never invoked. It is worse than that: **it could not fail if it were.**

  `two_build_check` runs `cargo xtask build`, hashes, runs `cargo xtask build`
  again, hashes, compares. **There is no clean between the builds and no
  separate target directory**, so the second build is an incremental no-op, the
  files are never rewritten, and the comparison is between a file and itself:

  ```
  fjell-repro-check: build 1 / 2 …
      Finished `release` profile [optimized] target(s) in 0.41s
  fjell-repro-check: build 2 / 2 …
      Finished `release` profile [optimized] target(s) in 0.40s
  fjell-repro-check: PASS (30 artefacts identical)
  ```

  Neither build compiled anything, and the tree was unchanged afterwards. This
  is **`ci-proptest`'s shape** (RFC-0.24-002 Slice 6) — a gate named for a
  comparison that compares nothing.
- **And neither mode covers the kernel.** `DEFAULT_TARGETS` is the kernel ELF
  plus `prebuilt/`, so the two-build path collects **30** artefacts.
  `tests/repro/baseline-digests.txt` holds **29**, every one a `prebuilt/*.bin`.
  **The kernel binary is in no baseline** — collected by one code path, absent
  from the other, mentioned by neither.
- **What `--skip-build` is worth, stated fairly:** it catches a committed
  prebuilt rebuilt without re-recording, which is a real incident this project
  has had. It is a **staleness check on committed artefacts**, and T20 claims a
  reproducibility check.
- **Resolution:** ~~**ACCEPTED** (architect, 2026-09-08; widened 2026-09-09),
  tracked **RFC-0.30-001**. That RFC settles T20's text regardless of whether a
  real two-build check proves affordable — a threat model naming a defence that
  cannot fail is worse than one naming none. Either
  run the two-build check somewhere real (it is slow, which is presumably why it
  never was) or correct T20's defence line to describe what is actually done.
  **Correcting the claim is a legitimate outcome** — a weaker defence honestly
  stated beats a stronger one nothing performs.~~ → **CLOSED** by
  **RFC-0.30-001**.

  > **Closed by RFC-0.30-001, 2026-09-09.** The "it is slow" guess above was
  > never measured and was wrong: `fjell-repro-check`'s `two_build_check` now
  > runs a scoped `cargo clean --release --target riscv64gc-unknown-none-elf`
  > immediately before each build, and a genuine two-build run costs **on the
  > order of 4–7 seconds total** (measured twice, 2026-09-09; see
  > `RFC-0.30-001-reproducibility-that-reproduces-answer.md` §R1). It runs in
  > CI on every push now (`ci-repro-check`), not nowhere.
  >
  > **The build is, as measured, reproducible.** Two independent runs — each
  > preceded by its own clean — produced bit-for-bit identical output across
  > all 30 artefacts (kernel ELF + 29 service prebuilts), confirmed twice.
  > Also checked, out of caution: building the identical commit from a
  > *different absolute checkout path* (the classic embedded-build-path
  > hazard) — also identical, ruling out that specific cause for this
  > toolchain/profile combination.
  >
  > **The check's sensitivity was demonstrated, not assumed:** forcing a
  > differing `-C metadata` value for the `riscv64gc-unknown-none-elf` target
  > only (`CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUSTFLAGS`, no source
  > touched — kernel/service source stayed off-limits per this line's
  > Non-goals) reproduced the exact failure mode this project has already
  > seen once (RFC-v0.16-005 H-04: `-C metadata` moving digests on a version
  > bump). Run through the actual fixed comparison code (not a synthetic
  > harness): 14 of 29 service prebuilts differed, correctly reported `FAIL`.
  >
  > **The kernel's coverage:** `DEFAULT_TARGETS` already included the kernel
  > ELF alongside `prebuilt/`, so the real two-build check has always covered
  > it (30 artefacts, re-derived and confirmed correct this time); this was
  > never stated anywhere, which is now fixed (T20, this entry,
  > `v1-limitations.md`). `--skip-build`'s committed baseline stays
  > services-only, unchanged, and unable to include the kernel — it isn't a
  > committed artefact, so there is nothing there to record a baseline
  > digest of. Two checks, two different, now-honestly-scoped claims.
  >
  > **T20 corrected** to state exactly this: genuinely independent builds,
  > same-machine only (E-037 still open for cross-machine), 30 artefacts, run
  > in CI every push.

## E-037 — the toolchain is declared twice and recorded nowhere

- **Claim:** builds are reproducible from the pinned toolchain.
- **Tree:** the toolchain is declared in **two** places that cannot see each
  other, and neither is recorded alongside the artefacts it produced:

  | Where | What |
  |---|---|
  | `rust-toolchain.toml` | `channel = "1.91"`, `rust-src`, `riscv64gc-unknown-none-elf` |
  | `.github/workflows/ci.yml:33-55` | `apt-get install rustc-1.91 cargo-1.91`, symlinked into `PATH`, with `rust-src` hand-added in one job |

  **CI never reads `rust-toolchain.toml`** — it bypasses rustup entirely — so
  the file governs local builds only, and CI hand-maintains the same intent
  separately. E-015's family.
- **`channel = "1.91"` floats.** It matches any `1.91.x`; the machine that
  produced the current baseline ran `1.91.1`. A patch bump changes codegen and
  therefore digests, exactly as the workspace version does through `-C metadata`
  (0.24.0's release record).
- **Nothing records which toolchain produced the baseline.**
  `tests/repro/baseline-digests.txt` is digests and a header line; the trust
  report does not carry a toolchain either. So a digest mismatch on another
  machine is indistinguishable from a real reproducibility failure.
- **Why this has never bitten:** **E-036** (now **CLOSED** by RFC-0.30-001) — at
  filing time, the two-build check never ran, and `--skip-build` compares
  committed files to a baseline recorded from those same files on the same
  machine. The two errata insulated each other. **This is only partly fixed by
  E-036's closure**: the real two-build check now runs in CI, but each CI run
  is itself one machine building twice — nothing here yet compares digests
  *across* two different machines/toolchains, so a toolchain-driven drift is
  still exactly as invisible as before. E-037 remains open on its own merits.
- **`rust-toolchain.toml` is not the defect and must not be removed.** It is
  load-bearing: `-Z build-std=core,compiler_builtins`
  (`crates/fjell-tools/src/qemu.rs:63,107`) requires `rust-src`, and the RISC-V
  target comes from it. The defect is that it is one of two declarations and
  that neither is captured with the output.
- **Changed 2026-09-09: the file was removed** (`4cebbc4`), to clear a VS Code
  warning that the pinned toolchain was *"too old for the extension shipped
  rust-analyzer."* The stated reason for it being safe — *"`Cargo.toml` had
  already pointed MSRV to 1.91"* — **does not hold: there is no `rust-version`
  field anywhere in the workspace.** `git grep rust-version -- '*.toml'` returns
  nothing.
- **What that changed, precisely.** The erratum's "declared twice" is now
  "declared once, in CI only":

  | | Before | After |
  |---|---|---|
  | CI | `apt-get install rustc-1.91 cargo-1.91 rust-src` | unchanged — **CI is unaffected** |
  | Local | `rust-toolchain.toml`: channel, `rust-src`, target | **nothing** |
  | MSRV | none | none |

  `crates/fjell-tools/src/qemu.rs` builds with `-Z build-std=core,compiler_builtins`
  under `RUSTC_BOOTSTRAP=1`, which **requires the `rust-src` component**. This
  machine still builds because `rust-src` was installed while the file existed;
  a fresh clone has nothing that installs it and nothing that says it is needed.
- **So the defect moved rather than closed.** It is no longer two declarations
  that can drift; it is one declaration that covers CI and a local setup with no
  declaration at all, and still no record of which toolchain produced any
  artefact.
- **The narrower fix for the warning that prompted this** is adding
  `rust-analyzer` to the toolchain file's `components`, which makes the
  extension use the toolchain's own server instead of its shipped one. Untested
  here — the architect cannot exercise the extension — and offered as the option
  that would have kept both properties, not as a correction to the owner's call.
- **Restored 2026-09-09**, at the owner's direction, after the removal was found
  to have silently moved local builds from **1.91.1 to 1.98.1** — seven minor
  versions — changing all 24 committed prebuilt binaries and turning
  `repro-check` red. Discovered only because the architect happened to rebuild
  while checking the removal was safe. **This is the erratum's own predicted
  failure mode, realised within a day.**
- **`rust-version = "1.91"` added** to `[workspace.package]` — the field the
  removal was believed to rely on, which had **never existed**. It is recorded
  as a *verified floor*, not a bisected minimum: the project is known to build on
  1.91 and on 1.98.1, and the true minimum has never been determined.
- **The version is declared in five places, not two.** Bumping it is a scoped
  piece of work, not an edit:

  | Site | What it says |
  |---|---|
  | `rust-toolchain.toml` | `channel = "1.91"` |
  | `.github/workflows/ci.yml` | `apt-get install rustc-1.91 cargo-1.91`, in several jobs |
  | `docs/release/release-checklist.md:25` | `rustc --version \| grep "1.91"` — **a verification step** |
  | `docs/src/internals/local-development.md:7,21` | documented prerequisite, `rustup toolchain install 1.91` |
  | `Cargo.toml` | `rust-version = "1.91"` (added today) |

  Two of those are checks that would go on asserting 1.91 after a bump. And CI
  installs from **apt on ubuntu-24.04**, which does not carry a current rustc —
  so bumping CI means changing its install method to rustup, not editing a
  number. **E-015's family, in the project's own toolchain declaration.**
- **The channel still floats within `1.91.x`.** Restoring verbatim kept that
  deliberately: pinning an exact patch is a behaviour change, and it belongs
  with the bump rather than smuggled into a restore.
- **Resolution:** **ACCEPTED** (architect, 2026-09-08; updated 2026-09-09),
  `unscheduled`. Closing it means one declaration, an exact pin, and a record of
  which toolchain produced each artefact. Whatever closes it must state where `rust-src` and the target
  come from for a fresh clone, and record the toolchain with the artefacts.

## E-038 — three subchecks fail silently when an RFC folder is absent

- **Claim:** `consistency-check` reports which subcheck failed and why.
- **Tree:** when `rfcs/accepted/` does not exist, **`rfc-status-folder`,
  `errata-tracking` and `doc-counts` produce no output at all** — not a failure
  line, not an error, nothing. They vanish from the results list, and the run
  ends on a bare `consistency-check: FAIL` with no indication of which of the
  ten subchecks failed or what was wrong.
- **How it surfaced:** the `0.28.0` cut. All five 0.28 RFCs moved to `done/`
  together, emptying `rfcs/accepted/` for the first time; git does not track
  empty directories, so the folder vanished from a fresh clone. The clean-clone
  verification step caught it **before the tag** — the working tree was green
  throughout.
- **Fail-closed on the aggregate, silent on the diagnosis.** The overall result
  is `FAIL`, so nothing ships broken. But a reader gets no name, no path and no
  reason, and the three checks that would have said "`rfcs/accepted/` is
  missing" are exactly the three that cannot speak.
- **This is the third discovery of one property of git.** `proposed/` emptied at
  `d5edf31` and turned Gate 12 red for every clone; `archive/` was given a stub
  by RFC-0.25-002 R1 for the same reason; `accepted/` has now done it a third
  time. Each was fixed with a keeper file and none of the three fixes prevented
  the next.
- **Corrected 2026-09-09, while scoping RFC-0.30-002, by reproducing it.** This
  entry says *three* subchecks emit *no output at all*. **Both halves are
  wrong.** Moving `rfcs/accepted/` aside and running the tool shows **four**
  affected — `rfc-status-folder`, **`handoff-status`**, `errata-tracking`,
  `doc-counts` — and three of them do print a `consistency-check: cannot read …`
  line. What none of them prints is a **result line naming itself**, which is the
  format every passing subcheck uses and the thing a reader scans for. The
  aggregate ends `consistency-check: FAIL` with no `<name>: FAIL` anywhere.
  `handoff-status` is the one this entry never named.

  > **Correction, RFC-0.30-002 R2, 2026-09-10.** The paragraph above (and the
  > RFC's own §0.1 reproduction) says `handoff-status` "really is silent —
  > header and nothing else." **Reproduced directly against two real
  > commits — the one that filed E-038 and the one immediately before this
  > RFC — and that is not what happens either time.** At the E-038 filing
  > commit it **passes** (`handoff-status: PASS (23 handoffs checked)`) with
  > `rfcs/accepted/` missing entirely, because no live handoff's Governing
  > RFC link happened to resolve into that folder at that moment. At the
  > commit immediately before this RFC, it **fails with a real, if unnamed,
  > message** (`... links to governing RFC ... which could not be read`),
  > because by then one did (RFC-0.30-001's). Neither is silence. The first
  > is worse: unlike the other three, which enumerate
  > `rfcs/{proposed,accepted,done}` directly and so notice a missing one
  > unconditionally, `handoff-status` only ever touched them incidentally —
  > through whichever RFC a handoff happened to cite — so right after a cut,
  > the exact moment `rfcs/accepted/` is emptiest, it could pass while blind
  > to the very thing it was supposed to notice. Fixed by making it check
  > all three lifecycle folders directly, same as the others, independent of
  > which RFCs currently have handoffs.
- **Interim fix applied:** `rfcs/accepted/README.md`, matching the existing
  keepers, added during this cut. **The silence is not fixed** — that is what
  this erratum tracks.
- **Two candidates.** A subcheck that cannot read a required directory should
  say so by name. And a keeper-file check would close the family rather than its
  third instance — the same "close the class, not the case" argument
  RFC-0.28-002 made for `a6`.
- **Slipped 0.29 → 0.30** (architect, 2026-09-09), recorded rather than
  re-dated. `errata-tracking` refused the 0.29.0 cut on it. RFC-0.29-002
  repaired seven literal-matching predicates and did not touch this one: three
  subchecks still emit **no diagnostic at all** when an RFC folder is absent.
  The keeper files added at 0.28.0 mean the condition is unlikely to recur, which
  is precisely why it stayed unfixed — and why it will stay unfixed until
  something forces it.
- **Resolution:** ~~**ACCEPTED** (architect, 2026-09-08), tracked **0.29**.~~ →
  **CLOSED** by **RFC-0.30-002**. Text corrected in place above: it is
  **four** subchecks, not three, and none of the four fails to speak — what
  none of them does is name itself.

  > **Closed by RFC-0.30-002, 2026-09-10.** `read_file`/`read_dir_named`
  > (`tools/fjell-consistency-check/src/main.rs`) now take the calling
  > subcheck's own name and print `<name>: FAIL — <reason>` on any I/O
  > failure, replacing the generic `consistency-check: cannot read ...`
  > shape. Applied everywhere that shape appeared in this crate — not only
  > the four named here, but three more instances found by the same sweep
  > (`doc-links`, `version-currency`, `syscall-surface`, `evidence` each had
  > at least one unnamed early-failure message of their own, undemonstrated
  > only because their specific inputs were not the ones this reproduction
  > happened to break). D2's "fix the class" reached all of them, since the
  > mechanism already did once built.
  >
  > **`handoff-status` fixed on its actual defect** (see the correction
  > above, not the one originally filed): it now enumerates
  > `rfcs/{proposed,accepted,done}` directly before touching any handoff, so
  > a missing lifecycle folder fails unconditionally rather than only when
  > some handoff's link happens to route through it.
  >
  > **Demonstrated** (D5/R4): `mv rfcs/accepted /tmp/... && cargo run -p
  > fjell-consistency-check -- --all` now shows all four —
  > `rfc-status-folder`, `handoff-status`, `errata-tracking`, `doc-counts` —
  > reporting `<name>: FAIL — cannot read rfcs/accepted: ...`. Isolated
  > further: with the two live handoffs that reference `rfcs/accepted/`
  > *also* moved aside (simulating the true post-cut state, no live handoff
  > routing through it at all), `handoff-status` still correctly fails —
  > confirming the blind-pass defect, not just the naming defect, is fixed.
  > Folder and handoffs restored; `git status` empty both times.

## E-039 — the architect has performed the implementer's role at every cut for five releases

- **Claim:** `docs/src/release/v0-release-cycle.md`'s Roles table assigns the
  release cycle's work:

  | Step | Owner | Architect | Implementer |
  |---|---|---|---|
  | Verify exit criteria | I | **A** | **R** |
  | Produce the release record | I | **C** | **R** |
  | Apply the tag | A | C | **R** |

  Read as RACI, the **implementer is Responsible** for verifying exit criteria
  and producing the release record; the architect is *Accountable* and
  *Consulted* respectively.
- **Practice:** the architect has been Responsible for all of it. Every release
  record from `0.25.0` to `0.29.0` reads **"Prepared by: architect"** — five
  consecutive releases. The architect has bumped the versions, rebuilt, deleted
  and re-recorded the repro baseline, run the tiers and gates, written the
  CHANGELOG entry, corrected documents under criterion 8, moved RFCs to `done/`,
  rescheduled slipped errata, and staged and committed the cut.
- **The cost is not tidiness. It is that the cut is the only work in this
  project nobody reviews.** Every change the implementation model makes is
  reviewed by the architect before it lands. Changes the architect makes at a
  cut land unreviewed, and they have not been defect-free:
  - `0.27.0` — a `git add` naming a path `git mv` had already moved aborted on
    the pathspec and staged nothing; the acceptance took four commits and was
    pushed broken twice before being noticed.
  - `0.28.0` — moving all five RFCs to `done/` emptied `rfcs/accepted/`, which
    git then dropped from every fresh clone, turning `consistency-check` red for
    anyone cloning. Caught only because the architect chose to run a clean-clone
    check that no procedure requires.
  - `0.29.0` — two documents corrected under criterion 8 with nobody checking
    the corrections.
- **Why it drifted:** **there is no handoff for the release cycle.** Every RFC
  has one, written by the architect and handed to the implementer. The cut has
  none, so it has been done by whoever was holding it.
- **A contributing gap:** the Roles table uses `A`/`R`/`C`/`I` and **the legend
  is defined nowhere** in the cycle document or in RFC-v0.21.3-002. The
  conventional reading is unambiguous, but a table whose key is absent is easy
  to read past.
- **Found:** the owner asked, at the `0.29.0` cut, whether it was right for the
  architect to do the preparation rather than hand part of it over. It is not
  what the cycle says.
- **Resolution:** **ACCEPTED** (architect, 2026-09-09), `unscheduled` pending an
  owner decision, because two fixes are available and the choice is the owner's:
  write a release-cycle handoff and hand the cut to the implementer, or amend the
  Roles table to describe what is actually done. **What must not happen is the
  table continuing to say one thing while practice does another** — that is the
  E-023 family, in the document governing releases.

## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-001 Ed25519 vectors | RFC-v0.16-001 | CLOSED |
| E-002 key encryption | RFC-v0.16-006 | CLOSED |
| E-003 wire length | 0.15 | CLOSED |
| E-004 hardware boot | RFC-v0.16-005 | ACCEPTED (v1.0 limitation) |
| E-005 recovery drill | RFC-v0.16-003 | CLOSED |
| E-006 catalog tags | RFC-v0.16-007 | CLOSED |
| E-007 threat review | RFC-v0.16-005 | CLOSED |
| E-008 recovery follow-test | RFC-v0.16-003 | CLOSED |
| E-009 non-goals review | RFC-v0.16-005 | CLOSED |
| E-010 IPC words delivery | 0.20 | CLOSED |
| E-011 cap_install rights validation | RFC-v0.21.3-001 | ACCEPTED |
| E-012 release checklist Step 9 bundle path | RFC-v0.22-001 | ACCEPTED |
| E-013 gate tools' own tests run under no mechanism (tier 1 `--lib`, and never named in CI) | RFC-0.29-001 | CLOSED |
| E-014 instruments deciding by fixed-string match | RFC-0.29-002 | ACCEPTED |
| E-015 hand-enumerated instrument scopes drifted from reality | RFC-0.29-002 | CLOSED |
| E-016 no link, index, or count integrity instrument | RFC-0.27-001 | CLOSED |
| E-017 audit `sound` verdicts not all demonstration-backed | RFC-0.29-002 | CLOSED |
| E-018 `PRIORITY_USER` three copies, two values — init starves other tasks | RFC-0.26-001 | CLOSED |
| E-019 `ipc` negative profile assumes an unsynchronised scheduling order | RFC-0.28-003 | CLOSED |
| E-020 ABDD live path no longer runs — `sample-service` asserts peer readiness instead of synchronising | RFC-0.26-004 | CLOSED |
| E-021 `init::wait_ready_exact` consumes and drops other tasks' IPC, blocking callers forever | RFC-0.26-004 | CLOSED |
| E-022 `sys_ipc_send`'s one-way path blocks the sender on `Queued`, against its own documented contract | RFC-0.27-002 | CLOSED |
| E-023 release tool's `RELEASE.md` generation and consistency checks never built (4 of 5 behaviours) | RFC-0.27-001 | CLOSED |
| E-024 `init` co-receives on four services' own endpoints (corrected from "nine"); RFC-0.26-004's one-receiver invariant is narrower than its text | RFC-0.28-001 | CLOSED |
| E-025 `trust-report` and `unsafe-audit` walk untracked scratch trees; three tools carry three disagreeing exclusion lists | RFC-0.28-005 | CLOSED |
| E-026 no QEMU evidence has ever been committed with the document citing it; `tests/runs/` tier logs carry no serial transcript | RFC-0.27-004 | CLOSED |
| E-027 the "threat-model gate" asserted by the v0.9–v0.15 handoff was never built | unscheduled | ACCEPTED |
| E-028 RFC-v0.7.3-002's specified crypto-profile/crypto-roadmap docs do not exist in the tree | unscheduled | ACCEPTED |
| E-029 two historical QEMU-log citations (RFC-0.26-004, archived RFC-0.26-002) remain unresolvable | RFC-0.28-005 | CLOSED |
| E-030 nothing checks that `[workspace.package] version` and `fjell-os`'s `fjell-abi` version pin agree | RFC-0.28-005 | CLOSED |
| E-031 RFC 058's `READY_ACCEPTED` is unreachable by construction; the svc profile expects 2 of 4 markers | RFC-0.28-001 | CLOSED |
| E-032 35 hand-rolled syscall `asm!` blocks in 14 crates carried three register-contract bugs (`a6` omitted ×12, `a0` as plain `in` ×18, `IpcCall` reply words ×3) | RFC-0.28-002 | CLOSED |
| E-033 `sys_ipc_recv`/`sys_cap_inspect`/`sys_ipc_call_words` carried E-032's bug classes inside fjell-syscall itself; `sys_cap_inspect`'s second call was `CapRevoke`, not a race window | RFC-0.28-004 | CLOSED |
| E-034 four `send` helpers take a payload word the kernel has never carried (no word count packed in the tag) | unscheduled | ACCEPTED |
| E-035 the ABI baseline is never re-recorded; additive drift accumulates and would be absorbed unreviewed | RFC-0.30-002 | CLOSED |
| E-036 T20's two-build check is invoked nowhere, could not fail if it were (no clean between builds), and no mode covers the kernel | RFC-0.30-001 | CLOSED |
| E-037 the toolchain is declared for CI only since `rust-toolchain.toml` was removed; no MSRV exists, and nothing tells a fresh clone it needs `rust-src` | unscheduled | ACCEPTED |
| E-038 four subchecks fail without a result line naming themselves when an RFC folder is absent | RFC-0.30-002 | CLOSED |
| E-039 the architect has been Responsible for the cut at five consecutive releases; the Roles table assigns that to the implementer | unscheduled | ACCEPTED |

E-018 was filed during RFC-0.25-001 (ACCEPTED, after the 0.24.0 cut) and
closed by RFC-0.26-001; E-019 was filed during RFC-0.26-001 itself, as the
newly-surfaced collateral its own investigation document names. At the
0.24.0 cut (2026-08-03): **0 OPEN, 9 CLOSED, 8
ACCEPTED.** E-014,
E-015 and E-016 were filed together as the instrument audit's
disposition — grouped by root cause rather than one per finding, so the register
records four families instead of thirty-three individually-true rows. Each names
its member findings explicitly; `docs/verification/instrument-audit.md` remains
the authoritative row-level record and
`docs/verification/instrument-audit-closeout.md` the disposition. All three are
**ACCEPTED, not OPEN**, on the same grounds as E-004/E-011/E-012/E-013:
scheduled deferral to a named future line is a deliberate decision, not live
unresolved drift. Seven of the audit's findings were repaired in RFC-0.24-002
and are therefore not filed here; one more (Gate 4's ABI identity collapse) is
in flight under RFC-0.24-003 and blocks the 0.24 cut, so it is not filed either.

**E-017 was filed at the cut itself**, and is the one that qualifies the rest:
RFC-0.24-001 requires every instrument claimed `sound` to carry a committed
demonstration of it failing, and two rows were found violating that — one on
each side of the review boundary. Both were repaired, but the re-derivation of
the remaining `sound` rows is incomplete, and the first one re-derived (Gate 4)
fell immediately. It is why RFC-0.24-001 ships `Implemented-with-Errata` and
why the audit's 22 `sound` verdicts are recorded as provisional.

At v0.23 update: 0 OPEN, 9 CLOSED, 4 ACCEPTED. The ACCEPTED items
(hardware boot, `cap_install` rights validation) are reflected in the v1.0
scope statement / RFC-v0.21.3-001; both are disclosed limitations, not
silent drift. E-012 is a v1.0-checklist-specific finding recorded per
RFC-v0.22-001 §Scope item 5. **Classified ACCEPTED, not OPEN** (architect,
2026-07-31): this register defines OPEN as live, unresolved drift and ACCEPTED
as a documented, deliberate limitation. Not investigating E-012 was a deliberate
owner decision (2026-07-30, cutting the v1.0 checklist audit from v0.22 scope
because v1.0 is not in view), which is ACCEPTED semantics — the same grounds on
which E-004 is ACCEPTED. To be revisited when v1.0 preparation actually begins.
E-013 is recorded per RFC-v0.23-002, found while authoring that RFC's required
unit tests. **Classified ACCEPTED, not OPEN** (architect, 2026-08-01): deferring
the fix to a dedicated RFC after the `0.23.0` cut is a deliberate decision, not
live unresolved drift — the same distinction applied to E-004/E-011/E-012.
