# RFC-0.28-004 §5 — What should the helper layer be, what should the guard check, should `sys_ipc_recv` survive?

**Governing RFC:** [rfcs/accepted/RFC-0.28-004-syscall-helper-contracts.md](../../rfcs/accepted/RFC-0.28-004-syscall-helper-contracts.md)

Per the handoff's required order, this is written before any repair. §5(c) is
answered as an escalation, not a ruling, per the handoff's explicit instruction.

---

## The counts, re-derived

| | RFC claimed | Actual |
|---|---|---|
| Helpers | 4 (`ecall0`/`1`/`2`/`3`) | 4, confirmed |
| `ecall2(` occurrences in the crate | 27 | **27 lines match, but one is the function's own definition** — 26 real call expressions |
| Of those, internal delegation from `ecall0`/`ecall1` | (not distinguished) | **2** (`ecall1` and `ecall0` both route through `ecall2` with `a2=a3=0`) — checked against the kernel handlers for `Yield`, `Exit`, `DebugWrite`, `CapDrop` (the four syscalls that reach `ecall2` this way): all four write **only `a0`** (`crates/fjell-kernel/src/trap/syscall.rs` `sys_yield`/`sys_exit`/`sys_debug_write`; `crates/fjell-kernel/src/cap/syscall.rs` `sys_cap_drop`, each via `ok(tf)`/direct `tf.gpr[REG_A0]` writes only) — safe, not a third site |
| Named `sys_*` functions calling `ecall2` directly | 27 | **24** |
| Of those 24, syscalls writing past `a1` | 2 (`IpcRecv`, `CapInspect`) | **2, confirmed** — verified against every one of the 24 kernel handlers, not assumed from the two named |
| Raw `asm!` blocks in the crate | 6, four via register | 6, confirmed (`ecall2`, `ecall3`, `sys_dma_alloc`, `sys_ipc_call_words` via `in("a7")`; `sys_ipc_recv_msg`, `sys_cap_inspect`'s second block via literal `"li a7, N"`) |

The "27" survives as a literal `grep -c` count (27 lines contain the text
`ecall2(`) but is not 27 *call sites* — it is 26 calls plus the definition
itself, and 2 of the 26 are `ecall0`/`ecall1`'s own plumbing, independently
verified safe rather than assumed so because they were "already covered."

## A finding beyond the RFC's own diagnosis: `sys_cap_inspect`'s second call is not `CapInspect`

**This changes the mechanism, not just the count, and it is more precise
than what the RFC describes — read this before the repair, because it
changes what "repaired" needs to mean.**

`fjell_abi::syscall::SyscallNumber::CapInspect = 14`
(`crates/fjell-abi/src/syscall.rs:32`). The second raw block in
`sys_cap_inspect` issues `"li a7, 13", "ecall"` — **`CapRevoke`, not
`CapInspect`** (`CapRevoke = 13`, same file, line 30). The comment beside it
(`// SyscallNumber::CapInspect`) is simply wrong; `13` is stale, most likely
left over from before `CapRevoke` was inserted into the enum ahead of
`CapInspect`'s current slot — the exact "a number that drifted and nothing
caught it" defect this whole 0.28 arc exists to eliminate, now found one
level deeper, in the crate doing the eliminating.

**Demonstrated live, not reasoned about:** instrumented
`fjell-proxy-text::dispatch_action` (the one real caller) to print the
`rights` value `sys_cap_inspect` returns, ran `cargo xtask qemu-test m8`.
Result: `granted_rights=0x00000448` on every call — which **is** the
correct value (`CapRights::SEND | REPLY | INSPECT` = `(1<<3)|(1<<6)|(1<<10)`
= `0x448`, matching `spawn.rs`'s installed rights for `DEMO_CAP_SLOT`
exactly) — and both `action accepted` and `action DENIED` outcomes were
observed in the same run, meaning today's demonstration is not silently
broken in its observable behaviour.

**Why it "works" is the actual defect.** `CapRevoke`'s kernel handler
(`crates/fjell-kernel/src/cap/syscall.rs:142`) checks
`cap.rights.contains(CapRights::REVOKE)` before touching anything;
`DEMO_CAP_SLOT` is deliberately installed without `REVOKE`
(`spawn.rs`'s own comment: "SEND | REPLY | INSPECT only"), so the erroneous
second call fails closed with `PermissionDenied` — a failure path that
writes only `a0`. The **first** call (the real `CapInspect`, correctly
numbered, via `ecall2`) already wrote the true `rights`/`badge` into the
physical `a2`/`a3` registers as part of its own success path
(`cap/syscall.rs:217-218`: `tf.gpr[12] = rights.0; tf.gpr[13] = badge;`) —
`ecall2`'s asm just doesn't *declare* them as outputs, so the compiler
doesn't know it can read them. The second (failed, wrong-numbered) call
never touches `a2`/`a3` at all, so the real values from the first call
survive in those physical registers by coincidence, and the raw block's
`lateout("a2") r, lateout("a3") b` picks up whatever is there — which today
happens to still be correct.

**That is undefined behaviour appearing correct, not a mechanism.** Nothing
in Rust's `asm!` semantics or this crate's own contract guarantees `a2`/`a3`
survive unperturbed between two separate `asm!` blocks with no declared
relationship between them. It is true today because the two blocks are
adjacent with no intervening code that would give the compiler a reason to
reuse those specific registers — a fact about today's codegen, not a
guarantee.

**And it is worse than "coincidentally correct today" in the direction that
matters:** if any future caller ever inspects a capability that *does* hold
`REVOKE` (an entirely ordinary thing for a broker- or admin-level
capability to hold), this same code path would not fail closed — it would
**actually revoke the capability subtree it was asked only to inspect**, as
a side effect of the wrong syscall number, before returning `Ok` with
values that happened to already be sitting in the right registers.
Confirmed no such caller exists today (checked both other call sites:
`fjell-neg-test`'s two uses either only check `Ok`/`Err` on empty slots, or
deliberately test a cap missing `INSPECT` — the *first* call fails in that
case, so the second call's identity never matters). This has never fired as
a real revocation. It is one `REVOKE`-holding caller away from doing so.

## §5(a) — the helper layer: shape 3

**Two bespoke, correctly-declared raw `asm!` blocks, one per broken syscall,
using the syscall's symbolic `SyscallNumber` constant via `in("a7")` rather
than a literal number** — completing a pattern this crate already uses
correctly three times (`sys_ipc_recv_msg`, `sys_ipc_call_words`,
`sys_dma_alloc` are all bespoke per-syscall blocks; only `sys_ipc_recv` and
`sys_cap_inspect` still route through the generic, under-provisioned
`ecall2`). Shape 3 is not a new pattern for this crate; it is finishing one.

**Rejected — shape 1 (one `a0`-`a6` helper for everything).** "Always safe"
is not free: declaring `a2`-`a6` clobbered on the 24 sites that use only
`a0`/`a1` tells the compiler those registers are dead across every `Yield`,
`TaskStatus`, `CapCopy`, `LeaseCreate`, … call — a real, non-hypothetical
codegen cost (fewer registers available to keep live across a syscall that
does not, in fact, disturb them) paid at every one of those 24 sites, for
zero correctness benefit, to fix exactly 2. Widening because widening
cannot break anything is the reasoning the RFC's own risk section names and
rejects in advance.

**Rejected — shape 2 (widen the per-arity helpers).** Widening `ecall2`
itself imposes the same cost on all 24 safe callers as shape 1, while still
not fitting either broken syscall's actual shape: `CapInspect` needs a
*different return arity* (`a0`-`a3`, four values) than `ecall2` provides
(two), and `IpcRecv` needs `a0`-`a6` (seven). "Widen `ecall2` to cover
`IpcRecv`" is not widening — it is building a third, `IpcRecv`-shaped
helper and calling it `ecall2` anyway.

**The stale-number finding independently confirms shape 3 is the more
defensible choice, not just the smaller diff.** A per-syscall block that
names its syscall symbolically (`SyscallNumber::CapInspect as usize`, not
`13`) cannot silently drift the way the literal did — the compiler would
fail to build if the enum variant were renamed or removed, where a bare
integer literal degrades silently into referring to a different syscall
entirely. Shape 1 or 2's generic helpers take the syscall number as a
runtime `usize` parameter regardless of shape, so this specific protection
is a property of writing the two fixes as named, syscall-specific
functions — which shape 3 already is by definition.

## §5(b) — what the guard should check inside `fjell-syscall`

**The "weakest useful rule": within `crates/fjell-syscall/`, any raw
`asm!` block issuing `IpcRecv`(21), `IpcCall`(22), or `CapInspect`(14) must
declare the full register set that syscall's kernel handler writes.**

Not "every block declares `a0`-`a6`" — that reintroduces shape 1's cost
inside the crate's own already-correct blocks (`sys_ipc_call_words`
legitimately only touches `a0`-`a4`; forcing it to also declare `a5`/`a6`
buys nothing, since `IpcCall`'s own reply path, per RFC-0.28-002's Finding
1, only ever touches `a0`-`a5`). A per-syscall table, owned by the guard,
naming exactly what each of these three writes — mirroring
`SYSCALL-CALLSITE-001`'s existing `registers_requiring_inlateout` table
from RFC-0.28-002, which already does precisely this for the *outside-the-
crate* allowlist. This is the same instrument, extended to look inside the
one crate it previously exempted entirely, not a new one.

**Detection must not rely on the literal `"li a7, N"` pattern alone** — this
crate's own four register-based blocks (`ecall2`, `ecall3`, `sys_dma_alloc`,
`sys_ipc_call_words`) are exactly the blind spot that let RFC-0.28-002's own
regex undercount `fjell-syscall`'s block total by four. The guard's new
`fjell-syscall`-internal check also matches the symbolic form
(`SyscallNumber::<Variant>` appearing as the `a7` operand), which is
precisely why the two repairs below use that symbolic form rather than a
literal — the fix and the check are the same discipline, not independent
choices.

## §5(c) — should `sys_ipc_recv` survive? Escalating, with a recommendation

**Escalated, per the handoff's explicit instruction — not decided here.**
Independent reasoning, not the RFC's stated lean, checked against the
actual call sites:

`sys_ipc_recv`'s three real callers (`fjell-auditd`, `fjell-bootctl`,
`fjell-configd`) use only the tag; none reads a payload word, and
`fjell-configd`'s one branch that would need exact-tag matching
(`CONFIG_GET`) has **no live sender today** — checked, not assumed;
grepping the whole service tree for `CONFIG_GET` finds only its own
definition and its own `match` arm. `fjell-auditd` discards the return
value entirely, treating any arrival on its endpoint as a drain trigger
regardless of tag.

**The information `sys_ipc_recv` throws away — the RFC-055 attested sender
identity — is exactly what two of its three callers would need to defend
themselves against a hazard this project has already found and left open.**
RFC-0.28-001 documented, live, that object 0 has at least three
uncoordinated receivers (`service-manager`, `fjell-auditd`, `fjell-bootctl`
— the same two services calling `sys_ipc_recv` here) racing for the same
messages, and that the fix applied there (giving service-manager its own
object) did not touch auditd's or bootctl's side of that collision, which
is still live. `sys_ipc_recv_msg` returns the sender identity precisely so
a receiver can tell who actually sent a message instead of assuming; a
wrapper whose contract is "discard exactly the field that would let you
notice you're not the intended receiver" is a strange thing to keep
maintaining a second, independently-broken implementation of.

**My recommendation: `sys_ipc_recv` does not earn a permanently-maintained
second contract next to `sys_ipc_recv_msg`.** A future line should migrate
its three callers to `sys_ipc_recv_msg` (a mechanical change — each caller
already discards everything but the tag, or in bootctl's case, already
masks it the same way `sys_ipc_recv_msg` does internally) and remove
`sys_ipc_recv`. This is a lean, not a ruling: removing a function from a
`STABLE_CRATES` crate is an ABI removal, Gate 4 will and should call it
breaking, and that decision belongs to the owner, not to this line.

**This line's own repair does not wait on that decision.** `sys_ipc_recv`
is broken today, independent of whether it has a future — the repair below
fixes its register contract, without changing its signature, discarded
values, or any caller's observable behaviour, so that whichever way §5(c)
is eventually decided, no service is depending on a currently-incorrect
wrapper in the meantime.
