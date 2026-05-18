//! Signature verification types for Fjell OS M7.
//!
//! Development-grade: uses a simple digest-based trust model.
//! The "signature" is a 32-byte HMAC-SHA256 stand-in (simulated for smoke tests).
#![no_std]

/// A 32-byte development-grade signature (Ed25519 placeholder).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DevSignature(pub [u8; 32]);

impl DevSignature {
    /// The canonical "valid" signature used by all M7 smoke-test bundles.
    pub const VALID: DevSignature = DevSignature([
        0x46, 0x4A, 0x45, 0x4C, 0x4C, 0x5F, 0x4F, 0x53, // "FJELL_OS"
        0x5F, 0x53, 0x49, 0x47, 0x4E, 0x45, 0x44, 0x5F, // "_SIGNED_"
        0x44, 0x45, 0x56, 0x5F, 0x4B, 0x45, 0x59, 0x5F, // "DEV_KEY_"
        0x4D, 0x37, 0x5F, 0x30, 0x30, 0x31, 0x00, 0x00, // "M7_001\0\0"
    ]);
}

/// Trust anchor — the embedded development public key fingerprint.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TrustAnchor(pub [u8; 8]);

impl TrustAnchor {
    pub const DEV: TrustAnchor = TrustAnchor(*b"FJELL_M7");
    pub fn is_valid(&self) -> bool { *self == Self::DEV }
}

/// A signed object reference (manifest, policy bundle, etc.).
#[derive(Clone, Copy)]
pub struct SignedObject {
    pub kind:      ObjectKind,
    pub id:        [u8; 16],
    pub digest:    [u8; 32],
    pub signature: DevSignature,
}

impl SignedObject {
    /// Returns true if the embedded signature matches the development key.
    pub fn verify_dev(&self) -> bool {
        self.signature == DevSignature::VALID
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    ReleaseManifest,
    RootfsManifest,
    PolicyBundle,
    KernelImage,
    RootfsImage,
}

/// Result of a verification operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationResult { Verified, Rejected, NotFound }

/// Boot evidence collected by the bootloader / kernel at startup.
#[derive(Clone, Copy)]
pub struct BootEvidence {
    pub slot:           u8,   // 0=A, 1=B
    pub boot_count:     u32,
    pub kernel_digest:  [u8; 32],
    pub anchor:         TrustAnchor,
}

impl BootEvidence {
    pub const fn for_slot(slot: u8) -> Self {
        BootEvidence {
            slot, boot_count: 1,
            kernel_digest: [0u8; 32],
            anchor: TrustAnchor::DEV,
        }
    }
}

/// Release manifest (development-grade).
#[derive(Clone, Copy)]
pub struct ReleaseManifest {
    pub release_id:  [u8; 16],
    pub version:     u32,
    pub obj:         SignedObject,
}

impl ReleaseManifest {
    pub fn valid_dev(release_id: [u8; 16]) -> Self {
        ReleaseManifest {
            release_id, version: 1,
            obj: SignedObject {
                kind: ObjectKind::ReleaseManifest, id: release_id,
                digest: [0xAB; 32], signature: DevSignature::VALID,
            },
        }
    }
    pub fn invalid_dev(release_id: [u8; 16]) -> Self {
        ReleaseManifest {
            release_id, version: 1,
            obj: SignedObject {
                kind: ObjectKind::ReleaseManifest, id: release_id,
                digest: [0xAB; 32], signature: DevSignature([0xFF; 32]),
            },
        }
    }
}

/// Rootfs manifest.
#[derive(Clone, Copy)]
pub struct RootfsManifest {
    pub rootfs_id: [u8; 16],
    pub obj:       SignedObject,
}

impl RootfsManifest {
    pub fn valid_dev(rootfs_id: [u8; 16]) -> Self {
        RootfsManifest {
            rootfs_id,
            obj: SignedObject {
                kind: ObjectKind::RootfsManifest, id: rootfs_id,
                digest: [0xCD; 32], signature: DevSignature::VALID,
            },
        }
    }
}

/// Policy bundle.
#[derive(Clone, Copy)]
pub struct PolicyBundle {
    pub policy_id: [u8; 16],
    pub version:   u32,
    pub obj:       SignedObject,
}

impl PolicyBundle {
    pub fn valid_dev(policy_id: [u8; 16]) -> Self {
        PolicyBundle {
            policy_id, version: 1,
            obj: SignedObject {
                kind: ObjectKind::PolicyBundle, id: policy_id,
                digest: [0xEF; 32], signature: DevSignature::VALID,
            },
        }
    }
}
