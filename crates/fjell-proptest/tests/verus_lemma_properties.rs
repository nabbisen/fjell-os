//! Property tests for the Verus pilot lemmas.
//!
//! Real Verus machine-checking is blocked in the build sandbox (GitHub
//! release-asset hosts are not in the network allowlist; see
//! verification/verus/TOOLCHAIN.md). Until a developer runs Verus with
//! network access, these property tests give the strongest available
//! empirical evidence for *exactly the lemmas the proofs assert* — not just
//! the point cases in the conformance tests. The pack explicitly lists
//! property/corpus tests as a valid conformance artifact
//! (guides/03 §3).
//!
//! Each property below names the Verus lemma it mirrors.

use proptest::prelude::*;
use fjell_cap::rights::CapRights;
use fjell_abi::lease::{lease_usable, lease_revoke_epoch};
use fjell_upgrade_format::{BootControlBlock, BcbMirrorSelection, select_bcb_mirror};

// ── Capability rights (RFC-v0.17-002) ─────────────────────────────────────────

proptest! {
    // Verus lemma: subset_is_transitive
    #[test]
    fn prop_subset_transitive(a in any::<u64>(), b in any::<u64>(), c in any::<u64>()) {
        let (ra, rb, rc) = (CapRights(a), CapRights(b), CapRights(c));
        if ra.is_subset_of(rb) && rb.is_subset_of(rc) {
            prop_assert!(ra.is_subset_of(rc), "transitivity violated: {a:#x} {b:#x} {c:#x}");
        }
    }

    // Verus lemma: mint_never_amplifies — an allowed mint sets no bit outside parent.
    #[test]
    fn prop_no_amplification(parent in any::<u64>(), child in any::<u64>()) {
        if CapRights(child).is_subset_of(CapRights(parent)) {
            prop_assert_eq!(child & !parent, 0, "amplification slipped through");
        }
    }

    // Verus lemma: zero_is_subset
    #[test]
    fn prop_zero_is_subset(parent in any::<u64>()) {
        prop_assert!(CapRights(0).is_subset_of(CapRights(parent)));
    }

    // Verus lemma: equal_rights_allowed (reflexivity)
    #[test]
    fn prop_reflexive(r in any::<u64>()) {
        prop_assert!(CapRights(r).is_subset_of(CapRights(r)));
    }

    // The mint relation is antisymmetric over distinct equal-bit sets:
    // if a ⊆ b and b ⊆ a then a == b.
    #[test]
    fn prop_subset_antisymmetric(a in any::<u64>(), b in any::<u64>()) {
        if CapRights(a).is_subset_of(CapRights(b)) && CapRights(b).is_subset_of(CapRights(a)) {
            prop_assert_eq!(a, b);
        }
    }
}

// ── Lease epoch revocation (RFC-v0.17-003) ─────────────────────────────────────

proptest! {
    // Verus lemma: revoke_advances_epoch (strict monotonicity, no wrap region)
    #[test]
    fn prop_revoke_monotonic(e in 0u32..u32::MAX) {
        prop_assert!(lease_revoke_epoch(e) > e, "epoch must strictly advance");
    }

    // Verus lemma: revoked_binding_not_usable — usable before ⇒ not usable after.
    #[test]
    fn prop_revoked_not_usable(epoch in 0u32..u32::MAX) {
        // Binding issued at `epoch`, lease active at `epoch` → usable.
        prop_assert!(lease_usable(true, epoch, epoch));
        // After revoke: lease inactive, epoch advanced → not usable.
        let new_epoch = lease_revoke_epoch(epoch);
        prop_assert!(!lease_usable(false, new_epoch, epoch));
    }

    // Verus lemma: revoke_blocks_even_new_epoch_binding — inactive rejects any binding.
    #[test]
    fn prop_inactive_rejects_any(cur in any::<u32>(), issue in any::<u32>()) {
        prop_assert!(!lease_usable(false, cur, issue),
            "inactive lease must reject every binding");
    }

    // Usability requires BOTH active and exact epoch match.
    #[test]
    fn prop_usable_iff_active_and_match(active in any::<bool>(),
                                        cur in any::<u32>(), issue in any::<u32>()) {
        prop_assert_eq!(lease_usable(active, cur, issue), active && cur == issue);
    }
}

// ── Boot-control mirror selection (RFC-v0.17-004) ──────────────────────────────

fn mk(valid: bool, generation: u64) -> BootControlBlock {
    let mut b = BootControlBlock::new(generation);
    b.generation = generation;
    b.seal();
    if !valid { b.magic = *b"BADMAGIC"; }
    b
}

proptest! {
    // Verus lemma: selection_is_total — never panics, always one of four variants.
    #[test]
    fn prop_selection_total(va in any::<bool>(), ga in any::<u64>(),
                            vb in any::<bool>(), gb in any::<u64>()) {
        let a = mk(va, ga);
        let b = mk(vb, gb);
        let sel = select_bcb_mirror(&a, &b);
        let ok = matches!(sel,
            BcbMirrorSelection::SelectedA(_) | BcbMirrorSelection::SelectedB(_)
            | BcbMirrorSelection::BothValidSameGeneration(_) | BcbMirrorSelection::NoneValid);
        prop_assert!(ok);
    }

    // Verus lemma: none_only_when_both_invalid
    #[test]
    fn prop_none_iff_both_invalid(ga in any::<u64>(), gb in any::<u64>(),
                                  va in any::<bool>(), vb in any::<bool>()) {
        let a = mk(va, ga); let b = mk(vb, gb);
        let is_none = matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::NoneValid);
        prop_assert_eq!(is_none, !a.is_valid() && !b.is_valid());
    }

    // Verus lemma: higher_generation_a_wins / b_wins (both valid, strict gen order).
    #[test]
    fn prop_higher_generation_wins(ga in any::<u64>(), gb in any::<u64>()) {
        prop_assume!(ga != gb);
        let a = mk(true, ga); let b = mk(true, gb);
        match select_bcb_mirror(&a, &b) {
            BcbMirrorSelection::SelectedA(_) => prop_assert!(ga > gb),
            BcbMirrorSelection::SelectedB(_) => prop_assert!(gb > ga),
            other => prop_assert!(false, "unexpected {:?} for distinct gens", other),
        }
    }

    // Verus lemma: equal_generation_is_tiebreak (both valid, equal gen → tie variant).
    #[test]
    fn prop_equal_generation_tiebreak(g in any::<u64>()) {
        let a = mk(true, g); let b = mk(true, g);
        prop_assert!(matches!(select_bcb_mirror(&a, &b),
            BcbMirrorSelection::BothValidSameGeneration(_)));
    }
}
