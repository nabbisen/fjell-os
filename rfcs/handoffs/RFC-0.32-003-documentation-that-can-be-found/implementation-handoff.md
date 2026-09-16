# Developer Handoff — RFC-0.32-003

**Governing RFC:** [RFC-0.32-003](../../accepted/RFC-0.32-003-documentation-that-can-be-found.md)
**Milestone:** 0.32
**Status:** inherited from the governing RFC (Accepted, 2026-09-16)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

**Start after RFC-0.32-002's review has landed.** That line's closure edits
`ERRATA.md`, `v1-limitations.md` and two ADRs — three files this one moves —
and this project fixes forward rather than rebasing. While this line runs, it
should be the only one touching `docs/`.

---

## 0. The measure is a reader who can find the document, not a tree that looks tidy

A file that moved to a better path and is still in no navigation tree has not
been fixed. **When this line is done:**

- every maintained page is reachable from `SUMMARY.md`,
- the book is **published** and an ADR is reachable from its navigation,
- and four instruments fail the moment either stops being true.

**Do not start by moving files.** Start by building the instruments and running
them against today's tree, where they must fail and name what is wrong (D11).
That failing output is this line's demonstration, and it is more honest than
any fixture you could write.

## 0.1 Re-derive first (R1), each absence with a control

```
find docs/src -name '*.md' | wc -l                                   # expect 135
grep -oE '\]\(\.?/?[^)]+\.md\)' docs/src/SUMMARY.md | sort -u | wc -l  # expect 59
cd docs && mdbook build && ls book/adr                               # 0 html, 0 md
grep -rn '"docs/' --include='*.rs' crates tools | wc -l              # 21 literals in 10 files
```

The reference counts that size the work — derive them, do not trust this table:

| Moving path | Files that reference it |
|---|---:|
| `rfcs/ERRATA.md` | 44 |
| `docs/release/…` | 75 |
| `docs/verification/…` | 29 |
| `docs/security/…` | 21 |
| `docs/compliance/`, `docs/operations/` | 10 each |
| `docs/perf/`, `docs/deployment/` | 9, 5 |

**A grep that finds nothing must first be shown finding something.** The audit's
own count was wrong once before correction (a `./` prefix made 76 look like
135); re-derive rather than inherit.

*Corrected at the mid-line ruling, 2026-09-16, from the implementation's own
re-derivation: it is **21 literals in 10 files**, not 18 in 8 — of which one is
a test fixture, three already point inside the book, and 17 are real paths into
moving documents. Every reference count in the table above is 1–3 low, because
RFC-0.32-002 landed between this handoff and the line starting and every
document it wrote cites `ERRATA.md` and `v1-limitations.md`. The work is
slightly larger than the table says, not differently shaped.*

## 0.2 Settled — do not re-open

D1–D11 as written, plus the owner's answers **D12–D18**: `docs/` holds the book
only; records to a root `releases/`; the baseline to `benches/`; `ROADMAP.md`
survives and is cleaned; `ERRATA.md` and the answers to `rfcs/`; every document
classified; no redirects; mdBook 0.5 pinned; the book published.

---

## 1. Order

**R1 → the four subchecks, failing on today's tree → move groups (§2) →
`SUMMARY.md` complete → classification and status lines → mdBook pin and CI →
Pages deploy → ROADMAP merge (its own commit) → errata → evidence.**

**The subchecks come first and the moves make them pass.** Building them
afterwards would mean writing a test against a tree you have already shaped to
it, which proves nothing about either.

## 2. The move groups

One group per commit. **Each commit: `git mv` only, references repointed in the
same commit, `doc-links` green, `consistency-check --all` exit 0.** No file's
content is rewritten in a commit that moves it.

| # | Move | Notes |
|---|---|---|
| 1 | `rfcs/ERRATA.md` → `rfcs/ERRATA.md`; `rfcs/answers/*-answer.md` → `rfcs/answers/` | 44 referencing files; 3 tools hard-code the old path |
| 2 | `releases/*` → `releases/`; `docs/release/*trust-report.txt` → `releases/` | dated records; `errata_tracking` and `trust_report` read these |
| 3 | `docs/perf/baseline.json` → `benches/baseline.json` | `fjell-tools bench` writes it |
| 4 | `docs/{release,verification,security,compliance,operations,deployment,perf}/*.md` → `docs/src/…` | the prose; delete the stub pages they replace |
| 5 | `docs/src/release/` → `docs/src/releasing/`; `docs/src/releases/` → `docs/src/history/` | the `release`/`releases` collision |
| 6 | `SUMMARY.md` rewritten to cover every page, ADRs as a section with an index | the point of the line |

**Group 4 is where stubs die.** For each pair, the surviving file is the
substantive one — `v1-readiness.md` is 6.0 KB outside and 226 B inside; keep the
6.0 KB and delete the stub, do not merge them. The sentence claiming a symlink
goes with it.

**Two directories keep their name and do not move:** `verification/verus/`
(proof source) and `assets/`. If a leaf name still collides after group 5,
report it rather than inventing a name.

## 3. The four subchecks

Register in `SUBCHECK_NAMES` (`tools/fjell-consistency-check/src/lib.rs`); the
list is asserted against `main.rs` by an existing test, and Gate 12 reads it, so
the count moves from 11 to 15 in the rehearsal's own output.

| Name | Fails when | Demonstration |
|---|---|---|
| `summary-completeness` | a `.md` under `docs/src` is absent from `SUMMARY.md`, **or** a `SUMMARY.md` entry resolves to nothing | today's tree: 76 named files; then delete one entry, and add an unlisted page |
| `prose-in-the-book` | any `.md` under `docs/` outside `src/` | today's tree: 69 files; then re-add one |
| `no-stub-pages` | a page under `docs/src` whose body is essentially a link to a path outside `docs/src` | today's tree: the four known stubs; then re-create one |
| `unique-doc-directory-names` | two directories under `docs/`, or one against a repository-root directory, share a leaf name | today's tree: `perf`, `release`, `security`, `verification`, `assets`, `rfcs` |

**Derive, never hand-list.** The root-page exception (`README.md`,
`CHANGELOG.md`, `ROADMAP.md`, `TERMS_OF_USE.md`) is the one list, and it lives
in one place with a comment saying why each is there.

**Do not edit dated records to match the new count.** `releases/0.31.0.md`
says *"11 subchecks"* because that is what ran that day; under D12 a record is
never edited after its cut. That is also the first test of the
maintained-versus-dated rule — get it right here and the rest follows.

## 4. Publishing (D18)

Pages is configured: `has_pages=true`, **`build_type: workflow`**, site
`https://nabbisen.github.io/fjell-os/`. Because the build type is *workflow*,
nothing is served until a job deploys — a branch push will not do it.

- A job on `main` that builds the book under the pinned mdBook and deploys it
  (`actions/upload-pages-artifact` + `actions/deploy-pages`), with the
  `pages: write` and `id-token: write` permissions and a `concurrency` group so
  two pushes cannot race.
- **It deploys only after `SUMMARY.md` is complete** (group 6). Publishing a
  book that omits 76 pages would make the erratum's claim true on a public URL.
- **R8's site check is not optional:** fetch the deployed site and confirm an
  ADR is reachable from the navigation, and that no chapter links to a path the
  site does not contain. The filesystem link gate cannot see this.

## 5. mdBook 0.5 (D17)

`docs/MDBOOK.lock`, following `verification/verus/TOOLCHAIN.lock`: the exact
version, the date, and why. `toolchain-declarations` compares it to the version
`ci.yml` installs — CI currently fetches **0.4.40**, and this line moves it to
0.5.x in the same commit as the pin. Demonstrate the check failing on a
disagreement.

The audit built today's `book.toml` under 0.5.4 with exit 0, so the move is
expected to be small — **that is evidence it is possible, not that it is
free.** Read 0.5's changelog for `book.toml` keys and report anything that
changes behaviour.

## 6. Classification (D15)

Every document is **current**, **historical** or a **record**.

- **Historical** — superseded ADRs, session handoffs, `v0.1.x` scope and gate
  documents — lives under `docs/src/history/` (and `adr/superseded/`), and
  **each page carries a status line**: what superseded it, when, and that it is
  kept for the record. A subcheck enforces the line inside those directories.
- **Records** are the dated files in `releases/`; they are not edited.
- Everything else is current, which means someone is willing to keep it true.

**If a document is neither current nor honestly historical, say so in the
review rather than filing it under history to make the check pass.** Prose that
looks maintained and is not is the debt this decision exists to stop.

## 7. Prohibited shortcuts

- **No content edits in a move commit** — the single exception is the ROADMAP
  merge (D13), in its own commit.
- **`git mv`, never delete-and-add.** History must survive the move.
- Do not create a new stub to "keep the old path working" — D16 says no
  redirects, and a stub is a redirect with worse failure modes.
- Do not hand-list files in any subcheck.
- Do not skip the failing-first run (D11); it is the demonstration.
- Do not deploy the site before `SUMMARY.md` is complete.
- Do not edit dated records to match new counts.
- Do not touch `verification/verus/`, the RFC folder structure, or any crate
  source beyond the hard-coded paths.
- Do not run `cargo fmt --all --check` in your head.

## 8. Required evidence

1. R1 re-derived, with controls, and every disagreement reported.
2. **The four subchecks failing on today's tree**, output quoted, before any
   move.
3. Each move group as its own commit, with `consistency-check` exit status.
4. `SUMMARY.md` covering all 135 pages (or the number you re-derive), ADRs as a
   navigable section.
5. The stub pages deleted, named.
6. `docs/` containing only `book.toml`, `theme/`, `src/` and `MDBOOK.lock`.
7. Classification done; status lines in place; the subcheck enforcing them.
8. `docs/MDBOOK.lock`, CI moved off 0.4.40, the declaration check demonstrated
   failing.
9. **The deployed site**, with the ADR-from-navigation check and the no-broken-
   chapter-link check (R8), plus the deploy run id.
10. The ROADMAP merge, in its own commit, with what moved and what was dropped.
11. E-050 CLOSED or survivors named; register and `v1-limitations.md` together.
12. `release-rehearsal` green (Gate 12 now 15 subchecks); `consistency-check
    --all` **by exit status**; `cargo fmt --all --check`; a CI run id.

## 9. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **The four subchecks' failing output on today's tree**, first.
- **The live site URL and what you checked on it** (R8).
- **Any document you could not classify** (§6) — those are the interesting ones.
- **Anything that turned out to be referenced more widely than the table in
  §0.1** — the link-repointing is the part most likely to hide a mistake.
- Any figure of mine you re-derived and found different.
