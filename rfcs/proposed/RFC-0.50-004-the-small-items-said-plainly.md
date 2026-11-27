# RFC-0.50-004: The small items said plainly — E-135's repair and its three-set check, the `MmioUnmap` comments and the sweep for their shape, criterion 9's classification, and the figure that is not mine

**Status:** Proposed — drafted by the implementer for the architect's review. **Small items A and B are implemented in the stage-1 commits and described below; C is designed and not implemented; D is untouched.**
**Milestone:** 0.50
**Tracks.** Handoff small items **A** (E-135), **B** (the two stale `MmioUnmap` comments), **C** (criterion 9), **D** (the readiness figure).
**Errata:** **E-135** is item A's subject; its headline and one figure in its body are corrected in the register by this stage (the amendment the handoff asked for, and only that: no status line, no limitations row, no released record). Item C asks the architect for a new erratum (the 0.49.0 record's criterion 9), **CLOSED as a record**; this RFC does not file it.
**Touches.** A: `crates/fjell-tools/src/{qemu_run.rs, test_all.rs}`, `tools/fjell-consistency-check/src/{qemu_records.rs, qemu_records_gate.rs, lib.rs, main.rs}`, `tests/qemu/profiles/fleet-demo.toml` (comments), `tests/qemu/artifacts/{mmio-unmap,task-lifecycle}/` (written **by the mechanism**), `rfcs/ERRATA.md` (E-135's figures). B: `crates/fjell-kernel/src/task/spawn.rs`, `crates/fjell-syscall/src/lib.rs`, `crates/drivers/fjell-driver-uart/src/main.rs` (comments). C (proposed): `docs/src/releasing/v0-release-cycle.md`, `tools/fjell-consistency-check/src/release_records.rs`.
**Not in this RFC:** D's number; a fix for E-133; any change to a released record.

---

## A. E-135 — a newly gated profile records its own first artefacts, and a check compares the sets

### What changed

**The writer.** `ArtifactDir::for_run(name, release_gated)` now sets `writes_summaries = release_gated || already_committed`. The authority for *gated* is the profile TOML's own `release_gated` field (the loader already requires a `not_gated_reason` when it is `false`); *already committed* is kept so that **the one case the new rule alone would regress — a profile that is not gated but whose files are tracked (`store`) — keeps being refreshed in place** rather than silently going stale. The handoff said to gate on *being gated*, not on the file's presence; the disjunction is that, plus not breaking `store`, and I name it as a choice so it can be overruled.

**One consequence the handoff did not mention: the smoke tier was not gated by the profile's own field.** `Profile::smoke` synthesised `release_gated: true` for **every** milestone `qemu-test` accepts, including `m7`, which `test-all` does not run (`SMOKE_PROFILES` is the four it does). Keyed on that field, the new rule would have made every ad-hoc `qemu-test m7` leave three untracked files behind — **E-040 again, one level up**. So `Profile::smoke` now derives its gate from `test_all::SMOKE_PROFILES` and says why when it is `false`. *The control was run, not asserted:* `qemu-test m7` after the change leaves nothing untracked (its `smoke-m7` directory holds only ignored files), and prints the announced skip.

**The check: `qemu-records`** (library module `qemu_records`, thin gate half, registered in `ALL_SUBCHECKS` and `SUBCHECK_NAMES`). **Three sets, as the handoff said, and not two:** the 28 profile TOMLs, the smoke tier (`SMOKE_PROFILES`, read from `test_all.rs`'s source), and the tracked `expected-markers.txt` directories — listed through `fjell-repo-scope` (`Scope::Tracked`: the claim is about what is *shipped*, and a file written by a local run and never committed is exactly the failure). Per profile:

* **release-gated and single-node, or a smoke milestone `test-all` runs** — must have a record; a missing one **fails and says to run it** (`qemu-run --profile <name>`), *not* to write the files by hand;
* **`release_gated = false`** (`store`) — a record is optional; its presence is not an orphan;
* **`multi_node`** (`fleet-demo`) — a record **or a named waiver with a reason**; a waiver for a profile that has since gained a record, is not `multi_node`, or no longer exists **fails** (the `unsafe-audit` exemption shape);
* **a record owned by nothing** — an orphan, a failure. The four `smoke-*` directories are owned by `SMOKE_PROFILES`, not orphans — I nearly filed them as orphans too, until I asked which tool owns the name.

### What I found that the handoff did not say — `fleet-demo`

The handoff expects the check to **fail on `fleet-demo`** (*"the obvious two-set check would still miss `fleet-demo` … The check needs three sets"*). Measured: it does fail on it, **unless it is waived — and I have waived it, so as built it reports it on every run instead of failing the gate.** The reason, checked against the tree:

* `fleet_demo.rs` **never calls `run_profile` or `ArtifactDir`** — the only two things that write the three files — so **no change to `write_summary` could ever produce a `fleet-demo` record**; its `up`/`verify` read and write `tests/fleet-demo/logs/`;
* `test-all` does not run it (`SMOKE_PROFILES` and the derived categories both exclude it) and **no workflow in `.github/workflows/` names it**;
* **its own profile TOML said the opposite:** *"Runs as part of the QEMU smoke tier in `cargo xtask test-all`"* and *"the xtask smoke runner detects the `fleet-demo` profile name and delegates"*. Neither was ever true (no smoke runner mentions it). Corrected in the same change.

So the handoff's *"recorded by the mechanism"* cannot hold for `fleet-demo`: **E-135's repair covers two of its three profiles, and the third needs a different change** (make the fleet runner record — a design change to a runner that boots three QEMUs, and not a condition in `write_summary`). **This is the architect's decision, and I made a provisional one:** waive it by name, with the reason in the gate's source, so its absence is printed in the gate's own PASS line — *not* a gap by construction. **The alternative is to fail the gate until `fleet-demo` records**, which would leave `consistency-check --all` red for a profile nothing runs. *Shown both ways:* with the waiver removed the check is **RED, naming `fleet-demo`** (mutation A2 below).

### Evidence

* **The red run, before the artefacts existed** (`qemu-records` against the tree with the writer fixed and nothing yet recorded): `FAIL` naming **`mmio-unmap`** and **`task-lifecycle`**, each with the instruction to run it and not to hand-write; `fleet-demo` waived.
* **The mechanism wrote the files.** `cargo run -p fjell-tools -- qemu-run --profile mmio-unmap` and `… task-lifecycle` (the command the failure message gives, run verbatim) each wrote `expected-markers.txt`, `qemu-command.txt` and `result-summary.txt` into a directory that had none; **nothing was seeded by hand**. `git status` then showed exactly those six files, nothing else; they are committed as written.
* **`qemu-records` after:** PASS — 28 profile TOMLs, 4 smoke milestones, 31 tracked records (29 + the two), 26 gated profiles + 4 smoke recorded, `store` recorded and not gated, `fleet-demo` waived with its reason.
* **Mutations (checked backups, sha-exact restores, named failures):**
  **A1** the old rule restored (`writes_summaries: already_committed`) → `a_newly_gated_profile_writes_its_first_summaries` **RED**; **A2** the `fleet-demo` waiver removed → `qemu-records` **RED**: `` `fleet-demo` is `multi_node` and has no recorded … and no waiver``; **A3** `Profile::smoke` always gated → `a_smoke_milestone_is_gated_exactly_when_test_all_runs_it` **RED**.
* **`marker:mmio-unmap:…` and `marker:task-lifecycle:…` now have something to resolve against**, which is what RFC-0.50-003's Evidence line needs.

### E-135's figures, corrected in the register (not a released record)

*Measured at `b5d488d` with the handoff's own comparison:* **27** categories (`qemu-negative`'s "Known categories"), **29** recorded directories, **gated with no record: `mmio-unmap`, `task-lifecycle`**, **recorded and not a category: the four `smoke-*`**. So **25 of the 27 categories agree**; the headline *"three of twenty-seven have none"* added `fleet-demo`, a 28th profile outside that set. The entry's headline and its *"tracked for **24** of the **27** gated profiles"* line are amended **in the entry** (a dated *Corrected* note beside each, quoting what it read) and in the Summary table's row for it. **The body's own figure was wrong too, in the other direction:** 24 is not 25 under either reading (27 categories less two, or 28 profile TOMLs less three). One more instance of the register carrying a figure a person typed; named, corrected, not hidden. **`v1-limitations.md` row 30 repeats *"Three of the twenty-seven gated boot profiles"*; the row is the architect's and I have not touched it** — it needs the same correction at the cut (and, since 27 includes `store`, *"gated"* there means *"categories"*: 26 are release-gated).

---

## B. The two stale `MmioUnmap` comments, and the sweep for their shape

### What changed

Each comment keeps its conclusion and replaces its reason, as asked:

* `crates/fjell-kernel/src/task/spawn.rs:1393` — *"an MMIO mapping already obtained is not reclaimed: `MmioUnmap` is declared and never dispatched"* → *"revoking the lease does not take back an MMIO mapping already obtained: that is the holder's own `MmioUnmap`, or its death"*. A fact about revocation, which is what `bootstrap-leases` premise 5's successor still guards.
* `crates/fjell-syscall/src/lib.rs:312` (published rustdoc on `sys_task_spawn_leased`) — *"An MMIO mapping the task already holds is not reclaimed: `MmioUnmap` is declared in the ABI and never dispatched"* → *"Revoking the lease does not take back an MMIO mapping the task already holds: unmapping is the holder's own act, `MmioUnmap`, or its death."*

Both edits are **line-count neutral on purpose**: a service's panic locations embed line numbers, so a comment that adds a line to `fjell-syscall` shifts every prebuilt's bytes and forces a baseline re-record for no reason. (`repro-check` re-run at the tip; result in the stage-1 review request.)

### The sweep — for the shape, not the name (the handoff's instruction, because the 0.49 sweep looked for the retired name)

*The claim that went stale was about the **sibling** of the syscall that was retired.* So the probe is every sentence that asserts a **dispatch state**, in `crates/` and `docs/src/` (excluding `docs/src/history/`, `docs/src/audit/`, released records and `rfcs/done/`, whose past tense is legitimate): `never dispatched | not dispatched | undispatched | no dispatch arm | declared (in the ABI)? (but|and) | dispatch arm | dispatched`, then `UnknownSyscall`. Findings, each with its disposition:

| Where | Says | Disposition |
|---|---|---|
| `crates/drivers/fjell-driver-uart/src/main.rs:28` | *"it cannot yet use cap-broker, since `sys_cap_install` has no dispatch arm (RFC-0.25-001 D4)"* | **false since 0.45; the same shape as the two named.** Conclusion kept (the driver cannot use the broker), reason replaced: `sys_cap_install` installs **`Endpoint` capabilities only** (`INSTALLABLE_KINDS`), and this driver's are `Interrupt` and `MmioRegion`. **Fixed (line-neutral).** *A third instance the handoff did not list.* |
| `docs/src/api/syscalls.md` header; `kernel.md:21`, `:27`, `:123` | the 26 / "remaining 9" / "no dispatch arm" claims | **line 2's** (RFC-0.50-002), including a **seventh** (`kernel.md:21`, the opening paragraph) the handoff did not list |
| `docs/src/api/syscalls.md` row 12 and note | *"no `fjell-syscall` wrapper"* for `CapDelete` | **false since 0.47** (`sys_cap_delete` exists at `lib.rs:743`) — a finding about a *wrapper*, not a dispatch state, caught by checking the claim the gate's wrapper rule would check (RFC-0.50-002 finding 10) |
| `docs/src/external-design/kernel.md:32` | alias `ipc_try_send` | names a wrapper that does not exist (renamed at E-022) — RFC-0.50-002 finding 11 |
| `docs/src/adr/0009…`, `docs/src/roadmap/v0.23-direction-options.md`, `v1-limitations.md` correction blocks, `rfcs/ERRATA.md` | undispatched-era statements | **dated and historical**; left alone (an ADR and a roadmap option document record what was true when written, and `v1-limitations.md`'s blocks are existing dated corrections) |
| `crates/fjell-syscall/src/lib.rs:1079` | `sys_platform_region_resolve`: *"the kernel handler is not yet implemented; returns `UnknownSyscall`"* | **true**: an explicit host-side stub that issues no `ecall` (`kernel.md` says so too) |
| `docs/src/compliance/standards-mapping.md` CRA-I-2j | `syscall-surface` *"mechanically holds declared, dispatched, and committed-expectation syscall sets equal"* | **true as written and now incomplete** once `syscall-doc` exists; the row is the architect's |

*Re-run at the tip, with the control:* the same probes, run again after the edits, find the three old sentences **at `b5d488d`** (the control, so a zero is not a broken probe) and **none** in the live source after.

---

## C. Criterion 9 — the classification, and the gap at the 0.49.0 cut (designed here, not implemented)

The classification and the erratum are the architect's. What the architect asked of me is the **`release-records` subcheck** and **the report of how many records predate the dispatch step**. I measured the second, and designed the first.

### Survey — what every release record says about criterion 9 (all 29, by reading each record's own row)

| Records | Count | What they carry |
|---|---|---|
| `0.21.3` … `0.30.0` | **10** | **no criterion 9 at all** — it did not exist; the first record to exercise it says *"first exercise"* (`0.31.0.md:74`), and the cycle document says it exists *"because for four months nothing in this cycle consulted CI"* |
| `0.31.0` | **1** | criterion 9 **with one run** (`34703606384`) — correct: the dispatch step did not exist yet |
| `0.32.0` … `0.48.0` | **17** | an exit-criteria row `\| 9 \| The release commit's CI runs read \| …` naming **two** runs (push and dispatch), 0.43.0 naming three (a first dispatch that went red) |
| `0.49.0` | **1** | criterion 9 as a **subsection** (`### CI for the tagged commit — criterion 9`) naming **one** run (`45843432462`), *"the tagged commit's own run, not a dispatch"*; the exit-criteria entry reads *"see Tag and publish"*; the only other 11-digit id in the record is the `release` workflow's (`45843464701`), not CI |

**Pre-dispatch records: 11** (the ten that predate criterion 9, and `0.31.0`, which has it without the step). **That number is a finding either way, and it is the floor's justification:** 11 of 29 records can *never* name two runs, so a rule that fails on one id must start where the step started, **0.32.0** (`v0-release-cycle.md`'s own *"Added by RFC-0.32-001 D6"*), and not at the oldest record.

### Design (proposed; not built)

* **The check** lives in `release_records.rs` (library, host-tested) beside the structure checks it already makes, and fails a record at or after the **dispatch floor `0.32.0`** whose criterion-9 section names **fewer than two distinct run ids** (a run id: an integer of ≥ 10 digits inside the section).
* **Finding the section** is the brittle part, because the shape changed: `0.31.0`–`0.48.0` use a table row (`| 9 |`), `0.49.0` a subsection. *Recommended:* accept **either** anchor — the row `| 9 |` in the exit-criteria table, or a heading containing `criterion 9` — and treat *neither found* as **a failure** (a check that passes over no section is the E-014 shape), and say which anchor it used per record.
* **Exemptions, in the `unsafe-audit` shape** (hand-written, a reason each, and **a stale entry fails**): the **floor** is one (*"the dispatch step did not exist"*, 11 records covered, stated as a count the check prints); **`0.49.0`** is the one named record (*reason: the gap itself, "released records are not rewritten", and the erratum*), and it goes stale — and the check fails — the day the record names two runs, so the exemption cannot outlive its cause.
* **Going forward** the record names the runs in a fixed form (`push <id>; dispatch <id>`) so the next shape change cannot silently defeat the anchor.
* **The cycle document** gets the two-kind classification the architect specified (correctness-gating jobs block the tag; `ci-pages` alone is a **publishing** job — a red one is *recorded with its run id and what was not published*, not waived), **and the statement of which run each kind is read from** (the push run for `ci-pages`; the dispatched run for `fuzz-run` and `miri`, which are skipped on push, while `ci-pages` is guarded by `github.event_name != 'workflow_dispatch'`). *Every command written into it will be run verbatim, with a control, before it is committed* (the standing rule; this is how E-012 was closed). I have not written the document text: it is a procedure the architect owns, and the draft would be mine to run, not to rule.

**Decisions asked:** (1) the floor and its justification as above; (2) whether the exemption for `0.49.0` is named in the gate or carried by the erratum's number; (3) the either-anchor rule versus requiring the fixed form from `0.50.0` and **only** exempting the legacy shapes by name.

---

## D. The readiness matrix's obligation figure — untouched

`docs/src/releasing/v1-readiness.md:60` promises **20**. **Not changed, as instructed**; the figure is the owner's. *(I have not re-run `verus-count` in this stage: the handoff gives 19 / 26 / 12 / 17 and my recommendation would be its own — **19** — so a re-measurement now would only add a second source for a number I am not to write. It goes in the stage-2 record if the owner rules during the line.)*

---

## Reproduce

```
cargo run -q -p fjell-consistency-check -- qemu-records                          # PASS; with the waiver removed in the gate: FAIL naming fleet-demo
git ls-tree -r --name-only HEAD tests/qemu/artifacts | /usr/bin/grep '/expected-markers.txt$' | sed 's|tests/qemu/artifacts/||; s|/expected-markers.txt||' | sort   # 29 at b5d488d
cargo run -q -p fjell-tools -- qemu-negative 2>&1 | sed -n 's/^Known categories: //p' | tr ', ' '\n\n' | sed '/^$/d' | sort                         # 27
/usr/bin/grep -rn 'fleet' crates/fjell-tools/src/qemu_run.rs crates/fjell-tools/src/test_all.rs crates/fjell-tools/src/smoke.rs .github/workflows   # comments and tests only; nothing launches it (ci.yml's `fjell-fleetd` is another crate)
git grep -n -i -E 'never dispatched|not dispatched|undispatched|no dispatch arm|dispatch arm' -- crates docs/src ':!docs/src/history' ':!docs/src/audit'
/usr/bin/grep -n -E '^\| 9 \|' releases/0.3[1-9].0.md releases/0.4[0-8].0.md      # 0.31 one run; the 17 after it two
```

---

## As built (stage 2)

### A. The architect's three-part ruling on `fleet-demo`, applied

1. **The waiver's reason now carries the real state**, not only the `run_profile` half: multi_node and run by nothing automated; the profile still defaults to release-gated; its four pass markers were **declared in three places and read in one**; since 0.50 `verify` reads the profile; the three-node *topology* is still hard-coded in `fleet_demo.rs`, recorded and not fixed.
2. **The duplication is deleted, not described.** `load_profile` is `pub(crate)`; `fleet_demo.rs`'s `PASS_MARKERS` and its hard-coded `120` seconds are gone, replaced by `verify_spec` reading `tests/qemu/profiles/fleet-demo.toml` (an unreadable profile, or one with no markers, is an error, so `verify` cannot wait for nothing). The `[verify]` block of `examples/three-node-fleet/fleet-demo.toml` — **read by nothing** — is replaced by a pointer. **Compared mechanically before deleting:** the three lists were identical, reconciled by nothing. Two tests (the profile's markers and timeout are what `verify` reads; editing the profile changes what it waits for). **Mutation F1:** a hard-coded copy again → `editing_the_profile_changes_what_verify_waits_for` RED.
3. **`PORTS` stays hard-coded**, and the topology duplication with the examples TOML's node table is **recorded, not fixed**: parsing that table is not this line (the architect's reason, and mine).

*E-135's disposition is the architect's and I have not written it:* the writer defect is fixed, `qemu-records` gates the sets, two of the three records exist because the mechanism wrote them, and what remains is `fleet-demo`, waived by a checked waiver (a stale waiver fails).

### C. Criterion 9 — built (`release-records` now judges it)

* **Section-scoped and label-based, as ruled** (`criterion9.rs`, library + tests; the gate half is `release_records_gate.rs`). The section is **either** anchor — the exit-criteria row `| 9 |`, or a heading naming "criterion 9" to the next heading of the same or a higher level — **neither found is a failure at or above the floor.** A run id counts only when a `push` or `dispatch` label (whole word) precedes it within 40 non-digit characters on its line.
* **The hole in the first design, shown by a test, not asserted:** counted over the *document*, `0.49.0` has three long numbers (`45843432462`, the push run; `45843464701`, the **`release` workflow's** run; a Pages artifact id) and passes. The test counts them (3), then shows the section-scoped, labelled rule finds one push run and **no dispatch run**.
* **The red run — the item's evidence — captured before the exemption existed:** `release-records: FAIL — criterion 9: release record 0.49.0 names a push run (45843432462) and **no dispatch run**: criterion 9 reads two runs, because `fuzz-run` and `miri` are skipped on push and `ci-pages` is skipped on dispatch` — **and nothing else**: 29 records read, 17 name both, 11 are below the derived floor. The unit test `the_real_records_are_red_on_0_49_0_alone_and_the_floor_is_0_32_0` holds the same on the real files.
* **The floor is derived, not written** (the architect's ruling): the lowest-version record that names both labels — **`0.32.0`**; the **11** below it are the ten that predate criterion 9 and `0.31.0`. They are reported by count and up-to name in the PASS line, and **not** seeded as eleven exemptions.
* **The one named exemption is `0.49.0`**, with its reason in the gate's source (*a released record is not rewritten; the gap is the architect's erratum, CLOSED as a record*). The ruling did not say where it lives; I put it in the gate, **stale-checked**: it fails the gate the day the record names both runs (shown by a test), when the version is below the floor, or if it has no reason. If the architect prefers the exemption to cite an erratum number, that is a one-line change after the erratum is filed.
* **The classification is written into `docs/src/releasing/v0-release-cycle.md`** as ruled: correctness-gating (a red job blocks the tag or takes an accepted-risk statement) vs publishing (**`ci-pages` alone**: recorded with its run id and what was not published; does not block), **and which run each is read from** (the dispatched run for `fuzz-run` and `miri`; the push run for `ci-pages`; both for the rest), **and that the record names them labelled, which the gate reads.** The one command the paragraph carries (`grep -n 'pages: write\|deploy-pages\|^    environment:' .github/workflows/ci.yml`) was **run verbatim before it was committed**: 5 hits, all between the `ci-pages:` (230) and `ci-qemu-smoke:` (306) job keys, two of them comments; and the job count the paragraph states (21) was re-counted (`grep -c '^  ci-[a-z0-9-]*:$'` → 21).
* **Mutation C1** (an unlabelled one-run record counted as both) turns five tests RED, including the real-records one.
* **Not done:** the erratum (the architect's, CLOSED as a record); whether the 0.49.0 dispatch run was made and left out of the record (the tree cannot say, and the settling call names this repository).

### D. Untouched, as instructed.
