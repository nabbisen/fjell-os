# Developer Handoff — RFC-0.45-002

**Governing RFC:** [RFC-0.45-002](../../done/RFC-0.45-002-capability-installation-runs-and-the-brokers-grant-is-lease-bound.md)
**Milestone:** 0.45 · first line — implement this one first
**Status:** inherited from the governing RFC (Implemented, 0.45.0)
**Audience:** implementation model

---

## 1. Read the RFC and its Rulings section first

Eight requirements and a *Rulings* section that settles the five decisions you asked for. **The
Rulings are part of the RFC now** — reply-edge targeting, `Endpoint`-only, the `NEG_TEST` rule with
its two conditions, two audit variants, and the three other syscalls out of scope.

**Read RFC-0.42-001 before writing anything.** It set the rule your R3(e) copies — a lease must be
`Active` and **owned by the caller** — and it is the precedent for granting a right **by name**
rather than through an aggregate, which R2 needs.

## 2. The line in the sand

**R2 is what makes this line real, and it is the part E-123 itself got wrong.** No capability in the
tree carries `CAP_INSTALL`: it is bit 26, `ALL_NON_META` is `(1 << 26) - 1`, and both grants confer
`ALL_NON_META`. **Dispatching the arm with its promised validation and changing nothing else
refuses every caller** — a path that runs and refuses everyone, which reads as policy working. If
your implementation ends with the happy path refused, R2 is why.

**R8 is the deliverable, not the evidence.** Fourteen cases, each shown red by mutation. The one I
will read first is **grant then `CAP_REVOKE` kills the installed capability** — that is the check
`BROKER-004` exists to be, and without it R5 is a claim.

## 3. Yours to argue, not mine to dictate

* The shape of the non-consuming reply-edge read. `take_reply` (`cap/table.rs:144`) consumes;
  yours must not, and the reply must still be sent afterwards.
* Whether `sys_cap_install_with_rights`'s deletion needs a deprecation window. My view: no — it
  discards a parameter, which *is* E-011 — but say so if a caller surprises you.
* The `DelegationRecord`'s shape once it carries a real lease id.

## 4. Mechanics and traps

* **`ReplyEdge.lease` is `Option<LeaseBinding>` and `None` for bootstrap/unbound calls.** The
  install's lease is R3(e)'s, owned by the broker, and the call's own binding is irrelevant. Say so
  in the code, or the next reader will conclude an unbound call cannot receive an install.
* **R7 is the precondition of R5, not hardening.** The broker's `CAP_REVOKE` arm has no sender check
  (`cap-broker/src/main.rs:633-645`), while the `CAP_REQUEST` arm above it uses
  `ipc_sender_image_id`. It is latent **only** because the broker holds no `LeaseAdmin`. R5 grants
  one. Land them together or land R7 first.
* **`tests/syscall/expected.toml` moves to 36 / 33** and `undispatched` loses `CapInstall`. That
  file is what `syscall-surface` holds equal to the source — change it in the same commit.
* **E-110 becomes reachable. Do not fix it here.** Report whether it is live and from what call
  pattern; recycling is a separate item.
* **Name what gates a runtime install.** `capability-topology` only sees spawn-time installs. This
  is the system's first runtime-installed capability; if nothing else gates it, R8's audit assertion
  is the gate and the RFC must say so.

## Evidence the review request must carry

* The mutation table: **every required refusal shown red**, each by its own mutation, each restored
  byte-exact afterwards.
* A full `cargo xtask test-all` on the tip with this line in it, with the tier counts.
* If a prebuilt moved: `tests/repro/baseline-digests.txt` re-recorded **in the same commit**, and
  the `numstat` compared against the binaries the build reported as modified — **exactly those and
  no others**.
* `consistency-check --all` and `fmt --check`.
* **Anything you measured that contradicts this handoff or the RFC.** Three of the last four
  milestones had one, and every time the implementer was right to raise it.

## Stop and escalate if

* a requirement cannot be met without widening authority beyond what the RFC's Rulings settle;
* a gate you add would pass for a reason other than the one it claims to test;
* you find a register row or limitations row that is **false** rather than merely due an update.

## Not yours

`rfcs/ERRATA.md`, every `docs/src/releasing/v1-limitations.md` row, and every RFC status line.
Flag them in the review request rather than editing them.
