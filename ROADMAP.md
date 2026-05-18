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

### M1 · Bootable Kernel
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

## Post v0.1.0

### v0.1.x — Stabilization / Audit / CI Foundation (in progress)

The v0.1.x release line freezes the v0.1.0 prototype, documents its
limitations, and adds the audit + CI foundation needed before
v0.2 modifies security boundaries. It adds no new OS functionality.

See [`docs/src/roadmap/v0.1.x-stabilization.md`](docs/src/roadmap/v0.1.x-stabilization.md)
and RFCs 024–030, 044–047 (`rfcs/`).

| Version  | Theme                                       | RFCs landed       |
|----------|---------------------------------------------|-------------------|
| v0.1.1   | Release freeze + CI foundation              | 024, 025          |
| v0.1.2   | Negative tests + threat model + ABI         | 026, 027, 028     |
| v0.1.3   | Capability / Lease / MMIO / DMA / Evidence  | 029, 030, 044     |
| v0.1.4   | ADR sync + release checklist                | 045, 046          |
| v0.1.5   | v0.2 preparation backlog                    | 047               |

### v0.2.0 — Security Boundary Closure

The first post-v0.1.x hardening milestone. Turns Fjell OS from a
local verified prototype into a system whose core security
boundaries are uniformly enforced. See the v0.2 RFC set (RFCs
031–043) and [`docs/src/security/v0.1.0-threat-model.md`](docs/src/security/v0.1.0-threat-model.md) §14.

| Phase | Name                                        | RFC      |
|-------|---------------------------------------------|----------|
| 1     | Capability Enforcement Core                 | 031, 032 |
| 2     | Lease Revocation Semantics                  | 033, 034 |
| 3     | MMIO Boundary Closure                       | 035      |
| 4     | DMA Boundary Closure                        | 036      |
| 5     | Cooperative Service Separation              | 037, 038 |
| 6     | User Copy and Audit Drain                   | 039      |
| 7     | cap-broker Bootstrap and Policy Enforcement | 040      |
| 8     | Persistent Evidence Hardening               | 041      |
| 9     | Negative Test Completion + Release Gate     | 042, 043 |

### Beyond v0.2

| Version | Theme |
|---------|-------|
| v0.3.0 | Hardware Trust Abstraction |
| v0.4.0 | Minimal Secure Networking |
| v0.5.0 | Multi-Platform Foundation + Semantic API Stabilization |
| v0.6.0 | Verification / Property Testing |
| v0.7.0 | Distributed Snapshot Sync Foundation |
| v0.8.0 | Fleet / Edge Operations Plane |
| v0.9.0 | Developer Service Platform |
| v1.0.0 | First Supported Profile |
