//! The `BootInfo` record handed to `init` (RFC-0.38-001 R2).
//!
//! The kernel holds every service image's bytes (`task/image.rs`). Hashing them
//! here, once, before `init` runs makes a recorded measurement a fact about the
//! bytes that booted rather than a literal (E-086). The digest reaches the
//! measurement chain by the route RFC-0.38-001's design ruling chose:
//! kernel → a read-only page in `init`'s address space (`BootInfo`, whose
//! address is in `a1` at entry) → `init` → `measuredd`.
//!
//! What is hashed: the embedded **service** images only — not the kernel's own
//! image, and nothing at runtime. The set digest is `fjell-measure-format`'s
//! `compute_image_set_digest` (documented there and described by its generated
//! `.frozen` file); this module only collects the entries.

use crate::task::image::{SERVICE_STACK_TOP, image_bytes};
use fjell_abi::service::ImageId;
use fjell_measure_format::{
    Digest32, IMAGE_SET_MAX, ImageEntry, ImageSetError, compute_image_set_digest,
};

/// Where `init` sees its `BootInfo`: the page after a one-page unmapped guard
/// that follows the top of its stack (`0x90000` is the guard, this is `0x91000`).
/// Nothing else of `init`'s layout moves.
pub const BOOT_INFO_VA: usize = SERVICE_STACK_TOP + 0x1000;

/// The embedded service images, hashed: how many, and the set digest.
///
/// Every possible `ImageId` is asked for (`image_bytes` answers `None` for an id
/// with no image), so an image added to the table is covered without anyone
/// remembering to widen a range here.
pub fn image_set() -> Result<(u16, Digest32), ImageSetError> {
    let mut entries = [ImageEntry {
        image_id: 0,
        digest: Digest32::ZERO,
    }; IMAGE_SET_MAX];
    let mut n = 0usize;
    for id in 0..=u16::MAX {
        if let Some(bytes) = image_bytes(ImageId(id)) {
            if n == IMAGE_SET_MAX {
                return Err(ImageSetError::TooMany);
            }
            entries[n] = ImageEntry {
                image_id: id,
                digest: Digest32::of(bytes),
            };
            n += 1;
        }
    }
    let digest = compute_image_set_digest(&entries[..n])?;
    Ok((n as u16, digest))
}
