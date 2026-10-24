//! Known-answer and refusal tests for the image-set digest.
//!
//! The vectors were produced by Python's `hashlib` from the layout documented
//! in the module doc — a third SHA-256 implementation, independent of the
//! in-tree one under test and of the host tool's `sha2`.

extern crate std;
use super::*;
use crate::CHAIN_DOMAIN;
use std::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

fn entry(image_id: u16, bytes: &[u8]) -> ImageEntry {
    ImageEntry {
        image_id,
        digest: Digest32::of(bytes),
    }
}

fn hex(d: &Digest32) -> String {
    let mut buf = [0u8; 64];
    d.to_hex(&mut buf).to_string()
}

#[test]
fn three_images_match_the_independent_vector() {
    let set = [entry(0, b"alpha"), entry(1, b"beta"), entry(5, b"gamma")];
    assert_eq!(
        hex(&compute_image_set_digest(&set).unwrap()),
        "5ca1c2f9b3322797c51f9beebd987d5e1fc9ebef90e9c1ce6243d5c1b116705f"
    );
}

#[test]
fn per_image_digests_match_the_independent_vector() {
    assert_eq!(
        hex(&Digest32::of(b"alpha")),
        "8ed3f6ad685b959ead7022518e1af76cd816f8e8ec7ccdda1ed4018e8f2223f8"
    );
    assert_eq!(
        hex(&Digest32::of(b"gamma")),
        "be9d587defa1f0c09ef49eb17e206983a5f8f8289e4281860bd0ee5a19592c67"
    );
}

#[test]
fn one_image_and_an_empty_image_match_the_independent_vectors() {
    assert_eq!(
        hex(&compute_image_set_digest(&[entry(0, b"alpha")]).unwrap()),
        "d6038da7f295a9d71b7dcdbe881f8efa9d8a86bf4a30d7ce12f2777cb0c063eb"
    );
    // An image of zero bytes is still an image: it is hashed, not skipped.
    assert_eq!(
        hex(&compute_image_set_digest(&[entry(7, b"")]).unwrap()),
        "353c70148da3759fef7889624d1e0014d0b441abe45600e22096546fcffe562a"
    );
}

#[test]
fn a_full_set_matches_the_independent_vector_and_fills_the_buffer_exactly() {
    let owned: Vec<(u16, Vec<u8>)> = (0..IMAGE_SET_MAX as u16)
        .map(|i| (i, vec![i as u8; 3]))
        .collect();
    let set: Vec<ImageEntry> = owned.iter().map(|(i, b)| entry(*i, b)).collect();
    assert_eq!(set.len(), IMAGE_SET_MAX);
    assert_eq!(IMAGE_SET_PREIMAGE_MAX, 2196);
    let mut sink = BufSink::<IMAGE_SET_PREIMAGE_MAX>::new();
    write_image_set_canonical(&mut sink, &set);
    assert_eq!(sink.len(), IMAGE_SET_PREIMAGE_MAX);
    assert_eq!(
        hex(&compute_image_set_digest(&set).unwrap()),
        "358d56150bdb473863303a28a5c8e76dcfd363480c3878566c26381181407678"
    );
}

#[test]
fn the_stream_is_exactly_the_documented_layout() {
    let set = [entry(2, b"x"), entry(0x0102, b"y")];
    let mut sink = BufSink::<IMAGE_SET_PREIMAGE_MAX>::new();
    write_image_set_canonical(&mut sink, &set);
    let mut want = Vec::new();
    want.extend_from_slice(b"FJELL-IMAGE-SET-V1");
    want.extend_from_slice(&2u16.to_le_bytes());
    want.extend_from_slice(&2u16.to_le_bytes());
    want.extend_from_slice(&Digest32::of(b"x").0);
    want.extend_from_slice(&0x0102u16.to_le_bytes());
    want.extend_from_slice(&Digest32::of(b"y").0);
    assert_eq!(sink.bytes(), &want[..]);
    assert_eq!(&want[18..20], &[2, 0], "the count is little-endian");
    assert_eq!(&want[20..22], &[2, 0]);
    assert_eq!(&want[22 + 32..22 + 32 + 2], &[0x02, 0x01]);
}

#[test]
fn changing_any_of_image_byte_id_count_or_domain_changes_the_digest() {
    let base = [entry(0, b"alpha"), entry(1, b"beta")];
    let d = compute_image_set_digest(&base).unwrap();
    // one byte of one image
    let flipped = [entry(0, b"alphb"), entry(1, b"beta")];
    assert_ne!(d, compute_image_set_digest(&flipped).unwrap());
    // the same bytes under different ids
    let renumbered = [entry(0, b"alpha"), entry(2, b"beta")];
    assert_ne!(d, compute_image_set_digest(&renumbered).unwrap());
    // the same bytes, swapped between ids
    let swapped = [entry(0, b"beta"), entry(1, b"alpha")];
    assert_ne!(d, compute_image_set_digest(&swapped).unwrap());
    assert_eq!(
        hex(&compute_image_set_digest(&swapped).unwrap()),
        "f637bb9231abd69a699b78581b198d40b1f8848091a4d2b1f5536c733226b659"
    );
    // a dropped image
    assert_ne!(d, compute_image_set_digest(&base[..1]).unwrap());
    // the stream without its domain tag is a different digest
    let mut bare = BufSink::<IMAGE_SET_PREIMAGE_MAX>::new();
    write_image_set_canonical(&mut bare, &base);
    assert_ne!(d, Digest32::of(&bare.bytes()[IMAGE_SET_DOMAIN.len()..]));
}

#[test]
fn a_bad_set_is_refused_and_never_hashed() {
    assert_eq!(compute_image_set_digest(&[]), Err(ImageSetError::Empty));
    let e = |i| entry(i, b"x");
    assert_eq!(
        compute_image_set_digest(&[e(1), e(1)]),
        Err(ImageSetError::NotAscending),
        "a repeated id"
    );
    assert_eq!(
        compute_image_set_digest(&[e(2), e(1)]),
        Err(ImageSetError::NotAscending),
        "descending"
    );
    let many: Vec<ImageEntry> = (0..=IMAGE_SET_MAX as u16).map(e).collect();
    assert_eq!(many.len(), IMAGE_SET_MAX + 1);
    assert_eq!(compute_image_set_digest(&many), Err(ImageSetError::TooMany));
}

#[test]
fn the_chain_digest_stream_is_untouched() {
    // RFC-0.38-001 non-goal: adding this digest must not move the frozen
    // chain-digest stream (RFC-0.35-004). Its golden lives with the chain; this
    // pins that the two domains differ so they cannot be confused.
    assert_ne!(IMAGE_SET_DOMAIN, CHAIN_DOMAIN);
}
