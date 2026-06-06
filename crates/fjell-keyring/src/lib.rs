//! Fjell OS Keyring (`fjell-keyring`).
//!
//! Stable identifiers, signature-provider abstraction, and signed
//! key-epoch model used by `verifyd`, `attestd`, and `upgraded`.
//!
//! Design source: RFC-v0.3-002 — *Keyring, Key-Purpose Signature Provider,
//! and Key-Epoch Model*.
//!
//! This is the v0.3.0-alpha.1 skeleton landing.  The minimum surface is
//! defined here; full anchor / snapshot / DevSignatureProvider work
//! continues in `crates/fjell-keyring/src/*.rs`.
#![no_std]
#![deny(unsafe_code)]

pub use fjell_trust_provider::KeyPurpose;

/// Domain separator embedded in every keyring digest so signatures cannot
/// be cross-replayed against other Fjell domains.
pub const KEYRING_DOMAIN: &[u8] = b"FJELL-KEYRING-V1";

/// Schema version for `KeyringSnapshot` serialisation.
pub const SCHEMA_VERSION: u16 = 1;

/// Maximum number of anchors per `KeyPurpose`.
///
/// Mirrors RFC-v0.3-002 §6.1: 4 anchors per purpose is enough for
/// genesis + 2 in-flight rotations + 1 head-room.
pub const ANCHORS_PER_PURPOSE: usize = 4;

/// Maximum signature size we accept anywhere in the v0.3 keyring.
///
/// 64 bytes is sufficient for Ed25519, ECDSA P-256, and the development
/// `DevDigest32` algorithm.
pub const SIGNATURE_LEN_MAX: usize = 64;

/// Maximum raw anchor key size: 64 B covers Ed25519 (32), ECDSA P-256
/// uncompressed (65 → truncated below; production providers handle the
/// full encoding outside the keyring), and the SHA-256 development hash.
pub const ANCHOR_KEY_BYTES_MAX: usize = 64;

pub mod algorithm;
pub mod anchor;
pub mod dev_provider;
pub mod epoch;
pub mod error;
pub mod keyring;
pub mod provider;
pub mod snapshot;

pub use algorithm::SignatureAlgorithm;
pub use anchor::{AuthorityClass, TrustAnchor};
pub use dev_provider::DevSignatureProvider;
pub use epoch::KeyEpoch;
pub use error::SigError;
pub use keyring::{Keyring, PURPOSE_SLOT_COUNT};
pub use provider::SignatureProvider;
pub use snapshot::{KeyringSnapshot, KEYRING_SNAPSHOT_MAGIC, MAX_SNAPSHOT_ANCHORS};

#[cfg(test)]
mod tests;
pub mod revocation;
pub use revocation::{AnchorState, RevocationReason, RevocationRecord, RevocationTable};
