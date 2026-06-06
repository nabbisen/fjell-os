# RFC Lifecycle Policy

## Status model

```
Proposed → Accepted → Implemented → Closed
                   ↘ Withdrawn
        Implemented → Implemented-with-Errata → Closed
                   ↘ Superseded
```

| Status | Meaning |
|---|---|
| **Proposed** | Filed; under discussion or awaiting review |
| **Accepted** | Approved by architect; implementation may begin |
| **Implemented** | Code merged; smoke tests pass; CHANGELOG updated |
| **Implemented-with-Errata** | Code merged, but the RFC text claims more than what shipped; the divergence is recorded in `ERRATA.md` |
| **Superseded** | Replaced by a later RFC; pointer to successor required |
| **Withdrawn** | Superseded or no longer relevant |
| **Closed** | Implemented + documented; no further action needed |

### Drift and errata rule

An RFC may not be marked **Implemented** if its normative text makes a
claim the merged code does not satisfy. In that case it is marked
**Implemented-with-Errata** and an entry is added to `docs/rfcs/ERRATA.md`
naming: the RFC, the claim, what actually shipped, and the tracking RFC
for closure. No RFC may silently carry drift into a release.

## File naming

```
rfcs/<NNN>-<slug>.md
```

`NNN` is a zero-padded sequential number starting at 001.  
`000` is reserved for this policy document.

## Required sections

Each RFC must contain:

1. **Title** — one sentence
2. **RFC ID** — matches filename
3. **Status** — one of the states above
4. **Problem** — what is broken or missing; evidence (file + line if applicable)
5. **Proposed fix** — concrete change; code snippets where helpful
6. **Rationale** — why this fix and not alternatives
7. **Impact** — crates affected; backward compatibility
8. **Test plan** — how correctness is verified after the fix
9. **Implementation notes** — any constraints the implementer must know

## Scope

RFCs cover:
- Confirmed bugs with observable impact
- Architecture decisions with cross-crate effect
- Breaking changes to the kernel ABI or syscall table
- Deviations from the design documents that must be tracked

RFCs do **not** cover:
- Routine feature additions within a milestone's scope (tracked in ROADMAP.md)
- Cosmetic or style changes
- Dependency version bumps
