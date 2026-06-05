# RFC-v0.9-005: QEMU Developer Workflow and Service Test Harness

## Status

Draft (revised, supersedes pack v0.9-005 draft)

## Target Version

`v0.9.0`.

## Phase

Developer Service Platform — Epic E (Developer Workflow).

## Related Work

- v0.9 RFCs 001-004 — SDK, manifest, semantic toolkit, bundle.
- v0.4 RFC 001/002 — networking for cross-node smoke tests.
- v0.6 RFC 001 — proptest harness (parallel testing infrastructure).
- v0.5 RFC 001 — board profiles (the QEMU board is selectable).

---

## 1. Summary

Ship a complete **developer loop**: a `fjell-tools dev` CLI command that
builds a service, packages it into a bundle, launches Fjell in QEMU with
the bundle pre-staged, runs a service-specific test harness, captures
audit / semantic streams, and reports pass/fail. Add a **service test
harness** library that lets service authors write integration tests that
run against a real Fjell QEMU instance with hermetic state.

This RFC closes the developer feedback loop: edit → cargo build → see a
working service in a QEMU image with capabilities, semantic emissions,
and audit visibility in under a minute.

---

## 2. Motivation

Today's loop for a new service author is:

- write code;
- rebuild the kernel image;
- run a hand-rolled QEMU command;
- grep through serial output.

This is enough friction that small experiments don't happen. The dev
workflow makes "press one button, see output" the default.

---

## 3. Goals

```text
- `fjell-tools dev run --svc <name>` does:
    cargo build → manifest lint → bundle build → QEMU launch with bundle
    pre-staged → service start → capture streams → exit on idle or
    timeout.
- Test harness `fjell-dev-harness` exposes:
    - launch_qemu(image, options) -> QemuHandle
    - assert_intent_emitted(tag, fields, timeout) -> Result
    - assert_audit_event(kind, timeout) -> Result
    - send_ipc(endpoint, tag, payload) -> Result
    - read_proxy_text(timeout) -> Vec<line>
- Hermetic state: each run starts from a frozen baseline image; storaged
  log is empty.
- Snapshot-based regression: capture the run's audit + semantic stream
  for comparison against checked-in baselines.
```

## 4. Non-Goals

```text
- No GUI / IDE plugin.
- No remote debugging into QEMU (gdb stub support is a separate v0.9.x
  task).
- No automatic generation of test cases.
- No multi-host test orchestration (single QEMU per test).
- No support for non-QEMU emulators in v0.9.0.
```

---

## 5. External Design

### 5.1 dev CLI

```text
$ fjell-tools dev run --svc my-service
[1/6] cargo build my-service
[2/6] manifest lint my-service                       OK
[3/6] bundle build my-service-0.1.0.bundle           OK
[4/6] launch QEMU (image baseline-v0.9.0.elf)        pid 12345
[5/6] stage bundle via shared dir                    OK
[6/6] start my-service                                READY at 2.4s

semantic intents observed:
  0x0100 UPDATE.STAGING_STARTED      cand=R000063
  0x0103 UPDATE.STAGING_CONFIRMED    cand=R000063 slot=B
  ...

dev: idle 10s after READY; shutting down QEMU
dev: exit code 0
```

Options:

```text
--svc <name>            service name from workspace
--no-build              skip cargo build (use existing binary)
--baseline <file>       choose a different baseline image
--timeout <secs>        max wall time (default 60s)
--keep-running          do not exit on idle
--harness-test <name>   run a service-specific test instead of idle wait
--capture <path>        write captured streams to file
--profile <name>        select QEMU board profile (default qemu-virt-v0.9)
```

### 5.2 Test harness API

```rust
pub struct Qemu {
    pid:       u32,
    serial:    SerialStream,
    proxy:     ProxyStream,
    semantic:  SemanticStream,
    audit:     AuditStream,
}

impl Qemu {
    pub fn launch(opts: LaunchOptions) -> Result<Self, DevError>;
    pub fn shutdown(&mut self) -> Result<(), DevError>;

    pub fn wait_intent(&mut self, tag: u16, timeout: Duration)
        -> Result<DecodedIntent, DevError>;
    pub fn wait_audit(&mut self, kind: u16, timeout: Duration)
        -> Result<DecodedAudit, DevError>;
    pub fn read_proxy(&mut self, max_lines: usize, timeout: Duration)
        -> Result<Vec<ProxyLine>, DevError>;
    pub fn send_ipc(&mut self, endpoint: &str, tag: u16, payload: &[u8])
        -> Result<(), DevError>;
    pub fn snapshot_streams(&self, out_path: &Path) -> Result<(), DevError>;
}
```

### 5.3 Hermetic baselines

A "baseline" is a Fjell image plus a frozen storaged snapshot. Baselines
live under:

```text
test/baselines/
   baseline-v0.9.0.elf
   baseline-v0.9.0.storaged.bin
   baseline-v0.9.0.notes.md
```

Each baseline is reproducibly built from the workspace at a tagged
commit. Tests pick the baseline closest to their needs:

- minimal-boot baseline (no extra services);
- with-networking baseline;
- with-fleet baseline (enrolled in a stub fleet).

---

## 6. Data Model

### 6.1 LaunchOptions

```rust
pub struct LaunchOptions {
    pub baseline:       PathBuf,
    pub board_profile:  PathBuf,
    pub extra_bundles:  Vec<PathBuf>,
    pub timeout:        Duration,
    pub serial:         SerialMode,        // CaptureOnly | Tee
    pub net_backend:    NetBackend,        // None | UserSlirp
    pub harness_test:   Option<String>,
}
```

### 6.2 Decoded stream items

```rust
pub struct DecodedIntent {
    pub tag:      u16,
    pub at_tick:  u64,
    pub fields:   Vec<(String, FieldValue)>,
}

pub struct DecodedAudit {
    pub kind:     u16,
    pub seq:      u32,
    pub at_tick:  u64,
    pub fields:   Vec<(String, FieldValue)>,
}

pub struct ProxyLine {
    pub kind:     ProxyLineKind,         // Banner | Pinned | Scroll | Unknown
    pub bytes:    Vec<u8>,
    pub at_tick:  u64,
}
```

### 6.3 Snapshot file

```text
snapshot/
   intents.jsonl
   audit.jsonl
   proxy.txt
   metadata.json     — { baseline_digest, bundle_digests, qemu_args }
```

JSON-lines format chosen because diffs are tractable in CI and human
review is easier than binary.

---

## 7. Internal Design

### 7.1 Stream wiring

QEMU is launched with:

- one serial port → proxy-text stream;
- one virtio-console → semantic stream JSON-lines;
- one virtio-console → audit stream binary;
- optional user-mode networking;
- a 9p-fs export pointing at a directory containing bundles to stage.

Fjell's diagnostics service is configured at baseline-build time to emit
to these consoles in addition to the in-system audit ring.

### 7.2 Hermetic-state guarantee

Before each run, the harness:

- copies the baseline storaged.bin into a scratch directory;
- launches QEMU with the scratch as backing store;
- the QEMU image's storaged uses the scratch file;
- on shutdown, scratch is captured (snapshot) or deleted.

Two consecutive runs of the same harness test produce byte-identical
snapshots modulo a single hermetic_seed and start_tick.

### 7.3 Service-specific harness tests

Services may ship a `tests/dev_harness/` directory:

```text
crates/services/my-service/tests/dev_harness/
   t01_starts_and_ready.rs
   t02_emits_intent.rs
   t03_responds_to_ipc.rs
```

Each test is a `#[fjell_dev_harness_test]`-annotated function. The
harness CLI runs all tests for one service, or a specific name.

### 7.4 Baseline build

```text
$ fjell-tools baseline build --variant minimal --out test/baselines/baseline-v0.9.0.elf
```

Baselines are checked-in artefacts; rebuilding requires the same Rust
toolchain version. CI verifies baseline reproducibility on a schedule
(not on every PR).

---

## 8. Security Design

This RFC introduces no runtime path on the device. Security
considerations:

```text
- The dev workflow runs on the developer's host. It can install bundles
  into a baseline image but cannot affect a real device unless the
  developer manually flashes the image.
- Bundles installed via dev are signed with a *development-only*
  signing key generated by `fjell-tools dev init`. The baseline image
  embeds the dev key as a "dev-only" anchor (DevDigest32 algorithm).
  Production images do not include this anchor.
- Baseline images include the development trust provider (RFC v0.3-001)
  and explicitly do *not* enter Enforcing mode by default, allowing
  the dev workflow to install null-provider fixtures.
```

### 8.1 Audit emission

The dev workflow itself emits no Fjell-side audit beyond what services
emit. The harness records the streams; no privileged operation.

---

## 9. Memory / Resource Design

- Per-run QEMU memory: 256 MiB.
- Snapshot files: ~MB per minute of run.
- Baseline images: ~10 MiB each; small set checked in.

---

## 10. Compatibility and Migration

- The dev workflow is additive; existing service developers can continue
  to use `cargo build` + manual QEMU as before.
- Existing in-tree integration tests can be migrated to the harness; the
  migration ADR plans this for v0.10.

---

## 11. Test Strategy

### 11.1 Self-tests of the harness

```text
- launch_then_shutdown_idempotent
- wait_intent_returns_first_matching
- wait_intent_timeout_returns_error
- send_ipc_round_trip_through_echo_service
- snapshot_streams_jsonl_well_formed
- baseline_reproducibility_check
```

### 11.2 Service-test conventions

A service-specific test passes iff:

```text
- service binary builds clean;
- manifest lints clean;
- bundle builds with reproducible flag;
- harness launches without timeout;
- declared intents emit within timeout;
- declared audit events emit within timeout;
- snapshot matches checked-in baseline (or first-time baseline is
  recorded with --update).
```

### 11.3 Negative

| Marker                                                  | Profile  |
|---------------------------------------------------------|----------|
| `NEG:DEV:BAD_BUNDLE_AT_DEV_INSTALL_REJECTED`            | dev      |
| `NEG:DEV:BASELINE_MISSING_REJECTED`                     | dev      |
| `NEG:DEV:HARNESS_TIMEOUT_FAILS_RUN`                     | dev      |
| `NEG:DEV:SNAPSHOT_MISMATCH_FAILS`                       | dev      |

(All are CI-level harness behaviours, not QEMU markers.)

---

## 12. Acceptance Criteria

```text
- fjell-tools dev subcommand ships.
- fjell-dev-harness library ships.
- At least 3 baseline images checked in (minimal, with-net, with-fleet).
- At least 2 services in the workspace ship dev_harness tests.
- ≥ 6 harness self-tests pass.
- ADR-v0.9-005 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/sdk/dev-loop.md
docs/src/sdk/harness-tutorial.md
docs/src/development/baselines.md
docs/src/operator/qemu-developer.md
docs/src/adr/v0.9-005-baseline-as-artefact.md
docs/src/adr/v0.9-005-jsonl-snapshots.md
```

---

## 14. Open Questions

1. **GDB stub** — debugging into Fjell with `gdb` would be helpful. The
   kernel side of this is non-trivial (single-stepping cap-broker IPC);
   tracked as a v0.9.x task.
2. **Time control** — tests using `wait_intent(timeout)` are wall-clock
   sensitive. A future enhancement may add virtual-time control to QEMU
   so tests are deterministic; for v0.9.0, generous timeouts are
   acceptable.
3. **Multi-service harness composition** — tests that depend on two
   services interacting need two bundles staged. The CLI supports this
   via `--extra-bundles`; coordination across multiple harness tests is
   manual for v0.9.0.

---

## 15. Release Gate (RFC-local)

```text
- dev CLI + harness ship.
- 3 baselines checked in.
- 2 services consume the harness.
- 6 self-tests green.
- ADRs Accepted.
```
