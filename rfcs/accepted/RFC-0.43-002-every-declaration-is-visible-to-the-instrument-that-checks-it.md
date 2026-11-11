# RFC-0.43-002: Every declaration is visible to the instrument that checks it

**Status:** Accepted — accepted 2026-11-12
**Milestone:** 0.43
**Tracks.** **E-117** — `capability-topology` checks nine fewer declarations than exist, and its
slot-0 exclusion rests on a fallback RFC-0.41-002 deleted.
**Touches** *(indicative)*: `tools/fjell-consistency-check/src/capability_topology.rs`,
`crates/drivers/fjell-driver-virtio-net/src/main.rs`.

---

## What is wrong

`capability-topology` exists so that *"a service's own CSpace declaration and the kernel's install
must agree"* (RFC-0.35-001 R3/R4, **E-072**), and its module doc states the principle it was
rewritten to serve: **"The universe is discovered, not enumerated"** — because an enumerated list
*"would be a third independent declaration of the topology"*, and the rewrite was forced by a
planted `CAP_ARCH_PROBE_12TH` passing silently against the old list.

**Two blind spots make that principle untrue today.**

1. **The universe is discovered within one directory.** `const SERVICES_DIR = "crates/services"`.
   The driver crates are in `crates/drivers/`, so **nine of the nineteen
   `const … CapHandle(<slot>)` declarations in the tree are outside the scan**. A constant planted
   in `crates/drivers/` passes silently — the same defect the rewrite was for, on a different axis.
2. **The slot-0 exclusion is justified by a sentence that is no longer true.** The module excludes
   slot 0 for every service, because it is *"installed unconditionally for every `image_id` by
   `spawn.rs`'s `_ => 0` fallback arm"*. **RFC-0.41-002 R3 deleted that fallback**; `spawn.rs:330`
   now reads `_ => None`, and its comment says *"An image without a dedicated arm gets no
   capability in slot 0."* The *mechanical* half of the exclusion still holds — the install sits
   inside a `match`, not an `if image_id == …` block the module's `image_block_installs_slot` can
   name. What changed is the **consequence**: before 0.41-002 a declared slot 0 could not be wrong,
   so excluding it cost nothing; now it can be wrong, and two declarations sit there unchecked.

**One real defect is hiding in the intersection.**
`crates/drivers/fjell-driver-virtio-net/src/main.rs:58` declares
`const CAP_MMIO: CapHandle = CapHandle(0)` under a header reading `slot 0 — MmioRegion (device
registers)`. It is wrong twice: `DRIVER_VIRTIO_NET` has **no arm** in the `ep_obj` match, so it
receives nothing in slot 0; and that site installs `CapKind::Endpoint`, not `MmioRegion`. MMIO
capabilities install at **`31 + region_idx`**, and virtio-net's region is **3**, so its slot is
**34**. `fjell-driver-uart` is the worked precedent and is correct — `CapHandle(32)` for region 1,
**and it uses it** in `sys_mmio_map`. virtio-net never uses its own, which is the only reason this
has never fired.

## The risk that shapes the design

**Fixing the declaration and leaving the instrument is the shape E-113 records.** That erratum is
about a handoff asking someone to confirm an action pin, nothing re-reading it, and the mismatch
widening for a year. These declarations were noted before 0.41, carried as unverified candidates
through the 0.42 scope, and survived a milestone whose stated subject was the measuring apparatus
— because the instrument that owns the class cannot see them. **The instrument is the line; the
declaration is a consequence of fixing it.**

**The second risk is the waiver list.** Once slot 0 is checked, declarations that are *correct*
will surface alongside the defect — `fjell-secure-transportd`'s `CAP_SESSION = CapHandle(0)` names
the slot where a **runtime cap-broker delivery** arrives, which `WAIVERS` exists for. A waiver is
the designed answer and not a silencer, so each needs a reason that is **true**, and the
temptation to clear a red by widening `WAIVERS` is the failure mode to refuse.

## Requirements

**R1 — The universe is every crate that declares one, not two directory names.** Discovery must
reach a `const … CapHandle(<slot>)` wherever it is written under `crates/`. Replacing one
hard-coded directory with two is the enumerated list the module's own doc rejects, one element
longer.

**R2 — Slot 0 is judged rather than excluded, by attributing the `match` arms.** The `ep_obj`
match maps eighteen images to an object and everything else to `None`; an image with an arm has
slot 0 installed, an image without one does not. Read those arms as installs. **Delete the stale
justification** either way: the module must not keep a sentence about a `_ => 0` fallback that no
longer exists.

**R3 — Correct `virtio-net`'s `CAP_MMIO` to its real slot, and the header line with it.** `34`,
derived as `31 + 3` and checked against `driver-uart`'s correct `32` for region 1 — not copied from
this RFC. If the derivation disagrees with what the tree installs, **report that instead of
applying the number in this requirement.**

**R4 — `secure-transportd`'s `CAP_SESSION` goes in `WAIVERS` with its reason**: slot 0 is where a
capability delivered at runtime by cap-broker arrives, so there is no spawn-time install to find.
Record that cap-broker's `SxtSession = 9` is a `ResourceClass` discriminant and **not** a slot, so
no reader mistakes it for a slot mismatch.

**R5 — Two demonstrations, both failing first, both quoted.**
  1. The extended check **red on today's tree**, naming virtio-net's `CAP_MMIO`, before R3 is
     applied.
  2. A **planted constant in `crates/drivers/`** — the `CAP_ARCH_PROBE_12TH` control on the new
     axis — shown caught, then removed. The module was rewritten because that control passed
     silently once; it must not pass silently again.

**R6 — Every declaration the widened universe surfaces is reported, not dispositioned quietly.**
Nine were unchecked. The expectation is **one defect and several correct declarations**, but that
is a prediction from reading, not a measurement: report what the instrument finds, including
anything this RFC did not predict, and do not waive a surprise to keep the gate green.

### Non-goals

- **Changing any install.** No capability is added, removed or repointed. R3 corrects a
  *declaration* to match an install that already exists.
- **`proxy-text`'s slot 2.** It is an `Endpoint` at `object_id: 0` and it is **correct**: the slot
  is an **introspection subject**, read by `sys_cap_inspect` at `main.rs:44` to obtain
  kernel-verified rights rather than self-asserting a bitmask, and `sys_cap_inspect` never
  dereferences the object. E-117 first called it a defect and withdrew that; **do not "fix" it.**
- **`fjell-init`.** It is spawned from `main.rs`, not through `spawn.rs`'s per-image blocks, and
  the module already says it cannot check it. Extending to `init` is its own question.

## Risks

- **The widened universe may surface more than the one predicted defect.** That is the point, and
  it is also the risk: a cluster of reds is a reason to report, not to narrow R1 until the cluster
  disappears.
- **Attributing `match` arms is a parser change, and a parser that over-attributes is worse than
  the exclusion it replaces** — it would report an install that does not exist. Prefer failing
  closed on a shape you cannot attribute, and say which shapes those are.
- **`WAIVERS` is the pressure point.** If the line ends with more waivers than fixes, say so
  plainly in the review request: that outcome may be correct, and it may equally be the check
  being talked out of its findings.
