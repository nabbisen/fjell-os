# Developer Handoff — RFC-0.33-005

**Governing RFC:** [RFC-0.33-005](../../accepted/RFC-0.33-005-a-parser-with-no-tree.md)
**Milestone:** 0.33
**Status:** inherited from the governing RFC (Accepted, 2026-09-24)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. What this line is, and what it is not

A deletion, and a test for the crate that stays. **The deletion is the easy half.**
The half that matters is that **every published figure the deletion moves is
re-derived and corrected in the same commit** (D3) — a fuzzed-decoder count in the
E-043 entry, in `v1-limitations.md`, and in `v1-readiness.md`'s fuzz row — and that
the record still explains **why** the crate went, so nobody rebuilds it in bring-up
(D4).

## 0.1 Read the amendment first — the ground moved under this RFC

`fjell-dtb-validate` **now has a caller, and it is the kernel**. RFC-0.33-001 D22
(erratum E-064) wired `fdt_extent` and `FDT_HEADER_PROBE_BYTES` into `kmain`: the
header's magic and size are checked on **every boot** before anything is stored or
reserved. So:

- Finding 2's *"has no caller"* is **false as of today** for that function, and
  true for the other 631 lines of the crate. R1 says which functions have a caller.
- **D5 still stands for full validation** (a board profile checked against a real
  tree at boot is hardware bring-up, E-004) and no longer stands for the header.
- **D6 is new and is yours: RFC-0.33-001 D24's split lands here.** `fdt_extent` is
  14 lines and uses **neither** of the crate's two dependencies
  (`fjell-platform-format`, `fjell-measure-format`). The kernel — the most
  privileged component in the tree — should not carry a 645-line boot-handoff
  validator and two format crates to read eight bytes. Move those 14 lines, their
  constants and their tests into a dependency-free leaf, and let the kernel depend
  on that.

## 0.2 Settled — do not re-open

1. `fjell-dtb-derive` is **deleted** — crate, fuzz target, seeds, workspace
   membership, dependency edges (D1).
2. `fjell-dtb-validate` **stays and gains its Gate 1 test**: the committed QEMU
   `virt` tree against `BoardProfile::qemu_virt_default` (D2).
3. Every moved figure corrected **in the same commit**, including E-047's note
   about where its regression seed went (D3).
4. The documents **keep the history**: it existed, it failed on real input, it was
   removed (D4).
5. Boot-time **full** validation is not built here (D5, as amended).
6. The header reader moves to a dependency-free leaf (D6).

## 1. Order

**R1 (re-derive, including the snapshot question and which functions have callers)
→ §A–§D in writing → D6's split → D2's test → D1's deletion → D3's figures →
R6 (errata) → evidence.**

**D6 before D1** because the split touches the crate that stays, and a deletion
commit that also moves code between crates is a commit nobody can review.
**D2 before D1** because a test written after the deletion is a test written
against a tree that can no longer fail the old way.

## 2. R1 — the probes, each with a control

- No crate depends on `fjell-dtb-derive` except `fuzz/` — **control the probe**
  against a crate you know is depended upon.
- `Err(MissingPlic)` on the committed QEMU tree; `classify_compat` mapping every
  `virtio,mmio` node to `VirtioNetMmio`; eight such nodes in QEMU's tree.
- `fjell-dtb-validate` returning `Ok` on the same tree against
  `BoardProfile::qemu_virt_default`.
- **Which of its functions have a caller** (the kernel's two; the rest).
- **Whether the ABI snapshot's scanned set includes either crate** — §D says
  verify, not assume.
- The three figures, at your tip, with `/usr/bin/grep -a`.

## 3. D3 — the figures, and the honest direction

Seven fuzzed decoders become **six**. State the new number and why it is smaller.
**Do not leave a bullet saying seven, and do not add a target to keep the number**
— the count was never the point; decoders exercised was (RFC-0.32-001 §0).
E-047 is CLOSED and stays so: its entry records where its regression seed went.

## 4. §B — the committed tree is one artefact, two consumers

The 5,044-byte `qemu-virt-bios-none.dtb` is a fuzz seed for two targets and, if
the lean holds, the input to D2's test. **One file, cited from both**, and say so
where each reads it — a copy is how the two silently stop being the same tree.
It is also the tree the kernel now validates at boot, which is a third consumer
worth naming.

## 5. Prohibited

- Preserving any of the deleted crate's code (§C) — git history is the archive.
- Making `fjell-dtb-validate` run **fully** at boot (D5, E-004).
- Reading the DTB in the kernel beyond the header the E-064 fix already reads.
- Re-deriving board profiles from device trees, on any board.
- Adjusting a published count to avoid explaining it.
- Deleting the crate and the figures in separate commits.
- A gate piped into `grep` instead of its own exit status.

## 6. Required evidence

1. R1's re-derivations with their controls, including the per-function caller list
   and the snapshot answer.
2. §A–§D in writing.
3. **D6**: the leaf crate, the kernel's dependency list before and after, and the
   tests that moved with the 14 lines.
4. D2's test and its Gate 1 run.
5. D1's deletion, with the workspace, `fuzz/`, CI's lists (or their replacement,
   if RFC-0.33-004 has landed first) and the prebuilt set consistent.
6. D3's three figures corrected in the same commit, and E-047's seed noted.
7. **E-048** resolved or survivors named, with `v1-limitations.md` in the same
   commit as the register.
8. The gates, each by **its own exit status**, `test-all`, a fuzz run or a stated
   reason none was needed, the repro baseline re-recorded in the same commit as
   any rebuild, and a CI run id.

## 7. Review request

Standard format, in `.git-exclude/review-request/`. Flag for focused review:

- **The kernel's dependency list before and after D6**, first.
- The per-function caller list for `fjell-dtb-validate`.
- **The three moved figures**, and anything else the deletion moved that the RFC
  did not name.
- Whether you kept the fuzz target (§A) and why.
- Anything you had to change outside the *Touches* list, which is indicative.
