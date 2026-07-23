# Fjell OS — IPC Register Layout

*Normative reference for all IPC syscall register usage.*
*ABI fix recorded as E-010; see `docs/rfcs/ERRATA.md`.*

---

## Overview

Fjell IPC is a synchronous rendezvous model: a caller blocks in
`sys_ipc_call`, a server recv-blocks in `sys_ipc_recv`/`sys_ipc_recv_msg`,
the kernel delivers the message, and the server replies via `sys_ipc_reply`.

All IPC syscalls use the RISC-V ecall convention.

---

## Register contract (all IPC paths)

| Register | Direction | Field | Notes |
|----------|-----------|-------|-------|
| a0 | in | endpoint cap handle (u32) | For call/send: the endpoint to call/send to. For recv: the endpoint to listen on. |
| a0 | out | status (0 = Ok, negative = SysError) | Set by the kernel on return from ecall. |
| a1 | in (call/send) | tag label (usize) | For `sys_ipc_call_words`: packed as `label \| (word_count << 16)`. Word count must be set explicitly. |
| a1 | out (recv) | packed tag (usize) | `label \| (words << 16) \| (caps << 24)` — exactly as packed by `build_msg`. |
| a2..a5 | in (call/send) | message words w0..w3 | Up to 4 words. Only `word_count` words are read from the sender's frame. |
| a2..a5 | out (recv) | message words w0..w3 | Words are placed starting at a2 regardless of how many were sent. Unset words are 0. |
| a6 | out (recv) | kernel-attested sender identity | `(sender_tid as u16) \| ((sender_image_id as u16) << 16)`. Written by `deliver()`; cannot be forged by the sender. |
| a7 | in | syscall number | `SyscallNumber::IpcCall`, `IpcRecv`, `IpcReply`, `IpcTrySend`. |

### What is NOT delivered

The sender badge is no longer written to any register. An earlier
implementation wrote it to a2 and shifted the words to a3..a6, causing
word 3 to collide with the identity write and w0 to always read 0 at the
receiver. The badge had no userspace consumer; it is dropped.

---

## Syscall details

### `sys_ipc_call(ep_handle, tag) → Result<reply_label, SysError>`

Synchronous call. Blocks until the server replies.

```
send:  a0 = ep_handle, a1 = tag (label only, word_count = 0), a7 = IpcCall
recv:  a0 = status, a1 = reply_label
```

For word-carrying calls use `sys_ipc_call_words`.

### `sys_ipc_call_words(ep, tag, w0, w1, w2) → Result<reply_label, SysError>`

Synchronous call with up to 3 payload words.

```
send:  a0 = ep, a1 = tag | (3 << 16), a2 = w0, a3 = w1, a4 = w2, a7 = IpcCall
recv:  a0 = status, a1 = reply_label
```

The word count (3) is packed into tag bits 16–23 by the wrapper. The
kernel's `build_msg` reads this field to know how many words to copy.
Sending the raw label without the packed count results in `tag.words = 0`
and silently drops all payload — this was the v0.20 ABI defect.

### `sys_ipc_recv(ep_handle) → Result<label, SysError>`

Block on `ep_handle`. Returns the message label.

```
send:  a0 = ep_handle, a7 = IpcRecv
recv:  a0 = status, a1 = label
```

### `sys_ipc_recv_msg(ep_handle) → Result<(label, w0, w1, w2, w3, identity), SysError>`

Block on `ep_handle`. Returns label + all four words + kernel-attested identity.

```
send:  a0 = ep_handle, a7 = IpcRecv
recv:  a0 = status, a1 = label, a2 = w0, a3 = w1, a4 = w2, a5 = w3, a6 = identity
```

### `sys_ipc_reply(reply_tag) → Result<(), SysError>`

Reply to the pending reply edge. Non-blocking.

```
send:  a0 = 0 (unused), a1 = reply_tag, a7 = IpcReply
recv:  a0 = status
       BadState     → reply edge was cancelled (e.g. by lease revoke)
       LeaseRevoked → defense-in-depth check: edge's lease was revoked
```

### `sys_ipc_try_send(ep_handle, tag) → Result<(), SysError>`

Non-blocking send. Returns `WouldBlock` if no receiver is waiting.

```
send:  a0 = ep_handle, a1 = tag, a7 = IpcTrySend
recv:  a0 = status
```

---

## Identity field (a6)

On delivery, a6 = `(sender_tid as u16) | ((sender_image_id as u16) << 16)`.

This field is written by the kernel's `deliver()` and reflects the actual
sending task's identity as recorded in the kernel task table. It cannot
be forged by the sender. Receivers that need to validate the sender's
authority (e.g. the cap-broker's policy dispatch) should use a6 rather
than any self-reported identity word in the message payload.

The identity encoding matches the RFC 055 contract.

---

## Lease-bound IPC

When a cap has a lease bound via `sys_cap_bind_lease`, the kernel records the
lease binding in both the outgoing message tag and the server-side reply edge.
On lease revocation:

1. Any sender blocked in the endpoint sendq with a lease-bound cap is cancelled
   and woken with `Err(LeaseRevoked)`.
2. Any reply edge with a matching lease is cleared via
   `cancel_replies_for_lease`, and the blocked caller is woken with
   `Err(LeaseRevoked)`.

`sys_ipc_reply` checks the reply edge's lease binding (defense-in-depth):
if the lease is revoked, it returns `Err(LeaseRevoked)` rather than delivering
the reply. This ensures correct behavior even if the revocation notification
is delayed or raced.

---

## Historical note — E-010

Prior to v0.20.0, this ABI was broken in two independent ways:

```
Defect A (sender): sys_ipc_call_words did not pack the word count
  → tag.words == 0 → build_msg copied 0 words → all payload lost

Defect B (receiver): deliver() wrote badge to a2, words to a3..a6
  → receiver read badge (always 0) as w0
  → word 3 was overwritten by the identity write at a6
```

Both defects were introduced with the IPC protocol and survived undetected
until v0.20.0, when the fail-closed negative-test harness (RB-01) exposed
that the ipc lease-revocation scenarios had been exercising an instant-failure
path rather than the real blocking protocol.

See `docs/rfcs/ERRATA.md` §E-010 for the full errata record.

---

## ABI stability

The IPC register layout is covered by the ABI stability commitment in
RFC-v0.10-002. Changes require an RFC with an architect decision record.
The layout above is normative as of v1.0.0.

