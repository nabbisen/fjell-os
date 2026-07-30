# Review Request — RFC-v0.22-001 Gate Integrity (Slices 1–4, complete)

**Governing RFC:** [rfcs/proposed/RFC-v0.22-001-gate-integrity.md](../../rfcs/proposed/RFC-v0.22-001-gate-integrity.md)
**Handoff:** [rfcs/handoffs/RFC-v0.22-001-gate-integrity/implementation-handoff.md](../../rfcs/handoffs/RFC-v0.22-001-gate-integrity/implementation-handoff.md)
**Submitted by:** implementation model
**Status:** All four slices implemented, tested, and committed. **Not
ready to tag `0.22.0`** — two real findings surfaced (see §4) require
architect/owner disposition before this line can close.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commits: `4cc9e55` (Slice 1), `b275035` (Slice 2), `deac327` (Slice 3),
`4f155c5` (Slice 4)
Release record: [docs/release/records/0.22.0.md](../../docs/release/records/0.22.0.md)
(explicitly marked BLOCKED — not ready to tag)

---

## 1. Implementation summary

All five §Scope items implemented, in the RFC's own sequencing (1 → 2 → 3
→ 4, with 5 folded into Slice 4):

| # | Item | Status |
|---|---|---|
| 1 | Mechanical syscall-count check | Done — `tools/fjell-consistency-check/`, `syscall-surface` subcheck |
| 2 | Gate 4 signature normalisation | Done — `tools/fjell-abi-snapshot/src/main.rs` |
| 3 | Gate 11 → function-body scan | Done — `crates/fjell-tools/src/callsite_audit.rs` |
| 4 | Bind documented rules to gates + Gate 12 wiring + "eleven gates" sweep | Done |
| 5 | Record v1.0 checklist Step 9 finding in ERRATA | Done — E-012, folded into Slice 4 |

The gate table grew from eleven to twelve gates
(`crates/fjell-tools/src/release_rehearsal.rs`), per the architect's
pre-made decision (handoff §0.1) to add one new tool
(`tools/fjell-consistency-check/`) rather than two.

## 2. Per-slice detail

### Slice 1 — syscall-surface check (`4cc9e55`)

New `tools/fjell-consistency-check/` crate. `syscall-surface` subcheck
parses `SyscallNumber` declarations in `crates/fjell-abi/src/syscall.rs`
and dispatch arms in `crates/fjell-kernel/src/trap/syscall.rs`, compares
both against committed `tests/syscall/expected.toml` (35 declared, 26
dispatched, the explicit 9-name undispatched set — not a bare count, so
one syscall can't silently replace another). Required failure
demonstration (both directions) present and passing:
`new_declared_syscall_not_in_expected_fails`,
`stale_expected_entry_no_longer_in_source_fails`.

### Slice 2 — ABI signature normalisation (`b275035`)

`tools/fjell-abi-snapshot/src/main.rs`: added `normalize_signature`
(whitespace-collapse) and `join_wrapped_declaration` (paren-depth-based
line joining, so a rustfmt-wrapped multi-line declaration is normalised
as one signature, not left half-fixed — the specific failure mode the
handoff called out as "the likely way this slice ships half-working").

Baseline regeneration provenance:
- Before: `--verify` against the unchanged tree, old algorithm: PASS
  404/404/0/0/0.
- After changing the algorithm, re-verify against the *old* baseline:
  404/404/0 added/0 removed/**28 changed** (all normalisation, confirmed
  by inspecting names — identical; only hashes differ).
- Regenerated `tests/abi/snapshot.json`; re-verify: PASS 404/404/0/0/0.
- Idempotency check: a further whole-tree `cargo fmt --all` produced
  **zero** additional file changes and zero additional signature changes.

Required failure demonstrations present and passing:
`realigned_whitespace_produces_no_signature_change`,
`differently_indented_wrapped_declaration_produces_no_signature_change`,
`genuine_signature_change_still_detected`,
`wrapped_declaration_does_not_swallow_the_body`.

**Flagged for focused review per handoff §9: the wrapped-declaration
handling.** `join_wrapped_declaration` uses paren-depth only (not
angle/square brackets, to avoid `->` desyncing the count) — correct for
every real declaration in this codebase's 404 tracked items, but worth an
independent look given it's the part of this slice most likely to have a
subtle edge case.

### Slice 3 — Gate 11 function-body scan (`deac327`)

`crates/fjell-tools/src/callsite_audit.rs` rewritten. Added
`strip_comments_and_strings` (strips `//`, `/* */`, and string-literal
contents; deliberately leaves char literals/lifetimes alone, with a
documented rationale — no search token is a single character, so this
can't produce a false positive, and disambiguating `'a` from `'x'`
textually needs a real parser). Added `find_function_body` +
`extract_braced_body` (locate `fn <name>`, brace-match the body,
search only within it). Applied: Check 1 (LEASE-CALLSITE-001) scoped to
`revoke`, Check 2 (CAP-CALLSITE-001) scoped to `mint`. Check 3
(BCB-CALLSITE-001) is inherently cross-file, so it keeps its existing
file-level WARN-only scan, now over stripped text — semantics unchanged,
per handoff requirement 3.

Required failure demonstrations present and passing (unit tests):
`lease_check_ignores_wrapping_add_mentioned_only_in_a_comment`,
`cap_check_fails_when_is_subset_of_only_in_comment`,
`cap_check_fails_when_is_subset_of_only_in_unrelated_function`.

**Live demonstration against the real codebase** (not just unit tests):
renamed `is_subset_of` in `crates/fjell-cap/src/cspace.rs`'s `mint`
function to a token that isn't a substring of the real one, ran the
compiled `fjell-tools callsite-audit` binary directly against the modified
source:
```
[FAIL] CAP-CALLSITE-001: `is_subset_of` not found in `mint`'s body
callsite-audit: FAIL (exit code 1)
```
File restored immediately after; `git diff --stat` confirmed
`crates/fjell-cap/src/cspace.rs` unmodified in the committed tree.

**Result against the real, unmodified codebase: all three checks PASS.**
The stronger scan did not expose a real LEASE/CAP/BCB-CALLSITE violation —
nothing to report as a found defect for this slice.

**Flagged for focused review per handoff §9: the comment/string
stripping.** `strip_comments_and_strings` doesn't handle nested block
comments (rustc supports them; no audited file uses them) and doesn't
special-case raw strings (`r"..."`) — both are documented limitations of
a deliberately-textual scanner, not oversights, but worth confirming
they're acceptable.

### Slice 4 — rule binding, Gate 12, eleven-gates sweep (`4f155c5`)

Three new subchecks in `tools/fjell-consistency-check/`, exactly the three
named in handoff §5 (no more, per R4):

- `errata-limitations` — every `ACCEPTED` erratum in `ERRATA.md`'s Summary
  table referenced in `docs/release/v1-limitations.md`.
- `rfc-status-folder` — each RFC's Status agrees with its folder
  (`rfcs/proposed/` ⇒ Proposed/Accepted; `rfcs/done/` ⇒
  Implemented/Implemented-with-Errata/Superseded/Withdrawn/Closed).
- `handoff-status` — each handoff's inherited Status matches its governing
  RFC's actual Status.

Gate 12 wired into `release_rehearsal.rs`, delegating through
`fjell-tools`'s existing `abi-snapshot`/`readiness-check` indirection
pattern (added a `consistency-check` subcommand to
`crates/fjell-tools/src/main.rs` rather than calling
`fjell-consistency-check` directly from `release_rehearsal.rs`, for
consistency with Gates 4/5).

"Eleven gates" sweep (`grep -rn "eleven gates\|11 gates\|Gates 1–11"`):
one live, forward-looking claim updated (`docs/src/roadmap/roadmap.md`
"remaining before the tag"); three frozen historical bundles left as
originally written with a correction note appended
(`handoff-0.21.2/implementation-notes.md` ×2,
`handoff-v0.19-v0.20.md`), matching the existing "no nesting" correction
already in that same bundle. RFC-v0.22-001's own text, its handoff, and
RFC-v0.21.3-002 (done) were left untouched — their "eleven gates" mentions
are correctly-historical narrative about why this RFC exists, not live
claims.

Required failure demonstrations present and passing:
`fails_when_an_accepted_erratum_is_missing_from_limitations`,
`done_rfc_with_accepted_status_fails`,
`stale_proposed_handoff_after_rfc_implemented_fails`.

## 3. Evidence (handoff §8, all items)

1. `cargo xtask release-rehearsal` — full twelve-gate table produced (see
   §4 below for the two red gates).
2. `cargo metadata --no-deps`: exit 0, 89 members (was 88; Slice 1 added
   `tools/fjell-consistency-check`). `cargo fmt --all --check`: clean.
   `cargo xtask build`: full workspace build succeeds.
3. `cargo xtask test-all --no-qemu`: 5/5 required tiers PASS.
   `cargo xtask test-all` (full): 18/18 PASS, zero regressions.
4. Every required failure demonstration listed per-slice above was
   **run and shown failing** — not just added as inert test code. Slice 3
   additionally has a live, real-file demonstration (see above).
5. ABI baseline before/after numbers: §2, Slice 2.
6. Any violation exposed by Slice 3: **none** — see §2, Slice 3. Two
   unrelated real findings were surfaced by Slice 4 instead; see next.

## 4. Real findings surfaced — reported, not fixed (handoff §6)

Both are disclosed in full detail, with evidence, in the release record
(`docs/release/records/0.22.0.md`, §Findings blocking this release). Summary:

1. **Gate 12 (`rfc-status-folder`) FAILS**:
   `rfcs/done/RFC-v0.17-001-trust-anchor-provisioning.md` sits in
   `rfcs/done/` but its Status still reads `Accepted (design options —
   requires architect decision)`, never updated despite its TOFU-flag
   deliverable shipping in v0.20.0. Exactly RFC 000's "status field that
   lies" anti-pattern, caught mechanically for the first time. Needs an
   architect decision on RFC-v0.17-001's true current status — a
   substantive RFC-content judgment, not something I should decide.

2. **Gate 7 (`ERRATA register`, pre-existing, unchanged) FAILS**: adding
   the honestly-`OPEN` erratum E-012 (the v1.0 checklist Step 9 finding,
   RFC scope item 5) makes Gate 7's existing "0 OPEN" rule correctly
   report 1. This is Gate 7 behaving correctly against newly-recorded real
   data. E-012 does not block a v0 release on its substance (it's a
   v1.0-checklist-specific gap) — only Gate 7's blanket rule turns it into
   one here, which is itself a question for the architect about whether
   Gate 7 should distinguish v0-relevant from v1.0-only errata.

Neither was fixed here, per handoff §6 and §7 ("do not fix violations
surfaced by the new rigour — report them"; "do not weaken a check to make
an exposed violation disappear"). No accepted-risk statement was written —
that requires an explicit owner/architect decision, which I should not
make on their behalf.

## 5. Open question for the architect

Given Findings 1 and 2 above, and that `Cargo.toml`'s version has not been
bumped to `0.22.0` (also an owner-level decision per RFC-v0.21.3-002's
roles table), **should I proceed to bump the version, add the CHANGELOG
entry, and finalize the tag once these two findings are dispositioned —
or is there a different disposition you'd prefer for one or both
findings first?** I've stopped here rather than guess at either the RFC
status correction or the ERRATA/Gate 7 question, consistent with handoff
§0 ("if you find a design conflict, stop and escalate — do not resolve it
in code").
