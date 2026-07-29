# RFC-v0.21.3-001: Build Restoration and As-Built Reconciliation

**Status:** Proposed — **accepted for implementation by the owner (nabbisen), 2026-07-30**
**Milestone:** v0.21.3 (patch)
**Depends on:** —
**Corrects:** regressions introduced by commit `5091e54` ("`Cargo.toml` format; CI actions checkout version")

## Summary

The repository at `9153cc3` does not build: the workspace manifest is
syntactically invalid, so every `cargo` entry point — including
`cargo xtask release-rehearsal`, which is the sole mechanism by which this
project produces release evidence — fails before doing any work. Separately,
the documentation describes a syscall surface larger than the kernel
implements, and several indexes and version stamps disagree with the tree.

v0.21.3 restores the build and reconciles the declared state of the repository
with its actual state. It adds no OS functionality and changes no security
boundary.

The governing principle: **this project's value proposition is that its claims
are checkable. A claim that cannot be re-derived from the tree is a defect of
the same kind as a wrong claim.**

## Motivation

Five verified problems, in severity order. Each was reproduced against
`9153cc3`; commands and outputs are recorded in the companion handoff.

### M1 — The workspace manifest does not parse (blocker)

`Cargo.toml:6-7` contains unterminated string literals:

```toml
members = [
    "crates/*,
    "tools/*,
    "benches",
]
```

`cargo metadata` fails with *"missing comma between array elements"*. Because
`cargo xtask` is an alias for `cargo run --package fjell-tools --`, the failure
is total: build, test, every one of the eleven release gates, and CI are all
unreachable.

Repairing the quoting is **not sufficient**. The globbed form is wrong for this
repository's layout on two independent counts, both verified experimentally:

1. `crates/*` does not recurse. It would exclude all 56 crates under
   `crates/arch/`, `crates/drivers/`, `crates/formats/`, and
   `crates/services/`.
2. Cargo hard-errors on any glob-matched directory lacking a `Cargo.toml`.
   That includes the four group directories and 56 empty directories left in
   `crates/` by the v0.21.0 reorganization.

The globbed form also contradicts a recorded design decision: the external
design states the workspace uses an *"explicit member list (no globs)"*
(`docs/src/external-design/…`, handoff `external-design.md` §5).

### M2 — The documented syscall surface exceeds the implemented one

`fjell-abi` declares **35** syscall numbers. `fjell-kernel`'s dispatcher
handles **26**. The remaining **9** fall through to
`Some(_) | None => SysError::UnknownSyscall`
(`crates/fjell-kernel/src/trap/syscall.rs:52`):

| Declared, not dispatched | Documented as as-built in |
|---|---|
| `CapInstall` (17) | `external-design/kernel.md` §2, `capability-lease.md` §2 |
| `PlatformReboot` (18), `Reboot` (120) | `external-design/kernel.md` §2 ("Platform: reboot") |
| `TaskKill` (43) | — |
| `MmioUnmap` (91) | — |
| `IrqBind` (100), `IrqAck` (101), `IrqWait` (102) | `external-design/kernel.md` §2 and §6 |
| `DmaShare` (111) | — |

Three consequences beyond the count:

- `sys_cap_install` **is implemented** at
  `crates/fjell-kernel/src/cap/syscall.rs:123` but no dispatch arm reaches it.
  It is unreachable code presented as the live cap-broker path.
- `sys_cap_install_with_rights` (`crates/fjell-syscall/src/lib.rs:530`)
  silently discards its `rights_bits` argument. Its own doc-comment promises
  *"the kernel validates that `rights` ⊆ installer authority"*. That validation
  does not execute.
- `sys_platform_region_resolve` (`crates/fjell-syscall/src/lib.rs:639`) is an
  explicit stub returning `UnknownSyscall`, yet appears in the documented
  Platform group.

The *normative* ABI reference `docs/src/abi/ipc-register-layout.md` names
`SyscallNumber::IpcTrySend`, which does not exist. The `sys_ipc_try_send`
wrapper issues `IpcSend` (20).

Additionally, the stated total of "38 syscalls" (`external-design/kernel.md`
§2; handoff `project-summary.md` §2 and `external-design.md` §2) matches
neither figure, and the doc's own group table sums to 36.

**Security note.** This is over-claim, not a hole: every undispatched number
fails closed to `UnknownSyscall`, which is the correct direction. The material
exposure is the `cap_install_with_rights` rights-check that is documented but
absent, and the risk that a future implementer trusts the documented surface.

### M3 — Formatting gate cannot pass

Once M1 is repaired, `cargo fmt --all --check` fails across **252 files**
(2374 hunks) — CI job `ci-format` is red. This is a direct consequence of M1:
`cargo fmt --all` requires a parseable workspace, so no formatting pass has
been possible.

### M4 — The reproducibility tier cannot fail

`tools/fjell-repro-check` has two modes. Default mode builds twice and compares
the two builds — this needs no stored baseline and is what
`ops-security.md` describes. `--skip-build` mode hashes the committed
`crates/fjell-kernel/prebuilt/*.bin` against
`tests/repro/baseline-digests.txt`; it is `test-all` tier 5 and runs in CI
because it is fast.

`tests/repro/baseline-digests.txt` is **absent** from the tree
(`tests/repro/` is empty and untracked). And `main.rs:92-97` reads:

```rust
// If baseline doesn't exist, create it and pass.
if !Path::new(baseline_path).exists() {
    match save_digests(&current, baseline_path) {
        Ok(_) => { …; return ExitCode::SUCCESS; }
```

So the tier records a baseline from whatever it just measured and returns
success. **It cannot fail.** It has been reporting green while detecting
nothing.

This is not a conflict between two conventions. `v1-limitations.md` and the
v0.17–v0.18 handoff §5.2 correctly describe the baseline as a committed
artifact; the `implementation-notes.md` §6 claim that it is "re-recorded per
run by design" describes a fail-open bug rather than a design.

**Decision (architect, within delegated authority over verification
strategy):** commit the baseline, and make `--skip-build` fail closed when the
baseline is absent — recording a new baseline must require an explicit flag.
The precedent is this project's own: v0.20 made the negative-test harness
fail-closed on exactly this reasoning. A gate whose failure mode is
"silently start passing" is not a gate.

This is a gate-behaviour change and is therefore called out explicitly rather
than folded in silently. It is contained to one host-side tool; it touches no
kernel, ABI, or security-boundary code.

### M5 — Index, link, and version drift

| Item | Declared | Actual |
|---|---|---|
| `docs/src/SUMMARY.md:65-71` | `./releases/handoff/…` | `./releases/handoff-0.21.2/` (7 dead links) |
| Handoff bundle version stamp | v0.21.1 throughout | directory `-0.21.2`, workspace `0.21.2` |
| `rfcs/README.md` §"Implemented" | 99 RFCs | 154 files in `done/` |
| `rfcs/README.md` §"Proposed (proposed/) — 25 RFCs" | v0.11–v0.15 in `proposed/` | all in `done/`; `rfcs/proposed/` did not exist |
| Root `README.md` | badge 0.21.0, prose v0.21.1 | workspace 0.21.2 |
| Root `README.md` | "268 audited unsafe sites" | 203 `// SAFETY:` / 207 `unsafe` in kernel+arch |
| `tests/repro/baseline-digests.txt` | cited by `v1-limitations.md`, `ops-security.md` | absent; `tests/repro/` empty and untracked |
| `docs/src/getting-started/`, `docs/src/development/` | — | empty directories |

The `rfcs/README.md` state is a direct violation of this project's own RFC
lifecycle policy (§"README integrity", anti-pattern §"Letting cross-references
rot"): 25 RFCs are indexed as Proposed, at paths that do not exist, while
living in `done/`.

## Goals

1. `cargo metadata`, `cargo xtask build`, and `cargo xtask release-rehearsal`
   run again, with the workspace membership matching the recorded design.
2. Every documented syscall, ABI register contract, and surface count matches
   the implementation, or is explicitly marked as not-yet-dispatched.
3. `cargo fmt --all --check` passes.
4. Indexes, links, counts, and version stamps resolve and agree.
5. The eleven mechanical gates are re-run and their real results recorded —
   replacing the currently unreproducible evidence in the handoff bundle.

## Non-goals

- **No new kernel surface.** Wiring up the 9 undispatched syscalls is a
  feature change, out of scope for a patch. See §Deferred.
- **No removal from the ABI enum.** Deleting declared numbers would be an ABI
  break governed by the snapshot gate; also deferred.
- No change to capability, lease, IPC, or crypto semantics.
- No v1.0 tagging, publication, or Gate 9 activity.

## The design decision

**Decision: correct the documentation down to the 26 dispatched syscalls;
do not add kernel handlers in v0.21.3.**

Alternatives considered:

| Option | Assessment |
|---|---|
| **(A) Document down to as-built** *(chosen)* | Patch-appropriate; the surface already fails closed; restores checkability immediately; zero risk to the security core. |
| (B) Implement the 9 handlers | New kernel surface — IRQ routing and `cap_install` both touch the authority path. Requires its own RFC, negative tests, and Verus impact review. Not patch work. |
| (C) Remove the 9 from the ABI enum | An ABI removal; the snapshot gate (Gate 4) forbids removals by design. Would require an RFC and an architect decision record. |
| (D) Leave as-is, note in limitations | Rejected. The limitations document is a Gate 9 artifact; using it to absorb a documentation defect degrades the one instrument the owner signs against. |

Rationale for (A): the owner has directed that v1.0 is not being tagged and v0
development continues. That removes the pressure that would otherwise argue for
(B) — there is runway to add the surface deliberately, under its own RFC, rather
than at a freeze point. Shrinking a claim to the truth is always available and
always cheap; growing the kernel is neither.

The documentation must state the position honestly: these numbers are
**reserved and declared but not dispatched**; calling one yields
`UnknownSyscall`.

## Scope

Three independently reviewable slices. Slice 1 unblocks everything else.

| Slice | Content | Reviewable independently |
|---|---|---|
| **S1 Build restoration** | Restore the explicit 88-entry member list; verify `cargo metadata`; remove the 56 empty `crates/` directories | yes |
| **S2 Formatting** | `cargo fmt --all`, single mechanical commit, no hand edits | yes |
| **S3 As-built reconciliation** | Syscall docs + ABI reference (M2); indexes, links, counts, stamps (M5); re-run and record gate evidence | yes |

S2 is isolated deliberately: a 252-file formatting diff must not be mixed with
substantive changes, or neither can be reviewed.

## Compatibility

No source-compatible change to any published interface. The `fjell-abi`
snapshot surface is untouched, so Gate 4 must continue to report zero
removals — that is an acceptance criterion, not an assumption.

Formatting (S2) changes source bytes without changing semantics. The
reproducible-build baseline and `crates/fjell-kernel/prebuilt/*.bin` must be
re-derived and the result *inspected*: identical binaries are expected, and a
difference is a finding to report, not to paper over.

## Security considerations

- No security boundary is modified.
- M2's documentation correction **narrows** a claim; it does not weaken a
  control.
- The `sys_cap_install_with_rights` rights-check gap must be recorded as a
  known limitation with an explicit pointer to the deferred decision, so it
  cannot be silently inherited.
- Gate 2 (unsafe audit) and Gate 3 (MMIO audit) must be re-run after S2;
  formatting can move `// SAFETY:` comments relative to their `unsafe` blocks,
  which is exactly what those gates exist to catch.

## Testing and verification requirements

Evidence is required, not asserted. Each item must be produced by a real run:

1. `cargo metadata --no-deps` — exit 0, 88 workspace members.
2. `cargo fmt --all --check` — exit 0.
3. `cargo xtask build` — succeeds; warning count recorded.
4. `cargo xtask test-all --no-qemu` — all required host tiers pass.
5. `cargo xtask test-all` — host tiers + QEMU.
6. `cargo xtask release-rehearsal` — all mechanical gates; Gate 9 remains
   unsigned and out of scope.
7. Verus (Gate 10) — capability and lease machine-checked, or an explicit
   honest record that the toolchain was unavailable. `CONFORMANCE-ONLY` is a
   failure for a release-required target and must be reported as such.
8. A syscall-surface check: the count in the documentation equals the number of
   dispatch arms. Prefer a mechanical check over prose if it is cheap to add.

## Acceptance criteria

- [ ] `cargo metadata --no-deps` exits 0 with 88 members; no globs in `members`.
- [ ] `cargo fmt --all --check` exits 0.
- [ ] `cargo xtask release-rehearsal` runs to completion and its real output is
      recorded (pass or fail — a failure is a valid, reportable outcome).
- [ ] Gate 4 reports zero ABI removals.
- [ ] No documentation states a syscall count or names a syscall that the
      dispatcher does not handle, except where explicitly labelled
      "declared, not dispatched".
- [ ] `docs/src/abi/ipc-register-layout.md` no longer references
      `SyscallNumber::IpcTrySend`.
- [ ] Every link in `docs/src/SUMMARY.md` and `rfcs/README.md` resolves.
- [ ] `rfcs/README.md` counts and state sections match the folders on disk.
- [ ] Version stamps agree across `Cargo.toml`, `README.md` (badge and prose),
      and `CHANGELOG.md`.
- [ ] The `README.md` unsafe-site count is either re-derived from the audit
      tool or removed.
- [ ] `tests/repro/baseline-digests.txt` is committed; `repro-check
      --skip-build` fails closed when the baseline is absent (see §M4); the
      `implementation-notes.md` "re-recorded per run" wording is corrected.
- [ ] `CHANGELOG.md` has a v0.21.3 entry describing the above.
- [ ] The handoff bundle's evidence is regenerated or explicitly re-stamped
      against v0.21.3.

## Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Restoring the build reveals further latent breakage masked by M1 | Medium | Medium | Slice order puts S1 first precisely to surface this early; report findings rather than absorbing them into scope |
| R2 | The 252-file fmt pass perturbs the repro baseline or prebuilt binaries | Medium | Low | Isolated commit; re-derive and inspect; treat any binary difference as a finding |
| R3 | Gate 2/3 regressions from comment movement during fmt | Low | Medium | Re-run both gates after S2 and before S3 |
| R4 | Scope creep into fixing the 9 syscalls | Medium | Medium | Explicit non-goal; escalate rather than implement |
| R5 | Gate 10 unavailable (no Verus on PATH) | Medium | Low | Record honestly as unavailable; do not report CONFORMANCE-ONLY as a pass |

## Deferred — requires a decision before v0.22

The durable disposition of the 9 declared-but-undispatched syscalls is **not**
settled by this RFC. Three futures exist: implement them, remove them from the
ABI (an ABI break), or keep them permanently reserved with documented
semantics. This should be resolved as a v0.22 roadmap theme, with the IRQ group
and `cap_install` likely warranting separate treatment — the IRQ group is
driver-enabling work, whereas `cap_install` sits on the authority path and
carries a Verus-adjacent review burden.

## Open questions

1. ~~**Repro baseline.**~~ **Resolved** — investigation showed the two
   documents describe different `repro-check` modes rather than conflicting
   conventions, and that the real defect is a fail-open branch in
   `--skip-build`. Settled by the architect under delegated verification-strategy
   authority; see §M4. No owner decision required.
2. **RFC coverage of v0.19–v0.21.** No RFCs exist for the v0.19, v0.20, or
   v0.21 release lines, though the roadmap records them as complete themes.
   This is inconsistent with the project's stated rule that every significant
   change is RFC-governed. Raised for the record; no v0.21.3 action proposed.
