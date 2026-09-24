# RFC-0.33-005 R1 and §A–§D: a deletion, and a test for what stays

**Governing RFC:** [../accepted/RFC-0.33-005-a-parser-with-no-tree.md](../accepted/RFC-0.33-005-a-parser-with-no-tree.md)
**Handoff:** [../handoffs/RFC-0.33-005-a-parser-with-no-tree/implementation-handoff.md](../handoffs/RFC-0.33-005-a-parser-with-no-tree/implementation-handoff.md)

Written after R1 and before any code. Absences probed with `/usr/bin/grep -a`, each
with a control. Tip at R1: `a143f73` (RFC-0.33-004's last commit).

---

## R1 — what the tree says now

| Probe | Result | Control |
|---|---|---|
| Who depends on `fjell-dtb-derive` | **only** `fuzz/Cargo.toml` and its target `fuzz/fuzz_targets/dtb_derive_board_profile.rs`, the root `Cargo.toml` (two lists), and **one comment** in `fjell-dtb-validate/src/lib.rs:135` (prose, not a dependency). No `Cargo.toml` of any crate names it. | the same probe on `fjell-dtb-validate` finds its real dependents: the kernel (`Cargo.toml`, `main.rs`), the fuzz crate and target, the root manifest |
| `derive_board_profile` on the committed QEMU tree | **`Err(MissingPlic)`** (5,044 bytes) | `classify_compat(b"ns16550a")` on the same build gives `Some(Uart8250)`: the probe reaches real code |
| `virtio,mmio` nodes in the tree | **8**, and `classify_compat(b"virtio,mmio")` = **`Some(VirtioNetMmio)`** for every one | (same control) |
| `fjell-dtb-validate` on the same tree against `BoardProfile::qemu_virt_default` | returns `Ok` — **D2's test asserts it**, so it is shown by the test, not by a scratch script | the test's own negative cases (below) |
| **Which functions of `fjell-dtb-validate` have a caller** | **`fdt_extent`, `FDT_HEADER_PROBE_BYTES`, `FdtHeaderError`: the kernel** (`fjell-kernel/src/main.rs`, RFC-0.33-001 D22). **`validate_dtb`: the fuzz target only.** **`ValidationCheck`, `DtbValidationError`, `DtbDigest`, `FDT_MAX_TOTALSIZE`: nobody outside the crate.** (`FDT_MAGIC` is used by `fjell-dtb-derive` — but that is derive's *own* constant of the same name, not an import.) | `AuditRecordBin` finds its callers in the kernel and `fjell-syscall` |
| Is either crate in the ABI snapshot's scanned set | **No**: the tool scans eight crates (`fjell-sdk`, `-syscall`, `-cap`, `-abi`, `-service-api`, `-semantic-v1`, `-audit-format`, `-bundle-format`); `tests/abi/snapshot.json` has **0** mentions of `dtb` | the same file has **18** entries for `fjell-audit-format` |
| The committed tree, twice | `fuzz/corpora/dtb_derive_board_profile/qemu-virt-bios-none.dtb` and `fuzz/corpora/dtb_validate/qemu-virt-bios-none.dtb` are **byte-identical** (`cmp`), 5,044 bytes | — |
| The kernel's dependency list (`cargo tree -p fjell-kernel --target riscv64gc-unknown-none-elf`) | **10 lines / 8 distinct crates:** `fjell-abi`, `fjell-audit-format`, `fjell-cap`, `fjell-ipc`, **`fjell-dtb-validate`**, and, only through it, **`fjell-platform-format`, `fjell-measure-format` and `fjell-canon`** | after D6 this is re-measured |

**A finding about my own previous line.** `fjell-canon` is in that list. It reaches
the kernel because RFC-0.33-003 made `fjell-platform-format` depend on it, and
`fjell-platform-format` reaches the kernel only through `fjell-dtb-validate`. So the
kernel — the most privileged component — grew a dependency by a route neither line
was looking at. D6 removes all three (`platform-format`, `measure-format`, `canon`) from
the kernel's graph, and says so.

### The three figures

At this tip, with `/usr/bin/grep -a`:

| Where | It says |
|---|---|
| `v1-readiness.md`, the fuzz row | **"six decoders, all six fuzzed … run `34976532420`"** |
| `v1-limitations.md`, the fuzzing bullet | **"seven decoders"**, fuzzed at the 0.32.0 cut (run `35089305545`) |
| E-043's closure table (`ERRATA.md`) | **both**: "six kept or added" in one row and "all seven targets" in the weekly-cadence note |

**They already disagree.** `fuzz/Cargo.toml` has **seven** `[[bin]]` targets today; the
readiness row still says six because RFC-0.32-002 added the seventh and updated the
limitations bullet (its own note says so) but not this row. So the deletion does
**not** move "seven → six" uniformly: it moves seven to six in two places and
**corrects a row that was already wrong** in a third — where six is, by coincidence,
the right number *after* the deletion and the wrong number *before* it. I will not
let the coincidence hide it: the row's cited run (`34976532420`) fuzzed the *earlier*
six, and the correction says so and names the run the remaining six were fuzzed in.
The count was never the point; decoders exercised was.

---

## §A — Does `fjell-dtb-validate` keep a fuzz target? **Yes — and it fuzzes more than it did.**

Keep `dtb_validate`. "No caller" does not weaken the case for `validate_dtb`: it takes
firmware-supplied bytes, and a decoder with no caller today is exactly the one that
will be wired in at bring-up (E-004) without having been looked at. **And "no caller"
is false for the header reader:** `fdt_extent` runs on every boot of every tier on the
pointer firmware hands the kernel. The target as it stands does not reach it. So the
same target, **not a new one** (the handoff forbids adding a target to keep a count),
also calls `fdt_extent` on the input — the code the kernel really runs on firmware
bytes is now fuzzed, which it was not.

## §B — The committed QEMU tree. **Keep it; one file, cited from each place that reads it.**

After the deletion the derive copy goes with its corpus directory and
`fuzz/corpora/dtb_validate/qemu-virt-bios-none.dtb` is the **only** copy (they were
identical, checked). It has three consumers, and each says so where it reads it:
the **fuzzer** (the corpus directory, named in the target's doc), **D2's test**
(`include_bytes!` of that exact path — not a copy), and — named because it is the
third — **the kernel**: it validates the header of QEMU's *live* tree every boot, of
which this file is a dump. The test ties the file to the kernel's own claim: it asserts
`fdt_extent(file) == Ok(file.len())` (5,044 — the extent E-064 reserved as two frames),
so replacing the file with another tree would fail there before it silently changed what
two consumers read.

## §C — Anything in the deleted crate worth keeping? **No, and plainly.**

It never parsed a real tree. The header walk and token iterator are the parts a
bring-up line would want, and they are exactly the untested ones: preserving them
preserves a parser that has only ever seen a synthetic tree at the depth it expected.
The record (D4) says it existed, failed on real input, and was removed; git history is
the archive. **Nothing of it is moved, copied or re-exported.** D6's leaf takes 14 lines
from `fjell-dtb-validate`, which *works*, not from the crate being deleted.

## §D — ABI or snapshot change? **None — verified, not assumed.** (R1: 0 entries, control 18.)

---

## D6 — the leaf: `fjell-fdt-header`

`fdt_extent` and what it needs move to a new dependency-free crate,
`crates/fjell-fdt-header/` (`no_std`, `forbid(unsafe_code)`, **no dependencies**):
`FDT_MAGIC`, `FDT_HEADER_PROBE_BYTES`, `FDT_MIN_TOTALSIZE`, `FDT_MAX_TOTALSIZE`,
`FdtHeaderError`, `fdt_extent`, and the five `fdt_extent_tests` — moved with the code,
not rewritten. `fjell-dtb-validate` depends on it for `FDT_MAGIC`; **nothing is
re-exported**, so there is one path to the symbol. The kernel's `Cargo.toml` swaps
`fjell-dtb-validate` for `fjell-fdt-header` and its three uses are renamed. Recorded
before and after: the kernel's dependency list, by `cargo tree`.

## Order

R1 (this) → §A–§D → **D6** (a commit that moves code and touches nothing else) → **D2**
(the QEMU-tree test, red-then-green: it fails when the tree or the profile changes) →
**D1 + D3 in one commit** (deletion of the crate, its target, seeds, membership and
edges; the three figures; E-047's note; ADRs) → E-048 → evidence. **D6 before D1**
because the split touches the crate that stays; **D2 before D1** so the test is written
against a tree that can still fail the old way.

## Decisions the RFC did not specify

1. The leaf's name and location (`fjell-fdt-header`, `crates/`), following
   `fjell-canon`.
2. The fuzz target also calls `fdt_extent` (§A) — one target, more code exercised.
3. The test pins the tree to the kernel's claimed extent (§B).
