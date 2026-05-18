//! System snapshot types for Fjell OS M7.
#![no_std]

/// Unique identifier for a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotId(pub u64);

/// Reason a snapshot was created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotReason {
    Boot,
    PreUpgrade,
    PostConfirmation,
    Rollback,
    Periodic,
}

/// Compact digest of the system state at snapshot time.
#[derive(Clone, Copy)]
pub struct SnapshotDigest {
    pub slot:          u8,
    pub release_hash:  [u8; 8],
    pub rootfs_hash:   [u8; 8],
    pub policy_hash:   [u8; 8],
    pub store_seq:     u64,
    pub audit_seq:     u64,
}

impl SnapshotDigest {
    pub const fn current(slot: u8, store_seq: u64) -> Self {
        SnapshotDigest {
            slot,
            release_hash: *b"REL_HASH",
            rootfs_hash:  *b"RFS_HASH",
            policy_hash:  *b"POL_HASH",
            store_seq,
            audit_seq: 0,
        }
    }
}

/// A system snapshot record.
#[derive(Clone, Copy)]
pub struct SystemSnapshot {
    pub id:      SnapshotId,
    pub reason:  SnapshotReason,
    pub digest:  SnapshotDigest,
    pub seq:     u64,
}

impl SystemSnapshot {
    pub fn new(id: u64, reason: SnapshotReason, slot: u8, store_seq: u64) -> Self {
        SystemSnapshot {
            id: SnapshotId(id), reason,
            digest: SnapshotDigest::current(slot, store_seq),
            seq: store_seq,
        }
    }
    pub fn reason_str(&self) -> &'static str {
        match self.reason {
            SnapshotReason::Boot             => "boot",
            SnapshotReason::PreUpgrade       => "pre-upgrade",
            SnapshotReason::PostConfirmation => "post-confirmation",
            SnapshotReason::Rollback         => "rollback",
            SnapshotReason::Periodic         => "periodic",
        }
    }
}
