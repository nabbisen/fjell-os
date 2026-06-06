//! X25519 Diffie-Hellman key exchange (RFC 7748 §5).
//!
//! Uses a 5×51-bit limb representation with u128 intermediate arithmetic.
//! Constant-time Montgomery ladder.

/// 32-byte X25519 scalar (private key).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct X25519Secret(pub [u8; 32]);

/// 32-byte X25519 u-coordinate (public key or shared secret).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct X25519Public(pub [u8; 32]);

/// The canonical X25519 base point u = 9.
pub const BASE_POINT: X25519Public = X25519Public({
    let mut b = [0u8; 32];
    b[0] = 9;
    b
});

impl X25519Secret {
    /// Clamp a scalar per RFC 7748 §5.
    pub fn clamp(mut bytes: [u8; 32]) -> Self {
        bytes[0]  &= 248;
        bytes[31] &= 127;
        bytes[31] |= 64;
        Self(bytes)
    }
}

// ── GF(2^255-19) using 5 × 51-bit limbs with u128 intermediates ──────────────

type Fe = [u64; 5];
pub(crate) const MASK51: u64 = (1u64 << 51) - 1;
pub(crate) const A24: u64 = 121665;

pub(crate) fn fe_zero() -> Fe { [0u64; 5] }
pub(crate) fn fe_one()  -> Fe { [1u64, 0, 0, 0, 0] }

pub(crate) fn fe_from_bytes(b: &[u8; 32]) -> Fe {
    // Load 256 bits as little-endian u64 words, then extract 5×51-bit limbs.
    let mut buf = [0u64; 4];
    for i in 0..4 { buf[i] = u64::from_le_bytes(b[i*8..i*8+8].try_into().unwrap()); }
    // Clear bit 255 (sign bit per RFC 7748 §5).
    buf[3] &= (1u64 << 63) - 1;
    let mut h = [0u64; 5];
    h[0] =  buf[0]                                                     & MASK51;
    h[1] = (buf[0] >> 51 | buf[1] << 13)                              & MASK51;
    h[2] = (buf[1] >> 38 | buf[2] << 26)                              & MASK51;
    h[3] = (buf[2] >> 25 | buf[3] << 39)                              & MASK51;
    h[4] = (buf[3] >> 12)                                              & MASK51;
    h
}

pub(crate) fn fe_to_bytes(h: &Fe) -> [u8; 32] {
    // Reduce h mod p = 2^255 - 19.
    let mut f = *h;
    // Propagate carries.
    for _ in 0..2 {
        for i in 0..5 {
            let c = f[i] >> 51;
            f[i] &= MASK51;
            f[(i + 1) % 5] = f[(i + 1) % 5].wrapping_add(if i < 4 { c } else { c * 19 });
        }
    }
    // Conditional subtraction of p if f >= p.
    let p = [MASK51 - 18, MASK51, MASK51, MASK51, MASK51];
    let mut borrow = 0i64;
    let mut t = [0u64; 5];
    for i in 0..5 {
        let diff = (f[i] as i64) - (p[i] as i64) + borrow;
        borrow = diff >> 63;
        t[i] = (diff & MASK51 as i64) as u64;
    }
    // If borrow == 0, use t; else keep f (constant-time select not needed here
    // since this is the output path, not a secret-dependent branch in the ladder).
    let out = if borrow == 0 { t } else { f };
    // Pack 5×51-bit into 4×64-bit then 32 bytes.
    let mut buf = [0u64; 4];
    buf[0] = out[0] | out[1] << 51;
    buf[1] = out[1] >> 13 | out[2] << 38;
    buf[2] = out[2] >> 26 | out[3] << 25;
    buf[3] = out[3] >> 39 | out[4] << 12;
    let mut b = [0u8; 32];
    for i in 0..4 { b[i*8..i*8+8].copy_from_slice(&buf[i].to_le_bytes()); }
    b
}

pub(crate) fn fe_add(a: &Fe, b: &Fe) -> Fe {
    [a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1]),
     a[2].wrapping_add(b[2]), a[3].wrapping_add(b[3]),
     a[4].wrapping_add(b[4])]
}

pub(crate) fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    // Add 2p to avoid underflow.
    let two_p0 = 2 * (MASK51 - 18);
    let two_p  = 2 * MASK51;
    [a[0].wrapping_add(two_p0).wrapping_sub(b[0]),
     a[1].wrapping_add(two_p  ).wrapping_sub(b[1]),
     a[2].wrapping_add(two_p  ).wrapping_sub(b[2]),
     a[3].wrapping_add(two_p  ).wrapping_sub(b[3]),
     a[4].wrapping_add(two_p  ).wrapping_sub(b[4])]
}

pub(crate) fn fe_reduce(h: &mut Fe) {
    // Two full carry sweeps to fully normalise.
    for _ in 0..2 {
        let c = h[0] >> 51; h[0] &= MASK51; h[1] = h[1].wrapping_add(c);
        let c = h[1] >> 51; h[1] &= MASK51; h[2] = h[2].wrapping_add(c);
        let c = h[2] >> 51; h[2] &= MASK51; h[3] = h[3].wrapping_add(c);
        let c = h[3] >> 51; h[3] &= MASK51; h[4] = h[4].wrapping_add(c);
        let c = h[4] >> 51; h[4] &= MASK51; h[0] = h[0].wrapping_add(c.wrapping_mul(19));
    }
}

pub(crate) fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let b19 = [b[0]*19, b[1]*19, b[2]*19, b[3]*19, b[4]*19];
    // Each ai is ≤ 2^51; each bj is ≤ 2^51; products ≤ 2^102; sums ≤ 5 × 2^102 < 2^105 < 2^128.
    let mut t = [0u128; 5];
    for i in 0..5 {
        for j in 0..5 {
            // h[k] += a[i] * b[j]  where k = (i+j) mod 5, with b shifted by 19 for wrapped terms.
            let k = (i + j) % 5;
            let use_b = if i + j < 5 { b[j] } else { b19[j] };
            t[k] = t[k].wrapping_add(a[i] as u128 * use_b as u128);
        }
    }
    let mut h = [0u64; 5];
    let mask = MASK51 as u128;
    let mut carry: u128 = 0;
    for i in 0..5 {
        let v = t[i] + carry;
        h[i] = (v & mask) as u64;
        carry = v >> 51;
    }
    h[0] = h[0].wrapping_add((carry as u64).wrapping_mul(19));
    fe_reduce(&mut h);
    h
}

pub(crate) fn fe_sq(a: &Fe) -> Fe { fe_mul(a, a) }

pub(crate) fn fe_mul_a24(a: &Fe) -> Fe {
    // Multiply by 121665 (= (A-2)/4 for curve25519).
    let mut t = [0u128; 5];
    for i in 0..5 { t[i] = a[i] as u128 * A24 as u128; }
    let mut h = [0u64; 5];
    let mask = MASK51 as u128;
    let mut carry: u128 = 0;
    for i in 0..5 {
        let v = t[i] + carry;
        h[i] = (v & mask) as u64;
        carry = v >> 51;
    }
    h[0] = h[0].wrapping_add((carry * 19) as u64);
    let c = h[0] >> 51; h[0] &= MASK51; h[1] = h[1].wrapping_add(c);
    h
}

pub(crate) fn fe_invert(z: &Fe) -> Fe {
    // z^(2^255-21) = z^(p-2) — standard djb addition chain.
    let z2      = fe_sq(z);
    let z4      = fe_sq(&z2);
    let z8      = fe_sq(&z4);
    let z9      = fe_mul(z, &z8);
    let z11     = fe_mul(&z2, &z9);
    let z22     = fe_sq(&z11);
    let z_5_0   = fe_mul(&z9, &z22);                                           // z^31
    let z_10_0  = { let mut t=z_5_0;  for _ in 0.. 5 { t=fe_sq(&t); } fe_mul(&t,&z_5_0)  };
    let z_20_0  = { let mut t=z_10_0; for _ in 0..10 { t=fe_sq(&t); } fe_mul(&t,&z_10_0) };
    let z_40_0  = { let mut t=z_20_0; for _ in 0..20 { t=fe_sq(&t); } fe_mul(&t,&z_20_0) };
    let z_50_0  = { let mut t=z_40_0; for _ in 0..10 { t=fe_sq(&t); } fe_mul(&t,&z_10_0) };
    let z_100_0 = { let mut t=z_50_0; for _ in 0..50 { t=fe_sq(&t); } fe_mul(&t,&z_50_0) };
    let z_200_0 = { let mut t=z_100_0;for _ in 0..100{ t=fe_sq(&t); } fe_mul(&t,&z_100_0)};
    let z_250_0 = { let mut t=z_200_0;for _ in 0..50 { t=fe_sq(&t); } fe_mul(&t,&z_50_0) };
    let z_255_5 = { let mut t=z_250_0; for _ in 0..5 { t=fe_sq(&t); } t };    // z^(2^255-32)
    fe_mul(&z_255_5, &z11)                                                      // z^(2^255-21)
}

pub(crate) fn cswap(swap: u64, a: &mut Fe, b: &mut Fe) {
    let mask = 0u64.wrapping_sub(swap); // all-1s if swap != 0
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

pub(crate) fn ladder(u: &[u8; 32], k: &[u8; 32]) -> [u8; 32] {
    let x1    = fe_from_bytes(u);
    let mut x2 = fe_one();
    let mut z2 = fe_zero();
    let mut x3 = x1;
    let mut z3 = fe_one();

    let mut swap = 0u64;
    for i in (0..255).rev() {
        let bit = ((k[i / 8] >> (i % 8)) & 1) as u64;
        swap ^= bit;
        cswap(swap, &mut x2, &mut x3);
        cswap(swap, &mut z2, &mut z3);
        swap = bit;

        let a   = fe_add(&x2, &z2);
        let aa  = fe_sq(&a);
        let b   = fe_sub(&x2, &z2);
        let bb  = fe_sq(&b);
        let e   = fe_sub(&aa, &bb);
        let c   = fe_add(&x3, &z3);
        let d   = fe_sub(&x3, &z3);
        let da  = fe_mul(&d, &a);
        let cb  = fe_mul(&c, &b);
        let sum = fe_add(&da, &cb);
        let dif = fe_sub(&da, &cb);
        x3 = fe_sq(&sum);
        z3 = fe_mul(&x1, &fe_sq(&dif));
        x2 = fe_mul(&aa, &bb);
        z2 = fe_mul(&e, &fe_add(&aa, &fe_mul_a24(&e)));
    }
    cswap(swap, &mut x2, &mut x3);
    cswap(swap, &mut z2, &mut z3);

    let r = fe_mul(&x2, &fe_invert(&z2));
    fe_to_bytes(&r)
}

/// Perform X25519 scalar multiplication.
pub fn x25519_diffie_hellman(scalar: &X25519Secret, point: &X25519Public) -> X25519Public {
    let mut k = scalar.0;
    // Clamp (idempotent if already clamped).
    k[0]  &= 248;
    k[31] &= 127;
    k[31] |= 64;
    let mut u = point.0;
    u[31] &= 127; // clear sign bit on u-coordinate
    X25519Public(ladder(&u, &k))
}
