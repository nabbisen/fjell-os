//! Kernel-side capability and endpoint tables.
//!
//! `CapTable` holds one `CSpace` per task slot.
//! `EndpointTable` holds a fixed pool of `Endpoint` objects.
//!
//! All tables are fixed-capacity with no heap allocation.

#![allow(dead_code)]

use fjell_cap::cspace::CSpace;
use fjell_ipc::endpoint::Endpoint;
use fjell_ipc::reply::ReplyEdge;
use fjell_abi::error::SysError;
use crate::task::tcb::MAX_TASKS;

// ── Endpoint table ────────────────────────────────────────────────────────────

/// Maximum number of IPC endpoints.
pub const MAX_ENDPOINTS: usize = 32;

/// Global endpoint pool.
pub struct EndpointTable {
    eps:  [Endpoint; MAX_ENDPOINTS],
    used: [bool;     MAX_ENDPOINTS],
}

impl EndpointTable {
    pub const fn new() -> Self {
        const EP: Endpoint = Endpoint::new();
        EndpointTable {
            eps:  [EP;    MAX_ENDPOINTS],
            used: [false; MAX_ENDPOINTS],
        }
    }

    /// Allocate a new endpoint, returning its `object_id`.
    pub fn alloc(&mut self) -> Option<u32> {
        let idx = self.used.iter().position(|&u| !u)?;
        self.used[idx] = true;
        Some(idx as u32)
    }

    /// Get a mutable reference to an endpoint.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Endpoint> {
        let idx = id as usize;
        if idx < MAX_ENDPOINTS && self.used[idx] { Some(&mut self.eps[idx]) }
        else { None }
    }

    /// Free an endpoint (called when the last capability to it is deleted).
    pub fn free(&mut self, id: u32) {
        let idx = id as usize;
        if idx < MAX_ENDPOINTS {
            self.used[idx] = false;
            self.eps[idx] = Endpoint::new();
        }
    }
}

// ── CSpace per task ───────────────────────────────────────────────────────────

/// One-shot reply edge stored per server task.
pub struct ReplySlot {
    pub edge: Option<ReplyEdge>,
}

/// Per-task capability space storage.
pub struct CapTable {
    cspaces:  [CSpace;    MAX_TASKS],
    replies:  [ReplySlot; MAX_TASKS],
}

impl CapTable {
    pub fn new() -> Self {
        // CSpace and ReplySlot are large — initialise directly.
        let cspaces = core::array::from_fn(|_| CSpace::new());
        let replies = core::array::from_fn(|_| ReplySlot { edge: None });
        CapTable { cspaces, replies }
    }

    pub fn cspace_mut(&mut self, task_idx: usize) -> Option<&mut CSpace> {
        self.cspaces.get_mut(task_idx)
    }

    pub fn cspace(&self, task_idx: usize) -> Option<&CSpace> {
        self.cspaces.get(task_idx)
    }

    /// Install a reply edge for task `server_idx` pointing back to `caller_idx`.
    ///
    /// Optionally carries the lease binding observed at call time (RFC 034).
    pub fn set_reply(
        &mut self,
        server_idx: usize,
        caller_idx: u16,
    ) {
        if let Some(r) = self.replies.get_mut(server_idx) {
            r.edge = Some(ReplyEdge::new(caller_idx));
        }
    }

    /// Install a reply edge with a lease binding (RFC 034).
    pub fn set_reply_with_lease(
        &mut self,
        server_idx: usize,
        caller_idx: u16,
        lease: fjell_cap::slot::LeaseBinding,
    ) {
        if let Some(r) = self.replies.get_mut(server_idx) {
            r.edge = Some(ReplyEdge::with_lease(caller_idx, lease));
        }
    }

    /// RFC 034: cancel all reply edges whose lease binding matches
    /// `(lease_id, old_epoch)` and return the caller TIDs.
    ///
    /// The caller must wake each returned TID with `SysError::LeaseRevoked`.
    pub fn cancel_replies_for_lease(
        &mut self,
        lease_id:  fjell_abi::lease::LeaseId,
        old_epoch: u32,
    ) -> ([u16; MAX_TASKS], usize) {
        let mut cancelled = [0u16; MAX_TASKS];
        let mut n = 0usize;
        for reply_slot in self.replies.iter_mut() {
            if let Some(edge) = &reply_slot.edge {
                let matches = edge.lease.map_or(false, |lb| {
                    lb.lease_id == lease_id && lb.epoch_at_issue.0 == old_epoch
                });
                if matches {
                    if n < MAX_TASKS {
                        cancelled[n] = edge.caller_tid;
                        n += 1;
                    }
                    reply_slot.edge = None;
                }
            }
        }
        (cancelled, n)
    }

    /// Consume the reply edge for task `server_idx`.
    ///
    /// Returns `Err(SysError::BadState)` if no reply edge exists.
    pub fn take_reply(&mut self, server_idx: usize) -> Result<ReplyEdge, SysError> {
        self.replies.get_mut(server_idx)
            .and_then(|r| r.edge.take())
            .ok_or(SysError::BadState)
    }
}

impl EndpointTable {
    /// Iterate over all allocated endpoints with their IDs.
    ///
    /// Used by the unified lease-revocation path to walk every endpoint
    /// and cancel waiters whose lease has been revoked
    /// (RFC-v0.7.4-003 / W-H-02).
    pub fn iter_allocated_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.used
            .iter()
            .enumerate()
            .filter_map(|(i, &u)| if u { Some(i as u32) } else { None })
    }
}
