# Adversarial Review — v0.16 Validation Closure

**Scope:** threat model (`docs/security/threat-model-v1.md`) and v1.0
non-goals (`docs/release/v1-non-goals.md`).
**Purpose:** close errata E-007 and E-009 — both documents shipped without
a recorded adversarial review.
**Method:** red-team pass asking, for each in-scope threat, "what would an
attacker actually try, and does the stated defence hold under v0.16's
*validated* (not merely *claimed*) state?"; and for each non-goal, "could
omitting this invite scope creep or a false sense of safety?"

---

## 1. Threat model findings

### Confirmed adequate for the narrowed v1.0 (QEMU profile)

- **T10 signature forgery (algorithmic).** Previously the weakest claim
  due to the Ed25519 test-vector gap. Now closed: sign path is conformant
  and cross-verified against three implementations (RFC-v0.16-001). The
  defence holds.
- **T13 fleet partition exploitation.** The partition reconcile path is
  now exercised end-to-end with a rollback-rejection arm
  (RFC-v0.16-002). The defence is no longer type-only.
- **T11 key compromise.** Revocation FSM + re-sign drilled (RFC-v0.16-003).
  Key-at-rest now encrypted (RFC-v0.16-006), removing the plaintext-key
  amplifier from the workstation-compromise scenario.

### Findings that adjust the model

- **F-1 (downgrade to plaintext key).** An attacker who can run
  `key gen --insecure-plaintext` on an operator host produces a plaintext
  key. **Mitigation folded in:** the flag prints a non-suppressible
  warning and the threat model now notes that `--insecure-plaintext` is a
  CI-only affordance; operator runbooks must forbid it. Residual risk
  accepted for v1.0 (operator-host trust boundary).
- **F-2 (replay cache cold after reboot).** Confirmed real (handoff §3.2.3).
  An attacker who forces a verifier reboot can replay attestations within
  the nonce window. **Disposition:** unchanged for v1.0 — defence-in-depth
  only; nonce challenge is primary. Documented limitation, not a new
  control.
- **F-3 (IPC tag collision).** Two independently authored services could
  collide on ad-hoc tags (lesson L3). The runtime trial used the declared
  CONFIG.* tags without collision, but no registry enforces uniqueness.
  **Disposition:** v1.x; listed as a non-goal explicitly below.

### Out-of-scope items re-examined

OS1 (TrustAnchorRoot compromise + filesystem write) and OS6 (post-quantum)
remain correctly out of scope for a QEMU developer profile. No change.

---

## 2. Non-goals findings

The non-goals list was tested for "scope-creep bait" — omissions a reader
might wrongly assume are present.

- **Added explicitly:** kernel-mediated IPC for the SDK reference service
  (the runtime trial is handler-level), multi-VM fleet validation, and
  `ZeroizeOnDrop` byte-level key-erasure guarantee. Each could be mistaken
  for "done" by a generous reader; each is now a stated non-goal.
- **Confirmed present and correct:** real-hardware deployment, production
  industrial readiness, full DR rehearsal, hardware-rooted trust,
  multi-hart safety, POSIX.
- **Trust-anchor provisioning (H-02):** confirmed as a genuine operational
  gap, now a named non-goal with a successor RFC (RFC-v0.17-001).

---

## 3. Outcome

No finding requires a v1.0 architecture change. Three findings (F-1, F-2,
F-3) are folded into the threat model and/or non-goals as documented
residual risks. The threat model and non-goals are attested as
adversarially reviewed for the narrowed v1.0 scope.

*Closes E-007 and E-009.*
