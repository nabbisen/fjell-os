# RFC-0.45-001: The test environment presents a NIC, and the driver reports only what it read

**Status:** Implemented (0.45.0) — accepted 2026-11-17
**Evidence:** marker:net:magic=0x74726976, marker:net-absent:driver-virtio-net: no virtio-net device found (scanned 8 transports), test:crates/fjell-tools/src/qemu_run.rs::the_loader_rejects_expect_absent_with_no_required_marker
**Milestone:** 0.45
**Tracks.** **E-122** — narrows its foundation half: no test environment has ever presented a network
device. Follows through on **E-118** (RFC-0.43-003), whose authority list it explicitly neither took
nor foreclosed. Closes nothing.
**Touches** *(indicative)*: `tests/qemu/profiles/` (two new profiles), `crates/fjell-tools/src/qemu_run.rs`
(one new profile key, §D3), `crates/drivers/fjell-driver-virtio-net/src/main.rs`,
`crates/fjell-kernel/src/task/spawn.rs`, `crates/services/fjell-devmgr/src/main.rs` (§D5),
`tests/qemu/artifacts/{net,net-absent}/`, prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.43-003** (made the image an honest stub and removed its MMIO grant),
**E-111** (the harness's forbidden-marker lesson, §D3), **RFC-0.45-002** (independent — different
files, different authority).

---

## What is true now

Re-measured at `ec74c47`, not inherited:

- `extra_args` is appended to the QEMU argv (`qemu_run.rs:600`) and **every** `extra_args` in
  `tests/qemu/profiles/*.toml` is `[]`; the argv is `-machine virt -bios none -nographic -kernel …
  -drive … -device virtio-blk-device,drive=hd0`. No `-netdev`, no `virtio-net-device`. So this is a
  **configuration absence, not a missing harness capability**.
- This QEMU (11.1.3) offers `virtio-net-device` and the `user`, `socket`, `hubport` netdev backends
  (queried with `-device help` / `-netdev help`; nothing started).
- `driver-virtio-net` is a 44-line stub and holds **no MMIO region** (RFC-0.43-003 R4). It is 177 bytes.
- The virtio-mmio range is kernel region 3 (`0x1000_1000`, `0xF000` long; eight 0x1000-byte
  transports). `storaged` already scans it: for `bus` in 0..8 it maps `bus * 0x1000`, reads
  `MagicValue` (+0x000) and `DeviceID` (+0x008) and takes the first `DeviceID == 2`. The driver would
  do the same for `DeviceID == 1`. Where QEMU puts a second device among the eight is **not assumed**;
  it is measured at implementation and the scan does not depend on it.

### A correction to my own earlier statement, and a new finding of the E-118 class

In RFC-0.43-003's review request I called `devmgr: registered virtio-net` *"devmgr's own line and
… true"*. **That was wrong.** `devmgr` builds `BoardProfile::qemu_virt_default` — a static table — and
prints `registered virtio-net` for every `DeviceClass::VirtioNetMmio` entry it contains. It reads no
device. **The line appears on every boot, with no NIC attached**, and has since the profile was
written. It is the same shape as the four lines RFC-0.43-003 deleted (a verb implying work, nothing
read), one layer out. It matters here specifically: the absence tier of §D4 would print `devmgr:
registered virtio-net` immediately above `driver-virtio-net: no virtio-net device found`. §D5.

## The decision that shapes the design: who holds the region, and with what

Four candidate holders were weighed against *"who is the one component whose job is this device"*:

| Holder | For | Against |
|---|---|---|
| **`driver-virtio-net`** (chosen) | It is the image whose name, `ImageId` and spawn already exist; the discovery is its first act; a driver with no region is the stub RFC-0.43-003 had to make honest. | Region 3 is the whole virtio range — see "the over-grant". |
| `devmgr` | It already enumerates and has `MMIO_MAP` in the policy table. | Its enumeration is a static table today; making it read devices re-scopes a different service, and it would hold the region on every boot whether or not any driver wanted it. |
| `netd` | It will consume the device. | It must stay `netd` — the session router. A router that also maps device registers is the privileged-driver-plane shape RFC-0.43-003 declined to build. |
| `storaged` | It already scans the range. | It is the block service; lending it NIC discovery crosses two planes. |

**Rights: `MMIO_MAP` only.** No `INSPECT`, no `COPY`/`MINT`: the driver never inspects or derives the
capability. **No IRQ, no DMA, no endpoint.** Discovery reads two ID registers; an interrupt line and a
DMA region would be exactly the standing, unexercised authority RFC-0.43-003 removed
(`DRIVER_VIRTIO_BLK`'s `DmaRegion` followed it out in the 0.43 small items). They are granted by the
line that exercises them.

**The over-grant, stated rather than hidden.** Region 3 covers all eight transports, including the
block device `storaged` drives. `sys_mmio_map` maps the page read-write; a driver holding region 3
*could* write the block device's registers, and **the kernel cannot enforce that the driver only
reads**. What bounds this is the driver's source (two `read_volatile`s, each annotated
`MMIO-ORDER: status_read`, auditable by the existing `mmio-audit`) and the fact that the same reach
is already held by `storaged`, `driver-virtio-blk` was *granted* until 0.43-003, and `neg-test`.
RFC-0.43-003 ruled *"three images sharing the whole virtio-mmio range"* a pre-existing least-privilege
question and not its own; this RFC follows that ruling and names the residual. The cure — a per-
transport region object, so a driver holds only its own slot — needs the slot to be known before the
grant (QEMU assigns it), which is a harness-coupled kernel change and its own line.

## Requirements

**R1 — two profiles, and the committed command shows the device.** `tests/qemu/profiles/net.toml`
(`extra_args = ["-netdev", "user,id=n0,restrict=on", "-device", "virtio-net-device,netdev=n0"]`) and
`net-absent.toml` (no network `extra_args`). `restrict=on` stops the guest reaching the host network;
there is no packet path in this line and no reason to offer one. The profiles' `qemu-command.txt`
(tracked, one per gated profile) is the evidence a reader checks; the `net` one must show
`virtio-net-device`, and the `net-absent` one must not. The existing smoke `v0.4-net` is untouched.

**R2 — the driver discovers, and reports only what it read.** It scans the eight transports via its
`MmioRegion` capability (slot 34 = `31 + 3`, declared as a `CapHandle` constant so
`capability-topology` judges the declaration against the install, as it now does for `driver-uart`),
reads `MagicValue` and `DeviceID`, and prints **one** line:

* found: `driver-virtio-net: found virtio-mmio device id=1 at slot <N> magic=0x74726976`;
* not found, after a **completed** scan: `driver-virtio-net: no virtio-net device found (scanned 8 transports)`;
* the scan could not run (`sys_mmio_map` refused every slot): `driver-virtio-net: cannot scan the
  virtio-mmio region (<error>)` — **a distinct line, never "no device found"**, because absence is
  only reportable after a successful scan. Claiming absence it did not verify is the E-118 shape from
  the other side.

No feature negotiation, no queue initialisation, no rx/tx, no `Ready` claim. It exits 0 in all three.
`src/core/`'s 1,293 lines stay unlinked, as ruled.

**R3 — the authority is installed and nothing more.** `spawn.rs` grants region 3 to
`DRIVER_VIRTIO_NET` again (`MMIO_MAP` only) — the exact inverse of RFC-0.43-003 R4 for this image
(`DRIVER_VIRTIO_BLK` is **not** re-granted). Nothing else changes.

**R4 — a negative tier for absence, red if the driver claims a device.** With no NIC attached the
driver must report absence and **no device fact**. `net-absent` requires the absence line and fails
if any `driver-virtio-net: found` line is present (§D3 for how). This is the first gate on the E-118
class: for five thousand artefacts the old image printed three device facts with `sys_mmio_map` never
called and nothing failed, and **a tier that passes only when a device is present leaves that hole
exactly where it was**.

**R5 — it is shown failing.** A mutation table in the review request, each restored byte-exact:
 1. the device **absent** from `net` → red (tier 4 red by construction: it expects `found`);
 2. the grant **removed** → the driver prints the *cannot scan* line, `net` red **and `net-absent`
    red too** (it must not print "no device found" when it did not look) — the case the two-line
    distinction exists for;
 3. the discovery read **stubbed to a constant** (always `DeviceID == 1`) → `net-absent` red via R4;
 4. the discovery read stubbed to always miss → `net` red;
 5. a planted `CapHandle` constant in the driver crate → `capability-topology` red (the 0.43-002
    control, on this crate).

**R6 — what moves is stated.** Prebuilt `fjell-driver-virtio-net.bin` and the kernel; the repro
baseline is re-recorded in the same commit (rebuild → delete → record → verify); the ABI snapshot is
unmoved unless a `negative_markers` constant is added (none is planned — the profiles assert console
lines, not `NEG:*` markers).

### Decisions I am asking for rather than making

**D3 — "red if it prints a device fact" needs a profile-level *absent* assertion, and E-111 refused
the nearest thing.** The harness's forbidden list is global and hard-coded; `net` *must* print the
`found` line, so it cannot be global. E-111's closure refused a profile-level `forbidden_markers` field
because *"the hazard is prose drifting from a hard-coded list"* — a field duplicating the global
list. This is a different thing: a **scenario-specific** negation with no hard-coded twin and no prose
that could drift from it. I propose `expect_absent = ["driver-virtio-net: found"]` — each prefix must
**not** occur in the run — with a loader check that rejects a profile whose `expect_absent` also
appears in its own `expected_markers` (a self-contradiction), and the same discipline as
`expect_equal`: shown red on a real boot. If the architect rules it out, the alternative is to encode
the absence in the *driver's* output (one marker the absent profile requires and the present one
forbids — which still needs a negation), so the field is the honest form. **Please rule.**

**D5 — `devmgr: registered virtio-net`.** Smallest honest fix: the line says what the table says —
`devmgr: board profile declares virtio-net` (and likewise for the other four classes). No judge or
profile depends on the wording (swept: `tests/qemu`, `crates/fjell-tools`, `tools`, `docs/src`). It
widens this line by one service and one prebuilt; the alternative is leaving the contradiction on the
absent tier's console and naming it in the RFC. I recommend fixing it here, because the absent tier is
the thing the architect will read first.

### Non-goals

A packet path; `netd`'s event loop; `src/core/`; `fjell-net-format`'s two layouts (0.44-002 was
withdrawn for want of a consumer, and a NIC does not supply one: discovery reads a register, it does
not write a frame); feature negotiation; interrupts; the per-transport region (named above).

## Risks

- **Slot assignment.** QEMU's choice of transport for the second device is not assumed; the scan
  covers all eight and reports the slot it found. If a QEMU version places it outside the eight, the
  `net` tier goes red rather than the driver lying.
- **CI's QEMU.** `-netdev user` needs the slirp/passt backend; present here, to be confirmed against
  the CI image at implementation. `restrict=on` is a property of that backend.
- **Timing.** The `net` profile adds a device; the kernel never touches it, so no logical counter
  should move. Measured and reported, not assumed (the 0.42-002 lesson).
- **The over-grant** (above) is real and is not closed here.

## Rulings — architect, 2026-11-17 (`.git-exclude/reviewed/review-record-0.45.md`)

**Accepted. The decisions this RFC asked for are settled here; reasoning is in the review record.**

* **D3 `expect_absent`: accepted as proposed**, including the loader check that rejects a profile
  whose `expect_absent` also appears in its own `expected_markers`. **E-111's refusal does not reach
  this:** E-111 refused a profile field that would *duplicate a global hard-coded list*, with prose
  free to drift from it; this has **no hard-coded twin** and is scenario-specific. Required, as
  offered: **shown red on a real boot**, the discipline `expect_equal` already follows.
* **D5 `devmgr` wording: fix it here.** `devmgr: board profile declares virtio-net`, and the same
  for the other four classes. Leaving the contradiction directly above the absence tier's own output
  would put the defect on the operator's console.
* **The holder — `driver-virtio-net`, `MMIO_MAP` only, no IRQ, no DMA, no endpoint: accepted.** The
  over-grant of region 3 is accepted as **inherited, not as fine**, on RFC-0.43-003's ruling; it is
  accepted *because* this RFC names it rather than hiding it. The per-transport region is its own
  line.
* **The three distinct driver lines — found / none-after-a-full-scan / cannot-scan: accepted**, and
  this is the part the architect would have asked for had it not been proposed. "No device found"
  printed when the grant is missing would be **E-118 from the other side**.
