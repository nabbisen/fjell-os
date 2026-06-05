//! `DevSignatureProvider` — verifies / signs using the `DevDigest32`
//! development algorithm.
//!
//! The signature for a payload `p` and anchor `a` is:
//!
//! ```text
//! sig = SHA256( KEYRING_DOMAIN || "DEV-V1" || alg_tag || epoch(LE u32)
//!               || key_bytes || payload )
//! ```
//!
//! The first 32 bytes of the signature buffer hold the digest; subsequent
//! bytes are zero.  This is intentionally **not** cryptographically
//! secure — it only proves the verifier knows the same anchor and payload.
//!
//! In release mode the keyring forbids `DevDigest32`, so this provider
//! becomes a dev-only path and never gates production signatures.

use fjell_measure_format::Digest32;

use crate::algorithm::SignatureAlgorithm;
use crate::anchor::TrustAnchor;
use crate::error::SigError;
use crate::provider::SignatureProvider;
use crate::KEYRING_DOMAIN;

const DEV_DOMAIN: &[u8] = b"DEV-V1";

/// Development-only signature provider.
pub struct DevSignatureProvider;

impl DevSignatureProvider {
    pub const fn new() -> Self {
        Self
    }

    fn compute_dev_digest(anchor: &TrustAnchor, payload: &[u8]) -> [u8; 32] {
        let alg_byte = [anchor.algorithm.tag()];
        let epoch_le = anchor.epoch.raw().to_le_bytes();
        let d = Digest32::of_parts(&[
            KEYRING_DOMAIN,
            DEV_DOMAIN,
            &alg_byte,
            &epoch_le,
            anchor.key(),
            payload,
        ]);
        d.0
    }
}

impl Default for DevSignatureProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureProvider for DevSignatureProvider {
    fn supports(&self, alg: SignatureAlgorithm) -> bool {
        matches!(alg, SignatureAlgorithm::DevDigest32)
    }

    fn verify(
        &self,
        anchor: &TrustAnchor,
        payload: &[u8],
        signature: &[u8],
    ) -> Result<(), SigError> {
        if anchor.algorithm != SignatureAlgorithm::DevDigest32 {
            return Err(SigError::SignatureVerifyFailed);
        }
        if signature.len() != 32 {
            return Err(SigError::SignatureVerifyFailed);
        }
        let expect = Self::compute_dev_digest(anchor, payload);
        // Constant-time compare.
        let mut diff: u8 = 0;
        for i in 0..32 {
            diff |= expect[i] ^ signature[i];
        }
        if diff == 0 {
            Ok(())
        } else {
            Err(SigError::SignatureVerifyFailed)
        }
    }

    fn sign(
        &self,
        anchor: &TrustAnchor,
        payload: &[u8],
        out: &mut [u8; 64],
    ) -> Result<usize, SigError> {
        if anchor.algorithm != SignatureAlgorithm::DevDigest32 {
            return Err(SigError::ReleaseModeViolation);
        }
        let d = Self::compute_dev_digest(anchor, payload);
        out.fill(0);
        out[..32].copy_from_slice(&d);
        Ok(32)
    }
}
