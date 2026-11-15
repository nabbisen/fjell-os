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

### The service plane (as-built, 29 programs)

| Service | Responsibility | Requirement |
|---|---|---|
| `init` | First user-space task; spawns the service plane | FR-BOOT-002 |
| `cap-broker` | Installs capabilities into other tasks' CSpaces (RFC 040/056) | FR-SEC-001 |
| `configd` | Configuration daemon: load, validate, apply, record | FR-SVC-005 |
| `auditd` | Audit event collection and persistence | FR-AUD-* |
| `service-manager` | Service lifecycle: start, health, fault, restart (RFC 058) | NFR-REL-002 |
| `devmgr` | Device discovery, capability assignment, driver startup | FR-SVC-002 |
| `driver-virtio-blk` | Block device driver (user space) | FR-SVC-001 |
| `driver-virtio-net` | Registration stub; exits immediately and is not wired to the device | FR-SVC-001 |
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
| `fleetd` | Fleet operations manager | (fleet, Could-tier) |
| `sample-service` | SDK reference service | FR-DEV-001 |
| `svc-fault`, `svc-timeout`, `neg-test` | Negative-test helpers | (testing) |

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

- **The whole v0.4 network plane is non-functional** (**E-122**), and no test
  environment has ever presented a network device — the QEMU harness attaches
  only `virtio-blk-device`. `driver-virtio-net` is a registration stub,
  `netd` exits before its event loop, and `secure-transportd` reaches a failed
  handshake. The owner has ruled the plane in view; it is a milestone, and a NIC
  in the test environment is its first step rather than the driver.
- **Some services are smoke-test stubs** at v1.0 (`netd`,
  `driver-virtio-net`, and others signal ready and exit by design). This is
  documented in `docs/src/releasing/v1-limitations.md`.
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
