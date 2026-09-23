# RFC-0.33-004: Checks that cover what they claim

**Status:** Proposed
**Milestone:** 0.33
**Tracks.** Four instruments that report success over less than they claim:
**E-049** (CI names test packages by hand and misses ten crates), **E-052**
(eleven citations the published book cannot follow, and two subchecks that
require exactly that spelling), **E-056** (the ABI baseline is blind to enum
variants), **E-057** (the QEMU profile reader splits markers on commas and
brackets).
**Touches** *(indicative)*: `.github/workflows/ci.yml`,
`tools/fjell-consistency-check/`, `tools/fjell-abi-snapshot/`,
`crates/fjell-tools/src/qemu_run.rs`, `tools/fjell-ci-coverage/` (deleted),
`docs/src/compliance/standards-mapping.md`, the root `Cargo.toml`.
**Relates to:** E-014 (instruments deciding by fixed-string match — all four are
its family); RFC-0.32-003 (which published the book these citations break in).

## Summary

Four independent instruments, one shape: **each reports success over a smaller
set than its name implies, and none of them says so.**

### Finding 1 — CI names its test packages by hand (E-049)

`ci.yml` contains **85** `-p fjell-…` entries across three jobs. Ten crates
appear in none of them, so their `--lib` tests run nowhere in CI — 118 tests,
including `fjell-sig-ed25519` and `fjell-replay-cache`. Some entries test
nothing: a service crate with no lib target, passed with `--lib` beside crates
that have one, is **skipped silently**. The tool meant to catch this,
`fjell-ci-coverage`, runs nowhere and reports in both directions wrongly.

**Measured replacement:** `cargo test --workspace --lib --exclude fjell-proptest
--features fjell-sxt-crypto/crypto-profile-development` — **49 crates, 580
tests, exit 0** — covering all ten.

### Finding 2 — the book cannot follow its own citations (E-052)

**Twenty-three** links in `compliance/standards-mapping.md` (and one in the
release cycle) are relative paths that leave the book: correct on disk, **404 on
the site**, with `.md` rewritten to `.html`. They were left deliberately, because
`standards-mapping` and `evidence` resolve a citation **as a filesystem path** and
refuse an absolute URL — so converting them turns Gate 12 red.

### Finding 3 — the ABI baseline is blind to enum variants (E-056)

The snapshot hashes an item's **declaration line**. For an `enum` that is
`pub enum SyscallNumber {`, so retiring `Reboot = 120` and adding
`PlatformReboot = 18` — both syscall-ABI changes — registered **`Added: 0,
Removed: 0, Changed sig: 0`**. New `pub fn`/`pub const` items are caught
correctly.

### Finding 4 — a profile's markers can be silently shortened (E-057)

`qemu_run.rs`'s array reader does `t.split(',')` and stops at the first `]`,
inside quoted strings too. `semantic.toml` documents the bracket half costing it
**2 of 4** markers; RFC-0.33-001 hit the comma half. Both fail **open**: the tier
passes on less text than its author wrote.

## The settled part

**D1 — CI runs the workspace-derived invocations, not hand lists** (E-049). The
lib run joins the bins/tests run that `ci-host-bins` already derives, from **one**
shared definition used by Gate 1, `test-all` and CI — the three cannot differ.

**D2 — A subcheck refuses a hand-written package list in a CI test job**,
demonstrated failing. Without it, D1 is one edit from being undone.

**D3 — `fjell-ci-coverage` and `[workspace.metadata.fjell.ci_excluded]` are
deleted.** Nothing else reads the metadata; the tool polices a list that will no
longer exist.

**D4 — `standards-mapping` and `evidence` accept an absolute repository URL as a
citation** (E-052), and the citations become URLs that work from the site and
from a clone. A citation that resolves nowhere is still refused.

**D5 — The ABI item hash covers an enum's variants** (E-056), and the baseline is
re-recorded **once**, with the drift that accumulated invisibly **named** rather
than absorbed.

**D6 — The profile reader respects quoting** (E-057), and a marker containing a
character the reader cannot carry is **refused at load** rather than silently
split — the existing workaround in `health-fail.toml` then comes out.

**D7 — Each of the four is demonstrated failing**: a crate added with no CI
entry still tested; a hand list reintroduced and refused; a citation that
resolves nowhere refused while a URL passes; a variant added and removed, each
seen; a marker with a comma matched whole.

## The open questions

**§A — Does the lib run replace the three `-p` jobs, or join them?** Replacing
them loses per-job parallelism and the feature isolation the v0.7-formats job
has. **Lean: replace, and pass the feature explicitly** as `host_bin_test_argv`
already does — but measure the wall-clock cost and say it.

**§B — What does D2 actually forbid?** A `-p` in a *test* job, or any `-p`?
`ci-check` legitimately names packages for `cargo check`. **Lean: test jobs
only**, with the rule stated in the subcheck's own message.

**§C — E-056's re-record will show real drift.** What is the rule for what it
reveals: absorb with a note, or stop and review each? **Lean: name every change
in the commit message and review the list here** — additive variants are expected;
a *removed* variant nobody noticed is a finding.

**§D — Does D4 belong to those subchecks' owners?** RFC-0.27-003 and -004 built
them. **Lean: change them here**, because E-052 is their defect and splitting the
fix across lines is how it stayed unfixed — but say what their tests now assert.

**§E — Order.** Four errata, one line. **Lean: E-057 → E-056 → E-052 → E-049**,
smallest and most independent first, so a red gate is always attributable.

**Answer all five in writing before implementing.**

## Requirements

**R1 — Re-derive** all four findings, with `/usr/bin/grep -a` and a control per
absence: the 85 entries, the ten crates, the 23+1 citations, the enum blindness
(by adding and removing a variant), and the comma split.

**R2 — §A–§E answered in writing.**

**R3–R6 — D1/D2/D3, D4, D5, D6**, each with its demonstration (D7).

**R7 — The counts stated after**: how many tests CI now runs that it did not,
how many citations resolve from the site, what the ABI re-record revealed, and
that `health-fail.toml`'s workaround is gone.

**R8 — E-049, E-052, E-056, E-057 CLOSED** or survivors named; register and
`v1-limitations.md` in the same commit.

**R9 — The gates**, each by its own exit status, `test-all`, the published site
checked for the converted citations (RFC-0.32-003 R8's shape), and a CI run id.

### Non-goals

- Making the kernel host-testable (E-013's separate, still-open question).
- Adding tests to crates that have none.
- Changing what the compliance mapping *claims* — only how it cites.
- Rewriting the profile loader as a real TOML parser, if quoting is enough.

## Risks

**Replacing the `-p` lists hides per-crate feature breakage** behind workspace
feature unification: `cargo test -p fjell-sxt-crypto --lib` alone fails its own
guard today, and a workspace run passes only because another crate enables the
feature. Passing it explicitly is not optional, and §A must say what is lost.

**The ABI re-record is the moment to be most careful** (§C): a baseline
regenerated without reading it is the instrument this project has twice found
reporting on nothing.
