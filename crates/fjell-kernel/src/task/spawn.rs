//! Task spawn primitive — M4.
//!
//! Loads a flat service binary from the embedded image table into a fresh
//! address space and returns the new `TaskId`.

use fjell_abi::service::ImageId;
use fjell_abi::error::SysError;
use crate::task::tcb::{Task, TaskState};
use crate::mm::frame_alloc::{FrameAllocator, FrameOwner};
use crate::mm::address::{VirtAddr, PhysFrame};
use crate::mm::vspace::{AddressSpace, AddressSpaceId, VmPerms};
use crate::mm::region::VmRegionKind;
use crate::task::image::{image_bytes, SERVICE_BASE_VA, SERVICE_STACK_TOP};
use fjell_abi::task::TaskId;

const PRIORITY_USER: u8 = 2;

/// Spawn a new task from `image_id`.
///
/// Allocates frames for text + stack, maps them in a new address space,
/// creates a TCB and inserts it into the task table.
///
/// Returns `(TaskId, task_handle_raw)`.  The task is in `Created` state
/// and must be started with `sys_task_start`.
pub fn spawn(
    image_id:    ImageId,
    table:       &mut crate::task::tcb::TaskTable,
    _sched:      &mut crate::task::scheduler::Scheduler,  // reserved for M5 enqueue-on-spawn
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa:          &mut FrameAllocator<'_>,
) -> Result<TaskId, SysError>
{
    let bytes = image_bytes(image_id).ok_or(SysError::InvalidCap)?;

    // Find a fresh task slot index.
    let tid_index = table.next_free_index().ok_or(SysError::NoMemory)?;
    let tid       = TaskId::new(tid_index, 0);
    let asp_id    = AddressSpaceId(tid_index);

    // Allocate root page table.
    let root_f = fa.alloc_frame(FrameOwner::KernelPageTable)
                   .map_err(|_| SysError::NoMemory)?;
    let mut aspace = AddressSpace::new(asp_id, root_f);
    aspace.clone_kernel_half(kernel_root);

    // Map UART for kernel debug output from trap handler.
    let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
    aspace.map_page(VirtAddr(0x1000_0000), uart_f,
        VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa)
        .map_err(|_| SysError::NoMemory)?;

    // Map all 8 virtio-mmio slots (0x10001000..0x10008000) with R|W (no U).
    // Supervisor-mode trap handlers (sys_platform_info_get, sys_mmio_map) can
    // then scan/access them.  User-mode drivers call sys_mmio_map to get U+R+W.
    for i in 0..8usize {
        let mmio_pa = 0x1000_1000 + i * 0x1000;
        if let Ok(f) = PhysFrame::from_pa(mmio_pa) {
            let _ = aspace.map_page(VirtAddr(mmio_pa), f,
                VmPerms::R | VmPerms::W, VmRegionKind::Mmio, fa);
        }
    }

    // Allocate text frame, copy flat binary.
    if bytes.len() > 4096 {
        // Allocate additional pages if needed (up to 8 pages for now)
        let pages = (bytes.len() + 4095) / 4096;
        for i in 0..pages {
            let f = fa.alloc_frame(FrameOwner::UserText { task: tid })
                      .map_err(|_| SysError::NoMemory)?;
            let start = i * 4096;
            let end   = (start + 4096).min(bytes.len());
            // SAFETY: category=raw-pointer-deref task stack and entry point are validated during service manifest parsing.
            unsafe {
                let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
                dst.fill(0);
                dst[..(end - start)].copy_from_slice(&bytes[start..end]);
            }
            aspace.map_page(VirtAddr(SERVICE_BASE_VA + i * 4096), f,
                VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa)
                .map_err(|_| SysError::NoMemory)?;
        }
    } else {
        let f = fa.alloc_frame(FrameOwner::UserText { task: tid })
                  .map_err(|_| SysError::NoMemory)?;
        // SAFETY: category=raw-pointer-deref task stack and entry point are validated during service manifest parsing.
        unsafe {
            let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
            dst.fill(0);
            dst[..bytes.len()].copy_from_slice(bytes);
        }
        aspace.map_page(VirtAddr(SERVICE_BASE_VA), f,
            VmPerms::R | VmPerms::X | VmPerms::U, VmRegionKind::UserText, fa)
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate and map all stack pages (64 KiB = 16 pages).
    // The linker script places __stack_bottom = 0x80000, __stack_top = 0x90000.
    // Mapping only the top page caused StorePageFault when stack usage exceeded 4K.
    const STACK_PAGES: usize = 16;
    let stack_base = SERVICE_STACK_TOP - STACK_PAGES * 4096;
    for pg in 0..STACK_PAGES {
        let sf = fa.alloc_frame(FrameOwner::UserStack { task: tid })
                   .map_err(|_| SysError::NoMemory)?;
        aspace.map_page(VirtAddr(stack_base + pg * 4096), sf,
            VmPerms::R | VmPerms::W | VmPerms::U, VmRegionKind::UserStack, fa)
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate kernel stack.
    let kstack_f = fa.alloc_frame(FrameOwner::KernelStack)
                     .map_err(|_| SysError::NoMemory)?;

    // Build TCB.
    let mut t = Task::new(tid, PRIORITY_USER, asp_id,
                          kstack_f.pa() + 4096, SERVICE_STACK_TOP);
    t.satp_root_pfn     = root_f.pfn as usize;
    t.trap_frame.sepc   = SERVICE_BASE_VA;
    t.trap_frame.gpr[2] = SERVICE_STACK_TOP;
    t.trap_frame.sstatus = 1 << 5; // SPIE, SPP=0 (user mode)
    t.state = TaskState::Created;

    let ins_id = table.insert(t).map_err(|_| SysError::NoMemory)?;

    // Install bootstrap capabilities in the new task's CSpace (RFC 016, M7.1).
    // Uses ins_id.index (the actual slot) so the index is always correct.
    {
        use fjell_cap::{CapKind, CapRights, CapState, ObjectScope};
        use fjell_cap::slot::Capability;
        use crate::platform::qemu_virt::{mmio_region_table, MMIO_REGION_COUNT};

        // SAFETY: category=kernel-global-mutable task stack and entry point are validated during service manifest parsing.
        let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
        if let Some(cs) = ct.cspace_mut(ins_id.index as usize) {
            // Slot 0: IPC endpoint.
            // Private endpoint assignments (init holds caps to these):
            //   0 = shared (all non-special services)
            //   1 = storaged (RFC 019)
            //   2 = measuredd (M8)
            //   3 = attestd   (M8)
            //   4 = recoveryd (M8)
            let ep_obj: u32 = match image_id {
                fjell_abi::service::ImageId::STORAGED   => 1,
                fjell_abi::service::ImageId::MEASUREDD  => 2,
                fjell_abi::service::ImageId::ATTESTD    => 3,
                fjell_abi::service::ImageId::RECOVERYD  => 4,
                // RFC 040: cap-broker gets its own dedicated endpoint (5)
                // so policy tests can route to it without ambiguity.
                fjell_abi::service::ImageId::CAP_BROKER => 5,
                _                                       => 0,
            };
            let _ = cs.install_raw(0, Capability {
                kind: CapKind::Endpoint, object_id: ep_obj,
                rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
            });
            // Slots 31-35: MmioRegion caps.
            // RFC-v0.7.4-003 (closes C-RB-03): MMIO caps now granted ONLY to the driver
            // that owns the specific device, not to all services.
            // Non-driver services must request device authority from cap-broker.
            //
            // Bootstrap exceptions (require MMIO at spawn, cannot yet use cap-broker):
            //   - devmgr: reads BoardProfile to enumerate devices; needs all regions.
            //   - driver-virtio-blk: block device driver (region 0).
            //   - driver-virtio-net: network device driver (region 1).
            //   - neg-test: integration test harness (all regions for test coverage).
            let mmio_table = mmio_region_table();
            let mmio_regions_for_service: Option<&[usize]> = match image_id {
                fjell_abi::service::ImageId::DEVMGR => {
                    // devmgr needs the full table to enumerate devices on first boot.
                    static ALL: &[usize] = &[0, 1, 2, 3, 4];
                    Some(ALL)
                }
                fjell_abi::service::ImageId::DRIVER_VIRTIO_BLK => {
                    static BLK: &[usize] = &[0];
                    Some(BLK)
                }
                fjell_abi::service::ImageId::DRIVER_VIRTIO_NET => {
                    static NET: &[usize] = &[1];
                    Some(NET)
                }
                fjell_abi::service::ImageId::NEG_TEST => {
                    // Kept for integration test coverage; audited exception.
                    static ALL: &[usize] = &[0, 1, 2, 3, 4];
                    Some(ALL)
                }
                _ => None,
            };
            if let Some(regions) = mmio_regions_for_service {
                for &region_idx in regions {
                    if region_idx < MMIO_REGION_COUNT {
                        if let Some(_r) = mmio_table.get(region_idx) {
                            let _ = cs.install_raw(31 + region_idx, Capability {
                                kind: CapKind::MmioRegion, object_id: region_idx as u32,
                                rights: CapRights::MMIO_MAP, badge: 0,
                                scope: ObjectScope::Any, state: CapState::Active,
                                parent: None, lease: None,
                            });
                        }
                    }
                }
            }
            // Slot 1: AuditDrain cap — granted to auditd only (RFC 020).
            // Fixed in v0.2.9: was RECV (wrong right), now AUDIT_DRAIN per sys_audit_drain check.
            if image_id == fjell_abi::service::ImageId::AUDITD
            || image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(1, Capability {
                    kind: CapKind::AuditDrain, object_id: 0,
                    rights: CapRights::AUDIT_DRAIN, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
            // Slot 2: DmaAlloc cap — granted to services that perform DMA
            // (storaged, driver-virtio-blk).  RFC 017.
            let needs_dma = matches!(
                image_id,
                fjell_abi::service::ImageId::STORAGED |
                fjell_abi::service::ImageId::DRIVER_VIRTIO_BLK |
                fjell_abi::service::ImageId::NEG_TEST
            );
            if needs_dma {
                let _ = cs.install_raw(2, Capability {
                    kind: CapKind::DmaAlloc, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
            // Slot 1: LeaseAdmin for SAMPLE_SERVICE (RFC 042 IPC blocked-recv test).
            // sample-service binds a lease to a copied endpoint cap and blocks
            // in ipc_recv to allow the lease-revoked wakeup scenario to be tested.
            if image_id == fjell_abi::service::ImageId::SAMPLE_SERVICE {
                let _ = cs.install_raw(1, Capability {
                    kind: CapKind::LeaseAdmin, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any,
                    state: CapState::Active, parent: None, lease: None,
                });
            }
            // Slot 3: cap-broker endpoint cap for NEG_TEST (RFC 042 policy tests).
            // object_id=5 is cap-broker's dedicated endpoint.
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(3, Capability {
                    kind: CapKind::Endpoint, object_id: 5,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
            // Slot 4: LeaseAdmin cap for NEG_TEST — required by sys_cap_bind_lease
            // so the neg-test service can create and revoke lease-bound caps (RFC 042).
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(4, Capability {
                    kind: CapKind::LeaseAdmin, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
            // Slots 5-6: TaskCreate + TaskControl for NEG_TEST (RFC 042 SVC tests).
            // Allows neg-test to spawn and monitor the svc-timeout/svc-fault services.
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(5, Capability {
                    kind: CapKind::TaskCreate, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
                let _ = cs.install_raw(6, Capability {
                    kind: CapKind::TaskControl, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
        }
    }

    // RFC 056: CapInstall cap for CAP_BROKER (slot 10).
    {
        use fjell_cap::{CapKind, CapRights, CapState, ObjectScope};
        use fjell_cap::slot::Capability;
        if image_id == fjell_abi::service::ImageId::CAP_BROKER {
            // SAFETY: category=kernel-global-mutable task stack and entry point are validated during service manifest parsing.
            let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
            if let Some(cs) = ct.cspace_mut(ins_id.index as usize) {
                let _ = cs.install_raw(10, Capability {
                    kind: CapKind::CapInstall, object_id: 0,
                    rights: CapRights::ALL, badge: 0, scope: ObjectScope::Any, state: CapState::Active, parent: None, lease: None,
                });
            }
        }
    }

    // RFC 055: store the image_id in the TCB for kernel-attested IPC identity.
    if let Some(task) = table.get_mut(ins_id) {
        task.image_id = image_id;
    }

    Ok(ins_id)
}
