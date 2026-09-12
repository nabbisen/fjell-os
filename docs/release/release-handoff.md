# Release Handoff — the standing instruction for cutting a release

**Audience:** implementation model
**Governing document:** [`docs/src/release/v0-release-cycle.md`](../src/release/v0-release-cycle.md)
**Status:** standing — this document is the handoff for **every** cut, not one of them.

This handoff directs execution. It does not redefine the cycle. If you find a
conflict between this document and the cycle, **stop and escalate** — do not
resolve it in a commit.

---

## 0. Why this document exists

Every RFC in this project gets a handoff, written by the architect and handed
to the implementer. **The release cut had none**, so it was done by whoever was
holding it — the architect, at five consecutive releases (`0.25.0` through
`0.29.0`, every record reading *"Prepared by: architect"*), against a Roles
table that assigns it to you:

| Step | Owner | Architect | Implementer |
|---|---|---|---|
| Verify exit criteria | I | A | **R** |
| Produce the release record | I | C | **R** |
| Apply the tag | A | C | **R** |

**RACI legend** — `R` Responsible (does the work), `A` Accountable (answers for
the outcome), `C` Consulted (asked before it lands), `I` Informed (told after).
The cycle document used this table for five releases without ever defining its
key; that is recorded as part of **E-039** and the legend is now stated here
and there.

**The cost of the drift was not tidiness.** Every change you make is reviewed
before it lands. Changes made at a cut landed unreviewed, and they were not
defect-free — `0.27.0` was pushed broken twice, `0.28.0` shipped a tree that
turned `consistency-check` red in any fresh clone, `0.29.0` corrected two
documents with nobody checking the corrections. **You are being handed this so
the cut gets reviewed like everything else.**

## 0.1 The traps, each named by the release that found it

Read all six before starting. Every one of them produced a real, recorded
failure, and four of them **look like success at the moment they happen**.

1. **Two version strings, not one.** `[workspace.package] version` in the root
   `Cargo.toml`, and `crates/fjell-os/Cargo.toml`'s `fjell-abi = { path = ...,
   version = "<same>" }`. Cargo cannot inherit the workspace version into a
   dependency requirement, so the second is bumped by hand. **If they disagree
   the workspace does not resolve at all**, exit criterion 1 fails, and every
   other criterion becomes unevaluable. *(0.27.0; E-030.)*

2. **Rebuild before re-recording the repro baseline, not after.** Order:
   **bump → `cargo xtask build` → delete and re-record → verify the diff.**
   Recording first captures the *previous* version's committed binaries and
   produces an **empty `git diff`** — which reads as a clean result and is
   actually the failure. *(0.27.0.)*

3. **A non-zero `Added` in the ABI snapshot is now a finding, not cut work.**
   Since RFC-0.30-002, `fjell-abi-snapshot --verify` fails on `Added != 0`, so
   an ABI-adding line cannot merge without having already regenerated. If you
   see `Added: N` here, **the enforcement failed upstream** — report it, do not
   quietly regenerate. *(E-035.)*

4. **Moving every RFC to `done/` empties `rfcs/accepted/`, and git drops empty
   directories from every fresh clone.** `consistency-check` then fails for
   anyone who clones. Each lifecycle folder holds a keeper `README.md` for
   exactly this reason — **do not move or delete them**, and if a folder would
   end the cut empty, confirm its keeper is still there. *(0.28.0; E-038.)*

5. **Never `git add -A`.** Stage explicit paths, always. A single `git add -A`
   at `78d8ea3` swept 44 unrelated files into a release commit. *(E-015's
   neighbourhood; the `.gitignore` rule that should have caught it did not
   exist, despite the CHANGELOG claiming for two versions that it did.)*

6. **Verify what you staged, not what you meant to stage.** Check both
   `git status --short` and `git diff --cached --name-only` before every commit.
   At `0.27.0` a `git add` named a path that `git mv` had already moved: it
   aborted on the pathspec, staged nothing, and the acceptance took four commits
   to repair. *(0.27.0.)*

## 0.2 Settled — do not re-open

- **The tag carries no `v` prefix.** `0.30.0`, never `v0.30.0` (Rust crate
  convention). A `/v.../` URL 404s.
- **`docs/release/trust-report.txt` is committed at a cut.** Mid-line it is
  regenerated incidentally and reverted; **at a cut the regenerated file is the
  artifact** and goes in. This is the one place that rule inverts.
- **You do not apply the tag or publish.** You prepare everything and stop. The
  tag and `cargo publish` stay with the owner and architect — publishing is
  irreversible and `v1.0.0` specifically is under owner publication control
  (DEC-002).
- **Criterion 8 is not yours** (see §2). Do not "fix" `v1-limitations.md` or
  `standards-mapping.md` to make a gate pass.

## 1. Order

The sequence is load-bearing — trap 2 exists because it was done in a different
order once.

1. **Entry criteria** — tree clean, RFCs reviewed, no open finding.
2. **Bump both version strings** (trap 1).
3. **Pin the three crates.io logo URLs** to this release's tag, in the
   version-bump commit (§3).
4. **`cargo xtask build`.**
5. **Delete and re-record the repro baseline**, then verify the diff names only
   the binaries the version bump actually moved (trap 2).
6. **ABI snapshot `--verify`** — expect `Added: 0` (trap 3).
7. **Move this milestone's RFCs** `accepted/` → `done/`, `Status:` updated,
   `rfcs/README.md` counts updated, keepers left in place (trap 4).
8. **CHANGELOG entry**, dated, under the version being released.
9. **Exit criteria 1-7**, capturing real command output for the record.
10. **Regenerate and commit `trust-report.txt`** (§0.2).
11. **Release record** at `docs/release/records/<version>.md` (§4).
12. **Re-run exit criterion 6 after the record is committed.** The record's
    own commit is what makes the milestone count as *shipped* to
    `errata-tracking`, so an erratum tracked to this milestone and still
    `ACCEPTED` turns Gate 12 red only *after* step 11. A gate run taken at
    step 9 cannot see that. *(Found by the implementer at the 0.30.0 cut —
    the first executed from this document — where the clean-clone check
    caught it instead. If it fires, report it; do not close the erratum
    yourself.)*
13. **Read the release commit's CI run** — `gh run list --workflow ci.yml` for
    the run whose `headSha` is the release commit, then `gh run view <id>
    --json jobs` for the per-job conclusions. Both go in the record (§4.10).
    A red job **blocks the tag** or takes an accepted-risk statement under
    the existing rule (§4.7); it is not noted and walked past. *(RFC-0.31-002
    D7. Until 2026-09-12 nothing in this cycle read CI at all, and nothing
    noticed that no CI job had ever built the product — E-041. The gates
    stay local and that is deliberate; this step records CI, it does not
    defer to it.)* **Read the run, not the badge, and not `ci.yml`.**
14. **Read how far behind the toolchain pin is** — `rustup check` (or
    `curl -s https://static.rust-lang.org/dist/channel-rust-stable.toml
    | grep -m1 version`) against `rust-toolchain.toml`'s `channel`. Record
    the pin, current stable, and the gap in minor versions (§4.11).
    **More than three minor versions behind blocks the tag**, or takes an
    accepted-risk statement under the existing rule (§4.7). *(RFC-0.31-003
    §7. The pin removed E-037's silent drift and replaced it with silent
    staleness; this is what makes staleness cost a decision instead of
    nothing. The project reached ten months and seven minor versions behind
    without anyone deciding to be.)* **This is an observation recorded at a
    moment, not a gate** — `consistency-check` stays a pure function of the
    committed tree, so that re-running it on an old tree still answers the
    question it answered then.
15. **Clean-clone check** — clone the committed tree into a scratch directory
    and run `consistency-check --all` there. This is what caught E-038, and no
    procedure required it at the time. It is required now.
16. **Stop.** Hand over for review; do not tag.

## 2. What stays with the architect, and why you must not do it

**Exit criterion 8 — reading `docs/release/v1-limitations.md` and
`docs/compliance/standards-mapping.md` by hand against this release's actual
changes.**

Gate 12's `standards-mapping` subcheck confirms every cited path still exists.
It does **not** confirm the cited artifact still supports the row's claim — a
path that resolves is not a row that is still true, and the subcheck's own
module doc says so. The same limit applies to `tests/evidence/`: the `evidence`
subcheck confirms a citation resolves to a file with well-formed provenance
whose commit is a real ancestor of `HEAD`; it cannot confirm the provenance is
*honest* or that the citing document's reading of the log is correct.

That judgement is the architect's and it happens on your submission, not
before it. **If you believe a row is false, say so in the review request** —
that is exactly the finding worth having. Do not edit the row.

## 3. The crates.io logo URLs

Three URLs are pinned to `main` between releases and must point at this
release's tag **in the version-bump commit, before the tag exists**:

```
crates/fjell-os/README.md:2      the crates.io landing-page banner
crates/fjell-os/src/lib.rs:5     html_logo_url
crates/fjell-os/src/lib.rs:6     html_favicon_url
```

Rewrite `/main/` to `/<version>/`. crates.io renders a version's README forever
but re-fetches the image every time, so a `main`-pinned URL lets an old
release's page change appearance — or 404 — whenever the logo moves.

**Nothing mechanical checks these three.** They are a live instance of the
E-014 literal-predicate family, disclosed rather than fixed.

## 4. Required evidence

The release record at `docs/release/records/<version>.md` carries:

1. The **entry-criteria** check.
2. The **exit-criteria table**, with real command output for each row — not
   "PASS", the output.
3. The **full `release-rehearsal` gate table**, all 12 rows.
4. **`test-all`**, with the tier count and run id.
5. **What shipped** — one section per RFC in the milestone.
6. **Known limitations at this release.**
7. An **accepted-risk statement if and only if** a gate that should have
   blocked did not pass and the owner explicitly accepted the risk. **Never
   write one to paper over a red gate.**
8. The **repro-baseline diff**, showing which binaries moved and why that is
   the expected set.
9. The **clean-clone check** result (§1 step 15), and the post-record re-run of
   criterion 6 (§1 step 12).
10. **The release commit's CI run** (§1 step 13): the **run id**, and a
    per-job conclusion table — every job, named, with its conclusion. Not a
    summary, not a badge, and not a sentence containing the words "runs in
    CI". If a job is red, say which and why, and either the tag is blocked or
    item 7 applies.
11. **Toolchain currency** (§1 step 14): the pinned version, current stable,
    and the gap in minor versions, with the command that produced it. Beyond
    three, either the tag is blocked or item 7 applies — and an accepted-risk
    statement here says *why staying behind is the right call for this
    release*, not that nobody got to it.

Follow the shape of [`records/0.29.0.md`](records/0.29.0.md); it is the most
recent and the most complete.

## 5. Prohibited shortcuts

- Do not tag, push a tag, or publish.
- Do not `git add -A`.
- Do not edit `v1-limitations.md` or `standards-mapping.md` under criterion 8.
- Do not regenerate the ABI snapshot to clear a non-zero `Added` (trap 3).
- Do not delete a lifecycle folder's keeper `README.md` (trap 4).
- Do not write an accepted-risk statement (§4.7) on your own judgement.
- Do not reschedule a slipped erratum by changing its date. If
  `errata-tracking` refuses the cut, **report it** — it has correctly refused
  two cuts already, and both times the answer was to reschedule with the slip
  recorded, not to move the date.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Anything that looked like success and you checked anyway** — traps 2, 3 and
  6 all read as clean at the moment they go wrong.
- **Any row in `v1-limitations.md` or `standards-mapping.md` you believe is no
  longer true** (§2).
- **Any count or figure you re-derived and found different** from what a
  document claims.
- The repro-baseline diff, and why exactly that set of binaries moved.
