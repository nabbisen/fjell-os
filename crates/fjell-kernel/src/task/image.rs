//! Embedded service image table and flat-binary task loader.
//!
//! Each user-space service binary is compiled to a flat binary (ELF stripped
//! with `objcopy -O binary`), linked at `SERVICE_BASE_VA` (0x0004_0000), and
//! embedded here with `include_bytes!`.  When `sys_task_spawn` is called the
//! kernel copies the flat image into a fresh physical page, maps it at the
//! service VA, and returns a new `TaskId`.
//!
//! The stack page is placed immediately below the stack top defined in the
//! service linker script.

use fjell_abi::service::ImageId;

// ── Service image base VA (must match service linker scripts) ─────────────────
pub const SERVICE_BASE_VA:   usize = 0x0004_0000;
/// Stack top address as emitted by the service linker script:
///   __stack_bottom = ALIGN(4096) after .bss ≈ 0x41000
///   __stack_top    = __stack_bottom + 64K   = 0x51000
pub const SERVICE_STACK_TOP: usize = 0x0005_1000;

// ── Embedded flat binaries ────────────────────────────────────────────────────
// Flat binaries are pre-built from the service crates and committed to
// `crates/fjell-kernel/prebuilt/`.  They are automatically refreshed by
// `cargo xtask build-services` or by the kernel's `build.rs`.
//
// To rebuild manually:
//   cargo xtask build-services
// or:
//   RUSTC_BOOTSTRAP=1 cargo build --release \
//     --target riscv64gc-unknown-none-elf \
//     -Z build-std=core,compiler_builtins \
//     -p fjell-init -p fjell-configd -p fjell-cap-broker \
//     -p fjell-auditd -p fjell-service-manager -p fjell-sample-service
//   # then objcopy -O binary each ELF to prebuilt/<name>.bin

static INIT_BIN:    &[u8] = include_bytes!("../../prebuilt/fjell-init.bin");
static CONFIGD_BIN: &[u8] = include_bytes!("../../prebuilt/fjell-configd.bin");
static BROKER_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-cap-broker.bin");
static AUDITD_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-auditd.bin");
static SM_BIN:      &[u8] = include_bytes!("../../prebuilt/fjell-service-manager.bin");
static SAMPLE_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-sample-service.bin");

/// Resolve an `ImageId` to its raw flat-binary slice.
pub fn image_bytes(id: ImageId) -> Option<&'static [u8]> {
    match id {
        ImageId::INIT            => Some(INIT_BIN),
        ImageId::CONFIGD         => Some(CONFIGD_BIN),
        ImageId::CAP_BROKER      => Some(BROKER_BIN),
        ImageId::AUDITD          => Some(AUDITD_BIN),
        ImageId::SERVICE_MANAGER => Some(SM_BIN),
        ImageId::SAMPLE_SERVICE  => Some(SAMPLE_BIN),
        _                        => None,
    }
}
