# Fjell OS — v1.0 Readiness Matrix

*Governed by RFC-v0.10-007. Every cell must be DONE or DEFERRED
(with rationale) before the v1.0.0 tag. OPEN cells block the release.*

*Amended 2026-09-24 (RFC-0.33-002's review): **IN PROGRESS is a transitional
state, not a third verdict.** A row may stand at `IN PROGRESS → <version>` while
the work is scheduled, and **must** have become DONE or DEFERRED before the
v1.0.0 tag — the rule above is unchanged, and an in-progress cell at the tag is a
blocked release. Only the literal blocking marker reddens Gate 5, which is why
the distinction is written here rather than left to the tool.*

*Last updated: 0.33 (three inclusion rows added, RFC-0.33-002 D9; the rest last reviewed at v0.20.0)*

---

## Dimension 1 — Identity

| Item | RFC | Status |
|------|-----|--------|
| Identity statement adopted | RFC 061 §2 | **DONE** (v0.9.4) |
| Archetypes A1, A2, A3 defined | RFC 061 §3 | **DONE** (v0.9.4) |
| Non-goals explicitly listed | RFC 061 §3.4, §7 | **DONE** (v0.9.4) |
| Identity guide published (`docs/src/identity/`) | RFC-v0.10-006 | **DONE** (v0.9.4) |
| A second presentation modality, end to end — speech or braille output driven by the same Intent Stream as `proxy-text`, observed in a QEMU tier | RFC-0.33-002 D9; RFC-0.34-001 | **DONE** (0.34 line, unreleased: braille **cells on a serial console**, asserted by content in `semantic-braille`; `test-all` run `20260924-104654`. **Driven on no braille device and read by no braille reader** — QEMU `virt` has none; see the accessibility section of `v1-limitations.md`) |
| An input path, decided — a recorded decision (an ADR) for how a person operating through a proxy reaches the system without bypassing capability policy; a decision, not necessarily an implementation | RFC-0.33-002 D9, §C; ADR-v0.5-005 | **IN PROGRESS** → v1.x |
| Accessibility limitations, written — the section of `v1-limitations.md` saying what a person needing speech, braille or a simplified presentation cannot do today, kept true at each cut | RFC-0.33-002 D6, D9 | **IN PROGRESS** → v1.x |

*The inclusion rows still open — an input path, and the limitations section — are marked `IN PROGRESS → v1.x`, not the blocking status (the first, a second presentation, is done): the gate counts the literal blocking marker only, and a row that is honest about future work must not redden a release that never claimed it (RFC-0.33-002 D9). The 1.x series begins at the v1.0 tag, and these are its criteria. The header above says every cell must be DONE or DEFERRED before that tag; whether an in-progress cell may reach it is not decided here.*

## Dimension 2 — Surface / ABI

| Item | RFC | Status |
|------|-----|--------|
| Stable surface enumerated (S1–S9) | RFC-v0.10-002 §2 | **DONE** (v0.9.4) |
| Stability tiers per item | RFC-v0.10-002 §3 | **DONE** (v0.9.4) |
| `ci-abi-check` gate live | RFC-v0.10-002 §6 | **DONE** (v0.9.4) |
| ABI snapshot committed (`tests/abi/snapshot.json`) | RFC-v0.10-002 | **DONE** (v0.9.4) |
| SDK_API_REV bound to surface | RFC v0.9-001 | **DONE** (v0.9.0) |

## Dimension 3 — Trust Spine

| Item | RFC | Status |
|------|-----|--------|
| HardwareTrustProvider interface | RFC v0.3-001 | **DONE** (v0.3.0) |
| Keyring and KeyEpoch model | RFC v0.3-002 | **DONE** (v0.3.0) |
| Anti-rollback metadata | RFC v0.3-003 | **DONE** (v0.3.0) |
| Attestation profile v2 | RFC v0.3-004 | **DONE** (v0.3.0) |
| Real Ed25519 signature backend | RFC-v0.11-002 | **DONE** (v0.11.0) |
| Bundle signing pipeline | RFC-v0.11-003 | **DONE** (v0.11.0) |
| Key rotation and revocation records | RFC-v0.11-004 | **DONE** (v0.11.0) |
| Replay cache and attestation freshness | RFC-v0.11-005 | **DONE** (v0.11.0) |

## Dimension 4 — Quality / Verification

| Item | RFC | Status |
|------|-----|--------|
| Host test suite (≥ 487 tests; 566 confirmed at v0.20.0) | — | **DONE** (v0.9.4) |
| Proptest harness (≥ 10 properties; 14 confirmed at v0.20.0) | RFC v0.6-001 | **DONE** (v0.6.0) |
| Verus formal proofs — capability + lease (`release_required=true`), boot-control (pilot, `release_required=false`); 20 obligations machine-checked; callsite-audit Gate 11 blocking | RFC-v0.17-002…006, RFC-v0.18-001 | **DONE** (v0.18.1 proofs; v0.20.0 gate) |
| Reproducible build gate (SHA-256 baseline, two-build mode) | RFC-v0.16-005 H-04 | **DONE** (v0.18.2) |
| Every byte decoder a host fuzz crate can reach is fuzzed | RFC v0.6-003, RFC-0.32-001 | **DONE** (0.32; corrected and re-run 2026-09-25) — six decoders, **all six fuzzed on CI for 301 s each in dispatch run `36064695680`** (2026-09-25, at the tip that deleted `fjell-dtb-derive`): `semantic_record_parse` 149,369,918 runs, `revocation_record_parse` 202,879,453, `audit_record_parse` 293,020,079, `cap_manifest_parse` 9,968,854, `dtb_validate` 17,537,434, `semantic_envelope_wire_decode` 98,808,034 — each read from its own job log, not from the run's conclusion. Decoders a host fuzz crate cannot reach are named in E-043's closure, not counted. *(This row said "six … run `34976532420`", and was wrong: that run fuzzed a different six, and there were seven decoders from RFC-0.32-002 until RFC-0.33-005 deleted `fjell-dtb-derive`; the run cited before this one, `35089305545`, was the 0.32.0 cut's, when `dtb_validate` had not yet been changed by RFC-0.33-005.)* *(This row read "Fuzz targets (≥ 4)" until 2026-09-15. Eight target files satisfied it while five had never compiled and one fuzzed anything; a count of targets was the defect.)* |
| Unsafe-audit gate, zero gaps | RFC v0.6-004, RFC 060 | **DONE** (v0.8.24) |
| QEMU smoke tier (≥ 4 profiles) | — | **DONE** (v0.8.0) |
| QEMU negative tier (≥ 9 categories, fail-closed, 27 real markers) | RFC-v0.7.1-002 | **DONE** (v0.20.0 — real from v0.19.0; fail-closed gate v0.20.0) |
| Reproducible-build gate | RFC-v0.10-003 | **DONE** (v0.9.4) |
| ABI snapshot gate | RFC-v0.10-002 | **DONE** (v0.9.4) |
| Benchmark baseline + regression gate | RFC-v0.10-004 | **DONE** (v1.0.0) |
| MMIO ordering audit | RFC-v0.12-004 | **DONE** (v0.12.0) |

## Dimension 5 — Operability

| Item | RFC | Status |
|------|-----|--------|
| Reference QEMU fleet demo | RFC-v0.10-005 | **DONE** (v1.0.0) |
| Trust Report (six sections) | RFC 061 §6 | **DONE** (v0.9.4) |
| Fleet partition reconciliation | RFC-v0.13-002 | **DONE** (v0.13.0) |
| Key compromise recovery playbook | RFC-v0.13-003 | **DONE** (v0.13.0) |
| Bulk re-attestation workflow | RFC-v0.13-004 | **DONE** (v0.13.0) |
| Staged rollout failure handling | RFC-v0.13-004 | **DONE** (v0.13.0) |
| Disaster recovery patterns | RFC-v0.13-005 | **DONE** (v0.13.0) |

## Dimension 6 — Reach / Deployment

| Item | RFC | Status |
|------|-----|--------|
| QEMU `virt` profile supported | — | **DONE** (v0.1.0) |
| DTB and boot handoff validation | RFC-v0.12-003 | **DONE** (v0.12.0) |
| First real RISC-V board profile | RFC-v0.12-002 | **DONE** (v0.12.0) |
| Field operations deployment guide | RFC-v0.12-005 | **DONE** (v0.12.0) |
| ARM64 second-platform | RFC 061 §P1 | **DEFERRED** — post-v1.0 (RFC 061 §5 P1) |

## Dimension 7 — Ecosystem / SDK

| Item | RFC | Status |
|------|-----|--------|
| `fjell-sdk` published | RFC v0.9-001 | **DONE** (v0.9.0) |
| CapManifest format | RFC v0.9-002 | **DONE** (v0.9.0) |
| Bundle format | RFC v0.9-004 | **DONE** (v0.9.0) |
| Dev-harness | RFC v0.9-005 | **DONE** (v0.9.0) |
| Typed catalog structs + cookbook | RFC-v0.14-003 | **DONE** (v0.14.0) |
| First external service (reference) | RFC-v0.14-002 | **DONE** (v0.14.0) |
| Bundle publishing flow + registry | RFC-v0.14-004 | **DONE** (v0.14.0) |
| Developer mode tooling | RFC-v0.14-005 | **DONE** (v0.14.0) |

## Dimension 8 — Governance and Process

| Item | RFC | Status |
|------|-----|--------|
| RFC lifecycle policy | RFC 000 | **DONE** (v0.1.0) |
| Unsafe charter | RFC v0.6-004 | **DONE** (v0.6.0) |
| Threat model finalized | RFC-v0.15-002 | **DONE** (v0.15.0) |
| Release checklist | RFC-v0.15-003 | **DONE** (v0.15.0) |
| Security advisory process | RFC-v0.15-003 | **DONE** (v0.15.0) |
| Operator recovery guide | RFC-v0.15-004 | **DONE** (v0.15.0) |
| v1.0 non-goals locked | RFC-v0.15-005 | **DONE** (v0.15.0) |
| LTS branch policy | — | **DEFERRED** — post-v1.0 |
| Contributor governance | — | **DEFERRED** — post-v1.0 |

---

## Summary at v0.9.4

| Status | Count |
|--------|-------|
| DONE | 42 |
| IN PROGRESS | 0 |
| DEFERRED | 3 |
| OPEN | 0 |

*Zero OPEN cells. Three IN PROGRESS items (RFC-0.33-002 D9's inclusion rows).*

*Corrected 2026-09-24: this line read "v1.0.0 released. Zero OPEN cells. Zero IN
PROGRESS items." **There is no v1.0.0 tag** — 96 tags exist, the most recent
`0.32.0` — and the owner's direction of 2026-07-30 is that v1.0 is explicitly not
in view. The sentence stated a release that has not happened, in the document that
defines what the release requires, and nothing read it. Found at RFC-0.33-002's
review, having predated that line.*

---

*CI gate: `cargo xtask readiness-check` counts OPEN cells and fails
if any are present. Maintained by [`tools/fjell-readiness-check/`](https://github.com/nabbisen/fjell-os/blob/main/tools/fjell-readiness-check)
(RFC-v0.10-007 §4 — tool lands in v0.10 cycle).*
