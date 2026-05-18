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


pub const STORE_MAGIC:  [u8; 8] = *b"FJSTORE\0";
pub const RECORD_MAGIC: u32     = 0x464A_4C52; // "FJLR"

pub const LBA_BOOT_CTL_A_START: u64 = 1;
pub const LBA_BOOT_CTL_A_END:   u64 = 32;
pub const LBA_BOOT_CTL_B_START: u64 = 33;
pub const LBA_BOOT_CTL_B_END:   u64 = 64;
pub const LBA_SUPERBLOCK_A:     u64 = 65;
pub const LBA_SUPERBLOCK_B:     u64 = 129;
pub const LBA_LOG_START:        u64 = 193;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StoreSuperblock {
    pub magic:                [u8; 8],
    pub version:              u16,
    pub generation:           u64,
    pub sector_size:          u32,
    pub log_start_lba:        u64,
    pub log_tail_seq:         u64,
    pub active_checkpoint_seq: u64,
    pub crc32:                u32,
}

impl StoreSuperblock {
    pub fn new(generation: u64) -> Self {
        StoreSuperblock {
            magic: STORE_MAGIC, version: 1, generation,
            sector_size: 512, log_start_lba: LBA_LOG_START,
            log_tail_seq: 0, active_checkpoint_seq: 0, crc32: 0,
        }
    }

    /// Compute and store CRC32 (RFC 008).  Call before writing to disk.
    pub fn seal(&mut self) {
        self.crc32 = 0;
        let bytes = unsafe { core::slice::from_raw_parts(
            self as *const _ as *const u8, core::mem::size_of::<Self>()) };
        self.crc32 = crc32(bytes);
    }

    /// Returns true if magic is correct AND CRC32 matches (RFC 008).
    pub fn is_valid(&self) -> bool {
        if self.magic != STORE_MAGIC { return false; }
        let mut copy = *self;
        copy.crc32 = 0;
        let bytes = unsafe { core::slice::from_raw_parts(
            &copy as *const _ as *const u8, core::mem::size_of::<Self>()) };
        crc32(bytes) == self.crc32
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordHeader {
    pub magic:      u32,
    pub version:    u16,
    pub kind:       u16,
    pub seq:        u64,
    pub total_len:  u32,
    pub crc32:      u32,
}

impl RecordHeader {
    pub fn new(kind: RecordKind, seq: u64, payload_len: usize) -> Self {
        RecordHeader {
            magic: RECORD_MAGIC, version: 1,
            kind: kind as u16, seq,
            total_len: (core::mem::size_of::<RecordHeader>() + payload_len) as u32,
            crc32: 0,
        }
    }
    pub fn is_valid(&self) -> bool { self.magic == RECORD_MAGIC }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    AuditEvent        = 1,
    ConfigSnapshot    = 2,
    ServiceState      = 3,
    DeviceInventory   = 4,
    StoreCheckpoint   = 5,
    UpgradeTransaction= 6,
    BootControlEvent  = 7,
    PowerTelemetry    = 8,
}
