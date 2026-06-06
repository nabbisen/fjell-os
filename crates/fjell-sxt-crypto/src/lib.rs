//! Crypto primitives for `secure-transportd` (RFC v0.4-003 §7.4).
//!
//! - AES-128-GCM (constant-time, table-free reference implementation).
//! - X25519 key exchange.
//! - HKDF-SHA256 key derivation.
//!
//! All primitives are pure functions with no side-effects; fully host-testable.
//! Tested against RFC test vectors in `tests.rs`.
#![no_std]

pub mod aes128;
pub mod gcm;
pub mod aead;
pub mod x25519;
pub mod sha256;
pub mod hkdf;
pub mod tls_state;

pub use aead::{Aead128Gcm, AeadError, AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN};
pub use x25519::{X25519Secret, X25519Public, x25519_diffie_hellman};
pub use hkdf::{hkdf_extract, hkdf_expand};
pub use tls_state::{TlsState, TlsHandshakeState, SxtError};

#[cfg(test)]
mod tests;
