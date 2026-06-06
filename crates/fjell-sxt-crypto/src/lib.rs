//! Crypto primitives for `secure-transportd` (RFC v0.4-003 §7.4).
//!
//! # Development Profile Notice (RFC-v0.7.3-002)
//!
//! This crate requires the `crypto-profile-development` feature to compile.
//! The AES-128 implementation uses a 256-byte S-box with data-dependent
//! indexing; on cache-bearing targets this may leak key material via
//! cache-timing side channels. The `constant-time` claim in earlier docs
//! was inaccurate and has been removed.
//!
//! **Do not use in production.** Migration to a vetted no_std crypto library
//! is planned for v0.9. See `docs/src/security/crypto-roadmap.md`.
#![no_std]

#[cfg(not(feature = "crypto-profile-development"))]
compile_error!(
    "fjell-sxt-crypto requires the `crypto-profile-development` feature. \
     This crate is a development/reference profile and is NOT suitable for \
     production cryptographic use. Enable the feature explicitly to acknowledge \
     this limitation. See RFC-v0.7.3-002 and docs/src/security/crypto-profile.md."
);

pub mod aes128;
pub mod gcm;
pub mod aead;
pub mod x25519;
pub mod sha256;
pub mod hkdf;
pub mod tls_state;

#[allow(unused_imports)] // v0.7: AEAD constants used by SXT handshake tests
pub use aead::{Aead128Gcm, AeadError, AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN};
#[allow(unused_imports)] // v0.7: X25519Public used in key-exchange tests
pub use x25519::{X25519Secret, X25519Public, x25519_diffie_hellman};
pub use hkdf::{hkdf_extract, hkdf_expand};
pub use tls_state::{TlsState, TlsHandshakeState, SxtError};

#[cfg(test)]
mod tests;
