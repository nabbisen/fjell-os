# RFC 045: ADR and documentation synchronization

**RFC ID:** 045  
**Also known as:** RFC-v0.1.x-009  
**Status:** Proposed  
**Target version:** v0.1.4  
**Affects:** `docs/src/adr/`

## Problem

The repository already contains ADRs 0001–0010 covering many of
the v0.1.0 decisions, but the v0.1.x-rfcs document mandates a
*specific* set of ADRs whose titles do not all match the existing
ones.  The mismatch leaves future contributors guessing which
decisions are tracked where.

Without an explicit synchronisation, the v0.2 work cannot
confidently reference ADRs.

## Proposed fix

Reconcile the ADR set to the mandated list, either by:

- **renaming** an existing ADR to the mandated title (preferred
  where the content matches), or
- **adding** a new ADR (where content does not yet exist), or
- **superseding** an existing ADR with a new one and marking the
  old one *Superseded* (per ADR convention).

### Mandated ADR titles

```
docs/adr/0001-minimal-microkernel.md
docs/adr/0002-capability-based-ipc.md
docs/adr/0003-lease-epoch-revocation.md
docs/adr/0004-user-space-service-plane.md
docs/adr/0005-semantic-stream-first.md
docs/adr/0006-user-space-driver-model.md
docs/adr/0007-append-only-state-store.md
docs/adr/0008-verified-immutable-rootfs.md
docs/adr/0009-ab-boot-control-health-confirmation.md
docs/adr/0010-local-evidence-and-recovery.md
docs/adr/0011-development-grade-crypto-before-hardware-trust.md
docs/adr/0012-no-general-network-before-security-closure.md
```

### Required ADR fields

Each ADR must include:

```
Status
Context
Decision
Consequences
Security Boundary Impact
Deferred Work
Related RFCs
```

### Mapping from existing ADRs

| Existing | Mandated | Action |
|---|---|---|
| 0001 Target Architecture | 0001 Minimal Microkernel | Rename + retitle |
| 0002 Microkernel Boundary | 0002 Capability-Based IPC | Supersede (split content) |
| 0003 Capability Security | 0003 Lease Epoch Revocation | Supersede (split content) |
| 0004 Semantic Stream | 0005 Semantic Stream First | Renumber |
| 0005 v0.1.0 Scope | (kept as historical archive) | Mark *Superseded by RFC 024* |
| 0006 Device Driver Model | 0006 User-Space Driver Model | Rename |
| 0007 Persistent Store Model | 0007 Append-Only State Store | Rename |
| 0008 Verified Rootfs Trust Model | 0008 Verified Immutable Rootfs | Rename |
| 0009 A/B Boot Control | 0009 A/B Boot Control + Health Confirmation | Rename |
| 0010 Inline Init Workaround | (kept as historical archive) | Mark *Superseded by RFC 038* |
| — | 0004 User-Space Service Plane | New |
| — | 0010 Local Evidence and Recovery | New |
| — | 0011 Development-Grade Crypto Before Hardware Trust | New |
| — | 0012 No General Network Before Security Closure | New |

Renumbering is allowed because no external link depends on the
exact ADR numbers; internal cross-links are updated.

## Rationale

ADRs are durable engineering artefacts.  Letting them drift from
implementation reality renders them worse than useless — a
contributor reading the ADRs and the code will get *different
mental models* of the same system.

Adding new ADRs (0004, 0010, 0011, 0012) captures decisions that
were made implicitly in M0–M8 but never written down.

## Impact

- Documentation only.  Existing ADR numbers may change; an
  `ADR-RENAME.md` migration note records the mapping so external
  citations can be updated.
- Backward compatibility: full at the source-tree level; ADR
  citation links in non-versioned external documents may need
  updating once (recorded in the migration note).

## Test plan

- Every mandated ADR exists.
- Each ADR contains all required fields.
- Each ADR lists at least one Related RFC.
- mdBook builds the new ADR set without warnings.
- A `link-check` job over `docs/src/` does not flag broken refs.

## Implementation notes

- Non-goals: rewriting all of `docs/src/`, full user manual.
- `Superseded` ADRs remain in the tree (they record history).  They
  must contain a forward link to the superseding ADR or RFC at the
  top of the file.
- "Related RFCs" should reference RFCs by both filesystem number
  and `Also known as` name where relevant.
