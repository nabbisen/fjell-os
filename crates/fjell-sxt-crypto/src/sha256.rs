//! SHA-256 wrapper for use in the TLS crypto layer.
//!
//! Delegates to `fjell-measure-format`'s `Digest32::of_parts` to avoid
//! duplicating the SHA-256 implementation.

use fjell_measure_format::Digest32;

/// 32-byte SHA-256 digest.
pub type Sha256Digest = [u8; 32];

/// Compute SHA-256 over a slice.
pub fn sha256(data: &[u8]) -> Sha256Digest {
    Digest32::of(data).0
}

/// Compute SHA-256 over a sequence of parts.
pub fn sha256_parts(parts: &[&[u8]]) -> Sha256Digest {
    Digest32::of_parts(parts).0
}

/// HMAC-SHA256.
///
/// Implements the standard HMAC construction using the `ipad` / `opad`.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Sha256Digest {
    const BLOCK_LEN: usize = 64;
    // Shorten key if needed.
    let key_hash;
    let key_bytes: &[u8] = if key.len() > BLOCK_LEN {
        key_hash = sha256(key);
        &key_hash
    } else {
        key
    };
    let mut k_ipad = [0x36u8; BLOCK_LEN];
    let mut k_opad = [0x5cu8; BLOCK_LEN];
    for i in 0..key_bytes.len() {
        k_ipad[i] ^= key_bytes[i];
        k_opad[i] ^= key_bytes[i];
    }
    let inner = sha256_parts(&[&k_ipad, data]);
    sha256_parts(&[&k_opad, &inner])
}
