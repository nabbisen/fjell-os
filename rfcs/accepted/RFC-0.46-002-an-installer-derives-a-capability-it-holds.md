# RFC-0.46-002: An installer derives a capability it holds; it does not fabricate one from an integer

**Status:** Accepted — accepted 2026-11-21
**Milestone:** 0.46
**Tracks.** The first of the three places 0.45 put cap-broker inside the kernel's trust boundary: *"install
a capability to object N"* is accepted without the kernel asking whether the broker holds anything on N.
This is the property named for 0.46 in `crates/fjell-cap/src/install.rs`, in `v1-limitations.md` row 26
and in the carried-forward bullet of the erratum that tracked RFC 056. No register entry is resolved by this
line on its own.
**Touches** *(indicative)*: `crates/fjell-cap/src/install.rs`, `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-syscall/src/lib.rs`, `crates/fjell-abi/src/service.rs` (one table),
`crates/fjell-kernel/src/task/spawn.rs`, `crates/services/fjell-cap-broker/src/main.rs`,
`crates/fjell-tools/src/policy_eval.rs`, `tests/qemu/profiles/{policy,audit}.toml`,
`tools/fjell-consistency-check/src/{endpoint_envelope,capability_topology}.rs`, `tests/abi/snapshot.json`,
prebuilts and `tests/repro/baseline-digests.txt`.
**Relates to:** **RFC-0.45-002** (this line is the property it deferred), **0.46 small item C**
(`endpoint-envelope`, which this changes the meaning of), **RFC-0.46-003** and **-001** (the other two
bounds), **E-123**, `v1-limitations.md` row 26 (`Scheduled: **0.46**` — moves if this line is dropped).

---

## What is true now

`sys_cap_install` bounds six things: kind, rights, target, lease, `object_id != 0`, and `object_id` names an
allocated endpoint. It does **not** bound *which* allocated endpoint. The installer names an integer in a
register and the kernel builds a capability to it. Demonstrated on a real boot at the 0.45 follow-ups: asking
for object 22 — `configd`'s endpoint — installs successfully, `policy` stays green at 13 markers, and only
the `audit` tier (which pins `"arg1":23,`) notices.

`cap-broker` holds **no capability to any object it brokers**. It holds a policy table of integers
(`PolicyRule.object: Option<u32>`), a `CapInstall` capability, and a `LeaseAdmin`. This line is therefore not
a kernel check bolted on: it changes what cap-broker *is* — from a service that **describes** grants to one
that **holds the authority it delegates**.

## The design decision, and the two shapes considered

**Chosen — the call names the capability it derives from.**

```
sys_cap_install(install_cap, src_cap, rights, lease)  ->  handle in the served task's CSpace
```

`src_cap` is a slot in the **installer's own** CSpace. The kernel does not accept an `object_id` at all: the
object, the kind and the ceiling on the rights come from `src_cap`. The integer is **unrepresentable**, which
is the move 0.45-002 made with the target (no target argument) and 0.44 made with `Option<Digest32>`.

The kernel checks, in `check_install`'s existing order, replacing today's object checks:

| | Check | Error |
|---|---|---|
| c′ | `src_cap` resolves to a live `Endpoint` capability in the installer's CSpace (`require_cap`-style: generation, state, lease) | `InvalidCap` / `WrongType` |
| c″ | the object is not endpoint object 0 and is allocated (**kept**: a held capability to an unallocated object is a state the kernel should still refuse to propagate) | `PermissionDenied` / `InvalidArg` |
| d′ | `rights ⊆ src_cap.rights` **and** `rights ⊆ envelope` (the installer's `CAP_INSTALL` capability) | `PermissionDenied` |

`ObjectScope`: the installed capability inherits the **source's** scope — derivation never widens, the
rule `Capability::derive` already enforces, reused rather than re-implemented.

**Rejected — the kernel compares against the installer's whole CSpace** (*"does the installer hold any
Endpoint capability to object N with at least these rights?"*). It keeps the integer in the ABI, so the
fabrication shape survives as a lookup; it makes the answer depend on every capability the installer happens
to hold (a confused deputy about *which* one authorised the install); and it cannot say "this install was
derived from that capability" in the audit record. The named-capability form is also the smaller kernel
change: no search.

## Requirements

**R1 — the ABI.** `a2` is `src_cap` (a handle), not `object_id`. `sys_cap_install(install_cap, src_cap,
rights, lease)`; one wrapper, the old one deleted (two wrappers, one of which takes a forgeable integer, *is*
the shape 0.45-002 R1 refused). `CapInstalled`'s audit record keeps `arg1 = object_id`, now read **from the
source**, so the `audit` tier's `"arg1":23,` assertion keeps pinning the same fact from the other end.

**R2 — the broker comes to hold what it delegates.** At spawn, `spawn.rs` installs into `cap-broker`'s CSpace
one `Endpoint` capability per **brokered endpoint**, at fixed slots, each carrying the rights the policy may
confer on it. The set is one table in `fjell_abi::service`:

```
BROKERED_ENDPOINTS: [BrokeredEndpoint { object, rights, broker_slot }]
```

read by **both** `spawn.rs` (to install) and the broker (to name its sources). This is deliberate: the
alternative — the broker's policy table and `spawn.rs` each spelling the pairs — is a *new* hand-kept copy,
the family 0.46 small item C just gated for the envelope. One table, two readers.

**R3 — the policy table's `object` field becomes a source slot.** `PolicyRule.object: Option<u32>` →
`PolicyRule.source: Option<BrokeredEndpointId>` (an index into `BROKERED_ENDPOINTS`). A rule can no longer
name an arbitrary integer: it can name only something the broker holds. Today there is one such rule
(`NEG_TEST` → `NEG_TEST_OWN_EP_OBJECT`, `SEND`), so `BROKERED_ENDPOINTS` has **one** entry.

**R4 — the kernel half is checked red by mutation, on a real boot.** The scratch experiment of the 0.45
follow-ups — the broker asks for object 22 — becomes a *standing* demonstration: with the broker's request
pointed at a slot it does not hold (or a slot holding a different object), the install is refused, `policy`
goes red, and the refusal is audited as `cap.install_rejected`. Today that experiment is green in `policy`
and red only in `audit`.

**R5 — `endpoint-envelope` (0.46 small item C) is updated, not retired.** Its claim — the envelope equals
the union of the broker's `Endpoint` grants — stays true and still gates drift. It gains the sibling claim
that **for every `BROKERED_ENDPOINTS` entry and every rule naming it, `rule.rights ⊆ entry.rights`**: a rule
that confers more than the broker holds would die `PermissionDenied` at runtime, the same fail-closed
confusing-UX failure the envelope check exists to prevent.

**R6 — `capability-topology` sees the sources.** They are *spawn-time* installs, so the instrument that
could not see 0.45-002's runtime install now sees the authority the runtime install derives from. The broker
declares its source slots as `CapHandle` constants (as it already does for `CAP_INSTALL_CAP`) and the
instrument judges them against `spawn.rs`. This is a real improvement over 0.45, where the gate on a
runtime install was the audit trail alone.

## The cost, stated rather than hidden

**Holding is authority.** A derive-what-you-hold rule means the broker **must hold at least the rights it
confers**. For the one brokered endpoint that is `SEND` on object 23, `neg-test`'s own scratch endpoint:
the broker thereby gains the ability to **send to** it, which it did not have. That is the direct price of
the property, and it is small today because the one rule confers only `SEND`.

It scales badly in one direction, and the RFC names it so the next rule is written with it in mind: a rule
that confers **`RECV`** (or `MINT`/`COPY`) on endpoint E requires the broker to hold `RECV` on E, i.e. to
be able to **take messages out of another service's endpoint**. `ENDPOINT_ENVELOPE` includes `RECV`, and the
policy table contains `EP_RW` rules (for `init`, `service-manager`, `netd`, `secure-transportd`) that today
never reach an install because their `object` is `None`. **If any of those is ever given a source, the broker
acquires `RECV` on a real service endpoint.** I propose the constraint go in as a rule of the RFC — *a rule
that names a source may confer only rights the architect has approved the broker holding on that object* —
and that R5's check enforces `rights ⊆ entry.rights` with the entries written by hand in review, so the
growth is visible in a diff of `BROKERED_ENDPOINTS`.

**A second consequence, honest about its limit.** The broker does not need to *use* what it holds; nothing in
its source sends to object 23. The kernel cannot express "hold for delegation only". Adding such a flag
(`CapRights::DELEGATE_ONLY`, a right that is checked at `sys_cap_install` and refused by every other
syscall) is the clean version and a larger change to the rights lattice; I did not propose it. The
alternative is to accept the small SEND as the cost; say which.

## Non-goals

- Installing kinds other than `Endpoint` (`INSTALLABLE_KINDS` is unchanged; a kind grows with a consumer).
- Making `secure-transportd`'s `CAP_SESSION` deliver — no object exists for the `SxtSession` class to
  name, and that is unchanged by this line.
- Changing `ObjectScope` handling beyond "derivation never widens".
- A second never-allowed `Allow` in the production table (0.46's explicit prohibition): the one fixture
  stays one.

## Risks

- **ABI break.** `sys_cap_install`'s signature changes (arg 2's meaning). The only callers are the broker and
  `neg-test`'s refusal cases; the snapshot is regenerated deliberately, as 0.45-002 did.
- **The broker's CSpace grows** by one slot per brokered endpoint; `CSPACE_SLOTS` is 64 and the broker holds a handful.
- **Slot-0 / object-0 discipline.** If a brokered endpoint were ever object 0 the source would be refused by
  c″ at install time rather than at spawn. R2's table should refuse object 0 at *build* (a `const` assertion),
  not discover it on a boot.
- **E-122 / E-073 / E-093 / E-095 are not touched.**

## If this line is dropped

Say so explicitly and **move `v1-limitations.md` row 26's `Scheduled: **0.46**`** with it. Row 26 is the
architect's; tell them. The line is the droppable one: it is the largest design change, and the only one that
alters what a service *is*. A half-derived installer — the kernel check without the broker holding sources —
would be worse than either, because it would refuse every grant.

## What this needs from the architect

1. **The shape** — the call names the source capability (chosen) rather than the kernel searching the CSpace.
2. **Whether the cost is acceptable**: the broker holding `SEND` on object 23 now, and the rule that any
   future source confers only rights the broker may hold. And whether to go further and add a
   `DELEGATE_ONLY` right (not proposed).
3. **`BROKERED_ENDPOINTS` as one shared table** in `fjell-abi` (R2), read by `spawn.rs` and the broker.
4. What happens to the policy table's `object` field — replaced by a source index (R3), as proposed, or kept
   beside it as a redundant check.

## Rulings — architect, 2026-11-21 (`.git-exclude/reviewed/review-record-0.46-stage-1.md`)

**Accepted. The decisions this RFC asked for are settled here, and the requirements below are added to it; reasoning is in the review record.**

* **The call names the source capability; no CSpace search: accepted.** A search makes the kernel's decision
  depend on CSpace layout and is ambiguous when two holdings name one object; naming it is what "no ambient
  authority" means.
* **`DELEGATE_ONLY` is the right idea and the wrong milestone.** Enforcing a negative right needs a single
  funnel for rights checks, and there is none (~20 `require_cap*` sites plus at least six direct
  `cap.rights.contains(...)` checks in `cap/syscall.rs`). **Accept the small `SEND`.** Recorded for 0.47:
  consolidate capability-rights checks behind one funnel.
* **New requirement: `BROKERED_ENDPOINTS` must be the *only* way a rule acquires a source**, so giving one is
  impossible without editing the reviewed table (an `EP_RW` rule given a source would make the broker hold
  `RECV` on a real service endpoint).
* **The policy's `object` field stays and does not merge into `BROKERED_ENDPOINTS`.** The policy says who may
  receive what; the table says what the broker holds. **0.46 small item C's subcheck then compares the
  two** — the comparer that makes keeping them separate safe.
* **002 remains the droppable line.** If dropped: say so, and the architect moves row 26's schedule.
