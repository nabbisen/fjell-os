//! Whether a `LeaseAdmin` holder may revoke a given lease (RFC-0.46-003), as a pure function.
//!
//! The kernel gathers one fact -- does the caller own the lease -- and hands it here with the capability's
//! rights, so the rule is testable on the host and each clause can be shown red by mutation.
//!
//! * the **owner** may revoke (the capability check -- kind, `LEASE_REVOKE`, scope -- happened first);
//! * a **non-owner** may revoke only with `LEASE_REVOKE_ANY` on the capability it presents;
//! * anyone else is `PermissionDenied`.

use crate::rights::CapRights;
use fjell_abi::error::SysError;

/// `Ok(())` if the revocation may proceed.
pub fn revoke_permitted(caller_owns_lease: bool, cap_rights: CapRights) -> Result<(), SysError> {
    if caller_owns_lease || cap_rights.contains(CapRights::LEASE_REVOKE_ANY) {
        Ok(())
    } else {
        Err(SysError::PermissionDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADMIN: CapRights = CapRights(CapRights::LEASE_REVOKE.0 | CapRights::LEASE_CREATE.0);

    #[test]
    fn the_owner_may_revoke_its_own_lease() {
        assert_eq!(revoke_permitted(true, ADMIN), Ok(()));
    }

    #[test]
    fn a_non_owner_without_the_named_right_is_refused() {
        assert_eq!(
            revoke_permitted(false, ADMIN),
            Err(SysError::PermissionDenied)
        );
        // Even holding every aggregate: the right is in neither, so a blanket grant does not carry it.
        assert_eq!(
            revoke_permitted(false, CapRights::ALL_DEFINED),
            Err(SysError::PermissionDenied)
        );
    }

    #[test]
    fn a_non_owner_with_the_named_right_may_revoke() {
        let any = CapRights(ADMIN.0 | CapRights::LEASE_REVOKE_ANY.0);
        assert_eq!(revoke_permitted(false, any), Ok(()));
    }

    #[test]
    fn the_named_right_alone_is_not_the_capability_check() {
        // This function is only the ownership clause; it does not stand in for LEASE_REVOKE.
        assert_eq!(revoke_permitted(false, CapRights::LEASE_REVOKE_ANY), Ok(()));
        assert_eq!(revoke_permitted(true, CapRights::NONE), Ok(()));
    }
}
