//! Golden digests: the bytes each format's encoder produced **before**
//! RFC-0.33-003 rewrote it to write through `Canon`.
//!
//! They were captured from the unmodified encoders (the commit that added this
//! file touches no format crate) and must never be regenerated to make a change
//! pass: an encoder that no longer reproduces one has changed a wire format, and
//! every digest and signature stored under the old bytes stops verifying. Change
//! a constant here only with a version bump and a note.
//!
//! Each sample gives every field a distinct value (`fjell_schema::samples`), so a
//! reordered, dropped or re-widthed field changes the digest.

use fjell_schema::samples as s;

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn check(name: &str, got: &[u8], want: &str) {
    assert_eq!(
        hex(got),
        want,
        "{name}: the canonical byte stream changed — this is a wire-format change"
    );
}

#[test]
fn rollback_record() {
    check(
        "rollback_record",
        &s::rollback_record().compute_digest().0,
        "711a40e750f357b3590e5cc26e8e9c7280f00ab50aa7cb60daaa8e70979f10be",
    );
}

#[test]
fn release_metadata() {
    check(
        "release_metadata",
        &s::release_metadata().compute_digest().0,
        "26ff3ba15eb1353854f8ec6c818aef13524dc3470b6e1447f0bc1b044c7111c4",
    );
}

#[test]
fn attestation_v2() {
    check(
        "attestation_v2",
        &s::attestation_v2().canonical_digest().0,
        "92913e8021da2ff2216e4b7bcc06a507e8023b5d121a4b1a436ab823ffd48d3e",
    );
}

#[test]
fn diag_bundle() {
    check(
        "diag_bundle",
        &s::diag_bundle().bundle_digest.0,
        "0f778f1b455602c0aeee5692a60fb5480aa3647a8297b6b2fcc8f124d78522bb",
    );
}

#[test]
fn identity() {
    check(
        "identity",
        &fjell_identity_format::digest::identity_digest(&s::node_identity()).0,
        "2cc3199076e7aa529192f90a5476431d7bcf58524a3fbda989c6673e4e8b76a1",
    );
}

#[test]
fn platform() {
    check(
        "platform",
        &fjell_platform_format::digest::platform_digest(&s::platform_profile()).0,
        "6bab6ef79928e390d519940bb6b56fb23d43c669994a3686c3d37c50eb984bef",
    );
}

#[test]
fn board() {
    check(
        "board",
        &fjell_platform_format::digest::board_digest(&s::board_profile()).0,
        "9b25b44469ec9aae647b546d045ddb904044d606fcc1e7489b191842b88fabdb",
    );
}

#[test]
fn keyring_snapshot() {
    check(
        "keyring_snapshot",
        &s::keyring_snapshot().snapshot_digest.0,
        "f7a41190c5bb8552702ad9d5d081ccbb77cfb9e23116bc63955cbec547ca29b3",
    );
}

#[test]
fn measurement_summary() {
    check(
        "measurement_summary",
        &fjell_summary_format::digest::measurement_summary_digest(&s::measurement_summary()).0,
        "ecb541accf62f16f83572369b68bb91b4181abd0c2b658073f78c2a6c7b9fe50",
    );
}

#[test]
fn release_summary() {
    check(
        "release_summary",
        &fjell_summary_format::digest::release_summary_digest(&s::release_summary()).0,
        "1f50aea04d4550a8edbeddd30eb2f772087097d1a654deedfc56d8e35d2955ed",
    );
}

#[test]
fn snapshot_v1_has_no_domain_byte() {
    check(
        "snapshot_v1",
        &fjell_snapshot_format::snapshot_digest(&s::snapshot_envelope(1)).0,
        "39103194679985baf1a375c10eff4e73312f7764fb75645832dc11d63e48f505",
    );
}

#[test]
fn snapshot_v2_has_the_domain_byte() {
    check(
        "snapshot_v2",
        &fjell_snapshot_format::snapshot_digest(&s::snapshot_envelope(2)).0,
        "bcb4aea040371bad12bf59a771d04c3ac30e9ffb5a9758069608a0ac5ae1059a",
    );
}

/// The control for the whole file: a golden that cannot fail proves nothing. The
/// same sample with one field changed must NOT reproduce its digest.
#[test]
fn a_changed_field_does_not_reproduce_its_golden() {
    let mut r = s::rollback_record();
    r.min_counter ^= 1;
    assert_ne!(
        hex(&r.compute_digest().0),
        "711a40e750f357b3590e5cc26e8e9c7280f00ab50aa7cb60daaa8e70979f10be"
    );
}
