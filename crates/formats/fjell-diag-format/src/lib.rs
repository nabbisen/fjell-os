//! Wire format and bundle builder for `diagnosticsd` (RFC v0.4-005).
//!
//! Provides:
//! - `DiagnosticBundle` — a fixed-shape, schema-versioned diagnostic blob.
//! - `DiagAuditEvent` / `DiagIntent` — redacted record types.
//! - `BundleBuilder` — accumulates events, enforces the allow-list, finalises
//!   with a SHA-256 canonical digest.
//! - Allow-listed audit-event and semantic-intent tag constants.
#![no_std]

pub mod builder;
pub mod bundle;
pub mod events;
pub mod intents;

pub use builder::BundleBuilder;
pub use bundle::{
    DIAG_BUNDLE_VERSION, DiagAuditEvent, DiagIntent, DiagnosticBundle, MAX_AUDIT_EVENTS,
    MAX_SEMANTIC_INTENTS,
};
pub use events::is_audit_event_allowed;
pub use intents::is_intent_allowed;

#[cfg(test)]
mod tests;
