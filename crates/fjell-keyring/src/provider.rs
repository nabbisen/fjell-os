//! `SignatureProvider` trait.
//!
//! Algorithm-agnostic verification (and, for some purposes, signing) over
//! a `TrustAnchor` set.  Concrete providers live alongside the v0.3
//! development trust provider; production providers will appear in v0.3.x.

use crate::algorithm::SignatureAlgorithm;
use crate::anchor::TrustAnchor;
use crate::error::SigError;

/// Provider that knows how to verify (and optionally produce) signatures
/// for one or more `SignatureAlgorithm`s.
///
/// Verification and signing both operate on a *raw payload*; the keyring
/// is responsible for embedding the canonical domain separator
/// `KEYRING_DOMAIN` before invoking the provider.
pub trait SignatureProvider {
    /// True if this provider can verify signatures produced under `alg`.
    fn supports(&self, alg: SignatureAlgorithm) -> bool;

    /// Verify `signature` over `payload` using `anchor`.
    ///
    /// Returns `Ok(())` on success and `Err(SignatureVerifyFailed)` on
    /// any failure.  Other `SigError` codes report structural problems
    /// (algorithm mismatch, malformed signature length, etc.).
    fn verify(
        &self,
        anchor: &TrustAnchor,
        payload: &[u8],
        signature: &[u8],
    ) -> Result<(), SigError>;

    /// Optional signing path.  Default returns `SigError::ReleaseModeViolation`
    /// to make "I don't sign here" the safer default.
    fn sign(
        &self,
        _anchor: &TrustAnchor,
        _payload: &[u8],
        _out: &mut [u8; 64],
    ) -> Result<usize, SigError> {
        Err(SigError::ReleaseModeViolation)
    }
}
