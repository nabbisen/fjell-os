# RFC-0.28-002: Thirty-five hand-rolled syscall blocks, two register-contract bugs, and one wrapper that was already correct

**Status:** Accepted — by the owner (nabbisen), 2026-09-07; implementation may begin (RFC 000)
**Milestone:** 0.28
**Tracks.** **E-032**, widened: the defect is not twelve missing `a6`
declarations but **35 hand-rolled syscall `asm!` blocks in services**, each
independently responsible for a register contract only `fjell-syscall` gets
right.
**Touches.** Eleven service crates, `tools/` or `crates/fjell-tools` for the
guard, `docs/rfcs/ERRATA.md`. **Does not touch the kernel, the ABI, or the
syscall surface.**
**Relates to:** RFC-0.28-001 (which hit both bugs and fixed its own two
instances); **E-013** (why the evidence must be a QEMU log); RFC-v0.22-001
(the guard must be demonstrated failing); Gate 11's existing callsite family.

## Summary

RFC-0.28-001 spent its bring-up on a permanent, non-deterministic boot hang
whose cause was one missing register in an inline-asm clobber list. It fixed the
two blocks it had written. **It did not — and was right not to — look at the
other thirty-three.**

| | |
|---|---|
| Raw syscall `asm!` blocks in `crates/` | **37** |
| …in `fjell-syscall`, where they belong | **2** |
| …hand-rolled inside services | **35** |
| Distinct syscalls they issue | 13, 20, 21, 22, 23 |
| Of those syscalls, how many already have a wrapper | **all five** |

Every one of the 35 duplicates a wrapper that already exists, is already
audited, and already gets the contract right.

## The two bugs, neither containing the other

**A — `a6` omitted: 12 sites, all `IpcRecv`.** `cap/syscall.rs:497` writes the
kernel-attested sender identity into `a6` (`tf.gpr[16]`) on **every** successful
delivery, one-way sends included (RFC 055). A block that does not declare it
tells the compiler `a6` survives the `ecall`, so the compiler is free to keep
loop-carried state there and the kernel silently overwrites it.

This is not hypothetical. It produced a permanent hang in RFC-0.28-001,
reproducible across six consecutive builds, **which adding debug prints made
disappear** — the timing-shaped mask that this project's last five milestones
exist to refuse.

**B — `a0` declared as a plain `in`: 18 sites, `IpcRecv` and `IpcReply`.** The
kernel writes its return status into `a0`. Declaring it an unclobbered input
violates the constraint contract in the same way. RFC-0.28-001 found this
independently, in its own new code, and fixed it there; **it is recorded
nowhere else and is live in eighteen places.**

The two overlap and neither is a subset of the other. Both are the same root
cause wearing different registers: **thirty-five places each independently
responsible for knowing what the kernel writes.**

## The settled part — decisions not to be re-opened

**D1 — The fix is deletion, not annotation.** Do **not** add `lateout("a6") _`
and correct `a0` in thirty-five places. Twelve hand-edits invite a thirteenth
omission, and the next person to add a service starts from a copied block.
**Call the wrapper.** `crates/services/fjell-storaged/src/main.rs:248` is
byte-for-byte `sys_ipc_recv_msg`'s operation minus the status and minus `a6`;
the wrapper is a correct superset, and **all eleven affected crates already
depend on `fjell-syscall`.**

**D2 — Audit each site; do not assume the wrapper fits.** I verified the shape
on one recv site and read the wrapper's signature. That is not thirty-five
verifications. Where a site genuinely needs something no wrapper provides,
**escalate** — do not invent a local variant, which is how the thirty-five
happened.

**D3 — A guard, or this recurs.** Gate 11 already carries
`LEASE-CALLSITE-001`, `CAP-CALLSITE-001`, `BCB-CALLSITE-001`. This line adds
one so a thirty-sixth block cannot appear unnoticed. Its exact rule is §6.

**D4 — The `unsafe` count will fall. That is a result, not a target.** Removing
these blocks shrinks the unsafe-audit surface materially. **Do not delete a
block to lower the number**, and do not report the reduction as the
achievement — the achievement is that the contract is stated once.

**D5 — Evidence is a committed log.** E-013 leaves nothing kernel-side
host-testable and this changes how every service talks to the kernel. Promote a
log per RFC-0.27-004 with provenance; do not assert a green run.

## The open question — §6

**What exactly should the guard forbid?**

1. **Raw syscall `asm!` outside `fjell-syscall` is refused, full stop.** The
   strongest rule and the simplest to state. Risk: a future syscall with no
   wrapper has nowhere legal to live, and the guard becomes the thing people
   work around.
2. **Raw syscall `asm!` is permitted, but must declare every register the kernel
   writes for that syscall number.** Keeps flexibility, and encodes the actual
   contract — but that contract now lives in a checker as well as in the kernel,
   which is a second copy that can drift.
3. **Refused where a wrapper exists; clobbers enforced where one does not.**
   The compromise, and the most moving parts.

**Answer in writing before implementing.** I have no inclination worth stating
here — 1 and 3 both look defensible and the argument matters more than my guess.

## Scope

The 35 blocks across `fjell-attestd`, `fjell-diagnosticsd`, `fjell-measuredd`,
`fjell-netd`, `fjell-proxy-text`, `fjell-recoveryd`, `fjell-secure-transportd`,
`fjell-semantic-stream`, `fjell-storaged`, `fjell-upgraded`, `fjell-verifyd`,
and any sibling the audit turns up; the new guard; **E-032** → `CLOSED`, widened
first to name bug B.

### Non-goals

- **Changing any syscall's behaviour, number, or signature.** `syscall-surface`
  ends this line at **35/29/6**, unchanged.
- Adding a wrapper for a syscall that has none, unless D2's audit demands it —
  and then it is an escalation first.
- Touching `fjell-kernel`, `fjell-abi`, or `fjell-syscall`'s own two blocks.
- E-013, E-019/RFC-0.26-003, E-025, E-027, E-028, E-029, E-030.

## Risks

**This touches how every service reaches the kernel, and the failure mode is a
hang.** That is the same shape as RFC-0.28-001's `a6` bug and RFC-0.26-001's M6
hang: nothing goes red, something stops. **Expect to run the QEMU tiers more
than once** — RFC-0.28-001 needed 13 consecutive clean runs before it believed
its own fix, and that was for two blocks.

**A wrapper call is not always a free swap.** Register-level code can depend on
what it does *not* clobber. A site that compiled and worked with a wrong
constraint list may have been relying on the wrongness. Where behaviour changes,
that is a finding about the site, not a reason to keep the asm.

**The reduction is seductive.** Thirty-five `unsafe` blocks disappearing is a
satisfying number, and satisfying numbers are where this project has repeatedly
found itself asserting rather than checking. D4 exists for that reason.
