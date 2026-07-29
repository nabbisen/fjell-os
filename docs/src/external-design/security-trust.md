# External Design — Security & Trust

*Subsystem 8 of 9. Anchored to FR-SEC-004, FR-SEM-004, NFR-SEC-001…004 and
`fjell-sig-ed25519`, `fjell-keyring`, `fjell-trust-provider`, `fjell-sxt-crypto`,
`fjell-bundle-format`, `fjell-verifyd` at v0.21.2.*

## 1. Responsibility

This subsystem provides the cryptographic trust that the boot, upgrade, and
semantic-stream subsystems depend on: signed-bundle verification, the trust
anchor, the keyring, authenticated channels, and the secure-failure discipline
(FR-SEC-004). It also defines the crypto boundary — Fjell relies on audited
primitives, not custom crypto.

## 2. External surface

### Signed bundles (as-built, `fjell-bundle-format` + `fjell-verifyd`)

Deployed binaries and upgrade images are content-addressed and signed. `verifyd`
verifies Ed25519 (RFC 8032) signatures before execution. Key formats are `FJK2`
/ `FJKY`. Verification failure means the artifact is not executed.

### Trust anchor & provisioning (as-built)

The trust anchor is provisioned explicitly. `cargo xtask provision-dev
--allow-tofu-provision` writes a dev trust-anchor key and provenance file; there
is **no silent default TOFU** (RFC-v0.17-001). `verifyd` embeds the provisioned
key when present and warns loudly when unprovisioned.

### Crypto stack (as-built)

| Crate | Purpose |
|---|---|
| `fjell-sig-ed25519` | Ed25519 / RFC 8032 signatures |
| `fjell-keyring` | Key storage and epoch management |
| `fjell-trust-provider` | Trust-anchor provider model |
| `fjell-sxt-crypto` | Authenticated transport crypto (AES-256-GCM, Argon2id) |

### Authenticated channels (FR-SEM-004, as-built `secure-transportd`)

The semantic streams between OS and proxy are protected by a session capability,
message signing, monotonic sequence numbers, replay prevention, read-only
channels, and per-proxy authority scope.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-SEC-004 Secure failure | Verify/config/boot/update failure → no grant, no start, no apply, revert, audit | `verifyd`, bootctl, default-deny |
| FR-SEM-004 Stream auth & integrity | Session cap + signing + sequence + replay-prevent + per-proxy scope | `secure-transportd`, capability scope |
| NFR-SEC-001 Default deny | Undelegated/unvalidated → rejected | universal cap-gating |
| NFR-SEC-002 Minimal attack surface | No complex FS/net/GUI/plugins in kernel; small dependency set | kernel module list |
| NFR-SEC-003 Tamper detection | Boot image, config, audit, state store tamper-evident | signatures + append-only store |
| NFR-SEC-004 Auditability | Security events traceable | audit subsystem |

## 4. The secure-failure contract

"Fail safe" has a precise meaning in Fjell (FR-SEC-004): on any validation
failure the system grants no authority, starts no service, applies no change,
reverts to the previous version, and leaves an audit record. This is realized by
default-deny capability mapping (`PermissionDenied` on missing rights),
unconfirmed-slot boot avoidance, and audit emission on denial (`IpcDenied`,
`CapRevoke`, `LeaseRevoked`).

## 5. Verification-scope statement (important)

Fjell's formal verification is **selective and honest**. Verus machine-checks
three narrow pure-logic predicates (capability non-amplification, lease epoch
revocation, boot-control mirror selection). It does **not** cover the full
kernel, the IPC implementation, the service-manager, or crypto. The correct
claim:

> The *predicates* governing capability minting and lease revocation are
> machine-checked. The *implementation paths* that invoke them are unit-tested,
> property-tested, and QEMU-negative-tested — not formally verified end-to-end.

Crypto correctness rests on audited upstream primitives, not on Fjell proofs.

## 6. As-built scope limits & gaps

- **Dev/QEMU TOFU only.** Factory-station provisioning (v1.1) and
  hardware-anchored provisioning (v2+) are not implemented (requirements
  limitation item 6, RFC-v0.17-001).
- **Signing-side coupling is operational, not enforced.** The operator must sign
  bundles with the provisioned authority's key; no gate verifies the
  correspondence end-to-end. Required for v1.1.
- **No independently verified byte-level ZeroizeOnDrop guarantee** (non-goal
  N23).
