# Developer Handoff — RFC-0.32-002

**Governing RFC:** [RFC-0.32-002](../../done/RFC-0.32-002-bytes-are-not-a-struct.md)
**Milestone:** 0.32
**Status:** inherited from the governing RFC (Implemented, 0.32.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure is `unsafe` removed, not `unsafe` explained

Every site in this line already carries a `// SAFETY:` comment, and every one
of those comments is an argument that the compiler is not making. That is the
defect class, so **a comment is not a deliverable here.** When this line is
done, `fjell-service-api`'s `chunked` module and both format crates' checksum
paths contain **no `unsafe` blocks at all**, and the bytes that cross a service
boundary are decoded by safe code that can say no.

**The boundary is bytes. A type is what you build *after* checking them.**

## 0.1 Re-derive first (R1), each absence with a positive control

```
grep -rn 'reassemble\|from_raw_parts' --include='*.rs' crates | grep -v fjell-kernel   # 6 sites, 3 crates
sed -n '/pub mod chunked/,/^}/p' crates/fjell-service-api/src/lib.rs                   # no length check
grep -n 'BEGIN =>' -A3 crates/services/fjell-semantic-stream/src/main.rs               # w0 discarded
```

Sizes and padding — measure, do not trust this table:

| Type | `size_of` | Fields | Padding |
|---|---|---|---|
| `SemanticEnvelope` | 4936 | — | `repr(Rust)`, unspecified |
| `BootControlBlock` | 88 | 73 | 15 |
| `StoreSuperblock` | 64 | 50 | 14 |

A scratch crate outside the workspace (its own `[workspace]` table, under
`.git-exclude/tmp/`) is the cheapest way; that is how these were taken.

**Re-derive one thing the RFC could not settle:** whether a **zero-filled**
buffer decodes as a valid `SemanticEnvelope` today — `StreamKind`'s first
variant, `Option<CorrelationId>`'s niche, `SemanticPayload`'s first variant.
It decides how bad Finding 2's stale-buffer case is in practice, and it is the
input your Miri demonstration should use if it is *not* valid.

**Positive controls are not optional.** A grep that finds nothing must first be
shown finding something it should.

## 0.2 Settled — do not re-open

1. **A defined wire format** with a version field; `decode` returns `Result`
   (D1).
2. **`reassemble` is deleted**, not length-checked (D2).
3. **The receive loop enforces framing** (D3).
4. **The sender encodes** (D4).
5. **Checksums over explicit serialisation** (D5).
6. **`HkdfRoundBuf` borrows with a lifetime** (D6).
7. **Miri demonstrates the defect and its absence** (D7).
8. **Zero `unsafe` in the touched paths** (D8).

---

## 1. Order

**R1 → §A–§D answered in writing → codec (D1) → receivers, sender and the
forward path (D2/D3/D4) → checksums (D5) → crypto (D6) → Miri (D7) → fuzz
target (R7) → QEMU tiers (R8) → errata (R9) → evidence.**

Two constraints on that order:

- **Demonstrate the UB before you delete the code that has it.** Once
  `reassemble` is gone the old path cannot be shown failing. Capture D7's
  "before" transcript first, in a scratch copy if that is cleaner.
- **The codec lands before any call site changes.** A half-converted path
  means two services disagreeing about the wire, and the QEMU tiers will tell
  you only that the system hangs.

## 2. The reinterpretation is chained — decode once, re-encode to forward

`fjell-semantic-stream` does **not** forward a decoded envelope. It forwards
**the raw received buffer**: `forward_to_proxy_text(&buf[..])`, straight into
`chunked::send`, and `fjell-proxy-text` then reinterprets those same bytes a
second time. So a single malformed message from `sample-service` is reassembled
twice, in two services.

**After this line, nothing forwards received bytes.** semantic-stream decodes,
validates, and *re-encodes* what it forwards. If that seems wasteful, say so in
the review with a measurement — but a proxy that passes untrusted bytes through
as "already checked" is the shape this whole erratum is about.

## 3. §A–§D — and the arguments against my leans

**§A (where the codec lives).** I lean to `fjell-semantic-format` itself. The
argument against: that crate is a pure data model with **no dependencies**, and
putting a codec in it means every consumer of the types takes the codec too.
If you split it, say what stops the two drifting — because "the types and their
wire form in different crates, kept in step by hand" is E-045's exact shape.

**§B (what the wire form carries).** Today: 4936 bytes, **157 blocking IPC
calls** per envelope (`BEGIN` + 155 `CHUNK` + `COMMIT`; this said 155, which is
the chunk count — corrected at review), most of it the unused arms of an enum. I lean to encoding
only the live variant and the fields in use. The argument against doing it
here: it makes this line both a soundness fix and a format redesign, and the
soundness fix is the urgent half. If you take the smaller scope, the encoder
still must not emit padding — a fixed-size encoding is acceptable, a
struct-shaped one is not. **Name the wire size you chose and how many chunks it
costs.**

**§C (checksum byte compatibility).** I lean to changing the bytes and moving
the on-disk `version`. The argument against is that a format change without a
reader is unverifiable — nothing will ever tell you the old and new disagree.
**So verify it the only way available:** a test that seals a block, flips one
field, and shows `is_valid` rejecting it; and one that shows a block sealed by
the new code validating under the new code. If you find any reader anywhere —
in `bootctl`, in a tool, in a test fixture — that changes the answer; report it
rather than deciding it.

**§D (where Miri runs).** The project's rule is that gates stay local and CI
records (RFC-0.31-002 D7). I lean to local plus the scheduled CI run. The
argument against: a check that runs only where the toolchain happens to be
installed is a check nobody runs, which is E-049 one crate over. If you put it
in CI, it installs the component explicitly and is named for what it proves.

## 4. D3 — what "enforces its own framing" means

The receive loop must refuse, not absorb. Each of these is a test:

| Case | Today | Required |
|---|---|---|
| `BEGIN` then `COMMIT`, no chunks | reinterprets the previous message | refused |
| fewer chunk bytes than `BEGIN` declared | stale tail from the last message | refused |
| more chunks than declared | silently dropped past the buffer | refused |
| `COMMIT` with no `BEGIN` | reinterprets whatever is in `buf` | refused |
| unknown discriminant in the bytes | undefined behaviour | decode error |

"Refused" means the protocol's existing error reply, and the buffer reset so
the next message starts clean. *(Corrected at review: this named `RENDER_ERR`,
which does not exist. `semantic_stream` has `PUBLISH_ERR`; `proxy_text` has
`ERR = 0x51F`, which its default arm already replies. Using the constant that
exists was right — adding a protocol tag is a wire change this line did not
scope.)*

**The live evidence is the `semantic` negative profile**
(`tests/qemu/profiles/semantic.toml`), which already drives sample-service →
semantic-stream → proxy-text with four markers, and runs in CI's
`qemu-negative` matrix. If you add a marker there, note the trap the profile
records in its own comment: **a marker containing `]` silently fails to load**,
because the hand-rolled TOML reader stops at the first `]`. Two of four markers
once loaded, silently, for that reason.

## 5. D5 — a correction to the RFC, made while writing this

The RFC said to bump `schema_version` and update the frozen schema files.
**Neither exists for these two structs.** The tree has eleven `.frozen` files;
none covers `BootControlBlock` or `StoreSuperblock`, and the field is a plain
`version: u16`. Move that constant, and **do not add a frozen file here** —
that mechanism is E-045's line, and adding a file the gate only checks for
existence would be one more thing claiming to be enforced.

## 6. D7 — Miri, concretely

`cargo +nightly miri` reports *"'cargo-miri' is not installed"*. Install it:

```
rustup component add --toolchain nightly miri
cargo +nightly miri test -p fjell-semantic-format        # and the two format crates
```

`fjell-semantic-format` is `no_std` with no dependencies and four existing
tests, so it runs under Miri without ceremony. Two transcripts are required:

1. **The defect, before the fix** — a test that drives the old path with the
   byte pattern R1 identified. Miri must report undefined behaviour, and the
   report must name it (invalid enum discriminant, or uninitialised read).
2. **The same input after the fix** — the decoder returns an error, Miri
   clean.

**If Miri cannot report the "before" case at all, stop and say so.** That is a
finding about the demonstration, not a reason to proceed without one — and it
would mean the erratum's central claim needs re-derivation rather than
implementation.

## 7. Prohibited shortcuts

- **Do not keep `reassemble` with a length check.** D2.
- **Do not forward received bytes** (§2). Decode, then re-encode.
- **Do not put the decode behind `debug_assert!`** or a validation function
  that runs after the value exists — by then it is too late (Finding 2).
- **Do not add `#[repr(C)]` to `SemanticEnvelope` and call it sound.** It makes
  the layout defined; it does not make an arbitrary byte pattern a valid enum,
  and it does not initialise padding.
- Do not touch the kernel's four raw sites, or wire anything to the
  boot-control block (E-044).
- Do not add a `.frozen` file (§5).
- Do not run `cargo fmt --all --check` in your head.
- Do not write "sound" anywhere without saying what makes it so.

## 8. Required evidence

1. R1 re-derived, with controls, and the zero-buffer question answered.
2. §A–§D answered in writing, engaging the arguments above.
3. The codec, with its refusal tests, and `reassemble` deleted (`git grep
   reassemble` empty).
4. D3's five refusals, each a test.
5. The forward path re-encoding (§2).
6. Checksums over explicit serialisation, with §C's two tests and the version
   constant moved.
7. `HkdfRoundBuf` borrowing, its crate's tests green.
8. **Miri: both transcripts** (§6), and where you decided it runs (§D).
9. The fuzz target for the new decoder, with a run id and its `Done N runs in
   M second(s)` line at 300 s — and your answer on whether this decoder, the
   first on a live cross-service path, argues for fuzzing on push
   (RFC-0.32-001 §7).
10. `cargo xtask test-all` all tiers, **naming the `semantic` negative
    category's result**; `release-rehearsal` green; `consistency-check --all`
    **by its exit status**; `cargo fmt --all --check`.

## 9. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Miri's "before" transcript** — the line where it names the undefined
  behaviour, first.
- **The wire size and chunk count** you chose (§B), against today's 4936 bytes
  in 157 calls.
- **Your §A answer**, and what keeps the types and their wire form together.
- **Anything you found while converting the call sites that the RFC missed** —
  it was scoped from six grep hits and two receive loops.
- Any figure of mine you re-derived and found different.
