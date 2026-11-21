//! The decision `sys_cap_install` makes, as a pure function (RFC-0.45-002 R3).
//!
//! The kernel gathers its facts and hands them here; **this function owns the order of
//! the refusals and each refusal's error**, so both are testable on the host and each
//! can be shown red by mutation (R8). The kernel arm does no deciding of its own beyond
//! the first check (the caller's `CapInstall` authority), which needs the CSpace and
//! the lease table and is `require_cap`.
//!
//! The checks, in order (the numbering is the RFC's):
//!
//! * **b** the caller has a pending reply edge whose caller is `Blocked` -- else `BadState`;
//! * **c** the **source capability** the installer names resolves (else its own error), is of a kind
//!   on [`INSTALLABLE_KINDS`] (`PermissionDenied`), and its **object** is not endpoint object `0`
//!   (`PermissionDenied`: `object-zero`'s invariant -- no task can receive on it -- is preserved by
//!   *refusal*, not by the policy table) and is **allocated** (`InvalidArg`);
//! * **d** `rights` is non-empty and inside **both** the source capability's rights and the
//!   installer's envelope -- else `PermissionDenied`;
//! * **e** `lease` is non-zero, owned by the caller, and `Active` -- else `PermissionDenied`
//!   / the lease's own error;
//! * **f** the target CSpace has a free slot -- else `NoMemory`.
//!
//! **Why the object is not an argument (RFC-0.46-002).** The installer names a capability **it holds**
//! (`src_cap`, a slot in its own CSpace); the kernel takes the object, the kind and the ceiling on the
//! rights from it. An integer naming an object is unrepresentable -- authority is derived and
//! attenuated, never minted from nothing. (Until 0.46 the broker passed `object_id` in a register and
//! the kernel built a capability to it: a compromised broker could confer `SEND|RECV|CALL|REPLY|COPY|MINT`
//! on any of the 32 endpoints, `configd`'s included.)
//!
//! **Why the target is not an argument.** The capability is installed into the caller of
//! the pending reply edge -- the task the broker is serving right now. A stale or forged
//! target is therefore unrepresentable, and a compromised broker can write only into a
//! task that is blocked in a call to it. See RFC-0.45-002 §"The risk that shapes the design".

use crate::rights::{CapKind, CapRights, ObjectScope};
use fjell_abi::error::SysError;

/// The kinds `sys_cap_install` will install. **`Endpoint` only** (RFC-0.45-002 ruling 2):
/// deny by default, grown one kind at a time **with a consumer**, never ahead of one.
/// Every other kind is refused `PermissionDenied`, **which is what bounds the *kind* a
/// compromised broker can install, whatever its policy table says.**
///
/// **It is not the whole bound, and the gap is named here rather than left to be inferred.**
/// The kernel bounds the kind (here), the rights (check **d**), the target (there is no target
/// argument), the lease (check **e**), and -- since the 0.45 follow-ups -- two things about
/// `object_id`: it is **not object 0** (so `object-zero`'s invariant, "no task can receive on
/// endpoint object 0", no longer rests on the broker's policy table; `RECV` is inside
/// [`ENDPOINT_ENVELOPE`]) and it **names an allocated endpoint**.
///
/// **It is still not the whole bound.** *Which* allocated endpoint a compromised broker may confer
/// -- any of them, with the envelope's rights -- is bounded by the policy table alone: the
/// installer **fabricates** a capability from an integer rather than deriving one it holds. That
/// is the 0.46 RFC-level property the architect filed at RFC-0.45-002's review (2026-11-17):
/// require the installer to hold a capability to that object carrying at least the rights conferred.
pub const INSTALLABLE_KINDS: [CapKind; 1] = [CapKind::Endpoint];

/// The rights an `Endpoint` install may confer: exactly the union the broker's policy
/// grants for the `Endpoint` class (`EP_RW | MINT`). It is the *envelope* the broker's
/// `CapInstall` capability carries beside `CAP_INSTALL` -- an install can never confer a
/// right outside it, whatever the broker asks for.
pub const ENDPOINT_ENVELOPE: CapRights = CapRights(
    CapRights::SEND.0
        | CapRights::RECV.0
        | CapRights::CALL.0
        | CapRights::REPLY.0
        | CapRights::COPY.0
        | CapRights::MINT.0,
);

/// What the caller's pending reply edge looks like (check **b**).
///
/// The edge's own lease binding (`ReplyEdge.lease`, `None` for a bootstrap or unbound
/// call) is **irrelevant here**: the install's lease is check **e**'s -- owned by the
/// installer -- not the call's. An unbound call can receive an install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyEdgeFact {
    /// The installer has no pending reply edge: nothing is being served.
    Absent,
    /// There is an edge but the task it points at is not `Blocked` (exited, faulted,
    /// or already woken): nothing is waiting for this answer.
    CallerNotBlocked,
    /// There is an edge and its caller is `Blocked` in the call.
    CallerBlocked,
}

/// What the lease named by the install looks like (check **e**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseFact {
    /// The id is `0`: no lease. An install is always lease-bound (`BROKER-004`).
    Zero,
    /// The lease table could not describe it (`NotFound`, ...): that error is the answer.
    Unreadable(SysError),
    /// The lease exists and its owner is not the installer.
    NotOwned,
    /// The installer owns it; `Ok(())` if `Active`, else the lease's own state error.
    Owned(Result<(), SysError>),
}

/// What the installer's `src_cap` is (check **c**).
#[derive(Clone, Copy, Debug)]
pub enum SourceFact {
    /// The handle did not resolve to a usable capability in the installer's CSpace (empty slot,
    /// stale generation, revoked lease, ...): the error the lookup gave is the answer.
    Unusable(SysError),
    /// A live capability the installer holds.
    Held {
        kind: CapKind,
        object_id: u32,
        rights: CapRights,
        scope: ObjectScope,
        /// Whether `object_id` names an **allocated** endpoint (`EndpointTable`'s own `used` flag).
        object_allocated: bool,
    },
}

/// The facts, gathered by the kernel.
#[derive(Clone, Copy, Debug)]
pub struct InstallFacts {
    pub edge: ReplyEdgeFact,
    pub source: SourceFact,
    /// The rights asked for.
    pub rights: CapRights,
    /// The rights the installer's `CapInstall` capability carries.
    pub installer_rights: CapRights,
    pub lease: LeaseFact,
    /// Whether the target's CSpace has a free slot.
    pub target_has_free_slot: bool,
}

/// What may be installed once every check has passed. **Everything here comes from the source
/// capability** -- the caller supplied none of it but the rights, and those are bounded by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Validated {
    pub kind: CapKind,
    pub object_id: u32,
    /// The source's scope: derivation never widens it.
    pub scope: ObjectScope,
}

/// Decide checks **b** to **f**.
pub fn check_install(f: &InstallFacts) -> Result<Validated, SysError> {
    // b. something is being served, and its caller is waiting.
    match f.edge {
        ReplyEdgeFact::Absent | ReplyEdgeFact::CallerNotBlocked => return Err(SysError::BadState),
        ReplyEdgeFact::CallerBlocked => {}
    }

    // c. the source: a capability the installer holds, of an installable kind, naming a real object.
    let (kind, object_id, src_rights, scope, allocated) = match f.source {
        SourceFact::Unusable(e) => return Err(e),
        SourceFact::Held {
            kind,
            object_id,
            rights,
            scope,
            object_allocated,
        } => (kind, object_id, rights, scope, object_allocated),
    };
    if !INSTALLABLE_KINDS.contains(&kind) {
        return Err(SysError::PermissionDenied);
    }
    // Never object 0: nobody may be handed `RECV` on it (`object-zero`'s invariant), and no runtime
    // install has a use for it. Then: it must be a real, allocated endpoint.
    if object_id == 0 {
        return Err(SysError::PermissionDenied);
    }
    if !allocated {
        return Err(SysError::InvalidArg);
    }

    // d. rights: non-empty, inside what the SOURCE holds and inside the installer's envelope (and
    // never a meta-right). Authority is attenuated, never amplified.
    let envelope = CapRights(f.installer_rights.0 & CapRights::ALL_NON_META.0);
    let ceiling = CapRights(envelope.0 & src_rights.0);
    if f.rights.0 == 0 || f.rights.0 & !ceiling.0 != 0 {
        return Err(SysError::PermissionDenied);
    }

    // e. the lease: a real one, the installer's own, still Active. The owner is checked
    // before the state so a non-owner learns nothing about a lease that is not theirs --
    // the order RFC-0.42-001 R2 set for `sys_cap_bind_lease`.
    match f.lease {
        LeaseFact::Zero | LeaseFact::NotOwned => return Err(SysError::PermissionDenied),
        LeaseFact::Unreadable(e) => return Err(e),
        LeaseFact::Owned(Err(e)) => return Err(e),
        LeaseFact::Owned(Ok(())) => {}
    }

    // f. room in the target; refusing here touches nothing.
    if !f.target_has_free_slot {
        return Err(SysError::NoMemory);
    }

    Ok(Validated {
        kind,
        object_id,
        scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(kind: CapKind, object_id: u32, rights: CapRights) -> SourceFact {
        SourceFact::Held {
            kind,
            object_id,
            rights,
            scope: ObjectScope::Any,
            object_allocated: true,
        }
    }

    fn good() -> InstallFacts {
        InstallFacts {
            edge: ReplyEdgeFact::CallerBlocked,
            source: held(CapKind::Endpoint, 23, CapRights::SEND),
            rights: CapRights::SEND,
            installer_rights: CapRights(CapRights::CAP_INSTALL.0 | ENDPOINT_ENVELOPE.0),
            lease: LeaseFact::Owned(Ok(())),
            target_has_free_slot: true,
        }
    }

    fn ok(object: u32) -> Result<Validated, SysError> {
        Ok(Validated {
            kind: CapKind::Endpoint,
            object_id: object,
            scope: ObjectScope::Any,
        })
    }

    #[test]
    fn the_happy_path_is_accepted_and_everything_installed_comes_from_the_source() {
        assert_eq!(check_install(&good()), ok(23));
    }

    // ── b ─────────────────────────────────────────────────────────────────────
    #[test]
    fn no_pending_reply_edge_is_bad_state() {
        let mut f = good();
        f.edge = ReplyEdgeFact::Absent;
        assert_eq!(check_install(&f), Err(SysError::BadState));
    }

    #[test]
    fn a_caller_that_is_not_blocked_is_bad_state() {
        let mut f = good();
        f.edge = ReplyEdgeFact::CallerNotBlocked;
        assert_eq!(check_install(&f), Err(SysError::BadState));
    }

    // ── c: the source ─────────────────────────────────────────────────────────
    #[test]
    fn a_source_that_does_not_resolve_returns_the_lookups_own_error() {
        for e in [
            SysError::InvalidCap,
            SysError::SlotEmpty,
            SysError::LeaseRevoked,
        ] {
            let mut f = good();
            f.source = SourceFact::Unusable(e);
            assert_eq!(check_install(&f), Err(e));
        }
    }

    #[test]
    fn every_source_kind_but_endpoint_is_permission_denied() {
        // Walk every discriminant the type knows: all but Endpoint must be refused.
        let mut checked_endpoint = false;
        let mut refused = 0usize;
        for raw in 0u8..=255 {
            let Some(kind) = CapKind::from_u8(raw) else {
                continue;
            };
            let mut f = good();
            f.source = held(kind, 23, CapRights::SEND);
            if kind == CapKind::Endpoint {
                assert_eq!(check_install(&f), ok(23));
                checked_endpoint = true;
            } else {
                assert_eq!(
                    check_install(&f),
                    Err(SysError::PermissionDenied),
                    "{kind:?}"
                );
                refused += 1;
            }
        }
        assert!(
            checked_endpoint,
            "control: Endpoint itself was never exercised"
        );
        assert!(
            refused > 10,
            "control: the walk found only {refused} other kinds"
        );
    }

    #[test]
    fn endpoint_object_zero_is_never_installable_even_from_a_held_capability() {
        // `object-zero`'s invariant as a preventive control: RECV is in the envelope, so the
        // refusal -- not the policy table, and not the broker's holdings -- keeps anyone
        // receiving on object 0.
        let mut f = good();
        f.source = held(CapKind::Endpoint, 0, ENDPOINT_ENVELOPE);
        f.rights = CapRights::RECV;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
        // Control: the same request against a real object passes.
        f.source = held(CapKind::Endpoint, 23, ENDPOINT_ENVELOPE);
        assert_eq!(check_install(&f), ok(23));
    }

    #[test]
    fn a_source_naming_an_unallocated_object_is_invalid_arg() {
        let mut f = good();
        f.source = SourceFact::Held {
            kind: CapKind::Endpoint,
            object_id: 23,
            rights: CapRights::SEND,
            scope: ObjectScope::Any,
            object_allocated: false,
        };
        assert_eq!(check_install(&f), Err(SysError::InvalidArg));
    }

    #[test]
    fn the_installed_scope_is_the_sources_derivation_never_widens() {
        let mut f = good();
        f.source = SourceFact::Held {
            kind: CapKind::Endpoint,
            object_id: 23,
            rights: CapRights::SEND,
            scope: ObjectScope::Lease(fjell_abi::lease::LeaseId(7)),
            object_allocated: true,
        };
        let v = check_install(&f).unwrap();
        assert_eq!(v.scope, ObjectScope::Lease(fjell_abi::lease::LeaseId(7)));
    }

    // ── d ─────────────────────────────────────────────────────────────────────
    #[test]
    fn empty_rights_are_refused() {
        let mut f = good();
        f.rights = CapRights(0);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    #[test]
    fn rights_outside_the_envelope_are_refused_even_if_the_source_holds_them() {
        let mut f = good();
        // INSPECT is a real right, in ALL_NON_META, but not in the envelope; the source holds it.
        f.source = held(
            CapKind::Endpoint,
            23,
            CapRights(CapRights::SEND.0 | CapRights::INSPECT.0),
        );
        f.rights = CapRights(CapRights::SEND.0 | CapRights::INSPECT.0);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    #[test]
    fn rights_the_source_does_not_hold_are_refused_even_inside_the_envelope() {
        // THE 0.46 PROPERTY: the broker holds SEND on the object; RECV is inside the envelope and the
        // policy might grant it, and still it cannot be conferred -- authority is attenuated, never
        // amplified.
        let mut f = good();
        f.rights = CapRights(CapRights::SEND.0 | CapRights::RECV.0);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
        f.rights = CapRights::RECV;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    #[test]
    fn a_subset_of_the_source_is_conferrable() {
        let mut f = good();
        f.source = held(CapKind::Endpoint, 23, ENDPOINT_ENVELOPE);
        f.rights = CapRights(CapRights::SEND.0 | CapRights::CALL.0);
        assert_eq!(check_install(&f), ok(23));
        f.rights = ENDPOINT_ENVELOPE;
        assert_eq!(
            check_install(&f),
            ok(23),
            "the whole of a source that holds the whole envelope"
        );
    }

    #[test]
    fn the_install_right_itself_is_never_conferred() {
        let mut f = good();
        // Even a source and an installer whose own rights name it cannot pass it on.
        f.installer_rights = CapRights(f.installer_rights.0 | CapRights::CAP_INSTALL.0);
        f.source = held(
            CapKind::Endpoint,
            23,
            CapRights(CapRights::SEND.0 | CapRights::CAP_INSTALL.0),
        );
        f.rights = CapRights::CAP_INSTALL;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    // ── e ─────────────────────────────────────────────────────────────────────
    #[test]
    fn lease_zero_is_refused() {
        let mut f = good();
        f.lease = LeaseFact::Zero;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    #[test]
    fn a_lease_the_installer_does_not_own_is_refused() {
        let mut f = good();
        f.lease = LeaseFact::NotOwned;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied));
    }

    #[test]
    fn a_revoked_lease_is_refused_with_its_own_error() {
        let mut f = good();
        f.lease = LeaseFact::Owned(Err(SysError::LeaseRevoked));
        assert_eq!(check_install(&f), Err(SysError::LeaseRevoked));
    }

    #[test]
    fn an_unreadable_lease_returns_the_tables_error() {
        let mut f = good();
        f.lease = LeaseFact::Unreadable(SysError::InvalidArg);
        assert_eq!(check_install(&f), Err(SysError::InvalidArg));
    }

    // ── f ─────────────────────────────────────────────────────────────────────
    #[test]
    fn a_full_target_is_no_memory() {
        let mut f = good();
        f.target_has_free_slot = false;
        assert_eq!(check_install(&f), Err(SysError::NoMemory));
    }

    // ── order ─────────────────────────────────────────────────────────────────
    #[test]
    fn the_checks_run_in_the_documented_order() {
        // Everything wrong at once: b is what is reported. Fix b, c is reported; and so on down to f.
        // A reordering shows here, not only in a boot.
        let mut f = InstallFacts {
            edge: ReplyEdgeFact::Absent,
            source: SourceFact::Unusable(SysError::InvalidCap),
            rights: CapRights(0),
            installer_rights: CapRights(CapRights::CAP_INSTALL.0 | ENDPOINT_ENVELOPE.0),
            lease: LeaseFact::Zero,
            target_has_free_slot: false,
        };
        assert_eq!(check_install(&f), Err(SysError::BadState)); // b
        f.edge = ReplyEdgeFact::CallerBlocked;
        assert_eq!(check_install(&f), Err(SysError::InvalidCap)); // c (unusable source)
        f.source = held(CapKind::TaskControl, 0, ENDPOINT_ENVELOPE);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied)); // c (uninstallable kind)
        f.source = held(CapKind::Endpoint, 0, CapRights::SEND);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied)); // c (object 0)
        f.source = SourceFact::Held {
            kind: CapKind::Endpoint,
            object_id: 23,
            rights: CapRights::SEND,
            scope: ObjectScope::Any,
            object_allocated: false,
        };
        assert_eq!(check_install(&f), Err(SysError::InvalidArg)); // c (unallocated)
        f.source = held(CapKind::Endpoint, 23, CapRights::SEND);
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied)); // d (empty)
        f.rights = CapRights::SEND;
        f.lease = LeaseFact::NotOwned;
        assert_eq!(check_install(&f), Err(SysError::PermissionDenied)); // e
        f.lease = LeaseFact::Owned(Ok(()));
        assert_eq!(check_install(&f), Err(SysError::NoMemory)); // f
        f.target_has_free_slot = true;
        assert_eq!(check_install(&f), ok(23));
    }

    #[test]
    fn a_full_cspace_has_no_free_slot_and_refusing_leaves_every_slot_as_it_was() {
        use crate::cspace::{CSPACE_SLOTS, CSpace};
        let mut cs = CSpace::new();
        assert!(cs.has_free_slot(), "control: an empty CSpace has room");
        for i in 0..CSPACE_SLOTS {
            cs.install_root(CapKind::Endpoint, i as u32, CapRights::SEND)
                .unwrap();
        }
        // Full: the predicate the kernel asks is false, and it agrees with `install_any`.
        assert!(!cs.has_free_slot());
        let before: alloc_free::Snapshot = alloc_free::snapshot(&cs);
        let attempt = crate::slot::Capability {
            kind: CapKind::Endpoint,
            object_id: 9999,
            rights: CapRights::SEND,
            badge: 0,
            scope: crate::rights::ObjectScope::Any,
            state: crate::rights::CapState::Active,
            parent: None,
            lease: None,
        };
        assert_eq!(cs.install_any(attempt), Err(SysError::NoMemory));
        assert!(
            alloc_free::snapshot(&cs) == before,
            "a refused install touched a slot"
        );
        // And the check function reports the same refusal for it.
        let mut f = good();
        f.target_has_free_slot = cs.has_free_slot();
        assert_eq!(check_install(&f), Err(SysError::NoMemory));
        // Free one: the predicate flips, and so does the answer.
        let h = crate::handle::CapHandle::new(3, 0);
        crate::enforcement::cap_drop(&mut cs, h).unwrap();
        assert!(cs.has_free_slot());
    }

    /// A fixed-size comparison of a full CSpace's slots (no allocator in this crate).
    mod alloc_free {
        use crate::cspace::{CSPACE_SLOTS, CSpace};
        #[derive(PartialEq)]
        pub struct Snapshot([(u16, bool, u32, u64); CSPACE_SLOTS]);
        pub fn snapshot(cs: &CSpace) -> Snapshot {
            let mut out = [(0u16, false, 0u32, 0u64); CSPACE_SLOTS];
            for (i, s) in cs.slots().iter().enumerate() {
                out[i] = (
                    s.generation,
                    s.cap.is_some(),
                    s.cap.map_or(0, |c| c.object_id),
                    s.cap.map_or(0, |c| c.rights.0),
                );
            }
            Snapshot(out)
        }
    }

    #[test]
    fn the_envelope_is_exactly_the_endpoint_policy_union() {
        // `fjell-cap-broker`'s `EP_RW | RIGHT_MINT`; if the broker's table moves, so must this.
        assert_eq!(
            ENDPOINT_ENVELOPE.0,
            (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8)
        );
        assert!(!ENDPOINT_ENVELOPE.contains(CapRights::CAP_INSTALL));
    }
}
