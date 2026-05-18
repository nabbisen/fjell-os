# RFC 004: Capability gating for TaskSpawn / TaskStart / Lease syscalls

**RFC ID:** 004  
**Status:** Implemented  
**Affects:** `crates/fjell-cap/src/rights.rs`, `crates/fjell-kernel/src/trap/syscall.rs`,
`crates/fjell-kernel/src/main.rs`

---

## 1. Problem (RB-01)

`sys_task_spawn`, `sys_task_start`, `sys_task_status`, `sys_lease_create`,
`sys_lease_revoke` have no capability check.  Any user-space task can spawn
arbitrary service images, start tasks in other address spaces, and create/revoke
leases.  This violates the No Ambient Authority principle and makes the capability
broker and service manager bypassable.

---

## 2. Proposed fix

### 2.1 New CapKind variants

```rust
pub enum CapKind {
    Task,
    AddressSpace,
    Endpoint,
    Frame,
    Reply,
    // M7.1 additions:
    TaskCreate,   // authorises sys_task_spawn
    TaskControl,  // authorises sys_task_start / sys_task_status
    LeaseAdmin,   // authorises sys_lease_create / sys_lease_revoke / sys_lease_inspect
}
```

### 2.2 Bootstrap capability grant

At kernel init, give the init task (index 1) three bootstrap capabilities in its CSpace:

```rust
// slot 28: TaskCreate
// slot 29: TaskControl
// slot 30: LeaseAdmin
```

### 2.3 Syscall checks

```rust
// sys_task_spawn: caller must hold TaskCreate cap
// sys_task_start: caller must hold TaskControl cap
// sys_lease_create/revoke/inspect: caller must hold LeaseAdmin cap
```

### 2.4 Delegation

init grants TaskCreate/TaskControl to service-manager via cap_derive at startup.
This is the only path to spawning new tasks.

---

## 3. Rationale

ID allowlist (checking `current_task_idx() == 1`) is explicitly rejected per
architect review.  A minimal capability check connects syscall authority to the
existing CSpace/CapHandle mechanism without a full redesign.

This is a minimum viable implementation.  Full rights-based policy (spawn only images
in capability's object_id set, etc.) is deferred to M8.

---

## 4. Impact

| Crate | Change |
|---|---|
| `fjell-cap/src/rights.rs` | Add 3 CapKind variants |
| `fjell-kernel/src/main.rs` | Grant bootstrap caps to init task CSpace |
| `fjell-kernel/src/trap/syscall.rs` | Add cap check to 5 syscall handlers |
| `fjell-syscall/src/lib.rs` | No ABI change |

---

## 5. Test plan

1. `cargo xtask qemu-test m7` passes (init holds the caps, so spawn works).
2. Unit test: spawn from a task without TaskCreate cap returns `InvalidCap`.

---

## 6. Implementation notes

- CSpace slot assignment: bootstrap caps at slots 28-30.  These do not conflict
  with existing IPC endpoint slots (0-3).
- init delegates TaskCreate to service-manager by calling cap_derive then ipc_send.
  For M7.1, this delegation happens inline in fjell-init.
- LeaseAdmin owner check: `sys_lease_revoke` must also verify the caller
  is the lease creator (owner field in LeaseObject).
