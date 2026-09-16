# Developer Handoff — RFC-0.32-004

**Governing RFC:** [RFC-0.32-004](../../accepted/RFC-0.32-004-advisories-that-exist.md)
**Milestone:** 0.32
**Status:** inherited from the governing RFC (Accepted, 2026-09-16)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

**Start after RFC-0.32-003.** That line decides where `docs/src/security/` lives,
and this one writes new documents into it. Use the post-restructure paths.

---

## 0. This line is written for a day that has not happened

Every other line this milestone fixed something that was already wrong in a way
a test could show. **This one prepares for a single future event** — the first
real vulnerability report — and the only honest test of it is that event.

So the measure is not "the process is good". It is:

- **a reporter reaches a channel that answers**, and finds one commitment, not
  two;
- **the register exists and is checked while empty**, so the first record ever
  written is validated by an instrument that has been running for months;
- **a published advisory against a dependency reaches us mechanically**, not by
  chance.

**Nothing here may overclaim.** The documents must not imply that a
single-maintainer project can guarantee a 30-day patch, and the dependency
check must not be readable as "the product has no third-party risk".

## 0.1 Re-derive first (R1), each absence with a positive control

```
ls docs/src/security/                                  # no advisory-process.md, no advisories/
find . -name 'FSAD-*' -not -path './target/*'      # empty; control: find . -name 'SECURITY.md' hits
grep -n 'security@' docs/src/releasing/release-checklist.md
grep -c '^name = ' Cargo.lock                      # 243 total, 153 third-party
cargo tree -p fjell-os --depth 2 -e normal         # two lines: fjell-os -> fjell-abi
```

The last one is the most important number in this line: **the published crates
have no third-party dependencies.** Verify it for `fjell-abi` too, and report
the result whichever way it goes.

## 0.2 Settled — do not re-open

D1 one process document at one path, referenced not restated; D2 one intake
channel and one commitment; D3 a register with the errata register's
discipline, valid while empty; D4 a mechanical dependency check; D5 the check
names its surface; D6 `RFC-v0.15-003` reclassified `Implemented-with-Errata`;
D7 three demonstrations on real runs.

---

## 1. Order

**R1 → §A, §B, §C, §E answered in writing (§D is the owner's, below) → the
process document and the channel fix → the register and its subcheck → the
dependency check → demonstrations → release-cycle integration → RFC-v0.15-003
reclassified → errata → evidence.**

**The register's subcheck lands with the register, not after it.** An empty
directory with no check is what `docs/src/security/advisories/` has effectively
been since v0.15.

## 2. §D is not yours to answer

The acknowledgement commitment — **72 hours** (release checklist) or **"a small
number of days"** (`SECURITY.md`) — is a promise to a stranger, and the owner
makes it. The architect is asking them.

**Proceed as follows:** write both documents with a single placeholder token
for that one value, land everything else, and say in the review request which
value you would choose and why. Do not pick one silently, and do not ship two.

## 3. §A — the network question, concretely

Every gate in this project is offline and deterministic; RFC-0.31-003 turned
down a network-dependent gate on exactly that ground. An advisory database is
a network resource. The RFC's three shapes are CI-only, a pinned vendored
snapshot, or both — my lean is both, with the database's commit id recorded in
the release record.

**Whatever you choose, these are the facts it must handle:**

- A cut on a day the database is unreachable needs a **stated rule** — block,
  or proceed with a recorded note. Write it, do not leave it to the day.
- A vendored snapshot **goes stale silently**, which is E-037 in a place where
  staleness means a known vulnerability stays unknown. If you vendor, the
  freshness of the snapshot is itself checked, and the check says how old it is.
- The tool is installed, not depended on: `cargo-audit` or `cargo-deny` belongs
  in a CI step or a documented local install, **not** in
  `[workspace.dependencies]`.
- Whichever tool you pick, **pin it** (`--locked --version`), the way
  RFC-0.32-001 pinned cargo-fuzz, and declare it wherever this project declares
  tool versions after RFC-0.32-003 (`toolchain-declarations` is the check that
  will want to know).

## 4. The register (D3), and the trap in it

Shape it on the errata register, which this project already trusts: an index
page listing every advisory, one file per advisory, required fields as the RFC
template gives them (id, severity, reported, disclosed, affected, fixed-in,
reporter, description, threat ref, mitigation, references, reproducer).

**The subcheck must pass on an empty register and fail on a malformed one.**
That is the whole design. Concretely, it fails when:

- a record is missing a required field, or has one it does not recognise;
- two records share an id, or ids skip a number in a year;
- the index lists a record that does not exist, or a record is missing from the
  index;
- a record claims `Fixed in: vX` for a version that was never tagged.

The last one is the one to get right: it is the only field a reader acts on.

## 5. The dependency check's report (D5)

Every run states, in its own output:

1. how many packages it covered, from `Cargo.lock`;
2. **that the published crates carry no third-party dependencies**, with the
   command that shows it;
3. the database identity (commit id or snapshot date).

Without (2), a green run reads as a statement about the shipped kernel. It is
not: it is a statement about tools, tests, benchmarks and the
development-grade crypto crate.

## 6. D7 — three demonstrations, on real runs

1. **A malformed advisory record** — a missing field, then a duplicate id. The
   subcheck names the file and the field. Revert.
2. **The empty register passes** — this is the control for (1), and it is the
   state the tree actually ships in.
3. **The dependency check goes red** against a crate with a published advisory:
   in a **scratch clone** under `.git-exclude/tmp/`, add a dependency whose
   version the database flags, run the check, quote the RUSTSEC id it reports,
   and throw the clone away. **Do not commit that dependency to the tracked
   tree**, not even briefly.

## 7. Prohibited shortcuts

- **Do not write a first advisory.** The register ships empty. An invented
  FSAD-2026-001 is a fabricated security record, and it would outlive you in
  the repository.
- **Do not fill the placeholder with a second address.** D2 is one channel: the
  one `SECURITY.md` publishes and the project can serve.
- Do not restate the process in the release checklist — reference it (D1); the
  two existing copies already disagree, which is how this erratum reads today.
- Do not let the dependency check's output claim more than §5 allows.
- Do not vendor a database without a freshness check (§3).
- Do not add the audit tool to the workspace's dependencies.
- Do not edit dated release records to mention the new check.
- Do not run `cargo fmt --all --check` in your head.

## 8. Required evidence

1. R1 re-derived, with controls, including both published crates' dependency
   graphs, and every disagreement reported.
2. §A, §B, §C and §E answered in writing; §D proposed, not decided.
3. The process document at one path; the checklist referencing it; the
   placeholder address gone; `SECURITY.md` and the process agreeing.
4. The register, its index, and its subcheck — **with the empty-register pass
   shown** (demonstration 2).
5. Demonstration 1 (malformed, duplicate id) and demonstration 3 (a real
   RUSTSEC id, in a scratch clone), with transcripts.
6. The dependency check wired as §A answers, with its report showing all three
   items from §5, and a CI run id if it runs there.
7. The release cycle stating when both are read and what a red check does to a
   cut.
8. `RFC-v0.15-003` reclassified, its two artefacts named.
9. E-051 CLOSED or survivors named; register and `v1-limitations.md` together.
10. `release-rehearsal`; `consistency-check --all` **by exit status**; `cargo
    fmt --all --check`; a CI run id.

## 9. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §A answer and the unreachable-database rule** — first.
- **The empty-register pass**, and what the subcheck would have said about a
  malformed first record.
- **The RUSTSEC id** your demonstration produced.
- **Your §D proposal**, with the reason — the owner decides, and a maintainer's
  honest estimate is the useful input.
- Anything in `SECURITY.md` or the checklist that turned out to be wrong beyond
  the two findings in the RFC.
