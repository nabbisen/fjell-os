# Developer Handoff — RFC-0.42-001

**Governing RFC:** [RFC-0.42-001](../../accepted/RFC-0.42-001-every-capability-has-a-revocation-path.md)
**Milestone:** 0.42 · the milestone's lead; independent of 0.42-003 (done) and 0.42-002
**Status:** inherited from the governing RFC (Accepted, 0.42)
**Audience:** implementation model

---

## 1. Read the RFC and the investigation first

The RFC carries R1–R6 and the non-goals. `.git-exclude/reviewed/investigation-0.42-001-revocation-coverage.md`
carries **why the shape is what it is** — in particular why per-task anchoring is refused on
arithmetic rather than taste. **Where this document and the RFC disagree, the RFC governs**; say so
in the review request rather than choosing.

## 2. The defect in one paragraph

The derivation tree **does** form — `Capability::derive()` sets `parent: Some(self_slot)` and
inherits `lease: self.lease`. What is missing is an anchor: 58 grant sites create `Active`
capabilities with `parent: None` *and* `lease: None`, and `derive` inherits that `None`, so
everything descended from a grant is unrevocable. **And nothing distinguishes "lease-less because I
am the root of authority" from "lease-less because nobody set one."** R3 and R4 are about making
that distinction exist and then enforcing it; R1 and R2 are RFC-v0.7.4-003's unimplemented
`LEASE_BIND` and lease-scope check.

## 3. Mechanics and traps

- **The ABI snapshot will move, and this is expected.** R3 changes a type in `fjell-cap`, which is
  in the snapshot's crate list. Regenerate in this line's own commit and **say in the review
  request that movement was predicted** — trap 3 calls a non-zero `Added` "a finding, not cut
  work", and that wording is about *unexpected* movement.
- **Prebuilts move**: `spawn.rs` and `main.rs` change, so service and kernel artefacts shift.
  `cargo xtask build-services` **then** `build`, then `rm tests/repro/baseline-digests.txt` and
  `cargo run -p fjell-repro-check -- --skip-build --record-baseline`, **in the same commit**, and
  account for every moved line.
- **Do not regenerate `releases/trust-report.txt`** — reverted mid-line, regenerated at the cut.
- **Do not touch `MAX_LEASES`, lease slot recycling, or `sys_cap_bind_lease`'s existing
  authority gate** (E-088, closed). All three are non-goals with reasons.
- `/usr/bin/grep -a` for anything in a `.bin`. One `test-all` at a time. Stage by explicit path and
  check with `git diff --cached --name-only` — not `git add -A`, and not `git add -A <dir>`.
- The shell starts in the repository; no `cd` is needed, and a relative one if you use it.

## 4. Evidence the review request must carry

1. **R5's invariant check shown red against the current tree first**, then green — as
   RFC-0.42-003's five tests were. A check that only ever passes does not establish it would catch
   this.
2. **R6's audit line**: `0x0604` in a real boot's trail for a refused out-of-scope bind, quoted.
3. **The ABI diff**: `Added N`, every added item named, no removals, no signature changes.
4. **The baseline `--numstat`** with every moved line accounted for.
5. `build-services && build` with **0 warning lines**.
6. **Full `cargo xtask test-all`**, tier counts. `store` stays NOT-GATED (expected).
7. `consistency-check --all` (21 subchecks), `fmt --check`, mmio-audit, unsafe-audit,
   `abi-snapshot --verify`.

**If a tier goes red, report which and do not re-run to green.** `storaged: io incomplete` is
**E-104** — closed on a cause, disclosed as limitations row 19, a known host-latency intermittent:
quote it and hand me the serial log.

## 5. Stop and escalate, rather than implement, if

- **A grant site cannot supply a lease** under R4. **Do not declare it root to make the check
  pass.** "Declared root" is a claim about authority; an over-broad root declaration would reproduce
  E-093 with better paperwork. That shape is mine to rule.
- **R2 breaks a service** that binds a lease it does not own today. That is a finding about
  authority, not a regression to soften.
- **R3's distinction seems to want `MAX_LEASES` raised, or slot recycling.** It should not — R3/R4
  were chosen so the line does not need either. If you find otherwise, that is the most useful
  thing you could tell me, and it means my arithmetic is wrong.

## 6. Not yours

`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`, `docs/src/compliance/standards-mapping.md`,
the done RFC's annotation (R1 — mine), the `io incomplete` line, `do_io`, and any tag. **Criterion 8
is not yours**: if you believe a row is false, say so in the review request and do not edit it.
