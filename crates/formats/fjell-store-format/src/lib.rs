//! Persistent append-only state store format for Fjell OS M6.
#![no_std]

// ── CRC32 (RFC 008) ───────────────────────────────────────────────────────────
/// Compute CRC32 (ISO 3309 / Castagnoli, poly 0xEDB88320).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

pub const STORE_MAGIC: [u8; 8] = *b"FJSTORE\0";
pub const RECORD_MAGIC: u32 = 0x464A_4C52; // "FJLR"

/// On-disk version of `StoreSuperblock`.
///
/// 2 (RFC-0.32-002): the CRC is computed over an explicit field serialisation
/// rather than over the struct's memory, so the checksum bytes differ from
/// version 1's for an otherwise identical block.
pub const STORE_SUPERBLOCK_VERSION: u16 = 2;

pub const LBA_BOOT_CTL_A_START: u64 = 1;
pub const LBA_BOOT_CTL_A_END: u64 = 32;
pub const LBA_BOOT_CTL_B_START: u64 = 33;
pub const LBA_BOOT_CTL_B_END: u64 = 64;
pub const LBA_SUPERBLOCK_A: u64 = 65;
pub const LBA_SUPERBLOCK_B: u64 = 129;
pub const LBA_LOG_START: u64 = 193;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StoreSuperblock {
    pub magic: [u8; 8],
    pub version: u16,
    pub generation: u64,
    pub sector_size: u32,
    pub log_start_lba: u64,
    pub log_tail_seq: u64,
    pub active_checkpoint_seq: u64,
    pub crc32: u32,
}

/// Width of `StoreSuperblock`'s checksum serialisation: every field except
/// `crc32`, summed by hand so a field added without a line in
/// `checksum_input` fails the `debug_assert_eq!` there.
const SUPERBLOCK_CHECKSUM_BYTES: usize = 8 + 2 + 8 + 4 + 8 + 8 + 8;

impl StoreSuperblock {
    pub fn new(generation: u64) -> Self {
        StoreSuperblock {
            magic: STORE_MAGIC,
            version: STORE_SUPERBLOCK_VERSION,
            generation,
            sector_size: 512,
            log_start_lba: LBA_LOG_START,
            log_tail_seq: 0,
            active_checkpoint_seq: 0,
            crc32: 0,
        }
    }

    /// The bytes the CRC is computed over: each field, named, little-endian,
    /// in declaration order, with `crc32` itself excluded.
    ///
    /// This is the serialisation, not a view of the struct's memory. It
    /// contains no padding, so it does not depend on the layout the compiler
    /// chose, and `seal` and `is_valid` cannot disagree about what it holds.
    fn checksum_input(&self) -> [u8; SUPERBLOCK_CHECKSUM_BYTES] {
        let mut out = [0u8; SUPERBLOCK_CHECKSUM_BYTES];
        let mut at = 0usize;
        {
            let mut put = |bytes: &[u8]| {
                out[at..at + bytes.len()].copy_from_slice(bytes);
                at += bytes.len();
            };
            put(&self.magic);
            put(&self.version.to_le_bytes());
            put(&self.generation.to_le_bytes());
            put(&self.sector_size.to_le_bytes());
            put(&self.log_start_lba.to_le_bytes());
            put(&self.log_tail_seq.to_le_bytes());
            put(&self.active_checkpoint_seq.to_le_bytes());
        }
        debug_assert_eq!(at, SUPERBLOCK_CHECKSUM_BYTES);
        out
    }

    /// Compute and store CRC32 (RFC 008).  Call before writing to disk.
    pub fn seal(&mut self) {
        self.crc32 = crc32(&self.checksum_input());
    }

    /// Returns true if magic is correct AND CRC32 matches (RFC 008).
    pub fn is_valid(&self) -> bool {
        self.magic == STORE_MAGIC && crc32(&self.checksum_input()) == self.crc32
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordHeader {
    pub magic: u32,
    pub version: u16,
    pub kind: u16,
    pub seq: u64,
    pub total_len: u32,
    pub crc32: u32,
}

impl RecordHeader {
    pub fn new(kind: RecordKind, seq: u64, payload_len: usize) -> Self {
        RecordHeader {
            magic: RECORD_MAGIC,
            version: 1,
            kind: kind as u16,
            seq,
            total_len: (core::mem::size_of::<RecordHeader>() + payload_len) as u32,
            crc32: 0,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == RECORD_MAGIC
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    AuditEvent = 1,
    ConfigSnapshot = 2,
    ServiceState = 3,
    DeviceInventory = 4,
    StoreCheckpoint = 5,
    UpgradeTransaction = 6,
    BootControlEvent = 7,
    PowerTelemetry = 8,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §C, second test: a block sealed by this code validates under this code.
    #[test]
    fn sealed_superblock_is_valid() {
        let mut sb = StoreSuperblock::new(7);
        sb.seal();
        assert!(sb.is_valid(), "a sealed superblock must pass is_valid");
        assert_eq!(sb.version, STORE_SUPERBLOCK_VERSION);
    }

    /// §C, first test: seal, flip one field, and is_valid rejects it. Every
    /// field in the serialisation is flipped in turn, so a field left out of
    /// `checksum_input` is a failure here rather than a silent hole.
    #[test]
    fn one_flipped_field_fails_the_crc() {
        let base = {
            let mut sb = StoreSuperblock::new(7);
            sb.seal();
            sb
        };
        assert!(base.is_valid());

        let flips: [fn(&mut StoreSuperblock); 6] = [
            |sb| sb.version ^= 0xFF,
            |sb| sb.generation ^= 0xFF,
            |sb| sb.sector_size ^= 0xFF,
            |sb| sb.log_start_lba ^= 0xFF,
            |sb| sb.log_tail_seq ^= 0xFF,
            |sb| sb.active_checkpoint_seq ^= 0xFF,
        ];
        for (i, flip) in flips.iter().enumerate() {
            let mut sb = base;
            flip(&mut sb);
            assert!(!sb.is_valid(), "flip {i} was not caught by the CRC");
        }

        // The magic is rejected by its own check, not by the CRC.
        let mut sb = base;
        sb.magic = [0u8; 8];
        assert!(!sb.is_valid());
    }

    /// §C, third test: the old defect's exact shape. `is_valid` used to
    /// checksum `let mut copy = *self` — a struct copy, which is not
    /// required to reproduce the padding bytes `seal` saw, so a structurally
    /// identical block could fail its own CRC. A copy is now indistinguishable
    /// from its original, because neither one's padding is read.
    #[test]
    fn a_copy_of_a_sealed_block_validates() {
        let mut sb = StoreSuperblock::new(11);
        sb.seal();
        let copy = sb;
        assert!(copy.is_valid());
        assert_eq!(copy.checksum_input(), sb.checksum_input());
        assert_eq!(crc32(&copy.checksum_input()), sb.crc32);
    }

    /// The checksum input carries no padding: its width is the sum of the
    /// named fields, which is smaller than the struct it came from.
    #[test]
    fn checksum_input_is_smaller_than_the_struct() {
        assert_eq!(SUPERBLOCK_CHECKSUM_BYTES, 46);
        assert!(SUPERBLOCK_CHECKSUM_BYTES < core::mem::size_of::<StoreSuperblock>());
    }
}
