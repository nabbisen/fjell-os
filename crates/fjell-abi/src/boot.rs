//! Boot-time information passed from kernel to `fjell-init`.
//!
//! The kernel writes a `BootInfo` record into a read-only page of init's
//! address space before starting it, and passes that page's virtual address in
//! register `a1` at entry. (Until RFC-0.38-001 this module said exactly that
//! and the kernel did not do it: it set `a1 = 0` and built no record.)

/// Kernel-to-init boot information.
///
/// # The contract is the wire encoding, not the struct's memory layout
///
/// A `BootInfo` crosses an address-space boundary as exactly
/// [`BootInfo::WIRE_BYTES`] bytes, written by [`BootInfo::to_bytes`] and read by
/// [`BootInfo::from_bytes`]: every field little-endian, in declaration order,
/// with **no padding**. The struct is deliberately not `#[repr(C)]` — its
/// in-memory layout has padding and is not what is transferred, and a reader
/// that cast the page to the struct would read a different record than the one
/// written.
///
/// | offset | width | field |
/// |---|---|---|
/// | 0 | 4 | `version` |
/// | 4 | 2 | `bootstrap_cap_count` |
/// | 6 | 2 | `cap_task_create` |
/// | 8 | 2 | `cap_derive` |
/// | 10 | 2 | `cap_audit_drain` |
/// | 12 | 2 | `cap_bootstrap_ep` |
/// | 14 | 8 | `ram_bytes` |
/// | 22 | 2 | `image_set_count` *(version 2)* |
/// | 24 | 32 | `image_set_digest` *(version 2)* |
///
/// # Evolution
///
/// New fields are **appended** and `version` is bumped. A reader accepts any
/// `version >= BootInfo::VERSION` it was built for and ignores bytes beyond the
/// ones it knows; it refuses a record that is shorter than it needs or older
/// than it understands ([`BootInfoError`]). It never guesses.
///
/// # What the kernel provides
///
/// * `version`, `ram_bytes`, `image_set_count` and `image_set_digest` — real.
/// * `bootstrap_cap_count`, `cap_task_create`, `cap_derive`, `cap_audit_drain`,
///   `cap_bootstrap_ep` — **not provided**: they hold [`BootInfo::ABSENT`].
///   They were specified in version 1 and no kernel ever filled them; `init`
///   names its capability slots with the constants in [`crate::service`]. They
///   are kept so the record's prefix does not move, and say plainly that they
///   are empty rather than carrying a slot number that is not true.
///
/// # `image_set_digest`
///
/// The digest of the service images embedded in the kernel image — the
/// two-level digest `fjell-measure-format` documents
/// (`compute_image_set_digest`, `FJELL-IMAGE-SET-V1`) — and `image_set_count`,
/// how many images it covers. It is **not** a digest of the kernel itself.
/// `init` hands it to `measuredd` to record; nothing else reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootInfo {
    /// Struct version — [`BootInfo::VERSION`].
    pub version: u32,
    /// Not provided; [`BootInfo::ABSENT`].
    pub bootstrap_cap_count: u16,
    /// Not provided; [`BootInfo::ABSENT`].
    pub cap_task_create: u16,
    /// Not provided; [`BootInfo::ABSENT`].
    pub cap_derive: u16,
    /// Not provided; [`BootInfo::ABSENT`].
    pub cap_audit_drain: u16,
    /// Not provided; [`BootInfo::ABSENT`].
    pub cap_bootstrap_ep: u16,
    /// Physical RAM size in bytes (informational).
    pub ram_bytes: u64,
    /// Number of service images `image_set_digest` covers (version 2).
    pub image_set_count: u16,
    /// Digest of the embedded service-image set (version 2).
    pub image_set_digest: [u8; 32],
}

/// Why a byte buffer is not a usable `BootInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootInfoError {
    /// Fewer than [`BootInfo::WIRE_BYTES`] bytes.
    Short,
    /// A `version` older than [`BootInfo::VERSION`] (carries the one found).
    Version(u32),
}

impl BootInfo {
    /// The version this build writes and requires at least.
    pub const VERSION: u32 = 2;
    /// Size of the wire encoding, in bytes.
    pub const WIRE_BYTES: usize = 56;
    /// The value of a `u16` field the kernel does not provide.
    pub const ABSENT: u16 = 0xFFFF;

    /// Bootstrap capability CSpace indices used by `fjell-init`.
    pub const SLOT_TASK_CREATE: u16 = 0;
    pub const SLOT_CAP_DERIVE: u16 = 1;
    pub const SLOT_AUDIT_DRAIN: u16 = 2;
    pub const SLOT_BOOTSTRAP_EP: u16 = 3;

    /// A version-2 record with every unprovided field set to [`Self::ABSENT`].
    pub const fn new(ram_bytes: u64, image_set_count: u16, image_set_digest: [u8; 32]) -> Self {
        Self {
            version: Self::VERSION,
            bootstrap_cap_count: Self::ABSENT,
            cap_task_create: Self::ABSENT,
            cap_derive: Self::ABSENT,
            cap_audit_drain: Self::ABSENT,
            cap_bootstrap_ep: Self::ABSENT,
            ram_bytes,
            image_set_count,
            image_set_digest,
        }
    }

    /// The wire encoding: little-endian, declaration order, no padding.
    pub fn to_bytes(&self) -> [u8; Self::WIRE_BYTES] {
        let mut b = [0u8; Self::WIRE_BYTES];
        b[0..4].copy_from_slice(&self.version.to_le_bytes());
        b[4..6].copy_from_slice(&self.bootstrap_cap_count.to_le_bytes());
        b[6..8].copy_from_slice(&self.cap_task_create.to_le_bytes());
        b[8..10].copy_from_slice(&self.cap_derive.to_le_bytes());
        b[10..12].copy_from_slice(&self.cap_audit_drain.to_le_bytes());
        b[12..14].copy_from_slice(&self.cap_bootstrap_ep.to_le_bytes());
        b[14..22].copy_from_slice(&self.ram_bytes.to_le_bytes());
        b[22..24].copy_from_slice(&self.image_set_count.to_le_bytes());
        b[24..56].copy_from_slice(&self.image_set_digest);
        b
    }

    /// Read a record. Accepts `version >= VERSION` and ignores any bytes past
    /// [`Self::WIRE_BYTES`] (fields a later version appended); refuses a short
    /// buffer or an older version.
    pub fn from_bytes(b: &[u8]) -> Result<Self, BootInfoError> {
        if b.len() < Self::WIRE_BYTES {
            return Err(BootInfoError::Short);
        }
        let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        let version = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if version < Self::VERSION {
            return Err(BootInfoError::Version(version));
        }
        let mut ram = [0u8; 8];
        ram.copy_from_slice(&b[14..22]);
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&b[24..56]);
        Ok(Self {
            version,
            bootstrap_cap_count: u16_at(4),
            cap_task_create: u16_at(6),
            cap_derive: u16_at(8),
            cap_audit_drain: u16_at(10),
            cap_bootstrap_ep: u16_at(12),
            ram_bytes: u64::from_le_bytes(ram),
            image_set_count: u16_at(22),
            image_set_digest: digest,
        })
    }
}

#[cfg(test)]
mod tests;
