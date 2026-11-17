# RFC-0.45-002: Capability installation is made to run — narrowly — and the broker's grant is lease-bound

**Status:** Accepted — accepted 2026-11-17
**Milestone:** 0.45
**Tracks.** **E-123** — `rfcs/done/056-…` says `Implemented` and the kernel rejects its primitive.
**E-011** — narrows it: its live half, the rights validation that has never executed, becomes real.
**Touches** *(indicative)*: `crates/fjell-abi/src/syscall.rs`, `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`, `crates/fjell-kernel/src/task/spawn.rs`,
`crates/fjell-kernel/src/main.rs`, `crates/fjell-kernel/src/audit/ring.rs`,
`crates/formats/fjell-audit-format/src/lib.rs`, `crates/fjell-syscall/src/lib.rs`,
`crates/services/fjell-cap-broker/src/main.rs`, `crates/services/fjell-neg-test/src/main.rs`,
`tests/syscall/expected.toml`, `tests/qemu/profiles/policy.toml`, `tests/abi/snapshot.json`,
prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC 056** (the done RFC this makes true), **RFC-0.42-001** (owner-only lease bind,
reused here), **E-110** (lease slots are never recycled — this line makes it reachable, §Risks),
**RFC-0.45-003** (the instrument that would have caught RFC 056), **E-027**'s class (`BROKER-004`).

---

## What is wrong

The handoff and E-123 list six facts. Reading the whole path — kernel arm, wrappers, grants, broker —
shows **ten absent links**, not one, and only the first is the one the register names. Every row was
read from the tree at `ec74c47`; none of them is visible while the first stands, because the first
makes the rest unreachable.

| # | Absent link | Where, and what it says |
|---|---|---|
| 1 | **No dispatch arm** | `SyscallNumber::CapInstall` (17) has no arm in `trap/syscall.rs`; `syscall-surface`: 36 declared / 32 dispatched / 4 undispatched. Every call is `UnknownSyscall`. |
| 2 | **No capability carries the right the arm would require** | `sys_cap_install` (`cap/syscall.rs:231`) demands `CapKind::CapInstall` **and** `CapRights::CAP_INSTALL` (bit 26). Both `CapInstall` grants — `spawn.rs:985` (cap-broker, slot 10) and `main.rs:1373` (init, slot 27) — use `ALL_NON_META`, and `rights.rs:423` **pins that `ALL_NON_META` excludes `CAP_INSTALL`**. So *had* the arm existed, every call would have returned `PermissionDenied`. |
| 3 | **The ABI cannot carry what the doc-comments promise** | The kernel reads `a0` (install cap), `a1` (target), `a2` (kind), `a3` (object). RFC 056 specified rights, scope and lease; `sys_cap_install_with_rights` takes `rights_bits` and `let _ = rights_bits;`s it (E-011). The kernel installs `ALL_NON_META`, `ObjectScope::Any`, `lease: None` regardless. The wrapper's note — *"cap-broker calls cap_mint after to attenuate rights"* — **cannot work**: `sys_cap_mint` acts on the caller's own CSpace, never the target's. |
| 4 | **The target is an index without a generation, and is never checked** | The broker passes `ipc_sender_tid(identity)` — `(identity & 0xFFFF) as u16`, the index only. The kernel parses a generation, binds it to `_target_id`, and does not read it (`// scope-check deferred (V02-A-005)`); it then calls `ct.cspace_mut(target_idx)` with no liveness or state test. A stale index installs into whichever task now occupies the slot. |
| 5 | **The object is hard-coded to 0 and the kind map fails open** | `main.rs:594-601`: the `object_id` argument is the literal `0`; class 1 → `Endpoint`, class 2 → **`TaskControl`**, `_ =>` **`Endpoint`** (lines 596-598). With the kernel's `ObjectScope::Any` and `ALL_NON_META`, a granted class-2 request would install a `TaskControl` capability over **every** task (start and query today; kill as soon as `TaskKill` is dispatched). |
| 6 | **The broker holds no `LeaseAdmin`, so its revocation path has never been able to work** | `spawn.rs` installs a `LeaseAdmin` for exactly three images (init, `sample-service`, `neg-test`). The broker's `CAP_REVOKE` arm (`tag 0x023`) calls `sys_lease_revoke(11u32, …)` under the comment *"slot 11 = broker's LeaseAdmin"*: **slot 11 is empty**. |
| 7 | **`CAP_REVOKE` has no sender check** | The arm revokes whatever `lease_id` the message names. Harmless today only because row 6 makes it fail. Give the broker a `LeaseAdmin` without fixing this and **any service that can call the broker can revoke any lease in the system** — a confused deputy created by the very change that makes the grant lease-bound. |
| 8 | **`BROKER-004` is defined nowhere** | `main.rs:587-588` cites it; `let _lease_id: u16 = 0; // simplified` discards it; the `DelegationRecord` stores `lease_id: 0`. |
| 9 | **`init` is told to give up an authority it never gives up** | `main.rs:1369-1370`: *"init can delegate caps during bootstrap; revoked after BOOTSTRAP_COMPLETE is sent to cap-broker (v0.3 cleanup)"*. Nothing in `fjell-init` revokes slot 27 (`grep` for slot 27, `cap_revoke`, `sys_cap_drop` finds only the `BOOTSTRAP_COMPLETE` call). `object_zero.rs:34-35` repeats the claim ("`init` -- the only holder of `CapInstall` -- revokes it at bootstrap"). RFC 056 itself says only the broker holds the cap after bootstrap. |
| 10 | **No grant has ever run, so nothing tests one** | The `policy` tier asserts four **denials**. RFC 056's own test plan items 8 and 9 (`NEG:POLICY:GRANT_INSTALLS_USABLE_CAP`, `NEG:CAP_INSTALL:WITHOUT_AUTHORITY_REJECTED`) appear in no source file in the tree. |

**The consumer is named and cannot yet be served — and that is a finding, not a reason to stop.**
`capability-topology` waives `SECURE_TRANSPORTD` slot 0 (`CAP_SESSION`, delivery by the broker). The
policy grants `SECURE_TRANSPORTD` the `SxtSession` class, but **no object exists for that class to
name**: sessions are `netd`'s, and `netd` exits before its loop. So this line makes the *mechanism*
real and exercises it on a requester and an object that exist; it does not make `CAP_SESSION`
arrive. (And, as the handoff says, the release handshake would still fail: it is an unconditional
`Err`.) The waiver's reason is reworded to say what is now actually missing.

## The risk that shapes the design

**Dispatching this syscall is the most authority-bearing change in the milestone**, because it is
the first runtime path by which one task's CSpace is written by another's decision. A compromised or
buggy broker is, by RFC 056's own words, *"nothing in the kernel"* stops it. The design question is
therefore not "how do we make it work" but **"how much can the holder of this capability do, and what
does each refusal protect"**. Two designs were considered for the target.

**Rejected — RFC 056's explicit target (`target_task_id`).** Fixing row 4 means making the broker
name a task and the kernel check generation, liveness and state. That is possible and still leaves
the broker able to write *any* live task's CSpace, whether or not that task asked for anything.

**Chosen — the target is the task being served.** The kernel already tracks exactly this: each
server has at most one **reply edge** (`ReplyEdge { caller_tid, lease }`, invariant IPC-C) created
when a call is delivered and consumed by `sys_ipc_reply`. The install is made **into the caller of
the pending reply edge**, with no target argument at all. Consequences: the broker can install only
into a task that is *blocked in a call to it right now* (and the kernel checks that it is); a stale
or forged target is unrepresentable; and the capability is delivered into the very task whose
request was evaluated. It cannot be used to push authority into a task that never asked. This costs
one non-consuming read of the pending edge (`take_reply` consumes; the install must not, because the
reply still has to be sent).

## Requirements

**R1 — `CapInstall` (17) is dispatched.** The ABI is `a0` install capability, `a1` kind, `a2`
object id, `a3` rights, `a4` lease id; **there is no target argument** (§ above). One wrapper,
`sys_cap_install(install_cap, kind, object_id, rights, lease)`; **`sys_cap_install_with_rights` is
deleted**, not kept as an alias: two wrappers, one of which discards a parameter, *is* E-011.
`tests/syscall/expected.toml` moves to 36 / 33, `undispatched` loses `CapInstall`.

**R2 — the right is granted where the arm requires it, and nowhere else.** The broker's slot-10
capability carries `CAP_INSTALL` (a `CapRights` value, in neither aggregate — the 0.42-001/0.42-002
precedent for a right granted by name) plus the **envelope** of rights the kernel will let it confer
(R3 d). **`init`'s slot-27 `CapInstall` capability is removed**: nothing uses it, RFC 056 says the
broker alone holds it after bootstrap, and a comment promising a revocation that never happens is
the shape this milestone exists to remove. `object_zero.rs`'s sentence about init is corrected in
the same change.

**R3 — the kernel validates, and each check has its own refusal.** In this order:
 a. the caller holds a `CapKind::CapInstall` capability carrying `CAP_INSTALL` — else the existing
    `require_cap` errors (`InvalidCap` / `WrongType` / `PermissionDenied`);
 b. the caller has a pending reply edge whose caller is `Blocked` — else `BadState`;
 c. `kind` is a known `CapKind` (else `InvalidArg`, as today) **and is on the installable list**,
    which in this RFC is `Endpoint` only — every other kind is `PermissionDenied`. (The broker's
    policy names ten resource classes; none of the others has an object for it to point at yet, and
    a kernel allow-list is what bounds a compromised broker. The list grows with a consumer.);
 d. `rights` is non-empty and `rights ⊆ installer_cap.rights & ALL_NON_META` — else `PermissionDenied`;
 e. `lease` is non-zero, **Active, and owned by the caller** (the rule RFC-0.42-001 R2 set for
    `sys_cap_bind_lease`) — else `PermissionDenied` / the lease's own state error. A broker cannot
    bind a capability to a lease it did not create;
 f. the target CSpace has a free slot — else `NoMemory`, and **no existing slot is touched**.
The installed capability has exactly the requested rights, `ObjectScope::Any`, the lease bound at
its current epoch, and `parent: None`.

**R4 — success and every refusal are audited**, as `bind_rejected` was (0.42-001 R6): a new
published `AuditKind` variant (`CapInstalled`, `CapInstallRejected` or one variant with the result
code — the architect's call, §Decisions), `arg0` target task, `arg1` object, `result` the error.
`CapMint` is no longer reused for an install.

**R5 — the broker's grant is genuinely lease-bound; `BROKER-004` becomes a requirement.** The broker
holds a `LeaseAdmin` carrying `LEASE_CREATE | LEASE_REVOKE` (not `LEASE_BIND`: the kernel does the
binding inside the install). Each granted request: create a lease, install bound to it, record the
**real** lease id in the `DelegationRecord`. **`BROKER-004` is written down here** as: *every
capability the broker installs is bound to a lease the broker owns, so that revoking the lease kills
the capability.* It is checkable, and R8 checks it.

**R6 — the grant maps a policy rule to an object, and an unmapped class is refused.** `PolicyRule`
gains the object the rule confers (for installable classes). The class → kind `match` loses its
`_ =>` arm: a class with no defined mapping is `CAP_DENIED` and says so on the console, the way
`RFC-0.41-002` made the default endpoint arm `None`. Class 2 (`TaskControl`) **stays refused** until
there is a scoped design for it (row 5).

**R7 — `CAP_REVOKE` is authorised.** It revokes only a lease **present in the broker's own
delegation table**, and only at the request of that record's grantee or of `init`. (`SERVICE_SHUTDOWN`
being unauthenticated is noted and left: it is not made worse here.)

**R8 — the demonstrations are the deliverable.** The `policy` tier gains grant-path markers and the
mutation table is in the review request. Required cases, **each shown red by mutation**: wrong
rights (`rights ⊄ envelope`); empty rights; no `CAP_INSTALL`; no `CapInstall`-kind capability; an
uninstallable kind; an unknown kind; no pending reply edge; a non-blocked caller; a lease the caller
does not own; a revoked lease; a full CSpace leaves every existing slot unchanged; **grant then
`CAP_REVOKE` kills the installed capability** (use returns `LeaseRevoked` — the check `BROKER-004`
exists to be); and the happy path — the installed capability is usable.

### Decisions I am asking for rather than making

1. **Reply-edge targeting vs RFC 056's explicit target** (§ above). It changes RFC 056's signature.
2. **The installable list is `Endpoint` only.** Wider is a one-line change per kind with a consumer.
3. **The requester used to exercise the happy path.** A new `NEG_TEST` policy rule conferring
   `Endpoint` object `NEG_TEST_OWN_EP_OBJECT` with `SEND` only — a capability to an object it already
   holds with more rights, so the test adds no authority. A new rule is an authority decision.
4. **One audit variant or two.**
5. **Whether `DmaShare`, `MmioUnmap`, `TaskKill` are in scope** — I propose not, as the handoff did;
   nothing here makes any of them trivially correct or wrong.

### Non-goals

- **Delivering `CAP_SESSION`** (§ above) — no object exists.
- **Making the handshake pass.** It is an unconditional `Err`.
- **Recycling lease slots** (E-110) — see §Risks; reported, not fixed.
- **Per-kind scoping of `TaskControl`**, `SERVICE_SHUTDOWN`'s authentication, and the broker's
  reply-`0` for an unknown tag (a `0` that a caller could read as success).

## Risks

- **E-110 becomes reachable, and this RFC says from what.** `lease::create` scans only for `Empty`
  and slots are never recycled (`MAX_LEASES = 32`). A boot already consumes several (lease ids up to
  8 appear in the `audit` profile's trail, from `neg-test` and `sample-service`). **Every granted
  request now consumes one slot permanently, even if the grant is revoked.** About 24 grants per boot
  before `create` fails — and the failure is a refused grant, which is safe, but a node that serves
  a grant per session would stop serving. The recycling item is therefore live the moment a real
  consumer exists; today, with a single test requester, it is not.
- **The broker is a deputy.** `CALL` on its endpoint is the right to ask; R3 (c, d, e) and R7 bound
  what a compromised broker or a hostile caller can do, but a broker that is wrong about *policy*
  still grants wrongly. Nothing in this line audits the policy table.
- **Dispatching changes the existing `policy` tier's meaning.** Its four markers are denials and
  stay so; verify at implementation that no denial was produced by the *missing syscall* rather than
  by policy (the broker maps an install error to `CAP_DENIED`, so such a marker would have been
  right for the wrong reason).
- **The kernel moves**, so the repro baseline is re-recorded in the same commit; the ABI moves
  (syscall surface, `CapKind`/`CapRights`, `AuditKind`) and is predicted in the review request.

## Rulings — architect, 2026-11-17 (`.git-exclude/reviewed/review-record-0.45.md`)

**Accepted. The decisions this RFC asked for are settled here; reasoning is in the review record.**

1. **Reply-edge targeting: accepted**, over RFC 056's explicit target. It makes a stale or forged
   target **unrepresentable** rather than validated — the move 0.44 made with `Option<Digest32>` —
   and bounds a compromised broker to tasks *currently blocked in a call to it*. Changing RFC 056's
   signature is not a cost worth protecting: that RFC's status is already false. **Two additions:**
   * the non-consuming read is a **distinct accessor** from `take_reply` (`cap/table.rs:144`
     consumes), and this RFC states that the install leaves the edge's lifecycle untouched so the
     reply is still sent;
   * `ReplyEdge.lease` is `Option<LeaseBinding>`, `None` for bootstrap/unbound calls. **State that
     the call's own binding is irrelevant** — the install's lease is R3(e)'s, owned by the broker —
     or a later reader will conclude an unbound call cannot receive an install.
2. **Installable list = `Endpoint` only: accepted.** Grow it per kind **with a consumer**, never
   ahead of one.
3. **The `NEG_TEST` policy rule: accepted**, on this RFC's own argument — the capability conferred
   is **strictly weaker** than one `neg-test` already holds, so the demonstration adds no authority,
   which is what distinguishes it from a mechanism kept because it is convenient for a test. **Two
   conditions:** the rule carries its reason **inline in the policy**, not only here; and **name what
   gates the install** — `capability-topology` compares declarations against **spawn-time** installs
   and cannot see a runtime one, so if nothing else gates it, R8's audit assertion is the gate and
   this RFC must say so. The project must not acquire a second kind of install with no comparer.
4. **Two audit variants** (`CapInstalled`, `CapInstallRejected`), not one with a result code.
   `LeaseBindRejected = 0x0604` already stands as its own kind beside plain `CapMint = 11`, and the
   **Readable state** principle is the reason: the ring is read *"by any tool without specialist
   knowledge"*, so a tool must not decode a result field to tell a grant from a refusal.
5. **`DmaShare` / `MmioUnmap` / `TaskKill`: out of scope, accepted.**
6. **R2's removal of `init`'s slot-27 `CapInstall`: accepted, and verified safe** — nothing in
   `fjell-init` references `sys_cap_install` or that slot.
7. **E-110: small item B stays conditional and now has its trigger.** When R5 lands, report whether
   the ceiling is live and from what call pattern. Do not fix it in this line.

**E-123 was amended in the same review**, because the ten absent links showed its closing conditions
were necessary and not sufficient: with the grants as they stand, dispatch plus validation refuses
**every** caller.
