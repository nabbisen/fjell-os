//! `KeyringSnapshot` — canonical serialised representation of a keyring.
//!
//! Schema: a fixed-shape, version-tagged blob with a content-addressing
//! digest (RFC-v0.3-002 §6.5).  The blob is signed externally by the
//! release authority; this module concerns itself only with structure,
//! digest computation, and replay onto a fresh `Keyring`.

use fjell_measure_format::Digest32;

use crate::anchor::TrustAnchor;
use crate::error::SigError;
use crate::keyring::{Keyring, PURPOSE_SLOT_COUNT};
use crate::{ANCHORS_PER_PURPOSE, ANCHOR_KEY_BYTES_MAX, KEYRING_DOMAIN, SCHEMA_VERSION};
use fjell_trust_provider::KeyPurpose;

/// Magic bytes at the start of every snapshot.
pub const KEYRING_SNAPSHOT_MAGIC: [u8; 4] = *b"FJLR";

/// Maximum anchors in a serialised snapshot.
pub const MAX_SNAPSHOT_ANCHORS: usize = PURPOSE_SLOT_COUNT * ANCHORS_PER_PURPOSE;

/// On-disk snapshot of a keyring's anchors at a point in time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyringSnapshot {
    pub schema_version: u16,
    pub anchor_count:   u8,
    pub anchors:        [Option<TrustAnchor>; MAX_SNAPSHOT_ANCHORS],
    pub snapshot_digest: Digest32,
}

impl KeyringSnapshot {
    /// Build a snapshot from a live `Keyring`.  The digest is computed at
    /// build time using the canonical formula in `compute_digest`.
    pub fn from_keyring(keyring: &Keyring) -> Self {
        let mut anchors: [Option<TrustAnchor>; MAX_SNAPSHOT_ANCHORS] =
            [None; MAX_SNAPSHOT_ANCHORS];
        let mut count: u8 = 0;
        for purpose in KeyPurpose::all().iter().copied() {
            for a in keyring.anchors_for(purpose) {
                if (count as usize) < MAX_SNAPSHOT_ANCHORS {
                    anchors[count as usize] = Some(a);
                    count += 1;
                }
            }
        }
        let mut s = Self {
            schema_version: SCHEMA_VERSION,
            anchor_count: count,
            anchors,
            snapshot_digest: Digest32::ZERO,
        };
        s.snapshot_digest = compute_digest(&s);
        s
    }

    /// Re-apply this snapshot onto an empty `Keyring`.
    ///
    /// Returns `SigError::SnapshotDigestMismatch` if the stored digest
    /// doesn't match the recomputed one.
    pub fn apply_to(&self, dest: &mut Keyring) -> Result<usize, SigError> {
        let recomputed = compute_digest(self);
        if recomputed != self.snapshot_digest {
            return Err(SigError::SnapshotDigestMismatch);
        }
        let mut installed = 0usize;
        for slot in self.anchors.iter().take(self.anchor_count as usize) {
            if let Some(a) = slot {
                dest.install(*a)?;
                installed += 1;
            }
        }
        Ok(installed)
    }
}

/// Compute the canonical digest of a snapshot.
///
/// Domain:  `KEYRING_DOMAIN || "SNAP-V1"`.
fn compute_digest(snap: &KeyringSnapshot) -> Digest32 {
    let header = [
        snap.schema_version.to_le_bytes()[0],
        snap.schema_version.to_le_bytes()[1],
        snap.anchor_count,
    ];

    // Pass 1: materialise per-anchor scratch buffers (no borrows of parts
    // here, so the borrow checker is happy).
    let mut present: [u8; MAX_SNAPSHOT_ANCHORS] = [b'-'; MAX_SNAPSHOT_ANCHORS];
    let mut scratch: [[u8; 8]; MAX_SNAPSHOT_ANCHORS] = [[0u8; 8]; MAX_SNAPSHOT_ANCHORS];
    let mut keylens: [[u8; 1]; MAX_SNAPSHOT_ANCHORS] = [[0u8; 1]; MAX_SNAPSHOT_ANCHORS];

    for i in 0..MAX_SNAPSHOT_ANCHORS {
        if let Some(a) = snap.anchors[i] {
            present[i] = b'+';
            let epoch = a.epoch.raw().to_le_bytes();
            scratch[i] = [
                a.purpose.tag(),
                a.algorithm.tag(),
                a.authority.tag(),
                epoch[0],
                epoch[1],
                epoch[2],
                epoch[3],
                0,
            ];
            keylens[i] = [a.key_len];
        }
    }

    // Pass 2: build the parts list, taking borrows of the scratch arrays
    // and of the in-place key bytes inside `snap.anchors`.
    let mut parts: [&[u8]; 3 + MAX_SNAPSHOT_ANCHORS * 5] =
        [&[]; 3 + MAX_SNAPSHOT_ANCHORS * 5];
    parts[0] = KEYRING_DOMAIN;
    parts[1] = b"SNAP-V1";
    parts[2] = &header;

    let mut p = 3;
    for i in 0..MAX_SNAPSHOT_ANCHORS {
        parts[p] = core::slice::from_ref(&present[i]);
        parts[p + 1] = &scratch[i];
        parts[p + 2] = &keylens[i];
        // Reference the anchor's key bytes through `snap.anchors[i]` so the
        // borrow is tied to `snap`'s lifetime, not to a local `Some(a)`.
        parts[p + 3] = match &snap.anchors[i] {
            Some(a) => &a.key_bytes[..a.key_len as usize],
            None => &[],
        };
        parts[p + 4] = &[]; // padding slot for future fields
        p += 5;
    }

    Digest32::of_parts(&parts[..p])
}

/// Anchor key byte cap is reflected in the snapshot via the per-anchor
/// `key_len`.  This `_compile_time` assertion documents the invariant for
/// future readers.
const _: () = {
    let _ = ANCHOR_KEY_BYTES_MAX;
};
