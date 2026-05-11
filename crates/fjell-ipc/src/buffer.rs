//! IPC message tag and per-task IPC buffer.

/// Maximum number of 64-bit words in one IPC message payload.
pub const IPC_WORDS: usize = 8;
/// Maximum number of capability handles transferred in one message.
pub const IPC_CAPS: usize = 1;

/// Fixed-size IPC message header.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct MessageTag {
    /// Application-defined label (e.g. operation selector).
    pub label:    u16,
    /// Number of payload words (≤ `IPC_WORDS`).
    pub words:    u8,
    /// Number of capability handles transferred (0 or 1 in M3).
    pub caps:     u8,
    pub flags:    u16,
    pub reserved: u16,
}

impl MessageTag {
    /// Validate that `words` and `caps` are within the allowed limits.
    pub fn is_valid(self) -> bool {
        (self.words as usize) <= IPC_WORDS && (self.caps as usize) <= IPC_CAPS
    }
}

/// Per-task IPC buffer.
///
/// Mapped into the user address space at a fixed VA.  In M3 the kernel reads
/// and writes this buffer directly (no zero-copy yet).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct IpcBuffer {
    pub tag:          MessageTag,
    /// Badge of the sender endpoint capability (filled by the kernel on recv).
    pub sender_badge: u64,
    pub words:        [u64; IPC_WORDS],
    /// Capability handles transferred.
    pub caps:         [u32; IPC_CAPS],
    /// Slot index in the receiver's CSpace where an incoming cap is installed.
    pub recv_slot:    u16,
    pub reserved:     [u16; 3],
}

impl IpcBuffer {
    pub const fn zeroed() -> Self {
        IpcBuffer {
            tag:          MessageTag { label: 0, words: 0, caps: 0, flags: 0, reserved: 0 },
            sender_badge: 0,
            words:        [0; IPC_WORDS],
            caps:         [u32::MAX; IPC_CAPS],
            recv_slot:    0,
            reserved:     [0; 3],
        }
    }
}
