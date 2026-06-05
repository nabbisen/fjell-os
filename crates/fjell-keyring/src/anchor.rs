//! `TrustAnchor` and `AuthorityClass` definitions.

use crate::algorithm::SignatureAlgorithm;
use crate::epoch::KeyEpoch;
use crate::{ANCHOR_KEY_BYTES_MAX};
use fjell_trust_provider::KeyPurpose;

/// Distinguishes a "genesis" anchor (the one baked into the OS image and
/// trusted unconditionally for its purpose) from "standard" anchors
/// installed via rotation or by snapshot import.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AuthorityClass {
    Genesis  = 0x01,
    Standard = 0x02,
}

impl AuthorityClass {
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// A registered trust anchor for a given `KeyPurpose`.
///
/// Anchors are `Copy` and fixed-size so the keyring stays alloc-free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrustAnchor {
    pub purpose:    KeyPurpose,
    pub algorithm:  SignatureAlgorithm,
    pub authority:  AuthorityClass,
    pub epoch:      KeyEpoch,
    pub key_len:    u8,
    pub key_bytes:  [u8; ANCHOR_KEY_BYTES_MAX],
}

impl TrustAnchor {
    /// Construct a `TrustAnchor` from a raw key slice.  Slices longer than
    /// `ANCHOR_KEY_BYTES_MAX` are rejected by returning `None`.
    pub fn new(
        purpose: KeyPurpose,
        algorithm: SignatureAlgorithm,
        authority: AuthorityClass,
        epoch: KeyEpoch,
        key: &[u8],
    ) -> Option<Self> {
        if key.len() > ANCHOR_KEY_BYTES_MAX {
            return None;
        }
        let mut buf = [0u8; ANCHOR_KEY_BYTES_MAX];
        buf[..key.len()].copy_from_slice(key);
        Some(Self {
            purpose,
            algorithm,
            authority,
            epoch,
            key_len: key.len() as u8,
            key_bytes: buf,
        })
    }

    /// Borrow the meaningful prefix of the key bytes.
    pub fn key(&self) -> &[u8] {
        &self.key_bytes[..self.key_len as usize]
    }
}
