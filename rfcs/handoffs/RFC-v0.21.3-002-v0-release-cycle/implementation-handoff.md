# Developer Handoff — RFC-v0.21.3-002

**Governing RFC:** [RFC-v0.21.3-002](../../proposed/RFC-v0.21.3-002-v0-release-cycle.md)
**Milestone:** v0.21.3
**Status:** inherited from the governing RFC (Proposed — both owner decisions closed 2026-07-30)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Read before starting

- The governing RFC — especially §The v0 development release cycle and the two
  **accepted** decision requests.
- `docs/release/release-checklist.md` — the v1.0 procedure you are repairing,
  not replacing.
- RFC 000 (`rfcs/done/000-rfc-lifecycle-policy.md`) — index integrity and the
  cross-reference sweep rule.

### Both owner decisions are closed

| Decision | Outcome |
|---|---|
| Archive layout | **Keep** the internal `fjell-os-v{version}/` directory. `package_release.rs` does **not** change. |
| `0.21.2` disposition | `KNOWN-BAD` — **already done**, `CHANGELOG.md`. Do not redo. |

### State you are starting from

`cargo xtask release-rehearsal` currently reports **ALL MECHANICAL GATES PASS**,
including Gate 10 (Verus certified under the newly pinned prover). Gate 9 is
manual and correctly unsigned. This is the first time the full table has been
green on evidence rather than assertion — do not disturb it.

---

## 1. Change scope

**In scope:** `docs/src/release/`, `docs/src/SUMMARY.md`,
`docs/release/release-checklist.md`, `docs/release/records/`,
`verification/verus/TOOLCHAIN.md`, and the IMP-06 wording wherever the decision
log records it.

**Explicitly NOT in scope:**

- `crates/fjell-tools/src/package_release.rs` — the archive layout was accepted
  as-is. Changing it contradicts a closed owner decision.
- Any kernel, ABI, capability, lease, IPC, or crypto code.
- `verification/verus/TOOLCHAIN.lock` and the proof sources — the pin was just
  updated under a recorded re-run. Leave both alone.
- CI enforcement of the new cycle. Explicit RFC non-goal.
- Applying the `0.21.3` tag. See §5 — that step is owner-gated.

---

## 2. Slice A — Document the v0 release cycle

Create `docs/src/release/v0-release-cycle.md` from the RFC's §The v0 development
release cycle: trigger, entry criteria, the 8 exit criteria, required artifacts,
the roles table, and the known-bad convention.

This is the **operative** document — the RFC records *why*, this records *what to
do*. Write it to be followed, not read once. Do not paraphrase the exit criteria
loosely; they are the substance.

Two points to carry over verbatim in meaning:

- **A gate that cannot run is not a passing gate.** `CONFORMANCE-ONLY` from
  Gate 10 is a failure, not an abstention.
- **Do not batch releases.** Two triggers close together means two releases.

Wire it into `docs/src/SUMMARY.md` beside the existing release entries
(currently lines 57–58, `./release/reproducibility.md` and
`./release/v1-readiness.md`).

## 3. Slice B — Repair the v1.0 checklist

In `docs/release/release-checklist.md` — repair, do not rewrite. It is
authoritative for v1.0 and stays that way.

| Line | Now | Required |
|---|---|---|
| 88 | `cargo xtask docs build` | No `docs` subcommand exists. Either use a real command or drop Step 6 and state that mdBook builds the docs directly. State which you chose and why. |
| 169 | `git tag -s v1.0.0 -m …` | Contradicts the project's bare-tag convention. Tags carry no `v` prefix (Rust crate style, owner-confirmed). Should be `1.0.0`. |
| 178 | `cargo xtask release --version v1.0.0` | Subcommand is `package-release`. |

Also: retitle so its **v1.0 scope is explicit** in the heading, not just the
opening line, and cross-link `v0-release-cycle.md` for everything below v1.0.

Verify every remaining `cargo xtask …` in that file against the real subcommand
list (`crates/fjell-tools/src/main.rs`). Three were wrong; check the rest rather
than assuming only three.

## 4. Slice C — State the archive convention once, and fix IMP-06

**The point of this slice is a single source of truth.** The owner's decision was
explicitly *not* "log a deviation against the generic rule" — that would leave
two documents that must be read together.

1. State the archive convention **once**, authoritatively, in the project's
   release documentation (`v0-release-cycle.md` is the natural home): the release
   archive is `fjell-os-v{version}.tar.gz` and unpacks to a single top-level
   `fjell-os-v{version}/` directory. State it as this project's convention. Do
   **not** phrase it as an exception, and do **not** cross-reference the generic
   rule.
2. **Correct the IMP-06 wording.** It currently reads *"Release archive unpacks
   to `fjell-os-v{version}/`, no nesting."* "No nesting" reads as though there is
   no parent directory, when it means "no *double* nesting" — that phrasing is
   exactly how the conflict stayed invisible. Reword so it cannot be misread.
   IMP-06 appears in the `handoff-0.21.2/` bundle's decision log; per the frozen
   bundle convention, add a correction rather than rewriting history.
3. **Fold in the stale `TOOLCHAIN.md` section.** Its "Conformance-only mode"
   states Verus absence *"does not fail the build … proofs are additive, never a
   blocker, until promoted."* They were promoted at v0.18.0, and
   `verus-check --release-required` blocks. Anyone reading it today concludes
   they do not need Verus — the opposite of the truth. Also add the install path
   that now works: AUR `verus-bin`, plus the rustc toolchain it requires.

## 5. Slice D — Cut v0.21.3

**Do not apply the tag.** Per the RFC's roles table, the owner is accountable for
the tag. Prepare everything, then hand back.

1. Verify the **entry criteria** (RFC §Entry criteria) and record the result.
2. Run the full **exit criteria** sweep at the final commit and capture real
   output. Do not reuse output from earlier in this RFC's review chain — the tree
   has changed since. Re-run everything.
3. Write the release record at **`docs/release/records/0.21.3.md`**: the
   exit-criteria table with real output, the full gate table, known limitations,
   and any accepted-risk statement.

   Gate 10 should now be **PASS**, so no accepted-risk statement is expected. If
   it is not PASS, stop and report — do not write one unilaterally.
4. Note in the record that this is the **first release cut under this cycle**,
   and that `0.21.2` is superseded and marked `KNOWN-BAD`.
5. Confirm `Cargo.toml` version is `0.21.3` and the CHANGELOG entry is dated.

Then submit for review and owner tag approval. **Stop there.**

---

## 6. Prohibited shortcuts

- Do not tag, publish, or announce anything.
- Do not write an accepted-risk statement to make a red gate acceptable.
- Do not reuse earlier gate output as evidence for this release.
- Do not touch `TOOLCHAIN.lock`, the proof sources, or `package_release.rs`.
- Do not add CI enforcement of the cycle.
- Do not mark unexecuted commands as passed.

## 7. Required evidence

Real output, not summaries:

1. `cargo metadata --no-deps` — exit code and member count
2. `cargo fmt --all --check` — exit code
3. `cargo xtask build` — result and warning count
4. `cargo xtask test-all --no-qemu` — tier table
5. `cargo xtask test-all` — full tier table including QEMU
6. `cargo xtask release-rehearsal` — full gate table
7. `cargo xtask verus-check --release-required` — exit code
8. Every link in `SUMMARY.md` and the checklist resolves after your edits

## 8. Review request format

Per the established format: implementation summary, addressed sections, changed
files by slice, important decisions, differences from this handoff, executed
commands with real output, unresolved/blocked items, known limitations, and
requested review focus.

Place it where the owner has directed (`.git-exclude/review-request/`). The
architect copies it into `rfcs/handoffs/` during review — you do not need to.

Flag for focused review: the Step 6 decision in Slice B, the wording in Slice C
item 1 (it must read as a convention, not an exception), and the completeness of
the release record.
