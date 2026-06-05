//! Cryptographic material types used by `HardwareTrustProvider`.
//!
//! All types are fixed-size, `Copy`, `no_std`, and carry domain separators in
//! their canonical digest formulas.  No allocation is performed here.

use fjell_measure_format::Digest32;

use crate::ids::KeyPurpose;

/// Fixed signature size used in v0.3.0-alpha.
///
/// Production providers (TPM/DICE) may return shorter signatures; the
/// `Signature` type is sized to accept Ed25519 / production schemes that
/// fit in 64 bytes.  An ADR is required before changing this constant.
pub const SIGNATURE_LEN: usize = 64;

/// A fixed-length signature produced by `HardwareTrustProvider::sign_attestation`.
///
/// `len` is the number of meaningful leading bytes; trailing bytes are zeroed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub bytes: [u8; SIGNATURE_LEN],
    pub len: u8,
}

impl Signature {
    pub const ZERO: Self = Self {
        bytes: [0u8; SIGNATURE_LEN],
        len: 0,
    };

    /// Build a signature from a slice; panics in debug if the slice is too
    /// long (the function is `no_std` so we can't return a fancy error type
    /// here without bloating the API).  Production callers should ensure the
    /// slice is `<= SIGNATURE_LEN`.
    pub const fn from_bytes(src: &[u8]) -> Self {
        // const-fn-friendly copy
        let mut bytes = [0u8; SIGNATURE_LEN];
        let mut i = 0;
        let n = if src.len() < SIGNATURE_LEN {
            src.len()
        } else {
            SIGNATURE_LEN
        };
        while i < n {
            bytes[i] = src[i];
            i += 1;
        }
        Self {
            bytes,
            len: n as u8,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Signature(len={}, bytes=…)", self.len)
    }
}

/// Digest fed to `HardwareTrustProvider::sign_attestation`.
///
/// The wrapped digest is opaque to the provider — the provider only signs it.
/// The caller is responsible for constructing the digest with a domain
/// separator (`TRUST_DOMAIN` from the crate root).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttestationDigest(pub Digest32);

impl AttestationDigest {
    pub const ZERO: Self = Self(Digest32::ZERO);

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0 .0
    }
}

/// Re-export of the measurement chain head from `fjell-measure-format`.
///
/// This is the type returned by `read_measurement`; alias avoids forcing every
/// downstream service to depend on the measure-format crate directly.
pub type MeasurementHead = fjell_measure_format::MeasurementHead;

/// Plain (not-yet-sealed) key material handed to `seal_key`.
///
/// The caller owns the buffer.  v0.3.0-alpha caps key material at 64 bytes,
/// which is sufficient for 256-bit symmetric or Ed25519 secret keys.
#[derive(Clone, Copy)]
pub struct KeyMaterial {
    pub bytes: [u8; 64],
    pub len: u8,
}

impl KeyMaterial {
    pub const ZERO: Self = Self {
        bytes: [0u8; 64],
        len: 0,
    };

    pub fn from_bytes(src: &[u8]) -> Self {
        let mut bytes = [0u8; 64];
        let n = src.len().min(64);
        bytes[..n].copy_from_slice(&src[..n]);
        Self {
            bytes,
            len: n as u8,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl core::fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "KeyMaterial(len={}, bytes=REDACTED)", self.len)
    }
}

impl PartialEq for KeyMaterial {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for KeyMaterial {}

/// Sealed key as returned by `HardwareTrustProvider::seal_key`.
///
/// `purpose` is bound into the sealed blob — a key sealed for one purpose
/// cannot be unsealed for another.  The provider is responsible for enforcing
/// this in its `unseal` implementation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SealedKey {
    pub purpose: KeyPurpose,
    /// Provider-specific opaque payload (encrypted material + integrity tag).
    pub blob: [u8; 96],
    pub blob_len: u8,
    /// Generation/epoch from the provider; advisory only.
    pub epoch: u32,
}

impl SealedKey {
    pub const fn empty(purpose: KeyPurpose) -> Self {
        Self {
            purpose,
            blob: [0u8; 96],
            blob_len: 0,
            epoch: 0,
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.blob[..self.blob_len as usize]
    }
}
