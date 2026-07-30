# Review Record — RFC-v0.21.3-001, Slices 2b & 2c, and Finding C

**Reviewer:** architect
**Reviewing:** [review-request-slice-2b-2c.md](./review-request-slice-2b-2c.md)
**Commits reviewed:** `e54a700` (Slice 2b), `c3e24f1` (Slice 2c)
**Date:** 2026-07-30

## Outcome

**Approved.** Both slices are accepted as executed. Every claim was
independently re-derived and held.

Finding C is ruled on in §4. **I am reversing my own prior instruction**: §4.3
is unblocked and Slice 3 proceeds in full. Reasoning in §4.2 — the block was
based on a property I had not checked, and checking it removed the basis for
the block.

## 1. What was verified independently

| Claim | Method | Result |
|---|---|---|
| Slice 2b is comment-only | read the full diff at `-U2` | **Confirmed** — 4 comment lines relocated, zero compiled tokens changed |
| Gate 2 restored | `fjell-unsafe-audit` at HEAD | **Confirmed** — 274/274, 0 missing |
| Fix is fmt-stable | `cargo fmt --all --check` at HEAD | **Confirmed** — exit 0, CI `ci-format` green |
| Slice 2c is the authorized change only | read the full diff | **Confirmed** — 2 paths + `Write` import, nothing else |
| Gate 4 green | `fjell-abi-snapshot --verify` at HEAD | **Confirmed** — 404/404, Added 0, Removed 0, Changed sig 0, PASS |
| Regeneration laundered nothing | generated a snapshot at `f3519dc` + path fix, compared item sets against the committed regenerated snapshot | **Confirmed** — 404 vs 404, **0 items only in either side**, only sig hashes differ |

The last row is the one that mattered. The risk in authorizing a 178-signature
regeneration was that a genuine item change could ride along inside it. It did
not: the pre-fmt and post-regeneration item-name sets are *identical*. No
addition, removal, or rename passed through.

The request's independent re-derivation of my own §2 provenance claim (isolated
worktree, path fix only, `Changed sig: 0`) is confirmed and was the right
instinct. Reviewers are not exempt from being checked.

## 2. Note on a number that looks like a discrepancy and is not

The request reports 163 changed signatures; I measure 178. Both are correct —
different comparison sets. 163 is *old 401-item baseline* vs post-fmt tree; 178
is *pre-fmt tree* vs post-regeneration snapshot across all 404 items, which
includes the 27 items in `fjell-audit-format` / `fjell-bundle-format` that the
stale paths had made invisible to the older comparison. No action.

## 3. Correction to the Finding C characterization

The unexplained set is **9 binaries, not 11.**

`a02f4a9` ("Audit: RFC compliance, dead code, test/doc alignment") landed
*after* `a5b5167`, which was the last commit to rebuild and commit
`prebuilt/*.bin`. `a02f4a9` changed source in exactly four places:
`fjell-kernel`, `fjell-fleet-format`, `fjell-diagnosticsd`, `fjell-syncd`.

So `fjell-diagnosticsd.bin` and `fjell-syncd.bin` **should** have changed on
rebuild — their committed binaries were genuinely stale against source. That is
correct behaviour, not non-determinism, and it is a benign finding that belongs
in the record separately.

The remaining 9 — `attestd`, `devmgr`, `init`, `measuredd`, `neg-test`,
`secure-transportd`, `storaged`, `upgraded`, `verifyd` — have had no source
change since `a5b5167` and still rebuilt to different bytes, while 17 others
rebuilt byte-identically. That core anomaly stands, and the request's
byte-level analysis of it (clustered non-random deltas in instruction-adjacent
content, not a metadata blob) is sound and well-evidenced.

## 4. Ruling on Finding C

### 4.1 The characterization is accepted; the diagnosis is not yet decidable

"Non-deterministic layout or codegen-unit ordering" is a reasonable reading of
the byte evidence, but it is not the only one, and the request's own framing
("expected non-reproducibility **across environments**") is the competing
hypothesis. These have opposite implications:

- **Within-environment non-determinism** → the reproducible-build NFR is
  broken, which is a significant finding.
- **Cross-environment only** → the NFR holds as claimed and scoped, and this is
  a provenance/staleness issue about binaries built elsewhere.

One cheap experiment discriminates them, and it is the tool the project already
has: **run `cargo xtask repro-check` in default (two-build) mode.** It builds
twice in one environment and compares. If it passes, the phenomenon is
cross-environment and the NFR holds. If it fails, determinism is broken within
a single environment.

Required before Finding C is filed anywhere. Until it is run, the diagnosis is
an inference presented as a conclusion.

### 4.2 §4.3 is unblocked — reversing my prior instruction

My §6 instruction in the previous review record said not to record the repro
baseline over an unexplained delta. That was over-cautious, and it rested on an
assumption I had not verified: that the baseline's stability depends on build
determinism.

It does not. `collect_digests()` in `tools/fjell-repro-check` hashes the
**committed** `crates/fjell-kernel/prebuilt/*.bin` (`target/` entries are
explicitly stripped in `--skip-build` mode). Those files change only when
someone deliberately rebuilds and commits them. So:

- The baseline is stable under ordinary work regardless of build determinism.
- Recording it now records the *correct* state — Slice 1 rebuilt those binaries
  from current source, which also fixed the two genuinely stale ones.
- Build non-determinism affects the *two-build* mode and the cost of
  regenerating prebuilt binaries. It does not make the `--skip-build` baseline
  wrong.

**Therefore: proceed with §4.3 in full** — the fail-closed change and the
baseline recording. The fail-closed change was always independent and correct: a
missing baseline must never auto-pass.

Record alongside it, as a known limitation: regenerating `prebuilt/*.bin`
requires re-recording the baseline, and per Finding C a rebuild may produce
different bytes even with unchanged source.

### 4.3 Disposition: v0.22, its own RFC

Not v0.21.3. A fix touches `codegen-units`, incremental compilation, or linker
input ordering — build configuration with real performance and output
consequences. That is outside a patch whose premise is "no new surface, no
behaviour change," and it is pre-existing rather than introduced here.

It warrants its **own RFC**, not a line on the v0.22 candidate list, because it
bears directly on a stated non-functional requirement (reproducible build) and
because the answer to §4.1's experiment determines its severity. Filing it
alongside the ABI gate's formatting sensitivity would understate it.

**Escalation to the owner:** if the two-build check fails, the project's
reproducible-build claim does not hold as documented, and the v1.0 limitations
and trust-report wording would need review. That is an owner-facing conclusion,
not one I will absorb. Report the two-build result and I will bring it forward
if it is negative.

## 5. Ruling on review-focus question 3 — Slice 3 scope

**Proceed with all of Slice 3: §4.1, §4.2, and §4.3.** Nothing is blocked.

## 6. Process note

The review request was placed in `.git-exclude/review-request/`, which is
gitignored, and its relative links (`../../proposed/…`, `./implementation-handoff.md`)
do not resolve from there. I have copied it to
`rfcs/handoffs/RFC-v0.21.3-001-.../review-request-slice-2b-2c.md`, alongside
the slice-1-2 request, so the review chain is tracked and self-linking.

Put future review requests in the handoff directory. An untracked review record
is not a record — it is exactly the drift this RFC exists to remove.

## 7. Items accepted without change

- Gate 10 reported as failure, not absorbed — correct, unchanged standing
  instruction.
- Cleaning gate-run side effects (`trust-report.txt`, `tests/runs/`) before
  committing — correct; those are outputs, not deliverables.
- Declining to fix Finding C without authorization — correct.
- Folding the `Write` import into 2c — done as directed.

## 8. Required next deliverables

Slice 3, complete (§4.1, §4.2, §4.3), plus:

1. The two-build `repro-check` result from §4.1 above, reported as data
   whichever way it comes out.
2. The Finding C limitation recorded alongside the §4.3 baseline.
3. `diagnosticsd` / `syncd` staleness noted in the record as explained and
   benign, so it is not re-litigated later.
