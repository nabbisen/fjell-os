//! Device Tree Blob (DTB) parser stub.
//!
//! Full DTB parsing (reading `/memory` reg, MMIO ranges, interrupt routing)
//! is deferred to a future milestone.  For M2 the DTB pointer is forwarded
//! through `PlatformInfo` but not interpreted.
