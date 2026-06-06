//! Measurement and release summary wire formats (RFC v0.7-003).
//!
//! Both summaries are signed via `attestd` and stored in the append-only log.
//! They propagate across nodes via the snapshot-sync channel.
#![no_std]

pub mod measurement;
pub mod release;
pub mod digest;

pub use measurement::{
    MeasurementSummary, MeasurementKindCount,
    MSUMMARY_SCHEMA_VERSION, MAX_KIND_COUNTS,
};
pub use release::{
    ReleaseSummary, ChannelSummary, AdvanceSource,
    RSUMMARY_SCHEMA_VERSION, MAX_CHANNEL_SUMMARIES,
};
pub use digest::{measurement_summary_digest, release_summary_digest};

#[cfg(test)]
mod tests;
