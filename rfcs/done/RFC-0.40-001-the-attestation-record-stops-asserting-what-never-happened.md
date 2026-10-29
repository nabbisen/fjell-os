# RFC-0.40-001: The attestation record stops asserting what never happened

**Status:** Implemented (0.40.0) — accepted 2026-10-29
**Milestone:** 0.40
**Tracks.** **E-094** — `attestd` signs a record built from literals that asserts
`release_verified`, `rootfs_verified` and `policy_verified` **true** while nothing
verifies; `sign_v2` takes the measurement sequence and never reads it; and `attestd` holds
no route to `measuredd`, so 0.39 making the chain real changed nothing about what any
record says.
**Touches** *(indicative)*: `crates/formats/fjell-attestation-format/src/v2.rs`,
`crates/services/fjell-attestd/`, `crates/fjell-kernel/src/task/spawn.rs`,
`crates/formats/fjell-measure-format/src/attest.rs`, `crates/fjell-service-api/src/lib.rs`,
`tests/qemu/profiles/`, `docs/src/releasing/v1-limitations.md`, `rfcs/ERRATA.md`.
**Relates to:** **RFC-0.39-002**, whose slot-4 install this mirrors; **RFC-0.39-001**,
whose append mapping this *narrows*; **E-095**, which this line discloses and does not
close.

## What is wrong

`attestd` always runs `sign_v2`, which builds the record with
`AttestationRecordV2::dev` (`v2.rs:321`–`:336`). Every security-relevant field is a
literal:

| Field | Value | What it should mean |
|---|---|---|
| `boot.kernel_digest` | `[0x11; 32]` | a measurement of the kernel that booted |
| `verification.release_digest` | `[0x22; 32]` | the release manifest that was verified |
| `verification.rootfs_digest` | `[0x33; 32]` | the rootfs that was verified |
| `verification.policy_digest` | `[0x44; 32]` | the policy bundle that was verified |
| **`release_verified`** | **`true`** | a release verification succeeded |
| **`rootfs_verified`** | **`true`** | a rootfs verification succeeded |
| **`policy_verified`** | **`true`** | a policy verification succeeded |
| `measurement.head_seq` | `1` | the chain's latest sequence |
| `measurement.chain_digest` | `[0x55; 32]` | the chain head |
| `measurement.included_from/to_seq` | `1` / `1` | the range covered |
| `snapshot.snapshot_digest` | `[0x66; 32]` | the snapshot attested |

`sign_v2`'s parameter is `#[allow(dead_code)] _meas_seq: u64`, commented *"v0.7: forwarded
to measurement chain header"*, and never read. `init` passes `GENERATE` the literal `4u64`
anyway.

### Why this is the worst-shaped falsehood left, and not the same as E-095

E-095's three chain events are **honest placeholders**: they record a literal and the
register, the limitations page and the bundle's own counts all say so. This record
**asserts a positive result** — three verifications *succeeded* — that nothing produced,
and it does so **under a signature**, where a consumer's entire reason for trusting the
content is that it was signed. A reader who checks the signature gets `VERIFY_OK` over
assertions with nothing behind them.

The owner's standard is that an API must not permit the misunderstanding. A signed
assertion of a verification that did not happen is the strongest available form of
permitting it.

## Requirements

**R1 — Re-derive, with controls.** Every row of the table above; that `sign_v2` discards
`_meas_seq`; that `init` passes the literal `4u64`; that `registry.enter_enforcing()` is
unconditional so `sign_v2` is always the path; and that `attestd` holds only its own
endpoint (`EP_SLOT = 0`) and the readiness send. `/usr/bin/grep -a`.

**Then sweep for the class:** any other **signed or exported** artefact asserting a
positive result from a literal. The diagnostic bundle was dealt with in 0.39-002; this asks
whether a third exists. **If you find one, stop and escalate.**

**R2 — The three `*_verified` flags assert `false`.** Not omitted, not left `true`.
Nothing in the tree verifies a release, a rootfs or a policy bundle, so `false` is what is
true, and it costs nothing to say today. Omitting the fields would change the record's
shape and lose the disclosure; keeping `true` is indefensible under a signature.

**R3 — The digests nothing computes are zeroed, each behind its own adjacent flag.**
`kernel_digest`, `release_digest`, `rootfs_digest`, `policy_digest` and `snapshot_digest`
become **zero with an availability flag beside them**, following the pattern
`bundle-v1.frozen` already states and `fjell-diag-format` already implements: *a field that
may be unavailable carries its own flag, adjacent to it, zeroed by the encoder.*

A zero digest beside `available: 0` cannot be read as a measurement. `0x22` beside
`verified: true` can, and is why this entry exists. **Use the existing pattern rather than
inventing a second one**, and bump the record's version; the attestation format is not in
the ABI snapshot, so say in the review request what the version move costs.

**R4 — The measurement section becomes real, by the route 0.39-002 established.**
`attestd` gets a **`CALL`** capability to `measuredd`, queries the head, and fills
`head_seq`, `chain_digest` and the `included_from_seq`/`included_to_seq` range from the
answer. `sign_v2` stops taking a sequence number it does not read — either it uses one or
it does not have the parameter.

**If `measuredd` cannot be reached, the measurement section is unavailable and says so**
via R3's flags. A record that silently reports a stale or literal head on a failed query
would be this entry again.

**R5 — And `ATTESTD` comes out of `measuredd`'s append mapping.** This is the half that
would otherwise be found after the capability existed.

`GET_HEAD` is **ungated by sender** — reading the head needs only the endpoint — but
`ATTESTD` *is* in RFC-0.39-001's `source_for_sender` mapping
(`fjell-measure-format/src/attest.rs:44`), so the capability R4 grants would also let
`attestd` **append** events recording `source: Attestd`. Nothing appends as `Attestd`
today: the only occurrence outside the mapping is the mapping's own test table.

**Grant the read, refuse the append.** Remove `ATTESTD` from the mapping so an append from
it is `PermissionDenied`, and extend `the_services_that_must_be_refused_are` to name it.
This keeps 0.39-001's property — a service that wants to be a recorded source must be
named — and `attestd` has no reason to be.

**R6 — A tier asserts the record's new content, demonstrated failing.** The three
`*_verified` flags read `false`, the measurement section carries the **real** chain head,
and the zeroed digests report unavailable. Compare the head against an independent
derivation, as `remote-diag` does — not merely non-zero.

**And assert that `attestd` cannot append**, with the scratch-capability technique if
needed: the mapping change is the kind of thing that silently regresses.

**R7 — No retro-claim, and one sentence nobody may write.** Nothing in this line may say
or imply that **attestation is now real**, or that the kernel's own image is measured. This
line makes the record *truthful*, not complete: the kernel image is unmeasured (0.39
measures the *service* images), snapshots are a different subsystem, and release/rootfs/
policy verification is **E-095**. `v1-limitations.md` rows 13 and 14 must say what changed
and what did not, in the same commit.

**R8 — E-094 closes in this line's commit.** It is ACCEPTED, so nothing is blocked; it
closes because the record stops lying, not because the milestone ended.

**R9 — Gates by exit status.** `release-rehearsal`, `consistency-check --all`,
`readiness-check`, `abi-snapshot --verify`, `fjell-repro-check`, `test-all` to completion,
`callsite-audit`, `cargo fmt --check`. A capability install and a format bump move the
prebuilts: rebuild **before** re-recording the repro baseline and commit the binaries with
it. One `test-all` at a time. **Expect `capability-topology` to move** — state the new
figures for the next cut's handoff.

### Non-goals

- **E-095** — real release, rootfs and policy verification. The successor milestone. This
  line makes the record honest about not having it.
- **Measuring the kernel's own image.** A running kernel hashing itself is weak attestation
  and a design question; `kernel_digest` becomes zero-and-unavailable, not invented.
- **Snapshots** — a different subsystem; `snapshot_digest` gets the same treatment.
- **`sign_v1`.** Unreachable in practice (`enter_enforcing()` is unconditional). If
  touching it is free, say so; do not make it a project.
- **Any addition to the kernel's ABI surface.** Escalate.

## Risks

- **Making the record *complete* instead of *truthful*.** The temptation is to find
  something to put in `kernel_digest`. There is nothing honest available; zero plus a flag
  is the answer. R7 exists for this.
- **The capability widening the chain's writers.** R5 is the mitigation and it must land
  with R4, not after.
- **A tier that checks the head is non-zero.** Third milestone running where this
  distinction decides a requirement; R6 asks for an independent derivation.
- **A second availability pattern.** `fjell-diag-format` already has one. Copy it.
