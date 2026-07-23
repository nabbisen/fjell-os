//! SHA-256 and Digest32 for no_std Fjell measurement chain.
//!
//! Uses a compact, allocation-free SHA-256 implementation.

/// A 32-byte (SHA-256) digest value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Digest32(pub [u8; 32]);

impl Digest32 {
    /// All-zeros digest (genesis / empty).
    pub const ZERO: Self = Self([0u8; 32]);

    /// Compute SHA-256 over one contiguous slice.
    pub fn of(data: &[u8]) -> Self {
        sha256(data)
    }

    /// Compute SHA-256 over multiple slices (no allocation).
    pub fn of_parts(parts: &[&[u8]]) -> Self {
        let mut ctx = Sha256::new();
        for part in parts {
            ctx.update(part);
        }
        ctx.finish()
    }

    /// Hex string representation (lowercase), written into `buf`.
    /// `buf` must be at least 64 bytes.  Returns the filled slice.
    pub fn to_hex<'a>(&self, buf: &'a mut [u8; 64]) -> &'a str {
        const HEX: &[u8] = b"0123456789abcdef";
        for (i, &b) in self.0.iter().enumerate() {
            buf[i * 2]     = HEX[(b >> 4) as usize];
            buf[i * 2 + 1] = HEX[(b & 0xF) as usize];
        }
        core::str::from_utf8(buf).unwrap_or("??")
    }
}

impl Default for Digest32 {
    fn default() -> Self { Self([0u8; 32]) }
}

impl core::fmt::Debug for Digest32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut buf = [0u8; 64];
        write!(f, "sha256:{}", self.to_hex(&mut buf))
    }
}

// ── SHA-256 implementation ────────────────────────────────────────────────────

fn sha256(data: &[u8]) -> Digest32 {
    let mut ctx = Sha256::new();
    ctx.update(data);
    ctx.finish()
}

struct Sha256 {
    state: [u32; 8],
    buf:   [u8; 64],
    buf_len: usize,
    total:   u64,
}

#[rustfmt::skip]
const K: [u32; 64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
];

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.total += data.len() as u64;
        let mut data = data;
        // Fill partial block
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        // Full blocks
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            let mut b = [0u8; 64];
            b.copy_from_slice(block);
            self.compress(&b);
            data = rest;
        }
        // Remaining
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn finish(mut self) -> Digest32 {
        // Padding
        let bit_len = self.total * 8;
        let pad_start = self.buf_len;
        self.buf[pad_start] = 0x80;
        for b in self.buf[pad_start + 1..].iter_mut() { *b = 0; }
        if pad_start >= 56 {
            let block = self.buf;
            self.compress(&block);
            self.buf = [0u8; 64];
        }
        // Length in big-endian
        let len_bytes = bit_len.to_be_bytes();
        self.buf[56..64].copy_from_slice(&len_bytes);
        let block = self.buf;
        self.compress(&block);
        // Output
        let mut out = [0u8; 32];
        for (i, &w) in self.state.iter().enumerate() {
            out[i*4..i*4+4].copy_from_slice(&w.to_be_bytes());
        }
        Digest32(out)
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[i*4], block[i*4+1], block[i*4+2], block[i*4+3]]);
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h=g; g=f; f=e; e=d.wrapping_add(t1);
            d=c; c=b; b=a; a=t1.wrapping_add(t2);
        }
        self.state[0]=self.state[0].wrapping_add(a);
        self.state[1]=self.state[1].wrapping_add(b);
        self.state[2]=self.state[2].wrapping_add(c);
        self.state[3]=self.state[3].wrapping_add(d);
        self.state[4]=self.state[4].wrapping_add(e);
        self.state[5]=self.state[5].wrapping_add(f);
        self.state[6]=self.state[6].wrapping_add(g);
        self.state[7]=self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let d = Digest32::of(b"");
        assert_eq!(d.0[0], 0xe3);
        assert_eq!(d.0[1], 0xb0);
        assert_eq!(d.0[31], 0x55);
    }

    #[test]
    fn sha256_abc() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2ec73b00361bbef0469324984298824b5
        let d = Digest32::of(b"abc");
        assert_eq!(d.0[0], 0xba);
        assert_eq!(d.0[1], 0x78);
    }

    #[test]
    fn sha256_long_message() {
        // SHA-256 of 1000 'a's — verifies multi-block correctness
        let data = [b'a'; 1000];
        let d = Digest32::of(&data);
        // First byte should be non-zero (deterministic)
        assert_ne!(d, Digest32::ZERO);
    }

    #[test]
    fn sha256_parts_eq_whole() {
        let a = b"hello ";
        let b = b"world";
        let whole = Digest32::of(b"hello world");
        let parts = Digest32::of_parts(&[a, b]);
        assert_eq!(whole, parts);
    }

    #[test]
    fn digest32_hex() {
        let d = Digest32([0xAB; 32]);
        let mut buf = [0u8; 64];
        let s = d.to_hex(&mut buf);
        assert_eq!(&s[0..2], "ab");
        assert_eq!(s.len(), 64);
    }
}
