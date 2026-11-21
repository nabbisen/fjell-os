# Developer Handoff — RFC-0.46-002

**Governing RFC:** [RFC-0.46-002](../../accepted/RFC-0.46-002-an-installer-derives-a-capability-it-holds.md)
**Milestone:** 0.46 · third line — after 0.46-001. **The droppable one.**
**Status:** inherited from the governing RFC (Accepted, 0.46)
**Audience:** implementation model

---

## 1. What is left unbounded, and why it is the last field

`sys_cap_install` bounds kind, rights, target, lease, object ≠ 0 and object-allocated. It does **not**
bound *which* allocated endpoint. Demonstrated on a real boot at the 0.45 follow-ups: asking for
object **22**, `configd`'s endpoint, installs successfully, `policy` stays green, and only the `audit`
tier catches it.

**The rule this restores is the one the project is built on:** authority is derived and attenuated,
never minted from nothing. The installer must itself hold a capability to that object, carrying **at
least** the rights it confers.

## 2. The design decision, which is why this is an RFC

**cap-broker holds no capability to the objects it brokers** — it holds a table of integers. This
line changes what the broker *is*: from a service that **describes** grants to one that **holds** the
authority it delegates. How it comes to hold them is the central question and costs more than the
kernel half.

**The call names the source capability; no CSpace search.** A search makes the kernel's decision
depend on CSpace *layout*, and two holdings of one object make the answer ambiguous. Naming it is
also what *no ambient authority* means: the caller says which authority it is exercising.

## 3. Holding is authority — the price, and the constraint that bounds it

To confer `SEND` on object 23 the broker must hold `SEND` there, so **it can now send to it**. That is
small today. It scales badly in one direction: a rule conferring `RECV`, `MINT` or `COPY` on a service
endpoint would require the broker to hold that right — **to be able to take messages out of another
service's endpoint.** `ENDPOINT_ENVELOPE` includes `RECV`, and `EP_RW` rules exist for `init`,
`service-manager`, `netd` and `secure-transportd` that never reach an install only because their
`object` is `None`.

**Requirement: `BROKERED_ENDPOINTS` is the only way a rule acquires a source**, so giving one is
impossible without editing the reviewed table, and `endpoint-envelope` enforces `rights ⊆ entry.rights`.
A hazard that is free to add quietly gets added quietly.

**`DELEGATE_ONLY` is not taken.** A right that permits delegation and is refused by every other
syscall is the clean form, and enforcing a *negative* right needs a single funnel for capability
checks — there is none (~20 `require_cap*` sites plus direct `rights.contains` checks in
`cap/syscall.rs`). A right whose enforcement is scattered is **E-014 in the rights lattice**. The
prerequisite — one funnel — is recorded for 0.47.

## 4. The policy's `object` field stays

It and `BROKERED_ENDPOINTS` answer different questions: the policy says *who may receive what*, the
table says *what the broker holds*. Collapsing them makes one table mean two things, and the first
casualty would be the ability to deny a requester an object the broker legitimately holds.
**`endpoint-envelope` compares the two**, which is what makes keeping them separate safe.

## 5. The demonstration's limit, stated in advance

As with 0.46-003, the kernel's bound is shown by a **pair**. With an honest broker nobody over-asks,
so mutating the kernel's check alone shows nothing. Report that green as a property of the
demonstration surface, not as a hole, and show the pair.
