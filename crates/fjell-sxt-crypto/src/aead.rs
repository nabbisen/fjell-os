//! AEAD interface wrapping AES-128-GCM with a monotonic per-record nonce.

use crate::gcm::{gcm_encrypt, gcm_decrypt, GCM_TAG_LEN};

pub const AEAD_KEY_LEN:   usize = 16;
pub const AEAD_NONCE_LEN: usize = 12;
pub const AEAD_TAG_LEN:   usize = GCM_TAG_LEN;

/// Errors from AEAD operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AeadError {
    /// Authentication tag did not match; data is discarded.
    AuthFailed    = 0x01,
    /// Output buffer is too small for ciphertext + tag.
    BufferTooSmall = 0x02,
    /// The per-record nonce counter has wrapped (2^32 records).
    NonceExhausted = 0x03,
}

/// AES-128-GCM AEAD cipher with monotonic nonce management.
///
/// The nonce is derived as `iv_fixed XOR (counter as 32-bit BE)` in the
/// last 4 bytes — matching TLS 1.3 per-record nonce construction (RFC 8446
/// §5.3).
pub struct Aead128Gcm {
    key:     [u8; AEAD_KEY_LEN],
    iv:      [u8; AEAD_NONCE_LEN],
    counter: u64,
}

impl Aead128Gcm {
    /// Construct from a 16-byte key and 12-byte IV (write key + IV from TLS
    /// key schedule).
    pub fn new(key: [u8; AEAD_KEY_LEN], iv: [u8; AEAD_NONCE_LEN]) -> Self {
        Self { key, iv, counter: 0 }
    }

    /// Derive the per-record nonce by XOR-ing the counter into the last 8
    /// bytes of the IV (TLS 1.3 §5.3).
    fn record_nonce(&self) -> [u8; AEAD_NONCE_LEN] {
        let mut nonce = self.iv;
        let ctr_bytes = self.counter.to_be_bytes();
        for i in 0..8 { nonce[4 + i] ^= ctr_bytes[i]; }
        nonce
    }

    /// Encrypt `plaintext` with `aad`, writing ciphertext + tag into `out`.
    ///
    /// `out.len()` must be ≥ `plaintext.len() + AEAD_TAG_LEN`.
    pub fn seal(&mut self, aad: &[u8], plaintext: &[u8], out: &mut [u8])
        -> Result<usize, AeadError>
    {
        let ct_len = plaintext.len();
        let needed = ct_len + AEAD_TAG_LEN;
        if out.len() < needed { return Err(AeadError::BufferTooSmall); }
        if self.counter == u64::MAX { return Err(AeadError::NonceExhausted); }
        let nonce = self.record_nonce();
        let tag = gcm_encrypt(&self.key, &nonce, aad, plaintext, &mut out[..ct_len]);
        out[ct_len..ct_len + AEAD_TAG_LEN].copy_from_slice(&tag);
        self.counter += 1;
        Ok(needed)
    }

    /// Decrypt `ciphertext_with_tag` (last 16 bytes are the tag), writing
    /// plaintext into `out`.
    ///
    /// Returns the plaintext length on success, or `AuthFailed`.
    pub fn open(&mut self, aad: &[u8], ciphertext_with_tag: &[u8], out: &mut [u8])
        -> Result<usize, AeadError>
    {
        if ciphertext_with_tag.len() < AEAD_TAG_LEN {
            return Err(AeadError::AuthFailed);
        }
        let ct_len = ciphertext_with_tag.len() - AEAD_TAG_LEN;
        if out.len() < ct_len { return Err(AeadError::BufferTooSmall); }
        let tag: [u8; AEAD_TAG_LEN] = ciphertext_with_tag[ct_len..]
            .try_into().map_err(|_| AeadError::AuthFailed)?;
        if self.counter == u64::MAX { return Err(AeadError::NonceExhausted); }
        let nonce = self.record_nonce();
        let ok = gcm_decrypt(
            &self.key, &nonce, aad,
            &ciphertext_with_tag[..ct_len], &mut out[..ct_len], &tag,
        );
        if ok { self.counter += 1; Ok(ct_len) }
        else  { Err(AeadError::AuthFailed) }
    }

    pub fn record_count(&self) -> u64 { self.counter }
}
