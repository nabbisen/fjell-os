//! GHASH and GCM counter mode (used by AES-128-GCM).

use crate::aes128::aes128_encrypt_block;

pub const GCM_BLOCK_LEN: usize = 16;
pub const GCM_TAG_LEN:   usize = 16;

// ── GHASH ─────────────────────────────────────────────────────────────────────

fn ghash_mul(x: &mut [u8; 16], y: &[u8; 16]) {
    let mut z = [0u8; 16];
    let mut v = *y;
    for i in 0..128 {
        if (x[i / 8] >> (7 - (i % 8))) & 1 != 0 {
            for j in 0..16 { z[j] ^= v[j]; }
        }
        let lsb = v[15] & 1;
        // right-shift v by 1
        let mut new_v = [0u8; 16];
        for j in 0..15 { new_v[j + 1] = v[j]; new_v[j + 1] |= (v[j + 1] >> 7) << 7; }
        for j in (1..16).rev() { new_v[j] = (v[j] >> 1) | ((v[j - 1] & 1) << 7); }
        new_v[0] = v[0] >> 1;
        v = new_v;
        if lsb != 0 { v[0] ^= 0xe1; }
    }
    *x = z;
}

/// Compute GHASH over authenticated data and ciphertext.
///
/// `h` is AES_K(0) — the hash subkey.
pub fn ghash(h: &[u8; 16], aad: &[u8], ct: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];

    fn process_block(y: &mut [u8; 16], h: &[u8; 16], block: &[u8]) {
        let mut xi = [0u8; 16];
        let n = block.len().min(16);
        xi[..n].copy_from_slice(&block[..n]);
        for i in 0..16 { y[i] ^= xi[i]; }
        ghash_mul(y, h);
    }

    for chunk in aad.chunks(16)  { process_block(&mut y, h, chunk); }
    for chunk in ct.chunks(16)   { process_block(&mut y, h, chunk); }

    // Encode bit lengths as big-endian u64 pairs.
    let aad_bits = (aad.len() as u64).wrapping_mul(8);
    let ct_bits  = (ct .len() as u64).wrapping_mul(8);
    let mut len_block = [0u8; 16];
    len_block[..8] .copy_from_slice(&aad_bits.to_be_bytes());
    len_block[8..] .copy_from_slice(&ct_bits .to_be_bytes());
    for i in 0..16 { y[i] ^= len_block[i]; }
    ghash_mul(&mut y, h);
    y
}

// ── CTR32 ─────────────────────────────────────────────────────────────────────

/// Increment the 32-bit counter in the last 4 bytes of the IV block.
fn incr32(block: &mut [u8; 16]) {
    let ctr = u32::from_be_bytes([block[12], block[13], block[14], block[15]])
        .wrapping_add(1);
    block[12..].copy_from_slice(&ctr.to_be_bytes());
}

/// AES-128-GCM encrypt.  Returns the ciphertext followed by the 16-byte tag.
///
/// `nonce` must be exactly 12 bytes.
pub fn gcm_encrypt(
    key:   &[u8; 16],
    nonce: &[u8; 12],
    aad:   &[u8],
    pt:    &[u8],
    ct:    &mut [u8],   // must be >= pt.len()
) -> [u8; GCM_TAG_LEN] {
    // Compute H = AES_K(0)
    let h = aes128_encrypt_block(&[0u8; 16], key);

    // J0 = nonce || 0x00000001
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    // Encrypt
    let mut ctr = j0;
    incr32(&mut ctr);
    for (i, chunk) in pt.chunks(16).enumerate() {
        let ks = aes128_encrypt_block(&ctr, key);
        let start = i * 16;
        for (j, &b) in chunk.iter().enumerate() {
            ct[start + j] = b ^ ks[j];
        }
        incr32(&mut ctr);
    }

    // Tag = GHASH(H, aad, ct) XOR AES_K(J0)
    let raw_tag = ghash(&h, aad, &ct[..pt.len()]);
    let ks0 = aes128_encrypt_block(&j0, key);
    let mut tag = [0u8; 16];
    for i in 0..16 { tag[i] = raw_tag[i] ^ ks0[i]; }
    tag
}

/// AES-128-GCM decrypt.  Returns `true` if the tag is valid.
///
/// Writes plaintext to `pt` only if the tag is valid (constant-time verify).
pub fn gcm_decrypt(
    key:   &[u8; 16],
    nonce: &[u8; 12],
    aad:   &[u8],
    ct:    &[u8],
    pt:    &mut [u8],
    tag:   &[u8; GCM_TAG_LEN],
) -> bool {
    let h = aes128_encrypt_block(&[0u8; 16], key);
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    // Verify tag first (constant-time).
    let raw_tag = ghash(&h, aad, ct);
    let ks0 = aes128_encrypt_block(&j0, key);
    let mut expected_tag = [0u8; 16];
    for i in 0..16 { expected_tag[i] = raw_tag[i] ^ ks0[i]; }
    let mut diff = 0u8;
    for i in 0..16 { diff |= expected_tag[i] ^ tag[i]; }
    if diff != 0 { return false; }

    // Decrypt.
    let mut ctr = j0;
    incr32(&mut ctr);
    for (i, chunk) in ct.chunks(16).enumerate() {
        let ks = aes128_encrypt_block(&ctr, key);
        let start = i * 16;
        for (j, &b) in chunk.iter().enumerate() {
            pt[start + j] = b ^ ks[j];
        }
        incr32(&mut ctr);
    }
    true
}
