//! Who may append, and what an append's word may say (RFC-0.39-001, E-096).
//!
//! `measuredd` used to read `source` out of the caller's own word and record it
//! verbatim, while the kernel-attested sender it had just received went
//! unused. The chain therefore recorded whatever the caller claimed, and for
//! three events per boot the claim was false. The decisions now live here, as
//! pure functions of the attested image id and the request word, so they are
//! tested on the host over every possible image id rather than only by
//! booting.
//!
//! Order of refusal, fixed so that a caller learns nothing about a request
//! from an appender it could not have made:
//!
//! 1. an appender with no [`MeasurementSource`] -> [`MeasurementError::PermissionDenied`];
//! 2. a non-zero declared `source` byte -> [`MeasurementError::InvalidSource`];
//! 3. a `kind` byte naming no kind -> [`MeasurementError::InvalidKind`];
//! 4. a `subject` byte naming no subject -> [`MeasurementError::InvalidSubject`].

use crate::event::{MeasurementError, MeasurementKind, MeasurementSource, MeasurementSubject};
use fjell_abi::service::ImageId;

/// The source to record for an event appended **by message** by `image_id`,
/// the kernel-attested image id of the sender.
///
/// Every mapped service records *itself*. Anything else is refused: there is
/// deliberately no default arm, because a default would record a false source
/// for the next appender nobody thought to list -- the defect this function
/// exists to remove, one level up. [`MeasurementSource::Kernel`] has no image:
/// it is reached only through `APPEND_IMAGE_SET`, whose classification
/// `measuredd` fixes (and whose sender is gated by [`require_init`]).
///
/// **`ATTESTD` is deliberately not mapped** (RFC-0.40-001 R5). It was, to
/// `MeasurementSource::Attestd`, and nothing ever appended as it. `attestd` now holds
/// a `CALL` capability to `measuredd` so it can READ the chain head for the record it
/// signs; the endpoint cannot tell a read from an append, so the only thing keeping
/// `attestd` from writing the chain is that an append from it is `PermissionDenied`.
/// Grant the read, refuse the append. (`MeasurementSource::Attestd` stays in the
/// published enum; it is simply not a source any service may currently be recorded
/// as.) A service that wants to be a recorded source must be named here, on purpose.
pub fn source_for_sender(image_id: u16) -> Result<MeasurementSource, MeasurementError> {
    Ok(match ImageId(image_id) {
        ImageId::INIT => MeasurementSource::Init,
        ImageId::MEASUREDD => MeasurementSource::Measuredd,
        ImageId::VERIFYD => MeasurementSource::Verifyd,
        ImageId::ROOTFSD => MeasurementSource::Rootfsd,
        ImageId::CONFIGD => MeasurementSource::Configd,
        ImageId::CAP_BROKER => MeasurementSource::CapBroker,
        ImageId::SERVICE_MANAGER => MeasurementSource::ServiceManager,
        ImageId::SNAPSHOTD => MeasurementSource::Snapshotd,
        ImageId::BOOTCTL => MeasurementSource::Bootctl,
        ImageId::UPGRADED => MeasurementSource::Upgraded,
        ImageId::RECOVERYD => MeasurementSource::Recoveryd,
        _ => return Err(MeasurementError::PermissionDenied),
    })
}

/// `APPEND_IMAGE_SET` may be sent by `init` alone. Its classification is fixed
/// (`source` = `Kernel`) *because* the digest comes from the kernel's
/// `BootInfo`; without this check `source: Kernel` would be a label any holder
/// of the capability could obtain.
pub fn require_init(image_id: u16) -> Result<(), MeasurementError> {
    if ImageId(image_id) == ImageId::INIT {
        Ok(())
    } else {
        Err(MeasurementError::PermissionDenied)
    }
}

/// Decode an `APPEND_EVENT` request word
/// (`kind << 24 | source << 16 | subject << 8 | flags`) sent by
/// `sender_image_id`. Returns the kind, the **attested** source and the
/// subject, or the first refusal in the order documented at module level.
///
/// The `source` byte is reserved and must be zero: accepting it and ignoring
/// it would leave every caller believing it had declared something.
pub fn decode_append_word(
    sender_image_id: u16,
    w0: usize,
) -> Result<(MeasurementKind, MeasurementSource, MeasurementSubject), MeasurementError> {
    let source = source_for_sender(sender_image_id)?;
    if (w0 >> 16) & 0xFF != 0 {
        return Err(MeasurementError::InvalidSource);
    }
    let kind =
        MeasurementKind::from_u8(((w0 >> 24) & 0xFF) as u8).ok_or(MeasurementError::InvalidKind)?;
    let subject = MeasurementSubject::from_u8(((w0 >> 8) & 0xFF) as u8)
        .ok_or(MeasurementError::InvalidSubject)?;
    Ok((kind, source, subject))
}

#[cfg(test)]
mod tests;
