# RFC-0.48-003: A GitHub Release is published from a tag that has been checked

**Status:** Implemented (0.48.0) — accepted 2026-11-25
**Evidence:** file:.github/workflows/release.yml, file:tools/fjell-consistency-check/src/release_records.rs, test:tools/fjell-consistency-check/src/release_records.rs::the_matcher_agrees_with_the_documented_examples, test:tools/fjell-consistency-check/src/release_records.rs::the_trigger_pattern_matches_what_it_must_and_nothing_else
**Milestone:** 0.48
**Tracks.** The owner's request of 2026-11-23: *a GitHub Release is created automatically when a release tag is
pushed.* **Authorized by the owner, who overruled the architect's recommendation of a draft: the tag is already the
owner's authorization signal, so publishing from it bypasses nothing — on one condition: "the trigger should be
designed and managed carefully."** **The trigger is the deliverable.** No erratum; this adds a mechanism.
**Touches** *(indicative)*: `.github/workflows/release.yml` (new — the repository's **second** workflow),
`crates/fjell-tools/src/` (one tested subcommand), `tools/fjell-consistency-check/src/` (one subcheck),
`docs/src/releasing/v0-release-cycle.md`, `release-handoff.md`, `release-checklist.md`, and one committed public key
(D2). **No kernel, ABI or prebuilt change.**
**Relates to:** **E-130 / small item A** (the order the cut now has: re-read, tag, **then** this runs), the cut
procedure's Step 12b (the archive's signature), **E-114** (two drifting copies of one fact).
**Not in this line:** attaching the source archive (D3 — recommended against), publishing to crates.io (irreversible
and the owner's), signing anything in CI.

*Re-measured at `587778d`. The mechanism the trigger depends on — verifying a tag signature on a clean machine from
a committed public key — was run with controls (§"Measured") before this RFC claimed it.*

---

## What is true now

* `.github/workflows/` holds **one** file, `ci.yml`. It triggers on `push` to `main`, `pull_request`, `schedule` and
  `workflow_dispatch`; **its only declared permissions are `pages: write` and `id-token: write`, on the pages job
  alone** (`ci.yml:235`–`:237`). **`contents: write` would be a permission surface this repository has never had.**
* Tags are bare (`0.47.0`, never `v0.47.0`); the convention is stated at `release-handoff.md` and
  `release-checklist.md`. **105 bare tags exist, every one annotated and carrying a PGP signature** (measured with
  `git cat-file tag`); 6 further tags are `0.2.0-alpha.N`.
* **The CHANGELOG/record correspondence does not hold for history, and the trigger must not pretend it does.** Of the
  105 bare tags only **35 have a `## [<version>] — ` CHANGELOG section** and **27 have a non-empty
  `releases/<version>.md`**. The correspondence is **total from `0.21.3` onward (27 tags, 0 exceptions)** and
  absent before — the record convention did not exist. A rule that fails 70 historical tags is not a rule; the
  floor must be **derived** (D5), not written.
* There is **no committed public key** and no allowed-signers file: nothing in the tree lets a machine other than the
  owner's verify a tag.
* The tag is signed by an RSA key whose public half is 2,273 bytes; the keyring that signed it is the owner's.

### Measured — the mechanism, with controls

On a throwaway keyring (`GNUPGHOME` pointing at an empty directory), with the owner's **public** key exported and
imported — no private material touched:

| Check | Result |
|---|---|
| `git verify-tag 0.47.0`, **empty keyring** | **exit 1**, *"Can't check signature: No public key"* |
| the same, keyring holding only the exported public key | **exit 0**, *"Good signature"* |
| a tag object with one byte of its message changed (`0.47.0 — The project` → `…The PROJECT`) | **exit 1**, *"BAD signature"* |

So **a clean runner can verify the tag from a committed key, refuses the absence of the key, and refuses a tampered
tag.** What it cannot do is establish that the *key is the owner's* — that is the trust anchor, and D2 is about it.

---

## The shape — two jobs, one tested tool

```
on:
  push:
    tags: ['[0-9]+.[0-9]+.[0-9]+']     # bare, three components: 0.48.0. Not v0.48.0. Not 0.2.0-alpha.1.

jobs:
  verify:   # permissions: contents: read, actions: read  -- every precondition; publishes nothing
  publish:  # permissions: contents: write     -- needs: verify; creates the Release
```

* **Deviation from the handoff, accepted at the stage-2 review: `verify` also holds `actions: read`.** The handoff's
  grant for `verify` was `contents: read`. Precondition 7 (the `ci.yml` push run for the tagged commit is green) asks the
  API for that run (`gh run list`), and listing workflow runs needs `actions: read`; without it the call is refused and
  the precondition could only fail closed. The scope is read-only, so `verify` still cannot write anything, and
  `contents: write` remains in exactly one job.

* **`contents: write` exists in exactly one job**, the last, which runs only if the first passed. The first job can
  fail without there being anything to clean up.
* **The preconditions are one subcommand — `cargo xtask release-preflight <tag>` — not shell in YAML**, so each is a
  pure function with fixtures, shown red locally, and runnable by the owner *before* tagging (a dry run of the thing
  that will judge the tag). A precondition written as inline `bash` could be shown red only by pushing a bad tag.
* **No third-party action creates the release.** `gh release create`, which the runner ships, with `--verify-tag`.
  The only actions are `actions/checkout` and the repository's own toolchain action; **both pinned by commit SHA
  in this workflow** (D6), because this is the first workflow whose token can write.
* **A `v`-prefixed tag does not trigger it.** That is the point of the convention and it is **silent** — which is why
  the gate (R7) fails on a `v*` tag existing at all, rather than relying on the absence of a Release.

## The preconditions — each **fails** the job; none publishes around a failure

| # | Precondition | How | Judged by |
|---|---|---|---|
| 1 | tag name = workspace version **at the tagged commit** | read `Cargo.toml` `[workspace.package].version` from the tag's tree (`git show <tag>:Cargo.toml`), not the checkout | pure function over two strings |
| 2 | the tagged commit is an **ancestor of `main`** | `git merge-base --is-ancestor <tag> origin/main` (full-depth fetch) | `git` |
| 3 | `CHANGELOG.md` **at the tagged commit** has `## [<version>] — ` | read from the tag's tree | pure function over text |
| 4 | `releases/<version>.md` **at the tagged commit** exists and is non-empty | read from the tag's tree | pure function |
| 5 | the tag is an **annotated object**, carries a signature, and **`git verify-tag` passes against the committed key, whose fingerprint equals the pinned one** | throwaway `GNUPGHOME`; import `docs/src/releasing/release-signing-key.asc`; compare fingerprint to `release-signing-key.fingerprint`; `git verify-tag` | `git`/`gpg`, exit status |
| 6 | **no Release already exists for the tag** | `gh release view <tag>` must fail "not found" | `gh` — a re-run neither duplicates nor overwrites |
| 7 | **CI on the tagged commit concluded `success`** | `gh api repos/…/commits/<sha>/check-runs`; every run `success` or `skipped`, none missing the required jobs | `gh` (D4) |

Reading 1, 3 and 4 **from the tag's tree** is deliberate: reading the checkout of `main` would let a later commit
satisfy a tag that its own commit does not.

### Notes — extracted, never generated, and never copied

* The Release body is **the CHANGELOG section for that exact version** (from its `## [<version>] — ` line to the next
  `## [`), extracted by the same function that precondition 3 uses, **plus one line linking
  `releases/<version>.md` at the tag** (`…/blob/<tag>/releases/<version>.md`). **GitHub's generated commit list is
  never used.** The release record is **linked, not copied**: two drifting copies of one fact is the E-114 family, and
  the record is long, hand-written and the architect's.
* An **empty extracted section fails the job** — a heading with nothing beneath it satisfies precondition 3 and would
  publish a blank Release; the extraction function requires a non-empty body and a test pins it.

## The decisions this RFC asks for

**D1 — a Release, not a draft; prerelease never.** The owner ruled it. Recorded here so the workflow does not grow a
`draft: true` "for safety" and so a reviewer sees it was a choice. **The compensating control is that every
precondition refuses**, because deleting a Release leaves the tag behind and a Release is outward-facing.

**D2 — the trust anchor for precondition 5. RULED: (a), as the owner's act — commit the owner's *public* key and pin its fingerprint.**
Three options:

* **(a) a committed public key + a pinned fingerprint file** *(recommended)* — verified on a clean runner with
  nothing but `git` and `gpg`, as measured above. The anchor is **reviewed in a commit and visible in the tree**;
  rotating the key is a commit, which is the project's pattern for every other trust root (`TOOLCHAIN.lock`,
  `baseline-digests.txt`). **Cost:** a new committed key — the owner's to export and the architect's to review,
  **not mine to generate or commit** — and a procedure for rotation.
* (b) trust GitHub's own verdict — `git/tags/<sha>`'s `verification.verified`. No key to commit, but the anchor is
  *GitHub's copy of a key uploaded to an account*, outside the tree, unreviewable by a diff, and **it makes the
  project's release authorization depend on a third party's reading of a signature.**
* (c) no verification — the preconditions minus #5. Rejected: an unsigned tag pushed by anyone with write access
  would publish. **The owner's authorization signal is only a signal if it is checked.**

**D3 — assets. RULED: none — notes only.** The handoff's rule — *"if it attaches `fjell-os-v{version}.tar.gz`, attach the
signature and digest and **verify them**"* — has a consequence I measured the cost of: **Step 12b signs the archive
*after* the tag, with the offline release key.** At the moment a tag push triggers this workflow **the signature does
not exist and cannot**: the workflow could attach an unsigned archive (an unverifiable artefact beside a trust
report, which the handoff names as the defect), or attach nothing. **Recommended: the workflow publishes notes only.**
The archive and signature attach as a **cut step** (`gh release upload <tag> fjell-os-v<v>.tar.gz{,.sig,.sha256}`
after Step 12b), **verified by the same `verify-bundle-sig` run the checklist already prescribes**, and *that* step
gets a verbatim run with controls when it is written into the checklist. If the owner wants assets automated, the
signature must be producible in CI, which is a different trust model (a CI-held signing key) and a different RFC.
**Written here so nobody tries it later as an obvious improvement:** *automating the assets requires a signing key that CI can use, which means the release key is no longer offline; that trades the property Step 12b exists to keep for a convenience, and it is not an improvement.*

**D4 — precondition 7 (CI green on the commit). RULED: required, with a bounded wait that fails on timeout; default 30 minutes, a tunable the owner may change** (0.47.0's queue sat about ten minutes, so 30 is slack without being an eternity). **A timeout fails; it never publishes.** The cut already reads CI before tagging
(step 13); this makes the *Release* refuse a tag whose commit CI did not pass, so an owner who tags early cannot
publish a red build by accident. **Cost:** a tag pushed *seconds* after the commit has no finished CI run, so the job
must **wait with a bounded timeout and then fail**, not pass on absence; the timeout is `RELEASE_CI_WAIT_MINUTES`, default **30**, a named constant in the subcommand and a workflow `env:` the
owner may change. **Alternative: omit it** and rely on step 13. The failure
mode of omitting it is one human slip, which is the kind this milestone's theme is about.

**D5 — the correspondence gate, split by what a gate may depend on. RULED on the project's own written rule** (`release-handoff.md`: *"`consistency-check` stays a pure function of the committed tree, so that re-running it on an old tree still answers the same question"*). `consistency-check` is *a pure function of the
committed tree* (so re-running on an old tree answers what it answered then). **Tags are not in the tree**, and at the
moment Gate 12 runs in the cut **the new tag does not exist yet.** So:

* **`release-records` (new subcheck, tree-pure, in Gate 12):** (i) every `releases/<v>.md` has a CHANGELOG section
  `## [<v>] — `; (ii) every CHANGELOG section for a version **strictly below the workspace version** and **at or above
  the floor** has a non-empty record; **the floor is the lowest version that has a record — derived from the
  directory, not written** (today `0.21.3`); (iii) `release.yml`'s `tags:` filter is exactly the bare three-component
  pattern. *(Fixtures; and the real tree: 27 records, 27 sections, floor `0.21.3`, no exception.)*
* **`release-preflight` (the subcommand, run by the workflow and by the owner beforehand):** the tag-dependent facts —
  annotated, signed and verifying, no `v*` tag exists, tag = version at that commit, ancestor of `main`. **These
  cannot be a tree check**, and the RFC says so rather than adding a subcheck that reads `.git/refs` and is
  red or green depending on the clone.

**D6 — pin actions by commit SHA in this workflow. RULED: accepted.** The existing `ci.yml` pins by major (`actions/checkout@v7`, `upload-artifact@v7`, `deploy-pages@v5` …), and
criterion 12 *reads* those pins against upstream at each cut. A workflow whose token has `contents: write` is
where a moved tag matters most. **Recommended: SHA pins with a trailing `# v7.x.y` comment**, and the criterion-12
read extended to this file (it already greps `uses:` across `.github/`). Alternative: match `ci.yml` and accept the
weaker pin.

**D7 — what happens when it fails. RULED: refuse, never publish.** A red `verify` job leaves **no Release and no half-state** (nothing in `verify`
writes). The owner's remedy is stated in the cycle document: fix the cause, **delete and re-push the tag only if the
tag itself was wrong** (a moved tag is the owner's act, in the open), and a re-run of the job is safe by precondition
6. **A failed workflow is not a failed release** — the tag, the crates and the cut stand — and the document says so,
because the first time this fires red it will look like a failed release to whoever sees the email.

## Requirements

**R1 — `release.yml`**: the trigger and two jobs above; `permissions: {}` at the workflow level and **`contents:
read` / `contents: write` per job**; `concurrency: { group: release-${{ github.ref }}, cancel-in-progress: false }`
(two pushes of one tag must not race into two Releases; the loser fails precondition 6 either way).
**R2 — `cargo xtask release-preflight <tag> [--dry-run]`**: preconditions 1–7 as pure functions over a `Facts` struct
(tag name, tree-at-tag versions and files, tag-object kind, signature verdict, ancestor verdict, release-exists,
check-runs), **with its own tests**, exiting non-zero with every failed precondition named.
**R3 — notes extraction** (`release-notes <version>`): the section-and-link text, failing on a missing or empty
section. Tested on the real CHANGELOG for every version from the floor.
**R4 — the committed key and fingerprint** (D2a) — **by the owner**; I will not generate or commit key material.
**R5 — the procedure**: the cut document gains *"after the tag, the Release is created by `release.yml`; check it
exists, and attach the archive (Step 12b) with `gh release upload`"*; the handoff's order and the checklist agree.
**R6 — `release-records`** (D5), shown red.
**R7 — the `v`-prefix rule**, enforced by `release-preflight` (a `v*` tag exists → fail) **and** stated where the
convention already is.

## Negative cases — what I will show red

1. **Each precondition, alone, against a scratch clone** (never the real repository's refs): a tag whose name differs
   from the version at its commit; a tag on a commit not reachable from `main`; a tag whose commit has no CHANGELOG
   section; one with an **empty** section; no record; a **lightweight** tag; an annotated **unsigned** tag; a signed tag
   with **the wrong key**; a **tampered** tag (the measured case); a tag for which a Release already exists; a commit
   whose CI is red. **Eleven named failures, each quoted.**
2. **The trigger itself:** the `tags:` pattern evaluated against `0.48.0` (matches), `v0.48.0`, `0.2.0-alpha.1` and
   `0.48` (do not) — with a small matcher that implements GitHub's documented filter grammar, **and a test that the
   matcher agrees with the documented examples**, because a trigger that silently never fires is the failure mode
   this line is most exposed to.
3. **`release-records` on the real tree and on fixtures:** a record with no CHANGELOG section; a section below the
   workspace version with no record; the filter changed to `v*`.
4. **Permissions:** a test (a parse of the YAML) that `contents: write` appears **exactly once**, in a job that
   `needs:` the verify job; adding it to the workflow level or the verify job turns the gate red.
5. **Mutations, each a named failure** (compile error is INVALID): precondition 5 accepts an unsigned tag;
   precondition 6 removed; the notes extractor reading the checkout instead of the tag's tree; the pin check removed.
6. **A real first fire is the owner's.** I cannot push a tag. The RFC's honest limit: **everything except the live
   GitHub run is demonstrated**; the first real tag (0.48.0) is the live exercise, and the release record should say
   so. A `workflow_dispatch` is **not** offered to rehearse it, because a manual-trigger path to publication would
   bypass the tag as the authorization signal.

## What this RFC does not do, and why

* **It does not create a draft** (D1) **or attach assets** (D3).
* **It does not generate, store or touch a signing key.** Verification uses a *public* key the owner commits.
* **It does not make a failed Release a failed cut** (D7).
* **It does not publish to crates.io**, and nothing here can: no registry token is introduced.
* **It does not claim a Release proves anything about the artefact.** A Release is a *pointer* to a verified tag and a
  verified CHANGELOG section; the archive's own authenticity remains Step 12b's.

## Reproduce

```
ls .github/workflows                                              # ci.yml only
/usr/bin/grep -n 'permissions\|pages: write\|id-token' .github/workflows/ci.yml
for t in $(git tag -l | /usr/bin/grep -E '^[0-9]+\.[0-9]+\.[0-9]+$'); do git cat-file tag $t | /usr/bin/grep -c 'BEGIN PGP SIGNATURE'; done | sort | uniq -c
```

The CHANGELOG/record correspondence figures (35 sections, 27 records, floor `0.21.3`) and the three `git verify-tag`
controls come from throwaway scripts; the implementation commits them as `release-records`'s and `release-preflight`'s
own tests over the real tree and a scratch clone.
