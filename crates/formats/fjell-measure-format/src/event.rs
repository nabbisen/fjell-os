//! Core measurement event and chain head types.

use crate::{CHAIN_DOMAIN, SCHEMA_VERSION, digest::Digest32};

// ── Enumerations ──────────────────────────────────────────────────────────────

/// What category of measurement event occurred.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementKind {
    BootEvidenceImported = 0x01,
    ReleaseManifestVerified = 0x02,
    RootfsManifestVerified = 0x03,
    PolicyBundleVerified = 0x04,
    PolicyLoaded = 0x05,
    ServiceGraphReady = 0x06,
    SnapshotCreated = 0x07,
    HealthTargetResult = 0x08,
    BundleFreshnessChecked = 0x09,
    RecoveryTargetEntered = 0x0A,
    ManualRollbackRequested = 0x0B,
    AttestationGenerated = 0x0C,
    ProvenanceSidecarChecked = 0x0D,
    VerificationFailed = 0x0E,
    FreshnessRejected = 0x0F,
    /// PlatformProfile loaded and digest verified (RFC v0.5-001 §7.3).
    PlatformProfileLoaded = 0x10,
    /// BoardProfile loaded, digest verified, and platform_ref matched (RFC v0.5-001 §7.3).
    BoardProfileLoaded = 0x11,
}

impl MeasurementKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// The kind with wire value `b`, or `None` for a byte that names no kind.
    /// There is no default: an unrecognised byte must not be recorded as some
    /// other, plausible claim (RFC-0.39-001 R2/E-096's neighbour).
    pub fn from_u8(b: u8) -> Option<Self> {
        Some(match b {
            0x01 => Self::BootEvidenceImported,
            0x02 => Self::ReleaseManifestVerified,
            0x03 => Self::RootfsManifestVerified,
            0x04 => Self::PolicyBundleVerified,
            0x05 => Self::PolicyLoaded,
            0x06 => Self::ServiceGraphReady,
            0x07 => Self::SnapshotCreated,
            0x08 => Self::HealthTargetResult,
            0x09 => Self::BundleFreshnessChecked,
            0x0A => Self::RecoveryTargetEntered,
            0x0B => Self::ManualRollbackRequested,
            0x0C => Self::AttestationGenerated,
            0x0D => Self::ProvenanceSidecarChecked,
            0x0E => Self::VerificationFailed,
            0x0F => Self::FreshnessRejected,
            0x10 => Self::PlatformProfileLoaded,
            0x11 => Self::BoardProfileLoaded,
            _ => return None,
        })
    }
}

/// Who produced the **measured fact** (RFC-0.39-001, E-096).
///
/// This is *not* a field the appender declares. `measuredd` determines it from
/// the operation plus the kernel-attested identity of the sender, and a caller
/// that tries to supply one is refused. Two consequences worth stating, because
/// the two append paths look different and mean the same thing:
///
/// * The boot-time image-set digest is [`MeasurementSource::Kernel`]: the
///   *kernel* computed it; `init` only carried it to the chain.
/// * An event whose subject the appender itself invented is the appender: the
///   three placeholder verification results are [`MeasurementSource::Init`],
///   because `init` produced the literal they record.
///
/// "Who produced the fact" and "who sent the message" are therefore different
/// questions that coincide for `APPEND_EVENT` and differ for the image set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementSource {
    Kernel = 0x01,
    Verifyd = 0x02,
    Rootfsd = 0x03,
    Configd = 0x04,
    CapBroker = 0x05,
    ServiceManager = 0x06,
    Snapshotd = 0x07,
    Bootctl = 0x08,
    Upgraded = 0x09,
    Recoveryd = 0x0A,
    Attestd = 0x0B,
    Measuredd = 0x0C,
    /// `fjell-init` (RFC-0.39-001). Added because `init` is the only task that
    /// appends events by message, and there was no value that said so.
    Init = 0x0D,
}

impl MeasurementSource {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// What was measured (the subject of the event).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementSubject {
    BootEvidence = 0x01,
    ReleaseManifest = 0x02,
    RootfsManifest = 0x03,
    PolicyBundle = 0x04,
    LoadedPolicy = 0x05,
    ServiceGraph = 0x06,
    SystemSnapshot = 0x07,
    HealthResult = 0x08,
    BundleMetadata = 0x09,
    RecoveryAction = 0x0A,
    AttestationRecord = 0x0B,
    ProvenanceSidecar = 0x0C,
}

impl MeasurementSubject {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// The subject with wire value `b`, or `None` for a byte that names none.
    /// No default, as for [`MeasurementKind::from_u8`].
    pub fn from_u8(b: u8) -> Option<Self> {
        Some(match b {
            0x01 => Self::BootEvidence,
            0x02 => Self::ReleaseManifest,
            0x03 => Self::RootfsManifest,
            0x04 => Self::PolicyBundle,
            0x05 => Self::LoadedPolicy,
            0x06 => Self::ServiceGraph,
            0x07 => Self::SystemSnapshot,
            0x08 => Self::HealthResult,
            0x09 => Self::BundleMetadata,
            0x0A => Self::RecoveryAction,
            0x0B => Self::AttestationRecord,
            0x0C => Self::ProvenanceSidecar,
            _ => return None,
        })
    }
}

/// Errors that measurement operations can return.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MeasurementError {
    InvalidSequence = 0x01,
    InvalidSource = 0x02,
    InvalidDigest = 0x03,
    StoreUnavailable = 0x04,
    ExportFailed = 0x05,
    PermissionDenied = 0x06,
    Internal = 0x07,
    NotFound = 0x08,
    /// The kind byte of an append names no `MeasurementKind` (RFC-0.39-001).
    InvalidKind = 0x09,
    /// The subject byte of an append names no `MeasurementSubject`
    /// (RFC-0.39-001).
    InvalidSubject = 0x0A,
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

/// `compute_chain_digest`'s byte stream, written through `Canon`
/// (RFC-0.35-004 R3) so a second implementation of the measurement chain
/// has a generated description to match, not just a hash to agree with by
/// luck.
///
/// The metadata slot is a presence byte plus a **fixed** 32 bytes, written
/// either way — deliberately not `Canon::optional`, whose body is written
/// only when present. This stream's width never depends on whether
/// metadata is some or none; an `optional` here would describe a format
/// this one is not.
pub fn write_chain_canonical(
    c: &mut dyn fjell_canon::Canon,
    seq: u64,
    kind: MeasurementKind,
    source: MeasurementSource,
    subject: MeasurementSubject,
    subject_digest: &Digest32,
    metadata_digest: Option<&Digest32>,
    previous_chain_digest: &Digest32,
) {
    c.domain(CHAIN_DOMAIN);
    c.u16("schema_version", SCHEMA_VERSION);
    c.u64("seq", seq);
    c.u8("kind", kind.as_u8());
    c.u8("source", source.as_u8());
    c.u8("subject", subject.as_u8());
    c.bytes("subject_digest", &subject_digest.0);
    c.u8("metadata_present", metadata_digest.is_some() as u8);
    // RFC-0.35-004 review, 2026-10-15: the description must be sufficient to
    // reproduce the digest, and without this it is not. `metadata_digest` is
    // thirty-two bytes **whatever `metadata_present` says** — zero-filled when
    // absent, never omitted and never arbitrary. A second implementation
    // reading only `field metadata_digest u8 [32]` could write 0xFF padding
    // and match the text while producing a different digest, or omit the field
    // and produce a different length. Both are refused here in words, because
    // nothing reads these bytes back to refuse them in code. `note` writes no
    // bytes (`fjell-canon`'s `notes_are_not_written`), so this cannot move the
    // digest.
    c.note(
        "metadata_fill",
        "fixed width: 32 bytes whether or not metadata_present is 1; \
         zero-filled when absent, not omitted and not arbitrary",
    );
    c.bytes(
        "metadata_digest",
        &metadata_digest.map(|d| d.0).unwrap_or([0u8; 32]),
    );
    c.bytes("previous_chain_digest", &previous_chain_digest.0);
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
        // 20 (CHAIN_DOMAIN) + 2 + 8 + 1+1+1 + 32 + 1 + 32 + 32 = 130 bytes.
        let mut sink = fjell_canon::BufSink::<130>::new();
        write_chain_canonical(
            &mut sink,
            seq,
            kind,
            source,
            subject,
            subject_digest,
            metadata_digest,
            previous_chain_digest,
        );
        Digest32::of(sink.bytes())
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
            seq,
            kind,
            source,
            subject,
            &subject_digest,
            metadata_digest.as_ref(),
            &previous_chain_digest,
        );
        Self {
            seq,
            tick,
            kind,
            source,
            subject,
            subject_digest,
            metadata_digest,
            previous_chain_digest,
            chain_digest,
        }
    }

    /// Verify that this event's chain_digest is correctly computed.
    pub fn verify_chain_digest(&self) -> bool {
        let expected = Self::compute_chain_digest(
            self.seq,
            self.kind,
            self.source,
            self.subject,
            &self.subject_digest,
            self.metadata_digest.as_ref(),
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
        kind: MeasurementKind,
        source: MeasurementSource,
        subject: MeasurementSubject,
        subject_digest: Digest32,
        metadata_digest: Option<Digest32>,
    },
    GetHead,
    GetEvent {
        seq: u64,
    },
    ExportLog {
        from_seq: u64,
        max: u16,
    },
}

/// Response from the measuredd service.
#[derive(Clone, Copy, Debug)]
pub enum MeasurementResponse {
    Appended { seq: u64, chain_digest: Digest32 },
    Head { head: MeasurementHead },
    Event { event: MeasurementEvent },
    ExportStart { total_events: u64 },
    ExportDone,
    Error { error: MeasurementError },
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_digest(b: u8) -> Digest32 {
        Digest32([b; 32])
    }

    #[test]
    fn golden_digests_from_the_unmodified_encoder_rfc_0_35_004_r2() {
        let no_meta = MeasurementEvent::compute_chain_digest(
            7,
            MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10),
            None,
            &Digest32::ZERO,
        );
        let with_meta = MeasurementEvent::compute_chain_digest(
            7,
            MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10),
            Some(&dummy_digest(0x20)),
            &Digest32::ZERO,
        );
        assert_eq!(
            no_meta.0,
            [
                205, 126, 77, 241, 79, 67, 91, 0, 62, 222, 244, 206, 242, 125, 30, 36, 253, 99,
                214, 232, 214, 132, 184, 231, 229, 75, 163, 109, 120, 179, 253, 131
            ],
            "GOLDEN no_meta"
        );
        assert_eq!(
            with_meta.0,
            [
                65, 70, 1, 137, 236, 82, 231, 230, 169, 197, 154, 71, 174, 174, 90, 59, 74, 225,
                62, 35, 181, 155, 60, 230, 95, 29, 245, 101, 105, 139, 202, 118
            ],
            "GOLDEN with_meta"
        );
        // Raw concatenated byte stream for the no_meta case too -- the
        // stronger golden: not just "hashes the same" but "wrote the same
        // bytes". Reconstructed here exactly as compute_chain_digest's own
        // body does (`#![no_std]`, no `alloc`: a fixed array and a cursor),
        // to capture it once before the encoder moves.
        let mut raw = [0u8; 130];
        let mut i = 0;
        let put = |raw: &mut [u8; 130], i: &mut usize, b: &[u8]| {
            raw[*i..*i + b.len()].copy_from_slice(b);
            *i += b.len();
        };
        put(&mut raw, &mut i, CHAIN_DOMAIN);
        put(&mut raw, &mut i, &SCHEMA_VERSION.to_le_bytes());
        put(&mut raw, &mut i, &7u64.to_le_bytes());
        put(
            &mut raw,
            &mut i,
            &[MeasurementKind::PolicyBundleVerified.as_u8()],
        );
        put(&mut raw, &mut i, &[MeasurementSource::Verifyd.as_u8()]);
        put(
            &mut raw,
            &mut i,
            &[MeasurementSubject::PolicyBundle.as_u8()],
        );
        put(&mut raw, &mut i, &dummy_digest(0x10).0);
        put(&mut raw, &mut i, &[0u8]);
        put(&mut raw, &mut i, &[0u8; 32]);
        put(&mut raw, &mut i, &Digest32::ZERO.0);
        assert_eq!(i, 130, "GOLDEN raw byte length");
        assert_eq!(
            Digest32::of(&raw),
            no_meta,
            "GOLDEN raw bytes hash to the same digest"
        );
    }

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
            2,
            MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::ReleaseManifest,
            &dummy_digest(0xAB),
            None,
            &Digest32::ZERO,
        );
        let d_with_other_prev = MeasurementEvent::compute_chain_digest(
            2,
            MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::ReleaseManifest,
            &dummy_digest(0xAB),
            None,
            &Digest32([0xFF; 32]),
        );
        assert_ne!(
            d_with_zero_prev, d_with_other_prev,
            "chain digest must depend on previous_chain_digest"
        );
    }

    #[test]
    fn chain_digest_depends_on_seq() {
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
            2,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            &dummy_digest(0x01),
            None,
            &Digest32::ZERO,
        );
        assert_ne!(d1, d2, "chain digest must depend on seq");
    }

    #[test]
    fn event_new_and_verify() {
        let ev = MeasurementEvent::new(
            1,
            12345,
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
            1,
            100,
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            dummy_digest(0x01),
            None,
            Digest32::ZERO,
        );
        let ev2 = MeasurementEvent::new(
            2,
            200,
            MeasurementKind::ReleaseManifestVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::ReleaseManifest,
            dummy_digest(0x02),
            None,
            ev1.chain_digest,
        );
        assert_eq!(ev2.previous_chain_digest, ev1.chain_digest);
        assert!(ev2.verify_chain_digest());
    }

    #[test]
    fn metadata_digest_changes_chain() {
        let without = MeasurementEvent::compute_chain_digest(
            1,
            MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10),
            None,
            &Digest32::ZERO,
        );
        let with_meta = MeasurementEvent::compute_chain_digest(
            1,
            MeasurementKind::PolicyBundleVerified,
            MeasurementSource::Verifyd,
            MeasurementSubject::PolicyBundle,
            &dummy_digest(0x10),
            Some(&dummy_digest(0x20)),
            &Digest32::ZERO,
        );
        assert_ne!(without, with_meta, "metadata digest must affect chain");
    }
}
