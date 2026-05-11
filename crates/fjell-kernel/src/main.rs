//! Fjell OS Kernel — v0.0.2 / M2: Memory and Task Isolation
//!
//! Boot flow:
//!   _start  (boot.rs, M-mode assembly)
//!   └─ m_mode_setup  (M-mode Rust)
//!      └─ mret → s_mode_entry  (S-mode Rust)
//!         └─ kmain  → scheduler bootstrap → first_entry (sret, noreturn)
//!                                        → trap_dispatch (all future scheduling)

#![no_std]
#![no_main]

mod arch;
mod audit;
mod boot;
mod cap;
mod console;
mod mm;
mod platform;
mod task;
mod trap;
mod uart;

use core::{cell::UnsafeCell, mem::MaybeUninit, panic::PanicInfo};

#[cfg(target_arch = "riscv64")]
use arch::riscv64::{csr, satp};
use audit::ring::{AuditKindInternal, AUDIT};
use mm::{
    address::{PhysFrame, VirtAddr},
    boot_alloc::BootAllocator,
    frame_alloc::{FrameAllocator, FrameOwner, FRAME_SIZE},
    region::VmRegionKind,
    vspace::{AddressSpace, AddressSpaceId, VmPerms},
};
use platform::qemu_virt::{MMIO_REGIONS, RAM_BASE, RAM_END};
use task::{
    scheduler::{Scheduler, PRIORITY_IDLE, PRIORITY_USER},
    tcb::{Task, TaskState, TaskTable},
    user_image::{USER_ENTRY_VA, USER_STACK_TOP, USER_TEXT_VA, USER_TASK_A, USER_TASK_B},
    TaskId,
};
use trap::entry::init_trap;

// ── Linker symbols ────────────────────────────────────────────────────────────

unsafe extern "C" {
    static __bss_end:   u8;
    static __stack_top: u8;
}

fn kernel_end_pa() -> usize {
    let bss_end = unsafe { &__bss_end as *const u8 as usize };
    (bss_end + 0xFFF) & !0xFFF
}

// ── Static kernel state ───────────────────────────────────────────────────────

pub(crate) struct KS<T>(UnsafeCell<T>);
// SAFETY: single-hart M2; no concurrent access.
unsafe impl<T> Sync for KS<T> {}

static FRAME_BITMAP: KS<[u64; 512]>            = KS(UnsafeCell::new([0u64; 512]));
static TASK_TABLE:   KS<MaybeUninit<TaskTable>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
static SCHEDULER:    KS<MaybeUninit<Scheduler>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
static CAP_TABLE:    KS<MaybeUninit<cap::table::CapTable>>      = KS(UnsafeCell::new(MaybeUninit::uninit()));
static EP_TABLE:     KS<MaybeUninit<cap::table::EndpointTable>> = KS(UnsafeCell::new(MaybeUninit::uninit()));
/// Per-hart trap scratch record. Layout: [0] = kernel sp, [1] = TrapFrame ptr.
/// Must be static — sscratch holds a pointer to it across sret/trap boundaries.
pub(crate) static TRAP_SCRATCH: KS<[usize; 2]> = KS(UnsafeCell::new([0usize; 2]));

macro_rules! ks_init {
    ($ks:expr, $val:expr) => { unsafe { (*$ks.0.get()).write($val) } };
}
macro_rules! ks_get {
    ($ks:expr) => { unsafe { (*$ks.0.get()).assume_init_mut() } };
}

/// Called by `trap/dispatch.rs` to access all mutable kernel state.
///
/// # Safety
/// All tables must have been initialised before any trap fires.
/// Single-hart M2/M3; no concurrent access.
pub unsafe fn get_kernel_state() -> (
    &'static mut task::tcb::TaskTable,
    &'static mut task::scheduler::Scheduler,
    &'static mut cap::table::CapTable,
    &'static mut cap::table::EndpointTable,
) {
    (
        ks_get!(TASK_TABLE),
        ks_get!(SCHEDULER),
        ks_get!(CAP_TABLE),
        ks_get!(EP_TABLE),
    )
}

// ── kprintln! — usable from trap/dispatch.rs ─────────────────────────────────

#[macro_export]
macro_rules! kprintln {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::println!($($arg)*));
}

// ── M-mode shim ───────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn m_mode_setup(hart_id: usize, dtb_pa: usize) -> ! {
    // SAFETY: M-mode CSR writes; called exactly once from boot assembly.
    unsafe {
        csr::write_medeleg(0xFFFF);
        csr::write_mideleg(0x0222);
        csr::write_mstatus(1usize << 11); // MPP = S
        csr::write_mepc(s_mode_entry as *const () as usize);
        core::arch::asm!(
            "mv a0, {hart}", "mv a1, {dtb}", "mret",
            hart = in(reg) hart_id, dtb = in(reg) dtb_pa,
            options(noreturn),
        );
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn m_mode_setup(_: usize, _: usize) -> ! { loop {} }

// ── S-mode entry ──────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn s_mode_entry(hart_id: usize, dtb_pa: usize) -> ! {
    // SAFETY: called once via mret; first use of console.
    unsafe { console::init() };
    println!("Fjell OS kernel started.");
    println!("mode: S");
    kmain(hart_id, dtb_pa)
}

#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn s_mode_entry(_: usize, _: usize) -> ! { loop {} }

/// Host-build stub: binary needs a no-arg kmain for `#[no_main]`.
#[cfg(not(target_arch = "riscv64"))]
#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! { loop {} }

// ── Kernel main ───────────────────────────────────────────────────────────────

#[cfg(target_arch = "riscv64")]
#[allow(unused_unsafe)] // fa!() macro contains unsafe that may nest under caller's unsafe blocks
fn kmain(_hart_id: usize, dtb_pa: usize) -> ! {
    println!("platform: qemu-virt");

    let platform = platform::detect(dtb_pa);
    println!("memory: detected ({} MiB)", platform.ram_size / (1024 * 1024));

    // BootAllocator (watermark used to calculate kernel_end_pa only).
    let boot_start = kernel_end_pa();
    let boot_end   = boot_start + 2 * 1024 * 1024;
    let _boot = BootAllocator::new(boot_start, boot_end);
    println!("mm: boot allocator ready");

    // FrameAllocator.
    let bitmap = unsafe { &mut *FRAME_BITMAP.0.get() };
    let fa_cell = UnsafeCell::new(FrameAllocator::new(
        (RAM_BASE >> 12) as u64,
        ((RAM_END - RAM_BASE) / FRAME_SIZE) as u64,
        bitmap, None,
    ));
    // SAFETY: single-hart M2; fa_cell is local to kmain; all accesses are
    // sequential and non-overlapping within this function.
    let fa_ptr = fa_cell.get();
    macro_rules! fa { () => {
        // SAFETY: fa_ptr is valid, aligned, and not aliased in this context.
        unsafe { &mut *fa_ptr }
    } }

    fa!().reserve_range(RAM_BASE, kernel_end_pa(), FrameOwner::KernelText)
         .expect("rsv kern");
    fa!().reserve_range(boot_start, boot_end, FrameOwner::ReservedBoot)
         .expect("rsv boot");
    if dtb_pa != 0 {
        fa!().reserve_range(dtb_pa, dtb_pa + 4096, FrameOwner::Dtb)
             .expect("rsv dtb");
    }
    for &(start, end, _) in MMIO_REGIONS {
        if start < RAM_BASE { let _ = fa!().reserve_range(start, end, FrameOwner::Mmio); }
    }
    println!("mm: frame allocator ready  ({} free frames)", fa!().free_count());

    // Kernel page table.
    let kernel_root = fa!().alloc_frame(FrameOwner::KernelPageTable)
                           .expect("kernel root PT");
    // SAFETY: freshly allocated 4-KiB frame.
    unsafe { core::ptr::write_bytes(kernel_root.pa() as *mut u8, 0, 4096) };

    // Identity-map kernel + boot scratch.
    let mut va = RAM_BASE;
    while va < boot_end {
        let f = PhysFrame::from_pa(va).unwrap();
        // SAFETY: kernel_root valid; sfence inside enable_sv39.
        unsafe {
            mm::page_table::map_page(kernel_root.pa(), VirtAddr(va), f,
                VmPerms::R | VmPerms::W | VmPerms::X, fa!())
                .expect("kernel map");
        }
        va += 4096;
    }
    // UART identity-map.
    let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
    // SAFETY: same.
    unsafe {
        mm::page_table::map_page(kernel_root.pa(), VirtAddr(0x1000_0000), uart_f,
            VmPerms::R | VmPerms::W, fa!())
            .expect("uart map");
    }

    // Enable Sv39.
    // SAFETY: all required kernel mappings present; sfence inside.
    unsafe { satp::enable_sv39(kernel_root.pfn as usize) };
    println!("vm: sv39 enabled");
    AUDIT.lock_free_append(AuditKindInternal::Boot, 0, 0, 0);

    // Install trap vector.
    // SAFETY: called once before any user-mode entry or interrupt enable.
    unsafe { init_trap() };
    println!("trap: stvec installed");

    // Initialise task table and scheduler.
    ks_init!(TASK_TABLE, TaskTable::new());
    ks_init!(SCHEDULER,  Scheduler::new());
    ks_init!(CAP_TABLE,  cap::table::CapTable::new());
    ks_init!(EP_TABLE,   cap::table::EndpointTable::new());
    let table  = ks_get!(TASK_TABLE);
    let sched  = ks_get!(SCHEDULER);
    let ct     = ks_get!(CAP_TABLE);
    let et     = ks_get!(EP_TABLE);

    // Allocate a shared IPC endpoint for M3 smoke test.
    // user0 gets SEND|CALL rights (client), user1 gets RECV rights (server).
    let ep_obj_id = et.alloc().expect("alloc endpoint");
    println!("ipc: endpoint {} created", ep_obj_id);

    // Idle task — no capabilities needed.
    let idle_ksp = unsafe { &__stack_top as *const u8 as usize };
    let mut idle = Task::new(TaskId::new(0, 0), PRIORITY_IDLE,
                             AddressSpaceId(0), idle_ksp, 0);
    idle.state = TaskState::Runnable;
    let idle_id = table.insert(idle).expect("idle insert");
    sched.set_idle(idle_id);
    println!("task: idle created");

    // User tasks.
    for (i, (image, name)) in [(USER_TASK_A, "user0"), (USER_TASK_B, "user1")]
        .iter().enumerate()
    {
        let tid    = TaskId::new((i + 1) as u16, 0);
        let asp_id = AddressSpaceId((i + 1) as u16);

        let root_f = fa!().alloc_frame(FrameOwner::KernelPageTable).expect("user root");
        let mut aspace = AddressSpace::new(asp_id, root_f);
        aspace.clone_kernel_half(kernel_root);

        let text_f = fa!().alloc_frame(FrameOwner::UserText { task: tid })
                          .expect("text frame");
        // SAFETY: text_f freshly allocated, 4-KiB aligned.
        unsafe {
            let dst = core::slice::from_raw_parts_mut(text_f.pa() as *mut u8, 4096);
            dst[..image.len()].copy_from_slice(image);
        }
        aspace.map_page(VirtAddr(USER_TEXT_VA), text_f,
            VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa!())
            .expect("map text");

        let stack_f = fa!().alloc_frame(FrameOwner::UserStack { task: tid })
                           .expect("stack frame");
        aspace.map_page(VirtAddr(USER_STACK_TOP - 4096), stack_f,
            VmPerms::R | VmPerms::W | VmPerms::U, VmRegionKind::UserStack, fa!())
            .expect("map stack");

        let kstack_f = fa!().alloc_frame(FrameOwner::KernelStack).expect("kstack");

        let mut t = Task::new(tid, PRIORITY_USER, asp_id,
                              kstack_f.pa() + 4096, USER_STACK_TOP);
        t.trap_frame.sepc    = USER_ENTRY_VA;
        t.trap_frame.gpr[2]  = USER_STACK_TOP;
        t.trap_frame.sstatus = 1 << 5; // SPIE, SPP=0
        t.state = TaskState::Runnable;

        let ins_id = table.insert(t).expect("task insert");
        sched.enqueue_runnable(ins_id, PRIORITY_USER);
        AUDIT.lock_free_append(AuditKindInternal::TaskCreate, i, 0, 0);

        // Install endpoint capability:
        //   user0 (i=0) → SEND | CALL (client role)
        //   user1 (i=1) → RECV        (server role)
        let rights = if i == 0 {
            fjell_cap::CapRights::SEND | fjell_cap::CapRights::CALL
        } else {
            fjell_cap::CapRights::RECV
        };
        ct.cspace_mut(ins_id.index as usize)
          .and_then(|cs| cs.install_root(fjell_cap::CapKind::Endpoint, ep_obj_id, rights).ok())
          .expect("install endpoint cap");

        println!("task: {} created", name);
    }

    println!("sched: started");

    // Choose the first task and enter user mode.
    // From this point on, all scheduling is handled in trap_dispatch.
    let first_id = sched.choose_next();
    sched.set_current(first_id);
    if let Some(t) = table.get_mut(first_id) {
        t.state = TaskState::Running;
        t.accounting.run_count += 1;
    }

    let first_tf = &table.get(first_id).unwrap().trap_frame;

    // Set up sscratch pointing to the STATIC trap scratch record.
    // scratch[0] = boot stack top (kernel sp restored on trap entry).
    // scratch[1] = &TrapFrame of the first task.
    // Must be static — sscratch is read on every future trap, long after
    // this stack frame is gone.
    // SAFETY: TRAP_SCRATCH is static; valid for the entire kernel lifetime.
    unsafe {
        let s = &mut *TRAP_SCRATCH.0.get();
        s[0] = idle_ksp;
        s[1] = first_tf as *const _ as usize;
        csr::write_sscratch(s.as_ptr() as usize);
    }

    // SAFETY: first_tf is valid; sepc in user VA; sstatus.SPP=0.
    unsafe { trap::dispatch::first_entry(first_tf) }
}

// ── Panic handler ─────────────────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n[KERNEL PANIC] {}", info);
    loop {}
}
