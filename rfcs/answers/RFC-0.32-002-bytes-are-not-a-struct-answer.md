# RFC-0.32-002 §A–§D — where the codec lives, what it carries, what the checksums cover, and where Miri runs

**Governing RFC:** [rfcs/done/RFC-0.32-002-bytes-are-not-a-struct.md](../../rfcs/done/RFC-0.32-002-bytes-are-not-a-struct.md)

Answered after R1 and before the codec, in the handoff's order. R1's
measurements are what these four rest on, so they come first.

---

## What R1 established

- **The three probes reproduce**: `reassemble` has no length check, `BEGIN`'s
  declared length is discarded by both receive loops, and six raw sites sit
  outside the kernel in three crates.
- **`SemanticEnvelope` is 4936 bytes**, `repr(Rust)`, align 8 — so a transfer
  is **155 chunks and 157 blocking IPC calls**, not 155 calls: `BEGIN`, 155
  `CHUNK`s, `COMMIT`. `ENV_BUF_SIZE` rounds to 4960, so the receive buffer is
  24 bytes larger than the value read out of it.
- **A zero-filled buffer decodes today.** Reading 4960 zero bytes as a
  `SemanticEnvelope` yields `schema_version 0`, `stream Intent`,
  `correlation_id None`, payload variant `State` — a well-formed, entirely
  fabricated envelope. **Finding 2's stale-buffer case is therefore silent
  corruption, not a trap**, which is worse than a crash and is why the Miri
  demonstration uses a non-zero pattern.
- **The padding figures are right, and they undercount the bytes the CRC
  reads.** `BootControlBlock` is 88 bytes with fields summing to 73 (15
  padding) — but `SlotInfo` is itself 24 bytes carrying 12 of padding, so the
  byte view `from_raw_parts` hands to `crc32` contains **39** padding bytes.
  `StoreSuperblock`: 64, fields 50, **14** padding, as stated.
- **Neither struct has a version *constant*.** Each has a `version: u16` field
  assigned the literal `1` in `new()`. D5's "move that constant" means
  introducing one.

## §A — the codec lives in `fjell-semantic-format`

**Decision: a `wire` module in the format crate itself.**

The argument against is real and I checked it: `fjell-semantic-format` has
**no dependencies at all**, and six crates depend on it. Anything added there
is added to all six.

**It costs those six nothing.** The codec needs no dependency: it is
`no_std`, allocation-free, encodes into a caller's `&mut [u8]` and decodes
from a `&[u8]`, exactly as `fjell-semantic-v1::codec` already does in this
tree. What the six crates gain is the guarantee that the types and their wire
form are versioned together, in one file, by one commit.

**And the alternative is a named defect.** "The types in one crate, their wire
form in another, kept in step by hand" is E-045 stated as a design: eleven
frozen schema files that drifted from the code they describe, with a gate that
only checked they existed. A separate `fjell-semantic-wire` crate would need
exactly the discipline E-045 proves this project does not have. If the codec
ever does need a dependency, that is the moment to split it — and the split
would then be visible as a new dependency edge rather than invisible as drift.

## §B — the wire carries one variant, with explicit lengths

**Decision: encode the live variant and the bytes actually in use**, not the
4896-byte union.

Today's envelope is mostly nothing: `SemanticPayload` is 4896 bytes because
`StateNode` is the largest of three variants, and every message pays for all
of it. Worse, most of what it does pay for is empty `BoundedText` buffers —
128 bytes reserved for each text regardless of its length.

The encoding is therefore:

- a magic and a **format version**, refused if unknown;
- a **payload tag** with an exhaustive wire-tag → variant mapping, so an
  unknown tag is a decode error and never a discriminant;
- **explicit lengths**: `BoundedText` writes `len` and `len` bytes, not 128;
  `FixedVec<T, N>` writes a count and that many elements;
- **no padding**, by construction — nothing is copied from struct memory.

**I keep all three variants.** `fjell-proxy-text` renders `State`, `Event` and
`Intent`, and dropping the two that nothing sends today would delete working
code to make a wire smaller; §B's scope is the encoding, not the model.

**What I am not doing** (the handoff's smaller scope, and I agree with it):
the chunk transport is untouched — still `BEGIN`/`CHUNK`/`COMMIT` at 32 bytes
a call. The wire size and chunk count this actually costs are measured and
named in the review request, against today's 4936 bytes in 157 calls.

## §C — the checksum bytes change, and the version moves with them

**Decision: explicit serialisation, new bytes, `version` bumped to 2.**

**Nothing reads either structure's bytes.** The only callers of `seal` or
`is_valid` anywhere are two in-memory test files
(`fjell-proptest/tests/verus_lemma_properties.rs`,
`fjell-upgrade-format/tests/mirror_conformance.rs`). `bootctl` does not touch
the block, no service reads a superblock, and there are no on-disk fixtures.
Byte compatibility with a format nothing has ever read is not worth an
encoder that deliberately reproduces padding — and reproducing padding is
exactly the undefined behaviour being removed.

**Verified the only way available**, since there is no reader to disagree with:

1. seal a block, flip one field, and show `is_valid` rejecting it — the CRC
   covers the fields;
2. seal with the new code and show it validating — round-trip;
3. and, because the old bug was that padding could differ between `seal` and
   `is_valid`, a test that seals, copies, and revalidates.

If any reader turns up during implementation — in a tool, a fixture, anywhere
— that changes the answer and I report it rather than deciding it.

## §D — Miri runs locally, and in CI on schedule and dispatch

**Decision: both, with a scope flag.**

Local only is the project's rule for gates, and it is also **E-049's shape**: a
check that runs where the toolchain happens to be installed is a check nobody
runs. E-049 was filed three days ago for exactly that — a validator nothing
executed, red for two releases. Miri is not installed by default; it was not
installed in this tree until this line installed it. A local-only Miri would be
run by whoever remembers.

So it runs in both places, and neither claims to be the other:

- **Locally**, `cargo +nightly miri test -p fjell-semantic-format`, documented
  with the component it needs, because that is where a developer changing the
  codec sees a result in seconds.
- **In CI, on `schedule` and `workflow_dispatch`** — the shape RFC-0.32-001
  established for slow checks — installing the component explicitly and named
  for what it proves, not for the tool it uses.

**The scope flag, stated rather than assumed:** the RFC's *Touches* list does
**not** include `.github/workflows/ci.yml`, while the handoff's §D explicitly
contemplates putting Miri in CI ("If you put it in CI, it installs the
component explicitly and is named for what it proves") and D7 says installing
the component is part of this line. I read that as an omission in *Touches*
rather than a prohibition, and I am flagging it for review rather than either
silently widening the line or dropping the CI half on a technicality.
