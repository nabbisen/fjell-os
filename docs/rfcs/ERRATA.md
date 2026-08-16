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
- **Resolution:** **ACCEPTED** (architect, 2026-08-01). Found during
  RFC-v0.23-002 Slice 1 while writing the two-demonstration unit tests that
  RFC requires — they could not be proven to run under tier 1 or any other
  `cargo test` invocation. The fix is architectural (add a `[lib]` target,
  or split a host-testable subset out of the kernel crate) and is real
  design work deserving its own RFC rather than an in-line exception during
  a marker-emission fix. Pre-existing; makes nothing worse; does not block
  `0.23.0`. RFC to follow after the release. See
  `docs/release/v1-limitations.md`.

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
- **Resolution:** **ACCEPTED** (architect, 2026-08-03). Recorded, not fixed.
  Each individual patch would be a better string; the family needs one answer to
  *how these instruments should decide*, which is design work and a 0.25
  candidate. None is a live false-green today — Gate 5's is the closest, and
  requires someone to use a status word outside the recognised four. See
  `docs/verification/instrument-audit-closeout.md` §3.1 and
  `docs/release/v1-limitations.md`.

## E-015 — Hand-enumerated instrument scopes that no longer match reality

- **Claim:** RFC 025 (CI/QEMU automation foundation) and RFC 026 (negative test
  harness) present CI and the negative-test harness as covering the workspace
  and its negative categories.
- **Shipped:** both enumerate their subjects by hand, and the lists have drifted.
  Found by RFC-0.24-001 Passes 2 and 4:
  - **19 of 89 workspace crates are never named in any `ci.yml` job.** Six are
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
- **Resolution:** **ACCEPTED** (architect, 2026-08-03). Recorded, not fixed.
  Distinct in kind from E-014: these are checks that **do not run**, or run over
  an incomplete set — not checks that report success without checking. That
  distinction is why they were held out of the pre-cut repair line
  (RFC-0.24-002). Fixing them is mechanical once someone decides whether CI
  should enumerate or derive its package list; 0.25 candidate. See
  `docs/verification/instrument-audit-closeout.md` §3.2.

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
- **Resolution:** **ACCEPTED** (architect, 2026-08-03). The audit's stated
  standard is right and is not being weakened; what is disclosed is that
  compliance with it has been verified for two rows by counter-example and
  assumed for the rest. **The 22 `sound` verdicts are provisional**, and the
  count should be read that way until the re-derivation completes — listed as a
  0.25 candidate in the close-out. This is why RFC-0.24-001 ships
  `Implemented-with-Errata` rather than `Implemented`: its normative text claims
  more than the merged work verifies. See
  `docs/verification/instrument-audit-closeout.md` §4.1 and
  `docs/release/v1-limitations.md`.

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
  (broken) ordering in a way not yet understood. RFC-0.25-001 ships a narrow,
  `image_id`-keyed stopgap instead — `crates/drivers/fjell-driver-uart` alone
  is spawned at `init`'s own priority bucket
  (`crates/fjell-kernel/src/task/spawn.rs`, `crates/fjell-kernel/src/trap/
  syscall.rs::sys_task_start`) — rather than correcting the general constant.
- **Resolution:** **ACCEPTED** (architect, 2026-08-16, reviewing
  RFC-0.25-001). The stopgap is keyed on task identity (`image_id`), not
  table position, so it does not reintroduce the class of defect
  RFC-v0.23-002 removed from milestone markers. The general fix is real
  investigation — why the M6 hang happens — not a cleanup, and needs its own
  RFC. See `docs/release/v1-limitations.md`.

---

## Summary

| Errata | Tracking RFC | Status |
|--------|--------------|--------|
| E-001 Ed25519 vectors | v0.16-001 | CLOSED |
| E-002 key encryption | v0.16-006 | CLOSED |
| E-003 wire length | (v0.15.x) | CLOSED |
| E-004 hardware boot | v0.16-005 | ACCEPTED (v1.0 limitation) |
| E-005 recovery drill | v0.16-003 | CLOSED |
| E-006 catalog tags | v0.16-007 | CLOSED |
| E-007 threat review | v0.16-005 | CLOSED |
| E-008 recovery follow-test | v0.16-003 | CLOSED |
| E-009 non-goals review | v0.16-005 | CLOSED |
| E-010 IPC words delivery | v0.20.0 fix | CLOSED |
| E-011 cap_install rights validation | v0.21.3-001 (v0.22 disposition) | ACCEPTED |
| E-012 release checklist Step 9 bundle path | v0.22-001 (recorded, not fixed) | ACCEPTED |
| E-013 gate tools' own tests run under no mechanism (tier 1 `--lib`, and never named in CI) | RFC after v0.23.0 (recorded, not fixed) | ACCEPTED |
| E-014 instruments deciding by fixed-string match | 0.25 candidate (recorded, not fixed) | ACCEPTED |
| E-015 hand-enumerated instrument scopes drifted from reality | 0.25 candidate (recorded, not fixed) | ACCEPTED |
| E-016 no link, index, or count integrity instrument | 0.25 candidate (recorded, not fixed) | ACCEPTED |
| E-017 audit `sound` verdicts not all demonstration-backed | 0.25 candidate (re-derivation incomplete) | ACCEPTED |
| E-018 `PRIORITY_USER` three copies, two values — init starves other tasks | RFC after 0.25 (recorded, not fixed) | ACCEPTED |

E-018 was filed during RFC-0.25-001, after the 0.24.0 cut. At the 0.24.0 cut
(2026-08-03): **0 OPEN, 9 CLOSED, 8 ACCEPTED.** E-014,
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
