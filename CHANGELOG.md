# Changelog

All notable changes to Fjell OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

---

## [0.0.6] — 2026-05-12 — M6: Device / Persistent State / Immutable Upgrade Foundation

### Added
- `fjell-device-format`: `DeviceDescriptor`, `DeviceKind`, `MmioRegionDescriptor`,
  `DeviceState`; hardcoded `QEMU_VIRTIO_BLK` descriptor
- `fjell-block-format`: `BlockDeviceInfo`, `BlockError`
- `fjell-store-format`: `StoreSuperblock` (magic `FJSTORE\0`), `RecordHeader`
  (magic `0x464A4C52`), `RecordKind`, `LBA_*` layout constants
- `fjell-upgrade-format`: `BootControlBlock` (magic `FJBOOT\0\0`), `SlotInfo`,
  `SlotState`, `SlotId`, `UpgradeState`
- `fjell-devmgr`: calls `sys_platform_info_get`, emits "M6: virtio-mmio blk discovered"
- `fjell-driver-virtio-blk`: stub (virtio I/O done inline by fjell-init in M6)
- `fjell-storaged`, `fjell-bootctl`, `fjell-upgraded`, `fjell-powerd`: stubs
- Kernel syscalls: `sys_platform_info_get` (80), `sys_mmio_map` (90),
  `sys_dma_alloc` (110); all wired in dispatch table
- `sys_platform_info_get`: scans virtio-mmio slots 0..7 in reverse, returns base
  PA of first slot with magic=0x74726976 and device_id=2
- `sys_mmio_map`: calls `remap_page` to add R|W|U to an existing kernel-mode
  mapping; uses identity map (user_va == phys_addr)
- `sys_dma_alloc`: returns pre-allocated static `DMA_BUF` at user VA 0x20000000
- `DMA_BUF`: `#[repr(align(4096))]` 16 KiB static DMA buffer; mapped with R|W|U
  into init's page table during task creation (4 pages at 0x20000000-0x20004000)
- `remap_page`: new page-table primitive that overwrites existing PTEs
  (needed to upgrade R|W kernel-only mappings to R|W|U user-accessible)
- Kernel identity-maps all 8 virtio-mmio slots (0x10001000-0x10008000) with R|W
  in all user page tables so `sys_platform_info_get` can scan from kernel mode
- fjell-init M6: inline virtio-blk driver (legacy v1 init, split virtqueue,
  block write), storaged simulation, bootctl A/B mirror write, upgrade staging
- `cargo xtask build-services` and `cargo xtask qemu-test m6` with disk image
  creation (`qemu-img create -f raw fjell-disk.img 16M`)

### Fixed
- `sys_platform_info_get`: added VPN[2]=0 virtio-mmio mapping to ALL user page
  tables (kernel-mode mapping in spawn.rs + kmain init) so scan works from
  within the user task's satp context
- `blk_write_sector` `base` re-read after ecalls: local variable `base` holding
  the MMIO VA was potentially corrupted by t5/t6 register-save bug across ecalls;
  fixed by calling `sys_mmio_map` again to reload `base` from the syscall
- virtio-blk QueueAlign: changed from 4096 to 512 so desc+avail+used fit in
  one DMA page; with QueueAlign=4096 the used ring was at page+4096 which was
  beyond the mapped DMA region
- virtio-blk avail ring offset: was 0x200 (wrong ALIGN interpretation); corrected
  to 0x080 (= N*16 = 8*16 = 128, immediately after descriptor table)
- virtio-blk poll: increased from 500_000 to 5_000_000 iterations to handle
  QEMU subprocess execution slower than interactive mode
- `sys_dma_alloc`: replaced dynamic frame allocation + user-VA mapping with
  pre-mapped static DMA_BUF to avoid `AlreadyMapped` errors when kernel-shared
  L1 tables prevented adding U bit via `map_page`

---

## [0.0.5] — 2026-05-12 — M5: Semantic Operations Plane

### Added
- `fjell-semantic-format`: `IntentNode`, `StateNode`, `EventNode`,
  `SemanticEnvelope`, `TextToken`, `BoundedText`, `FixedVec<T,N>`,
  `ActionRequest`, `ActionResult`, `StreamFilter`, `ExportFormat`;
  invariant validators (`validate_intent`, `validate_state`);
  4 unit tests (valid intent, empty-action error, failed-state error,
  BoundedText roundtrip)
- `fjell-semantic-stream` (new service): publish/subscribe/validate/
  action dispatch; memory-backed intent/state/event rings
- `fjell-proxy-text` (full implementation): text rendering for
  `[STATE]`, `[EVENT]`, `[INTENT]` nodes; `SmokeScenario` auto-selects
  first action; no pixel/color/layout metadata required
- `fjell-init` (M5): full M5 smoke scenario — starts semantic-stream
  and proxy-text; publishes ServiceGraph, ConfigValidated, AuditSummary,
  CapabilityGranted, sample IntentNode; drives action dispatch;
  exports system state as plain text; emits `TEST:M5:PASS`
- `fjell-abi`: `ImageId::SEMANTIC_STREAM = 6`, `ImageId::PROXY_TEXT = 7`
- Kernel image table: `fjell-semantic-stream.bin`, `fjell-proxy-text.bin`
- `cargo xtask qemu-test m5` smoke gate

### Fixed
- Service linker script: stack start pinned to fixed VA `0x80000`
  (`__stack_top = 0x90000`) — previously `__stack_top` varied with
  binary size, causing `SERVICE_STACK_TOP` mismatches after the init
  binary grew to include semantic-format and proxy-text code
- `spawn.rs` + kmain: map all 16 stack pages (64 KiB) instead of one —
  init uses ~32 KB of stack for M5 scenario; single-page mapping caused
  `StorePageFault` at `0x87ec8`
- `Cargo.toml workspace.default-members`: trailing `]` was accidentally
  dropped by a prior edit, corrupting the TOML array

---

## [0.0.4] — 2026-05-12 — M4: Service Plane Bootstrap

### Added
- `fjell-abi`: `LeaseId`, `LeaseEpoch`, `BootInfo`, `ImageId`, `ServiceId`,
  `TaskLifecycle`; M4 syscall numbers (TaskSpawn=40, TaskStart=41,
  TaskStatus=42, TaskKill=43, LeaseCreate=50, LeaseRevoke=51,
  LeaseInspect=52, AuditDrain=60)
- `fjell-syscall`: complete user-space wrappers for all M3+M4 syscalls;
  service runtime module (`rt.rs`) with `_start` + `#[panic_handler]`
- `fjell-service-api`: IPC protocol tag constants for service lifecycle,
  config, cap-broker, audit, and service-manager protocols
- `fjell-cap-broker`: policy-evaluation service; static rule table;
  lease-create/revoke/inspect smoke demonstration
- `fjell-configd`: bootstrap manifest validation; config-get endpoint
- `fjell-auditd`: kernel audit ring drain; JSON Lines emission
- `fjell-service-manager`: service graph bootstrap; sample-service spawn
- `fjell-sample-service`: minimal ready/heartbeat/shutdown service
- `fjell-init` (full implementation): orchestrates configd → cap-broker →
  auditd → service-manager → sample-service; emits audit JSON Lines;
  prints `TEST:M4:PASS`
- `fjell-kernel/src/lease/`: `LeaseTable` with `create`/`revoke`/
  `check_active`/`current_epoch`; MAX_LEASES=32
- `fjell-kernel/src/task/image.rs`: embedded flat-binary service image table
  (`include_bytes!` of six release binaries); `SERVICE_BASE_VA=0x40000`,
  `SERVICE_STACK_TOP=0x51000`
- `fjell-kernel/src/task/spawn.rs`: flat-binary task spawner; multi-page
  text loading; UART + kernel-half mapping in spawned address space
- Kernel M4 syscall handlers: `sys_task_spawn`, `sys_task_start`,
  `sys_task_status`, `sys_lease_create`, `sys_lease_revoke`,
  `sys_lease_inspect`, `sys_audit_drain`
- `sys_debug_write` updated: directly writes a user byte to UART MMIO
- `FRAME_ALLOC` static: `FrameAllocator` moved from kmain stack to BSS static
- `KERNEL_ROOT_PFN` static: kernel root page-table PFN stored for trap-time
  `sys_task_spawn` access
- `FA_RAW_PTR` static: frame-allocator raw pointer stored for `sys_task_spawn`
- TRAP_SCRATCH expanded to `[usize; 3]`: slot [2] holds user sp temp save
- `Scheduler::suspend_current()` used in `block()` (M3 fix carried forward)
- `TaskTable::next_free_index()` helper

### Fixed
- `first_entry`: changed `in(reg) tf` to `in("a0") tf` — when the compiler
  chose `s3` (x19) as the base register, `ld x19, 19*8(s3)` would overwrite
  the base with `TrapFrame.gpr[19]` (often 0), faulting on the next load
- `FrameAllocator` local-to-kmain: trap handler resets sp to `__stack_top`
  on every entry, overwriting kmain's stack frame including `fa_cell`; fix:
  moved to `static FRAME_ALLOC` in BSS
- `SERVICE_STACK_TOP`: was `0x50000`, linker script produces `0x51000`
  (`__stack_bottom=0x41000 + 64K = 0x51000`); incorrect value caused
  `StorePageFault` on first service function call
- `trap/entry.rs`: `sd sp, 2*8(t6)` saved kernel sp (already loaded by
  `ld sp, 0(t6)`) instead of user sp; fixed by: (a) storing user sp to
  `SCRATCH[2]` before loading kernel sp, (b) restoring sscratch to
  SCRATCH_ADDR, (c) loading user sp back from `SCRATCH[2]` into the
  TrapFrame; without this, all service stacks were silently corrupted after
  their first ecall
- `check_smoke_pass` / `task_label`: updated for M4 single-task model (init
  exit = PASS criterion; task labels: init/configd/cap-broker/auditd/…)
- `task/spawn.rs`: `alloc_frame` returns `Result`, not `Option`; replaced
  `ok_or` with `map_err` in all spawn allocation paths
- `task/spawn.rs`: `next_free_index()` returns `Option`; kept `ok_or` there
- `TaskState`: added `#[derive(PartialEq)]` for state comparison in
  `sys_task_status` and `sys_task_start`
- `BlockReason`, `FaultInfo`: added `#[derive(PartialEq)]` for
  `TaskState::PartialEq` to work transitively

---

## [0.0.3] — 2026-05-11 — M3: IPC and Capability

### Added
- `fjell-abi`: expanded `SyscallNumber` (CapCopy/Mint/Delete/Revoke/Inspect,
  IpcSend/Recv/Call/Reply) and `SysError` (InvalidCap, WrongType, SlotEmpty,
  SlotOccupied, RightsExceed, CapTransferDenied, WouldBlock, QueueFull,
  MsgTooLong, Canceled, NoMemory, AlreadyMapped, NotMapped, InvalidAddress)
- `fjell-cap`: `CapHandle` (generation-tagged), `CapRights` bitmask,
  `CapKind`, `Capability`, `CapSlot`, `CSpace` with copy/mint/delete/revoke/
  inspect; derivation tree; host unit tests
- `fjell-ipc`: synchronous rendezvous `Endpoint` (sendq + recvq),
  `PendingMessage` snapshot, `ReplyEdge` (one-shot reply), `MessageTag`;
  host unit tests for IPC-A/B/C invariants
- `fjell-kernel/src/cap/`: `table.rs` (EndpointTable, CapTable, ReplySlot),
  `syscall.rs` (all M3 cap + IPC syscall handlers)
- `fjell-kernel/src/audit/ring.rs`: M3 audit kinds
  (CapCopy/Mint/Delete/Revoke, IpcSend/Recv/Call/Reply/Denied)
- `fjell-kernel/src/task/user_image.rs`: `USER_TASK_C` (denied task — no
  capability, ipc_call → denied → exit(0))
- `main.rs`: 3-task M3 smoke scenario (client, server, denied) with M3 boot
  messages; per-task satp switching; UART mapped in each user page table
- `Scheduler::suspend_current()`: clears current without dequeuing next task
- M3 kernel log messages: "client: call sent", "server: request received",
  "server: reply sent", "client: reply received", "denied: ipc denied as
  expected", "audit: capability.denied", "audit: ipc.call", "audit: ipc.reply"
- Smoke test marker: `TEST:M3:PASS` (all three tasks exit(0) via IPC flow)
- `cargo xtask qemu-test m3` smoke gate

### Fixed
- `fjell-cap/src/cspace.rs`: `gen` renamed to `slot_gen` (Rust 2024 reserved
  keyword)
- `fjell-ipc/src/endpoint.rs`: removed duplicate `pub use` re-export of
  `IPC_WORDS`/`IPC_CAPS` that caused E0252 in Rust 2024
- `fjell-ipc/src/endpoint.rs`: added `#[derive(Debug)]` to `SendResult` and
  `RecvResult` (required by `unwrap_err` in tests)
- `fjell-cap/src/slot.rs`: added `#[derive(PartialEq)]` to `Capability`
  (required by `assert_eq!` in tests)
- `task/user_image.rs`: removed stray `}` after `USER_TASK_A` slice literal
- `cap/table.rs`: removed unused imports `CSPACE_SLOTS`, `CapHandle`,
  `CapKind`, `CapRights`
- `main.rs (m_mode_setup)`: added PMP entry 0 (NAPOT, RWX, all memory) so
  S-mode can access RAM after `mret` — without this every S-mode fetch faults
- `main.rs (kmain)`: DTB reservation now ignores `AlreadyReserved` (DTB may
  overlap kernel image on QEMU virt)
- `main.rs (kmain)`: identity map extended to `__stack_top` (was only
  `boot_end = BSS+2MiB`; stack is BSS+4MiB, causing store_page_fault on
  first kernel stack write after enabling Sv39)
- `mm/page_table.rs (clone_kernel_half)`: now copies root entry 2
  (VA 0x80000000, kernel code/data/stack) instead of entries 256..511 (which
  are all empty for this identity-mapped kernel)
- `trap/dispatch.rs (schedule_next)`: `write_sscratch` called on every
  schedule to restore the sscratch → TRAP_SCRATCH pointer (the trap entry
  `csrrw t6, sscratch, t6` leaves sscratch = user's t6; without this restore
  the second trap would fault at `ld sp, 0(t6)` with t6=0)
- `cap/syscall.rs (block)`: replaced `sched.on_exit()` (which internally
  dequeues and discards the next task) with `sched.suspend_current()` — fixes
  silent task loss when caller blocks on IPC
- `task/scheduler.rs (on_exit/on_fault/on_yield)`: removed internal
  `choose_next()` calls; callers in `schedule_next` now always call
  `choose_next()` exactly once, preventing double-dequeue and task loss
- `cap/syscall.rs (sys_ipc_recv)`: for `is_call=true`, sender is no longer
  prematurely woken on message delivery (it must wait for explicit `ipc_reply`)
- `cap/syscall.rs (sys_ipc_reply)`: caller state is checked before waking;
  Exited/Faulted callers are silently skipped to prevent zombie resurrection


---

## [v0.0.2 — M2: Memory and Task Isolation]

### Added
- M-mode shim → S-mode kernel handoff; `kmain(hart_id, dtb_pa)` signature
- `arch/riscv64/`: CSR helpers, `satp` write, `sfence.vma`, timer (CLINT), PTE types
- `platform/`: `PlatformInfo` with hardcoded QEMU virt constants; DTB pointer forwarding
- `mm/`: `BootAllocator`, bitmap `FrameAllocator`, `FrameOwner`, `MmError`
- `mm/`: `PhysAddr`, `VirtAddr`, `PhysFrame` address types
- `mm/`: Sv39 `PageTable`, `map_page`, `unmap_page`, `translate`; kernel shared map
- `mm/`: `AddressSpace`, `VmRegion`, `VmPerms`, `VmRegionKind`
- `task/`: `TaskId`, `TaskState`, `FaultInfo`, `TrapFrame`, `KernelContext`, `Task`,
           `TaskTable`, `TaskAccounting`
- `task/scheduler.rs`: fixed-priority round-robin `ReadyQueue`, idle task
- `task/user_image.rs`: two embedded RISC-V user tasks (task-a: yield/exit,
                        task-b: yield/fault)
- `trap/`: assembly trap entry (`stvec` direct mode), Rust dispatch, syscall handler,
           fault containment → `TaskState::Faulted`
- `audit/ring.rs`: fixed-capacity append-only `AuditRing`
- `fjell-abi`: split into `lib.rs`, `syscall.rs`, `error.rs`, `task.rs`
- `fjell-tools`: `qemu.rs`, `smoke.rs` submodules; QEMU exit hint (Ctrl-A X)
- QEMU smoke test: `TEST:M2:PASS` marker

### Fixed (v0.0.1 carry-over)
- `publish = false` added to all crate `Cargo.toml` files
- All documentation and comments are now English only
- `cargo xtask qemu` now prints "Press Ctrl-A then X to exit QEMU" on launch
- `code-model=medany` → `code-model=medium` (LLVM name)
- Linker script supplied via `build.rs` (`CARGO_MANIFEST_DIR`-relative absolute path)
- `default-members` excludes `fjell-kernel`; host and kernel builds are cleanly separated
- `#[no_mangle]` → `#[unsafe(no_mangle)]` (edition 2024)
- `static mut UART` → `UnsafeCell`-based console (edition 2024 `static_mut_refs`)

---

## [v0.0.1 — M1: Bootable Kernel]

### Added
- Cargo workspace: resolver = "2", Rust 2024 edition, 17 crate skeletons
- `link.ld`: kernel image at `0x8000_0000`, 4 MiB stack
- `boot.rs`: `_start` assembly — hart-0 selection, BSS zero-fill, stack pointer, `kmain`
- `uart.rs`: NS16550A UART driver (MMIO `0x1000_0000`), `fmt::Write`, CRLF on `\n`
- `console.rs`: `print!` / `println!` macros backed by `UnsafeCell<Uart>`
- `main.rs`: `kmain()` boot banner; panic handler writes to UART then spins
- `build.rs`: linker script via `cargo:rustc-link-arg` with absolute path
- `.cargo/config.toml`: `code-model=medium` for RISC-V target; `xtask` alias
- `crates/fjell-kernel/.cargo/config.toml`: default RISC-V target, QEMU runner
- `.github/workflows/ci.yml`: host check/test + kernel check/build + QEMU M1 smoke test
- `docs/`: full mdBook skeleton (design-philosophy, architecture, ADRs 0001–0005, …)
- `LICENSE` (Apache-2.0), `NOTICE`, `TERMS_OF_USE.md`, `README.md`, `ROADMAP.md`

---

*Releases are tagged once each milestone passes its acceptance criteria.*
