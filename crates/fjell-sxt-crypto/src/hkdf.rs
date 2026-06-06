//! HKDF-SHA256 (RFC 5869).

use crate::sha256::{hmac_sha256, Sha256Digest};

pub const HKDF_HASH_LEN: usize = 32;

/// HKDF-Extract: PRK = HMAC-SHA256(salt, IKM).
///
/// If `salt` is empty, uses a zero-filled 32-byte salt per RFC 5869 §2.2.
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> Sha256Digest {
    let zero_salt = [0u8; HKDF_HASH_LEN];
    let actual_salt = if salt.is_empty() { &zero_salt[..] } else { salt };
    hmac_sha256(actual_salt, ikm)
}

/// HKDF-Expand: OKM = T(1) || T(2) || ... (up to `okm.len()` bytes).
///
/// `info` is the context label. `okm.len()` must be ≤ 255 * HKDF_HASH_LEN.
///
/// T(i) = HMAC(PRK, T(i-1) || info || i_byte)
pub fn hkdf_expand(prk: &Sha256Digest, info: &[u8], okm: &mut [u8]) {
    let n = (okm.len() + HKDF_HASH_LEN - 1) / HKDF_HASH_LEN;
    assert!(n <= 255, "hkdf_expand: requested length too large");

    // We build each round's input by hand to avoid a fixed-size buffer.
    // Max round input: 32 (T_prev) + info.len() + 1 (counter) ≤ 32 + 80 + 1 = 113 bytes.
    let mut t_prev: [u8; 32] = [0u8; 32];
    let mut produced = 0usize;

    for i in 1..=(n as u8) {
        // Build the HMAC data: T(i-1) || info || counter byte.
        let data = if i == 1 {
            // First round: no T(i-1).
            build_hmac_data(&[], info, i)
        } else {
            build_hmac_data(&t_prev, info, i)
        };
        let t = hmac_sha256(prk, &data);
        let take = (okm.len() - produced).min(HKDF_HASH_LEN);
        okm[produced..produced + take].copy_from_slice(&t[..take]);
        t_prev = t;
        produced += take;
    }
}

/// Build the HMAC data for one HKDF-Expand round.
/// Layout: [t_prev | info | counter_byte]
fn build_hmac_data(t_prev: &[u8], info: &[u8], counter: u8) -> HkdfRoundBuf {
    HkdfRoundBuf::new(t_prev, info, counter)
}

// A small fixed-size buffer that can hold one round's worth of HKDF data.
// Max: 32 (t_prev) + 80 (info max) + 1 (counter) = 113.
const ROUND_BUF_MAX: usize = 200;

struct HkdfRoundBuf {
    data: [u8; ROUND_BUF_MAX],
    len: usize,
}

impl HkdfRoundBuf {
    fn new(t_prev: &[u8], info: &[u8], counter: u8) -> Self {
        let mut buf = [0u8; ROUND_BUF_MAX];
        let mut pos = 0;
        let parts: &[&[u8]] = &[t_prev, info, &[counter]];
        for part in parts {
            let end = (pos + part.len()).min(ROUND_BUF_MAX);
            buf[pos..end].copy_from_slice(&part[..end - pos]);
            pos = (pos + part.len()).min(ROUND_BUF_MAX);
        }
        Self { data: buf, len: pos }
    }
}

impl core::ops::Deref for HkdfRoundBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] { &self.data[..self.len] }
}
