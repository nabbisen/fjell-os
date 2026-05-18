# RFC 027: Threat model and security boundary documentation

**RFC ID:** 027  
**Also known as:** RFC-v0.1.x-004  
**Status:** Proposed  
**Target version:** v0.1.2  
**Affects:** `docs/src/security/`

## Problem

Fjell OS is security-oriented but currently has no single document
that defines:

- the assets being protected,
- the trusted computing base in v0.1.0,
- the attacker model (and what attackers are out of scope),
- the trust boundaries between kernel, services, drivers, and stored
  artefacts,
- the known weaknesses that v0.1.0 ships with, and
- the path to closure in v0.2.

Without this document a contributor cannot tell which limitations are
intentional (deferred to a later version) and which are bugs.

## Proposed fix

Write `docs/src/security/threat-model-v0.1.md` with the following
sections:

1. Scope
2. Assets
3. Trusted Computing Base
4. Attacker Model
5. Trust Boundaries
6. Capability Boundary
7. IPC Boundary
8. MMIO / DMA Boundary
9. Persistent Store Boundary
10. Verified Artifact Boundary
11. Recovery Boundary
12. Known Weaknesses
13. Deferred Threats
14. v0.2 Security Boundary Closure Plan

### Minimum assets to enumerate

- kernel memory
- task address spaces
- capability tables
- lease table
- endpoint table
- DMA regions
- persistent state store
- boot-control block
- signed release metadata
- signed policy metadata
- immutable rootfs metadata
- snapshot records
- measurement chain
- local attestation records

### Explicitly deferred threats

The document must declare the following out of scope for v0.1.x:

- physical attacker
- hardware DMA attacker
- malicious firmware
- compromised boot ROM
- remote attacker over network
- supply-chain compromise beyond development-grade signing
- side-channel attacks
- SMP race attacks

## Rationale

A flat threat-model document is easier to maintain than scattered
notes in ADRs.  ADRs reference the threat model; the threat model is
the authoritative list of what v0.1.0 does and does not defend
against.

Deferring SMP and physical-attacker threats matches the v0.1.0
single-hart, QEMU-virt-only scope from RFC 024.

## Impact

- Documentation only.  No code changes.
- `README.md` gets a link to the threat model.
- Backward compatibility: full.

## Test plan

- File exists at the required path.
- Every section listed above has at least a brief paragraph (no
  placeholders).
- Every asset listed above appears under §2.
- Every deferred threat listed above appears under §13.
- `README.md` links to the document.
- mdBook build (`mdbook build docs`) succeeds and the threat model
  appears in the rendered SUMMARY.

## Implementation notes

- No formal proof.  No production certification.  No hardware-rooted
  threat model.
- The document is updated, not replaced, in v0.2.  v0.2's RFCs reference
  specific sections of this document.
