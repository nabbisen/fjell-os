use super::*;
use fjell_measure_format::{
    Digest32, MeasurementEvent, MeasurementKind, MeasurementSource, MeasurementSubject,
};

const SET: &str = "5ca1c2f9b3322797c51f9beebd987d5e1fc9ebef90e9c1ce6243d5c1b116705f";

/// The in-tree implementation, run over the same four events. This is the
/// agreement test: the host oracle above shares no code with it.
fn in_tree_head(image_set: [u8; 32], source_of_placeholders: MeasurementSource) -> [u8; 32] {
    let mut ph = [0u8; 32];
    ph[0..8].copy_from_slice(&0xABu64.to_le_bytes());
    let rows = [
        (
            MeasurementKind::BootEvidenceImported,
            MeasurementSource::Kernel,
            MeasurementSubject::BootEvidence,
            image_set,
        ),
        (
            MeasurementKind::ReleaseManifestVerified,
            source_of_placeholders,
            MeasurementSubject::ReleaseManifest,
            ph,
        ),
        (
            MeasurementKind::RootfsManifestVerified,
            source_of_placeholders,
            MeasurementSubject::RootfsManifest,
            ph,
        ),
        (
            MeasurementKind::PolicyBundleVerified,
            source_of_placeholders,
            MeasurementSubject::PolicyBundle,
            ph,
        ),
    ];
    let mut prev = Digest32([0u8; 32]);
    for (i, (k, s, j, d)) in rows.into_iter().enumerate() {
        let ev = MeasurementEvent::new(i as u64 + 1, 0, k, s, j, Digest32(d), None, prev);
        prev = ev.chain_digest;
    }
    prev.0
}

#[test]
fn the_host_head_agrees_with_the_in_tree_chain() {
    let (events, head) = boot_chain_head_from(SET).unwrap();
    assert_eq!(events, 4);
    let set = unhex32(SET).unwrap();
    assert_eq!(head, hex(&in_tree_head(set, MeasurementSource::Init)));
}

#[test]
fn the_oracle_distinguishes_the_provenance_it_encodes() {
    // The head changes with the placeholders' source. A tier built on this
    // would therefore notice the old `Verifyd` label coming back.
    let (_, head) = boot_chain_head_from(SET).unwrap();
    let set = unhex32(SET).unwrap();
    assert_ne!(head, hex(&in_tree_head(set, MeasurementSource::Verifyd)));
}

#[test]
fn the_head_depends_on_the_image_set_digest() {
    let a = boot_chain_head_from(SET).unwrap().1;
    let mut other = SET.to_string();
    other.replace_range(0..2, "00");
    assert_ne!(a, boot_chain_head_from(&other).unwrap().1);
}

#[test]
fn a_malformed_image_set_digest_is_refused() {
    assert!(boot_chain_head_from("abc").is_err());
    assert!(boot_chain_head_from(&"zz".repeat(32)).is_err());
}

#[test]
fn on_the_real_prebuilts_it_matches_the_in_tree_chain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/fjell-tools is two levels under the workspace root");
    let (_n, set) = crate::image_set::image_set_digest(root).unwrap();
    let (_e, head) = boot_chain_head(root).unwrap();
    let want = in_tree_head(unhex32(&set).unwrap(), MeasurementSource::Init);
    assert_eq!(head, hex(&want));
}
