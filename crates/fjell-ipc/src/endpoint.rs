//! Synchronous rendezvous IPC endpoint (RFC 034: lease-aware revocation).
//!
//! # Invariants
//!
//! - IPC-A: a task cannot be in both sendq and recvq simultaneously.
//! - IPC-B: payload and cap-transfer commit atomically.
//! - IPC-D: sender_badge matches the endpoint capability badge at send time.
//! - IPC-E (RFC 034): a blocked task whose lease is revoked is woken/cancelled.
//!
//! # RFC 034 additions
//!
//! `PendingMessage` carries the sender's lease binding so the revoke path can
//! identify which queued senders are bound to a given lease.
//!
//! `recvq` stores `RecvWaiter { tid, lease }` instead of bare TIDs.
//!
//! `Endpoint::cancel_by_lease` removes all entries bound to a given lease and
//! returns the cancelled TIDs so the kernel can wake them with `LeaseRevoked`.

use super::message::{MessageTag, IPC_WORDS};
use fjell_abi::error::SysError;
use fjell_abi::lease::LeaseId;
use fjell_cap::slot::LeaseBinding;

const QUEUE_DEPTH: usize = 32;

// ── Tiny fixed-capacity circular queue ───────────────────────────────────────

#[derive(Clone, Copy)]
struct FixedQueue<T: Copy, const N: usize> {
    items: [Option<T>; N],
    head:  usize,
    len:   usize,
}

impl<T: Copy, const N: usize> FixedQueue<T, N> {
    const fn new() -> Self {
        FixedQueue { items: [None; N], head: 0, len: 0 }
    }
    fn push(&mut self, v: T) -> bool {
        if self.len == N { return false; }
        self.items[(self.head + self.len) % N] = Some(v);
        self.len += 1;
        true
    }
    fn pop(&mut self) -> Option<T> {
        if self.len == 0 { return None; }
        let v = self.items[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        v
    }
    fn contains<F: Fn(T) -> bool>(&self, f: F) -> bool {
        (0..self.len).any(|i| {
            self.items[(self.head + i) % N].map_or(false, |x| f(x))
        })
    }
    /// Retain items for which `keep` returns true; collect dropped items into
    /// `removed[..*removed_count]`.
    fn retain_collect<F: Fn(T) -> bool>(
        &mut self,
        keep: F,
        removed: &mut [T; N],
        removed_count: &mut usize,
    ) {
        let mut new: FixedQueue<T, N> = FixedQueue::new();
        while let Some(v) = self.pop() {
            if keep(v) {
                new.push(v);
            } else if *removed_count < N {
                removed[*removed_count] = v;
                *removed_count += 1;
            }
        }
        *self = new;
    }
}

// ── Message snapshot ──────────────────────────────────────────────────────────

/// A message payload snapshotted at send time.
///
/// `lease` carries the sender's endpoint-cap lease binding (RFC 034).
#[derive(Clone, Copy, Debug)]
pub struct PendingMessage {
    pub tag:              MessageTag,
    pub sender_tid:       u16,
    /// RFC 055: kernel-attested ImageId of the sender.  Filled by the kernel
    /// from the sender's TCB at message-build time; cannot be forged by user space.
    pub sender_image_id:  u16,
    pub sender_badge:     u64,
    pub words:            [u64; IPC_WORDS],
    pub cap_present:      bool,
    pub cap_kind:         u8,
    pub cap_obj_id:       u32,
    pub cap_rights:       u32,
    /// True if sent via `ipc_call` (expects a reply).
    pub is_call: bool,
    /// RFC 034: endpoint cap lease binding at send/call time.
    pub lease: Option<LeaseBinding>,
}

// ── RecvWaiter ────────────────────────────────────────────────────────────────

/// A task blocked in `recvq`, carrying its endpoint lease binding (RFC 034).
#[derive(Clone, Copy, Debug)]
pub struct RecvWaiter {
    pub tid:   u16,
    /// RFC 034: endpoint cap lease binding at recv time.
    pub lease: Option<LeaseBinding>,
}

impl RecvWaiter {
    /// Waiter without a lease binding (backward compat / bootstrap).
    pub fn no_lease(tid: u16) -> Self { RecvWaiter { tid, lease: None } }
    /// Waiter with an endpoint cap lease binding.
    pub fn with_lease(tid: u16, lease: LeaseBinding) -> Self {
        RecvWaiter { tid, lease: Some(lease) }
    }
}

// ── Cancellation result ───────────────────────────────────────────────────────

/// TIDs cancelled from an endpoint by `cancel_by_lease`.
pub struct CancelledByLease {
    pub sender_tids:   [u16; QUEUE_DEPTH],
    pub receiver_tids: [u16; QUEUE_DEPTH],
    pub n_senders:   usize,
    pub n_receivers: usize,
}

impl CancelledByLease {
    fn empty() -> Self {
        CancelledByLease {
            sender_tids: [0; QUEUE_DEPTH], receiver_tids: [0; QUEUE_DEPTH],
            n_senders: 0, n_receivers: 0,
        }
    }
    /// Iterate over cancelled sender TIDs.
    pub fn senders(&self) -> &[u16] { &self.sender_tids[..self.n_senders] }
    /// Iterate over cancelled receiver TIDs.
    pub fn receivers(&self) -> &[u16] { &self.receiver_tids[..self.n_receivers] }
}

// ── Error / result types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointError {
    SendQueueFull,
    RecvQueueFull,
    AlreadyQueued,
    InvalidTag,
    /// No message pending; for non-blocking try_recv (RFC 019).
    WouldBlock,
    /// Lease revoked while blocked (RFC 034).
    LeaseRevoked,
}

impl From<EndpointError> for SysError {
    fn from(e: EndpointError) -> Self {
        match e {
            EndpointError::SendQueueFull |
            EndpointError::RecvQueueFull  => SysError::QueueFull,
            EndpointError::AlreadyQueued  => SysError::BadState,
            EndpointError::InvalidTag     => SysError::InvalidArg,
            EndpointError::WouldBlock     => SysError::WouldBlock,
            EndpointError::LeaseRevoked   => SysError::LeaseRevoked,
        }
    }
}

#[derive(Debug)]
pub enum SendResult {
    /// Message delivered directly to a waiting receiver.
    Delivered { receiver_tid: u16 },
    /// No receiver; sender enqueued — must block.
    Queued,
}

#[derive(Debug)]
pub enum RecvResult {
    /// A queued sender's message was delivered.
    Delivered(PendingMessage),
    /// No sender; receiver enqueued — must block.
    Queued,
}

// ── Endpoint ──────────────────────────────────────────────────────────────────

pub struct Endpoint {
    sendq: FixedQueue<PendingMessage, QUEUE_DEPTH>,
    recvq: FixedQueue<RecvWaiter,     QUEUE_DEPTH>,
}

impl Endpoint {
    pub const fn new() -> Self {
        Endpoint { sendq: FixedQueue::new(), recvq: FixedQueue::new() }
    }

    pub fn send(&mut self, msg: PendingMessage) -> Result<SendResult, EndpointError> {
        if !msg.tag.is_valid() { return Err(EndpointError::InvalidTag); }
        if self.recvq.contains(|w: RecvWaiter| w.tid == msg.sender_tid) {
            return Err(EndpointError::AlreadyQueued);
        }
        if let Some(w) = self.recvq.pop() {
            return Ok(SendResult::Delivered { receiver_tid: w.tid });
        }
        if !self.sendq.push(msg) { return Err(EndpointError::SendQueueFull); }
        Ok(SendResult::Queued)
    }

    /// Block-receive.  Pass a `RecvWaiter` carrying the lease binding (RFC 034).
    ///
    /// For callers without a lease use `RecvWaiter::no_lease(tid)`.
    pub fn recv(&mut self, waiter: RecvWaiter) -> Result<RecvResult, EndpointError> {
        if self.sendq.contains(|m: PendingMessage| m.sender_tid == waiter.tid) {
            return Err(EndpointError::AlreadyQueued);
        }
        if let Some(msg) = self.sendq.pop() {
            return Ok(RecvResult::Delivered(msg));
        }
        if !self.recvq.push(waiter) { return Err(EndpointError::RecvQueueFull); }
        Ok(RecvResult::Queued)
    }

    /// Non-blocking receive: return a queued message without sleeping.
    pub fn try_recv(&mut self) -> Result<PendingMessage, EndpointError> {
        self.sendq.pop().ok_or(EndpointError::WouldBlock)
    }

    /// Cancel all pending entries for `tid`.
    pub fn cancel(&mut self, tid: u16) {
        let mut rm = [PendingMessage { tag: MessageTag::new(0,0,0), sender_tid: 0, sender_image_id: 0,
            sender_badge: 0, words: [0; IPC_WORDS], cap_present: false,
            cap_kind: 0, cap_obj_id: 0, cap_rights: 0, is_call: false, lease: None,
        }; QUEUE_DEPTH];
        let mut n = 0;
        self.sendq.retain_collect(|m: PendingMessage| m.sender_tid != tid, &mut rm, &mut n);

        let mut rw = [RecvWaiter { tid: 0, lease: None }; QUEUE_DEPTH];
        n = 0;
        self.recvq.retain_collect(|w: RecvWaiter| w.tid != tid, &mut rw, &mut n);
    }

    /// RFC 034: Cancel all sendq and recvq entries whose lease binding
    /// matches `(lease_id, epoch)`.
    ///
    /// Returns the set of cancelled TIDs so the kernel can wake them with
    /// `LeaseRevoked`.
    ///
    /// Complexity: O(sendq.len + recvq.len).  Per-lease waiter lists for O(1)
    /// are a future optimisation (no use case requires it at v0.2 scale).
    pub fn cancel_by_lease(
        &mut self,
        lease_id: LeaseId,
        epoch:    u32,
    ) -> CancelledByLease {
        let mut result = CancelledByLease::empty();

        // --- cancelled senders ---
        let dummy_msg = PendingMessage {
            tag: MessageTag::new(0,0,0), sender_tid: 0, sender_image_id: 0, sender_badge: 0,
            words: [0; IPC_WORDS], cap_present: false, cap_kind: 0,
            cap_obj_id: 0, cap_rights: 0, is_call: false, lease: None,
        };
        let mut removed = [dummy_msg; QUEUE_DEPTH];
        let mut n = 0usize;
        self.sendq.retain_collect(
            |m: PendingMessage| !lease_binding_matches(m.lease, lease_id, epoch),
            &mut removed, &mut n,
        );
        for i in 0..n {
            if result.n_senders < QUEUE_DEPTH {
                result.sender_tids[result.n_senders] = removed[i].sender_tid;
                result.n_senders += 1;
            }
        }

        // --- cancelled receivers ---
        let dummy_waiter = RecvWaiter { tid: 0, lease: None };
        let mut removed_w = [dummy_waiter; QUEUE_DEPTH];
        n = 0;
        self.recvq.retain_collect(
            |w: RecvWaiter| !lease_binding_matches(w.lease, lease_id, epoch),
            &mut removed_w, &mut n,
        );
        for i in 0..n {
            if result.n_receivers < QUEUE_DEPTH {
                result.receiver_tids[result.n_receivers] = removed_w[i].tid;
                result.n_receivers += 1;
            }
        }

        result
    }

    pub fn send_queue_len(&self) -> usize { self.sendq.len }
    pub fn recv_queue_len(&self) -> usize { self.recvq.len }
}

fn lease_binding_matches(binding: Option<LeaseBinding>, id: LeaseId, epoch: u32) -> bool {
    match binding {
        Some(lb) => lb.lease_id == id && lb.epoch_at_issue.0 == epoch,
        None     => false,
    }
}

// ── Host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fjell_abi::lease::{LeaseEpoch, LeaseId};

    fn lb(idx: u16, epoch: u32) -> LeaseBinding {
        LeaseBinding { lease_id: LeaseId::new(idx, 1), epoch_at_issue: LeaseEpoch(epoch) }
    }

    fn msg(tid: u16, badge: u64, is_call: bool, lease: Option<LeaseBinding>) -> PendingMessage {
        PendingMessage {
            tag: MessageTag::new(1, 0, 0),
            sender_tid: tid, sender_image_id: 0, sender_badge: badge,
            words: [0; IPC_WORDS],
            cap_present: false, cap_kind: 0, cap_obj_id: 0, cap_rights: 0,
            is_call, lease,
        }
    }

    // ── Existing IPC behaviour preserved ─────────────────────────────────────

    #[test]
    fn send_to_waiting_receiver() {
        let mut ep = Endpoint::new();
        assert!(matches!(ep.recv(RecvWaiter::no_lease(10)).unwrap(), RecvResult::Queued));
        let s = ep.send(msg(5, 42, false, None)).unwrap();
        assert!(matches!(s, SendResult::Delivered { receiver_tid: 10 }));
    }

    #[test]
    fn recv_from_waiting_sender() {
        let mut ep = Endpoint::new();
        assert!(matches!(ep.send(msg(5, 99, true, None)).unwrap(), SendResult::Queued));
        let r = ep.recv(RecvWaiter::no_lease(10)).unwrap();
        if let RecvResult::Delivered(m) = r {
            assert_eq!(m.sender_tid, 5);
            assert_eq!(m.sender_badge, 99);
            assert!(m.is_call);
        } else { panic!("expected Delivered"); }
    }

    #[test]
    fn ipc_a_no_duplicate_tid() {
        let mut ep = Endpoint::new();
        ep.recv(RecvWaiter::no_lease(7)).unwrap();
        assert_eq!(
            ep.send(msg(7, 0, false, None)).unwrap_err(),
            EndpointError::AlreadyQueued
        );
    }

    #[test]
    fn cancel_removes_sender() {
        let mut ep = Endpoint::new();
        ep.send(msg(3, 0, false, None)).unwrap();
        ep.cancel(3);
        assert_eq!(ep.send_queue_len(), 0);
    }

    #[test]
    fn queue_full_error() {
        let mut ep = Endpoint::new();
        for i in 0..32u16 { ep.send(msg(i, 0, false, None)).unwrap(); }
        assert_eq!(ep.send(msg(99, 0, false, None)).unwrap_err(), EndpointError::SendQueueFull);
    }

    // ── RFC 034: cancel_by_lease ───────────────────────────────────────────────

    #[test]
    fn cancel_by_lease_removes_matching_sender() {
        let mut ep = Endpoint::new();
        let lease = lb(1, 5);
        ep.send(msg(10, 0, false, Some(lease))).unwrap();     // matches
        ep.send(msg(11, 0, false, None)).unwrap();              // no lease — keep
        ep.send(msg(12, 0, false, Some(lb(2, 5)))).unwrap();  // different id — keep

        let c = ep.cancel_by_lease(lease.lease_id, 5);

        assert_eq!(c.n_senders, 1);
        assert_eq!(c.sender_tids[0], 10);
        assert_eq!(ep.send_queue_len(), 2);
    }

    #[test]
    fn cancel_by_lease_removes_matching_receiver() {
        let mut ep = Endpoint::new();
        let lease = lb(3, 7);
        ep.recv(RecvWaiter::with_lease(20, lease)).unwrap();         // matches
        ep.recv(RecvWaiter::no_lease(21)).unwrap();                   // no lease — keep
        ep.recv(RecvWaiter::with_lease(22, lb(4, 7))).unwrap();     // different id — keep

        let c = ep.cancel_by_lease(lease.lease_id, 7);

        assert_eq!(c.n_receivers, 1);
        assert_eq!(c.receiver_tids[0], 20);
        assert_eq!(ep.recv_queue_len(), 2);
    }

    #[test]
    fn cancel_by_lease_old_epoch_not_matched() {
        let mut ep = Endpoint::new();
        let lease = lb(1, 5);
        ep.send(msg(10, 0, false, Some(lease))).unwrap();
        // Revoke epoch 6 — sender has epoch 5; should NOT be cancelled.
        let c = ep.cancel_by_lease(lease.lease_id, 6);
        assert_eq!(c.n_senders, 0);
        assert_eq!(ep.send_queue_len(), 1);
    }

    #[test]
    fn cancel_by_lease_empty_endpoint_no_panic() {
        let mut ep = Endpoint::new();
        let c = ep.cancel_by_lease(LeaseId::new(0, 1), 1);
        assert_eq!(c.n_senders, 0);
        assert_eq!(c.n_receivers, 0);
    }

    #[test]
    fn late_reply_lease_field_preserved_in_message() {
        // Verify that lease binding survives the send → recv path.
        let mut ep = Endpoint::new();
        let lease = lb(9, 3);
        ep.send(msg(5, 0, true, Some(lease))).unwrap();
        if let RecvResult::Delivered(m) = ep.recv(RecvWaiter::no_lease(10)).unwrap() {
            assert!(m.lease.is_some());
            assert_eq!(m.lease.unwrap().lease_id, lease.lease_id);
        } else { panic!("expected Delivered"); }
    }
}
