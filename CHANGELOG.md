# Changelog

All notable changes to Fjell OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.1] - 2026-05-17

### v0.1.x stabilization — Release freeze + CI foundation

This is the first stabilization release in the v0.1.x line.  It adds
no new OS functionality.  It freezes the v0.1.0 prototype, documents
its limitations, lays down the CI / negative-test infrastructure, and
files the v0.2 design RFCs so v0.2 can begin with a coherent plan.

### Added

- **RFC set 024–047** in `rfcs/`:
  - 024 (RFC-v0.1.x-001) — release freeze and scope declaration *(Accepted)*.
  - 025 (RFC-v0.1.x-002) — CI / QEMU automation foundation *(Accepted)*.
  - 026 (RFC-v0.1.x-003) — negative-test harness *(Proposed)*.
  - 027 (RFC-v0.1.x-004) — threat model and security boundaries *(Proposed)*.
  - 028 (RFC-v0.1.x-005) — syscall / ABI / protocol inventory *(Proposed)*.
  - 029 (RFC-v0.1.x-006) — capability / lease enforcement audit *(Proposed)*.
  - 030 (RFC-v0.1.x-007) — MMIO / DMA boundary audit *(Proposed)*.
  - 031–043 (RFC-v0.2-001..013) — full v0.2 *Security Boundary Closure*
    RFC set *(Proposed)*: unified capability enforcement, CSpace GC,
    lease epoch revocation, blocked-IPC wake/cancel, MmioRegion ABI
    replacement, DmaRegion zeroize/quarantine, non-blocking IPC + timer
    fail-safe, service-plane separation, safe user copy + real audit
    drain, cap-broker bootstrap handoff and default deny, persistent
    evidence hardening, v0.2 negative-test expansion, v0.2 security
    boundary release gate.
  - 044 (RFC-v0.1.x-008) — audit / snapshot / semantic evidence audit
    *(Proposed)*.
  - 045 (RFC-v0.1.x-009) — ADR and documentation synchronization
    *(Proposed)*.
  - 046 (RFC-v0.1.x-010) — v0.1.x release checklist *(Proposed)*.
  - 047 (RFC-v0.1.x-011) — v0.2 preparation backlog *(Proposed)*.
- **Documentation** under `docs/src/`:
  - `releases/v0.1.0-scope.md` — what v0.1.0 includes.
  - `releases/v0.1.0-limitations.md` — what v0.1.0 is *not* (no
    production secure boot, no remote attestation, no networking, no
    POSIX, etc.).
  - `security/v0.1.0-known-non-goals.md` — non-goals contributors
    must not extend into.
  - `security/v0.1.0-threat-model.md` — skeleton; full body lands
    with RFC 027 in v0.1.2.
  - `roadmap/v0.1.x-stabilization.md` — v0.1.1 → v0.1.5 sequence.
- **`fjell-tools` xtask extensions** (RFC 025):
  - `cargo xtask qemu-negative <category>` — runs a profile-driven
    negative test under `tests/qemu/profiles/`.
  - `cargo xtask qemu-log-check <log-file> <marker>` — generic
    substring-match validator.
  - `cargo xtask qemu-run --profile <name>` — explicit profile runner.
  - All QEMU runs now write to `tests/qemu/artifacts/<run-id>/` with
    `serial.log`, `qemu-command.txt`, `expected-markers.txt`, and
    `result-summary.txt`.
- **Placeholder profile TOMLs** for the six v0.1.x negative-test
  categories (`capability`, `ipc`, `mmio`, `dma`, `store`, `upgrade`).
  Each profile asserts no markers yet — they are real PASSes
  *infrastructure-wise* per RFC 025 §"chicken-and-egg" exemption; case
  bodies land per v0.2 RFC.
- **`.github/workflows/ci.yml`** with five jobs (`ci-format`,
  `ci-check`, `ci-test-host`, `ci-qemu-smoke`, `ci-qemu-negative`),
  matrix-parameterised over milestones / categories, with artefact
  upload.

### Changed

- `README.md` updated: version stamp v0.0.2 → v0.1.1, prominent
  limitation warning block linking to
  `docs/src/releases/v0.1.0-limitations.md`.
- `ROADMAP.md` updated: replaced placeholder v0.2–v0.4 stub with the
  full v0.1.x stabilization table, v0.2 nine-phase plan, and v0.3
  through v1.0 progression.
- `docs/src/SUMMARY.md` updated: new top-level sections *Releases*,
  *Roadmap*, *Security* preceding *Getting Started*.
- `crates/fjell-tools/src/main.rs` rewritten to dispatch the four
  RFC-025 subcommands.
- `crates/fjell-tools/src/smoke.rs` refactored to use the shared
  `Profile` / `run_profile` runner; semantics preserved
  (TEST:Mx:PASS marker map unchanged).
- Workspace version bumped `0.1.0 → 0.1.1`.

### Fixed

- *(none — this release intentionally adds no OS functionality)*

### Security

- No security-boundary changes in v0.1.1 itself. The v0.2 RFC set
  (RFCs 031–043) defines every boundary closure that will land in
  v0.2.0.
- Threat-model and limitations are now explicit project documents
  rather than implicit assumptions.

### Known Limitations

All limitations documented in
`docs/src/releases/v0.1.0-limitations.md` apply unchanged to v0.1.1.
In particular: no production secure boot, no hardware-rooted trust,
no remote attestation, no networking, no POSIX, no GUI, no fully
verified components, no uniform security-boundary enforcement.

### Deferred to v0.2

- Implementation of every RFC-v0.2 design (RFCs 031–043).
- Replacing `caller_has_cap` style checks with `require_cap`.
- O(1) lease epoch revocation across syscall and IPC paths.
- Blocked-IPC wake/cancel on revoke.
- MmioRegion / DmaRegion capability ABIs.
- DMA zeroize / quarantine.
- `sys_ipc_try_recv` + cooperative service loops + timer fail-safe.
- Real service-plane separation (ADR-0010 supersession).
- Safe `copy_to_user` + real audit drain (binary AuditRecord).
- `cap-broker` bootstrap handoff and default-deny policy engine.
- Persistent evidence hardening matrix.
- v0.2 negative-test expansion and v0.2 release gate.

## [0.1.0] - 2026-05-17

### M8 completion — Local Evidence / Attestation / Recovery Plane

This is the v0.1.0 release of Fjell OS, completing all eight milestones (M0-M8).

#### PR-M8-07: Semantic stream integration

- `fjell-semantic-format` extended with M8 StateKind (MeasurementStatus,
  AttestationStatus, BundleFreshnessStatus, RecoveryStatus), EventKind
  (BundleFreshnessRejected, RollbackSelected, etc.), and ActionKind
  (SelectRollback, InspectSnapshots).

- `fjell-proxy-text` extended with M8 render helpers: `render_measurement_status`,
  `render_attestation_status`, `render_freshness_status`, `render_recovery_status`,
  `render_recovery_intent`, `render_freshness_rejected_event`,
  `render_rollback_selected_event`.

- `fjell-init` now publishes the four M8 StateNodes during the smoke run:
  `[STATE][Ok] Measurement status`, `[STATE][Ok] Attestation status`,
  `[STATE][Ok] Bundle freshness`, `[STATE][Ok] Recovery status`.
  RecoveryIntent IntentNode is rendered with Inspect/Rollback actions.

#### PR-M8-08/09: Negative path smoke and hardening

- Negative path (generation-rollback rejection to recovery target) exercised
  inline. A bundle with gen=3 against last_accepted_gen=5 is rejected.
  `[EVENT][Important][Failed] Bundle freshness rejected` and
  `[STATE][Failed] Bundle freshness` are published. `recoveryd` ENTER_RECOVERY
  is called; `[STATE][Ok] Recovery status` and RecoveryIntent are re-published.

#### v0.1.0 Acceptance criteria

All M8 acceptance criteria from the internal design document (sections 20.1-20.8)
are met. `TEST:M8:PASS` and `TEST:M7:PASS` are emitted. 24 format-crate unit
tests pass covering SHA-256 correctness, chain-digest determinism, attestation
tamper detection, and all freshness rejection paths.

## [0.0.15] - 2026-05-17

### Added (M8: Local Evidence / Attestation / Recovery Plane)

- **PR-M8-01 — Format crates**: three new `no_std` library crates establish
  the M8 data model.
  - `fjell-measure-format`: append-only measurement chain types.  Implements
    a compact SHA-256 (no external crate) for deterministic chain-digest
    computation: `chain_digest[n] = SHA256(domain || seq || kind || source ||
    subject || subject_digest || metadata_digest || prev_chain_digest)`.
    `MeasurementEvent`, `MeasurementHead`, `MeasurementKind/Source/Subject`
    enums.  24 unit tests pass including SHA-256 known-answer vectors,
    determinism, chain-linkage, and metadata influence.
  - `fjell-attestation-format`: `AttestationRecord`, `SignedAttestationRecord`,
    `DevAttestation` (development-grade Ed25519 stand-in via SHA-256 keyed
    under `dev-attest-m8-01`).  Canonical digest covers all claims fields.
    Tamper-detection unit tests confirm that modified records fail verify.
  - `fjell-recovery-format`: `RecoveryRequest/Response`, `BundleMetadataV2`,
    `FreshnessCheck`, `FreshnessStatus`, `SlotInspection`, rollback types.
    `BundleMetadataV2::check_freshness` enforces validity window, generation
    anti-rollback, and key-epoch anti-rollback.  8 unit tests cover all
    rejection paths (expired, not-yet-valid, generation rollback, key-epoch
    rollback, unsupported schema).

- **PR-M8-02 — Service API extension**: `fjell-service-api` extended with
  `measuredd`, `attestd`, `recoveryd`, and `verifyd` protocol modules
  (tags 0x300–0x33F).  Backward-compatible with all M7 tags (0x200–0x215).

- **PR-M8-03 — measuredd**: new `fjell-measuredd` service.  Maintains an
  in-memory append-only measurement chain (up to 64 events, ring buffer).
  Accepts `APPEND_EVENT` (kind | source | subject | digest), replies with
  `APPEND_OK` carrying the new sequence number and chain-digest prefix.
  `GET_HEAD` returns current chain state.  Announces readiness on private
  endpoint 2.

- **PR-M8-04 — verifyd freshness**: bundle freshness validation implemented
  in `fjell-recovery-format::BundleMetadataV2::check_freshness` and exercised
  in `fjell-init`.  Valid bundle (gen=5, epoch=3, tick∈[1000,9000]) is
  admitted.  Expired bundle (tick=9999) and generation-rollback bundle
  (last_gen=6 > gen=5) are both rejected, satisfying FRESH-INV-001 through
  FRESH-INV-003.

- **PR-M8-05 — attestd**: new `fjell-attestd` service.  Generates local
  development-grade attestation records from measurement chain state.  Signs
  with `DevAttestation` (SHA-256 keyed under `dev-attest-m8-01`).  Reports
  `GENERATED` and passes `VERIFY_LATEST` check in the smoke test.

- **PR-M8-06 — recoveryd**: new `fjell-recoveryd` service with
  `LIST_SNAPSHOTS`, `INSPECT_SLOT`, `ENTER_RECOVERY`, `SELECT_ROLLBACK`, and
  `EXPORT_DIAGNOSTICS` handlers.  Enforces `REC-001`: `SELECT_ROLLBACK`
  without `confirmed_by_operator=true` returns `ERR::NotConfirmed`.  Confirmed
  rollback returns `ROLLBACK_SELECTED`.

- **Kernel endpoint allocation**: endpoints 2, 3, 4 pre-allocated in
  `main.rs` for measuredd, attestd, recoveryd respectively.  `spawn.rs`
  updated to assign private endpoint IDs per service.  Init's CSpace gains
  slots 3–5 for the M8 service endpoints.

- **M8 smoke test**: `cargo xtask qemu-test m8` now emits `TEST:M8:PASS`
  after exercising all six M8 acceptance criteria: (1) boot evidence import,
  (2) verification result append, (3) freshness valid path, (4) expired-bundle
  rejection, (5) generation-rollback rejection, (6) attestation generation and
  verification, (7) unconfirmed-rollback rejection, (8) confirmed-rollback
  acceptance.  `TEST:M7:PASS` is preserved.

## [0.0.14] - 2026-05-17

### Added (RFC 019, storaged / virtio-blk IPC completion)

- **storaged: virtio-blk device discovery** — corrected QEMU command from
  `-drive if=virtio` (which creates `virtio-blk-pci` on PCIe, leaving all
  eight virtio-mmio buses empty) to
  `-drive file=…,if=none,id=hd0 -device virtio-blk-device,drive=hd0`.
  storaged now scans virtio-mmio buses 0–7 for DeviceID = 2 and correctly
  finds the block device placed by QEMU on bus 1 (bus 0 = virtio-rng).

- **storaged: virtio v1 legacy init ordering** — DRIVER_OK is now written
  *before* QueueNum / QueuePFN so that `virtio_bus_start_ioeventfd` fires
  while `vring.num == 0` (default after reset = 256, but the guest has not
  yet written QueueNum).  This prevents the ioeventfd from being registered
  at offset 0x050, which would silently intercept every subsequent
  QueueNotify write and bypass `virtio_mmio_write`.  All writes therefore
  take the synchronous direct-call path through `virtio_queue_notify`.

- **IpcReply ABI fix** — `reply(tag)` in storaged was placing the reply
  tag in `a0`; the kernel's `sys_ipc_reply` reads the reply label from
  `a1` (`gpr[REG_A1]`).  Fixed to `in("a1") tag`, `in("a0") 0`.  Without
  this fix every `WRITE_OK` reply delivered garbage to init, causing all
  `storaged_write` calls to return `false` even though the virtio I/O
  completed correctly.

- **M7 smoke test passes end-to-end** — `TEST:M7:PASS` is now reliably
  emitted.  All six `storaged_write` calls (sector 193, LBA_SUPERBLOCK_A,
  LBA_LOG_START, LBA_SUPERBLOCK_A×2, LBA_BOOT_CTL_A_START,
  LBA_BOOT_CTL_B_START) succeed; `virtio_blk_req_complete status=0` is
  confirmed for each via QEMU trace.

### Fixed

- `fjell-tools` smoke runner: QEMU command updated to use
  `virtio-blk-device` (virtio-mmio transport) instead of the implicit
  `virtio-blk-pci` created by `-drive if=virtio`.

## [0.0.13] - 2026-05-16

### Added (RFC 020, 021, 016, 017)

- **RFC 020 — Audit drain** (`sys_audit_drain`): kernel audit ring is now
  fully drained by `auditd` via a capability-gated syscall.  `AuditRing` gains
  a `drain_cursor` + `compact()` so consumed slots are reclaimed.  A new
  `copy_to_user_bytes` helper translates user VA → PA through the Sv39 page
  table and writes via the kernel identity map.  `AuditRecordBin` (32-byte
  flat struct) and `AuditKind::label()` are added to `fjell-audit-format`.
  `auditd` emits one JSON Lines record per kernel event on startup and on
  each IPC signal.

- **RFC 021 — cap-broker real policy evaluation**: replaced the tag-byte stub
  with a three-pass evaluator (explicit Deny → explicit Allow → default Deny)
  matching RFC requirements BROKER-001 through BROKER-008.  `ResourceClass`,
  `PolicyKind`, `PolicyResult`, and `PolicyRule` are now proper types.
  Granted capabilities are lease-bound via `sys_lease_create`.  The IPC
  protocol is extended to carry `(requester_id, resource_class, requested_rights)`
  as three IPC words.  `sys_ipc_recv_msg` added to `fjell-syscall` to return
  all five values (label + 4 words).

- **RFC 017 — DmaAlloc capability gate**: `sys_dma_alloc` now requires the
  caller to hold `CapKind::DmaAlloc` (CSpace slot 2).  Granted to
  `fjell-storaged` and `fjell-driver-virtio-blk` at spawn time.  `release_task`
  now also frees (returns to allocator) the physical DMA frame after zeroing it,
  preventing frame leaks on task exit.

- **RFC 016 confirmed complete**: `sys_mmio_map` cap-gate, bounds-check against
  `MmioRegionTable`, and defense-in-depth RAM exclusion were already fully
  implemented in v0.0.11/v0.0.12; this release documents the confirmation.

### Changed

- `sys_dma_alloc` ABI: `(size_bytes)` → `(dma_cap_handle, size_bytes)`.
  Callers must pass the `CapKind::DmaAlloc` handle as `a0`; size moves to `a1`.
- `sys_audit_drain` ABI: `(buf_ptr, buf_len)` → `(buf_va, buf_cap, cap_handle)`;
  returns `(status, n_records, n_dropped)`.

## [0.0.12] - 2026-05-14

### Added
- RFC 019: `storaged` IPC service separation — virtio-blk block I/O is now
  owned by the `storaged` service; `fjell-init` communicates with it via
  structured IPC (WRITE_BEGIN / WRITE_CHUNK×16 / WRITE_COMMIT protocol).
- M7 smoke test (`TEST:M7:PASS`): verifies end-to-end IPC block writes,
  store format operations, and boot-slot confirmation simulation.

### Fixed
- `wait_storaged_ready`: declared `a2`–`a5` as asm clobbers so the compiler
  does not cache the READY constant in `a2` across the `ecall` (which
  `deliver()` always overwrites with `sender_badge = 0`).
- `ipc_call` / `wait_storaged_ready` / storaged IPC wrappers: declared `a7`
  as an asm clobber for all `"li a7, N", "ecall"` blocks.  Without this, the
  compiler allocated `a7` as a live loop variable (e.g. the chunk pointer in
  `storaged_write`); the kernel restores `a7 = ecall_nr` from the trapframe on
  wake, corrupting the variable.
- Removed duplicate `spawn(ImageId::STORAGED)` call that created a second
  storaged instance competing on endpoint 1.
- `sys_ipc_reply`: now reads the reply label from `a1` (was incorrectly
  reading from `a0 = 0`) and copies reply words to the caller's trapframe.

---

## [Unreleased]

---

## [0.0.11] — 2026-05-12 — M7.1 Security Hardening Round 2 (RFC 014, 015, 018, 022)

Third batch of architect-review-driven fixes.  Implements RFC 014, 015, 018, 022.
RFCs 016, 017, 019–021, 023 are specified and accepted; implementation deferred to
M8 or requires service separation (RFC 019) as prerequisite.

### Fixed / Added

- **RFC 014** (`trap/syscall.rs`): Replaced `caller_has_cap(kind)` with
  `require_cap(kind, rights)` — validates CapKind, CapRights, AND lease liveness.
  Added missing capability gates: `sys_task_status` now requires
  `TaskControl | INSPECT`; `sys_lease_revoke` and `sys_lease_inspect` now require
  `LeaseAdmin`.  All 6 task/lease syscalls are gated (RFC 004 gated only 3).

- **RFC 015** (`cap/syscall.rs`): Lease validation wired into all capability check
  paths.  `check_right()` (IPC send/recv/call) calls `cap.check_lease()`.
  `sys_cap_copy` and `sys_cap_mint` validate source cap lease before derivation.
  `sys_cap_inspect` validates lease before returning metadata.  Revoked caps now
  fail all IPC and cap operations.

- **RFC 018** (`link.ld`, `main.rs`): W^X three-region kernel permissions achieved.
  Linker script adds `ALIGN(4096)` between .text / .rodata / .data / .bss and
  includes RISC-V orphan sections (.srodata, .sdata, .sbss, .got).  The map loop
  now uses three regions: `.text=R|X`, `.rodata=R`, `.data/.bss/stack=R|W`.
  Section starts are page-aligned; no page straddles two permission regions.
  Verified: `__rodata_start=0x80005000`, `__data_start=0x8000c000` (both page-aligned).

- **RFC 022** (`trap/syscall.rs`): `sys_task_start` validates `entry_pc` and
  `stack_top` against user address range (`[0x1000, RAM_BASE)` and `[0x2000, RAM_BASE)`
  respectively).  Kernel addresses are rejected with `InvalidCap`.

### Accepted / Deferred

RFC 016 (MmioRegion cap), RFC 017 (DmaRegion cap), RFC 019 (try_recv + service loop),
RFC 020 (audit drain), RFC 021 (cap-broker policy), RFC 023 (BCB mirror tests) — all
specified in `rfcs/`; implementation targets M8 or requires RFC 019 as prerequisite.

---

## [0.0.10] — 2026-05-12 — M7.1 hardening (RFC 006, 007, 009, 013)

Second batch of architect-review-driven fixes.  Implements RFC 006, 007, 009, 013.
Remaining deferred RFCs: 011 (service separation), 012 (real crypto) — both require
M8 preemptive scheduler as prerequisite.

### Added / Changed

- **RFC 006** (`fjell-cap/src/slot.rs`, `fjell-kernel/src/lease/mod.rs`):
  `LeaseBinding { lease_id, epoch_at_issue }` added to `Capability` (field `lease:
  Option<LeaseBinding>`).  `Capability::check_lease(&dyn LeaseChecker)` validates
  lease liveness.  `LeaseChecker` trait defined in `fjell-cap`; `LeaseTable` implements
  it in the kernel.  All capability constructors (`install_root`, `install_raw`,
  `derive`, bootstrap literals) set `lease: None` for unbound caps.  Infrastructure is
  in place; lease-bound delegation used by M8 cap-broker.

- **RFC 007** (`fjell-kernel/src/main.rs`, `trap/syscall.rs`): Replaced singleton
  `DMA_BUF` static with a per-task DMA VA bump allocator at `0x60000000+` (VPN[2]=1).
  `sys_dma_alloc` allocates frames from the frame allocator, maps them at
  `DMA_VA_NEXT` in the calling task's page table, and returns `(user_va, device_pa)`.
  VPN[2]=1 is task-local (not shared via `clone_kernel_half`), resolving the
  `AlreadyMapped` conflict from the M6 static-buffer approach.

- **RFC 009** (`crates/fjell-kernel/link.ld`, `main.rs`): W^X kernel page permissions.
  Linker script exports `__text_start`, `__text_end`, `__rodata_start`, `__rodata_end`.
  Kernel identity map now uses a two-region split:
  - `.text` (RAM_BASE .. __text_end): **R | X** — execute, not writable
  - everything else (.rodata / .data / .bss / stack): **R | W** — read-write, not executable
  No page is simultaneously writable and executable.  Full three-region split
  (.rodata = R only) deferred: requires confirming no writable statics in .rodata.

- **RFC 013** (`docs/src/adr/`): Created ADR 0006–0010 documenting all M6/M7 design
  decisions, workarounds, and deprecation plans:
  - ADR 0006: User-space driver model and MMIO/DMA capability boundary
  - ADR 0007: Persistent append-only store and recovery model
  - ADR 0008: Verified immutable rootfs and signed artifact model
  - ADR 0009: A/B boot-control and health-based confirmation model
  - ADR 0010: Inline init smoke workaround and service separation deprecation plan

---

## [0.0.9] — 2026-05-12 — M7.1 Security & Architecture Hardening

Implements RFC 004, 005, 008, 010 in response to architect review of v0.0.8.
RFCs 006, 007, 009, 011–013 are specified and accepted; implementation deferred
to M7.1 hardening sprint or M8.

### Fixed / Added

- **RFC 004** (`fjell-cap`, kernel): Added `CapKind::TaskCreate`, `CapKind::TaskControl`,
  `CapKind::LeaseAdmin`.  `sys_task_spawn` now requires `TaskCreate`; `sys_task_start`
  requires `TaskControl`; `sys_lease_create/revoke/inspect` require `LeaseAdmin`.
  init task receives all three as bootstrap capabilities at slot 28/29/30 in its CSpace.
  `CSpace::slots()` and `CSpace::install_raw()` added for kernel-internal use.

- **RFC 005** (`kernel/trap/syscall.rs`): `sys_mmio_map` rejects any request whose
  physical range overlaps kernel RAM (`RAM_BASE..RAM_END`).  Prevents user-space from
  mapping kernel text, data, or stack with R|W|U.

- **RFC 008** (`fjell-upgrade-format`, `fjell-store-format`): `BootControlBlock` and
  `StoreSuperblock` now have `seal()` (compute and store CRC32) and updated `is_valid()`
  (checks magic AND CRC32).  `fjell-init` calls `seal()` before writing BCB and
  superblocks to disk.  CRC32 uses ISO 3309 / Castagnoli (0xEDB88320) in `no_std`.
  Added 2 regression tests (seal + corrupt byte detection).

- **RFC 010** (`kernel/trap/syscall.rs`, `fjell-syscall`): `sys_task_spawn` now returns
  `handle = index | (generation << 16)`.  `sys_task_start` and `sys_task_status` decode
  the generation from the handle and look up the task with generation check, preventing
  stale handle reuse.

### Accepted / Deferred

RFC 006 (LeaseBinding in Capability), RFC 007 (per-task DMA), RFC 009 (W^X kernel
permissions), RFC 011 (service separation), RFC 012 (real digest verification),
RFC 013 (ADR 0006–0010) are fully specified in `rfcs/` with implementation deferred.

---

## [0.0.8] — 2026-05-12 — RFC bugfix release

Implements RFC 001, RFC 002, RFC 003 identified during M7 self-review.

### Fixed

- **RFC 001** (`trap/entry.rs`): t5 (x30) and t6 (x31) saved with wrong values at
  trap entry. `gpr[30]` received `user_sp`; `gpr[31]` received `scratch_addr`.
  Added `TRAP_SCRATCH[3]` (4th slot) to save the true user t6 immediately after the
  `csrrw t6, sscratch, t6` swap. `gpr[30]` is now saved directly after the TrapFrame
  pointer is loaded, while x30 is still unmodified. Both caller-saved registers are
  now faithfully preserved across ecalls.
  Removed 5 redundant `sys_mmio_map` re-read workaround calls from `fjell-init`
  that were masking the corruption.

- **RFC 002** (`fjell-upgrade-format/src/lib.rs`): `BootControlBlock::new()`
  initialised `slot_b` with `SlotInfo::bootable(generation)`. Slot B is unprovisioned
  on a fresh disk and must be `SlotInfo::empty()`. Confirmed already correct in current
  code; added 3 regression unit tests to prevent recurrence.

- **RFC 003** (`fjell-kernel/Cargo.toml`): `version = "0.0.3"` hard-pinned while
  workspace was at 0.0.7. Changed to `version.workspace = true`; kernel version now
  correctly tracks workspace in build output and `cargo metadata`.

---


## [0.0.7] — 2026-05-12 — M7: Verified Immutable System / Snapshot / Complete Rollback Foundation

### Added
- `fjell-verify-format`: `DevSignature`, `TrustAnchor`, `SignedObject`, `ObjectKind`,
  `VerificationResult`, `BootEvidence`, `ReleaseManifest`, `RootfsManifest`,
  `PolicyBundle`; development-grade Ed25519 placeholder with hardcoded `DevSignature::VALID`
- `fjell-rootfs-format`: `ServiceImageRef`, `RootfsNamespace`, `RootfsStatus`
- `fjell-snapshot-format`: `SnapshotId`, `SnapshotReason`, `SnapshotDigest`,
  `SystemSnapshot`; reasons: Boot, PreUpgrade, PostConfirmation, Rollback, Periodic
- `fjell-verifyd`, `fjell-rootfsd`, `fjell-snapshotd`: stubs (verification and
  snapshot logic driven inline by fjell-init for M7 smoke test)
- `ImageId::VERIFYD` (14), `ImageId::ROOTFSD` (15), `ImageId::SNAPSHOTD` (16)
- fjell-init M7 scenario:
  - Boot evidence loading (`BootEvidence::for_slot`, `TrustAnchor::DEV.is_valid()`)
  - Release manifest, rootfs manifest, policy bundle signature verification
    (`SignedObject::verify_dev()`)
  - Immutable rootfs namespace (`RootfsNamespace::empty() + add()`)
  - Pre-upgrade system snapshot (`SystemSnapshot::new(SnapshotReason::PreUpgrade)`)
  - Upgrade staging with verified bundle, slot marking, candidate set
  - Candidate boot simulation → health check → `slot confirmed after health`
  - Post-confirmation snapshot (`SnapshotReason::PostConfirmation`)
  - Semantic state export: `[STATE][Ok] Verified boot status`, `Immutable rootfs`,
    `System snapshot`, `[EVENT][Normal][Ok] Slot confirmed after health`
  - Negative test: `ReleaseManifest::invalid_dev` → signature rejected
  - Health failure rollback: `rollback selected as expected` + rollback snapshot
- `MAX_TASKS` increased from 16 to 32 to accommodate M4–M7 services (17 tasks)

### Fixed
- Task table overflow: M7 requires 17+ tasks; `MAX_TASKS=16` caused spawn failure
  at `fjell-rootfsd`; increased to 32 in `tcb.rs` and `scheduler.rs`

---

## [0.0.6] — 2026-05-12 — M6: Device / Persistent State / Immutable Upgrade Foundation

### Added
- `fjell-device-format`: `DeviceDescriptor`, `DeviceKind`, `MmioRegionDescriptor`,
  `DeviceState`; hardcoded `QEMU_VIRTIO_BLK` descriptor
- `fjell-block-format`: `BlockDeviceInfo`, `BlockError`
- `fjell-store-format`: `StoreSuperblock` (magic `FJSTORE\0`), `RecordHeader`
  (magic `0x464A4C52`), `RecordKind`, `LBA_*` layout constants
- `fjell-upgrade-format`: `BootControlBlock` (magic `FJBOOT\0\0`), `SlotInfo`,
  `SlotState`, `SlotId`, `UpgradeState`
- `fjell-devmgr`: calls `sys_platform_info_get`, emits "M6: virtio-mmio blk discovered"
- `fjell-driver-virtio-blk`: stub (virtio I/O done inline by fjell-init in M6)
- `fjell-storaged`, `fjell-bootctl`, `fjell-upgraded`, `fjell-powerd`: stubs
- Kernel syscalls: `sys_platform_info_get` (80), `sys_mmio_map` (90),
  `sys_dma_alloc` (110); all wired in dispatch table
- `sys_platform_info_get`: scans virtio-mmio slots 0..7 in reverse, returns base
  PA of first slot with magic=0x74726976 and device_id=2
- `sys_mmio_map`: calls `remap_page` to add R|W|U to an existing kernel-mode
  mapping; uses identity map (user_va == phys_addr)
- `sys_dma_alloc`: returns pre-allocated static `DMA_BUF` at user VA 0x20000000
- `DMA_BUF`: `#[repr(align(4096))]` 16 KiB static DMA buffer; mapped with R|W|U
  into init's page table during task creation (4 pages at 0x20000000-0x20004000)
- `remap_page`: new page-table primitive that overwrites existing PTEs
  (needed to upgrade R|W kernel-only mappings to R|W|U user-accessible)
- Kernel identity-maps all 8 virtio-mmio slots (0x10001000-0x10008000) with R|W
  in all user page tables so `sys_platform_info_get` can scan from kernel mode
- fjell-init M6: inline virtio-blk driver (legacy v1 init, split virtqueue,
  block write), storaged simulation, bootctl A/B mirror write, upgrade staging
- `cargo xtask build-services` and `cargo xtask qemu-test m6` with disk image
  creation (`qemu-img create -f raw fjell-disk.img 16M`)

### Fixed
- `sys_platform_info_get`: added VPN[2]=0 virtio-mmio mapping to ALL user page
  tables (kernel-mode mapping in spawn.rs + kmain init) so scan works from
  within the user task's satp context
- `blk_write_sector` `base` re-read after ecalls: local variable `base` holding
  the MMIO VA was potentially corrupted by t5/t6 register-save bug across ecalls;
  fixed by calling `sys_mmio_map` again to reload `base` from the syscall
- virtio-blk QueueAlign: changed from 4096 to 512 so desc+avail+used fit in
  one DMA page; with QueueAlign=4096 the used ring was at page+4096 which was
  beyond the mapped DMA region
- virtio-blk avail ring offset: was 0x200 (wrong ALIGN interpretation); corrected
  to 0x080 (= N*16 = 8*16 = 128, immediately after descriptor table)
- virtio-blk poll: increased from 500_000 to 5_000_000 iterations to handle
  QEMU subprocess execution slower than interactive mode
- `sys_dma_alloc`: replaced dynamic frame allocation + user-VA mapping with
  pre-mapped static DMA_BUF to avoid `AlreadyMapped` errors when kernel-shared
  L1 tables prevented adding U bit via `map_page`

---

## [0.0.5] — 2026-05-12 — M5: Semantic Operations Plane

### Added
- `fjell-semantic-format`: `IntentNode`, `StateNode`, `EventNode`,
  `SemanticEnvelope`, `TextToken`, `BoundedText`, `FixedVec<T,N>`,
  `ActionRequest`, `ActionResult`, `StreamFilter`, `ExportFormat`;
  invariant validators (`validate_intent`, `validate_state`);
  4 unit tests (valid intent, empty-action error, failed-state error,
  BoundedText roundtrip)
- `fjell-semantic-stream` (new service): publish/subscribe/validate/
  action dispatch; memory-backed intent/state/event rings
- `fjell-proxy-text` (full implementation): text rendering for
  `[STATE]`, `[EVENT]`, `[INTENT]` nodes; `SmokeScenario` auto-selects
  first action; no pixel/color/layout metadata required
- `fjell-init` (M5): full M5 smoke scenario — starts semantic-stream
  and proxy-text; publishes ServiceGraph, ConfigValidated, AuditSummary,
  CapabilityGranted, sample IntentNode; drives action dispatch;
  exports system state as plain text; emits `TEST:M5:PASS`
- `fjell-abi`: `ImageId::SEMANTIC_STREAM = 6`, `ImageId::PROXY_TEXT = 7`
- Kernel image table: `fjell-semantic-stream.bin`, `fjell-proxy-text.bin`
- `cargo xtask qemu-test m5` smoke gate

### Fixed
- Service linker script: stack start pinned to fixed VA `0x80000`
  (`__stack_top = 0x90000`) — previously `__stack_top` varied with
  binary size, causing `SERVICE_STACK_TOP` mismatches after the init
  binary grew to include semantic-format and proxy-text code
- `spawn.rs` + kmain: map all 16 stack pages (64 KiB) instead of one —
  init uses ~32 KB of stack for M5 scenario; single-page mapping caused
  `StorePageFault` at `0x87ec8`
- `Cargo.toml workspace.default-members`: trailing `]` was accidentally
  dropped by a prior edit, corrupting the TOML array

---

## [0.0.4] — 2026-05-12 — M4: Service Plane Bootstrap

### Added
- `fjell-abi`: `LeaseId`, `LeaseEpoch`, `BootInfo`, `ImageId`, `ServiceId`,
  `TaskLifecycle`; M4 syscall numbers (TaskSpawn=40, TaskStart=41,
  TaskStatus=42, TaskKill=43, LeaseCreate=50, LeaseRevoke=51,
  LeaseInspect=52, AuditDrain=60)
- `fjell-syscall`: complete user-space wrappers for all M3+M4 syscalls;
  service runtime module (`rt.rs`) with `_start` + `#[panic_handler]`
- `fjell-service-api`: IPC protocol tag constants for service lifecycle,
  config, cap-broker, audit, and service-manager protocols
- `fjell-cap-broker`: policy-evaluation service; static rule table;
  lease-create/revoke/inspect smoke demonstration
- `fjell-configd`: bootstrap manifest validation; config-get endpoint
- `fjell-auditd`: kernel audit ring drain; JSON Lines emission
- `fjell-service-manager`: service graph bootstrap; sample-service spawn
- `fjell-sample-service`: minimal ready/heartbeat/shutdown service
- `fjell-init` (full implementation): orchestrates configd → cap-broker →
  auditd → service-manager → sample-service; emits audit JSON Lines;
  prints `TEST:M4:PASS`
- `fjell-kernel/src/lease/`: `LeaseTable` with `create`/`revoke`/
  `check_active`/`current_epoch`; MAX_LEASES=32
- `fjell-kernel/src/task/image.rs`: embedded flat-binary service image table
  (`include_bytes!` of six release binaries); `SERVICE_BASE_VA=0x40000`,
  `SERVICE_STACK_TOP=0x51000`
- `fjell-kernel/src/task/spawn.rs`: flat-binary task spawner; multi-page
  text loading; UART + kernel-half mapping in spawned address space
- Kernel M4 syscall handlers: `sys_task_spawn`, `sys_task_start`,
  `sys_task_status`, `sys_lease_create`, `sys_lease_revoke`,
  `sys_lease_inspect`, `sys_audit_drain`
- `sys_debug_write` updated: directly writes a user byte to UART MMIO
- `FRAME_ALLOC` static: `FrameAllocator` moved from kmain stack to BSS static
- `KERNEL_ROOT_PFN` static: kernel root page-table PFN stored for trap-time
  `sys_task_spawn` access
- `FA_RAW_PTR` static: frame-allocator raw pointer stored for `sys_task_spawn`
- TRAP_SCRATCH expanded to `[usize; 3]`: slot [2] holds user sp temp save
- `Scheduler::suspend_current()` used in `block()` (M3 fix carried forward)
- `TaskTable::next_free_index()` helper

### Fixed
- `first_entry`: changed `in(reg) tf` to `in("a0") tf` — when the compiler
  chose `s3` (x19) as the base register, `ld x19, 19*8(s3)` would overwrite
  the base with `TrapFrame.gpr[19]` (often 0), faulting on the next load
- `FrameAllocator` local-to-kmain: trap handler resets sp to `__stack_top`
  on every entry, overwriting kmain's stack frame including `fa_cell`; fix:
  moved to `static FRAME_ALLOC` in BSS
- `SERVICE_STACK_TOP`: was `0x50000`, linker script produces `0x51000`
  (`__stack_bottom=0x41000 + 64K = 0x51000`); incorrect value caused
  `StorePageFault` on first service function call
- `trap/entry.rs`: `sd sp, 2*8(t6)` saved kernel sp (already loaded by
  `ld sp, 0(t6)`) instead of user sp; fixed by: (a) storing user sp to
  `SCRATCH[2]` before loading kernel sp, (b) restoring sscratch to
  SCRATCH_ADDR, (c) loading user sp back from `SCRATCH[2]` into the
  TrapFrame; without this, all service stacks were silently corrupted after
  their first ecall
- `check_smoke_pass` / `task_label`: updated for M4 single-task model (init
  exit = PASS criterion; task labels: init/configd/cap-broker/auditd/…)
- `task/spawn.rs`: `alloc_frame` returns `Result`, not `Option`; replaced
  `ok_or` with `map_err` in all spawn allocation paths
- `task/spawn.rs`: `next_free_index()` returns `Option`; kept `ok_or` there
- `TaskState`: added `#[derive(PartialEq)]` for state comparison in
  `sys_task_status` and `sys_task_start`
- `BlockReason`, `FaultInfo`: added `#[derive(PartialEq)]` for
  `TaskState::PartialEq` to work transitively

---

## [0.0.3] — 2026-05-11 — M3: IPC and Capability

### Added
- `fjell-abi`: expanded `SyscallNumber` (CapCopy/Mint/Delete/Revoke/Inspect,
  IpcSend/Recv/Call/Reply) and `SysError` (InvalidCap, WrongType, SlotEmpty,
  SlotOccupied, RightsExceed, CapTransferDenied, WouldBlock, QueueFull,
  MsgTooLong, Canceled, NoMemory, AlreadyMapped, NotMapped, InvalidAddress)
- `fjell-cap`: `CapHandle` (generation-tagged), `CapRights` bitmask,
  `CapKind`, `Capability`, `CapSlot`, `CSpace` with copy/mint/delete/revoke/
  inspect; derivation tree; host unit tests
- `fjell-ipc`: synchronous rendezvous `Endpoint` (sendq + recvq),
  `PendingMessage` snapshot, `ReplyEdge` (one-shot reply), `MessageTag`;
  host unit tests for IPC-A/B/C invariants
- `fjell-kernel/src/cap/`: `table.rs` (EndpointTable, CapTable, ReplySlot),
  `syscall.rs` (all M3 cap + IPC syscall handlers)
- `fjell-kernel/src/audit/ring.rs`: M3 audit kinds
  (CapCopy/Mint/Delete/Revoke, IpcSend/Recv/Call/Reply/Denied)
- `fjell-kernel/src/task/user_image.rs`: `USER_TASK_C` (denied task — no
  capability, ipc_call → denied → exit(0))
- `main.rs`: 3-task M3 smoke scenario (client, server, denied) with M3 boot
  messages; per-task satp switching; UART mapped in each user page table
- `Scheduler::suspend_current()`: clears current without dequeuing next task
- M3 kernel log messages: "client: call sent", "server: request received",
  "server: reply sent", "client: reply received", "denied: ipc denied as
  expected", "audit: capability.denied", "audit: ipc.call", "audit: ipc.reply"
- Smoke test marker: `TEST:M3:PASS` (all three tasks exit(0) via IPC flow)
- `cargo xtask qemu-test m3` smoke gate

### Fixed
- `fjell-cap/src/cspace.rs`: `gen` renamed to `slot_gen` (Rust 2024 reserved
  keyword)
- `fjell-ipc/src/endpoint.rs`: removed duplicate `pub use` re-export of
  `IPC_WORDS`/`IPC_CAPS` that caused E0252 in Rust 2024
- `fjell-ipc/src/endpoint.rs`: added `#[derive(Debug)]` to `SendResult` and
  `RecvResult` (required by `unwrap_err` in tests)
- `fjell-cap/src/slot.rs`: added `#[derive(PartialEq)]` to `Capability`
  (required by `assert_eq!` in tests)
- `task/user_image.rs`: removed stray `}` after `USER_TASK_A` slice literal
- `cap/table.rs`: removed unused imports `CSPACE_SLOTS`, `CapHandle`,
  `CapKind`, `CapRights`
- `main.rs (m_mode_setup)`: added PMP entry 0 (NAPOT, RWX, all memory) so
  S-mode can access RAM after `mret` — without this every S-mode fetch faults
- `main.rs (kmain)`: DTB reservation now ignores `AlreadyReserved` (DTB may
  overlap kernel image on QEMU virt)
- `main.rs (kmain)`: identity map extended to `__stack_top` (was only
  `boot_end = BSS+2MiB`; stack is BSS+4MiB, causing store_page_fault on
  first kernel stack write after enabling Sv39)
- `mm/page_table.rs (clone_kernel_half)`: now copies root entry 2
  (VA 0x80000000, kernel code/data/stack) instead of entries 256..511 (which
  are all empty for this identity-mapped kernel)
- `trap/dispatch.rs (schedule_next)`: `write_sscratch` called on every
  schedule to restore the sscratch → TRAP_SCRATCH pointer (the trap entry
  `csrrw t6, sscratch, t6` leaves sscratch = user's t6; without this restore
  the second trap would fault at `ld sp, 0(t6)` with t6=0)
- `cap/syscall.rs (block)`: replaced `sched.on_exit()` (which internally
  dequeues and discards the next task) with `sched.suspend_current()` — fixes
  silent task loss when caller blocks on IPC
- `task/scheduler.rs (on_exit/on_fault/on_yield)`: removed internal
  `choose_next()` calls; callers in `schedule_next` now always call
  `choose_next()` exactly once, preventing double-dequeue and task loss
- `cap/syscall.rs (sys_ipc_recv)`: for `is_call=true`, sender is no longer
  prematurely woken on message delivery (it must wait for explicit `ipc_reply`)
- `cap/syscall.rs (sys_ipc_reply)`: caller state is checked before waking;
  Exited/Faulted callers are silently skipped to prevent zombie resurrection


---

## [v0.0.2 — M2: Memory and Task Isolation]

### Added
- M-mode shim → S-mode kernel handoff; `kmain(hart_id, dtb_pa)` signature
- `arch/riscv64/`: CSR helpers, `satp` write, `sfence.vma`, timer (CLINT), PTE types
- `platform/`: `PlatformInfo` with hardcoded QEMU virt constants; DTB pointer forwarding
- `mm/`: `BootAllocator`, bitmap `FrameAllocator`, `FrameOwner`, `MmError`
- `mm/`: `PhysAddr`, `VirtAddr`, `PhysFrame` address types
- `mm/`: Sv39 `PageTable`, `map_page`, `unmap_page`, `translate`; kernel shared map
- `mm/`: `AddressSpace`, `VmRegion`, `VmPerms`, `VmRegionKind`
- `task/`: `TaskId`, `TaskState`, `FaultInfo`, `TrapFrame`, `KernelContext`, `Task`,
           `TaskTable`, `TaskAccounting`
- `task/scheduler.rs`: fixed-priority round-robin `ReadyQueue`, idle task
- `task/user_image.rs`: two embedded RISC-V user tasks (task-a: yield/exit,
                        task-b: yield/fault)
- `trap/`: assembly trap entry (`stvec` direct mode), Rust dispatch, syscall handler,
           fault containment → `TaskState::Faulted`
- `audit/ring.rs`: fixed-capacity append-only `AuditRing`
- `fjell-abi`: split into `lib.rs`, `syscall.rs`, `error.rs`, `task.rs`
- `fjell-tools`: `qemu.rs`, `smoke.rs` submodules; QEMU exit hint (Ctrl-A X)
- QEMU smoke test: `TEST:M2:PASS` marker

### Fixed (v0.0.1 carry-over)
- `publish = false` added to all crate `Cargo.toml` files
- All documentation and comments are now English only
- `cargo xtask qemu` now prints "Press Ctrl-A then X to exit QEMU" on launch
- `code-model=medany` → `code-model=medium` (LLVM name)
- Linker script supplied via `build.rs` (`CARGO_MANIFEST_DIR`-relative absolute path)
- `default-members` excludes `fjell-kernel`; host and kernel builds are cleanly separated
- `#[no_mangle]` → `#[unsafe(no_mangle)]` (edition 2024)
- `static mut UART` → `UnsafeCell`-based console (edition 2024 `static_mut_refs`)

---

## [v0.0.1 — M1: Bootable Kernel]

### Added
- Cargo workspace: resolver = "2", Rust 2024 edition, 17 crate skeletons
- `link.ld`: kernel image at `0x8000_0000`, 4 MiB stack
- `boot.rs`: `_start` assembly — hart-0 selection, BSS zero-fill, stack pointer, `kmain`
- `uart.rs`: NS16550A UART driver (MMIO `0x1000_0000`), `fmt::Write`, CRLF on `\n`
- `console.rs`: `print!` / `println!` macros backed by `UnsafeCell<Uart>`
- `main.rs`: `kmain()` boot banner; panic handler writes to UART then spins
- `build.rs`: linker script via `cargo:rustc-link-arg` with absolute path
- `.cargo/config.toml`: `code-model=medium` for RISC-V target; `xtask` alias
- `crates/fjell-kernel/.cargo/config.toml`: default RISC-V target, QEMU runner
- `.github/workflows/ci.yml`: host check/test + kernel check/build + QEMU M1 smoke test
- `docs/`: full mdBook skeleton (design-philosophy, architecture, ADRs 0001–0005, …)
- `LICENSE` (Apache-2.0), `NOTICE`, `TERMS_OF_USE.md`, `README.md`, `ROADMAP.md`

---

*Releases are tagged once each milestone passes its acceptance criteria.*
