//! Task spawn primitive — M4.
//!
//! Loads a flat service binary from the embedded image table into a fresh
//! address space and returns the new `TaskId`.

use crate::mm::address::{PhysFrame, VirtAddr};
use crate::mm::frame_alloc::{FrameAllocator, FrameOwner};
use crate::mm::region::VmRegionKind;
use crate::mm::vspace::{AddressSpace, AddressSpaceId, VmPerms};
use crate::task::image::{SERVICE_BASE_VA, SERVICE_STACK_TOP, image_bytes};
use crate::task::scheduler::PRIORITY_USER;
use crate::task::tcb::{Task, TaskState};
use fjell_abi::error::SysError;
use fjell_abi::service::ImageId;
use fjell_abi::task::TaskId;

// RFC-0.26-001 (closes Errata E-018): this used to be a local `const
// PRIORITY_USER: u8 = 2`, shadowing `task::scheduler::PRIORITY_USER` (32)
// with a different value. See docs/rfcs/RFC-0.26-001-scheduler-priority-
// unification-investigation.md for why unifying just this constant alone
// (without also fixing `sys_task_start`'s own separate hardcoded literal)
// hung the M6 boot sequence, and why the two sites had to be fixed together.

/// Spawn a new task from `image_id`.
///
/// Allocates frames for text + stack, maps them in a new address space,
/// creates a TCB and inserts it into the task table.
///
/// Returns `(TaskId, task_handle_raw)`.  The task is in `Created` state
/// and must be started with `sys_task_start`.
pub fn spawn(
    image_id: ImageId,
    table: &mut crate::task::tcb::TaskTable,
    _sched: &mut crate::task::scheduler::Scheduler, // reserved for M5 enqueue-on-spawn
    kernel_root: crate::mm::frame_alloc::PhysFrame,
    fa: &mut FrameAllocator<'_>,
) -> Result<TaskId, SysError> {
    let bytes = image_bytes(image_id).ok_or(SysError::InvalidCap)?;

    // Find a fresh task slot index.
    let tid_index = table.next_free_index().ok_or(SysError::NoMemory)?;
    let tid = TaskId::new(tid_index, 0);
    let asp_id = AddressSpaceId(tid_index);

    // Allocate root page table.
    let root_f = fa
        .alloc_frame(FrameOwner::KernelPageTable)
        .map_err(|_| SysError::NoMemory)?;
    let mut aspace = AddressSpace::new(asp_id, root_f);
    aspace.clone_kernel_half(kernel_root);

    // Map UART for kernel debug output from trap handler.
    let uart_f = PhysFrame::from_pa(0x1000_0000).unwrap();
    aspace
        .map_page(
            VirtAddr(0x1000_0000),
            uart_f,
            VmPerms::R | VmPerms::W,
            VmRegionKind::Mmio,
            fa,
        )
        .map_err(|_| SysError::NoMemory)?;

    // RFC-0.25-001: map the PLIC pages the trap handler touches on every
    // external interrupt (`crate::plic::claim`/`complete`, and `enable` from
    // `sys_irq_bind`). Trap-handling code runs under whichever task's page
    // table happens to be active, same reasoning as the UART mapping above —
    // R|W, no U: only S-mode trap-handling code touches the PLIC directly.
    for &pa in crate::plic::MAPPED_PAGES.iter() {
        if let Ok(f) = PhysFrame::from_pa(pa) {
            aspace
                .map_page(
                    VirtAddr(pa),
                    f,
                    VmPerms::R | VmPerms::W,
                    VmRegionKind::Mmio,
                    fa,
                )
                .map_err(|_| SysError::NoMemory)?;
        }
    }

    // Map all 8 virtio-mmio slots (0x10001000..0x10008000) with R|W (no U).
    // Supervisor-mode trap handlers (sys_platform_info_get, sys_mmio_map) can
    // then scan/access them.  User-mode drivers call sys_mmio_map to get U+R+W.
    for i in 0..8usize {
        let mmio_pa = 0x1000_1000 + i * 0x1000;
        if let Ok(f) = PhysFrame::from_pa(mmio_pa) {
            let _ = aspace.map_page(
                VirtAddr(mmio_pa),
                f,
                VmPerms::R | VmPerms::W,
                VmRegionKind::Mmio,
                fa,
            );
        }
    }

    // Allocate text frame, copy flat binary.
    if bytes.len() > 4096 {
        let pages = (bytes.len() + 4095) / 4096;
        for i in 0..pages {
            let f = fa
                .alloc_frame(FrameOwner::UserText { task: tid })
                .map_err(|_| SysError::NoMemory)?;
            let start = i * 4096;
            let end = (start + 4096).min(bytes.len());
            // SAFETY: category=raw-pointer-deref frame is exclusively owned; within physical RAM.
            unsafe {
                let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
                dst.fill(0);
                dst[..(end - start)].copy_from_slice(&bytes[start..end]);
            }
            aspace
                .map_page(
                    VirtAddr(SERVICE_BASE_VA + i * 4096),
                    f,
                    VmPerms::R | VmPerms::X | VmPerms::U,
                    VmRegionKind::UserText,
                    fa,
                )
                .map_err(|_| SysError::NoMemory)?;
        }
    } else {
        let f = fa
            .alloc_frame(FrameOwner::UserText { task: tid })
            .map_err(|_| SysError::NoMemory)?;
        // SAFETY: category=raw-pointer-deref frame is exclusively owned; within physical RAM.
        unsafe {
            let dst = core::slice::from_raw_parts_mut(f.pa() as *mut u8, 4096);
            dst.fill(0);
            dst[..bytes.len()].copy_from_slice(bytes);
        }
        aspace
            .map_page(
                VirtAddr(SERVICE_BASE_VA),
                f,
                VmPerms::R | VmPerms::X | VmPerms::U,
                VmRegionKind::UserText,
                fa,
            )
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate and map all stack pages (64 KiB = 16 pages).
    // The linker script places __stack_bottom = 0x80000, __stack_top = 0x90000.
    // Mapping only the top page caused StorePageFault when stack usage exceeded 4K.
    const STACK_PAGES: usize = 16;
    let stack_base = SERVICE_STACK_TOP - STACK_PAGES * 4096;
    for pg in 0..STACK_PAGES {
        let sf = fa
            .alloc_frame(FrameOwner::UserStack { task: tid })
            .map_err(|_| SysError::NoMemory)?;
        aspace
            .map_page(
                VirtAddr(stack_base + pg * 4096),
                sf,
                VmPerms::R | VmPerms::W | VmPerms::U,
                VmRegionKind::UserStack,
                fa,
            )
            .map_err(|_| SysError::NoMemory)?;
    }

    // Allocate kernel stack.
    let kstack_f = fa
        .alloc_frame(FrameOwner::KernelStack)
        .map_err(|_| SysError::NoMemory)?;

    // Build TCB.
    let mut t = Task::new(
        tid,
        PRIORITY_USER,
        asp_id,
        kstack_f.pa() + 4096,
        SERVICE_STACK_TOP,
    );
    t.satp_root_pfn = root_f.pfn as usize;
    t.trap_frame.sepc = SERVICE_BASE_VA;
    t.trap_frame.gpr[2] = SERVICE_STACK_TOP;
    t.trap_frame.sstatus = 1 << 5; // SPIE, SPP=0 (user mode)
    t.state = TaskState::Created;

    let ins_id = table.insert(t).map_err(|_| SysError::NoMemory)?;

    // Install bootstrap capabilities in the new task's CSpace (RFC 016, M7.1).
    // Uses ins_id.index (the actual slot) so the index is always correct.
    {
        use crate::platform::qemu_virt::{MMIO_REGION_COUNT, mmio_region_table};
        use fjell_cap::slot::Capability;
        use fjell_cap::{CapKind, CapRights, CapState, ObjectScope};

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
            //   9 = uart-rx (RFC-0.25-001 — driver-uart posts received bytes here)
            let ep_obj: u32 = match image_id {
                fjell_abi::service::ImageId::STORAGED => 1,
                fjell_abi::service::ImageId::MEASUREDD => 2,
                fjell_abi::service::ImageId::ATTESTD => 3,
                fjell_abi::service::ImageId::RECOVERYD => 4,
                // RFC 040: cap-broker gets its own dedicated endpoint (5)
                // so policy tests can route to it without ambiguity.
                fjell_abi::service::ImageId::CAP_BROKER => 5,
                // RFC 042: dedicated endpoint so the neg-test IPC protocol
                // cannot be stolen by other shared-endpoint receivers.
                fjell_abi::service::ImageId::SAMPLE_SERVICE => 6,
                // RFC-v0.23-001: dedicated endpoints for the ABDD live path.
                fjell_abi::service::ImageId::SEMANTIC_STREAM => 7,
                fjell_abi::service::ImageId::PROXY_TEXT => 8,
                // RFC-0.25-001: driver-uart's send end of the uart-rx endpoint.
                fjell_abi::service::ImageId::DRIVER_UART => 9,
                _ => 0,
            };
            let _ = cs.install_raw(
                0,
                Capability {
                    kind: CapKind::Endpoint,
                    object_id: ep_obj,
                    rights: CapRights::ALL_NON_META,
                    badge: 0,
                    scope: ObjectScope::Any,
                    state: CapState::Active,
                    parent: None,
                    lease: None,
                },
            );
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
            // Region index → CSpace slot: slot = 31 + region_idx
            //   region 0 (id=0): CLINT/boot-ROM   → slot 31
            //   region 1 (id=1): UART0             → slot 32
            //   region 2 (id=2): PLIC              → slot 33
            //   region 3 (id=3): virtio-mmio range → slot 34  ← MMIO_REGION_VIRTIO
            //   region 4 (id=4): neg-test-RAM       → slot 35
            //
            // RFC-v0.7.4-003: MMIO caps granted only to services that have
            // documented device access at this architectural phase.
            //
            // STORAGED is here as a transitional exception: it still maps
            // virtio-mmio (region 3) directly to drive virtio-blk.  Full
            // decoupling (storaged → driver-virtio-blk via IPC) is the
            // RFC-v0.7.4-003 follow-on, targeted for v0.8.x once driver-virtio-blk
            // exposes a stable service-api endpoint.
            let mmio_regions_for_service: Option<&[usize]> = match image_id {
                fjell_abi::service::ImageId::DEVMGR => {
                    // devmgr reads every region to enumerate the board profile.
                    static ALL: &[usize] = &[0, 1, 2, 3, 4];
                    Some(ALL)
                }
                fjell_abi::service::ImageId::STORAGED => {
                    // storaged maps region 3 (virtio-mmio) for virtio-blk I/O.
                    // CSpace slot 34 = 31 + 3 = the slot storaged looks up (MMIO_SLOT).
                    static VQ: &[usize] = &[3];
                    Some(VQ)
                }
                fjell_abi::service::ImageId::DRIVER_VIRTIO_BLK => {
                    // Dedicated virtio-blk driver — region 3 (virtio-mmio range).
                    static VQ: &[usize] = &[3];
                    Some(VQ)
                }
                fjell_abi::service::ImageId::DRIVER_VIRTIO_NET => {
                    // Dedicated virtio-net driver — region 3 (same virtio-mmio range).
                    static VQ: &[usize] = &[3];
                    Some(VQ)
                }
                fjell_abi::service::ImageId::NEG_TEST => {
                    // Integration test harness — needs all regions for negative-test
                    // coverage. Audited exception.
                    static ALL: &[usize] = &[0, 1, 2, 3, 4];
                    Some(ALL)
                }
                fjell_abi::service::ImageId::DRIVER_UART => {
                    // RFC-0.25-001: UART0's own registers (region 1) — slot
                    // 31+1=32, matching `fjell-driver-uart`'s CAP_MMIO constant.
                    static UART0: &[usize] = &[1];
                    Some(UART0)
                }
                _ => None,
            };
            if let Some(regions) = mmio_regions_for_service {
                for &region_idx in regions {
                    if region_idx < MMIO_REGION_COUNT {
                        if let Some(_r) = mmio_table.get(region_idx) {
                            let _ = cs.install_raw(
                                31 + region_idx,
                                Capability {
                                    kind: CapKind::MmioRegion,
                                    object_id: region_idx as u32,
                                    rights: CapRights::MMIO_MAP,
                                    badge: 0,
                                    scope: ObjectScope::Any,
                                    state: CapState::Active,
                                    parent: None,
                                    lease: None,
                                },
                            );
                        }
                    }
                }
            }
            // Slot 1: Interrupt cap — granted to driver-uart only (RFC-0.25-001).
            // object_id = 10 = UART0's IRQ line on QEMU virt (handoff §2, R6).
            if image_id == fjell_abi::service::ImageId::DRIVER_UART {
                let _ = cs.install_raw(
                    1,
                    Capability {
                        kind: CapKind::Interrupt,
                        object_id: 10,
                        rights: CapRights(
                            CapRights::IRQ_BIND.0 | CapRights::IRQ_UNBIND.0 | CapRights::IRQ_ACK.0,
                        ),
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 1: AuditDrain cap — granted to auditd only (RFC 020).
            // Fixed in v0.2.9: was RECV (wrong right), now AUDIT_DRAIN per sys_audit_drain check.
            if image_id == fjell_abi::service::ImageId::AUDITD
                || image_id == fjell_abi::service::ImageId::NEG_TEST
            {
                let _ = cs.install_raw(
                    1,
                    Capability {
                        kind: CapKind::AuditDrain,
                        object_id: 0,
                        rights: CapRights::AUDIT_DRAIN,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 2: DmaRegion cap — granted to services that perform DMA
            // (storaged, driver-virtio-blk, neg-test).  RFC 017 / RFC 052.
            // Modernised from the legacy DmaAlloc alias (architect review
            // v0.18 follow-up): sys_dma_alloc accepts both kinds, but
            // sys_dma_revoke requires DmaRegion, so the legacy grant made
            // explicit revocation silently impossible for these services.
            let needs_dma = matches!(
                image_id,
                fjell_abi::service::ImageId::STORAGED
                    | fjell_abi::service::ImageId::DRIVER_VIRTIO_BLK
                    | fjell_abi::service::ImageId::NEG_TEST
            );
            if needs_dma {
                let _ = cs.install_raw(
                    2,
                    Capability {
                        kind: CapKind::DmaRegion,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 9: Interrupt cap for NEG_TEST, deliberately WITHOUT
            // IRQ_BIND (RFC-0.25-001 Demonstration 4: a task without the
            // right must be refused, not merely lack the capability kind —
            // IRQ_ACK is granted so the rejection is specifically a
            // MissingRight, not a WrongKind or EmptySlot).
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(
                    9,
                    Capability {
                        kind: CapKind::Interrupt,
                        object_id: 10,
                        rights: CapRights::IRQ_ACK,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 7: endpoint cap to sample-service (object 6) for NEG_TEST —
            // both legs of the RFC 042 IPC protocol run on this endpoint.
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(
                    7,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 6,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 2: shared-endpoint (object 0) cap for SAMPLE_SERVICE so its
            // SERVICE_READY signal still reaches service-manager after the
            // move to the dedicated endpoint.
            if image_id == fjell_abi::service::ImageId::SAMPLE_SERVICE {
                let _ = cs.install_raw(
                    2,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 1: LeaseAdmin for SAMPLE_SERVICE (RFC 042 IPC blocked-recv test).
            // sample-service binds a lease to a copied endpoint cap and blocks
            // in ipc_recv to allow the lease-revoked wakeup scenario to be tested.
            if image_id == fjell_abi::service::ImageId::SAMPLE_SERVICE {
                let _ = cs.install_raw(
                    1,
                    Capability {
                        kind: CapKind::LeaseAdmin,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 3: cap-broker endpoint cap for NEG_TEST (RFC 042 policy tests).
            // object_id=5 is cap-broker's dedicated endpoint.
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(
                    3,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 5,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 4: LeaseAdmin cap for NEG_TEST — required by sys_cap_bind_lease
            // so the neg-test service can create and revoke lease-bound caps (RFC 042).
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(
                    4,
                    Capability {
                        kind: CapKind::LeaseAdmin,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slots 5-6: TaskCreate + TaskControl for NEG_TEST (RFC 042 SVC tests).
            // Allows neg-test to spawn and monitor the svc-timeout/svc-fault services.
            if image_id == fjell_abi::service::ImageId::NEG_TEST {
                let _ = cs.install_raw(
                    5,
                    Capability {
                        kind: CapKind::TaskCreate,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
                let _ = cs.install_raw(
                    6,
                    Capability {
                        kind: CapKind::TaskControl,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 3: semantic-stream endpoint cap (object 7) for SAMPLE_SERVICE
            // (RFC-v0.23-001) — lets the SDK reference service emit an intent
            // node over IPC, demonstrating the authoring pattern.
            if image_id == fjell_abi::service::ImageId::SAMPLE_SERVICE {
                let _ = cs.install_raw(
                    3,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 7,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slot 1: proxy-text endpoint cap (object 8) for SEMANTIC_STREAM
            // (RFC-v0.23-001) — lets semantic-stream forward a node on to the
            // proxy for rendering.
            if image_id == fjell_abi::service::ImageId::SEMANTIC_STREAM {
                let _ = cs.install_raw(
                    1,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 8,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
            // Slots 1-2 for PROXY_TEXT (RFC-v0.23-001):
            //   1 = semantic-stream endpoint cap (object 7), for the
            //       capability-checked ActionRequest return leg.
            //   2 = a deliberately narrow-rights capability (SEND | REPLY |
            //       INSPECT only — no MMIO_MAP, no DMA_*, no TASK_*) that
            //       proxy-text introspects via sys_cap_inspect to obtain its
            //       OWN kernel-verified rights, rather than self-asserting a
            //       bitmask. INSPECT must be included on *this* capability
            //       itself, not just held generally — RFC 049's
            //       sys_cap_inspect checks `cap.rights.contains(INSPECT)` on
            //       the exact capability being inspected
            //       (crates/fjell-kernel/src/cap/syscall.rs), confirmed live:
            //       omitting it here made every sys_cap_inspect call fail
            //       with PermissionDenied, silently defaulting
            //       `granted_rights` to 0 and denying every action
            //       regardless of its required right. This is what makes
            //       the accept/refuse demonstration real: an action whose
            //       required right is a subset of slot 2's rights is
            //       accepted; one that is not, is refused. The object_id is
            //       unused (this capability is never sent through) and is
            //       set to 0.
            if image_id == fjell_abi::service::ImageId::PROXY_TEXT {
                let _ = cs.install_raw(
                    1,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 7,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
                let _ = cs.install_raw(
                    2,
                    Capability {
                        kind: CapKind::Endpoint,
                        object_id: 0,
                        rights: CapRights(
                            CapRights::SEND.0 | CapRights::REPLY.0 | CapRights::INSPECT.0,
                        ),
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
        }
    }

    // RFC 056: CapInstall cap for CAP_BROKER (slot 10).
    {
        use fjell_cap::slot::Capability;
        use fjell_cap::{CapKind, CapRights, CapState, ObjectScope};
        if image_id == fjell_abi::service::ImageId::CAP_BROKER {
            // SAFETY: category=kernel-global-mutable task stack and entry point are validated during service manifest parsing.
            let (_, _, ct, _) = unsafe { crate::get_kernel_state() };
            if let Some(cs) = ct.cspace_mut(ins_id.index as usize) {
                let _ = cs.install_raw(
                    10,
                    Capability {
                        kind: CapKind::CapInstall,
                        object_id: 0,
                        rights: CapRights::ALL_NON_META,
                        badge: 0,
                        scope: ObjectScope::Any,
                        state: CapState::Active,
                        parent: None,
                        lease: None,
                    },
                );
            }
        }
    }

    // RFC 055: store the image_id in the TCB for kernel-attested IPC identity.
    if let Some(task) = table.get_mut(ins_id) {
        task.image_id = image_id;
    }

    Ok(ins_id)
}
