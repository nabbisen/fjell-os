# RFC-0.49-003: The next Pages failure yields evidence — and what I did and did not do of the small items

**Status:** Proposed
**Milestone:** 0.49
**Tracks.** Handoff small items **A** (instrument `ci-pages`), **B** (the clippy gate) and **C** (the readiness obligation figure, owner-gated).
**Errata:** **E-133** is the subject — a record of two misdiagnoses and one unrelated fix, with its residue in `v1-limitations.md`. This RFC adds instrumentation and leaves that disposition as it is. No new erratum.
**Touches:** `.github/scripts/pages-artifact-probe.sh` (new), `.github/workflows/ci.yml` (`ci-pages`: one permission, one step id,
two steps). **No source, kernel, ABI or prebuilt change.**
**Not in this RFC:** **a fix for E-133** (the handoff says "do not attempt a fix", and three theories have been wrong); small item
**C** (the number is the owner's; untouched).

---

## Small item A — instrument `ci-pages` (implemented in the stage-1 commit)

**What it is.** One read-only script and two steps in `ci-pages`. Immediately **before** `deploy-pages`, and again **after** it
(`if: always()`), the job logs what the GitHub API says this run's Pages artifact is:

* the run, attempt, event and sha; the **`artifact_id` that `upload-pages-artifact` reported it finalised**;
* `GET /repos/{r}/actions/runs/{run}/artifacts` — `total_count`, and per artifact `id`, `name`, `size`, `expired`, `created`, `updated`,
  the **run it is attributed to**, and its digest;
* `GET /repos/{r}/actions/artifacts?name=github-pages&per_page=5` — the same fields for the repository's newest `github-pages`
  artifacts, **so an artifact attributed to a different run, a different attempt or already expired is visible**, which is what
  "`Found 0 artifact(s)`" cannot distinguish from "never uploaded".

The two listings are the whole instrument. The *before* listing says what existed when `deploy-pages` was about to look; the *after*
listing says whether it exists once the deploy has failed. **If the artifact the upload step named is in both, the failure is on
`deploy-pages`'s side of the API; if it is in neither, it is on the upload's; if it is in *before* and not *after*, something
removed or re-attributed it.** That table is the thing three theories were missing, and it is written down here so it is not
re-derived from a log at the next failure.

**It cannot change the outcome**, by construction and by test:

* the script ends `exit 0` whatever happens (`set +e`, `timeout 60` on each request, no `gh`, a 403, unparsable JSON);
* both steps are `continue-on-error: true`, so even a runner fault in the step does not fail the job;
* it makes two `GET` requests and writes nothing.

**Run locally against a stub `gh`** (the real API was not called: the repository's own identifier would have gone to an outside
service, and a stub is what the test needs): documented JSON shape → both listings in the format above, exit 0; `gh` answering HTTP 403
→ the error text logged, exit 0; `gh` absent from `PATH` → exit 127 logged, exit 0. **What was *not* demonstrated, stated plainly:**
a live GitHub run; that `steps.upload.outputs.artifact_id` is the output name `actions/upload-pages-artifact@v5` publishes (an
absent output reads as `<none>`, which is itself information, and the script says so); and that `actions: read` is sufficient for
the second request on this repository's settings (a 403 is logged, not hidden).

**The one cost, and it is a permission.** `ci-pages` declares `pages: write` and `id-token: write`; a job that names any permission
gets none it does not name, so listing artifacts needs `actions: read` **added to that job**. It is read-only and widens nothing a
step can write, but it is a change to a job whose token deploys the site and I am saying so rather than leaving it in a diff.
**`release.yml`'s `verify` job needed the same grant for the same reason** (0.48 ruled it acceptable); this is the second use of
the shape.

### The question the handoff asked me to raise and not decide

*Should a documentation deploy be able to hold a release at all?* The facts, from the cycle document:

* **Criterion 9 is read from the run, "never from anything else"**: *every job* of the release commit's `ci.yml` run, in a per-job
  table in the release record; **"a red job blocks the tag or takes an accepted-risk statement under the existing rule."**
* **`ci-pages` is one of those jobs** and fails intermittently for a reason nobody has established (five observations: **0.44.0, 0.46.0, and three at the 0.48.0 cut**). Its remedy is a job re-run, which the action's own error text prescribes.
* **Nothing about the product depends on it.** The book is built by `ci-docs` (which `ci-pages` `needs`), rebuilt and checked for its
  ADR pages *inside* `ci-pages` before the upload, and the step that fails is only the last one — **the deploy**.

Three shapes, none recommended by me, because the choice is about what a release is allowed to wait for:
**(1) leave it** — a red `ci-pages` blocks until a re-run, and criterion 9 is exactly as strict as it was. **(2) exclude
`ci-pages` from criterion 9's *blocking* set but keep it in the per-job table**, with a requirement that the site be re-deployed
before the next milestone's first commit. **(3) mark the deploy step non-blocking in `ci.yml`**, which changes what "CI is green"
*means* to every reader of the badge and removes the signal that told the project the site was stale. The instrumented failure
decides which of these is *needed*: if the evidence shows the failure is GitHub's, (2) is cheap and honest; if it shows something in the
project's own upload, the right answer is none of the three.

---

## Small item B — the clippy gate: **not done, and the reason is a measurement**

The handoff makes this the weakest item and droppable, and says: *if done, gate `--lib` for the tool crates only, where a clean
baseline already exists.* I measured the tool crates, per crate, before deciding:

| `tools/` crate | `--lib` | default targets (lib + bins), `-D warnings` |
|---|---|---|
| `fjell-consistency-check` | **clean** | **31 locations in 12 files** (all in the bin target; the all-targets count reported at 0.48 was 32 in 13) |
| `fjell-repo-scope`, `fjell-rust-scan` | **clean** | **clean** |
| `fjell-unsafe-audit`, `fjell-readiness-check` | **no lib target** | **clean** |
| `fjell-abi-snapshot` | no lib target | **8 locations in 2 files** |
| `fjell-mmio-audit` | no lib target | 3 in 1 file |
| `fjell-repro-check`, `fjell-summary-check` | no lib target | 1 each |

**So "`--lib` for the tool crates" is a gate over three crates**, because six of the nine tool crates are **bin-only**; it would be
green and would say nothing about the code that actually runs the gates. A gate over *default targets* is red today with **44
locations** (path dependencies may be counted under more than one crate). I did **not** add a gate, and I did **not** clean the 44,
because **neither is what the handoff asked for and both would spend this milestone's attention on the one item its author
calls droppable, beside a line about a node that cannot stop a hung service.** **If you want the gate, this is its measured
shape, and the choice is `--lib` over three crates (cheap, near-meaningless) or `-D warnings` over all targets of the tool crates after 44
mechanical fixes (meaningful, and not trivial).** The workspace-wide and `no_std` baselines remain unmeasured.

## Small item C — untouched

The readiness row's obligation figure is **owner-gated, a fifth milestone**; I did not run `verus-count` to change anything and did not
read the matrix for the purpose. The handoff's figures (19 / 26 / 12 / 17 against a promised 20) are the architect's.

---

## Reproduce

```
bash .github/scripts/pages-artifact-probe.sh before-deploy        # with GITHUB_* set and a stub gh on PATH; exits 0
for c in $(ls tools); do n=$(grep -m1 '^name' tools/$c/Cargo.toml | cut -d'"' -f2); cargo clippy -q -p $n -- -D warnings; done
```
