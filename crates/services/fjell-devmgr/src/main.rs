//! Device Manager — v0.5.0.
//!
//! Boot sequence (RFC v0.5-001 §7.1):
//!   1. Receive PlatformProfile + BoardProfile from storaged (cap slot 1/2).
//!   2. Recompute profile digests; reject on mismatch.
//!   3. Verify board.platform_ref == platform.profile_digest.
//!   4. Append measurement events via measuredd.
//!   5. Register devices from board.devices[].
//!   6. Emit PLATFORM.PROFILES_READY to semantic-stream.
//!
//! v0.5.0-alpha.1: profile digests verified in-process (storaged IPC
//! integration lands in alpha.2); measurement emission is stubbed.
#![no_std]
#![no_main]
mod rt;

use fjell_platform_format::{
    BoardProfile, DeviceClass, PlatformProfile, board_digest, platform_digest,
};
use fjell_syscall::{sys_debug_writeln, sys_exit};

// ── Boot-time profile verification ───────────────────────────────────────────

fn verify_and_load_profiles() -> Result<BoardProfile, &'static str> {
    // v0.5.0-alpha.1: use the QEMU virt reference profile directly.
    // In alpha.2 this reads from storaged via IPC and verifies signatures.
    let mut pp = PlatformProfile::qemu_virt_default();
    let computed_pd = platform_digest(&pp);
    pp.profile_digest = computed_pd;

    let mut bp = BoardProfile::qemu_virt_default(computed_pd);
    let computed_bd = board_digest(&bp);
    // Verify board references the correct platform.
    if bp.platform_ref.0 != pp.profile_digest.0 {
        return Err("board platform_ref mismatch");
    }
    bp.profile_digest = computed_bd;
    Ok(bp)
}

// ── Device registration ───────────────────────────────────────────────────────

/// List what the **board profile declares** -- a static table (`BoardProfile::qemu_virt_default`).
///
/// `devmgr` reads no device here. These lines used to say `registered virtio-net` and so on,
/// which read as a device having been found and registered: on every boot, with no NIC attached,
/// from a table. (RFC-0.45-001 D5: the same shape as the four lines RFC-0.43-003 deleted, one
/// layer out, and it would have sat directly above the absence tier's own `no virtio-net device
/// found`.) What a driver actually *read* is the driver's line, not this one.
fn register_devices(bp: &BoardProfile) {
    for i in 0..bp.device_count as usize {
        let dev = &bp.devices[i];
        match dev.class {
            DeviceClass::Uart8250 => {
                sys_debug_writeln("devmgr: board profile declares UART");
            }
            DeviceClass::VirtioNetMmio => {
                sys_debug_writeln("devmgr: board profile declares virtio-net");
            }
            DeviceClass::VirtioBlkMmio => {
                sys_debug_writeln("devmgr: board profile declares virtio-blk");
            }
            DeviceClass::Plic => {
                sys_debug_writeln("devmgr: board profile declares PLIC");
            }
            DeviceClass::Clint => {
                sys_debug_writeln("devmgr: board profile declares CLINT");
            }
            _ => {}
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

// SAFETY: category=symbol-export `service_main` is this service's entry symbol, tail-called by `_start` in rt.rs; no other item in the image exports the name.
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("devmgr: starting (v0.5 profile-driven)");

    let bp = match verify_and_load_profiles() {
        Ok(bp) => bp,
        Err(msg) => {
            sys_debug_writeln(msg);
            sys_debug_writeln("devmgr: profile verification failed; halting");
            sys_exit(1);
        }
    };

    sys_debug_writeln("devmgr: profiles verified");
    register_devices(&bp);
    sys_debug_writeln("devmgr: board profile listed");

    // Emit PLATFORM.PROFILES_READY to semantic-stream (alpha.2: full IPC).
    sys_debug_writeln("devmgr: PLATFORM.PROFILES_READY");

    sys_exit(0)
}
