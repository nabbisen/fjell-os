//! `BundleBuilder` — collects records, enforces the allow-list, computes the
//! canonical SHA-256 bundle digest (RFC v0.4-005 §6.4).

use fjell_canon::BufSink;
use fjell_measure_format::Digest32;
use fjell_trust_provider::ids::TrustProviderId;

use crate::bundle::{
    DiagAuditEvent, DiagIntent, DiagnosticBundle, MAX_AUDIT_EVENTS, MAX_SEMANTIC_INTENTS,
};
use crate::events::is_audit_event_allowed;
use crate::intents::is_intent_allowed;

/// Errors from `BundleBuilder` operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BuilderError {
    /// The event kind_tag is not on the allow-list (silently dropped).
    NotAllowed = 0x01,
    /// The audit events buffer is full (MAX_AUDIT_EVENTS reached).
    AuditFull = 0x02,
    /// The semantic intents buffer is full (MAX_SEMANTIC_INTENTS reached).
    IntentFull = 0x03,
}

/// Incrementally builds a `DiagnosticBundle`.
///
/// Call `add_audit_event` / `add_intent` to accumulate records, then
/// `finalise` to compute the digest and obtain the sealed bundle.
pub struct BundleBuilder {
    bundle: DiagnosticBundle,
}

impl BundleBuilder {
    /// Construct a new builder seeded with metadata.
    pub fn new(
        bundle_id: [u8; 8],
        created_tick: u64,
        provider_id: TrustProviderId,
        keyring_anchor_epoch: u32,
        measurement_head: Digest32,
        last_attestation: Digest32,
    ) -> Self {
        let mut b = DiagnosticBundle::zeroed();
        b.bundle_id = bundle_id;
        b.created_tick = created_tick;
        b.provider_id = provider_id;
        b.keyring_anchor_epoch = keyring_anchor_epoch;
        b.measurement_head = measurement_head;
        b.last_attestation = last_attestation;
        Self { bundle: b }
    }

    /// Add a raw audit event, enforcing the allow-list.
    ///
    /// Returns `Err(NotAllowed)` if the `kind_tag` is not on the allow-list;
    /// returns `Err(AuditFull)` if the buffer is full.
    /// The event is silently dropped on `NotAllowed` — callers should ignore
    /// this variant in production.
    pub fn add_audit_event(
        &mut self,
        seq: u32,
        kind_tag: u16,
        code: u16,
        at_tick: u64,
    ) -> Result<(), BuilderError> {
        if !is_audit_event_allowed(kind_tag) {
            return Err(BuilderError::NotAllowed);
        }
        let idx = self.bundle.audit_event_count as usize;
        if idx >= MAX_AUDIT_EVENTS {
            return Err(BuilderError::AuditFull);
        }
        self.bundle.audit_events[idx] = DiagAuditEvent {
            seq,
            kind_tag,
            code,
            at_tick,
        };
        self.bundle.audit_event_count += 1;
        Ok(())
    }

    /// Add a raw semantic intent, enforcing the allow-list.
    pub fn add_intent(
        &mut self,
        seq: u32,
        intent_tag: u16,
        code: u16,
        at_tick: u64,
    ) -> Result<(), BuilderError> {
        if !is_intent_allowed(intent_tag) {
            return Err(BuilderError::NotAllowed);
        }
        let idx = self.bundle.semantic_intent_count as usize;
        if idx >= MAX_SEMANTIC_INTENTS {
            return Err(BuilderError::IntentFull);
        }
        self.bundle.semantic_intents[idx] = DiagIntent {
            seq,
            intent_tag,
            code,
            at_tick,
        };
        self.bundle.semantic_intent_count += 1;
        Ok(())
    }

    /// Finalise the bundle: compute the canonical SHA-256 digest and return
    /// the sealed `DiagnosticBundle`.
    ///
    /// The digest covers (§6.4):
    /// ```text
    /// SHA256("FJELL-DIAG-V1" || schema u16 LE || bundle_id 8 B ||
    ///        created_tick u64 LE || measurement_head 32 B ||
    ///        last_attestation 32 B || audit_event_count u8 ||
    ///        [per-event: seq u32 LE || kind_tag u16 LE || code u16 LE || tick u64 LE] ||
    ///        semantic_intent_count u8 ||
    ///        [per-intent: seq u32 LE || intent_tag u16 LE || code u16 LE || tick u64 LE])
    /// ```
    pub fn finalise(mut self) -> DiagnosticBundle {
        self.bundle.bundle_digest = self.compute_digest();
        self.bundle
    }

    fn compute_digest(&self) -> Digest32 {
        // Maximum size: 13 (prefix) + 2 + 8 + 8 + 32 + 32 + 1 + 64*16 + 1 + 32*16 = 1605 bytes.
        let mut sink = BufSink::<1700>::new();
        self.bundle.write_canonical(&mut sink);
        Digest32::of(sink.bytes())
    }

    /// Number of audit events accumulated so far.
    pub fn audit_count(&self) -> u8 {
        self.bundle.audit_event_count
    }
    /// Number of semantic intents accumulated so far.
    pub fn intent_count(&self) -> u8 {
        self.bundle.semantic_intent_count
    }
}
