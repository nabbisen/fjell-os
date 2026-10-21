# RFC-0.38-002: A lease binding needs the authority it claims

**Status:** Accepted (0.38) — accepted 2026-10-21
**Milestone:** 0.38
**Tracks.** **E-088** — `sys_cap_bind_lease` performs no authority check, is
dispatched ungated, overwrites any existing binding, and reads the epoch through a
lookup that admits a `Revoked` slot, so a task can rebind its capability to its own
revoked lease and recover the authority revocation took away.
**Touches** *(indicative)*: `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-kernel/src/lease/mod.rs`, `crates/fjell-abi/src/syscall.rs`
*(doc comment only)*, `crates/fjell-cap/src/cspace.rs` *(doc comment only)*,
`tests/qemu/profiles/`, `crates/services/fjell-neg-test/`, `rfcs/ERRATA.md`.
**Relates to:** **RFC 006**, which introduced `Capability::check_lease()`;
**RFC 015**, which installed it in `check_right()`, `sys_cap_copy`, `sys_cap_mint`
and `sys_cap_inspect` — and not here; **ADR-0003** (lease epoch revocation) and
`verification/verus/lease/lease_epoch.rs`, whose invariant this defeats;
**E-011**, whose ACCEPTED status rests on failing *closed*, which is what
distinguishes it from this.

## What is wrong

`sys_cap_bind_lease` (`crates/fjell-kernel/src/cap/syscall.rs:408`) documents a
precondition it does not enforce:

> Requires: caller holds `CapKind::LeaseAdmin` with `CapRights::LEASE_CREATE`.
> This ensures only privileged services (init, service-manager) can create
> lease-bound caps, preventing unprivileged escalation.

The body contains no check. In its place is a comment recording that the
enforcement was reverted because the implementation called `get_kernel_state()` a
second time while the function already held `ct: &mut CapTable`, creating two
mutable references to the same table — undefined behaviour under optimisation. The
revert is sound; what was left behind is not. The comment closes:

> For now the check passes (all spawned services receive a LeaseAdmin cap at slot 4
> with LEASE_CREATE right, so the invariant holds implicitly).

**That is false in both halves.** `LeaseAdmin` is granted to exactly three tasks:
`SAMPLE_SERVICE` at slot 1 (`task/spawn.rs:622`), `NEG_TEST` at slot 4 (`:657`), and
`init` at slot 30 (`main.rs:1141`). Not "all spawned services", and only one of them
at slot 4. **And it is self-defeating even if it were true**: if every service held
the capability, checking for it would gate nothing, and the doc comment's actual
claim — "only privileged services … preventing unprivileged escalation" — would
still be false. There is no reading on which the documentation describes the
shipped behaviour.

The syscall is reachable by any task. `CapBindLease` (number 16) is routed into
`dispatch_m3` (`trap/syscall.rs:29`) and dispatched at `:393` with no per-syscall
gate, and the function performs none.

### The part that makes this urgent

`LeaseTable::get` (`lease/mod.rs:172`) refuses only a generation mismatch or an
`Empty` slot. A **`Revoked`** slot passes. `revoke` (`:135`) sets
`state = Revoked` and advances the epoch; capabilities still holding the old
`epoch_at_issue` then fail `require_cap`. But `sys_cap_bind_lease` reads the epoch
through `current_epoch` → `get`, and then writes

```rust
cap.lease = Some(LeaseBinding { lease_id, epoch_at_issue: <current epoch> });
```

unconditionally. So a task whose lease was revoked calls this syscall **on its own
lease id**, the capability is re-stamped with the post-revocation epoch, and
`require_cap`'s lease check passes again. No second lease, no guessing, no
privilege. **Epoch revocation is undone at the binding step** — the one step RFC 015
never brought under `check_lease()`, and the mechanism ADR-0003 and a Verus proof
target exist to defend.

The step-2 comment claims the lease "must be Active". It need not be, and that is
precisely what makes the above work.

### Two further false statements in the same area

- The doc comment's provenance, "RFC 042: bind a lease to an existing capability so
  that `require_cap` step 7 verifies the lease is still active on every use", names
  the wrong RFC. `rfcs/done/042-v02-negative-test-expansion.md` is v0.2.0
  negative-test expansion and mentions neither `bind_lease` nor `LeaseBinding`. The
  mechanism is RFC 006's. The same citation is repeated at
  `crates/fjell-abi/src/syscall.rs:38`.
- `lease/mod.rs` claimed a slot generation "incremented when the slot is freed", and
  `revoke`'s retire-before-wrap note offered "slot reuse bumps the generation" as a
  second ground for the epoch never wrapping. Neither happens — nothing returns a
  lease slot to `Empty` and no lease generation is ever incremented. **Both were
  corrected on 2026-10-21** (commit `3b28713`); the retire-before-wrap conclusion
  stands on its first ground, 2^32 revocations of one slot. Named here because it is
  why a `LeaseId` is simply its slot index, `0..32`.

### Why this fix is smaller than it sounds

`sys_cap_bind_lease` has exactly two callers in the tree:
`crates/services/fjell-neg-test/src/main.rs:176` and
`crates/services/fjell-sample-service/src/main.rs:214` and `:278`. **Both hold
`LeaseAdmin`** — neg-test at slot 4, sample-service at slot 1. So enforcing the
documented precondition does not break either of them, and no existing tier depends
on the ungated path. Confirm this in R1 rather than taking it from me.

## Requirements

**R1 — Re-derive, with controls.** Every element above: the absent check; the three
`LeaseAdmin` grants and their slots; the ungated routing and dispatch; the
unconditional `cap.lease` write; that `get` admits a `Revoked` slot; that
`sys_cap_bind_lease` appears in neither RFC 015's list nor anywhere `check_lease()`
is called; and that the only two callers both hold the capability. Use
`/usr/bin/grep -a` — plain `grep` skips NUL-containing files here.

**Then sweep for the class, not just the instance.** This defect is "a doc comment
promises a check the body does not perform, behind an ungated dispatch arm". Walk
every arm of `dispatch_m3` and the other dispatch tables and report any other
syscall whose documented precondition has no corresponding code. **If you find one,
stop and escalate** — do not fix it inside this line. I have looked at this one
syscall only.

**R2 — The check, through the borrow the function already holds.** Implement the
precondition the doc comment states: the caller must hold a `CapKind::LeaseAdmin`
capability carrying `CapRights::LEASE_CREATE`. The reverted attempt failed because
it reached for a second `&mut` to the capability table; the function already has
`ct: &mut CapTable` and `tidx`, so the caller's CSpace is reachable without a second
borrow and without `unsafe`.

**If you cannot express it without a second mutable borrow, stop and escalate.** Do
not reintroduce the aliasing the revert removed, and do not paper over it with a raw
pointer. A correct check that needs a small refactor of the surrounding signature is
a design question I will answer in one round trip; UB is not a trade I will take.

**R3 — The lease must be Active to be bound.** The epoch read currently accepts a
`Revoked` slot. Binding to a lease that is not `Active` must fail closed. **State
which error you return and why** — the ABI already has candidates and I am not
choosing for you — and make the step-2 comment true instead of aspirational.

**R4 — Revocation must survive a rebind attempt, and the test must prove the
authority is still gone.** A tier that creates a lease, binds a capability, exercises
it, revokes the lease, attempts the rebind, and asserts **both** that the syscall is
refused **and** that the capability still fails `require_cap` afterwards.

The second half is the requirement. "The syscall returned an error" is not the
property; "the task cannot use the capability" is. A test that asserts only the
return value would pass against an implementation that refuses the call and leaves a
usable binding in place.

**R5 — An unprivileged caller is refused, demonstrated failing.** A negative tier in
which a task without `LeaseAdmin` calls `sys_cap_bind_lease` and is refused. Per this
project's evidence standard the tier must be **shown failing before the fix** — build
it against the pre-fix binary, capture the transcript showing it passes the bind (the
defect), then show it refusing after. A tier that has only ever been green proves
nothing about the check it names.

**R6 — The documentation becomes true, including the provenance.** The doc comment at
`cap/syscall.rs:413` describes the implemented behaviour; the stale `RFC 042`
citation is corrected to RFC 006 both there and at `crates/fjell-abi/src/syscall.rs:38`;
the "has been reverted … targets v0.8.x" implementation note goes, because it will no
longer describe anything. Check `crates/fjell-cap/src/cspace.rs:176` while you are
there — it documents binding a source and its copy to one lease and may now need a
word about authority.

**R7 — No retro-claim.** Do not write, anywhere, that the check was present and
weak, or that the gap was known and accepted. It was documented as present and was
absent, and E-088 is the record of that. Correct the statements; do not rewrite the
history of them.

**R8 — E-088 closes in this line's commit**, with `docs/src/releasing/v1-limitations.md`
updated if the gate requires it. The entry is **OPEN**, so Gate 7 refuses until it
closes — which is the mechanism that keeps this honest, not an obstacle to route
around.

**R9 — Gates by exit status, not by eye.** `cargo xtask release-rehearsal`,
`consistency-check --all`, `readiness-check`, `abi-snapshot --verify`,
`fjell-repro-check`, `test-all` to completion. If the check changes a prebuilt,
rebuild **before** re-recording the repro baseline and commit the binaries with it.

### Non-goals

- **Lease slot recycling and the lifetime question generally.** Nothing frees a lease
  slot, a task slot, an endpoint or a frame. That is a real design question and it is
  not this line's; it stays with the register.
- **The four declared-but-undispatched syscalls** (`CapInstall`, `TaskKill`,
  `MmioUnmap`, `DmaShare`). Out of scope.
- **Widening `LeaseId` or changing the generation scheme.** The generation check is
  live and correct; it simply has nothing to discriminate yet.
- **Any ABI addition.** If you believe one is needed, that is an escalation, not an
  implementation step.

## Risks

- **The aliasing trap is the whole reason this is still open.** Someone already tried
  and backed out. Read the reverted shape before writing a new one, and treat R2's
  escalation clause as a live option rather than an admission of failure.
- **A green test that proves nothing.** R4 and R5 both specify what the assertion must
  be *about*. The failure mode here is a tier that checks a return code and calls the
  revocation invariant tested.
- **Scope pull toward F2b.** Reading this code leads directly to the fact that nothing
  is ever reclaimed. Note what you see, report it, and leave it.
