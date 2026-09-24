//! The formats that have a frozen file, and the crates that do not, with the reason.

use fjell_canon::Canon;

use crate::samples as s;

/// One format with a generated `.frozen` file.
pub struct Format {
    /// File stem, e.g. `rollback-record-v1`.
    pub id: &'static str,
    pub krate: &'static str,
    /// The format's name in the file's header.
    pub name: &'static str,
    /// The version line. It moves where bytes moved (D4), and only then.
    pub version: &'static str,
    /// Where the file lives, relative to the workspace root.
    pub path: &'static str,
    /// Dated notes on what the file used to claim (D3). Documentation about the
    /// past, not a description of a layout.
    pub notes: &'static [&'static str],
    /// The format's **on-disk version**, where it has one (RFC-0.33-003 D11): read from
    /// the crate's own constant, so the header says the format's version — what a
    /// reader of `store-superblock.frozen` wants — and not the release the file was
    /// generated in (that is `version`, kept beside it).
    pub on_disk: Option<fn() -> u16>,
    /// The encoder, run on the representative value.
    pub write: fn(&mut dyn Canon),
}

fn rollback(c: &mut dyn Canon) {
    s::rollback_record().write_canonical(c)
}
fn release_metadata(c: &mut dyn Canon) {
    s::release_metadata().write_canonical(c)
}
fn attestation(c: &mut dyn Canon) {
    s::attestation_v2().write_canonical(c)
}
fn keyring(c: &mut dyn Canon) {
    fjell_keyring::snapshot::write_canonical(&s::keyring_snapshot(), c)
}
fn diag(c: &mut dyn Canon) {
    s::diag_bundle().write_canonical(c)
}
fn identity(c: &mut dyn Canon) {
    fjell_identity_format::digest::write_canonical(&s::node_identity(), c)
}
fn board(c: &mut dyn Canon) {
    fjell_platform_format::digest::write_board_canonical(&s::board_profile(), c)
}
fn measurement_summary(c: &mut dyn Canon) {
    fjell_summary_format::digest::write_measurement_canonical(&s::measurement_summary(), c)
}
fn release_summary(c: &mut dyn Canon) {
    fjell_summary_format::digest::write_release_canonical(&s::release_summary(), c)
}
fn snapshot(c: &mut dyn Canon) {
    fjell_snapshot_format::write_canonical(
        &s::snapshot_envelope(fjell_snapshot_format::SNAPSHOT_ENVELOPE_V2),
        c,
    )
}
fn semantic(c: &mut dyn Canon) {
    let (entry, values) = s::semantic_widest();
    fjell_semantic_v1::codec::write_canonical(
        entry.schema,
        entry.tag,
        s::SEMANTIC_TICK,
        &values,
        c,
    );
    c.note("recorded_on", entry.name);
    c.note(
        "catalog_entries",
        &fjell_semantic_v1::catalog_len().to_string(),
    );
    c.note(
        "catalog_version",
        &format!(
            "major={} minor={}",
            fjell_semantic_v1::CATALOG_V1_VERSION.major,
            fjell_semantic_v1::CATALOG_V1_VERSION.minor
        ),
    );
    c.note(
        "max_fields",
        &fjell_semantic_v1::codec::MAX_FIELDS.to_string(),
    );
}
fn fleet_roster(c: &mut dyn Canon) {
    fjell_fleet_format::digest::write_roster_canonical(&s::fleet_roster(), c)
}
fn fleet_policy(c: &mut dyn Canon) {
    fjell_fleet_format::digest::write_policy_canonical(&s::fleet_policy(), c)
}
fn platform(c: &mut dyn Canon) {
    fjell_platform_format::digest::write_platform_canonical(&s::platform_profile(), c)
}
fn store_superblock(c: &mut dyn Canon) {
    s::store_superblock().write_canonical(c);
    c.note(
        "sector_bytes",
        "512 (the caller zero-fills the rest of the sector explicitly)",
    );
}
fn record_header(c: &mut dyn Canon) {
    s::record_header().write_canonical(c);
}
fn boot_control(c: &mut dyn Canon) {
    s::boot_control_block().write_canonical(c);
    c.note(
        "sector_bytes",
        "512 (the caller zero-fills the rest of the sector explicitly)",
    );
}

pub const FORMATS: &[Format] = &[
    Format {
        id: "rollback-record-v1",
        krate: "fjell-upgrade-format",
        name: "RollbackRecordV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-upgrade-format/schema/rollback-record-v1.frozen",
        notes: &[
            "2026-09-24: this file used to claim `channel` u8[16], `min_counter` u32 and `updated_tick`, and omitted `last_advance_source` and the 32-byte `record_digest_placeholder`; the encoder writes `channel_id` u8[8], `min_counter` u64, `last_advance_tick`, `last_advance_source` and the placeholder. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: rollback,
    },
    Format {
        id: "release-metadata-v1",
        krate: "fjell-upgrade-format",
        name: "ReleaseMetadataV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-upgrade-format/schema/release-metadata-v1.frozen",
        notes: &[
            "2026-09-24: this file used to describe a different structure (domain FJELL-RELEASE-V1; `release_id`, `channel` u8[16], `counter` u32, `min_counter` u32, kernel/rootfs/policy digests and a `signature` triple); the encoder writes domain FJELL-RELEASE-META-V1 and the fields below. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: release_metadata,
    },
    Format {
        id: "v2",
        krate: "fjell-attestation-format",
        name: "AttestationRecordV2",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-attestation-format/schema/v2.frozen",
        notes: &[
            "2026-09-24: this file used to list 16 fields (provider, measurement_head, three digests, health_result, signature); the encoder writes the keyring, boot, verification, measurement, snapshot, health, rollback, freshness and provenance groups as well and has no signature in the digest stream. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: attestation,
    },
    Format {
        id: "snapshot-v1",
        krate: "fjell-keyring",
        name: "KeyringSnapshotV1",
        version: "v0.6.0 (frozen)",
        path: "crates/fjell-keyring/schema/snapshot-v1.frozen",
        notes: &[
            "2026-09-24: this file used to claim `purpose_count` and six `purpose[0..6]` records with a 64-byte `anchor_bytes`; the encoder writes the SNAP-V1 domain, `anchor_count` and 28 slots of present/purpose/algorithm/authority/epoch/reserved/key_len/key_bytes. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: keyring,
    },
    Format {
        id: "bundle-v1",
        krate: "fjell-diag-format",
        name: "DiagnosticBundleV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-diag-format/schema/bundle-v1.frozen",
        notes: &[
            "2026-09-24: this file used to claim `export_tick`, `source_service`, `entry_count` and `entries[]` of tag/tick/len/body; the encoder writes `created_tick`, `measurement_head`, `last_attestation` and two counted groups, `audit_events` and `semantic_intents`. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: diag,
    },
    Format {
        id: "node-identity-v1",
        krate: "fjell-identity-format",
        name: "NodeIdentityV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-identity-format/schema/node-identity-v1.frozen",
        notes: &[],
        on_disk: None,
        write: identity,
    },
    Format {
        id: "board-v1",
        krate: "fjell-platform-format",
        name: "BoardProfileV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-platform-format/schema/board-v1.frozen",
        notes: &[],
        on_disk: None,
        write: board,
    },
    Format {
        id: "measurement-summary-v1",
        krate: "fjell-summary-format",
        name: "MeasurementSummaryV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-summary-format/schema/measurement-summary-v1.frozen",
        notes: &[],
        on_disk: None,
        write: measurement_summary,
    },
    Format {
        id: "release-summary-v1",
        krate: "fjell-summary-format",
        name: "ReleaseSummaryV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-summary-format/schema/release-summary-v1.frozen",
        notes: &[],
        on_disk: None,
        write: release_summary,
    },
    Format {
        id: "snapshot-v2",
        krate: "fjell-snapshot-format",
        name: "SnapshotEnvelopeV2",
        version: "v0.7.0 (frozen) BREAKING-SCHEMA: see ADR-v0.7-004",
        path: "crates/formats/fjell-snapshot-format/schema/snapshot-v2.frozen",
        notes: &[],
        on_disk: None,
        write: snapshot,
    },
    Format {
        id: "intent-v1",
        krate: "fjell-semantic-v1",
        name: "SemanticIntentV1",
        version: "v0.6.0 (frozen)",
        path: "crates/fjell-semantic-v1/schema/intent-v1.frozen",
        notes: &[
            "2026-09-24: this file used to claim one `fields[].value` of width FieldKind.wire_size and carry hand-typed `catalog_entries`/`catalog_version` lines; it now records the widest catalogue entry and derives the notes from the catalogue. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
        ],
        on_disk: None,
        write: semantic,
    },
    Format {
        id: "fleet-roster-v1",
        krate: "fjell-fleet-format",
        name: "FleetRosterV1",
        version: "v0.8.0 (first generated)",
        path: "crates/formats/fjell-fleet-format/schema/fleet-roster-v1.frozen",
        notes: &[],
        on_disk: None,
        write: fleet_roster,
    },
    Format {
        id: "fleet-policy-v1",
        krate: "fjell-fleet-format",
        name: "FleetPolicyV1",
        version: "v0.8.0 (first generated)",
        path: "crates/formats/fjell-fleet-format/schema/fleet-policy-v1.frozen",
        notes: &[],
        on_disk: None,
        write: fleet_policy,
    },
    Format {
        id: "platform-v1",
        krate: "fjell-platform-format",
        name: "PlatformProfileV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-platform-format/schema/platform-v1.frozen",
        notes: &[],
        on_disk: None,
        write: platform,
    },
    Format {
        id: "store-superblock",
        krate: "fjell-store-format",
        name: "StoreSuperblock",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-store-format/schema/store-superblock.frozen",
        notes: &[],
        on_disk: Some(|| fjell_store_format::STORE_SUPERBLOCK_VERSION),
        write: store_superblock,
    },
    Format {
        id: "record-header",
        krate: "fjell-store-format",
        name: "RecordHeader",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-store-format/schema/record-header.frozen",
        notes: &[],
        on_disk: Some(|| fjell_store_format::RECORD_HEADER_VERSION),
        write: record_header,
    },
    Format {
        id: "boot-control-block",
        krate: "fjell-upgrade-format",
        name: "BootControlBlock",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-upgrade-format/schema/boot-control-block.frozen",
        notes: &[],
        on_disk: Some(|| fjell_upgrade_format::BOOT_CONTROL_VERSION),
        write: boot_control,
    },
];

/// Why a crate under `crates/formats/` has no generated file.
///
/// "Nobody wrote one" is not on this list, and cannot be: each reason is a fact a
/// test re-checks, so a crate cannot stay here after the fact stops being true.
pub enum Exclusion {
    /// The crate defines **no function that turns a value into bytes** (no
    /// serialiser, no digest stream, no `#[repr(C)]` layout). A test scans its
    /// sources for the producers that exist elsewhere in the tree and fails if it
    /// finds one, so this stops being true visibly.
    InMemoryOnly,
    /// The crate **does** produce bytes and has no generated file yet. This is a
    /// survivor of E-045, named in the register under `erratum`, not a reason to
    /// leave it: a test checks the register mentions it.
    Survivor {
        erratum: &'static str,
        what: &'static str,
    },
}

/// Every crate under `crates/formats/` that has no entry in [`FORMATS`].
pub const EXCLUSIONS: &[(&str, Exclusion)] = &[
    ("fjell-block-format", Exclusion::InMemoryOnly),
    ("fjell-config-format", Exclusion::InMemoryOnly),
    ("fjell-device-format", Exclusion::InMemoryOnly),
    (
        "fjell-net-format",
        Exclusion::Survivor {
            erratum: "E-065",
            what: "`NetDescriptorHeader` and `NetDriverPacket` are `#[repr(C)]` layouts with no encoder to record; nothing outside the crate uses either",
        },
    ),
    ("fjell-policy-format", Exclusion::InMemoryOnly),
    ("fjell-recovery-format", Exclusion::InMemoryOnly),
    ("fjell-remote-diag-format", Exclusion::InMemoryOnly),
    ("fjell-rootfs-format", Exclusion::InMemoryOnly),
    ("fjell-verify-format", Exclusion::InMemoryOnly),
    (
        "fjell-audit-format",
        Exclusion::Survivor {
            erratum: "E-065",
            what: "`AuditRecordBin` (written raw by the kernel's `sys_audit_drain`), `AuditPersistRecord` and `AuditLogHeader` are `#[repr(C)]` layouts, padding-free with asserted sizes, and have no encoder to record",
        },
    ),
    (
        "fjell-bundle-format",
        Exclusion::Survivor {
            erratum: "E-065",
            what: "`compute_bundle_digest` writes big-endian integers, which `Canon` does not yet express",
        },
    ),
    (
        "fjell-measure-format",
        Exclusion::Survivor {
            erratum: "E-065",
            what: "`MeasurementEvent::compute_chain_digest` is a digest stream over `of_parts`; not yet on `Canon`",
        },
    ),
    (
        "fjell-semantic-format",
        Exclusion::Survivor {
            erratum: "E-065",
            what: "`wire` is the IPC codec for `SemanticEnvelope` (RFC-0.32-002 D1): an encoder and decoder with unnamed writer calls, the largest format in the tree, and it needs its own line",
        },
    ),
];
