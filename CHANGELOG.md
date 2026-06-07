# Fjell OS — Changelog

All notable changes to this project are documented in this file.
Versions follow `MAJOR.MINOR.PATCH` semantics from v1.0.0 onward.

---

## [0.18.1] — Architect Stage-A conditions landed; v0.17/v0.18 RFC sets completed

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
