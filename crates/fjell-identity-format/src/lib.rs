//! Node identity and snapshot-exchange trust model (RFC v0.7-001).
//!
//! Each Fjell OS node has a `NodeIdentity` that is measured, signed, and
//! stored in the append-only log. Other nodes verify the identity before
//! accepting a snapshot export.
#![no_std]

pub mod identity;
pub mod policy;
pub mod digest;

pub use identity::{
    NodeIdentity, NodeId, NodeAlias, AttestationPubkey,
    NODE_IDENTITY_SCHEMA_VERSION, STORE_RECORD_KIND_IDENTITY,
};
pub use policy::{NodeIdentityPolicy, TrustMode, RosterRef};
pub use digest::identity_digest;

#[cfg(test)]
mod tests;
