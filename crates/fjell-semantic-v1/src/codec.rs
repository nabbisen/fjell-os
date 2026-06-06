//! Encode / decode for v1 semantic intent envelopes (RFC v0.5-004 §5.3, §7.1).
//!
//! Wire layout:
//! ```text
//! "FJSI-V1" (7 B) ||
//! intent_tag u16 LE ||
//! created_tick u64 LE ||
//! for each field in schema order:
//!   present u8 (always 1 for required, 0/1 for optional) ||
//!   if present: field bytes in fixed encoding
//! trailing sentinel u8 = 0xFF
//! ```

use crate::catalog::lookup_tag;
use crate::schema::{FieldKind};

/// Maximum number of fields per intent (catalog v1 never exceeds this).
pub const MAX_FIELDS: usize = 6;
/// Maximum encoded envelope size (prefix 18 B + 6 × 32 B fields + 1 sentinel).
pub const MAX_ENVELOPE_BYTES: usize = 18 + MAX_FIELDS * 33 + 1;

const MAGIC: &[u8; 7] = b"FJSI-V1";
const SENTINEL: u8 = 0xFF;

/// Errors from `encode` / `decode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SemanticError {
    /// The intent tag is not in the v1 catalog.
    UnknownTag        = 0x01,
    /// The output buffer is too small.
    BufferTooSmall    = 0x02,
    /// A required field is missing from the `fields` slice.
    RequiredFieldMissing = 0x03,
    /// Input is too short to contain the magic prefix.
    TruncatedMagic    = 0x10,
    /// Magic bytes do not match `FJSI-V1`.
    BadMagic          = 0x11,
    /// Input is truncated mid-envelope.
    TruncatedBody     = 0x12,
    /// Sentinel byte was not `0xFF`.
    MissingSentinel   = 0x13,
    /// Trailing bytes after sentinel.
    TrailingBytes     = 0x14,
    /// A field present-byte had an invalid value (> 1).
    InvalidPresentByte= 0x15,
}

/// A decoded field value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Bytes16([u8; 16]),
    Bytes32([u8; 32]),
    Absent,
}

/// A fully decoded intent envelope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodedIntent {
    pub tag:          u16,
    pub created_tick: u64,
    pub field_count:  u8,
    pub fields:       [FieldValue; MAX_FIELDS],
}

/// Encode a v1 intent envelope.
///
/// `fields` must supply one `FieldValue` per schema field in order.
/// Required fields must not be `Absent`; optional fields may be `Absent`.
/// Returns the number of bytes written.
pub fn encode(
    tag:          u16,
    created_tick: u64,
    fields:       &[FieldValue],
    out:          &mut [u8],
) -> Result<usize, SemanticError> {
    let entry = lookup_tag(tag).ok_or(SemanticError::UnknownTag)?;
    let schema = entry.schema;

    // Compute required size.
    let mut needed = 7 + 2 + 8 + 1; // magic + tag + tick + sentinel
    for (i, fd) in schema.fields.iter().enumerate() {
        needed += 1; // present byte
        let fv = fields.get(i).copied().unwrap_or(FieldValue::Absent);
        if fv != FieldValue::Absent { needed += fd.kind.wire_size(); }
    }
    if out.len() < needed { return Err(SemanticError::BufferTooSmall); }

    // Validate required fields.
    for (i, fd) in schema.fields.iter().enumerate() {
        if fd.required {
            let fv = fields.get(i).copied().unwrap_or(FieldValue::Absent);
            if fv == FieldValue::Absent { return Err(SemanticError::RequiredFieldMissing); }
        }
    }

    let mut pos = 0;
    out[pos..pos+7].copy_from_slice(MAGIC); pos += 7;
    out[pos..pos+2].copy_from_slice(&tag.to_le_bytes()); pos += 2;
    out[pos..pos+8].copy_from_slice(&created_tick.to_le_bytes()); pos += 8;

    for (i, _fd) in schema.fields.iter().enumerate() {
        let fv = fields.get(i).copied().unwrap_or(FieldValue::Absent);
        if fv == FieldValue::Absent {
            out[pos] = 0; pos += 1;
        } else {
            out[pos] = 1; pos += 1;
            match fv {
                FieldValue::U8(v)        => { out[pos] = v; pos += 1; }
                FieldValue::U16(v)       => { out[pos..pos+2].copy_from_slice(&v.to_le_bytes()); pos += 2; }
                FieldValue::U32(v)       => { out[pos..pos+4].copy_from_slice(&v.to_le_bytes()); pos += 4; }
                FieldValue::U64(v)       => { out[pos..pos+8].copy_from_slice(&v.to_le_bytes()); pos += 8; }
                FieldValue::Bytes16(b)   => { out[pos..pos+16].copy_from_slice(&b); pos += 16; }
                FieldValue::Bytes32(b)   => { out[pos..pos+32].copy_from_slice(&b); pos += 32; }
                FieldValue::Absent       => unreachable!(),
            }
        }
    }
    out[pos] = SENTINEL; pos += 1;
    Ok(pos)
}

/// Decode a v1 intent envelope.
///
/// Returns `Err` if magic is wrong, the tag is unknown, the buffer is
/// truncated, or the sentinel is missing / followed by trailing bytes.
pub fn decode(bytes: &[u8]) -> Result<DecodedIntent, SemanticError> {
    if bytes.len() < 7 { return Err(SemanticError::TruncatedMagic); }
    if &bytes[..7] != MAGIC { return Err(SemanticError::BadMagic); }
    if bytes.len() < 17 { return Err(SemanticError::TruncatedBody); }

    let tag          = u16::from_le_bytes([bytes[7], bytes[8]]);
    let created_tick = u64::from_le_bytes(bytes[9..17].try_into().unwrap());

    let entry = lookup_tag(tag).ok_or(SemanticError::UnknownTag)?;
    let schema = entry.schema;

    let mut pos = 17usize;
    let mut fields = [FieldValue::Absent; MAX_FIELDS];

    for (i, fd) in schema.fields.iter().enumerate() {
        if pos >= bytes.len() { return Err(SemanticError::TruncatedBody); }
        let present = bytes[pos]; pos += 1;
        if present > 1 { return Err(SemanticError::InvalidPresentByte); }
        if present == 0 {
            fields[i] = FieldValue::Absent;
            continue;
        }
        let wsz = fd.kind.wire_size();
        if pos + wsz > bytes.len() { return Err(SemanticError::TruncatedBody); }
        fields[i] = match fd.kind {
            FieldKind::U8      => { let v = bytes[pos]; pos += 1; FieldValue::U8(v) }
            FieldKind::U16     => { let v = u16::from_le_bytes([bytes[pos],bytes[pos+1]]); pos += 2; FieldValue::U16(v) }
            FieldKind::U32     => { let v = u32::from_le_bytes(bytes[pos..pos+4].try_into().unwrap()); pos += 4; FieldValue::U32(v) }
            FieldKind::U64     => { let v = u64::from_le_bytes(bytes[pos..pos+8].try_into().unwrap()); pos += 8; FieldValue::U64(v) }
            FieldKind::Bytes16 => { let mut b = [0u8;16]; b.copy_from_slice(&bytes[pos..pos+16]); pos += 16; FieldValue::Bytes16(b) }
            FieldKind::Bytes32 => { let mut b = [0u8;32]; b.copy_from_slice(&bytes[pos..pos+32]); pos += 32; FieldValue::Bytes32(b) }
        };
    }

    if pos >= bytes.len() { return Err(SemanticError::TruncatedBody); }
    if bytes[pos] != SENTINEL { return Err(SemanticError::MissingSentinel); }
    pos += 1;
    if pos != bytes.len() { return Err(SemanticError::TrailingBytes); }

    Ok(DecodedIntent {
        tag,
        created_tick,
        field_count: schema.fields.len() as u8,
        fields,
    })
}
