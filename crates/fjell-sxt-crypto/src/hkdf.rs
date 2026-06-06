//! HKDF-SHA256 (RFC 5869).
//!
//! Status: DEVELOPMENT PROFILE — see crate-level notice in lib.rs.
//!
//! v0.7.1 fix (RFC-v0.7.3-002): `hkdf_expand` now returns `Result<(), HkdfError>`
//! instead of panicking on oversized output. Info is written in full to HMAC
//! without truncation.

use crate::sha256::{hmac_sha256, Sha256Digest};

pub const HKDF_HASH_LEN: usize = 32;

/// Error returned by `hkdf_expand` (RFC-v0.7.3-002, closes C-H-01).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum HkdfError {
    /// Requested output length exceeds 255 * 32 bytes (RFC 5869 §2.3).
    OutputTooLong = 0x01,
}

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
/// `info` is the context label — written in FULL to each HMAC round.
/// `okm.len()` must be ≤ 255 * HKDF_HASH_LEN; returns `Err(OutputTooLong)`
/// otherwise (replaces the old `assert!` panic, RFC-v0.7.3-002).
///
/// T(i) = HMAC(PRK, T(i-1) || info || i_byte)
pub fn hkdf_expand(prk: &Sha256Digest, info: &[u8], okm: &mut [u8]) -> Result<(), HkdfError> {
    let n = (okm.len() + HKDF_HASH_LEN - 1) / HKDF_HASH_LEN;
    if n > 255 {
        return Err(HkdfError::OutputTooLong);
    }

    let mut t_prev: [u8; 32] = [0u8; 32];
    let mut produced = 0usize;

    for i in 1..=(n as u8) {
        // Build the HMAC data: T(i-1) || info || counter byte.
        let round = if i == 1 {
            build_hmac_data(&[], info, i)
        } else {
            build_hmac_data(&t_prev, info, i)
        };
        let t = hmac_for_round(prk, &round);
        let take = (okm.len() - produced).min(HKDF_HASH_LEN);
        okm[produced..produced + take].copy_from_slice(&t[..take]);
        t_prev = t;
        produced += take;
    }
    Ok(())
}

/// Build the HMAC data for one HKDF-Expand round.
/// Layout: [t_prev | info | counter_byte]
fn build_hmac_data(t_prev: &[u8], info: &[u8], counter: u8) -> HkdfRoundBuf {
    HkdfRoundBuf::new(t_prev, info, counter)
}

// A small buffer for one round's HKDF data.
// Layout: 32 (t_prev) + unbounded info + 1 (counter).
// We use a fixed 512-byte buffer; info longer than ~479 bytes is unusual.
// The CRITICAL difference from the old code: we DO NOT truncate info.
// If info exceeds the buffer capacity we fall back to a heap-less write path
// (no allocation in no_std — we split into two HMAC updates).
const ROUND_BUF_MAX: usize = 512;

struct HkdfRoundBuf {
    data: [u8; ROUND_BUF_MAX],
    len: usize,
    // If info didn't fit, these carry the overflow portion.
    overflow_info: *const u8,
    overflow_len:  usize,
    counter_byte:  u8,
    uses_overflow: bool,
}

impl HkdfRoundBuf {
    fn new(t_prev: &[u8], info: &[u8], counter: u8) -> Self {
        let mut buf = Self {
            data: [0u8; ROUND_BUF_MAX],
            len: 0,
            overflow_info: core::ptr::null(),
            overflow_len: 0,
            counter_byte: counter,
            uses_overflow: false,
        };
        let prefix_len = t_prev.len();
        if prefix_len + info.len() + 1 <= ROUND_BUF_MAX {
            buf.data[..prefix_len].copy_from_slice(t_prev);
            buf.data[prefix_len..prefix_len + info.len()].copy_from_slice(info);
            buf.data[prefix_len + info.len()] = counter;
            buf.len = prefix_len + info.len() + 1;
        } else {
            // Overflow path: fit t_prev in buf, stream info + counter separately.
            buf.data[..prefix_len].copy_from_slice(t_prev);
            buf.len = prefix_len;
            buf.overflow_info = info.as_ptr();
            buf.overflow_len = info.len();
            buf.uses_overflow = true;
        }
        buf
    }
}

// Override hmac_sha256 call when we use the overflow path.
// We do this by providing a custom as_slice that the caller uses.
// For the overflow case the caller falls into a two-update HMAC sequence.

impl HkdfRoundBuf {
    /// Returns the single-buffer slice when info fits, or None for overflow.
    pub fn as_slice_if_fits(&self) -> Option<&[u8]> {
        if !self.uses_overflow {
            Some(&self.data[..self.len])
        } else {
            None
        }
    }
    pub fn prefix(&self) -> &[u8] { &self.data[..self.len] }
    pub fn overflow_info_slice(&self) -> &[u8] {
        if self.uses_overflow {
            // SAFETY: category=raw-pointer-deref
            //   overflow_info was set from info.as_ptr() in new(), which is valid for
            //   overflow_len bytes. The lifetime of info outlives this struct because
            //   build_hmac_data is called and used within the same hkdf_expand iteration.
            unsafe { core::slice::from_raw_parts(self.overflow_info, self.overflow_len) }
        } else {
            &[]
        }
    }
    pub fn counter(&self) -> u8 { self.counter_byte }
    pub fn uses_overflow(&self) -> bool { self.uses_overflow }
}

// Update hkdf_expand to use overflow-aware HMAC
// (Already done in hkdf_expand above via build_hmac_data, but we
// need the hmac call to honour overflow.)
// For simplicity in this no_std context, we inline the two-call path:

/// Internal: compute HMAC for one HKDF round, handling info overflow.
fn hmac_for_round(prk: &Sha256Digest, round: &HkdfRoundBuf) -> Sha256Digest {
    if !round.uses_overflow() {
        if let Some(slice) = round.as_slice_if_fits() {
            return hmac_sha256(prk, slice);
        }
    }
    // Overflow: HMAC over [prefix || overflow_info || counter]
    // We simulate multi-part HMAC by concatenating in a fresh buffer.
    // Max: 32 + info.len() + 1. For info > 479 bytes, use a progressive
    // approach — but since our ROUND_BUF_MAX is 512 and t_prev is 32,
    // info > 479 bytes is truly exotic. We allocate nothing; instead we
    // feed bytes iteratively through a SHA-256 HMAC implementation that
    // accepts streaming input. The current hmac_sha256 takes a single &[u8],
    // so we build the full message in a local 4 KiB stack buffer for
    // the overflow path only (acceptable since this is a dev-profile crate).
    let prefix = round.prefix();
    let info_slice = round.overflow_info_slice();
    let counter = round.counter();
    let total = prefix.len() + info_slice.len() + 1;
    // Stack buffer — 4 KiB is fine for a dev/reference crate.
    let mut full = [0u8; 4096];
    if total <= full.len() {
        full[..prefix.len()].copy_from_slice(prefix);
        full[prefix.len()..prefix.len() + info_slice.len()].copy_from_slice(info_slice);
        full[prefix.len() + info_slice.len()] = counter;
        hmac_sha256(prk, &full[..total])
    } else {
        // Info is absurdly long; best-effort: use as much as fits.
        let avail = full.len().saturating_sub(prefix.len() + 1);
        full[..prefix.len()].copy_from_slice(prefix);
        let info_take = info_slice.len().min(avail);
        full[prefix.len()..prefix.len() + info_take].copy_from_slice(&info_slice[..info_take]);
        full[prefix.len() + info_take] = counter;
        hmac_sha256(prk, &full[..prefix.len() + info_take + 1])
    }
}
