//! One representative value per format, built through each crate's public API.
//!
//! Every field carries a **distinct** value, so a reordered or dropped field
//! changes the bytes: a sample whose fields were all zero would let two swapped
//! fields hide. They are used two ways — the recorder runs an encoder over them to
//! learn a layout, and the golden tests pin the bytes each encoder produces.

use fjell_attestation_format::AttestationRecordId;
use fjell_attestation_format::v2::AttestationRecordV2;
use fjell_keyring::{
    AuthorityClass, KeyEpoch, Keyring, KeyringSnapshot, SignatureAlgorithm, TrustAnchor,
};
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::{KeyPurpose, TrustProviderId};
use fjell_upgrade_format::release_metadata::{Provenance, ReleaseMetadata};
use fjell_upgrade_format::rollback_record::{AdvanceSource, RollbackRecord};

/// A digest whose 32 bytes are `base, base+1, …` — distinct within itself.
pub fn digest(base: u8) -> Digest32 {
    let mut d = [0u8; 32];
    for (i, b) in d.iter_mut().enumerate() {
        *b = base.wrapping_add(i as u8);
    }
    Digest32(d)
}

pub fn rollback_record() -> RollbackRecord {
    RollbackRecord::new(
        *b"stable\0\0",
        0x0102_0304_0506_0708,
        0x1112_1314_1516_1718,
        AdvanceSource::RecoveryReset,
    )
}

pub fn release_metadata() -> ReleaseMetadata {
    ReleaseMetadata::new(
        *b"lts\0\0\0\0\0",
        0x2122_2324_2526_2728,
        0x3132_3334_3536_3738,
        digest(0x40),
        KeyEpoch(0x4142_4344),
        TrustProviderId::new(0x5152_5354),
        digest(0x60),
        0x6162_6364_6566_6768,
        Provenance {
            builder_tool_id: *b"toolname",
            builder_version: *b"1.2.3\0\0\0",
        },
    )
}

pub fn attestation_v2() -> AttestationRecordV2 {
    let mut r = AttestationRecordV2::dev(
        AttestationRecordId(*b"AT7654 1"),
        TrustProviderId::new(0x0A0B_0C0D),
        *b"chan-x\0\0",
        [
            0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE,
            0xCF, 0xD0,
        ],
        0x7071_7273_7475_7677,
        0x0E0F_1011,
    );
    r.created_tick = 0x8081_8283_8485_8687;
    r.provider.provider_generation = 0x9091;
    r.keyring.active_epoch_attestation = 0xA1A2_A3A4;
    r.keyring.active_epoch_release = 0xB1B2_B3B4;
    r.keyring.active_epoch_policy = 0xC1C2_C3C4;
    r.keyring.keyring_snapshot_digest = digest(0x10);
    r.boot.selected_slot = 1;
    r.boot.boot_id = 0xD1D2_D3D4_D5D6_D7D8;
    r.boot.kernel_digest = digest(0x20);
    r.verification.release_digest = digest(0x30);
    r.verification.rootfs_digest = digest(0x40);
    r.verification.policy_digest = digest(0x50);
    r.verification.release_verified = true;
    r.verification.rootfs_verified = false;
    r.verification.policy_verified = true;
    r.measurement.head_seq = 0x1A1B_1C1D_1E1F_2021;
    r.measurement.chain_digest = digest(0x60);
    r.measurement.included_from_seq = 0x2A2B_2C2D_2E2F_3031;
    r.measurement.included_to_seq = 0x3A3B_3C3D_3E3F_4041;
    r.snapshot.snapshot_id = *b"SNAPSH01";
    r.snapshot.snapshot_digest = digest(0x70);
    r.snapshot.reason = 3;
    r.health.target = *b"healthd\0";
    r.health.status = 2;
    r.rollback.last_advance_source = 2;
    r.rollback.trust_provider_counter_supported = true;
    r.rollback.trust_provider_counter_value = 0x5A5B_5C5D_5E5F_6061;
    r.freshness.generation = 0xE1E2_E3E4;
    r.freshness.key_epoch = 0xF1F2_F3F4;
    r.freshness.status = 4;
    r.freshness.nonce_class = 2;
    r
}

/// A keyring with three anchors of different purposes and epochs.
pub fn keyring_snapshot() -> KeyringSnapshot {
    let mut k = Keyring::new();
    for (purpose, epoch, byte) in [
        (KeyPurpose::ReleaseVerification, 1u32, 0xA1u8),
        (KeyPurpose::PolicyVerification, 2, 0xB2),
        (KeyPurpose::AttestationSigning, 3, 0xC3),
    ] {
        let anchor = TrustAnchor::new(
            purpose,
            SignatureAlgorithm::DevDigest32,
            AuthorityClass::Standard,
            KeyEpoch(epoch),
            &[byte; 32],
        )
        .expect("anchor fits");
        k.install(anchor).expect("install");
    }
    KeyringSnapshot::from_keyring(&k)
}

pub fn diag_bundle() -> fjell_diag_format::DiagnosticBundle {
    use fjell_diag_format::builder::BundleBuilder;
    use fjell_diag_format::events::{AUDIT_KERNEL_BOOT_BANNER, AUDIT_UPGRADE_STATE_TRANSITION};
    use fjell_diag_format::intents::{INTENT_UPDATE_STAGING_FAILED, INTENT_UPDATE_STAGING_STARTED};
    let mut b = BundleBuilder::new(
        *b"DG123456",
        0x0102_0304_0506_0708,
        TrustProviderId::new(0x1112_1314),
        0x2122_2324,
        digest(0x30),
        digest(0x50),
    );
    b.add_audit_event(
        0x3132_3334,
        AUDIT_KERNEL_BOOT_BANNER,
        0x4142,
        0x5152_5354_5556_5758,
    )
    .expect("allowed");
    b.add_audit_event(
        0x6162_6364,
        AUDIT_UPGRADE_STATE_TRANSITION,
        0x7172,
        0x8182_8384_8586_8788,
    )
    .expect("allowed");
    b.add_intent(
        0x9192_9394,
        INTENT_UPDATE_STAGING_STARTED,
        0xA1A2,
        0xB1B2_B3B4_B5B6_B7B8,
    )
    .expect("allowed");
    b.add_intent(
        0xC1C2_C3C4,
        INTENT_UPDATE_STAGING_FAILED,
        0xD1D2,
        0xE1E2_E3E4_E5E6_E7E8,
    )
    .expect("allowed");
    b.finalise()
}

pub fn node_identity() -> fjell_identity_format::NodeIdentity {
    use fjell_identity_format::identity::NodeIdentityBuilder;
    use fjell_identity_format::{AttestationPubkey, NodeAlias, NodeId, NodeIdentity};
    let mut alias = [0u8; 32];
    alias[..9].copy_from_slice(b"node-test");
    NodeIdentity::build(NodeIdentityBuilder {
        node_id: NodeId([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ]),
        alias: NodeAlias(alias),
        created_tick: 0x1112_1314_1516_1718,
        trust_provider_id: 0x2122_2324,
        trust_profile_tag: 0x25,
        attestation_pubkey: AttestationPubkey(digest(0x30).0),
        platform_digest: digest(0x50),
        board_digest: digest(0x70),
    })
    .expect("identity builds")
}

pub fn platform_profile() -> fjell_platform_format::PlatformProfile {
    fjell_platform_format::PlatformProfile::qemu_virt_default()
}

pub fn board_profile() -> fjell_platform_format::BoardProfile {
    fjell_platform_format::BoardProfile::qemu_virt_default(digest(0x90))
}

pub fn measurement_summary() -> fjell_summary_format::MeasurementSummary {
    let mut s = fjell_summary_format::MeasurementSummary::new(
        [
            0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD,
            0xAE, 0xAF,
        ],
        0x0102_0304_0506_0708,
        0x1112_1314_1516_1718,
        digest(0x20),
        digest(0x40),
    );
    s.add_kind_count(0x31, 0x3231_3033).expect("room");
    s.add_kind_count(0x41, 0x4241_4043).expect("room");
    s
}

pub fn release_summary() -> fjell_summary_format::ReleaseSummary {
    use fjell_summary_format::{ChannelSummary, ReleaseSummary};
    let mut s = ReleaseSummary::new(
        [
            0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD,
            0xBE, 0xBF,
        ],
        0x0102_0304_0506_0708,
    );
    for (i, ch) in [*b"stable\0\0", *b"lts\0\0\0\0\0"].into_iter().enumerate() {
        let n = (i as u64 + 1) * 0x0101_0101_0101_0101;
        s.add_channel(ChannelSummary {
            channel_id: ch,
            current_counter: n,
            min_counter: n + 1,
            active_anchor_epoch: 0x5150_0000 + i as u32,
            last_confirm_tick: n + 2,
            last_advance_source: fjell_summary_format::AdvanceSource::SnapshotSync,
        })
        .expect("room");
    }
    s
}

/// A snapshot envelope of the given schema version (1 or 2). The domain byte of
/// each record is written only from version 2.
pub fn snapshot_envelope(version: u16) -> fjell_snapshot_format::SnapshotEnvelope {
    use fjell_snapshot_format::{ConflictDomain, SnapshotEnvelope, SnapshotRecord};
    let mut e =
        SnapshotEnvelope::new_v2(digest(0x10), 0x0102_0304_0506_0708, [0xC0; 16].map(|_| 0));
    e.nonce = [
        0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE,
        0xCF,
    ];
    e.schema_version = version;
    for (i, domain) in [ConflictDomain::LocallyConfirmed, ConflictDomain::Contested]
        .into_iter()
        .enumerate()
    {
        let mut body = [0u8; 64];
        for (j, b) in body.iter_mut().enumerate() {
            *b = (0xD0 + i as u8).wrapping_add(j as u8);
        }
        e.push_record(SnapshotRecord {
            domain,
            kind: 0x1100 + i as u16,
            seq: 0x2120_0000 + i as u64,
            body,
            body_len: 5 + i as u32,
        })
        .expect("room");
    }
    e
}
