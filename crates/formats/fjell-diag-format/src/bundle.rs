//! `DiagnosticBundle` and its component record types (RFC v0.4-005 §5.2).

use fjell_canon::Canon;
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;

/// Schema version for all `v0.4.0` bundles.
///
/// RFC-0.35-002 (closes E-068): bumped 1 -> 2. `audit_available` is a new
/// field in the canonical byte stream `bundle_digest` covers — every
/// digest and signature computed over a v1 bundle stops verifying against
/// a v2 one, by design; `tests/golden.rs`'s own header requires exactly
/// this pairing (a version bump and a note) before its stored digest may
/// change. No new domain separator: unlike `fjell-attestation-format`'s
/// V1/V2 split, this bundle is never cross-verified against a differently
/// versioned signature — `schema_version` already written into the bytes
/// is the field a reader checks.
///
/// RFC-0.36-001 (closes E-076, E-079, E-080): bumped 2 -> 3. Three changes
/// to the canonical byte stream, all expected to move this digest (the one
/// line in this project's history where that is the point, not a defect to
/// avoid — see `tests/golden.rs`'s own note on this bump): `provider_id`
/// and `keyring_anchor_epoch` are now written (E-080 — both were struct
/// fields the signed digest silently omitted, though `RFC-v0.4-005` §6.4
/// lists both); `measurement_head_available` and `last_attestation_available`
/// are new fields, each adjacent to the field it qualifies; the fabricated
/// semantic intent is gone from every bundle this crate can build.
///
/// RFC-0.39-002 (R4): bumped 3 -> 4. Two fixed-width fields are written beside
/// `measurement_head`, **zeroed by the encoder** whenever the head is unavailable:
/// `measurement_event_count` and `measurement_events_kernel_digested`. They exist
/// because installing the capability that makes the head readable would
/// otherwise let a signed bundle say "here is this node's measurement root" while
/// withholding that three of the chain's four events carry a digest the appending
/// service supplied (E-095). Both move the bundle digest, by design.
pub const DIAG_BUNDLE_VERSION: u16 = 4;
/// Maximum audit events stored per bundle.
pub const MAX_AUDIT_EVENTS: usize = 64;
/// Maximum semantic intents stored per bundle.
pub const MAX_SEMANTIC_INTENTS: usize = 32;

/// A redacted, schema-versioned diagnostic record (RFC v0.4-005 §5.2).
///
/// All variable-length or payload fields are dropped; only typed, enumerated
/// data reaches the bundle (§6.3 redaction rules).
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticBundle {
    pub schema_version: u16,
    pub bundle_id: [u8; 8],
    pub created_tick: u64,
    pub provider_id: TrustProviderId,
    /// `false` until something queries a real keyring anchor epoch — RFC-0.36-001
    /// (closes E-079's third instance, folded in at review): nothing in this
    /// crate or `fjell-diagnosticsd` has ever obtained one (`diagnosticsd` has
    /// no `fjell-keyring` dependency at all), so this is permanently `false`
    /// today, the same shape as `audit_available`. `write_canonical` zeroes
    /// `keyring_anchor_epoch` in the digest whenever this is `false`, so a
    /// caller cannot pass a value and have it count.
    pub keyring_anchor_epoch_available: bool,
    pub keyring_anchor_epoch: u32,
    /// `true` only when `query_measurement_head` actually received a reply —
    /// RFC-0.36-001 (closes E-076): a recv error or an unexpected tag used to
    /// produce a 32-byte zero digest indistinguishable from a real one.
    /// Whatever sets this `true` must be the code that obtained the value; see
    /// `rfcs/ERRATA.md`'s E-076 entry.
    pub measurement_head_available: bool,
    /// SHA-256 of the current measurement chain head.
    pub measurement_head: Digest32,
    /// How many events the chain had appended when the head was read
    /// (RFC-0.39-002 R3/R4). Written whether or not the head is available, and
    /// **zero when it is not**: `write_canonical` does not trust the caller's raw
    /// value.
    pub measurement_event_count: u32,
    /// How many of those events arrived by the path where **the kernel produced
    /// the digest** (`measuredd`'s `APPEND_IMAGE_SET`), as opposed to the path
    /// where the appending service supplied it (`APPEND_EVENT`).
    ///
    /// **A structural count, not a semantic one.** `measuredd` cannot look at 32
    /// bytes and know whether they are a hash or a literal, so this does not say
    /// how many events are "real". `measurement_event_count` minus this is the
    /// number of events whose digest a service supplied; what those digests are
    /// is not knowable here (today they are `init`'s literal `0xAB` -- E-095). A
    /// reader who treats this field as "how much of the chain is trustworthy"
    /// has read more into it than it says. Zero when the head is unavailable.
    pub measurement_events_kernel_digested: u32,
    /// `false` until something queries a real last-attestation digest —
    /// RFC-0.36-001 (closes E-079): nothing has ever obtained one, the same
    /// shape as `audit_available` and `keyring_anchor_epoch_available`.
    /// Permanently `false` today.
    pub last_attestation_available: bool,
    /// SHA-256 digest of the last `AttestationRecordV2`.
    pub last_attestation: Digest32,
    /// `false` whenever the audit section was never queried — RFC-0.35-002
    /// (closes E-068): the audit-query sub-protocol diagnosticsd once sent
    /// was retracted, never implemented by anything, so `audit_event_count
    /// == 0` alone cannot say whether that means "queried, no records" or
    /// "not implemented." This is the distinction the caller needs; see
    /// `rfcs/ERRATA.md`'s E-068 entry for what implementing it requires.
    pub audit_available: bool,
    pub audit_event_count: u8,
    pub audit_events: [DiagAuditEvent; MAX_AUDIT_EVENTS],
    pub semantic_intent_count: u8,
    pub semantic_intents: [DiagIntent; MAX_SEMANTIC_INTENTS],
    /// SHA-256 over the canonical serialisation of this bundle (§6.4).
    pub bundle_digest: Digest32,
}

/// A single redacted audit event projected into the diagnostic bundle.
///
/// Only fields needed for operator triage — no payload bytes, no strings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagAuditEvent {
    /// Monotonic sequence number from the audit log.
    pub seq: u32,
    /// Allow-listed event kind tag (§6.1).
    pub kind_tag: u16,
    /// Reason / error code; 0 if not applicable.
    pub code: u16,
    /// Kernel tick at which the event was recorded.
    pub at_tick: u64,
}

impl DiagAuditEvent {
    pub const EMPTY: Self = Self {
        seq: 0,
        kind_tag: 0,
        code: 0,
        at_tick: 0,
    };
}

/// A single redacted semantic intent projected into the diagnostic bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagIntent {
    pub seq: u32,
    /// Allow-listed intent tag (§6.2).
    pub intent_tag: u16,
    pub code: u16,
    pub at_tick: u64,
}

impl DiagIntent {
    pub const EMPTY: Self = Self {
        seq: 0,
        intent_tag: 0,
        code: 0,
        at_tick: 0,
    };
}

impl DiagnosticBundle {
    /// Construct a zero-initialised bundle (before builder finalises it).
    pub const fn zeroed() -> Self {
        Self {
            schema_version: DIAG_BUNDLE_VERSION,
            bundle_id: [0u8; 8],
            created_tick: 0,
            provider_id: TrustProviderId(0),
            keyring_anchor_epoch_available: false,
            keyring_anchor_epoch: 0,
            measurement_head_available: false,
            measurement_head: Digest32([0u8; 32]),
            measurement_event_count: 0,
            measurement_events_kernel_digested: 0,
            last_attestation_available: false,
            last_attestation: Digest32([0u8; 32]),
            audit_available: false,
            audit_event_count: 0,
            audit_events: [DiagAuditEvent::EMPTY; MAX_AUDIT_EVENTS],
            semantic_intent_count: 0,
            semantic_intents: [DiagIntent::EMPTY; MAX_SEMANTIC_INTENTS],
            bundle_digest: Digest32([0u8; 32]),
        }
    }
}

/// What `measuredd` reported about its chain, when the bundle was built
/// (RFC-0.39-002). One value for the whole measurement section: a bundle either
/// has one of these or has **no** measurement section, so "unavailable" cannot be
/// combined with a head or a count -- the combination the previous
/// `(head, available)` pair allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasurementReport {
    head: Digest32,
    event_count: u32,
    kernel_digested: u32,
}

/// Why a [`MeasurementReport`] was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementReportError {
    /// More events claimed to be kernel-digested than the chain holds. No chain
    /// can be in that state, so a service reporting it is wrong, and the bundle
    /// must not repeat it: the caller treats the whole section as unavailable.
    Incoherent,
}

impl MeasurementReport {
    /// A report, or [`MeasurementReportError::Incoherent`] if `kernel_digested >
    /// event_count`.
    pub fn new(
        head: Digest32,
        event_count: u32,
        kernel_digested: u32,
    ) -> Result<Self, MeasurementReportError> {
        if kernel_digested > event_count {
            return Err(MeasurementReportError::Incoherent);
        }
        Ok(Self {
            head,
            event_count,
            kernel_digested,
        })
    }
    pub fn head(&self) -> Digest32 {
        self.head
    }
    pub fn event_count(&self) -> u32 {
        self.event_count
    }
    pub fn kernel_digested(&self) -> u32 {
        self.kernel_digested
    }
}

impl DiagnosticBundle {
    /// The canonical byte stream `bundle_digest` is taken over (RFC v0.4-005
    /// §6.4) — **the function the digest is computed from and the frozen schema
    /// is generated from**. The schema version written is the crate's constant,
    /// as it always was, not the value in `self.schema_version`.
    pub fn write_canonical(&self, c: &mut dyn Canon) {
        c.domain(b"FJELL-DIAG-V1");
        c.u16("schema_version", DIAG_BUNDLE_VERSION);
        c.bytes("bundle_id", &self.bundle_id);
        c.u64("created_tick", self.created_tick);
        // RFC-0.36-001 (closes E-080): `provider_id` and `keyring_anchor_epoch`
        // are struct fields `RFC-v0.4-005` §6.4's own digest formula lists, in
        // this order, right after `created_tick` and before `measurement_head`
        // — previously omitted entirely, so the trust provider and key epoch a
        // signed bundle claims were outside the signature.
        c.u32("provider_id", self.provider_id.0);
        c.u8(
            "keyring_anchor_epoch_available",
            self.keyring_anchor_epoch_available as u8,
        );
        c.note(
            "keyring_anchor_epoch_zeroed_when_unavailable",
            "fixed width: a u32 written whether or not \
             keyring_anchor_epoch_available is 1; zero when unavailable, not \
             omitted and not the caller's raw value",
        );
        c.u32(
            "keyring_anchor_epoch",
            if self.keyring_anchor_epoch_available {
                self.keyring_anchor_epoch
            } else {
                0
            },
        );
        c.u8(
            "measurement_head_available",
            self.measurement_head_available as u8,
        );
        c.note(
            "measurement_head_zeroed_when_unavailable",
            "fixed width: 32 bytes written whether or not \
             measurement_head_available is 1; zero when unavailable, not \
             omitted and not the caller's raw value",
        );
        c.bytes(
            "measurement_head",
            if self.measurement_head_available {
                &self.measurement_head.0
            } else {
                &[0u8; 32]
            },
        );
        c.note(
            "measurement_counts_zeroed_when_unavailable",
            "fixed width: two u32 written whether or not measurement_head_available \
             is 1; zero when unavailable, not omitted and not the caller's raw value",
        );
        c.u32(
            "measurement_event_count",
            if self.measurement_head_available {
                self.measurement_event_count
            } else {
                0
            },
        );
        c.note(
            "measurement_events_kernel_digested_is",
            "a structural count: the events whose digest arrived by the path where \
             the kernel produced it. It is not a count of 'real' measurements; the \
             rest are events whose digest the appending service supplied",
        );
        c.u32(
            "measurement_events_kernel_digested",
            if self.measurement_head_available {
                self.measurement_events_kernel_digested
            } else {
                0
            },
        );
        c.u8(
            "last_attestation_available",
            self.last_attestation_available as u8,
        );
        c.note(
            "last_attestation_zeroed_when_unavailable",
            "fixed width: 32 bytes written whether or not \
             last_attestation_available is 1; zero when unavailable, not \
             omitted and not the caller's raw value",
        );
        c.bytes(
            "last_attestation",
            if self.last_attestation_available {
                &self.last_attestation.0
            } else {
                &[0u8; 32]
            },
        );
        c.u8("audit_available", self.audit_available as u8);
        c.u8("audit_event_count", self.audit_event_count);
        c.each(
            "audit_events",
            self.audit_event_count as usize,
            Some(MAX_AUDIT_EVENTS),
            &mut |c, i| {
                let ev = &self.audit_events[i];
                c.u32("seq", ev.seq);
                c.u16("kind_tag", ev.kind_tag);
                c.u16("code", ev.code);
                c.u64("at_tick", ev.at_tick);
            },
        );
        c.u8("semantic_intent_count", self.semantic_intent_count);
        c.each(
            "semantic_intents",
            self.semantic_intent_count as usize,
            Some(MAX_SEMANTIC_INTENTS),
            &mut |c, i| {
                let it = &self.semantic_intents[i];
                c.u32("seq", it.seq);
                c.u16("intent_tag", it.intent_tag);
                c.u16("code", it.code);
                c.u64("at_tick", it.at_tick);
            },
        );
    }
}
