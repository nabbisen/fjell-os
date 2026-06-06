# Fjell OS — Threat Model v1.0

*Governed by RFC-v0.15-002. Authoritative for v1.0.0.*

---

## Adversary model

Every threat is keyed to an adversary capability, not a named actor.

| Code | Capability | Description |
|------|-----------|-------------|
| C-NET-PASSIVE | Read fleet network traffic | Passive on-path attacker |
| C-NET-ACTIVE | Read + modify + inject fleet traffic | Active on-path attacker |
| C-NODE-EXEC | Execute arbitrary code on one node | Full node compromise (post-exploit) |
| C-NODE-PHYS | Physical access to one node, once | Single-visit hardware adversary |
| C-NODE-PHYS-PERS | Physical access on demand | Persistent hardware adversary |
| C-OPER-MISTAKE | Operator error within authorised scope | Accidental misconfiguration |
| C-OPER-MAL | Malicious operator within authorised scope | Insider threat |
| C-SIGN-COMPR | Possession of a current signing key | Compromised release signing key |
| C-ANCHOR-COMPR | Possession of TrustAnchorRoot key | Highest-privilege compromise |
| C-SUPPLY | Modify the build supply chain | Software supply chain attack |

---

## In-scope threats (v1.0 commits to mitigating)

### T1 — Unauthorised capability acquisition

**Adversary:** C-NODE-EXEC  
**Defence:** Capability system (I1–I3); `require_cap` enforces handle + rights + kind matching. RFC 031, 040, RFC v0.9-002.  
**Residual:** A bug in `require_cap` itself. Mitigated by the unsafe-audit gate and the property test suite (RFC v0.6-001).

---

### T2 — Cap-broker policy bypass

**Adversary:** C-NODE-EXEC, C-OPER-MAL  
**Defence:** RFC 031 (cap-broker), RFC 040 (intent ledger). The broker enforces manifests; operator approval required for all grants.  
**Residual:** Typo in a manifest. Mitigated by `cargo xtask dev lint` (RFC v0.9-002) and the ABI snapshot gate (RFC-v0.10-002).

---

### T3 — Lease-expired authority replay

**Adversary:** C-NET-ACTIVE, C-NODE-EXEC  
**Defence:** I3 (lease epoch bound) + RFC-v0.11-005 (replay cache + nonce challenge). Expired leases are invalidated in the CSpace.  
**Residual:** Clock skew within configured tolerance window (24h default).

---

### T4 — MMIO ownership confusion

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 016 (MmioRegion), RFC 035, RFC 051. All MMIO access goes through the `MmioRegion` handle. RFC-v0.12-004 annotated all 23 MMIO sites.  
**Residual:** Unknown MMIO regions not yet imported into `devmgr`.

---

### T5 — DMA-based memory aliasing

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 017 (DMA capability), RFC 036, RFC 052. DMA grants are per-region and epoch-bound.  
**Residual:** Hardware IOMMU not present on v0.12 target; purely software enforcement.

---

### T6 — Unsafe-code regression

**Adversary:** C-SUPPLY  
**Defence:** I6 (`forbid(unsafe_code)` except at audited boundaries); unsafe-audit gate; RFC v0.6-004 + RFC 060.  
**Residual:** Unsoundness in the Rust standard library or LLVM.

---

### T7 — Trap-frame corruption

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 001 (trap handling), RFC 022 (trap-frame layout). Trap frames are kernel-only memory.  
**Residual:** Microarchitectural fault-injection requiring C-NODE-PHYS-PERS.

---

### T8 — Update rollback

**Adversary:** C-NODE-EXEC, C-NODE-PHYS  
**Defence:** RFC v0.3-003 (anti-rollback counter), RFC 002 (boot control).  
**Residual:** Anti-rollback counter is software; hardware OTP rollback counter not present on v0.12 target.

---

### T9 — Bundle tampering

**Adversary:** C-NET-ACTIVE, C-SUPPLY  
**Defence:** RFC v0.9-004 (content-addressed bundle), RFC-v0.11-003 (Ed25519 signature).  
**Residual:** A tampered bundle that also has a valid signature requires C-SIGN-COMPR.

---

### T10 — Signature forgery (algorithmic)

**Adversary:** C-NET-ACTIVE  
**Defence:** Ed25519 (RFC 8032) via `fjell-sig-ed25519` (RFC-v0.11-002). Ed25519 provides 128-bit security level.  
**Residual:** Out-of-scope OS3 (CRQC); see below.

---

### T11 — Signing key compromise

**Adversary:** C-SIGN-COMPR  
**Defence:** RFC-v0.11-004 (key rotation + revocation records), RFC-v0.13-003 (compromise playbook).  
**Residual:** Time window between compromise and revocation distribution. Bounded by replay cache window (24h).

---

### T12 — Attestation replay

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.11-005 (replay cache, nonce challenge). One nonce, one response; sliding window cache for async reports.  
**Residual:** Post-reboot cache emptiness. Mitigated by `verifier_boot_count` in challenge.

---

### T13 — Fleet partition exploitation

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.13-002 (partition FSM). Partitioned nodes refuse authority-class operations; reconcile manifest required.  
**Residual:** Coordinator itself being the partitioned node. Operator promotion required (RFC-v0.13-005).

---

### T14 — Stale trust anchors

**Adversary:** C-NET-ACTIVE  
**Defence:** RFC-v0.11-004 §6 (stale-anchor degraded posture). Nodes refuse bundle installs if trust anchor update window exceeded.  
**Residual:** Operator must configure update window appropriate to connectivity profile.

---

### T15 — Persistent-store corruption

**Adversary:** C-NODE-EXEC, C-NODE-PHYS  
**Defence:** RFC 053 (storaged integrity). Content-addressed storage with epoch binding.  
**Residual:** Store corruption before first epoch write detected only on subsequent read.

---

### T16 — Audit-ring evidence gap

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 053, 054 (audit drain). Audit ring is kernel-managed; services emit records through the drain capability.  
**Residual:** A compromised kernel could suppress records. Mitigated by the measurement chain (T8 defences).

---

### T17 — IPC sender forgery

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 055 (kernel-attested sender identity). The kernel stamps sender `TaskId` on every IPC; services cannot forge it.  
**Residual:** None within the current architecture.

---

### T18 — Service init authority escalation

**Adversary:** C-NODE-EXEC  
**Defence:** RFC 057, 058 (service lifecycle). Services are spawned from the init image with pre-bound caps; no escalation path.  
**Residual:** A vulnerability in `cap-broker`'s grant logic (covered by T2).

---

### T19 — Operator mistake within authorised scope

**Adversary:** C-OPER-MISTAKE  
**Defence:** Bounded blast-radius operations (RFC-v0.13-001 §3); confirmations for fleet-wide actions.  
**Residual:** Mistake in the TrustAnchorRoot ceremony (see OS7).

---

### T20 — Reproducibility-failure-as-substitution

**Adversary:** C-SUPPLY  
**Defence:** RFC-v0.10-003 (reproducible build gate). Two-build SHA-256 digest comparison (hardened from FNV-1a in RFC-v0.16-005, H-04).  
**Residual:** Malicious toolchain that produces identical output for different inputs.

---

## Out-of-scope threats (v1.0 explicitly does NOT defend against)

| Code | Threat | Rationale |
|------|--------|-----------|
| OS1 | TrustAnchorRoot compromise + node filesystem write | Irrecoverable; re-provision physically |
| OS2 | Compromise of build environment before signing | Supply-chain hygiene is operator responsibility |
| OS3 | Cryptographic break of Ed25519 (CRQC) | Post-quantum hybrid is research track |
| OS4 | Byzantine fault from > 0 cooperating malicious nodes | BFT deferred post-v1.0 |
| OS5 | Persistent physical adversary with anti-tamper bypass | Anti-tamper is hardware-side |
| OS6 | Side-channel attacks (timing, power, EM) | Primitives have mitigations; no defence claimed |
| OS7 | Malicious insider with TrustAnchorRoot access | Organisational control, not technical |
| OS8 | Covert channel via audit-ring timing | Possible; unmitigated at v1.0 |

---

## Operator obligations

1. **Key custody:** TrustAnchorRoot on offline cold-storage; rotation procedure per RFC-v0.13-003.
2. **Physical security:** Nodes meeting A3 archetype require physical tamper evidence.
3. **Build environment hygiene:** Clean CI environments; reproducible builds verified.
4. **Audit monitoring:** Review Trust Report at each release cycle.
5. **Re-attestation cadence:** Schedule per RFC-v0.13-004 §2.4.
6. **Incident response:** Follow RFC-v0.13-003/005 playbooks.

---

*Gate: `ci-threat-model-check` verifies every T<n> references a merged RFC in `done/`.*
