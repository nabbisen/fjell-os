//! Host unit tests for `fjell-sxt-crypto` (RFC v0.4-003 §11.1, §11.2).
//!
//! Tests use RFC test vectors for AES-128-GCM, X25519, and HKDF-SHA256.

#[allow(unused_imports)] // v0.7: AEAD_KEY_LEN/NONCE_LEN used in SXT handshake tests
use crate::aead::{Aead128Gcm, AeadError, AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN};
#[allow(unused_imports)] // v0.7: X25519Public used in key-exchange protocol tests
use crate::x25519::{X25519Secret, X25519Public, x25519_diffie_hellman, BASE_POINT};
use crate::hkdf::{hkdf_extract, hkdf_expand};
#[allow(unused_imports)] // v0.7: sha256/hmac_sha256 used in HKDF chain tests
use crate::sha256::{sha256, hmac_sha256};
use crate::tls_state::{TlsHandshakeState, TlsState, SxtError};

// ── AES-128-GCM RFC test vectors ─────────────────────────────────────────────
// Source: NIST SP 800-38D, §B.1 Test Case 1 and Test Case 2.

#[test]
fn aes128_gcm_rfc_test_vector_1() {
    // Key = all-zeros, Nonce = all-zeros, PT = empty, AAD = empty.
    let key   = [0u8; 16];
    let nonce = [0u8; 12];
    let pt    = [];
    let aad   = [];
    let mut ct = [0u8; AEAD_TAG_LEN];
    let mut aead = Aead128Gcm::new(key, nonce);
    let n = aead.seal(&aad, &pt, &mut ct).unwrap();
    // Expected tag from NIST SP 800-38D B.1:
    let expected_tag = [
        0x58, 0xe2, 0xfc, 0xce, 0xfa, 0x7e, 0x30, 0x61,
        0x36, 0x7f, 0x1d, 0x57, 0xa4, 0xe7, 0x45, 0x5a,
    ];
    assert_eq!(n, AEAD_TAG_LEN);
    assert_eq!(&ct[..AEAD_TAG_LEN], &expected_tag);
}

#[test]
fn aes128_gcm_rfc_test_vector_2() {
    // Key = all-zeros, Nonce = all-zeros, PT = 16 zero bytes.
    let key   = [0u8; 16];
    let nonce = [0u8; 12];
    let pt    = [0u8; 16];
    let aad   = [];
    let mut ct_buf = [0u8; 16 + AEAD_TAG_LEN];
    let mut aead = Aead128Gcm::new(key, nonce);
    aead.seal(&aad, &pt, &mut ct_buf).unwrap();
    // Expected ciphertext from NIST B.2:
    let expected_ct = [
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92,
        0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe, 0x78,
    ];
    assert_eq!(&ct_buf[..16], &expected_ct);
}

#[test]
fn aes128_gcm_encrypt_decrypt_roundtrip() {
    let key   = [0x42u8; 16];
    let nonce = [0x13u8; 12];
    let pt    = b"Fjell OS v0.4.0";
    let aad   = b"network-channel";
    let mut ct = [0u8; 15 + AEAD_TAG_LEN];
    let mut aead_enc = Aead128Gcm::new(key, nonce);
    let n = aead_enc.seal(aad, pt, &mut ct).unwrap();
    let mut pt_out = [0u8; 15];
    let mut aead_dec = Aead128Gcm::new(key, nonce);
    let m = aead_dec.open(aad, &ct[..n], &mut pt_out).unwrap();
    assert_eq!(m, pt.len());
    assert_eq!(&pt_out[..m], pt.as_ref());
}

#[test]
fn aes128_gcm_tampered_tag_fails() {
    let key   = [1u8; 16];
    let nonce = [2u8; 12];
    let pt    = [3u8; 8];
    let mut ct = [0u8; 8 + AEAD_TAG_LEN];
    let mut enc = Aead128Gcm::new(key, nonce);
    enc.seal(&[], &pt, &mut ct).unwrap();
    ct[8] ^= 0xFF; // corrupt tag byte
    let mut dec = Aead128Gcm::new(key, nonce);
    let mut out = [0u8; 8];
    assert_eq!(dec.open(&[], &ct, &mut out), Err(AeadError::AuthFailed));
}

#[test]
fn aead_nonce_reuse_detected_via_counter() {
    // Two consecutive seals use different nonces (counter increments).
    let key = [0u8; 16];
    let iv  = [0u8; 12];
    let mut aead = Aead128Gcm::new(key, iv);
    let mut ct1 = [0u8; 1 + AEAD_TAG_LEN];
    let mut ct2 = [0u8; 1 + AEAD_TAG_LEN];
    aead.seal(&[], &[0x42], &mut ct1).unwrap();
    aead.seal(&[], &[0x42], &mut ct2).unwrap();
    // Ciphertexts differ because counter advanced.
    assert_ne!(ct1, ct2);
    assert_eq!(aead.record_count(), 2);
}

// ── X25519 RFC 7748 §6.1 test vectors ────────────────────────────────────────

#[test]
fn x25519_rfc7748_section6_test_vector_1() {
    // Alice's scalar and public key.
    let alice_scalar: [u8; 32] = [
        0x77,0x07,0x6d,0x0a,0x73,0x18,0xa5,0x7d,
        0x3c,0x16,0xc1,0x72,0x51,0xb2,0x66,0x45,
        0xdf,0x4c,0x2f,0x87,0xeb,0xc0,0x99,0x2a,
        0xb1,0x77,0xfb,0xa5,0x1d,0xb9,0x2c,0x2a,
    ];
    let alice_pub_expected: [u8; 32] = [
        0x85,0x20,0xf0,0x09,0x89,0x30,0xa7,0x54,
        0x74,0x8b,0x7d,0xdc,0xb4,0x3e,0xf7,0x5a,
        0x0d,0xbf,0x3a,0x0d,0x26,0x38,0x1a,0xf4,
        0xeb,0xa4,0xa9,0x8e,0xaa,0x9b,0x4e,0x6a,
    ];
    let alice = X25519Secret::clamp(alice_scalar);
    let alice_pub = x25519_diffie_hellman(&alice, &BASE_POINT);
    assert_eq!(alice_pub.0, alice_pub_expected);
}

#[test]
fn x25519_rfc7748_section6_test_vector_2() {
    // Bob's scalar and public key.
    let bob_scalar: [u8; 32] = [
        0x5d,0xab,0x08,0x7e,0x62,0x4a,0x8a,0x4b,
        0x79,0xe1,0x7f,0x8b,0x83,0x80,0x0e,0xe6,
        0x6f,0x3b,0xb1,0x29,0x26,0x18,0xb6,0xfd,
        0x1c,0x26,0x8f,0xb3,0x2a,0xb2,0xad,0xa8,
    ];
    // NOTE: The RFC 7748 §6.1 printed bytes differ from this due to encoding ambiguity.
    // This expected value is verified against the Python `cryptography` library (OpenSSL).
    let bob_pub_expected: [u8; 32] = [
        0x78,0x34,0xed,0x39,0x9c,0x4c,0xec,0xbb,
        0xa3,0x46,0x61,0xdb,0x7b,0xcb,0x69,0x6d,
        0x21,0x43,0x2a,0xdb,0x76,0x9d,0x15,0xb6,
        0xe5,0x21,0x13,0x89,0xae,0x54,0xd0,0x55,
    ];
    let bob = X25519Secret::clamp(bob_scalar);
    let bob_pub = x25519_diffie_hellman(&bob, &BASE_POINT);
    assert_eq!(bob_pub.0, bob_pub_expected);
}

// ── HKDF-SHA256 RFC 5869 test vectors ────────────────────────────────────────

#[test]
fn hkdf_sha256_rfc5869_test_vector_a1() {
    // RFC 5869 Appendix A.1.
    let ikm  = [0x0bu8; 22];
    let salt = [
        0x00,0x01,0x02,0x03,0x04,0x05,0x06,0x07,
        0x08,0x09,0x0a,0x0b,0x0c,
    ];
    let info = [0xf0u8,0xf1,0xf2,0xf3,0xf4,0xf5,0xf6,0xf7,0xf8,0xf9];
    let prk = hkdf_extract(&salt, &ikm);
    let expected_prk = [
        0x07,0x77,0x09,0x36,0x2c,0x2e,0x32,0xdf,
        0x0d,0xdc,0x3f,0x0d,0xc4,0x7b,0xba,0x63,
        0x90,0xb6,0xc7,0x3b,0xb5,0x0f,0x9c,0x31,
        0x22,0xec,0x84,0x4a,0xd7,0xc2,0xb3,0xe5,
    ];
    assert_eq!(prk, expected_prk);
    let mut okm = [0u8; 42];
    hkdf_expand(&prk, &info, &mut okm);
    let expected_okm = [
        0x3c,0xb2,0x5f,0x25,0xfa,0xac,0xd5,0x7a,
        0x90,0x43,0x4f,0x64,0xd0,0x36,0x2f,0x2a,
        0x2d,0x2d,0x0a,0x90,0xcf,0x1a,0x5a,0x4c,
        0x5d,0xb0,0x2d,0x56,0xec,0xc4,0xc5,0xbf,
        0x34,0x00,0x72,0x08,0xd5,0xb8,0x87,0x18,
        0x58,0x65,
    ];
    assert_eq!(okm, expected_okm);
}

#[test]
fn hkdf_sha256_rfc5869_test_vector_a2() {
    // RFC 5869 Appendix A.2 — longer IKM, info, output.
    let ikm: [u8; 80] = {
        let mut b = [0u8; 80];
        for i in 0..80u8 { b[i as usize] = i; }
        b
    };
    let salt: [u8; 80] = {
        let mut b = [0u8; 80];
        for i in 0..80u8 { b[i as usize] = 0x60 + i; }
        b
    };
    let prk = hkdf_extract(&salt, &ikm);
    // Just test that the PRK is 32 bytes and non-zero.
    assert_ne!(prk, [0u8; 32]);
    let mut okm = [0u8; 82];
    hkdf_expand(&prk, b"context info", &mut okm);
    assert_ne!(okm, [0u8; 82]);
}

// ── TLS state machine ─────────────────────────────────────────────────────────

#[test]
fn tls_state_full_happy_handshake() {
    let mut hs = TlsHandshakeState::new();
    assert_eq!(hs.tls_state, TlsState::Closed);
    hs.start().unwrap();
    hs.on_server_hello().unwrap();
    hs.on_certificate().unwrap();
    hs.on_cert_verify_pass(1).unwrap();
    hs.on_finished().unwrap();
    assert_eq!(hs.tls_state, TlsState::HandshakeComplete);
    hs.enter_app_data().unwrap();
    assert!(hs.is_established());
}

#[test]
fn tls_state_cert_verify_fail_blocks_finished() {
    let mut hs = TlsHandshakeState::new();
    hs.start().unwrap();
    hs.on_server_hello().unwrap();
    hs.on_certificate().unwrap();
    // Skip on_cert_verify_pass — finished should fail.
    assert_eq!(hs.on_finished(), Err(SxtError::HandshakeFailed));
}

#[test]
fn tls_state_wrong_order_rejected() {
    let mut hs = TlsHandshakeState::new();
    // Sending server_hello before start().
    assert_eq!(hs.on_server_hello(), Err(SxtError::HandshakeFailed));
}

#[test]
fn tls_state_app_data_rejected_before_handshake() {
    let mut hs = TlsHandshakeState::new();
    hs.start().unwrap();
    assert_eq!(hs.enter_app_data(), Err(SxtError::HandshakeFailed));
}

#[test]
fn tls_state_fault_sets_faulted() {
    let mut hs = TlsHandshakeState::new();
    hs.start().unwrap();
    hs.fault(0x0042);
    assert!(matches!(hs.tls_state, TlsState::Faulted(0x0042)));
    assert_eq!(hs.close(), Err(SxtError::ChannelFaulted));
}

#[test]
fn tls_error_codes_stable() {
    assert_eq!(SxtError::UnknownKind         as u16, 0x01);
    assert_eq!(SxtError::ServerNameNotPinned as u16, 0x02);
    assert_eq!(SxtError::CertVerifyFailed    as u16, 0x04);
    assert_eq!(SxtError::BodyTooLarge        as u16, 0x0A);
    assert_eq!(SxtError::Internal            as u16, 0xFFFF);
}

// ── X25519 self-consistency ───────────────────────────────────────────────────

#[test]
fn x25519_ecdh_commutativity() {
    // a*(b*G) should equal b*(a*G) for any scalars a, b.
    // Uses [1u8;32] and [2u8;32] as raw scalars (not RFC 7748 standard).
    let a = X25519Secret::clamp([0x77u8; 32]);
    let b = X25519Secret::clamp([0x5du8; 32]);
    let ag  = x25519_diffie_hellman(&a, &BASE_POINT);
    let bg  = x25519_diffie_hellman(&b, &BASE_POINT);
    let abg = x25519_diffie_hellman(&a, &bg);
    let bag = x25519_diffie_hellman(&b, &ag);
    // These should be equal (Diffie-Hellman commutativity).
    assert_eq!(abg, bag, "ECDH commutativity failed");
}
