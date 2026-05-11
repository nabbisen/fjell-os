//! Synchronous rendezvous IPC endpoint.
//!
//! Invariants:
//!   IPC-A: a task cannot be in both sendq and recvq simultaneously.
//!   IPC-B: payload and cap-transfer commit atomically.
//!   IPC-D: sender_badge matches the endpoint capability badge at send time.

use super::message::{MessageTag, IPC_WORDS, IPC_CAPS};
use fjell_abi::error::SysError;

/// Re-export for kernel use.
pub use super::message::{IPC_WORDS, IPC_CAPS};

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
        (0..self.len).any(|i| self.items[(self.head + i) % N].map_or(false, |x| f(x)))
    }
    fn retain<F: Fn(T) -> bool>(&mut self, keep: F) {
        let mut new = FixedQueue::new();
        while let Some(v) = self.pop() { if keep(v) { new.push(v); } }
        *self = new;
    }
}

// ── Message snapshot ──────────────────────────────────────────────────────────

/// A message payload snapshotted at send time.
#[derive(Clone, Copy, Debug)]
pub struct PendingMessage {
    pub tag:          MessageTag,
    pub sender_tid:   u16,
    pub sender_badge: u64,
    pub words:        [u64; IPC_WORDS],
    /// Optional cap transfer (snapshotted at send time for atomic commit).
    pub cap_present:  bool,
    pub cap_kind:     u8,
    pub cap_obj_id:   u32,
    pub cap_rights:   u32,
    /// True if sent via `ipc_call` (expects a reply).
    pub is_call:      bool,
}

// ── Error / result types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointError {
    SendQueueFull,
    RecvQueueFull,
    AlreadyQueued,
    InvalidTag,
}

impl From<EndpointError> for SysError {
    fn from(e: EndpointError) -> Self {
        match e {
            EndpointError::SendQueueFull |
            EndpointError::RecvQueueFull  => SysError::QueueFull,
            EndpointError::AlreadyQueued  => SysError::BadState,
            EndpointError::InvalidTag     => SysError::InvalidArg,
        }
    }
}

pub enum SendResult {
    /// Message delivered directly to a waiting receiver.
    Delivered { receiver_tid: u16 },
    /// No receiver; sender enqueued — must block.
    Queued,
}

pub enum RecvResult {
    /// A queued sender's message was delivered.
    Delivered(PendingMessage),
    /// No sender; receiver enqueued — must block.
    Queued,
}

// ── Endpoint ──────────────────────────────────────────────────────────────────

pub struct Endpoint {
    sendq: FixedQueue<PendingMessage, QUEUE_DEPTH>,
    recvq: FixedQueue<u16,            QUEUE_DEPTH>,
}

impl Endpoint {
    pub const fn new() -> Self {
        Endpoint { sendq: FixedQueue::new(), recvq: FixedQueue::new() }
    }

    pub fn send(&mut self, msg: PendingMessage) -> Result<SendResult, EndpointError> {
        if !msg.tag.is_valid()                                 { return Err(EndpointError::InvalidTag); }
        if self.recvq.contains(|tid| tid == msg.sender_tid)   { return Err(EndpointError::AlreadyQueued); }

        if let Some(receiver_tid) = self.recvq.pop() {
            return Ok(SendResult::Delivered { receiver_tid });
        }
        if !self.sendq.push(msg) { return Err(EndpointError::SendQueueFull); }
        Ok(SendResult::Queued)
    }

    pub fn recv(&mut self, receiver_tid: u16) -> Result<RecvResult, EndpointError> {
        if self.sendq.contains(|m: PendingMessage| m.sender_tid == receiver_tid) {
            return Err(EndpointError::AlreadyQueued);
        }
        if let Some(msg) = self.sendq.pop() {
            return Ok(RecvResult::Delivered(msg));
        }
        if !self.recvq.push(receiver_tid) { return Err(EndpointError::RecvQueueFull); }
        Ok(RecvResult::Queued)
    }

    /// Cancel all pending entries for `tid`.
    pub fn cancel(&mut self, tid: u16) {
        self.sendq.retain(|m: PendingMessage| m.sender_tid != tid);
        self.recvq.retain(|t: u16| t != tid);
    }

    pub fn send_queue_len(&self) -> usize { self.sendq.len }
    pub fn recv_queue_len(&self) -> usize { self.recvq.len }
}

// ── Host-side unit tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(tid: u16, badge: u64, is_call: bool) -> PendingMessage {
        PendingMessage {
            tag: MessageTag::new(1, 0, 0),
            sender_tid: tid, sender_badge: badge,
            words: [0; IPC_WORDS],
            cap_present: false, cap_kind: 0, cap_obj_id: 0, cap_rights: 0,
            is_call,
        }
    }

    #[test]
    fn send_to_waiting_receiver() {
        let mut ep = Endpoint::new();
        assert!(matches!(ep.recv(10).unwrap(), RecvResult::Queued));
        let s = ep.send(msg(5, 42, false)).unwrap();
        assert!(matches!(s, SendResult::Delivered { receiver_tid: 10 }));
    }

    #[test]
    fn recv_from_waiting_sender() {
        let mut ep = Endpoint::new();
        assert!(matches!(ep.send(msg(5, 99, true)).unwrap(), SendResult::Queued));
        let r = ep.recv(10).unwrap();
        if let RecvResult::Delivered(m) = r {
            assert_eq!(m.sender_tid, 5);
            assert_eq!(m.sender_badge, 99);
            assert!(m.is_call);
        } else { panic!("expected Delivered"); }
    }

    #[test]
    fn ipc_a_no_duplicate_tid() {
        let mut ep = Endpoint::new();
        ep.recv(7).unwrap();
        assert_eq!(ep.send(msg(7, 0, false)).unwrap_err(), EndpointError::AlreadyQueued);
    }

    #[test]
    fn cancel_removes_sender() {
        let mut ep = Endpoint::new();
        ep.send(msg(3, 0, false)).unwrap();
        ep.cancel(3);
        assert_eq!(ep.send_queue_len(), 0);
    }

    #[test]
    fn queue_full_error() {
        let mut ep = Endpoint::new();
        for i in 0..32u16 { ep.send(msg(i, 0, false)).unwrap(); }
        assert_eq!(ep.send(msg(99, 0, false)).unwrap_err(), EndpointError::SendQueueFull);
    }
}
