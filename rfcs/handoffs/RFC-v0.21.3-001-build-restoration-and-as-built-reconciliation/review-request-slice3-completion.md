# Review Request — RFC-v0.21.3-001, Slice 3 (complete) and RFC completion

**Governing RFC:** [RFC-v0.21.3-001](../../rfcs/proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md)
**Handoff:** [implementation-handoff.md](../../rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/implementation-handoff.md) §4
**Prior reviews:** [review-record-slice-1-2.md](../../rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/review-record-slice-1-2.md), [review-record-slice-2b-2c.md](../../rfcs/handoffs/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation/review-record-slice-2b-2c.md)
**Submitted by:** implementation model
**Status:** Slice 3 complete (§4.1, §4.2, §4.3) and committed. All RFC-required
evidence collected. This closes the implementation side of RFC-v0.21.3-001,
pending review.

**Placement note:** this file is in `.git-exclude/review-request/` per the
owner's direct instruction, not `rfcs/handoffs/.../`, where
review-record-slice-2b-2c.md §6 asked review requests to live (citing broken
relative links from the gitignored location). That instruction/preference
conflict is still open — see the previous review request's trailing note. I
have not resolved it; flagging again so it isn't lost. Links below use paths
relative to this file's actual location so they resolve either way you view
it.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commits this submission covers: `de2a74d`, `14ea16b`, `9e521e3`, `f3ad586`
(Slice 1/2/2b/2c were covered by the two prior review requests and are
already approved).

---

## 1. Implementation summary

Completed all three subsections of Slice 3 exactly as scoped by handoff §4,
each as its own commit: §4.1 reconciled every syscall-surface documentation
claim against the actual dispatcher (verified against source myself, not
taken from the handoff's own ground-truth table on faith); §4.2 fixed the
index/link/version drift across `docs/src/SUMMARY.md`, `rfcs/README.md`,
root `README.md`, `ROADMAP.md`, `docs/src/roadmap/roadmap.md`, and the
`handoff-0.21.2/` bundle's version stamps; §4.3 made the repro-check
baseline fail closed, ran the two-build reproducibility check the prior
review required, and recorded the resulting baseline. Added a `[0.21.3]`
CHANGELOG entry summarizing the whole release. Then ran every command in
handoff §6 against the final state.

**Result: all mechanical gates that can run in this environment now pass.**
Gate 9 (manual) is correctly untouched and unsigned. Gate 10 (Verus) fails
only because Verus is not installed here — reported honestly as a failure,
not absorbed as CONFORMANCE-ONLY.

## 2. Addressed RFC sections

- §M2 (syscall surface) — Slice 3 §4.1, commit `de2a74d`.
- §M5 (index/link/stamp drift) — Slice 3 §4.2, commit `14ea16b`.
- §M4 (repro baseline) — completed in Slice 2c-era commit `9e521e3`
  (fail-closed fix, baseline commit, two-build evidence).
- Acceptance criteria — see §6 below, mapped item by item.
- §Testing and verification requirements items 1–7 — all executed, real
  output in §6. Item 8 (mechanical syscall-count check) — not added; see §7.

## 3. Changed files, by commit

**`de2a74d` — §4.1 syscall docs (7 files):** `docs/rfcs/ERRATA.md` (new
E-011), `docs/src/abi/ipc-register-layout.md`, `docs/src/api/syscalls.md`
(full rewrite), `docs/src/external-design/{capability-lease,kernel}.md`,
`docs/src/releases/handoff-0.21.2/{external-design,project-summary}.md`.

**`14ea16b` — §4.2 index/link/version drift (15 files):** `Cargo.lock`,
`Cargo.toml` (version bump), `README.md`, `ROADMAP.md`,
`crates/fjell-kernel/prebuilt/fjell-neg-test.bin` (rebuild side effect of
the version bump — see §4 below), `docs/src/SUMMARY.md`,
`docs/src/releases/handoff-0.21.2/*.md` (6 files, version-stamp fix),
`docs/src/roadmap/roadmap.md`, `rfcs/README.md` (full rewrite). Also removed
(no git diff, untracked): `docs/src/getting-started/`, `docs/src/development/`.

**`9e521e3` — §4.3 repro-check (5 files):** `crates/fjell-tools/src/main.rs`,
`docs/src/releases/handoff-0.21.2/implementation-notes.md`,
`tests/repro/baseline-digests.txt` (new), `tools/fjell-repro-check/README.md`,
`tools/fjell-repro-check/src/main.rs`.

**`f3ad586` — CHANGELOG (1 file):** `CHANGELOG.md`.

## 4. Important implementation decisions

- **SUMMARY.md handoff-directory links: fixed the links, did not rename the
  directory.** The handoff said "prefer whichever keeps CHANGELOG 0.21.2
  truthful; state your choice." Kept `handoff-0.21.2/` as-is (matches the
  established naming pattern of the frozen historical session handoffs:
  `handoff-v0.9-v0.15.md`, `handoff-v0.17-v0.18.md`, `handoff-v0.19-v0.20.md`)
  and updated the 7 dead `SUMMARY.md` links to point at it.
- **Handoff bundle version stamps: re-stamped to v0.21.2, not v0.21.3.** The
  bundle's content (crate counts, "80 crates", DEC-005, etc.) describes the
  v0.21.2 release state; only the label said v0.21.1. Re-stamping to v0.21.3
  would have made a historical snapshot claim to describe a release it
  doesn't. Flagging because the RFC's own acceptance criteria list says "The
  handoff bundle's evidence is regenerated or **explicitly re-stamped
  against v0.21.3**" — I read that as satisfied by explicitly correcting the
  stamp to the version the content actually describes (v0.21.2), per the
  handoff table's own more specific instruction, not by relabeling frozen
  v0.21.2 content as v0.21.3. If that reading is wrong, this needs a
  correction pass, not a new decision — say which version each of the 6
  files should claim.
- **rfcs/README.md: did not add v0.19/v0.20/v0.21 sections.** No RFCs exist
  for those lines (confirmed against `rfcs/done/`'s actual file list, not
  assumed). Added a note pointing at the RFC's own §Open questions item 2
  instead of inventing placeholder sections.
- **README.md unsafe count and RFC count: re-derived, not copied from the
  handoff table.** Ran `fjell-unsafe-audit` and counted `rfcs/done/` myself
  rather than trusting the 274 / 154 figures already established earlier in
  this RFC's own review chain, in case anything had drifted since — they
  matched.
- **Left 3 more instances of the "Gate 9 is the only blocker" claim
  unedited**, in `docs/src/releases/handoff-0.21.2/{README,project-summary,
  testing-and-gates}.md`. The handoff table named only `ROADMAP.md` and
  `docs/src/roadmap/roadmap.md` for this specific correction. Same latent
  inaccuracy, deliberately left alone rather than silently expanding scope —
  flagging so it reads as a choice, not an oversight.
- **Did not add a mechanical syscall-count CI check** (testing requirement
  item 8). The RFC phrases it as "prefer... if it is cheap to add" — adding
  new CI/gate surface wasn't authorized by either review record, and a
  correct version (parsing both the ABI enum and the dispatcher match arms
  robustly) is more than a few lines. Left as prose, cross-referenced
  between `kernel.md`, `capability-lease.md`, `ipc-register-layout.md`, and
  `api/syscalls.md`, all citing the same 26/9 split.

## 5. Differences from the handoff/RFC

None beyond what's stated as a decision above. No scope was widened into the
9 syscalls, the ABI enum, or kernel/capability/lease/IPC/crypto logic.

## 6. Executed commands and real output — full §6 sweep

```
1. cargo metadata --no-deps
   exit 0, 88 members, versions: {"0.21.3"} (all 88 workspace-internal
   packages, confirming the version bump propagated cleanly)

2. cargo fmt --all --check
   exit 0

3. cargo xtask build
   exit 0, 0 warnings
   git status crates/fjell-kernel/prebuilt/ — empty (no drift from this build)

4. cargo xtask test-all --no-qemu
   Tier  Label                              Result
   1     Host library tests                 PASS
   2     Property tests (proptest)          PASS
   3     Unsafe site audit                  PASS
   4     MMIO ordering audit                PASS
   5     Reproducible build (skip-build)     PASS
   Total: 18 | PASS: 5 | FAIL: 0 | SKIP: 13 (QEMU tiers via --no-qemu)
   ALL REQUIRED TIERS PASSED

5. cargo xtask test-all  (full, with QEMU — ran in background, ~14 min wall
   clock for 13 real QEMU invocations)
   Tier   Label                          Time    Result
   1-5    (as above)                             PASS
   6-9    QEMU smoke (m8, v0.4-net,
          v0.5-platform, v0.7-sync)      ~61s ea  PASS (all 4)
   10-18  QEMU negative (capability,
          mmio, dma, user-copy, audit,
          policy, ipc, svc, harness)     60-90s   PASS (all 9)
   Total: 18 | PASS: 18 | FAIL: 0 | SKIP: 0
   ALL REQUIRED TIERS PASSED — first time every tier has actually run
   since the build was restored.

6. cargo xtask release-rehearsal
     [PASS] Gate 1  Host test suite (0 failures)
     [PASS] Gate 2  Unsafe audit (0 missing)
     [PASS] Gate 3  MMIO audit (0 missing)
     [PASS] Gate 4  ABI snapshot verify
     [PASS] Gate 5  Readiness matrix (0 OPEN)
     [PASS] Gate 6  Trust report (6 sections)
     [PASS] Gate 7  ERRATA register (0 OPEN)
     [PASS] Gate 8  Validation drills (markers)
     [ -- ] Gate 9  Release-notes limitations    MANUAL (untouched, unsigned)
     [FAIL] Gate 10 Verus release-required proofs   verus not on PATH
     [PASS] Gate 11 Callsite conformance
   RELEASE-REHEARSAL: ONE OR MORE GATES FAILED (Gate 10 only, environment)

7. Gate 10 / Verus: verus is not installed in this environment.
   verus-check falls back to conformance-only and both release-required
   targets report VERUS:TARGET:*:CONFORMANCE-ONLY. Per standing instruction
   this is reported as FAIL, not as a pass — capability conformance (6/6
   tests) and lease conformance (10/10 tests) both passed at the Rust level,
   which is not the same claim as machine-checked.

8. Gate 4 zero removals: confirmed — "Removed: 0" in every verify run this
   session, both before and after the snapshot regeneration.

9. Prebuilt-binary diff, Slice 2 (already reported and accepted in the prior
   review) — reconfirmed here as still true at HEAD: identical.
```

Side effects of these runs (`docs/release/trust-report.txt` regeneration,
`tests/runs/`, `tests/qemu/artifacts/`) were reverted/deleted after each run,
consistent with the prior two reviews' guidance that these are outputs, not
deliverables. Working tree is clean at `f3ad586` (HEAD).

## 7. Unresolved issues and blocked items

Nothing is blocked. Two items are explicitly deferred, not unresolved:

- **Finding C** (build-output non-determinism, 9 of 28 prebuilt binaries) —
  characterized and ruled on by the architect (review-record-slice-2b-2c.md
  §3-4); disposition is its own v0.22 RFC. Not this RFC's to fix.
- **The 9 declared-but-undispatched syscalls** — disposition explicitly
  deferred by the RFC itself to v0.22 (§Deferred). Documented as such
  throughout, not resolved here.

One open question carried forward from my prior submission, still
unanswered: the review-request placement conflict (`.git-exclude/review-request/`
per your instruction vs. `rfcs/handoffs/.../` per review-record-slice-2b-2c.md
§6).

## 8. Known limitations

- ERRATA E-011 (`sys_cap_install_with_rights` rights-check gap) — ACCEPTED,
  deferred to v0.22, does not affect Gate 7 (0 OPEN entries).
- Finding C — recorded in `tools/fjell-repro-check/README.md`, deferred to
  v0.22 per architect ruling.
- The ABI snapshot gate's formatting-sensitivity (recorded as a design
  trade-off in the Slice 2c commit message, per review-record-slice-1-2.md
  §4) — deferred to v0.22 candidate list, not fixed here.
- Gate 10 requires Verus on PATH to produce a machine-checked result; this
  environment does not have it installed. Not a defect introduced by this
  RFC — a pre-existing environment gap, reported honestly rather than
  worked around.
- 3 more instances of the "Gate 9 is the only blocker" claim remain
  unedited in the `handoff-0.21.2/` bundle — see §4 above.

## 9. Requested review focus

1. The handoff-bundle version-stamp decision (§4) — did I read "explicitly
   re-stamped against v0.21.3" correctly as "re-stamp to the version it
   actually describes" (v0.21.2), or does the bundle need to describe
   v0.21.3?
2. Whether RFC-v0.21.3-001 is now complete and ready for its own
   disposition (move to `rfcs/done/`?), given all three slices are approved
   or, for this submission, pending approval, and every gate that can run
   in this environment passes.
3. The review-request placement conflict — needs a ruling so it stops
   recurring in every submission.
