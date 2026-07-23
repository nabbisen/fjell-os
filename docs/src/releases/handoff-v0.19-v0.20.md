# HANDOFF — Fjell OS v0.19.0 → v0.20.0

**Period:** v0.19.0 (architect review response: real QEMU negative tests) →
v0.20.0 (fail-closed harness + IPC ABI fix)
**Status at handoff:** v0.20.0 archived (`fjell-os-0_20_0.tar.gz`);
`release-rehearsal` Gates 1–11 PASS; v1.0.0 tag not applied (Gate 9
manual sign-off pending; architect re-review of v0.20.0 pending).
**Audience:** architect review.
**Author:** implementation assistant.

This document is written from six perspectives in sequence. Each is
intentionally narrow — read all of them before forming a judgment.

---

## 0. Read this first

Three things the architect must see before the rest of the document.

### 0.1 What is solid

Every required and strongly-recommended action from the v0.19.0 architect
review has been implemented. The release rehearsal for v0.20.0 produces:

```
  [PASS] Gate 1  Host test suite (0 failures)     host lib tests
  [PASS] Gate 2  Unsafe audit (0 missing)         unsafe-audit
  [PASS] Gate 3  MMIO audit (0 missing)           mmio-audit
  [PASS] Gate 4  ABI snapshot verify              abi-snapshot
  [PASS] Gate 5  Readiness matrix (0 OPEN)        via readiness-check
  [PASS] Gate 6  Trust report (6 sections)        6 sections
  [PASS] Gate 7  ERRATA register (0 OPEN)         0 OPEN errata
  [PASS] Gate 8  Validation drills (markers)      all 5 markers present
  [ -- ] Gate 9  MANUAL: confirm docs/release/v1-limitations.md
  [PASS] Gate 10 Verus release-required proofs    MACHINE-CHECKED-PASS
  [PASS] Gate 11 Callsite conformance             LEASE/CAP/BCB-CALLSITE (static heuristic guard)
RELEASE-REHEARSAL: ALL MECHANICAL GATES PASS
```

Gate 11 now appears in the output. It was absent from v0.19.0 despite the
CHANGELOG claiming it was wired — the v0.19.0 Python edit silently no-opped
(the architect's static inspection of the tarball was correct to flag RB-02).
v0.20.0 verifies by output presence.

The 27 QEMU negative markers that PASS are real: the fail-closed enforcement
(RB-01) immediately exposed that two of the nine ipc markers had been false
passes since the protocol's introduction, forcing a kernel bug fix before
they could legitimately pass.

### 0.2 What is *not* solid

**1. The IPC words ABI was broken since introduction and reached v0.19.0
undetected.**

The defect described in §1.3 passed through the entire history of the project
— through formal verification of adjacent code, through 566 host tests, through
9 QEMU negative profiles — invisibly. It was only exposed by making the harness
fail-closed (RB-01). The ipc BLOCKED_RECV and BLOCKED_CALL markers in v0.19.0
were false passes: sample-service was binding `LeaseId(0)` (a previously-revoked
lease) and failing instantly rather than exercising the real blocked-recv/call
protocol. The fix is in v0.20.0 and the three ipc markers are now real.

The architect should assess: what does the undetected presence of this defect
imply about the reliability of the other 24 confirmed markers and the project's
testing depth?

**2. The v0.19.0 marker count was inflated.**

The v0.19.0 count of 22 markers included the two false ipc passes. The honest
pre-v0.20.0 real count is 20. The v0.20.0 count of 27 is 27 genuinely
fail-closed passes, three of which are new (the real ipc triple). All 27 have
been verified to complete "all scenarios complete" without producing forbidden
markers.

**3. The svc READY pair (2 of 4 expected markers) remains pending.**

`NEG:SVC:READY_ACCEPTED` and `NEG:SVC:UNAUTHORIZED_READY_REJECTED` have no
emitting implementation — they require timing-sensitive service-manager
coordination that is not yet reliable. The svc profile's expected_markers TOML
lists only the confirmed 2; the pending 2 are documented. This is not a false
pass — the confirmed markers are real — but the coverage is incomplete.

**4. The store and upgrade negative profiles have zero emitting scenarios.**

`store.toml` and `upgrade.toml` each list 4 markers that are specification
ahead of implementation. No kernel or service code emits them. Running them
manually fails honestly; they are not in the test-all or CI matrix and are
explicitly not v1 release-gated. For an OS whose storage and upgrade integrity
are part of its identity, the absence of real negative coverage in those areas
is a documented weakness.

**5. The divergent `WrongKind → SysError` mapping is unresolved.**

`trap/syscall.rs:98` maps `CapError::WrongKind → SysError::InvalidCap`, while
the canonical `to_sys_error()` in `rights.rs` maps it to `SysError::WrongType`.
No current test fails because of this — the affected syscalls aren't covered by
the WrongType expectation — but it is a real error-contract inconsistency.
Annotated as a follow-up cleanup item.

**6. Gate 9 remains manual.** The six items in `docs/release/v1-limitations.md`
have been updated (see §4.2). The confirmer must read and sign off.

### 0.3 The one concrete finding that dominates this milestone

The fail-closed harness (RB-01) exposed a stack of three cooperating defects
whose joint effect was that the IPC lease-protocol negative tests had never
exercised real blocking behavior. This is a more important result than the
marker count: it demonstrates that the previous testing regime had a blind spot
in a security-relevant path (lease revocation during blocked IPC), and that the
fail-closed fix correctly identified it.

The v0.19.0 architect finding was correct:

> v0.19.0 proves that moving from placeholder tests to real QEMU negative
> tests was the right decision. But the negative-test harness itself must now
> be made fail-closed before v1.0 can rely on it.

v0.20.0 confirms the inverse: making the harness fail-closed immediately
revealed a latent kernel defect that had survived the entire project history.

---

## 1. Perspective: Engineering Lead

*What got built. Code-level facts.*

### 1.1 Production code changes — complete list

#### Kernel (v0.19.0)

| Location | Change | Why |
|----------|--------|-----|
| `task/spawn.rs` | `et.alloc()` for ep ids 5 (cap-broker), 6 (sample-service) | Never allocated; every IPC to these endpoints returned InvalidCap since RFC 040 |
| `task/spawn.rs` | DMA grant: `CapKind::DmaAlloc` → `CapKind::DmaRegion` | `sys_dma_revoke` requires DmaRegion; DmaAlloc only worked for alloc, not revoke |
| `trap/syscall.rs` | `sys_audit_drain`: upfront `UserPtr::new` validation | RFC 039: null/kernel-space buffers silently returned Ok(0) when ring was empty |
| `trap/syscall.rs` | Per-task debug line buffer (32 tasks × 160 bytes), SIE-masked atomic flush on `\n` | Timer preemption character-interleaved concurrent service output, destroying QEMU markers |
| `console.rs` | SIE-masked `_print` | Line atomicity for kernel-side writes under preemption |
| `lease/mod.rs` | `kprintln!` in MustRetire arm | Observability for the retire-before-wrap path (architect D8) |

#### Kernel (v0.20.0)

| Location | Change | Why |
|----------|--------|-----|
| `cap/syscall.rs::deliver()` | Words → `gpr[12..15]` (a2..a5); identity → `gpr[16]` (a6); badge write removed | ABI fix: userspace reads w0 from a2; old code wrote badge at a2 and words at a3..a6 |
| `lease/mod.rs::wake_or_cancel` | Bind `ct`; call `ct.cancel_replies_for_lease(id, old_epoch)`; wake cancelled callers with `LeaseRevoked` | RFC 050: revocation must cancel server-side reply edges and wake blocked callers |
| `trap/syscall.rs:98` | Follow-up annotation on `WrongKind → InvalidCap` divergence | Documents the cleanup path |

#### Userspace

| Location | Change | Why |
|----------|--------|-----|
| `fjell-syscall::sys_ipc_call_words` | `a1 = tag \| (3usize << 16)` | Word count must be in tag bits 16–23; kernel's `build_msg` copies `tag.words` words |
| `fjell-neg-test::check_err` | PASS marker only on exact expected error | RB-01a: wrong error and unexpected Ok no longer produce false PASS |
| `fjell-neg-test` inline match sites | Same fix applied to late-reply and other arms | Several had the same false-pass structure |
| `fjell-neg-test` all setup failures | `Err(_) => return` → `setup_failed(scenario)` emitting `NEG:HARNESS:SETUP_FAILED` | M-03: silent setup failures now diagnostically visible |
| `fjell-neg-test` scenario order | IPC pair moved last in `service_main` | Coordination stall cannot shadow policy/audit/svc scenarios |
| `fjell-sample-service::BLOCKED_CALL` | `Err(_)` → `Err(LeaseRevoked)` specifically | BLOCKED_CALL should fire only on the contract-specified error |
| `fjell-sample-service` handle threading | `call_h` threaded through bind/call/drop | Same raw-slot-constant antipattern as the v0.19.0 neg-test quartet |
| `fjell-verifyd` | Build-time embed of `provision/dev-trust-anchor.key`; loud startup warning when unprovisioned | RB-04: hardcoded silent all-zero anchor eliminated |

#### New: `cargo xtask provision-dev`

Refuses without `--allow-tofu-provision` (policy text + exit 1). With the flag:
writes `provision/dev-trust-anchor.key` (32-byte random, 64 hex chars) +
`provision/PROVENANCE.toml` (mechanism = "tofu-dev", date, flag-acknowledged).
The next build embeds the key in verifyd via `build.rs`. The release archive
ships without `provision/`; each operator provisions explicitly.

### 1.2 The IPC words ABI defect — precise technical description

This is the most significant code finding in this development period.

#### Defect A — sender: word count never packed

`sys_ipc_call_words(ep, tag, w0, w1, w2)` sent `a1 = tag` (raw label).
The kernel's `build_msg` parsed `tag.words = (tag >> 16) & 0xFF` to determine
how many words to copy from the caller's a2..a4. For every label constant below
`0x10000` (all of them), `tag.words == 0`. `build_msg` copied nothing.

Fix: the wrapper now sends `a1 = tag | (3usize << 16)` — it knows it carries
3 words and packs the count into the tag.

#### Defect B — receiver: register offset and identity collision

`deliver()` wrote:
```rust
tf.gpr[12] = msg.sender_badge;     // a2 = badge (always 0 for these protocols)
tf.gpr[13 + i] = msg.words[i];     // words at a3, a4, a5, a6
tf.gpr[16] = tid | (image_id<<16); // a6 = identity  ← OVERWRITES word[3]
```

Userspace `sys_ipc_recv_msg` extracted w0 from a2 (always the badge = 0),
w1 from a3 (real w0), etc. Word 3 was overwritten by the identity.

Fix: `deliver()` now writes words to a2..a5 and identity to a6. Badge removed
(no userspace consumer existed).

#### Why it went undetected

The two defects stacked cooperatively. Together, `w0` received at the
destination was always 0, independent of what was sent. The protocol
consequence: sample-service received `w0 = 0`, called `LeaseId(0)` — the
lease from the earlier `test_cap_lease_revoked` scenario, which was already
revoked — and `sys_ipc_recv` on the leased cap failed instantly with
`LeaseRevoked`. The old `Err(_)` arm in sample-service printed the
`BLOCKED_RECV` / `BLOCKED_CALL` marker unconditionally.

Label-only protocols (cap-broker policy, service-manager READY) worked
because they only use `tag.label` and `sender identity`, neither of which
was affected.

#### How the fail-closed harness found it

After RB-01a, `BLOCKED_CALL` in sample-service required `Err(LeaseRevoked)`
specifically — still satisfied (the instant-failure was `LeaseRevoked`). But
the subsequent step — neg-test's `sys_ipc_recv` waiting for sample's callback
— had no callback arriving. The service hung in recv, the `NEG:IPC:LATE_REPLY_REJECTED`
marker was missing, the profile failed. The failure drove the investigation.

### 1.3 The reply-edge cancellation defect — precise technical description

`wake_or_cancel_blocked_ipc_for_lease` in `lease/mod.rs` destructured
`get_kernel_state()` as `(tasks, sched, _, et)`, discarding the `CapTable`.
It cancelled endpoint sendq/recvq waiters but never called
`ct.cancel_replies_for_lease(id, old_epoch)`.

A caller blocked awaiting a reply is not in any endpoint queue — it is in the
reply table. When lease revocation fired, the reply edge was not cleared and
the caller was never woken. The server's later `sys_ipc_reply` hit the
defense-in-depth check (`edge.lease` revoked → Err(LeaseRevoked)) and returned
correctly, but the caller remained permanently Blocked.

Additionally, when sample's callback call was still in the sendq at revocation
time (timing variant), `cancel_by_lease` fired and woke sample — but neg-test's
recv found the sendq empty (the queued call was cancelled) and blocked forever.

The fix: bind `ct`, call `cancel_replies_for_lease`, wake each cancelled caller
with `LeaseRevoked` under the same terminal-state guard as the queue wakes.
This makes behavior deterministic across all timing variants.

---

## 2. Perspective: Verification Lead

*What is tested. What is not. The honest distinction.*

### 2.1 Test counts at v0.20.0

| Layer | Count | Status |
|-------|-------|--------|
| Host unit tests | 566 | All pass. The deliver() change did not disturb any host assertion (no host test asserts gpr register layout) |
| Proptest properties | 14 | 1000 cases each |
| Verus obligations | 20 | capability 8, lease 5, boot-control 7 — unchanged since v0.18.1 |
| Conformance tests | 23 | Unchanged |
| QEMU smokes | 4 | m8, v0.4-net, v0.5-platform, v0.7-sync |
| QEMU negative (confirmed) | 27 markers / 9 categories | All fail-closed |
| QEMU negative (pending) | 8 markers / 2 categories | store 4, upgrade 4 — no emitters |
| Unsafe audit | 0 missing | Including per-task debug flush (MMIO-ORDER inside loop) |
| callsite-audit | 3/3 | LEASE-CALLSITE-001, CAP-CALLSITE-001, BCB-CALLSITE-001 |
| Repro-check | PASS (28 artefacts) | Two-build mode |

### 2.2 The fail-closed discovery chain

```
RB-01a: check_err no longer emits marker on Err(_)
  → exposes: BLOCKED_CALL any-error arm was firing on LeaseRevoked (wrong reason)
    → exposes: sample's callback never reached neg-test
      → exposes: IPC words dropped → LeaseId(0) bound → instant failure
        → root cause: deliver() ABI off-by-one + word count not packed

RB-01b: qemu_run fails on forbidden markers
  → exposes: two WRONG_ERROR lines in user-copy profile
    → root cause: UserPtr maps NullPointer/KernelAddress → InvalidArg
      → test expectations had InvalidAddress; aligned to canonical mapping

Scenario reorder (ipc last)
  → without the reorder, the blocked-call hang shadowed
    policy/audit/svc in timing variants where the hang manifested

Reply-edge cancellation fix
  → fixes the hang (caller now woken deterministically)
    → enables LATE_REPLY to fire deterministically
      → ipc 3/3 real for the first time
```

### 2.3 callsite-audit: scope statement

Gate 11 is labelled "static heuristic guard" in both the rehearsal output
and `proof-gate-policy.md`. The three checks:

**LEASE-CALLSITE-001:** Line-by-line scan of `lease/mod.rs` for `epoch && wrapping_add`. Catches the pre-C6 silent-wrap pattern. Does NOT catch: different variable names, multiplication-based increment, or non-epoch wrapping additions. Comment lines excluded.

**CAP-CALLSITE-001:** Checks `is_subset_of` exists in `cspace.rs`. Confirms the proved predicate is present in the minting module; does NOT confirm it is called in the mint branch.

**BCB-CALLSITE-001:** Scans for `.generation && .valid` co-occurrence outside `fjell-upgrade-format/src/lib.rs`. Detects naive duplication; does not verify callers use the return value.

Recommended future strengthening (architect v0.19.0 M-02): narrow to function bodies, fail on raw rights bitmask comparisons in the mint path, scan all call sites of `is_subset_of`.

### 2.4 What the 27 QEMU negative markers prove

Each `NEG:*:PASS` marker asserts that in a specific QEMU boot, the kernel
returned the specific expected error code for the specific invalid operation,
and that the service completed ("all scenarios complete") without forbidden
markers.

What they do NOT prove:
- Rejection across all timing variants (single-run evidence)
- Rejection from all kernel entry points for the same operation
- Absence of partial effects before rejection
- Correct behavior under concurrent access (single-hart, not applicable today)

### 2.5 Verus proofs: unchanged

The Verus proof corpus (20 obligations) is unchanged. The v0.18.3
verification assessment remains current, with the addition:
- `verus-check` now fails `--release-required` on unknown/mismatched versions.
- CALLSITE-001 checks confirm the proved predicates remain the enforced call sites.

---

## 3. Perspective: Security Lead

### 3.1 IPC words defect: security scope

Current word-bearing IPC callers: only `fjell-neg-test` and `fjell-sample-service`.
No production service uses `sys_ipc_call_words` in the shipped image. The
cap-broker, policy, and storage protocols are label-only. No production-path
authority transfer was broken by this defect.

The defect created an **unverifiable testing claim** for any future service
that would use word-bearing IPC. The fix is prerequisite to credibly testing
such protocols.

### 3.2 Reply-edge defect: security scope

The defect caused Blocked-task leaks (resource exhaustion), not privilege
escalation. In the current static service configuration (MAX_TASKS = 32,
7 long-running services), the leak is bounded. The fix aligns system state
with the RFC 050 contract and removes a timing-dependent hang.

### 3.3 Trust-anchor provisioning

**Before v0.20.0:** `const DEV_ANCHOR_KEY: [u8; 32] = [0u8; 32]` — every
boot implicitly trusted the zero key. No provisioning act required.

**After v0.20.0:** The zero-key default is preserved for backwards
compatibility but is loud:
```
verifyd: WARNING unprovisioned dev trust anchor (legacy all-zero dev key);
run `cargo xtask provision-dev --allow-tofu-provision`
```
Explicit provisioning writes a random key + traceable PROVENANCE.toml.
Archives ship unprovisioned; each operator provisions explicitly.

**Remaining gap:** The signing side (sign-bundle / DevSignatureProvider) still
uses the all-zero dev key. A fully provisioned end-to-end flow requires the
operator to also re-sign bundles with the provisioned authority's key. This
is documented in PROVENANCE.toml as an operational requirement, not enforced
by a gate. The architect should confirm whether this coupling requires
enforcement before v1.0.0.

---

## 4. Perspective: Release Manager

### 4.1 Gate matrix at v0.20.0

| Gate | Status | What it checks |
|------|--------|----------------|
| 1 | PASS | 566 host tests |
| 2 | PASS | Unsafe audit: 0 missing SAFETY comments |
| 3 | PASS | MMIO audit: 0 missing MMIO-ORDER annotations |
| 4 | PASS | ABI snapshot: no removals |
| 5 | PASS | Readiness matrix: 0 OPEN cells |
| 6 | PASS | Trust report: 6 sections present |
| 7 | PASS | Errata register: 0 OPEN (E-010 IPC words added as CLOSED) |
| 8 | PASS | Validation drills: 5 required markers |
| 9 | PENDING | Manual: confirm v1-limitations.md |
| 10 | PASS | Verus release-required: capability + lease MACHINE-CHECKED-PASS |
| 11 | PASS | Callsite-audit: 3 heuristic checks |

### 4.2 Gate 9 reference document current state

`docs/release/v1-limitations.md` at v0.20.0:

| # | Item | Change from v0.19.0 |
|---|------|---------------------|
| 1 | Hardware (E-004) | Unchanged |
| 2 | Multi-hart | Unchanged |
| 3 | POSIX non-goal | Unchanged |
| 4 | Kernel-IPC SDK reference service (N21) | Unchanged |
| 5 | ZeroizeOnDrop (N23) | Unchanged |
| 6 | Trust-anchor provisioning | Updated: "flag implemented in v0.20.0" |
| Op. note | QEMU negative coverage | Updated: ipc 3/3 (was "2/3 pending") |

### 4.3 What is not gated

- svc READY pair (timing-dependent; documented in svc.toml pending section)
- store/upgrade profiles (no emitters; documented in v1-limitations)
- Divergent WrongKind mapping (annotated in code)
- Signing-side provisioning coupling (documented in PROVENANCE.toml)

---

## 5. Perspective: Project Historian

### 5.1 v0.19.0 — what was found by the first real negative tests

| Finding | In codebase since |
|---------|-------------------|
| Profile loader TOML array silently empty | v0.1.1 (RFC 025 placeholder era) |
| `sys_audit_drain` null/kernel-space bypass | RFC 039 (v0.2.x) |
| Endpoint 5 (cap-broker) never allocated | RFC 040 (v0.2.x) |
| DMA revoke kind mismatch (DmaAlloc vs DmaRegion) | Original DMA work |
| Raw slot-constant handles in neg-test + sample | IPC test introduction |
| Console character interleaving | Multi-service era |

### 5.2 v0.20.0 — what was found by fail-closed enforcement

| Finding | In codebase since |
|---------|-------------------|
| `check_err` false-pass pattern | neg-test introduction |
| Gate 11 absent from rehearsal | v0.19.0 (silent Python no-op) |
| IPC words ABI broken (both defects) | RFC 034 / IPC protocol introduction |
| Reply-edge not cancelled on revoke | RFC 050 / lease-IPC introduction |
| sample-service `BLOCKED_CALL` any-error arm | IPC test introduction |

### 5.3 The cumulative picture

The move from placeholder tests (v0.18.3) to real tests (v0.19.0) found 6
kernel bugs. Making those tests fail-closed (v0.20.0) found 2 more (the IPC
words defect and the reply-edge gap). The pattern is: each incremental
tightening of the test validity criteria reveals bugs that the previous
criteria could not detect.

This is the correct engineering trajectory. It also means that the absence
of new bug discoveries at any particular milestone should not be taken as
evidence of correctness — it may simply mean the current test criteria are
not tight enough.

---

## 6. Perspective: Honest Weaknesses / Items the Architect Must Decide

### A. The false-pass history in v0.19.0

The v0.19.0 architect review concluded the new tests "found real bugs." That
was true about audit_drain, endpoint 5, DMA kind, and raw handles. It was
not true about the ipc markers — those were false passes. This document must
be honest about that. The architect should factor this into the assessment
of v0.20.0's 27 markers.

### B. ABI snapshot does not cover IPC register layout

Gate 4 tracks syscall numbers and struct layouts. It did not catch the
deliver() ABI bug. The architect should decide whether to extend the ABI
snapshot to cover the IPC register protocol (which registers carry which
fields), or accept this as a known gap in the snapshot coverage.

### C. Gate 11 scope

Is "static heuristic guard" an acceptable designation for a blocking v1
release gate? The three checks are useful smoke guards that have already
demonstrated diagnostic value. They are not proofs. The architect's v0.19.0
M-02 noted that future strengthening (narrow to function bodies, raw bitmask
checks, call-site scanning) would be needed for a stronger claim.

### D. Store/upgrade profile status

The v0.19.0 review recommended adding them to the CI matrix "unless runtime
cost is too high." The v0.20.0 decision is Option B (not gated; documented).
The architect should confirm this for v1.0.0 or override it.

### E. Signing-side provisioning coupling

The operator must both provision the dev anchor key AND re-sign bundles with
the matching authority to have an end-to-end provisioned system. Step 2 is
not enforced by any gate. Should it be before v1.0.0?

### F. v1.0.0 approval

All code items from the v0.19.0 required-before-tag list are resolved. Gate 9
requires the owner's manual sign-off. The architect must decide whether to
approve v1.0.0 after reviewing v0.20.0.

---

## Summary table

| Item | v0.19.0 state | v0.20.0 state |
|------|--------------|--------------|
| Confirmed QEMU negative markers | 22 (2 ipc were false) | 27 (all fail-closed) |
| Honest pre-v0.20.0 count | ~20 | — |
| ipc profile | 2/3 ("LATE_REPLY pending") | 3/3 real |
| svc profile | 2/4 | 2/4 (documented) |
| store/upgrade | 0 emitters | 0 emitters (explicitly not v1-gated) |
| Gate 11 in rehearsal output | No | Yes |
| Trust-anchor silent default | Hardcoded [0u8;32] | Loud warning; explicit provision-dev |
| IPC words ABI | Broken (all payload words dropped) | Fixed |
| Reply-edge cancellation | Missing (recorded finding) | Fixed |
| `check_err` false-pass | Any-error pass | Exact-error-only |
| Divergent WrongKind mapping | Unrecorded | Annotated; cleanup follow-up |
| Verus proofs | 20 obligations | 20 obligations (unchanged) |
| Gates 1–10 | PASS | PASS |
| Gate 11 | Absent | PASS |
| Gate 9 | PENDING | PENDING |
| v1.0.0 tag | Not approved | Not approved; awaiting architect re-review |
