# Review Request — RFC-v0.21.3-002, Slices A–D (complete)

**Governing RFC:** [rfcs/proposed/RFC-v0.21.3-002-v0-release-cycle.md](../../rfcs/proposed/RFC-v0.21.3-002-v0-release-cycle.md)
**Handoff:** [implementation-handoff.md](../../rfcs/handoffs/RFC-v0.21.3-002-v0-release-cycle/implementation-handoff.md)
**Submitted by:** implementation model
**Status:** All four slices complete and committed. Tag not applied —
handed back for owner approval per the RFC's roles table.

Branch: `docs/v0.21.3-rfc-and-design-baseline`
Commits: `219e406` (Slice A), `6039eaa` (Slice B), `c8b1655` (Slice C),
`947191d` (Slice D)

---

## 1. Implementation summary

Completed all four slices exactly as scoped: documented the v0 release
cycle as the operative procedure (Slice A), repaired — not rewrote — the
v1.0 checklist's three drifted commands plus its tag-convention
contradiction (Slice B), single-sourced the archive convention and
corrected the IMP-06 and stale-Verus-claim documentation gaps (Slice C),
and prepared the first release record under the new cycle with a complete,
freshly-run evidence sweep (Slice D). Tag not applied — that step is
explicitly the owner's per the RFC's own roles table.

## 2. Addressed sections

- §The v0 development release cycle (Slice A)
- §Repairs to the existing checklist / M3, M4 (Slice B)
- §M5, Decision request 1 (Slice C)
- §Required artifacts per release, entry/exit criteria (Slice D)
- Acceptance criteria — mapped below (§6)

## 3. Changed files, by commit

**`219e406` (Slice A, 2 files):** new
`docs/src/release/v0-release-cycle.md` (self-contained canonical doc, not
a pointer stub); `docs/src/SUMMARY.md` (wired in beside the existing
release entries).

**`6039eaa` (Slice B, 1 file):** `docs/release/release-checklist.md` —
retitled + scope note + cross-link; Step 6 (real mdBook command replacing
the nonexistent `docs` subcommand); Step 11 (bare-tag convention); Step 12
(`package-release`, no `--version` flag, corrected expected output,
verified by actually running it).

**`c8b1655` (Slice C, 3 files):** `docs/src/releases/handoff-0.21.2/
{decision-log,implementation-notes}.md` (IMP-06 correction, both
occurrences); `verification/verus/TOOLCHAIN.md` (current AUR pin,
corrected conformance-only claim, retitled the now-previous-pin recipe).

**`947191d` (Slice D, 2 files):** new `docs/release/records/0.21.3.md`;
`CHANGELOG.md` (dated the `[0.21.3]` heading).

## 4. Important implementation decisions

- **Fixed the IMP-06 wording in both places it appears**
  (`decision-log.md` and `implementation-notes.md`), not just "the decision
  log" as the handoff text named. Both files carry the identical
  misleading row; fixing only one would have left the other saying the
  same wrong thing.
- **Retitled the old hand-unpack Verus recipe as a pattern reference for
  the *previous*, retired pin**, rather than updating its download
  commands to the current version. I have no way to verify the current
  release's exact GitHub asset name without a network fetch I can't make
  reliably; fabricating a URL would be exactly the kind of unverifiable
  claim this release line exists to remove. AUR `verus-bin` is documented
  as the recommended, current path instead.
- **Verified every remaining `cargo xtask` command in `release-checklist.md`**
  against `crates/fjell-tools/src/main.rs`'s actual subcommand list, not
  just the three named in the RFC — all others (`build`, `repro-check`,
  `abi-snapshot`, `trust-report`, `sign-bundle`) are valid. Also ran Step
  3a/3c's exact `cargo run -p fjell-unsafe-audit -- --workspace . --check`
  / `fjell-mmio-audit` command lines directly to confirm they produce the
  documented output (they do — the tools silently accept the extra flags).
- **Did not touch Steps 9–10's bundle-signing logic**, despite finding
  that `target/release-bundles/*.bundle` (Step 9's glob) has no
  corresponding output anywhere in `package_release.rs`. This may be
  separate, not-yet-built v1.0 infrastructure rather than checklist drift,
  and tracing it fully is a different kind of investigation than "verify
  the xtask subcommand list." Flagged in the release record's known
  limitations rather than guessed at or silently left unmentioned.
- **Dated the CHANGELOG `[0.21.3]` heading** even though no prior entry in
  this project has a date — the RFC's exit criterion 7 explicitly requires
  "version and date correct." Did not retroactively date historical
  entries; only this one, going forward.
- **Confirmed `release_required` flags directly against
  `verification/verus/verus-targets.toml`** before writing the
  TOOLCHAIN.md correction, rather than trusting my own memory of the
  tier/promotion history from earlier in this review chain.

## 5. Differences from the handoff/RFC

None. Followed the handoff's slice boundaries, scope list, and prohibited
shortcuts exactly. Did not touch `package_release.rs`,
`TOOLCHAIN.lock`, any kernel/ABI/capability/lease/IPC/crypto code, or CI
enforcement. Did not apply the tag.

## 6. Executed commands and real output

All captured fresh at the Slice D evidence commit (`c8b1655`, working tree
clean); the full table is in `docs/release/records/0.21.3.md` — not
duplicating it verbatim here, but the headline results:

```
cargo metadata --no-deps           exit 0, 88 members, all at 0.21.3
cargo fmt --all --check            exit 0
cargo xtask build                  exit 0, 0 warnings, prebuilt unchanged
cargo xtask test-all --no-qemu     5/5 required host tiers PASS
cargo xtask test-all               18/18 PASS, 0 SKIP (full QEMU, ~14 min)
cargo xtask release-rehearsal      ALL MECHANICAL GATES PASS
cargo xtask verus-check --release-required   exit 0, both targets MACHINE-CHECKED-PASS
```

Every `SUMMARY.md` and `release-checklist.md` link checked programmatically
against the filesystem after all edits: 0 missing in both files.

Side effects of these runs (`trust-report.txt` regeneration, `tests/runs/`,
`tests/qemu/artifacts/`) reverted/deleted after each; working tree clean at
`947191d` (current HEAD).

## 7. Unresolved issues and blocked items

Nothing blocked. One item flagged, not resolved (see §4): Step 9 of the
v1.0 checklist assumes a build output path that doesn't appear to exist.
It's v1.0-specific, so it doesn't block this v0.21.3 release, but it's a
real open question for whenever `v1.0.0` prep actually starts.

## 8. Known limitations

Full list in `docs/release/records/0.21.3.md` §Known limitations at
0.21.3 — carried forward from the RFC-v0.21.3-001 chain (Finding C,
the ABI snapshot gate's formatting sensitivity, the 9 undispatched
syscalls, ERRATA E-011), plus the new Step 9 observation above.

## 9. Requested review focus

1. The release record's completeness and accuracy (`docs/release/records/0.21.3.md`)
   — it's the RFC's substantive deliverable and the first of its kind.
2. Whether the Step 9/10 bundle-signing gap deserves its own tracked
   finding now or can wait until v1.0 prep actually begins.
3. Whether `0.21.3` is ready to tag once this is reviewed, or whether
   anything else needs to land first.
