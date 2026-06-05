//! `NullTrustProvider` — test-only provider that always fails.
//!
//! Required by the v0.3.0 acceptance criteria so that the negative test
//! `NEG:TRUST:NULL_PROVIDER_FORBIDDEN_IN_RELEASE` has a real object to point
//! at.  Must never be registered in a release-mode `ProviderRegistry`.

use crate::descriptor::TrustProviderDescriptor;
use crate::error::TrustError;
use crate::ids::TrustProviderId;
use crate::profile::{
    TrustProfile, TrustProviderCapabilities, TrustProviderKind, TrustProviderState,
};

/// A provider that returns `NotSupported` for every operation.  Used in
/// negative tests to assert release-profile rejection.
pub struct NullTrustProvider {
    descriptor: TrustProviderDescriptor,
}

impl NullTrustProvider {
    pub fn new(id: TrustProviderId, generation: u16) -> Self {
        Self {
            descriptor: TrustProviderDescriptor::new(
                id,
                TrustProviderKind::Null,
                TrustProfile::FjellLocalV1,
                TrustProviderCapabilities::NONE,
                TrustProviderState::Active,
                generation,
                *b"null---\0",
            ),
        }
    }
}

impl crate::provider::HardwareTrustProvider for NullTrustProvider {
    fn provider_id(&self) -> TrustProviderId {
        self.descriptor.id
    }

    fn descriptor(&self) -> TrustProviderDescriptor {
        self.descriptor
    }
    // All other operations inherit the default `NotSupported` implementations,
    // which is the contract for a Null provider.
    fn read_anti_rollback_counter(&self) -> Result<u64, TrustError> {
        Err(TrustError::NotSupported)
    }
}
