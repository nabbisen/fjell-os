//! Frozen v1 semantic intent catalog and codec (RFC v0.5-004).
//!
//! The catalog is a `const` table locked at v0.5.0. New entries may be
//! appended in v1.x patch releases provided they are additive; existing
//! tags cannot be repurposed. v2 introduces a separate catalog file.
//!
//! # Catalog layout
//! - Update domain:           `0x0100..=0x011F`
//! - Attestation domain:      `0x0120..=0x012F`
//! - Security-boundary domain:`0x0130..=0x013F`
//! - Net domain:              `0x0140..=0x014F`
//! - Recovery domain:         `0x0150..=0x015F`
//! - Platform domain:         `0x0160..=0x016F`
//! - Health domain:           `0x0170..=0x017F`
//! - FLEET domain (reserved): `0x0200..=0x02FF`
//! - SDK domain (reserved):   `0x0300..=0x03FF`
#![no_std]

pub mod catalog;
pub mod schema;
pub mod codec;
pub mod version;

pub use catalog::{CATALOG_V1, IntentEntry, lookup_tag, catalog_len};
pub use schema::{IntentSchema, FieldKind, FieldDef};
pub use codec::{encode, decode, DecodedIntent, FieldValue, SemanticError};
pub use version::{CatalogVersion, CATALOG_V1_VERSION};

#[cfg(test)]
mod tests;
