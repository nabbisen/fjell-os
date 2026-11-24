//! driver-virtio-net -- finds the virtio network device and **reports only what it read**
//! (RFC-0.45-001). It does not drive the device.
//!
//! This image is spawned by `init` on every boot. It scans the eight virtio-mmio transports
//! through its one `MmioRegion` capability (region 3, `MMIO_MAP` only -- no interrupt, no DMA, no
//! endpoint), reads each transport's `MagicValue` and `DeviceID`, prints **one** line, and exits 0:
//!
//! * `driver-virtio-net: found virtio-mmio device id=1 at slot <N> magic=0x74726976` -- a transport
//!   whose magic is `virtio` and whose `DeviceID` is 1 (a network device);
//! * `driver-virtio-net: no virtio-net device found (scanned 8 transports)` -- **only** after all
//!   eight were mapped and read and none was a network device;
//! * `driver-virtio-net: cannot scan the virtio-mmio region (read <k> of 8 transports, last error
//!   code <e>)` -- the scan could not run, or could not finish. **Never "no device found"**: absence
//!   is only reportable after a successful scan, and printing it when the grant was missing would
//!   be E-118 from the other side -- a true-sounding line from a path that never looked.
//!
//! Each line is something this image read from a register or was refused. It negotiates no
//! features, sets up no queue, moves no frame and claims no `Ready`.
//!
//! **History.** Until RFC-0.43-003 this file claimed six capabilities, a header said `devmgr`
//! installed them, and the console carried *"device descriptor loaded"*, *"features negotiated"*
//! and *"rx queue initialised"* -- none of it true (`sys_mmio_map` was never called). It was made
//! an honest stub. This is the first line that may print a device fact, and it earns each one.
//!
//! **The over-grant, named.** Region 3 covers all eight transports, including the block device
//! `storaged` drives, and `sys_mmio_map` maps the page read-write: the kernel cannot enforce "reads
//! only". What bounds it is this file -- two `read_volatile`s, each annotated for `mmio-audit` --
//! and that `storaged` and `neg-test` hold the same reach. The per-transport region that would end
//! it is its own line (RFC-0.45-001 §"The over-grant").
//!
//! The host-testable virtqueue, ring, state-machine and feature-negotiation code in `src/core/`
//! (the `fjell_driver_virtio_net` library) is untouched and is **not linked into this image**.
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_driver_virtio_net::scan::{Observation, Report, Scan};
use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_exit, sys_mmio_map};

/// The `MmioRegion` capability for kernel region 3 (the virtio-mmio range): slot `31 + 3`. Declared
/// as a `CapHandle` constant so `capability-topology` judges the declaration against `spawn.rs`'s
/// install -- a driver that names a slot nothing installs is exactly what E-072 was.
const CAP_MMIO_VIRTIO: CapHandle = CapHandle(34);

/// virtio-mmio transports in the region, 0x1000 bytes each.
const TRANSPORTS: usize = 8;
const TRANSPORT_SIZE: usize = 0x1000;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("driver-virtio-net: panic");
    sys_exit(1);
}

/// Read one 32-bit register of a mapped transport.
#[inline(always)]
// SAFETY: category=raw-pointer-deref `base` is the address `sys_mmio_map` returned for a 0x1000-byte window of this task's own `MmioRegion`; offsets 0x000 and 0x008 are inside it.
unsafe fn rd32(base: usize, off: usize) -> u32 {
    // SAFETY: category=raw-pointer-deref see the function's own SAFETY note above.
    // MMIO-ORDER: status_read
    unsafe { core::ptr::read_volatile((base + off) as *const u32) }
}

/// Print `n` in decimal (no allocator, no `fmt`).
fn print_dec(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    // The bytes are ASCII digits, so this is always `Ok`.
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
}

/// Print a 32-bit value as `0x` + eight lowercase hex digits.
fn print_hex32(v: u32) {
    sys_debug_write("0x");
    let mut buf = [0u8; 8];
    for (i, b) in buf.iter_mut().enumerate() {
        let nib = ((v >> (28 - 4 * i)) & 0xF) as u8;
        *b = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + nib - 10
        };
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        sys_debug_write(s);
    }
}

// SAFETY: category=symbol-export `service_main` is this service's entry symbol, tail-called by `_start` in rt.rs; no other item in the image exports the name.
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // What the scan may *say* is `fjell_driver_virtio_net::scan` (pure, host-tested and mutated
    // there): found only on magic AND id; absence only after all eight transports were read;
    // otherwise cannot-scan. This function only maps, reads and prints what that decides.
    let mut scan = Scan::new(TRANSPORTS);

    for slot in 0..TRANSPORTS {
        let obs = match sys_mmio_map(CAP_MMIO_VIRTIO, slot * TRANSPORT_SIZE, TRANSPORT_SIZE) {
            Ok(va) if va != 0 => {
                // SAFETY: category=raw-pointer-deref `va` is the window `sys_mmio_map` just returned for this transport.
                let magic = unsafe { rd32(va, 0x000) };
                // SAFETY: category=raw-pointer-deref same window, `DeviceID` register.
                let device_id = unsafe { rd32(va, 0x008) };
                Observation::Read { magic, device_id }
            }
            Ok(_) => Observation::Refused(0),
            Err(e) => Observation::Refused(e as isize),
        };
        if let Some(report) = scan.observe(slot, obs) {
            print_report(report);
            sys_exit(0);
        }
    }
    print_report(scan.finish());
    sys_exit(0)
}

/// Print the one line `report` allows.
fn print_report(report: Report) {
    match report {
        Report::Found { slot, magic } => {
            sys_debug_write("driver-virtio-net: found virtio-mmio device id=1 at slot ");
            print_dec(slot as u64);
            sys_debug_write(" magic=");
            print_hex32(magic);
            sys_debug_writeln("");
        }
        Report::NoDevice { scanned } => {
            sys_debug_write("driver-virtio-net: no virtio-net device found (scanned ");
            print_dec(scanned as u64);
            sys_debug_writeln(" transports)");
        }
        Report::CannotScan { read, of, last_err } => {
            sys_debug_write("driver-virtio-net: cannot scan the virtio-mmio region (read ");
            print_dec(read as u64);
            sys_debug_write(" of ");
            print_dec(of as u64);
            sys_debug_write(" transports, last error code ");
            if last_err < 0 {
                sys_debug_write("-");
            }
            print_dec(last_err.unsigned_abs() as u64);
            sys_debug_writeln(")");
        }
    }
}
