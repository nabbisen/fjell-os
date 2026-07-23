//! `DiagnosticBundle` and its component record types (RFC v0.4-005 §5.2).

use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;

/// Schema version for all `v0.4.0` bundles.
pub const DIAG_BUNDLE_VERSION: u16 = 1;
/// Maximum audit events stored per bundle.
pub const MAX_AUDIT_EVENTS:     usize = 64;
/// Maximum semantic intents stored per bundle.
pub const MAX_SEMANTIC_INTENTS: usize = 32;

/// A redacted, schema-versioned diagnostic record (RFC v0.4-005 §5.2).
///
/// All variable-length or payload fields are dropped; only typed, enumerated
/// data reaches the bundle (§6.3 redaction rules).
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticBundle {
    pub schema_version:        u16,
    pub bundle_id:             [u8; 8],
    pub created_tick:          u64,
    pub provider_id:           TrustProviderId,
    pub keyring_anchor_epoch:  u32,
    /// SHA-256 of the current measurement chain head.
    pub measurement_head:      Digest32,
    /// SHA-256 digest of the last `AttestationRecordV2`.
    pub last_attestation:      Digest32,
    pub audit_event_count:     u8,
    pub audit_events:          [DiagAuditEvent; MAX_AUDIT_EVENTS],
    pub semantic_intent_count: u8,
    pub semantic_intents:      [DiagIntent; MAX_SEMANTIC_INTENTS],
    /// SHA-256 over the canonical serialisation of this bundle (§6.4).
    pub bundle_digest:         Digest32,
}

/// A single redacted audit event projected into the diagnostic bundle.
///
/// Only fields needed for operator triage — no payload bytes, no strings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagAuditEvent {
    /// Monotonic sequence number from the audit log.
    pub seq:      u32,
    /// Allow-listed event kind tag (§6.1).
    pub kind_tag: u16,
    /// Reason / error code; 0 if not applicable.
    pub code:     u16,
    /// Kernel tick at which the event was recorded.
    pub at_tick:  u64,
}

impl DiagAuditEvent {
    pub const EMPTY: Self = Self { seq: 0, kind_tag: 0, code: 0, at_tick: 0 };
}

/// A single redacted semantic intent projected into the diagnostic bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagIntent {
    pub seq:        u32,
    /// Allow-listed intent tag (§6.2).
    pub intent_tag: u16,
    pub code:       u16,
    pub at_tick:    u64,
}

impl DiagIntent {
    pub const EMPTY: Self = Self { seq: 0, intent_tag: 0, code: 0, at_tick: 0 };
}

impl DiagnosticBundle {
    /// Construct a zero-initialised bundle (before builder finalises it).
    pub const fn zeroed() -> Self {
        Self {
            schema_version:        DIAG_BUNDLE_VERSION,
            bundle_id:             [0u8; 8],
            created_tick:          0,
            provider_id:           TrustProviderId(0),
            keyring_anchor_epoch:  0,
            measurement_head:      Digest32([0u8; 32]),
            last_attestation:      Digest32([0u8; 32]),
            audit_event_count:     0,
            audit_events:          [DiagAuditEvent::EMPTY; MAX_AUDIT_EVENTS],
            semantic_intent_count: 0,
            semantic_intents:      [DiagIntent::EMPTY; MAX_SEMANTIC_INTENTS],
            bundle_digest:         Digest32([0u8; 32]),
        }
    }
}
