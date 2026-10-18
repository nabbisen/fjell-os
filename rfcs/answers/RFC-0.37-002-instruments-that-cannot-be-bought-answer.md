# RFC-0.37-002 R1–R8: instruments that cannot be bought

**Governing RFC:** [../done/RFC-0.37-002-instruments-that-cannot-be-bought.md](../done/RFC-0.37-002-instruments-that-cannot-be-bought.md)
**Handoff:** [../handoffs/RFC-0.37-002-instruments-that-cannot-be-bought/implementation-handoff.md](../handoffs/RFC-0.37-002-instruments-that-cannot-be-bought/implementation-handoff.md)

Written in the handoff's own numbered order (R1, R2, R3, R4, R6, R5, R7, R8 —
R6 before R5 is trusted, per the handoff's explicit reordering), against tip
`a76a7ad`.

---

## R1 — the inventory, re-derived at the tip, with a control per `unreachable_code` suppressor

**13 crates** carry a crate-level `#![allow(...)]` at `a76a7ad`, matching the
governing RFC's own frequency table exactly once re-measured independently:

| Lint | Count | Crates |
|---|---|---|
| `unused_assignments` | 10 | `fjell-measuredd`, `fjell-proxy-text`, `fjell-recoveryd`, `fjell-semantic-stream`, `fjell-upgraded`, `fjell-verifyd`, `fjell-attestd`, `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |
| `dead_code` | 7 | `fjell-cap`, `fjell-ipc`, `fjell-attestd`, `fjell-storaged`, `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |
| `unused_imports` | 4 | `fjell-attestd`, `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |
| `unused_variables` | 3 | `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |
| `unreachable_code` | 3 | `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |
| `unused_mut` | 3 | `fjell-secure-transportd`, `fjell-driver-virtio-net`, `fjell-netd` |

**Per `unreachable_code` suppressor, deleted and built, independently of the
RFC's own claim:**

- **`fjell-netd`** — deleting the allow and building (`touch` + `cargo build
  --target riscv64gc-unknown-none-elf`) surfaces `unreachable_code` on the
  event loop after `sys_exit(0)`, plus five more warnings across the other
  five suppressed lints. **Suppresses something** — this is E-084.
- **`fjell-secure-transportd`** — deleting the allow and building surfaces 6
  warnings, none of them `unreachable_code` (its loop is reachable; its only
  `sys_exit` is the panic handler's). To attribute precisely rather than
  conflate, the allow was re-added narrowed to `dead_code, unused_imports`
  only and rebuilt clean — confirming the other four lints, `unreachable_code`
  among them, **suppress nothing**.
- **`fjell-driver-virtio-net`** — same test, same result: 6 warnings with the
  allow removed, none `unreachable_code`. **Suppresses nothing.** Its own top
  comment, independently, made the same false "exits early by design" claim
  `fjell-secure-transportd` and `fjell-proxy-text` had already been corrected
  for by earlier lines — a third instance of the identical copied sentence,
  found while re-deriving this crate's inventory rather than trusting R1's own
  three-crate list.

This confirms the RFC's measurement exactly: one of three `unreachable_code`
suppressors hides something real, two hide nothing.

## R2 — every allow that suppresses nothing, removed

**9 of the 13** crate-level allows suppressed nothing and were deleted
outright, each confirmed by a **forced** rebuild (`touch` the crate-root file,
then build) staying clean without it:

`fjell-cap`, `fjell-ipc`, `fjell-measuredd`, `fjell-proxy-text`,
`fjell-recoveryd`, `fjell-semantic-stream`, `fjell-upgraded`, `fjell-verifyd`,
`fjell-driver-virtio-net`.

**A forced rebuild, not a plain one, is load-bearing here.** The first pass
over `fjell-attestd` (below, in R3) reported zero warnings after deleting its
blanket on a plain rebuild — a cargo build-cache false negative, since the
file's content had round-tripped back to content cargo had already compiled
clean once before. A `touch` to force the mtime forward caught a genuine
`unused_imports` warning for five names. Every one of the 9 "suppresses
nothing" crates above, and the 3 narrowed survivors in R3, was re-verified
with a forced rebuild after this was found, not trusted from the first pass.

## R3 — every survivor states its reason, at the narrowest scope that works

**3 crates survive** R2 with at least one lint genuinely suppressed, each
narrowed and reasoned:

- **`fjell-attestd`** — crate-level 3-lint blanket (`unused_assignments,
  unused_imports, dead_code`) removed; only `unused_imports` was real, for
  five names in one `use fjell_attestation_format::v2::{...}` block (planned
  M7 fields, not yet constructed). One item-level `#[allow(unused_imports)]`
  directly above that `use`, with the reason stated inline.
- **`fjell-secure-transportd`** — crate-level 6-lint blanket removed. Three
  item/enum-level survivors, each with its own stated reason: `CAP_SESSION`
  (declared per the module doc, not yet wired to cap-broker); `ChannelState`'s
  `Draining`/`Faulted` variants (the full lifecycle this type models; no code
  constructs either yet); `SxtError` (of eleven §7.3 codes, only two are
  returned today) — found in a second pass, after narrowing `main.rs` surfaced
  it as a separate warning from `ChannelState`'s. `count_established` (a
  diagnostic accessor nothing calls yet) carries a fourth. A conditionally-used
  import (`TlsHandshakeState`, needed only under `simulated-transport`) was
  separated from two genuinely-dead siblings (`SxtError as TlsSxtError`,
  `TlsState`, unused even with the feature on) by `#[cfg(feature =
  "simulated-transport")]`-gating the one that's needed and deleting the two
  that are not — verified clean under both the default build and
  `--features simulated-transport`.
- **`fjell-storaged`** — kept **crate-level**, deliberately, with the reason
  stated in the file: 13 single-purpose VirtIO register/offset constants
  scattered through a hardware register map (9 already commented "unused in
  v1 mode" since this driver runs the legacy register set only, plus
  `OFF_DESC`, `READ_DATA`, `REGION_BASE`, `BUS_OFFSETS`). Item-level
  annotation on each would change how the allow is spelled, not what it
  covers — the narrowest scope that *works* here is the crate level, which is
  what R3 asks for, not "eliminate" (the RFC's own non-goals say so
  explicitly).

`fjell-measuredd`'s existing one-line comment (`// IPC polling idiom: t/w*
are overwritten by sys_ipc_recv`), the shape R3 names to copy, turned out
itself to be stale — re-derived rather than inherited, and removed along with
the rest of that crate's blanket in R2, since a forced rebuild showed it
suppressed nothing today.

`fjell-netd`'s allow is the 13th and is **E-084**, below — it survives as two
narrowed item-level allows, not a blanket, and is reported separately because
unlike the other three it was hiding something.

## R4 — E-084: `netd`'s code now says what the service does

**The loop was kept, not deleted, and not wired live.** `service_main` now
ends cleanly at `sys_exit(0)` with nothing after it. The event loop — real,
finished code: receiving messages, handling `LinkUp`, pre-allocating an
update-metadata session for `secure-transportd` — moved into a new free
function, `run_event_loop(sessions: SessionTable) -> !`, which `service_main`
never calls.

**The stated reason, in the file's own top comment:** the loop is real,
finished implementation whose deletion would be a real loss for whichever
future line wires session/cap-broker integration in, and this line's own
non-goals forbid wiring it in here. As `run_event_loop`, a reader meets it as
`dead_code` the compiler names explicitly ("function `run_event_loop` is
never used") rather than as a code path sitting, unremarked, after a
diverging call in the function that runs — the exact distinction the
governing RFC draws between "the lint doing its job" and "the lint silenced".

Five items needed their own stated, narrow `#[allow(dead_code)]` once the
blanket was gone, confirmed by a forced rebuild after each:
`run_event_loop` itself, `CAP_NETDEV` (already dead since RFC-0.35-002,
re-derived still true), and `SessionTable::new`/`activate`/`count_active` —
the three methods only `run_event_loop` would ever call, itself never called.
`SessionTable::alloc` needed none: it is referenced from inside
`run_event_loop`'s body and the compiler counts that as used even though
`run_event_loop` itself is unreached — confirmed empirically by the warning
list, not assumed from how `dead_code` is usually described.

`ChannelKind`, `MAX_CHANNELS`, and `NetDeviceState` were confirmed genuinely
unused anywhere in the file, including inside the still-dead loop, and were
deleted from the import list rather than allowed.

## R6 — the failing case, shown, before R5 is trusted

Two independent demonstrations that the counter actually moves, not a
counter first seen reporting zero:

1. **An isolated unit test** (`crates/fjell-tools/src/lint_allow_report.rs`,
   `detects_a_scratch_crate_level_allow_appearing_and_disappearing`): writes a
   scratch crate under a temp directory with no allow (not counted), then with
   `#![allow(dead_code)]` (counted), then without it again (not counted) —
   asserting all three states against the same scratch crate. A second test,
   `a_module_level_allow_deeper_in_the_tree_is_not_counted`, confirms a nested
   module's own inner attribute (not a crate root) is correctly excluded.
   Both pass (`cargo test -p fjell-tools lint_allow_report`).
2. **Live, against the real tree**: `fjell-netd`'s `main.rs` was prefixed with
   a scratch `#![allow(dead_code)]` and `cargo xtask lint-allow-report` run —
   count moved **1 → 2** (`fjell-netd, fjell-storaged`) — then the line was
   removed and the count moved back to **1** (`fjell-storaged`). The file was
   restored byte-for-byte from a backup taken before the scratch edit; `git
   diff --stat` on it afterward shows only this line's own intended 43/20
   line change, nothing from the demonstration.

## R5 — criterion 2 now reports the figure it could not see before

`crates/fjell-tools/src/lint_allow_report.rs` (new): walks `crates/` for
crate-root files (`src/main.rs`/`src/lib.rs` directly under a crate's own
`src/`) whose first attribute block opens with a crate-level
`#![allow(...)]`, and returns the sorted crate names. Wired into two places,
neither of which can fail the build:

- `cargo xtask build` (exit criterion 2's own command — `qemu::build_all` in
  `crates/fjell-tools/src/qemu.rs`) prints the census after the kernel build
  succeeds.
- `cargo xtask lint-allow-report`, standalone, for the same figure without a
  full build.

Today's output, against the tip of this line:

```
[xtask] crate-level #![allow(...)]: 1 crate(s): fjell-storaged — each allow is
expected to state its reason inline (RFC-0.37-002 R3); this count does not
fail the build.
```

The release record can now state *"0 warning lines; 1 crate-level allow,
with a stated reason"* — the sentence R5 asks for, with the figure measured
rather than asserted.

## R7 — E-082 and E-084 CLOSED, `v1-limitations.md` in the same commit

Both dated CLOSED blockquotes are in `rfcs/ERRATA.md`, naming exactly what
closed them (the R2/R3 breakdown for E-082; the `run_event_loop` extraction
for E-084); both summary-table rows moved ACCEPTED → CLOSED.
`docs/src/releasing/v1-limitations.md`'s E-081/E-082 bullets are rewritten in
the same commit: E-081's own `fjell-fleetd` half is untouched (still
ACCEPTED, unscheduled — **not this line's business**, per the handoff's own
trap 4), its `fjell-netd`/E-084 half rewritten past-tense as closed; E-082's
bullet rewritten past-tense with the R2/R3/R5 breakdown. The smoke-test-stub
list's stale `fjell-driver-virtio-net` membership (found in R1, a third
instance of the copied "exits by design" sentence) is corrected in the same
file, matching the shape of the two prior corrections already there for
`fjell-secure-transportd` and `fjell-proxy-text`.

`fjell-fleetd` is visibly untouched: no file under
`crates/services/fjell-fleetd/` appears in this line's diff. E-081's own
entry, E-078, and RFC-0.37-001's own escalation (still unruled as of this
line) are all left exactly as they were.

## R8 — the gates, each by its own exit status

| Gate | Result |
|---|---|
| `cargo xtask build` | **exit 0**, 0 warning lines, plus the new `1 crate-level allow(s): fjell-storaged` figure |
| 14 touched crates, forced rebuild (`touch` + build), `riscv64gc-unknown-none-elf` individually | **all 0 warnings**: `fjell-cap`, `fjell-ipc`, `fjell-measuredd`, `fjell-proxy-text`, `fjell-recoveryd`, `fjell-semantic-stream`, `fjell-upgraded`, `fjell-verifyd`, `fjell-attestd`, `fjell-storaged`, `fjell-secure-transportd` (default **and** `--features simulated-transport`), `fjell-driver-virtio-net`, `fjell-netd` |
| `cargo fmt --all --check` | **exit 0** (two files reformatted by a first `cargo fmt --all` pass before this: `fjell-netd/src/main.rs`, `fjell-tools/src/lint_allow_report.rs`) |
| `cargo test --workspace --lib --exclude fjell-proptest` | **exit 0**, every crate `0 failed`, including the two new `lint_allow_report` tests |
| `abi-snapshot --verify` | **PASS**, 455 → 455, 0 added/removed/changed — this line touches no public API surface |
| `consistency-check --all` | **exit 0 — PASS, 20 subchecks**, including `errata-tracking: PASS (84 entries)` and `errata-limitations: PASS (10 ACCEPTED errata, all referenced)` with E-082/E-084 now CLOSED |
| `repro-check` | **rebuilt first** (`fjell-netd`'s source changed): `fjell-netd.bin` moved, all 31 others bit-identical. Baseline re-recorded in the same commit (`rm tests/repro/baseline-digests.txt && cargo run -p fjell-tools -- repro-check --record-baseline`); `repro-check` re-run afterward: **PASS, 32 artefacts identical** |

| `test-all` | **31 tiers, 30 PASS, 0 FAIL, 1 NOT-GATED** (`store`, pre-existing, documented in `v1-limitations.md`). Run `tests/runs/20261017-212430` |
| `release-rehearsal` | **exit 0 — RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS** — gates 1–8, 10, 11, 12 all PASS, including Gate 7 (ERRATA register, 0 OPEN) with E-082/E-084 now CLOSED and Gate 12's `consistency-check` (20 subchecks) |

## Decisions made along the way

1. **`fjell-storaged`'s allow stays crate-level, by choice, not by default.**
   R3 asks for the narrowest scope that *works*; for 13 single-purpose
   constants scattered through a hardware register map, narrower would only
   mean 13 near-identical item-level annotations, not less suppression.
2. **R5's counter lives in `cargo xtask build`'s own code path
   (`qemu::build_all`), not a separate gate.** It is literally what exit
   criterion 2's own command runs; a separate tool would have reported a
   figure for a build nobody ran.
3. **R6 was done twice — an isolated unit test and a live demonstration
   against the real tree.** The unit test is the permanent regression; the
   live demonstration is the literal thing R6 describes ("a scratch allow
   added somewhere, the figure moving, then removed"), kept out of the
   committed diff.
4. **`SessionTable::alloc` needed no allow; `new`/`activate`/`count_active`
   did.** Found empirically, not assumed: a method called from inside a dead
   function's body is not itself flagged dead by the compiler, even though
   the function that calls it is. Worth stating plainly since it is not the
   intuitive shape of `dead_code`.

## Afterword

This line's own risk ("the temptation is to delete code to make a lint
quiet") was real for exactly one crate — `fjell-netd` — and the loop survives
intact, callable by name, not by accident. The other two `unreachable_code`
suppressors turned out to be pure copied boilerplate with nothing behind
them, confirmed rather than assumed, including a third independent instance
of the same stale "exits early" sentence this milestone has now corrected
three times.
