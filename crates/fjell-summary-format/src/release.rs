//! `ReleaseSummary` wire type (RFC v0.7-003 §6.1).

use fjell_measure_format::Digest32;

pub const RSUMMARY_SCHEMA_VERSION: u16 = 1;
pub const MAX_CHANNEL_SUMMARIES: usize = 8;

/// How the current counter was last advanced.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum AdvanceSource {
    #[default]
    Unknown       = 0,
    LocalInstall  = 1,
    SnapshotSync  = 2,
    ManualPinned  = 3,
}

impl AdvanceSource {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Unknown),
            1 => Some(Self::LocalInstall),
            2 => Some(Self::SnapshotSync),
            3 => Some(Self::ManualPinned),
            _ => None,
        }
    }
}

/// Per-channel upgrade state exported in a `ReleaseSummary`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ChannelSummary {
    pub channel_id:           [u8; 8],
    pub current_counter:      u64,
    pub min_counter:          u64,
    pub active_anchor_epoch:  u32,
    pub last_confirm_tick:    u64,
    pub last_advance_source:  AdvanceSource,
}

/// Exported per-channel upgrade counters from `upgraded`.
#[derive(Clone, Copy, Debug)]
pub struct ReleaseSummary {
    pub schema_version:  u16,
    pub source_node_id:  [u8; 16],
    pub issued_tick:     u64,
    pub channel_count:   u8,
    pub channels:        [ChannelSummary; MAX_CHANNEL_SUMMARIES],
    /// Canonical digest; compute with `release_summary_digest` before storing.
    pub summary_digest:  Digest32,
}

impl ReleaseSummary {
    pub fn new(source_node_id: [u8; 16], issued_tick: u64) -> Self {
        Self {
            schema_version:  RSUMMARY_SCHEMA_VERSION,
            source_node_id,
            issued_tick,
            channel_count:   0,
            channels:        [ChannelSummary::default(); MAX_CHANNEL_SUMMARIES],
            summary_digest:  Digest32([0u8; 32]),
        }
    }

    /// Add a per-channel summary.
    ///
    /// Returns `Err(SummaryError::DuplicateChannel)` if the `channel_id` is
    /// already present, or `Err(SummaryError::CapacityExhausted)` when full
    /// (RFC-v0.7.5-001, closes C-M-04).
    pub fn add_channel(&mut self, ch: ChannelSummary) -> Result<(), crate::measurement::SummaryError> {
        use crate::measurement::SummaryError;
        if self.channels[..self.channel_count as usize]
            .iter().any(|e| e.channel_id == ch.channel_id)
        {
            return Err(SummaryError::DuplicateChannel);
        }
        if self.channel_count as usize >= MAX_CHANNEL_SUMMARIES {
            return Err(SummaryError::CapacityExhausted);
        }
        self.channels[self.channel_count as usize] = ch;
        self.channel_count += 1;
        Ok(())
    }
}
