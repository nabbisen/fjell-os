# RFC-0.28-002 §6 — What exactly should the guard forbid?

**Governing RFC:** [rfcs/done/RFC-0.28-002-syscall-asm-consolidation.md](../../rfcs/done/RFC-0.28-002-syscall-asm-consolidation.md)

Per the handoff's required order, this is written before any swap, informed
by the per-site audit below (the audit had to happen first to know whether
shape 1 is even survivable — it isn't, and that is itself the answer).

**Evidence (D5):** post-consolidation, clean (`tree_dirty_at_run_time =
false`) `qemu-negative svc` run, all four markers present including
`READY_ACCEPTED` and `UNAUTHORIZED_READY_REJECTED`:
[`tests/evidence/RFC-0.28-002/svc-post-consolidation.log`](../../tests/evidence/RFC-0.28-002/svc-post-consolidation.log).
This is one of 16 consecutive clean runs against the final tree (8×
`qemu-test m8`, 8× `qemu-negative svc`), on top of a clean 21/21
`test-all` and every per-crate swap's own verification run along the way.

---

## The counts, re-derived

| | RFC claimed | Actual |
|---|---|---|
| Raw syscall `asm!` blocks in `crates/` | 37 | **41** |
| …in `fjell-syscall` | 2 | **6** |
| …hand-rolled elsewhere | 35 | **35** (same total, different shape) |
| Distinct syscalls issued by the 35 | 13, 20, 21, 22, 23 | **20, 21, 22, 23 only** — 13 (`CapInspect`) appears only inside `fjell-syscall`'s own `sys_cap_inspect`, never in a service |
| Crates containing the 35 | 11 named | **14**: the 11 named, plus `fjell-init` (3), `fjell-service-api` (1), and `fjell-driver-virtio-net` (1) — all three anticipated by the RFC's own "and any sibling the audit turns up" |

`fjell-syscall`'s other 4 blocks (`ecall2`, `ecall3`, `sys_dma_alloc`,
`sys_ipc_call_words`) set `a7` via `in("a7") nr` rather than the literal
`"li a7, N"` the RFC's regex searched for — the same blind spot the handoff
warned its own count could have ("a block that builds its syscall number
differently is invisible to it"). Confirmed by reading, not re-guessed.

**`crates/fjell-kernel/src/task/user_image.rs` is excluded, correctly.** It
contains literal encoded instruction bytes (`0x93, 0x05, 0x10, 0x00, // li
a7, 22`) for synthetic test task images, not a `core::arch::asm!` block —
zero real matches. Confirms the RFC's own non-goal ("does not touch the
kernel") without needing to touch it.

## Why shape 1 does not survive contact with the audit

Auditing all 35 sites (table below) found **seven that do not have a clean
wrapper to call**, not because a wrapper is missing entirely, but because
the existing wrapper for that syscall covers a narrower shape than the site
needs:

- **`IpcCall` (22), 4 data words needed, `sys_ipc_call_words` covers 3:**
  `fjell-init::ipc_call`, `fjell-service-api::chunked::ipc_call4`,
  `fjell-proxy-text::ipc_call_action` — three independent local
  reimplementations of the same missing capability, discovered
  independently by three different authors at three different times. That
  is not noise; it is three votes for the same gap.
- **`IpcReply` (23), 3 data words needed, `sys_ipc_reply` covers 0:**
  `fjell-measuredd::reply`, `fjell-recoveryd::reply`,
  `fjell-semantic-stream::reply`, `fjell-proxy-text::reply`. The kernel's
  `sys_ipc_reply` (`crates/fjell-kernel/src/cap/syscall.rs`) unconditionally
  copies all four of the replier's `a2..a5` into the caller's trap frame on
  every reply, regardless of word count — there is no way to send reply
  data through today's wrapper at all.

**Shape 1 (refuse raw syscall `asm!` outside `fjell-syscall`, full stop)
would fail its own guard on code this RFC is not authorized to delete and
cannot make wrapper-shaped without touching `fjell-syscall`** — which the
RFC's own non-goals forbid in this line, and which D2 already answers
("escalate — do not invent a local variant") rather than pre-authorizes.
Adopting shape 1 here would mean either quietly extending `fjell-syscall`
(out of scope, undiscussed with the architect) or deleting functionality
that seven real call sites depend on (a behaviour change, also forbidden).
Neither is available, so shape 1 is not a defensible choice for *this*
tree today, whatever its abstract merits.

**Shape 2 (permit raw asm anywhere, enforce clobbers)** gives up the actual
goal. The RFC's own framing is that 35 places independently responsible for
a contract is the defect; a guard that lets all 35 keep existing, merely
checked for correct clobbers, does not reduce that number at all — it
freezes it. D1 already settled this: delete, don't annotate.

## Answer: shape 3, with a named exception list rather than a free-form escape

**Refuse any raw syscall-issuing `asm!` block outside `fjell-syscall`, with
one exception: a small, explicit allowlist maintained *inside the guard's
own source*, naming exactly the sites this audit found the wrapper cannot
serve. An allowlisted site must still declare every register the kernel
writes for that syscall as a correct clobber — the exception buys a
different receiver for the check, not an exemption from it.**

This is not shape 3 as stated verbatim in the RFC (a free-form "clobbers
enforced where [a wrapper] does not [exist]"), because that phrasing has a
gap the audit exposed: **whether a wrapper "exists" is not a per-syscall-
number question, it is a per-shape question.** `IpcCall` (22) has a
wrapper. Three of this RFC's own kept sites still call `IpcCall`(22) raw,
because the *word count* the wrapper covers is the wrong dimension, not the
syscall number. A guard that asks "does syscall 22 have a wrapper?" would
refuse `sys_ipc_call_words`'s own existence-check and pass judgement on the
wrong axis. A guard that asks "is this exact block's shape the *same*
inputs/outputs, IPC_WORDS, cap-transfer flags as an existing wrapper?" is
the right question but not statically decidable by a regex-based Gate-11-
family checker of the kind this project has built four of already
(`LEASE-CALLSITE-001`, `CAP-CALLSITE-001`, `BCB-CALLSITE-001`, and now this
one) — it would need to parse and compare asm operand lists, which is a
different, heavier kind of tool this project does not otherwise have and
this line does not justify building.

**The allowlist resolves this without that tool.** It does not ask the
guard to decide whether a wrapper "fits" a site — a human decided that,
once, during this RFC's audit, and wrote the decision down as a fixed table
in the guard's own source. The guard's job becomes two much simpler,
statically-checkable questions:

1. Is this raw syscall block's `(file, function)` in the allowlist? If not:
   **FAIL, naming the file and line** (Demonstration 1).
2. If it is allowlisted, does it declare every register the kernel writes
   for that syscall number as a clobber (`a6` for any `IpcRecv`; `a0` for
   any `IpcRecv`/`IpcReply`; and, for the three `IpcCall`(22) sites
   specifically, `a2`-`a5` as `inlateout`, since `sys_ipc_reply` copies all
   four unconditionally on completion — the same defect class as `a0`,
   applied to the call side, found during this audit and detailed below)?
   If not: **FAIL, naming the register** (Demonstration 2).

## What happens when someone needs a syscall with no wrapper

**They cannot add a raw block and have it silently pass.** The guard fails
demonstration 1 the moment the new block exists, because it is not in the
allowlist — a hard stop, visible in code review as a diff to the guard file
itself, not a comment convention someone can add carelessly next to their
own new block (a comment is exactly the kind of self-certifying escape
hatch RFC-v0.22-001's `ci-proptest` defect and this project's repeated
"trust but verify" corrections argue against). To add a legitimately
unwrapped syscall, they must:

1. Add a wrapper to `fjell-syscall` if the shape is common enough to
   justify one (the normal, preferred path — this is what this RFC itself
   argues four times over: three duplicate `IpcCall`-4-word
   reimplementations and four duplicate no-wrapper `IpcReply` sites are
   *evidence* a wrapper is justified, not a reason to keep hand-rolling); or
2. If a wrapper genuinely does not fit (a one-off shape, or the syscall has
   no wrapper of any kind), add exactly one entry to the guard's allowlist,
   which forces a two-line diff in a reviewed, single-purpose file, next to
   a comment stating which escalation authorized it — mirroring how this
   RFC's own three `IpcCall`-4-word and four `IpcReply`-word sites are about
   to be recorded.

The guard does not become the thing people work around, because working
around it means editing the guard itself, in the open, which is
indistinguishable from asking permission.

## The two register-contract findings this audit adds to the RFC's own

Recorded here rather than silently fixed, matching the RFC's own D5/culture
of surfacing rather than patching quietly.

**Finding 1 — a third bug, `IpcCall`'s reply words, same shape as Bug B.**
`sys_ipc_reply` copies all four of the replier's `a2..a5` into the caller's
frame unconditionally on completion (not gated by the call's own declared
word count). All three kept `IpcCall`-4-word sites (`fjell-init::ipc_call`,
`fjell-service-api::chunked::ipc_call4`, and — already half-fixed —
`fjell-proxy-text::ipc_call_action`) declared `a2`-`a5` as plain `in`
instead of `inlateout`, telling the compiler those registers keep their
*input* values after the call when the kernel actually overwrites them with
the reply's data on completion. `proxy-text::ipc_call_action`'s own comment
already documents a live incident from exactly this bug class ("an earlier
version only captured a1 ... which is why every action came back 'not
applicable'") — this audit found the same defect, not yet fixed, still
present in the other two sites and in `a3`-`a5` of the already-partially-
fixed third. Fixed as part of this RFC's per-site pass (these three sites
are being kept, not deleted, so D1's "delete, don't annotate" does not
apply to them — fixing their own contract is the only option D2 leaves).

**Finding 2 — `fjell-syscall`'s own `sys_ipc_recv` (not `sys_ipc_recv_msg`)
has the RFC's Bug A's shape, live, in six call sites.** `sys_ipc_recv(ep) ->
Result<usize, SysError>` is implemented via the generic `ecall2(nr, a0, a1,
a2, a3)` helper, passing `0, 0` for `a2`/`a3` as if they were real inputs.
For `IpcRecv` specifically they are not inputs at all — the kernel writes
`w0`/`w1` into them — and `ecall2` does not touch `a4`, `a5`, or `a6` in its
asm operand list, so none of `w2`, `w3`, or the RFC-055 sender identity are
declared as clobbered, despite the kernel writing all three on every
delivery. This is used today by `fjell-auditd`, `fjell-bootctl`,
`fjell-configd`, and three sites in `fjell-neg-test`/`fjell-sample-service`
— **not a theoretical risk; the same shape of bug that produced
RFC-0.28-001's permanent hang, live in a function this project calls a
"wrapper" and therefore trusts.** **Not fixed here** — `fjell-syscall`'s own
code is this RFC's explicit non-goal, and fixing it is a decision about
`fjell-syscall`'s public contract, not a mechanical swap. Filed as a new
erratum (see review request) recommending a follow-up line, not resolved
unilaterally.

**A third, related but *not* filed as a defect:** several one-way
`IpcSend`(20) sites (`fjell-attestd::sxt_send`, `fjell-diagnosticsd::
send_tag`, `fjell-secure-transportd::send_tag`, `fjell-upgraded::
send_sxt`) pass a data word in `a2` without packing a word count into the
tag's bits 16-23. The kernel's `build_msg` only copies `(tag.words).min(4)`
words, so with `words = 0` that argument is never actually placed in the
delivered message — it has always been silently discarded, on every one of
these sites, independent of this RFC. This is a **pre-existing functional
gap in the protocol, not a register-clobber bug**, and fixing it would
change observable behaviour (a receiver would start seeing a real word
where it has only ever seen zero) — squarely the RFC's own forbidden
"changing any syscall's behaviour" by way of changing what a caller
actually transmits. Left exactly as it behaves today: the swap to
`sys_ipc_send(ep, tag)` (which also cannot carry a payload word) is
therefore bit-for-bit behaviour-preserving, not a regression introduced by
this line. Noted for the record; not filed as an erratum, because unlike
Findings 1 and 2 it is not a hazard this RFC's own defect class produces —
it is a message nobody has ever received a payload word from, which is a
protocol-completeness gap for whichever RFC eventually wires these
push-style flows for real.
