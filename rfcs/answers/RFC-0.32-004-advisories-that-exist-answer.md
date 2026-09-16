# RFC-0.32-004 §A–§E — where the dependency check runs, where advisories live, how they relate to errata, and SBOM

**Governing RFC:** [../done/RFC-0.32-004-advisories-that-exist.md](../done/RFC-0.32-004-advisories-that-exist.md)

Answered after R1 and before implementation, in the handoff's order. §D is the
owner's; it is proposed here, not decided.

---

## What R1 established

Every finding reproduces, each absence with a control that finds something:

- **Neither artefact exists.** `docs/src/security/` holds four files, none of
  them `advisory-process.md`, and no `advisories/` directory. `find . -name
  'FSAD-*'` returns 0; the same `find` for `SECURITY.md` returns 6.
- **The placeholder is live** at `release-checklist.md:209`:
  `security@<domain>` *(fill in before v1.0 landing)*.
- **243 packages in `Cargo.lock`, 153 third-party** — agreeing with the RFC —
  though that is package-*versions*: it is **147 distinct crates**, because
  some appear more than once (`getrandom` at three versions).
- **Both published crates carry no third-party dependencies**, verified for
  each: `cargo tree -p fjell-os -e normal` is two lines (`fjell-os` →
  `fjell-abi`), and `fjell-abi` has **no dependencies at all**. The kernel's
  normal tree for `riscv64gc-unknown-none-elf` is eight lines, every one
  `fjell-*`. Control: the same filter on `fjell-sig-ed25519` prints
  `ed25519-dalek` and its tree. (My first control, on `fjell-sxt-crypto`,
  printed nothing — because that crate has no third-party dependencies either —
  and so proved nothing; it was replaced rather than counted.)

### Four things wrong beyond the RFC's findings

1. **The dependency check is red on today's tree.** The RFC's third
   demonstration adds a vulnerable crate in a scratch clone, which assumes
   there is none. There are two:

   | Advisory | Crate | Kind | Patched | Reaches |
   |---|---|---|---|---|
   | **RUSTSEC-2026-0204** | `crossbeam-epoch 0.9.18` | vulnerability | `>=0.9.20` | `criterion` → `rayon` → … — a **dev-dependency of `fjell-benchmarks`** |
   | **RUSTSEC-2026-0190** | `anyhow 1.0.102` | unsound | `>=1.0.103` | in the lock only via `wit-bindgen`/`wasm-metadata`; `cargo tree --target all` resolves it into **no build graph at all** |

   Neither reaches a published crate, the kernel, or a service. That is the
   D5 distinction doing its job on day one. It is filed as **E-053** and
   handled below.
2. **`SECURITY.md` says "Fjell's direct external dependency surface is nine
   crates."** It is **seven**: `aes-gcm`, `argon2`, `ed25519-dalek`,
   `getrandom`, `proptest`, `sha2` as normal dependencies, and `criterion`
   dev-only. The sentence beside it — *"the kernel itself has none"* — is
   true.
3. **The checklist's template has already drifted from the RFC's.**
   RFC-v0.15-003 §3.4 specifies 12 fields; the checklist's copy has 10, having
   lost **Mitigation** and **Reproducer**. D1's premise — duplicated process
   text drifts — was already true.
4. **The RFC promises a CVE id its template has no field for.** §3.5: *"The
   advisory record carries the CVE id once assigned."* A register check that
   rejects unrecognised fields would reject the first advisory that ever got
   one. `CVE` is therefore a recognised **optional** field.

Two smaller ones: the template writes `Fixed in: vA.B.C`, but **none of this
repository's 95 tags has a `v`** (89 `N.N.N`, 6 `N.N.N-alpha.N`); and the
checklist's toolchain step runs `grep "1.98.1"` under a comment that says
*Expected: line containing "1.91"*.

---

## §A — the check runs in CI, against the live database, and the cut reads it

**Decision: shape 3 without a vendored snapshot — CI against the live RustSec
database, with the database's commit id recorded, and the cut reading that
record.**

The RFC's shape 3 does not need a vendored snapshot, and dropping it removes
§3's worst failure outright. **Reproducibility comes from the commit id, not
from a copy**: `cargo-audit` reports the database's `last-commit` and
`last-updated` in every run, and any past result can be re-derived by checking
out that commit of `rustsec/advisory-db`. A vendored snapshot would buy
offline operation at the price of a directory that goes stale silently — E-037
in the one place where staleness means a known vulnerability stays unknown.
This project has no offline requirement that justifies that trade.

**Why not a local gate.** Every Gate in `release-rehearsal` is offline and
deterministic, and RFC-0.31-003 declined a network gate on that ground. This
does not break that rule; it follows RFC-0.31-002 D7 instead — **gates stay
local and CI records** — the same way exit criterion 9 reads a CI run rather
than re-running it.

**When it runs:** on every push (it takes seconds, and catches a change that
*adds* a vulnerable dependency), on the weekly schedule (catching an advisory
*published* against an unchanged lockfile — the case the RFC exists for), and
on dispatch.

**The unreachable-database rule, written now rather than on the day:**

> A cut requires a **green dependency-check run against the release commit's
> exact `Cargo.lock`**, whose database is **no older than 7 days** at the time
> of the cut. The release record names the run id, the database commit and its
> date.
>
> If the database is unreachable on the day of the cut, **a green run from the
> previous 7 days against the identical `Cargo.lock` satisfies the rule**, and
> the record says so explicitly. Nothing older does, and nothing against a
> different `Cargo.lock` does. **Otherwise the cut waits.**

The cut does not proceed "with a note". A check that can be waived by writing
that it was not run is a note, not a check. The 7-day window exists only so a
third-party outage cannot hold a release indefinitely, and it is bounded and
recorded rather than discretionary.

**A red check blocks the cut**, unless the finding is recorded as an erratum
and **ACCEPTED under the existing rule** — by the architect, with a limitation
in `v1-limitations.md`. That is the path every other known defect takes, and a
dependency advisory is not special.

**Tool: `cargo-audit`, pinned `0.22.2`.** It does exactly D4 and nothing
else, needs no configuration file, and reports in its own JSON the three
numbers D5 needs — dependency count, database commit, database date. The
broader `cargo-deny` checks licences and sources too, which is not this line.
Installed in the CI step with `--locked --version 0.22.2`, never added to
`[workspace.dependencies]`, and the local command documented in the process
document is checked against CI's by `toolchain-declarations`, the way
`docs/MDBOOK.lock` is.

## §B — advisories live in the book

**Decision: `docs/src/security/advisories/`, with the index in navigation.**

This applies RFC-0.32-003's maintained/dated line rather than making an
exception to it. The test D12 draws is **whether a document is edited after
the date it records**: a release record never is. **An advisory is**, and
legitimately — §3.5 of the process adds the CVE id after disclosure, an
affected range can widen when a second report arrives, and a backported fix
adds a "Fixed in" version. A document that must be kept true as facts arrive
is maintained, and maintained documents live in the book.

It is also the one document here a user outside the project goes looking for,
which is the test RFC-0.32-003 was named for.

## §C — separate registers, cross-referenced, because of embargo

**Decision: two registers, linked both ways. An advisory is not an erratum
class.**

The decisive difference is not audience or format. **It is disclosure
timing.** An erratum is public the moment it is committed — that is how this
project works, and why the register is trusted. An advisory describes a
vulnerability that **must not be public until a fix ships**. Making advisories
a class of erratum would mean either committing an embargoed vulnerability to
a public register, or keeping one kind of erratum out of the register until
later — and a register that is sometimes silently incomplete is exactly what
Gate 7 is built to stop.

So, in order:

1. **During embargo**, the report and the fix are worked in GitHub's private
   security advisory. No erratum is filed; nothing public exists.
2. **At disclosure**, both are written in the same commit: the `FSAD-` record
   in the book, and an erratum filed **CLOSED** by the fix.
3. **Each names the other.** The advisory's `References` field names the
   erratum; the erratum's resolution names the advisory.

**What a reader follows:** an operator starts at the advisory — what is
affected, what to upgrade to, what to do meanwhile — and follows `References`
to the erratum for what was actually wrong and how it was found. A maintainer
starts at the erratum and follows it to the advisory for the public statement.

**Recorded rather than decided, because it is policy:** whether a defect
found *internally* in a *released* version warrants an advisory. Who found it
should not decide; whether a shipped version carries an in-scope, exploitable
defect should. E-046 — an unsound IPC decode path in released versions — is
the case that would test it. **This line does not apply that retroactively**,
and names it for the owner.

## §D — proposed, not decided

**Proposal: 7 days.** The two documents will carry one placeholder,
`{{ACKNOWLEDGEMENT-WINDOW}}`, until the owner chooses.

- **72 hours is a promise one maintainer cannot keep through an ordinary
  week away.** It is the right number for a project with two responders.
- **"A small number of days" is not a commitment at all.** A reporter who
  hears nothing on day 5 cannot tell whether they are waiting or ignored, and
  whether to follow up or go public.
- **7 days is the shortest honest one**: a reporter knows exactly when to
  follow up, and a single maintainer can meet it through a normal absence.

**A cost the owner should weigh:** `.github/SECURITY.md` is what GitHub shows
reporters. Until the placeholder is replaced it will show a token where a
promise should be. I would rather the owner decide before this ships than after.

## §E — SBOM is out of scope; the seam is named

**Agreed: out of scope.** An SBOM is a format and distribution question —
CycloneDX or SPDX, attached to which artefact, signed by what — and none of
those is an advisory question.

**The seam:** this line's inventory is `Cargo.lock` as `cargo-audit` reads it,
plus the surface statement. An SBOM generator (`cargo-cyclonedx`,
`cargo-sbom`) consumes the same lockfile; nothing built here would change for
one. What an SBOM needs and this line does not provide: component hashes,
licences, a supplier field, and a distribution channel. `CRA-II-1` stays
**not-met**, correctly.
