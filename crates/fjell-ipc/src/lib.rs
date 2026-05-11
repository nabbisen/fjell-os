//! IPC state machine for Fjell OS — pure logic, host-testable.
//!
//! Implements synchronous rendezvous endpoints (L4/seL4 style).
//! No arch dependencies; the full endpoint state machine can be
//! property-tested on the host.
//!
//! # Modules
//! - [`message`]  — `MessageTag`, `IpcBuffer` layout constants
//! - [`endpoint`] — `Endpoint`, `PendingMessage`, send/recv/call/reply
//! - [`reply`]    — One-shot `ReplyEdge`

#![no_std]
#![allow(dead_code)]

pub mod endpoint;
pub mod message;
pub mod reply;

pub use endpoint::{Endpoint, EndpointError, IPC_CAPS, IPC_WORDS};
pub use message::MessageTag;
pub use reply::ReplyEdge;
