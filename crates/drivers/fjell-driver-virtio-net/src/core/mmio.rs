//! virtio-mmio register layout (virtio spec 1.2 §4.2).
//!
//! All register offsets are in bytes from the device-base MMIO address.
//! The driver reads through a `[u8]` slice at the mapped region; register
//! widths are LE32 unless noted.
//!
//! This module is pure logic — no actual MMIO reads/writes — so it is
//! host-testable.

// ── Magic and version ─────────────────────────────────────────────────────────

pub const VIRTIO_MMIO_MAGIC:          usize = 0x000; // LE32, must read 0x74726976
pub const VIRTIO_MMIO_VERSION:        usize = 0x004; // LE32, 1 = legacy, 2 = modern
pub const VIRTIO_MMIO_DEVICE_ID:      usize = 0x008; // LE32, 1 = net
pub const VIRTIO_MMIO_VENDOR_ID:      usize = 0x00C; // LE32

pub const VIRTIO_MMIO_MAGIC_VALUE:    u32   = 0x74726976;
pub const VIRTIO_NET_DEVICE_ID:       u32   = 1;

// ── Feature registers ─────────────────────────────────────────────────────────

/// Features offered by the device (read, write FEATURE_SEL first).
pub const VIRTIO_MMIO_DEVICE_FEATURES:     usize = 0x010;
/// Selector for DEVICE_FEATURES (0 = bits 0..31, 1 = bits 32..63).
pub const VIRTIO_MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
/// Features activated by the driver (write, write FEATURES_SEL first).
pub const VIRTIO_MMIO_DRIVER_FEATURES:     usize = 0x020;
pub const VIRTIO_MMIO_DRIVER_FEATURES_SEL: usize = 0x024;

// ── Queue configuration ───────────────────────────────────────────────────────

pub const VIRTIO_MMIO_QUEUE_SEL:       usize = 0x030;
pub const VIRTIO_MMIO_QUEUE_NUM_MAX:   usize = 0x034; // read-only max supported size
pub const VIRTIO_MMIO_QUEUE_NUM:       usize = 0x038; // write queue size
pub const VIRTIO_MMIO_QUEUE_READY:     usize = 0x044; // 0 = not ready, 1 = ready
pub const VIRTIO_MMIO_QUEUE_NOTIFY:    usize = 0x050; // write queue index to notify
pub const VIRTIO_MMIO_QUEUE_DESC_LOW:  usize = 0x080; // low 32 bits of descriptor table PA
pub const VIRTIO_MMIO_QUEUE_DESC_HIGH: usize = 0x084; // high 32 bits
pub const VIRTIO_MMIO_QUEUE_AVAIL_LOW: usize = 0x090;
pub const VIRTIO_MMIO_QUEUE_AVAIL_HIGH:usize = 0x094;
pub const VIRTIO_MMIO_QUEUE_USED_LOW:  usize = 0x0A0;
pub const VIRTIO_MMIO_QUEUE_USED_HIGH: usize = 0x0A4;

// ── Status register ───────────────────────────────────────────────────────────

pub const VIRTIO_MMIO_STATUS:          usize = 0x070;

/// Reset: write 0 to STATUS.
pub const VIRTIO_STATUS_RESET:         u32 = 0x00;
/// Device acknowledged.
pub const VIRTIO_STATUS_ACKNOWLEDGE:   u32 = 0x01;
/// Driver present.
pub const VIRTIO_STATUS_DRIVER:        u32 = 0x02;
/// Feature negotiation complete.
pub const VIRTIO_STATUS_FEATURES_OK:   u32 = 0x08;
/// Driver is live.
pub const VIRTIO_STATUS_DRIVER_OK:     u32 = 0x04;
/// Device needs reset (set by device on error).
pub const VIRTIO_STATUS_NEEDS_RESET:   u32 = 0x40;
/// Driver or device failure.
pub const VIRTIO_STATUS_FAILED:        u32 = 0x80;

// ── Interrupt status and ack ──────────────────────────────────────────────────

pub const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
pub const VIRTIO_MMIO_INTERRUPT_ACK:    usize = 0x064;
/// Used buffer notification (bit 0 of INTERRUPT_STATUS).
pub const VIRTIO_INTR_USED_BUFFER:     u32 = 0x01;
/// Configuration change notification (bit 1).
pub const VIRTIO_INTR_CONFIG_CHANGE:   u32 = 0x02;

// ── Net device config space ───────────────────────────────────────────────────

/// Start of virtio-net device-specific config space.
pub const VIRTIO_MMIO_CONFIG_BASE:     usize = 0x100;
/// MAC address: 6 bytes at CONFIG_BASE + 0..6.
pub const VIRTIO_NET_CONFIG_MAC:       usize = 0x100;
/// Status: u16 LE at CONFIG_BASE + 6.
pub const VIRTIO_NET_CONFIG_STATUS:    usize = 0x106;
/// Link up bit in STATUS.
pub const VIRTIO_NET_STATUS_LINK_UP:   u16 = 0x0001;

// ── Register access helpers ───────────────────────────────────────────────────

/// Read a LE32 register from a byte slice (simulating MMIO).
///
/// The slice must cover `[offset .. offset+4]`.  Returns `None` if out of
/// range.
#[inline]
pub fn read_le32(mmio: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    if end > mmio.len() { return None; }
    let bytes: [u8; 4] = mmio[offset..end].try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

/// Write a LE32 register into a byte slice (simulating MMIO write).
#[inline]
pub fn write_le32(mmio: &mut [u8], offset: usize, value: u32) -> bool {
    let end = match offset.checked_add(4) {
        Some(e) if e <= mmio.len() => e,
        _ => return false,
    };
    mmio[offset..end].copy_from_slice(&value.to_le_bytes());
    true
}

/// Read the 6-byte MAC address from the config space slice.
pub fn read_mac(mmio: &[u8]) -> Option<[u8; 6]> {
    let start = VIRTIO_NET_CONFIG_MAC;
    let end   = start + 6;
    if end > mmio.len() { return None; }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&mmio[start..end]);
    Some(mac)
}

/// Read the link-status bit from the config space.
pub fn read_link_up(mmio: &[u8]) -> bool {
    let start = VIRTIO_NET_CONFIG_STATUS;
    let end   = start + 2;
    if end > mmio.len() { return false; }
    let val = u16::from_le_bytes([mmio[start], mmio[start + 1]]);
    (val & VIRTIO_NET_STATUS_LINK_UP) != 0
}

/// Verify that the magic value and device ID match a virtio-net device.
pub fn verify_device_identity(mmio: &[u8]) -> bool {
    read_le32(mmio, VIRTIO_MMIO_MAGIC)     == Some(VIRTIO_MMIO_MAGIC_VALUE)
        && read_le32(mmio, VIRTIO_MMIO_DEVICE_ID) == Some(VIRTIO_NET_DEVICE_ID)
}

// ── Init sequence helpers ─────────────────────────────────────────────────────

/// Minimum size of the MMIO region required to access all registers.
pub const VIRTIO_MMIO_REGION_SIZE: usize = 0x110;

/// Produce the sequence of STATUS register values to write during driver init.
///
/// Returns `[ACKNOWLEDGE, DRIVER, FEATURES_OK, DRIVER_OK]`.
/// The caller writes each value in turn, checking `FEATURES_OK` readback
/// in between.
pub fn init_status_sequence() -> [u32; 4] {
    [
        VIRTIO_STATUS_ACKNOWLEDGE,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK
            | VIRTIO_STATUS_DRIVER_OK,
    ]
}
