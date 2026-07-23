//! Wire format and bundle builder for `diagnosticsd` (RFC v0.4-005).
//!
//! Provides:
//! - `DiagnosticBundle` — a fixed-shape, schema-versioned diagnostic blob.
//! - `DiagAuditEvent` / `DiagIntent` — redacted record types.
//! - `BundleBuilder` — accumulates events, enforces the allow-list, finalises
//!   with a SHA-256 canonical digest.
//! - Allow-listed audit-event and semantic-intent tag constants.
#![no_std]

pub mod bundle;
pub mod events;
pub mod intents;
pub mod builder;

pub use bundle::{
    DiagnosticBundle, DiagAuditEvent, DiagIntent,
    DIAG_BUNDLE_VERSION, MAX_AUDIT_EVENTS, MAX_SEMANTIC_INTENTS,
};
pub use events::is_audit_event_allowed;
pub use intents::is_intent_allowed;
pub use builder::BundleBuilder;

#[cfg(test)]
mod tests;
