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
  receives on."* RFC-0.27-002's audit (required by its R2) found this
  incomplete: **five more services** — `fjell-measuredd`, `fjell-attestd`,
  `fjell-recoveryd`, `fjell-storaged` (via raw `core::arch::asm!("li a7,
  20", ...)`, bypassing the wrapper entirely, not caught by a search for
  the wrapper's name alone) — share the identical self-targeting shape
  (`send_ready()` and `recv_call()` both using one fixed `EP_SLOT`). None
  currently self-deadlocks: `init`'s `wait_service_ready` /
  `wait_storaged_ready` still hold full receive rights on each of these
  services' own endpoints and reach the corresponding wait before or
  exactly when it matters, because `init`'s boot sequence deterministically
  visits every wait and a queued one-way send is drained whenever the
  receiver gets there, not dropped. This is the exact masking arrangement
  RFC-0.26-004 removed for `semantic-stream`/`proxy-text` — intact here
  only because `init` was never asked to stop receiving on these four
  objects. `wait_service_ready`/`wait_storaged_ready` retain the identical
  missing-`else` defect `wait_ready_exact` had (disclosed, not fixed, since
  RFC-0.26-004; reworking RFC 058's readiness protocol is a non-goal of
  both that RFC and this one); **now tracked as E-024**, filed in the
  RFC-0.27-002 review because a live defect described only inside a CLOSED
  erratum is tracked by nothing). Full audit and the design-answer document
  naming this a real, six-instance, unmet primitive need — not decided
  here — in `docs/rfcs/RFC-0.27-002-one-way-send-contract-answer.md`. May
  be relevant to **E-019 / RFC-0.26-003**'s `ipc` investigation — flagged,
  not absorbed. See `docs/release/v1-limitations.md`.

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
- **Resolution:** **ACCEPTED** (architect, 2026-08-28). Recorded, not fixed —
  reworking RFC 058's readiness protocol is a non-goal of RFC-0.26-004 and
  RFC-0.27-002 alike, and the arrangement is currently load-bearing. Two things
  are required before it is closed: the readiness protocol reworked so services
  do not announce into their own receive endpoint, **and RFC-0.26-004's
  invariant text corrected** to state the scope it actually has. A stated
  invariant the tree violates, with no instrument checking it, is precisely the
  class RFC-0.27-001 was built to make derivable. Related: **E-019** /
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
- **Two defects, not one.** The inventory can be inflated by anything sitting in
  a scratch directory — the report is a function of the developer's working
  tree, not of the repository. And the skip list is an **explicit enumeration**
  that drifts from reality, the same family as **E-015**; adding `.git-exclude`
  to it fixes today's instance and leaves the family intact. The scan should be
  bounded by what git tracks.
- **Resolution:** **ACCEPTED** (architect, 2026-08-28), scheduled **0.27**.
  Small, but it is an instrument whose output depends on untracked state, so the
  fix carries RFC-v0.22-001's demonstration requirement: show the inventory
  wrong with a scratch checkout present, then right with the fix in place.

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
| E-013 gate tools' own tests run under no mechanism (tier 1 `--lib`, and never named in CI) | unscheduled | ACCEPTED |
| E-014 instruments deciding by fixed-string match | unscheduled | ACCEPTED |
| E-015 hand-enumerated instrument scopes drifted from reality | unscheduled | ACCEPTED |
| E-016 no link, index, or count integrity instrument | RFC-0.27-001 | CLOSED |
| E-017 audit `sound` verdicts not all demonstration-backed | unscheduled | ACCEPTED |
| E-018 `PRIORITY_USER` three copies, two values — init starves other tasks | RFC-0.26-001 | CLOSED |
| E-019 `ipc` negative profile assumes an unsynchronised scheduling order | RFC-0.26-003 | ACCEPTED |
| E-020 ABDD live path no longer runs — `sample-service` asserts peer readiness instead of synchronising | RFC-0.26-004 | CLOSED |
| E-021 `init::wait_ready_exact` consumes and drops other tasks' IPC, blocking callers forever | RFC-0.26-004 | CLOSED |
| E-022 `sys_ipc_send`'s one-way path blocks the sender on `Queued`, against its own documented contract | RFC-0.27-002 | CLOSED |
| E-023 release tool's `RELEASE.md` generation and consistency checks never built (4 of 5 behaviours) | RFC-0.27-001 | CLOSED |
| E-024 `init` co-receives on four services' own endpoints; RFC-0.26-004's one-receiver invariant is narrower than its text | unscheduled | ACCEPTED |
| E-025 `trust-report`'s cap-manifest scan walks untracked scratch trees (`.git-exclude/` not skipped) | 0.27 | ACCEPTED |

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
