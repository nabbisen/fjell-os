# Developer Handoff — RFC-0.43-003

**Governing RFC:** [RFC-0.43-003](../../done/RFC-0.43-003-driver-virtio-net-says-only-what-it-does.md)
**Milestone:** 0.43 · **0.43-002 is held and lands with this line** (D24); independent of 0.43-001
**Status:** inherited from the governing RFC (Implemented, 0.43.0)
**Audience:** implementation model

---

## 1. Read the RFC first

Six requirements. **R1 is separable and lands even if the rest stalls** — four console lines that
assert device work with the registers never mapped. R6 is the one that ends the line: your
`e117-wip.patch` applied on top, `capability-topology` green, both pushed as one sequence.

**This line exists because of your submission.** You found layers 1 and 2 and stopped at §6
rather than waiving; the third layer came out of checking your evidence. Your recommended
option (a) was refused and the reasoning is in
`.git-exclude/reviewed/review-record-rfc-0.43-001-and-0.43-002.md` §B.2 — the short form is that
`WAIVERS` means authority a component *does not exercise*, and these slots are exercised.

## 2. The line in the sand

**Do not build a driver.** Not two `DmaRegion`s, not an `Interrupt`, not an `Endpoint`, not slot
20. That is a milestone with an authority design in it, and the RFC says explicitly that it is
neither decided nor foreclosed here. This line makes an image stop claiming what it does not do.

**Do not make the console line optimistic.** R3 allows one line. A reader who sees it should learn
that the image is a stub, not infer that something worked.

## 3. Yours to argue, not mine to dictate

1. **How much of the crate survives.** R2 deletes the declarations *and the paths that use them*.
   Whether that leaves a `main` worth keeping, or whether the honest end state is literally
   `driver-virtio-blk`'s fourteen lines, is yours — **say which you chose and why.** If it
   collapses to the blk stub's shape, that is a result, not a failure of nerve.
2. **The wording of R3's one line.**
3. **Whether `DRIVER_VIRTIO_BLK` losing region 3 belongs in this line or its own commit** (R4). It
   is the same defect found in passing; I think one commit, but argue it if the diff reads better
   split.

## 4. Mechanics and traps

- **`storaged` keeps region 3.** Its `MMIO_SLOT = 34` is the same slot number the two stubs hold,
  because all three get region 3 and the install is `31 + region_idx`. **Confirm before deleting**
  that nothing else resolves slot 34 for the two stub images, and say what you ran.
- **`spawn.rs` is kernel code, so the kernel binary moves** — and so does
  `fjell-driver-virtio-net.bin`. **R5: rebuild, then delete and re-record
  `tests/repro/baseline-digests.txt`, then verify the diff.** The order matters: `--skip-build`
  before a rebuild captures the *previous* binaries and locks in a digest a later real build will
  not reproduce.
- **The absence probe is yours to re-run.** I measured that no judge, expected-marker file or
  harness matches the driver's console lines — only retained logs do. **Re-run that at your tip**
  with `/usr/bin/grep -a` (plain `grep` here skips files containing NUL bytes, and a prebuilt in
  the path will silently drop a file).
- **`IRQ bind failed` is in 5,075 retained artifact files.** Do not try to clean them; they are
  dated evidence of what was true. If a *live* document asserts the driver works, that is a
  finding — report it, do not edit it.
- Expect `cargo xtask build`'s warning count to stay at **0**. Deleting code paths tends to leave
  unused imports and an `#[allow(dead_code)]` that no longer applies.

## 5. Evidence the review request must carry

1. **The console before and after**, from real boots — the eight-line sequence as it is today, and
   what the image prints once R1–R3 land.
2. **The crate's diff summary**: what survives, and your answer to §3.1.
3. **Proof the grant removal is safe** (R4): what you ran to confirm nothing else resolves slot 34
   for those images, and that `storaged` still maps its region — a `store`-touching boot is the
   natural control, even though that tier is NOT-GATED.
4. **The repro-baseline diff** with the lines it contains, and the statement that each is explained
   by the kernel moving or by the driver prebuilt moving. Nothing else may appear.
5. **R6**: `capability-topology` green with `e117-wip.patch` applied, the five cluster lines named
   as gone, and `consistency-check --all` at 21 subchecks.
6. **The absence probe re-run at your tip**, with the command, including a positive control (a
   string you know *is* matched by a judge) so a zero result means something.
7. `cargo xtask build` warning count, and `test-all` — this line changes the kernel, so the QEMU
   tiers are not optional here.

## 6. Stop and escalate if

- Any tier, judge or marker file does depend on the driver's output. That changes the line.
- Removing region 3 from either stub turns anything red, including the `store` tier. It should not;
  if it does, something resolves that slot in a way neither of us has found.
- Deleting R2's code paths leaves the image unable to compile as anything but a bare stub **and**
  you think the stub is the wrong end state. Say so before writing it rather than after.
- The repro-baseline diff contains a line you cannot explain by the kernel or the driver prebuilt.

## 7. Not yours

`rfcs/ERRATA.md` — **E-118**'s closure and **E-117**'s — and any `v1-limitations.md` row, are the
architect's. **`netd` and `TEST:V0.4-NET:PASS` are explicitly not yours to change**: that a reader
of that tier cannot tell the driver died is recorded in E-118, and editing the marker would be
changing an instrument to describe a defect.
