# Developer Handoff — RFC-v0.22-001

**Governing RFC:** [RFC-v0.22-001](../../done/RFC-v0.22-001-gate-integrity.md)
**Milestone:** v0.22
**Status:** inherited from the governing RFC — **Implemented (v0.22.0)**
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The one rule that governs this whole line

> **Every gate you add or strengthen must be demonstrated failing on a
> deliberately broken input before it is accepted.**

All four defects this line exists to fix shared one property: the gate had
never been observed to fail. A green gate that has never been seen red is a
gate with no evidence it works.

So for each slice, a test proving the check **fails** is a **required
deliverable**, not optional coverage. A slice without one is incomplete
regardless of what else it does.

## 0.1 Two design decisions already made — do not re-open

Settled by the architect so they do not land on you mid-implementation:

1. **One new tool, not two.** Slices 1 and 4 both answer "does the repo's
   declared state match its actual state," so they live in a single new
   `tools/fjell-consistency-check/` with named subchecks, exposed as **one**
   new gate. This limits the gate table to +1.
2. **It becomes Gate 12.** The table grows from eleven to twelve. Every
   "eleven gates" reference in the repo must be updated — that is part of
   Slice 4's work, not separate. RFC-v0.21.3-002 exit criterion 8 requires
   docs match reality, and this is exactly that.

---

## 1. Change scope

**In scope:** `crates/fjell-tools/src/callsite_audit.rs`,
`crates/fjell-tools/src/release_rehearsal.rs`, `tools/fjell-abi-snapshot/`,
`tools/fjell-consistency-check/` (new), `tests/abi/snapshot.json`,
`docs/rfcs/ERRATA.md`, and the "eleven gates" references.

**Explicitly NOT in scope:**

- Any kernel, ABI, capability, lease, IPC, MM, or crypto **behaviour**.
- `crates/fjell-abi/src/syscall.rs` — do not add, remove, or renumber variants.
  Slice 1 *reads* it.
- Adding dispatch arms for the 9 undispatched syscalls. Their disposition is
  still open and is explicitly not decided by this RFC.
- `verification/verus/**`, `TOOLCHAIN.lock`, `package_release.rs`.
- Rewriting the release-rehearsal harness structure.
- **Fixing anything a strengthened gate exposes.** See §6.

---

## 2. Slice 1 — Syscall surface check

New `tools/fjell-consistency-check/`, subcheck `syscall-surface`.

**What it does:** parse the `SyscallNumber` enum in
`crates/fjell-abi/src/syscall.rs` for declared variants, parse the dispatch
arms in `crates/fjell-kernel/src/trap/syscall.rs`, and compare both against a
committed expectations file.

**Expectations file:** `tests/syscall/expected.toml`, listing the declared
count, the dispatched count, and the **explicit set** of undispatched names.
Deliberately not a bare number — the ABI-snapshot pattern works because
changing the surface forces a deliberate edit to a committed file, and a bare
count would let one syscall silently replace another.

Current ground truth, verified 2026-07-30: **35 declared, 26 dispatched, 9
undispatched** — `CapInstall(17)`, `PlatformReboot(18)`, `TaskKill(43)`,
`MmioUnmap(91)`, `IrqBind(100)`, `IrqAck(101)`, `IrqWait(102)`,
`DmaShare(111)`, `Reboot(120)`.

**Required failure demonstration:** a test that the check fails when the
expectations file and the source disagree — in *both* directions (a syscall
added to the enum without updating expectations; an expectation listing a
syscall that no longer exists).

## 3. Slice 2 — ABI signature normalisation

In `tools/fjell-abi-snapshot/src/main.rs`.

Today: `simple_hash(trimmed_line)`. A reflow changes the hash.

**Required:** normalise before hashing — collapse all whitespace runs to a
single space, then trim. **And handle wrapping**: rustfmt splits long
declarations across lines, so when a declaration line has unbalanced
delimiters, join following lines until balanced *before* normalising.
Whitespace collapse alone will not fix a wrapped signature, and missing this
is the likely way this slice ships half-working.

**Baseline regeneration**, under the v0.21.3 Slice 2c discipline:

1. Before changing the algorithm, record `--verify` output on the unchanged
   tree.
2. Change the algorithm, regenerate, record the diff size.
3. Demonstrate the delta is normalisation alone — item names identical, only
   hashes differ. Put the numbers in the commit message so the diff's
   provenance stays recoverable.

**Required failure demonstration:** two tests — a purely reflowed declaration
produces **no** signature change; a genuine change (a parameter type, a return
type) **does**.

## 4. Slice 3 — Gate 11 function-body scan

In `crates/fjell-tools/src/callsite_audit.rs`. The largest slice.

Today it decides capability enforcement is intact with `src.contains(...)`
over whole file text. It passes if the token appears in a comment, a test, or
a doc-string.

**Required:**

1. **Strip comments and string literals** before searching. This alone closes
   the largest hole.
2. **Scope the search to the relevant function body.** Locate `fn <name>`,
   brace-match to find the body extent, search within it — not the file.
3. Keep all three existing checks (`LEASE-CALLSITE-001`, `CAP-CALLSITE-001`,
   `BCB-CALLSITE-001`) semantically the same; only the *rigour* changes.

Textual scanning is intended here. Do **not** add a parser dependency — the
RFC's open question records AST as a possible later step, deliberately not
taken now.

**Required failure demonstrations:** the check fails when the token appears
only in a comment; fails when it appears only in an unrelated function; passes
when present in the correct body.

## 5. Slice 4 — Rule binding, and the gate table

Subchecks in `tools/fjell-consistency-check/`. Bind exactly these three — each
has a recorded real violation, which is why they qualify:

| Subcheck | Rule | Violation on record |
|---|---|---|
| `errata-limitations` | Every `ACCEPTED` erratum appears in `docs/release/v1-limitations.md` | E-011 |
| `rfc-status-folder` | Each RFC's `Status:` agrees with its folder | RFC 000's named anti-pattern |
| `handoff-status` | Each handoff's status matches its governing RFC | The RFC-v0.21.3-001 handoff |

Do not add further rules. R4 in the RFC is about exactly this temptation.

**Then wire Gate 12** into `release_rehearsal.rs` and update every "eleven
gates" reference — `docs/`, `README.md`, the handoff bundle, and anywhere else
`grep -rn "eleven gates\|11 gates\|Gates 1–11"` finds. Per the frozen-bundle
convention, historical bundles get a correction note rather than a rewrite.

**Required failure demonstration:** one per subcheck.

## 6. If a strengthened gate exposes a real violation

**Likely** — RFC R1 rates it medium-high, specifically for Gate 11.

**Report it. Do not fix it in this slice.** Write it up in the review request
with the evidence, and stop. Fixing capability-enforcement findings inside a
tooling slice is how a bounded line becomes unbounded, and it would make the
tooling change unreviewable at the same time.

If a violation looks severe enough that leaving it is unsafe, escalate
immediately rather than deciding either way yourself.

---

## 7. Prohibited shortcuts

- Do not accept any gate without a committed test showing it can fail.
- Do not weaken a check to make an exposed violation disappear.
- Do not fix violations surfaced by the new rigour — report them.
- Do not add a parser dependency.
- Do not touch the 9 undispatched syscalls.
- Do not regenerate the ABI baseline without the before/after provenance.
- Do not mark unexecuted commands as passed.

## 8. Required evidence

1. `cargo xtask release-rehearsal` — full gate table, now twelve
2. `cargo metadata --no-deps`, `cargo fmt --all --check`, `cargo xtask build`
3. `cargo xtask test-all --no-qemu` and `cargo xtask test-all`
4. **Each failure demonstration, run and shown failing** — this is the
   line's whole point; a summary saying "test added" is not evidence
5. ABI baseline before/after numbers (§3)
6. Any violation exposed by Slice 3, with evidence

## 9. Review request

Standard format. Place it in `.git-exclude/review-request/` per the owner's
direction; the architect copies it into `rfcs/handoffs/` during review.

Flag for focused review: the wrapped-declaration handling in Slice 2, the
comment/string stripping in Slice 3, and any Slice 3 findings.
