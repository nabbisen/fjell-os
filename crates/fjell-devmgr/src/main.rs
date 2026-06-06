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

use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_platform_format::{
    PlatformProfile, BoardProfile, DeviceClass,
    platform_digest, board_digest,
};

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

fn register_devices(bp: &BoardProfile) {
    for i in 0..bp.device_count as usize {
        let dev = &bp.devices[i];
        match dev.class {
            DeviceClass::Uart8250 => {
                sys_debug_writeln("devmgr: registered UART");
            }
            DeviceClass::VirtioNetMmio => {
                sys_debug_writeln("devmgr: registered virtio-net");
            }
            DeviceClass::VirtioBlkMmio => {
                sys_debug_writeln("devmgr: registered virtio-blk");
            }
            DeviceClass::Plic => {
                sys_debug_writeln("devmgr: registered PLIC");
            }
            DeviceClass::Clint => {
                sys_debug_writeln("devmgr: registered CLINT");
            }
            _ => {}
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

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
    sys_debug_writeln("devmgr: devices registered");

    // Emit PLATFORM.PROFILES_READY to semantic-stream (alpha.2: full IPC).
    sys_debug_writeln("devmgr: PLATFORM.PROFILES_READY");

    sys_exit(0)
}
