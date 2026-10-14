# RFC-0.35-004: Two digests that describe themselves

**Status:** Accepted (0.35) — accepted 2026-10-15
**Milestone:** 0.35
**Tracks.** **E-065**, in the scope left after RFC-0.34-003 took the semantic wire
codec and E-073 took the two `#[repr(C)]` layouts: **`fjell-measure-format`'s
`compute_chain_digest`** and **`fjell-bundle-format`'s `compute_bundle_digest`** —
two byte streams that are hashed and described by nothing a gate compares. Closing
them closes E-065.
**Touches** *(indicative)*: `crates/fjell-canon/src/lib.rs`,
`crates/formats/fjell-measure-format/src/event.rs`,
`crates/formats/fjell-bundle-format/src/lib.rs`,
`crates/fjell-schema/src/registry.rs`, `crates/fjell-schema/tests/`,
`crates/formats/fjell-measure-format/schema/`,
`crates/formats/fjell-bundle-format/schema/`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.33-003**, which gave eleven formats a description
generated from their encoder; **RFC-0.34-003**, which did the same for the
semantic wire codec and taught `Canon` `scope`/`optional`/`variant`/`i64`/`refuse`
— this line is the third application of a method that has worked twice;
**E-073**, the two `#[repr(C)]` layouts that stay out because closing them needs
the kernel's audit write to move.

## Why this line exists, stated plainly

Not because a gate refuses a cut. **The tracking field that creates that deadline
is mine** — I wrote `0.35` into E-065's row on 2026-10-14, in the same commit that
split E-073 out, in anticipation of a fourth line the owner's scope answer
(*"A1+B+C"*) never authorised. A gate refusing a cut is a reason to **report**,
not a reason to do engineering, and §5 of the standing cut instruction says so:
rescheduling *with the slip recorded* has twice been the right answer. If the
deadline were the only argument here, the right act would be to correct the
tracking value and say I had set it on an assumption.

The argument is the design, and it is the owner's own bar:

**"Finally clean."** Today these two formats are handled by
`fjell_schema::registry::EXCLUSIONS`, which types each as
`Survivor { erratum, what }`, and `crates/fjell-schema/tests/frozen.rs` fails when
a formats crate is neither described nor excused, when a survivor names an erratum
the register does not contain, or when a survivor's probe stops finding a producer.
That machinery is honest, tested, and **not a design** — it is well-engineered
bookkeeping of something undone. *A workaround kept in the architecture fails the
bar.* Two deferrals are a schedule; a third would make the exclusion the answer, by
not deciding.

**"APIs and UI/UX for users not to be confused or misunderstand."** Nobody is
misled today — the exclusion says exactly what is not described and why. That is
the clause being **satisfied**, which makes it a floor, not a licence to stop.

And these two are the formats where a description is **most** load-bearing, not
least. Every other format in the census has a decoder, so a round trip per sample
catches a reader that drifts from its writer. **You cannot decode a hash.** The
only way a second implementation of the measurement chain can agree with this one
is by matching a written description of the byte stream — and when it does not
agree, the symptom is a digest that differs, with nothing to say why. One of the
two is the attestation chain.

## What the work actually is

Measured at `234e395`, because the register's own words overstate the gap and this
RFC should not inherit that.

**`compute_chain_digest`** (`event.rs:133`) — ten parts: `CHAIN_DOMAIN`, `u16` LE
schema version, `u64` LE seq, three `u8` enum discriminants, a 32-byte subject
digest, a one-byte presence flag, a **fixed 32-byte** metadata slot (zeros when
absent), and the 32-byte previous digest. **`Canon` needs nothing new for this.**
Note what the shape is *not*: `Canon::optional` writes its body only when present,
and this stream writes 32 bytes either way, so the metadata slot is a presence
byte plus `bytes`, not an `optional`. That is the better property — the stream is
**fixed-width by construction** and has no length ambiguity at all — and the
description should make it visible rather than smuggle it into an `optional` that
would describe a different format.

**`compute_bundle_digest`** (`lib.rs:204`) — seven parts: `BUNDLE_DOMAIN`, a
**variable-length, unlengthed** `name`, three `u32` **big-endian**, a 16-byte
manifest digest, a 32-byte binary hash. The one genuine gap is **big-endian
`u32`**: one method, not an endianness system, and not the "big-endian integers"
plural the register says.

So: one `Canon` method, two encoders moved, two registry entries, two generated
files, goldens first. This is the smallest of the three lines that have used this
method.

## Requirements

**R1 — Re-derive both encoders at your tip** and confirm or refute the two claims
above: that the chain digest needs no new `Canon` primitive, and that the only gap
is a big-endian `u32`. If `Canon` turns out to need more, **say so before writing
it** — the size of this line is the argument for doing it now, so a bigger line is
a decision to re-take, not a surprise to absorb.

**R2 — Goldens captured from the unmodified encoders, first.** The discipline
RFC-0.33-003 and RFC-0.34-003 both used, and the only one that makes this safe:
record each digest (and the byte stream, where you can reach it) from the encoder
**as it stands**, before the first edit, and show the moved encoder reproducing
them **byte for byte**. A digest that changes is a format change in the attestation
path, and this line does not make one.

**R3 — `Canon` gains exactly one primitive**, a big-endian `u32`, with the same
treatment the five RFC-0.34-003 added got: a default that writes what the encoder
already wrote, so **no existing description moves**. State the file count before
and after (18 unchanged, 20 total) and show that the eighteen are untouched.

**R4 — The unlengthed `name` gets its argument written down, not assumed.**
`compute_bundle_digest` concatenates a variable-length name with no length prefix,
which is the classic shape for a concatenation ambiguity. My reading is that it is
**safe here** because every field after `name` is fixed-width, so the total length
determines the name's length — but that argument exists nowhere in the tree, and
describing the format is exactly the moment to write it. Put it in the generated
file's `# not described:` or a note, or refute it.

**R5 — Both descriptions say what they do not cover.** These have no decoder, so
the caveat that applies to every other file — *"the decoder is a second,
hand-written reading and is not described"* — does not. The honest caveat here is
the opposite and stronger: **nothing reads these bytes back, so nothing but this
file and the encoder can tell a second implementation what to write.** Say that.

**R6 — The format's own version in the header**, through `format_version`, as the
wire and on-disk formats do — not the release the file was generated in. If a
stream has no version constant, say that in the entry rather than leaving the
header to imply a release.

**R7 — E-065 CLOSED**, with `v1-limitations.md` in the same commit, both registry
entries moved from `Survivor` to described, and the `frozen.rs` survivor count
down to **E-073's two**. E-073 stays open and must not be quietly swept in.

**R8 — The gates**, each by its own exit status — `release-rehearsal`,
`abi-snapshot --verify`, `consistency-check --all`, `cargo fmt --all --check`,
`test-all`, `repro-check` — reported as the **required set**, failures included.
Two lines running, the instrument missing from the table was the failing one.

### Non-goals

- E-073's two `#[repr(C)]` layouts. They need the kernel's raw audit write to
  move; that is their own line.
- A general endianness mechanism in `Canon`. One method for one real need.
- Changing any digest. If a byte moves, stop and escalate.

## Risks

- **A version-bump deadline is the wrong reason to hurry this**, and it is now
  removed as a reason: the ruling is on the design. If goldens-first cannot be
  done properly before a cut, **the cut waits or E-065 slips with the slip
  recorded** — in that order of preference, and never by editing a tracking field
  to clear a gate.
- **`optional` is the trap.** It is the nearest-looking primitive and it describes
  a different format. A description that used it would be wrong in the one way
  this whole census exists to prevent.
- **The bundle digest's `name` invites a "fix".** R4 asks for the argument, not a
  length prefix. Adding one would change the digest.
