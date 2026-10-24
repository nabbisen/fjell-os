//! The service-image set digest, recomputed on the host (RFC-0.38-001 R6).
//!
//! The kernel hashes the service images it embeds and `measuredd` records the
//! result (`fjell-measure-format::compute_image_set_digest`). E-086's root cause
//! was that *no instrument compared a recorded measurement with the thing it
//! claims to measure*. This is the other side of that comparison, and it is
//! built to be **independent** of what it checks:
//!
//! * the hash is the external `sha2` crate, not the in-tree SHA-256 the kernel
//!   runs;
//! * the preimage is assembled here, byte by byte, from the layout documented in
//!   `fjell-measure-format/src/image_set.rs` and generated into
//!   `schema/image-set-digest-v1.frozen` — this module does **not** import that
//!   crate, so a change to the encoder that is not made here fails the tier
//!   instead of passing it;
//! * the file → `ImageId` table is derived from the prebuilt **file names** and
//!   `fjell-abi`'s `ImageId` constants, not from the kernel's `task/image.rs`
//!   `match`, so an image that table maps to the wrong binary is a disagreement
//!   rather than being agreed with.
//!
//! What it reads is `crates/fjell-kernel/prebuilt/*.bin` — the bytes the kernel
//! was built to embed. If those files were rebuilt and the kernel was not, the
//! two digests differ, which is the correct answer.

use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

/// Domain tag of the preimage. Deliberately a second copy, not an import: see
/// the module doc.
const DOMAIN: &[u8] = b"FJELL-IMAGE-SET-V1";

const PREBUILT_DIR: &str = "crates/fjell-kernel/prebuilt";
const SERVICE_ABI: &str = "crates/fjell-abi/src/service.rs";

/// `pub const NAME: ImageId = ImageId(<n>);` → `(NAME, n)`, for every such line
/// of `fjell-abi/src/service.rs`.
fn image_id_constants(src: &str) -> BTreeMap<String, u16> {
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(": ImageId = ImageId(") else {
            continue;
        };
        let Some((lit, _)) = rest.split_once(')') else {
            continue;
        };
        let lit = lit.trim();
        let n = match lit.strip_prefix("0x").or_else(|| lit.strip_prefix("0X")) {
            Some(h) => u16::from_str_radix(h, 16),
            None => lit.parse::<u16>(),
        };
        if let Ok(n) = n {
            out.insert(name.trim().to_string(), n);
        }
    }
    out
}

/// `fjell-driver-uart.bin` → `DRIVER_UART`: strip `fjell-` and `.bin`,
/// uppercase, hyphens to underscores — the naming `ImageId`'s constants follow
/// for every service in the tree.
fn const_name_for(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".bin")?;
    let stem = stem.strip_prefix("fjell-")?;
    Some(stem.to_uppercase().replace('-', "_"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The set digest of the images in `<root>/crates/fjell-kernel/prebuilt/`, as
/// `(image count, 64 lowercase hex digits)`.
///
/// Fails — rather than skipping — on a `.bin` with no `ImageId` constant, two
/// files that resolve to one id, an unreadable file, or no images at all: each
/// would otherwise produce a digest of something other than the image set, and
/// a comparison against it would be evidence of nothing.
pub fn image_set_digest(root: &Path) -> Result<(usize, String), String> {
    let abi_path = root.join(SERVICE_ABI);
    let abi = fs::read_to_string(&abi_path)
        .map_err(|e| format!("cannot read {}: {e}", abi_path.display()))?;
    let ids = image_id_constants(&abi);
    if ids.is_empty() {
        return Err(format!(
            "no `ImageId` constants found in {} — cannot map the prebuilt images",
            abi_path.display()
        ));
    }

    let dir = root.join(PREBUILT_DIR);
    let mut images: BTreeMap<u16, (String, [u8; 32])> = BTreeMap::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".bin") {
            continue;
        }
        let Some(cname) = const_name_for(&name) else {
            return Err(format!(
                "{name}: not named `fjell-<service>.bin`, so no ImageId can be derived from it"
            ));
        };
        let Some(&id) = ids.get(&cname) else {
            return Err(format!(
                "{name}: no `ImageId::{cname}` constant in {SERVICE_ABI}"
            ));
        };
        let bytes = fs::read(entry.path())
            .map_err(|e| format!("cannot read {}: {e}", entry.path().display()))?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if let Some((other, _)) = images.insert(id, (name.clone(), digest)) {
            return Err(format!(
                "{other} and {name} both resolve to image id {id} — the set would be ambiguous"
            ));
        }
    }
    if images.is_empty() {
        return Err(format!("no .bin images under {}", dir.display()));
    }
    if images.len() > u16::MAX as usize {
        return Err("more images than a u16 count can state".to_string());
    }

    // The preimage, written out from the documented layout: the domain tag, the
    // image count (u16 LE), then each image's id (u16 LE) and digest, ascending
    // by id. `BTreeMap` iterates in key order.
    let mut pre = Vec::with_capacity(DOMAIN.len() + 2 + images.len() * 34);
    pre.extend_from_slice(DOMAIN);
    pre.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (id, (_, digest)) in &images {
        pre.extend_from_slice(&id.to_le_bytes());
        pre.extend_from_slice(digest);
    }
    Ok((images.len(), hex(&Sha256::digest(&pre))))
}

#[cfg(test)]
mod tests;
