# RFC-0.28-001 §5 — Should `init` wait for readiness at all?

**Governing RFC:** [rfcs/accepted/RFC-0.28-001-readiness-topology.md](../../rfcs/accepted/RFC-0.28-001-readiness-topology.md)

Per the handoff's required order, this is written before any topology code
changed. §3's arithmetic is re-derived first, because it changes what "wait
for readiness" even means for four of the services involved.

---

## §3 — the arithmetic, re-derived

The RFC's own count: *"14 images, 9 with dedicated endpoints, at most 5 can
reach service-manager, threshold 10, therefore unreachable."* Checked
directly against the source rather than the table alone. It undercounts the
depth of the problem, not its conclusion.

**Only 4 services are self-deadlocking-if-`init`-stops-waiting, not 9.** Of
the 9 dedicated-endpoint holders in `spawn.rs`'s `ep_obj` table
(STORAGED, MEASUREDD, ATTESTD, RECOVERYD, CAP_BROKER, SAMPLE_SERVICE,
SEMANTIC_STREAM, PROXY_TEXT, DRIVER_UART):

- **CAP_BROKER and DRIVER_UART never send a readiness signal at all** —
  grepped for `SERVICE_READY` and `send_ready` project-wide; neither
  appears in either service.
- **SEMANTIC_STREAM and PROXY_TEXT's `send_ready()` calls were already
  deleted** — by RFC-0.26-004, for exactly this self-deadlock reason, on
  those two objects specifically. Their source now carries only a comment
  explaining the removal.
- **SAMPLE_SERVICE is already patched around it** — `spawn.rs` installs an
  *extra* capability (CSpace slot 2, object 0) specifically so its
  `SERVICE_READY` reaches service-manager despite its dedicated endpoint
  being object 6. Live-verified below: this patch does not currently work
  either, for an unrelated reason.
- **STORAGED, MEASUREDD, ATTESTD, RECOVERYD remain — 4, not 9.**

**Those 4 do not use the RFC-058 protocol at all — a deeper problem than
routing.** Each defines its own local `send_ready()` sending its own
distinct tag to `EP_SLOT = 0` (its own dedicated object):
`storaged_proto::READY = 0x210`, `measuredd_proto::READY = 0x200`,
`attestd_proto::READY = 0x300`, `recoveryd_proto::READY = 0x310`. None of
these equals `tags::SERVICE_READY = 0x001`. Service-manager's match
(`tag == (tags::SERVICE_READY & 0xFFFF)`) would never recognise any of
them **even if routing were fixed**. These four run a separate, older,
per-service protocol whose only consumer has ever been `init`'s own
`wait_storaged_ready`/`wait_service_ready` — not RFC 058's tracker. Fixing
"where slot 0 points" alone, as D2 already forbids for a different reason,
would also leave this tag mismatch untouched.

**Only 4 images attempt the generic protocol today, and one of those four
is silently broken.** Grepped for literal `tags::SERVICE_READY` /
`READY_TAG = 0x0001` usage:

| Image | Route | Works? |
|---|---|---|
| SAMPLE_SERVICE | patched slot 2 → object 0 | Syscall succeeds (`Ok`) — see live trace below for where the message actually goes |
| VERIFYD | `EP_SLOT = 0`, not in `ep_obj` table → object 0 | Works |
| NETD | `CAP_SMGR_EP = CapHandle(2)` | **Never installed anywhere in `spawn.rs`.** `netd`'s own header comment documents "slot 2 — Endpoint to service-manager" as its intended design; nothing makes that true. `check_right` fails closed; the send never leaves `netd`. Silent — no hang, no error print, because the raw `asm!` discards the return value. |
| NEG_TEST | slot 0, not in `ep_obj` table → object 0 | Works (used for the identity-spoofing test) |

**A finding the RFC does not mention: object 0 has at least three
uncoordinated receivers today, not one.** `fjell-auditd` calls
`sys_ipc_recv(0u32)` — its own comment: *"Other services (or init) send a
message to endpoint 0 to trigger a drain"* — treating *any* arrival as a
trigger, never inspecting the tag. `fjell-bootctl` calls
`sys_ipc_recv(SLOT_OWN_EP)` with `SLOT_OWN_EP = 0` for its own reboot
protocol. Neither image is in the `ep_obj` table, so both default to
object 0 — the same object service-manager listens on. **Live-verified**
(instrumented every arrival at service-manager and every send's own return
code, ran `cargo xtask qemu-negative svc`):

```
TEMPDIAG:sample-service:send_ready:Ok
TEMPDIAG:neg-test:send_ready:Ok
TEMPDIAG:service-manager:recv:sender_img=14:tag=0x0001
TEMPDIAG:netd:send_ready:result_a0=<InvalidCap>
```

Exactly **one** message ever reached service-manager — VERIFYD's
(image 14). SAMPLE_SERVICE's and NEG_TEST's sends both returned `Ok`
(genuinely delivered to *some* receiver) but neither arrived at
service-manager: they were consumed by whichever of {service-manager,
auditd, bootctl} happened to call `recv` first, which is not
deterministic and is not service-manager reliably. **This is the same
one-receiver-invariant hazard E-021/E-024 already document, live today on
the "shared" object itself, not only on the four per-service ones the RFC
names.** Reported here rather than filed as a fifth erratum, because the
fix below removes the collision at its root (service-manager gets its own
object) rather than needing a separate patch.

**Net effect: the real reachable count is lower than the RFC's own "at
most 5", not higher.** Even granting every current sender its best case,
at most 3 messages (SAMPLE_SERVICE, VERIFYD, NEG_TEST) could ever land at
service-manager, and today only 1 reliably does. `n_ready >= 10` was
already unreachable by the RFC's count; it is unreachable by a wider
margin once the tag mismatch and the object-0 collision are accounted for.

---

## 0.1 — the hang, reproduced and explained

**Reproduced live**, per the handoff's mandate, before writing any fix:
commented out `init`'s `wait_storaged_ready(2)` call, rebuilt, ran
`cargo xtask qemu-test m8`. Result: **total, permanent boot hang.** The
serial log's last two lines are:

```
TEMPDIAG:init:skipped_wait_storaged_ready
M6: storaged ready
```

Nothing after this point ever printed; the run hit the 60s timeout.

**Which task, and why:**

- `storaged`'s `send_ready()` sends `0x210` to its own dedicated object
  (1) with no receiver waiting (init no longer calls
  `wait_storaged_ready` there) — rendezvous IPC blocks the sender, so
  `storaged` suspends permanently before reaching its own receive loop.
- `init`'s very next action is `storaged_write(storaged_ep, ...)` — a
  blocking `ipc_call` to the same object. `storaged` can never reach its
  receive loop to answer it, so `init` blocks too, immediately after.
- `init` is the sole spawner of every later service (M6 onward, M7, M8,
  the v0.4/v0.7 plane, `driver-uart`). Once `init` itself is blocked,
  **nothing after this point in the boot sequence is ever spawned at
  all** — not a hang in one corner of the system, a hang of the entire
  boot past this line.

**Why it completes today:** not luck — a deliberate two-party rendezvous.
`init` spawns `storaged`, then *immediately* calls `wait_storaged_ready`,
which is a blocking receive on the same object `storaged` sends to.
Whichever of the two reaches the object first, the other's arrival
completes the rendezvous. This works *only* because `init`'s spawn-then-
wait ordering guarantees it is present as receiver at exactly the moment
`storaged` needs one — which is also, precisely, the E-024 hazard: a
second receiver, undocumented as load-bearing, is what stands between
today's working boot and the hang just reproduced.

**What the discard loop was eating:** instrumented both `wait_storaged_ready`
and `wait_service_ready` to log every tag they discard. **Nothing.** Across
a full `svc`-profile boot, neither loop ever received a tag other than the
one it was waiting for. The missing-`else` is a live structural hazard
(anything arriving out of the expected shape would be silently dropped,
exactly as E-021 was), but it has not yet been triggered by anything this
project currently runs. Recorded as confirmed-latent, not confirmed-firing
— a materially different finding from "already biting," and worth stating
precisely rather than rounding up to match the more dramatic reading.

---

## Answer: shape 1, argued from the corrected picture, not adopted on the architect's inclination

**`init` stops receiving directly on any per-service object for readiness
purposes.** Given what §3 found, this is not merely the tidier design —
it is close to the only design that can be made to work at all:

- The current arrangement already has service-manager (RFC 058's intended
  single tracker) actively losing messages to two other, unrelated
  receivers on the same object. Shape 3 (keep per-service waits, permit a
  documented co-receiver) would have to bless *that* too, or leave it
  broken while re-blessing the narrower four-object version — an
  inconsistent standard for the same defect.
- Shape 2 (move `init`'s waits onto service-manager's endpoint, keep them
  otherwise as-is) still leaves four services on a protocol
  service-manager cannot parse (the tag mismatch), so it would need
  either widening service-manager's match to four more magic constants
  (an ad hoc growth of exactly the kind D2 forbids for the *routing* side)
  or migrating those four senders anyway — at which point it has become
  shape 1 with extra steps.
- Shape 1 gives the one-receiver invariant a chance to be **true** rather
  than restated: once `init` holds no receive right on objects 1-4
  (narrowed to `CALL`, the same narrowing RFC-0.26-004 already applied to
  object 7), each of those four objects has exactly one receiver — the
  service itself — for the first time.

**What it costs, checked rather than assumed:** the handoff's own cost
note anticipated a boot-sequence reshape. It is not required. `init`'s
existing wait *sites* stay in the same places, in the same order — a
storaged-specific wait in M6, three M8 waits for measuredd/attestd/
recoveryd — because each site now blocks on a **relay** from
service-manager instead of a **direct** receive from the service, over a
dedicated, single-purpose object nothing else ever sends on. Boot ordering
is unchanged; only who receives what changes.

## What this answers back to RFC-0.27-002 §4

**Checked, not assumed, per the instruction.** RFC-0.27-002 §4 asked
whether a genuinely non-blocking one-way send should exist, on the
evidence of six services wanting to "announce and continue." Of that six,
two (`semantic-stream`, `proxy-text`) already had their sends deleted as
dead code by RFC-0.26-004; the remaining four are exactly this RFC's
`storaged`/`measuredd`/`attestd`/`recoveryd`. Once they send to a task
that is *already sitting in a receive loop* (service-manager, via the new
dedicated object, from the moment it starts near M4) rather than to their
own endpoint (where no one is receiving until `init`'s wait arrives),
their sends rendezvous immediately — `Delivered`, not `Queued` — every
time. **The motivating need was an artifact of the topology, not a gap in
the send primitive.** RFC-0.27-002 §4 stands answered: no, a non-blocking
send is not needed for this use case; a correctly-addressed blocking one
already behaves like one in practice, because the receiver is already
waiting.

## A second, independent bug found during bring-up: `a6` register clobber

Not part of the topology design above — discovered only once the shape-1
mechanism was implemented and tested, and disclosed here rather than
quietly fixed, per the same instinct that put the object-0 finding in §3
rather than filing it separately.

**Symptom:** after the topology change (new endpoint objects allocated,
relay wired end-to-end), `cargo xtask qemu-negative svc` failed
non-deterministically — `n_ready` stuck at 7, one short of the
re-derived threshold of 8. Instrumented and found: service-manager's
relay `sys_ipc_send` for `attestd`'s readiness (the third of the three
M8 relays) printed a "before" trace but never an "after" — the send
never returned. Six consecutive builds reproduced this identically; six
more with an unrelated `inlateout`/`in` register-constraint fix applied
(a genuine bug, described below, but not this one) reproduced it
identically again. Adding extra `sys_debug_write`/`writeln` calls inside
`init`'s relay-receive loop made the failure disappear, reproducibly, in
every run tried — the opposite of what a real fix should look like, and
exactly the kind of unexplained, timing-shaped "fix" this project's
RFC-0.24-through-0.28 arc exists to catch rather than ship.

**Root cause, found by reading `crates/fjell-ipc/src/endpoint.rs` and
`crates/fjell-kernel/src/cap/syscall.rs`'s `sys_ipc_recv`/`deliver`, not
guessed:** every successful IPC delivery — one-way sends included, per
RFC 055 — writes the kernel-attested sender identity into register `a6`
(`deliver()`, `tf.gpr[16]`). The established, correct syscall wrapper
(`fjell_syscall::sys_ipc_recv_msg`) declares `lateout("a6") sender` for
exactly this reason. `fjell-init`'s two hand-rolled `IpcRecv` asm blocks
(`wait_relay_exact`, `wait_relay_all_m8_ready` — written for this RFC,
not pre-existing) omitted `a6` from the clobber list entirely. This
tells the compiler `a6` survives the `ecall` unchanged, so it is free to
keep loop-carried state (any of `m`/`a`/`r`/`tag`, or something else live
across the loop) in `a6` — which the kernel then silently overwrites on
every relay delivery. The result was non-deterministic depending on
what the compiler happened to allocate to `a6`, and specifically
manifested as `wait_relay_all_m8_ready`'s three-way loop exiting after
only two of the three relays landed, leaving service-manager's
already-queued third relay send permanently undelivered (`init` never
issued the receive that would have drained and woken it) — a genuine
kernel/ABI-contract bug in userspace code, not a scheduler or IPC-layer
defect. Fixed by adding `lateout("a6") _` to both blocks; confirmed by
13 consecutive clean runs (5× `qemu-test m8`, 8× `qemu-negative svc`)
with every temporary diagnostic removed, plus a clean 21/21
`test-all` and all 12 `release-rehearsal` gates.

**Separately, an independent and lower-severity fix found and applied
during the same debugging pass:** two new functions written for this
RFC used `in("a0") ep => _` for the endpoint-slot argument instead of
the established `inlateout("a0") ep => _` pattern (the kernel writes its
return status into `a0`; declaring it a plain unclobbered input violates
the asm constraint contract). Fixed to match the existing convention.
Testing before and after this specific fix (six runs each, both still
exhibiting the `a6` symptom) confirms this bug, while real, was not the
cause of the flaky hang above.

## Observation versus judgement

- **Observation:** the 4-vs-9 count, the tag mismatch, the object-0
  collision, and the confirmed hang are all directly checked — grep,
  source reading, and a live instrumented boot, not inference.
- **Observation:** removing `init`'s `wait_storaged_ready` call reproduces
  a total boot hang; this is measured, not predicted.
- **Observation:** the `a6` clobber bug and its mechanism were confirmed
  by reading `fjell-ipc`'s and the kernel's actual `recv`/`deliver` code,
  not inferred from the symptom alone; the fix was verified by 13
  consecutive clean runs plus a full clean `test-all`/`release-rehearsal`
  pass, not assumed from a single success.
- **Judgement:** that shape 1 is the right response to what was found —
  argued above from the corrected picture, not from the RFC's original
  framing, which the picture only partially matches.
- **Judgement:** the specific mechanism (a dedicated relay object from
  service-manager to `init`, preserving today's wait *sites* and *order*)
  is a design choice among several that would satisfy shape 1; it is
  argued for on the grounds of minimising boot-order risk (the RFC's own
  named risk), not derived from the RFC's text.
