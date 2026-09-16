# Documentation Structure Audit — 2026-09-16

**Auditor:** architect
**Scope:** every tracked `.md`/`.txt` documentation path in the repository.
**Method:** filesystem inventory, `mdbook build`, and the tools' own hard-coded
paths. Every count below was measured; none is quoted from a document.
**Occasion:** the owner reported `docs/` as unclassified and confusing, and
supplied a guideline used on another project
(`.git-exclude/rules/DOCUMENTATION_GUIDELINES.md`).

Its two structural rules are:

> All documentation source files reside under `docs/src/`. Use `SUMMARY.md` to
> define the book's navigation tree.

**Both are broken here, in opposite directions**: 68 documentation files live
outside `docs/src/`, and 76 files inside `docs/src/` are absent from
`SUMMARY.md`.

---

## The corpus, measured

| Location | `.md` files | In the book? |
|---|---:|---|
| `docs/src/` — listed in `SUMMARY.md` | **59** | yes |
| `docs/src/` — **not** listed | **76** | **no** |
| `docs/` outside `src/` | 68 | no |
| `rfcs/` | 224 | no |
| `verification/` (repo root) | 2 | no |
| repo root (`README`, `CHANGELOG`, `ROADMAP`, `TERMS_OF_USE`) | 4 | no |

## F1 — 76 files under the book root are in no book

`docs/src` holds 135 `.md` files; `SUMMARY.md` lists 59. The remaining **76
are published nowhere**: not rendered, not copied. `cd docs && mdbook build`
exits 0 and prints no warning; `docs/book/adr/` is created but contains **0
`.html` and 0 `.md`**, while the listed `intro/` chapters produce 3 `.html`
(the control).

What is invisible:

| Directory | Files | What they are |
|---|---:|---|
| `adr/` + `adr/superseded/` | **41** | every architecture decision record |
| `internals/` | 9 | including `local-development.md`, which a gate treats as a live toolchain declaration |
| `releases/` | 6 | session handoffs |
| `reference/` | 6 | capability model, intent-stream schema |
| `audit/`, `security/`, `roadmap/`, `architecture/`, others | 14 | — |

The ADRs are the project's decision history, cited by RFCs, errata and the
compliance mapping. A reader of the published book cannot reach one.

## F2 — the authoritative copy usually lives outside the book, and the book holds a stub

| Book page | Size | The real document | Size |
|---|---:|---|---:|
| `docs/src/release/v1-readiness.md` | 226 B | `docs/release/v1-readiness.md` | 6.0 KB |
| `docs/src/release/v1-non-goals.md` | 815 B | `docs/release/v1-non-goals.md` | 8.5 KB |
| `docs/src/dev/trust-report.md` | 284 B | `docs/release/trust-report.txt` | 5.3 KB |
| `docs/src/verification/unsafe-inventory.md` | 241 B | `docs/verification/instrument-audit.md` | 83.9 KB |

So the navigable book shows the pointer and hides the document. **And the
pointer misdescribes itself:** `docs/src/release/v1-readiness.md` says *"This
file symlinks to the live matrix"*. There are **zero symlinks** under `docs/`
(`find docs -type l`). It is a copy of a sentence, not a link.

A link from a book page to `../../release/v1-readiness.md` also cannot resolve
in the built site: nothing outside `docs/src` is copied into `docs/book`.
Thirteen such links point at `release/`, four at `verification/`, four at
`adr/`, two at `rfcs/`.

## F3 — four directory names exist twice, one exists three times

| Name | Outside the book | Inside the book | Relationship |
|---|---|---|---|
| `perf` | `docs/perf/baseline.md` | `docs/src/perf/baseline.md` | **copies**: they differ in one character — `../../` vs `../../../` — a copy made to relocate its own link |
| `security` | `docs/security/threat-model-v1.md` | `docs/src/security/threat-model-v0.1.md` | different vintages of the same document; the current one is outside |
| `verification` | `docs/verification/` (4 files, 108 KB) | `docs/src/verification/` (4 stubs, 4.9 KB) | stubs inside, substance outside |
| `release` | `docs/release/` (10 files) | `docs/src/release/` (4 files) | process inside, records and matrices outside |

`verification` exists a third time at the repository root
(`verification/verus/`, the proof toolchain and targets) — a different thing
again, and the book's `verus-setup.md` links out to it.

## F4 — names that differ only by a letter, or not at all

- **`docs/src/release/` vs `docs/src/releases/`.** Singular is release
  *process*; plural is *session handoffs*. Both are in `SUMMARY.md`, eight
  lines apart.
- **`docs/rfcs/` vs `rfcs/`.** The root `rfcs/` is the RFC corpus (224 files).
  `docs/rfcs/` is the errata register plus 18 "answer" documents that belong to
  those RFCs. Two directories with the same name, different owners, neither
  named for what it holds.
- **`ROADMAP.md` (root, 23.8 KB) vs `docs/src/roadmap/roadmap.md` (16.1 KB).**
  Two roadmaps, different content, neither marked as superseding the other.

## F5 — the build output is not ignored

`mdbook build` writes `docs/book/`. It is **not** in `.gitignore`: `git status`
lists it as untracked, so a routine `git add -A` would commit a copy of the
site. (This audit's own build was removed with `git clean -fdq docs/book`.)
The same shape as the artefact leak fixed in 0.30, one directory over.

## F6 — what a migration has to update

Eighteen `docs/…` paths are hard-coded in eight Rust files:

| File | Paths |
|---|---|
| `tools/fjell-consistency-check/src/errata_tracking.rs` | `docs/rfcs/ERRATA.md`, `docs/release/records` |
| `tools/fjell-consistency-check/src/errata_limitations.rs` | `docs/rfcs/ERRATA.md`, `docs/release/v1-limitations.md` |
| `tools/fjell-consistency-check/src/standards_mapping.rs` | `docs/compliance`, `docs/compliance/standards-mapping.md` |
| `tools/fjell-consistency-check/src/toolchain_declarations.rs` | `docs/src`, `docs/src/internals/local-development.md`, `docs/src/tutorials/quick-start.md`, `docs/src/releases`, `docs/release/release-checklist.md` |
| `crates/fjell-tools/src/{main,release_rehearsal,trust_report,bench}.rs` | `docs/release/v1-readiness.md`, `docs/release/trust-report.txt`, `docs/rfcs/ERRATA.md`, `docs/perf/baseline.json` |
| `tools/fjell-readiness-check/src/main.rs` | `docs/release/v1-readiness.md` |

**The link gate is on our side.** `doc-links` walks the whole tree (excluding
`target/`, `.git/`, `.git-exclude/`) and now fails on stale allow-list entries
too, so a move that breaks a link is caught locally before it is committed.
`doc-counts` only knows about `rfcs/`, so it is unaffected.

## The shape this points to

Not a proposal — the owner decides scope and schedule — but the audit's
conclusion in one table:

| Kind of document | Where it should live | Why |
|---|---|---|
| Everything written for a human to read | `docs/src/`, **in `SUMMARY.md`** | the guideline's rule, and the only way it is reachable |
| Decision records (ADRs) | `docs/src/adr/`, as a book section | 41 files currently reachable only by path |
| RFC corpus, answers, errata register | `rfcs/` (one root: `rfcs/answers/`, `rfcs/ERRATA.md`) | removes the `docs/rfcs` vs `rfcs` collision |
| Machine-read or generated data (`trust-report.txt`, `baseline.json`, release records) | one data directory, named as data | a tool reads them; they are not chapters |
| Build output (`docs/book/`) | `.gitignore` | F5 |

Two mechanisms would keep it true, in this project's own style:

1. **A subcheck: every `.md` under `docs/src` appears in `SUMMARY.md`**, and
   every `SUMMARY.md` entry resolves — demonstrated failing in both
   directions. Today's 76 would have been caught the day the first one landed.
2. **A subcheck for the stub pattern**: a page whose body is a pointer to a
   file outside `docs/src` is either the real document or is not a page.

Filed as **E-050**.
