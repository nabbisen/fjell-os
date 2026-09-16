# Security Advisory Process

What happens between *a vulnerability is reported* and *a fixed release
ships* — and what does not happen, stated as plainly as what does.

This is the one statement of the process. The release checklist and
`.github/SECURITY.md` refer to it rather than restating it; the last time the
process was written in two places, the two copies disagreed about the intake
address, the acknowledgement time and the record format (erratum E-051).

## Read this first: what the project can promise

Fjell OS is **pre-1.0 and has one maintainer.** Every time below is a target
the maintainer works to, **not a guarantee**. A single person cannot guarantee
a 30-day patch for a vulnerability whose fix turns out to need a kernel
redesign, and this document does not pretend otherwise. What it does promise is
narrower and keepable: one place to report, an acknowledgement inside a stated
window, a severity decision you are told about, and no public disclosure by the
project before a fix exists.

## 1. Reporting

**There is one channel:** a private security advisory on the repository —
<https://github.com/nabbisen/fjell-os/security/advisories/new>. It is private
to the reporter and the maintainer, and it is where the fix is worked until
disclosure.

There is no security mailing address. An earlier version of the release
checklist named `security@<domain>`, a placeholder that could not receive mail;
it has been removed rather than filled with a second channel.

**Acknowledgement:** within **7 days** of the report. If you
have heard nothing by then, follow up on the same advisory.

Include the release tag or commit, the QEMU profile or board, the
configuration, and the smallest reproduction you have.

## 2. Triage

Severity is decided against the system invariants I1–I8, defined in
[v1.0 Direction and Identity](../identity/v1-direction.md), and against the
threats in [the v1 threat model](./threat-model-v1.md):

| Severity | Meaning |
|---|---|
| **Critical** | Defeats an invariant I1–I8 directly, affects all deployed nodes, and has no operator-side mitigation. |
| **High** | Defeats one invariant under a realistic adversary; an operator-side mitigation exists but is not obvious. |
| **Medium** | Weakens an invariant's defence in depth; the operator-side mitigation is straightforward. |
| **Low** | Cosmetic, denial-of-service only, or requires an unrealistic adversary. |

**Severity decision:** within **14 days** of the report. The reporter is told
the severity and the reason — and because the fix targets below run from this
decision, a bound on it is what makes them mean anything. The report receives an
identifier of the form `FSAD-<year>-<seq>`, assigned in order of disclosure.

## 3. Fixing, and how long it takes

| Severity | Target, from triage to a fixed release |
|---|---|
| Critical | 30 days |
| High | 60 days |
| Medium | 90 days |
| Low | the next regular release |

**These are targets.** When one will be missed, the reporter is told before
the date passes, with the reason, and the advisory record says so.

## 4. Disclosure

The default is **coordinated disclosure**: the reporter is asked not to discuss
the issue publicly until a fixed release exists. In return, the target above is
the longest the project asks a reporter to wait. **If a target passes without a
fix, the reporter is free to disclose** — the project does not ask for an
indefinite embargo in exchange for a promise it did not keep.

The project itself **publishes nothing before the fix ships**: no issue, no
commit message that describes the vulnerability, no erratum.

## 5. The record

At disclosure, one record is committed to
[`advisories/`](./advisories/), and its index updated, in the same commit as
the corresponding erratum (§6).

The record is a Markdown file named `FSAD-<year>-<seq>.md` whose fields are
lines of the form `**Field:** value`:

| Field | Required | Content |
|---|---|---|
| ID | yes | `FSAD-2026-001` — matches the file name |
| Severity | yes | `Critical`, `High`, `Medium` or `Low` |
| Reported | yes | `YYYY-MM-DD` |
| Disclosed | yes | `YYYY-MM-DD` |
| Affected | yes | the released versions affected, as tags: `0.20.0..0.31.0` |
| Fixed in | yes | **a tag that exists**, as `git tag` spells it: `0.32.0`, no `v` |
| Reporter | yes | a name, or `anonymous` with the reporter's permission |
| Description | yes | what an attacker can do, and under what conditions |
| Threat ref | yes | a threat id from the threat model: `T13` |
| Mitigation | yes | what an operator can do before upgrading, or `none` |
| References | yes | the fix commit, the erratum it corresponds to, related RFCs |
| Reproducer | yes | `available` or `withheld` |
| CVE | no | the CVE id, once one is assigned — often after disclosure |

The shape is checked on every run of `consistency-check` by the
`security-advisories` subcheck: every required field present, no field it does
not recognise, ids unique and consecutive within a year, the index and the
directory agreeing, and **"Fixed in" naming a tag that was actually made** —
the one field a reader acts on. The register is empty today, and the check
passes on it, so the first record ever written is validated by an instrument
that has been running since before it was needed.

A record is **maintained, not frozen**: the CVE id is added when it arrives,
and an affected range is corrected if a second report widens it.

## 6. Advisories and errata

They are separate registers, and each names the other.

The difference that decides it is **timing**. An erratum
([`rfcs/ERRATA.md`](https://github.com/nabbisen/fjell-os/blob/main/rfcs/ERRATA.md))
is public the moment it is committed. An advisory describes something that
**must not be public until it is fixed**. So during the embargo no erratum is
filed; at disclosure, the advisory record and an erratum — **CLOSED** by the
fix — land together. The advisory's `References` names the erratum; the
erratum's resolution names the advisory.

**An operator** starts at the advisory: what is affected, what to upgrade to,
what to do meanwhile. **A maintainer** starts at the erratum: what was wrong,
how it was found, what changed.

**When a defect found internally gets an advisory** (owner, 2026-09-16). An
advisory is published when a defect reaches something a user obtains as a
release artefact — the published crates, `fjell-os` and `fjell-abi`. A defect
confined to code no published crate contains is recorded as an erratum only.
The first case ruled under this is E-046, an unsound decode path in service
code: **no advisory**. The question is asked again the first time a defect
reaches a published crate.

## 7. Advisories against dependencies

A published advisory against a crate in `Cargo.lock` is found **mechanically**,
not by chance. CI runs [`cargo-audit`](https://github.com/rustsec/rustsec) on
every push, on the weekly schedule and on demand, against the live
[RustSec advisory database](https://github.com/rustsec/advisory-db). Every run
reports how many packages it covered and the database commit it read.

**What that check does and does not cover.** The two published crates,
`fjell-os` and `fjell-abi`, have **no third-party dependencies** — `cargo tree
-p fjell-os -e normal` is two lines, and `fjell-abi` depends on nothing — and
neither does the kernel. A green run is therefore **not** a statement about
what Fjell OS ships. It is a statement about the build and host surface: the
tools, tests, benchmarks and the development-grade crypto crate, which is where
every third-party crate in the lockfile lives.

A finding is an erratum like any other.

To run the same check locally — the same script CI runs, with the same tool:

```bash
cargo install cargo-audit --locked --version 0.22.2
.github/scripts/dependency-advisories.sh
```

Run the script rather than `cargo audit` alone. The script is what states the
surface above, covers every `Cargo.lock` in the tree rather than only the
root's, and refuses a result read from a stale database: when the database
cannot be fetched, `cargo-audit` falls back to its local cache **without
saying so**, and a months-old cache produces a report that looks exactly like
a fresh one.

## 8. Releases

A release is not cut on an unread dependency check. The rule — a green run
against the release commit's exact `Cargo.lock`, no more than 7 days old, with
what happens when the database is unreachable — is in the
[release cycle](../releasing/v0-release-cycle.md), because that is where a cut
reads it.
