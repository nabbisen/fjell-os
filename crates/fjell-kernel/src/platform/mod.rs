//! Platform-specific constants and memory-range discovery.
//!
//! For M2, physical memory ranges are obtained from hardcoded QEMU `virt`
//! constants.  The DTB pointer from the M-mode shim is forwarded but not
//! yet parsed (full DTB parsing is deferred to a future milestone).

pub mod dtb;
pub mod qemu_virt;

pub use qemu_virt::PlatformInfo;

/// Discover the platform memory layout.
///
/// In M2 this always returns the hardcoded QEMU `virt` layout.
/// The `dtb_pa` argument is stored for future use.
pub fn detect(dtb_pa: usize) -> PlatformInfo {
    let mut info = qemu_virt::qemu_virt_platform();
    info.dtb_pa = dtb_pa;
    info
}
