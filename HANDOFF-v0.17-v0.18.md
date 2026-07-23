# HANDOFF — Fjell OS v0.17 → v0.18

**Period:** v0.17.0 (Verus adoption foundation) → v0.18.3 (v1 hardening)
**Status at handoff:** v0.18.3 archived; all v0.17/v0.18 RFCs in `done/` (RFC-v0.17-001
in `proposed/`, awaiting architect decision); `release-rehearsal` gates 1–10 PASS on
owner hardware; v1.0.0 tag not applied (gate 9 manual sign-off pending; RFC-v0.17-001
pending).
**Audience:** architect review.
**Author:** implementation assistant (this session).

This document is written from six perspectives in sequence. Each one is intentionally
narrow — read all of them, not just the executive summary, before forming a judgment.

---

## 0. Read this first

Three things the architect must see before the rest of the document.

### 0.1 What is solid

The core result is honest and independently confirmed: the owner ran
`cargo xtask verus-check --all-pilot` and `cargo xtask release-rehearsal` on their
own hardware (not in the development sandbox) and obtained:

```
verification results:: 8 verified, 0 errors   (capability)
VERUS:TARGET:capability:MACHINE-CHECKED-PASS
verification results:: 5 verified, 0 errors   (lease)
VERUS:TARGET:lease:MACHINE-CHECKED-PASS
[PASS] Gate 10 Verus release-required proofs  every release-required target MACHINE-CHECKED-PASS
RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS
```

The proof obligations are real — the SMT solver (z3 4.12.5) ran, discharged them, and
reported 0 errors. The two release-critical invariants (capability rights
non-amplification and lease epoch revocation) are machine-checked for the first time
in this project's history. The C6 retire-before-wrap change closes a real semantic
divergence between the kernel's u32 epoch and the proof's unbounded nat, and is the
only production-code change in this layer. All pre-existing gates (gates 1–8: 566 host
tests, unsafe audit, MMIO audit, ABI snapshot, readiness matrix, trust report, errata,
validation drills) continue to PASS.

### 0.2 What is *not* solid

The architect should explicitly note these before endorsing the work.

**1. The two milestones of Verus PASS landed in the same development session.**

The promotion criterion (RFC-v0.17-005) requires "proof passed in CI across at least
two releases/milestone tags." Both recorded PASS tags (v0.17.1, v0.18.0) were produced
in a single continuous session rather than across real intervening development. The
criterion's spirit — seeing the proof survive real code churn — is only partially
satisfied. RFC-v0.18-001 states this honestly and argues that the conformance and
property tests provide the additional stability evidence; the demotion path remains as
the safety valve. The architect must decide whether to ratify this timing or require a
genuine gap (e.g. one more milestone of independent development) before the promotion
stands.

**2. The proofs verify the model, not the kernel call paths that use it.**

The Verus proofs operate on abstract spec functions (`subset`, `usable`, `revoke`,
`select`). The conformance tests bridge these to the Rust mirror functions
(`CapRights::is_subset_of`, `fjell_abi::lease::lease_usable`, `select_bcb_mirror`).
Neither the proof nor the conformance tests reach the kernel call paths that invoke
those functions: `fjell-cap/src/cspace.rs:210` (the minting enforcement point) and
`fjell-kernel/src/lease/mod.rs:120` (the revoke call). An adversarial execution of
the kernel itself — concurrency, IPC reordering, race on the lease table — is outside
the current proof scope. The proofs establish that the *design* is correct; they do
not prove the *implementation* correct end-to-end.

**3. All nine QEMU negative-test categories are RFC 025 placeholders.**

`cargo xtask qemu-negative <cat>` for all nine categories (capability, mmio, dma,
user-copy, audit, policy, ipc, svc, harness) reports PASS without booting QEMU. This
is documented behavior (RFC 025 §chicken-and-egg), but it means test-all tiers 10–18
provide no fault-injection coverage today. A project that promotes its capability
invariant to "release-required" and formally proves it should arguably have at least
one negative test that exercises a real capability refusal path in QEMU. This gap is
recorded in `docs/release/v1-limitations.md` but the architect may view it as a
precondition for v1.0.0 tagging rather than a post-v1.0 item.

**4. RFC-v0.17-001 trust-anchor provisioning is unratified.**

The full design-options RFC is drafted and in `rfcs/proposed/v0.17/`. It presents
three mechanisms (TOFU first-boot, factory station, hardware-anchored) with a
tier→mechanism recommendation table. The architect's decision on §4 and §6 of that
RFC is a precondition for the v1.0.0 tag, which the release rehearsal explicitly
records: "v1.0.0 tag remains owner/architect-gated."

### 0.3 One explicit design decision that needs ratification

The promotion to `release_required = true` for capability and lease has a concrete
operational consequence that is new: **a release of Fjell OS cannot be certified for
these two targets without an installed, passing Verus run.** The gate enforces this:
`cargo xtask verus-check --release-required` exits non-zero if Verus is absent or
if either proof fails, even if all 566 host tests pass. Conformance-only is no longer
sufficient for a release.

This is the intended semantics of "release-required" and was documented in
RFC-v0.18-001. The architect should explicitly ratify it, because it changes the
release workflow permanently and cannot be undone without demoting the targets.

---

## 1. Perspective: Engineering Lead

*What got built. Code-level facts.*

### 1.1 New files in `verification/verus/`

| File | Obligations | Purpose |
|------|-------------|---------|
| `capability/rights_lattice.rs` | 8 | Non-amplification proof for `CapRights` |
| `lease/lease_epoch.rs` | 5 | Epoch revocation + retire-before-wrap |
| `boot-control/mirror_selection.rs` | 7 | BCB mirror selection totality |
| `verus-targets.toml` | — | Target registry (tier, release_required, maps_to, conformance_cmd) |
| `TOOLCHAIN.md` | — | Install recipe (validated) |
| `TOOLCHAIN.lock` | — | Exact pin: verus release/0.2026.05.24.ecee80a, rustup 1.95.0, z3 4.12.5 |

### 1.2 Production code changes (the only ones in this layer)

| Location | Change | Why |
|----------|--------|-----|
| `crates/fjell-abi/src/lease.rs` | `lease_revoke_epoch(u32) -> u32` removed; `lease_revoke(u32) -> RevokeOutcome{Advanced, MustRetire}` added | C6: close nat/u32 divergence; retire-before-wrap at MAX |
| `crates/fjell-kernel/src/trap/dispatch.rs:313-326` | Removed duplicate arms for slots 7/8; `7=>"neg-test"`, `8=>"sem-stream"`, `9=>"proxy-text"` | Stale debug-name table caused wrong labels in crash traces |
| `crates/fjell-kernel/src/lease/mod.rs:120-123` | `wrapping_add(1)` → `match fjell_abi::lease::lease_revoke(slot.epoch)` | C6: kernel now routes through the bounded ABI helper |
| `crates/fjell-kernel/src/console.rs` | `TODO(M2+)` relabeled as documented v1.0 single-hart design decision | No longer a pending action item; closes the TODO |

Everything else is tooling, documentation, and test infrastructure. No capability table
logic, no IPC path, no syscall numbers, and no cryptographic code was modified.

### 1.3 New xtask subcommands and gates

| Subcommand / flag | Purpose |
|------------------|---------|
| `cargo xtask verus-check --all-pilot` | Run all pilot proof targets |
| `cargo xtask verus-check --release-required` | Run only release-required targets; exits non-zero if any not PROVED |
| `cargo xtask verus-check <name>` | Run a single named target |
| `release-rehearsal` Gate 10 | Calls `verus-check --release-required`; blocking |
| `ci-verus` GitHub Actions job | Non-blocking push-CI marker recorder (records `VERUS:TARGET:*:PASS` per milestone) |

### 1.4 RFC set

| RFC | Location | Summary |
|-----|----------|---------|
| RFC-v0.17-001 | `proposed/` | Trust anchor provisioning design options — awaiting architect §4/§6 decision |
| RFC-v0.17-002 | `done/` | Capability rights non-amplification proof |
| RFC-v0.17-003 | `done/` | Lease epoch revocation proof; C6 amendment appended |
| RFC-v0.17-004 | `done/` | Boot-control BCB mirror selection proof |
| RFC-v0.17-005 | `done/` | CI proof gate and staging policy |
| RFC-v0.17-006 | `done/` | Selective Verus adoption (umbrella) |
| RFC-v0.18-001 | `done/` | Target promotion to release-required |

### 1.5 New documents

| Path | Purpose |
|------|---------|
| `docs/verification/verus/proof-gate-policy.md` | Staging table, R-V1 rule, promotion artifact checklist (9 items), promotion ledger |
| `docs/verification/verus/review-records/v0.17-pilot-targets.md` | Per-obligation review; machine-check results; C4–C8 addendum |
| `docs/release/v1-limitations.md` | Gate 9 consolidated reference: 6 items with governing records |
| `tools/fjell-repro-check/README.md` | Baseline maintenance procedure |
| `docs/src/intro/what-is-fjell.md` | mdbook intro (was TODO stub) |
| `docs/src/intro/why-fjell.md` | mdbook archetypes and rationale (was TODO stub) |
| `docs/src/tutorials/quick-start.md` | Verified boot output; corrected apt packages (was TODO stub) |
| `docs/src/architecture/overview.md` | Kernel/service layout; verification tiers (was TODO stub) |
| `docs/src/sdk/writing-a-service.md` | Canonical template steps; `static mut` and IpcReply-a1 invariants (was TODO stub) |
| `docs/src/release/v1-non-goals.md` | Pointer page to `docs/release/v1-non-goals.md` (was TODO stub) |

### 1.6 Code-quality posture after this period

- **fjell-tools**: warning count reduced from 8 to 0. Dead `load_raw_key` removed;
  unused imports fixed; `MEAS_PORT` annotated as reserved.
- **Stub services** (`fjell-netd`, `fjell-secure-transportd`, `fjell-driver-virtio-net`):
  riscv cross-build warning count reduced from 35+ to 0 via scoped `#![allow]` with
  explicit STUB headers documenting the intentional early-exit pattern.
- **`fjell-syncd`, `fjell-diagnosticsd`**: 4 warnings remain in the riscv release
  build (unused imports in syncd; unused assignment initializers in diagnosticsd).
  These were present before this period and were not addressed.
- **Repro-check**: two structural bugs fixed: (a) legacy FNV-1a baseline caused
  algorithm-mismatched comparisons since RFC-v0.16-005 H-04 — now rejects
  non-64-char hex entries with a clear error message; (b) `target/` artefacts
  excluded from the committed baseline (volatile across `cargo clean`, unsafe on fresh
  checkouts); (c) test-all `SKIP` counter previously double-counted as `FAIL`.

---

## 2. Perspective: Verification Lead

*What is tested. What is not. The honest distinction.*

### 2.1 Test counts at v0.18.3

| Layer | Count | Verdict |
|-------|-------|---------|
| Host unit tests | 566 | All pass |
| Proptest properties | 14 | 1000 cases each |
| Verus machine-checked obligations | 20 | capability 8, lease 5, boot-control 7 |
| Conformance tests (proof↔Rust bridge) | 23 | 6 cap, 10 lease (incl. 4 C6 boundary), 7 BCB |
| QEMU smoke profiles | 4 | m8, v0.4-net, v0.5-platform, v0.7-sync |
| QEMU negative profiles | 9 categories | All placeholders — PASS without booting QEMU |
| Unsafe audit | 208 sites (kernel) | 0 missing SAFETY comments |
| MMIO audit | 0 missing | Per pre-existing gate |
| ABI snapshot | 0 removals | Per pre-existing gate |
| Repro-check (two-build mode) | 29 artefacts | PASS on v0.18.3 fresh build |
| Verus toolchain | release/0.2026.05.24.ecee80a | Machine-checked on owner hardware and in sandbox |

### 2.2 What the machine-check found that Rust tests could not

Two genuine proof failures were discovered during the first machine-check run that
were invisible to all conformance and property tests:

**`capability`: `zero_is_subset` and `equal_rights_allowed` needed `by(bit_vector)`.**

These lemmas assert universally-quantified bitwise facts:
- `zero_is_subset`: `(0 & !parent) == 0` for all `Rights`
- `equal_rights_allowed`: `(parent & !parent) == 0` for all `Rights`

The conformance tests verify these hold for specific concrete values. The property
tests verify them over random `u32` inputs. The SMT solver without the `by(bit_vector)`
hint cannot discharge them over all `u64`; the bit-vector solver mode (a different
backend) can. The proofs were wrong — Verus correctly reported errors — and would have
remained wrong indefinitely under the Rust test suite. Adding the explicit hints fixed
both obligations.

This is the clearest demonstration in this work of why machine-checking is worth the
investment: a class of universal quantification that property tests approximate but
cannot prove was caught and corrected.

**`verus-check` xtask: `--crate-type=lib` was missing.**

Proof-only library files (no `main`) fail with `E0601` without this flag. The xtask
would have reported FAIL for all three targets even if the proofs were correct.
Discovered on the first real invocation of Verus.

### 2.3 Three-layer verification model (how the proof connects to the code)

Each pilot target is checked at three levels:

```
Level 1: Verus machine-check
         Proves abstract model obligations using the SMT solver.
         e.g. "for all nat epochs e < u32::MAX, revoke advances strictly"
              "for all u64 bitsets, mint_allowed implies child & !parent == 0"

Level 2: Conformance tests (cargo test)
         Pure Rust, host-testable. Checks that the shipped predicate
         matches the abstract model at specific inputs.
         e.g. "lease_usable(false, 6, 5) == false"
              "lease_revoke(u32::MAX) == MustRetire"

Level 3: Property tests (proptest)
         Random inputs. Checks that the Rust mirror holds the model
         property over a distribution.
         e.g. "for random u32 e in 0..MAX: lease_revoke(e) == Advanced(e+1)"
```

Level 1 is the only one that constitutes a proof. Levels 2 and 3 are evidence.
The conformance fallback in `verus-check` (when Verus is absent) runs level 2 only
and correctly reports `CONFORMANCE-ONLY`, not `PASS`.

### 2.4 What the proofs cover

| Target | What is proved |
|--------|----------------|
| `capability` | `mint_never_amplifies`: if minting is allowed then `child & !parent == 0`. `zero_is_subset`, `equal_rights_allowed`, `copy_preserves_rights`, `subset_is_transitive` (with `by(bit_vector)` hints). |
| `lease` | `revoked_binding_not_usable`, `revoke_blocks_even_new_epoch_binding`, `drop_allowed_after_revoke`, `revoke_advances_epoch` (all under `epoch < u32::MAX`), `revoke_bounded_in_domain` (C6: advanced epoch stays ≤ MAX). |
| `boot-control` | `none_only_when_both_invalid`, `valid_beats_invalid_a/b`, `higher_generation_a/b_wins`, `equal_generation_is_tiebreak`, `selection_is_total`. |

### 2.5 What the proofs do *not* cover

The architect should explicitly note these:

| Claim | Proof coverage |
|-------|----------------|
| The kernel's CSpace minting path (`cspace.rs:210`) enforces non-amplification correctly | Not proved; `is_subset_of` is correct (Level 1), the call site is unit-tested (Level 2 at best) |
| No two concurrent IPC calls can race to amplify rights | Not modeled; single-execution proof only |
| The lease table's revoke implementation (`lease/mod.rs`) correctly delegates to the ABI helper in all paths | Kernel call site is exercised by QEMU smoke; not formally proved |
| BCB mirror selection is correct when called under the kernel's actual boot sequencing | `select_bcb_mirror` is proved correct (Level 1 + Level 2); kernel integration tested by qemu-test m8 |
| The retire-before-wrap invariant holds under rollback (a node whose epoch reaches MAX is never revived) | C6 proves the ABI helper is correct; the kernel-level semantics of a MustRetire slot being permanently unusable rest on the state machine logic in `lease/mod.rs`, not on the formal proof |

### 2.6 Proof assumptions (per module)

These are stated in the proof file headers. The architect should verify they match the
implementation.

**capability:**
- A1: Rights are a `u64` bitset matching `CapRights(pub u64)`. ✓ matches.
- A2: "Mint" produces a child checked by `mint_allowed`; the real check is
  `new_rights.is_subset_of(source_cap.rights)`. ✓ matches via `is_subset_of`.

**lease:**
- A1: `epoch` is a monotonic counter; revoke increments by one. ✓ matches (C6 ensures
  it only increments; at MAX, the slot retires rather than wraps).
- A2: A binding records `epoch_at_issue`; usable only while `epoch_at_issue == epoch`.
  ✓ matches `lease_usable`.
- A3: `cap_drop` is always permitted (models `LEASE-VERUS-004`). The real kernel
  drop path must be verified manually — it is not proved.

**boot-control:**
- A1: A mirror is valid iff its `valid` flag is set. ✓ matches BCB header format.
- A2: Higher generation wins on valid/valid tie. ✓ matches `select_bcb_mirror` logic.
- A3: `generation` is a monotonically increasing counter. ✓ enforced by upgrade flow.

---

## 3. Perspective: Architect (self-review)

*Design decisions, invariant preservation, architectural tensions.*

### 3.1 Invariants honored

The eight permanent invariants from RFC 061 §4 are unaffected by this layer. The
verification work is additive: it provides machine-checked evidence for two invariants
(I3 lease-bounded grants; I1 capability handles / non-amplification) but does not
change their enforcement paths.

The retire-before-wrap change (C6) tightens the I3 invariant — the epoch counter now
has a provably bounded domain rather than silent wraparound — but it does not weaken
or redefine any invariant.

### 3.2 Design decisions made in this period

The architect should specifically review each of these.

#### 3.2.1 Target selection uses the existing `tier` field (not ad hoc)

Promotion to `release_required = true` applies to tier-3 targets only. The tier field
was already in `verus-targets.toml` with defined semantics (tier 3 = release-critical,
tier 2 = pilot-required). No new judgment was introduced at promotion time; the
selection follows the existing classification. boot-control (tier 2) stays
`release_required = false`.

**Question for architect:** does the existing tier classification correctly capture the
intended release-blocking criterion, or should the promotion criteria be decoupled from
the tier field?

#### 3.2.2 Conformance-only now blocks a release for release-required targets

If Verus is not on PATH, `verus-check --release-required` exits non-zero for
capability and lease targets. The gate reports `CONFORMANCE-ONLY` (not `FAIL` — a
distinction that matters for log honesty) but still blocks.

Rationale: a release-required proof cannot be *certified* without the prover running.
Allowing conformance-only to pass a release-required target would make the "required"
designation hollow.

This is a permanent operational requirement for anyone cutting a Fjell release:
Verus must be installed and on PATH. The install recipe is validated and documented in
`verification/verus/TOOLCHAIN.md`. The `ci-verus` CI job installs it automatically
(non-blocking on push).

**Question for architect:** is the operational burden of requiring Verus at release
time acceptable for the target deployment community (A1 industrial gateways, A2 fleet
nodes, A3 regulated devices)? The Fjell build and test workflow remains Verus-free for
everyday contributors; only the person cutting a release is affected.

#### 3.2.3 Retire-before-wrap: the only production-code semantic change

Prior to this work, `LeaseTable::revoke` called `slot.epoch.wrapping_add(1)`. The
Verus lease model used unbounded `nat` and proved strict monotonicity. At `u32::MAX`
the kernel would wrap to 0 — silently re-issuing epoch 0, potentially making stale
bindings usable again if a slot were ever reused with the same `LeaseId` generation
after 2^32 revocations.

The C6 change routes the kernel through `fjell_abi::lease::lease_revoke`, which
returns `RevokeOutcome::MustRetire` at `u32::MAX`. The kernel leaves the slot
permanently Revoked with epoch frozen at MAX rather than wrapping. The Verus model
now carries the `epoch < u32::MAX` precondition and a new bounded-domain lemma
(LEASE-VERUS-005).

The practical risk before C6 was near-zero: reaching `u32::MAX` on a single slot
requires 4 billion revocations, and slot reuse with generation bump would invalidate
old `LeaseId` values independently. However, the formal proof was making a claim
(strict monotonicity over nat) that was not implementationally grounded (u32 with
wrap). C6 closes that gap honestly.

**Note for architect:** the MustRetire path has zero runtime observable consequence
except at `u32::MAX`. The four boundary conformance tests (0, 1, MAX-1, MAX) cover it.
There is no QEMU test that exercises a MustRetire outcome; constructing a kernel test
harness that drives a slot to MAX would require 4 billion revocations or a forced
epoch write, neither of which is in scope.

#### 3.2.4 R-V1 rule: a Verus FAIL stays Experimental regardless of Rust tests

Added to `proof-gate-policy.md` as architect condition C5. A target whose Verus
machine-check fails stays at Experimental level (or blocks a release for a
release-required target) even if all 566 host tests, all 23 conformance tests, and all
14 property tests pass.

**Question for architect:** the R-V1 rule is stated as policy but was not explicitly
requested by a prior RFC. The architect should ratify it. Its practical effect is that
the Verus failure is never masked by Rust-test success — a CONFORMANCE-ONLY result
explicitly encodes "the prover did not run" rather than implying all-clear.

#### 3.2.5 Push CI non-blocking; release gate blocking

The `ci-verus` job is `continue-on-error: true` on push. The blocking enforcement lives
exclusively in `release-rehearsal` Gate 10. Normal `cargo build` and `cargo test` never
need Verus.

This matches the Stage A/B philosophy: Verus is never a build dependency, never blocks
a merge, and never blocks a developer who does not have it installed. It blocks only
the person cutting a tagged release.

#### 3.2.6 The two-milestone timing caveat

Stated in §0.1 and in RFC-v0.18-001 §Honest caveat. The architect's explicit decision
is needed:

- **(a) Ratify as-is.** The letter of the policy is met (two distinct tags, two
  distinct recorded PASSes). The conformance + property tests provide the stability
  evidence the spirit of the criterion was designed to capture.
- **(b) Require a third milestone.** Delay final promotion acceptance until one more
  natural development cycle produces a third independent PASS. In the meantime, all
  proofs remain machine-checked and the CI gate records markers, but promotion is
  held as "conditional."
- **(c) Demote to pilot-required.** Revert `release_required = true` to `false` for
  both targets until the timing criterion is satisfied without qualification.

The implementation's recommendation is (a): the proofs are stable, the conformance
bridge is comprehensive, and requiring a synthetic third milestone would delay
recognition of work that is genuinely complete. But this is an architect judgment.

### 3.3 Architectural tensions left unresolved

| Tension | Description |
|---------|-------------|
| **Proof scope vs. implementation scope** | The proofs cover the model functions. The kernel's actual minting and revocation call sites are not formally proved. The gap is bridged by conformance tests and code review, not by the proof itself. Closing this gap would require Verus integration with the kernel codebase — a substantially larger effort than the pilot layer. |
| **MustRetire behavior undefined at kernel level** | At `u32::MAX` the lease slot retires permanently. There is no kernel-level API to signal this to service holders. A service that held a capability bound to a MAX-epoch lease simply finds it permanently Revoked. No semantic record is emitted for the MustRetire case. Whether this is acceptable depends on whether observability of that event matters operationally. |
| **Boot-control kept experimental** | `select_bcb_mirror` is proved correct. The BCB selection invariant is as security-critical as the others — a wrong mirror selection risks booting a compromised image. The decision to keep boot-control at pilot-required (tier 2) was a policy choice following the existing tier classification, not a quality judgment. The architect may wish to revisit the tier assignment. |
| **IPC tag registry gap** | This tension was documented in v0.9→v0.15 (§3.3). It remains unresolved at v0.18.3. No formal verification has been applied to the IPC dispatch layer. |

---

## 4. Perspective: Security Lead

*Trust spine, invariant coverage, attack surface, formal verification claims.*

### 4.1 What formal proof adds to the trust spine

Prior to this work, the two release-critical invariants were defended by:
- Code review (human judgment).
- `forbid(unsafe_code)` (memory-safety argument).
- 566 host tests (point coverage).
- Property tests (random input sampling).

After this work, they are additionally defended by:
- SMT-solver proof that the model is correct for *all inputs* (not just tested ones).
- Explicit proof assumptions written in the module headers (auditable by the architect).
- A gate that blocks a release if the proof fails or the prover did not run.

The architect should assess whether these layers together constitute an acceptable
confidence level for the v1.0 release claim. They do not prove the kernel
*implementation* correct; they prove the *design model* correct.

### 4.2 Non-amplification: what is and is not guaranteed

**What is guaranteed (machine-checked):** if `mint_allowed(parent, child)` holds
(i.e. `(child & !parent) == 0`), then the child has no bits not in the parent.
The minting check in `cspace.rs:210` uses `!new_rights.is_subset_of(source_cap.rights)`,
which compiles to the same bitwise predicate. The conformance test verifies the Rust
function produces the same truth table as the model.

**What is not guaranteed:** that `cspace.rs:210` is the *only* place where a capability
with elevated rights can be produced, that no IPC path bypasses it, that no unsafe block
in the kernel forges a capability with elevated rights. These are defended by the unsafe
audit (0 missing SAFETY comments) and the `forbid(unsafe_code)` enforcement in
`fjell-cap`, not by the formal proof.

### 4.3 Lease revocation: what is and is not guaranteed

**What is guaranteed (machine-checked):** under the model, after a revoke, no binding
issued at or before the pre-revoke epoch is usable. The kernel's revoke path uses the
ABI helper, which the conformance test verifies. The retire-before-wrap property is
machine-checked.

**What is not guaranteed:** that `LeaseTable::revoke` is always called before a
capability is invalidated (i.e. there is no out-of-band epoch increment path that skips
the formal model). The kernel tests and QEMU smoke exercise the path; it is not proved.
The `cap_drop` assumption (A3: always permitted) is modeled as unconditional but the
kernel drop path is not formally verified — it is relied upon not to increment the epoch
without revoking.

### 4.4 Attack surface delta

**Items that changed attack surface in v0.17–v0.18:**

- `fjell_abi::lease::lease_revoke` is a new public API function that replaces
  `lease_revoke_epoch`. External callers that consumed `lease_revoke_epoch` must
  update to handle `RevokeOutcome`. This is a breaking API change.
- `cargo xtask verus-check` now requires `verus` on PATH for release-required gate
  operation. An attacker who can forge a Verus binary and put it on PATH of the release
  environment could produce a fake PASS. The Verus binary is pinned by hash in
  TOOLCHAIN.lock but the gate does not currently verify the binary hash before
  invoking it.

**No change to:** the IPC path, the capability minting path, the cryptographic
backend, the signing pipeline, the key handling, or the ABI surface. The ABI snapshot
gate confirms 0 removals and 0 signature changes.

### 4.5 Formal verification claims the architect can sign

The following claims are now machine-checked and accurate:

- "For any capability rights `parent` and `child`, if `child.is_subset_of(parent)` is
  false, minting is refused." (proved: `mint_never_amplifies` + conformance)
- "For any lease with epoch `e < u32::MAX`, after revoke, the epoch is `e + 1` and
  any binding issued at `e` is permanently unusable." (proved: LEASE-VERUS-001 through
  LEASE-VERUS-005 + conformance)
- "For any two BCB mirrors, `select_bcb_mirror` returns exactly one of them (or None
  if both invalid), prefers the valid mirror, and prefers the higher generation when
  both are valid." (proved: MIRROR-VERUS-001 through MIRROR-VERUS-007 + conformance)

The following claims are *not* machine-checked and should not be stated as proved:

- "The kernel's CSpace implementation correctly enforces non-amplification in all IPC
  paths."
- "No concurrency hazard exists in the lease table under concurrent IPC."
- "A lease retired at MAX cannot be revived."

---

## 5. Perspective: Operations Lead

*Deployment readiness, toolchain requirements, known friction.*

### 5.1 What an operator can do at v0.18.3

**Fully validated (owner-confirmed):**

| Operation | Command | Result |
|-----------|---------|--------|
| Build kernel + services | `cargo xtask build` | All 28 services + kernel |
| Run m8 smoke | `cargo xtask qemu-test m8` | `TEST:M8:PASS` |
| Run all test tiers | `cargo xtask test-all` | `Total: 18 \| PASS: 18 \| FAIL: 0` |
| Machine-check proofs | `cargo xtask verus-check --all-pilot` | 3× `MACHINE-CHECKED-PASS` |
| Release gate | `cargo xtask release-rehearsal` | `ALL MECHANICAL GATES PASS` |
| Repro check | `cargo xtask repro-check` | 29 artefacts identical (two-build) |

**Required toolchain** (Ubuntu 24.04 x86_64, validated):

```bash
sudo apt install rustc-1.91 cargo-1.91 rust-1.91-src lld llvm qemu-system-misc qemu-utils
# Verus (for release gate and machine-check):
# Install per verification/verus/TOOLCHAIN.md (recipe validated)
export PATH="$HOME/.cargo/bin:$HOME/tools/verus/verus-x86-linux:$PATH"
```

Note: `qemu-utils` (provides `qemu-img`) is required for the QEMU disk image creation
step. Its absence causes a silent `fjell-disk.img not found` failure — discovered
during owner verification.

### 5.2 Baseline maintenance procedure

The repro-check `--skip-build` tier (test-all tier 5) compares committed
`prebuilt/*.bin` against a committed SHA-256 baseline. Whenever the prebuilt binaries
are rebuilt (any `qemu-test` or `build-services` invocation after a source change to
a service crate or to `fjell-abi`), the baseline must be re-recorded and committed:

```bash
rm tests/repro/baseline-digests.txt
cargo xtask repro-check --skip-build   # re-records; prints "baseline written"
cargo xtask repro-check --skip-build   # second run must print PASS
git add crates/fjell-kernel/prebuilt tests/repro/baseline-digests.txt
```

Full procedure is documented in `tools/fjell-repro-check/README.md`. This is a
required step in the release workflow that was not previously documented.

### 5.3 Known friction for the release workflow

- **Verus PATH.** The PATH must include both `$HOME/.cargo/bin` (for rustup 1.95.0)
  and the Verus binary directory. Running `release-rehearsal` without Verus on PATH
  fails Gate 10. The error message points to `TOOLCHAIN.md`.
- **`qemu-utils` separate from `qemu-system-misc`.** These are different Ubuntu
  packages; installing only `qemu-system-misc` leaves `qemu-img` missing, causing
  QEMU smokes to fail immediately with `fjell-disk.img not found` (0.7s failure time
  with no QEMU actually started).
- **QEMU negative tests are RFC 025 placeholders.** All nine negative categories
  report PASS without booting QEMU. The test-all summary correctly shows 18/18 PASS,
  but tiers 10–18 contribute no actual kernel behaviour coverage.

---

## 6. Open questions for the architect

These items require a decision before v1.0.0 is tagged.

### 6.1 RFC-v0.17-001 trust-anchor provisioning (REQUIRED DECISION)

`rfcs/proposed/v0.17/RFC-v0.17-001-trust-anchor-provisioning.md` is ready for review.
It presents three mechanisms and asks for two decisions:

- **§4:** Ratify the tier→mechanism table (TOFU dev profile, factory station v1.1,
  hardware-anchored v2).
- **§6:** Decide whether first-boot TOFU requires an explicit `--allow-tofu-provision`
  flag or is acceptable as default for the dev profile.

This RFC is already listed under the v1.0 limitations in `docs/release/v1-limitations.md`
as item 6. It does not block the Verus work but it does block the v1.0.0 tag.

### 6.2 Two-milestone promotion timing (REQUIRED DECISION)

See §3.2.6 options (a), (b), (c). Ratify as-is, require a third milestone, or demote.

### 6.3 R-V1 rule (RATIFICATION REQUESTED)

The rule that "a Verus FAIL stays Experimental regardless of Rust test results" was
added as architect condition C5. It is operational policy now but was not requested
by a prior RFC. Please ratify or revise.

### 6.4 Boot-control tier and promotion timeline (ADVISORY)

boot-control (BCB mirror selection) is proved correct (7 obligations, 0 errors) but
held at pilot-required (tier 2, `release_required = false`) because that was its
pre-existing tier classification. The selection invariant is security-critical: a
wrong selection could boot a compromised image. The architect may wish to revisit the
tier-2 assignment and schedule promotion.

### 6.5 QEMU negative-test coverage (ADVISORY)

All nine negative categories are placeholders. A project that has formally proved the
capability non-amplification invariant and promotes it to release-required should
arguably have at least one QEMU negative test that validates a real capability refusal
at the kernel call site. This is post-v1.0 roadmap work as currently classified, but
the architect should confirm this priority is acceptable before tagging v1.0.0.

---

## Appendix A: File inventory for this period

### A.1 Files added

```
verification/verus/capability/rights_lattice.rs
verification/verus/lease/lease_epoch.rs
verification/verus/boot-control/mirror_selection.rs
verification/verus/verus-targets.toml
verification/verus/TOOLCHAIN.md           (replaced stub)
verification/verus/TOOLCHAIN.lock         (new)
verification/verus/README.md              (new)
docs/verification/verus/proof-gate-policy.md
docs/verification/verus/review-records/v0.17-pilot-targets.md
docs/verification/verus/templates/verus-module-template.rs
docs/verification/verus/templates/rust-conformance-test-template.rs
docs/release/v1-limitations.md
tools/fjell-repro-check/README.md
docs/src/intro/what-is-fjell.md           (replaced stub)
docs/src/intro/why-fjell.md               (replaced stub)
docs/src/tutorials/quick-start.md         (replaced stub, verified output)
docs/src/architecture/overview.md         (replaced stub)
docs/src/sdk/writing-a-service.md         (replaced stub)
docs/src/release/v1-non-goals.md          (replaced stub)
rfcs/proposed/v0.17/RFC-v0.17-001-trust-anchor-provisioning.md
rfcs/done/RFC-v0.17-002-capability-rights.md
rfcs/done/RFC-v0.17-003-lease-epoch.md
rfcs/done/RFC-v0.17-004-boot-control-mirror.md
rfcs/done/RFC-v0.17-005-ci-proof-gate.md
rfcs/done/RFC-v0.17-006-verus-selective-adoption.md
rfcs/done/RFC-v0.18-001-verus-target-promotion.md
.github/workflows/ci.yml                  (ci-verus job added)
crates/fjell-proptest/tests/verus_lemma_properties.rs  (new test crate)
crates/fjell-cap/tests/verus_conformance.rs
crates/fjell-cap/tests/lease_conformance.rs
```

### A.2 Files modified (production code only)

```
crates/fjell-abi/src/lease.rs             (lease_revoke + RevokeOutcome)
crates/fjell-kernel/src/lease/mod.rs      (revoke: wrapping_add → RevokeOutcome match)
crates/fjell-kernel/src/trap/dispatch.rs  (name table: remove duplicate arms 7/8)
crates/fjell-kernel/src/console.rs        (TODO relabeled as design decision)
crates/fjell-tools/src/verus_check.rs     (new subcommand)
crates/fjell-tools/src/release_rehearsal.rs  (Gate 10 + Gate 9 message)
crates/fjell-tools/src/main.rs            (verus-check routing)
tools/fjell-repro-check/src/main.rs       (FNV→SHA256 fix, target/ exclusion,
                                           legacy detection, skip-mode target filter)
crates/fjell-tools/src/test_all.rs        (SKIP counter bug fix)
Cargo.toml                                (workspace version 0.17.0 → 0.18.3)
tests/repro/baseline-digests.txt          (re-recorded from 0.18.3 prebuilt bins)
crates/fjell-kernel/prebuilt/*.bin        (28 files, rebuilt from 0.18.x source)
```

### A.3 Warnings remaining after this period

| Crate | Count | Nature |
|-------|-------|--------|
| `fjell-syncd` (riscv release) | 1 | Unused imports (pre-existing) |
| `fjell-diagnosticsd` (riscv release) | 3 | Unused assignment initializers (pre-existing) |
| `fjell-kernel` (riscv release) | 3 | Unused variable (`stack_top`), dead struct fields (`DmaRegionEntry`), dead function (`unmap_user_va_for`) — pre-existing |

All of these were present before this period. This period reduced the workspace
warning count by 43 (8 fjell-tools, 35+ stub-service riscv build).

---

*End of handoff. Total: 6 perspectives, 6 open questions (2 required decisions before
v1.0.0 tag), all pre-existing gates PASS, 20 proof obligations machine-checked.*
