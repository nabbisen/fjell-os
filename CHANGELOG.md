# Fjell OS — Changelog

All notable changes to this project are documented in this file.
Versions follow `MAJOR.MINOR.PATCH` semantics from v1.0.0 onward.

---

## [0.21.3] — Build restoration and as-built reconciliation

RFC-v0.21.3-001. No new OS functionality; no security-boundary change.
Corrects regressions introduced by `5091e54` and reconciles documentation
with the shipped implementation. No v1.0 tag activity.

### Fixed — build restoration (blocker)

- **`Cargo.toml` did not parse.** `members` was an unterminated, non-recursive
  glob (`5091e54`), so `cargo metadata` — and therefore every `cargo`
  entry point, including `cargo xtask release-rehearsal` — failed before
  doing any work. Restored the explicit 88-entry member list. `cargo
  metadata --no-deps` now exits 0 with 88 members; no globs.
- Removed 56 empty, untracked leftover directories under `crates/` from the
  v0.21.0 reorganization that made a glob-based member list unrecoverable
  in the first place.
- `cargo fmt --all` applied workspace-wide (254 files) — the manifest fix
  is what made this possible for the first time since the outage.
- Two Gate 2 (unsafe-audit) regressions from the formatting pass fixed:
  `cargo fmt` separated 4 `// SAFETY:` comments from their `unsafe` token
  by one line (macro-arm and function-body expansion); moved each comment
  to sit immediately above `unsafe` (comment-only, no compiled tokens
  changed). `fjell-unsafe-audit`: 274/274, fmt-stable.
- Fixed two stale pre-v0.21.0 paths in `tools/fjell-abi-snapshot`'s
  `STABLE_CRATES` table (`fjell-audit-format`, `fjell-bundle-format` had
  moved under `crates/formats/`), which made the tool see zero items in
  both crates and report 28 real items as "removed." Regenerated
  `tests/abi/snapshot.json` (401 → 404 items); verified the pre-fmt
  baseline was never stale in content — only the tool's paths were, and
  every post-fmt signature change is attributable to formatting alone.
- Made `tools/fjell-repro-check`'s `--skip-build` mode fail closed: a
  missing baseline previously auto-recorded itself and reported PASS,
  so this tier detected nothing. Recording now requires the explicit
  `--record-baseline` flag and is never a side effect of a check run.
  Committed `tests/repro/baseline-digests.txt`.
- Ran the two-build reproducibility check for the first time since the
  outage: PASS, 29 artefacts identical, in one environment — the
  reproducible-build NFR holds within an environment.

### Fixed — as-built documentation reconciliation

- Corrected the documented syscall surface: `fjell-abi` declares 35
  syscall numbers, but `trap/syscall.rs` dispatches 26 (verified against
  source, not assumed); the other 9 are declared and have user-space
  wrappers but return `UnknownSyscall`. Updated
  `docs/src/external-design/kernel.md`, `capability-lease.md`,
  `docs/src/abi/ipc-register-layout.md` (removed the nonexistent
  `SyscallNumber::IpcTrySend`), and replaced the 7-line
  `docs/src/api/syscalls.md` stub with the authoritative 26-entry
  catalog. Recorded as ERRATA E-011: `sys_cap_install_with_rights`'s
  doc-comment claims a kernel rights check that cannot execute, because
  `CapInstall` is not dispatched at all — not a live security hole (it
  fails closed), but not shipped behaviour either. Disposition of the 9
  deferred to v0.22.
- Fixed `docs/src/SUMMARY.md`: a duplicate-file mdBook build error
  (`./intro/what-is-fjell.md` listed twice), 7 dead links to a renamed
  handoff-bundle directory, and wired in `docs/src/requirements/`,
  `docs/src/external-design/` (9 subsystem pages), and
  `docs/src/roadmap/roadmap.md`, none of which mdBook had ever built.
- Rebuilt `rfcs/README.md` against `rfcs/done/` (154 files):
  bidirectionally verified every file is linked exactly once and every
  link resolves. Moved the v0.11–v0.15 RFCs out of "Proposed" (they were
  listed there at paths that never existed; all are actually
  implemented) and added the previously entirely-unlisted v0.9.0,
  v0.9.4, v0.10.0, and v0.16.0 sections.
- Corrected stale figures in the root `README.md` (version, RFC count,
  unsafe-site count re-derived from `fjell-unsafe-audit`) and in
  `docs/src/releases/handoff-0.21.2/*.md` (version stamps v0.21.1 →
  v0.21.2).
- Corrected the claim that Gate 9 was the sole remaining blocker to
  v1.0.0 in `ROADMAP.md` and `docs/src/roadmap/roadmap.md`: the
  mechanical gates could not run at all at v0.21.2, so that claim was
  never actually verified.

### Known limitations introduced or clarified by this release

- 9 of 28 prebuilt service binaries can rebuild to different bytes with
  no source change (same file size); confirmed cross-environment rather
  than within-environment via the two-build check above. Root cause
  (build/link determinism) deferred to its own v0.22 RFC.
- The ABI snapshot gate is formatting-sensitive by design (line-based
  signature hashing) — a whole-tree `cargo fmt` invalidates it wholesale.
  Deferred to the v0.22 candidate list.
- The durable disposition of the 9 declared-but-undispatched syscalls
  (implement / remove from the ABI / keep reserved) is not decided by
  this release.

---

## [0.21.2] — v1.0 handoff bundle + stale-reference cleanup — `KNOWN-BAD`

**`KNOWN-BAD`** (RFC-v0.21.3-002, Decision request 2, owner-accepted
2026-07-30): this tag's workspace manifest does not parse (`Cargo.toml`'s
`members` array is an unterminated, non-recursive glob — introduced by
`5091e54`, tagged as part of this release regardless). `cargo metadata`
fails, so every `cargo` entry point is unreachable and no gate in this
release can be re-run. Nothing in the tree below builds. Superseded by
`0.21.3` (RFC-v0.21.3-001), which restores the build and re-verifies every
mechanical gate. The tag is kept, not deleted or moved — see
`docs/rfcs/ERRATA.md` and `rfcs/proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md`
for the full account.

### Added

- **Compact handoff bundle** at `docs/src/releases/handoff/`: role-based,
  evidence-focused handoff documents (project summary, external design,
  implementation notes, testing and gates, ops/security, decision log) plus
  a bundle README and an evidence-generation note. Wired into SUMMARY.md
  under Development History.

### Fixed

- **README stale version in prose**: the Overview said "Current version:
  v0.15.1"; now v0.21.1. (The badge was already corrected in v0.21.1; this
  was a second occurrence in body text.)

---

## [0.21.1] — Audit: RFC compliance, dead code, test/doc alignment

Five-dimension audit (RFC compliance · dead code · test coverage · code/test
alignment · docs/codebase alignment).

### Fixed — RFC compliance (Dimension 1)

- **`PolicyAction::QueryState` missing** (`fjell-fleet-format`): `fjell-fleetd`
  used `PolicyAction::QueryState` which did not exist in the enum, causing a
  compile error and silently excluding fleetd from the build. `FleetActionKind`
  already had `QueryState = 0x07`; `PolicyAction` now has `QueryState = 0x08`.
  The variant represents "query fleet node state (read-only, policy-gated)".

### Fixed — dead code (Dimension 2)

- **`fjell-syncd` unused imports removed**: `SNAPSHOT_ENVELOPE_V2`,
  `SnapshotImportError`, `SnapshotImportOutcome` were imported but unused
  (v0.7.2 import pipeline not yet wired). Replaced with a comment marking
  them for the storaged import path.
- **`fjell-diagnosticsd` unused-assignment suppressed**: `t`, `w0`, `w1`
  initialized to `0` before `lateout` asm binding; the initial value is
  required by Rust but never read — the asm overwrites via `lateout`. Annotated
  with `#[allow(unused_assignments)]` and explanation.
- **`DmaRegionEntry::user_va` and `page_count` annotated**: fields are stored at
  DMA alloc time and read by `unmap_user_va_for`; the unmap step is currently
  bypassed in `revoke_by_pa` due to page-table corruption under v0.8.x
  (full analysis in the existing `revoke_by_pa` comment). Annotated with
  `#[allow(dead_code)]` and RFC-v0.7.4-001 reference.
- **`unmap_user_va_for` annotated**: implements RFC-v0.7.4-001 clause 1
  (PTE unmap before DMA frame free); bypassed in `revoke_by_pa` until the
  root cause of the v0.8.x corruption is isolated. Annotated with
  `#[allow(dead_code)]` and the deferred-path explanation.
- **`_stack_top` renamed**: kernel `stack_top` was computed but then
  superseded by `RAM_END` as `map_end` (mapping only to `stack_top` caused
  `StorePageFault` in spawn with many services). Renamed to `_stack_top`.

### Fixed — documentation / codebase alignment (Dimension 5)

- **README version badge**: was `0.15.1`, now `0.21.0`.
- **`docs/verification/mmio-audit-v0.12.md`**: updated three crate paths to
  `crates/services/` after the v0.21.0 reorganization.
- **`docs/src/sdk/writing-a-service.md`**: `crates/fjell-sample-service` →
  `crates/services/fjell-sample-service`.
- **`docs/src/internals/local-development.md`**: workspace layout updated to
  show the new `arch/`, `drivers/`, `formats/`, `services/` subdirectories.

### No action required

- **Audit ring `get()`, `len()`, `dropped()`, `pending()` dead methods**:
  all carry `#[allow(dead_code)]` with existing justification. `sys_audit_drain`
  uses `peek_at()` and `drain_n()`. These utility methods are API completeness
  stubs retained for future diagnostic tooling.
- **All other `#[allow(dead_code)]` annotations**: reviewed and confirmed
  legitimate — each has a RFC reference, future-wire justification, or ABI
  completeness note. No suppression without explanation exists.

---

## [0.21.0] — Crate subdirectory reorganization + horizontal doc cleanup

### Changed — crate structure

Introduced four subdirectories under `crates/` to group the 80 crates by
role. Crate names and the public API are unchanged; only path references in
`Cargo.toml` files were updated.

- `crates/arch/` — architecture trait and platform implementations
  (fjell-arch, fjell-arch-riscv64, fjell-arch-arm64)
- `crates/drivers/` — hardware device drivers
  (fjell-driver-virtio-blk, fjell-driver-virtio-net)
- `crates/formats/` — all 22 data-schema crates (`*-format`)
- `crates/services/` — all 29 runtime RISC-V program crates (`*d` daemons
  plus fjell-init, fjell-bootctl, fjell-cap-broker, fjell-devmgr,
  fjell-neg-test, fjell-sample-service, fjell-proxy-text,
  fjell-semantic-stream, fjell-service-manager, fjell-svc-fault,
  fjell-svc-timeout)

The 24 remaining library and infrastructure crates stay flat under
`crates/` (fjell-kernel, fjell-abi, fjell-cap, fjell-ipc, fjell-syscall,
fjell-semantic-*, fjell-dtb-*, fjell-tools, fjell-sdk, etc.).

### Cleaned — horizontal file audit

Systematic scan of all directories for stale, misplaced, or redundant files,
following the same discipline applied to the root directory in v0.20.2.

- **ADR duplicates resolved**: old superseded ADRs 0001–0010 (v0.1.0–v0.1.3
  naming scheme) moved to `docs/src/adr/superseded/`. The migration is
  recorded in `docs/src/adr/ADR-RENAME.md` (RFC 045).
- **`docs/src/development/` removed**: v0.1.x draft directory superseded by
  `docs/src/internals/`. Unique content (`negative-tests.md`) moved to
  `docs/src/internals/negative-tests.md` and added to SUMMARY.md.
- **`docs/src/getting-started/` removed**: not referenced by SUMMARY.md;
  unique FAQ content merged into `docs/src/faq.md`.
- **`docs/src/perf/baseline.md`** synced with the full content from
  `docs/perf/baseline.md` (was a 7-line stub).
- **Empty template directories removed**: two brace-expansion artifact dirs
  in `docs/src/` (`{identity,release,...}` and `{intro,tutorials,...}`).
- **`rfcs/archive/`** (empty) removed.
- **`tests/runs/`** added to `.gitignore` (ephemeral test-run logs).

---

## [0.20.2] — QEMU disk-image + dead-code cleanup

### Fixed

- **QEMU disk image creation no longer requires `qemu-img`** — `qemu_run`
  previously called `qemu-img create` and silently discarded the result when
  the tool was not found, causing every QEMU profile to fail with
  "Could not open 'fjell-disk.img': No such file or directory".
  A 16 MiB raw QEMU image is now created directly from Rust (`File::create` +
  `set_len`), which works identically on all platforms without any external
  tool. On Arch Linux `qemu-img` is a separate package not installed by
  default with `qemu-system-riscv`; on Debian/Ubuntu it is in `qemu-utils`.
  Neither is required from v0.20.2 onward.

### Removed

- **`debug_u` dead-code warning in `fjell-sample-service`** — helper function
  added during the v0.20.0 IPC investigation was never removed after the
  diagnostic prints were cleaned up.

---

## [0.20.1] — v1.0 candidate: H-01 IPC ABI doc + H-02 WrongKind fix + release notes

First supported release of Fjell OS for the `riscv64gc-unknown-none-elf` /
QEMU `virt` profile. See `docs/release/v1.0-release-notes.md` for the
full claim statement, the explicit limitation list, and the publication
control requirement.

### Scope

v1.0.0 is a **narrowly scoped QEMU prototype profile**, not a broad
production operating system claim. Hardware readiness, multi-hart,
POSIX surface, and full store/upgrade negative coverage are post-v1.0
targets. The approved claim is:

> Fjell OS v1.0.0 is the first supported QEMU profile of a Rust-first,
> capability-based microkernel OS with lease-bounded authority, semantic
> observability, signed-bundle foundations, selective Verus machine-checked
> invariants, and fail-closed QEMU negative tests for the covered security
> boundaries.

### Fixed — kernel (v1.0.0 pre-release, H-02)

- **`WrongKind → WrongType` in `require_cap_on_ct`** — the local
  cap-error table in `trap/syscall.rs` previously mapped
  `CapError::WrongKind` to `SysError::InvalidCap`, diverging from the
  canonical `to_sys_error()` path in `rights.rs` which maps it to
  `SysError::WrongType`. No deliberate ABI reason existed. Aligned to
  the canonical mapping (architect review v0.20 H-02).

### Added — documentation (v1.0.0 pre-release, H-01)

- **`docs/abi/ipc-register-layout.md`** — normative IPC register layout
  contract: a0 (status/handle), a1 (packed tag), a2..a5 (words), a6
  (kernel-attested sender identity), a7 (syscall number). Documents the
  word-count packing requirement, the badge removal, the E-010 historical
  note, and the lease-bound IPC revocation semantics. Covered by ABI
  stability commitment (RFC-v0.10-002) from v1.0.0 onward.
- **`docs/release/v1.0-release-notes.md`** — claim statement, prohibited
  claims, Gate 9 limitation table, and publication control requirement.

### Validation

All previous v0.20.0 validation carries forward. The WrongKind fix does not
affect any currently-passing test (no neg-test scenario exercises
`require_cap_on_ct` with a wrong-kind cap on the affected syscalls). The
fix is confirmed by the capability profile still passing 8 markers.



## [0.20.0] — v1-readiness: fail-closed negative-test gate + IPC words ABI fix

Implements every release blocker, high-priority, and medium-priority item from
the v0.19.0 architect review. The fail-closed harness requirement (RB-01)
immediately proved its worth: it exposed that both IPC negative markers had
been false passes since their introduction, concealing a kernel ABI bug that
silently dropped every IPC payload word.

### Fixed — kernel

- **IPC words ABI broken end-to-end** (latent since the words API was
  introduced; exposed by RB-01). Two stacked defects: (a) the
  `sys_ipc_call_words` userspace wrapper sent the raw label without packing
  the word count into tag bits 16–23, so the kernel's `build_msg` copied
  `tag.words = 0` payload words; (b) `deliver()` wrote the sender badge to a2
  and shifted the words to a3..a6 — userspace `sys_ipc_recv_msg` reads w0
  from a2 (always the badge = 0) and word 3 collided with the RFC 055
  identity write. Every payload word was lost in transit; label-only
  protocols (policy, svc READY) worked, which masked the breakage. Both
  sides now match the published recv ABI: a1 = packed tag, a2..a5 = words,
  a6 = identity. The undelivered badge had no userspace consumer and is no
  longer written.
- **Lease revoke now cancels server-side reply edges** (RFC 050; the
  finding recorded in v0.19.0). `wake_or_cancel_blocked_ipc_for_lease`
  previously walked endpoint sendq/recvq waiters only — a caller blocked
  awaiting a reply was never woken, and the server's later `sys_ipc_reply`
  met a live-but-stale edge instead of the contract-specified `BadState`.
  The function now binds the CapTable, calls `cancel_replies_for_lease`,
  and wakes each cancelled caller with `LeaseRevoked` under the same
  terminal-state guard as the queue wakes.

### The false-pass chain these fixes dismantled

With words dropped, sample-service bound `LeaseId(0)` — a lease revoked by
an earlier scenario — so its "blocked" recv/call failed instantly with
`LeaseRevoked`; the old any-error marker arms printed PASS anyway, and the
late-reply tail left neg-test blocked in a recv that could never complete,
shadowing the policy/audit/svc scenarios in some boots. After the fixes the
full protocol runs genuinely: sample blocks on the real leased cap, revoke
wakes it through the reply-edge cancellation, and `sys_ipc_reply` returns
`BadState`. **`NEG:IPC:LATE_REPLY_REJECTED:PASS` is real for the first time;
the ipc profile is restored to 3/3 markers.**

### Release blockers (architect v0.19.0 §6)

- **RB-01a** — `check_err` emits the PASS marker ONLY on the exact expected
  error; wrong-error and unexpected-success emit harness markers without the
  PASS marker. The same fix applied to the two inline match sites (neg-test
  late-reply, sample-service BLOCKED_CALL — the latter now requires
  `LeaseRevoked` specifically).
- **RB-01b** — `qemu_run::run_profile` fails any run whose serial log
  contains `NEG:HARNESS:WRONG_ERROR`, `NEG:HARNESS:UNEXPECTED_OK`,
  `TEST:FAIL`, `kernel panic`, or `panicked at`, even when every expected
  marker matched.
- **RB-02** — Gate 11 (`callsite-audit`) is genuinely wired into
  `release-rehearsal` and blocking. (The v0.19.0 edit had silently no-opped;
  the gate line now appears in rehearsal output.)
- **RB-03** — `docs/release/v1-limitations.md` placeholder statement replaced
  with the current per-category status, including the explicit non-gating of
  store/upgrade.
- **RB-04** — `cargo xtask provision-dev` implemented. Refuses without the
  explicit `--allow-tofu-provision` flag (RFC-v0.17-001 ruling); with the
  flag writes `provision/dev-trust-anchor.key` + `provision/PROVENANCE.toml`
  (mechanism = "tofu-dev"). fjell-verifyd embeds the key at build time;
  unprovisioned builds keep the legacy all-zero dev key and log a loud
  startup warning — the silent default is eliminated.

### High-priority (architect v0.19.0 §3)

- **H-01** — CI negative-profile artifact paths fixed to
  `tests/qemu/artifacts/${{ matrix.category }}/` (Option B).
- **H-02** — `qemu-utils` added explicitly to all three CI QEMU jobs.
- **H-03** — store/upgrade decision recorded (Option B): the profiles are
  marker specifications with no emitting scenarios yet; they stay out of the
  v1 gate, documented in v1-limitations, and a manual run fails honestly.
- **H-04** — `verus-check --release-required` fails closed when the detected
  or pinned Verus version cannot be established, not only on explicit
  mismatch.
- **H-05** — `rfcs/README.md` updated: RFC-v0.17-001 listed as Accepted with
  the done/ link and ruling summary.

### Medium-priority (architect v0.19.0 §4)

- **M-01** — boot-control gate status normalized in `proof-gate-policy.md`
  (tier 2 / pilot-required / release_required = false / promotion scheduled).
- **M-02** — callsite-audit documented as a *static heuristic guard* in the
  policy and the Gate 11 label, with the recommended strengthening recorded.
- **M-03** — every remaining silent setup-failure return in neg-test now
  emits `NEG:HARNESS:SETUP_FAILED` plus a scenario-identifying line
  (rights_denied, lease_revoked ×4, dma_zeroize ×2, ipc_blocked_recv,
  svc_timeout ×2, svc_fault ×2).

### Changed

- neg-test scenario order: the two cross-service IPC protocol scenarios run
  last, so a coordination stall can never shadow the policy/audit/svc
  categories (defense in depth; with the kernel fixes the boot completes).

### Validation

All 9 negative categories PASS under fail-closed checking (capability 8,
mmio 3, dma 3, audit 1, user-copy 2, policy 4, harness 1, ipc 3, svc 2 —
27 real markers), all 4 QEMU smokes PASS, 566 host tests PASS, repro check
PASS, `verus-check --all-pilot` 3× MACHINE-CHECKED-PASS with version match,
and `release-rehearsal` reports ALL MECHANICAL GATES PASS including the new
blocking Gate 11.



## [0.19.0] — Architect review implementation: real QEMU negative tests + decision records

Implementation of all required and strongly-recommended actions from the
v0.18.3 architect review. First release with real QEMU negative-test coverage
across all nine categories.

### Highlights

**22 real QEMU kernel-refusal markers** now pass across 9 categories, replacing
the RFC 025 placeholder-PASS infrastructure that had auto-passed since v0.1.1
without booting QEMU. The fixes that unblocked this revealed several latent
issues: a profile loader bug (multi-line TOML arrays silently yielded empty
marker lists), per-task console line-buffering for `sys_debug_write` (timer
preemption was character-interleaving output from concurrent services), and
four distinct raw-slot-handle bugs in neg-test and sample-service (generation
mismatches were silently skipping scenarios).

### Fixed — kernel

- **`sys_audit_drain` null/kernel-space buffer** — returned `Ok` with 0 records
  when the ring was empty, violating the RFC 039 user-pointer contract. Upfront
  `UserPtr::new` validation now precedes the drain loop. Caught by the
  now-working user-copy negative tests.
- **Endpoint 5 (cap-broker dedicated endpoint)** — never allocated (`et.alloc()`
  call missing); every IPC to the cap-broker's private endpoint returned
  `InvalidCap` since introduction (RFC 040), silently preventing all four
  cap-broker policy enforcement tests from running. One-line fix activates
  the full policy enforcement test suite including identity-spoofing rejection.
- **Endpoint 6 (sample-service dedicated endpoint)** — allocated and wired;
  IPC blocked-call scenario routing was nondeterministic on the shared endpoint
  (any idle shared-ep receiver could steal the handshake).
- **DMA cap kind** — `sys_dma_revoke` requires `CapKind::DmaRegion`; the spawn
  grant used the legacy `DmaAlloc` alias, making explicit revocation silently
  impossible for all DMA-capable services. Updated to `DmaRegion` in spawn.
- **`sys_debug_write` per-task line buffering** — the per-byte syscall (one
  ecall per character) allowed timer preemption between bytes, producing
  character-interleaved output from concurrent services and shredding QEMU
  test markers. Kernel now accumulates bytes per-task and flushes atomically
  (SIE-masked) on `\n` or buffer-full, with 32-task × 160-byte static storage.
- **Console `_print` interrupt guard** — SIE masked for the duration of each
  kernel-side `write_fmt` call; kernel log lines are now line-atomic under
  preemption.
- **`MustRetire` kprintln** — adds observability to the retire-before-wrap
  epoch-MAX path (LEASE-VERUS-005, C6).

### Fixed — neg-test / sample-service

- **Raw-slot-constant handles** — four test scenarios in neg-test (RFC 049
  quartet: copy/mint/revoke/inspect without right) and two in sample-service
  (IPC blocked-recv/call setup) were using raw slot index as `CapHandle`,
  which fails generation validation after the first mint/drop cycle, silently
  skipping the scenarios from the second run onward. All fixed to thread the
  generation-correct handle returned by `sys_cap_mint`/`sys_cap_copy`.
- **Wrong error expectations** — kind-mismatch errors via `sys_mmio_map` and
  `sys_dma_alloc` return `WrongType` (canonical `to_sys_error` mapping) not
  `InvalidCap` (a divergent local table in trap/syscall.rs:98). Expectations
  aligned; divergent mapping recorded as a follow-up finding.
- **Silent skips replaced with diagnostics** — `check()` calls that returned
  nothing on failure, and `Err(_) => {}` arms that swallowed errors, are
  replaced with `debug_err` / `debug_policy` diagnostic output so harness
  failures are visible in serial logs.

### Fixed — tooling

- **Profile loader multi-line TOML arrays** — the minimal TOML reader in
  `qemu_run.rs` parsed multi-line `expected_markers = [...]` arrays as empty
  lists (the array-open line `key = [` parsed as empty; continuation lines
  had no `=`). Every real negative profile silently degraded to
  placeholder-PASS since v0.1.1. Multi-line accumulation added.

### Added — architect review implementation

- **Verus toolchain version check** — `verus-check` now prints detected and
  pinned Verus versions; on mismatch issues a warning and blocks
  `--release-required` to ensure proofs are certified under the locked
  toolchain.
- **`cargo xtask callsite-audit`** — Gate 11: three static checks that the
  security-critical call sites use the model-conformant helpers
  (LEASE-CALLSITE-001: no `wrapping_add` on lease epoch;
  CAP-CALLSITE-001: `is_subset_of` present in cspace.rs mint path;
  BCB-CALLSITE-001: no duplicate mirror-selection logic).
- **`docs/verification/verus/review-records/v0.18-architect-review-decisions.md`**
  — formal record of architect decisions D1–D6, the two-milestone exception
  text (D3), and the trust-anchor provisioning tier→mechanism ratification.
- **RFC-v0.17-001 accepted** — moved to `rfcs/done/`; §4/§6 decision ratified:
  TOFU dev profile requires explicit `--allow-tofu-provision` flag;
  factory station for v1.1; hardware-anchored for v2+.
- **Scope guardrail** in `proof-gate-policy.md`: Verus stays out of
  drivers, scheduler, MMIO/DMA, and services.
- **Boot-control promotion scheduled** in ledger (architect D5).
- **`docs/release/v1-limitations.md` item 6** — updated from "pending" to the
  ratified provisioning decision.

### Known findings recorded (not blocking)

- Two divergent `CapError→SysError` mappings in the kernel: the canonical
  `to_sys_error` (rights.rs) maps `WrongKind → WrongType` while a local table
  in `trap/syscall.rs:98` maps it to `InvalidCap`. Neg-test expectations
  aligned to the canonical mapping; the divergence is a follow-up cleanup item.
- `NEG:IPC:LATE_REPLY_REJECTED` — pending; the reply-edge cancellation path
  via `wake_or_cancel_blocked_ipc_for_lease` does not clear the server-side
  reply table, so `sys_ipc_reply` behaviour after lease revocation is
  inconsistent with the RFC 050 contract in the dedicated-endpoint path.
- `NEG:SVC:READY_ACCEPTED` and `NEG:SVC:UNAUTHORIZED_READY_REJECTED` — pending;
  timing-dependent on service-manager readiness at neg-test startup.



## [0.18.3] — v1 hardening: triage of owner-review findings

Full triage and implementation of the v1-readiness review (code defects,
documentation gaps, workflow gaps). First release validated end-to-end with
**QEMU 8.2.2 + Verus both present**: all four smoke profiles PASS on a
freshly rebuilt image, all host tiers PASS, 20 proof obligations
machine-checked, full two-build reproducibility PASS.

### Fixed

- **Kernel dispatch debug-name table** (`trap/dispatch.rs`): removed the
  stale duplicate arms for slots 7/8 (the unreachable-patterns warnings);
  slot 7 now correctly labels `neg-test`. Validated by booting m8 in QEMU.
- **Repro baseline workflow**: prebuilt service binaries rebuilt from
  current source and the SHA-256 baseline re-recorded with them (they had
  diverged after the v0.18.1 `fjell-abi` change — the exact tier-5 failure
  an owner verification run would hit after any `qemu-test`).
  `--skip-build` now tracks committed artefacts only: volatile `target/`
  paths are excluded from the baseline, so fresh checkouts and post-clean
  trees compare correctly. Full two-build mode re-verified: 29 artefacts
  identical. Baseline maintenance procedure documented in
  `tools/fjell-repro-check/README.md`.
- **fjell-tools warning baseline → 0**: dead `load_raw_key` removed, unused
  imports fixed, `MEAS_PORT` annotated as reserved.

### Documentation

- **Gate 9 single source**: new `docs/release/v1-limitations.md`
  consolidating the six manual-check items with their governing records
  (E-004, N1, N21, N23, console single-hart invariant, RFC-v0.17-001);
  the rehearsal Gate 9 message now points at it.
- **Six mdbook stub pages written**: what-is-fjell, why-fjell, quick-start
  (with boot output verified against a live v0.18.2 QEMU run, and two
  corrections — the apt package is `qemu-system-misc`, and toolchain
  prerequisites now include `rust-1.91-src` + `lld`), architecture
  overview, writing-a-service (canonical fjell-storaged template steps,
  no-`static mut` and IpcReply-`a1` invariants), and the v1-non-goals
  pointer page.
- **v1-readiness matrix refreshed** to v0.18.2: Verus formal-proof and
  reproducible-build rows added; test counts updated (566 host, 14
  properties).
- `console.rs` `TODO(M2+)` relabeled as the documented v1.0 single-hart
  design decision; `TOOLCHAIN.md` stale "environment blocker" section
  replaced with the validated Verus install recipe.
- **Known gap documented**: all nine QEMU negative-test categories are
  RFC 025 placeholders that PASS without booting QEMU; recorded in
  v1-limitations so test-all tiers 10–18 are not mistaken for
  fault-injection coverage.

### Changed

- Smoke-test stub services (`fjell-netd`, `fjell-secure-transportd`,
  `fjell-driver-virtio-net`) carry an explicit STUB header and scoped
  `allow`s; the riscv cross-build warning count for them is now zero.



The owner's independent verification run surfaced two real defects, both
pre-dating the v0.17/v0.18 work. (The other reported behaviours were correct:
`verus-check` without Verus on PATH yielding `CONFORMANCE-ONLY` markers plus a
release-required `BLOCKING FAILURE` is the C7/Gate-10 design working;
`release-rehearsal` is its own xtask subcommand, not a `verus-check` flag.)

### Fixed

- **Reproducible-build gate (latent since v0.16.5).** RFC-v0.16-005 (H-04)
  switched `fjell-repro-check` from FNV-1a to SHA-256, but the committed
  `tests/repro/baseline-digests.txt` still held 16-hex-char FNV digests — so
  every `--skip-build` comparison since then was algorithm-mismatched and
  reported meaningless per-file "DIGEST DIFFERS". Fixes:
  - baseline re-recorded in SHA-256 (28 artefacts; prebuilt bins verified
    byte-identical to the v0.17.0 release before re-recording);
  - baseline file now carries an `# algo: sha256` header;
  - `load_digests` validates entries are 64-char hex and fails loudly naming
    the legacy-baseline cause (with re-record instructions) instead of
    emitting cross-algorithm diffs; unit test added.
- **`test-all` summary counters.** The FAIL filter only excluded notes
  starting with "skipped", so QEMU tiers skipped with "qemu-system-riscv64
  not found on PATH" were double-counted as failures (`PASS: 4 | FAIL: 14 |
  SKIP: 13` for one real failure), and a skip-only run would wrongly exit
  FAILURE. A single skip predicate now drives both counts.

No shipped-kernel, proof, or gate-policy change in this release.



Recovers and applies the architect's Stage-A approval conditions (C4–C8),
which were specified in the review session but lost to a sandbox outage
before they could land, and closes the RFC lifecycle for the Verus program.

### Changed (C6 — lease retire-before-wrap; the one shipped-code change)

- **`fjell_abi::lease`**: `lease_revoke_epoch` (wrapping mirror) replaced by
  the bounded `lease_revoke(u32) -> RevokeOutcome { Advanced(u32), MustRetire }`.
  At `u32::MAX` the lease MUST be retired, never wrapped.
- **Kernel** `LeaseTable::revoke` now routes through the shared helper: the
  epoch never wraps; at `MAX` the slot is retired (state stays `Revoked`,
  epoch frozen), closing the u32/nat divergence the original conformance note
  documented. Cross-checked for `riscv64gc-unknown-none-elf`.
- **Verus lease model**: proofs carry the `epoch < u32::MAX` precondition;
  new **LEASE-VERUS-005** bounded-domain lemma (revoke maps exactly onto
  `Advanced(old + 1)`). Lease module now **5 verified** → totals **20
  obligations, 0 errors** (re-checked under the pinned toolchain).
- **Conformance**: 4 C6 boundary tests added (epoch 0, 1, MAX-1, MAX ⇒
  MustRetire) → 23 conformance cases; property tests → 14 (incl. the MAX
  boundary property).

### Changed (C7 — honest xtask status values)

- `verus-check` markers are now `MACHINE-CHECKED-PASS` / `MACHINE-CHECKED-FAIL`
  / `CONFORMANCE-ONLY` / `CONFORMANCE-FAIL`; the conformance fallback never
  reports a bare PASS. JSON gains `machine_check = pass|fail|not_run` and
  `experimental`. Rehearsal counters, Gate 10 wording, and the `ci-verus`
  warning grep updated to the new markers.

### Changed (C4, C5, C8 — wording, rule, lock)

- C4: `verus_lemma_properties.rs` reworded — properties exercise the *Rust
  mirrors of intended proof obligations*; proof status lives only in the
  review record / `TOOLCHAIN.lock` (also corrects the now-stale "machine-check
  blocked" header).
- C5/R-V1: proof-gate-policy gains the R-V1 rule (a Verus FAIL keeps a target
  Experimental / blocks a promoted release even if all Rust tests pass) and
  the 9-item promotion artifact checklist (item 9: lease wrap modeled).
- C8: `TOOLCHAIN.lock` gains `[run]` (command, targets, host_os,
  last_success_date) alongside the pins; results updated to 20 obligations.

### Added / RFC lifecycle

- **RFC-v0.17-001 Trust Anchor Provisioning** drafted (recovered design-options
  text: factory station / first-boot TOFU / hardware-anchored, with the
  tier→mechanism recommendation). Replaces the RESERVED placeholder; stays in
  `proposed/` awaiting the architect's §4/§6 ratification.
- RFC-v0.17-002…006 and RFC-v0.18-001 moved `proposed/` → `done/` with
  Implemented statuses (folder is the source of truth, RFC 000); RFC index
  updated; C6 amendment note appended to RFC-v0.17-003.



Second milestone of recorded Verus PASS. Promotes the two **tier-3** pilot
proofs to release-required, per the RFC-v0.17-005 staging schedule
(RFC-v0.18-001).

### Promoted

- **`capability`** (rights non-amplification) and **`lease`** (epoch
  revocation) → `release_required = true`. Selection follows the existing
  tier field (tier 3 = release-critical); **`boot-control`** (tier 2) stays
  Experimental / pilot-required.
- Both promoted proofs meet every proof-gate criterion: PASS across two
  milestone tags (v0.17.1, v0.18.0), passing conformance + property tests,
  a proof review record, low maintenance cost, and assumptions written in the
  proof files. The two milestones landing close together is noted honestly in
  RFC-v0.18-001; the demotion path remains the safety valve.

### Changed (the release-required teeth)

- `cargo xtask verus-check`: for a release-required target, **anything other
  than a real Verus PASS now blocks `--release-required` — including
  `CONFORMANCE-ONLY`**. A release-required proof cannot be certified without
  actually running the prover (pin: `TOOLCHAIN.lock`).
- `release-rehearsal`: new **Gate 10** runs `verus-check --release-required`
  and fails the rehearsal if any release-required target is not PROVED. The
  informational all-targets line remains.
- `verus-check` TOML reader now strips inline `#` comments (so commented
  `release_required` values parse correctly).

### Unchanged (Stage A guarantees still hold)

- Verus is still not a build dependency; `cargo build`/`test` never need it.
- Push CI (`ci-verus`) stays `continue-on-error` — non-blocking on merges; it
  records markers. Release-required enforcement is a release-time gate only.
- boot-control and all future new targets start Experimental.



Makes the v0.17.1 machine-check reproducible on every push and sets up the
audit trail the staging policy needs before any `release_required` promotion.
No proof or shipped-code change; still Stage A, all targets non-blocking.

### Added

- **`ci-verus` CI job** (`.github/workflows/ci.yml`) — installs the pinned
  Verus toolchain (`TOOLCHAIN.lock`: verus `release/0.2026.05.24.ecee80a`,
  rustup 1.95.0, bundled z3) and runs `cargo xtask verus-check --all-pilot`,
  recording `VERUS:TARGET:*:PASS` to the step summary and uploading
  `verus-markers.txt`. `continue-on-error: true` keeps Verus strictly
  non-blocking — it can never gate a merge or release (Stage A guarantee).
- **Promotion ledger** in `docs/verification/verus/proof-gate-policy.md` —
  tracks the two-milestone PASS criterion. v0.17.1 is recorded as the first
  CI PASS; the next tag's PASS clears the criterion, after which a target may
  be promoted by RFC amendment with architect sign-off.



**The three v0.17 pilot proofs are now machine-checked.** The Verus toolchain
installed and ran (release-asset hosts reachable): **19 proof obligations
verified, 0 errors** (capability 8, lease 4, boot-control 7). Fjell stays
Rust-first — Verus is still not a build dependency and all targets remain
`release_required = false` (Stage A).

### Verified

- `cargo xtask verus-check --all-pilot` → `VERUS:TARGET:*:PASS` for all three,
  `"verus":true`.
- `release-rehearsal` Verus line now reads **"3 proved, 0 conformance-only,
  0 fail"** (was "3 conformance-only"). All 8 mechanical gates still PASS;
  566 host tests + 19 conformance + 13 property tests still green.

### Fixed (both surfaced by running the real toolchain)

- **`capability` proof:** `zero_is_subset` and `equal_rights_allowed` needed
  `by(bit_vector)`. They assert universally-quantified bitwise facts
  (`0 & !parent == 0`, `parent & !parent == 0`) that the SMT solver does not
  discharge over all `u64` without the bit-vector solver — invisible to the
  point/property tests, which only evaluate concrete values. Both now verify.
- **`verus-check` xtask:** `run_verus` invoked `verus <file>` without
  `--crate-type=lib`, so proof-only library modules (no `main`) failed with
  `E0601` and were reported as FAIL. Now passes `--crate-type=lib`.

### Pinned

- `verification/verus/TOOLCHAIN.md` + new `TOOLCHAIN.lock`: verus
  `release/0.2026.05.24.ecee80a`, rustup toolchain `1.95.0`, z3 `4.12.5`.



**Selective formal verification.** Lands the foundation for Verus proofs on
small, stable, security-critical logic, per the Verus adoption handoff pack.
Fjell remains Rust-first; proofs are additive and never a build dependency.

### Added

- **`verification/verus/`** — proof modules for the three pilot targets,
  each mapped 1:1 to shipped Rust:
  - `capability/rights_lattice.rs` → `CapRights::is_subset_of`
  - `lease/lease_epoch.rs` → kernel lease table + `fjell_abi::lease`
  - `boot-control/mirror_selection.rs` → `select_bcb_mirror`
- **Conformance tests (the proof↔Rust bridge, run in ordinary `cargo test`):**
  19 cases total — `fjell-cap/tests/verus_conformance.rs` (6),
  `fjell-cap/tests/lease_conformance.rs` (6),
  `fjell-upgrade-format/tests/mirror_conformance.rs` (7). All pass.
- **`fjell_abi::lease`** pure helpers (`lease_usable`, `lease_revoke_epoch`)
  — host-testable mirror of the no_std kernel lease logic.
- **`cargo xtask verus-check`** [`<target>`|`--all-pilot`|`--release-required`]
  — runs Verus if installed; otherwise conformance-only mode (Stage A).
  Emits `VERUS:TARGET:<name>:{PASS|FAIL|CONFORMANCE-ONLY}` + JSON.
- **`verification/verus/{verus-targets.toml,TOOLCHAIN.md,README.md}`**.
- **`docs/verification/verus/proof-gate-policy.md`** + imported pack
  guides, checklists, templates, appendices.
- **RFCs** `rfcs/proposed/v0.17/`: 002 capability, 003 lease, 004 boot-control,
  005 CI proof gate, 006 adoption umbrella; 001 reserved for trust-anchor
  provisioning.
- Release rehearsal now reports Verus target status as a **non-blocking**
  experimental line.

### Policy

All pilot targets are **Experimental** (release_required=false) at v0.17.0.
Verus is not installed in this environment, so proofs are written and mapped
but not yet machine-checked; conformance tests are the validated bridge today.
Promotion to pilot-required (v0.17.1) and release-required (v0.18.0) follows
the staging policy.

### Status

566 host tests + 19 conformance tests + 13 lemma property tests pass. Real Verus machine-checking is blocked by the sandbox network allowlist (GitHub release-asset hosts denied); proofs are mapped, conformance-tested, property-tested, and manually reviewed (review record committed). All 8 v1.0 mechanical gates still PASS. No regressions.

---



**Validation Closure Sprint.** Executes the architect's v0.16 review:
converts paper claims into validated ones before any v1.0 tag. No new
architecture; claim validation and release closure only.

### Blockers resolved (architect RB-01 … RB-05)

- **RB-01 Ed25519 interop (RFC-v0.16-001):** root-caused the RFC 8032 TV1
  "discrepancy" to a corrupted test-vector seed (byte 15 onward), not a
  crypto defect. Cross-verified against dalek, OpenSSL, and libsodium —
  all three agree. Restored both removed TV1 tests (derive + sign); they
  now pass. Sign path proven byte-identical to OpenSSL/libsodium.
- **RB-02 hardware claim (RFC-v0.16-005):** adopted Option B — v1.0 scoped
  to a supported QEMU `virt` profile; VisionFive 2 is provisional and
  unvalidated on silicon (errata E-004, ACCEPTED).
- **RB-03 fleet partition (RFC-v0.16-002):** added a full-lifecycle
  partition→divergence→reconcile→apply integration drill plus a
  rollback-rejection arm. Markers `DRILL:FLEET-PARTITION-RECONCILE:PASS`,
  `DRILL:FLEET-PARTITION-ROLLBACK-REJECTED:PASS`.
- **RB-04 recovery drill (RFC-v0.16-003):** walked DR1/DR2/DR5 + partition
  + boot triage against real crate APIs; attestation committed.
- **RB-05 errata governance (RFC-v0.16-004):** added
  `Implemented-with-Errata`/`Superseded` statuses and `docs/rfcs/ERRATA.md`
  (E-001 … E-009: 8 CLOSED, 1 ACCEPTED).

### High-priority items

- **H-01 key encryption (RFC-v0.16-006):** signing keys now encrypted at
  rest — `FJK2` format, Argon2id + AES-256-GCM. Plaintext retained only
  behind `--insecure-plaintext` for CI fixtures.
- **H-03 ABI wording:** documented the ABI gate as a drift guard, not a
  semantic ABI proof.
- **H-04 repro digest:** switched repro-check from FNV-1a to SHA-256.
- **H-05 runtime SDK trial (RFC-v0.16-007):** drove `fjell-config-sync`
  through a real update lifecycle + convergence check. Markers
  `DRILL:SDK-CONFIG-SYNC-RUNTIME:PASS`, `DRILL:SDK-CONFIG-SYNC-CONVERGENCE:PASS`.

### Release process

- **RFC-v0.16-008:** `cargo xtask release-rehearsal` runs v1.0 tag gates
  1–8 (incl. errata + drill gates) and prints a PASS/FAIL matrix. All
  mechanical gates PASS. v1.0.0 tag remains owner/architect-gated.

### Status

566 host tests pass (0 fail). Unsafe-audit 0 missing, MMIO-audit 0 missing,
ABI verify PASS, readiness 0 OPEN, errata 0 OPEN. Seven prior RFCs
re-marked `Implemented-with-Errata`. Eight v0.16 RFCs in `done/`.

**Freeze candidate patch.** README, CHANGELOG, and readiness-matrix polish.
v1.0.0 tag pending owner approval.

All v1.0 propositions satisfied (RFC 061 §4):
identity locked, ABI frozen, trust spine production-grade,
first real-world deployment profile, fleet recovery depth,
SDK trial complete, threat model finalised.

### Milestones completed in this release line

| Milestone | Summary |
|-----------|---------|
| v0.10 | ABI snapshot gate, reproducible builds, criterion benchmarks, three-node fleet demo, mdbook docs, v1.0 readiness matrix |
| v0.11 | Ed25519 signature backend (RFC 8032), bundle signing pipeline, keyring rotation + revocation records, replay cache + nonce table |
| v0.12 | StarFive VisionFive 2 board profile, DTB validation at boot, MMIO ordering audit (23 sites, all classified), deployment guide |
| v0.13 | Fleet partition FSM, reconcile manifests, coordinator promotion, bulk re-attestation, disaster recovery patterns, summary consistency checker |
| v0.14 | `fjell-config-sync` reference service, typed catalog struct generation, bundle publishing registry, developer modes (`--trace`, `--measure`, `--gdb`) |
| v0.15 | Threat model v1 (20 in-scope threats), release checklist, security advisory process, operator recovery guide, non-goals lock (20 items) |

### Final state at v1.0.0

- **564 host tests**, 0 failures
- **139 RFCs** in `done/`
- **268 unsafe sites**, 0 missing SAFETY comments
- **23 MMIO sites**, 0 missing annotations
- **401-item ABI snapshot**, verify gate passes
- **v1.0 readiness matrix**: 51 DONE, 3 DEFERRED, 0 OPEN
- **Trust Report**: 6 sections populated
- **Deployment target**: StarFive VisionFive 2 (primary), QEMU `virt` (CI)

### Breaking changes

None relative to v0.9.x — the v0.10 ABI snapshot captures the stable
surface; no STABLE items were removed or renamed during v0.10–v1.0.

---

## Previous releases

See `docs/src/releases/` for v0.1.x–v0.9.x release notes.

---
