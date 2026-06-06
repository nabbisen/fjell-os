//! Global fixed-capacity endpoint table and per-task CSpace storage.
//!
//! All state is static to avoid heap allocation.  Single-hart M3.

use fjell_cap::cspace::{CSpace, CSPACE_SLOTS};
use fjell_ipc::endpoint::Endpoint;
use crate::task::tcb::MAX_TASKS;

/// Maximum number of endpoints in the system.
pub const MAX_ENDPOINTS: usize = 32;

/// Global endpoint table.
pub struct EndpointTable {
    endpoints: [Option<Endpoint>; MAX_ENDPOINTS],
    /// Generation counters for endpoint object IDs.
    generations: [u16; MAX_ENDPOINTS],
}

impl EndpointTable {
    pub const fn new() -> Self {
        const NONE_EP: Option<Endpoint> = None;
        EndpointTable {
            endpoints:   [NONE_EP; MAX_ENDPOINTS],
            generations: [0; MAX_ENDPOINTS],
        }
    }

    /// Allocate a new endpoint, returning its object ID.
    pub fn alloc(&mut self) -> Option<u32> {
        for (i, slot) in self.endpoints.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(Endpoint::new());
                return Some(i as u32);
            }
        }
        None
    }

    /// Get a mutable reference to an endpoint by object ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Endpoint> {
        self.endpoints.get_mut(id as usize)?.as_mut()
    }

    /// Free an endpoint by object ID, bumping its generation.
    pub fn free(&mut self, id: u32) {
        if let Some(slot) = self.endpoints.get_mut(id as usize) {
            *slot = None;
            self.generations[id as usize] = self.generations[id as usize].wrapping_add(1);
        }
    }
}

/// Per-task CSpace storage (one CSpace per task slot).
pub struct CSpaceTable {
    spaces: [CSpace; MAX_TASKS],
}

impl CSpaceTable {
    pub fn new() -> Self {
        CSpaceTable {
            spaces: core::array::from_fn(|_| CSpace::new()),
        }
    }

    pub fn get_mut(&mut self, task_index: usize) -> Option<&mut CSpace> {
        self.spaces.get_mut(task_index)
    }
}
