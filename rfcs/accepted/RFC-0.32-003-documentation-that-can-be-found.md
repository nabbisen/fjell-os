# RFC-0.32-003: Documentation that can be found

**Status:** Accepted — by the owner (nabbisen), 2026-09-16; implementation may begin (RFC 000)
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
- **Nothing publishes the book.** `ci-docs` runs `mdbook build` and the output
  is discarded: no deploy step, no artifact upload, no `gh-pages` branch, and
  the repository has **`has_pages=false`** (the Pages API returns 404). The 59
  chapters that *are* in `SUMMARY.md` are as unreadable to an outsider as the
  76 that are not. Found 2026-09-16, while confirming the owner's answer to §D.
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

## Settled by the owner, 2026-09-16

The five questions this RFC opened were answered before acceptance. They are
decisions now, not leanings.

**D12 — `docs/` holds the book and nothing else** (§A). Its children are
`book.toml`, `theme/`, `src/` and the version pin. Everything that is not a
book page leaves it:

| What | Where | Why |
|---|---|---|
| Every maintained page a human reads | `docs/src/…`, in `SUMMARY.md` | D1 |
| Per-release records and trust reports | **`releases/`** (repository root) | dated records, never edited after the cut — not maintained pages |
| Benchmark baseline data | `benches/baseline.json`, beside the benchmark crate that writes it | data read by `fjell-tools bench`; its prose becomes a book chapter |
| RFC corpus, answers, errata register | `rfcs/…` | D4 |
| Verus proof sources | `verification/verus/` (unchanged) | source, not documentation |

**The maintained/dated line is the rule that decides future cases**: a document
that is kept true lives in the book; a record of what was true on a date lives
in `releases/`. **No two of these directories share a leaf name** — `releases/`
(records), `docs/src/releasing/` (how a release is made), `docs/src/history/`
(session handoffs) — so the `release`/`releases` collision cannot come back
under a new spelling.

**D13 — `ROADMAP.md` at the repository root is the surviving roadmap** (§B),
and it is **rearranged and cleaned up** as part of this line.
`docs/src/roadmap/roadmap.md` (16.1 KB, overlapping content) is merged into it
and removed, and the book's roadmap entry goes with it. **This is the one
content edit this line performs**, and it is carved out of the non-goals below:
it happens in its own commit, separate from every move, so both diffs stay
reviewable. The four repository-root pages — `README.md`, `CHANGELOG.md`,
`ROADMAP.md`, `TERMS_OF_USE.md` — are the declared exception to D1, because
they are what a visitor finds first; the subchecks encode that list rather than
leaving it to judgement.

**D14 — `ERRATA.md` and the answer documents move to `rfcs/`** (§C), with every
tool path and relative link updated in the same commit.

**D15 — Documentation is technical debt too, so this line classifies it** (§C,
the owner's caution). Every document is **current**, **historical** or a
**record**. Historical documents — superseded ADRs, session handoffs, `v0.1.x`
scope and gate documents — live under one section that says so, and **each
carries a status line naming what superseded it and when**. A subcheck enforces
that line inside that section. Prose that looks current and is not is the
documentation form of a comment the compiler does not check.

**D16 — No redirects** (§D). Old paths are not preserved; the repository and
the site are updated in place. **Best effort on links**: `doc-links` must be
green, `README.md` and the root pages repointed, and R8 checks the built site
rather than only the filesystem.

**D17 — mdBook 0.5** (§E), declared in one file — `docs/MDBOOK.lock`, following
`verification/verus/TOOLCHAIN.lock`'s precedent — and checked by
`toolchain-declarations` against the version CI installs. CI moves off 0.4.40 in
the same commit. The audit's own build under 0.5.4 succeeded on today's
`book.toml`, which is evidence that the move is small, not that it is free.

**D18 — The book is published** (new finding above). A built book that is
thrown away cannot be "documentation that can be found", and this line is named
for that. GitHub Pages, deployed from CI on `main`. **The owner configured
Pages on 2026-09-16** — verified: `has_pages=true`, `build_type: workflow`,
source `main`, site `https://nabbisen.github.io/fjell-os/`. `build_type:
workflow` means a deployment comes from a workflow job, not from a branch, so
this line writes that job; until it exists the site has nothing to serve.

## Settled at the mid-line ruling, 2026-09-16

The implementation built the four subchecks first, ran them against the tree as
D11 requires, and stopped at the two collisions the move table cannot reach —
reporting rather than inventing a name, as §2 instructs. These are the rulings.

**D19 — `assets`: the book's copy is renamed `docs/src/images/`.** The
repository root's `assets/` holds brand material (favicons, logos) that README
and the published crates reference; it does not move. **The predicate is not
narrowed to prose-bearing directories.** A check that stops seeing a collision
because the colliding thing is a picture is a check with an exception in it,
and the exception would have to be maintained.

**D20 — `verification`: the book's copy is renamed `docs/src/assurance/`**, and
the audits from `docs/verification/` land there. The repository root's
`verification/` is proof source and does not move (non-goal).

**`docs/verification/verus/`'s 19 files are in scope**, and the non-goal does
not cover them: it names `verification/verus/` at the **root**, which holds the
proofs, `verus-targets.toml` and `TOOLCHAIN.lock`. The directory under `docs/`
holds guides, checklists, review records and appendices — prose about proving,
which is documentation. It becomes **`docs/src/assurance/proofs/`**, a name
that does not repeat `verus`, so no new collision is created.

**Two of those 19 are not prose** — `templates/rust-conformance-test-template.rs`
and `templates/verus-module-template.rs`. They are source templates and go to
`verification/verus/templates/` at the root, beside the proof source they
belong to. Adding files there is not "moving `verification/verus/`".

**D21 — `docs/src/verification/unsafe-inventory.md` is deleted, not
regenerated.** Verified at this review: the page states *"Total unsafe sites:
0"*, while `fjell-unsafe-audit` reports **277 sites, 277 with a SAFETY comment,
0 missing**. It is a generated report frozen at v0.6.0, asserting something
false about the current tree, in the book. Its replacement is a short chapter
describing what the unsafe gate checks and how to run it, **carrying no
counts** — a number in prose is a number that goes stale, and the gate prints
the live one. It is not regenerated into the book: a generated page needs a
generator and a freshness check, which is E-045's shape, and this line does not
build one.

**D22 — `RFC-0.26-001-…-investigation.md` goes to `rfcs/answers/`** with the
answer documents, and that directory's README says what it holds: documents
attached to an RFC — answers and investigations. One file does not earn its own
directory.

**D23 — both instrument decisions are approved as built.** Counting a full
`https://github.com/…/blob/main/docs/…` URL as a pointer is right: a stub is a
stub in either spelling, and the relative-path-only rule would have missed one
of the two real stubs. Rejecting an undeclared `.md` at the repository root is
**in scope and wanted**: it is what makes D13's four-page exception a list the
tree enforces rather than a sentence in an RFC.

**D24 — the deliberately-red tree is not pushed.** Gate 12 is red by design
between the instruments landing and the moves completing (D11), and a red
`main` is not the demonstration — the transcript is. The whole sequence is
pushed when the checks pass. *(Endorsing what the implementation already did.)*

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

**R9 — D13's roadmap merge and cleanup**, in its own commit, with a summary of
what moved and what was dropped.

**R10 — D15's classification**: every document marked current, historical or
record, the historical ones under one section with their status lines, and the
subcheck that enforces them.

**R11 — D17 and D18**: the mdBook pin and its check; the Pages deploy workflow,
**prepared and not enabled** until the owner switches Pages on, with the run id
of a successful build under the pinned version.

**R12 — E-050 CLOSED**, or its survivors named; register and
`v1-limitations.md` in the same commit.

**R13 — The gates:** `release-rehearsal`, `consistency-check --all` by exit
status, `cargo fmt --all --check`, and a CI run id for the push.

### Non-goals

- **Rewriting any document's content** — with one carved-out exception, D13's
  roadmap merge and cleanup, which happens in its own commit. Otherwise this
  line moves, indexes and gates; it does not edit prose, beyond the stub pages
  it deletes, the status lines D14 adds and the links it must repoint.
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
