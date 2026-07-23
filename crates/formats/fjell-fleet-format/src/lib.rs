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

pub mod roster;
pub mod policy;
pub mod rollout;
pub mod action;
pub mod digest;

pub use roster::{
    NodeRoster, RosterEntry, RosterRef,
    FLEET_SCHEMA_VERSION, MAX_ROSTER_ENTRIES, STORE_RECORD_KIND_ROSTER,
};
pub use policy::{
    FleetPolicy, PolicyStatement, PolicyAction, PolicyCondition,
    FLEET_POLICY_SCHEMA_VERSION, MAX_POLICY_STATEMENTS,
};
pub use rollout::{
    FleetRolloutPlan, RolloutStage, RolloutStrategy,
    ROLLOUT_SCHEMA_VERSION, MAX_ROLLOUT_STAGES,
};
pub use action::{
    FleetAction, FleetActionKind, FleetActionResult, FleetActionError,
};
pub use digest::{roster_digest, policy_digest};

#[cfg(test)]
mod tests;
