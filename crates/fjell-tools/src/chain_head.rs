//! The measurement chain head, recomputed on the host (RFC-0.39-002 R5).
//!
//! RFC-0.38-001 R6 set the standard: a recorded measurement is evidence only if
//! it is **equal to a value derived independently**, not merely non-zero. This is
//! the same standard applied to the *chain head* a signed diagnostic bundle now
//! carries. A tier that asserted `measurement_head_available=true` and a non-zero
//! head would pass against a bundle reporting a placeholder chain as a measured
//! one; this module is the other side of that comparison.
//!
//! It is built to be independent of what it checks, like [`crate::image_set`]:
//!
//! * the hash is the external `sha2` crate, not the in-tree SHA-256 `measuredd`
//!   runs;
//! * the preimage is assembled here, byte by byte, from the layout documented in
//!   `fjell-measure-format/src/lib.rs` and generated into
//!   `schema/chain-digest-v1.frozen` -- this module does **not** import that
//!   crate (a dev-dependency does, in the tests only, to prove agreement);
//! * the *events* are written out here too: what a boot is supposed to append,
//!   not what any service says it appended.
//!
//! # What a boot appends
//!
//! Four events, in this order (RFC-0.38-001, RFC-0.39-001):
//!
//! | seq | kind | source | subject | digest |
//! |---|---|---|---|---|
//! | 1 | `BootEvidenceImported` (1) | `Kernel` (1) | `BootEvidence` (1) | the image-set digest ([`crate::image_set`]) |
//! | 2 | `ReleaseManifestVerified` (2) | `Init` (13) | `ReleaseManifest` (2) | placeholder |
//! | 3 | `RootfsManifestVerified` (3) | `Init` (13) | `RootfsManifest` (3) | placeholder |
//! | 4 | `PolicyBundleVerified` (4) | `Init` (13) | `PolicyBundle` (4) | placeholder |
//!
//! The placeholder digest is `init`'s literal `0xAB` in the first word and zeros
//! after it: `APPEND_EVENT` carries 16 of a digest's 32 bytes (E-095).
//!
//! The table above **is a statement that three of the four are placeholders**.
//! This module deliberately does not hide that: if those events ever become real,
//! this oracle must change with them, and a tier that still passes would mean it
//! did not.

use sha2::{Digest, Sha256};
use std::path::Path;

const DOMAIN: &[u8] = b"FJELL-MEASUREMENT-V1";
const SCHEMA_VERSION: u16 = 1;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64 {
        return Err(format!("expected 64 hex digits, got {}", s.len()));
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|e| format!("not hex at byte {i}: {e}"))?;
    }
    Ok(out)
}

/// `init`'s placeholder subject digest: the first word `0xAB`, little-endian, the
/// second word zero, and 16 bytes that never travel.
fn placeholder_digest() -> [u8; 32] {
    let mut d = [0u8; 32];
    d[0..8].copy_from_slice(&0xABu64.to_le_bytes());
    d
}

/// One link of the chain: `SHA-256(domain || schema u16 LE || seq u64 LE || kind
/// || source || subject || subject_digest || metadata_present(0) || metadata[32]
/// zero || previous)`.
fn link(
    seq: u64,
    kind: u8,
    source: u8,
    subject: u8,
    digest: &[u8; 32],
    prev: &[u8; 32],
) -> [u8; 32] {
    let mut pre = Vec::with_capacity(DOMAIN.len() + 2 + 8 + 3 + 32 + 1 + 32 + 32);
    pre.extend_from_slice(DOMAIN);
    pre.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    pre.extend_from_slice(&seq.to_le_bytes());
    pre.extend_from_slice(&[kind, source, subject]);
    pre.extend_from_slice(digest);
    pre.push(0); // metadata_present
    pre.extend_from_slice(&[0u8; 32]); // metadata digest, zero when absent
    pre.extend_from_slice(prev);
    Sha256::digest(&pre).into()
}

/// The chain head after the four events a boot appends, given the image-set
/// digest as 64 hex digits. Returns `(events, head as 64 lowercase hex digits)`.
pub fn boot_chain_head_from(image_set_digest_hex: &str) -> Result<(u64, String), String> {
    let image_set = unhex32(image_set_digest_hex)?;
    let ph = placeholder_digest();
    let events: [(u8, u8, u8, [u8; 32]); 4] = [
        (1, 1, 1, image_set),
        (2, 13, 2, ph),
        (3, 13, 3, ph),
        (4, 13, 4, ph),
    ];
    let mut prev = [0u8; 32];
    for (i, (kind, source, subject, digest)) in events.iter().enumerate() {
        prev = link(i as u64 + 1, *kind, *source, *subject, digest, &prev);
    }
    Ok((events.len() as u64, hex(&prev)))
}

/// The same, with the image-set digest recomputed from the prebuilt images under
/// `root` ([`crate::image_set::image_set_digest`]).
pub fn boot_chain_head(root: &Path) -> Result<(u64, String), String> {
    let (_images, set) = crate::image_set::image_set_digest(root)?;
    boot_chain_head_from(&set)
}

#[cfg(test)]
mod tests;
