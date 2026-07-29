# External Design — Boot & Upgrade

*Subsystem 4 of 9. Anchored to FR-BOOT-001…003, NFR-REL-003 and
`fjell-bootctl` / `fjell-bootctl-model` / `fjell-upgraded` at v0.21.2.*

## 1. Responsibility

This subsystem covers three requirements: the boot chain verifies what it loads
(FR-BOOT-001), only a defined minimal set of services starts at boot
(FR-BOOT-002), and OS updates are atomic with rollback (FR-BOOT-003,
NFR-REL-003).

## 2. External surface

### Boot slot model (as-built, `fjell-bootctl-model`)

An A/B slot model:

```text
BootModel {
  slot_a, slot_b : SlotState,
  active         : Slot,        // currently booted
  pending        : Option<Slot>,// staged for next boot
  last_known_good: Slot,        // rollback target
}
```

`Slot::A.other() == Slot::B`. An upgrade stages a new image into the inactive
slot, verifies it, marks it pending, and the boot control switches `active` on
next boot. A failed boot rolls back to `last_known_good`.

### Minimal initial service set (FR-BOOT-002, as-built)

The boot brings up, in order: `init`, `configd`, `cap-broker`, `auditd`,
`service-manager`, then the device and storage plane. This matches the
requirement's candidate set (init, capability broker, configuration, audit,
state store, device manager, semantic output).

### Upgrade transaction (as-built, `fjell-upgraded`)

Immutable A/B staging: the new image is written to the inactive slot, its
signature and health are verified before the slot is confirmed, and the switch
is atomic on reboot.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-BOOT-001 Verifiable boot | Kernel + initial services + config integrity checked | `fjell-verifyd`, measurement chain |
| FR-BOOT-002 Minimal initial services | Explicit ordered bring-up of the minimal set | boot spawn order, `fjell-init` |
| FR-BOOT-003 Atomic upgrade | A/B slots; stage → verify → switch; rollback | `fjell-bootctl` (RFC 057), `fjell-upgraded` |
| NFR-REL-003 Update resilience | Power loss / verify failure cannot brick: unconfirmed slot is not booted | slot confirm logic |
| FR-SEC-004 Secure failure | Verify failure → do not switch, keep previous slot, audit | `verifyd` + bootctl |

## 4. Verifiable boot chain (external contract)

The boot control mirror selection (`select_bcb_mirror`) is machine-checked in
Verus (boot-control target: total and deterministic, 7 obligations) — a
pilot-tier proof, not release-gated but demonstrating the boot decision logic is
verifiable. The measurement chain (`fjell-measuredd`) and attestation
(`fjell-attestd`) provide the integrity evidence that verification consumes.

## 5. As-built scope limits & gaps

- **No real-hardware boot.** The validated profile is QEMU `virt`; the
  VisionFive 2 profile is provisional (requirements limitation item 1, errata
  E-004).
- **Secure-boot CA design is out of MVP scope** (requirements §7.3). v1.0 has
  signed-bundle verification but not a commercial-grade certificate authority
  lifecycle.
- **Store/upgrade negative emitters absent.** The upgrade path's
  negative-test markers (`NEG:UPGRADE:*`) are specified but not runtime-emitting
  at v1.0; documented as non-gated, required for v1.1.

## 6. Related subsystems

Boot verification depends on [Security & Trust](./security-trust.md) (signature
verification, trust anchor) and [Audit & Observability](./audit-observability.md)
(the append-only store that records upgrade transactions).
