# RFC-0.38-001: The measurement chain measures something

**Status:** Implemented (0.38.0) — accepted 2026-10-19
**Milestone:** 0.38
**Tracks.** **E-086** — the measurement chain is fed placeholder literals on every
boot because nothing in the tree computes a digest of anything — and **E-085**,
`measuredd`'s `GET_HEAD` being unable to reply with a whole chain digest. One line
because the second is unsafe to close without the first: transmitting a complete
placeholder is worse than transmitting a truncated one.
**Touches** *(indicative)*: `crates/fjell-kernel/src/task/image.rs`,
`crates/fjell-kernel/src/` (wherever the digest is held and exposed),
`crates/fjell-abi/src/boot.rs` *(possibly)*, `crates/services/fjell-init/`,
`crates/services/fjell-measuredd/`, `crates/fjell-service-api/src/lib.rs`,
`crates/services/fjell-diagnosticsd/` *(read side only)*, `tests/qemu/profiles/`
or a host check, `rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.37-001**, which made the first consumer of this data and
whose `remote-diag` tier asserts the flags; **E-083**'s demonstration technique
(a temporary capability install, used and reverted), which this line needs for
R5; **RFC-0.36-001**, whose availability flags stay `false` here by the owner's
own scoping decision.

## What is wrong

`fjell-init` appends **four** measurement events on every boot (`main.rs:955`,
`:970` ×3). The "subject digest" it sends is a **literal**: `0x0011` for boot
evidence, `0xAB` for each of three verification results, remaining words **0**.
`measuredd` rebuilds `sd[0..8]` from `w1` and `sd[8..16]` from `w2`, leaving
`sd[16..32]` zero. **Each recorded measurement is a magic number followed by
twenty-four zero bytes.**

**Nothing computes a real digest anywhere.** `init`'s other digest fields are
`Digest32([0xBB; 32])` and `[0xCC; 32]`. The kernel embeds **33** service binaries
with `include_bytes!` in `task/image.rs` and **never hashes one**.

> **Correction, 2026-10-21 (architect).** This paragraph said **34**. The count
> is **33**: `/usr/bin/grep -a -c 'include_bytes!("'` on
> `crates/fjell-kernel/src/task/image.rs` returns 33, and the 33 tracked `.bin`
> files and 33 repro-baseline lines agree. The loose pattern `include_bytes!`
> returns 34 because line 5 of that file is a `//!` doc comment naming the
> macro — which is how the wrong figure was arrived at. Found by the newly
> onboarded developer, who supplied that control. **R1 states 33.**

The chain is the node's attestation root: `attestd` sets its provider measurement
from `chain.head`, and a diagnostic bundle reports that head in a **signed**
artefact. Every step is faithful. The value is a constant.

> **Correction, 2026-10-22 (architect). The paragraph above is wrong about
> `attestd`, and the original is left as written because the line was scoped from
> it.** `provider.set_measurement(chain.head)` has exactly one call site in the
> tree — `fjell-measuredd/src/main.rs:204` — and it is on **`measuredd`'s own**
> `DevelopmentTrustProvider`, in `measuredd`'s address space, which nothing else
> reads. **`attestd` never receives the chain head**: it holds no endpoint to
> `measuredd`, and `init` passes `GENERATE` the literal `4u64`. The signed record
> is built by `AttestationRecordV2::dev`, which supplies `kernel_digest`
> `[0x11;32]`, `release_digest` `[0x22;32]`, `rootfs_digest` `[0x33;32]`,
> `policy_digest` `[0x44;32]`, `chain_digest` `[0x55;32]`, `head_seq: 1` and
> `release_verified`/`rootfs_verified`/`policy_verified: **true**`; `sign_v2`
> receives the sequence number as `_meas_seq` and never uses it.
>
> **So making the chain real will not change what any attestation record says.**
> "Every step is faithful" described a chain feeding a signed artefact; the signed
> artefact is independent constants. Found by the implementer's extended sweep,
> re-derived by the architect, and filed as **E-094** — a defect larger than E-086
> and one hop downstream of it. It is **not** in this line's scope (ruling 4
> below), and R7 now forbids this line from implying that attestation became real.

## The owner's scoping decision, 2026-10-19

**This line closes E-086 and E-085 and stops there.** Slot 4 stays uninstalled,
`diagnosticsd` still cannot ask, and `measurement_head_available` keeps reading
**`false`**. Installing the slot and flipping the flag is **0.39**. So this line
makes the chain true without yet making it visible — which is the right order,
because a visible measurement of a placeholder is the thing E-086 is about.

## Design ruling (R2 escalation), 2026-10-22

The implementer escalated before building, as R2 requires, twice: a first request on
the transfer shape and a dated addendum after extending R1's sweep past the three
crates it had covered. Both were right to stop. The sweep found a placeholder feeding
a signed artefact this RFC does not name, which is R1's own stop condition. Rulings,
in the order asked:

**1. Transfer shape: extend `BootInfo`. Not the entry registers.** This overrules the
implementer's recommendation, and the reason is this line's own subject.

Option D — four entry registers carrying a 32-byte digest — is **the shape that
produced E-085**: a digest squeezed into a fixed, unversioned register payload with no
room to grow, which is why `GET_HEAD` returns eight of thirty-two bytes and why R4
exists. Choosing it in the line that repairs E-085 would re-make the error being
repaired. It also makes the service entry contract non-uniform and invisible: every
service's `rt.rs` is `la sp, __stack_top; tail service_main`, so `init`'s
`service_main` would silently take four arguments that no other service's does and
that no `rt.rs` mentions.

`BootInfo` is the designed home and its own doc comment already says so — *"Passed as
a pointer in `a1` at `fjell-init` entry. All fields are ABI-stable; new fields are
appended and the `version` field bumped."* **That comment is false today** (the kernel
sets `a1 = 0` and builds no struct). Option A makes a false claim true and uses the
extension mechanism the struct documents; Option D leaves the claim false and adds a
second, undocumented channel beside it. Given that claims matching mechanism is what
this entire milestone has been about, that settles it.

The cost the implementer correctly surfaced — `BootInfo` is snapshot-tracked, so a new
field is a **Changed sig**, which `abi-snapshot --verify` reports as breaking — is
real but **formal**: nothing in the tree constructs or reads a `BootInfo`, so there is
no consumer to break. Pay it properly: regenerate the snapshot **in this line**
(RFC-0.30-002 §5) and write the `### Breaking` CHANGELOG entry. Do not route around a
gate by choosing a weaker design.

**Do not use `BootEvidence`.** The implementer's sweep found
`fjell-verify-format::BootEvidence::for_slot`'s `kernel_digest: [0u8; 32]` has one
write site and **zero** read sites. By name it looks exactly like R2's home. It is
not: it feeds nothing, and putting a real digest into a struct nothing reads would
produce the appearance of a measurement with no consumer. Reporting it rather than
reaching for it was the right call, and it is named here because the next reader will
reach for it.

**Escalate if the `a1` plumbing grows.** Mapping one page into `init`'s address space
and writing the struct is the expected shape. If it forces a change to the
address-space layout constants or to how `init`'s space is built, stop — that is a
different line.

**2. Granularity: the two-level digest, approved, with three constraints.**
`digest_i = SHA-256(bytes_i)` per image, then one digest over the per-image results in
ascending `ImageId` order, as proposed. Constraints:

- **Include the image count in the preimage.** The proposal omits it. With the count
  bound in, "there were 33 images" becomes part of what is covered, and an image added
  or dropped cannot coincide with a reordering.
- **The domain string and the byte layout live in `fjell-measure-format` as a named
  constant and a documented encoding** — not as a literal at the kernel call site.
  Otherwise the kernel and R6's host recomputation drift, which is the E-014 family.
- **This is a different digest from `compute_chain_digest`**, whose byte stream
  RFC-0.35-004 froze. Adding this one does not touch that stream, so the non-goal
  "stop if the frozen stream moves" is not triggered — confirm it stays untouched
  rather than assuming it.

**3. R7 scope: approved as described.** The three verification-result events stay
placeholders, disclosed as such; the non-goals exclude release/rootfs/policy
verification and this line does not implement it.
`docs/src/releasing/v1-limitations.md` must say, **in the same commit**, that three of
the chain's four recorded measurement kinds remain placeholders — and now also that
the attestation record is independent of the chain (ruling 4).

**4. The attestation record: filed, not widened. Agreed.** Filed as **E-094**,
ACCEPTED. This line stays as scoped. Two consequences:

- The premise paragraph above is corrected, because it was mine and it was wrong.
- **R7 gains a clause:** nothing in this line, its commits, its limitations entry or
  its release note may say or imply that attestation is now real. The chain measuring
  something does not make a signed record of constants true, and the two will be read
  as one thing unless the disclosure says otherwise.

**5. Kernel dependency: take `fjell-measure-format`.** Verified: it depends only on
`fjell-canon`, which has **no dependencies at all**, is `#![forbid(unsafe_code)]` and
allocation-free; `fjell-measure-format` itself contains zero `unsafe`. So the kernel's
TCB grows by two auditable crates with no unsafe code, no external dependencies, and a
terminating graph.

A third SHA-256 in a tree that already has two is the worse outcome — divergence
risk, and this project already polices duplicated mechanism. The decisive reason is
R6: reusing the same `Digest32::of_parts` the chain digest uses means R6 compares one
implementation against an **independent host recomputation**, which is evidence. Two
in-tree implementations agreeing with each other is not.

Expect Gate 2's audited set to move when the kernel gains crates, and say so rather
than letting the count drift unexplained.

**6. `init`'s literal `measurement.seq = 4`: in scope for R2's wiring, replace it.**
It is true only because there are four appends today, and this line changes what the
appends contain. Take the sequence from `APPEND_OK`'s reply; the same applies to
`emit_measurement_status(4, 0)`. A number that is accidentally correct is the E-014
family with a smaller blast radius.

**Proceeding:** start **R4** and the shell of **R5/R6** now, in parallel with the
`BootInfo` work — R4 is independent of the transfer shape and is the E-085 repair.
**E-085 still must not close before E-086**, unchanged.

## Requirements

**R1 — Re-derive, with controls.** The four append sites and their literal
arguments; `measuredd`'s 16-of-32 reconstruction and its discarded `w3`; the
8-of-32 reply; that `Digest32::of` appears nowhere in `task/image.rs`; and that
`GET_HEAD` has exactly two occurrences tree-wide — **corrected 2026-10-22: there
are three code sites plus the declaration.** RFC-0.37-001 gave `diagnosticsd` a
real call site (`main.rs:144`) after E-085 was filed, and I wrote this requirement
without re-deriving it against a line I had myself reviewed. Re-derive the current
count; the remedy is unaffected. **Sweep for other placeholder
digests** beyond the four I name — `[0xBB; 32]` and `[0xCC; 32]` are two more in
`init` alone, and I have not looked past that file. Report what you find; if a
placeholder turns out to feed something I have not named, **stop and escalate**.

**R2 — Something measures something.** The kernel already holds every service
image's bytes; hashing them there makes a recorded measurement a fact about what
booted. **The design choice is yours, and it must be stated**: where the digest is
computed, where it is held, and how it reaches the chain. The constraint that
matters: `measuredd` is a userspace service and the kernel does not speak its
protocol, so the digest has to travel — `BootInfo`, a dedicated syscall, or the
spawn path.

**If your answer needs a new syscall or a `BootInfo` field, escalate before
building it.** That is an ABI addition, it lands in the snapshot, and it is not a
decision to make inside an implementation step.

**R3 — Hashing cost is measured, not assumed.** Thirty-three images hashed at boot
is work on a path that currently does none. Report the boot-time effect from a real
run; if a tier's 60-second timeout moves materially, say so rather than tuning the
timeout.

**R4 — E-085, now small.** The kernel's `sys_ipc_reply` copies **four** words
(`for i in 0..4 { caller.trap_frame.gpr[12 + i] = tf.gpr[12 + i] }`), so a reply
carries **32 bytes — exactly one `Digest32`**. Widen `measuredd`'s `reply` helper
to four words; `HEAD_REPLY` carries the digest whole; `latest_seq` and `dropped`
get their own tag rather than riding in a reply that now has no room for them; and
the caller stops discarding `w2`/`w3`. **No chunking** — and do not use
`EXPORT_LOG` as a model, which is a stub that sends no content (**E-087**).

**R5 — Demonstrate the whole digest arriving, with a temporary capability.**
Nothing can call `GET_HEAD`: it has no caller, and slot 4 is 0.39's.
**Corrected 2026-10-22: it does have a caller** — `diagnosticsd/src/main.rs:144`
sends it — and the accurate statement is that the caller **cannot reach**
`measuredd`, because DIAGNOSTICSD slot 4 is declared and uninstalled (the waiver
`capability-topology` already prints). The requirement is unchanged: the scratch
capability is still the only way to exercise the reply, and now for a stated
reason rather than a wrong one. So the only
way to show R4 works is the technique RFC-0.37-001 used for E-083 — **install slot
4 in a scratch change, call it, capture the transcript, revert, and prove the
revert** (`grep` for the scratch marker on the committed tree returning nothing).
A line whose central change cannot be exercised by anything in the tree must
exercise it deliberately and say so. The committed tree must still have slot 4
**out**.

**R6 — The instrument E-086's own root cause names.** *"No instrument here compares
a recorded measurement against the thing it claims to measure."* **Build that
comparison.** Something — a host check or a tier assertion — must compare a
recorded subject digest against a digest computed independently from the same
bytes. A measurement that is merely non-zero is not evidence; the assertion is
**equality with an independently derived value**.

**R7 — The chain's own history is not retro-claimed.** Events appended before this
line were placeholders. Whatever the chain carries forward, **do not describe past
measurements as having measured anything** — in the register, the limitations, or
a comment.

**R8 — E-086 and E-085 CLOSED**, `v1-limitations.md` in the same commit, with
`measurement_head_available` still `false` and the `remote-diag` tier **unchanged**
and still passing. **E-087 untouched** — it is unscheduled and not this line's.

**R9 — The gates**, each by its own exit status, as the required set with failures
included. The kernel and guest services change, so prebuilts move: rebuild
**then** re-record the baseline, in the same commit — **and commit the binaries
with it**, which is the trap the 0.37.0 cut found the hard way (`repro-check` reads
the working tree, not git, so it reports PASS either way).

### Non-goals

- **Installing slot 4, or flipping any availability flag.** 0.39's, by the owner's
  decision.
- `EXPORT_LOG` (**E-087**).
- Measuring anything beyond the service images — no attestation of configuration,
  rootfs or runtime state.
- Changing what `compute_chain_digest` hashes. The stream is described by a frozen
  file (RFC-0.35-004); if a real subject digest changes that stream, **stop**.

## Risks

- **A new syscall is the likely shape and the thing most easily done quietly.** R2
  requires escalation first. An ABI addition that appears in a cut's `Added: 0`
  check as a surprise is a worse outcome than a round trip.
- **R6 is the requirement that will be dropped under pressure**, because the line
  "works" without it. It is also the only part that makes the next placeholder
  detectable. If anything is cut, it is not this.
- **Hashing at boot can push a tier's timeout.** R3 asks for the measurement; the
  wrong response is a longer timeout.
- **The temptation to install slot 4 "while we are here"** so the result is
  visible. The owner scoped that out deliberately; visibility is 0.39's and the
  honest `false` is this line's.
