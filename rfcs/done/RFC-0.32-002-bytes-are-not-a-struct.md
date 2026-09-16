# RFC-0.32-002: Bytes from another service are not a struct

**Status:** Implemented (0.32.0) — accepted 2026-09-16
**Milestone:** 0.32
**Tracks.** **E-046** — Rust structs are reinterpreted as raw bytes without the
guarantees that would make it sound, including across a service boundary.
**Touches.** `crates/fjell-service-api/src/lib.rs` (`chunked`),
`crates/formats/fjell-semantic-format/`, `fjell-semantic-stream`,
`fjell-proxy-text`, `fjell-sample-service`,
`crates/formats/fjell-upgrade-format/`, `crates/formats/fjell-store-format/`,
`crates/fjell-sxt-crypto/src/hkdf.rs`, `fuzz/`, and
`.github/workflows/ci.yml` (the Miri job §D contemplates — *added at review:
this list omitted it while the handoff's §D required a decision about where
Miri runs, and the implementer flagged the gap rather than widening the line
silently*). **Does not touch the kernel.**
**Relates to:** E-043's line (which deliberately left this path unfuzzed until
it is sound); E-044 (nothing reads the boot-control block from disk today);
E-045 (the frozen schemas these checksums belong to); E-014 (an instrument
whose predicate is weaker than its claim).

## Summary

Every figure below was measured in this tree, not read from a comment.

### Finding 1 — a safe function that turns any bytes into any `Copy` type

`fjell_service_api::chunked::reassemble<T: Copy>(buf: &[u8]) -> T` is a **safe
`pub fn`** whose body is `unsafe { core::ptr::read_unaligned(buf.as_ptr().cast()) }`.
It has **no length check**: a caller passing a slice shorter than
`size_of::<T>()` reads out of bounds, and the compiler will not stop them. Its
doc comment calls this a *"documented caller contract, not a compiler-enforced
one"* — which is an accurate description of the defect.

`T: Copy` does not mean every bit pattern is a valid `T`. The type actually
reassembled is `SemanticEnvelope`: **4936 bytes**, `#[derive(Clone, Copy)]`
with **no `#[repr]`**, containing `StreamKind` (a 3-variant enum),
`Option<CorrelationId>`, and `SemanticPayload` (a 3-variant enum of node
structs). **An invalid discriminant makes the value itself invalid, and
producing it is undefined behaviour** — before any code inspects it. Rust also
does not specify the layout of a `repr(Rust)` type, so "sender and receiver
share the identical definition, built by the identical compiler" is an
argument about *this* build, restated as a safety property.

### Finding 2 — the receivers trust a length they never read

Both receive loops (`fjell-semantic-stream` `PUBLISH_*`, `fjell-proxy-text`
`RENDER_*`) are shaped:

```rust
let mut buf = [0u8; ENV_BUF_SIZE];   // outside the loop
let mut offset = 0usize;
loop { match tag {
    BEGIN  => { offset = 0; reply(OK) }                       // w0 = declared length: ignored
    CHUNK  => { write_chunk(&mut buf, offset, w0..w3); offset += 32; reply(OK) }
    COMMIT => { let envelope: SemanticEnvelope = reassemble(&buf); … }
} }
```

- **`BEGIN`'s declared length is discarded.** Nothing compares it to the bytes
  that arrive.
- **`buf` outlives the message.** A sender that sends `BEGIN` then `COMMIT`
  with **no chunks at all** causes the receiver to reinterpret **the previous
  message's bytes** — or, on the first message, 4936 zero bytes. A short
  message leaves the previous message's tail in place.
- **`write_chunk` silently drops anything past the end** of `buf`, so extra
  chunks are ignored rather than refused.
- **`validate_envelope` cannot help**: it runs *after* `reassemble`, and it
  matches on `env.payload` — reading the discriminant that may already be
  invalid. It also checks only payload semantics, never the envelope framing.

There is no malice needed for the stale-buffer case: any sender bug produces it.

### Finding 3 — the sender publishes its own padding

`fjell-sample-service` builds the envelope on its stack and ships
`from_raw_parts(&envelope as *const _ as *const u8, 4936)`. For a
`repr(Rust)` type the padding bytes are not required to be initialised, so
**the bytes another service receives can include whatever was on this
service's stack** — an information leak across a capability boundary, in the
one place the system exists to separate.

### Finding 4 — the checksums are computed over padding

| Struct | `size_of` | Sum of fields | Padding |
|---|---|---|---|
| `BootControlBlock` (`#[repr(C)]`) | 88 | 73 | **15** |
| `StoreSuperblock` (`#[repr(C)]`) | 64 | 50 | **14** |

`seal` and `is_valid` both view `size_of::<Self>()` bytes through
`from_raw_parts`. Reading padding through a byte slice is undefined behaviour,
and `is_valid`'s `let mut copy = *self` is not required to reproduce the
padding `seal` saw — so the CRC can differ for a structurally identical block.
Nothing reads either from disk today (E-044), which is exactly why this is the
cheapest moment to fix it.

### Finding 5 — a lifetime held by a comment, in the crypto crate

`HkdfRoundBuf` stores `overflow_info: *const u8` taken from a caller's slice,
and `overflow_info_slice()` rebuilds a slice from it with:
*"The lifetime of info outlives this struct because `build_hmac_data` is
called and used within the same `hkdf_expand` iteration."* True today, enforced
by nothing. A borrowed slice with a lifetime parameter is the same code with
the compiler checking it.

### Finding 6 — why the instruments are quiet

Gate 2 verifies that every `unsafe` site **has** a `// SAFETY:` comment. All
six sites have one; none mentions `repr`, discriminant validity, padding
initialisation, or provenance. The gate is honest about what it checks
(E-014's family), and it is not the thing to fix here. The fuzz line
(RFC-0.32-001 D7) deliberately left this path unfuzzed, because fuzzing
undefined behaviour only rediscovers it.

## The settled part

**D1 — The envelope crosses the boundary as a defined wire format.** An
explicit encoder and decoder for `SemanticEnvelope`, in safe code, with a
version field, explicit lengths, and an exhaustive mapping from wire tags to
variants. Decoding returns `Result`; an unknown discriminant is an error
value, not a value.

**D2 — `reassemble` is deleted, not repaired.** A generic "bytes → any `T`" is
the defect; a length check would keep the shape and remove only the crudest
symptom. Nothing in this workspace may reconstruct a Rust type from IPC bytes
by reinterpretation.

**D3 — The receive loop enforces its own framing.** The `BEGIN` length is
recorded and checked against the bytes actually received; the buffer is reset
per message; a `COMMIT` whose byte count disagrees is refused with the
protocol's existing error reply; chunks past the declared length are refused,
not dropped.

**D4 — The sender encodes; it does not view itself as bytes.**
`fjell-sample-service`'s `from_raw_parts` goes with `reassemble`.

**D5 — The checksums are computed over an explicit serialisation** of the
named fields, shared by `seal` and `is_valid`, with no struct-memory view and
no padding. The on-disk bytes change, so the on-disk version moves with them.
*(Corrected while writing the handoff: this said "`schema_version` moves and the
frozen schema files are updated with it". **Neither struct has either.** The
tree holds eleven `.frozen` files and none covers `BootControlBlock` or
`StoreSuperblock`, and the field is a plain `version: u16`. Move that field's
constant; do not invent a frozen file here — that mechanism is E-045's line.)*

**D6 — `HkdfRoundBuf` borrows with a lifetime** instead of storing a raw
pointer.

**D7 — Demonstrated failing, under Miri.** The defect is undefined behaviour,
so the demonstration is a tool that detects undefined behaviour: a host test
that drives the old path with a crafted byte pattern must be **reported by Miri
as UB**, and the same test against the new decoder must pass. **Miri is not
installed** (`cargo +nightly miri` → *"'cargo-miri' is not installed"*);
installing the component is part of this line.

**D8 — The measure is `unsafe` removed, not `unsafe` annotated.** The target is
zero `unsafe` blocks in `fjell-service-api`'s `chunked` module and in both
format crates' checksum paths — *corrected at review: `chunked` keeps one, the
`asm!` block that is the IPC syscall itself (`ipc_call4`). It reinterprets
nothing and cannot be written in safe code; the target is every `unsafe` that
turns bytes into a value.* A SAFETY comment left anywhere in this line's
scope must name what makes it true — `repr`, initialisation, provenance — not
who promised it.

## The open questions

**§A — Where does the codec live?** In `fjell-semantic-format` beside the types
(no new crate, no new dependency edge), or in a separate `fjell-semantic-wire`
crate that the format crate does not depend on? **I lean to the format crate**:
the types and their wire form drift apart when they live apart, which is E-045
in one sentence. Argue it.

**§B — What does the wire form contain?** Today's transfer is 4936 bytes in
**157 blocking IPC calls** per envelope (`BEGIN` + 155 `CHUNK` + `COMMIT`),
most of it the unused arms of an enum. *(This said "155 blocking IPC calls";
155 is the chunk count. Corrected at review from the implementation's
re-derivation.)*
An encoding that carries only the live variant and the fields in use would be a
fraction of that. **Is shrinking it in scope here, or a separate line?** My
lean: encode only the live variant — it falls out of writing the encoder at all
— but do not tune the chunk transport in this line. Say if you disagree, and
name the wire size you chose.

**§C — Do the checksums keep byte compatibility?** Explicit serialisation
changes the CRC input, so every existing sealed block would read as invalid.
Nothing reads them from disk today (E-044). **I lean to changing the bytes and
bumping `schema_version`**, because compatibility with a format nothing has
ever read is not worth an explicitly padded encoder. The alternative — encode
the padding deliberately to preserve today's CRC — is the wrong trade if I am
right that nothing depends on it; say so if the evidence disagrees.

**§D — Where does Miri run?** Locally only, like every other gate; a scheduled
CI job; or on push for the affected crates? Miri is slow and the project's rule
is that gates stay local and CI is recorded (RFC-0.31-002 D7). My lean: a local
gate plus the scheduled run, with the decision written down rather than
inherited.

**Answer all four in writing before implementing.**

## Requirements

**R1 — Re-derive before changing anything.** Each finding above, in this tree,
with a positive control for every absence. Report every disagreement. In
particular re-derive the padding figures and the envelope size, and confirm
whether `Option<CorrelationId>` and `StreamKind` admit a zero byte pattern —
the stale-buffer case in Finding 2 turns on it.

**R2 — D1 and D2:** the codec, `reassemble` deleted, both receivers and the
sender converted. No new `unsafe`.

**R3 — D3:** framing enforced, with a test per refusal (no chunks; too few;
too many; length mismatch; unknown tag).

**R4 — D5:** checksums over explicit serialisation; `seal`/`is_valid`
round-trip tests; `schema_version` and the frozen files moved together.

**R5 — D6:** the crypto borrow, with its tests still passing.

**R6 — D7:** Miri installed; the UB demonstrated on the old path and absent on
the new; both transcripts in the review request.

**R7 — A fuzz target for the new decoder**, added to `fuzz/` under
RFC-0.32-001's rules (a real decoder of bytes from outside its component,
seeds verified through the decoder, a `Done N runs` line). **This is the target
RFC-0.32-001 §7 recorded as the condition for arguing shape 3** — a decoder on
a live cross-service path — so answer that question here too.

**R8 — The QEMU tiers still pass**, including the milestones that exercise the
semantic path end to end. A wire-format change that no service can decode is
the failure mode to watch for; say which tier would have caught it.

**R9 — E-046 CLOSED**, or its survivors named; register and
`v1-limitations.md` in the same commit.

### Non-goals

- The kernel's four raw-reinterpretation sites.
- Wiring the boot-control block to anything (E-044).
- Enforcing the frozen schemas (E-045) — this line updates the files it
  changes; the mechanism is 0.33.
- Making the chunk transport efficient (§B names it; fixing it is separate).
- Changing Gate 2.

## Risks

**The wire format is a new format, and new formats are where E-045 came from.**
It ships with a version field, a decoder that refuses unknown versions, and a
fuzz target from day one.

**A behavioural regression in the semantic path is possible** — this is the
first change to how services exchange envelopes since they were written. The
QEMU tiers are the evidence, not the host tests.

**Miri may report further undefined behaviour** in code this line did not set
out to touch. That is a finding: record it as an erratum, fix it here only if
it is inside this line's scope.
