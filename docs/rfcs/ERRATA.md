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

> **Scope widened 2026-08-02 (RFC-v0.24-001 Pass 1).** This entry originally
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
| E-013 fjell-kernel has no host-testable `[lib]` target | RFC after v0.23.0 (recorded, not fixed) | ACCEPTED |

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
