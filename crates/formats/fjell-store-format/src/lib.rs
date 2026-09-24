//! Persistent append-only state store format for Fjell OS M6.
#![no_std]

use fjell_canon::{BufSink, Canon};

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
///
/// 3 (RFC-0.33-003 D5, E-055): **the bytes written to the sector are that same
/// serialisation**, not a view of the struct's memory. Through version 2 `init`
/// wrote `size_of::<StoreSuperblock>()` bytes — 64 — of which 14 were compiler
/// padding whose value nothing defined; the block is now 50 named,
/// little-endian bytes (`SUPERBLOCK_BYTES`) and the rest of the sector is zero.
/// Nothing reads a superblock back from disk, so no data is migrated.
pub const STORE_SUPERBLOCK_VERSION: u16 = 3;

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
/// `write_covered` fails the `assert_eq!` in the tests.
const SUPERBLOCK_CHECKSUM_BYTES: usize = 8 + 2 + 8 + 4 + 8 + 8 + 8;

/// Width of a serialised `StoreSuperblock`: the checksummed fields and `crc32`.
pub const SUPERBLOCK_BYTES: usize = SUPERBLOCK_CHECKSUM_BYTES + 4;

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

    /// Every field the CRC covers, named, little-endian, in declaration order,
    /// with `crc32` itself excluded.
    ///
    /// This is the serialisation, not a view of the struct's memory. It
    /// contains no padding, so it does not depend on the layout the compiler
    /// chose, and `seal` and `is_valid` cannot disagree about what it holds.
    pub fn write_covered(&self, c: &mut dyn Canon) {
        c.bytes("magic", &self.magic);
        c.u16("version", self.version);
        c.u64("generation", self.generation);
        c.u32("sector_size", self.sector_size);
        c.u64("log_start_lba", self.log_start_lba);
        c.u64("log_tail_seq", self.log_tail_seq);
        c.u64("active_checkpoint_seq", self.active_checkpoint_seq);
    }

    /// The whole on-disk block: [`Self::write_covered`], then `crc32`. **This is
    /// the function the sector is written from and the frozen schema is
    /// generated from** — one description, read two ways.
    pub fn write_canonical(&self, c: &mut dyn Canon) {
        self.write_covered(c);
        c.u32("crc32", self.crc32);
    }

    /// The bytes to put on disk: every field, named, no padding. The caller
    /// zero-fills the rest of the sector explicitly.
    pub fn encode(&self) -> [u8; SUPERBLOCK_BYTES] {
        let mut sink = BufSink::<SUPERBLOCK_BYTES>::new();
        self.write_canonical(&mut sink);
        debug_assert_eq!(sink.len(), SUPERBLOCK_BYTES);
        *sink
            .bytes()
            .first_chunk::<SUPERBLOCK_BYTES>()
            .expect("full")
    }

    /// The bytes the CRC is computed over.
    fn checksum_input(&self) -> [u8; SUPERBLOCK_CHECKSUM_BYTES] {
        let mut sink = BufSink::<SUPERBLOCK_CHECKSUM_BYTES>::new();
        self.write_covered(&mut sink);
        debug_assert_eq!(sink.len(), SUPERBLOCK_CHECKSUM_BYTES);
        *sink
            .bytes()
            .first_chunk::<SUPERBLOCK_CHECKSUM_BYTES>()
            .expect("full")
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

/// Width of a serialised `RecordHeader`: 4 + 2 + 2 + 8 + 4 + 4. It has no
/// padding, so this equals `size_of::<RecordHeader>()` — a test says so with the
/// field offsets rather than assuming it.
pub const RECORD_HEADER_BYTES: usize = 4 + 2 + 2 + 8 + 4 + 4;

/// On-disk version of `RecordHeader`. **Unchanged by RFC-0.33-003**: unlike
/// `StoreSuperblock` this header has no padding, so the bytes the serialiser
/// writes are the bytes the old struct view wrote (checked in the tests by field
/// offset), and D4's "version moves where bytes moved" does not apply.
pub const RECORD_HEADER_VERSION: u16 = 1;

impl RecordHeader {
    pub fn new(kind: RecordKind, seq: u64, payload_len: usize) -> Self {
        RecordHeader {
            magic: RECORD_MAGIC,
            version: RECORD_HEADER_VERSION,
            kind: kind as u16,
            seq,
            total_len: (RECORD_HEADER_BYTES + payload_len) as u32,
            crc32: 0,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == RECORD_MAGIC
    }

    /// Every field, named, little-endian, in declaration order. **The function
    /// the header is written from and its frozen schema is generated from.**
    ///
    /// `crc32` is written as it stands and is `0`: nothing computes a record
    /// checksum today and `is_valid` checks only the magic. That is a fact about
    /// the format's design, recorded here rather than papered over, and out of
    /// this line's scope.
    pub fn write_canonical(&self, c: &mut dyn Canon) {
        c.u32("magic", self.magic);
        c.u16("version", self.version);
        c.u16("kind", self.kind);
        c.u64("seq", self.seq);
        c.u32("total_len", self.total_len);
        c.u32("crc32", self.crc32);
    }

    /// The bytes to put on disk: every field, named, no padding.
    pub fn encode(&self) -> [u8; RECORD_HEADER_BYTES] {
        let mut sink = BufSink::<RECORD_HEADER_BYTES>::new();
        self.write_canonical(&mut sink);
        debug_assert_eq!(sink.len(), RECORD_HEADER_BYTES);
        *sink
            .bytes()
            .first_chunk::<RECORD_HEADER_BYTES>()
            .expect("full")
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

    // ── RFC-0.33-003 D5 / E-055: the bytes on disk ─────────────────────────────

    /// **No byte of the block is indeterminate.** Not "it round-trips": the same
    /// serialiser is run into a buffer pre-filled with `0x00` and into one
    /// pre-filled with `0xFF`, and the two outputs must agree everywhere. A byte
    /// that no named field wrote — which is what padding copied out of a struct
    /// is — would show the fill and make them differ (`fjell-canon` has the
    /// control that proves that).
    #[test]
    fn no_byte_of_the_superblock_is_indeterminate() {
        let mut sb = StoreSuperblock::new(0x0102_0304_0506_0708);
        sb.log_tail_seq = 0x1112_1314_1516_1718;
        sb.active_checkpoint_seq = 0x2122_2324_2526_2728;
        sb.seal();
        let (mut a, mut b) = (
            BufSink::<SUPERBLOCK_BYTES>::filled(0x00),
            BufSink::<SUPERBLOCK_BYTES>::filled(0xFF),
        );
        sb.write_canonical(&mut a);
        sb.write_canonical(&mut b);
        assert_eq!(
            a.bytes().len(),
            SUPERBLOCK_BYTES,
            "every byte of the block is written"
        );
        assert_eq!(a.bytes(), b.bytes());
    }

    #[test]
    fn no_byte_of_the_record_header_is_indeterminate() {
        let h = RecordHeader::new(RecordKind::ServiceState, 0x0102_0304_0506_0708, 9);
        let (mut a, mut b) = (
            BufSink::<RECORD_HEADER_BYTES>::filled(0x00),
            BufSink::<RECORD_HEADER_BYTES>::filled(0xFF),
        );
        h.write_canonical(&mut a);
        h.write_canonical(&mut b);
        assert_eq!(a.bytes().len(), RECORD_HEADER_BYTES);
        assert_eq!(a.bytes(), b.bytes());
    }

    /// What E-055 was: the struct HAS padding, and the old write copied it.
    /// Fourteen of the superblock's 64 in-memory bytes are padding; the
    /// serialisation is the 50 that are fields.
    #[test]
    fn the_struct_has_padding_the_serialisation_does_not() {
        assert_eq!(core::mem::size_of::<StoreSuperblock>(), 64);
        assert_eq!(SUPERBLOCK_BYTES, 50);
        assert_eq!(
            core::mem::size_of::<StoreSuperblock>() - SUPERBLOCK_BYTES,
            14
        );
        // The padding is where it always was: `generation` does not start at 10.
        assert_eq!(core::mem::offset_of!(StoreSuperblock, generation), 16);
    }

    /// `RecordHeader` has none, which is why its version did not move: the bytes
    /// the serialiser writes are the offsets the struct always had.
    #[test]
    fn the_record_header_has_no_padding_so_its_bytes_did_not_change() {
        assert_eq!(core::mem::size_of::<RecordHeader>(), RECORD_HEADER_BYTES);
        assert_eq!(core::mem::offset_of!(RecordHeader, magic), 0);
        assert_eq!(core::mem::offset_of!(RecordHeader, version), 4);
        assert_eq!(core::mem::offset_of!(RecordHeader, kind), 6);
        assert_eq!(core::mem::offset_of!(RecordHeader, seq), 8);
        assert_eq!(core::mem::offset_of!(RecordHeader, total_len), 16);
        assert_eq!(core::mem::offset_of!(RecordHeader, crc32), 20);
        let h = RecordHeader::new(RecordKind::AuditEvent, 5, 0);
        let e = h.encode();
        assert_eq!(&e[0..4], &RECORD_MAGIC.to_le_bytes());
        assert_eq!(&e[8..16], &5u64.to_le_bytes());
        assert_eq!(&e[16..20], &(RECORD_HEADER_BYTES as u32).to_le_bytes());
    }

    /// The CRC covers exactly the serialised bytes before `crc32`, and `crc32`
    /// is the last four bytes: the checksum and the sector cannot disagree
    /// about what a block contains, because they are one function.
    #[test]
    fn the_crc_covers_exactly_the_bytes_written_before_it() {
        let mut sb = StoreSuperblock::new(9);
        sb.seal();
        let e = sb.encode();
        assert_eq!(&e[..SUPERBLOCK_CHECKSUM_BYTES], &sb.checksum_input());
        assert_eq!(&e[SUPERBLOCK_CHECKSUM_BYTES..], &sb.crc32.to_le_bytes());
        assert_eq!(crc32(&e[..SUPERBLOCK_CHECKSUM_BYTES]), sb.crc32);
    }

    /// The version moved because the bytes moved.
    #[test]
    fn the_superblock_version_moved_with_its_bytes() {
        assert_eq!(STORE_SUPERBLOCK_VERSION, 3);
        assert_eq!(StoreSuperblock::new(1).version, 3);
        assert_eq!(
            RecordHeader::new(RecordKind::AuditEvent, 1, 0).version,
            RECORD_HEADER_VERSION
        );
    }
}
