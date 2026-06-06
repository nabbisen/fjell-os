//! virtio-mmio ring descriptor management (RFC v0.4-001 §6.4).
//!
//! The ring is a power-of-two circular buffer of fixed-size descriptors.
//! All index arithmetic is wrapping; the driver tracks `avail` and `used`
//! counters separately from the virtio-specified available/used rings to
//! minimise DMA coherency requirements.

use fjell_net_format::{NET_RING_DESCRIPTORS, NET_DESCRIPTOR_PAYLOAD};

/// Number of ring slots (must equal `NET_RING_DESCRIPTORS`).
pub const RING_SIZE: usize = NET_RING_DESCRIPTORS;

/// A raw ring index (0 to `RING_SIZE - 1`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RingIndex(pub u8);

impl RingIndex {
    /// Advance the index, wrapping at `RING_SIZE`.
    pub fn next(self) -> Self {
        Self((self.0 + 1) % RING_SIZE as u8)
    }
    pub const fn as_usize(self) -> usize { self.0 as usize }
}

/// A monotonically-increasing counter used to derive ring indices.
///
/// `idx()` returns the counter modulo `RING_SIZE` — invariant of
/// how many times the ring has wrapped.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RingIndexCounter(pub u32);

impl RingIndexCounter {
    pub fn idx(self) -> RingIndex {
        RingIndex((self.0 % RING_SIZE as u32) as u8)
    }
    pub fn advance(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
    pub fn distance_to(self, other: Self) -> u32 {
        other.0.wrapping_sub(self.0)
    }
}

/// A single ring descriptor (header + payload-length metadata).
///
/// The payload bytes themselves live in DMA-mapped memory; only the
/// metadata is tracked here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RingDescriptor {
    /// Number of valid payload bytes in this slot.
    pub len:   u16,
    /// Driver-private flags; MBZ for external consumers.
    pub flags: u16,
    /// Whether this slot is occupied (a packet is in-flight).
    pub used:  bool,
}

impl RingDescriptor {
    pub const EMPTY: Self = Self { len: 0, flags: 0, used: false };
}

/// Errors from ring operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RingError {
    /// The ring is full (all `RING_SIZE` slots are occupied).
    RingFull               = 0x01,
    /// The ring index is out of bounds (>= `RING_SIZE`).
    IndexOutOfBounds       = 0x02,
    /// The slot is already free when a free operation was requested.
    SlotAlreadyFree        = 0x03,
    /// The requested packet length exceeds `NET_DESCRIPTOR_PAYLOAD`.
    PacketTooLarge         = 0x04,
    /// A descriptor has an internal inconsistency (reserved MBZ set).
    MalformedDescriptor    = 0x05,
}

/// Descriptor ring state for one direction (RX or TX).
#[derive(Clone, Copy, Debug)]
pub struct Ring {
    descriptors: [RingDescriptor; RING_SIZE],
    /// Next slot to fill for outbound (TX) or driver-fill (RX pre-post).
    head: RingIndexCounter,
    /// Next slot to consume / return.
    tail: RingIndexCounter,
    /// Count of currently occupied slots.
    occupied: u8,
    /// Set when any descriptor's reserved MBZ bits were non-zero.
    faulted: bool,
}

impl Ring {
    pub const fn new() -> Self {
        Self {
            descriptors: [RingDescriptor::EMPTY; RING_SIZE],
            head: RingIndexCounter(0),
            tail: RingIndexCounter(0),
            occupied: 0,
            faulted: false,
        }
    }

    pub fn is_full(&self) -> bool   { self.occupied as usize >= RING_SIZE }
    pub fn is_empty(&self) -> bool  { self.occupied == 0 }
    pub fn is_faulted(&self) -> bool { self.faulted }
    pub fn occupied(&self) -> u8    { self.occupied }

    /// Push a descriptor, returning the ring index it was placed at.
    pub fn push(&mut self, len: u16, flags: u16) -> Result<RingIndex, RingError> {
        if self.faulted { return Err(RingError::MalformedDescriptor); }
        if self.is_full() { return Err(RingError::RingFull); }
        if len as usize > NET_DESCRIPTOR_PAYLOAD { return Err(RingError::PacketTooLarge); }
        // MBZ check: bits 8..15 of flags must be zero in v0.4.
        if flags & 0xFF00 != 0 {
            self.faulted = true;
            return Err(RingError::MalformedDescriptor);
        }
        let idx = self.head.idx();
        self.descriptors[idx.as_usize()] = RingDescriptor { len, flags, used: true };
        self.head = self.head.advance();
        self.occupied += 1;
        Ok(idx)
    }

    /// Pop the descriptor at the tail, returning the index and descriptor.
    pub fn pop(&mut self) -> Result<(RingIndex, RingDescriptor), RingError> {
        if self.is_empty() { return Err(RingError::SlotAlreadyFree); }
        let idx = self.tail.idx();
        let desc = self.descriptors[idx.as_usize()];
        if !desc.used { return Err(RingError::SlotAlreadyFree); }
        self.descriptors[idx.as_usize()].used = false;
        self.tail = self.tail.advance();
        self.occupied -= 1;
        Ok((idx, desc))
    }

    /// Peek at a descriptor by raw ring index without consuming it.
    pub fn peek(&self, idx: RingIndex) -> Result<RingDescriptor, RingError> {
        if idx.as_usize() >= RING_SIZE { return Err(RingError::IndexOutOfBounds); }
        Ok(self.descriptors[idx.as_usize()])
    }

    /// Return the head-counter modulo `RING_SIZE` (for IPC ring_idx field).
    pub fn head_idx(&self) -> RingIndex { self.head.idx() }
}
