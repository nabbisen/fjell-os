//! Kernel lease table — M4 lease-based capability delegation.

use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::task::TaskId;
use fjell_abi::error::SysError;

pub const MAX_LEASES: usize = 32;

#[derive(Clone, Copy, PartialEq)]
enum LeaseState { Empty, Active, Revoked }

struct LeaseObject {
    state:      LeaseState,
    generation: u16,
    epoch:      u32,
}

impl LeaseObject {
    const fn empty() -> Self {
        LeaseObject { state: LeaseState::Empty, generation: 0, epoch: 0 }
    }
}

pub struct LeaseTable {
    slots: [LeaseObject; MAX_LEASES],
}

impl LeaseTable {
    pub const fn new() -> Self {
        LeaseTable { slots: [const { LeaseObject::empty() }; MAX_LEASES] }
    }
    pub fn create(&mut self, _owner: TaskId, _flags: u32) -> Result<LeaseId, SysError> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.state == LeaseState::Empty {
                slot.state = LeaseState::Active;
                slot.epoch = 0;
                return Ok(LeaseId::new(i as u16, slot.generation));
            }
        }
        Err(SysError::NoMemory)
    }
    pub fn current_epoch(&self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        Ok(LeaseEpoch(self.get(id)?.epoch))
    }
    pub fn revoke(&mut self, id: LeaseId) -> Result<LeaseEpoch, SysError> {
        let idx = id.index() as usize;
        let slot = self.slots.get_mut(idx).ok_or(SysError::InvalidCap)?;
        if slot.generation != id.generation() { return Err(SysError::InvalidCap); }
        if slot.state == LeaseState::Empty { return Err(SysError::InvalidCap); }
        slot.epoch = slot.epoch.wrapping_add(1);
        slot.state = LeaseState::Revoked;
        slot.generation = slot.generation.wrapping_add(1);
        Ok(LeaseEpoch(slot.epoch))
    }
    pub fn check_active(&self, id: LeaseId, bound_epoch: LeaseEpoch) -> Result<(), SysError> {
        let slot = self.get(id)?;
        if slot.state != LeaseState::Active { return Err(SysError::PermissionDenied); }
        if slot.epoch != bound_epoch.0 { return Err(SysError::PermissionDenied); }
        Ok(())
    }
    fn get(&self, id: LeaseId) -> Result<&LeaseObject, SysError> {
        let slot = self.slots.get(id.index() as usize).ok_or(SysError::InvalidCap)?;
        if slot.generation != id.generation() || slot.state == LeaseState::Empty {
            return Err(SysError::InvalidCap);
        }
        Ok(slot)
    }
}
