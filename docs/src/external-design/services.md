# External Design — User-Space Services

*Subsystem 5 of 9. Anchored to FR-SVC-001…006, NFR-REL-001/002 and
`crates/services/` at v0.21.2.*

## 1. Responsibility

Everything that is not the kernel's ten core duties runs here: drivers, the
device manager, storage/file-system, configuration management, and the base
userland. Each service is an independent `no_std` RISC-V program in its own
address space, communicating only over kernel-mediated IPC. A fault in one
service does not halt the kernel (NFR-REL-001) and services are individually
restartable (NFR-REL-002).

## 2. External surface

### The service plane (as built: every program under `crates/services/` and `crates/drivers/`)

*This heading used to carry a count (29), and the table beneath it matched that count while the tree held more:
five programs were missing from it, and two other figures in circulation (the kernel's prebuilt images, the
number of crates) disagreed with both. A count a person types is the thing that drifted, so none is stated
here. To count: `ls -d crates/services/*/ crates/drivers/*/ | wc -l` for programs and
`git ls-files 'crates/fjell-kernel/prebuilt/*.bin' | wc -l` for the images the kernel embeds (every program but
`summaryd`, which has none). The `service-table` subcheck compares the **set**: every program
directory is named by a row below and every row names a program (RFC-0.50-002 D5); it does not
read the Responsibility or Requirement columns, and nothing does.*

| Service | Responsibility | Requirement |
|---|---|---|
| `init` | First user-space task; spawns the service plane | FR-BOOT-002 |
| `cap-broker` | Installs capabilities into other tasks' CSpaces (RFC 040/056) | FR-SEC-001 |
| `configd` | Configuration daemon: load, validate, apply, record | FR-SVC-005 |
| `auditd` | Audit event collection and persistence | FR-AUD-* |
| `service-manager` | Service lifecycle: start, health, fault, restart (RFC 058) | NFR-REL-002 |
| `devmgr` | Device discovery, capability assignment, driver startup | FR-SVC-002 |
| `driver-virtio-blk` | Block device driver (user space) | FR-SVC-001 |
| `driver-virtio-net` | Discovery only: scans the eight virtio-mmio transports through one `MMIO_MAP` capability, reports whether a `DeviceID=1` device is present — or that it could not scan — and exits. It does **not** drive the device: no DMA, no interrupt, no virtqueue (RFC-0.45-001) | FR-SVC-001 |
| `driver-uart` | UART RX driver: binds UART0's interrupt, reads one byte per interrupt and delivers it by IPC to `init` (no shell, no echo) | RFC-0.25-001 |
| `storaged` | virtio-blk I/O / storage service | FR-SVC-003 |
| `bootctl` | Boot-control (A/B slot management) | FR-BOOT-003 |
| `upgraded` | Immutable A/B upgrade staging | FR-BOOT-003 |
| `verifyd` | Signature verification | FR-BOOT-001 |
| `rootfsd` | Immutable rootfs namespace | FR-SVC-003 |
| `snapshotd` | State snapshots | FR-SVC-004 |
| `measuredd` | Measurement chain | FR-BOOT-001 |
| `attestd` | Local attestation | NFR-SEC-004 |
| `recoveryd` | Recovery plane | FR-SEC-004 |
| `powerd` | Power/sustainability telemetry | FR-KRN-006, NFR-GRN-* |
| `netd` | Packet/session routing | FR-SVC-001 |
| `secure-transportd` | Authenticated control-plane channel — **present but non-functional**: in a release build its handshake is an **unimplemented stub that returns `HandshakeFailed` unconditionally** (`perform_tls_handshake`, commented *"for now fails closed"*), so it fails on every boot **whatever capabilities it holds**. Two further gaps sit behind it, each independent: its `CAP_SESSION` is never delivered (**E-123**), and the implementation plan's first step reads a `sys_net_recv` that **does not exist** in the ABI, the dispatch table or the wrapper (**E-122**) | FR-SEM-004 |
| `syncd` | Offline-first snapshot sync | (fleet, Could-tier) |
| `diagnosticsd` | Diagnostic bundle builder | FR-AUD-002 |
| `semantic-stream` | Semantic/Intent stream service | FR-SEM-001 |
| `proxy-text` | Text Presentation Proxy (sample) | FR-SEM-002 |
| `proxy-braille` | Braille Presentation Proxy: the second presentation of the same stream, as uncontracted braille patterns on the console (no braille display is driven, E-004) | RFC-0.34-001 |
| `fleetd` | Fleet operations manager | (fleet, Could-tier) |
| `summaryd` | Measurement and release summary exporter (no prebuilt image: the kernel does not embed it) | RFC v0.7-003 |
| `sample-service` | SDK reference service | FR-DEV-001 |
| `svc-fault`, `svc-timeout`, `svc-presentation-fault`, `svc-presentation-liar`, `neg-test` | Negative-test helpers (the two `svc-presentation-*` are test-only presentations: one that faults mid-run, one that claims rights it does not hold) | (testing) |

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-SVC-001 User-space drivers | Drivers are sandboxed user programs; restartable in isolation | `driver-virtio-blk/net`, `service-manager` |
| FR-SVC-002 Device manager | Discovery + cap assignment + driver startup + fault detect | `devmgr` |
| FR-SVC-003 File-system service | Read-only system, append-only state, config, audit regions | `storaged`, `rootfsd` |
| FR-SVC-004 Append-only state store | Tamper-evident, ordered, exportable | `snapshotd`, append-only audit store |
| FR-SVC-005 Config management | TOML load, schema validate, diff, pre-apply validate, history, rollback | `configd`, `fjell-config-format` |
| FR-SVC-006 Single-binary base userland | Statically-linked minimal-dependency service binaries | each service is one flat binary |
| NFR-REL-001 Fault isolation | Driver/service fault is contained to that address space | per-service address space |
| NFR-REL-002 Restartability | `service-manager` restarts individual services | RFC 058 lifecycle |

## 4. Service bootstrap contract (external)

Every private-endpoint service follows a four-point coordination pattern:
`et.alloc()` in `main.rs`, a match arm in `spawn.rs`, a `cs.install_raw`
capability in the init CSpace, and a `wait_service_ready` call before first IPC.
`static mut` is forbidden in services (it causes BSS-write page faults in
`no_std` RISC-V) — services use loop-local stack variables.

## 5. As-built scope limits & gaps

- **The v0.4 network plane still moves no packet** (**E-122**), though its first
  step landed in **0.45.0**. A test environment now presents a network device: the
  `net` profile attaches `-netdev user,restrict=on -device virtio-net-device` and
  `net-absent` attaches none, each profile's committed `qemu-command.txt` being the
  evidence, and `driver-virtio-net` **reads the virtio-mmio range and reports what it
  found** — never claiming an absence it did not verify. **What is still missing is
  everything above discovery:** nothing sends or receives a packet, `sys_net_recv`
  exists nowhere, `netd` exits before its event loop, `secure-transportd`'s release
  handshake fails by construction, and the virtqueue logic in
  `fjell-driver-virtio-net`'s library is **not linked into the image**. The owner has
  ruled the plane in view; it is a milestone, and 0.45.0 did the first step — the
  environment and honest discovery — not the driver.
  *(Corrected 2026-11-18 at the 0.45.0 cut, by exit criterion 8: this bullet said no
  test environment had ever presented a network device and that the harness attaches
  only `virtio-blk-device`. RFC-0.45-001 made both false.)*
- **Some services are smoke-test stubs** at v1.0 (`netd` and others signal ready
  and exit by design). This is documented in
  `docs/src/releasing/v1-limitations.md`. **`driver-virtio-net` is no longer among
  them** since 0.45.0: it exits immediately, but it does so having mapped and read
  eight transports and reported a measured result.
- **Base userland commands (FR-SVC-006)** — the requirement lists list/read/
  write/etc. as a single-binary userland. At v1.0 the service plane exists but
  the full interactive command set is not the focus; the SDK reference service
  demonstrates the pattern.
- **svc lifecycle negative coverage is partial** (READY pair 2/4, timing
  sensitive).

## 6. Related subsystems

Services depend on [Capability & Lease](./capability-lease.md) for their
authority, [IPC](./ipc.md) for communication, and emit into
[Audit & Observability](./audit-observability.md) and
[ABDD / Semantic Streams](./abdd-semantic.md).
