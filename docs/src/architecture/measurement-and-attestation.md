# Measurement and Attestation

*References RFC v0.3-001..004; updated by RFC-0.38-001.*

## The measurement chain

At boot the kernel hashes every service image it embeds (`include_bytes!`) and
combines the per-image digests into **one** digest — the *image-set digest*, a
documented, generated-described format
(`crates/formats/fjell-measure-format/schema/image-set-digest-v1.frozen`,
domain `FJELL-IMAGE-SET-V1`). It hands that digest to `init` in a read-only
`BootInfo` page (`fjell_abi::boot::BootInfo`; its address is in `a1` at entry),
and `init` gives it to `measuredd`, which records it as the first event of the
chain. The recorded value is therefore a fact about the bytes that booted.

What that does and does not cover:

- **Only the service images.** Not the kernel's own image, configuration, rootfs
  or runtime state.
- **One of the chain's four recorded events is real.** The other three — the
  release, rootfs and policy "verified" events — still record the literal `0xAB`:
  nothing in the tree verifies a release, a rootfs or a policy bundle yet.
  (`docs/src/releasing/v1-limitations.md` says so.)
- **The chain was not a measurement before RFC-0.38-001.** Earlier boots recorded
  placeholders for all four events.
- `measuredd` takes the image-set digest on `init`'s word, which is the kernel's
  (`BootInfo`); nothing attests that relay's *content*. Since RFC-0.39-001 it does
  check *who* is relaying: only `init`, by the kernel-attested sender, may send it.
- A tier compares what `measuredd` recorded with a digest the **host** recomputes
  from `crates/fjell-kernel/prebuilt/*.bin` by an independent route
  (`tests/qemu/profiles/measurement.toml`).

### What an event's `source` says

`MeasurementSource` is *who produced the measured fact*, and the caller does not
declare it: `measuredd` derives it from the operation plus the kernel-attested
identity of the sender (`fjell_measure_format::source_for_sender`), and a request
that tries to supply one is refused. The image-set event is `Kernel` (the kernel
computed the digest; `init` only carried it). The three placeholder verification
events are `Init`, because `init` produced the literal they record. An appender
with no source — a driver, a test service, a service added tomorrow — is refused
rather than given a default. Chains recorded before RFC-0.39-001 asserted
`Verifyd` for those three events; that was never attested, it was what `init` put
in the request, and `verifyd` had no part in them.

`GET_HEAD` returns the chain digest whole (four words, 32 bytes); the head's
sequence number, drop count and composition are `GET_HEAD_STATS`.

### The head in the diagnostic bundle

Since RFC-0.39-002 `diagnosticsd` holds a `CALL` capability to `measuredd`
(`CAP_MEASUREDD_EP`, slot 4) and a signed diagnostic bundle carries the head:
`measurement_head_available` reads `true` when `measuredd` answers. The bundle
(`DIAG_BUNDLE_VERSION` 4) also carries two numbers beside the head, so that the
head is not read as more than it is:

* `measurement_event_count` — events the chain had appended (4 on a normal boot);
* `measurement_events_kernel_digested` — how many of those arrived by the path
  where **the kernel produced the digest** (1: the image set).

The second is a **structural** count. `measuredd` cannot look at 32 bytes and
know whether they are a hash or a literal, so it does not claim to count "real"
measurements; it reports which path an event came in by. The other events'
digests were supplied by the appending service — today, `init`'s literal `0xAB`
(E-095) — and the number does not describe them, it only shows that the kernel
did not produce them. `diagnosticsd` reads the counts before and after the head
and reports the section unavailable if they changed in between, so a head is never
paired with another moment's counts.

What this is **not**: strong attestation (the kernel's own image is unmeasured and
the signature is the development provider's), and a fully measured chain (three of
four events are placeholders). The bundle's `last_attestation_available` stays
`false`: the attestation record is a separate artefact.

## Attestation

`attestd` signs an `AttestationRecordV2` (schema version 4) that **says only what is
true** (RFC-0.40-001, closes E-094). It used to be built from literals and asserted,
under a signature, that a release, a rootfs and a policy bundle had been verified.
Now:

* the three `*_verified` flags are **`false`** — nothing in the tree verifies any of
  them, and a verified bit is only ever written beside a digest that is available;
* the five digests nothing computes — kernel, release, rootfs, policy, snapshot — are
  **zero, behind an availability flag that is `false`**, written fixed-width and zeroed
  by the encoder (the pattern the diagnostic bundle already uses), so a zero digest
  beside `available = 0` cannot be read as a measurement;
* the **measurement section is the real chain**: `attestd` reads the head and its
  range from `measuredd` through a `CALL` capability, as one consistent reading, and if
  `measuredd` cannot be read the section is **unavailable** — there is no fallback to a
  stale or literal head;
* the **rollback counter pair** says whether `attestd`'s own
  `read_anti_rollback_counter()` call to its trust provider **succeeded** and what it
  **returned** (`false` and `0` when it did not). It is read once per record, never
  cached; it is not the rollback record's minimum, which is a different quantity and
  was once reported in its place.

* the **placeholders** — the three keyring anchor epochs, the snapshot id with its
  reason, the health status, the last-advance source and the freshness key epoch —
  each carry their own availability flag, written immediately before the claim and
  **`false`** for every record `attestd` builds, and the claim is written as zero by the
  encoder (RFC-0.41-003, closes E-101; schema version 3 → 4). Nothing computes any of
  them, so none is claimed; a `0` beside flag `0` is not a reading, and a valid `0` (a
  reason, a status) beside flag `1` is. `attestd` prints the five flags beside the
  others (`attestd: record availability keyring_epochs=0 …`).

`attestd` has no way to write the chain: the capability cannot tell a read from an
append, so `measuredd` refuses an append from `attestd` (it is not a source
`measuredd` maps). The record is **truthful, not complete**: the kernel's own image
is still unmeasured, release/rootfs/policy verification does not exist (E-095), and
the signature is the development trust provider's keyed SHA-256 digest, not a
signature scheme; Ed25519 is used by host tooling only.
