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

At v0.16.0 close: 0 OPEN, 8 CLOSED, 1 ACCEPTED. The one ACCEPTED item
(hardware boot) is reflected in the v1.0 scope statement and release
notes; it is a disclosed limitation, not silent drift.
