//! Export format types for measurement log export.

/// Format for exporting measurement events.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ExportFormat {
    /// Compact binary (Fjell canonical).
    Binary   = 0x01,
    /// JSON Lines — one JSON object per event.
    JsonLines = 0x02,
    /// TOML — human-readable.
    Toml     = 0x03,
    /// Plain text — operator-readable summary.
    PlainText = 0x04,
}
