//! Provider registry with bootstrap-to-enforcing one-way handoff.
//!
//! The registry mirrors the v0.2 `cap-broker` pattern: it starts in
//! `Bootstrap`, accepts provider registrations and any initial wiring during
//! that phase, then transitions to `Enforcing` exactly once.  Once enforcing,
//! `Null` providers are rejected and no new providers may be added without
//! explicit policy (which lives in `verifyd`/`upgraded`, not here).
//!
//! The registry is a generic-free, fixed-capacity container intended for use
//! in both `no_std` services and host tools.  Provider trait objects are
//! avoided so the crate stays free of `alloc`.

use crate::descriptor::TrustProviderDescriptor;
use crate::ids::{ProviderHandle, TrustProviderId};
use crate::profile::TrustProviderKind;

/// Maximum number of providers a single Fjell node may register.
///
/// 8 is enough for a development board + TPM + DICE + headroom; raising this
/// is a configuration change and does not require an RFC.
pub const MAX_PROVIDERS: usize = 8;

/// Phase of the registry — strictly one-way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegistryPhase {
    Bootstrap,
    Enforcing,
}

/// Errors returned by the registry API.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegistryError {
    /// The internal capacity (`MAX_PROVIDERS`) has been reached.
    CapacityExhausted,
    /// The registry is already in `Enforcing` phase and refuses the operation.
    PhaseLocked,
    /// Attempted to register a `Null` provider in `Enforcing` phase.
    NullProviderForbidden,
    /// The handle's generation is stale (slot was replaced).
    StaleHandle,
    /// The handle refers to a slot that does not contain a provider.
    NotFound,
    /// `Enforcing` mode requires a policy-authorization token for replace/remove
    /// (RFC-v0.7.4-003 / W-H-06).
    PolicyAuthorizationRequired,
}

/// Opaque policy-authorization token required for replace/remove in Enforcing mode.
///
/// In v0.7.x the token is checked for non-emptiness only; full signature
/// verification against the PolicyAdmin keyring anchor is v0.8.
/// See RFC-v0.7.4-003 §6 "Provider registry constraints".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PolicyAuth {
    /// Non-zero = token present.  Full Ed25519 sig verification in v0.8.
    pub token: u32,
}

impl PolicyAuth {
    /// An empty (invalid) token — rejected in Enforcing mode.
    pub const EMPTY: Self = Self { token: 0 };
    /// A non-empty token — accepted in v0.7.x as "policy authorised".
    pub fn valid(token: u32) -> Self { Self { token } }
    pub fn is_present(self) -> bool { self.token != 0 }
}

/// Owning storage for one provider slot.
///
/// We store the descriptor inline (`Copy`) and track the per-slot generation
/// so a stale `ProviderHandle` always fails the use-time check.
#[derive(Clone, Copy)]
struct Slot {
    descriptor: TrustProviderDescriptor,
    generation: u16,
    occupied: bool,
}

impl Slot {
    const fn empty() -> Self {
        Self {
            descriptor: TrustProviderDescriptor {
                id: TrustProviderId::UNSET,
                kind: TrustProviderKind::Null,
                profile: crate::profile::TrustProfile::FjellLocalV1,
                capabilities: crate::profile::TrustProviderCapabilities::NONE,
                state: crate::profile::TrustProviderState::Withdrawn,
                generation: 0,
                name: [0u8; 8],
            },
            generation: 0,
            occupied: false,
        }
    }
}

/// Fixed-capacity registry of `TrustProviderDescriptor`s.
///
/// The registry stores descriptors only, not trait objects: the actual
/// provider implementation lives in the owning service (`verifyd`,
/// `attestd`), and the descriptor is the public, semantic-visible projection.
pub struct ProviderRegistry {
    slots: [Slot; MAX_PROVIDERS],
    phase: RegistryPhase,
    next_id: u32,
}

impl ProviderRegistry {
    pub const fn new() -> Self {
        Self {
            slots: [const { Slot::empty() }; MAX_PROVIDERS],
            phase: RegistryPhase::Bootstrap,
            next_id: 1,
        }
    }

    pub fn phase(&self) -> RegistryPhase {
        self.phase
    }

    /// Transition the registry to `Enforcing`.  This is one-way; subsequent
    /// calls are no-ops.
    pub fn enter_enforcing(&mut self) {
        self.phase = RegistryPhase::Enforcing;
    }

    /// Register a provider descriptor, returning a handle.
    ///
    /// In `Enforcing` phase:
    ///   - `Null` providers are rejected with `NullProviderForbidden`;
    ///   - any new registration is rejected with `PhaseLocked` unless the
    ///     caller explicitly intends to *replace* an existing slot — replace
    ///     is a separate API.
    pub fn register(
        &mut self,
        mut descriptor: TrustProviderDescriptor,
    ) -> Result<ProviderHandle, RegistryError> {
        if self.phase == RegistryPhase::Enforcing {
            if !descriptor.permitted_in_release() {
                return Err(RegistryError::NullProviderForbidden);
            }
            return Err(RegistryError::PhaseLocked);
        }
        // Bootstrap path: pick the first free slot.
        for slot in self.slots.iter_mut() {
            if !slot.occupied {
                let id = TrustProviderId::new(self.next_id);
                self.next_id = self.next_id.wrapping_add(1).max(1);
                descriptor.id = id;
                descriptor.generation = descriptor.generation.max(1);
                slot.descriptor = descriptor;
                slot.generation = descriptor.generation;
                slot.occupied = true;
                return Ok(ProviderHandle::new(id, slot.generation));
            }
        }
        Err(RegistryError::CapacityExhausted)
    }

    /// Look up a descriptor by handle, validating the generation.
    pub fn lookup(&self, handle: ProviderHandle) -> Result<TrustProviderDescriptor, RegistryError> {
        if handle.is_unset() {
            return Err(RegistryError::NotFound);
        }
        let slot = self.find(handle.id).ok_or(RegistryError::NotFound)?;
        if !slot.occupied {
            return Err(RegistryError::NotFound);
        }
        if slot.generation != handle.generation {
            return Err(RegistryError::StaleHandle);
        }
        Ok(slot.descriptor)
    }

    /// Replace the descriptor in an existing slot.  Increments the slot's
    /// generation so old handles fail.
    ///
    /// In `Enforcing` phase: requires a non-empty `PolicyAuth` token
    /// (RFC-v0.7.4-003 / W-H-06) and rejects `Null` descriptors.
    pub fn replace(
        &mut self,
        handle: ProviderHandle,
        mut new_descriptor: TrustProviderDescriptor,
        auth: PolicyAuth,
    ) -> Result<ProviderHandle, RegistryError> {
        if self.phase == RegistryPhase::Enforcing {
            if !auth.is_present() {
                return Err(RegistryError::PolicyAuthorizationRequired);
            }
            if !new_descriptor.permitted_in_release() {
                return Err(RegistryError::NullProviderForbidden);
            }
        }
        let id = handle.id;
        for slot in self.slots.iter_mut() {
            if slot.occupied && slot.descriptor.id == id {
                if slot.generation != handle.generation {
                    return Err(RegistryError::StaleHandle);
                }
                let next_gen = slot.generation.wrapping_add(1).max(1);
                new_descriptor.id = id;
                new_descriptor.generation = next_gen;
                slot.descriptor = new_descriptor;
                slot.generation = next_gen;
                return Ok(ProviderHandle::new(id, next_gen));
            }
        }
        Err(RegistryError::NotFound)
    }

    /// Remove a provider slot.  After removal, any stale handle fails the
    /// generation check.
    ///
    /// In `Enforcing` phase: requires a non-empty `PolicyAuth` token
    /// (RFC-v0.7.4-003 / W-H-06).
    pub fn remove(&mut self, handle: ProviderHandle, auth: PolicyAuth) -> Result<(), RegistryError> {
        if self.phase == RegistryPhase::Enforcing && !auth.is_present() {
            return Err(RegistryError::PolicyAuthorizationRequired);
        }
        for slot in self.slots.iter_mut() {
            if slot.occupied && slot.descriptor.id == handle.id {
                if slot.generation != handle.generation {
                    return Err(RegistryError::StaleHandle);
                }
                *slot = Slot::empty();
                return Ok(());
            }
        }
        Err(RegistryError::NotFound)
    }

    /// Iterate live descriptors.  Used by audit/semantic-stream projections.
    pub fn descriptors(&self) -> impl Iterator<Item = TrustProviderDescriptor> + '_ {
        self.slots
            .iter()
            .filter(|s| s.occupied)
            .map(|s| s.descriptor)
    }

    /// Count of currently-registered providers.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.occupied).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn find(&self, id: TrustProviderId) -> Option<&Slot> {
        self.slots
            .iter()
            .find(|s| s.occupied && s.descriptor.id == id)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
