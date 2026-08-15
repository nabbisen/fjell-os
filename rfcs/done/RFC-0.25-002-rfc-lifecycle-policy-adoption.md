# RFC-0.25-002: Adopt the 5-folder RFC lifecycle policy, and make RFC 000 say what the project actually does

**Status:** Implemented (0.25) — accepted 2026-08-03, R1–R5 complete
**Milestone:** 0.25 — runs alongside RFC-0.25-001; does not displace it
**Tracks.** RFC governance: folder layout, lifecycle states, and the document
that defines them.
**Touches.** `rfcs/` (new `accepted/`), `rfcs/done/000-rfc-lifecycle-policy.md`,
`rfcs/README.md`, `tools/fjell-consistency-check/src/rfc_status_folder.rs`.
No kernel, ABI, capability, lease, IPC, or crypto behaviour.
**Relates to:** `.git-exclude/rules/000-rfc-lifecycle-policy.md` (the source
policy), RFC-v0.22-001 (Gate 12's `rfc-status-folder` subcheck changes here),
RFC-0.24-001 (E-016 — nothing verifies a document's claims).

## Summary

The owner has directed adoption of the **5-folder variant**, and replacement of
the in-repo `rfcs/done/000-rfc-lifecycle-policy.md` with the rules document,
**after re-confirmation.**

Re-confirmation done. **The diagnosis is correct and understated.** But a
verbatim replacement would delete five rules this project actively depends on,
one of which is in use today. This RFC therefore **merges** rather than
replaces, and says exactly what it keeps and why.

## Motivation

### `archive/` is documented and does not exist

`rfcs/` contains `done/`, `handoffs/`, and `proposed/`. **There is no
`archive/`.** `rfcs/README.md`'s structure block listed one anyway, until this
RFC's review corrected it.

Nothing has been withdrawn or superseded yet, so the folder was never needed —
but documenting a folder that is not there is the same defect as citing a rule
that is not written. R1 therefore creates **both** `accepted/` and `archive/`.

### The in-repo RFC 000 is worse than stale

It is 64 lines. It documents file naming as flat `rfcs/<NNN>-<slug>.md`.

**It mentions folders zero times.** Not `proposed/`, not `done/`, not
`archive/`, not the word "folder".

Yet two places cite it as the authority for exactly that:

- `rfcs/README.md:3` — *"Folder is the source of truth for state (see RFC 000)"*
- `tools/fjell-consistency-check/src/rfc_status_folder.rs:3` — *"`rfcs/README.md`
  documents the folder as the source of truth for an RFC's lifecycle state
  (RFC 000)"*

**Both cite a rule the cited document does not contain.** The instrument whose
entire purpose is catching a status field that lies is itself resting on a false
citation, in its own doc comment.

This is E-016's shape — *no instrument verifies any document's claims* — landing
in the governance document. It is also the fourth instance this project has
found of a reference asserting something the target does not say.

### And the naming section describes a scheme nothing uses

| Scheme | Files | Where documented |
|---|---|---|
| `RFC-<milestone>-NNN-slug.md` | **99** | nowhere |
| `NNN-slug.md` | **62** | RFC 000, but as flat `rfcs/NNN-slug.md`, not in folders |

The project also **restarts numbering per milestone** — `0.24-001`, `0.24-002`,
`0.24-003`, then `0.25-001`. The rules document mandates the opposite:
sequential from `001`, stable forever, never reused.

So RFC 000 documents a third scheme that no file follows.

### What a verbatim replacement would delete

This is why the owner's "after re-confirmed" mattered.

| In-repo RFC 000 has | Rules document | Consequence of straight replacement |
|---|---|---|
| **`Implemented-with-Errata`** | absent | **RFC-0.24-001 is in this state right now.** Its status would become undefined, and `rfc_status_folder.rs` accepts it in `done/` |
| **`Closed`** | absent | Same — a currently-legal state becomes unspecified |
| **The drift/errata rule** — *"An RFC may not be marked Implemented if its normative text makes a claim the merged code does not satisfy"* | absent | **`ERRATA.md`'s entire premise.** Gate 7 and Gate 12's `errata-limitations` both rest on it. E-001 through E-017 exist because of this rule |
| **Required sections** (9, incl. Problem/Rationale/Test plan) | explicitly disclaimed — *"makes no claim about what an RFC contains"* | RFCs lose their required shape |
| — | mandates `NNN-slug.md`, sequential, never reused | Declares **99 of 161** existing files non-conforming |

The rules document is a **portable, general-purpose policy**, and says so: it is
written to be *"adopted verbatim"* by *"any project starting an `rfcs/`
directory."* This project is not starting one. It has 161 RFCs, a seventeen-item
errata register that depends on a state the general policy does not define, and
a naming convention the general policy forbids.

**Adopting it verbatim would make the policy correct and the project
non-conforming.** The merge goes the other way.

## Design decisions

### D1 — Merge, and enumerate what is kept

Take from the **rules document**: the folder layout and the 5-folder variant,
folder-as-source-of-truth stated *explicitly* (the thing currently only cited),
transitions, handoff conventions, README integrity, cross-reference discipline,
the anti-patterns, and the CI invariants.

Keep from the **in-repo policy**: `Accepted`, `Implemented-with-Errata`,
`Closed`, the drift/errata rule, and the required-sections list.

**The drift/errata rule is the one to protect.** It is this project's most
distinctive governance rule, it is why the errata register exists, and the
general policy has no equivalent because most projects have no such register.

### D2 — Document the naming the project uses, **and what its prefix does not mean**

`RFC-<milestone>-NNN-slug.md`, numbered per milestone. Historical `NNN-slug.md`
files (`000`–`061`, gapless) keep their names.

This contradicts the rules document, deliberately. **A policy that declares 99
existing files non-conforming is a policy nobody will follow**, and the
alternative — renaming 99 files — breaks every commit message, release record,
and errata entry pointing at them, which is the rules document's own
"Renumbering RFCs during reorganisation" anti-pattern.

**But documenting the scheme is not enough, because the scheme misleads.**
The prefix looks like a release and is not one. Measured across the 99 prefixed
RFCs, **nine shipped in a release different from their prefix** — and three
shipped *earlier* than it:

| RFC | Prefix implies | Actually shipped |
|---|---|---|
| `RFC-v0.7.4-001` — DMA Lifetime Safety | v0.7.4 | **v0.7.1** |
| `RFC-v0.7.3-002` — Crypto Profile Documentation | v0.7.3 | **v0.7.1** |
| `RFC-v0.7.5-001` — Catalog Ownership | v0.7.5 | **v0.7.4** |
| …six more | | |

**Nine of the sixty-four prefixed RFCs that record a shipped release — 14%.**
*(Corrected in the R2 review from "nine of ninety-nine": 37 prefixed RFCs have
not shipped and so cannot diverge, so the wider denominator counted things
outside the population.)*

**These are not mistakes. They are the scheme working as designed**, and it will
keep producing them: the prefix records the milestone an RFC was *raised under*,
milestones get re-planned, and an immutable identifier cannot follow a mutable
fact.

So the merged policy must state, normatively:

1. The prefix is a **batch label — the milestone under which the RFC was
   raised** — and is **not** a claim about where it shipped.
2. **`rfcs/README.md`'s "Shipped" column is the authority** for where an RFC
   landed.
3. The divergence is **measured at nine of sixty-four** — prefixed RFCs that
   record a shipped release, 14% — recorded so the convention is not mistaken
   for an invariant. *(This RFC originally stated "nine of ninety-nine". That
   denominator included 37 RFCs which have not shipped and therefore cannot
   diverge; corrected in the R2 review.)*
4. Two schemes coexist: flat `NNN-slug.md` for `000`–`061`, prefixed from v0.3
   onward. The break is historical and frozen; new RFCs use the prefixed form.

**Nothing currently checks (1) or (2)** — no instrument compares an RFC's
identifier to its Shipped column. That is a concrete instance for **E-016**'s
link-and-count instrument, and is recorded there rather than built here.

An identifier that invites a false inference is the same defect class the 0.24
audit spent a milestone on. This RFC cannot fix the scheme without a rename it
should not do; it can stop the scheme from lying by saying what it means.

### D3 — The 5-folder variant fits, by the source's own test

The rules document warns that `accepted/` sits empty where "proposed" and
"implemented" collapse, because the same person does both.

**They do not collapse here.** The architect proposes, the owner accepts, a
separate implementation model builds, the architect reviews. "The owner signed
off" and "the implementer finished" are distinct, dated, separately-recorded
events — which is precisely the criterion the source gives for adopting the
variant.

### D4 — `proposed/` narrows when `accepted/` exists

Today `rfc_status_folder.rs` allows **both** `Proposed` and `Accepted` in
`proposed/`, because there was nowhere else to put an accepted RFC. Once
`accepted/` exists, that tolerance becomes a hole: an Accepted RFC left in
`proposed/` would pass.

`proposed/` narrows to `Proposed` only. `accepted/` takes `Accepted` only.

## Implementation status

**R1, R3, and R4 landed on 2026-08-03, ahead of R2, by owner direction.**

The handoff ordered R2 (write the policy) before R1 (create the folder), so the
policy would describe the layout before the layout shipped. The owner directed
`rfcs/accepted/` be created immediately. That inverts the ordering, and the
architect noted it once and complied.

**The inversion was not taken cheaply.** Moving the two `Accepted` RFCs without
touching the instrument would have left them in a folder
`rfc_status_folder` does not read — **unchecked, while Gate 12 still reported
PASS.** That is mode 1 scope blindness, and it is what this project spent 0.24
removing. So R3 landed in the same change, with both failure demonstrations,
verified against the old predicate:

```
old predicate (Accepted tolerated in proposed/) → test FAILED   (miss)
new predicate                                   → test ok       (caught)
```

R4's two false citations were corrected in the same change, because both files
were being edited anyway and leaving a known-false claim in place while
touching the file around it is not defensible.

**Remaining: R2** — the merged policy document itself. Until it lands, the
repository runs the 5-folder layout under a policy that describes neither it
nor the 4-folder one. That gap is the cost of the inversion, and it is stated
here rather than left implicit.

| # | Requirement | Status |
|---|---|---|
| R1 | `accepted/` + `archive/`, two RFCs moved | **done 2026-08-03** |
| R2 | The merged policy | **done 2026-08-03** |
| R3 | Instrument learns `accepted/`; `proposed/` narrows | **done 2026-08-03** |
| R4 | Both false citations corrected | **done 2026-08-03** |
| R5 | README restructured, links swept (3 → 0) | **done 2026-08-03** |

## Scope

| # | Requirement |
|---|---|
| **R1** | Create `rfcs/accepted/` **and `rfcs/archive/`**; move every RFC whose status is `Accepted` into `accepted/`. Today that is **RFC-0.25-001 and RFC-0.25-002 itself** |
| **R2** | Replace `rfcs/done/000-rfc-lifecycle-policy.md` with the merged policy per D1/D2/D3 |
| **R3** | `rfc_status_folder.rs` learns `accepted/`; `proposed/` narrows to `Proposed` only (D4) |
| **R4** | Fix both false citations — `rfcs/README.md:3` and `rfc_status_folder.rs:3` — so they cite a rule the target actually states |
| **R5** | `rfcs/README.md` restructured for five folders; inbound links to moved files swept |

### Non-goals

- **Renaming any existing RFC.** See D2.
- **Renumbering.** The source document's own anti-pattern.
- Adding a `draft/` folder. Nothing needs it; the source says add it only when
  multiple authors need shared drafts.
- Any of E-016's other instances (13 broken doc links, the `v`-prefixed
  "Shipped" column). Those remain 0.25 candidates.
- Any kernel, ABI, capability, lease, IPC, or crypto behaviour.
- Gate 12 `syscall-surface` must stay **35/26/9** — untouched by this line.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The merge quietly drops one of the five kept rules | Medium | **High** | D1 enumerates them. The review checks each by name against the new document, not by reading for general sense |
| R2 | **R3 strengthens a gate and is not demonstrated failing** | Medium | High | RFC-v0.22-001's governing rule. An `Accepted` RFC left in `proposed/`, and a `Proposed` RFC placed in `accepted/`, must both be **observed failing** before the change is trusted |
| R3 | The link sweep misses inbound references to moved files | Medium | Medium | Only one file moves (R1). `grep -rl` before and after, and the count of matches reported |
| R4 | Scope creep into fixing E-016's other instances | Medium | Medium | Explicit non-goal. This line fixes the two citations it creates work adjacent to, nothing else |

## Acceptance criteria

- [ ] `rfcs/accepted/` **and `rfcs/archive/`** exist; RFC-0.25-001 and
      RFC-0.25-002 are in `accepted/` with `Status: Accepted`; `proposed/` is
      empty; no folder is documented that does not exist.
- [ ] The merged RFC 000 **states folder-as-source-of-truth explicitly** — the
      rule currently cited but unwritten.
- [ ] All five kept rules present and checked **by name**:
      `Accepted`, `Implemented-with-Errata`, `Closed`, the drift/errata rule,
      the required-sections list.
- [ ] The naming section describes `RFC-<milestone>-NNN-slug.md` and per-milestone
      numbering, and states where and why it departs from the source policy.
- [ ] It states **normatively** that the prefix is a batch label and not a
      release claim, that the README's "Shipped" column is authoritative, and
      that nine of sixty-four prefixed-and-shipped RFCs shipped elsewhere.
- [ ] **R3 demonstrated failing both ways** before being trusted: an `Accepted`
      RFC in `proposed/` → FAIL; a `Proposed` RFC in `accepted/` → FAIL.
- [ ] Both false citations corrected; no document cites RFC 000 for a rule it
      does not contain.
- [ ] `cargo xtask release-rehearsal` green, Gate 12 `rfc-status-folder` passing
      across all five folders, `syscall-surface` still **35/26/9**.
- [ ] `cargo fmt --all --check` clean.

## A note on why this is worth doing now

The owner deprioritised release-stability work in favour of functional lines,
and this is neither. It earns its slot on a narrower ground: **the project's
governance document does not describe the project**, and two artefacts —
including a verification instrument — cite it for a rule it does not contain.

It is also small, it does not compete with RFC-0.25-001 for the same work, and
it is the kind of drift that gets more expensive the longer 161 files accumulate
against a policy describing something else.
