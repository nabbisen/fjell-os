//! Intent Stream / State Stream / Event Stream schema types for Fjell OS.
//!
//! These types implement the ABDD (Accessible by Default and by Design)
//! principle: applications emit structured *intent* rather than pixel
//! coordinates.  A Presentation Proxy translates the stream into whatever
//! output modality the user requires.

#![no_std]

/// The severity of an `IntentNode`, used by the Presentation Proxy to
/// determine urgency and visual/audio weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Low,
    Normal,
    Important,
    Critical,
}

/// A unit of semantic output emitted by a service.
///
/// Full field set (description, actions, children, …) is added in M7.
#[derive(Clone, Copy, Debug)]
pub struct IntentNode {
    pub severity: Severity,
    // Additional fields (title, description, actions, required_caps) in M7.
}
