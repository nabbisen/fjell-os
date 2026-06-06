# RFC 055: Kernel-attested sender identity in IPC

**RFC ID:** 055
**Also known as:** RFC-v0.2-021
**Status:** Implemented
**Target version:** v0.2.12
**Phase:** Service separation + release-gate close
**Closes review item:** RB-11 (identity half)
**Depends on:** RFC 034 (Blocked IPC revocation)
**Blocks:** RFC 056

## Problem

`crates/fjell-cap-broker/src/main.rs:394-396` reads requester identity
from the message payload:

```rust
let requester_id   = w0 as u16;  // ← sender claims their identity here
let resource_class = ResourceClass::from_u32(w1 as u32) as u16;
let requested_rights = w2 as u64;
```

A service with access to the broker endpoint can claim *any* requester id.
The cap-broker's bootstrap-vs-enforcing typestate provides no protection
against this — both modes trust the payload.

The kernel knows the true sender (it executed the ecall on their behalf)
but does not surface this information to the receiver.  `PendingMessage`
records `sender_tid` but `sys_ipc_recv_msg` only returns `(label, w0..w3)`.

## Proposed fix

### Kernel side

Extend `PendingMessage` (already has `sender_tid: u16`) and expose it to
userspace via `sys_ipc_recv_msg`.  The recv ABI gains one return register:

```
Existing: a0=status, a1=label, a2..a5 = w0..w3
New:      a0=status, a1=label, a2..a5 = w0..w3, a6 = sender_tid_with_image_id
```

`a6` packs:
- low 16 bits: `TaskId` of the sender (forgeable by no service — written
  by kernel on rendezvous)
- high 16 bits: sender's `ImageId` (looked up by kernel from the sender's
  TCB)

For services that need only image identity (cap-broker, service-manager),
the high 16 bits give the authoritative answer.  For services that need
exact task tracking (lease/cap delegation), the low 16 bits suffice.

### Userspace wrapper

`fjell-syscall::sys_ipc_recv_msg` returns a sixth element:

```rust
pub fn sys_ipc_recv_msg(ep: u32)
    -> Result<(usize, usize, usize, usize, usize, SenderIdentity), SysError>;

pub struct SenderIdentity {
    pub task_id:  TaskId,    // exact sender
    pub image_id: ImageId,   // class
}
```

### cap-broker change

`crates/fjell-cap-broker/src/main.rs`:

```rust
// BEFORE (v0.2.8):
let requester_id = w0 as u16;

// AFTER (RFC 055):
let requester_id = sender.image_id.0;  // kernel-attested, not from payload
// w0 is now unused for identity; can be repurposed for request nonce.
```

The on-wire protocol changes slightly: `w0` is no longer the requester id.
A request nonce or sequence number could occupy it instead, but for v0.2
the field is simply ignored.

### Sender identity in PendingMessage

`crates/fjell-ipc/src/lib.rs`:

```rust
pub struct PendingMessage {
    pub sender_tid:      u16,         // already exists
    pub sender_image_id: u16,         // NEW
    pub label:           u16,
    pub w:               [u32; 4],
    pub lease:           Option<LeaseBinding>,
    pub is_call:         bool,
}
```

`sender_image_id` is filled by the kernel at message-build time
(`build_msg`).  It is read-only from the kernel's side; userspace cannot
write it.

## Rationale

**Why include both `task_id` and `image_id`?**  TaskId can be recycled
when a task exits; ImageId is stable for a given service binary.  Both
are useful for different purposes.

**Why expose via `a6`, not a separate syscall?**  A separate
`sys_ipc_get_sender()` would be racy — between recv and get, another
recv could intervene (in a service that loops).  Bundling sender
identity into the recv return value makes it atomic.

**Why pack two 16-bit values into one register?**  Conserves register
state; both values fit easily in 16 bits each.  The receiver decodes
trivially with `>> 16` and `& 0xFFFF`.

**Why does cap-broker need image_id and not just task_id?**  Cap-broker
policy is per-service-class (per `ImageId`), not per-task.  Multiple
tasks of the same image (if Fjell ever supports that) get the same
policy.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-ipc` | `PendingMessage::sender_image_id` field |
| `fjell-kernel/cap/syscall.rs` | `sys_ipc_recv_msg` writes `a6`; `build_msg` fills image_id |
| `fjell-kernel/task/tcb.rs` | TCB exposes `image_id()` accessor |
| `fjell-syscall` | `sys_ipc_recv_msg` returns 6-tuple including `SenderIdentity` |
| `fjell-cap-broker` | Read requester from sender, not payload |
| `fjell-sample-service`, `fjell-neg-test` | Update recv call sites (mostly ignore the new field) |

### Backward compatibility

The 6-tuple return breaks every caller of `sys_ipc_recv_msg`.  Same
hardening-line policy as RFC 048/049/052.

### Audit trail

`PendingMessage` audit records (currently `IpcSend`, `IpcCall`,
`IpcRecv`) get `arg1 = sender_image_id` — the existing `arg0` already
carries `sender_tid`.

## Test plan

### Host (unit tests in `fjell-ipc`)

1. `PendingMessage::new` sets `sender_image_id` correctly
2. `build_msg` (or equivalent) reads image_id from the sender's TCB

### QEMU

3. **NEW** `NEG:POLICY:IDENTITY_SPOOFING_REJECTED:PASS` — neg-test
   sends `CAP_REQUEST` to cap-broker with `w0 = STORAGED (10)` (claiming
   to be storaged).  cap-broker reads sender's true image_id from `a6`
   (= NEG_TEST), policy evaluates as NEG_TEST not STORAGED, returns
   `CAP_DENIED` (since NEG_TEST has no policy entry for the targeted
   resource).

This is the test that proves identity attestation works.  Without
RFC 055, this test would *succeed* in spoofing and pass under the wrong
policy entry.

## Implementation notes

- The `a6` register has not been used by any current syscall return —
  free to assign.
- `task/tcb.rs` already stores `image_id` per task (used for binary
  lookup during spawn).  Adding an accessor is a one-liner.
- Consider also recording `sender_image_id` in the reply edge
  (`ReplyEdge`) so that `sys_ipc_reply` can audit replies-to-class.
  Defer to follow-up.

## Open questions

- Should `sys_ipc_call` also surface its receiver's identity to the
  caller?  Useful for clients that want to verify they reached the
  intended service.  Recommendation: yes, add to `sys_ipc_call_words`
  return — but defer to v0.3 unless RFC 056 needs it.
- Should kernel-internal senders (e.g. cap-broker's bootstrap-complete
  send via init's slot 1) have a distinct image_id namespace?  No —
  init is just another task with `ImageId::INIT`.
