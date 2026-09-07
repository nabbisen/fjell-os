# Developer Handoff — RFC-0.28-004

**Governing RFC:** [RFC-0.28-004](../../accepted/RFC-0.28-004-syscall-helper-contracts.md)
**Milestone:** 0.28
**Status:** inherited from the governing RFC (Accepted, 2026-09-08)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The scope is small and verified. Do not let it grow.

**Two** of 27 `ecall2` call sites are broken: `sys_ipc_recv` (`lib.rs:193`) and
`sys_cap_inspect` (`lib.rs:672`). The other 25 issue syscalls the kernel writes
only `a0`/`a1` for, and they are correct today.

You will be tempted to widen everything because widening a clobber list cannot
break anything. That temptation is §5(a) shape 1, and it is a legitimate answer
— **but it has to be argued, not defaulted into.** "It is always safe" is a
reason not to think, and the RFC says so on purpose.

## 0.1 The one you will want to under-report

`sys_cap_inspect` issues the syscall **twice** and never checks the second
call's status. Between the two `ecall`s another task runs — RFC-0.28-001
established `schedule_next` runs after every trap — so a revocation in that
window makes the function return `Ok((kind, garbage, garbage))` for a security
question, to a live caller (`fjell-proxy-text:53`).

**Say plainly whether you can demonstrate that window, or whether it stays a
reasoned mechanism.** Both are acceptable answers. Reporting a mechanism you
could not exercise as though you had is not, and this project has an erratum
register full of the alternative.

## 0.2 Design decisions settled — do not re-open

1. **One syscall in `sys_cap_inspect`** (D1).
2. **`fjell-syscall` is in `STABLE_CRATES`** (D2). Show the ABI diff is exactly
   what you intend **before** regenerating. RFC-0.24-003's whole subject.
3. **The guard gains coverage of this crate** (D3). Total exemption is why the
   line that went looking for exactly this defect did not find it.
4. **Committed evidence** (D4).

---

## 1. Order

**§5(a), (b), (c) answered → helper repair → `sys_cap_inspect` → guard → ABI
diff shown → regenerate → evidence.**

The three questions first. (a) decides the shape of the repair, (b) decides what
the guard can even express, and (c) may mean `sys_ipc_recv` should not be
repaired at all but removed — and finding that out after repairing it is wasted
work.

## 2. §5(c) — escalate, do not decide

`sys_ipc_recv` returns one word and discards the tag words and the attested
sender identity **by design**. `sys_ipc_recv_msg` returns all of it. Repairing
the clobbers on a wrapper whose purpose is to throw away what it just correctly
received may be fixing the wrong thing.

**Removing it is an ABI removal.** Gate 4 will call it breaking, correctly.
Escalate with your reasoning; do not delete it and reconcile the snapshot.

## 3. The counts, to re-derive

I claim: 4 helpers (`ecall0`/`1`/`2`/`3`); 27 `ecall2` call sites; exactly 2
issuing a syscall that writes past `a1` (`IpcRecv`, `CapInspect`); `ecall3`
widens `a2` and stops; 6 raw `asm!` blocks in the crate, four setting `a7` via a
register.

**Re-derive it.** My last two RFCs both had counts corrected by you, and the
corrections were worth more than the counts. If `ecall1`/`ecall0` also front a
syscall writing past their declarations, that is a third site and I missed it.

## 4. Prohibited shortcuts

- Do not widen every helper without arguing §5(a).
- Do not leave `sys_cap_inspect` issuing two syscalls.
- Do not regenerate the ABI snapshot before showing the diff.
- Do not remove `sys_ipc_recv` without escalating (§5(c)).
- Do not touch the kernel or `fjell-abi` — the ABI is what it is.
- Do not adjust an audit-record count that moves because the double-call is
  gone. **That is a finding.**
- Do not claim host coverage — E-013; cite a committed log.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 5. Required evidence

1. **§5 (a), (b) and (c) answered in writing**, with rejected shapes.
2. The helper repair, and why the 25 unaffected sites were or were not touched.
3. **`sys_cap_inspect` issuing one syscall**, with the second call's status
   handling explained.
4. Whether the revocation window is demonstrable, answered honestly either way.
5. The guard extended into `fjell-syscall`, **demonstrated failing** on a block
   with a deliberately narrowed clobber list (RFC-v0.22-001).
6. **The ABI diff shown before regeneration** if any signature moved; if none
   moved, say so and show `abi-snapshot --verify` clean.
7. Any moved audit-record count, reported as a finding.
8. A promoted evidence log with provenance (D4).
9. **E-033 widened to name the double-call, then `CLOSED`** — register and
   `v1-limitations.md` in the same commit.
10. `release-rehearsal` green; `test-all` **21/21**; `syscall-surface`
    **35/29/6**; `callsite-audit` PASS.
11. `cargo fmt --all --check`.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Your §5(a) argument**, especially if you chose the widest contract — that is
  the answer most likely to be right for the wrong reason.
- Whether the `sys_cap_inspect` window was demonstrated or reasoned.
- Your §5(c) recommendation on whether `sys_ipc_recv` should exist.
- Any count of mine you re-derived and found different.
- Anything the guard's new coverage found in `fjell-syscall` beyond these two.
