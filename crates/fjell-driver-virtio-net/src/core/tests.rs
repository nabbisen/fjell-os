//! Host unit tests for the virtio-net driver core (RFC v0.4-001 §11.1).

#[allow(unused_imports)] // v0.7: DRIVER_ACCEPTED_FEATURES used in feature negotiation tests
use crate::features::{
    negotiate_features, VirtioFeatureFlags,
    VIRTIO_NET_F_MAC, VIRTIO_NET_F_STATUS, VIRTIO_NET_F_MRG_RXBUF,
    VIRTIO_F_EVENT_IDX, DRIVER_ACCEPTED_FEATURES,
};
use crate::ring::{Ring, RingError, RingIndex, RingIndexCounter, RING_SIZE};
use crate::state::{DriverState, DriverStateBlock, DriverStateError};
#[allow(unused_imports)] // v0.7: ring count and payload size used in descriptor allocation tests
use fjell_net_format::{NET_RING_DESCRIPTORS, NET_DESCRIPTOR_PAYLOAD};

// ── Feature negotiation ───────────────────────────────────────────────────────

#[test]
fn feature_negotiation_picks_mac_and_status() {
    let offered = VirtioFeatureFlags(VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS);
    let (neg, _) = negotiate_features(offered);
    assert!(neg.contains(VIRTIO_NET_F_MAC));
    assert!(neg.contains(VIRTIO_NET_F_STATUS));
}

#[test]
fn feature_negotiation_drops_unsupported_bits() {
    let offered = VirtioFeatureFlags(
        VIRTIO_NET_F_MAC | VIRTIO_NET_F_MRG_RXBUF | VIRTIO_F_EVENT_IDX,
    );
    let (neg, _) = negotiate_features(offered);
    assert!(neg.contains(VIRTIO_NET_F_MAC));
    assert!(!neg.contains(VIRTIO_NET_F_MRG_RXBUF));
    assert!(!neg.contains(VIRTIO_F_EVENT_IDX));
}

#[test]
fn feature_negotiation_picks_legacy_when_version1_unset() {
    // Device does NOT offer VIRTIO_F_VERSION_1 (bit 32).
    let offered = VirtioFeatureFlags(VIRTIO_NET_F_MAC);
    let (_, legacy) = negotiate_features(offered);
    assert!(legacy, "device without VIRTIO_F_VERSION_1 must be legacy");
}

#[test]
fn feature_negotiation_not_legacy_when_version1_set() {
    const VIRTIO_F_VERSION_1: u64 = 1 << 32;
    let offered = VirtioFeatureFlags(VIRTIO_NET_F_MAC | VIRTIO_F_VERSION_1);
    let (_, legacy) = negotiate_features(offered);
    assert!(!legacy);
}

#[test]
fn feature_flags_bitwise_ops() {
    let f = VirtioFeatureFlags::default();
    let f2 = f.with(VIRTIO_NET_F_MAC);
    assert!(f2.contains(VIRTIO_NET_F_MAC));
    let f3 = f2.without(VIRTIO_NET_F_MAC);
    assert!(!f3.contains(VIRTIO_NET_F_MAC));
}

// ── Ring index math ───────────────────────────────────────────────────────────

#[test]
fn ring_index_wraps_at_ring_size() {
    let last = RingIndex((RING_SIZE - 1) as u8);
    assert_eq!(last.next(), RingIndex(0));
}

#[test]
fn ring_index_counter_wraps() {
    // After RING_SIZE advances, idx() wraps back to 0.
    let mut c = RingIndexCounter(0);
    for _ in 0..RING_SIZE {
        c = c.advance();
    }
    assert_eq!(c.idx(), RingIndex(0));
}

#[test]
fn tx_ring_index_wraps_after_ring_size_pushes() {
    let mut ring = Ring::new();
    // Fill and drain the ring twice to confirm wrapping.
    for pass in 0..2u32 {
        for _ in 0..RING_SIZE {
            ring.push(64, 0).unwrap();
        }
        for _ in 0..RING_SIZE {
            ring.pop().unwrap();
        }
        assert!(ring.is_empty(), "ring should be empty after pass {pass}");
    }
}

#[test]
fn rx_ring_index_wraps() {
    // Mirror of the TX test: same ring type, same wrapping behaviour.
    let mut ring = Ring::new();
    for _ in 0..(RING_SIZE * 3) {
        ring.push(128, 0).unwrap();
        ring.pop().unwrap();
    }
    assert!(ring.is_empty());
}

// ── Ring push/pop ─────────────────────────────────────────────────────────────

#[test]
fn ring_full_push_returns_error() {
    let mut ring = Ring::new();
    for _ in 0..RING_SIZE {
        ring.push(1, 0).unwrap();
    }
    assert_eq!(ring.push(1, 0), Err(RingError::RingFull));
}

#[test]
fn ring_empty_pop_returns_error() {
    let mut ring = Ring::new();
    assert_eq!(ring.pop(), Err(RingError::SlotAlreadyFree));
}

#[test]
fn packet_too_large_rejected() {
    let mut ring = Ring::new();
    let too_large = (NET_DESCRIPTOR_PAYLOAD + 1) as u16;
    assert_eq!(ring.push(too_large, 0), Err(RingError::PacketTooLarge));
}

#[test]
fn malformed_descriptor_marks_ring_faulted() {
    let mut ring = Ring::new();
    // Flags with MBZ bits set triggers fault.
    let _ = ring.push(1, 0xFF00);
    assert!(ring.is_faulted());
    // Subsequent pushes also fail.
    assert_eq!(ring.push(1, 0), Err(RingError::MalformedDescriptor));
}

#[test]
fn ring_occupied_count_accurate() {
    let mut ring = Ring::new();
    assert_eq!(ring.occupied(), 0);
    ring.push(10, 0).unwrap();
    ring.push(20, 0).unwrap();
    assert_eq!(ring.occupied(), 2);
    ring.pop().unwrap();
    assert_eq!(ring.occupied(), 1);
}

// ── Driver state machine ──────────────────────────────────────────────────────

#[test]
fn driver_starts_in_boot_state() {
    let dsb = DriverStateBlock::new();
    assert_eq!(dsb.state, DriverState::Boot);
}

#[test]
fn driver_state_valid_transitions() {
    let mut dsb = DriverStateBlock::new();
    dsb.transition(DriverState::Init).unwrap();
    dsb.transition(DriverState::Ready).unwrap();
    dsb.transition(DriverState::HandleRx).unwrap();
    dsb.transition(DriverState::Ready).unwrap();
}

#[test]
fn driver_state_invalid_transition_rejected() {
    let mut dsb = DriverStateBlock::new();
    // Boot → Ready is not a valid transition (must go through Init).
    assert_eq!(
        dsb.transition(DriverState::Ready),
        Err(DriverStateError::InvalidTransition),
    );
}

#[test]
fn driver_fault_sets_faulted_from_ready() {
    let mut dsb = DriverStateBlock::new();
    dsb.transition(DriverState::Init).unwrap();
    dsb.transition(DriverState::Ready).unwrap();
    dsb.fault();
    assert_eq!(dsb.state, DriverState::Faulted);
}

#[test]
fn driver_max_restarts_enforced() {
    let mut dsb = DriverStateBlock::new();
    for _ in 0..DriverStateBlock::MAX_RESTARTS {
        assert!(dsb.attempt_restart());
    }
    assert!(!dsb.attempt_restart());
    assert!(!dsb.may_restart());
}

// ── MMIO register helpers ─────────────────────────────────────────────────────

use crate::mmio::{
    read_le32, write_le32, read_mac, read_link_up,
    verify_device_identity, init_status_sequence,
    VIRTIO_MMIO_MAGIC, VIRTIO_MMIO_MAGIC_VALUE,
    VIRTIO_MMIO_DEVICE_ID, VIRTIO_NET_DEVICE_ID,
    VIRTIO_NET_CONFIG_MAC, VIRTIO_NET_CONFIG_STATUS,
    VIRTIO_NET_STATUS_LINK_UP, VIRTIO_MMIO_REGION_SIZE,
    VIRTIO_MMIO_STATUS, VIRTIO_STATUS_DRIVER_OK,
};

fn make_mmio_buf() -> [u8; VIRTIO_MMIO_REGION_SIZE] {
    [0u8; VIRTIO_MMIO_REGION_SIZE]
}

#[test]
fn mmio_read_le32_roundtrip() {
    let mut buf = make_mmio_buf();
    write_le32(&mut buf, VIRTIO_MMIO_STATUS, 0xDEAD_BEEF);
    assert_eq!(read_le32(&buf, VIRTIO_MMIO_STATUS), Some(0xDEAD_BEEF));
}

#[test]
fn mmio_write_out_of_bounds_returns_false() {
    let mut buf = make_mmio_buf();
    assert!(!write_le32(&mut buf, VIRTIO_MMIO_REGION_SIZE - 2, 0));
}

#[test]
fn mmio_read_out_of_bounds_returns_none() {
    let buf = make_mmio_buf();
    assert_eq!(read_le32(&buf, VIRTIO_MMIO_REGION_SIZE), None);
}

#[test]
fn mmio_verify_device_identity_accepts_good() {
    let mut buf = make_mmio_buf();
    write_le32(&mut buf, VIRTIO_MMIO_MAGIC,     VIRTIO_MMIO_MAGIC_VALUE);
    write_le32(&mut buf, VIRTIO_MMIO_DEVICE_ID, VIRTIO_NET_DEVICE_ID);
    assert!(verify_device_identity(&buf));
}

#[test]
fn mmio_verify_device_identity_rejects_bad_magic() {
    let mut buf = make_mmio_buf();
    write_le32(&mut buf, VIRTIO_MMIO_MAGIC,     0xDEAD_DEAD);
    write_le32(&mut buf, VIRTIO_MMIO_DEVICE_ID, VIRTIO_NET_DEVICE_ID);
    assert!(!verify_device_identity(&buf));
}

#[test]
fn mmio_read_mac_succeeds() {
    let mut buf = make_mmio_buf();
    let mac = [0x52u8, 0x54, 0x00, 0x12, 0x34, 0x56];
    buf[VIRTIO_NET_CONFIG_MAC..VIRTIO_NET_CONFIG_MAC + 6].copy_from_slice(&mac);
    assert_eq!(read_mac(&buf), Some(mac));
}

#[test]
fn mmio_read_link_up_true() {
    let mut buf = make_mmio_buf();
    let s = VIRTIO_NET_CONFIG_STATUS;
    buf[s]     = (VIRTIO_NET_STATUS_LINK_UP & 0xFF) as u8;
    buf[s + 1] = (VIRTIO_NET_STATUS_LINK_UP >> 8) as u8;
    assert!(read_link_up(&buf));
}

#[test]
fn mmio_read_link_up_false() {
    let buf = make_mmio_buf();
    assert!(!read_link_up(&buf));
}

#[test]
fn init_status_sequence_ends_with_driver_ok() {
    let seq = init_status_sequence();
    assert_eq!(seq.len(), 4);
    assert_ne!(seq[3] & VIRTIO_STATUS_DRIVER_OK, 0);
}
