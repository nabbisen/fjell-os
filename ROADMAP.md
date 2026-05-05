# Fjell OS Roadmap

Development proceeds as a series of focused milestones.  Each milestone
produces a named release archive.  No milestone stretches into the territory
of the next; scope discipline is a first-class constraint.

---

## v0.1.0 — Initial Release

### M0 · Repository Foundation ✅
- Cargo workspace with all crate skeletons
- `no_std` kernel crate, panic handler
- Documentation skeleton, ADR template
- CI pipeline skeleton
- `LICENSE`, `NOTICE`, `TERMS_OF_USE.md`

### M1 · Bootable Kernel ✅
- Linker script (`link.ld`) for QEMU `virt` RAM at `0x8000_0000`
- `_start` assembly: hart selection, BSS clear, stack pointer
- UART 16550A driver (MMIO `0x1000_0000`)
- `kmain()` prints boot banner
- `cargo xtask qemu` runner

### M2 · Memory and Task Isolation
- M-mode shim → S-mode kernel handoff
- DTB-based physical memory discovery
- `BootAllocator` + bitmap `FrameAllocator`
- Sv39 page tables; shared kernel map + per-task user maps
- `TrapFrame`, `KernelContext`, `Task`, `TaskTable`
- Fixed-priority round-robin scheduler, idle task
- `sys_yield`, `sys_exit`
- User page-fault containment → `TaskState::Faulted`
- QEMU smoke test: `TEST:M2:PASS`

### M3 · IPC and Capability
- Synchronous rendezvous `Endpoint`
- `Capability`, `CapRights`, generation-tagged `CapHandle`
- Derivation tree, `cap_copy / cap_mint / cap_delete / cap_revoke`
- `ipc_send / ipc_recv / ipc_call / ipc_reply`
- One-shot reply edge
- Audit hooks for cap / IPC events
- QEMU smoke test: `TEST:M3:PASS`

### M4 · init / service-manager
- `fjell-init` user-space service
- `fjell-service-manager` with TOML service manifest
- Sample service lifecycle (start / exit / fault)

### M5 · Audit and State Export
- `AuditEvent` ring flush to `fjell-auditd`
- JSON Lines export
- `previous_hash` chain for tamper evidence

### M6 · Declarative Configuration
- TOML config schema + validation
- Dry-run, apply, rollback metadata

### M7 · Semantic Stream and Text Proxy
- `IntentNode` full schema
- `fjell-proxy-text` renderer
- `fjell-sample-service` emits intent

### M8 · v0.1.0 Hardening
- Property tests (`proptest`) for cap / IPC / scheduler
- Full unsafe audit with SAFETY comments
- Documentation review
- `CHANGELOG.md` entry, release tag

---

## Post v0.1.0 (Planned)

| Version | Theme |
|---------|-------|
| v0.2.0 | Persistent append-only state store; virtio block driver |
| v0.3.0 | User-space driver manager; immutable upgrade prototype |
| v0.4.0 | Network service; formal model for Capability/IPC |
