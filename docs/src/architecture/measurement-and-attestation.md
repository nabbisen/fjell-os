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
  (`BootInfo`); nothing attests that relay.
- A tier compares what `measuredd` recorded with a digest the **host** recomputes
  from `crates/fjell-kernel/prebuilt/*.bin` by an independent route
  (`tests/qemu/profiles/measurement.toml`).

`GET_HEAD` returns the chain digest whole (four words, 32 bytes); the head's
sequence number and drop count are `GET_HEAD_STATS`. Nothing in the committed
tree can ask for the head yet — `diagnosticsd`'s capability to `measuredd` is
deliberately not installed — so `measurement_head_available` reads `false` in
every bundle.

## Attestation

`AttestationRecordV2` does **not** capture the chain. `attestd` signs a record
built by `AttestationRecordV2::dev` from constants (`chain_digest` `0x55…`,
`head_seq: 1`, and `*_verified: true` flags) and has no endpoint to `measuredd`;
making the chain real does not change what any record says (Erratum **E-094**).
The signature is the development trust provider's keyed SHA-256 digest, not a
signature scheme; Ed25519 is used by host tooling only.
