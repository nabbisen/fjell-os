//! `DevelopmentTrustProvider` — software-only provider for QEMU/local tests.
//!
//! This provider is **not** suitable for production.  It is deterministic by
//! design so that QEMU smoke tests can pin signatures; production providers
//! must use a hardware root of trust.
//!
//! # Determinism
//!
//! The "signature" is a SHA-256 keyed digest computed as:
//!
//! ```text
//! sig_bytes = SHA256( TRUST_DOMAIN || provider_id(LE u32) || dev_key || digest )
//! ```
//!
//! and the signature occupies the first 32 bytes of the 64-byte signature
//! buffer.  The remaining bytes are zero.  This is intentional: production
//! providers will fill all 64 bytes with a real Ed25519 / ECDSA signature.

use core::cell::Cell;

use fjell_measure_format::{Digest32, MeasurementHead};

use crate::descriptor::TrustProviderDescriptor;
use crate::error::TrustError;
use crate::ids::{KeyPurpose, TrustProviderId};
use crate::material::{AttestationDigest, KeyMaterial, SealedKey, Signature, SIGNATURE_LEN};
use crate::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};
use crate::TRUST_DOMAIN;

/// Development-grade trust provider.
///
/// All fields are `Cell` rather than `RefCell` because the contained state is
/// `Copy` and updates are short, single-value writes.  This keeps the type
/// `Sync`-free without an explicit unsafe.
pub struct DevelopmentTrustProvider {
    descriptor: Cell<TrustProviderDescriptor>,
    dev_key: [u8; 32],
    rollback_counter: Cell<u64>,
    measurement: Cell<MeasurementHead>,
}

impl DevelopmentTrustProvider {
    /// Construct a development provider with a deterministic test key.
    ///
    /// The `dev_key` is **not secret** in any real sense — it is committed to
    /// source code by the build that registers the provider.  Production
    /// providers must derive their secret from hardware.
    pub fn new(id: TrustProviderId, generation: u16, dev_key: [u8; 32]) -> Self {
        let descriptor = TrustProviderDescriptor::new(
            id,
            TrustProviderKind::Development,
            TrustProfile::FjellLocalV1,
            TrustProviderCapabilities::DEVELOPMENT_BASELINE,
            TrustProviderState::Active,
            generation,
            *b"fjell-dv",
        );
        Self {
            descriptor: Cell::new(descriptor),
            dev_key,
            rollback_counter: Cell::new(1),
            measurement: Cell::new(MeasurementHead::EMPTY),
        }
    }

    /// Construct a development provider with the canonical zero-key.  Useful
    /// in tests that need deterministic signatures across runs.
    pub fn with_default_key(id: TrustProviderId, generation: u16) -> Self {
        Self::new(id, generation, [0xA5; 32])
    }

    /// Inject a measurement head (called by `measuredd` through the registry
    /// in a real boot flow).
    pub fn set_measurement(&self, head: MeasurementHead) {
        self.measurement.set(head);
    }

    /// Advance the rollback counter; returns `RollbackCounterExhausted` if the
    /// counter would overflow.  The counter is monotonic.
    pub fn advance_rollback_counter(&self) -> Result<u64, TrustError> {
        let cur = self.rollback_counter.get();
        let next = cur.checked_add(1).ok_or(TrustError::RollbackCounterExhausted)?;
        self.rollback_counter.set(next);
        Ok(next)
    }

    /// Force the provider into a faulted state — used by tests for negative
    /// paths and by services on observed internal error.
    pub fn force_fault(&self) {
        let mut d = self.descriptor.get();
        d.state = TrustProviderState::Faulted;
        self.descriptor.set(d);
    }
}

impl crate::provider::HardwareTrustProvider for DevelopmentTrustProvider {
    fn provider_id(&self) -> TrustProviderId {
        self.descriptor.get().id
    }

    fn descriptor(&self) -> TrustProviderDescriptor {
        self.descriptor.get()
    }

    fn read_measurement(&self) -> Result<MeasurementHead, TrustError> {
        if !self.descriptor.get().state.is_usable() {
            return Err(TrustError::ProviderUnavailable);
        }
        Ok(self.measurement.get())
    }

    fn sign_attestation(&self, input: AttestationDigest) -> Result<Signature, TrustError> {
        if !self.descriptor.get().state.is_usable() {
            return Err(TrustError::ProviderUnavailable);
        }
        let id = self.provider_id().0.to_le_bytes();
        let sig_digest = Digest32::of_parts(&[TRUST_DOMAIN, &id, &self.dev_key, input.as_bytes()]);
        let mut out = [0u8; SIGNATURE_LEN];
        out[..32].copy_from_slice(&sig_digest.0);
        Ok(Signature {
            bytes: out,
            len: 32,
        })
    }

    fn read_anti_rollback_counter(&self) -> Result<u64, TrustError> {
        if !self.descriptor.get().state.is_usable() {
            return Err(TrustError::ProviderUnavailable);
        }
        Ok(self.rollback_counter.get())
    }

    fn seal_key(&self, purpose: KeyPurpose, key: KeyMaterial) -> Result<SealedKey, TrustError> {
        if !self.descriptor.get().state.is_usable() {
            return Err(TrustError::ProviderUnavailable);
        }
        if key.len as usize > 64 {
            return Err(TrustError::KeyMaterialTooLarge);
        }
        // "Seal" = XOR with stretched dev_key and prepend a purpose-bound MAC.
        let mut blob = [0u8; 96];
        let mac = Digest32::of_parts(&[
            TRUST_DOMAIN,
            b"SEAL",
            &[purpose.tag()],
            &self.dev_key,
            key.as_slice(),
        ]);
        blob[..32].copy_from_slice(&mac.0);
        let body_len = key.len as usize;
        for i in 0..body_len {
            blob[32 + i] = key.bytes[i] ^ self.dev_key[i % 32];
        }
        Ok(SealedKey {
            purpose,
            blob,
            blob_len: (32 + body_len) as u8,
            epoch: self.rollback_counter.get() as u32,
        })
    }

    fn unseal_key(
        &self,
        purpose: KeyPurpose,
        sealed: &SealedKey,
    ) -> Result<KeyMaterial, TrustError> {
        if !self.descriptor.get().state.is_usable() {
            return Err(TrustError::ProviderUnavailable);
        }
        if sealed.purpose != purpose {
            return Err(TrustError::PurposeMismatch);
        }
        if sealed.blob_len < 32 {
            return Err(TrustError::SealIntegrityFailed);
        }
        let body_len = sealed.blob_len as usize - 32;
        let mut raw = [0u8; 64];
        for i in 0..body_len {
            raw[i] = sealed.blob[32 + i] ^ self.dev_key[i % 32];
        }
        // Recompute MAC and constant-time compare.
        let mac = Digest32::of_parts(&[
            TRUST_DOMAIN,
            b"SEAL",
            &[purpose.tag()],
            &self.dev_key,
            &raw[..body_len],
        ]);
        let mut diff: u8 = 0;
        for i in 0..32 {
            diff |= mac.0[i] ^ sealed.blob[i];
        }
        if diff != 0 {
            return Err(TrustError::SealIntegrityFailed);
        }
        Ok(KeyMaterial {
            bytes: raw,
            len: body_len as u8,
        })
    }
}
