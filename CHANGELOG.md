# Changelog

All notable changes to Fjell OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased — v0.0.3 / M3: IPC and Capability]

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
- `fjell-kernel/src/task/user_image.rs`: M3 smoke scenario
  (user0=client ipc_call, user1=server ipc_recv+ipc_reply)
- `main.rs`: static `CAP_TABLE` + `EP_TABLE`; endpoint allocated and
  capabilities installed into user task CSpaces at boot
- Smoke test marker: `TEST:M3:PASS` (both tasks exit(0) via IPC flow)
- `cargo xtask qemu-test m3` smoke gate

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
