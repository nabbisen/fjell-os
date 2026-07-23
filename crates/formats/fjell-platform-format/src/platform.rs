//! `PlatformProfile` — architectural family descriptor (RFC v0.5-001 §6.1).

use fjell_measure_format::Digest32;

pub const PLATFORM_PROFILE_VERSION: u16 = 1;

// ── ISA extension bit constants ───────────────────────────────────────────────

/// `I` extension (mandatory — base integer).
pub const ISA_EXT_I:         u64 = 1 << 0;
/// `M` extension (mandatory — multiply/divide).
pub const ISA_EXT_M:         u64 = 1 << 1;
/// `A` extension (mandatory — atomics).
pub const ISA_EXT_A:         u64 = 1 << 2;
/// `F` extension — single-precision FP.
pub const ISA_EXT_F:         u64 = 1 << 3;
/// `D` extension — double-precision FP.
pub const ISA_EXT_D:         u64 = 1 << 4;
/// `C` extension — compressed instructions.
pub const ISA_EXT_C:         u64 = 1 << 5;
/// `Zbb` extension — basic bit-manipulation.
pub const ISA_EXT_ZBB:       u64 = 1 << 6;
/// `Zicsr` extension — control and status registers.
pub const ISA_EXT_ZICSR:     u64 = 1 << 7;
/// `Zifencei` extension — instruction-fetch fence.
pub const ISA_EXT_ZIFENCEI:  u64 = 1 << 8;

/// Mandatory ISA extensions for `Riscv64Gc`.
pub const ISA_MANDATORY: u64 = ISA_EXT_I | ISA_EXT_M | ISA_EXT_A;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Architectural family identifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PlatformFamily {
    Riscv64Gc = 0x01,
    Arm64     = 0x02,   // reserved for v0.5 second-platform planning
}

/// Bit-set of active ISA extensions.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct IsaExtensions(pub u64);

impl IsaExtensions {
    pub fn contains(self, bit: u64) -> bool { (self.0 & bit) != 0 }
    pub fn with(self, bit: u64)    -> Self  { Self(self.0 | bit) }
    /// Return `true` if all mandatory bits for `Riscv64Gc` are set.
    pub fn is_riscv64gc_compliant(self) -> bool {
        (self.0 & ISA_MANDATORY) == ISA_MANDATORY
    }
}

/// Kernel ABI version that the profile expects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KernelAbiVersion {
    pub major: u8,
    pub minor: u8,
}

impl KernelAbiVersion {
    /// ABI version for v0.4 + v0.5 (major=0, minor=5).
    pub const V0_5: Self = Self { major: 0, minor: 5 };
}

/// Physical memory-map parameters for kernel and initrd placement.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MemMap {
    pub kernel_load_addr: u64,
    pub kernel_size_max:  u64,
    pub heap_start:       u64,
    pub heap_size:        u64,
    pub initrd_addr:      u64,  // 0 if no initrd
    pub initrd_size:      u64,
}

/// PLIC (Platform-Level Interrupt Controller) memory layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PlicLayout {
    pub base_addr:    u64,
    pub size_bytes:   u64,
    pub num_sources:  u16,
    pub num_contexts: u16,
}

/// Top-level platform descriptor.
///
/// `profile_digest` is computed over the canonical prefix by
/// `crate::digest::platform_digest`; callers must verify it on load.
#[derive(Clone, Copy, Debug)]
pub struct PlatformProfile {
    pub schema_version:  u16,
    pub family:          PlatformFamily,
    pub family_version:  u16,
    pub isa_extensions:  IsaExtensions,
    pub kernel_abi:      KernelAbiVersion,
    pub mem_map:         MemMap,
    pub plic_layout:     PlicLayout,
    /// SHA-256 over the canonical serialisation (set by the release tool).
    pub profile_digest:  Digest32,
}

impl PlatformProfile {
    /// A sensible default matching the QEMU `virt` machine used in CI
    /// (RFC v0.5-001 §7.2 — `qemu-virt-v0.5` reference profile).
    ///
    /// The `profile_digest` field is zeroed; call `platform_digest` and
    /// write the result back before storing or measuring.
    pub fn qemu_virt_default() -> Self {
        Self {
            schema_version:  PLATFORM_PROFILE_VERSION,
            family:          PlatformFamily::Riscv64Gc,
            family_version:  1,
            isa_extensions:  IsaExtensions(
                ISA_EXT_I | ISA_EXT_M | ISA_EXT_A
                | ISA_EXT_F | ISA_EXT_D | ISA_EXT_C
                | ISA_EXT_ZICSR | ISA_EXT_ZIFENCEI,
            ),
            kernel_abi: KernelAbiVersion::V0_5,
            mem_map: MemMap {
                kernel_load_addr: 0x8000_0000,
                kernel_size_max:  0x0100_0000,  // 16 MiB
                heap_start:       0x8100_0000,
                heap_size:        0x0400_0000,  // 64 MiB
                initrd_addr:      0,
                initrd_size:      0,
            },
            plic_layout: PlicLayout {
                base_addr:    0x0C00_0000,
                size_bytes:   0x0400_0000,  // 64 MiB PLIC on QEMU virt
                num_sources:  96,
                num_contexts: 2,
            },
            profile_digest: Digest32([0u8; 32]),
        }
    }
}
