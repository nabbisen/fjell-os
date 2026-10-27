use super::*;

/// (image id, the source it must record). The *whole* table: anything not in it
/// must be refused, and `every_image_id_is_mapped_or_refused` proves that over
/// all 65 536 ids, so adding a service cannot silently inherit a source.
const MAPPED: &[(ImageId, MeasurementSource)] = &[
    (ImageId::INIT, MeasurementSource::Init),
    (ImageId::MEASUREDD, MeasurementSource::Measuredd),
    (ImageId::VERIFYD, MeasurementSource::Verifyd),
    (ImageId::ROOTFSD, MeasurementSource::Rootfsd),
    (ImageId::CONFIGD, MeasurementSource::Configd),
    (ImageId::CAP_BROKER, MeasurementSource::CapBroker),
    (ImageId::SERVICE_MANAGER, MeasurementSource::ServiceManager),
    (ImageId::SNAPSHOTD, MeasurementSource::Snapshotd),
    (ImageId::BOOTCTL, MeasurementSource::Bootctl),
    (ImageId::UPGRADED, MeasurementSource::Upgraded),
    (ImageId::RECOVERYD, MeasurementSource::Recoveryd),
    (ImageId::ATTESTD, MeasurementSource::Attestd),
];

fn expected(id: u16) -> Option<MeasurementSource> {
    MAPPED.iter().find(|(i, _)| i.0 == id).map(|(_, s)| *s)
}

#[test]
fn every_image_id_is_mapped_or_refused() {
    let mut mapped = 0;
    for id in 0..=u16::MAX {
        match (source_for_sender(id), expected(id)) {
            (Ok(got), Some(want)) => {
                assert_eq!(got, want, "image id {id} records the wrong source");
                mapped += 1;
            }
            (Err(e), None) => assert_eq!(e, MeasurementError::PermissionDenied, "image id {id}"),
            (got, want) => panic!("image id {id}: got {got:?}, table says {want:?}"),
        }
    }
    assert_eq!(mapped, MAPPED.len());
}

#[test]
fn no_service_can_record_as_the_kernel() {
    for id in 0..=u16::MAX {
        assert_ne!(
            source_for_sender(id),
            Ok(MeasurementSource::Kernel),
            "image id {id}"
        );
    }
}

#[test]
fn the_services_that_must_be_refused_are() {
    // Spot checks by name, so a reader sees the intent and not only the sweep:
    // the test appender, drivers, storage and a presentation service are not
    // measurement producers.
    for id in [
        ImageId::NEG_TEST,
        ImageId::DRIVER_UART,
        ImageId::DRIVER_VIRTIO_BLK,
        ImageId::STORAGED,
        ImageId::AUDITD,
        ImageId::DEVMGR,
        ImageId::SVC_PRESENTATION_LIAR,
        ImageId::DIAGNOSTICSD,
    ] {
        assert_eq!(
            source_for_sender(id.0),
            Err(MeasurementError::PermissionDenied),
            "{id:?}"
        );
    }
}

#[test]
fn only_init_may_append_the_image_set() {
    assert_eq!(require_init(ImageId::INIT.0), Ok(()));
    for id in 1..=u16::MAX {
        assert_eq!(
            require_init(id),
            Err(MeasurementError::PermissionDenied),
            "image id {id}"
        );
    }
}

fn word(kind: u8, source: u8, subject: u8) -> usize {
    (kind as usize) << 24 | (source as usize) << 16 | (subject as usize) << 8
}

#[test]
fn a_mapped_sender_records_itself() {
    for (id, src) in MAPPED {
        assert_eq!(
            decode_append_word(id.0, word(2, 0, 2)),
            Ok((
                MeasurementKind::ReleaseManifestVerified,
                *src,
                MeasurementSubject::ReleaseManifest
            ))
        );
    }
}

#[test]
fn init_records_as_init_not_verifyd() {
    // The E-096 defect, as a regression: the three verification placeholders
    // were sent declaring source 2 and the chain believed it.
    for (k, s) in [(2u8, 2u8), (3, 3), (4, 4)] {
        let (_, src, _) = decode_append_word(ImageId::INIT.0, word(k, 0, s)).unwrap();
        assert_eq!(src, MeasurementSource::Init);
    }
    assert_eq!(
        decode_append_word(ImageId::INIT.0, word(2, 2, 2)),
        Err(MeasurementError::InvalidSource)
    );
}

#[test]
fn every_non_zero_source_byte_is_refused_not_ignored() {
    for b in 1..=u8::MAX {
        assert_eq!(
            decode_append_word(ImageId::INIT.0, word(2, b, 2)),
            Err(MeasurementError::InvalidSource),
            "source byte {b}"
        );
    }
}

#[test]
fn an_unmapped_sender_is_refused_before_the_word_is_read() {
    // A bad word from an unmapped appender reports the appender, not the word.
    assert_eq!(
        decode_append_word(ImageId::NEG_TEST.0, word(0xEE, 0xEE, 0xEE)),
        Err(MeasurementError::PermissionDenied)
    );
}

#[test]
fn an_unknown_kind_or_subject_is_refused_not_promoted() {
    // The old decoder recorded an unrecognised kind as BootEvidenceImported and
    // an unrecognised subject as BootEvidence -- the chain's most load-bearing
    // claim.
    for b in [0x00u8, 0x12, 0x55, 0xFF] {
        assert_eq!(
            decode_append_word(ImageId::INIT.0, word(b, 0, 2)),
            Err(MeasurementError::InvalidKind),
            "kind {b:#x}"
        );
    }
    for b in [0x00u8, 0x0D, 0x55, 0xFF] {
        assert_eq!(
            decode_append_word(ImageId::INIT.0, word(2, 0, b)),
            Err(MeasurementError::InvalidSubject),
            "subject {b:#x}"
        );
    }
}

#[test]
fn kind_and_subject_round_trip_over_every_byte() {
    let mut kinds = 0;
    let mut subjects = 0;
    for b in 0..=u8::MAX {
        if let Some(k) = MeasurementKind::from_u8(b) {
            assert_eq!(k.as_u8(), b);
            kinds += 1;
        }
        if let Some(s) = MeasurementSubject::from_u8(b) {
            assert_eq!(s.as_u8(), b);
            subjects += 1;
        }
    }
    assert_eq!((kinds, subjects), (17, 12));
}

#[test]
fn init_is_the_next_free_source_value() {
    assert_eq!(MeasurementSource::Init.as_u8(), 0x0D);
    assert_eq!(MeasurementSource::Measuredd.as_u8(), 0x0C);
}
