# External Design — IPC

*Subsystem 3 of 9. Anchored to FR-KRN-004, NFR-PERF-001 and `fjell-ipc` +
`fjell-kernel/src/trap/syscall.rs` at v0.21.2.*

## 1. Responsibility

IPC is the only general communication primitive in Fjell (there is no POSIX
surface). It provides safe, capability-checked, typed message passing between
user-space services and is the substrate for the entire service plane.

## 2. External surface

### Model

Synchronous rendezvous. A caller blocks in `sys_ipc_call`; a server blocks in
`sys_ipc_recv` / `sys_ipc_recv_msg`; the kernel delivers the message; the server
replies via `sys_ipc_reply`. One-way `sys_ipc_send` is the same rendezvous
primitive without a reply: it blocks the caller until a receiver takes the
message, not a buffered post (RFC-0.27-002, closes E-022 — this call was
named `sys_ipc_try_send` and documented as non-blocking until then, which
the kernel never implemented). A genuinely non-blocking receive exists
(`sys_ipc_try_recv`, a distinct syscall number); no non-blocking send does.

### Register ABI (normative)

The full contract is in [IPC Register Layout](../abi/ipc-register-layout.md).
Summary:

| Register | Meaning |
|---|---|
| a0 | endpoint cap handle (in) / status (out) |
| a1 | packed tag: `label \| (word_count << 16) \| (caps << 24)` |
| a2..a5 | message words w0..w3 |
| a6 | kernel-attested sender identity (out, on recv) |
| a7 | syscall number |

`sys_ipc_call_words` packs the word count into tag bits 16–23; `deliver()`
writes words to a2..a5 and the sender identity to a6. The identity is written by
the kernel and cannot be forged by the sender — a service validating a caller's
authority uses a6, not any self-reported payload word.

## 3. Requirements coverage

| Requirement | Design response | As-built evidence |
|---|---|---|
| FR-KRN-004 Capability-checked send/receive | Endpoint cap with `SEND`/`RECV`/`CALL`/`REPLY` right required | `require_cap_on_ct` on IPC syscalls |
| FR-KRN-004 Clear message boundaries | Fixed 4-word payload + packed tag; no streaming ambiguity | register ABI |
| FR-KRN-004 Typed messages | Tag `label` carries the message type; services define catalogs | per-service protocol constants |
| FR-KRN-004 Low-copy communication | Register-passed words for small messages; shared regions via caps for bulk | ABI + `MmioRegion`/`DmaRegion` caps |
| FR-KRN-004 Verifiable structure | Rendezvous + reply-edge model; lease-cancellation proven bounded | `lease/mod.rs`, Verus lease target |
| NFR-PERF-001 Low-overhead IPC | Register-passing for the common small-message path | `sys_ipc_call_words` |

## 4. Kernel-attested identity (external contract)

On delivery, a6 = `(sender_tid as u16) | ((sender_image_id as u16) << 16)`. This
is the trust anchor for server-side authorization: because the kernel writes it
from the live task table, a caller cannot impersonate another task. This backs
FR-SEC-002 (explicit delegation) at the IPC layer.

## 5. Lease-bound IPC

When a capability is lease-bound (`sys_cap_bind_lease`), the kernel records the
binding in the outgoing tag and the server-side reply edge. Revoking the lease
cancels blocked senders and pending reply edges (see
[Capability & Lease](./capability-lease.md) §5). This is the mechanism that
makes authority revocation propagate into in-flight communication.

## 6. Historical note

Prior to v0.20.0 the IPC words ABI was defective (word count not packed; badge
collided with payload/identity). This was found when the negative-test harness
became fail-closed, fixed as errata E-010, and the register layout was made
normative. The lesson recorded in the design: formally verifying adjacent pure
logic does not prove the runtime protocol plumbing — the plumbing needs
fail-closed runtime tests.

## 7. As-built scope limits & gaps

- Payload is bounded to 4 words inline; bulk transfer uses a shared region
  granted by capability rather than large in-band messages.

  **Correction (RFC-v0.23-001):** the shared-region mechanism this describes
  is `DmaShare` (syscall 111) — one of the nine declared-but-undispatched
  syscalls (`crates/fjell-abi/src/syscall.rs` declares it;
  `crates/fjell-kernel/src/trap/syscall.rs` does not dispatch it). It is
  **not currently available**. RFC-v0.23-001 needed to transfer
  kilobyte-scale `SemanticEnvelope` values (`fjell-semantic-format`) between
  services and, lacking the documented mechanism, used chunked byte
  transfer instead — the same shape as `storaged`'s
  `WRITE_BEGIN`/`WRITE_CHUNK`/`WRITE_COMMIT` protocol, 32 bytes (4 inline
  words) per round trip (`fjell-service-api::chunked`). This is a
  demonstration-scale workaround, not a designed transport:
  `SemanticEnvelope` is 4936 bytes, so one chunked transfer is 157 round
  trips (`BEGIN` + 155 `CHUNK` + `COMMIT`), and the full path — one
  emission plus one forward hop — is 314. The
  documented shared-region path remains the intended mechanism once the
  nine-syscall disposition (deferred to v0.22+, still open) resolves
  `DmaShare`.
- No zero-copy page-remap fast path yet (FR-KRN-004 allows "room for" it; not
  built at v1.0). Small-message register passing is the implemented low-copy
  form.
