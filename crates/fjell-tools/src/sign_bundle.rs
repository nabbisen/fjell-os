//! RFC-v0.11-003: bundle signing pipeline.
//!
//! Provides three xtask subcommands:
//!
//! `cargo xtask key gen   --out <file>`          — generate encrypted signing key
//! `cargo xtask key show  --in  <file>`          — print public key fingerprint
//! `cargo xtask sign-bundle --bundle <f> --key <k> --out <sig>`
//! `cargo xtask verify-bundle-sig --bundle <f> --sig <s> --pubkey <hex>`

use std::fs;
use std::path::Path;
use std::process::ExitCode;

// Domain separator for the signed message (RFC-v0.11-003 §2).
const SIG_DOMAIN: &[u8] = b"FJELL-BUNDLE-SIG-V1";

// Magic for the SignedManifest wire format.
const SIGMANIFEST_MAGIC: u32 = 0xFB_51_53_01; // "FBQS1"
const SIGMANIFEST_SCHEMA: u16 = 1;
const SIG_ALG_ED25519: u8 = 1;

/// 88-byte on-wire signed manifest.
/// Layout: magic(4) schema(2) key_id(16) alg(1) reserved(1)
///         bundle_digest(32) signed_at_ns(8) signature(64) = 128 bytes.
pub const SIGMANIFEST_LEN: usize = 128;

// ── Public API ────────────────────────────────────────────────────────────────

pub fn cmd_key(sub: Option<&str>, args: &[String]) -> ExitCode {
    match sub {
        Some("gen")  => key_gen(args),
        Some("show") => key_show(args),
        _ => {
            eprintln!("Usage: cargo xtask key <gen|show> [options]");
            ExitCode::FAILURE
        }
    }
}

pub fn cmd_sign_bundle(args: &[String]) -> ExitCode {
    let bundle_path = flag(args, "--bundle");
    let key_path    = flag(args, "--key");
    let out_path    = flag(args, "--out");
    let key_id_hex  = flag(args, "--key-id");

    let (bundle_path, key_path, out_path) = match (bundle_path, key_path, out_path) {
        (Some(b), Some(k), Some(o)) => (b, k, o),
        _ => {
            eprintln!("Usage: cargo xtask sign-bundle --bundle <f> --key <k> --out <sig> [--key-id <hex>]");
            return ExitCode::FAILURE;
        }
    };

    let bundle_bytes = match fs::read(&bundle_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("sign-bundle: cannot read bundle: {}", e); return ExitCode::FAILURE; }
    };

    let (secret_seed, key_id) = match load_key(&key_path, &key_id_hex, args) {
        Ok(v) => v,
        Err(e) => { eprintln!("sign-bundle: {}", e); return ExitCode::FAILURE; }
    };

    // Compute bundle_digest
    let bundle_digest = fnv_digest_32(&bundle_bytes);

    // Build canonical message: SIG_DOMAIN || key_id || alg || bundle_digest || signed_at_ns
    let signed_at_ns: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let message = build_signed_message(&key_id, &bundle_digest, signed_at_ns);

    // Sign using Ed25519
    let signature = ed25519_sign(&secret_seed, &message);

    // Write SignedManifest
    let manifest = build_sigmanifest(&key_id, &bundle_digest, signed_at_ns, &signature);

    if let Some(parent) = Path::new(&out_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::write(&out_path, &manifest) {
        Ok(_) => {
            println!("sign-bundle: wrote {} bytes to {}", manifest.len(), out_path);
            println!("sign-bundle: bundle_digest = {}", hex(&bundle_digest));
            println!("sign-bundle: key_id        = {}", hex(&key_id));
            ExitCode::SUCCESS
        }
        Err(e) => { eprintln!("sign-bundle: write error: {}", e); ExitCode::FAILURE }
    }
}

pub fn cmd_verify_bundle_sig(args: &[String]) -> ExitCode {
    let bundle_path = flag(args, "--bundle");
    let sig_path    = flag(args, "--sig");
    let pubkey_hex  = flag(args, "--pubkey");

    let (bundle_path, sig_path, pubkey_hex) = match (bundle_path, sig_path, pubkey_hex) {
        (Some(b), Some(s), Some(p)) => (b, s, p),
        _ => {
            eprintln!("Usage: cargo xtask verify-bundle-sig --bundle <f> --sig <s> --pubkey <hex32>");
            return ExitCode::FAILURE;
        }
    };

    let bundle_bytes = match fs::read(&bundle_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("verify-bundle-sig: {}", e); return ExitCode::FAILURE; }
    };
    let sig_bytes = match fs::read(&sig_path) {
        Ok(b) => b,
        Err(e) => { eprintln!("verify-bundle-sig: {}", e); return ExitCode::FAILURE; }
    };

    let pubkey: [u8; 32] = match unhex32(&pubkey_hex) {
        Ok(b) => b,
        Err(e) => { eprintln!("verify-bundle-sig: --pubkey: {}", e); return ExitCode::FAILURE; }
    };

    match verify_sigmanifest(&bundle_bytes, &sig_bytes, &pubkey) {
        Ok(()) => { println!("verify-bundle-sig: PASS"); ExitCode::SUCCESS }
        Err(e) => { eprintln!("verify-bundle-sig: FAIL — {}", e); ExitCode::FAILURE }
    }
}

// ── Key generation and management ────────────────────────────────────────────

fn key_gen(args: &[String]) -> ExitCode {
    let out_path = match flag(args, "--out") {
        Some(p) => p,
        None => { eprintln!("key gen: --out <file> required"); return ExitCode::FAILURE; }
    };

    // Generate 32 random bytes as seed via getrandom
    let mut seed = [0u8; 32];
    match getrandom::getrandom(&mut seed) {
        Ok(_) => {}
        Err(e) => { eprintln!("key gen: getrandom failed: {}", e); return ExitCode::FAILURE; }
    };

    let pubkey = ed25519_pubkey(&seed);
    let key_id = key_id_from_pubkey(&pubkey);

    let insecure = args.iter().any(|a| a == "--insecure-plaintext");

    let key_file: Vec<u8> = if insecure {
        // Legacy plaintext FJKY format — only for CI fixtures.
        let mut f = Vec::with_capacity(68);
        f.extend_from_slice(b"FJKY");
        f.extend_from_slice(&seed);
        f.extend_from_slice(&pubkey);
        f
    } else {
        // Encrypted FJK2 format (RFC-v0.16-006). Requires a passphrase.
        let pass = match crate::key_crypto::resolve_passphrase(args) {
            Some(p) => p,
            None => {
                eprintln!("key gen: {}", crate::key_crypto::KeyCryptoError::NoPassphrase);
                eprintln!("key gen: (or pass --insecure-plaintext for an unencrypted CI fixture)");
                return ExitCode::FAILURE;
            }
        };
        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 12];
        if getrandom::getrandom(&mut salt).is_err() || getrandom::getrandom(&mut nonce).is_err() {
            eprintln!("key gen: getrandom failed for salt/nonce");
            return ExitCode::FAILURE;
        }
        match crate::key_crypto::encrypt_key(&seed, &pubkey, &pass, &salt, &nonce) {
            Ok(f) => f,
            Err(e) => { eprintln!("key gen: {}", e); return ExitCode::FAILURE; }
        }
    };

    if let Some(parent) = Path::new(&out_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::write(&out_path, &key_file) {
        Ok(_) => {
            println!("key gen: wrote key to {}", out_path);
            println!("key gen: public key = {}", hex(&pubkey));
            println!("key gen: key_id     = {}", hex(&key_id));
            if insecure {
                println!("key gen: WARNING: plaintext key (FJKY) — CI fixture only, do not deploy");
            } else {
                println!("key gen: encrypted at rest (FJK2, Argon2id + AES-256-GCM)");
            }
            ExitCode::SUCCESS
        }
        Err(e) => { eprintln!("key gen: write error: {}", e); ExitCode::FAILURE }
    }
}

fn key_show(args: &[String]) -> ExitCode {
    let in_path = match flag(args, "--in") {
        Some(p) => p,
        None => { eprintln!("key show: --in <file> required"); return ExitCode::FAILURE; }
    };
    // For `key show` we only need the public key, which is stored cleartext
    // in both formats — no passphrase required.
    let pubkey = match read_pubkey_only(&in_path) {
        Ok(v) => v,
        Err(e) => { eprintln!("key show: {}", e); return ExitCode::FAILURE; }
    };
    let key_id = key_id_from_pubkey(&pubkey);
    println!("public key : {}", hex(&pubkey));
    println!("key_id     : {}", hex(&key_id));
    ExitCode::SUCCESS
}

// ── Signing internals ─────────────────────────────────────────────────────────

fn load_key(path: &str, key_id_override: &Option<String>, args: &[String]) -> Result<([u8; 32], [u8; 16]), String> {
    let (seed, pubkey) = load_raw_key_with_args(path, args)?;
    let key_id = match key_id_override {
        Some(hex_str) => unhex16(hex_str)?,
        None => key_id_from_pubkey(&pubkey),
    };
    Ok((seed, key_id))
}

/// Read only the public key, which is stored cleartext in both the
/// encrypted `FJK2` format (offset 34) and the plaintext `FJKY` format
/// (offset 36). No passphrase required.
fn read_pubkey_only(path: &str) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read key file: {}", e))?;
    if crate::key_crypto::is_encrypted(&bytes) {
        if bytes.len() < 66 { return Err("truncated FJK2 key file".into()); }
        return Ok(bytes[34..66].try_into().unwrap());
    }
    if bytes.len() < 68 || &bytes[..4] != b"FJKY" {
        return Err("invalid key file format (neither FJK2 nor FJKY)".into());
    }
    Ok(bytes[36..68].try_into().unwrap())
}

/// Load a key file, decrypting if it is the encrypted `FJK2` format.
/// Passphrase is resolved from `args` (`--passphrase`) or the
/// `FJELL_KEY_PASSPHRASE` environment variable.
fn load_raw_key_with_args(path: &str, args: &[String]) -> Result<([u8; 32], [u8; 32]), String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read key file: {}", e))?;

    if crate::key_crypto::is_encrypted(&bytes) {
        let pass = crate::key_crypto::resolve_passphrase(args)
            .ok_or_else(|| crate::key_crypto::KeyCryptoError::NoPassphrase.to_string())?;
        return crate::key_crypto::decrypt_key(&bytes, &pass)
            .map_err(|e| e.to_string());
    }

    // Legacy plaintext FJKY
    if bytes.len() < 68 || &bytes[..4] != b"FJKY" {
        return Err("invalid key file format (neither FJK2 nor FJKY)".into());
    }
    let seed: [u8; 32] = bytes[4..36].try_into().unwrap();
    let pubkey: [u8; 32] = bytes[36..68].try_into().unwrap();
    Ok((seed, pubkey))
}

fn key_id_from_pubkey(pubkey: &[u8; 32]) -> [u8; 16] {
    let h = fnv_digest_32(pubkey);
    h[..16].try_into().unwrap()
}

fn build_signed_message(key_id: &[u8; 16], bundle_digest: &[u8; 32], signed_at_ns: u64) -> Vec<u8> {
    let mut m = Vec::new();
    m.extend_from_slice(SIG_DOMAIN);
    m.extend_from_slice(key_id);
    m.push(SIG_ALG_ED25519);
    m.extend_from_slice(bundle_digest);
    m.extend_from_slice(&signed_at_ns.to_be_bytes());
    m
}

fn build_sigmanifest(
    key_id: &[u8; 16],
    bundle_digest: &[u8; 32],
    signed_at_ns: u64,
    signature: &[u8; 64],
) -> Vec<u8> {
    let mut m = Vec::with_capacity(SIGMANIFEST_LEN);
    m.extend_from_slice(&SIGMANIFEST_MAGIC.to_le_bytes());
    m.extend_from_slice(&SIGMANIFEST_SCHEMA.to_le_bytes());
    m.extend_from_slice(key_id);
    m.push(SIG_ALG_ED25519);
    m.push(0u8); // reserved
    m.extend_from_slice(bundle_digest);
    m.extend_from_slice(&signed_at_ns.to_le_bytes());
    m.extend_from_slice(signature);
    m
}

fn verify_sigmanifest(
    bundle_bytes: &[u8],
    sig_bytes: &[u8],
    pubkey: &[u8; 32],
) -> Result<(), &'static str> {
    if sig_bytes.len() != SIGMANIFEST_LEN {
        return Err("wrong manifest length");
    }
    // Parse
    let magic = u32::from_le_bytes(sig_bytes[0..4].try_into().unwrap());
    if magic != SIGMANIFEST_MAGIC { return Err("bad magic"); }
    let key_id: [u8; 16] = sig_bytes[6..22].try_into().unwrap();
    let alg = sig_bytes[22];
    if alg != SIG_ALG_ED25519 { return Err("unsupported algorithm"); }
    let manifest_digest: [u8; 32] = sig_bytes[24..56].try_into().unwrap();
    let signed_at_ns = u64::from_le_bytes(sig_bytes[56..64].try_into().unwrap());
    let signature: [u8; 64] = sig_bytes[64..128].try_into().unwrap();

    // Re-compute bundle digest
    let actual_digest = fnv_digest_32(bundle_bytes);
    if actual_digest != manifest_digest {
        return Err("Tampered: bundle digest mismatch");
    }

    // Rebuild canonical message
    let message = build_signed_message(&key_id, &manifest_digest, signed_at_ns);

    // Verify Ed25519 signature
    ed25519_verify(pubkey, &message, &signature)
        .map_err(|_| "SigVerifyFailed")
}

// ── Ed25519 thin wrappers ─────────────────────────────────────────────────────

fn ed25519_pubkey(seed: &[u8; 32]) -> [u8; 32] {
    use fjell_sig_ed25519::Ed25519SigningKey;
    Ed25519SigningKey::from_seed(seed).public_key_bytes()
}

fn ed25519_sign(seed: &[u8; 32], message: &[u8]) -> [u8; 64] {
    use fjell_sig_ed25519::Ed25519SigningKey;
    Ed25519SigningKey::from_seed(seed).sign_message(message)
}

fn ed25519_verify(pubkey: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<(), ()> {
    use fjell_sig_ed25519::Ed25519Provider;
    // Wrap the verify call through the provider's raw interface.
    // fjell-tools doesn't carry a fjell-keyring dependency; use the
    // lower-level dalek path directly from fjell-sig-ed25519.
    Ed25519Provider::verify_raw(pubkey, message, signature)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn flag<'a>(args: &'a [String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).and_then(|w| w.get(1)).cloned()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn unhex32(s: &str) -> Result<[u8; 32], String> {
    let bytes = unhex_bytes(s)?;
    bytes.try_into().map_err(|_| "expected 32-byte (64-char) hex".into())
}

fn unhex16(s: &str) -> Result<[u8; 16], String> {
    let bytes = unhex_bytes(s)?;
    bytes.try_into().map_err(|_| "expected 16-byte (32-char) hex".into())
}

fn unhex_bytes(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 { return Err("odd-length hex".into()); }
    (0..s.len()).step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i+2], 16).map_err(|_| "invalid hex char".into()))
        .collect()
}

fn fnv_digest_32(data: &[u8]) -> [u8; 32] {
    // 256-bit FNV-1a for content addressing (not for security; bundle_digest
    // uses this for fast change detection; real security in signature over it).
    let mut hash = [0u64; 4];
    let primes = [0xcbf29ce484222325u64, 0xcbf29ce484222327, 0xcbf29ce484222329, 0xcbf29ce484222331];
    for (i, &p) in primes.iter().enumerate() {
        let mut h: u64 = p;
        for (j, &b) in data.iter().enumerate() {
            h ^= b.wrapping_add(i as u8).wrapping_add(j as u8) as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        hash[i] = h;
    }
    let mut out = [0u8; 32];
    for (i, &h) in hash.iter().enumerate() {
        out[i*8..(i+1)*8].copy_from_slice(&h.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_round_trip() {
        // Generate a test key
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).unwrap();

        let pubkey = ed25519_pubkey(&seed);
        let bundle_bytes = b"fake bundle payload for test";

        // Build a manifest
        let bundle_digest = fnv_digest_32(bundle_bytes);
        let signed_at_ns = 123456789u64;
        let key_id = key_id_from_pubkey(&pubkey);
        let message = build_signed_message(&key_id, &bundle_digest, signed_at_ns);
        let signature = ed25519_sign(&seed, &message);
        let manifest = build_sigmanifest(&key_id, &bundle_digest, signed_at_ns, &signature);

        // Verify
        assert_eq!(manifest.len(), SIGMANIFEST_LEN);
        verify_sigmanifest(bundle_bytes, &manifest, &pubkey)
            .expect("round-trip must verify");
    }

    #[test]
    fn tampered_bundle_fails_verify() {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).unwrap();
        let pubkey = ed25519_pubkey(&seed);
        let bundle_bytes = b"original bundle";
        let bundle_digest = fnv_digest_32(bundle_bytes);
        let signed_at = 0u64;
        let key_id = key_id_from_pubkey(&pubkey);
        let message = build_signed_message(&key_id, &bundle_digest, signed_at);
        let signature = ed25519_sign(&seed, &message);
        let manifest = build_sigmanifest(&key_id, &bundle_digest, signed_at, &signature);

        let tampered = b"tampered bundle";
        let result = verify_sigmanifest(tampered, &manifest, &pubkey);
        assert!(result.is_err(), "tampered bundle must fail");
    }

    #[test]
    fn tampered_signature_fails_verify() {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).unwrap();
        let pubkey = ed25519_pubkey(&seed);
        let bundle_bytes = b"bundle";
        let bundle_digest = fnv_digest_32(bundle_bytes);
        let signed_at = 0u64;
        let key_id = key_id_from_pubkey(&pubkey);
        let message = build_signed_message(&key_id, &bundle_digest, signed_at);
        let signature = ed25519_sign(&seed, &message);
        let mut manifest = build_sigmanifest(&key_id, &bundle_digest, signed_at, &signature);
        manifest[64] ^= 0xFF; // corrupt first byte of signature
        let result = verify_sigmanifest(bundle_bytes, &manifest, &pubkey);
        assert!(result.is_err(), "tampered sig must fail");
    }

    #[test]
    fn fnv_digest_deterministic() {
        let a = fnv_digest_32(b"hello");
        let b = fnv_digest_32(b"hello");
        assert_eq!(a, b);
        let c = fnv_digest_32(b"world");
        assert_ne!(a, c);
    }

    #[test]
    fn unhex_round_trip() {
        let bytes = [0xABu8; 32];
        let s = hex(&bytes);
        let back = unhex32(&s).unwrap();
        assert_eq!(bytes, back);
    }
}
