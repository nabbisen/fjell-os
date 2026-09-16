# RFC-0.32-004: Advisories that exist

**Status:** Accepted — by the owner (nabbisen), 2026-09-16; implementation may begin (RFC 000)
**Milestone:** 0.32
**Tracks.** **E-051** — the security advisory process is specified, marked
Implemented, and has neither of its artefacts; two intake channels are
published at once; and nothing watches the advisory feeds for 153 third-party
packages.
**Touches.** `docs/src/security/` (post-restructure), `.github/SECURITY.md`,
the release checklist and cycle, `tools/fjell-consistency-check/`,
`.github/workflows/ci.yml`, and `rfcs/done/RFC-v0.15-003…`'s status line.
**Does not touch kernel, ABI, service or format source.**
**Relates to:** E-042, E-043, E-045, E-048 (the same shape — specified, marked
Implemented, never built); E-050's line, which moves the paths this one writes
into.

**Sequencing.** After **RFC-0.32-003**, whose move groups decide where
`docs/security/` lives. Writing new documents into a tree that is about to move
would create the merge this project avoids.

## Summary

Measured 2026-09-16.

### Finding 1 — the process exists as a description of itself

`RFC-v0.15-003` is **`Implemented (v0.15.0)`**. Its §3 specifies two artefacts:

| Specified | In the tree |
|---|---|
| `docs/security/advisory-process.md` — *"what happens between vulnerability reported and patched release shipped"* | **absent** |
| `docs/security/advisories/FSAD-<year>-<seq>.md` — one record per closed advisory | **directory absent**; no `FSAD-*` file anywhere |

What does exist is a condensed copy of the same process inside
`docs/release/release-checklist.md` — the severity tiers, the timeline, the
record template, and the sentence *"Committed to
`docs/security/advisories/FSAD-YYYY-NNN.md`"*, naming a directory that has never
existed. Nothing checks any of it.

**Zero advisories is the correct number today** — no vulnerability has been
reported. That is exactly why this is worth fixing now: the process will first
be exercised under time pressure, by one maintainer, on the day a real report
arrives.

### Finding 2 — two intake channels, two commitments

| Source | Channel | Acknowledgement |
|---|---|---|
| `.github/SECURITY.md` (published, and what GitHub surfaces) | private GitHub security advisory, real URL | *"within a small number of days"* |
| `docs/release/release-checklist.md` §Security advisory process | `security@<domain>` — *"fill in before v1.0 landing"* | **72 hours** |

A reporter who follows the checklist writes to a placeholder. A reporter who
follows `SECURITY.md` is fine. The project has therefore published, at the same
time, an address that cannot receive mail and a commitment it has not chosen.

### Finding 3 — nothing watches the dependency advisories

`Cargo.lock` holds **153 third-party packages** — among them `aes-gcm`,
`argon2`, `ed25519-dalek 2.2.0`, `curve25519-dalek`, `sha2`, `getrandom`,
`proptest`, `criterion`. There is **no `cargo-audit`, no `cargo-deny`, no
`deny.toml`, and no CI job** that consults the RustSec database or any other
advisory feed. A published advisory against a crate this project links would
reach the maintainer the way any other news does: by chance.

**The surface matters, and the honest statement of it is narrow.** The two
*published* crates — `fjell-os` and `fjell-abi` — have **zero** third-party
dependencies (`cargo tree -p fjell-os` is two lines). The exposure is the build
and host surface: tools, tests, benchmarks, and the development-grade crypto
crate. That distinction must survive into whatever this line writes, because
"no advisories affect us" and "no advisories affect what we ship" are different
claims and only one of them is cheap to defend.

### Finding 4 — the compliance mapping is already honest, and that is the point

CRA Part II records `CRA-II-1` (identify and document vulnerabilities and
components, incl. SBOM) as **not-met**, and II-2, II-4 and II-8 as **roadmap**.
Nothing in this RFC is needed to correct the mapping. What the mapping shows is
that the gap is *known and undated*: a "roadmap" row with no line behind it is a
promise with no owner, which is how E-045 spent three milestones.

## The settled part

**D1 — One process document, at one path.** The advisory process lives where a
reader can find it, and the release checklist references it rather than
restating it. Duplicated process text drifts: these two copies already differ.

**D2 — One intake channel and one acknowledgement commitment.** The published
`SECURITY.md` channel is the one the project can actually serve. The
placeholder address is removed, not filled with a second address.

**D3 — An advisory register with the errata register's discipline**: an index,
one record per advisory, required fields, stable ids. It is **valid and empty
today**, and a check must pass on empty while failing on malformed — fail-open
on absence is this project's most-repeated defect.

**D4 — Dependency advisories are checked mechanically**, against a named
database, over `Cargo.lock`. A finding is an erratum like any other.

**D5 — The check names its surface.** Every report says which crates it covered
and states that the published crates carry no third-party dependencies, so a
green run is never read as a claim about the shipped product's dependencies.

**D6 — `RFC-v0.15-003` is reclassified** `Implemented-with-Errata`, with the
two missing artefacts named, exactly as RFC-v0.6-003 and RFC-v0.5-002 were.

**D7 — Demonstrated failing**, on real runs: a malformed advisory record; an id
collision; and a dependency check shown red against a crate with a known
advisory, in a scratch clone, reverted.

## The open questions

**§A — Where does the dependency check run, and against what database?** The
advisory database is a network resource, and every gate in this project is
offline and deterministic (RFC-0.31-003 turned down a network-dependent gate
for exactly that reason). Three shapes:

1. **CI only** — a job that fails on an advisory affecting the graph. Cheap,
   but the local rehearsal then has a gate CI owns, which this project has
   never done.
2. **A pinned database snapshot**, vendored and refreshed deliberately, like
   `verification/verus/TOOLCHAIN.lock`. Offline and reproducible; goes stale
   silently, which is E-037's failure exactly.
3. **Both**: CI checks against the live database on a schedule and at each cut;
   the release record carries the database's commit id, so the check is
   reproducible after the fact.

**I lean to 3**, and name its cost: it adds a network dependency to the cut, and
a cut on a day the database is unreachable needs a stated rule. Say what that
rule is if you agree, and argue 2 if you think the offline property is worth
more than freshness.

**§B — Do advisory records live in the book or beside the release records?**
RFC-0.32-003 draws the line at maintained versus dated. An advisory is dated,
but it is also the document a user most needs to find, and it changes after
publication when a CVE id arrives. **My lean: in the book**, under security,
with the index in navigation. Argue it against the rule.

**§C — What is the relationship between an advisory and an erratum?** They
overlap: E-046 is a soundness defect at a trust boundary, and had it been
reported from outside it would have been an advisory. Options: keep the
registers separate with cross-references; or make an advisory a distinguished
erratum class. **Do not merge them silently** — say which, and what a reader
follows from one to the other.

**§D — Which acknowledgement commitment survives** (D2), 72 hours or "a small
number of days"? This is a promise to a stranger; it is the owner's to make,
and the RFC should carry whichever they choose rather than the one that was
written first.

**§E — Is an SBOM in scope?** `CRA-II-1` is not-met and names SBOM emission as
unbuilt. The dependency inventory this line must build to check advisories is
most of the input to one. **My lean: out of scope, seam named** — an SBOM is a
format and a distribution question, not an advisory question.

**Answer all five in writing before implementing.**

## Requirements

**R1 — Re-derive** the two absent artefacts, the two intake channels, the 153
packages, and the published crates' empty dependency graphs. Every absence with
a positive control. Report disagreements.

**R2 — D1 and D2:** one process document; the checklist referencing it; the
placeholder gone; `SECURITY.md` and the process agreeing on one commitment.

**R3 — D3:** the register, its index, and a subcheck — required fields, unique
ids, index and directory agreeing, and **passing on an empty register**.

**R4 — D4 and D5:** the dependency check as answered in §A, reporting its
surface, with the database identity recorded.

**R5 — D7's three demonstrations**, with transcripts, reverted.

**R6 — The release cycle** states when the advisory register and the dependency
check are read, and what a red dependency check does to a cut — block, or an
accepted-risk statement under the existing rule.

**R7 — D6:** `RFC-v0.15-003` reclassified, its two artefacts named.

**R8 — E-051 CLOSED**, or its survivors named; register and
`v1-limitations.md` in the same commit.

**R9 — The gates**, each by its own exit status, plus a CI run id.

### Non-goals

- **SBOM emission** (§E names the seam).
- Changing the threat model, the errata lifecycle, or the CRA mapping's
  verdicts — the mapping is already honest; it gains a line reference when this
  ships.
- Auditing the dependencies by hand, or removing any dependency.
- Writing a first advisory. The register ships empty.

## Risks

**A process written for an emergency is tested by the emergency.** This line
can only make the process exist, state one channel, and check the register's
shape. Whether the maintainer can meet a 30-day critical-patch target is not
something an instrument can assert, and the documents must not imply otherwise.

**A green dependency check will be read as "no third-party risk".** D5 exists
because of that, and the wording of the report matters more than the check.

**A pinned database goes stale invisibly** (§A shape 2) — E-037's exact
failure, in a place where staleness means a known vulnerability stays unknown.
