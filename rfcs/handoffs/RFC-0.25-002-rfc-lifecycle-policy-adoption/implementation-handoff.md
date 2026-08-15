# Developer Handoff — RFC-0.25-002

**Governing RFC:** [RFC-0.25-002](../../accepted/RFC-0.25-002-rfc-lifecycle-policy-adoption.md)
**Milestone:** 0.25 — runs alongside RFC-0.25-001
**Status:** inherited from the governing RFC (Accepted, 2026-08-03)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. This is a merge, not a copy

The owner's instruction was to replace `rfcs/done/000-rfc-lifecycle-policy.md`
with `.git-exclude/rules/000-rfc-lifecycle-policy.md`. Re-confirmation found
that a verbatim copy would **delete five rules the project depends on**, one of
them in use today (`RFC-0.24-001` is `Implemented-with-Errata`, a state the
source policy does not define).

So: take the source document's structure, keep this project's rules. **The five
to keep are named in the RFC's D1 and you will be reviewed against that list by
name**, not on general resemblance:

1. `Accepted`
2. `Implemented-with-Errata`
3. `Closed`
4. The drift/errata rule — *"An RFC may not be marked Implemented if its
   normative text makes a claim the merged code does not satisfy"*
5. The required-sections list (9 items)

**Rule 4 is the one to protect.** It is why `ERRATA.md` exists, why Gate 7 and
Gate 12's `errata-limitations` have anything to check, and the source policy has
no equivalent because most projects have no errata register. If it is missing
from the merged document, the merge failed regardless of what else is right.

## 0.1 The thing the new document must say that the old one never did

`rfcs/README.md:3` says *"Folder is the source of truth for state (see RFC
000)."* `rfc_status_folder.rs:3` says the same.

**The current RFC 000 mentions folders zero times.** Both citations point at a
rule the target does not contain.

So the merged document must **state folder-as-source-of-truth explicitly**, in
its own words, as a normative rule. That is the single most important line in
the new file — it is the one the rest of the repository has been assuming
existed.

## 0.2 Design decisions settled — do not re-open

1. **Merge, don't copy.** Five rules kept (above).
2. **Document the naming the project uses** — `RFC-<milestone>-NNN-slug.md`,
   numbered per milestone. This *contradicts* the source policy's "sequential
   from 001, never reused", deliberately. Say so in the document, and say why:
   renaming 99 files breaks every commit message and release record pointing at
   them, which is the source's own anti-pattern.
3. **State what the prefix does not mean.** It is a **batch label** — the
   milestone an RFC was *raised under* — **not** a claim about where it shipped.
   Nine of the ninety-nine prefixed RFCs shipped in a different release, three
   of them *earlier* than their prefix (`RFC-v0.7.4-001` shipped in `v0.7.1`).
   Those are not mistakes; they are the scheme working as designed. The
   README's "Shipped" column is authoritative, and the merged policy must say
   all of this normatively — see the RFC's D2 for the required four points.
4. **No renaming, no renumbering, no `draft/` folder.**
5. **`proposed/` narrows to `Proposed` only** once `accepted/` exists.

---

## 1. Order

**R2 → R1 → R3 → R4 → R5.**

Write the merged policy *first*. R1 moves a file into a folder the policy has
not yet described, and R3 changes an instrument to enforce a rule that must
already be written down. Doing R2 last would mean codifying a layout that had
already shipped — which is how the current mismatch arose in the first place.

## 2. Per-requirement notes

**R2 — the merged policy.** Replaces `rfcs/done/000-rfc-lifecycle-policy.md`.
It stays at that path and keeps its `Implemented` status — it is not a new RFC.

Structure from the source: folder layout, the 5-folder variant, transitions,
handoff conventions, README integrity, cross-references, anti-patterns, CI
invariants. Rules from the current one: the five in §0.

Where the merged document departs from the source, **say so inline and say
why.** A reader comparing the two should never have to guess whether a
difference was deliberate.

**R1 — `rfcs/accepted/` and `rfcs/archive/`.** Two RFCs move: `RFC-0.25-001`
and **this RFC itself**, both `Accepted`. After the move `proposed/` is empty,
which is correct and not a problem.

`archive/` does not currently exist either, though the README documented it
until review. Create it empty. Nothing has been withdrawn or superseded yet.

**Moving this RFC while implementing it is the bootstrap** — expected, not a
conflict. Move it in the same commit as the others.

Move files with `git mv` so history follows them.

**R3 — the instrument.** `rfc_status_folder.rs` currently has:

```rust
const PROPOSED_STATUSES: &[&str] = &["Proposed", "Accepted"];
```

`Accepted` was tolerated there only because there was nowhere else to put it.
Now there is. Narrow `proposed/` to `Proposed`, add `accepted/` taking
`Accepted` only.

**This is a gate being strengthened, so RFC-v0.22-001's rule applies: it must be
observed failing before it is trusted.** Both directions, separately:

- an `Accepted` RFC left in `proposed/` → **FAIL**
- a `Proposed` RFC placed in `accepted/` → **FAIL**

Use the tool's existing synthetic-fixture test path (`run_check` is pure in its
inputs precisely so this is possible) — you do not need to move real files to
demonstrate it.

**R4 — the two false citations.** `rfcs/README.md:3` and
`rfc_status_folder.rs:3`. After R2 the rule genuinely exists, so both become
true statements. Check that they name the section they now point at.

**R5 — README and links.** Restructure for five folders. One file moved, so run
`grep -rl "RFC-0.25-001-external-interrupt-plane" .` before and after and
**report the match count both times** — not "links updated".

## 3. Prohibited shortcuts

- Do not copy the source document verbatim. Five rules would be lost.
- Do not drop the drift/errata rule. It is load-bearing for the errata register
  and two gates.
- Do not rename or renumber any RFC.
- Do not add `draft/`.
- Do not trust R3 without demonstrating it failing **both ways**.
- Do not fix E-016's other instances (13 broken links, the `v`-prefixed
  "Shipped" column). Out of scope.
- Do not claim a link sweep without reporting the grep counts.

## 4. Required evidence

1. **The five kept rules, quoted from the new document**, one line each, so the
   review can check them by name rather than by reading for sense.
1a. **D2's four naming points, quoted** — including the statement that the
   prefix is a batch label and not a release claim.
2. The new document's explicit folder-as-source-of-truth statement, quoted.
3. **R3's two demonstrations**, each observed failing before the fix is trusted.
4. `rfcs/accepted/` containing RFC-0.25-001; `proposed/` empty.
5. Grep counts before and after the link sweep.
6. `cargo xtask release-rehearsal` green; Gate 12 `rfc-status-folder` passing
   across all five folders; `syscall-surface` still **35/26/9**.
7. `cargo fmt --all --check` — **run it, do not predict it.**

## 5. Review request

Standard format, in `.git-exclude/review-request/`. One request.

Flag for focused review:

- **Anything you kept or dropped that is not on the five-rule list.** Judgement
  calls in a merge are exactly where a rule goes missing quietly.
- Any place the merged document contradicts the source where you were unsure
  whether the departure was intended.
- Whether narrowing `proposed/` broke anything you did not expect.
