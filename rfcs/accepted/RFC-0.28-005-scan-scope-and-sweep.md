# RFC-0.28-005: Three tools, three different answers to "what is this repository?"

**Status:** Accepted — by the owner (nabbisen), 2026-09-08; implementation may begin (RFC 000)
**Milestone:** 0.28
**Tracks.** The three errata dated `0.28` with no RFC behind them: **E-025**,
**E-029**, **E-030**. E-025 and E-030 share a root; E-029 is small, dated to
this milestone, and swept here rather than slipped again.
**Touches.** `crates/fjell-tools` (`trust_report`), `tools/fjell-unsafe-audit`,
`tools/fjell-consistency-check` (`version-currency`),
`docs/rfcs/RFC-0.26-004-readiness-channel-answer.md`, `tests/evidence/`.
**Does not touch the kernel, the ABI, or any service.**
**Relates to:** **E-015** (hand-enumerated instrument scopes drifting) — E-025
is a live instance; RFC-v0.22-001 (every instrument change demonstrated
failing); RFC-0.27-004 (`tests/evidence/`, which E-029 uses).

## Summary

Three errata carry the `0.28` milestone and none has a line. Two of them are the
same defect.

**A tool that walks the filesystem has to decide what counts as "the
repository." Three tools decide differently, none of them authoritatively:**

| Tool | Excludes |
|---|---|
| `crates/fjell-tools/src/trust_report.rs:122` | `target`, `.git`, `tests/runs` |
| `tools/fjell-unsafe-audit/src/main.rs:280` | `target`, `.git`, `node_modules` |
| `tools/fjell-consistency-check/src/evidence.rs` | `target`, `.git`, **`.git-exclude`** |

Three hand-written lists, pairwise disagreeing. **Only the newest is right**, and
it is right because RFC-0.28-001's implementer added `.git-exclude` unprompted
after watching E-025 happen.

**E-025 is what the disagreement costs.** A scratch checkout under
`.git-exclude/tmp/` — which this project's own conventions put there — is walked
by the first two. Demonstrated live: the trust report's cap-manifest count goes
1 → 2, and its **unsafe-site inventory doubles, 311/311 → 622/622**. Both
readings are internally consistent, so nothing in the output says which
repository it describes.

**E-030 is the same shape one level up**: `version-currency` decides what to
check by naming `README.md`, and the version is written in **two** places that
must agree — `[workspace.package] version` and `crates/fjell-os/Cargo.toml`'s
`fjell-abi` pin. It sees one of them. When they disagree the workspace does not
resolve, so it fails loudly rather than silently; it cost the 0.27.0 cut its
first command.

**E-029 is unrelated and small**: two historical QEMU citations were annotated
rather than re-manufactured (correctly), and the `semantic` one can now simply
be re-run, because `tests/evidence/` exists.

## The settled part

**D1 — Scope is derived, not enumerated.** A tool's answer to "what is this
repository" must come from `git`, not from a literal list a person maintains.
`git ls-files` is the authority; the three lists above are E-015's family caught
in the act.

**D2 — One answer, in one place.** If three tools need to bound a walk, they
must agree, and agreement maintained by hand is what produced the table above.
Whatever mechanism is chosen, **there is one of it.**

**D3 — E-029's annotation is superseded, not deleted.** The note in
`RFC-0.26-004-readiness-channel-answer.md` is the honest record of what was true
between 0.26 and 0.28. A new citation supersedes it; the annotation stays. The
archived `RFC-0.26-002` citation is left alone — a `Superseded` RFC's
point-in-time measurement, per RFC-0.27-002's precedent.

**D4 — Every change here is an instrument change** (RFC-v0.22-001). Each must be
demonstrated failing on deliberately broken input **before** it is trusted:
E-025 with a scratch checkout present, E-030 on a deliberately mismatched
version pair.

**D5 — E-025's fix must be shown against the real symptom**, not a unit test
alone: the trust report generated with a checkout under `.git-exclude/tmp/`,
before and after, showing 622 → 311.

## The open question — §6

**Should the tools scan what `git` tracks, or the filesystem minus exclusions?**

1. **Tracked files only** (`git ls-files`). Principled, exactly answers "what is
   this repository", and immune to anything anyone leaves lying about.
   **The cost is real and points the wrong way for one tool:** `unsafe-audit`
   would stop seeing a new `.rs` file until it is committed — and uncommitted
   work is precisely when you want an unsafe-site audit to speak up.
2. **Filesystem minus a shared, single exclusion set.** Keeps uncommitted files
   visible; keeps a hand-maintained list, but only one of them.
3. **Per-tool, deliberately**: `unsafe-audit` walks the filesystem (it audits
   code, committed or not); `trust-report` reports on the repository and uses
   `git ls-files`. Two answers, each argued, rather than three by accident.

**Answer in writing before implementing.** I lean to 3 — the tools genuinely
have different jobs, and forcing one answer may be the tidier mistake — but it
is a lean, and shape 1's simplicity is worth arguing for.

## Requirements

**R1 — E-025.** `trust_report`'s `find_cap_manifests` and `fjell-unsafe-audit`'s
`walk` bounded per §6. Demonstrated per D5.

**R2 — E-030.** `version-currency` checks that `[workspace.package] version` and
`crates/fjell-os/Cargo.toml`'s `fjell-abi` pin agree, demonstrated failing on a
mismatched pair. The release-cycle step written at the 0.27.0 cut stays; the
check is what makes it more than a note.

**R3 — E-029.** Re-run the `semantic` profile, promote the log with
`cargo xtask evidence promote` and real provenance, cite it from
`RFC-0.26-004-readiness-channel-answer.md` **alongside** the existing annotation
(D3).

**R4 — All three errata `CLOSED`**, register and `v1-limitations.md` in the same
commit.

## Scope

The three tools named, the one answer document, `tests/evidence/`, and the three
errata entries.

### Non-goals

- **E-015 itself.** This closes one instance and does not take on the family.
  If the work suggests the family is now cheap to close, that is an escalation.
- Changing what any instrument *reports* — only what it looks at.
- E-013, E-027, E-028, E-034, E-035.
- Any kernel, ABI, or service change.

## Risks

**The numbers will move, and the right ones must move.** If `unsafe-audit`'s
total changes on a clean tree after this line, that is a **finding**, not a
target — the count should be 311 before and after on a tree with no scratch
directories. A number that moves without a scratch checkout present means the
new bound excluded something real.

**"Bounded by git" sounds obviously correct**, which is how shape 1 gets chosen
without the `unsafe-audit` cost being noticed. §6 exists because the obvious
answer is wrong for one of the three tools.
