# Developer Handoff — RFC-0.42-001

**Governing RFC:** [RFC-0.42-001](../../accepted/RFC-0.42-001-every-capability-has-a-revocation-path.md)
**Milestone:** 0.42 · the milestone's lead; independent of 0.42-003 (done) and 0.42-002
**Status:** inherited from the governing RFC (Accepted, 0.42)
**Revised 2026-11-10 for Amendments 1 and 2.** Your two escalations both held: R3, R4 and R5 are
**withdrawn**, E-093 has **left this line**, and the scope is now **E-091 only**. Read the RFC's
Amendment 2 first — it carries the rulings you asked for.
**Audience:** implementation model

---

## 1. Read the RFC and the investigation first

The RFC carries R1–R6 and the non-goals. `.git-exclude/reviewed/investigation-0.42-001-revocation-coverage.md`
carries **why the shape is what it is** — in particular why per-task anchoring is refused on
arithmetic rather than taste. **Where this document and the RFC disagree, the RFC governs**; say so
in the review request rather than choosing.

## 2. The defect in one paragraph

`RFC-v0.7.4-003` is marked Implemented and specified a `LEASE_BIND` right, a lease-scope check and
audit code `0x0604`. **None of the three exists.** So a `LeaseAdmin` holder may bind an unbound
capability to any `Active` lease, including one it did not create. That is all this line is now.

*E-093 — that authority cannot be withdrawn from a task without ending it — is **no longer part of
this RFC**. Your measurement showed there is no propagation to guard, so no propagation check is to
be built here, and `cap-anchor:`'s boot walk is dropped: without an invariant it would be the
`store` tier's shape, markers nothing produces.*

## 3. Mechanics and traps

- **The ABI snapshot will move, and this is expected** — from **R1** (`CapRights::LEASE_BIND` in
  `fjell-cap`) and the **`AuditKind` variant** (`fjell-audit-format`), both in the snapshot's crate
  list. Regenerate in this line's own commit and state that you predicted it, as you did in the
  escalation.
- **Prebuilts move**: `spawn.rs` and `main.rs` change, so service and kernel artefacts shift.
  `cargo xtask build-services` **then** `build`, then `rm tests/repro/baseline-digests.txt` and
  `cargo run -p fjell-repro-check -- --skip-build --record-baseline`, **in the same commit**, and
  account for every moved line.
- **Do not regenerate `releases/trust-report.txt`** — reverted mid-line, regenerated at the cut.
- **Do not touch `MAX_LEASES`, lease slot recycling (E-110), or `sys_cap_bind_lease`'s existing
  authority gate** (E-088, closed). All three are non-goals with reasons. **Nor build a propagation
  check** — there is no path to attach it to.
- `/usr/bin/grep -a` for anything in a `.bin`. One `test-all` at a time. Stage by explicit path and
  check with `git diff --cached --name-only` — not `git add -A`, and not `git add -A <dir>`.
- The shell starts in the repository; no `cd` is needed, and a relative one if you use it.

## 4. Evidence the review request must carry

1. **R6's audit line**: `0x0604` in a real boot's trail for a refused bind, **quoted**. Under
   ruling (ii) this falls out for free — `neg-test` attempting to bind a lease `sample-service`
   owns, while it is `Active`, is refused.
2. **The `NEG:IPC:*` markers still emitted** after the two test protocols are reversed:
   `BLOCKED_RECV_WAKES_ON_REVOKE` and `BLOCKED_CALL_WAKES_ON_REVOKE`. The protocols change; the
   markers must not.
3. **The ABI diff**, with the movement **you predicted** stated as predicted: `Added 1`
   (`CapRights::LEASE_BIND`) and **`Changed sig 1` (`AuditKind`)**. Say in the request that both
   were expected, so trap 3's "a finding, not cut work" reads against *unexpected* movement.
4. **The baseline `--numstat`** with every moved line accounted for.
5. `build-services && build` with **0 warning lines**.
6. **Full `cargo xtask test-all`**, tier counts. `store` stays NOT-GATED (expected).
7. `consistency-check --all` (21 subchecks), `fmt --check`, mmio-audit, unsafe-audit,
   `abi-snapshot --verify`.

**If a tier goes red, report which and do not re-run to green.** `storaged: io incomplete` is
**E-104** — closed on a cause, disclosed as limitations row 19, a known host-latency intermittent:
quote it and hand me the serial log.

## 5. Stop and escalate, rather than implement, if

Your last two escalations were both right and each cost me a withdrawn ruling, so this list is
shorter and sharper than the one it replaces:

- **Reversing the `sample-service`/`neg-test` protocols changes a `NEG:IPC:*` marker, or costs one.**
  The protocols are yours to rewrite; the markers are the tier's contract and are not.
- **The owner-only rule refuses a bind in some path neither of us has named.** You found the two;
  a third would mean the property is wrong, not that the path is.
- **The `AuditKind` variant moves more than `Changed sig 1`.** You predicted exactly that; anything
  beyond it is a finding.

## 6. Not yours

`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`, `docs/src/compliance/standards-mapping.md`,
the done RFC's annotation — both R1's `1 << 25` note and **R2's deliberate narrowing of its `caller_has_lease_admin_over` clause** (mine), the `io incomplete` line, `do_io`, and any tag. **Criterion 8
is not yours**: if you believe a row is false, say so in the review request and do not edit it.
