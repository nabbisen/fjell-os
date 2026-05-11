//! Virtual memory region kind discriminant.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmRegionKind {
    UserText,
    UserRodata,
    UserData,
    UserStack,
    UserGuard,
    KernelShared,
    Mmio,
}
