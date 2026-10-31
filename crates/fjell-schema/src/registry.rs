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
    /// The format's **own version** and what kind it is (`"on disk"`, `"wire"`), where it
    /// has one (RFC-0.33-003 D11, RFC-0.34-003 §E): read from
    /// the crate's own constant, so the header says the format's version — what a
    /// reader of `store-superblock.frozen` wants — and not the release the file was
    /// generated in (that is `version`, kept beside it).
    pub format_version: Option<(fn() -> u16, &'static str)>,
    /// What the file says the description does **not** cover, one `# not described:`
    /// line each — for a format whose *reader* is a second, hand-written reading of the
    /// layout (RFC-0.34-003 §A): the file describes what the encoder writes and must not
    /// be read as blessing the decoder.
    pub caveats: &'static [&'static str],
    /// The encoder, run on each representative value. A flat format has one sample; a
    /// format with tagged unions or optionals needs samples that between them reach
    /// every arm, and their recordings are merged (`crate::record`).
    pub samples: &'static [fn(&mut dyn Canon)],
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
fn chain(c: &mut dyn Canon) {
    let e = s::chain_event_with_metadata();
    fjell_measure_format::event::write_chain_canonical(
        c,
        e.seq,
        e.kind,
        e.source,
        e.subject,
        &e.subject_digest,
        e.metadata_digest.as_ref(),
        &e.previous_chain_digest,
    );
}
fn image_set(c: &mut dyn Canon) {
    use fjell_measure_format::{Digest32, ImageEntry};
    // Two representative images: the recorder learns an element's shape from
    // the first, and a set of one would not show that the group repeats.
    let entries = [
        ImageEntry {
            image_id: 0,
            digest: Digest32::of(b"image zero"),
        },
        ImageEntry {
            image_id: 1,
            digest: Digest32::of(b"image one"),
        },
    ];
    fjell_measure_format::write_image_set_canonical(c, &entries);
}
fn bundle(c: &mut dyn Canon) {
    let b = s::service_bundle();
    fjell_bundle_format::write_bundle_canonical(
        c,
        b.header.service_name.as_bytes(),
        b.header.bundle_version,
        b.header.sdk_api_rev,
        b.header.catalog_version,
        &b.header.manifest_digest,
        &b.header.binary_hash,
    );
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
fn wire_sample_0(c: &mut dyn Canon) {
    wire_sample(0, c)
}
fn wire_sample_1(c: &mut dyn Canon) {
    wire_sample(1, c)
}
fn wire_sample_2(c: &mut dyn Canon) {
    wire_sample(2, c)
}
fn wire_sample_3(c: &mut dyn Canon) {
    wire_sample(3, c)
}
fn wire_sample_4(c: &mut dyn Canon) {
    wire_sample(4, c)
}
fn wire_sample(i: usize, c: &mut dyn Canon) {
    fjell_semantic_format::wire::write_canonical(&s::semantic_envelopes()[i], c)
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
        format_version: None,
        caveats: &[],
        samples: &[rollback],
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
        format_version: None,
        caveats: &[],
        samples: &[release_metadata],
    },
    Format {
        id: "v2",
        krate: "fjell-attestation-format",
        name: "AttestationRecordV2",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-attestation-format/schema/v2.frozen",
        notes: &[
            "2026-09-24: this file used to list 16 fields (provider, measurement_head, three digests, health_result, signature); the encoder writes the keyring, boot, verification, measurement, snapshot, health, rollback, freshness and provenance groups as well and has no signature in the digest stream. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
            "2026-10-29: RFC-0.40-001 (closes E-094) bumped AttestationRecordV2::SCHEMA_VERSION 2 -> 3. Six availability bytes are written beside the claims they qualify (`boot.kernel_digest_available`, `verification.{release,rootfs,policy}_digest_available`, `measurement.available`, `snapshot.snapshot_digest_available`), and a claim whose flag is 0 is written as zero -- fixed width, zeroed by the encoder, the pattern `bundle-v1.frozen` states -- and a `verified_flags` bit is written only when its digest is available. The record used to assert, under a signature, that a release, a rootfs and a policy bundle had each been verified, over five literal digests and a literal chain head; the factory now says only what is true. The 2 -> 3 digest move is proved, not assumed: `tests/golden.rs::attestation_v3_is_v2_plus_six_flags` removes the six bytes, rewinds the version word and recovers the v2 golden.",
            "2026-10-31: RFC-0.41-003 R2 moved the zeroing of `rollback.trust_provider_counter_value` into the encoder: it is written as 0 whenever `rollback.trust_provider_counter_supported` is 0, as every *_digest and the measurement section already were, rather than trusting the caller's raw value. `RollbackClaims` is `pub` with `pub` fields, so `AttestationRecordV2::local` mapping a failed read to 0 was the only honest path, not the only path. `SCHEMA_VERSION` stays 3 and no byte moved: every record the tree can construct sets the flag 1 beside its value, or 0 beside the 0 `local()` writes; the golden digest is unchanged, which is the instrument. The one new line in this file is the `note` that says so. Corrected toward the encoder (the precedent is the 2026-09-24 note above, RFC-0.33-003 D3).",
            "2026-10-31: RFC-0.41-003 R1 (closes E-101) bumped AttestationRecordV2::SCHEMA_VERSION 3 -> 4, superseding the \"stays 3\" of the note above, which was true of that change (the counter value) and is not of this one. Five availability bytes are written, each immediately before the claim it qualifies -- `keyring.epochs_available` (the three `keyring.active_epoch_*`), `snapshot.id_available` (`snapshot.snapshot_id` and `snapshot.reason`), `health.status_available`, `rollback.last_advance_source_available`, `freshness.key_epoch_available` -- and the claim is written as zero when its flag is 0, by the encoder (one `note` line says so). Fixed width, zeroed by the encoder: the pattern this file already states for the digests. `snapshot.reason` and `health.status` get a flag although the factory already wrote 0, because 0 is a valid reason and a valid status, which is exactly the case a flag exists for. The 3 -> 4 digest move is proved, not assumed: `tests/golden.rs::attestation_v4_is_v3_plus_five_flags` removes the five bytes, rewinds the version word to 3 and recovers the v3 golden `241d4c91...b9`; the older chain to the v2 golden is kept (`attestation_v3_is_v2_plus_six_flags`). The golden for the sample moved 241d4c91...b9 -> adefb58c...ab69 and no field line of this file changed order or width: five field lines and one note line were added.",
            "2026-10-31: RFC-0.41-003 R1, before the version 4 cut: the field `snapshot.id_available` is renamed `snapshot.id_and_reason_available`, because it qualifies `snapshot.snapshot_id` **and** `snapshot.reason` and the old name said only the first. No byte moved: the name is not written into the digest stream (`Canon::u8` takes it and `SliceSink` discards it), so the golden digest is unchanged (`adefb58c...ab69`), `SCHEMA_VERSION` stays 4, and the `attestation_v4_is_v3_plus_five_flags` proof passes untouched. The stream order is unchanged too: `snapshot.reason` is still written after `snapshot.snapshot_digest`, not beside its flag, because moving it would reorder the stream and the removal of a version's flags must recover the previous version's stream exactly. The `note` line `placeholder_zeroed_when_unavailable` now says so. Lines changed in this file: one `field` line (the name), that `note` line (its text) and this header note.",
        ],
        format_version: None,
        caveats: &[],
        samples: &[attestation],
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
        format_version: None,
        caveats: &[],
        samples: &[keyring],
    },
    Format {
        id: "bundle-v1",
        krate: "fjell-diag-format",
        name: "DiagnosticBundleV1",
        version: "v0.6.0 (first frozen)",
        path: "crates/formats/fjell-diag-format/schema/bundle-v1.frozen",
        notes: &[
            "2026-09-24: this file used to claim `export_tick`, `source_service`, `entry_count` and `entries[]` of tag/tick/len/body; the encoder writes `created_tick`, `measurement_head`, `last_attestation` and two counted groups, `audit_events` and `semantic_intents`. Corrected toward the encoder; no byte moved (RFC-0.33-003 D3).",
            "2026-10-14: the format moved. RFC-0.35-002 added `audit_available` and bumped `DIAG_BUNDLE_VERSION` 1 -> 2, so this file's header said `(frozen)` about a layout that had just changed and named itself v1 while describing v2 bytes. The header now reads the crate's own constant (D11's mechanism, already used by the wire and on-disk formats), so it cannot say v1 again while the encoder writes v2; `version` above is kept as the release it was FIRST frozen in, which is what that field means.",
            "2026-10-15: RFC-0.36-001 bumped DIAG_BUNDLE_VERSION 2 -> 3 (closes E-076, E-079, E-080). `provider_id` and `keyring_anchor_epoch` are now written -- both were struct fields the digest formula in RFC-v0.4-005 §6.4 lists and the encoder had silently omitted (E-080), so the trust provider and key epoch a signed bundle claims were outside the signature. Three new availability flags (`keyring_anchor_epoch_available`, `measurement_head_available`, `last_attestation_available`) are each adjacent to the field they qualify, zeroing it in the encoder itself whenever unavailable rather than trusting the caller's raw value.",
            "2026-10-28: RFC-0.39-002 bumped DIAG_BUNDLE_VERSION 3 -> 4. `measurement_event_count` and `measurement_events_kernel_digested` (two u32) are written straight after `measurement_head`, zeroed by the encoder while the head is unavailable. They disclose the chain's composition beside the head -- how many events, and how many arrived by the path where the kernel produced the digest -- so that making the head readable does not let a signed bundle present a mostly-placeholder chain as measured (E-095). The second is a structural count, not a count of 'real' measurements. The 3 -> 4 digest move is proved, not assumed: `tests/golden.rs::diag_bundle_v4_is_v3_plus_two_counts` rewinds the v4 stream to v3 and recovers the v3 golden.",
        ],
        format_version: Some((|| fjell_diag_format::DIAG_BUNDLE_VERSION, "wire")),
        caveats: &[],
        samples: &[diag],
    },
    Format {
        id: "node-identity-v1",
        krate: "fjell-identity-format",
        name: "NodeIdentityV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-identity-format/schema/node-identity-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[identity],
    },
    Format {
        id: "board-v1",
        krate: "fjell-platform-format",
        name: "BoardProfileV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-platform-format/schema/board-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[board],
    },
    Format {
        id: "measurement-summary-v1",
        krate: "fjell-summary-format",
        name: "MeasurementSummaryV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-summary-format/schema/measurement-summary-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[measurement_summary],
    },
    Format {
        id: "release-summary-v1",
        krate: "fjell-summary-format",
        name: "ReleaseSummaryV1",
        version: "v0.7.0 (frozen)",
        path: "crates/formats/fjell-summary-format/schema/release-summary-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[release_summary],
    },
    Format {
        id: "snapshot-v2",
        krate: "fjell-snapshot-format",
        name: "SnapshotEnvelopeV2",
        version: "v0.7.0 (frozen) BREAKING-SCHEMA: see ADR-v0.7-004",
        path: "crates/formats/fjell-snapshot-format/schema/snapshot-v2.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[snapshot],
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
        format_version: None,
        caveats: &[],
        samples: &[semantic],
    },
    Format {
        id: "fleet-roster-v1",
        krate: "fjell-fleet-format",
        name: "FleetRosterV1",
        version: "v0.8.0 (first generated)",
        path: "crates/formats/fjell-fleet-format/schema/fleet-roster-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[fleet_roster],
    },
    Format {
        id: "fleet-policy-v1",
        krate: "fjell-fleet-format",
        name: "FleetPolicyV1",
        version: "v0.8.0 (first generated)",
        path: "crates/formats/fjell-fleet-format/schema/fleet-policy-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[fleet_policy],
    },
    Format {
        id: "wire-v1",
        krate: "fjell-semantic-format",
        name: "SemanticEnvelopeWireV1",
        version: "first generated in v0.34.0",
        path: "crates/formats/fjell-semantic-format/schema/wire-v1.frozen",
        notes: &[],
        format_version: Some((|| fjell_semantic_format::wire::WIRE_VERSION, "wire")),
        caveats: &[
            "what `decode` accepts or refuses -- the tag tables it maps back, the count limits it enforces, the stream/payload agreement it demands, decode_exact's trailing-byte rule. This file describes what `encode` WRITES; a decoder that read a different layout would not be caught by it (the round-trip test in fjell-schema is the only check)",
            "the values behind a tag byte (which byte means which enum variant lives in the tagged! tables), and the encoder's refusals (BufferTooSmall, TooLong) and their precedence",
        ],
        samples: &[
            wire_sample_0,
            wire_sample_1,
            wire_sample_2,
            wire_sample_3,
            wire_sample_4,
        ],
    },
    Format {
        id: "platform-v1",
        krate: "fjell-platform-format",
        name: "PlatformProfileV1",
        version: "v0.6.0 (frozen)",
        path: "crates/formats/fjell-platform-format/schema/platform-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[],
        samples: &[platform],
    },
    Format {
        id: "store-superblock",
        krate: "fjell-store-format",
        name: "StoreSuperblock",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-store-format/schema/store-superblock.frozen",
        notes: &[],
        format_version: Some((|| fjell_store_format::STORE_SUPERBLOCK_VERSION, "on disk")),
        caveats: &[],
        samples: &[store_superblock],
    },
    Format {
        id: "record-header",
        krate: "fjell-store-format",
        name: "RecordHeader",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-store-format/schema/record-header.frozen",
        notes: &[],
        format_version: Some((|| fjell_store_format::RECORD_HEADER_VERSION, "on disk")),
        caveats: &[],
        samples: &[record_header],
    },
    Format {
        id: "boot-control-block",
        krate: "fjell-upgrade-format",
        name: "BootControlBlock",
        version: "first generated in v0.33.0",
        path: "crates/formats/fjell-upgrade-format/schema/boot-control-block.frozen",
        notes: &[],
        format_version: Some((|| fjell_upgrade_format::BOOT_CONTROL_VERSION, "on disk")),
        caveats: &[],
        samples: &[boot_control],
    },
    Format {
        id: "chain-digest-v1",
        krate: "fjell-measure-format",
        name: "MeasurementEvent::compute_chain_digest",
        version: "first generated in v0.35.0",
        path: "crates/formats/fjell-measure-format/schema/chain-digest-v1.frozen",
        notes: &[],
        format_version: Some((|| fjell_measure_format::SCHEMA_VERSION, "chain")),
        caveats: &[
            "nothing reads these bytes back -- the measurement chain's whole point is a digest, not a decoded record -- so this file and the encoder are the only description a second implementation of the chain has",
        ],
        samples: &[chain],
    },
    Format {
        id: "image-set-digest-v1",
        krate: "fjell-measure-format",
        name: "compute_image_set_digest",
        version: "first generated in v0.38.0",
        path: "crates/formats/fjell-measure-format/schema/image-set-digest-v1.frozen",
        notes: &[],
        format_version: None,
        caveats: &[
            "nothing reads these bytes back -- the digest is the whole point, and the kernel and a host tool each recompute it from the same images -- so this file and the encoder are the only description a second implementation has; the format carries no version field of its own, its domain tag (`FJELL-IMAGE-SET-V1`) is the version (RFC-0.38-001 R2)",
        ],
        samples: &[image_set],
    },
    Format {
        id: "service-bundle-digest-v1",
        krate: "fjell-bundle-format",
        name: "compute_bundle_digest",
        version: "first generated in v0.35.0",
        path: "crates/formats/fjell-bundle-format/schema/service-bundle-digest-v1.frozen",
        notes: &[
            "this stream carries no version field of its own: `BUNDLE_SCHEMA_VERSION` exists in the crate and is written into the bundle header, but `compute_bundle_digest` never reads it (RFC-0.35-004 R6) -- None here states that rather than leaving the header below to imply a format version this digest does not have",
        ],
        format_version: None,
        caveats: &[
            "nothing reads these bytes back -- `verify_bundle` only recomputes and compares, it does not decode a reader's independent reading -- so this file and the encoder are the only description a second implementation has",
        ],
        samples: &[bundle],
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
            erratum: "E-073",
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
            erratum: "E-073",
            what: "`AuditRecordBin` (written raw by the kernel's `sys_audit_drain`), `AuditPersistRecord` and `AuditLogHeader` are `#[repr(C)]` layouts, padding-free with asserted sizes, and have no encoder to record",
        },
    ),
];
