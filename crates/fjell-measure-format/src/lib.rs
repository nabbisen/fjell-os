//! Measurement chain format for Fjell OS M8.
//!
//! Defines the append-only measurement event chain that records what the OS
//! loaded, verified, and reached.  The chain head digest is a deterministic
//! summary of all events in order.
//!
//! Chain digest formula:
//! ```text
//! chain_digest[n] = SHA256(
//!     "FJELL-MEASUREMENT-V1" ||
//!     schema_version(u16-LE)  ||
//!     seq(u64-LE)             ||
//!     kind(u8)                ||
//!     source(u8)              ||
//!     subject(u8)             ||
//!     subject_digest[32]      ||
//!     metadata_present(u8)    ||
//!     metadata_digest[32]     ||  // zeros if absent
//!     previous_chain_digest[32]
//! )
//! ```
#![no_std]

pub mod digest;
pub mod event;
pub mod export;

pub use digest::Digest32;
pub use event::{
    MeasurementEvent, MeasurementHead, MeasurementKind, MeasurementSource,
    MeasurementSubject, MeasurementError,
};
pub use export::ExportFormat;

/// Schema version embedded in every chain digest computation.
pub const SCHEMA_VERSION: u16 = 1;

/// Domain separator for chain digest inputs.
pub const CHAIN_DOMAIN: &[u8] = b"FJELL-MEASUREMENT-V1";

/// Maximum export events per chunk.
pub const MAX_EXPORT_CHUNK: usize = 8;
