# RFC Lifecycle Policy

## Status model

```
Proposed → Accepted → Implemented → Closed
                   ↘ Withdrawn
```

| Status | Meaning |
|---|---|
| **Proposed** | Filed; under discussion or awaiting review |
| **Accepted** | Approved by architect; implementation may begin |
| **Implemented** | Code merged; smoke tests pass; CHANGELOG updated |
| **Withdrawn** | Superseded or no longer relevant |
| **Closed** | Implemented + documented; no further action needed |

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
