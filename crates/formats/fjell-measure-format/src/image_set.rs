//! The digest of the set of service images a kernel image embeds
//! (RFC-0.38-001 R2, ruling 2).
//!
//! The kernel carries every service as a flat binary (`include_bytes!`). At
//! boot it hashes each one, then hashes the per-image results into **one**
//! digest, and hands that to `init`, which records it in the measurement chain.
//! A recorded measurement is then a fact about the bytes that booted, not a
//! literal (E-086).
//!
//! # The two levels
//!
//! 1. `digest_i = SHA-256(bytes_i)` — the whole flat binary, exactly as
//!    embedded, no prefix.
//! 2. `set_digest = SHA-256(preimage)`, where the preimage is the canonical
//!    stream below: a domain tag, the **image count**, then each image's id and
//!    its `digest_i` in strictly ascending `image_id` order.
//!
//! The count is in the preimage so that "there were N images" is itself covered:
//! an image added or dropped cannot coincide with a reordering or with another
//! set. The stream is written through [`fjell_canon::Canon`], like
//! `MeasurementEvent::compute_chain_digest`'s, so the same function that
//! produces the bytes also generates their description
//! (`schema/image-set-digest-v1.frozen`, RFC-0.35-004) and a second
//! implementation has something to match. This is a **different** digest from
//! the chain digest; adding it does not touch the chain's frozen stream.
//!
//! # What it does not measure
//!
//! Only the embedded service images. Not the kernel's own image, not
//! configuration, not rootfs or runtime state (RFC-0.38-001 non-goals).

use crate::digest::Digest32;
use fjell_canon::{BufSink, Canon};

/// Domain tag: separates this stream from every other digest in the tree.
pub const IMAGE_SET_DOMAIN: &[u8] = b"FJELL-IMAGE-SET-V1";

/// Most images one set may hold. The kernel embeds 33 today; the bound exists
/// so the preimage buffer is a compile-time size and so a corrupt count cannot
/// ask for more. Raising it changes [`IMAGE_SET_PREIMAGE_MAX`], not the format.
pub const IMAGE_SET_MAX: usize = 64;

/// Largest preimage: the domain tag, a `u16` count, and `IMAGE_SET_MAX` entries
/// of a `u16` id plus a 32-byte digest.
pub const IMAGE_SET_PREIMAGE_MAX: usize = 18 + 2 + IMAGE_SET_MAX * (2 + 32);

/// One image of the set: its id, and the SHA-256 of its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageEntry {
    pub image_id: u16,
    pub digest: Digest32,
}

/// Why a set was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageSetError {
    /// No images. A digest of nothing is not a measurement of anything.
    Empty,
    /// More than [`IMAGE_SET_MAX`] entries.
    TooMany,
    /// The ids are not strictly ascending (an id repeated, or out of order).
    /// The order is part of the format; accepting any order would make the
    /// digest depend on how the caller happened to iterate.
    NotAscending,
}

/// The canonical byte stream of an image set, written through `Canon`.
///
/// The caller is responsible for having validated `entries`
/// ([`compute_image_set_digest`] does); this writer states the layout.
pub fn write_image_set_canonical(c: &mut dyn Canon, entries: &[ImageEntry]) {
    c.domain(IMAGE_SET_DOMAIN);
    c.u16("image_count", entries.len() as u16);
    c.note(
        "image_order",
        "images follow in strictly ascending image_id order; image_count is the number of entries",
    );
    c.note(
        "image_digest_is",
        "SHA-256 of the image's flat binary exactly as embedded (the prebuilt .bin), no prefix",
    );
    c.each("images", entries.len(), Some(IMAGE_SET_MAX), &mut |c, i| {
        c.u16("image_id", entries[i].image_id);
        c.bytes("image_digest", &entries[i].digest.0);
    });
}

/// Validate `entries` and compute the set digest.
pub fn compute_image_set_digest(entries: &[ImageEntry]) -> Result<Digest32, ImageSetError> {
    if entries.is_empty() {
        return Err(ImageSetError::Empty);
    }
    if entries.len() > IMAGE_SET_MAX {
        return Err(ImageSetError::TooMany);
    }
    if entries.windows(2).any(|w| w[0].image_id >= w[1].image_id) {
        return Err(ImageSetError::NotAscending);
    }
    let mut sink = BufSink::<IMAGE_SET_PREIMAGE_MAX>::new();
    write_image_set_canonical(&mut sink, entries);
    Ok(Digest32::of(sink.bytes()))
}

#[cfg(test)]
mod tests;
