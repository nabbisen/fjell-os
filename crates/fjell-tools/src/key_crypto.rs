//! Encrypted signing-key storage — RFC-v0.16-006 (closes errata E-002).
//!
//! Replaces the plaintext `FJKY` key file with an encrypted `FJK2` format
//! whose 32-byte Ed25519 seed is sealed under AES-256-GCM, with the key
//! derived from an operator passphrase via Argon2id.
//!
//! ## `FJK2` wire format (113 bytes)
//!
//! ```text
//! magic:      [u8; 4]   "FJK2"
//! version:    u8        = 2
//! kdf_id:     u8        = 1   (Argon2id, default params)
//! salt:       [u8; 16]        (Argon2 salt)
//! nonce:      [u8; 12]        (AES-256-GCM nonce)
//! pubkey:     [u8; 32]        (public; stored cleartext for `key show`)
//! ct_and_tag: [u8; 48]        (AES-256-GCM of the 32-byte seed + 16 tag)
//! ```
//!
//! ## Passphrase source
//!
//! In order of precedence:
//! 1. `--passphrase <s>` flag (discouraged; visible in process args)
//! 2. `FJELL_KEY_PASSPHRASE` environment variable (CI / automation)
//!
//! Interactive TTY prompting is intentionally out of scope for the host
//! tool; operators wire the env var from their own secret store.
//!
//! ## Plaintext escape hatch
//!
//! The legacy plaintext `FJKY` format remains writable only behind an
//! explicit `--insecure-plaintext` flag, used for CI fixtures that must
//! not carry a passphrase. Reading transparently supports both formats.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;

pub const FJK2_MAGIC: &[u8; 4] = b"FJK2";
pub const FJK2_VERSION: u8 = 2;
pub const KDF_ARGON2ID: u8 = 1;
pub const FJK2_LEN: usize = 4 + 1 + 1 + 16 + 12 + 32 + 48; // 114

/// Errors from the encrypted key path.
#[derive(Debug)]
pub enum KeyCryptoError {
    NoPassphrase,
    BadFormat(&'static str),
    KdfFailed,
    DecryptFailed,
    EncryptFailed,
}

impl core::fmt::Display for KeyCryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            Self::NoPassphrase  => "no passphrase: set FJELL_KEY_PASSPHRASE or pass --passphrase",
            Self::BadFormat(m)  => return write!(f, "bad key format: {}", m),
            Self::KdfFailed     => "Argon2id key derivation failed",
            Self::DecryptFailed => "decryption failed (wrong passphrase or corrupted key)",
            Self::EncryptFailed => "encryption failed",
        };
        write!(f, "{}", s)
    }
}

/// Derive a 32-byte AES key from a passphrase and salt using Argon2id.
fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<[u8; 32], KeyCryptoError> {
    let argon = Argon2::default(); // Argon2id, v0x13, sane defaults
    let mut out = [0u8; 32];
    argon.hash_password_into(passphrase, salt, &mut out)
        .map_err(|_| KeyCryptoError::KdfFailed)?;
    Ok(out)
}

/// Encrypt a seed into the `FJK2` format.
pub fn encrypt_key(
    seed: &[u8; 32],
    pubkey: &[u8; 32],
    passphrase: &[u8],
    salt: &[u8; 16],
    nonce: &[u8; 12],
) -> Result<Vec<u8>, KeyCryptoError> {
    let aes_key = derive_key(passphrase, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|_| KeyCryptoError::EncryptFailed)?;
    let ct = cipher.encrypt(Nonce::from_slice(nonce), seed.as_slice())
        .map_err(|_| KeyCryptoError::EncryptFailed)?;
    // ct = ciphertext(32) + tag(16) = 48 bytes
    if ct.len() != 48 {
        return Err(KeyCryptoError::EncryptFailed);
    }

    let mut out = Vec::with_capacity(FJK2_LEN);
    out.extend_from_slice(FJK2_MAGIC);
    out.push(FJK2_VERSION);
    out.push(KDF_ARGON2ID);
    out.extend_from_slice(salt);
    out.extend_from_slice(nonce);
    out.extend_from_slice(pubkey);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt an `FJK2` key file, returning (seed, pubkey).
pub fn decrypt_key(
    bytes: &[u8],
    passphrase: &[u8],
) -> Result<([u8; 32], [u8; 32]), KeyCryptoError> {
    if bytes.len() != FJK2_LEN {
        return Err(KeyCryptoError::BadFormat("wrong length"));
    }
    if &bytes[0..4] != FJK2_MAGIC {
        return Err(KeyCryptoError::BadFormat("bad magic"));
    }
    if bytes[4] != FJK2_VERSION {
        return Err(KeyCryptoError::BadFormat("unsupported version"));
    }
    if bytes[5] != KDF_ARGON2ID {
        return Err(KeyCryptoError::BadFormat("unsupported kdf"));
    }
    let salt: [u8; 16]   = bytes[6..22].try_into().unwrap();
    let nonce: [u8; 12]  = bytes[22..34].try_into().unwrap();
    let pubkey: [u8; 32] = bytes[34..66].try_into().unwrap();
    let ct = &bytes[66..114];

    let aes_key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|_| KeyCryptoError::DecryptFailed)?;
    let pt = cipher.decrypt(Nonce::from_slice(&nonce), ct)
        .map_err(|_| KeyCryptoError::DecryptFailed)?;
    if pt.len() != 32 {
        return Err(KeyCryptoError::DecryptFailed);
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&pt);
    Ok((seed, pubkey))
}

/// Resolve the passphrase from `--passphrase` or `FJELL_KEY_PASSPHRASE`.
pub fn resolve_passphrase(args: &[String]) -> Option<Vec<u8>> {
    if let Some(p) = args.windows(2).find(|w| w[0] == "--passphrase").map(|w| w[1].clone()) {
        return Some(p.into_bytes());
    }
    std::env::var("FJELL_KEY_PASSPHRASE").ok().map(|s| s.into_bytes())
}

/// Is this byte buffer an encrypted `FJK2` file?
pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == FJK2_MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let seed = [0x11u8; 32];
        let pubkey = [0x22u8; 32];
        let pass = b"correct horse battery staple";
        let salt = [0x33u8; 16];
        let nonce = [0x44u8; 12];

        let enc = encrypt_key(&seed, &pubkey, pass, &salt, &nonce).unwrap();
        assert_eq!(enc.len(), FJK2_LEN);
        assert!(is_encrypted(&enc));

        let (dec_seed, dec_pub) = decrypt_key(&enc, pass).unwrap();
        assert_eq!(dec_seed, seed);
        assert_eq!(dec_pub, pubkey);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let seed = [0x11u8; 32];
        let pubkey = [0x22u8; 32];
        let salt = [0x33u8; 16];
        let nonce = [0x44u8; 12];
        let enc = encrypt_key(&seed, &pubkey, b"right", &salt, &nonce).unwrap();
        let result = decrypt_key(&enc, b"wrong");
        assert!(matches!(result, Err(KeyCryptoError::DecryptFailed)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let seed = [0x11u8; 32];
        let pubkey = [0x22u8; 32];
        let salt = [0x33u8; 16];
        let nonce = [0x44u8; 12];
        let mut enc = encrypt_key(&seed, &pubkey, b"pass", &salt, &nonce).unwrap();
        enc[70] ^= 0xFF; // flip a ciphertext byte
        let result = decrypt_key(&enc, b"pass");
        assert!(result.is_err(), "GCM tag must catch tampering");
    }

    #[test]
    fn bad_magic_rejected() {
        let mut enc = encrypt_key(&[1u8;32], &[2u8;32], b"p", &[0u8;16], &[0u8;12]).unwrap();
        enc[0] = b'X';
        assert!(matches!(decrypt_key(&enc, b"p"), Err(KeyCryptoError::BadFormat(_))));
    }

    #[test]
    fn pubkey_readable_without_decrypt() {
        // The public key is stored cleartext at a fixed offset so `key show`
        // works without a passphrase.
        let pubkey = [0xABu8; 32];
        let enc = encrypt_key(&[1u8;32], &pubkey, b"p", &[0u8;16], &[0u8;12]).unwrap();
        assert_eq!(&enc[34..66], &pubkey[..]);
    }

    #[test]
    fn different_salt_different_ciphertext() {
        let seed = [0x11u8; 32];
        let pubkey = [0x22u8; 32];
        let e1 = encrypt_key(&seed, &pubkey, b"p", &[1u8;16], &[0u8;12]).unwrap();
        let e2 = encrypt_key(&seed, &pubkey, b"p", &[2u8;16], &[0u8;12]).unwrap();
        // Ciphertext region must differ when salt differs
        assert_ne!(&e1[66..], &e2[66..]);
    }
}
