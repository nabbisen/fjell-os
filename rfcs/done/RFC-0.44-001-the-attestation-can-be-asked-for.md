# RFC-0.44-001: The attestation can be asked for, and the export nobody wants goes

**Status:** Implemented (0.44.0) — accepted 2026-11-15
**Milestone:** 0.44
**Tracks.** **E-121** — `attestd` receives on an endpoint no image can address. **E-120** and
**E-087** — two declared chunked exports that reply "done" having transferred nothing.
**Touches** *(indicative)*: `crates/fjell-abi/src/service.rs`,
`crates/fjell-kernel/src/task/spawn.rs`, `crates/fjell-service-api/src/lib.rs`,
`crates/services/fjell-attestd/src/main.rs`, `crates/services/fjell-diagnosticsd/src/main.rs`,
`crates/services/fjell-measuredd/src/main.rs`, `crates/formats/fjell-diag-format/src/`,
`tests/abi/snapshot.json`.

---

## What is wrong

**No diagnostic bundle can ask `attestd` for anything.** `fjell-diag-format`'s
`last_attestation_available` is documented *"permanently `false` today"* because nothing queries,
and `ATTESTD_EP_OBJECT` does not exist as a named constant, unlike `measuredd`'s, `bootctl`'s,
`verifyd`'s, `netd`'s and `configd`'s (**E-121**).

> **Corrected 2026-11-15, at this line's own review, by its implementer.** This paragraph first
> said `attestd` *"cannot be asked for anything"* and that its `GENERATE` arm's *"record generated
> (v2)"* line *"has never been printed by any boot"*. **Both were false.** The kernel's `main.rs`
> installs slot 4 → `object_id: 3`, `CapRights::CALL` into **`init`'s** CSpace, and `init`'s M8
> step 5 has sent `GENERATE` and `VERIFY_LATEST` on every boot. The architect enumerated every
> endpoint install **in `spawn.rs`** and wrote it up as every install — while `init` is spawned
> from `main.rs`, as `capability-topology`'s own module doc states. **The requirements below rest
> on the narrower truth and were unaffected**; it is the premise that was wrong, and E-121 carries
> the retraction.

**And the consumer has been waiting, with a comment explaining why it is empty.**
`fjell-diag-format`'s bundle carries `last_attestation: Digest32` beside
`last_attestation_available`, whose doc reads *"`false` until something queries a real
last-attestation digest … Permanently `false` today"*, and `builder.rs` declines to take the value
as a parameter *"on purpose. Nothing in [the tree] queries a real last attestation."*

**Separately, two declared chunked exports transfer nothing.** `measuredd`'s replies one
`EXPORT_CHUNK` carrying a sequence number then `EXPORT_DONE` (**E-087**); `attestd`'s is one line
that replies `EXPORT_DONE` with no chunk at all, and its `EXPORT_CHUNK` constant has a single
occurrence tree-wide — its own declaration (**E-120**).

## The decision that shapes the design, and the reasoning that was corrected

**The first recommendation on this was to delete `attestd`'s request surface, and it was wrong.**
It argued from *nothing calls it* to *remove it*, which **cannot distinguish unwanted from
unfinished** — and the tree documents it as unfinished. The owner asked whether introducing the
missing caller was not the reasonable move, and that is what sent anyone to look at
`fjell-diag-format`.

**The wiring is precedented in the same bundle, by a line built to do exactly this.**
**RFC-0.39-002** installed `CAP_MEASUREDD_EP` (slot 4) as a `CALL` capability, implemented
`query_measurement_head`, and made `measurement_head_available` read **`true` when the query
succeeds** — demonstrated failing first, with the capability absent and the flag staying false.

**Of the bundle's four availability flags**: measurement is **wired and working**;
`audit_available` was **deliberately retracted** (E-068 / RFC-0.35-002 — a decision, not a gap);
`keyring_anchor_epoch_available` is out of scope here; and **attestation's was simply never
built.** That is the one this line finishes.

**And knowing the consumer settles the export question properly.** The bundle wants **32 bytes** —
one `Digest32`, which a single `sys_ipc_reply` carries in four words. **Nothing wants the
dozen-field record.** So the export is not removed because nothing calls it; it is removed because
the actual consumer, once identified, does not want it.

## Requirements

**R1 — `ATTESTD_EP_OBJECT` becomes a named constant** in `fjell-abi/src/service.rs`, and
`spawn.rs`'s `ep_obj` arm uses it instead of the bare `Some(3)`. A service whose endpoint is a
literal is a service nobody can find by name.

**R2 — a `CALL` capability to it is installed for `diagnosticsd`**, at a **named send-slot
constant**, following `CAP_MEASUREDD_EP`'s shape exactly. One caller, named; not a broadcast.

**R3 — a digest query, not an export.** `attestd` replies with the last `AttestationRecordV2`'s
`Digest32` in four words. **It must distinguish "no record yet" from "here it is"** — a distinct
reply tag, not a zero digest.

**R4 — `last_attestation_available` reads `true` only when the query received a reply**, and
`builder.rs` takes the value now that something obtains it. **E-076's rule binds here and is not
negotiable:** *"whatever sets this `true` must be the code that obtained the value"* — E-076 was
filed because a recv error or unexpected tag produced a 32-byte zero digest indistinguishable from
a real one.

**R5 — the failing-first demonstration, exactly RFC-0.39-002's.** With the capability absent, the
query gets no reply and the flag stays `false` — quoted. Then present, and the flag reads `true`
with a real digest — quoted. **A flag that has only been seen `true` proves the field exists, not
that the query works.**

**R6 — both chunked exports go**, with the decision recorded: `measuredd`'s `EXPORT_LOG` /
`EXPORT_CHUNK` / `EXPORT_DONE` and `attestd`'s `EXPORT` / `EXPORT_CHUNK` / `EXPORT_DONE`, the way
`AUDIT_QUERY` went in **RFC-0.35-002** — deleted, not stubbed. **This is ABI movement**, so
`tests/abi/snapshot.json` is regenerated **in this line's own commit** and the item count is
predicted in the review request.

**R7 — "available" must not read as "attested", and this is a requirement rather than a note.**
The record's claims are mostly placeholders: a real boot reports `measurement=1` with every other
digest `0` (**E-095**). Making `last_attestation_available` `true` therefore publishes a *digest of
a record that is largely empty*. The bundle already draws this distinction for
`measurement_events_kernel_digested`, whose doc warns that *"a reader who treats this field as 'how
much of the chain is trustworthy' has read more into it than it says."* **The same warning is owed
here, in the field's own doc comment**, and `v1-limitations.md` row 24 follows it.

### Non-goals

- **A remote verifier, or any transport.** `push_attestation` stays deleted; RFC-0.35-002's reason
  stands. This line gives the record a *local* consumer, which is what `attestd`'s own header says
  it is for — a **local** attestation service.
- **Making the record's content real.** That is **E-095**, excluded from 0.44 on its own grounds.
- **`audit_available`** (retracted by decision) and **`keyring_anchor_epoch_available`**.
- **`GENERATE`'s scheduling.** Becoming reachable is not a reason to add a trigger; whether anything
  *should* call `GENERATE` is a separate question from whether it *can* be called.

## Risks

- **E-076's defect is the one to re-create by accident.** Any path where a failed query yields a
  zero digest that a reader cannot distinguish from a real one re-opens it. Prefer failing closed
  and leaving the flag `false`.
- **Removing six constants is ABI movement**, and Gate 4 fails on `Added != 0` in either direction.
  Regenerate in the same commit and predict the count.
- **Installing a new capability widens authority.** It is one `CALL` capability to one named caller;
  resist the temptation to give it to anything else because it is convenient.
- **The honest outcome may be unflattering.** When the query works, the bundle will report an
  available attestation whose claims are nearly all placeholders. **That is the truth and it should
  be visible** — R7 exists so the field cannot be read as more than it is.
