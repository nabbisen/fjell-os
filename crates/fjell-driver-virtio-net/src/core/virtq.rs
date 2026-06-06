//! Virtio split-queue descriptor management (virtio spec 1.2 §2.7).
//!
//! Implements the host-testable queue descriptor logic for the real
//! RX/TX path (RFC-v0.7.3-001).
//!
//! The physical DMA buffers are owned by the driver binary and passed
//! in as byte slices; this module tracks indices and metadata only.

use fjell_net_format::NET_RING_DESCRIPTORS;

/// Number of descriptors per virtqueue (power of 2, ≤ NET_RING_DESCRIPTORS).
pub const QUEUE_SIZE: u16 = NET_RING_DESCRIPTORS as u16;

/// Virtio queue descriptor flags (spec §2.7.5).
pub const VRING_DESC_F_NEXT:     u16 = 1;  // descriptor is part of a chain
pub const VRING_DESC_F_WRITE:    u16 = 2;  // device writes to this buffer
pub const VRING_DESC_F_INDIRECT: u16 = 4;  // buffer is an indirect table

/// One virtio queue descriptor.
/// 16 bytes. Stored contiguously in DMA-mapped memory.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct VirtqDesc {
    /// Physical address of the buffer.
    pub addr:  u64,
    /// Length of the buffer in bytes.
    pub len:   u32,
    /// Flags (VRING_DESC_F_*).
    pub flags: u16,
    /// Index of the next descriptor (if VRING_DESC_F_NEXT set).
    pub next:  u16,
}

/// Available ring — driver → device.
/// The driver writes descriptor indices here and bumps `avail_idx`.
#[derive(Debug)]
pub struct AvailRing {
    pub flags:     u16,
    pub idx:       u16,
    /// Descriptor indices posted by the driver.
    pub ring:      [u16; QUEUE_SIZE as usize],
}

impl AvailRing {
    pub const fn new() -> Self {
        Self { flags: 0, idx: 0, ring: [0; QUEUE_SIZE as usize] }
    }

    /// Post a descriptor to the device.
    pub fn post(&mut self, desc_idx: u16) {
        let slot = (self.idx % QUEUE_SIZE) as usize;
        self.ring[slot] = desc_idx;
        self.idx = self.idx.wrapping_add(1);
    }
}

/// Used ring element — device → driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct VirtqUsedElem {
    /// Descriptor index the device processed.
    pub id:  u32,
    /// Bytes written (for write descriptors).
    pub len: u32,
}

/// Used ring — device → driver.
/// The device writes `VirtqUsedElem`s and bumps `used_idx`.
#[derive(Debug)]
pub struct UsedRing {
    pub flags:     u16,
    pub idx:       u16,
    pub ring:      [VirtqUsedElem; QUEUE_SIZE as usize],
    /// Driver's last-seen `idx` from the used ring.
    pub last_seen: u16,
}

impl UsedRing {
    pub const fn new() -> Self {
        Self {
            flags:     0,
            idx:       0,
            ring:      [VirtqUsedElem { id: 0, len: 0 }; QUEUE_SIZE as usize],
            last_seen: 0,
        }
    }

    /// Check if there is a newly completed element from the device.
    pub fn has_pending(&self) -> bool {
        self.idx != self.last_seen
    }

    /// Consume the next completed element. Returns `None` if none pending.
    pub fn consume_next(&mut self) -> Option<VirtqUsedElem> {
        if !self.has_pending() { return None; }
        let slot = (self.last_seen % QUEUE_SIZE) as usize;
        let elem = self.ring[slot];
        self.last_seen = self.last_seen.wrapping_add(1);
        Some(elem)
    }
}

/// Tracks which descriptors are free vs. in-flight.
/// Simple bitset — 16 entries maximum.
#[derive(Debug)]
pub struct DescriptorAllocator {
    /// Bitmask: bit `i` set = descriptor `i` is free.
    free_mask: u16,
    total:     u16,
}

impl DescriptorAllocator {
    pub const fn new(total: u16) -> Self {
        // All descriptors start free.
        let free_mask = if total >= 16 { 0xFFFF } else { (1u16 << total) - 1 };
        Self { free_mask, total }
    }

    /// Allocate a free descriptor index. Returns `None` if all are in flight.
    pub fn alloc(&mut self) -> Option<u16> {
        if self.free_mask == 0 { return None; }
        let idx = self.free_mask.trailing_zeros() as u16;
        self.free_mask &= !(1 << idx);
        Some(idx)
    }

    /// Return a descriptor to the free pool.
    pub fn free(&mut self, idx: u16) {
        if idx < self.total {
            self.free_mask |= 1 << idx;
        }
    }

    pub fn available(&self) -> u16 {
        self.free_mask.count_ones() as u16
    }
}

/// Combined per-direction queue state (RFC-v0.7.3-001).
pub struct VirtQueue {
    pub descs:    [VirtqDesc; QUEUE_SIZE as usize],
    pub avail:    AvailRing,
    pub used:     UsedRing,
    pub alloc:    DescriptorAllocator,
}

impl VirtQueue {
    pub const fn new() -> Self {
        Self {
            descs: [VirtqDesc { addr: 0, len: 0, flags: 0, next: 0 }; QUEUE_SIZE as usize],
            avail: AvailRing::new(),
            used:  UsedRing::new(),
            alloc: DescriptorAllocator::new(QUEUE_SIZE),
        }
    }

    /// Post a TX buffer: allocate a descriptor, fill it, post to avail ring.
    /// Returns the descriptor index, or `None` if the queue is full.
    pub fn post_tx(&mut self, phys_addr: u64, len: u32) -> Option<u16> {
        let idx = self.alloc.alloc()?;
        self.descs[idx as usize] = VirtqDesc {
            addr:  phys_addr,
            len,
            flags: 0,  // no NEXT, device reads this buffer
            next:  0,
        };
        self.avail.post(idx);
        Some(idx)
    }

    /// Post an RX buffer: allocate a descriptor, mark WRITE, post to avail ring.
    /// Returns the descriptor index, or `None` if the queue is full.
    pub fn post_rx_buffer(&mut self, phys_addr: u64, buf_len: u32) -> Option<u16> {
        let idx = self.alloc.alloc()?;
        self.descs[idx as usize] = VirtqDesc {
            addr:  phys_addr,
            len:   buf_len,
            flags: VRING_DESC_F_WRITE,  // device writes received packet here
            next:  0,
        };
        self.avail.post(idx);
        Some(idx)
    }

    /// Check for and consume a completed used-ring entry from the device.
    /// Returns (descriptor_index, bytes_written) or `None`.
    pub fn pop_used(&mut self) -> Option<(u16, u32)> {
        let elem = self.used.consume_next()?;
        let idx  = elem.id as u16;
        self.alloc.free(idx);  // return descriptor to pool
        Some((idx, elem.len))
    }
}

#[cfg(test)]
mod virtq_tests {
    use super::*;

    #[test]
    fn descriptor_allocator_starts_full() {
        let mut alloc = DescriptorAllocator::new(4);
        assert_eq!(alloc.available(), 4);
    }

    #[test]
    fn descriptor_allocator_alloc_and_free() {
        let mut alloc = DescriptorAllocator::new(4);
        let idx = alloc.alloc().unwrap();
        assert_eq!(alloc.available(), 3);
        alloc.free(idx);
        assert_eq!(alloc.available(), 4);
    }

    #[test]
    fn descriptor_allocator_exhausted_returns_none() {
        let mut alloc = DescriptorAllocator::new(2);
        alloc.alloc().unwrap();
        alloc.alloc().unwrap();
        assert_eq!(alloc.alloc(), None);
    }

    #[test]
    fn avail_ring_post_wraps() {
        let mut avail = AvailRing::new();
        for i in 0..QUEUE_SIZE {
            avail.post(i);
        }
        assert_eq!(avail.idx, QUEUE_SIZE);
        // Wraps: post at slot 0 again
        avail.post(0);
        assert_eq!(avail.ring[0], 0);
    }

    #[test]
    fn used_ring_consume_next() {
        let mut used = UsedRing::new();
        used.ring[0] = VirtqUsedElem { id: 3, len: 100 };
        used.idx = 1;
        let elem = used.consume_next().unwrap();
        assert_eq!(elem.id, 3);
        assert_eq!(elem.len, 100);
        assert_eq!(used.last_seen, 1);
    }

    #[test]
    fn used_ring_empty_returns_none() {
        let mut used = UsedRing::new();
        assert_eq!(used.consume_next(), None);
    }

    #[test]
    fn virtqueue_post_tx_fills_descriptor() {
        let mut q = VirtQueue::new();
        let idx = q.post_tx(0xDEAD_BEEF, 128).unwrap();
        assert_eq!(q.descs[idx as usize].addr, 0xDEAD_BEEF);
        assert_eq!(q.descs[idx as usize].len,  128);
        assert_eq!(q.descs[idx as usize].flags, 0);  // TX: no WRITE flag
    }

    #[test]
    fn virtqueue_post_rx_buffer_sets_write_flag() {
        let mut q = VirtQueue::new();
        let idx = q.post_rx_buffer(0x1000_0000, 240).unwrap();
        assert_eq!(q.descs[idx as usize].flags, VRING_DESC_F_WRITE);
    }

    #[test]
    fn virtqueue_pop_used_frees_descriptor() {
        let mut q = VirtQueue::new();
        let tx_idx = q.post_tx(0x2000, 64).unwrap();
        // Simulate device completing the TX
        q.used.ring[0] = VirtqUsedElem { id: tx_idx as u32, len: 64 };
        q.used.idx = 1;
        let (ret_idx, len) = q.pop_used().unwrap();
        assert_eq!(ret_idx, tx_idx);
        assert_eq!(len, 64);
        // Descriptor should be free again
        assert!(q.alloc.available() > 0);
    }

    #[test]
    fn virtqueue_full_returns_none() {
        let mut q = VirtQueue::new();
        // Exhaust all descriptors
        for _ in 0..QUEUE_SIZE {
            q.post_tx(0x1000, 64).unwrap();
        }
        assert_eq!(q.post_tx(0x1000, 64), None);
    }
}
