# RFC-0.36-001: A field must say what it is

**Status:** Implemented (0.36.0) — accepted 2026-10-15
**Milestone:** 0.36
**Tracks.** **E-076** — a diagnostic bundle reports a 32-byte zero digest as a
measurement head when it could not ask for one — **E-079**, the same struct's
`last_attestation` **and `keyring_anchor_epoch`** set from literals that were never
queried **and** a fabricated semantic intent the builder signs, **E-080** (added
2026-10-15), the signed digest omitting two fields its own specification lists, and
**E-075**, two services announcing readiness to somewhere that is not
`service-manager` under comments saying otherwise. One line because all three are one defect: **a value that does
not say what it is.** Twice it is absence dressed as data; once it is invention
dressed as data; once it is a delivery that does not happen dressed as one that
does.
**Touches** *(indicative)*: `crates/formats/fjell-diag-format/src/{bundle.rs,builder.rs}`,
`crates/formats/fjell-diag-format/schema/bundle-v1.frozen`,
`crates/fjell-schema/src/{registry.rs,samples.rs}`,
`crates/fjell-schema/tests/golden.rs`,
`crates/services/fjell-diagnosticsd/`, `crates/services/fjell-secure-transportd/`,
`crates/services/fjell-service-manager/`, `tests/qemu/profiles/`,
`tools/fjell-consistency-check/src/capability_topology.rs`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.35-002**, which added `DiagnosticBundle::audit_available`
for exactly this reason and is the precedent to copy; **RFC-0.35-004**, whose
goldens-first discipline this line reuses; **E-078**, the premise that makes all of
this latent — nothing can ask `diagnosticsd` for a bundle — which this line does
**not** close and must not appear to.

## What is wrong, measured at `7971703`

**E-076 — the measurement head.** `query_measurement_head` sends on
`CAP_MEASUREDD_EP`, slot **4**, which `spawn.rs` installs for no image, and on a
recv error or an unexpected tag returns `Digest32([0u8; 32])`. The bundle carries
thirty-two zero bytes in the field that attests what the system measured, with no
flag saying the value was never obtained.

**E-079, first half — the attestation digest.** `build_bundle` passes
`Digest32([0u8; 32])` as `last_attestation`: a **literal zero, never queried at
all**. The measurement head at least asks and fails. This one does not ask.

**E-079, second half — the invented intent.** The same function calls
`builder.add_intent(1, INTENT_UPDATE_STAGING_CONFIRMED, 0, tick - 2000)` with
literal arguments. `add_intent` has **no other caller in the tree** outside
`fjell-diag-format`'s own unit tests. So the bundle's semantic-intents section is
one hardcoded record asserting an update staging was confirmed two seconds ago,
whether or not anything of the kind happened — **well-formed, plausible, and inside
an artefact the builder signs.** A consumer cannot tell it from a real record by
inspection, which is what signing it was for.

**E-075 — the readiness sends.** `secure-transportd:160` sends on slot **1**, which
nothing installs, so the send fails and the error is discarded;
`diagnosticsd:164` sends on slot **0**, its **own** identity endpoint on the shared
object, so the tag reaches whichever service receives there. Nine services use
`SERVICE_READY_SEND_SLOT`, installed uniformly by RFC-0.28-001 for precisely this
reason.

## The shape of the fix, and the one judgement in it

`audit_available` is the precedent and it is the right one: **a field that may be
unavailable carries its own availability.** Apply it to the measurement head and
the attestation digest, bump `DIAG_BUNDLE_VERSION` **2 → 3**, and let
`golden.rs` do what it did at 0.35 — refuse a regenerated digest without the bump
and the note.

**The intent is different and must not get a flag.** An availability flag says *"I
could not obtain this."* The intent was not unobtainable; it was **made up**.
Delete it. A bundle with no intents is honest; a bundle with an invented one is
not, and `audit_available: false` beside an empty list is already the shape that
says "none". Inventing a record is not a down payment on gathering real ones.

## Requirements

**R1 — Re-derive all four findings at your tip**, each with a control, and
**complete the sweep** rather than taking mine: for **every** field of
`DiagnosticBundle`, say where its value comes from and whether that path can fail
or is absent. E-076's entry asked for this sweep and it is how E-079 was found; if
it finds a fifth, **stop and escalate** rather than widening this line silently.

> **R1 was exercised and it worked, 2026-10-15.** The implementer ran the sweep,
> **found a fifth, wrote no code, and escalated** — which is what R1 asked for and
> is the reason this RFC is amended rather than quietly larger. Two outcomes:
>
> - **`keyring_anchor_epoch` is the same defect as `last_attestation`**, folded
>   into **E-079** rather than given a number of its own: `build_bundle` passes the
>   bare literal `1` into a real `u32` field, and `fjell-diagnosticsd` has **no
>   `fjell-keyring` dependency at all**, so unlike `attestd` there is no query and
>   no visible fallback. It gets the same treatment as the attestation digest.
> - **`provider_id` and `bundle_id` are correctly *excluded*.** The implementer's
>   reasoning — every service uses a `const` for its own identity, and an identity
>   that never varies is not the same shape as a literal standing in for state that
>   should — is right, and they flagged their uncertainty about it rather than
>   asserting it. Ruled, in E-079, so the line is reviewed rather than assumed.
>
> **R10 below is the second thing the sweep turned up**, and it is a different
> shape.

**R2 — Goldens from the unmodified encoder, first.** The digest and the byte
stream, before the first edit, for the cases you will change — as RFC-0.35-004 did
in a `no_std` crate and RFC-0.35-002 learned from `golden.rs`'s refusal. Every
subsequent assertion is measured against bytes recorded before they moved.

**R3 — Availability flags, adjacent to the fields they qualify.** Not appended at
the end of the struct where the diff is smaller. A reader of
`bundle-v1.frozen` must see the flag beside the field it governs, because that file
is the only description a second implementation has. Say in the description what
the flag means when the value is absent — **fixed width, zeroed, not omitted** —
which is the note RFC-0.35-004's review had to add to the chain digest for the same
reason.

**R4 — The fabricated intent is deleted, and its absence is correct.** No flag, no
placeholder. The bundle's intent count becomes 0 and `audit_available: false`
already says what an empty section means.

**R5 — `DIAG_BUNDLE_VERSION` 2 → 3**, the frozen description regenerated (its
header reads the constant since RFC-0.35-002's review, so it will say `v3 wire`
by itself), `golden.rs`'s stored digest updated **with a note citing the bump**,
and the eighteen other descriptions shown unchanged.

**R6 — Both readiness sends adopt
`fjell_service_api::ready::SERVICE_READY_SEND_SLOT`** and their own `CAP_SMGR_EP`
constants go. **And something must assert `service-manager` observes them**, which
is the part that costs: it prints only `started` today, nothing per-service. Add
**one observable** and assert it in a tier — `svc.toml` already asserts
`READY_ACCEPTED`/`UNAUTHORIZED_READY_REJECTED`, so the shape exists to copy.
A fix whose only evidence is that a send no longer targets an empty slot has not
shown the message arriving.

**R7 — `capability-topology`'s waivers shrink and stay honest.** Removing
`diagnosticsd`'s and `transportd`'s `CAP_SMGR_EP` constants removes two discovered
slots; slot 4 stays declared until E-078 decides it, and its waiver must then name
**E-078**, not E-076, since E-076 will be closed. A waiver that cites a closed
entry is the defect that review found last milestone.

**R8 — E-075, E-076 and E-079 CLOSED**, `v1-limitations.md` in the same commit,
and **E-078 untouched and visibly open**. The register and the release notes must
not let three closures read as *the diagnostics path works now*: it still cannot be
asked for a bundle.

**R10 — The signed digest covers what its specification says it covers
(E-080).** Added 2026-10-15, from the same escalation. `write_canonical` omits
**`provider_id`** and **`keyring_anchor_epoch`**, both of which `RFC-v0.4-005`
§6.4's own formula lists — so **the trust provider and key epoch a signed
diagnostic bundle claims are outside the signature** and can be altered without
invalidating it. Write both, **in the specification's order**, and write the
`bundle_digest_covers_all_fields` test §8 names and the tree does not have (zero
occurrences). **That test must fail against the present encoder before it passes
against yours** — a coverage test first seen passing has tested nothing.

This is in **this** line and not a later one for a concrete reason: this line
already bumps `DIAG_BUNDLE_VERSION` and re-takes this struct's goldens. Fixing
coverage afterwards would mean a **second** bump and a second goldens cycle on the
same digest, invalidating the first. And it **reconciles the encoder to an accepted
RFC** rather than designing a format, which is the only reason a digest change
belongs beside four honesty fixes.

Note what this does to R2's order: take the goldens of the **present** stream
first, as R2 says — they pin what the encoder wrote before anything moved and are
the evidence that the only intended change is the one you made — and then expect
the digest to change, deliberately, under the version bump. **A changed digest is
expected here, which is the opposite of RFC-0.35-004's rule**; say which change
each golden reflects.

**R9 — The gates**, each by its own exit status, reported as the **required set**
with failures included: `release-rehearsal`, `abi-snapshot --verify`,
`consistency-check --all`, `cargo fmt --all --check`, `test-all`, `repro-check`.
Guest services change here, so expect prebuilts to move and **re-record the repro
baseline in the same commit** as the rebuild.

### Non-goals

- Making `diagnosticsd` reachable, installing slot 4, or gathering real intents or
  attestations. That is **E-078**, ruled a service and scoped for 0.37.
- Touching `fleetd`.
- A general "optional field" mechanism in `Canon`. Two flags, modelled on the one
  that already exists.

## Risks

- **An availability flag can become its own lie.** `measurement_head_available:
  false` beside a zeroed digest is honest; the same flag set `true` by a path that
  did not actually obtain one would be worse than today. Whatever sets it must be
  the code that got the value, not a default.
- **The intent deletion will look like lost functionality.** It is not: nothing
  gathered intents, and the deleted call asserted something untrue. Say so in the
  register entry so a later reader does not "restore" it.
- **Adding per-service readiness output changes console content** that other tiers
  read. Check `clean_console` in `semantic.toml` and any marker-count assertion
  before assuming the new line is free.
- **Three closures in a component that still cannot run** is the misreading this
  line most invites. R8 exists for that.
- **R10 makes a digest change expected**, which inverts the discipline of the three
  previous format lines. The danger is that "the digest changed" stops being a
  stop-and-escalate signal for the rest of the work. Pin each golden to the one
  change it reflects, and treat any digest movement you cannot name that way as a
  defect.
- **E-080 is a security property, not a formatting detail.** If R10 turns out to be
  larger than writing two fields and a test — if, say, the order in §6.4 cannot be
  reconciled with what `Canon` emits — **stop and escalate**; do not reconcile the
  specification to the code.
