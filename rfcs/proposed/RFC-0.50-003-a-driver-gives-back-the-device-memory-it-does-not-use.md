# RFC-0.50-003: A driver gives back the device memory it does not use — and `storaged`'s scan says what it does

**Status:** Proposed — drafted by the implementer for the architect's review; **nothing in this line is implemented**
**Milestone:** 0.50
**Tracks.** Handoff line 3 (`.git-exclude/handoff/handoff-0.50.md`); the follow-up named by `crates/fjell-frame-alloc/src/mmio_mappings.rs:21`–`:22` (*"Having those drivers unmap the transports they do not use is now possible and is a follow-up, not this line."*).
**Errata:** none filed by this RFC. Finding 2 (a comment contradicted by the loop beside it, and a constant the crate-level `allow(dead_code)` hides) is the E-014 class in one function; whether it is an entry is the architect's.
**Touches (proposed):** `crates/services/fjell-storaged/src/main.rs`; `crates/drivers/fjell-driver-virtio-net/src/main.rs` (only if D1 says so); one QEMU profile (D5); `crates/fjell-frame-alloc/src/mmio_mappings.rs` (its follow-up comment); the prebuilts and `tests/repro/baseline-digests.txt` (a service rebuild changes its `.bin`).
**Not in this RFC:** freeing a page-table frame (the walker is 0.51's); any syscall addition or retirement; lease-driven reclamation of MMIO (RFC-0.49-002 D5 ruled it out and `bootstrap-leases` premise 5 guards that).
**Sequencing:** **depends on small item A (E-135), which has landed in this milestone's stage 1**: `mmio-unmap`'s marker artefacts are recorded, so `marker:mmio-unmap:…` now resolves and this RFC can cite its own evidence by marker instead of routing around it as both 0.49 RFCs did.

---

## What is true now

Re-measured at `b5d488d`. The commands are under *Reproduce*.

| Fact | Where / value |
|---|---|
| **`sys_mmio_unmap`'s callers in service and driver code** | **`fjell-neg-test` only** — 9 call sites (the handoff's number, confirmed), in `test_mmio_unmap` and `test_mmio_unmapped_va_faults`. *Count the call form, not the name:* a name-only grep returns four files — the kernel (3 mentions), the wrapper's own definition (1), `fjell-neg-test` (9) and `fjell-service-api`, which only names the syscall in a marker's doc comment (0 calls) |
| `driver-virtio-net` | maps the eight virtio-mmio transports in `0..TRANSPORTS` (`main.rs:109`), reads two registers of each, then **exits** (`sys_exit(0)` after printing). Maps **8** when no NIC is attached (the default boot), **`found slot + 1`** when one is (slot 6 in the `net` profile: 7 maps). Unmaps none |
| `storaged` | maps **eight** — see finding 2 — and keeps all of them for the life of the service. It does not exit |
| `driver-uart` | maps one, keeps it |
| `MAX_MMIO_MAPPINGS` | **64**; the constant's own comment records both drivers and the first boot that refuted 16 |
| Who reclaims what **today** | `mmio_mappings().release_task` runs on every exit and fault (`trap/dispatch.rs`, `task/destroy.rs`), so **the table is reclaimed when a task ends either way**. What is not reclaimed is the mapping **in a long-lived service's page table** while the service lives |

### Findings the handoff did not list

**1. The drivers are not the only code that holds a verb nobody calls.** A census of every `fjell-syscall` wrapper by *production* callers (services, drivers, `fjell-init`; `fjell-neg-test` and the wrapper's own definition excluded) finds **nine** wrappers with none:

```
sys_cap_delete  sys_cap_mint  sys_cap_revoke  sys_dma_revoke  sys_lease_inspect
sys_mmio_unmap  sys_task_spawn_leased  sys_platform_region_resolve (an explicit host-side stub)  sys_probe_unknown (a test probe, by design)
```

**`MmioUnmap` is the newest of a class, not the only member**: five dispatched syscalls — `CapDelete`, `CapMint`, `CapRevoke`, `DmaRevoke`, `LeaseInspect` — have `neg-test` as their only caller (plus `sys_task_spawn_leased`, whose non-leased sibling is what production calls). Some of those are verbs a negative test is *meant* to be the only caller of (a revoke path nobody legitimately exercises in an ordinary boot); that is a classification, not a defect. **I did not widen this line to them** and it is not the handoff's; it is recorded here because the handoff's own sentence — *"a syscall whose only caller is the negative-test suite is dispatched but not used"* — is true of six and not of one. D6 asks whether adoption becomes a rule.

**2. `storaged`'s scan: the comment says one thing, the loop does another, and the constant that would have said so is dead and silenced.**

```
// QEMU virt assigns drives to virtio-mmio buses in reverse order.
// Scan buses from 7 to 0 (highest offset first) for DeviceID=2 (blk).      main.rs:427
const BUS_OFFSETS: [usize; 8] = [0x7000, 0x6000, …, 0x0000];               main.rs:431   ← never read
…  for bus in 0..8usize { let off = bus * 0x1000; sys_mmio_map(…, off, 0x1000) …   main.rs:438  ← ascending
```

`BUS_OFFSETS` is exactly the descending table the comment describes and **nothing reads it**. It survives because `storaged` carries the **only crate-level `#![allow(...)]` the build reports** (`[xtask] crate-level #![allow(...)]: 1 crate(s): fjell-storaged`; `main.rs:14`), whose own comment (RFC-0.37-002) lists `BUS_OFFSETS` among constants *"declared for documentation/future use"*. So the one diagnostic that would have named the contradiction — *constant never used* — was switched off for the whole crate on the strength of a sentence that called this constant documentation. On QEMU `virt` the blk device, attached first, is the **highest** transport, so the ascending loop maps buses 0–6 (all fail the magic/device-id test) and finds blk on **bus 7**, the eighth mapping.

**3. The scan does not unmap what it does not use even when it has already decided it does not need it** — it `continue`s past a transport that is not blk and leaves its mapping in place, for ever. That is the leak this line is named for.

**4. Which transport blk occupies is a function of the QEMU command line, not of the machine.** The `net` profile attaches `virtio-blk-device` before `virtio-net-device` and the NIC is found at **slot 6**, so blk is at 7. A command line with the NIC first would put blk at slot 6. A scan that assumes "highest" is right today and wrong for a different `-device` order, and a scan that *probes and releases* is right for both.

---

## Measured at a boot (to be filled by the stage-2 print-only run, then stated here)

*The kernel counts live MMIO mappings, and a print-only patch in a scratch worktree (never committed) reports the table's length at the end of each task's scan: the peak, and the steady state once `driver-virtio-net` has exited. The handoff's 17 is an upper bound if every holder is alive at once; `driver-virtio-net` exits during its scan, so the steady state is lower. Both numbers go here **as measured**, not derived — I have been wrong on a derived figure in each of the last three milestones.*

---

## The decisions this RFC asks for

**D1 — Which problem, and for whom (the handoff's decision 1).** *Recommended: the problem is `storaged`'s, and only `storaged`'s.* The table is reclaimed at exit for every task, so for `driver-virtio-net` — which exits inside its scan — an unmap before exit gives nothing back that exit does not, and a driver that unmaps eight transports it is about to abandon is ceremony. **`storaged` is the one long-lived holder**, holding seven transports it will never touch; that is a real, permanent, bounded waste of mapping (7 of 64 table entries and 7 pages of its page table). *So `driver-virtio-net` is left alone, and the RFC says why.* **Alternative:** unmap in both for uniformity (and for the second production caller). It costs ~15 lines and buys nothing measurable; I recommend against it, and name the alternative so the architect can overrule.

**D2 — Compare the two changes and choose (the handoff's decision 3).**

| | Reverse the loop (use `BUS_OFFSETS`) | Unmap what does not match | **Both (recommended)** |
|---|---|---|---|
| mappings at the end of the scan, blk first (today's profiles) | **1** | 1 | **1** |
| mappings at the end, blk at slot 6 (NIC attached first) | 2 (7 is a miss, left mapped) | 1 | **1** |
| MMIO reads before blk is found, blk first | **1** | 8 | **1** |
| the comment and the loop agree | **yes** | no (still contradicts the comment) | **yes** |
| `BUS_OFFSETS` stops being dead | **yes** | no | **yes** |
| the production caller of `sys_mmio_unmap` executes in a gated boot | **never** (blk is first: nothing to release) | every boot (7 unmaps) | only in a boot where blk is not first — **needs D5** |

*Reversal alone* is the smallest change and makes the comment true, but it **leaves the leak in place for any other device order** and gives `sys_mmio_unmap` no production caller that ever runs. *Unmap alone* fixes the leak and **leaves the comment lying**. **Recommended: both** — descend through the table the comment already describes (the loop reads `BUS_OFFSETS`; the crate-level `allow(dead_code)` loses a constant it no longer needs), and release a transport the moment it fails the test. The comment and the loop agree afterwards, and a test (D5) holds them to it.

**D3 — What a refused unmap means mid-scan (the handoff's decision 2).** `sys_mmio_unmap` can refuse (another region's capability, an address that is not a recorded mapping). *Recommended: **continue, count, and say once.*** A refusal is bookkeeping, not a verdict on the device: treating it as fatal turns a failed release into a failed boot (a `storaged` that exits is a boot with no storage); ignoring it silently reports success it does not have. So the scan **continues**, counts refusals, and **prints one line after the scan** that states what happened in both cases:

```
storaged: virtio-blk found on transport 7; released 0 unused mapping(s), 0 refused
```

(the numbers vary; the form does not). *The line is the decision made visible:* a refusal changes the second number, and the profile (D5) requires `0 refused`, so **a release that stops working fails the gate instead of being absorbed.** The probe mapping that is *kept* (the one that matched) is not counted as unused.

**D4 — What is a miss.** A transport that is not blk — wrong magic, wrong `DeviceID`, **or a map that is refused** — is a miss and, if it was mapped, is released. A transport whose `sys_mmio_map` fails is not mapped and has nothing to release; it is neither a release nor a refusal, and the line says how many were *not mappable* so that `0 released, 8 unmappable` cannot read as a clean scan.

**D5 — Evidence, and the profile that makes the production caller run.** Because blk is first in every existing profile, D2's production unmap would execute in **no** gated boot, which would leave the adoption this line exists for unproven (the `neg-test` fixtures prove the *verb*, not that `storaged` *uses* it). *Recommended: one new gated profile* — the `store`-shaped command line with a NIC attached **before** blk — in which `storaged` must find blk at slot 6 and print `released 1 unused mapping(s), 0 refused`, with `expect_absent` on the *refused* wording; plus the existing profiles requiring the one-line form with `released 0`. Its markers resolve by `marker:<profile>:…` (E-135 is fixed), so this RFC's Evidence line is the first of the milestone to cite a gated profile by marker. *Alternative:* no new profile; the unmap path is then covered by a host-testable scan function only (extract the decision into the `fjell_driver_virtio_net::scan` pattern) and boots never run it. **I recommend the profile**, because the host function proves the decision and the boot proves the syscall is wired.

**D6 — Whether adoption becomes a rule** (finding 1). *Not recommended for 0.50, offered because it is this milestone's theme one level up.* A gate analogous to `syscall-surface` — *every dispatched syscall has a production caller, or is classified with a reason* — would fail today on the six, and each classification would be a sentence the architect rules on (`CapRevoke`: a negative-test-only verb by design; `DmaRevoke`: the owner's explicit release path nobody calls — a *finding*). It is a bigger decision than this line and I have not started it.

---

## Requirements

**R1** `storaged`'s scan reads `BUS_OFFSETS` in the order the comment states, releases a non-matching mapped transport immediately, never treats a refusal as fatal, and prints D3's line. **R2** `BUS_OFFSETS` no longer needs the crate-level `allow(dead_code)`; **the allow's comment is rewritten to name only what it still covers**, and a build with that constant removed from the list proves it (the RFC-0.37-002 method: delete it and build). **R3** the profile in D5 and the one-line requirement in the profiles that run `storaged` (`store`, `net`, `smoke-m8` and the gated categories that boot it — the list is derived from the profiles, not from memory). **R4** `mmio_mappings.rs`'s follow-up comment rewritten to say it was done and by whom. **R5** `repro-check` and `baseline-digests.txt` re-recorded **in the same commit** as the `storaged` rebuild (numstat shown; the number of prebuilts that change is **one**, and if it is more that is a finding). **R6** `unsafe-audit` unchanged or its delta explained (the scan keeps its `rd32` reads and their `SAFETY` lines).

## Negative cases — what I will show

* **N1.** *The unmap is wired:* in the D5 profile, delete the release (`let _ = sys_mmio_unmap(…)` → nothing) → the profile reads `released 0` where it requires `released 1` → **RED by name**. A compile error is INVALID.
* **N2.** *A refusal is visible:* make the release use a capability for a different region (or a wrong address) → the line says `1 refused` and the profile's `expect_absent` fires → **RED**.
* **N3.** *The comment and the loop cannot drift again:* revert the loop to ascending → the D5 profile's *which transport* wording (`found on transport 7` / `6`) disagrees with the descending expectation, or a host test over the extracted decision fails → **RED**. (If neither can catch it the RFC says so and the comment is the only guard.)
* **N4.** *Peak and steady mappings, measured* and compared with the handoff's 17 and `MAX_MMIO_MAPPINGS = 64`.
* **N5.** `store`, `net`, `net-absent` and `smoke-m8` stay as they were (storage still comes up; the read/write path still passes) — the whole `test-all`, one at a time.

## What this RFC does not do, and why

Does not change `driver-virtio-net` (D1) unless ruled; does not free a page-table frame; does not touch `driver-uart` (it maps the one transport it uses and keeps it); does not widen to the other five caller-less wrappers (D6); does not change `MAX_MMIO_MAPPINGS` (64 is still right — the point is that the *held* count falls, not that the table is too small).

## Reproduce

```
git ls-files 'crates/**/*.rs' | while read f; do n=$(/usr/bin/grep -c 'sys_mmio_unmap(' "$f"); \
  if [ "$n" -gt 0 ]; then printf '%-48s calls=%s\n' "$f" "$n"; fi; done
sed -n '105,130p' crates/drivers/fjell-driver-virtio-net/src/main.rs
sed -n '425,445p' crates/services/fjell-storaged/src/main.rs
/usr/bin/grep -n 'BUS_OFFSETS' crates/services/fjell-storaged/src/main.rs       # :9 (the allow's comment), :431 (definition), nothing else
sed -n '17,24p' crates/fjell-frame-alloc/src/mmio_mappings.rs
cat tests/qemu/artifacts/net/qemu-command.txt ; /usr/bin/grep -a 'driver-virtio-net: found' tests/qemu/artifacts/net/serial.log   # blk first, NIC at slot 6
# the census of finding 1 (production callers = services, drivers, init; neg-test and the definition excluded):
for fn in $(/usr/bin/grep -o -E '^pub fn sys_[a-z0-9_]+' crates/fjell-syscall/src/lib.rs | awk '{print $3}' | sort -u); do
  prod=$(git grep -l -E "\b$fn\(" -- 'crates/services/*' 'crates/drivers/*' ':!crates/services/fjell-neg-test/*' | wc -l)
  [ "$prod" = 0 ] && echo "$fn"; done
```
