# Developer Handoff — RFC-0.38-002

**Governing RFC:** [RFC-0.38-002](../../accepted/RFC-0.38-002-a-lease-binding-needs-the-authority-it-claims.md)
**Milestone:** 0.38
**Status:** inherited from the governing RFC (Accepted, 0.38)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a task whose lease has been revoked get its authority back?** Today it can, with
one syscall, on its own lease id, holding nothing. When that attempt is refused **and
the capability still fails `require_cap` afterwards**, and an unprivileged caller is
refused outright, this line is done.

**Not the measure:** that `sys_cap_bind_lease` returns an error in some test. The
property is the absence of authority, not the presence of an error code. See R4.

## 0.1 Read this before writing anything

**Someone already tried this fix and backed it out.** The comment sitting where the
check should be records why: the implementation called `get_kernel_state()` a second
time while the function already held `ct: &mut CapTable`, producing two mutable
references to one table — UB once the optimiser believes them. The revert was the
right call.

So the first thing to do is **not** to write the check. It is to establish that you can
reach the caller's CSpace through the `ct: &mut CapTable` and `tidx` the function
already has. If you can, the rest is small. If you cannot without a second mutable
borrow or a raw pointer, **stop and escalate** — R2 makes that an explicit, approved
outcome, and I will answer in one round trip. Do not reintroduce the aliasing, and do
not reach for `unsafe` to get around the borrow checker here of all places.

## 0.2 One fact that makes this safer than it looks

`sys_cap_bind_lease` has exactly **two** callers:

- `crates/services/fjell-neg-test/src/main.rs:176`
- `crates/services/fjell-sample-service/src/main.rs:214` and `:278`

Both already hold `LeaseAdmin` — neg-test at slot 4, sample-service at slot 1. So
enforcing the documented precondition should break neither, and no existing tier
depends on the ungated path. **Confirm it yourself in R1**; if a third caller turns
up, or one of these two turns out not to hold the capability, that changes the risk
and I want to hear about it before the fix lands.

## 1. Order

1. **R1 first, completely.** The re-derivation, then the class sweep over the dispatch
   arms. The sweep is the part that is easy to skip and the part most likely to find
   something: the defect shape is "a doc comment promises a check the body does not
   perform". If it finds a second instance, escalate and stop — do not fix two
   syscalls in one line.
2. **R3 before R2.** The `Revoked`-slot hole is a two-line change in the epoch read and
   it is the half that makes revocation recoverable. Landing it first means that even
   if R2's borrow question goes to escalation, the serious consequence is already shut.
3. **R4, demonstrated failing, before R2's check exists.** Write the
   revoke-then-rebind tier against the current binary and watch it rebind
   successfully. That transcript is the evidence that E-088 was real; it is much
   harder to produce convincingly after the fix.
4. **R2, the authority check.**
5. **R5, the unprivileged-caller tier**, also demonstrated failing first.
6. **R6, the documentation and the two stale `RFC 042` citations.**
7. **R8**, closing E-088, and **R9**, the gates.

R7 (no retro-claim) is not a step; it governs how you write every line of prose in
steps 6 and 8.

## 2. Traps specific to this line

1. **The aliasing trap.** §0.1. It is the reason this defect is three releases old.
2. **Asserting the error instead of the absence.** R4's second half is the requirement.
   An implementation that refuses the rebind but leaves the stale binding usable would
   pass a return-code assertion and fail the actual invariant. Exercise the capability
   after the refusal.
3. **`/usr/bin/grep -a`, not `grep`.** Plain `grep` here skips NUL-containing files,
   and a blind "0 sites" has closed an erratum wrongly in this project before. Control
   every absence probe against a file you know contains the string.
4. **Three grant sites, not one.** `task/spawn.rs:622` (sample-service, slot 1),
   `:657` (neg-test, slot 4), `main.rs:1141` (init, slot 30). The comment you are
   deleting says "slot 4"; do not inherit its arithmetic.
5. **The citation appears twice.** `cap/syscall.rs` and
   `crates/fjell-abi/src/syscall.rs:38` both say `RFC 042`. Fixing one and reporting
   the figure corrected is the exact mistake I made on RFC-0.38-001's image count
   today — I fixed one of two sites and said it was done. Sweep for the string.
6. **Do not touch lease lifetimes.** Reading this code leads straight to the fact that
   nothing is ever reclaimed. It is a real question, it is in the register, and it is
   not yours this line. Note what you see in the review request.
7. **Rebuild before re-recording the repro baseline**, and commit the `.bin`s in the
   same commit. A kernel change moves the prebuilts; a baseline recorded against the
   old ones will go red in a way that looks unrelated.

## 3. What to put in the review request

Per the roles document §7, and specifically for this line:

- **R1's sweep result**, stated as a conclusion with the command that produced it —
  including the negative case ("these N dispatch arms document a precondition; all N
  enforce it") with its control.
- **The borrow shape you used in R2**, in two or three sentences. If you escalated,
  what you tried and where it failed.
- **Which error R3 returns for a non-`Active` lease, and why that one.**
- **Both transcripts for R4 and R5**: the pre-fix run where the rebind succeeds and
  the unprivileged bind succeeds, and the post-fix run where both are refused — plus,
  for R4, the post-refusal capability use that still fails.
- **Exact commands and exit statuses** for every gate in R9. Exit status, not output
  you read.
- **Anything you saw and left alone**, especially around lifetimes and the other
  dispatch arms.
- **Any place my RFC is wrong.** Three of my statements in the last two days have been
  corrected by other people's reading, including two in this entry's own first draft.
  If the tree disagrees with me, the tree is right and I want it in the request rather
  than worked around.
