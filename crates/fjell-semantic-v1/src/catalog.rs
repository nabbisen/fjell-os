//! Frozen v1 semantic intent catalog (RFC v0.5-004 §6.1).
//!
//! This table is locked.  Additions require a v0.5.x patch RFC; removals
//! or tag reuse require a v2 catalog.

use crate::schema::{IntentSchema, FieldDef, FieldKind};

// ── Update domain (0x0100..=0x011F) ──────────────────────────────────────────

const SCHEMA_UPDATE_STAGING_STARTED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "candidate_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "channel_id",   kind: FieldKind::U32, required: true },
    FieldDef { name: "counter",      kind: FieldKind::U32, required: true },
]};

const SCHEMA_UPDATE_STAGING_ADVANCED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "candidate_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "from_state",   kind: FieldKind::U16, required: true },
    FieldDef { name: "to_state",     kind: FieldKind::U16, required: true },
]};

const SCHEMA_UPDATE_STAGING_FAILED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "candidate_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "error_code",   kind: FieldKind::U16, required: true },
]};

const SCHEMA_UPDATE_STAGING_CONFIRMED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "candidate_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "counter",      kind: FieldKind::U32, required: true },
    FieldDef { name: "slot",         kind: FieldKind::U8,  required: true },
]};

const SCHEMA_UPDATE_ROLLBACK_BLOCKED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "channel_id",   kind: FieldKind::U32, required: true },
    FieldDef { name: "attempted",    kind: FieldKind::U32, required: true },
    FieldDef { name: "min_counter",  kind: FieldKind::U32, required: true },
]};

const SCHEMA_UPDATE_ROLLBACK_TO_PREVIOUS_SLOT: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "previous_slot", kind: FieldKind::U8,  required: true },
    FieldDef { name: "reason_code",   kind: FieldKind::U16, required: true },
]};

// ── Attestation domain (0x0120..=0x012F) ─────────────────────────────────────

const SCHEMA_ATTEST_RECORD_SIGNED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "record_id",   kind: FieldKind::U32, required: true },
    FieldDef { name: "profile",     kind: FieldKind::U8,  required: true },
    FieldDef { name: "provider_id", kind: FieldKind::U32, required: true },
]};

const SCHEMA_ATTEST_RECORD_VERIFY_FAILED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "record_id",  kind: FieldKind::U32, required: true },
    FieldDef { name: "error_code", kind: FieldKind::U16, required: true },
]};

// ── Security-boundary domain (0x0130..=0x013F) ───────────────────────────────

const SCHEMA_SECURITY_REGISTRY_ENFORCING: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "providers_registered", kind: FieldKind::U8, required: true },
]};

const SCHEMA_SECURITY_PROVIDER_FAULTED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "provider_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "reason_code", kind: FieldKind::U16, required: true },
]};

const SCHEMA_SECURITY_KEYRING_EPOCH_ADVANCED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "purpose",   kind: FieldKind::U8,  required: true },
    FieldDef { name: "old_epoch", kind: FieldKind::U32, required: true },
    FieldDef { name: "new_epoch", kind: FieldKind::U32, required: true },
]};

// ── Net domain (0x0140..=0x014F) ─────────────────────────────────────────────

const SCHEMA_NET_LINK_UP: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "device_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "mtu",       kind: FieldKind::U16, required: true },
]};

const SCHEMA_NET_LINK_DOWN: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "device_id",  kind: FieldKind::U32, required: true },
    FieldDef { name: "reason_code",kind: FieldKind::U16, required: true },
]};

const SCHEMA_NET_SXT_CHANNEL_OPENED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "channel_id",  kind: FieldKind::U32,    required: true },
    FieldDef { name: "kind",        kind: FieldKind::U8,     required: true },
    FieldDef { name: "server_name", kind: FieldKind::Bytes16, required: true },
]};

const SCHEMA_NET_SXT_CHANNEL_CLOSED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "channel_id",  kind: FieldKind::U32, required: true },
    FieldDef { name: "reason_code", kind: FieldKind::U16, required: true },
]};

// ── Recovery domain (0x0150..=0x015F) ────────────────────────────────────────

const SCHEMA_RECOVERY_ENTERED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "reason_code", kind: FieldKind::U16, required: true },
]};

const SCHEMA_RECOVERY_EXITED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "outcome_code", kind: FieldKind::U16, required: true },
]};

// ── Platform domain (0x0160..=0x016F) ────────────────────────────────────────

const SCHEMA_PLATFORM_PROFILES_READY: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "platform_digest", kind: FieldKind::Bytes32, required: true },
    FieldDef { name: "board_digest",    kind: FieldKind::Bytes32, required: true },
]};

// ── Health domain (0x0170..=0x017F) ──────────────────────────────────────────

const SCHEMA_HEALTH_TARGET_REACHED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "target_id", kind: FieldKind::U32, required: true },
    FieldDef { name: "status",    kind: FieldKind::U8,  required: true },
]};

const SCHEMA_HEALTH_TARGET_FAILED: IntentSchema = IntentSchema { fields: &[
    FieldDef { name: "target_id",  kind: FieldKind::U32, required: true },
    FieldDef { name: "reason_code",kind: FieldKind::U16, required: true },
]};

// ── Catalog entry type and table ──────────────────────────────────────────────

/// One entry in the v1 frozen catalog.
#[derive(Clone, Copy, Debug)]
pub struct IntentEntry {
    /// Intent tag (unique, never reused).
    pub tag:    u16,
    /// Canonical dot-notation name (e.g. `"UPDATE.STAGING_STARTED"`).
    pub name:   &'static str,
    /// Field schema for this tag.
    pub schema: &'static IntentSchema,
}

/// The frozen v1 catalog.
pub const CATALOG_V1: &[IntentEntry] = &[
    // Update domain
    IntentEntry { tag: 0x0100, name: "UPDATE.STAGING_STARTED",
                  schema: &SCHEMA_UPDATE_STAGING_STARTED },
    IntentEntry { tag: 0x0101, name: "UPDATE.STAGING_ADVANCED",
                  schema: &SCHEMA_UPDATE_STAGING_ADVANCED },
    IntentEntry { tag: 0x0102, name: "UPDATE.STAGING_FAILED",
                  schema: &SCHEMA_UPDATE_STAGING_FAILED },
    IntentEntry { tag: 0x0103, name: "UPDATE.STAGING_CONFIRMED",
                  schema: &SCHEMA_UPDATE_STAGING_CONFIRMED },
    IntentEntry { tag: 0x0110, name: "UPDATE.ROLLBACK_BLOCKED",
                  schema: &SCHEMA_UPDATE_ROLLBACK_BLOCKED },
    IntentEntry { tag: 0x0111, name: "UPDATE.ROLLBACK_TO_PREVIOUS_SLOT",
                  schema: &SCHEMA_UPDATE_ROLLBACK_TO_PREVIOUS_SLOT },
    // Attestation domain
    IntentEntry { tag: 0x0120, name: "ATTEST.RECORD_SIGNED",
                  schema: &SCHEMA_ATTEST_RECORD_SIGNED },
    IntentEntry { tag: 0x0121, name: "ATTEST.RECORD_VERIFY_FAILED",
                  schema: &SCHEMA_ATTEST_RECORD_VERIFY_FAILED },
    // Security-boundary domain
    IntentEntry { tag: 0x0130, name: "SECURITY.REGISTRY_ENFORCING",
                  schema: &SCHEMA_SECURITY_REGISTRY_ENFORCING },
    IntentEntry { tag: 0x0131, name: "SECURITY.PROVIDER_FAULTED",
                  schema: &SCHEMA_SECURITY_PROVIDER_FAULTED },
    IntentEntry { tag: 0x0132, name: "SECURITY.KEYRING_EPOCH_ADVANCED",
                  schema: &SCHEMA_SECURITY_KEYRING_EPOCH_ADVANCED },
    // Net domain
    IntentEntry { tag: 0x0140, name: "NET.LINK_UP",
                  schema: &SCHEMA_NET_LINK_UP },
    IntentEntry { tag: 0x0141, name: "NET.LINK_DOWN",
                  schema: &SCHEMA_NET_LINK_DOWN },
    IntentEntry { tag: 0x0142, name: "NET.SXT_CHANNEL_OPENED",
                  schema: &SCHEMA_NET_SXT_CHANNEL_OPENED },
    IntentEntry { tag: 0x0143, name: "NET.SXT_CHANNEL_CLOSED",
                  schema: &SCHEMA_NET_SXT_CHANNEL_CLOSED },
    // Recovery domain
    IntentEntry { tag: 0x0150, name: "RECOVERY.ENTERED",
                  schema: &SCHEMA_RECOVERY_ENTERED },
    IntentEntry { tag: 0x0151, name: "RECOVERY.EXITED",
                  schema: &SCHEMA_RECOVERY_EXITED },
    // Platform domain
    IntentEntry { tag: 0x0160, name: "PLATFORM.PROFILES_READY",
                  schema: &SCHEMA_PLATFORM_PROFILES_READY },
    // Health domain
    IntentEntry { tag: 0x0170, name: "HEALTH.TARGET_REACHED",
                  schema: &SCHEMA_HEALTH_TARGET_REACHED },
    IntentEntry { tag: 0x0171, name: "HEALTH.TARGET_FAILED",
                  schema: &SCHEMA_HEALTH_TARGET_FAILED },
];

/// Number of entries in the v1 catalog.
pub const fn catalog_len() -> usize { CATALOG_V1.len() }

/// Look up a catalog entry by tag.  Returns `None` for unknown tags
/// (including future-reserved ranges).
pub fn lookup_tag(tag: u16) -> Option<&'static IntentEntry> {
    CATALOG_V1.iter().find(|e| e.tag == tag)
}
