# RFC 043: v0.2 security boundary release gate

**RFC ID:** 043  
**Also known as:** RFC-v0.2-013  
**Status:** Proposed  
**Target version:** v0.2.0  
**Phase:** Phase 9 — Negative Test Completion and Release Gate  
**Related epics:** G (Negative Test Expansion)

## Problem

v0.2.0 is the milestone that turns Fjell OS from a smoke-tested
prototype into a system whose security boundaries can be checked.
Without a binding release gate, individual RFCs can land and the
"v0.2.0" tag can still be applied even if the negative-test
matrix, the threat-model update, or the boundary audit is
incomplete.

## Proposed fix

A formal release-gate document
(`docs/releases/v0.2.0-release-gate.md`) blocks the v0.2.0 tag
until every checkbox below is satisfied:

### Build gates

- `cargo fmt --check` passes on every workspace member.
- `cargo check --workspace --exclude fjell-kernel` passes.
- Kernel builds for `riscv64gc-unknown-none-elf` in release.
- All service binaries build for the target.
- `fjell-tools` builds for the host.

### Positive smoke gates

- `TEST:M1:PASS` … `TEST:M8:PASS` all observed.
- A new `TEST:V02:PASS` marker emitted after the v0.2 smoke run
  exercises the new bootstrap → enforcing handoff (RFC 040) and
  a service-separation round trip (RFC 038).

### Negative gates (release blockers — all required)

- All `NEG:*:PASS` markers from RFC 042’s expanded matrix observed.
- `NEG:*:DEFERRED` markers exist **only** for cases the
  introducing RFC explicitly defers.

### Documentation gates

- `docs/security/threat-model-v0.2.md` exists and supersedes the
  v0.1 threat model.
- `docs/audit/v0.2-security-boundary-audit.md` exists and
  classifies every operation listed in RFC 029 as `OK` or
  `Deferred-with-RFC-link`.
- `docs/abi/v0.2-inventory.md` exists (a v0.2 update of RFC 028).
- ADRs for every new architectural decision in v0.2 exist (see
  RFC 032's docs list, RFC 040 typestate, etc).
- `CHANGELOG.md` v0.2.0 entry follows the Added / Changed / Fixed
  / Security / Known Limitations / Deferred to v0.3 rubric.

### Release artefacts

- `kernel-image` (ELF + flat binary)
- `rootfs-image` (if applicable)
- `boot-control-image` (if applicable)
- `qemu-disk-image` (if applicable)
- `serial-log` from the release smoke run
- `release-manifest` (digest of every artefact)

### One-page acceptance summary

The release gate document ends with a single page that asserts:

```
- every syscall requiring authority uses require_cap()
- revoked capability fails through every syscall and IPC path
- recursive policy revoke is handled by cap-broker without kernel
  tree traversal
- kernel revocation path remains O(1)
- IPC hot path remains O(1)
- blocked IPC wakes or cancels on revoke
- dead CSpace slots can be dropped
- services receive or observe LeaseRevoked / CapDropRequested
- MMIO mapping is impossible without MmioRegion capability
- RAM cannot be mapped as MMIO
- DMA allocation/use requires DmaRegion capability
- DMA memory is zeroized or quarantined before reuse
- misbehaving device cannot block DMA cleanup indefinitely
- cooperative services cannot monopolise the hart indefinitely
- cap-broker performs bootstrap handoff and enters Enforcing
- cap-broker enforces default deny
- copy_to_user rejects invalid user ranges
- auditd drains real kernel audit records
- storaged and bootctl run as separated services
- negative tests run in CI
- v0.2 threat model update is complete
```

Every line must be ticked off — no soft "mostly" allowed.

## Rationale

The release-gate document is the only way to make the v0.2
"Security Boundary Closure" claim verifiable.  Without it, the
release becomes a *story* told in the CHANGELOG instead of a
*checklist* signed off in a single place.

Modelling the gate after RFC-v0.1.x-010 (RFC 046 in this
repository) keeps the release process repeatable across versions.

The one-page acceptance summary is the *judge* against which a
contributor can self-test before opening a release PR.

## Impact

- Documentation only at gate-definition time.
- Procedural impact: a v0.2.0 tag cannot be applied without the
  document’s checkboxes being filled.

## Test plan

- The document exists at the listed path.
- Every gate line has a method of verification (CI job, file
  presence, marker observation).
- A dry-run release attempt missing one checkbox fails to produce
  a valid release archive (the archive-build script reads the gate
  document and refuses on an unchecked box).

## Implementation notes

- Out of scope: production release signing, public CVE process,
  vendor signing keys.  Those belong to v1.0.0 onward.
- The "archive-build script reads the gate document" is a
  desirable enforcement mechanism but not strictly required —
  human sign-off is acceptable for v0.2.0 if it is recorded in
  the release manifest.
