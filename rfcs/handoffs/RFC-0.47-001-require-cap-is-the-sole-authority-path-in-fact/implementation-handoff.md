# Developer Handoff — RFC-0.47-001

**Governing RFC:** [RFC-0.47-001](../../done/RFC-0.47-001-require-cap-is-the-sole-authority-path-in-fact.md)
**Milestone:** 0.47 · first line
**Status:** inherited from the governing RFC (Implemented, 0.47.0)
**Audience:** implementation model

---

## 1. The premise the architect got wrong, corrected before you build on it

**The 0.47 handoff said a single capability-rights funnel "and there is none". There is one, and it has
existed since v0.2.0.** `crates/fjell-cap/src/enforcement.rs:56` defines `require_cap`, and its own
module doc calls it *"the **sole** production authority-checking path"*. The architect published that
false claim in four places (E-126) and found it only by verifying the prerequisite before scoping a
milestone on it.

**The real defect is the opposite shape and worse:** the funnel exists, declares itself sole, and is
not. Three paths and three check orders, with the inline four testing the lease *before* the right —
so one input, a capability lacking the right and carrying a revoked lease, produced two different ABI
answers depending on which path happened to serve the syscall.

## 2. What this line does

One body in `enforcement.rs` behind three named entries — `require_cap` (kind mandatory),
`require_cap_any_kind`, `require_held` — reached by every authority decision in the kernel.
`check_right`, the four inline copies, the CSpace scan, `build_msg`'s duplicate kind check,
`sys_dma_alloc`'s dead `DmaAlloc` branch and the wrapper's second error table are **deleted**.

**The ABI-visible consequences were taken deliberately, not absorbed** (RFC §D2): no-right **and**
revoked lease now answers `PermissionDenied`, on the **disclosure** argument — telling a caller that
lacks the right at all that a lease was revoked discloses the state of an object it has no authority
over. A stale handle answers `GenerationMismatch` and an empty slot `SlotEmpty` through every path,
because the canonical `to_sys_error` is now the only map. **`abi-snapshot` cannot see any of this** —
it hashes declarations, not what a path returns — which is recorded in the RFC and in `enforcement.rs`.

## 3. The two things worth carrying forward

**`authority-funnel` (27th subcheck) derives its population and judges SAFETY claims.** Its second
half is the one `unsafe-audit` cannot make: a SAFETY comment asserting `require_cap` validated a
handle, in a function that reaches no funnel entry. Run against the kernel as it stood before the fix
it reports **26 findings**, including **exactly the seven false SAFETY claims** the architect had
derived by hand at stage 1 — two independent methods agreeing on one set of seven lines.

**It caught what nobody had:** `sys_cap_install`, written in 0.46 by the author of E-126's false
claim, had itself hand-rolled the funnel's steps 1–3 and 7. The line that claimed to be the one place
authority is decided had missed one in the line written just before it.

## 4. Not in this line

`DELEGATE_ONLY` (a negative right is a design smell and deserves its own RFC against a `Delegable`
kind or an install-source table); **E-093**, deferred to 0.48 on sequencing, now with its subject RFC
re-marked `Implemented-with-Errata`; and the 37 mismatched SAFETY justifications of the architect's
proposed line 3 — beyond the seven this line's own code makes true.
