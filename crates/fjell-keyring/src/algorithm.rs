//! Signature algorithms recognised by the v0.3 keyring.
//!
//! The numeric tags are persisted on disk and projected into the
//! semantic stream; reordering or repurposing them is a breaking change.

/// Public signature-algorithm tag.
///
/// `DevDigest32` is **forbidden** in release mode (`Keyring::release_mode == true`)
/// and is intended only for the development trust-provider path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SignatureAlgorithm {
    Ed25519     = 0x01,
    EcdsaP256   = 0x02,
    /// Development-only: signature is the SHA-256 of a domain-separated
    /// digest.  Permitted only when keyring is *not* in release mode.
    DevDigest32 = 0xFE,
}

impl SignatureAlgorithm {
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// `true` if this algorithm may be used to verify in release mode.
    pub const fn permitted_in_release(self) -> bool {
        !matches!(self, Self::DevDigest32)
    }

    /// Expected on-wire signature length for this algorithm.
    pub const fn signature_len(self) -> usize {
        match self {
            Self::Ed25519 => 64,
            Self::EcdsaP256 => 64, // r||s, fixed-width
            Self::DevDigest32 => 32,
        }
    }
}
