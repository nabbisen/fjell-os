# RFC-0.33-005: A parser with no tree

**Status:** Accepted — by the owner (nabbisen), 2026-09-24; implementation may begin (RFC 000)
**Milestone:** 0.33
**Tracks.** **E-048** — `fjell-dtb-derive` has never derived a board profile from
a real device tree, nothing uses it, and the documents that described its callers
have been corrected.
**Touches** *(indicative)*: `crates/fjell-dtb-derive/` (deleted),
`crates/fjell-dtb-validate/`, `fuzz/`, the root `Cargo.toml`,
`docs/src/adr/ADR-v0.5-002`, `v1-readiness.md`, `v1-limitations.md`,
`docs/src/releasing/…` counts.
**Relates to:** E-047 (a real crash the fuzz harness found in the crate this line
deletes); E-004 (hardware bring-up, where deriving a profile would matter);
E-043 (whose fuzzed-decoder count this line changes).

## Summary

Re-derived 2026-09-24.

### Finding 1 — the decision the owner already took

Deleting `fjell-dtb-derive` was approved in principle at the 0.32 review and
scheduled into 0.33. What remains is to do it without losing what the crate
proved.

### Finding 2 — what it is today

- **No crate depends on it** except `fuzz/`, which fuzzes it.
- On QEMU's own `virt` tree it returns `Err(MissingPlic)`: QEMU nests devices
  under `/soc` at a depth the parser does not read.
- Its `classify_compat` maps **every** `virtio,mmio` node to `VirtioNetMmio`, and
  QEMU's tree has **eight** of them, so no depth fix would let a derived profile
  match the declared one. Which virtio device a node is lives in the device's own
  MMIO register.
- `fjell-dtb-validate`, by contrast, **works**: on the committed QEMU tree
  against `BoardProfile::qemu_virt_default` it returns `Ok`. It also has no
  caller, and the kernel's DTB parser is a stub.

### Finding 3 — deleting it moves three published figures

- The fuzz target `dtb_derive_board_profile` goes, so the **seven** fuzzed
  decoders become six — and the E-043 entry's evidence table names a count, as
  does `v1-limitations.md`'s fuzzing bullet.
- **E-047's regression seed** — the crash the harness found in `get_string` —
  goes with the target. The erratum is CLOSED and stays so; its record must say
  where the seed went.
- `v1-readiness.md`'s fuzz row states a decoder count.

## The settled part

**D1 — `fjell-dtb-derive` is deleted**, with its fuzz target and seeds, its
workspace membership, and its dependency edges.

**D2 — `fjell-dtb-validate` stays, and gains the test it never had**: the
committed QEMU `virt` tree validated against `BoardProfile::qemu_virt_default`,
in Gate 1. A crate that works and is never exercised is how the other one got
here.

**D3 — Every figure the deletion moves is re-derived and corrected in the same
commit** (Finding 3), including the E-043 entry's evidence table and E-047's note about its
seed.

**D4 — The documents keep the history.** ADR-v0.5-002 already carries a
correction; deleting the crate does not delete the record that it existed, failed
on real input, and was removed — that is what a reader needs to not rebuild it by
accident.

**D5 — Boot-time DTB validation is not built here.** It belongs with hardware
bring-up (E-004), and ADR-v0.5-002 and RFC-v0.12-003 already say so.

## The open questions

**§A — Does `fjell-dtb-validate` keep a fuzz target?** It has one today
(`dtb_validate`), and it is the only remaining device-tree decoder. **Lean: keep
it** — firmware-supplied input is exactly what fuzzing is for, even with no
caller yet. Say whether "no caller" weakens that.

**§B — What happens to the committed QEMU tree** (`fuzz/corpora/*/qemu-virt-bios-none.dtb`,
5,044 bytes, used as a seed by two targets)? **Lean: keep it**, and let D2's test
use the same file, so one artefact serves the test and the fuzzer.

**§C — Is there anything in the deleted crate worth keeping** — the FDT header
walk, the token iterator — for the bring-up line that will need it? **Lean: no,
and say so plainly**: it never parsed a real tree, so preserving it preserves an
untested parser. Git history is the archive.

**§D — Does the deletion need an ABI or snapshot change?** The crate is not
published and has no `pub` surface in the snapshot's scanned set. **Verify rather
than assume** — that is R1.

**Answer all four in writing before implementing.**

## Amended at acceptance, 2026-09-24 — `fjell-dtb-validate` now has a caller, and it is the kernel

Finding 2 said the crate *"has no caller, and the kernel's DTB parser is a stub"*,
and **D5** said boot-time validation was not built here. **Both are superseded by
what landed in between** (RFC-0.33-001 D22, erratum E-064): the boot shim used to
destroy the DTB pointer, and the kernel now validates the header — via
`fjell_dtb_validate::fdt_extent` and `FDT_HEADER_PROBE_BYTES` — before it stores
or reserves anything, on **every boot** of every tier.

What that changes for this line:

- **D2's premise is half retired.** The crate is exercised at boot now, but only
  its 14-line header reader; the 645-line validator it lives beside is still
  called by nothing. D2's Gate 1 test is therefore still required, and R1 says
  which functions have a caller and which do not.
- **D5 stands as written for *full* validation** — a board profile checked against
  a real tree at boot is still hardware bring-up (E-004) — and no longer stands
  for the header.
- **D6 (new): RFC-0.33-001 D24's split lands on this line.** `fdt_extent` uses
  neither of the crate's two dependencies, and the kernel should not carry a
  645-line boot-handoff validator plus `fjell-platform-format` and
  `fjell-measure-format` to read eight bytes. Move those 14 lines, their
  constants and their tests into a dependency-free leaf the kernel depends on
  instead. It belongs here because it is this crate, and because doing it twice
  is worse than doing it once.

## Settled at the review, 2026-09-25

**D7 — D6 is accepted, and the figure in it is off by one.** `crates/fjell-fdt-header/`
is the right placement and the right name, and the crate is what it should be: no
dependencies, `no_std`, `forbid(unsafe_code)`, the five header tests **moved, not
rewritten**, nothing re-exported. Counted here with `cargo tree -p fjell-kernel
--target riscv64gc-unknown-none-elf`, the kernel's distinct dependencies are
**five** — `fjell-abi`, `fjell-audit-format`, `fjell-cap`, `fjell-fdt-header`,
`fjell-ipc` — not six. Your *before* figure of eight is right. **Correct the
closure to five**, because the number is a statement about the trusted base.

**And the finding inside it is the best thing in this line.** `fjell-canon` had
entered the kernel's graph one line earlier — RFC-0.33-003 made
`fjell-platform-format` depend on it, and that crate reached the kernel only
through the validator — so the kernel's trusted base grew through a route neither
line was watching. You found it in your own previous line and said so before the
code. **That is the standard: a line that audits its predecessor.**

**D8 — the readiness row does not wait, and the run is owed.** A row that states
what was true at the cut, plus what is owed, is honest; a row held blank until a
run exists tells a reader nothing. Your handling of the count is the part that
matters: the row *already* said six, wrongly, for a different six, and the
tempting move was to leave it alone because the deletion made it read correctly.
Recording that the count went **six → seven → six, and not the same six** is
exactly what the handoff's prohibition was about. **I repaired one thing in that
row at review**: inserting the new note split the sentence *"Decoders a host fuzz
crate cannot reach are named in E-043's closure, not counted"*, leaving a fragment
mid-row. The sentence is whole again; the substance is yours.

**D9 — the fuzz run: I will dispatch it.** The target changed, so the row's citation
should name a run of the changed target. That is a push-and-dispatch, which is
mine, and the row gains the id when it exists.

**Accepted as delivered:** the deletion, complete (no tracked file, no manifest,
no fuzz, no CI entry; the ADRs, the limitations and the readiness row keep the
record, which is D4's whole point); the per-function caller table with its
control; D2's test bound to the **one** committed tree by SHA-256 and to
`fdt_extent(file) == Ok(5044)`, with five controls that each break the tree or the
board and are refused by the check that names them; keeping the fuzz target and
extending it to the header reader rather than adding one to protect a count; and
stating plainly that R4 matches `compatible` strings, not `reg`, so *"the declared
profile still matches the machine we boot"* is weaker than my own phrasing in the
register. **That correction of my wording is right.**

## Requirements

**R1 — Re-derive** Finding 2 and every figure in Finding 3, with `/usr/bin/grep
-a` and a control per absence, plus whether the snapshot's set includes the crate.

**R2 — §A–§D answered in writing.**

**R3 — D1:** the deletion, in one commit, with the workspace, `fuzz/`, CI's lists
(or their replacement, if RFC-0.33-004 has landed) and the prebuilt set all
consistent.

**R4 — D2:** the validation test, from the committed tree, in Gate 1.

**R5 — D3:** the three figures corrected — the fuzzed-decoder count in the E-043
entry, `v1-limitations.md`, `v1-readiness.md` — and E-047's seed noted.

**R6 — E-048 CLOSED** or its survivors named, with `v1-limitations.md`.

**R7 — The gates**, each by its own exit status, `test-all`, a fuzz run (or its
absence explained if no target changed), the repro baseline re-recorded in the
same commit as any rebuild, and a CI run id.

### Non-goals

- Making `fjell-dtb-validate` run at boot (D5, E-004).
- Reading the DTB in the kernel.
- Preserving any of the deleted crate's code (§C).
- Re-deriving board profiles from device trees, on any board.

## Risks

**Deleting a crate that a fuzz target exercises reduces a published count**, and
the honest handling is to state the new number and why it is smaller — not to
leave a bullet saying seven. The count was never the point; decoders exercised
was.

**A deleted parser is easy to rebuild by accident** in bring-up. D4 exists so the
record explains why this one went.
