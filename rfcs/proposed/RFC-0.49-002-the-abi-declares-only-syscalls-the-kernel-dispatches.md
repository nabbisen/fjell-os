# RFC-0.49-002: The ABI declares only syscalls the kernel dispatches — `MmioUnmap` built, `DmaShare` decided, the gate made a rule

**Status:** Proposed
**Milestone:** 0.49
**Tracks.** Handoff line 2 (`.git-exclude/handoff/handoff-0.49.md`): *every declared syscall is dispatched, or it is retired.*
`v1-limitations.md` row 7 records three declared-and-never-dispatched syscalls; `TaskKill` is RFC-0.49-001's, and this RFC takes
the other two — **`MmioUnmap = 91`** and **`DmaShare = 111`** — and turns `syscall-surface` from an instrument that *holds a
figure* into one that *refuses the state*. It also restates row 12's bound, which `bootstrap-leases` is built to force the day
`MmioUnmap` is dispatched (RFC-0.48-001). **No erratum is filed**; rows 7 and 12 are the architect's, and proposed wording is in
D8, marked as a proposal.
**Touches** *(indicative)*: `crates/fjell-kernel/src/trap/syscall.rs` (`sys_mmio_map`, a new `sys_mmio_unmap`, the dispatch arm),
`crates/fjell-kernel/src/main.rs` (a new table beside `DmaRegionTable`) or a new module, `crates/fjell-kernel/src/trap/dispatch.rs`
(the death path), `crates/fjell-syscall/src/lib.rs`, `crates/fjell-abi/src/syscall.rs`, `tests/syscall/expected.toml`,
`tests/abi/snapshot.json`, `tools/fjell-consistency-check/src/{syscall_surface,bootstrap_leases}.rs`,
`docs/src/abi/v0.1-inventory.md`, `docs/src/external-design/{ipc,kernel}.md`, two comments in `fjell-service-api`,
`crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/mmio.toml`, prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.48-001** (the bound row 12 records, and premise 5 that guards it), **RFC-0.49-001** (destroy must
release device mappings), **RFC-0.39-004** (`unmap_dma_region_verified`, the shape to copy), **RFC-0.47-001** (one funnel).
**Not in this line:** making lease revocation reclaim anything (a question, D5, not answered by implementation).

*Re-measured at `b2c17b7`. Where the tree and the handoff differ, the section "Where the handoff and the tree differ" says so —
there is one, and it is small.*

---

## What is true now

* **`syscall-surface` already fails when source and `tests/syscall/expected.toml` disagree — and it passes today with three
  undispatched syscalls.** It reads *"36 declared, 33 dispatched, 3 undispatched"* and compares the **names** to an explicit
  `undispatched = ["TaskKill", "MmioUnmap", "DmaShare"]`. So it is not merely "holding a figure": it records a *list*, and a new
  declared-but-undispatched variant passes the moment someone adds its name to that list. **The gate records the state; the
  rule needs it to refuse it.**
* **`MmioUnmap = 91` answers `UnknownSyscall`.** The ABI inventory documents it (`virt_addr: usize` → `SysError`, "partial");
  `docs/src/abi/v0.1-inventory.md:107`. No wrapper exists in `fjell-syscall`.
* **`sys_mmio_map` records nothing** (`trap/syscall.rs:1050`–`:1215`, read in full): after `require_cap`, a bounds check against the
  region, a RAM guard, a per-task bump VA from `task.dev_vma_next`, and `remap_page` per page — and then `a0 = 0`, `a1 = va`. **No insert, no
  table write.** The kernel maps device pages into a task's address space and keeps no record that it did.
* **`mmio_region_table()` is not a table that holds state:** it is a *function* that builds a fresh `[MmioRegionObject; 5]` on every
  call (`qemu_virt.rs`), whose `MmioRegionState::Revoked` is unreachable. It describes the board, not what a task mapped.
* **DMA is the counter-example and the shape.** `DmaRegionTable` (`main.rs:446`, `MAX_DMA_REGIONS` = 16) records `owner`, `user_va`,
  `frame_pa`, `page_count` and a cleanup `state`; `sys_dma_revoke` takes the **caller's own** region by physical address through
  the funnel (`DmaRegion` + `DMA_REVOKE`); `unmap_dma_region_verified` reads the PTE first, requires it to point at the recorded
  frame, unmaps, requires the returned frame to match, **then** flushes the TLB; the death paths run `release_task`.
* **Lease revocation reclaims neither** (handoff correction, confirmed): `sys_lease_revoke` mentions neither table; the DMA table's
  revoke methods have exactly one call site, `sys_dma_revoke`.
* **Who maps MMIO.** Four tasks call `sys_mmio_map`: `driver-uart`, `driver-virtio-net`, `storaged` (one call each) and `neg-test`
  (three calls, **all refused by design** — the RAM guard, a wrong-kind capability, an out-of-bounds offset — so `neg-test` has never
  mapped anything successfully; it does hold `MmioRegion` capabilities for all five regions, slots 31–35, `spawn.rs:508`–`:513`). **Five** regions exist (`MMIO_REGION_COUNT`). The services hold `MmioRegion` capabilities
  with **only `MMIO_MAP`** (`spawn.rs:531`); `init`'s five are `ALL_NON_META` (`main.rs:1650`). **Production live mappings: ≤ 3 — a figure the first boot refuted, see "As built": the drivers map every transport they scan.**
* **The VA window is not the constraint.** Each task's device range is `0x7000_0000`–`0x8000_0000` = **256 MiB = 65,536 pages**, a
  bump allocator that **never reuses** (`dev_vma_next` only grows; `sys_mmio_map` says "never previously mapped"). A map-and-unmap
  loop of one page therefore ends with `NoMemory` after **65,536** cycles. Unmapping does not return the VA, and
  `page_table::unmap_page` **does not free intermediate page-table pages** (its own doc).

### Where the handoff and the tree differ

The handoff calls `DmaShare`'s retirement "cheap and honest". **The doc cost is larger than the code cost:** `ipc.md:84`–`:99`,
`kernel.md:57` and two comments in `fjell-service-api` (`lib.rs:990`, `:1237`) all name `DmaShare` as **"the intended mechanism"**
for bulk transfer, and `RFC-v0.23-001` built a chunked workaround *because* it was missing — **157 round trips for one 4,936-byte
`SemanticEnvelope`, 314 for the emission-plus-forward path.** Retiring it is a **statement that the project has no bulk-transfer
mechanism at v1**, not a tidy-up (D6).

```
cargo run -q -p fjell-consistency-check -- syscall-surface        # 36 declared, 33 dispatched, 3 undispatched
/usr/bin/grep -an 'MmioUnmap\|DmaShare' crates/fjell-abi/src/syscall.rs crates/fjell-kernel/src/trap/syscall.rs
/usr/bin/grep -an 'dev_vma_next' crates/fjell-kernel/src -r
/usr/bin/grep -an 'DEVICE_VMA_BASE\|DEVICE_VMA_END' crates/fjell-kernel/src/platform/qemu_virt.rs
```

---

## The decisions this RFC asks for

**D1 — the record, and its capacity.** A `MmioMappingTable` copying `DmaRegionTable`'s shape: `owner: TaskId`, `region_id`, `user_va`,
`pa`, `page_count: u16`, `state`. **Recommended (as drafted): `MAX_MMIO_MAPPINGS` = 16 — SUPERSEDED, 64 as built (see "As built")**, matching `MAX_DMA_REGIONS`, for a (wrong) **≤ 3** counted
production mappings (`neg-test`'s three attempts are all refused before any entry is made). ≈ 32 bytes × 16 = 512 B, no obligation anywhere
(the DMA table has none). **A new failure mode: the table can be full**, so `sys_mmio_map` **reserves the entry first** and a failure
after it (a `remap_page` error) **releases it again** — no mapping may exist without a record, and no record without a mapping, which
is exactly the property `bootstrap-leases` enforces for boot grants, restated for mappings.

**D2 — who may unmap, and with what.** `sys_mmio_unmap(cap, va)`, **through the funnel**: an `MmioRegion` capability with `MMIO_MAP`
and a live lease, **whose `object_id` equals the recorded region** (a `uart` capability cannot unmap a `net` mapping), and the
caller must be the record's `owner`. **Recommended: no new right** — `MMIO_MAP` (bit 19) is held by exactly the four tasks that map;
a new `MMIO_UNMAP` would need naming at every grant, to the same four. **Stated, not hidden:** the funnel's lease step means *a task whose
`MmioRegion` capability's lease has been revoked cannot unmap its own mapping* — the operator revoked its per-call authority, and the
mapping then stays until the task ends. That is row 12's bound, exact, and not a defect in this design; the alternative (own-record
unmap with no capability, on the `DROP` precedent that releasing needs no right) would let a revoked task shed a resource the
operator may want it to *keep holding to be audited*, which I do not think the project should decide silently. **Your call; the
recommendation is the capability-checked form the handoff names.**

**D3 — the verified unmap.** The DMA discipline, adapted to a frame that **must never be freed**:
1. every page of the mapping is **read first** (`translate`), and each PTE must point at the recorded `pa + i·4096` — **nothing is
   changed unless all pages verify** (two-phase; a mapping may be several pages, unlike DMA's one);
2. `unmap_page` for each; each returned frame must equal the recorded one;
3. **`free_frame` is never called.** `unmap_page` *returns* a frame, and for DMA the caller frees it; **an MMIO frame is a device
   register page, not RAM**, and returning it to the allocator would put a device into the free pool. A guard (`pa` outside
   `[RAM_BASE, RAM_END)`) asserts it, and a negative case below shows the allocator's free count **unchanged** by an unmap;
4. `sfence.vma` **after** the unmap, **before** `Ok`, so the task cannot keep reaching the device through a cached translation;
5. the record is dropped last. **Whole-mapping only**: `va` must equal a recorded base; a partial or interior address is
   `InvalidArg`; a second unmap of the same `va` is `InvalidArg`.

**D4 — death releases the record, and that is a requirement, not a nicety.** The table has 16 entries: **a task that maps and then exits
or faults must give its entries back**, or a restart loop fills it — the E-110 shape inside the new table. The two death arms in
`dispatch.rs` gain a `release_task`, as DMA has; RFC-0.49-001's destroy calls the same function. (The dead task's page table still
maps the device until its page-table frames are freed — deferred in RFC-0.49-001 D4 — which is harmless because the task never runs
again and the page is not RAM.)

**D5 — the question the handoff says to state and not answer: should lease revocation drive reclamation?** *Stated:* an operator
who revokes a lease today withdraws per-call authority and nothing else; DMA and MMIO mappings **stay** until the holder releases
them or ends. Making `LeaseTable::revoke` unmap would turn revocation into a resource-reclaiming act with consequences this RFC does
not weigh — a task faulting on a read it was told it could make, ordering against blocked-IPC wake-up, a verified-unmap failure with
no caller to receive it. **It is not implemented here and not recommended by implication.** What *is* recommended is that the claim
stay true by a guard rather than by habit (D7).

**D6 — `DmaShare`: retire it. Recommended, with the cost stated.**
* **Implementing it is not a bounded task.** A cross-task shared mapping is **a new thing a task holds that the system must be able
  to take back** — this milestone's subject — plus an authority question (who may share what with whom), a lifetime question (the
  owner dies; the sharer dies; both), and a verified-unmap shape for a frame mapped twice. **No task asks for it today**; the one consumer that wanted it (`RFC-v0.23-001`)
  used chunked transfer and works. Building it in the line that exists to *remove* un-reclaimable holdings, with no consumer to
  size it against, is the wrong order.
* **Retiring follows the precedent that exists:** `Reboot = 120` was removed from the enum with the comment that its number is not
  reused (`syscall.rs`, "120 was `Reboot`; reviving it would make old and new ABIs disagree silently"). `DmaShare = 111` becomes the
  same; `from_u32(111)` is `None`; `abi-snapshot` loses one variant.
* **What it costs, plainly:** the docs that call it "the intended mechanism" (`ipc.md` ×2, `kernel.md`, `fjell-service-api` ×2, the
  inventory row, `v0.2-preparation-backlog.md` V02-C-002) must say the opposite — **no shared-region syscall exists, bulk transfer
  is chunked at 32 bytes per round trip, and that is a stated v1 limit, not a pending feature.** *That sentence is a limitation the
  architect may want as a row of its own; I propose it, I do not write it.* **If the owner wants bulk transfer in v1, this is the decision to reverse, and the
  RFC for it is its own line.**

**D7 — the gate, and what replaces `bootstrap-leases` premise 5.**
* **`syscall-surface` refuses.** It computes the undispatched set from source (declared variants without a dispatch arm) and **fails on
  any member, whatever `expected.toml` says**; the `undispatched` key is **removed** from the file (a list the check cannot be
  satisfied by editing). The counts stay as a drift guard: after this line **35 declared, 35 dispatched, 0 undispatched** (36 − the
  retired `DmaShare`; `TaskKill` and `MmioUnmap` dispatched). The failure text names the variant and the two ways out — *dispatch it
  or retire it with its number recorded as unreusable*.
* **Premise 5 (`MmioUnmap` undispatched) fires by design** and must go in the same change. Its *claim* — *revocation does not reclaim
  a mapping* — survives, but its **reason changes**: not "nothing unmaps" but "unmapping is the holder's act and revocation does not
  drive it". The successor premise guards **that**: *the callers of the device-unmap functions are exactly `sys_mmio_unmap`,
  `sys_dma_revoke` and the death path* — a closed set, fail-closed, so the day `LeaseTable::revoke` (or anything else) acquires a
  call the claim must be restated rather than left standing after it stopped being true.

**D8 — proposed wording for rows 12 and 7 (the architect's; I propose, you rule).**
> **Row 12, "The bound that remains":** *…revocation withdraws authority exercised **per call**; it does not reclaim a **resource**. A
> revoked `MmioRegion` capability stops a task mapping again and — since 0.49 — also stops it unmapping what it holds, because
> `MmioUnmap` goes through the same funnel; the pages it mapped stay until it ends, or until a holder with no revoked capability
> unmaps them. `MmioUnmap` is dispatched and verified (0.49), but it is the holder's act. **Neither DMA nor MMIO is reclaimed by lease
> revocation**; `sys_dma_revoke` and `sys_mmio_unmap` are explicit, owner-only, capability-checked. `bootstrap-leases` fails if
> anything other than those two syscalls and the death path calls a device-unmap function.*
>
> **Row 7:** *RESOLVED at 0.49: `TaskKill` and `MmioUnmap` are dispatched; `DmaShare` is retired and its number is not reused.
> `syscall-surface` now fails on any declared-but-undispatched syscall.* A new row records **no shared-region syscall exists; bulk
> transfer is chunked** (D6).

---

## Requirements

**R1 — the table** (D1) and **R2 — `sys_mmio_map` records**, reserve-then-map, release on failure. **R3 — `sys_mmio_unmap`** (D2, D3) with a
`fjell-syscall` wrapper and the dispatch arm. **R4 — the death path** releases a task's mappings (D4). **R5 — `DmaShare` retired** (D6) with the
doc corrections and the `abi-snapshot` delta. **R6 — `syscall-surface` refuses** (D7). **R7 — premise 5 retired, its successor landed** in the
same commit as R3's dispatch arm. **R8 — the prebuilts and the repro baseline** re-recorded in the same commit as any service change.

## Negative cases — what I will show

1. **Map, unmap, read: the read faults.** `neg-test` holds a region-3 (virtio-mmio) capability at slot 34: it maps the first page,
   reads the device magic (a harmless read), unmaps, then reads the old VA — the kernel's own `fault(LoadPageFault)` line plus a
   marker, the `dma` profile's `NEG:DMA:REVOKED_VA_READ_ATTEMPTED` case for MMIO. *(The DMA scenario faults `neg-test` itself on purpose, and only when `init` sends it the trigger tag `RUN_DMA_REVOKED_READ`
   (`main.rs:2347`–`:2355`) — a profile-selected run, not part of the default sequence. The MMIO read takes the same shape: its own
   trigger tag and profile, and nothing after the fault in that run executes.)*
2. **Refused:** an unmap of a `va` that was never recorded; of an interior address; **twice**; by a task that is not the owner; with a
   capability for a **different region**; with a capability whose **lease was revoked** (`LeaseRevoked` — row 12's bound, shown rather than described).
3. **No frame is freed.** The allocator's free count is **identical before and after** a map-and-unmap (the guard of D3.3), and a test
   with the guard removed (a mutation) shows it changing.
4. **A failed map leaves no record**, and a full table refuses the map **before** any PTE is written.
5. **Death releases.** A task that mapped and exits/faults leaves `used = 0`; **mutation: remove `release_task` and the entry survives.**
   *(The boot scenario needs a task that holds an `MmioRegion` capability and can die: `neg-test` cannot die mid-suite, and the existing
   fixtures hold none. Either the table's release function is shown by a host test of the pure table and the syscall path by the
   scenario above, or a fixture is granted one **bound** capability — which adds a holder to the topology and is your call.)*
6. **`syscall-surface` is red when a variant is declared and not dispatched** — the real `syscall.rs` with one variant added, and
   again with one added **and** its name written into `expected.toml` (which must no longer help). **Premise 5's successor is red when
   `LeaseTable::revoke` is made to call a device-unmap function.** Each shown on the real files, restored sha-exact; a compile
   error is INVALID, a RED needs a named failure.
7. **What retiring `DmaShare` leaves behind:** `from_u32(111)` is `None`, `abi-snapshot` is the only delta, and a grep for `DmaShare` finds
   only the retirement comment and the history.

## What this RFC does not do, and why

* **It does not make lease revocation unmap anything** (D5): a question, written as one.
* **It does not recycle device VA.** The bump allocator is kept: 65,536 one-page cycles per task is far beyond any consumer, a free-list
  is a second allocator with a second bug surface, and the bound is stated here rather than discovered.
* **It does not implement `DmaShare`** (D6), and **does not free page-table frames** when a mapping is unmapped — `unmap_page` leaves
  them, as it does for DMA, and RFC-0.49-001 D4 defers the walker with its reason.
* **It does not add a right** (D2) **or a capability grant**; the four holders of `MMIO_MAP` are the four tasks that may now release.
* **It does not claim the unmap is shown on hardware**: QEMU's virtio/UART windows, no silicon (E-004, unchanged).

## Reproduce

```
cargo run -q -p fjell-consistency-check -- syscall-surface
/usr/bin/grep -an 'undispatched' tests/syscall/expected.toml
/usr/bin/grep -rn 'sys_mmio_map(' crates/drivers crates/services | /usr/bin/grep -v 'fn sys_mmio_map'
```


---

## As built (stage 2) — what the implementation changed in this text, and why

* **Capacity is 64, not 16, and the draft's figure was wrong.** I counted *call sites* (four tasks call `sys_mmio_map`, one site each)
  and read that as "≤ 3 live mappings". `driver-virtio-net` and `storaged` each loop over **all eight virtio transports and map every
  one**, never unmapping a transport that is not theirs, and `driver-uart` maps one: **up to 17 live mappings in an ordinary boot**.
  With 16 the first full run failed `net-absent` and `store` (the drivers' maps were refused `NoMemory`) — the gates caught it, not
  the review. 64 entries (≈ 2.5 KiB) leave room. **Follow-up, not this line:** the two drivers can now unmap the transports they do
  not use.
* **The refusal for "no `MMIO_MAP` right" is a wrong-kind refusal on the boot.** Every `MmioRegion` capability `neg-test` holds carries
  `MMIO_MAP` and none can be minted narrower (`MINT` is not granted), so the scenario shows the funnel with an *endpoint* capability
  (`WrongType`). The rights half of the funnel is shown for `TaskControl`/`TASK_KILL` in `fjell_cap::task_kill`'s host tests, not here.
* **`syscall-surface` is the rule (D7), with a dated hold.** `expected.toml`'s `undispatched` key is retired (the parser refuses it);
  `held`, `held_reason`, `held_until` carry the one exception: **`DmaShare`, held to 0.49.0**, because retiring it is the owner's
  decision (the architect has put it to them). **Today: 36 declared, 35 dispatched, 1 undispatched — held, not resolved**, and the PASS
  line says so. The hold fails when stale, reasonless, undated, or once the workspace version passes it.
* **Premise 5's successor (D7) is built:** every call to a device-unmap primitive (`unmap_page`, `unmap_dma_region_verified`,
  `revoke_entry`, `revoke_by_pa`, `revoke_by_id`, `release_task`) sits in one of 10 classified functions with a reason (the holder's
  syscalls, a task's death, the rollback of a failed map); a stale entry and an unclassified caller are both red. **Lease revocation is
  not among them**, shown red by putting a `release_task` call in `LeaseTable::revoke_owned_by`.
* **"No device page reaches the allocator" is observed, not asserted:** `sys_mmio_unmap` logs the allocator's free count before the first
  PTE and after the last, and the `mmio` profile requires `(unchanged)` **and `expect_absent` on the CHANGED wording**, because one
  `(unchanged)` line is no evidence about the others.
* **Death releases the records** (`schedule_next`'s two arms and `destroy_inner`); shown by the host test of `release_task` and the
  restart-loop test, **not by a boot scenario** — no fixture holds an `MmioRegion` capability and can die, and a third fixture exemption
  was ruled out at 0.48.
* **Not built, as stated:** `DmaShare` retirement or implementation (held), lease-driven reclamation (D5, a question), device-VA recycling.
