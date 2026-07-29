//! Fleet identity, roster, policy, and rollout wire formats for Fjell OS v0.8.
//!
//! # Design goals
//!
//! - **No general remote shell.** Every fleet operation is expressed as a
//!   typed semantic intent.
//! - **Capability-controlled.** Remote actions require an explicit capability
//!   grant that can be revoked.
//! - **Auditable.** Every fleet operation that changes state must produce an
//!   audit-trail record.
//!
//! # Crate layout
//!
//! - `roster`  — `NodeRoster` (the signed set of fleet members).
//! - `policy`  — `FleetPolicy` (what operations are allowed and under what conditions).
//! - `rollout` — `FleetRolloutPlan` (staged update delivery across nodes).
//! - `action`  — `FleetAction` (typed capability-controlled remote operations).
#![no_std]

pub mod action;
pub mod digest;
pub mod policy;
pub mod rollout;
pub mod roster;

pub use action::{FleetAction, FleetActionError, FleetActionKind, FleetActionResult};
pub use digest::{policy_digest, roster_digest};
pub use policy::{
    FLEET_POLICY_SCHEMA_VERSION, FleetPolicy, MAX_POLICY_STATEMENTS, PolicyAction, PolicyCondition,
    PolicyStatement,
};
pub use rollout::{
    FleetRolloutPlan, MAX_ROLLOUT_STAGES, ROLLOUT_SCHEMA_VERSION, RolloutStage, RolloutStrategy,
};
pub use roster::{
    FLEET_SCHEMA_VERSION, MAX_ROSTER_ENTRIES, NodeRoster, RosterEntry, RosterRef,
    STORE_RECORD_KIND_ROSTER,
};

#[cfg(test)]
mod tests;
