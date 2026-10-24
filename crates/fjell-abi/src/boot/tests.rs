//! `BootInfo`'s wire encoding: the record is the bytes, not the struct.

use super::*;

fn sample() -> BootInfo {
    let mut d = [0u8; 32];
    for (i, b) in d.iter_mut().enumerate() {
        *b = 0xA0 + i as u8;
    }
    BootInfo::new(0x0800_0000, 33, d)
}

#[test]
fn the_encoding_is_the_documented_offsets() {
    let b = sample().to_bytes();
    assert_eq!(b.len(), 56);
    assert_eq!(&b[0..4], &[2, 0, 0, 0], "version 2, little-endian");
    for off in [4, 6, 8, 10, 12] {
        assert_eq!(&b[off..off + 2], &[0xFF, 0xFF], "ABSENT at offset {off}");
    }
    assert_eq!(&b[14..22], &0x0800_0000u64.to_le_bytes());
    assert_eq!(&b[22..24], &[33, 0]);
    assert_eq!(b[24], 0xA0);
    assert_eq!(b[55], 0xA0 + 31);
}

#[test]
fn a_record_round_trips() {
    let info = sample();
    assert_eq!(BootInfo::from_bytes(&info.to_bytes()), Ok(info));
}

#[test]
fn a_short_buffer_is_refused() {
    let b = sample().to_bytes();
    assert_eq!(BootInfo::from_bytes(&b[..55]), Err(BootInfoError::Short));
    assert_eq!(BootInfo::from_bytes(&[]), Err(BootInfoError::Short));
}

#[test]
fn an_older_version_is_refused_and_a_newer_one_is_read_by_prefix() {
    let mut b = sample().to_bytes();
    b[0] = 1;
    assert_eq!(BootInfo::from_bytes(&b), Err(BootInfoError::Version(1)));
    b[0] = 0;
    assert_eq!(BootInfo::from_bytes(&b), Err(BootInfoError::Version(0)));

    // A later version appends fields: a version-2 reader keeps its prefix and
    // ignores the rest.
    let mut longer = [0u8; 80];
    longer[..56].copy_from_slice(&sample().to_bytes());
    longer[0] = 3;
    longer[56..].fill(0x77);
    let got = BootInfo::from_bytes(&longer).unwrap();
    assert_eq!(got.version, 3);
    assert_eq!(got.image_set_digest, sample().image_set_digest);
    assert_eq!(got.image_set_count, 33);
}

#[test]
fn an_all_zero_page_is_not_a_boot_info() {
    // What a mapped-but-unwritten page looks like: version 0.
    assert_eq!(
        BootInfo::from_bytes(&[0u8; 4096]),
        Err(BootInfoError::Version(0))
    );
}

#[test]
fn the_unprovided_fields_say_so() {
    let i = sample();
    assert_eq!(i.bootstrap_cap_count, BootInfo::ABSENT);
    assert_eq!(i.cap_task_create, BootInfo::ABSENT);
    assert_eq!(i.cap_derive, BootInfo::ABSENT);
    assert_eq!(i.cap_audit_drain, BootInfo::ABSENT);
    assert_eq!(i.cap_bootstrap_ep, BootInfo::ABSENT);
}
