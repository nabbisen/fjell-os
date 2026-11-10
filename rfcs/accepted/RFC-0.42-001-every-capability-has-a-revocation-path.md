# RFC-0.42-001: Every capability has a revocation path, and the premise that says so is enforced

**Status:** Accepted — accepted 2026-11-10
**Milestone:** 0.42
**Tracks.** **E-093** — recursive revocation covers nothing that was never lease-bound; **E-091** —
nothing limits *which* lease a holder may bind to, and the `LEASE_BIND` right that would say so
does not exist.
**Touches** *(indicative)*: `crates/fjell-cap/src/rights.rs`, `crates/fjell-cap/src/cspace.rs`,
`crates/fjell-cap/src/slot.rs`, `crates/fjell-kernel/src/lease/mod.rs`,
`crates/fjell-kernel/src/main.rs`, `crates/fjell-kernel/src/task/spawn.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`, `tests/abi/snapshot.json`, the prebuilts and
`tests/repro/baseline-digests.txt`, `rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`
(the architect's).
**Relates to:** **RFC-v0.7.4-003**, whose unimplemented remainder this is; **E-088**, **closed** by
RFC-0.38-002 — the `LeaseAdmin`/`LEASE_CREATE` gate on `sys_cap_bind_lease` **already exists** and
this line must not re-add it; **RFC-0.38-004**, which made a binding immutable but left the first
bind unconstrained; **E-100** / **RFC-0.40-003**, the precedent for coverage a gate can run.
**Investigation:** `.git-exclude/reviewed/investigation-0.42-001-revocation-coverage.md`.

## What is wrong

`RFC-v0.7.4-003-capability-authority-hardening.md` is in `rfcs/done/` — Implemented. It specified a
`LEASE_BIND` right at `1 << 25`, a lease-scope check and audit code `0x0604`. **`LEASE_BIND` does
not appear in one `.rs` file in this tree**, and the bit it reserved now belongs to `REBOOT`
(`rights.rs:66`).

The same RFC deleted kernel-local recursive revocation on the stated premise that *"every meaningful
cap MUST have a lease"*. **Nothing enforces that premise**, and `cspace.rs:166` still states the
design in its own words: *"`new_cap.parent = None` is deliberate… Recursive revocation is done via
lease revocation."*

**But the shape is not what the register said, and the investigation corrected it.**
`Capability::derive()` (`slot.rs:65`) sets **`parent: Some(self_slot)`** and inherits
`lease: self.lease`; only *copy* clears the parent (`cspace.rs:208-210`), deliberately. **The
derivation tree does form.** The defect is that **its roots have no anchor**: `lease: None` appears
at **58 grant sites** — `main.rs` 16, `spawn.rs` 33, and 9 elsewhere — each constructing a
`CapState::Active` capability with `parent: None` *and* `lease: None`, and because `derive`
inherits the lease, **everything derived from a grant inherits `lease: None` too.** The mechanism is
not missing; it is rooted in nothing, so it has nothing to revoke from.

**And the absence is silent.** Nothing distinguishes a capability that is lease-less *by design*
from one that is lease-less *by omission*. That is the part which makes this a defect rather than a
limitation.

## The constraint that decides the design

`MAX_LEASES = 32` (`lease/mod.rs:23`), and **slots are never recycled** — `lease/mod.rs:48`:
*"No lease slot is ever freed… this stays `0` for every lease a boot ever creates."* So 32 is a
**whole-boot lifetime budget**, not a concurrency limit. `create()` fails closed
(`Err(SysError::NoMemory)`), which is correct and is also the problem.

Measured from retained boots: lease ids **0–8** are already in use, varying by profile (`dma` uses
more than `harness`). Task slots up to **21** are observed, there are 35 service images, and
`MAX_TASKS = 40`.

**So a lease per task does not fit, and cannot be made to fit by a small raise** — roughly 20–40
slots on top of the ~9 already consumed, against 32 that are never returned. *(The scope said
"32 → 40+"; that figure was mine and it was too low, because it counted neither the leases a boot
already uses nor the never-freed property.)* Worse, it would exhaust **by profile**, so the failure
would be intermittent — the shape this project spent all of 0.41 chasing.

## Requirements

**R1 — `LEASE_BIND` exists, at `1 << 30`.** Not `1 << 25`: that bit is `REBOOT`'s and **moving it
is an ABI break, refused**. `CapRights` is `u64` (`rights.rs:6`) with 34 bits free, so nothing needs
widening. **The done RFC is annotated where it specifies `1 << 25`**, so a reader finds the reason
without reading a diff.

**R2 — the first bind is constrained to a lease the binder may target.** `sys_cap_bind_lease`
already has its `LeaseAdmin`/`LEASE_CREATE` gate (E-088, closed — **do not re-add it**). What is
missing is the **lease-scope check**: binding to an `Active` lease the caller does not own is
refused, and `LEASE_BIND` is the right that permits the operation at all. Audit code **`0x0604`**
on the refusal, as the done RFC specified.

**R3 — a lease-less capability is lease-less *by declaration*, never by omission.** The kernel's own
root grants are **unrevocable by construction** — they are the root of authority and there is
nothing above them to revoke them — and that must be **stated in the type**, not left as a `None`
that looks like every other `None`. Introduce the distinction (a variant, a marker, whatever the
implementer argues for) such that "no lease because I am a root" and "no lease because nobody set
one" are different states.

**R4 — every non-root path must supply a lease.** With R3's distinction in place, a grant that is
not declared root and carries no lease is a **bug the type system or an invariant check rejects**,
not a silent condition. `derive` already inherits the lease, so anchoring the roots is sufficient to
give every derived capability one.

**R5 — an invariant check a gate runs.** *"No `Active` capability is lease-less without being
declared root"* must be checked somewhere a gate executes — following E-100 and RFC-0.40-003, **not
`#[cfg(test)]` inside a `no_std` binary.** It must be shown **red against the current tree** before
the fix, as RFC-0.42-003's five tests were.

**R6 — the audit trail shows a refused bind.** `0x0604` must appear in a real boot's trail for a
refused out-of-scope bind, quoted in the review request. A right nothing exercises is a right
nothing tests.

### Non-goals

- **Lease slot recycling.** The budget is a whole-boot 32 and `create` fails closed; making slots
  recyclable is the prerequisite for *per-task* anchoring and is **its own line**, filed separately
  rather than smuggled in here. R3/R4 are chosen precisely so this line does not need it.
- **Per-task or per-service lease granularity.** Refused for 0.42-001 on the arithmetic above: it
  would exhaust the table intermittently by profile. It becomes available once recycling exists.
- **Restoring a recursive walk over `parent`.** The links already form on `derive`; adding a second
  partial mechanism beside an unanchored first one is what this line exists to avoid.
- **Re-adding `sys_cap_bind_lease`'s authority gate** (E-088, closed).
- **`MAX_LEASES`' value.** Not raised here; R3/R4 do not need it raised.

## Risks

- **R3 changes a type in `fjell-cap`, which is in the ABI snapshot's crate list.** Expect
  `Added != 0` and regenerate in the line's own commit — and note that trap 3 calls a non-zero
  `Added` "a finding, not cut work", so **say in the review request that this line was expected to
  move it.** *(I omitted exactly this prediction from RFC-0.42-003's handoff and the implementer had
  to work it out.)*
- **R4 may reveal grant sites that cannot supply a lease.** If one does, **stop and report it**
  rather than declaring it root to make the check pass. "Declared root" is a claim about authority,
  not an escape hatch, and an over-broad root declaration would reproduce E-093 with better
  paperwork.
- **Prebuilts and the baseline move**, since `spawn.rs` and `main.rs` change. Re-record in the same
  commit.
- **R2 can break a service that binds a lease it does not own today.** That would be a real finding
  about authority, not a regression to soften.
