//! IPC message tag and buffer layout constants.

/// Maximum number of data words in one IPC message.
pub const IPC_WORDS: usize = 8;

/// Maximum number of capabilities transferred in one IPC message.
pub const IPC_CAPS: usize = 1;

/// Compact message header passed in registers and the IPC buffer.
///
/// `label`  — application-defined discriminant (e.g. method selector)
/// `words`  — number of data words in the message (0..=IPC_WORDS)
/// `caps`   — number of capabilities transferred (0..=IPC_CAPS)
/// `flags`  — reserved for future use (must be 0 in M3)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct MessageTag {
    pub label: u16,
    pub words: u8,
    pub caps:  u8,
    pub flags: u16,
    pub _pad:  u16,
}

impl MessageTag {
    pub const fn new(label: u16, words: u8, caps: u8) -> Self {
        MessageTag { label, words, caps, flags: 0, _pad: 0 }
    }

    /// Is this a valid tag? (word/cap counts within limits)
    pub fn is_valid(self) -> bool {
        self.words as usize <= IPC_WORDS && self.caps as usize <= IPC_CAPS
    }
}
