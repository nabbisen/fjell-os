# RFC-v0.17-001: Trust Anchor Provisioning and Manufacturing Flow

**Status:** Proposed (design options — requires architect decision)
**Milestone:** v0.17
**Origin:** Deferred from RFC-v0.16-005 (architect review H-02).
**Supersedes:** RFC-v0.17-001 RESERVED placeholder.

## 1. Problem

A `TrustAnchorRoot` is Fjell's recovery authority of last resort: it
signs `CoordinatorPromotion` records (RFC-v0.13-003) and is the only key
that can re-establish authority after coordinator loss or key compromise.
Yet no RFC specifies **how the root is provisioned onto a node**. This is
the H-02 gap and an acknowledged v1.0 limitation: a node with no
provisioned root has no recovery authority, and a node provisioned by an
untrusted path has a forged one.

This RFC frames the provisioning approaches and asks for an architecture
decision. It deliberately does not commit a single mechanism, because the
right answer depends on the deployment tier (dev/QEMU vs. industrial
hardware) and on silicon features outside the v1.0 profile.

## 2. Requirements

- **R1.** Exactly one `TrustAnchorRoot` is bound to a node at end of provisioning.
- **R2.** The binding is tamper-evident: a node can attest which root it carries.
- **R3.** Provisioning is auditable: each event yields a typed semantic record.
- **R4.** The mechanism degrades safely: a failed/absent provision leaves the
  node in a clearly unprovisioned state, never a silently-trusting one.
- **R5.** No mechanism may weaken invariant I2 (every grant traceable to a
  signed authority).

## 3. Options

### Option A — Factory provisioning station (offline trusted tool)

A trusted offline tool injects the root at manufacture and records the
(device-id, root-pubkey) pair in a provisioning ledger.

- **Pros:** strongest control; per-device provenance; matches the A1
  industrial-gateway customer; no field trust assumptions.
- **Cons:** requires a secure facility and ledger custody; logistics for
  re-provisioning RMA'd units.
- **Fit:** v1.1 hardware path.

### Option B — First-boot TOFU (trust-on-first-use)

The node generates or accepts a root on first boot; an operator confirms
the fingerprint out-of-band before the node enters service.

- **Pros:** zero manufacturing infrastructure; works on the QEMU profile today.
- **Cons:** window of trust on first boot; relies on operator diligence;
  not appropriate for unattended hardware.
- **Fit:** v1.0 QEMU/developer profile only, explicitly labeled dev-grade.

### Option C — Hardware-anchored (RISC-V fuses / future TPM-like)

The root is derived from or sealed to an immutable per-device hardware
secret (eFuses, or a future measured-boot element).

- **Pros:** strongest; root cannot be exfiltrated or replaced in software.
- **Cons:** depends on silicon features absent from the v1.0 QEMU `virt`
  profile; board-specific.
- **Fit:** v2 direction.

## 4. Recommendation (for architect ratification)

| Tier | Mechanism |
|------|-----------|
| v1.0 (QEMU/dev) | Option B (TOFU), labeled dev-grade in release notes |
| v1.1 (hardware) | Option A (factory station) as the supported path |
| v2 | Option C (hardware-anchored) as the long-term root of trust |

This keeps v1.0 honest (no overclaim of hardware-rooted trust — consistent
with the v0.16 non-goals) while defining the growth path.

## 5. Provisioning semantic records (all options)

Provisioning emits typed records into the audit chain:
`TRUST.ROOT.PROVISIONED { device_id, root_pubkey_digest, method, ts }` and,
on failure, `TRUST.ROOT.PROVISION_FAILED { reason }`. This satisfies R3/R4
regardless of which mechanism is chosen.

## 6. Decision required

The architect must ratify: (a) the tier→mechanism table in §4, and
(b) whether the TOFU first-boot window is acceptable for the v1.0 dev
profile or must be gated behind an explicit `--allow-tofu-provision` flag.

## 7. Out of scope

Key escrow, root rotation post-provisioning (tracked separately), and the
hardware fuse programming protocol (Option C detail, deferred to v2).
