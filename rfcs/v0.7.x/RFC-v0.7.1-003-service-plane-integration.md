# RFC-v0.7.1-003: Service Plane Integration for v0.4-v0.7 Services

## Status

Draft (closes review findings **W-RB-01, W-M-04**)

## Target Version

`v0.7.1`

## Summary

Embed v0.4-v0.7 service binaries into the bootable kernel image table,
extend `ImageId`, add their entries to `fjell-tools` service build
list and the service-manager manifest, and add new `qemu-test`
categories (`v0.4-net`, `v0.5-platform`, `v0.6-verification`,
`v0.7-sync`) so they are exercised end-to-end.

## Motivation

Whole-project review §4 RB-01:

```text
crates/fjell-tools/src/qemu.rs SERVICES list ends at M8 services.
crates/fjell-kernel/src/task/image.rs embeds images only through M8.
crates/fjell-abi/src/service.rs defines ImageId only through SVC_FAULT = 22.
```

Missing from kernel image table and ImageId enum:

```text
fjell-driver-virtio-net
fjell-netd
fjell-secure-transportd
fjell-diagnosticsd
fjell-identityd
fjell-summaryd
fjell-syncd
```

The crates compile but cannot be spawned by the bootable OS image.
This is the single biggest reason the architect described v0.7.0 as
"closer to a format-first / scaffold-first release."

## Goals

```text
- ImageId enum extended with one variant per v0.4-v0.7 service.
- fjell-tools/src/qemu.rs SERVICES list includes every v0.4-v0.7 svc.
- service-manager manifest spawns the new services at the correct
  startup stage.
- QEMU smoke test categories v0.4-net, v0.5-platform, v0.6-verification,
  v0.7-sync exist and produce TEST:V0.X:PASS markers.
- xtask build-services produces all .bin artifacts.
```

## Non-Goals

```text
- This RFC does NOT make the services functionally complete. That is
  RFC-v0.7.2-001 (identityd/summaryd/syncd) and RFC-v0.7.3-001 (net).
- This RFC ONLY makes the services spawnable.
- No new IPC contracts are defined here.
```

## External Design

### Extended `ImageId`

```rust
// crates/fjell-abi/src/service.rs

#[repr(u8)]
pub enum ImageId {
    // existing v0.1-v0.3
    Init                 = 0x00,
    Configd              = 0x01,
    // ...
    SvcFault             = 0x16,   // 22 — last v0.3 entry

    // v0.4 networking
    DriverVirtioNet      = 0x17,
    Netd                 = 0x18,
    SecureTransportd     = 0x19,
    Diagnosticsd         = 0x1A,

    // v0.7 distributed sync
    Identityd            = 0x1B,
    Summaryd             = 0x1C,
    Syncd                = 0x1D,
}
```

`ImageId` value assignments are stable forever. No reuse of removed
slots.

### `SERVICES` list extension

```rust
// crates/fjell-tools/src/qemu.rs

const SERVICES: &[(&str, &str)] = &[
    // ... existing entries ...
    // v0.4 networking
    ("fjell-driver-virtio-net",  "fjell_driver_virtio_net"),
    ("fjell-netd",               "fjell_netd"),
    ("fjell-secure-transportd",  "fjell_secure_transportd"),
    ("fjell-diagnosticsd",       "fjell_diagnosticsd"),
    // v0.7 distributed sync
    ("fjell-identityd",          "fjell_identityd"),
    ("fjell-summaryd",           "fjell_summaryd"),
    ("fjell-syncd",              "fjell_syncd"),
];
```

### Service-manager manifest

`crates/fjell-service-manager/manifests/v0.7.toml`:

```toml
[stage.network]
order = 50
services = [
    "fjell-driver-virtio-net",
    "fjell-netd",
    "fjell-secure-transportd",
    "fjell-diagnosticsd",
]

[stage.distributed]
order = 70
services = [
    "fjell-identityd",
    "fjell-summaryd",
    "fjell-syncd",
]
depends_on = ["stage.storage", "stage.measurement"]
```

### New QEMU test categories

`cargo xtask qemu-test <category>` expansion:

| Category | Marker | Services involved |
|----------|--------|-------------------|
| `m1`..`m8` | `TEST:M{n}:PASS` | existing, unchanged |
| `v0.4-net` | `TEST:V0.4-NET:PASS` | virtio-net, netd, secure-transportd, diagnosticsd |
| `v0.5-platform` | `TEST:V0.5-PLATFORM:PASS` | devmgr platform/board derive |
| `v0.6-verification` | `TEST:V0.6-VERIFY:PASS` | proptest + unsafe-audit |
| `v0.7-sync` | `TEST:V0.7-SYNC:PASS` | identityd, summaryd, syncd self-checks |

Each category prints a banner on entry, runs the smoke sequence, prints
PASS on success.  Initial v0.7-sync smoke is the existing self-check
each stub already performs; subsequent RFCs grow it.

## Data Model

No new wire types.  Only enum extensions and metadata.

## Internal Design

### Static image table extension

`crates/fjell-kernel/src/task/image.rs` gains entries that
`include_bytes!()` the prebuilt `.bin` for each new service.  This is
the standard pattern from v0.3.

### ImageId allocation discipline

Once `ImageId` reaches `0xFF`, it must transition to a multi-byte
representation. v0.7 occupies 0x17..0x1D, leaving 226 free slots —
sufficient for v0.8 fleet services.

### Smoke test orchestration

`cargo xtask qemu-test v0.7-sync` flow:

```text
1. Build kernel + all services with prebuilt .bin.
2. Boot QEMU.
3. Wait for "Fjell OS kernel started".
4. Wait for "identityd: ready".
5. Wait for "summaryd: release summary ready".
6. Wait for "syncd: envelope self-check passed".
7. Wait for "TEST:V0.7-SYNC:PASS".
8. Exit QEMU with success.
```

If any marker is missed within 30 s, the test fails and the serial log
is uploaded as a CI artifact.

## Security Design

This RFC adds attack surface (more services are spawnable).  The
mitigations:

- Each new service gets only the capabilities its manifest declares,
  via the cap-broker policy bundle.  No bootstrap "all caps" grants.
  (This is the v0.7.4-003 hardening item — RFC-v0.7.4-003 lands the
  capability discipline; this RFC must use the *new* discipline once
  v0.7.4-003 is accepted.)
- Service-manager refuses to spawn a service whose manifest is not
  present.
- The QEMU smoke gate ensures the services boot under the actual
  startup graph, not a hand-built test stub.

## Memory / Resource Design

Each new service binary adds approximately 4-12 KiB to the kernel
image (prebuilt `.bin` size).  Total v0.4-v0.7 service additions are
under 80 KiB.

## Compatibility and Migration

- `ImageId` values 0x00..0x16 are unchanged.  Existing service
  manifests are unaffected.
- Downstream code that pattern-matches on `ImageId` may need to add
  wildcard arms; the compiler will warn.

## Test Strategy

```text
- ImageId::from_u8 / to_u8 round-trip tests cover new variants.
- ImageId variants are exhaustively enumerated.
- xtask build-services produces every .bin in prebuilt/.
- QEMU smoke v0.7-sync produces TEST:V0.7-SYNC:PASS.
- Service-manager rejects a manifest referencing a non-existent
  ImageId.
```

## Acceptance Criteria

```text
- ImageId enum extended with 7 new variants (0x17..0x1D).
- fjell-tools qemu.rs SERVICES list has 30 entries (23 + 7).
- Kernel image table embeds every entry in SERVICES.
- service-manager v0.7 manifest spawns the new services.
- TEST:V0.7-SYNC:PASS appears in QEMU smoke output.
- CI job qemu-v07 (added by RFC-v0.7.1-002) is green.
- ADR-v0.7.1-003 filed.
```

## Documentation Requirements

```text
- docs/src/internals/service-graph.md adds the new stage.network and
  stage.distributed stages.
- docs/src/reference/image-id-abi.md adds the new ImageId values.
- README.md "supported services" list is updated.
```

## Open Questions

```text
1. Should the spawn order between virtio-net and netd be enforced by
   the manifest, or by IPC handshake at runtime? Proposal: manifest
   order for v0.7.1; richer dependency resolution in v0.8.

2. Service-manager currently spawns sequentially. Should v0.7+
   services spawn concurrently within a stage? Proposal: no for v0.7.1;
   add concurrent-spawn in v0.8 with a feature flag.

3. ImageId is u8 today. Should we widen to u16 preemptively to avoid
   a future ABI break? Proposal: keep u8 until 0x80 is reached; the
   v0.8 ABI sweep will revisit.
```

## Release Gate

Acceptance test `TEST:V0.7-SYNC:PASS` must appear in serial output
during `cargo xtask qemu-test v0.7-sync` on a clean checkout of v0.7.1.
