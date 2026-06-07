//! Verus conformance: capability rights non-amplification.
//! RFC-v0.17-002 §6. Bridges the Verus proof
//! (verification/verus/capability/rights_lattice.rs) to the shipped
//! `CapRights::is_subset_of`. The proof reasons about the spec `subset`;
//! these cases drive the real function over the same scenarios.
//!
//! If this test and the proof ever disagree, the implementation has drifted
//! from the model — see the proof-drift checklist.
//!
//! Marker on success: CONFORMANCE:CAPABILITY-RIGHTS:PASS

use fjell_cap::rights::CapRights;

/// The model predicate, restated independently of the implementation, so the
/// test is not merely `is_subset_of == is_subset_of`. This is the same
/// formula the Verus `subset` spec proves over.
fn model_subset(child: u64, parent: u64) -> bool {
    (child & !parent) == 0
}

#[test]
fn rights_subset_matches_model_exhaustive_low_bits() {
    // Exhaustive over the low 8 bits (256 x 256) — every shipped result must
    // equal the model. This is the core CAP-RIGHTS-001/003 conformance.
    for child in 0u64..256 {
        for parent in 0u64..256 {
            let shipped = CapRights(child).is_subset_of(CapRights(parent));
            let model = model_subset(child, parent);
            assert_eq!(shipped, model,
                "drift at child={child:#x} parent={parent:#x}: shipped={shipped} model={model}");
        }
    }
}

#[test]
fn equal_rights_allowed() {
    // CAP-RIGHTS-001 boundary: equal rights are a valid mint source.
    let r = CapRights(0b1011);
    assert!(r.is_subset_of(r));
}

#[test]
fn strict_subset_allowed() {
    let parent = CapRights(0b1111);
    let child = CapRights(0b0101);
    assert!(child.is_subset_of(parent));
}

#[test]
fn adding_one_extra_right_rejected() {
    // CAP-RIGHTS-003: amplification must be refused.
    let parent = CapRights(0b0101);
    let child = CapRights(0b0111); // sets bit 1 not in parent
    assert!(!child.is_subset_of(parent), "amplifying mint must be rejected");
}

#[test]
fn zero_rights_subset_of_anything() {
    for parent in [0u64, 1, 0xFF, u64::MAX] {
        assert!(CapRights(0).is_subset_of(CapRights(parent)));
    }
}

#[test]
fn subset_is_transitive() {
    // Chain attenuation: a ⊆ b ⊆ c ⇒ a ⊆ c (proved in Verus too).
    let c = CapRights(0b1111);
    let b = CapRights(0b0111);
    let a = CapRights(0b0011);
    assert!(b.is_subset_of(c));
    assert!(a.is_subset_of(b));
    assert!(a.is_subset_of(c), "subset must be transitive");
    println!("CONFORMANCE:CAPABILITY-RIGHTS:PASS");
}
