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
- **Resolution:** **OPEN**. Recorded per RFC-v0.22-001 §Scope item 5 as a
  record-only finding — v1.0 is not in view for this RFC, and a checklist
  executability audit is explicitly out of its scope. Not investigated or
  fixed here; whoever next executes the v1.0 release checklist for real
  must resolve this before Step 9 can run.

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
| E-012 release checklist Step 9 bundle path | v0.22-001 (recorded, not fixed) | OPEN |

At v0.22 update: 1 OPEN, 9 CLOSED, 2 ACCEPTED. The ACCEPTED items
(hardware boot, `cap_install` rights validation) are reflected in the v1.0
scope statement / RFC-v0.21.3-001; both are disclosed limitations, not
silent drift. E-012 is a v1.0-checklist-specific finding recorded per
RFC-v0.22-001 §Scope item 5; it does not block a v0 release and was not
investigated further, per that RFC's explicit non-goal.
