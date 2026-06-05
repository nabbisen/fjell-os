//! The `HardwareTrustProvider` trait.
//!
//! See RFC-v0.3-001 §5 for the external contract.  Implementors must:
//!
//! - never call into the kernel directly (this is a user-space trait);
//! - never block forever on hardware — concrete providers must time out and
//!   return `TrustError::ProviderUnavailable` instead;
//! - never panic;
//! - report `TrustError::NotSupported` for capabilities they do not advertise.

use crate::descriptor::TrustProviderDescriptor;
use crate::error::TrustError;
use crate::ids::{KeyPurpose, TrustProviderId};
use crate::material::{
    AttestationDigest, KeyMaterial, MeasurementHead, SealedKey, Signature,
};

/// Provider-neutral interface for hardware-rooted (or development-grade
/// software-rooted) trust evidence.
///
/// The trait is intentionally narrow.  Anything that needs general policy or
/// orchestration belongs in `verifyd`, `attestd`, or `upgraded`.
pub trait HardwareTrustProvider {
    /// Return the stable id assigned by the registry.
    fn provider_id(&self) -> TrustProviderId;

    /// Return the public descriptor for this provider.
    fn descriptor(&self) -> TrustProviderDescriptor;

    /// Read the current measurement chain head as known to the provider.
    ///
    /// For software providers this returns the head supplied by `measuredd`
    /// via the registry; for hardware providers this consults the hardware
    /// PCRs / DICE state.
    fn read_measurement(&self) -> Result<MeasurementHead, TrustError> {
        Err(TrustError::NotSupported)
    }

    /// Sign an attestation digest.
    ///
    /// The provider does not interpret `input` — the caller is responsible
    /// for binding domain separators into the digest.
    fn sign_attestation(&self, _input: AttestationDigest) -> Result<Signature, TrustError> {
        Err(TrustError::NotSupported)
    }

    /// Read the anti-rollback counter exposed by the provider.
    ///
    /// Counters are monotonic across boots.  Concrete providers persist the
    /// counter externally (e.g. eFuse, TPM NVRAM); development providers
    /// store it in `storaged` via the registry helper.
    fn read_anti_rollback_counter(&self) -> Result<u64, TrustError> {
        Err(TrustError::NotSupported)
    }

    /// Seal a key for the given purpose.
    fn seal_key(
        &self,
        _purpose: KeyPurpose,
        _key: KeyMaterial,
    ) -> Result<SealedKey, TrustError> {
        Err(TrustError::NotSupported)
    }

    /// Unseal a previously-sealed key.
    ///
    /// Implementations **must** reject a `SealedKey` whose `purpose` does not
    /// match the requested `purpose` with `TrustError::PurposeMismatch`.
    fn unseal_key(
        &self,
        _purpose: KeyPurpose,
        _sealed: &SealedKey,
    ) -> Result<KeyMaterial, TrustError> {
        Err(TrustError::NotSupported)
    }
}
