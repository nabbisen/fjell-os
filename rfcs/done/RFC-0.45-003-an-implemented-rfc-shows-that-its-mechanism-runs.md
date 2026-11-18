# RFC-0.45-003: An RFC marked `Implemented` shows that its mechanism runs

**Status:** Implemented (0.45.0) — accepted 2026-11-17
**Evidence:** file:tools/fjell-consistency-check/src/rfc_evidence.rs, test:tools/fjell-consistency-check/src/rfc_evidence.rs::rfc_056s_exact_case_is_red_while_the_syscall_is_undispatched, test:crates/fjell-tools/src/rfc_promote.rs::refuses_an_rfc_from_0_45_with_no_evidence_line_and_names_it
**Milestone:** 0.45
**Tracks.** The finding that outranks the other 0.45 lines (`rulings-2026-11-15-three-open-questions.md`):
three RFCs in `rfcs/done/` assert `Implemented` where the work has not shipped, and **no instrument
could have found a fourth**. Closes nothing in the register on its own; it is the instrument that
would have caught **E-123** (RFC 056), the handshake's RFC (`RFC-v0.7.3-001`) and **E-028**
(`RFC-v0.7.3-002`).
**Touches** *(indicative)*: a new `tools/fjell-consistency-check/src/rfc_evidence.rs`,
`tools/fjell-consistency-check/src/{main,lib}.rs` (a 23rd subcheck),
`crates/fjell-tools/src/rfc_promote.rs` (a refusal, R6), `rfcs/done/000-rfc-lifecycle-policy.md`'s
*reader* documentation only if the architect wants the field described there — **not** its
definitions.
**Relates to:** **RFC-0.45-002** (do it first: it makes RFC 056's status true and removes the
first and most awkward waiver), **E-014**'s class (an instrument that reports coverage it does not
have), the `evidence` and `threat-model` subchecks (the patterns copied), **RFC-0.43-004** (`rfc
promote`, the one place an RFC becomes `Implemented`).

---

## What is wrong

**RFC 000 gives `Implemented` a truth condition** — *"The work has shipped (in a release, on
`main`, or wherever the project's stability marker lives)"* — **and nothing checks it.**
`rfc-status-folder` compares an RFC's status string against the **folder** it sits in. That is its
documented job and it does it correctly, across the 229 RFCs in `done/` (217 `Implemented`, 12
`Implemented-with-Errata`, the handoff's measurement). It has never compared a status against
evidence, because there is no evidence field to compare against.

Three RFCs assert the status where the artefact does not run, each **found by a person reading**, in
three unrelated readings months apart:

| RFC | Carries | What is there |
|---|---|---|
| `056-cap-broker-capability-installation-primitive` | `Implemented` | `sys_cap_install` has no dispatch arm (E-123) |
| `RFC-v0.7.3-001-networking-runtime-completion` | `Implemented (v0.7.2)` | the handshake is an unconditional `Err`; its step 1 reads a `sys_net_recv` that exists nowhere |
| `RFC-v0.7.3-002-crypto-profile-documentation` | `Implemented (v0.7.1)` | neither specified document has ever existed (E-028) |

*"I found three by accident"* is not a bound. The `done/` folder is the project's own record of what
it has built; a reader is entitled to read `Implemented` as meaning implemented, and a record that may
mislead is a worse defect than any one thing the system does not do, because every other judgement is
made on top of it.

## The constraint that outranks the feature

**You cannot derive "the mechanism" from an RFC's prose, and a gate that pretends to would be worse
than none.** A checker that scanned an RFC for the syscalls and markers it *mentions* would report
coverage it does not have — **E-014's class, the very defect this line closes**. Three consequences,
which are the design:

1. **Evidence is cited, not inferred.** An RFC that claims `Implemented` carries a field naming
   **machine-checkable** evidence. The checker verifies the citation **resolves**.
2. **The output states its own limits.** *Resolving* is not *sufficient*: a cited marker proves the
   marker is gated, not that it covers the claim. The PASS line says so, in words, every run.
3. **Forward-only.** The field is required of RFCs from 0.45 on; older RFCs are **counted and
   reported as having declared no evidence**, never failed. Backfilling 229 is not this milestone.

## Requirements

**R1 — the field.** An RFC's header block may carry one line:

```
**Evidence:** syscall:CapInstall, marker:policy:NEG:POLICY:GRANT_INSTALLS_USABLE_CAP:PASS, file:docs/src/…, test:crates/…/src/lib.rs::fn_name
```

Comma-separated citations, each `kind:value`. Four kinds, each resolved against a **committed
artefact the tree already guarantees is current**, never against freshly re-derived source:

| Kind | Resolves when | Resolved against |
|---|---|---|
| `syscall:<Name>` | `SyscallNumber::<Name>` is declared **and not in `undispatched`** | `tests/syscall/expected.toml` — the file the `syscall-surface` subcheck already holds equal to the source in both directions. **One rule, not a second parser** (the lesson of 0.44 item D). |
| `marker:<profile>:<MARKER>` | the marker is in that profile's `expected_markers` | `tests/qemu/artifacts/<profile>/expected-markers.txt`, the tracked, derived rendering of the profile (a profile without one has no gated markers). |
| `file:<path>` | the path exists and is not empty | the working tree. |
| `test:<path>::<name>` | `<path>` exists and contains `#[test]` immediately before `fn <name>(` | the file's text. The weakest kind, and named as such. |

Any other kind, a malformed citation, or an empty value is a **failure naming it** — never a skip.

**R2 — who must carry it.** A `done/` RFC whose status keyword is `Implemented` or
`Implemented-with-Errata` **and whose id is 0.45 or later** (`RFC-<major>.<minor>-NNN`, compared
numerically) must carry at least one citation, and **every** citation it carries must resolve.
Older RFCs, `Superseded`, `Withdrawn`, `Closed`, and RFCs in `proposed/` and `accepted/` are not
required to; a citation they *do* carry is still resolved (an old RFC may opt in, and a dangling
citation is a defect wherever it is written).

**R3 — waivers, keyed by name, printed every run, with a reason.** The precedent is 0.44's
threat-model gate. A waiver is an in-code `(rfc id, reason)`; it applies to **any** `done/` RFC,
including pre-0.45 ones, because its purpose is to keep a *known-false* status visible rather than to
excuse a missing field. It is printed on every run. A waiver for an RFC that no longer exists, or
that now carries resolving evidence, is **a failure in both cases** — **a waiver is not a place to
park a name**.

*Corrected 2026-11-17, at the implementation review, by the architect who wrote both sentences: R3
said the first case was "a note (\"can be retired\")" while **R5 listed "a waiver for an RFC that
does not exist" among the cases to show failing**. The two contradicted each other and the dev team
had to choose; they chose R5 and were right, so R3 is corrected to match rather than left for the
next reader to adjudicate. The ground is that a note is a thing this project has proved it can carry
for milestones — T23's has outlived several — while a stale entry is the register asserting a problem
that no longer exists, which is exactly what this gate exists to catch; and unlike T23 the remedy is
one deleted line. The implementation also added a third case, an entry whose RFC is no longer
`Implemented`, which is accepted: the owner resolving the lifecycle question is precisely when the
entry must go.*

Initial waivers:

* **`RFC-v0.7.3-001`** and **`RFC-v0.7.3-002`** — each with its reason (the table above) and the
  statement that **their status lines are not touched**: the disposition is a lifecycle question
  reserved to the owner (RFC 000's five states offer nothing true for "shipped except for the
  load-bearing part", and `Implemented-with-Errata` is not defined in RFC 000 at all).
* **RFC 056 — none**, if RFC-0.45-002 has landed (its status is then true and, being pre-0.45, it is
  not required to cite anything). If this lands first it is waived until then. **The order taken is
  stated in the review request.**

**R4 — the output is the deliverable as much as the check.** One line, in this shape:

```
rfc-evidence: PASS (N evidenced — their citations RESOLVE, which is not a guarantee the mechanism is
sufficient; M done RFCs from before 0.45 declare no evidence and are NOT covered by this check;
K known-false status(es) waived by name: <ids>)
```

A reader must not be able to mistake it for a guarantee about the 217. Zero required RFCs
(`N = 0`) is **printed as such**, not hidden: at the time of landing no 0.45 RFC is `Implemented`
yet, so `N` is the count of RFCs that *opted in*, and the line says "0 required" while 0.45's RFCs
are still `accepted/`.

**R5 — it is shown failing first, in the places that matter.** On scratch copies, each restored
byte-exact: an `Implemented` 0.45 RFC with no field; a citation to `syscall:CapInstall` **while it is
undispatched** (the live control — this is RFC 056's exact case, and it must be red until
RFC-0.45-002 lands); a marker that is not in the profile; a missing file; a `test:` naming a function
that is not a test; a malformed citation; a waiver for an RFC that does not exist; and the three known
instances, shown to be reported by name.

**R6 — `rfc promote` will not produce an RFC the new check rejects.** (Optional, the architect's
call.) For a milestone ≥ 0.45, `cargo xtask rfc promote` refuses to promote an RFC that has no
`**Evidence:**` line, naming it — so the requirement is met at the moment the status becomes true
rather than discovered at the next `consistency-check`. Without R6 the cut finds the gap by going red
after the move.

### Non-goals

- **Backfilling evidence for existing RFCs.** (Counted, reported, never failed.)
- **Changing RFC 000**, **any RFC's status line**, or **filing errata.** The three known instances
  are *reported and waived*, never rewritten.
- **Judging whether evidence is sufficient.** The check proves a citation exists; sufficiency is a
  review question, and the output says so.
- **Deriving evidence from prose.** (The constraint above.)

## Risks

- **A check that gives false comfort.** The failure mode is a reader taking "PASS" as a statement
  about all `Implemented` RFCs. Mitigated only by R4's wording, which is why that sentence is a
  requirement; the architect should read it as carefully as the code.
- **The weakest kind (`test:`).** A function named in a file with `#[test]` before it can still test
  nothing relevant. It is offered because some RFCs have no syscall, marker or file to cite, and it is
  named as weakest.
- **`expected-markers.txt` is derived.** If a profile has no committed rendering, `marker:` cannot
  resolve against it; that is correct (an ungated marker is not evidence) and the failure says why.
- **Shape drift.** The field is a header line; an RFC that writes it as prose in the body is "no
  field". The check reads only the header block, stated in its failure message.
- **It will fire at the cut.** From 0.45 on, an `Implemented` RFC with no evidence turns Gate 12 red
  at the step-7 move (R6 moves that earlier).

## What this needs from the architect

1. The grammar in R1 — in particular that `syscall:`, `marker:`, `file:`, `test:` are the right four.
2. Whether R6 is in or out.
3. Whether the three known instances should be reported **separately** from future waivers (a
   `KNOWN_FALSE` list distinct from `WAIVERS`), since their disposition belongs to the owner and a
   future waiver would be an implementer's.

## Rulings — architect, 2026-11-17 (`.git-exclude/reviewed/review-record-0.45.md`)

**Accepted. The decisions this RFC asked for are settled here; reasoning is in the review record.**

* **The four-kind grammar (`syscall:`, `marker:`, `file:`, `test:`): accepted.** One addition: the
  output **breaks its count down by kind**, so a reader sees how much rests on `test:`, which this
  RFC correctly names the weakest. A count that hides its composition is what R4 exists to prevent.
* **R6 is in.** Refusing the promotion beats discovering it red after the move — **with 0.44 item
  D's lesson as a condition: one predicate, not a second parser.** `rfc promote` calls the
  subcheck's own function, exactly as it now calls `readme_counts::disagreements`.
* **`KNOWN_FALSE` separate from `WAIVERS`: yes.** The ground is **provenance**: a waiver is an
  implementer's judgement inside the gate, while these three are the **owner's pending lifecycle
  decision**. Two lists make the output say who owes the resolution, and stop the three being
  quietly reclassified as ordinary waivers later.
* **"`N` will be 0 required at landing": accepted, and the line must say so in words.** A gate whose
  first run could be read as coverage is the defect; one that says it covers nothing yet is honest
  and still worth landing, because the mechanism is the point, not this run's number.
