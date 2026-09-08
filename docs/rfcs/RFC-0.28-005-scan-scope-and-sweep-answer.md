# RFC-0.28-005 §6 — Should the tools scan what `git` tracks, or the filesystem minus exclusions?

**Governing RFC:** [rfcs/accepted/RFC-0.28-005-scan-scope-and-sweep.md](../../rfcs/accepted/RFC-0.28-005-scan-scope-and-sweep.md)

Written before R1 was built, per the handoff's required order — R1 is only
real once it survives contact with both walkers named below.

---

## Answer: shape 3, and it is not "one rule for all three" in disguise

The two tools this line touches ask genuinely different questions, and the
fix is `git` answering each of them differently — not one hand list
consolidated into fewer files (shape 2), and not one literal answer
imposed on both (shape 1).

**`fjell-unsafe-audit` audits code, committed or not.** Uncommitted work is
exactly when an unsafe-site audit should speak up — a developer mid-change
adding a raw pointer deref without a `SAFETY:` comment is the case this
tool exists for, and `git ls-files` alone (tracked-only) would make it
blind to that file until it is committed. Its question is **"what belongs
to my working copy right now"**, and the derived, non-enumerated answer to
that is:

```
git ls-files -z --cached --others --exclude-standard
```

Tracked files, plus untracked files `.gitignore` does not exclude. A new,
uncommitted `.rs` file is still audited. A scratch checkout under
`.git-exclude/tmp/` is not — not because its name is hand-listed anywhere
in this tool, but because `.gitignore`'s `/.git-exclude/` entry already
says it doesn't belong to the working copy, and `--exclude-standard` reads
that same file `.git` itself already uses.

**`trust_report`'s cap-manifest scan reports on the repository as
committed.** Its question is **"what does this repository (as shipped)
contain"**, and that is `git ls-files` with no `--others` at all: tracked
files only, immune to anything sitting in a scratch checkout regardless of
whether `.gitignore` happens to cover it.

**Why this isn't shape 1 wearing shape 3's argument.** Both derive from
the same underlying authority — `git` — but the flag difference is
substantive, not incidental: `--cached --others --exclude-standard` and
plain `--cached`-only answer two different questions, and each tool uses
the one that matches its actual job. If the two invocations had turned
out identical, that would be the "one rule for the wrong reason" the
handoff warned to watch for; they don't, because the two tools' jobs
genuinely differ on the one axis that matters (does uncommitted work
count).

**Shape 2 (a single shared hand-list) was rejected, not merely
not-chosen.** Consolidating the three lists in the RFC's table into one
shared list would have stopped the three tools from *disagreeing*, but it
would not have stopped the list itself from drifting again — it is still
an enumerated set of names a person maintains, D1's actual complaint
("a literal list a person maintains" vs. "git is the authority"), just
with one copy instead of three. `.git-exclude` was added to the third
tool's list "unprompted, after watching E-025 happen" — the next
scratch-directory name will need the same kind of luck under shape 2. It
needs none under shape 3: `.gitignore` already names what shouldn't count,
`git` already reads it, and neither tool needs to know the name.

**Shape 1 (tracked-only, everywhere) was rejected** for the reason the
handoff states plainly: it is wrong for `fjell-unsafe-audit` specifically,
and forcing one answer onto a tool it doesn't fit would trade E-025's
disagreement for a new, quieter defect — a real unsafe site sitting
uncommitted and un-audited until the next commit.

**`tools/fjell-consistency-check/src/evidence.rs`'s own walker is left
alone**, per the RFC's own scope (it is not in "Touches"): it already
answers correctly with a hand list, is not part of the pairwise
disagreement this RFC exists to close, and this line does not import it
into scope on the theory that "one mechanism" must mean literally one
walker everywhere — D2 is about the tools that must agree because their
outputs are combined (`trust_report`'s report includes both its own
cap-manifest count and `unsafe-audit`'s shelled-out total), not about
every tool in the repository that happens to walk a directory.

---

## R1 (E-025) — demonstrated against today's tree, not the RFC's cited numbers

**A finding, checked rather than assumed:** the RFC's own cited numbers
(`622/622` broken, `311/311` clean) are from the 2026-08-28 measurement
and are **stale relative to `HEAD`** — re-running the *unmodified* tool on
today's clean tree already reports `284/284`, not `311/311` (confirmed by
`git stash`-ing this fix and running the old code standalone). The
codebase's actual unsafe-site count has moved in the intervening line
(most plausibly RFC-0.28-002's syscall-asm consolidation, which removed
several raw `asm!` sites); this is not a regression this line introduced,
and `docs/release/trust-report.txt` itself is separately known-stale
(still reads `311`) for the same, already-established reason — it is a
release artefact regenerated only at a cut, not this line's concern.

**The required demonstration, redone against the real number:**

A `git clone --depth 1` of this repository into
`.git-exclude/tmp/rfc-0.28-005-demo-checkout/` (the same shape as the
architect's own original clean-clone verification that triggered E-025)
was created, and `cargo xtask trust-report --dry-run` run against it, in
both directions:

| | Before this fix | After this fix |
|---|---|---|
| Cap-manifests found | **2** (the tree's + the checkout's) | **1** |
| Total unsafe sites | **568** (`284 × 2`) | **284** |
| With SAFETY comment | 568 | 284 |

`fjell-unsafe-audit --check` run standalone against the same tree state
confirms the same 568 → 284 (its own total is what `trust_report`'s §5
shells out to and reports verbatim).

**The other direction, also required:** on the clean tree (checkout
removed), the count is **284 before and 284 after** — it does not move.
This is the invariant the RFC's own risk section names: a number that
moves on a clean tree would mean the new bound excluded something real.
It does not.

---

## R2 (E-030) — demonstrated on a deliberately mismatched pair

`version-currency` now also parses `crates/fjell-os/Cargo.toml`'s
`fjell-abi` version pin and compares it against `[workspace.package]
version`. Demonstrated against the **real CLI**, not a unit test alone:
`crates/fjell-os/Cargo.toml`'s pin was temporarily changed from `0.27.0`
to `0.26.0` (the exact mismatch the 0.27.0 cut actually hit) and the
already-built `fjell-consistency-check` binary invoked directly —
necessary because, as the erratum itself documents, the mismatch stops
`cargo` from resolving the workspace at all, so `cargo run` cannot reach
the check in this state:

```
--- consistency-check: version-currency ---
version-currency: FAIL
  crates/fjell-os/Cargo.toml's fjell-abi version pin is 0.26.0, but the
  workspace version is 0.27.0 -- this is fail-closed already (a mismatch
  stops `cargo metadata` from resolving, so every gate, tier, and build
  fails with it too); this check only saves the time of discovering that
  the hard way
```

Reverted immediately after (`git diff crates/fjell-os/Cargo.toml` empty,
confirmed before continuing); the workspace resolves and
`cargo run -p fjell-tools -- consistency-check version-currency` reports
`PASS` again. 7 unit tests cover the pair check (2 new:
`mismatched_fjell_abi_pin_fails`, `agreeing_fjell_abi_pin_passes`, plus a
parser test), all 9 `version_currency::` tests passing.

The check's own `FAIL` message states the fail-closed caveat explicitly,
per the handoff's instruction: this buys the time of a cheap, named
failure instead of a `cargo metadata` stack trace at a cut — it does not
add correctness a passing run didn't already have.

---

## E-015 — not taken on

Building this line did not make E-015's family look cheap to close in
general. The two fixes here worked because `git` was already recording
the exact answer each tool needed (tracked files; tracked-plus-untracked-
not-ignored) — that will not generalise to every hand-enumerated list in
this project without checking each one on its own terms. No escalation is
warranted from this line's evidence.
