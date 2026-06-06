//! In-process model of Fjell OS capability/IPC/lease state (RFC v0.6-001 §6.2).

use std::collections::BTreeMap;

// ── Identifiers ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct CapId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct TaskId(pub u16);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct LeaseId(pub u32);

// ── Capability kinds and rights ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CapKind {
    Endpoint     = 0x01,
    MmioRegion   = 0x04,
    DmaRegion    = 0x05,
    TaskControl  = 0x02,
    AuditDrain   = 0x03,
    NetDevice    = 0x0F,
    Interrupt    = 0x0E,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CapRights(pub u64);

impl CapRights {
    pub const SEND:     Self = Self(1 << 0);
    pub const RECV:     Self = Self(1 << 1);
    pub const CALL:     Self = Self(1 << 2);
    pub const REPLY:    Self = Self(1 << 3);
    pub const COPY:     Self = Self(1 << 4);
    pub const MINT:     Self = Self(1 << 5);
    pub const INSPECT:  Self = Self(1 << 6);
    pub const ALL:      Self = Self(0xFF);

    pub fn contains(self, rhs: Self) -> bool { (self.0 & rhs.0) == rhs.0 }
    pub fn intersect(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
    pub fn is_subset_of(self, parent: Self) -> bool { (self.0 & !parent.0) == 0 }
}

// ── Cap state ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapState { Active, Revoked, Replaced }

#[derive(Clone, Debug)]
pub struct ModelCap {
    pub kind:        CapKind,
    pub rights:      CapRights,
    pub origin_task: TaskId,
    pub lease:       LeaseId,
    pub generation:  u32,
    pub state:       CapState,
}

// ── Task state ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState { Running, Faulted }

#[derive(Clone, Debug)]
pub struct ModelTask {
    pub state:  TaskState,
    pub mailbox: Vec<(CapId, u16, u64)>,  // (cap_used, tag, payload) inbox
}

// ── Lease state ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeaseState { Active, Expired }

#[derive(Clone, Debug)]
pub struct ModelLease {
    pub origin_task: TaskId,
    pub state:       LeaseState,
}

// ── Model state ───────────────────────────────────────────────────────────────

pub const MAX_CAP_TABLE: usize = 64;

#[derive(Clone, Debug)]
pub struct ModelState {
    pub caps:    BTreeMap<CapId, ModelCap>,
    pub tasks:   BTreeMap<TaskId, ModelTask>,
    pub leases:  BTreeMap<LeaseId, ModelLease>,
    pub now:     u64,
    /// Generation counter per cap_id (monotonically increasing).
    pub r#gen:     BTreeMap<CapId, u32>,
}

impl ModelState {
    pub fn new() -> Self {
        let mut s = Self {
            caps:   BTreeMap::new(),
            tasks:  BTreeMap::new(),
            leases: BTreeMap::new(),
            now:    0,
            r#gen:   BTreeMap::new(),
        };
        // Seed two tasks and a default lease.
        s.tasks.insert(TaskId(0), ModelTask { state: TaskState::Running, mailbox: vec![] });
        s.tasks.insert(TaskId(1), ModelTask { state: TaskState::Running, mailbox: vec![] });
        s.leases.insert(LeaseId(0), ModelLease { origin_task: TaskId(0), state: LeaseState::Active });
        s
    }

    pub fn live_cap_count(&self) -> usize {
        self.caps.values().filter(|c| c.state == CapState::Active).count()
    }

    pub fn generation_of(&self, id: CapId) -> u32 {
        self.r#gen.get(&id).copied().unwrap_or(0)
    }

    pub fn next_generation(&mut self, id: CapId) -> u32 {
        let g = self.r#gen.entry(id).or_insert(0);
        *g += 1;
        *g
    }

    /// Look up a cap by id — only returns `Some` for Active caps.
    pub fn active_cap(&self, id: CapId) -> Option<&ModelCap> {
        self.caps.get(&id).filter(|c| c.state == CapState::Active)
    }
}
