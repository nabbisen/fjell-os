//! Provider descriptor: the metadata block returned by the registry.
//!
//! Consumers (semantic-stream, audit) project the descriptor into their
//! formats; the descriptor itself never carries policy.

use crate::ids::TrustProviderId;
use crate::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};

/// Public, semantic-visible summary of one registered trust provider.
///
/// The descriptor never carries keys, signatures, or measurement bytes — those
/// flow through the `HardwareTrustProvider` trait methods.  This separation
/// lets the semantic stream and audit subsystems publish provider state
/// without crossing a privacy boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrustProviderDescriptor {
    pub id: TrustProviderId,
    pub kind: TrustProviderKind,
    pub profile: TrustProfile,
    pub capabilities: TrustProviderCapabilities,
    pub state: TrustProviderState,
    /// Monotonic generation incremented when this slot is replaced.
    pub generation: u16,
    /// 8-byte ASCII name for human-readable rendering (truncate/pad with zero).
    pub name: [u8; 8],
}

impl TrustProviderDescriptor {
    /// Builder for a descriptor.
    pub const fn new(
        id: TrustProviderId,
        kind: TrustProviderKind,
        profile: TrustProfile,
        capabilities: TrustProviderCapabilities,
        state: TrustProviderState,
        generation: u16,
        name: [u8; 8],
    ) -> Self {
        Self {
            id,
            kind,
            profile,
            capabilities,
            state,
            generation,
            name,
        }
    }

    /// True if the descriptor describes a provider that can currently service
    /// requests.
    pub const fn is_usable(&self) -> bool {
        self.state.is_usable()
    }

    /// True if this descriptor is acceptable in a release-profile registry.
    pub const fn permitted_in_release(&self) -> bool {
        self.kind.permitted_in_release()
    }
}
