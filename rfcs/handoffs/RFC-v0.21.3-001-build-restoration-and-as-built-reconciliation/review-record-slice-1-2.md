# Review Record — RFC-v0.21.3-001, Slices 1 & 2

**Reviewer:** architect
**Reviewing:** [review-request-slice-1-2.md](./review-request-slice-1-2.md)
**Commits reviewed:** `f3519dc` (Slice 1), `c0f5b9c` (Slice 2)
**Date:** 2026-07-30

## Outcome

**Conditionally Approved.** Slices 1 and 2 are accepted as executed. Findings A
and B are confirmed, correctly diagnosed, and correctly escalated rather than
resolved unilaterally. Both fixes are authorized below, with an amended change
scope. One new finding (C) is raised by this review and must be resolved before
Slice 3 §4.3.

The decision to stop and escalate was right. Two of the three review-focus
questions turned on scope authority that the implementer did not hold.

## 1. What was verified independently

Nothing below is taken from the review request. Each was re-derived.

| Claim | Method | Result |
|---|---|---|
| Manifest restored, 88 members, no globs | `cargo metadata --no-deps`; parsed `Cargo.toml` | **Confirmed** — exit 0, 88 packages, no glob patterns |
| Slice 2 is pure formatter output | Worktree at `f3519dc`, ran `cargo fmt --all`, diffed against `c0f5b9c` | **Confirmed** — zero differing `.rs` files. `c0f5b9c` *is* `fmt(f3519dc)` exactly |
| Slice 2 touched only `.rs` | `git show --name-only` | **Confirmed** — 254 `.rs`, 0 non-`.rs` |
| `cargo fmt --all --check` now passes | direct run | **Confirmed** — exit 0 |
| Finding A: 274/274 → 270/274 | ran `fjell-unsafe-audit` at both commits | **Confirmed** — same 4 sites |
| Finding A root cause | read the 4 sites | **Confirmed**, with an important addition — see §3 |
| Finding B: stale `STABLE_CRATES` paths | filesystem check of all 8 entries | **Confirmed** — exactly 2 stale, none missed |
| Finding B is pre-existing, not fmt-caused | snapshot generated at `f3519dc` | **Confirmed** — 375 items pre-fmt, same as post-fmt |

The Slice 2 purity check is the load-bearing one: because `c0f5b9c` is
byte-identically `fmt(f3519dc)`, and rustfmt is semantics-preserving, **every**
downstream difference between the two commits is provably formatting-only. That
result is used twice below.

## 2. Correction to the request's analysis

§6 states Gate 4's zero-removals criterion is *"not yet meaningfully
evaluable."* It is evaluable, and I evaluated it. Applying only the two-line
path fix from Finding B:

| Tree | Baseline | Current | Added | Removed | Changed sig | Result |
|---|---|---|---|---|---|---|
| `f3519dc` (pre-fmt) + path fix | 401 | 404 | 3 | **0** | **0** | **PASS** |
| `c0f5b9c` (post-fmt) + path fix | 401 | 404 | 3 | **0** | 163 | FAIL |

Two conclusions the request did not reach, both material:

1. **The committed `tests/abi/snapshot.json` was never stale in content.** It
   is exactly correct for the pre-fmt tree. Only the *tool's* paths were stale.
   The RFC acceptance criterion "Gate 4 reports zero ABI removals" is satisfied
   by the two-line fix alone.
2. **All 163 changed signatures are attributable to `cargo fmt` and nothing
   else.** Pre-fmt with the path fix yields `Changed sig: 0`. There is zero
   pre-existing signature drift hiding in that number.

This removes the concern in §7.2 that regenerating the snapshot "is not a small
diff." It is a large diff that is *fully explained and mechanically
re-derivable*. That is a different risk class, and it makes the regeneration
authorizable now.

## 3. Ruling on review-focus question 1 — Finding A

**Authorized: comment-only relocation.** The change scope is amended
accordingly (handoff §1).

The non-change scope on `crates/fjell-kernel/src/**` exists to prevent
**logic** changes to the kernel. Moving a `// SAFETY:` comment changes no
tokens the compiler sees. Escalating rather than assuming that was correct;
the scope is now explicitly amended so the record does not rest on "spirit."

**The fix is not a workaround — it is the correct end state.** Gate 2 requires
the annotation to be lexically adjacent to the `unsafe` it justifies. That
strictness is the feature: an annotation that can drift away from its operation
is an annotation that can outlive the reasoning it records. Do **not** propose
loosening the audit tool to scan backwards past a `fn` signature; that would
weaken the gate to make a symptom disappear.

`crates/services/fjell-recoveryd/src/main.rs`'s `recv_call()` already carries
the correct pattern — comment *inside* the function, directly above `unsafe` —
and it survived the formatting pass untouched. Use it as the model for the
other three sites and for the two kernel macros (move the comment inside the
macro arm body, immediately above `unsafe`).

**Required additional evidence:** after the fix, run `cargo fmt --all` again
and confirm it produces no diff, then re-run Gate 2. This proves the fix is
stable under formatting rather than merely correct once.

## 4. Ruling on review-focus question 2 — Finding B

**Authorized: the two-line path correction in
`tools/fjell-abi-snapshot/src/main.rs`.** Change scope amended.

**Also authorized: regenerating `tests/abi/snapshot.json`** after the path fix
and after Finding A's fix, subject to one mandatory gate:

> The regeneration must be preceded by demonstrating that the working tree is
> exactly `fmt(<pre-fmt tree>)` plus the authorized comment moves and the
> two-line tool fix — i.e. that no unexplained source change is present. The
> evidence is the §1 method: re-run `cargo fmt --all` and confirm no diff.

With that shown, the regeneration is provably formatting-only and needs no
follow-up RFC. Record in the commit message that `Changed sig: 0` was measured
pre-fmt with the path fix, so the diff's provenance is recoverable later.

**Recorded as a design weakness, not for fixing in this patch:** the ABI gate's
`simple_hash(trimmed_declaration_line)` is formatting-sensitive, so any
whole-tree `fmt` invalidates the baseline wholesale and forces a large
regeneration in which a genuine signature change could hide. The gate is
line-based deliberately (`docs/src/abi/policy.md`) to avoid a nightly-toolchain
dependency, so this is a real trade-off rather than a defect — but it should be
revisited. Carried to the v0.22 candidate list; **out of scope for v0.21.3.**

## 5. Ruling on review-focus question 3 — where the fixes land

Neither belongs in Slice 3, and neither belongs inside Slice 2 (which must stay
provably pure `fmt` output — that purity is what makes §2's proof possible).

Two new slices, executed in order, before Slice 3:

- **Slice 2b — annotation restoration.** Finding A's four comment moves. Gate 2
  back to 274/274; `cargo fmt --all` produces no diff afterward.
- **Slice 2c — ABI snapshot path fix and regeneration.** Finding B's two-line
  fix, then regeneration under §4's condition. Gate 4 green.

Separate commits. Slice 3 begins only once Gates 2 and 4 are green, since its
acceptance criteria depend on both.

## 6. New finding raised by this review — Finding C

**The 11 changed prebuilt binaries are not explained, and the explanation
offered is contradicted by the data.**

Slice 1 regenerated `crates/fjell-kernel/prebuilt/*.bin`; 11 of 28 changed
bytes at identical sizes. The request (§3, Slice 1) characterizes this as
*"expected non-reproducibility across environments."*

But **17 of the 28 reproduced byte-identically across that same environment
change.** If the environment were the cause, it would not spare 17 binaries.
Unchanged: `auditd`, `bootctl`, `cap-broker`, `configd`, `driver-virtio-blk`,
`driver-virtio-net`, `netd`, `powerd`, `proxy-text`, `recoveryd`, `rootfsd`,
`sample-service`, `semantic-stream`, `service-manager`, `snapshotd`,
`svc-fault`, `svc-timeout`.

For a project whose stated non-functional requirement is a reproducible build,
"11 of 28 binaries changed with no source change" is a finding, not a footnote.
Identical sizes with differing content suggests embedded non-determinism rather
than a toolchain or path difference.

**Required before Slice 3 §4.3:** characterize the 11. At minimum, determine
whether the difference is confined to a known-volatile region (e.g. embedded
build metadata) or reaches executable content — `cmp -l` byte-offset
distribution across a couple of them is enough to distinguish these. If it
cannot be explained, **do not record the repro baseline over it**: recording a
baseline across an unexplained delta is exactly the failure mode the §4.3
fail-closed change exists to prevent, and it would bake the anomaly in as
"correct."

This does not block Slices 2b/2c. It blocks §4.3 only.

## 7. Items accepted without change

- Gate 10 reported as a failure rather than absorbed as a pass — correct, and
  exactly what the RFC requires.
- Not committing the auto-created `tests/repro/baseline-digests.txt`, and
  deleting it instead — correct. Recording via the fail-open path would have
  been recording the bug.
- Including the Slice-1 prebuilt regeneration in the Slice 1 commit to give
  Slice 2 a well-defined "before" — sound reasoning, and it is what made §1's
  purity check possible.
- The M4 fail-open reproduction is useful corroboration; agreed it is not a new
  finding.

## 8. Minor, fold into Slice 2c

`tools/fjell-abi-snapshot/src/main.rs:32` — unused import `Write` produces a
build warning. Host-side tool, not covered by `cargo xtask build`'s zero-warning
claim.

## 9. Required next deliverables

Slices 2b and 2c, as separate commits, with:

1. Gate 2 at 274/274, plus proof of `fmt` stability (no diff on re-run).
2. Gate 4 green, with the pre-fmt `Changed sig: 0` measurement recorded.
3. Finding C characterization, or an explicit statement that §4.3 remains
   blocked on it.

Then a review request for 2b/2c before Slice 3 starts.
