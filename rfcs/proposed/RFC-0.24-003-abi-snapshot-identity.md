# RFC-0.24-003: ABI Snapshot Identity — what the gate compares, and what it is comparing it to

**Status:** Proposed — **accepted for implementation by the owner (nabbisen), 2026-08-03**
**Milestone:** 0.24 — **blocks the cut** (owner decision, 2026-08-03)
**Tracks.** Correctness of `fjell-abi-snapshot`'s scanner and diff identity.
**Touches.** `tools/fjell-abi-snapshot/src/main.rs`, `tests/abi/snapshot.json`.
No kernel, ABI, capability, lease, IPC, or crypto **behaviour** — but it changes
what the project *asserts* its ABI to be.
**Relates to:** RFC-0.24-001 (the audit; Gate 4's row is now `finding`),
RFC-0.24-002 (Slice 5 repaired this tool's *parser*; this repairs its *identity*),
RFC-v0.22-001.

## Summary

Gate 4 — ABI snapshot verify — was marked **sound** in the audit's Pass 1 and
reverted to **finding** in the RFC-0.24-002 review. Re-deriving it exposed not
one defect but four, and they compound:

| # | Defect | Items affected |
|---|---|---|
| A | Diff identity omits `module`; duplicate keys silently overwrite | **45 never compared** |
| B | `pub const fn` parsed as a `const` named `fn` | **15 misnamed** |
| C | Inline `mod` blocks untracked — all items take the file's path | **162 mis-attributed** |
| D | A file outside the module tree is scanned as public ABI | **17 phantom** |

Of 423 baseline items, **45 are invisible to the gate**, 15 carry a nonsense
name, 162 carry the wrong module, and 17 are not in the crate at all.

This RFC repairs all four. It was originally scoped as an eighth slice of
RFC-0.24-002 on the architect's estimate that it was "a duplicate-key check plus
one field in a tuple." **That estimate was wrong** — A alone leaves the gate
failing, because B, C and D are what make the duplicates. The owner re-decided
on the corrected scope.

## Motivation

### What the gate does today

```rust
let baseline_map: BTreeMap<(String, String, String), &AbiItem> = baseline
    .iter()
    .map(|i| ((i.crate_name.clone(), i.kind.clone(), i.name.clone()), i))
    .collect();
```

`module` is absent from the key, and `.collect()` into a `BTreeMap` **silently
overwrites duplicates — last one wins.** 423 items collapse to 378 keys.

Demonstrated in the RFC-0.24-002 review: corrupting the signature of a shadowed
entry — the first of ten `READY` rows — yields

```
  Baseline items : 423
  Changed sig    : 0
  Result         : PASS
```

A corrupted signature inside the file the gate exists to protect, and it reports
`PASS`.

### Why the duplicates exist — B, C and D

They are not accidental collisions. They are three scanner defects:

**B — `const fn`.** `crates/fjell-abi/src/service.rs:55` declares
`pub const fn from_bytes(…)`. The scanner sees `const`, takes the next token as
the name, and records `kind:"const", name:"fn"`. Fifteen items across seven
modules of `fjell-abi`, `fjell-cap`, and `fjell-semantic-v1` are named `fn`.
Seven of them share one key.

**C — inline `mod` blocks.** `crates/fjell-service-api/src/lib.rs` declares six
inline modules — `storaged`, `bootctl`, `measuredd`, `attestd`, `recoveryd`,
`verifyd` — each with its own `pub const READY` (`0x200`, `0x210`, `0x300`,
`0x310`, `0x320`, `0x330`). Six **distinct** ABI constants with six distinct
values. The scanner is line-oriented and derives `module` from the *file path*,
so all six are recorded as `module: ""` and collapse to one key. 162 of
`fjell-service-api`'s items are mis-attributed this way.

**D — a file outside the module tree.**
`crates/fjell-service-api/src/storaged.rs` exists, but **no `mod storaged;`
declaration exists anywhere** — `lib.rs` declares `pub mod storaged { … }`
inline instead. The file is not compiled into the crate. `scan_dir` walks the
directory, so its **17 items are recorded as stable public ABI that does not
exist.**

D is the inverse of the others: the gate asserts ABI stability over code that
is not in the crate.

### The design error underneath D

`scan_dir` walks the filesystem. The public ABI is what the crate *exports*,
which is the **module tree**, not the directory. Walking the directory is a
proxy for the module tree — **mode 2 of this audit's own taxonomy, proxy
attestation** — and D is what happens when the proxy and the property diverge.

**Decision (design authority): the scanner follows the module tree.** It begins
at `lib.rs` and descends through `mod` declarations, resolving `foo.rs` and
`foo/mod.rs`, and tracks inline `mod name { … }` blocks. A file no `mod`
declaration reaches is not scanned.

### Why Pass 1 called it sound

The row's demonstration was *"`cargo test -p fjell-abi-snapshot` — 8/8 pass."*
The tool's own unit suite passing is not the gate observed failing on a broken
repository state. It is mode 2 again, and it is exactly why this stayed
invisible: the unit tests use synthetic items that do not collide, are not
`const fn`, are not in inline modules, and are not orphaned.

That row is the architect's. It is the first of the 15 `sound` verdicts
re-derived under the audit's close-out item, and it fell on the first attempt.

## Scope — four repairs and one reconciliation

### R1 — Identity

Add `module` to the diff key, and **fail `--verify` when either map contains
duplicate identity keys.** The second half is the load-bearing one: it converts
a silent hole into a loud error, and it stays correct even if a future scanner
change reintroduces collisions.

### R2 — `const fn`

Recognise `pub const fn` as `kind: "fn"` with the correct name. Check the same
class of prefix confusion for `pub unsafe fn`, `pub async fn`, `pub extern
"C" fn`, and `pub const unsafe fn` — if any is mis-parsed, fix it here and say
so; if none is, say that too.

### R3 — Inline modules

Track `mod name { … }` blocks and qualify items with the resulting path. This
means brace-depth tracking in a line-oriented scanner. **Braces inside string
literals, char literals, and comments must not affect depth** — the tool already
has `unsafe_inside_string_literal_not_counted`-style precedent in its sibling
`fjell-unsafe-audit`, and the same class of bug is what this whole line exists
to find.

### R4 — Module tree

Follow `mod` declarations from `lib.rs` rather than walking the directory, per
the decision above. `storaged.rs`'s 17 items leave the surface.

**Not in scope:** whether `crates/fjell-service-api/src/storaged.rs` should be
deleted from the repository. It is dead code duplicating an inline module, and
that is a source-hygiene question for its owner, not an instrument repair.
**Record it; do not delete it.**

### R5 — Reconciliation, and this is the crux

R1–R4 will change many entries. Regenerating the baseline under a corrected
scanner is **the single most dangerous action in this RFC**: a genuine breaking
ABI change, if one is present, would be absorbed into the new baseline and
disappear.

**Required:** a reconciliation table, committed as part of the evidence, that
accounts for **every** difference between the old and new baselines and assigns
each to exactly one cause:

| Cause | Expected shape |
|---|---|
| B | `name:"fn"` → the real function name |
| C | `module:""` → a qualified inline-module path |
| D | item disappears (was never in the crate) |
| **unexplained** | **stop and escalate** |

**Any difference not explained by B, C, or D is a real ABI change and must be
escalated, not regenerated over.** The count is knowable in advance: 15 for B,
162 for C, 17 for D. If the reconciliation produces more than that, something
else moved.

## Non-goals

- Rewriting the scanner as a real Rust parser. It stays line-oriented; this
  repairs specific defects in it. *(If the implementer concludes the defects
  cannot be fixed without a parser, **stop and escalate** — that is a design
  conflict, not a coding decision.)*
- Changing `STABLE_CRATES`. Which crates are ABI-stable is a policy question and
  is not reopened here.
- Deleting `storaged.rs`.
- The deferred literal-matching family, or anything else from the audit's 33
  open findings.
- Any kernel, ABI, capability, lease, IPC, or crypto **behaviour**. The ABI
  itself does not change; only the project's record of what it is.
- Gate 12 `syscall-surface` must stay **35/26/9**.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R-A | **Regeneration masks a real ABI change** | Medium | **Critical** | R5's reconciliation, with expected counts stated in advance (15/162/17). Anything unexplained escalates. |
| R-B | Brace-depth tracking mis-handles braces in strings or comments, silently mis-attributing modules | **High** | High | Unit tests for each case before the scanner change is trusted. This is the exact bug class the audit exists to find; finding it here would be the line failing at its own subject. |
| R-C | Following the module tree drops items that *are* real, beyond the 17 | Medium | High | R5 catches it: a drop outside `storaged.rs` is unexplained by construction. |
| R-D | Scope creep into a full parser | Medium | High | Explicit non-goal, with escalation named as the response. |
| R-E | The duplicate-key check still fires after R2–R4 | Low | Medium | Then a defect remains unfound. **Do not weaken the check to get green** — report the surviving keys and escalate. |

## Acceptance criteria

- [ ] **The Gate 4 demonstration reproduced, then defeated:** corrupt a
      previously-shadowed entry's signature; `--verify` must now `FAIL` where it
      reported `PASS`.
- [ ] `--verify` fails on a baseline containing duplicate identity keys —
      demonstrated with a hand-made duplicate.
- [ ] Zero duplicate identity keys in the regenerated baseline. If any survive,
      escalate rather than adjust the check.
- [ ] No item named `fn` remains. No `fjell-service-api` item carries
      `module:""`. No item traces to `storaged.rs`.
- [ ] **R5's reconciliation table committed**, every difference assigned a
      cause, counts matching 15 / 162 / 17 or the discrepancy explained.
- [ ] Brace-depth tracking has unit tests covering braces in string literals,
      char literals, line comments, and block comments.
- [ ] `cargo xtask release-rehearsal` green; Gate 12 still **35/26/9**.
- [ ] `cargo xtask test-all` — all 19 tiers.
- [ ] `cargo fmt --all --check` clean.
- [ ] `docs/verification/instrument-audit.md`: Gate 4's row returns to `sound`,
      citing the demonstration above — not the tool's unit suite.

## A note on what this milestone is turning out to be

0.24 was scoped as an audit of instruments and became, in sequence: an audit, a
repair line, and now a correctness line in the tool that guards the ABI. Each
step was justified by the previous one finding something real.

That is the expected shape of this kind of work and not evidence of poor
scoping — but it is worth recording, because the same pattern will recur when
the deferred family is dispositioned, and because the 0.24 release record should
say plainly how much of the project's verification apparatus was found to be
reporting success without checking.
