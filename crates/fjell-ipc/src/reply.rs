//! One-shot reply edge.
//!
//! Invariant IPC-C: at most one `ReplyEdge` exists per task at any time.

pub type Tid = u16;

/// A pending one-shot reply edge installed by `ipc_call`.
/// Consumed exactly once by `ipc_reply`.
#[derive(Clone, Copy, Debug)]
pub struct ReplyEdge {
    /// Index of the task waiting for the reply (the caller).
    pub caller_tid: Tid,
}

impl ReplyEdge {
    pub fn new(caller_tid: Tid) -> Self {
        ReplyEdge { caller_tid }
    }
}
