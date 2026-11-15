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
//! is planned for v0.9. The disclosure is this notice (and `aes128.rs`'s header): development
//! profile only, not for production, with the cache-timing leak above. RFC-v0.7.3-002 also
//! specified `docs/src/security/crypto-profile.md` and `crypto-roadmap.md`; **those two
//! documents do not exist** (E-028), so this notice is the whole of the write-up.
#![no_std]

#[cfg(not(feature = "crypto-profile-development"))]
compile_error!(
    "fjell-sxt-crypto requires the `crypto-profile-development` feature. \
     This crate is a development/reference profile and is NOT suitable for \
     production cryptographic use. Enable the feature explicitly to acknowledge \
     this limitation. The disclosure is the Development Profile Notice at the top of \
     crates/fjell-sxt-crypto/src/lib.rs (RFC-v0.7.3-002); the dedicated crypto-profile \
     and crypto-roadmap write-ups it specified do not exist."
);

pub mod aead;
pub mod aes128;
pub mod gcm;
pub mod hkdf;
pub mod sha256;
pub mod tls_state;
pub mod x25519;

#[allow(unused_imports)] // v0.7: AEAD constants used by SXT handshake tests
pub use aead::{AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, Aead128Gcm, AeadError};
pub use hkdf::{hkdf_expand, hkdf_extract};
pub use tls_state::{SxtError, TlsHandshakeState, TlsState};
#[allow(unused_imports)] // v0.7: X25519Public used in key-exchange tests
pub use x25519::{X25519Public, X25519Secret, x25519_diffie_hellman};

#[cfg(test)]
mod tests;
