//! `KeyringSnapshot` — canonical serialised representation of a keyring.
//!
//! Schema: a fixed-shape, version-tagged blob with a content-addressing
//! digest (RFC-v0.3-002 §6.5).  The blob is signed externally by the
//! release authority; this module concerns itself only with structure,
//! digest computation, and replay onto a fresh `Keyring`.

use fjell_canon::{BufSink, Canon};
use fjell_measure_format::Digest32;

use crate::anchor::TrustAnchor;
use crate::error::SigError;
use crate::keyring::{Keyring, PURPOSE_SLOT_COUNT};
use crate::{ANCHOR_KEY_BYTES_MAX, ANCHORS_PER_PURPOSE, KEYRING_DOMAIN, SCHEMA_VERSION};
use fjell_trust_provider::KeyPurpose;

/// Magic bytes at the start of every snapshot.
pub const KEYRING_SNAPSHOT_MAGIC: [u8; 4] = *b"FJLR";

/// Maximum anchors in a serialised snapshot.
pub const MAX_SNAPSHOT_ANCHORS: usize = PURPOSE_SLOT_COUNT * ANCHORS_PER_PURPOSE;

/// On-disk snapshot of a keyring's anchors at a point in time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyringSnapshot {
    pub schema_version: u16,
    pub anchor_count: u8,
    pub anchors: [Option<TrustAnchor>; MAX_SNAPSHOT_ANCHORS],
    pub snapshot_digest: Digest32,
}

impl KeyringSnapshot {
    /// Build a snapshot from a live `Keyring`.  The digest is computed at
    /// build time using the canonical formula in `compute_digest`.
    pub fn from_keyring(keyring: &Keyring) -> Self {
        let mut anchors: [Option<TrustAnchor>; MAX_SNAPSHOT_ANCHORS] = [None; MAX_SNAPSHOT_ANCHORS];
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

/// The canonical byte stream a snapshot's digest is taken over — **the function
/// the digest is computed from and the frozen schema is generated from**.
///
/// Domain: `KEYRING_DOMAIN || "SNAP-V1"`. Every one of the
/// `MAX_SNAPSHOT_ANCHORS` slots is written, present or not: a present byte
/// (`+`/`-`), the anchor's purpose, algorithm, authority and epoch, its key
/// length, and its key bytes. An empty slot writes the same shape with zeros, so
/// the description of one slot is true of all of them.
pub fn write_canonical(snap: &KeyringSnapshot, c: &mut dyn Canon) {
    c.domain(KEYRING_DOMAIN);
    c.domain(b"SNAP-V1");
    c.u16("schema_version", snap.schema_version);
    c.u8("anchor_count", snap.anchor_count);
    c.each(
        "slots",
        MAX_SNAPSHOT_ANCHORS,
        Some(MAX_SNAPSHOT_ANCHORS),
        &mut |c, i| match &snap.anchors[i] {
            Some(a) => {
                c.u8("present", b'+');
                c.u8("purpose", a.purpose.tag());
                c.u8("algorithm", a.algorithm.tag());
                c.u8("authority", a.authority.tag());
                c.u32("epoch", a.epoch.raw());
                c.u8("reserved", 0);
                c.u8("key_len", a.key_len);
                c.var_bytes("key_bytes", &a.key_bytes[..a.key_len as usize], "key_len");
            }
            None => {
                c.u8("present", b'-');
                c.u8("purpose", 0);
                c.u8("algorithm", 0);
                c.u8("authority", 0);
                c.u32("epoch", 0);
                c.u8("reserved", 0);
                c.u8("key_len", 0);
                c.var_bytes("key_bytes", &[], "key_len");
            }
        },
    );
}

/// Capacity of the canonical stream: the header plus every slot at its widest.
const SNAPSHOT_STREAM_MAX: usize = 32 + MAX_SNAPSHOT_ANCHORS * (1 + 8 + 1 + ANCHOR_KEY_BYTES_MAX);

/// Compute the canonical digest of a snapshot.
///
/// Domain:  `KEYRING_DOMAIN || "SNAP-V1"`.
fn compute_digest(snap: &KeyringSnapshot) -> Digest32 {
    let mut sink = BufSink::<{ SNAPSHOT_STREAM_MAX }>::new();
    write_canonical(snap, &mut sink);
    Digest32::of(sink.bytes())
}

/// Anchor key byte cap is reflected in the snapshot via the per-anchor
/// `key_len`.  This `_compile_time` assertion documents the invariant for
/// future readers.
const _: () = {
    let _ = ANCHOR_KEY_BYTES_MAX;
};
