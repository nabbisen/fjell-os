# Fjell OS v0.7.1 Release

## Version

`v0.7.1` — v0.7.x hardening patch series, batch 1.

## Toolchain

```
rustup toolchain: 1.91 (stable)
linker: ld.lld (apt install lld)
cross-target: riscv64gc-unknown-none-elf
```

Or, using `rust-toolchain.toml` (added in this release):
```
rustup show
```

## Exact test command (produces the 376-test count)

```sh
cargo test --workspace --lib
```

Zero warnings, zero failures, 376 named tests.

## RFC changes in this release

Closes architect findings from the v0.7.0 reviews:

### v0.7.1 changes (release engineering, RFC-v0.7.1-001..003)
- Added `rust-toolchain.toml` pinning Rust 1.91
- Added `Cargo.lock` to repository and release tarball
- Set `default-members` in workspace for `cargo test` coverage
- Extended `ImageId` with v0.4 network services (0x17..0x1A)
  and v0.7 sync services (0x1B..0x1D) — closes W-RB-01

### v0.7.2 changes (snapshot safety + identity hardening, RFC-v0.7.2-002..003)
- `snapshot_digest()`: replaced fixed 4 KiB stack buffer with
  streaming `DigestWriter` — closes C-RB-02 (CRITICAL)
- `ConflictDomain`: removed `Default` derive; `V1_DEFAULT =
  ForeignAuthoritative` — closes C-M-01
- `SnapshotRecord::push_record`: returns `SnapshotError::BodyTooLarge`
  for `body_len > 64` — closes C-M-02
- `NodeIdentity::build()`: safe constructor that computes and
  validates digest at construction — closes C-H-04
- `NodeIdentity::validate_digest()`: reload validation helper
- `NodeIdentityPolicy::permits()`: returns `Decision` enum, never
  panics — closes C-H-02
- `NodeIdentityPolicy::validate()`: explicit policy validation
- `TrustMode::Fleet` without roster → `PolicyError::FleetWithoutRoster`
  → `Decision::Deny` — closes C-H-03
- `NodeAlias::try_as_str()`: strict UTF-8 (Err on invalid) — closes C-M-05
- `NodeAlias::as_str_lossy()`: lossy display-only helper

### v0.7.3 changes (crypto, RFC-v0.7.3-002)
- `fjell-sxt-crypto`: requires `crypto-profile-development` feature;
  `compile_error!` without it — closes C-H-01
- `hkdf_expand()`: returns `Result<(), HkdfError>` instead of panicking;
  `HkdfError::OutputTooLong` for >255×32 B requests — closes C-H-01
- Info is no longer silently truncated

### v0.7.4 changes (capability model, RFC-v0.7.4-003)
- `CapRights::ALL_NON_META`: new name for what was `ALL` (excludes
  `CAP_INSTALL`); `ALL` kept as deprecated alias
- `CapRights::ALL_DEFINED`: includes `CAP_INSTALL` and future meta-rights
- `CapKind::from_u8()`: unknown discriminant → `None` (was `Endpoint`)
  — closes C-RB-04 (format-level)

### v0.7.5 changes (documentation + catalog, RFC-v0.7.5-001)
- `MeasurementSummary::add_kind_count`: rejects duplicate kinds —
  closes C-M-04
- `ReleaseSummary::add_channel`: rejects duplicate channel_ids
- `SummaryError` enum: `DuplicateKind`, `DuplicateChannel`,
  `CapacityExhausted`
- `REL_HASH`, `RFS_HASH`, `POL_HASH`: `#[deprecated]` markers
- `ImageId` variant tests stable (v0.4 and v0.7 ranges)

## Kernel prebuilt images

The kernel and service binaries are provided as `.bin` flat images
under `crates/fjell-kernel/prebuilt/`. SHA-256 digests are listed in
`crates/fjell-kernel/prebuilt/DIGESTS`.

## Known limitations (from v0.8 entry criteria)

The following are intentionally deferred to subsequent v0.7.x patches:

- `identityd`, `summaryd`, `syncd` are still stubs (W-RB-03;
  RFC-v0.7.2-001 pending implementation)
- DMA revoke does not yet unmap user PTEs (C-RB-01; RFC-v0.7.4-001
  pending implementation)
- sys_mmio_map size overflow not yet fixed (W-H-01; RFC-v0.7.4-002
  pending)
- Spawn path still installs broad MMIO caps (C-RB-03; RFC-v0.7.4-003
  kernel-side pending)
- v0.4 networking still uses simulated TLS paths in parts (W-RB-04;
  RFC-v0.7.3-001 pending)

See `rfcs/v0.7.x/` for the full RFC set and remaining patch plan.
