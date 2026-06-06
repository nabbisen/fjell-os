//! Fjell OS Hardware Trust Provider (`fjell-trust-provider`).
//!
//! Provider-neutral interface that user-space services (`verifyd`, `attestd`,
//! `measuredd`, `upgraded`) use to consume the local trust evidence without
//! learning hardware-specific details such as TPM, DICE, or eFuse.
//!
//! This crate is `no_std`, host-testable, and carries **no policy**.  The
//! kernel never imports it; only user-space services and host tools do.
//!
//! Design source: RFC-v0.3-001 — *HardwareTrustProvider Interface and Provider
//! Registry*.
//!
//! # Layered position
//!
//! ```text
//! verifyd / attestd / measuredd / upgraded
//!     |
//!     v
//! HardwareTrustProvider trait        <-- this crate, provider-neutral
//!     |
//!     v
//! DevelopmentTrustProvider           <-- in this crate, software-only
//! TpmTrustProvider (future, v0.3.x)
//! DiceTrustProvider (future, v0.3.x)
//! ```
//!
//! # Non-goals
//!
//! - No kernel changes.
//! - No new ambient authority — providers are accessed via the
//!   service plane and remain capability-gated by `verifyd`/`attestd`.
//! - No remote attestation transport — that is v0.4+ scope.
//! - No production-grade signature scheme is required for v0.3.0-alpha; the
//!   `DevelopmentTrustProvider` uses a deterministic test-only signature.
//!
//! # Crate layout
//!
//! ```text
//! lib.rs            — crate root, re-exports
//! ids.rs            — TrustProviderId, ProviderHandle, key purposes
//! profile.rs        — TrustProfile, TrustProviderCapabilities, TrustProviderState
//! descriptor.rs     — TrustProviderDescriptor
//! material.rs       — AttestationDigest, Signature, KeyMaterial, SealedKey
//! error.rs          — TrustError
//! provider.rs       — HardwareTrustProvider trait
//! development.rs    — DevelopmentTrustProvider implementation
//! null.rs           — NullTrustProvider (test-only)
//! registry.rs       — ProviderRegistry (bootstrap → enforcing handoff)
//! tests.rs          — host unit tests
//! ```
#![no_std]
#![deny(unsafe_code)]

pub mod descriptor;
pub mod development;
pub mod error;
pub mod ids;
pub mod material;
pub mod null;
pub mod profile;
pub mod provider;
pub mod registry;

pub use descriptor::TrustProviderDescriptor;
pub use development::DevelopmentTrustProvider;
pub use error::TrustError;
pub use ids::{KeyPurpose, ProviderHandle, TrustProviderId};
pub use material::{
    AttestationDigest, KeyMaterial, MeasurementHead, SealedKey, Signature, SIGNATURE_LEN,
};
pub use null::NullTrustProvider;
pub use profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};
pub use provider::HardwareTrustProvider;
// PolicyAuth exported below
pub use registry::PolicyAuth;
pub use registry::{ProviderRegistry, RegistryError, RegistryPhase};

/// Schema version for `TrustProviderDescriptor` serialization.
pub const SCHEMA_VERSION: u16 = 1;

/// Domain separator embedded in trust-provider digest computations so that a
/// provider's signature commitments cannot be replayed against other Fjell
/// domains (release verification, snapshots, etc).
pub const TRUST_DOMAIN: &[u8] = b"FJELL-TRUST-V1";

#[cfg(test)]
mod tests;
