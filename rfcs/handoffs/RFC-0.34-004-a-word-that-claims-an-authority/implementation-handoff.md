# Developer Handoff — RFC-0.34-004

**Governing RFC:** [RFC-0.34-004](../../proposed/RFC-0.34-004-a-word-that-claims-an-authority.md)
**Milestone:** 0.34
**Status:** inherited from the governing RFC (Proposed — **do not start until the
owner accepts it**; written in advance so acceptance is the only thing between you
and R1)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a presentation obtain a permission it does not hold by sending different
bytes?** Today: yes, and the function that decides says in its own comment that it
cannot. When the answer is no, and a tier shows the refusal, this line is done.

Not the measure: that a permitted action now does something. It does not, and
**closing E-059 must not read as "actions work now"** (D6).

## 0.1 The failing case is live, so run it first

Every other line this milestone had to construct its failing case. **This one is
already in the tree**: a presentation that sends a `granted_rights` word it did not
earn is believed. Demonstrate that **before** you change anything — a test-only
service, or a scratch edit to a presentation, asserting rights it does not hold and
being accepted. That transcript is half of D5, and it is the only moment you can
take it.

## 0.2 Settled — do not re-open

1. A receiver **never** authorises on a value the sender chose (D1).
2. The authority comes from what the kernel **attests or validates**, and which one
   is argued in writing (D2, §A).
3. **No comment claims a verification the code does not perform** (D3).
4. E-034's four helpers carry their word or stop taking it (D4).
5. Both transcripts, and a tier (D5).
6. Making an action **act** is out of scope, and the register's wording is checked
   against that (D6).

## 1. Order

**R1 (re-derive, and take the live failing transcript) → §A–§E in writing → R3 (the
mechanism) → R4 (the comment, and its neighbours) → R6 (the refused transcript and
the tier) → R5 (E-034's four helpers) → R7 (errata) → evidence.**

E-034 late because it is independent and small; the authority mechanism is the line.

## 2. §B decides how big this is, so answer it with a control

`sys_cap_inspect` inspects **your own** capability. Before §A can be chosen, establish
whether anything in this kernel lets a task ask about **another** task's rights —
probe it, and control the probe against a syscall you know exists. If nothing does,
say so plainly: the capability route then needs a new syscall, which is an ABI change,
Gate 4, and a different line — **escalate rather than write it here.**

## 3. §A's constraint is the whole risk

If you choose the attested-sender route, the policy must not become the next
hand-maintained table. The stream already compiles in its presentation rows with
`image_id` and a `PresentationKind` (RFC-0.34-003 D15 put the kind on the row for
exactly this reason) — **derive the policy from what is already there**, and if you
cannot, say what the new source of truth is and what binds it to the rows.

## 4. R4 — the comment, and the sweep

The comment at `semantic-stream/src/main.rs:60`–`:61` is the defect's twin. Make it
true or delete it, **and sweep the neighbourhood**: this project has filed a
comment-versus-mechanism erratum against itself three times in two milestones
(E-059's own, E-064's *"we do not touch it"*, E-062's buffer). One sweep now is
cheaper than a fourth entry.

## 5. Prohibited

- Authorising on anything the sender can choose, however indirectly.
- A new syscall without escalating first (§B).
- A policy that is a new editable list (§3).
- Writing an erratum closure that implies dispatch became real (D6).
- Carrying E-034's word by changing the tag packing in this line (§C) — that is an
  ABI change and its own line.
- A gate piped into `grep` instead of its own exit status.

## 6. Required evidence

1. R1's four re-derivations with controls, **and the live failing transcript**.
2. §A–§E in writing, §B with its control and §A with its binding argument.
3. The mechanism, and the one-sentence answer to *"what stops a sender influencing
   this?"*
4. The comment, and what the sweep found.
5. The refused transcript, and the tier that asserts it.
6. E-034's four helpers, with the caller check that justified §C's choice.
7. **E-059 and E-034** resolved or survivors named; register and `v1-limitations.md`
   in the same commit; D6's wording checked.
8. The gates, each by **its own exit status**, `test-all`, the repro baseline
   re-recorded in the same commit as any rebuild, and a CI run id.

## 7. Review request

Standard format, in `.git-exclude/review-request/`. Flag for focused review:

- **The live failing transcript**, first — the one thing that cannot be re-taken.
- **§B's answer and its control.**
- What binds the policy to the presentation rows.
- What the comment sweep found.
- Anything outside the *Touches* list, which is indicative.
