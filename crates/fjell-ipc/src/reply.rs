//! One-shot reply edge (RFC 034: extended with lease binding).
//!
//! Invariant IPC-C: at most one `ReplyEdge` exists per task at any time.
//!
//! # RFC 034 addition
//!
//! `ReplyEdge` now carries the lease binding observed at `ipc_call` time.
//! When `ipc_reply` is delivered, `sys_ipc_reply` checks whether the binding
//! is still valid before delivering the reply to the caller.  If the lease
//! was revoked while the caller was blocked, the reply is silently discarded
//! (the caller has already been woken with `LeaseRevoked` by
//! `wake_or_cancel_blocked_ipc_for_lease`).

use fjell_cap::slot::LeaseBinding;

pub type Tid = u16;

/// A pending one-shot reply edge installed by `ipc_call`.
/// Consumed exactly once by `ipc_reply`.
///
/// Fields added in v0.2.0 (RFC 034):
/// - `lease` — the endpoint cap's lease binding at call time.
#[derive(Clone, Copy, Debug)]
pub struct ReplyEdge {
    /// Index of the task waiting for the reply (the caller).
    pub caller_tid: Tid,
    /// Lease binding observed at `ipc_call` time (RFC 034).
    ///
    /// `None` for bootstrap / unbound calls.
    pub lease: Option<LeaseBinding>,
}

impl ReplyEdge {
    /// Create a reply edge without a lease binding (backward compat / bootstrap).
    pub fn new(caller_tid: Tid) -> Self {
        ReplyEdge { caller_tid, lease: None }
    }

    /// Create a reply edge with a lease binding (RFC 034).
    pub fn with_lease(caller_tid: Tid, lease: LeaseBinding) -> Self {
        ReplyEdge { caller_tid, lease: Some(lease) }
    }
}
