# Developer Handoff — RFC-0.45-001

**Governing RFC:** [RFC-0.45-001](../../done/RFC-0.45-001-the-test-environment-presents-a-nic.md)
**Milestone:** 0.45 · third line — the one to drop if the milestone runs short
**Status:** inherited from the governing RFC (Implemented, 0.45.0)
**Audience:** implementation model

---

## 1. Read the RFC and its Rulings section first

The *Rulings* settle D3 (`expect_absent` **accepted**, with the self-contradiction loader check),
D5 (fix the `devmgr` wording here), the holder (`driver-virtio-net`, `MMIO_MAP` only), and the three
distinct driver lines.

**Read RFC-0.43-003 before writing anything.** It reduced this driver to a stub because the image
printed four device facts with `sys_mmio_map` never called. **This line is the first that may
legitimately print a device fact, and it must earn each one.**

## 2. The line in the sand

**The absence tier is the half that matters.** A gate that only passes when a device *is* present
leaves E-118's hole exactly where it was. With no NIC attached, the driver reports absence and
prints **no** device fact — red if it does.

**Absence is only reportable after a completed scan.** Three distinct lines: found / none after a
full scan / **cannot scan**. "No device found" printed when the grant is missing would be E-118 from
the other side: a true-sounding line from a path that never looked.

## 3. Yours to argue, not mine to dictate

* Whether this extends `v0.4-net` or adds a profile.
* The scan's shape across the eight transports, and what it reports when the device sits outside
  them.
* The exact `-netdev` backend flags, against CI's QEMU as well as this machine's.

## 4. Mechanics and traps

* **`extra_args` already reaches the argv** (`crates/fjell-tools/src/qemu_run.rs:600`); every
  profile's is `[]`. The committed `qemu-command.txt` must show the device — that file is the
  evidence a reader checks.
* **Serial output lives in `tests/qemu/artifacts/<profile>/serial.log`**, not in `tests/runs/`,
  whose tier logs keep only the marker summary. A grep of `tests/runs/` for a console line proves
  nothing — it cost me a wrong conclusion at the 0.44.0 cut.
* **The over-grant is accepted as inherited, not as fine.** Region 3 covers all eight transports and
  the kernel cannot enforce read-only. Two `read_volatile`s, each annotated for `mmio-audit`, and
  nothing more.
* **`expect_absent` must be shown red on a real boot**, the discipline `expect_equal` follows.
* **Timing:** measure and report whether any logical counter moved; do not assume. That is the
  0.42-002 lesson.

## Evidence the review request must carry

* The mutation table: **every required refusal shown red**, each by its own mutation, each restored
  byte-exact afterwards.
* A full `cargo xtask test-all` on the tip with this line in it, with the tier counts.
* If a prebuilt moved: `tests/repro/baseline-digests.txt` re-recorded **in the same commit**, and
  the `numstat` compared against the binaries the build reported as modified — **exactly those and
  no others**.
* `consistency-check --all` and `fmt --check`.
* **Anything you measured that contradicts this handoff or the RFC.** Three of the last four
  milestones had one, and every time the implementer was right to raise it.

## Stop and escalate if

* a requirement cannot be met without widening authority beyond what the RFC's Rulings settle;
* a gate you add would pass for a reason other than the one it claims to test;
* you find a register row or limitations row that is **false** rather than merely due an update.

## Not yours

`rfcs/ERRATA.md`, every `docs/src/releasing/v1-limitations.md` row, and every RFC status line.
Flag them in the review request rather than editing them.
