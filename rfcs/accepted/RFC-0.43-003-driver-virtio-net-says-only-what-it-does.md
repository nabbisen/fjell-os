# RFC-0.43-003: `driver-virtio-net` says only what it does

**Status:** Accepted — accepted 2026-11-12
**Milestone:** 0.43
**Tracks.** **E-118** — the image is spawned every boot, exits at its first capability-bearing
syscall, and prints four lines of device work it never did. **Unblocks E-117**, whose instrument
cannot land green while this cluster stands.
**Touches** *(indicative)*: `crates/drivers/fjell-driver-virtio-net/src/main.rs`,
`crates/fjell-kernel/src/task/spawn.rs`,
`crates/fjell-kernel/prebuilt/fjell-driver-virtio-net.bin`, `tests/repro/baseline-digests.txt`.

---

## What is wrong

Three layers, all in **E-118**. The one that decides this line's shape is the third:
**`sys_mmio_map` is never called in the crate.** `CAP_MMIO` is declared at `main.rs:58` and read
nowhere, so the device's registers are never mapped — and the console nevertheless carries
*"device descriptor loaded"*, *"legacy mode (no VIRTIO_F_VERSION_1)"*, *"features negotiated"* and
*"rx queue initialised"*. Four `sys_debug_writeln` calls standing in for work the code does not do.

Then `sys_irq_bind(CAP_IRQ)` on slot 3 — the first call that needs a capability the image was never
given — fails, and the image exits. `init` starts it on every boot.

**The evidence has been in the tree for over three months and nobody read it.**
`IRQ bind failed` appears in **5,075 retained QEMU artifact files** and in 8 run logs, the earliest
`tests/runs/20260731-111016/`. No tier is red, because `TEST:V0.4-NET:PASS` is emitted four times
per boot by `netd`, which never needed the driver. **No judge, expected-marker file or harness
matches any of the driver's console lines** (checked: only retained logs contain them), so nothing
was ever going to notice.

## The decision that shapes the design: who drives the NIC?

Measured, because the answer is already in the tree:

- **`storaged` drives the block device itself.** `sys_mmio_map(CapHandle::new(MMIO_SLOT, 0), …)` at
  `main.rs:439`, plus `sys_dma_alloc`, with `MMIO_SLOT = 34`.
- **`driver-virtio-blk` is a fourteen-line stub** whose body is `sys_exit(0)`. It claims nothing and
  it is honest.
- **`netd` holds only `CAP_OWN_EP`** and no MMIO, and emits the v0.4 net marker.

**So the project's pattern is that services drive devices, and the `driver-*` images are
registration stubs.** `driver-virtio-net` is the only image shaped like a driver, and it was never
granted DMA or an interrupt — it has the MMIO region (region 3, slot 34, the same range `storaged`
holds) and nothing else.

**Ruled: the image becomes an honest stub, matching `driver-virtio-blk`. This line does not build
a driver.** Granting two `DmaRegion`s, an `Interrupt`, an `Endpoint` and slot 20 to a driver image
would add a **permanent structural cost — a privileged driver plane — to avoid nothing**, and it
would contradict the pattern `storaged` and `driver-virtio-blk` already establish. The design bar
is explicit: weigh a permanent structural cost against the smaller diff it avoids, and the smaller
diff usually wins on cleanliness.

**This is not a decision against a real virtio-net driver.** The owner's standing direction is that
functional advancement must precede any v1.0 consideration, and a working NIC is squarely that. It
is a milestone with an authority design inside it — which device plane owns DMA, how an interrupt
is routed to a user task, whether three images should keep sharing the whole virtio-mmio range —
and **none of that is decided here or foreclosed here.** What this line refuses is shipping an
image that *claims* to do it.

## Requirements

**R1 — The four console lines go.** They assert device work with the registers never mapped. This
requirement is **separable from every other one** and is the whole of layer 3: if the rest of the
line stalls, this still lands.

**R2 — All six `CapHandle` declarations go** — `CAP_MMIO`, `CAP_DMA_RX`, `CAP_DMA_TX`, `CAP_IRQ`,
`CAP_NETD_EP`, `CAP_SMGR_EP` — together with the code paths that use them, since an image with no
capabilities cannot bind an interrupt or send on an endpoint. **Delete the header's false
explanation with them:** `main.rs:52` says *"devmgr installs capabilities into these CSpace slots
before starting the driver"*, and devmgr installs nothing.

**R3 — The image exits cleanly, as `driver-virtio-blk` does.** At most one line, and it must not
claim device work. Something of the shape *"driver-virtio-net: registration stub, not wired to the
device"* — the exact wording is yours, the constraint is that a reader of the console learns the
truth from it.

**R4 — The MMIO region grant goes, for both stub drivers.** `mmio_regions_for_service` gives
region 3 to `DRIVER_VIRTIO_NET` **and** to `DRIVER_VIRTIO_BLK`, neither of which maps anything. An
image that exits immediately should hold nothing. `storaged`'s region 3 is **untouched** — it is
the one that uses it.

**R5 — Rebuild and re-record the repro baseline in the same commit.** `spawn.rs` is kernel code, so
the kernel binary moves; `fjell-driver-virtio-net.bin` moves too. `git diff --numstat` on
`tests/repro/baseline-digests.txt` must show **only** the lines those two changes explain — a
baseline regenerated without that check silently absorbs a reproducibility failure and the gate
reports `PASS` from then on.

**R6 — Show E-117's cluster empty, and land the two together.** Apply the held
`.git-exclude/tmp/e117-wip.patch` on top of this line and show `capability-topology` **green** with
the five cluster lines gone and nothing newly red. **Neither line is pushed alone:** RFC-0.32-003
**D24** — *"the deliberately-red tree is not pushed; the whole sequence is pushed when the checks
pass."*

### Non-goals

- **Building a virtio-net driver.** See the ruling above.
- **`netd`, and `TEST:V0.4-NET:PASS`.** The marker is `netd`'s and says what it says. That a
  reader of that tier cannot tell the driver died is recorded in E-118 and is **not** repaired by
  changing netd's marker, which would be editing the instrument to describe a defect.
- **`storaged`'s region 3, or the fact that three images share the whole virtio-mmio range.** A
  real least-privilege question, pre-existing, and not E-118's.

## Risks

- **Removing a grant is the step most likely to break something quietly.** Confirm nothing else
  resolves slot 34 for those two images before deleting their region, and say what you ran.
- **Forgetting R5 turns tier 5 red** on the next run, not on this one — the failure mode that
  earned the release cycle its "Before criterion 4" note.
- **If any tier, judge or expected-marker file turns out to depend on the driver's output**, stop
  and report. Measured as none today — only retained logs match those strings — but that is an
  absence probe and it is **re-run at your tip**, not inherited from this RFC.
- **Deleting code paths can leave dead imports and an `#[allow(dead_code)]` that no longer applies.**
  `cargo xtask build` records a warning count; this line should not raise it.
