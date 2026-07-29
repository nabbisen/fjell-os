# Developer Handoff — RFC-v0.21.3-001

**Governing RFC:** [RFC-v0.21.3-001](../../proposed/RFC-v0.21.3-001-build-restoration-and-as-built-reconciliation.md)
**Milestone:** v0.21.3 (patch)
**Status:** inherited from the governing RFC (Proposed — accepted for implementation, 2026-07-30)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Read before starting

- The governing RFC (above) — especially §Non-goals and §The design decision.
- `docs/src/external-design/kernel.md` §2 and §6.
- `docs/src/abi/ipc-register-layout.md`.
- Project rules: `cargo fmt` is run **once**, after implementation is complete,
  and the formatted output is **not** hand-reviewed.

### Verified baseline (reproduced at `9153cc3`)

```
$ cargo metadata --no-deps
error: missing comma between array elements, expected `,`
 --> Cargo.toml:7:5
```

Everything below assumes you have fixed this first.

---

## 1. Change scope

**In scope:** `Cargo.toml`; empty directories under `crates/`; whole-tree
`cargo fmt`; documentation under `docs/`; `rfcs/README.md`; `README.md`;
`CHANGELOG.md`; regenerated gate evidence.

**Explicitly NOT in scope — do not touch:**

- `crates/fjell-kernel/src/**` logic. In particular **do not add dispatch arms**
  for the 9 undispatched syscalls.
- `crates/fjell-abi/src/syscall.rs` — **do not remove or renumber** any variant.
- Capability, lease, IPC, MM, or crypto behaviour.
- `verification/verus/**` proof content.
- Anything touching Gate 9, the v1.0 tag, or release publication.

If a change appears to require any of the above: stop, write an escalation
note, and hand it back.

---

## 2. Slice 1 — Build restoration

**Commit alone. Do not combine with Slice 2 or 3.**

### 2.1 Restore the explicit member list

Replace the broken globbed `members` array in `Cargo.toml` with the explicit
88-entry list from the commit before the regression:

```sh
git show 5091e54^:Cargo.toml | sed -n '/^members = \[/,/^\]/p'
```

Keep everything else in the current `Cargo.toml` as-is — `resolver = "3"`,
`version = "0.21.2"` (bumped in Slice 3), `default-members`,
`[workspace.package]`, `[workspace.metadata.fjell.ci_excluded]`,
`[workspace.dependencies]`, and the profiles. Only the `members` array changes.

**Verified:** this exact substitution was tested in an isolated worktree at
`9153cc3` and yields `cargo metadata --no-deps` exit 0 with 88 members. All 88
paths resolve to an existing `Cargo.toml`. No `exclude` key was required.

Do **not** use globs. `crates/*` does not recurse into the four group
directories and errors on directories without a manifest. This is also a
recorded design decision (RFC §M1).

### 2.2 Remove the empty leftover directories

56 empty directories remain in `crates/` from the v0.21.0 reorganization
(e.g. `crates/fjell-auditd/`, `crates/fjell-init/`, `crates/fjell-audit-format/`).
They are untracked, so this is working-tree hygiene, but they make the layout
misleading and are what makes a glob-based member list unrecoverable.

Remove only directories that are **empty** and are **not** one of the four
group directories (`arch`, `drivers`, `formats`, `services`). Verify each
contains zero files before removing. Do not remove anything git tracks.

### 2.3 Evidence required

```sh
cargo metadata --no-deps --format-version 1 | python3 -c 'import sys,json;print(len(json.load(sys.stdin)["packages"]))'   # expect 88
cargo xtask build          # record the warning count
```

---

## 3. Slice 2 — Formatting

**Commit alone.** Message should make clear it is mechanical.

```sh
cargo fmt --all
```

252 files / 2374 hunks are expected to change (measured at `9153cc3` after the
Slice 1 fix). Per project rules, **do not review or hand-adjust the formatted
output**.

### 3.1 Then re-run the annotation gates

Formatting can move a `// SAFETY:` or `MMIO-ORDER:` comment relative to the
block it annotates. These gates exist to catch exactly that:

```sh
cargo xtask release-rehearsal   # Gate 2 (unsafe audit) and Gate 3 (MMIO audit) must stay green
```

If either regresses, that is a **finding to report**, not something to fix by
editing formatter output. Escalate.

### 3.2 Re-derive the repro artifacts

Re-derive `crates/fjell-kernel/prebuilt/*.bin`. Formatting is semantically
inert, so **identical binaries are expected**. If any binary differs, stop and
report it.

Do not record the repro baseline yet — that happens in §4.3, after this slice,
and only via the explicit recording flag.

---

## 4. Slice 3 — As-built reconciliation

### 4.1 Syscall surface (RFC §M2)

Ground truth, verified at `9153cc3`:

- **35** declared in `crates/fjell-abi/src/syscall.rs`
- **26** dispatched in `crates/fjell-kernel/src/trap/syscall.rs`
- **9** declared but not dispatched → `UnknownSyscall` (`syscall.rs:52`):
  `CapInstall(17)`, `PlatformReboot(18)`, `TaskKill(43)`, `MmioUnmap(91)`,
  `IrqBind(100)`, `IrqAck(101)`, `IrqWait(102)`, `DmaShare(111)`, `Reboot(120)`

The 26 dispatched: `Yield`, `Exit`, `DebugWrite`, `CapCopy`, `CapMint`,
`CapDelete`, `CapRevoke`, `CapInspect`, `CapDrop`, `CapBindLease`, `IpcSend`,
`IpcRecv`, `IpcCall`, `IpcReply`, `IpcTryRecv`, `TaskSpawn`, `TaskStart`,
`TaskStatus`, `LeaseCreate`, `LeaseRevoke`, `LeaseInspect`, `AuditDrain`,
`PlatformInfoGet`, `MmioMap`, `DmaAlloc`, `DmaRevoke`.

Required edits:

| File | Change |
|---|---|
| `docs/src/external-design/kernel.md` §2 | Replace "38 syscalls" and the group table with the 26 dispatched. Add a clearly-labelled subsection listing the 9 as **declared and reserved, not dispatched — calling one returns `UnknownSyscall`**. Remove `cap_install`, `cap_install_with_rights`, `irq_bind/wait/ack`, `reboot`, `platform_region_resolve` from the as-built groups. |
| `docs/src/external-design/kernel.md` §6 | Delete the claim that IRQ syscalls "exist" and are "exercised by virtio drivers". There is no kernel handler. Note that `driver-virtio-net` calls them but is a documented early-exit stub. |
| `docs/src/external-design/capability-lease.md` §2 | Remove `cap_install` / `cap_install_with_rights` from the as-built capability operations; state that bootstrap installs go through the in-kernel `install_raw` path at spawn time. |
| `docs/src/abi/ipc-register-layout.md` | Remove `SyscallNumber::IpcTrySend` (does not exist). `sys_ipc_try_send` issues `IpcSend` (20). |
| `docs/src/api/syscalls.md` | Currently a 7-line stub. Either make it the authoritative 26-entry catalog or make it an explicit pointer. Do not leave it implying completeness. |
| `docs/src/releases/handoff-0.21.2/project-summary.md` §2, `external-design.md` §2 | Correct the "38 syscalls" claim. |

**Also record as a known limitation** (in the appropriate limitations/errata
document — check with the architect if the target is ambiguous):

> `sys_cap_install_with_rights` (`crates/fjell-syscall/src/lib.rs:530`) accepts
> a `rights_bits` argument and discards it. Its doc-comment states the kernel
> validates `rights ⊆ installer authority`; no such validation executes,
> because `CapInstall` is not dispatched. Disposition deferred to v0.22.

Do **not** "fix" this by editing the doc-comment alone — the limitation must be
recorded where a reader looking for limitations will find it.

### 4.2 Index, link, and stamp drift (RFC §M5)

| Item | Required end state |
|---|---|
| `docs/src/SUMMARY.md:65-71` | Links resolve. Either rename the directory to `handoff/` or update the 7 links to `handoff-0.21.2/`. Prefer whichever keeps CHANGELOG 0.21.2 truthful; state your choice. |
| Handoff bundle version stamps | All say v0.21.1. Re-stamp to the version the bundle actually describes. |
| `rfcs/README.md` | Counts and sections must match disk: 154 in `done/`, and the "Proposed (proposed/) — 25 RFCs" section lists v0.11–v0.15 RFCs that are **all in `done/`** at broken paths. Move those entries to the Implemented section with correct links. Add a Proposed section containing only this RFC. Add missing sections for v0.9, v0.10, v0.16, v0.19–v0.21 if the RFCs exist. |
| Root `README.md` | Badge (0.21.0) and prose (v0.21.1) → v0.21.3. RFC count (139) → actual. |
| Root `README.md` unsafe count | "268 audited sites" does not reconcile (203 `// SAFETY:` / 207 `unsafe` in kernel+arch). Re-derive from `tools/fjell-unsafe-audit` output, or remove the number. Do not guess. |
| `docs/src/getting-started/`, `docs/src/development/` | Empty. Remove, or populate. State which. |
| `docs/src/SUMMARY.md` — new docs unwired | `docs/src/requirements/`, `docs/src/external-design/` (10 files), and `docs/src/roadmap/roadmap.md` are not referenced from `SUMMARY.md`, so mdBook does not build them. Add them. `external-design/README.md` is the entry point for its nine subsystem pages. |
| `CHANGELOG.md` | New `[0.21.3]` entry. |
| `Cargo.toml` | `version = "0.21.3"`. |
| `ROADMAP.md`, `docs/src/roadmap/roadmap.md` | Both assert "the one remaining blocker is Gate 9". Correct the **factual** part only: the mechanical gates could not run at v0.21.2, so Gate 9 was not the only blocker. **Do not write a forward roadmap** — the v0.22+ direction is an owner decision and is not yours or the architect's to set. Note `ROADMAP.md` has uncommitted owner edits in the working tree; preserve them. |

### 4.3 Repro baseline — make the tier able to fail

Not blocked. Decided in RFC §M4; implement as follows.

`tools/fjell-repro-check` has two modes. Default mode builds twice and compares
— no stored baseline needed. `--skip-build` mode (this is `test-all` tier 5,
and it runs in CI) hashes the committed `crates/fjell-kernel/prebuilt/*.bin`
against `tests/repro/baseline-digests.txt`.

That baseline file is **absent from the tree**, and
`tools/fjell-repro-check/src/main.rs:92-97` currently reads:

```rust
// If baseline doesn't exist, create it and pass.
if !Path::new(baseline_path).exists() {
    match save_digests(&current, baseline_path) {
        Ok(_) => { …; return ExitCode::SUCCESS; }
```

So the tier writes a baseline from whatever it just measured and returns
success. It cannot fail. Required changes:

1. **Make the absent-baseline path fail closed.** Missing baseline →
   non-zero exit with a message naming the recording command. Recording must
   require an explicit opt-in flag (e.g. `--record-baseline`); it must never
   be a side effect of a check run.
2. **Commit `tests/repro/baseline-digests.txt`**, recorded from the
   `prebuilt/*.bin` set as it stands after Slice 2. Record it *after*
   formatting, not before.
3. **Correct `implementation-notes.md` §6** — the phrase "the repro baseline
   is re-recorded per run by design" describes the bug, not a design. Replace
   it with the two-mode explanation above.
4. Check `crates/fjell-tools/src/test_all.rs` and the CI workflow for any
   caller that would now fail, and make sure tier 5 invokes the check mode,
   not the record mode.

Keep this change **inside `tools/fjell-repro-check`** and its callers. It
touches no kernel, ABI, or security-boundary code.

Expected consequence: this tier may go red. **That is the point** — it has
been green while detecting nothing. If it goes red for a reason other than a
missing baseline, that is a real finding: report it, do not re-record over it.

---

## 5. Prohibited shortcuts

- Do not disable, skip, or weaken any gate to obtain a pass.
- Do not report `CONFORMANCE-ONLY` from Gate 10 as a pass — for a
  release-required target it is a failure. If Verus is unavailable, say so.
- Do not re-record the repro baseline over an unexplained binary difference,
  and do not make baseline recording a side effect of a check run.
- Do not widen scope into the 9 syscalls, the ABI enum, or kernel logic.
- Do not mix the formatting commit with substantive changes.
- Do not mark unexecuted commands as passed.

---

## 6. Required evidence

Paste real command output, not summaries:

1. `cargo metadata --no-deps` — exit code and member count
2. `cargo fmt --all --check` — exit code
3. `cargo xtask build` — result and warning count
4. `cargo xtask test-all --no-qemu` — tier results
5. `cargo xtask test-all` — tier results including QEMU
6. `cargo xtask release-rehearsal` — full gate table
7. Gate 10 result, or an explicit statement that Verus was unavailable
8. Confirmation that Gate 4 reports zero ABI removals
9. Diff of prebuilt binaries before/after Slice 2 (expected: identical)

A failing gate is a valid, reportable outcome. Report it.

---

## 7. Review request format

On completion, submit:

1. Implementation summary
2. Addressed RFC sections
3. Changed files (grouped by slice)
4. Important implementation decisions
5. Any differences from this handoff or the RFC — with reasons
6. Executed commands and their real output (§6)
7. Unresolved issues and blocked items
8. Known limitations
9. Requested review focus

Flag for focused review: the Slice 2 gate re-runs (§3.1), the prebuilt-binary
comparison (§3.2), the completeness of the syscall documentation edits (§4.1),
and the repro-check fail-closed change (§4.3).
