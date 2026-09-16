# RFC-0.32-003: Documentation that can be found

**Status:** Proposed
**Milestone:** 0.32
**Tracks.** **E-050** — 76 of the 135 files under the book root are in no book,
the book's pages point at documents it does not contain, and four directory
names exist twice.
**Touches.** `docs/` (all of it), `rfcs/`, `README.md`, `.gitignore`,
`.github/workflows/ci.yml`, `tools/fjell-consistency-check/`, and the eighteen
hard-coded `docs/…` paths in eight Rust files. **Does not touch kernel, ABI,
service or format source.**
**Relates to:** E-037 (a tool version declared in one place and checked
nowhere); E-016 (the link instrument this line leans on); the artefact leak
fixed in 0.30, whose shape `docs/book/` repeats.

**Sequencing.** This line **starts after RFC-0.32-002's review lands.** That
line's closure edits `ERRATA.md`, `v1-limitations.md` and two ADRs; moving
those files underneath it would collide, and this project fixes forward rather
than rebasing.

## Summary

The audit is `docs/verification/documentation-structure-audit.md`; its
measurements are not repeated here. What matters for scope:

- **135 `.md` under `docs/src`, 59 in `SUMMARY.md`.** The other 76 — including
  all 41 ADRs — are rendered nowhere and copied nowhere. `mdbook build` exits 0
  and warns about none of them.
- **The canonical document is usually outside the book** and the book page is a
  stub: 226 B against 6.0 KB (`v1-readiness`), 241 B against 83.9 KB
  (`unsafe-inventory`). One stub says it is a symlink; `find docs -type l`
  returns zero.
- **Duplicate names:** `perf`, `release`, `security`, `verification` exist
  inside and outside `docs/src`; `verification`, `assets` and `rfcs` also
  collide with repository-root directories; `docs/src/release` sits beside
  `docs/src/releases`; `ROADMAP.md` beside `docs/src/roadmap/roadmap.md`.
- **`docs/book/` is not ignored**, so a local build is committable.
- **The book's own toolchain is undeclared and already forked:** CI installs
  **mdBook 0.4.40**; the architect's machine built this audit with **0.5.4**.
  Nothing states which is correct, and nothing would notice a build that
  behaved differently under the other.

## The settled part

**D1 — One root for prose.** Every page written for a human to read lives under
`docs/src/` **and appears in `SUMMARY.md`**. That includes the ADRs, the
release process and its policy documents, the operations and deployment guides,
the compliance mapping, the audits, and the per-release records.

**D2 — `docs/` has exactly two kinds of child**: `src/` (the book, plus
`book.toml` and `theme/`) and one directory of **generated artefacts** — files a
tool writes or reads as data, not as prose (`trust-report.txt`,
`perf/baseline.json`). Nothing else sits at `docs/`'s top level.

**D3 — Prose a tool parses is still prose.** `v1-readiness.md`,
`v1-limitations.md`, `standards-mapping.md`, `release-checklist.md` and
`release-handoff.md` move **into the book**, and the tools' paths follow them.
A document does not leave the book because a program reads it.

**D4 — The RFC corpus has one root.** `docs/rfcs/ERRATA.md` becomes
`rfcs/ERRATA.md`; the eighteen `*-answer.md` documents become `rfcs/answers/`.
`docs/rfcs/` ceases to exist.

**D5 — No stubs.** A page is the document. Every pointer page is either
replaced by the document itself or deleted. The "symlinks" sentence goes with
them.

**D6 — No duplicate leaf directory names** anywhere under `docs/`, or between
`docs/` and the repository root. `docs/src/releases/` is renamed so it cannot
be confused with `docs/src/release/`.

**D7 — Moves preserve history.** `git mv` only, in coherent groups, each group
leaving `doc-links` green and `consistency-check` passing by its own exit
status. No file's content is rewritten in the same commit that moves it.

**D8 — The structure is held by instruments, not by intent.** Each of these is
a new `consistency-check` subcheck, and each is **demonstrated failing**:

1. **`summary-completeness`** — every `.md` under `docs/src` appears in
   `SUMMARY.md`, and every `SUMMARY.md` entry resolves to a file.
2. **`prose-in-the-book`** — no `.md` outside `docs/src` within `docs/`.
3. **`no-stub-pages`** — a page under `docs/src` whose body is essentially a
   link to a path outside `docs/src` fails, naming the target.
4. **`unique-doc-directory-names`** — no two directories under `docs/`, and
   none against the repository root, share a leaf name.

**D9 — `docs/book/` is ignored**, and a check asserts the ignore rather than
trusting it.

**D10 — The book's toolchain is declared in one place and checked**, the way
`rust-toolchain.toml` is (E-037). CI and a developer must build the same book
with the same mdBook.

**D11 — The demonstration is today's tree.** The four subchecks are built
**first** and run against the current structure: they must fail, naming the 76
files, the stubs and the duplicate names. That failing run *is* D8's evidence —
no synthetic fixture can be as honest, and nothing is moved until the
instrument that will keep it moved exists.

## The open questions

**§A — What is the artefact directory called, and what belongs in it?** My lean:
`docs/records/`, holding only what a tool writes or reads as data
(`trust-report.txt`, `baseline.json`). The argument against: the per-release
records under `docs/release/records/` are prose a person reads, and moving them
into the book while keeping the word "records" for machine artefacts invites
exactly the confusion this line is removing. **Name both, and say which word
each gets.**

**§B — Which roadmap survives?** `ROADMAP.md` (23.8 KB, root) and
`docs/src/roadmap/roadmap.md` (16.1 KB) are different documents. The guideline
keeps `README.md` lean and routes detail into the book, which argues for the
book's copy plus a root pointer — but the root file is the one a GitHub visitor
finds. Propose; do not merge two documents silently.

**§C — Does `ERRATA.md` really belong in `rfcs/`?** My lean is yes: it is
governance, cited by every RFC, and `docs/rfcs/` beside `rfcs/` is the sharpest
name collision in the tree. The argument against: the register is read by three
tools and by every reviewer, and moving it changes the most-referenced path in
the project. **If you move it, the same commit updates every tool path and
every relative link; say how many.**

**§D — What happens to existing URLs?** README links, crates.io's rendered
README, and any external reference to `docs/...` on GitHub break on a move.
mdBook supports redirects for the built site; the repository paths themselves
cannot redirect. **Say what you checked in the pinned mdBook version**, and
what the policy is for links this line breaks.

**§E — Which mdBook version is declared** (D10), 0.4.40 or 0.5.4, and where
does the declaration live so that `toolchain-declarations` can check it?

**Answer all five in writing before moving anything.**

## Requirements

**R1 — Re-derive the audit's figures** before acting: the 135/59/76 counts, the
stub sizes, the duplicate names, the eighteen hard-coded paths. Report every
disagreement. Positive control on every absence.

**R2 — D8's four subchecks, built first, failing on today's tree** (D11), with
the failing output quoted.

**R3 — D1–D6, as `git mv` groups** (D7), each group gated by its own
`consistency-check` exit status.

**R4 — Every hard-coded path updated in the same commit as the move it
follows**, and the tools' own tests updated with them.

**R5 — `SUMMARY.md` covers the whole book**, with the ADRs as a navigable
section and an index page.

**R6 — README and the root files** updated for the new paths, staying within
the guideline's 100–200 lines (it is 98 today).

**R7 — D9 and D10**, with `toolchain-declarations` extended to the book's
toolchain and demonstrated failing.

**R8 — `mdbook build` green under the declared version**, and the built site
spot-checked: an ADR reachable from the navigation, and no chapter linking to a
path the site does not contain.

**R9 — E-050 CLOSED**, or its survivors named; register and
`v1-limitations.md` in the same commit.

**R10 — The gates:** `release-rehearsal`, `consistency-check --all` by exit
status, `cargo fmt --all --check`, and a CI run id for the push.

### Non-goals

- **Rewriting any document's content.** This line moves, indexes and gates; it
  does not edit prose, except the stub pages it deletes and the links it must
  repoint.
- Merging the two roadmaps (§B proposes; the owner decides).
- Changing the RFC process, the errata lifecycle, or `rfcs/`'s own folder
  structure beyond D4.
- Moving `verification/verus/` — it is proof source, not documentation.
- Publishing the book anywhere new.

## Risks

**A move breaks a link nobody notices.** `doc-links` walks the whole tree and
now fails on stale allow-list entries too, so this is the one risk the project
is already instrumented against — but it checks paths on disk, not the built
site. R8 is the part that checks the site.

**The move collides with in-flight work.** Hence the sequencing note: this
starts after RFC-0.32-002's review lands, and it should be the only line
touching `docs/` while it runs.

**A restructure invites content edits.** It must not: a move commit that also
rewrites a paragraph makes the diff unreviewable, and this project has 224 RFCs
whose links into `docs/` must keep resolving.
