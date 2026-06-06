//! Intent schema field definitions (RFC v0.5-004 §5.2).

/// Primitive field kind used in the intent schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum FieldKind {
    /// Unsigned 8-bit integer.
    U8  = 0x01,
    /// Unsigned 16-bit integer, LE.
    U16 = 0x02,
    /// Unsigned 32-bit integer, LE.
    U32 = 0x03,
    /// Unsigned 64-bit integer, LE.
    U64 = 0x04,
    /// Fixed 16-byte blob (e.g. channel_id or server name prefix).
    Bytes16 = 0x10,
    /// Fixed 32-byte digest.
    Bytes32 = 0x20,
}

impl FieldKind {
    /// Wire size in bytes.
    pub const fn wire_size(self) -> usize {
        match self {
            FieldKind::U8      => 1,
            FieldKind::U16     => 2,
            FieldKind::U32     => 4,
            FieldKind::U64     => 8,
            FieldKind::Bytes16 => 16,
            FieldKind::Bytes32 => 32,
        }
    }
}

/// Single field definition in an intent schema.
#[derive(Clone, Copy, Debug)]
pub struct FieldDef {
    pub name:     &'static str,
    pub kind:     FieldKind,
    pub required: bool,
}

/// Schema for a single intent tag.
#[derive(Clone, Copy, Debug)]
pub struct IntentSchema {
    pub fields:      &'static [FieldDef],
}
