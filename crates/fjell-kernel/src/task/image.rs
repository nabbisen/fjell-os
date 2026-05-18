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
/// Stack top — FIXED in the service linker script at `0x80000 + 64K = 0x90000`.
/// The stack start address is pinned regardless of binary size so the kernel
/// always knows where to map the stack page.
pub const SERVICE_STACK_TOP: usize = 0x0009_0000;

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
// M5 services
static SEMANTIC_STREAM_BIN: &[u8] = include_bytes!("../../prebuilt/fjell-semantic-stream.bin");
static PROXY_TEXT_BIN:      &[u8] = include_bytes!("../../prebuilt/fjell-proxy-text.bin");

/// Resolve an `ImageId` to its raw flat-binary slice.
pub fn image_bytes(id: ImageId) -> Option<&'static [u8]> {
    match id {
        ImageId::INIT            => Some(INIT_BIN),
        ImageId::CONFIGD         => Some(CONFIGD_BIN),
        ImageId::CAP_BROKER      => Some(BROKER_BIN),
        ImageId::AUDITD          => Some(AUDITD_BIN),
        ImageId::SERVICE_MANAGER => Some(SM_BIN),
        ImageId::SAMPLE_SERVICE  => Some(SAMPLE_BIN),
        ImageId::SEMANTIC_STREAM => Some(SEMANTIC_STREAM_BIN),
        ImageId::PROXY_TEXT      => Some(PROXY_TEXT_BIN),
        ImageId::DEVMGR            => Some(DEVMGR_BIN),
        ImageId::DRIVER_VIRTIO_BLK => Some(VIRTIO_BLK_BIN),
        ImageId::STORAGED          => Some(STORAGED_BIN),
        ImageId::BOOTCTL           => Some(BOOTCTL_BIN),
        ImageId::UPGRADED          => Some(UPGRADED_BIN),
        ImageId::POWERD            => Some(POWERD_BIN),
        ImageId::VERIFYD   => Some(VERIFYD_BIN),
        ImageId::ROOTFSD   => Some(ROOTFSD_BIN),
        ImageId::SNAPSHOTD  => Some(SNAPSHOTD_BIN),
        ImageId::MEASUREDD  => Some(MEASUREDD_BIN),
        ImageId::ATTESTD    => Some(ATTESTD_BIN),
        ImageId::RECOVERYD  => Some(RECOVERYD_BIN),
        ImageId::NEG_TEST   => Some(NEG_TEST_BIN),
        ImageId::SVC_TIMEOUT => Some(SVC_TIMEOUT_BIN),
        ImageId::SVC_FAULT   => Some(SVC_FAULT_BIN),
        _                   => None,
    }
}

// M6 services
static DEVMGR_BIN:    &[u8] = include_bytes!("../../prebuilt/fjell-devmgr.bin");
static VIRTIO_BLK_BIN:&[u8] = include_bytes!("../../prebuilt/fjell-driver-virtio-blk.bin");
static STORAGED_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-storaged.bin");
static BOOTCTL_BIN:   &[u8] = include_bytes!("../../prebuilt/fjell-bootctl.bin");
static UPGRADED_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-upgraded.bin");
static POWERD_BIN:    &[u8] = include_bytes!("../../prebuilt/fjell-powerd.bin");

// M7 services
static VERIFYD_BIN:   &[u8] = include_bytes!("../../prebuilt/fjell-verifyd.bin");
static ROOTFSD_BIN:   &[u8] = include_bytes!("../../prebuilt/fjell-rootfsd.bin");
static SNAPSHOTD_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-snapshotd.bin");

// M8: Evidence / Attestation / Recovery
static MEASUREDD_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-measuredd.bin");
static ATTESTD_BIN:    &[u8] = include_bytes!("../../prebuilt/fjell-attestd.bin");
static RECOVERYD_BIN:  &[u8] = include_bytes!("../../prebuilt/fjell-recoveryd.bin");
// v0.2: negative-test + svc test services
static NEG_TEST_BIN:    &[u8] = include_bytes!("../../prebuilt/fjell-neg-test.bin");
static SVC_TIMEOUT_BIN: &[u8] = include_bytes!("../../prebuilt/fjell-svc-timeout.bin");
static SVC_FAULT_BIN:   &[u8] = include_bytes!("../../prebuilt/fjell-svc-fault.bin");
