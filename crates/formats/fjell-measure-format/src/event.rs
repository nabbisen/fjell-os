//! Core measurement event and chain head types.

use crate::{digest::Digest32, CHAIN_DOMAIN, SCHEMA_VERSION};

// ── Enumerations ──────────────────────────────────────────────────────────────

/// What category of measurement event occurred.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementKind {
    BootEvidenceImported      = 0x01,
    ReleaseManifestVerified   = 0x02,
    RootfsManifestVerified    = 0x03,
    PolicyBundleVerified      = 0x04,
    PolicyLoaded              = 0x05,
    ServiceGraphReady         = 0x06,
    SnapshotCreated           = 0x07,
    HealthTargetResult        = 0x08,
    BundleFreshnessChecked    = 0x09,
    RecoveryTargetEntered     = 0x0A,
    ManualRollbackRequested   = 0x0B,
    AttestationGenerated      = 0x0C,
    ProvenanceSidecarChecked  = 0x0D,
    VerificationFailed        = 0x0E,
    FreshnessRejected         = 0x0F,
    /// PlatformProfile loaded and digest verified (RFC v0.5-001 §7.3).
    PlatformProfileLoaded     = 0x10,
    /// BoardProfile loaded, digest verified, and platform_ref matched (RFC v0.5-001 §7.3).
    BoardProfileLoaded        = 0x11,
}

impl MeasurementKind {
    pub fn as_u8(self) -> u8 { self as u8 }
}

/// Which service produced the measurement event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementSource {
    Kernel         = 0x01,
    Verifyd        = 0x02,
    Rootfsd        = 0x03,
    Configd        = 0x04,
    CapBroker      = 0x05,
    ServiceManager = 0x06,
    Snapshotd      = 0x07,
    Bootctl        = 0x08,
    Upgraded       = 0x09,
    Recoveryd      = 0x0A,
    Attestd        = 0x0B,
    Measuredd      = 0x0C,
}

impl MeasurementSource {
    pub fn as_u8(self) -> u8 { self as u8 }
}

/// What was measured (the subject of the event).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementSubject {
    BootEvidence       = 0x01,
    ReleaseManifest    = 0x02,
    RootfsManifest     = 0x03,
    PolicyBundle       = 0x04,
    LoadedPolicy       = 0x05,
    ServiceGraph       = 0x06,
    SystemSnapshot     = 0x07,
    HealthResult       = 0x08,
    BundleMetadata     = 0x09,
    RecoveryAction     = 0x0A,
    AttestationRecord  = 0x0B,
    ProvenanceSidecar  = 0x0C,
}

impl MeasurementSubject {
    pub fn as_u8(self) -> u8 { self as u8 }
}

/// Errors that measurement operations can return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementError {
    InvalidSequence   = 0x01,
    InvalidSource     = 0x02,
    InvalidDigest     = 0x03,
    StoreUnavailable  = 0x04,
    ExportFailed      = 0x05,
    PermissionDenied  = 0x06,
    Internal          = 0x07,
    NotFound          = 0x08,
}

// ── MeasurementEvent ──────────────────────────────────────────────────────────

/// One event in the append-only measurement chain.
///
/// The `chain_digest` field is computed deterministically from all fields plus
/// the previous event's chain_digest, making the full history tamper-evident.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementEvent {
    /// Monotonically increasing sequence number (starts at 1).
    pub seq: u64,
    /// Kernel tick at event creation.
    pub tick: u64,
    /// Category of this measurement.
    pub kind: MeasurementKind,
    /// Service that produced the event.
    pub source: MeasurementSource,
    /// What was measured.
    pub subject: MeasurementSubject,
    /// SHA-256 digest of the measured subject.
    pub subject_digest: Digest32,
    /// Optional additional metadata digest (e.g., release_id for manifests).
    pub metadata_digest: Option<Digest32>,
    /// Chain digest of the immediately preceding event (Digest32::ZERO for seq=1).
    pub previous_chain_digest: Digest32,
    /// Chain digest of this event (computed by `compute_chain_digest`).
    pub chain_digest: Digest32,
}

impl MeasurementEvent {
    /// Compute the chain digest for a new event.
    ///
    /// This is the authoritative digest formula; the result is stored in
    /// `chain_digest` and becomes the `previous_chain_digest` of the next event.
    pub fn compute_chain_digest(
        seq: u64,
        kind: MeasurementKind,
        source: MeasurementSource,
        subject: MeasurementSubject,
        subject_digest: &Digest32,
        metadata_digest: Option<&Digest32>,
        previous_chain_digest: &Digest32,
    ) -> Digest32 {
        let schema_bytes = SCHEMA_VERSION.to_le_bytes();
        let seq_bytes    = seq.to_le_bytes();
        let meta_present = [metadata_digest.is_some() as u8];
        let meta_bytes   = metadata_digest
            .map(|d| d.0)
            .unwrap_or([0u8; 32]);

        Digest32::of_parts(&[
            CHAIN_DOMAIN,
            &schema_bytes,
            &seq_bytes,
            &[kind.as_u8()],
            &[source.as_u8()],
            &[subject.as_u8()],
            &subject_digest.0,
            &meta_present,
            &meta_bytes,
            &previous_chain_digest.0,
        ])
    }

    /// Construct a new event, computing `chain_digest` automatically.
    pub fn new(
        seq: u64,
        tick: u64,
        kind: MeasurementKind,
        source: MeasurementSource,
        subject: MeasurementSubject,
        subject_digest: Digest32,
        metadata_digest: Option<Digest32>,
        previous_chain_digest: Digest32,
    ) -> Self {
        let chain_digest = Self::compute_chain_digest(
            seq, kind, source, subject,
            &subject_digest, metadata_digest.as_ref(),
            &previous_chain_digest,
        );
        Self {
            seq, tick, kind, source, subject,
            subject_digest, metadata_digest,
            previous_chain_digest, chain_digest,
        }
    }

    /// Verify that this event's chain_digest is correctly computed.
    pub fn verify_chain_digest(&self) -> bool {
        let expected = Self::compute_chain_digest(
            self.seq, self.kind, self.source, self.subject,
            &self.subject_digest, self.metadata_digest.as_ref(),
            &self.previous_chain_digest,
        );
        expected == self.chain_digest
    }
}

// ── MeasurementHead ───────────────────────────────────────────────────────────

/// Summary of the current state of the measurement chain.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementHead {
    /// Sequence number of the latest event.
    pub latest_seq: u64,
    /// Chain digest at the latest event.
    pub chain_digest: Digest32,
    /// Number of events dropped due to storage pressure (should be 0 in M8).
    pub dropped: u64,
    /// Kind of the latest event.
    pub last_event_kind: MeasurementKind,
}

impl MeasurementHead {
    /// Initial (empty) head before any events.
    pub const EMPTY: Self = Self {
        latest_seq: 0,
        chain_digest: Digest32::ZERO,
        dropped: 0,
        last_event_kind: MeasurementKind::BootEvidenceImported,
    };
}

// ── IPC request / response ────────────────────────────────────────────────────

/// Request to the measuredd service.
#[derive(Clone, Copy, Debug)]
pub enum MeasurementRequest {
    AppendEvent {
        kind:            MeasurementKind,
        source:          MeasurementSource,
        subject:         MeasurementSubject,
        subject_digest:  Digest32,
        metadata_digest: Option<Digest32>,
    },
    GetHead,
    GetEvent { seq: u64 },
    ExportLog { from_seq: u64, max: u16 },
}

/// Response from the measuredd service.
#[derive(Clone, Copy, Debug)]
pub enum MeasurementResponse {
    Appended { seq: u64, chain_digest: Digest32 },
    Head     { head: MeasurementHead },
    Event    { event: MeasurementEvent },
    ExportStart { total_events: u64 },
    ExportDone,
    Error    { error: MeasurementError },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_digest(b: u8) -> Digest32 { Digest32([b; 32]) }

    #[test]
    fn chain_digest_deterministic() {
        let d1 = MeasurementEvent::compute_chain_digest(
            1,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            &dummy_digest(0x01),
            None,
            &Digest32::ZERO,
        );
        let d2 = MeasurementEvent::compute_chain_digest(
            1,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            &dummy_digest(0x01),
            None,
            &Digest32::ZERO,
        );
        assert_eq!(d1, d2, "chain digest must be deterministic");
    }

    #[test]
    fn chain_digest_depends_on_previous() {
        let d_with_zero_prev = MeasurementEvent::compute_chain_digest(
            2, MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd, MeasurementSubject::ReleaseManifest,
            &dummy_digest(0xAB), None, &Digest32::ZERO,
        );
        let d_with_other_prev = MeasurementEvent::compute_chain_digest(
            2, MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd, MeasurementSubject::ReleaseManifest,
            &dummy_digest(0xAB), None, &Digest32([0xFF; 32]),
        );
        assert_ne!(d_with_zero_prev, d_with_other_prev,
            "chain digest must depend on previous_chain_digest");
    }

    #[test]
    fn chain_digest_depends_on_seq() {
        let d1 = MeasurementEvent::compute_chain_digest(
            1, MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel, MeasurementSubject::BootEvidence,
            &dummy_digest(0x01), None, &Digest32::ZERO,
        );
        let d2 = MeasurementEvent::compute_chain_digest(
            2, MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel, MeasurementSubject::BootEvidence,
            &dummy_digest(0x01), None, &Digest32::ZERO,
        );
        assert_ne!(d1, d2, "chain digest must depend on seq");
    }

    #[test]
    fn event_new_and_verify() {
        let ev = MeasurementEvent::new(
            1, 12345,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            dummy_digest(0x42),
            None,
            Digest32::ZERO,
        );
        assert_eq!(ev.seq, 1);
        assert!(ev.verify_chain_digest(), "self-verification must pass");
    }

    #[test]
    fn event_chain_links_correctly() {
        let ev1 = MeasurementEvent::new(
            1, 100,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            dummy_digest(0x01), None, Digest32::ZERO,
        );
        let ev2 = MeasurementEvent::new(
            2, 200,
            MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::ReleaseManifest,
            dummy_digest(0x02), None, ev1.chain_digest,
        );
        assert_eq!(ev2.previous_chain_digest, ev1.chain_digest);
        assert!(ev2.verify_chain_digest());
    }

    #[test]
    fn metadata_digest_changes_chain() {
        let without = MeasurementEvent::compute_chain_digest(
            1, MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd, MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10), None, &Digest32::ZERO,
        );
        let with_meta = MeasurementEvent::compute_chain_digest(
            1, MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd, MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10), Some(&dummy_digest(0x20)), &Digest32::ZERO,
        );
        assert_ne!(without, with_meta, "metadata digest must affect chain");
    }
}
