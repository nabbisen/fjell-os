# RFC 047: v0.2 preparation backlog

**RFC ID:** 047  
**Also known as:** RFC-v0.1.x-011  
**Status:** Proposed  
**Target version:** v0.1.5  
**Affects:** `docs/src/roadmap/`

## Problem

v0.2 (Security Boundary Closure) is too large to start "ad hoc".
Without a structured backlog assembled from every v0.1.x audit and
RFC, work will begin from individual contributors’ to-do lists, and
release blockers may be discovered late.

## Proposed fix

Produce `docs/src/roadmap/v0.2-preparation-backlog.md` that
collates **every** finding from the v0.1.x stabilisation phase
into a single backlog, grouped by v0.2 epic.

### v0.2 epics (mirror RFC-v0.2-001..013)

```
Epic A: Unified Capability Enforcement                  (RFCs 031, 032)
Epic B: Lease Revocation Semantics                      (RFCs 033, 034)
Epic C: MMIO / DMA Boundary Closure                     (RFCs 035, 036)
Epic D: Service Plane Realization                       (RFCs 037, 038)
Epic E: Persistent Evidence Hardening                   (RFCs 039, 041)
Epic F: cap-broker Policy Closure                       (RFC 040)
Epic G: Negative Test Expansion + Release Gate          (RFCs 042, 043)
```

### Required backlog entry format

```markdown
### V02-A-001: Replace caller_has_cap with require_cap

Severity:               Release Blocker
Area:                   Capability
Source:                 v0.1.x audit (RFC 029)
Resolving RFC:          RFC 031
Required negative tests:
  - revoked cap rejected
  - missing right rejected
  - stale handle rejected
Acceptance criteria:
  - all syscall paths use require_cap
```

### Sources to ingest

```
- RFC 029 (Capability / Lease Enforcement Audit) findings
- RFC 030 (MMIO / DMA Boundary Audit) findings
- RFC 044 (Audit / Snapshot / Semantic Evidence Audit) findings
- RFC 045 (ADR sync) ADR-recorded deferrals
- existing v0.1.x stabilisation RFCs’ "Deferred to v0.2" sections
```

### Required summary fields

The backlog header must state:

```
- total items
- count by severity (Release Blocker / Should / Could)
- count by epic
- earliest dependency
- latest dependency
- list of release blockers
```

## Rationale

A single backlog is the only way to start v0.2 with a coherent
plan.  Tying every item to:

- a *source* (which v0.1.x audit found it),
- a *resolving RFC* (which v0.2 RFC closes it), and
- *acceptance criteria + negative tests* (how we know it’s done),

makes v0.2 work atomic.  An item without a resolving RFC is a
gap; an item without a negative test is not "Release Blocker"
eligible.

## Impact

- Documentation only.  No code changes.
- The backlog becomes the input contract for v0.2.0 planning.

## Test plan

- Document exists.
- Every v0.1.x audit finding appears in at least one entry.
- Every entry has a resolving RFC (RFCs 031–043).
- Every release-blocker entry names at least one negative test.
- Every v0.2 RFC has at least one backlog entry pointing to it.

## Implementation notes

- No implementation in this RFC.  No v0.2 schedule lock.
- The backlog is *frozen* at the v0.1.5 tag; mid-v0.2 additions
  become new RFCs (RFC-v0.2-014 onwards) and update the backlog
  via amendment notes, not in-place rewrites.
- The "earliest / latest dependency" fields support a simple Gantt
  view for v0.2 planning without committing to a specific dates.
