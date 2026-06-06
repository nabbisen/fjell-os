//! Catalog version negotiation types (RFC v0.5-004 §6.3).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CatalogVersion {
    pub major: u8,
    pub minor: u8,
}

impl CatalogVersion {
    pub const V1_0: Self = Self { major: 1, minor: 0 };
}

/// The catalog version shipped with this crate.
pub const CATALOG_V1_VERSION: CatalogVersion = CatalogVersion::V1_0;
