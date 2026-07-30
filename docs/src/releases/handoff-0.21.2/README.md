# Fjell OS — Compact Handoff Bundle (v0.21.2)

This bundle hands off Fjell OS at **v0.21.2**, the v1.0 freeze candidate. It
follows the compact, evidence-focused handoff structure: every section is
either `Done — evidence`, `Pending — owner/date`, or `N/A — reason`.

An incoming actor should be able to answer five questions from these files:

1. **What is the project trying to achieve?** → `project-summary.md`
2. **What requirements must be preserved?** → `project-summary.md` §2, `external-design.md` §3
3. **What external design and boundaries are decided?** → `external-design.md`
4. **What implementation and testing details matter most?** → `implementation-notes.md`, `testing-and-gates.md`
5. **Which decisions must not be forgotten?** → `decision-log.md`

## Files

| File | Role |
|---|---|
| `project-summary.md` | PM / stakeholder: goal, requirements, status, decisions, risks, next steps |
| `external-design.md` | Designer / architect: external surface, module boundaries, tradeoffs |
| `implementation-notes.md` | Implementer: repo map, build commands, key internals, known gaps |
| `testing-and-gates.md` | Tester / QA: test strategy, gate commands, coverage, confidence verdict |
| `ops-security.md` | DevOps / release / security: packaging, gates, security decisions, rollback |
| `decision-log.md` | Consolidated decision register (referenced by the role files) |

## How to verify this handoff

Restore the working tree from the release archive and run the gates:

```sh
tar xzf fjell-os-v0.21.2.tar.gz
cd fjell-os-v0.21.2
cargo xtask build
cargo xtask release-rehearsal      # all 11 mechanical gates
cargo xtask test-all               # host tiers + QEMU
```

See `testing-and-gates.md` for the full command set and expected results.

## Status at handoff

- **Version:** v0.21.2 (v1.0 freeze candidate, patch)
- **v1.0.0 tag:** architect-conditionally-approved; **Gate 9 manual sign-off by
  the owner (nabbisen) is the only remaining blocker.**
- **Publication control:** v1.0.0 must not be tagged, published, announced, or
  released without explicit confirmation from nabbisen.
