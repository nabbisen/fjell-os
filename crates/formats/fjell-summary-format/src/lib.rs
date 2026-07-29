//! Measurement and release summary wire formats (RFC v0.7-003).
//!
//! Both summaries are signed via `attestd` and stored in the append-only log.
//! They propagate across nodes via the snapshot-sync channel.
#![no_std]

pub mod digest;
pub mod measurement;
pub mod release;

pub use digest::{measurement_summary_digest, release_summary_digest};
pub use measurement::{
    MAX_KIND_COUNTS, MSUMMARY_SCHEMA_VERSION, MeasurementKindCount, MeasurementSummary,
};
pub use release::{
    AdvanceSource, ChannelSummary, MAX_CHANNEL_SUMMARIES, RSUMMARY_SCHEMA_VERSION, ReleaseSummary,
};

#[cfg(test)]
mod tests;

pub use measurement::SummaryError;
