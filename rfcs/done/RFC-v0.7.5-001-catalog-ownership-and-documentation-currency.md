# RFC-v0.7.5-001: Semantic Catalog Ownership and Documentation Currency

**Status.** Implemented (v0.7.4)

## Status

Draft (closes review findings **W-M-01, W-M-02, W-M-03, C-M-03,
C-M-04, C-M-06, W-H-05**)

## Target Version

`v0.7.5`

## Summary

Final pre-v0.8 cleanup: scrub alpha-era references from `README.md`
and source comments; add owner metadata to the frozen semantic
catalog reserved ranges (FLEET, SDK); deprecate the legacy
`SnapshotDigest` placeholder constants; reject duplicate entries in
measurement and release summaries; replace raw physical addresses in
`sys_platform_info_get` with symbolic region IDs; and upgrade the
unsafe-audit tool from comment-presence to structured categorical
review.

## Motivation

These items are individually medium-priority but collectively block a
clean v0.8 narrative. The whole-project review (§6 M-01..M-07 and
§5 H-05) and the crates review (§7 M-03..M-06) describe a system
where:

```text
- README still identifies the version as v0.3.0-alpha.1
- semantic catalog freezes the wire format but not the governance
- legacy SnapshotDigest placeholders look real but are not
- summaries with duplicate kinds are accepted silently
- platform_info returns raw QEMU physical addresses
- unsafe-audit gates on a comment being present, not what it says
```

Each is a small fix. Bundled together they make the v0.7 codebase
self-consistent before v0.8 fleet work begins.

## Goals

```text
- Every doc and comment in the v0.7.5 release tarball agrees the
  version is v0.7.5 (or current); historical references stay only
  in CHANGELOG and ADRs.
- The frozen semantic catalog v1 file has explicit owner metadata
  for each reserved range and each individual intent.
- Legacy SnapshotDigest is marked deprecated; new code uses Digest32.
- MeasurementSummary and ReleaseSummary push/build operations reject
  duplicate kinds / channels.
- sys_platform_info_get returns symbolic RegionId values; raw PAs are
  obtainable only via DeviceInventory capability.
- unsafe-audit requires comments to NAME the invariant relied upon,
  categorized by the structured taxonomy from the whole-project review.
```

## Non-Goals

```text
- No new IPC tags.
- No new ABI surface.
- No replacement of the catalog v1 contents (the freeze is preserved).
- No new attestation chain entries.
```

## External Design

### 1. README and comment scrub

A workspace-wide pass deletes / updates every reference to:

```text
- "v0.3.0-alpha.1"
- "alpha.2 land in ..."
- "deferred to v0.3" (where v0.3 has shipped)
- "future-use only" (where the future is now)
```

The `tools/fjell-doc-grep/` helper (added by RFC-v0.7.1-001) is
extended with a `--scrub` mode that lists every stale reference.
The CI gate `ci-version-consistency` is broadened to include comment
references, not just `README.md`.

Historical references in `CHANGELOG.md`, `rfcs/`, `docs/src/adr/`,
and per-release `RELEASE.md` files are explicitly exempt — they are
the history.

### 2. Semantic catalog owner metadata

The frozen catalog file at
`crates/fjell-semantic-v1/schema/catalog-v1.frozen` gains owner
fields:

```text
# fjell-semantic-v1 frozen catalog
SCHEMA_VERSION   0x0001
MAGIC            "FJSI-V1\0"

# Range ownership
RANGE 0x0000..0x00FF  CORE      owner=fjell-kernel
RANGE 0x0100..0x017F  SECURITY  owner=fjell-attestd
RANGE 0x0180..0x01FF  STORE     owner=fjell-storaged
RANGE 0x0200..0x02FF  FLEET     owner=fjell-syncd       reserved-for=v0.8
RANGE 0x0300..0x03FF  SDK       owner=fjell-sdk          reserved-for=v0.9
RANGE 0x0400..0x04FF  HEALTH    owner=fjell-recoveryd
RANGE 0x0500..0x05FF  NET       owner=fjell-netd

# Individual intents — only those landed in v0.7 are listed.
INTENT 0x0010  UPDATE.STAGED              owner=fjell-upgraded
INTENT 0x0020  ATTESTATION.SIGNED         owner=fjell-attestd
...
INTENT 0x01A0  NET.DMA_REGION_REVOKED     owner=fjell-driver-virtio-net
INTENT 0x01A1  NET.DMA_REGION_QUARANTINED owner=fjell-driver-virtio-net
```

A new Rust struct exposes this:

```rust
pub struct CatalogRangeOwner {
    pub range_start:     u16,
    pub range_end:       u16,
    pub name:            &'static str,
    pub owner_crate:     &'static str,
    pub reserved_for_version: Option<&'static str>,
}

pub const RANGES: &[CatalogRangeOwner] = &[
    CatalogRangeOwner { range_start: 0x0200, range_end: 0x02FF,
                        name: "FLEET", owner_crate: "fjell-syncd",
                        reserved_for_version: Some("v0.8") },
    // ...
];
```

CI gate `ci-catalog-owner`:

```text
- Every intent in catalog-v1.frozen has an owner_crate.
- The owner_crate exists in the workspace.
- A PR that adds an intent must touch the owner crate as well
  (or include a written exception in the commit body).
```

### 3. Legacy `SnapshotDigest` deprecation

```rust
// crates/fjell-snapshot-format/src/legacy.rs

/// Legacy snapshot digest type from v0.2.  Kept for backward
/// compatibility with persisted records; do NOT use for new code.
///
/// New code uses Digest32 + SnapshotEnvelope (RFC-v0.7.2-002).
///
/// The constants below are documented placeholders, not real
/// digests.  Any code path that compares to them is broken.
#[deprecated(since = "0.7.5", note = "use Digest32 + SnapshotEnvelope")]
pub struct SnapshotDigest {
    pub release_hash: [u8; 8],
    pub rootfs_hash:  [u8; 8],
    pub policy_hash:  [u8; 8],
}

#[deprecated(since = "0.7.5", note = "placeholder; do not trust")]
pub const REL_HASH: [u8; 8] = *b"REL_HASH";
#[deprecated(since = "0.7.5", note = "placeholder; do not trust")]
pub const RFS_HASH: [u8; 8] = *b"RFS_HASH";
#[deprecated(since = "0.7.5", note = "placeholder; do not trust")]
pub const POL_HASH: [u8; 8] = *b"POL_HASH";
```

The `#[deprecated]` markers cause `cargo check` to warn on any
in-tree reference. The CI `check` job runs with `RUSTFLAGS=-D warnings`
so any new use is a hard error.

Internal callers that still reference `SnapshotDigest::current()` or
`with_audit()` are migrated to `SnapshotEnvelope` in this RFC.

### 4. Summary duplicate-entry rejection

```rust
impl MeasurementSummary {
    pub fn push_kind(&mut self, k: MeasurementKind, count: u64)
        -> Result<(), SummaryError>
    {
        if self.entries.iter().any(|e| e.kind == k) {
            return Err(SummaryError::DuplicateKind);
        }
        if self.entries.is_full() {
            return Err(SummaryError::CapacityExhausted);
        }
        // Sorted insertion to enable binary search later.
        let pos = self.entries
            .binary_search_by_key(&(k as u8), |e| e.kind as u8)
            .unwrap_err();
        self.entries.insert(pos, MeasurementEntry { kind: k, count });
        Ok(())
    }
}

impl ReleaseSummary {
    pub fn push_channel(&mut self, c: ChannelId, ...)
        -> Result<(), SummaryError>
    {
        if self.channels.iter().any(|e| e.channel == c) {
            return Err(SummaryError::DuplicateChannel);
        }
        // ... same pattern as above
    }
}
```

Decoders also validate uniqueness; a parsed summary with duplicates
returns `SummaryError::DuplicateKind` rather than silently keeping the
first.

### 5. `sys_platform_info_get` symbolic regions

```rust
/// Returns symbolic region IDs and their kinds.  Does NOT expose
/// raw physical addresses to non-privileged callers.
///
/// To resolve a RegionId to a (pa, size) pair, the caller must hold
/// the DeviceInventory capability with REGION_RESOLVE right.
pub fn sys_platform_info_get(tf: &mut TrapFrame) -> SyscallResult {
    let mut info = PlatformInfo::default();
    for (idx, region) in board_profile().regions.iter().enumerate() {
        info.regions[idx] = PlatformRegionDescriptor {
            region_id:   RegionId(idx as u32),
            kind:        region.kind,        // UART, NET_MMIO, BLK_MMIO, ...
            requires_cap: region.requires_cap_kind,
        };
    }
    copy_to_user(tf.gpr[REG_A0], &info)?;
    Ok(SyscallReturn::ok())
}
```

A new syscall `sys_platform_region_resolve(region_id) -> (pa, size)`
is added; it requires `DeviceInventory` with `REGION_RESOLVE`. Only
devmgr and the relevant drivers hold this capability.

### 6. Structured unsafe-audit categories

The unsafe-audit tool grows a categorical model. SAFETY comments now
declare which invariant they appeal to:

```rust
// SAFETY: category=raw-pointer-deref
//   The pointer comes from copy_to_user validation that confirmed
//   PTE_W on every page in [dst, dst+len).  No aliasing because the
//   caller's CSpace lock is held.
unsafe { core::ptr::write_volatile(...) }
```

Categories (from whole-project review §H-05):

```text
raw-pointer-deref
page-table-mutation
csr-asm
mmio-access
phys-id-map-assumption
kernel-global-mutable
user-copy
```

The unsafe-audit tool:

- Greps each `unsafe { ... }` block for `// SAFETY:` within 4 lines
  above.
- Parses `category=` and validates against the known set.
- Requires a non-empty body after the `category=` tag (the invariant
  statement).
- Fails CI on:
  - missing comment,
  - missing category,
  - unknown category,
  - empty/placeholder body.

Existing 261 unsafe sites are categorized as part of this RFC.

## Data Model

### `SummaryError`

```rust
#[repr(u8)]
pub enum SummaryError {
    DuplicateKind     = 0x01,
    DuplicateChannel  = 0x02,
    CapacityExhausted = 0x03,
    InvalidDigest     = 0x04,
}
```

### `RegionId`, `PlatformRegionDescriptor`, region kind enum

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegionId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RegionKind {
    Uart      = 0x01,
    Plic      = 0x02,
    NetMmio   = 0x03,
    BlkMmio   = 0x04,
    RtcMmio   = 0x05,
    ClintMmio = 0x06,
}

pub struct PlatformRegionDescriptor {
    pub region_id:    RegionId,
    pub kind:         RegionKind,
    pub requires_cap: CapKind,    // which cap-kind controls access
}
```

## Internal Design

### `unsafe-audit` v2 implementation

```text
tools/fjell-unsafe-audit/
  src/
    main.rs
    parser.rs       line-based parser (no syn dependency)
    category.rs     enum + table of valid categories
    report.rs       structured output

A SAFETY block must look like:

  // SAFETY: category=<known-name>
  //   <invariant statement, at least one non-empty line>
  // [optional extra lines]
  unsafe { ... }

The parser:
1. For each `unsafe` keyword (block or fn), scan upward up to 12 lines.
2. Find `// SAFETY:` line. If absent → FAIL.
3. Extract `category=<x>`. If absent or unknown → FAIL.
4. Require at least one non-empty content line after the SAFETY tag,
   before the unsafe keyword. If absent → FAIL.
5. Emit a CSV summary: (file, line, category, invariant_first_line).
```

The CSV summary is committed under `docs/src/internals/unsafe-audit.csv`
and reviewed on every release.

### Migration plan for 261 unsafe sites

The 261 existing sites already have `// SAFETY:` comments (per v0.6).
The migration adds `category=` to each. Bulk task tracked in
ADR-v0.7.5-001.

## Security Design

- Symbolic region IDs reduce information leakage. A compromised
  service that can call `sys_platform_info_get` cannot, by itself,
  learn physical addresses; it must additionally hold
  `DeviceInventory`.
- Structured unsafe-audit reduces the false-confidence risk
  (whole-project §H-05). Reviewers can rely on the category to know
  what invariant they are validating.
- Deprecated `SnapshotDigest` constants prevent new code from
  trusting placeholder hashes as if they were real.

## Memory / Resource Design

- `PlatformInfo` grows slightly: each region now carries a region_id
  and cap-kind in addition to the kind. Within the existing budget.
- Summary entries now sorted on insert; insertion is O(n) but n is
  bounded (≤ 16 measurement kinds, ≤ 8 channels).

## Compatibility and Migration

- `sys_platform_info_get` return shape changes. Internal callers
  (devmgr) updated in this RFC. Out-of-tree callers must migrate.
- `MeasurementSummary::push_kind` now returns `Result`. The previous
  signature was infallible; callers must add error handling.
- `SnapshotDigest::current()` and `::with_audit()` produce
  deprecation warnings. In-tree callers migrated to
  `SnapshotEnvelope`. Out-of-tree callers will see warnings until
  v1.0 removal.
- unsafe-audit: existing SAFETY comments without `category=` will
  fail CI. Migration commit is included in this RFC.

## Test Strategy

```text
- Workspace-grep test: no source file mentions "v0.3.0-alpha.1" or
  similar except in CHANGELOG / rfcs / docs/src/adr.
- catalog-owner test: every catalog entry resolves to a workspace
  crate.
- SnapshotDigest::current() emits a deprecation warning under
  -D deprecated.
- MeasurementSummary::push_kind rejects duplicate.
- ReleaseSummary::push_channel rejects duplicate.
- Decoded summary with duplicate kinds → DuplicateKind error.
- sys_platform_info_get returns symbolic IDs only.
- sys_platform_region_resolve requires DeviceInventory.
- unsafe-audit v2 fails on missing/unknown category.
- unsafe-audit v2 passes on all 261 existing sites (after
  migration).
```

## Acceptance Criteria

```text
- ci-version-consistency includes comment scan; green.
- ci-catalog-owner green.
- ci-no-legacy-snapshot-digest green (no in-tree caller uses the
  deprecated symbols outside tests).
- SUMMARY:DUPLICATE_KIND_REJECTED, SUMMARY:DUPLICATE_CHANNEL_REJECTED
  unit tests green.
- PLATFORM:INFO_SYMBOLIC_ONLY, PLATFORM:REGION_RESOLVE_CAP_REQUIRED
  unit tests green.
- unsafe-audit v2 categorical check green; 261 sites categorized.
- ADR-v0.7.5-001 filed.
```

## Documentation Requirements

```text
- docs/src/reference/semantic-catalog-governance.md created.
- docs/src/reference/platform-info-syscalls.md updated.
- docs/src/internals/unsafe-audit.csv committed.
- UNSAFE_CHARTER.md updated with the seven categories and per-
  category invariant guidance.
- CHANGELOG.md v0.7.5 entry lists every deprecation.
```

## Open Questions

```text
1. Should the catalog freeze become version-stamped per release
   (catalog-v1.0.frozen, catalog-v1.1.frozen)? Proposal: not in
   v0.7.5. The v1 catalog is the v1 catalog; additions are tracked
   per intent in ADRs.

2. Should sys_platform_region_resolve be a syscall or a service-API
   call to devmgr? Proposal: syscall, because the page-table mapping
   step that follows benefits from a single round-trip. Cap-gated.

3. Should unsafe-audit accept multi-category comments (an unsafe
   block spans two categories)? Proposal: comma-separated list
   permitted; e.g. `category=raw-pointer-deref,user-copy`.

4. Should the eight existing fuzz targets get a structural unsafe
   audit before v0.8? Proposal: nice-to-have, not blocking. Track
   as v0.8.X.
```

## Release Gate

```text
- ci-version-consistency green (including comment scan)
- ci-catalog-owner green
- 261 unsafe sites carry valid category= tags
- ADR-v0.7.5-001 accepted
- This RFC is the last v0.7.x patch; v0.8 entry follows.
```
