# RFC-0.33-005: A parser with no tree

**Status:** Proposed
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
