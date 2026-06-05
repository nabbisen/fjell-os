//! Local attestation record format for Fjell OS M8.
//!
//! Defines the development-grade local attestation record (`AttestationRecord`)
//! and its signed wrapper (`SignedAttestationRecord`).
//!
//! This is **not** hardware-rooted attestation.  M8 attestation is local and
//! development-grade; the signature uses a development Ed25519 key embedded at
//! build time.  Remote verifiers and hardware roots of trust are v1+ scope.
//!
//! # Canonical digest
//!
//! The record digest covers all fields of `AttestationRecord` in a fixed binary
//! layout.  The signed form commits to this digest; export projections
//! (JSON/TOML/PlainText) are unsigned and advisory.
#![no_std]

pub mod v2;
pub use v2::{
    AttestationRecordV2, SignedAttestationRecordV2, SignedByDescriptor,
    ProviderClaims, KeyringClaims, RollbackClaims, FreshnessClaimsV2,
    NonceClass, ATTEST_V2_DOMAIN,
};

use fjell_measure_format::Digest32;

// ── ID types ─────────────────────────────────────────────────────────────────

/// Unique identifier for an attestation record (8-byte ASCII).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttestationRecordId(pub [u8; 8]);

impl AttestationRecordId {
    pub fn new(seq: u32) -> Self {
        let mut id = *b"AT000000";
        let s = seq.to_string_radix(10);
        let bytes = s.as_bytes();
        let n = bytes.len().min(6);
        id[2..2+n].copy_from_slice(&bytes[..n]);
        Self(id)
    }
}

// Minimal no_std u32→decimal helper.
trait ToStringRadix { fn to_string_radix(&self, _: u32) -> U32Str; }
struct U32Str { buf: [u8; 10], len: usize }
impl U32Str { fn as_bytes(&self) -> &[u8] { &self.buf[..self.len] } }
impl ToStringRadix for u32 {
    fn to_string_radix(&self, _: u32) -> U32Str {
        let mut buf = [0u8; 10];
        let mut n = *self;
        let mut len = 0;
        if n == 0 { buf[0] = b'0'; return U32Str { buf, len: 1 }; }
        while n > 0 { buf[9 - len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
        let mut out = [0u8; 10];
        out[..len].copy_from_slice(&buf[10-len..]);
        U32Str { buf: out, len }
    }
}

/// 32-byte nonce for freshness / replay protection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Nonce(pub [u8; 32]);

// ── Profile ───────────────────────────────────────────────────────────────────

/// Attestation profile — normative binary or export projection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AttestationProfile {
    /// Normative signed binary form (Fjell canonical).
    FjellLocalV1Binary   = 0x01,
    /// JSON projection (unsigned, advisory).
    FjellLocalV1Json     = 0x02,
    /// TOML projection (unsigned, advisory).
    FjellLocalV1Toml     = 0x03,
    /// Plain-text projection (unsigned, advisory).
    FjellLocalV1PlainText = 0x04,
    /// NEW v0.3.0: normative signed binary form with trust-provider,
    /// keyring epoch, and rollback binding (RFC v0.3-004).
    FjellLocalV2Binary    = 0x21,
    /// NEW v0.3.0: JSON projection of v2 (unsigned, advisory).
    FjellLocalV2Json      = 0x22,
}

// ── Claims structs ────────────────────────────────────────────────────────────

/// Boot-related claims extracted from BootEvidence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BootClaims {
    /// Which slot was booted.
    pub selected_slot: u8,      // 0=A, 1=B
    /// Boot attempt number (monotonic).
    pub boot_id: u64,
    /// Digest of the kernel image.
    pub kernel_digest: Digest32,
}

/// Verification results from verifyd.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VerificationClaims {
    /// Digest of the loaded release manifest.
    pub release_digest: Digest32,
    /// Digest of the loaded rootfs manifest.
    pub rootfs_digest: Digest32,
    /// Digest of the loaded policy bundle.
    pub policy_digest: Digest32,
    pub release_verified: bool,
    pub rootfs_verified:  bool,
    pub policy_verified:  bool,
}

/// Measurement chain summary at attestation time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MeasurementClaims {
    /// Sequence number of the latest measurement event.
    pub head_seq: u64,
    /// Chain digest at `head_seq`.
    pub chain_digest: Digest32,
    /// Sequence range included in this attestation.
    pub included_from_seq: u64,
    pub included_to_seq:   u64,
}

/// Snapshot claims.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SnapshotClaims {
    /// 8-byte ASCII snapshot identifier.
    pub snapshot_id: [u8; 8],
    /// Digest of the snapshot.
    pub snapshot_digest: Digest32,
    /// Reason code (0=boot, 1=pre-upgrade, 2=pre-confirmation, 3=manual).
    pub reason: u8,
}

/// Health target result claims.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HealthClaims {
    /// 8-byte ASCII health target name.
    pub target: [u8; 8],
    /// 0=passed, 1=failed, 2=timeout, 3=not-run.
    pub status: u8,
}

/// Bundle freshness check result.
#[derive(Clone, Copy, Debug)]
pub struct FreshnessClaims {
    pub generation: u64,
    pub key_epoch: u64,
    /// 0=valid, 1=expired, 2=not-yet-valid, 3=generation-rollback,
    /// 4=key-epoch-rollback, 5=unavailable.
    pub status: u8,
}

/// Optional advisory provenance claims (sidecar if present).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProvenanceClaims {
    pub sidecar_digest: Digest32,
    /// 0=verified, 1=advisory, 2=rejected, 3=absent.
    pub result: u8,
}

// ── AttestationRecord ─────────────────────────────────────────────────────────

/// A local development-grade attestation record.
///
/// All mandatory claims must be present.  Provenance is optional.
#[derive(Clone, Copy, Debug)]
pub struct AttestationRecord {
    pub schema_version: u16,
    pub record_id:      AttestationRecordId,
    pub created_tick:   u64,
    pub profile:        AttestationProfile,
    pub nonce:          Option<Nonce>,
    pub boot:           BootClaims,
    pub verification:   VerificationClaims,
    pub measurement:    MeasurementClaims,
    pub snapshot:       SnapshotClaims,
    pub health:         HealthClaims,
    pub freshness:      FreshnessClaims,
    pub provenance:     Option<ProvenanceClaims>,
}

impl AttestationRecord {
    /// Current schema version for M8.
    pub const SCHEMA_VERSION: u16 = 1;

    /// Compute the canonical digest of this record.
    ///
    /// The digest covers all mandatory fields in a deterministic binary layout.
    /// Optional fields (nonce, provenance) are included with a presence byte.
    pub fn canonical_digest(&self) -> Digest32 {
        use fjell_measure_format::digest::Digest32 as D;
        let sv  = self.schema_version.to_le_bytes();
        let tick = self.created_tick.to_le_bytes();
        let nonce_present = [self.nonce.is_some() as u8];
        let nonce_bytes = self.nonce.map(|n| n.0).unwrap_or([0u8; 32]);
        let slot  = [self.boot.selected_slot];
        let bid   = self.boot.boot_id.to_le_bytes();
        let rr    = [self.verification.release_verified as u8,
                     self.verification.rootfs_verified  as u8,
                     self.verification.policy_verified  as u8];
        let hseq  = self.measurement.head_seq.to_le_bytes();
        let ifrom = self.measurement.included_from_seq.to_le_bytes();
        let ito   = self.measurement.included_to_seq.to_le_bytes();
        let snap_reason = [self.snapshot.reason];
        let health_st   = [self.health.status];
        let fgen  = self.freshness.generation.to_le_bytes();
        let epoch = self.freshness.key_epoch.to_le_bytes();
        let fresh_st = [self.freshness.status];
        let prov_present = [self.provenance.is_some() as u8];
        let prov_bytes = self.provenance.map(|p| p.sidecar_digest.0).unwrap_or([0u8; 32]);
        let prov_res   = [self.provenance.map(|p| p.result).unwrap_or(3)];

        D::of_parts(&[
            b"FJELL-ATTEST-V1",
            &sv,
            &self.record_id.0,
            &[self.profile as u8],
            &tick,
            &nonce_present,
            &nonce_bytes,
            &slot,
            &bid,
            &self.boot.kernel_digest.0,
            &self.verification.release_digest.0,
            &self.verification.rootfs_digest.0,
            &self.verification.policy_digest.0,
            &rr,
            &hseq,
            &self.measurement.chain_digest.0,
            &ifrom,
            &ito,
            &self.snapshot.snapshot_id,
            &self.snapshot.snapshot_digest.0,
            &snap_reason,
            &self.health.target,
            &health_st,
            &fgen,
            &epoch,
            &fresh_st,
            &prov_present,
            &prov_bytes,
            &prov_res,
        ])
    }
}

// ── Development-grade signature ───────────────────────────────────────────────

/// Development-grade Ed25519 placeholder signature (32 bytes).
///
/// In M8 this is a keyed HMAC-style development stand-in; real Ed25519
/// signing is a v1+ requirement when hardware roots become available.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DevAttestation(pub [u8; 32]);

impl DevAttestation {
    /// Development key identifier embedded in the signature.
    pub const KEY_ID: &'static [u8] = b"dev-attest-m8-01";

    /// Sign a record digest with the development key.
    /// The "signature" is SHA-256(KEY_ID || record_digest).
    pub fn sign(record_digest: &Digest32) -> Self {
        let sig = Digest32::of_parts(&[Self::KEY_ID, &record_digest.0]);
        Self(sig.0)
    }

    /// Verify that this signature matches the record digest.
    pub fn verify(&self, record_digest: &Digest32) -> bool {
        *self == Self::sign(record_digest)
    }
}

// ── SignedAttestationRecord ───────────────────────────────────────────────────

/// A signed attestation record (normative form).
#[derive(Clone, Copy, Debug)]
pub struct SignedAttestationRecord {
    pub record:        AttestationRecord,
    pub record_digest: Digest32,
    pub signature:     DevAttestation,
}

impl SignedAttestationRecord {
    /// Create a new signed record.
    pub fn sign(record: AttestationRecord) -> Self {
        let record_digest = record.canonical_digest();
        let signature = DevAttestation::sign(&record_digest);
        Self { record, record_digest, signature }
    }

    /// Verify the signature and recomputed digest.
    pub fn verify(&self) -> bool {
        let expected = self.record.canonical_digest();
        expected == self.record_digest && self.signature.verify(&self.record_digest)
    }
}

// ── IPC request / response ────────────────────────────────────────────────────

/// Errors from attestd.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AttestationError {
    MeasurementUnavailable  = 0x01,
    SnapshotUnavailable     = 0x02,
    VerificationUnavailable = 0x03,
    SigningFailed           = 0x04,
    UnsupportedProfile      = 0x05,
    InvalidRecord           = 0x06,
    InvalidSignature        = 0x07,
    PermissionDenied        = 0x08,
    Internal                = 0x09,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_record() -> AttestationRecord {
        AttestationRecord {
            schema_version: AttestationRecord::SCHEMA_VERSION,
            record_id:     AttestationRecordId(*b"AT000001"),
            created_tick:  99999,
            profile:       AttestationProfile::FjellLocalV1Binary,
            nonce:         None,
            boot: BootClaims {
                selected_slot: 0,
                boot_id: 1,
                kernel_digest: Digest32([0x11; 32]),
            },
            verification: VerificationClaims {
                release_digest: Digest32([0x22; 32]),
                rootfs_digest:  Digest32([0x33; 32]),
                policy_digest:  Digest32([0x44; 32]),
                release_verified: true,
                rootfs_verified:  true,
                policy_verified:  true,
            },
            measurement: MeasurementClaims {
                head_seq: 10,
                chain_digest: Digest32([0x55; 32]),
                included_from_seq: 1,
                included_to_seq:   10,
            },
            snapshot: SnapshotClaims {
                snapshot_id: *b"SN000001",
                snapshot_digest: Digest32([0x66; 32]),
                reason: 0,
            },
            health: HealthClaims {
                target: *b"m7-hlth\0",
                status: 0,
            },
            freshness: FreshnessClaims {
                generation: 1,
                key_epoch: 1,
                status: 0,
            },
            provenance: None,
        }
    }

    #[test]
    fn canonical_digest_stable() {
        let r = test_record();
        let d1 = r.canonical_digest();
        let d2 = r.canonical_digest();
        assert_eq!(d1, d2, "canonical digest must be deterministic");
        assert_ne!(d1, Digest32::ZERO);
    }

    #[test]
    fn nonce_changes_digest() {
        let mut r1 = test_record();
        let mut r2 = test_record();
        r1.nonce = None;
        r2.nonce = Some(Nonce([0xAB; 32]));
        assert_ne!(r1.canonical_digest(), r2.canonical_digest());
    }

    #[test]
    fn signed_record_verify() {
        let r = test_record();
        let signed = SignedAttestationRecord::sign(r);
        assert!(signed.verify(), "freshly signed record must verify");
    }

    #[test]
    fn tampered_record_fails_verify() {
        let r = test_record();
        let mut signed = SignedAttestationRecord::sign(r);
        // Tamper with the record
        signed.record.boot.boot_id = 999;
        assert!(!signed.verify(), "tampered record must fail verification");
    }

    #[test]
    fn provenance_present_changes_digest() {
        let mut r = test_record();
        r.provenance = Some(ProvenanceClaims {
            sidecar_digest: Digest32([0xBB; 32]),
            result: 0,
        });
        let with_prov = r.canonical_digest();
        r.provenance = None;
        let without_prov = r.canonical_digest();
        assert_ne!(with_prov, without_prov);
    }
}

#[cfg(test)]
mod tests_v2;
