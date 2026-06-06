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

/// Ownership metadata for a catalog entry (RFC-v0.7.5-001 / W-M-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogOwner {
    /// Workspace crate that owns this intent.
    pub crate_name: &'static str,
    /// Human-readable description of the owning subsystem.
    pub subsystem:  &'static str,
}

impl CatalogOwner {
    pub const fn new(crate_name: &'static str, subsystem: &'static str) -> Self {
        Self { crate_name, subsystem }
    }
}

/// One entry in the v1 frozen catalog.
#[derive(Clone, Copy, Debug)]
pub struct IntentEntry {
    /// Intent tag (unique, never reused).
    pub tag:    u16,
    /// Canonical dot-notation name (e.g. `"UPDATE.STAGING_STARTED"`).
    pub name:   &'static str,
    /// Field schema for this tag.
    pub schema: &'static IntentSchema,
    /// Ownership metadata (RFC-v0.7.5-001 / W-M-02).
    pub owner:  CatalogOwner,
}


// ── Catalog owner constants ───────────────────────────────────────────────────

const OWN_UPGRADED:  CatalogOwner = CatalogOwner::new("fjell-upgraded", "upgrade");
const OWN_ATTESTD:   CatalogOwner = CatalogOwner::new("fjell-attestd",  "attestation");
const OWN_KERNEL:    CatalogOwner = CatalogOwner::new("fjell-kernel",   "security-boundary");
const OWN_NETD:      CatalogOwner = CatalogOwner::new("fjell-netd",     "networking");
const OWN_RECOVERYD: CatalogOwner = CatalogOwner::new("fjell-recoveryd","recovery");
const OWN_DEVMGR:    CatalogOwner = CatalogOwner::new("fjell-devmgr",   "platform");
const OWN_MEASUREDD: CatalogOwner = CatalogOwner::new("fjell-measuredd","health");
const OWN_SYNCD:     CatalogOwner = CatalogOwner::new("fjell-syncd",    "distributed-sync");

/// The frozen v1 catalog.
pub const CATALOG_V1: &[IntentEntry] = &[
    // Update domain
    IntentEntry { tag: 0x0100, name: "UPDATE.STAGING_STARTED",
                  schema: &SCHEMA_UPDATE_STAGING_STARTED, owner: OWN_UPGRADED },
    IntentEntry { tag: 0x0101, name: "UPDATE.STAGING_ADVANCED",
                  schema: &SCHEMA_UPDATE_STAGING_ADVANCED, owner: OWN_UPGRADED },
    IntentEntry { tag: 0x0102, name: "UPDATE.STAGING_FAILED",
                  schema: &SCHEMA_UPDATE_STAGING_FAILED, owner: OWN_UPGRADED },
    IntentEntry { tag: 0x0103, name: "UPDATE.STAGING_CONFIRMED",
                  schema: &SCHEMA_UPDATE_STAGING_CONFIRMED, owner: OWN_UPGRADED },
    IntentEntry { tag: 0x0110, name: "UPDATE.ROLLBACK_BLOCKED",
                  schema: &SCHEMA_UPDATE_ROLLBACK_BLOCKED, owner: OWN_UPGRADED },
    IntentEntry { tag: 0x0111, name: "UPDATE.ROLLBACK_TO_PREVIOUS_SLOT",
                  schema: &SCHEMA_UPDATE_ROLLBACK_TO_PREVIOUS_SLOT, owner: OWN_UPGRADED },
    // Attestation domain
    IntentEntry { tag: 0x0120, name: "ATTEST.RECORD_SIGNED",
                  schema: &SCHEMA_ATTEST_RECORD_SIGNED, owner: OWN_ATTESTD },
    IntentEntry { tag: 0x0121, name: "ATTEST.RECORD_VERIFY_FAILED",
                  schema: &SCHEMA_ATTEST_RECORD_VERIFY_FAILED, owner: OWN_ATTESTD },
    // Security-boundary domain
    IntentEntry { tag: 0x0130, name: "SECURITY.REGISTRY_ENFORCING",
                  schema: &SCHEMA_SECURITY_REGISTRY_ENFORCING, owner: OWN_KERNEL },
    IntentEntry { tag: 0x0131, name: "SECURITY.PROVIDER_FAULTED",
                  schema: &SCHEMA_SECURITY_PROVIDER_FAULTED, owner: OWN_KERNEL },
    IntentEntry { tag: 0x0132, name: "SECURITY.KEYRING_EPOCH_ADVANCED",
                  schema: &SCHEMA_SECURITY_KEYRING_EPOCH_ADVANCED, owner: OWN_KERNEL },
    // Net domain
    IntentEntry { tag: 0x0140, name: "NET.LINK_UP",
                  schema: &SCHEMA_NET_LINK_UP, owner: OWN_NETD },
    IntentEntry { tag: 0x0141, name: "NET.LINK_DOWN",
                  schema: &SCHEMA_NET_LINK_DOWN, owner: OWN_NETD },
    IntentEntry { tag: 0x0142, name: "NET.SXT_CHANNEL_OPENED",
                  schema: &SCHEMA_NET_SXT_CHANNEL_OPENED, owner: OWN_NETD },
    IntentEntry { tag: 0x0143, name: "NET.SXT_CHANNEL_CLOSED",
                  schema: &SCHEMA_NET_SXT_CHANNEL_CLOSED, owner: OWN_NETD },
    // Recovery domain
    IntentEntry { tag: 0x0150, name: "RECOVERY.ENTERED",
                  schema: &SCHEMA_RECOVERY_ENTERED, owner: OWN_RECOVERYD },
    IntentEntry { tag: 0x0151, name: "RECOVERY.EXITED",
                  schema: &SCHEMA_RECOVERY_EXITED, owner: OWN_RECOVERYD },
    // Platform domain
    IntentEntry { tag: 0x0160, name: "PLATFORM.PROFILES_READY",
                  schema: &SCHEMA_PLATFORM_PROFILES_READY, owner: OWN_DEVMGR },
    // Health domain
    IntentEntry { tag: 0x0170, name: "HEALTH.TARGET_REACHED",
                  schema: &SCHEMA_HEALTH_TARGET_REACHED, owner: OWN_MEASUREDD },
    IntentEntry { tag: 0x0171, name: "HEALTH.TARGET_FAILED",
                  schema: &SCHEMA_HEALTH_TARGET_FAILED, owner: OWN_MEASUREDD },
];

/// Number of entries in the v1 catalog.
pub const fn catalog_len() -> usize { CATALOG_V1.len() }

/// Look up a catalog entry by tag.  Returns `None` for unknown tags
/// (including future-reserved ranges).
pub fn lookup_tag(tag: u16) -> Option<&'static IntentEntry> {
    CATALOG_V1.iter().find(|e| e.tag == tag)
}

// ── Reserved range ownership metadata (RFC-v0.7.5-001 / W-M-02) ──────────────

/// Ownership descriptor for a tag-range reservation.
#[derive(Clone, Copy, Debug)]
pub struct CatalogRangeOwner {
    pub range_start:            u16,
    pub range_end:              u16,
    pub domain:                 &'static str,
    pub owner_crate:            &'static str,
    pub reserved_for_version:   Option<&'static str>,
}

impl CatalogRangeOwner {
    pub const fn new(
        range_start: u16, range_end: u16,
        domain: &'static str, owner_crate: &'static str,
        reserved_for_version: Option<&'static str>,
    ) -> Self {
        Self { range_start, range_end, domain, owner_crate, reserved_for_version }
    }
    pub fn contains(self, tag: u16) -> bool {
        tag >= self.range_start && tag <= self.range_end
    }
}

/// Ownership table for the complete v1 catalog range.
pub const CATALOG_RANGES: &[CatalogRangeOwner] = &[
    CatalogRangeOwner::new(0x0100, 0x011F, "UPDATE",   "fjell-upgraded",  None),
    CatalogRangeOwner::new(0x0120, 0x012F, "ATTEST",   "fjell-attestd",   None),
    CatalogRangeOwner::new(0x0130, 0x013F, "SECURITY", "fjell-kernel",    None),
    CatalogRangeOwner::new(0x0140, 0x014F, "NET",      "fjell-netd",      None),
    CatalogRangeOwner::new(0x0150, 0x015F, "RECOVERY", "fjell-recoveryd", None),
    CatalogRangeOwner::new(0x0160, 0x016F, "PLATFORM", "fjell-devmgr",    None),
    CatalogRangeOwner::new(0x0170, 0x017F, "HEALTH",   "fjell-measuredd", None),
    CatalogRangeOwner::new(0x0180, 0x01FF, "SUMMARY",  "fjell-syncd",     None),
    // Reserved ranges — populated in future versions
    CatalogRangeOwner::new(0x0200, 0x02FF, "FLEET",    "fjell-syncd",     Some("v0.8")),
    CatalogRangeOwner::new(0x0300, 0x03FF, "SDK",      "fjell-sdk",       Some("v0.9")),
    CatalogRangeOwner::new(0x0400, 0x04FF, "HEALTH-EXT","fjell-recoveryd",Some("v0.9")),
];

/// Look up the range owner for a tag.
pub fn range_owner_for(tag: u16) -> Option<&'static CatalogRangeOwner> {
    CATALOG_RANGES.iter().find(|r| r.contains(tag))
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn every_catalog_entry_has_an_owner() {
        for entry in CATALOG_V1 {
            assert_ne!(entry.owner.crate_name, "",
                "Entry 0x{:04X} ({}) has empty owner", entry.tag, entry.name);
        }
    }

    #[test]
    fn fleet_range_reserved_for_v08() {
        let r = range_owner_for(0x0200).unwrap();
        assert_eq!(r.domain, "FLEET");
        assert_eq!(r.reserved_for_version, Some("v0.8"));
    }

    #[test]
    fn sdk_range_reserved_for_v09() {
        let r = range_owner_for(0x0300).unwrap();
        assert_eq!(r.domain, "SDK");
        assert_eq!(r.reserved_for_version, Some("v0.9"));
    }

    #[test]
    fn update_tags_owned_by_upgraded() {
        // Every catalog entry in 0x0100-0x011F must be owned by fjell-upgraded
        for entry in CATALOG_V1.iter().filter(|e| e.tag >= 0x0100 && e.tag <= 0x011F) {
            assert_eq!(entry.owner.crate_name, "fjell-upgraded",
                "Tag 0x{:04X} wrong owner", entry.tag);
        }
    }

    #[test]
    fn range_owner_lookup_works_for_all_catalog_entries() {
        for entry in CATALOG_V1 {
            let r = range_owner_for(entry.tag);
            assert!(r.is_some(), "No range owner for tag 0x{:04X}", entry.tag);
        }
    }
}
