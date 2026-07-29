//! Node identity and snapshot-exchange trust model (RFC v0.7-001).
//!
//! Each Fjell OS node has a `NodeIdentity` that is measured, signed, and
//! stored in the append-only log. Other nodes verify the identity before
//! accepting a snapshot export.
#![no_std]

pub mod digest;
pub mod identity;
pub mod policy;

pub use digest::identity_digest;
pub use identity::{
    AttestationPubkey, NODE_IDENTITY_SCHEMA_VERSION, NodeAlias, NodeId, NodeIdentity,
    STORE_RECORD_KIND_IDENTITY,
};
pub use policy::{NodeIdentityPolicy, RosterRef, TrustMode};

#[cfg(test)]
mod tests;
