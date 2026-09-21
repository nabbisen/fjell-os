# RFC-0.33-002: Who Fjell is for

**Status:** Proposed
**Milestone:** 0.33
**Kind:** identity-level (required by `v1-non-goals.md`'s own rule: *"Changes
require an identity-level RFC"*, RFC-v0.15-005).
**Tracks.** **E-054** — the documentation contradicts itself about who Fjell is
for: inclusion is a founding pillar in the requirements and absent from every
page a reader meets first.
**Touches** *(indicative)*: `docs/src/intro/`, `docs/src/identity/`,
`docs/src/releasing/v1-non-goals.md`, `docs/src/requirements/`,
`docs/src/releasing/v1-limitations.md`, `docs/src/external-design/abdd-semantic.md`.
**Does not touch code.**
**Relates to:** ADR-0005 (semantic-stream-first), ADR-v0.5-005 (the proxy is
output-only), FR-SEM-001…005, the 2026-07-31 system proposal §6.3.

## Summary

The owner's question — *why is Fjell not for general-purpose computing, when
inclusiveness is one of its key concepts?* — turned out to be a question about
the documentation, not the architecture.

### Finding 1 — inclusion is a founding pillar, in the founding document

`fjell-os-requirements-v1-20260504.md` closes:

> The crucial point is treating ABDD not as an "additional accessibility
> feature" but as **a design principle for separating display from processing so
> the OS can carry semantic streams safely**. This makes formal verification,
> minimal Unix, sustainability, and **inclusion** all point in the same
> direction.

§2.6 makes ABDD one of six design principles. §3.1 lists *"devices requiring
integration with accessible external UIs"* among **primary** targets. §8 makes
"GUI-independent semantic-stream design" a **Must**.

### Finding 2 — what §4.1 actually excludes, and what it does not

§4.1 — *"Do Not Aim to Become a General-Purpose Desktop OS"* — lists: a full
desktop environment, a consumer app store, gaming, video/3D workstation use,
drop-in compatibility for existing desktop apps. §8 labels the same item
**"Won't (initial phase)"**.

**It is a statement about GUI stacks, application-ecosystem breadth and
sequencing. It excludes no person.** The architectural non-goal worth keeping is
a different one: §4.5, *no GUI rendering stack in the OS core* — which is what
makes presentation a proxy's job, and therefore what makes inclusion possible at
all.

### Finding 3 — what actually narrowed the audience, and where

Not §4.1. Two later statements, and one silence:

| Where | What it says |
|---|---|
| `v1-non-goals.md` N3 rationale | *"Fjell targets headless edge/fleet nodes (A1/A2/A3)"* — a rationale that quietly redefines the target set |
| `identity/v1-direction.md` | lists *"Desktop / laptop user environments"* under what Fjell is not |
| `intro/what-is-fjell.md`, `intro/why-fjell.md` | **no mention of accessibility, ABDD, or inclusion at all**; the three archetypes are industrial |

Meanwhile the same book's requirements chapter still lists accessible-UI devices
as a primary target. **A reader cannot tell from the book who Fjell is for.**

The 2026-07-31 system proposal §6.3 chose this deliberately — lead with
operational semantics, present accessibility as "the co-equal benefit it
genuinely is" — and called it *"a positioning change… not for code"*. The
positioning was applied; the co-equal half was not written down anywhere a
reader arrives.

### Finding 4 — the architectural gap inclusion does have: input

`FR-SEM-001` includes *"input requests"* in the Intent Stream. `FR-SEM-005`
requires basic operation from a console, an API, a proxy or tooling.
**ADR-v0.5-005 makes `proxy-text` output-only**, deliberately: an input path
through the proxy would bypass capability policy. Operator input goes through
`fjell-tools` over a separate channel.

So a person operating the system through a proxy — which is what a blind user
does — has no path *in*. That is a real design question with a real reason
behind the current answer, and it is the one thing on this subject that code
must eventually settle.

### Finding 5 — one proxy, and no conformance claim anywhere

`fjell-proxy-text` is the only proxy; the book calls it the reference and says
audio and braille are later. **No accessibility standard is claimed anywhere in
the tree** — no EN 301 549, no Section 508, no WCAG. Nothing to retract, and
nothing to lean on.

## The settled part

**D1 — Inclusion is named a primary goal**, co-equal with assurance, in the
pages a reader meets first: `what-is-fjell.md` and `why-fjell.md`. Stated as a
goal and a mechanism, never as delivery.

**D2 — §4.5 stands unchanged.** No GUI rendering stack in the core. It is not a
limit on who Fjell serves; it is the reason presentation can be anyone's.

**D3 — §4.1 is re-stated to say what it means**: no desktop replacement, no
application-ecosystem breadth, no drop-in compatibility — at v1, as its own §8
already said. **The sentence must not read as "not for personal or assistive
use", because the requirements never said that.**

**D4 — N3's rationale and the identity document are corrected.** *"Fjell
targets headless edge/fleet nodes"* becomes the accurate statement: Fjell
targets nodes whose interface is meaning rather than pixels — which includes
headless industrial nodes **and** nodes operated through an assistive
presentation.

**D5 — A fourth archetype.** A1–A3 are industrial. **A4 — a node operated
through an accessible presentation**: the same core, a proxy that speaks,
prints braille, or simplifies. It is the archetype the requirements' §3.1
already named and the book never carried.

**D6 — `v1-limitations.md` gains the honest counterpart**: what a person needing
speech, braille or simplified presentation cannot do today — one text proxy, no
input path through it (ADR-v0.5-005), no applications, no validated hardware, no
tested conformance. **A primary goal with no limitations section is how a goal
becomes an overclaim.**

**D7 — No conformance claim** until something is tested against a standard with
real assistive technology. Architecture and roadmap may be claimed; conformance
may not.

**D8 — This RFC changes documents, not code.** The proxy work is 0.34's (a
second presentation proxy, already approved by the owner).

**D9 — v1.0 has to demonstrate inclusion, so the readiness matrix gains rows**
(owner, 2026-09-22). The matrix's **80 rows mention ABDD, accessibility and
proxies nowhere** — not deferred, absent — so v1.0 could be tagged with no
inclusive capability and nothing would ask. Three rows, as release criteria:

| Row | Bar |
|---|---|
| A second presentation modality, end to end | speech or braille, driven by the same Intent Stream as `proxy-text`, observed in a QEMU tier |
| An input path, decided | a recorded decision — an ADR — for how a person operating through a proxy reaches the system without bypassing capability policy (§C). A decision, not necessarily an implementation |
| Accessibility limitations, written | D6's section, kept true at each cut |

**They are added as `**IN PROGRESS** → v1.x`, never `**OPEN**`.** Gate 5 blocks
on `**OPEN**` alone, and a row that is honest about future work must not redden a
release that never claimed it. *(That is also why this RFC states the marking:
the last time a row's wording met an instrument by accident, it was E-014's
family.)*

**D10 — The roadmap says v1.x for richer proxies** (owner, 2026-09-22), tied to
the second-proxy line already approved for 0.34 — which lands well before any
v1.0 tag. This **supersedes** both untracked papers: the system proposal's
"v2+ … richer proxies" and the deep-research report's "v1.5 Operator and proxy
expansion". They disagreed with each other; the tree's `ROADMAP.md` is what
counts, and it now says v1.x.

**The adaptive Personal Proxy** — continuous state measurement, per-user
optimisation — is **not** what D10 schedules, and this RFC does not schedule it.
It stays where the requirements put it: a "Could", beyond v1.x, until the owner
says otherwise.

## The open questions

**§A — Does general-purpose *personal* computing become a long-term goal, or
stay out?** D3 keeps it out of v1 as the requirements do. The owner's question
suggests the long-term answer may be different, and this RFC deliberately does
not decide it: a desktop-class personal OS implies an application model, an
input plane, display drivers and a font stack — the last of which §4.5 keeps out
of the core on purpose. **If the answer is "eventually yes", the honest place to
record it is a v2+ direction section, not a softened non-goal.**

**§B — What does "inclusive" commit to, concretely?** Candidates: speech output,
braille output, simplified presentation, and an input path. **My lean: name the
first three as roadmap and the input path as an open design question (§C)**,
because three modalities with one proxy is a promise, and one modality
demonstrated is evidence.

**§C — How does input reach the system without bypassing capability policy?**
ADR-v0.5-005's reason is sound; the answer is probably a capability-scoped input
channel with its own authority, not a relaxation of the proxy. **This RFC does
not settle it** — it names it as the architectural work inclusion actually
needs, for 0.34 or later.

**§D — Does A4 change the threat model?** A proxy on a personal device is a
different trust boundary from a proxy on the same node: T17's neighbouring class
(a correctly identified sender with a malformed payload) was just named at
0.32's cut, and an assistive proxy is exactly such a sender. **Likely yes, and
it should be a threat-model amendment rather than a sentence here.**

**Answer all four in writing before implementing.**

## Requirements

**R1 — Re-derive** Findings 1–5 against the originals in
`.git-exclude/specs/` and the tree, and report any disagreement.

**R2 — D1, D4, D5:** the intro pages, the identity document and N3's rationale,
with A4 written like A1–A3 (a concrete node, not an aspiration).

**R3 — D3:** §4.1 re-stated in the requirements chapter and N3's item, keeping
§4.5 untouched, with a note recording what the sentence used to imply.

**R4 — D6:** the limitations section, naming ADR-v0.5-005's output-only decision
as the reason there is no interactive path today.

**R5 — §A–§D answered in writing.** §C's answer may be "not yet, here is the
shape of the question".

**R6 — E-054 CLOSED**, or survivors named; register and `v1-limitations.md` in
the same commit.

**R7 — D9's three rows** in `v1-readiness.md`, marked `**IN PROGRESS** → v1.x`,
each with its bar written as the row's own text rather than in a comment. Run
`readiness-check` and quote its counts before and after: the DONE/IN PROGRESS
totals move, and Gate 5 must stay green.

**R8 — D10:** `ROADMAP.md` states v1.x for a second presentation modality and
names the 0.34 line; the adaptive Personal Proxy is recorded as unscheduled
rather than dated.

**R9 — The gates**, each by its own exit status, the book built under the pinned
mdBook, and the published site checked for the changed pages (R8's shape from
RFC-0.32-003).

### Non-goals

- **Writing a second proxy** — 0.34.
- **Deciding §A**, whether personal computing becomes a goal.
- **Implementing** the input path, or the second modality: D9 makes them v1.0
  criteria and 0.34 does the proxy; this line writes the rows, not the code.
- **Relaxing ADR-v0.5-005** or adding an input path.
- Claiming any accessibility standard.
- Changing FR-SEM-*, which already say what this RFC makes visible.

## Risks

**A goal stated without its limits becomes an overclaim**, and the audience it
would mislead is people who have been promised accessibility before. D6 is not
optional decoration; it is the condition on D1.

**"Inclusive" can absorb the roadmap.** Three modalities and an input plane is
years of work for a single maintainer. §B's lean — name modalities as roadmap,
demonstrate one — is what keeps the claim honest.

**The industrial framing is also true**, and the fix must not replace one
half-truth with another: A1–A3 are real targets with real buyers, and A4 joins
them rather than displacing them.
