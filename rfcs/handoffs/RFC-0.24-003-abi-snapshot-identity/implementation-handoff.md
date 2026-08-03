# Developer Handoff — RFC-0.24-003

**Governing RFC:** [RFC-0.24-003](../../proposed/RFC-0.24-003-abi-snapshot-identity.md)
**Milestone:** 0.24 — blocks the cut
**Status:** inherited from the governing RFC (Proposed — accepted for implementation 2026-08-03)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The dangerous part is not the code

R1–R4 are ordinary repairs. **R5 — regenerating the baseline — is where this
line can do real damage.**

The ABI snapshot is the project's record of what it promises not to break.
Regenerating it under a corrected scanner rewrites that promise. If a genuine
breaking change is sitting in the tree right now, regeneration absorbs it and it
is gone — and the gate will report `PASS` forever after.

So: **the reconciliation is the deliverable, and the code is what makes it
possible.** Do not treat R5 as the cleanup step at the end.

## 0.1 Expected counts, stated in advance

The RFC commits to these before you start, deliberately, so they cannot be
rationalised afterwards:

| Cause | Items | Shape |
|---|---|---|
| B — `const fn` | **15** | `name:"fn"` → the real function name |
| C — inline `mod` | **162** | `module:""` → a qualified path |
| D — orphaned file | **17** | disappears entirely |

**Any difference outside these is a real ABI change. Stop and escalate.** Do not
regenerate over it, do not explain it in the review request and proceed. If the
totals come out at 15/162/17 exactly, say so; if they do not, that discrepancy
is the most important thing in your submission.

## 0.2 Design decisions settled — do not re-open

1. **The scanner follows the module tree, not the directory.** Start at
   `lib.rs`, descend through `mod` declarations, resolve `foo.rs` and
   `foo/mod.rs`, track inline `mod name { … }`. A file no `mod` declaration
   reaches is not scanned. This is the architect's decision and the reason D
   exists.
2. **It stays line-oriented.** Not a Rust parser. If you conclude the defects
   cannot be fixed without one, that is a design conflict — **escalate**.
3. **`storaged.rs` is not deleted.** It is dead code duplicating an inline
   module, and that is its owner's call, not an instrument repair. Record it.

---

## 1. Order

**R2, R3, R4 first — then R1 last.**

This is the reverse of how the RFC lists them, and it matters. R1's duplicate-key
check will fail loudly while B, C and D are still present, because they are what
*make* the duplicates. Landing R1 first means working against a red gate for the
rest of the line for no diagnostic gain.

Do R1 last, and it should pass on the first run. **If it does not, a defect
remains unfound** — report the surviving keys and escalate. Do not adjust the
check until it goes green; that inverts the entire point of this milestone.

## 2. Per-repair notes

**R2 — `const fn`.** `crates/fjell-abi/src/service.rs:55` (`pub const fn
from_bytes`) is the clearest instance. While you are there, check the same
prefix-confusion class for `pub unsafe fn`, `pub async fn`, `pub extern "C" fn`,
and `pub const unsafe fn`. **Report the result either way** — "checked, none
mis-parsed" is a useful finding and takes one line.

**R3 — inline modules.** Brace-depth tracking in a line-oriented scanner.
`crates/fjell-service-api/src/lib.rs` is the worked example: six `pub mod`
blocks, each with its own `pub const READY` at `0x200`, `0x210`, `0x300`,
`0x310`, `0x320`, `0x330`.

**Braces in string literals, char literals, line comments, and block comments
must not affect depth.** Write those unit tests *before* trusting the scanner
change. This is precisely the bug class the audit exists to find — introducing
one here would be the line failing at its own subject, and it would be found by
someone else later, which is worse.

**R4 — module tree.** The 17 items under `module: "storaged"` come from the
orphaned `storaged.rs` and must disappear. Note the trap: `fjell-service-api`
has *both* an inline `pub mod storaged { … }` in `lib.rs` **and** an unreferenced
`storaged.rs`. After R3 and R4, the inline one supplies the real
`storaged::*` items and the file supplies none. Confirm that explicitly rather
than assuming the count works out.

**R1 — identity.** `module` joins the key; duplicate keys in either map fail
`--verify`. Demonstrate the check itself with a hand-made duplicate, separately
from the Gate 4 demonstration.

## 3. Required demonstrations

| # | Break | Instrument must |
|---|---|---|
| 1 | Corrupt a **previously-shadowed** entry's signature (e.g. the first of the ten `READY` rows in the current baseline) | `--verify` → `FAIL`, where it reported `PASS` before this RFC |
| 2 | Hand-add a duplicate identity key to a baseline copy | `--verify` → `FAIL`, naming the duplicated key |
| 3 | Existing Slice 5 guards (no `count` header; truncated file) | still `FAIL` — confirm this RFC did not regress them |

Demonstration 1 is the one that matters: it is the Gate 4 finding, defeated.
Capture the `PASS` on the pre-change tool and the `FAIL` on the post-change tool,
against the *same* corrupted input.

## 4. Prohibited shortcuts

- **Do not regenerate the baseline before the reconciliation is complete.**
- Do not explain away an unexpected difference. Escalate it.
- Do not weaken or bypass R1's duplicate-key check to get a green tree.
- Do not turn the scanner into a Rust parser. Escalate instead.
- Do not delete `storaged.rs`.
- Do not touch `STABLE_CRATES`.
- Do not mark unexecuted commands as passed.

## 5. Required evidence

1. **The three demonstrations**, each observed failing and passing as specified.
2. **The reconciliation table**, every old→new difference assigned to B, C, or
   D, with counts against the expected 15 / 162 / 17.
3. Unit tests for brace-depth handling: string literals, char literals, line
   comments, block comments.
4. Zero duplicate identity keys in the regenerated baseline; no item named `fn`;
   no `fjell-service-api` item with `module:""`; no item from `storaged.rs`.
5. `cargo xtask release-rehearsal` green; Gate 12 still **35/26/9**.
6. `cargo xtask test-all` — all 19 tiers.
7. `cargo fmt --all --check` clean.
8. `docs/verification/instrument-audit.md` — Gate 4's row back to `sound`,
   citing demonstration 1 and **not** the tool's unit suite. That substitution
   is what made the row wrong the first time.

## 6. Review request

Standard format, in `.git-exclude/review-request/`. One request.

Flag for focused review:

- **Any discrepancy against 15 / 162 / 17**, however small and however plausibly
  explained. That is the first thing I will check.
- The brace-depth edge cases you were least confident about.
- Anything you found while reading the scanner that is not in this RFC. Three of
  its four defects were found that way.
