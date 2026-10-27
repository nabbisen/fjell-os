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

What this is **not**: attestation (the signed attestation record is independent
constants, E-094; `last_attestation_available` stays `false`), and a fully
measured chain (three of four events are placeholders).

## Attestation

`AttestationRecordV2` does **not** capture the chain. `attestd` signs a record
built by `AttestationRecordV2::dev` from constants (`chain_digest` `0x55…`,
`head_seq: 1`, and `*_verified: true` flags) and has no endpoint to `measuredd`;
making the chain real does not change what any record says (Erratum **E-094**).
The signature is the development trust provider's keyed SHA-256 digest, not a
signature scheme; Ed25519 is used by host tooling only.
