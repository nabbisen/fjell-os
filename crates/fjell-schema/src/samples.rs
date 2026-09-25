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

/// A semantic intent v1 envelope, encoded by the codec: the catalogue entry with
/// the most fields. Every field gets a distinct value of its
/// declared kind, and the **last optional** field, if any, is left absent so the
/// `present = 0` branch is in the bytes too.
pub fn semantic_intent() -> (u16, Vec<u8>) {
    use fjell_semantic_v1::encode;
    let (entry, values) = semantic_widest();
    let mut out = [0u8; 256];
    let n = encode(entry.tag, SEMANTIC_TICK, &values, &mut out).expect("encodes");
    (entry.tag, out[..n].to_vec())
}

/// `created_tick` of the semantic sample.
pub const SEMANTIC_TICK: u64 = 0x1112_1314_1516_1718;

/// The widest catalogue entry and a distinct value for each of its fields.
pub fn semantic_widest() -> (
    &'static fjell_semantic_v1::IntentEntry,
    Vec<fjell_semantic_v1::FieldValue>,
) {
    use fjell_semantic_v1::{CATALOG_V1, FieldValue};
    // The entry with the most fields (the first of them, on a tie).
    let entry = CATALOG_V1
        .iter()
        .rev()
        .max_by_key(|e| e.schema.fields.len())
        .expect("a non-empty catalogue");
    let mut absent = None;
    for (i, f) in entry.schema.fields.iter().enumerate() {
        if !f.required {
            absent = Some(i);
        }
    }
    let values: Vec<FieldValue> = entry
        .schema
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if Some(i) == absent {
                return FieldValue::Absent;
            }
            let n = 0x10 + i as u8;
            match f.kind {
                fjell_semantic_v1::FieldKind::U8 => FieldValue::U8(n),
                fjell_semantic_v1::FieldKind::U16 => FieldValue::U16(0x0100 + n as u16),
                fjell_semantic_v1::FieldKind::U32 => FieldValue::U32(0x0102_0300 + n as u32),
                fjell_semantic_v1::FieldKind::U64 => {
                    FieldValue::U64(0x0102_0304_0506_0700 + n as u64)
                }
                fjell_semantic_v1::FieldKind::Bytes16 => FieldValue::Bytes16([n; 16]),
                fjell_semantic_v1::FieldKind::Bytes32 => FieldValue::Bytes32(digest(n).0),
            }
        })
        .collect();
    (entry, values)
}

pub fn store_superblock() -> fjell_store_format::StoreSuperblock {
    let mut sb = fjell_store_format::StoreSuperblock::new(0x0102_0304_0506_0708);
    sb.log_tail_seq = 0x1112_1314_1516_1718;
    sb.active_checkpoint_seq = 0x2122_2324_2526_2728;
    sb.seal();
    sb
}

pub fn record_header() -> fjell_store_format::RecordHeader {
    let mut h = fjell_store_format::RecordHeader::new(
        fjell_store_format::RecordKind::ServiceState,
        0x0102_0304_0506_0708,
        0x2000,
    );
    h.crc32 = 0;
    h
}

pub fn boot_control_block() -> fjell_upgrade_format::BootControlBlock {
    let mut b = fjell_upgrade_format::BootControlBlock::new(0x0102_0304_0506_0708);
    b.seal();
    b
}

/// A three-member roster, every field distinct.
pub fn fleet_roster() -> fjell_fleet_format::NodeRoster {
    use fjell_fleet_format::roster::{NodeRoster, RosterEntry, TrustProfileTag};
    use fjell_identity_format::NodeId;
    let mut r = NodeRoster::new([0x71; 16], [0x72; 32]);
    r.generation = 0x0102_0304;
    for i in 0..3u8 {
        r.add_member(RosterEntry {
            identity_digest: digest(0x80 + i),
            node_id: NodeId([0x90 + i; 16]),
            trust_profile_tag: TrustProfileTag(0xA0 + i),
            active: i != 1,
            generation: 0x0B0C_0D00 + i as u32,
        })
        .expect("room");
    }
    r
}

/// A three-statement policy, every field distinct.
pub fn fleet_policy() -> fjell_fleet_format::FleetPolicy {
    use fjell_fleet_format::{FleetPolicy, PolicyAction, PolicyCondition, PolicyStatement};
    let mut p = FleetPolicy::new([0x71; 16], digest(0xC0));
    p.policy_generation = 0x1112_1314;
    let acts = [
        PolicyAction::ReplaceProvider,
        PolicyAction::InitiateRollout,
        PolicyAction::RemoteDiag,
    ];
    for (i, a) in acts.into_iter().enumerate() {
        p.add_statement(PolicyStatement {
            action: a,
            condition: PolicyCondition::Always,
            allow: i != 1,
            audit_tag: 0x0D00 + i as u16,
        })
        .expect("room");
    }
    p
}

// ── fjell-semantic-format::wire ───────────────────────────────────────────────
//
// The wire codec is not a flat field list: it has a three-arm tagged payload, optional
// fields, and a five-arm tagged union (`FactValue`). One encoder run shows one arm, so its
// description is recorded from a **set** of samples (RFC-0.34-003 §B), and the set must
// reach every arm. The two `arm` helpers below are exhaustive `match`es on purpose:
// adding a variant to the model is a compile error here, not a silent hole in the file.

use fjell_semantic_format::{
    ActionId, ActionKind, ActionSpec, BoundedText, CapabilityRequirement, ConfirmationPolicy,
    Consequence, CorrelationId, EventKind, EventNode, EventResult, FactValue, FixedVec, Importance,
    IntentKind, IntentNode, NodeId, ResourceName, Reversibility, SemanticEnvelope, SemanticPayload,
    Severity, StateFact, StateKind, StateNode, Status, TextId, TextToken,
};

/// Which arm of the payload union an envelope takes (exhaustive by construction).
pub fn payload_arm(p: &SemanticPayload) -> usize {
    match p {
        SemanticPayload::Intent(_) => 0,
        SemanticPayload::State(_) => 1,
        SemanticPayload::Event(_) => 2,
    }
}
pub const PAYLOAD_ARMS: usize = 3;

/// Which arm of `FactValue` a fact takes (exhaustive by construction).
pub fn fact_arm(v: &FactValue) -> usize {
    match v {
        FactValue::Bool(_) => 0,
        FactValue::U64(_) => 1,
        FactValue::I64(_) => 2,
        FactValue::Text(_) => 3,
        FactValue::Ratio { .. } => 4,
    }
}
pub const FACT_ARMS: usize = 5;

fn tok(id: u32, s: &str) -> TextToken {
    TextToken {
        id: TextId(id),
        fallback: BoundedText::from_str(s),
    }
}

fn node_id(n: u16) -> NodeId {
    NodeId {
        producer_index: 0x0100 + n,
        local_sequence: 0x0201_0000 + n as u32,
    }
}

/// Five envelopes that together reach every arm of the codec: every payload arm, every
/// `FactValue` arm, each optional both present and absent, and every counted group with
/// at least one element (and one intent with none).
pub fn semantic_envelopes() -> Vec<SemanticEnvelope> {
    // 1. An intent with two actions (one with a required capability, one without), one
    //    consequence, an expiry, and a correlation id.
    let mut actions = FixedVec::new();
    actions.push(ActionSpec {
        action_id: ActionId(0x0A01),
        label: tok(0x1111, "acknowledge"),
        kind: ActionKind::Confirm,
        required_capability: Some(CapabilityRequirement {
            resource_class: BoundedText::from_str("service"),
            resource_name: ResourceName::new("storaged"),
            rights: 0x0000_00F1,
        }),
        reversibility: Reversibility::PartiallyReversible,
        confirmation: ConfirmationPolicy::Required,
    });
    actions.push(ActionSpec {
        action_id: ActionId(0x0A02),
        label: tok(0x2222, "retry"),
        kind: ActionKind::Retry,
        required_capability: None,
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    });
    let mut consequences = FixedVec::new();
    consequences.push(Consequence {
        level: Severity::Important,
        text: tok(0x3333, "data may be lost"),
    });
    let mut i1 = SemanticEnvelope::new_intent(
        node_id(1),
        0x1112_1314_1516_1718,
        IntentNode {
            kind: IntentKind::ActionRequest,
            title: tok(0x4444, "Update available"),
            description: tok(0x5555, "A new release is staged"),
            severity: Severity::Critical,
            actions,
            consequences,
            expires_at_tick: Some(0x2122_2324_2526_2728),
        },
    );
    i1.correlation_id = Some(CorrelationId(0x3132_3334_3536_3738));

    // 2. The minimal intent: no actions, no consequences, no expiry, no correlation.
    let i2 = SemanticEnvelope::new_intent(
        node_id(2),
        2,
        IntentNode {
            kind: IntentKind::Information,
            title: tok(0x6666, "Hello"),
            description: tok(0x7777, ""),
            severity: Severity::Low,
            actions: FixedVec::new(),
            consequences: FixedVec::new(),
            expires_at_tick: None,
        },
    );

    // 3. A state with one fact of every `FactValue` arm.
    let mut facts = FixedVec::new();
    let vals = [
        FactValue::Bool(true),
        FactValue::U64(0x4142_4344_4546_4748),
        FactValue::I64(-0x5152_5354_5556_5758),
        FactValue::Text(tok(0x8888, "ready")),
        FactValue::Ratio {
            numerator: 0x6162_6364_6566_6768,
            denominator: 0x7172_7374_7576_7778,
        },
    ];
    for (i, v) in vals.into_iter().enumerate() {
        facts.push(StateFact {
            key: tok(0x9000 + i as u32, "fact"),
            value: v,
            importance: Importance::High,
        });
    }
    let s3 = SemanticEnvelope::new_state(
        node_id(3),
        3,
        StateNode {
            kind: StateKind::ServiceStatus,
            title: tok(0xA111, "Service"),
            summary: tok(0xA222, "All well"),
            status: Status::Warning,
            facts,
        },
    );

    // 4. An event with a subject and a related audit sequence, and a correlation id.
    let mut e4 = SemanticEnvelope::new_event(
        node_id(4),
        4,
        EventNode {
            kind: EventKind::ActionCompleted,
            title: tok(0xB111, "Done"),
            description: tok(0xB222, "The action completed"),
            severity: Severity::Normal,
            result: EventResult::Ok,
            subject: Some(ResourceName::new("proxy-text")),
            related_audit_seq: Some(0x8182_8384_8586_8788),
        },
    );
    e4.correlation_id = Some(CorrelationId(9));

    // 5. An event with neither.
    let e5 = SemanticEnvelope::new_event(
        node_id(5),
        5,
        EventNode {
            kind: EventKind::ServiceReady,
            title: tok(0xC111, "Ready"),
            description: tok(0xC222, ""),
            severity: Severity::Low,
            result: EventResult::NotApplicable,
            subject: None,
            related_audit_seq: None,
        },
    );
    vec![i1, i2, s3, e4, e5]
}
