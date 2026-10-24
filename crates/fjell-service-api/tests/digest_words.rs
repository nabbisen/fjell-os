//! `measuredd::{digest_to_words, words_to_digest}`: one definition of how a
//! 32-byte digest rides in four message words.

use fjell_service_api::measuredd::{digest_to_words, words_to_digest};

fn digest() -> [u8; 32] {
    let mut d = [0u8; 32];
    for (i, b) in d.iter_mut().enumerate() {
        *b = i as u8 + 1;
    }
    d
}

#[test]
fn word_i_is_bytes_8i_to_8i_plus_8_little_endian() {
    let w = digest_to_words(&digest());
    assert_eq!(w[0], 0x0807_0605_0403_0201);
    assert_eq!(w[1], 0x100F_0E0D_0C0B_0A09);
    assert_eq!(w[2], 0x1817_1615_1413_1211);
    assert_eq!(w[3], 0x201F_1E1D_1C1B_1A19);
}

#[test]
fn the_two_functions_are_inverses_for_any_digest() {
    for seed in 0u8..=255 {
        let mut d = [0u8; 32];
        for (i, b) in d.iter_mut().enumerate() {
            *b = seed.wrapping_mul(31).wrapping_add(i as u8);
        }
        assert_eq!(words_to_digest(digest_to_words(&d)), d);
    }
    assert_eq!(words_to_digest([0; 4]), [0u8; 32]);
    assert_eq!(words_to_digest([usize::MAX; 4]), [0xFFu8; 32]);
}
