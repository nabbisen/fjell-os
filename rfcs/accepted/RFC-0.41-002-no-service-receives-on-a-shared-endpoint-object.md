# RFC-0.41-002: No service receives on a shared endpoint object

**Status:** Accepted — accepted 2026-10-30
**Milestone:** 0.41
**Tracks.** **E-102**, its latent half — twenty images hold slot 0 against endpoint object 0, each
with `SEND`; two tasks run a receive loop on it; nothing sends to it today, which is the only
reason the race has never fired.
**Touches** *(indicative)*: `crates/fjell-kernel/src/task/spawn.rs`,
`crates/services/fjell-netd/src/main.rs`, `crates/services/fjell-sample-service/src/main.rs`,
`crates/fjell-abi/src/service.rs`, `docs/src/` (capability topology),
`tests/qemu/profiles/`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.41-001**, which must land **first** — it removes `auditd` from object 0 as
a consequence of its redesign, and this line must not race it; **RFC-0.28-001**, which fixed
exactly this shape for `service-manager` by giving it object 10 and left the rest sharing 0;
**RFC-0.33-001 D8**, which gave `bootctl` its own object.

## What is wrong

`crates/fjell-abi/src/service.rs` declares **35** `ImageId`s. The `ep_obj` match in
`crates/fjell-kernel/src/task/spawn.rs:272` has **15** dedicated arms and ends `_ => 0`. So
**20 images are handed slot 0 pointing at endpoint object 0**, each installed with
`CapRights::ALL_NON_META` — `(1 << 26) - 1`, which includes `SEND` (`1 << 3`).

Exactly **two** tasks run a receive loop on that object:

| Task | Site | Call |
|---|---|---|
| `fjell-auditd` | `src/main.rs:105` | `sys_ipc_recv(0u32)` |
| `fjell-netd` | `src/main.rs:79` | `sys_ipc_recv_msg(0)` |

**Nothing sends to object 0 today, which is the only reason this is latent.** That now rests on a
full enumeration of every `sys_ipc_send`/`sys_ipc_call`/`sys_ipc_call_words` site in
`crates/services/` resolved against the `ep_obj` table, not on one grep: 14 readiness sends to
object 10, 3 `init` sends to named slots, `diagnosticsd`'s `send_tag` to slots 1–2,
`fjell-sample-service` to its **own dedicated object 6**, `neg-test` to named peer slots, and
`bootctl` to its health slot.

**The day anything sends to object 0, `auditd` or `netd` may win the receive race and take a
message meant for the other.** Two receivers on one object is not a latent bug in one service; it
is a topology that cannot be reasoned about locally. `netd`'s own comment calls slot 0 *"the netdev
endpoint"* (`src/main.rs:78`) — it is not; it is the shared default, and the comment reads as
though `netd` owns it.

**The defect is the shared default grant, not the race.** A default that silently points twenty
images at one contested object is a mechanism kept because removing it is awkward. Every fix that
only separates the two current receivers leaves the next service to inherit the same default.

## Requirements

**R1 — land after RFC-0.41-001.** That line removes `auditd`'s receive on object 0 entirely, as a
consequence of its wake redesign. Starting here first would mean building a separation for a
receiver that is about to disappear.

**R2 — `netd` gets a dedicated endpoint object.** Follow the shape RFC-0.28-001 used for
`service-manager`: a named constant, a dedicated arm in the `ep_obj` match, and the service
receiving on its own object. Replace the *"the netdev endpoint"* comment with one that is true.

After R1 and R2, **no task receives on object 0.** State that as the property, and give it a
check — see R5.

**R3 — `_ => 0` stops granting an endpoint.** The default arm must no longer hand out a capability
to a shared object. An image with no dedicated endpoint gets **no endpoint capability** in that
slot.

This is the requirement that stops the defect recurring; R1 and R2 only make the present instance
safe. Expect it to be the work: twenty images currently receive this grant, and each must be
checked for whether it uses it. **If any genuinely needs an endpoint, it earns a dedicated arm
like every other service** — it does not get the shared one back.

**R4 — report what the twenty actually used.** The enumeration above says nothing *sends* to object
0; it does not establish that no image *relies* on holding the capability (a slot-0 presence
check, an unconditional install, a test fixture). Before R3 is committed, enumerate per image
whether slot 0 is read at all, and record the result in the handoff. **A text search is a
hypothesis here**: resolve it against the running system where the question is behavioural, and
say which images were checked by which means.

**R5 — a check, so this cannot silently come back.** The invariant is "no two tasks receive on the
same endpoint object", and the stronger one R3 establishes is "no task receives on object 0". Add
it where the topology is already checked — `consistency-check`'s `capability-topology` is the
natural home, which today reads 6 installed / 1 waived / 7 checked and will move with this line.
A *failing* case must be shown: construct the two-receiver shape and watch the check reject it,
then remove it. Absence of a complaint on the fixed tree is not evidence that the check works.

**R6 — the stale comment in `fjell-sample-service`.** It still names *"auditd/bootctl's
shared-object-0 receive loops"*. `bootctl` has had its own object since RFC-0.33-001 D8, and after
RFC-0.41-001 `auditd` has none — so the comment is wrong today and wronger after. Fix it to say
what is true: the send goes to the service's **own dedicated object 6**, which is why it is safe.

### Non-goals

* **Collapsing the endpoint model.** This line does not redesign how endpoints are named or
  allocated. It removes a shared default and gives one service its own object.
* **Changing `CapRights::ALL_NON_META`.** The rights granted on a dedicated endpoint are out of
  scope; the defect is *which object* the capability points at, not which rights it carries.
* **`auditd`'s trigger.** RFC-0.41-001 owns it.

## Risks

**R3 is the step that can break a service quietly.** An image that reads slot 0 and gets nothing
may fail in a way that looks unrelated — a receive that returns an error the service ignores, or a
startup path that waits forever. R4 exists to find these before the grant is removed, and the
tiers must be run per image rather than in aggregate.

**A service that loses the grant may fail *later* than boot**, so a green boot is not sufficient
evidence. Run the profiles that exercise each affected service, and say in the record which
services were exercised and which were only built.

**`capability-topology`'s waiver line.** It currently waives one declared-but-not-installed slot
(DIAGNOSTICSD slot 2) to a named line. Do not widen that waiver to cover anything this line
removes; a waiver that absorbs new absences stops being a record of one accounted-for exception.

**Prebuilts will move** for `netd`, `sample-service` and the kernel, and for any image whose
slot-0 handling changes. The repro baseline is re-recorded in the same commit as the rebuild.

## Amendments — 2026-10-31, architect, after review

**The receiver figure in "What is wrong" was wrong in both directions, and the error is the
architect's.** It said exactly two tasks receive on object 0, `auditd` and `netd`. In fact:
**`configd`** is a real serving loop that was missed because it binds the literal first (`let ep =
0u32;` at `src/main.rs:64`, `sys_ipc_recv(ep)` at `:79`, invisible to a `recv(0…)` search);
**`netd`**'s receive is **dead code** (`run_event_loop` at `src/main.rs:169` is never called —
`service_main` reaches `sys_exit(0)` at `:154`); and **`neg-test`** receives on *copies* of its slot 0.
Had R1 and R2 been implemented as written, `configd` and `neg-test` would still have been receiving on
object 0 while the invariant read as established. E-102 carries the same correction, dated.

**`init` is granted in `main.rs`, not through `spawn.rs`'s default**, so R3 as written would have left
a `RECV` on object 0 in `init`'s CSpace. That install is removed.

**R5's invariant is corrected.** "No two tasks receive on the same endpoint object" is **false of this
tree by design**: the update channel (`secure-transportd` ↔ `upgraded`) and the tools channel
(`fleetd` ↔ `diagnosticsd`) are deliberate bidirectional rendezvous with `SEND|RECV` on both sides,
and many one-way senders hold `ALL_NON_META`, which carries an unused `RECV`. The invariant this line
establishes is R3's sentence — **no task can receive on object 0** — which is the defect. The stronger
one needs the rendezvous objects named as exceptions and the over-grants narrowed first; recorded as a
0.42 candidate, not part of this line.
