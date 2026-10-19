# RFC-0.38-001: The measurement chain measures something

**Status:** Accepted (0.38) — accepted 2026-10-19
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
`Digest32([0xBB; 32])` and `[0xCC; 32]`. The kernel embeds **34** service binaries
with `include_bytes!` in `task/image.rs` and **never hashes one**.

The chain is the node's attestation root: `attestd` sets its provider measurement
from `chain.head`, and a diagnostic bundle reports that head in a **signed**
artefact. Every step is faithful. The value is a constant.

## The owner's scoping decision, 2026-10-19

**This line closes E-086 and E-085 and stops there.** Slot 4 stays uninstalled,
`diagnosticsd` still cannot ask, and `measurement_head_available` keeps reading
**`false`**. Installing the slot and flipping the flag is **0.39**. So this line
makes the chain true without yet making it visible — which is the right order,
because a visible measurement of a placeholder is the thing E-086 is about.

## Requirements

**R1 — Re-derive, with controls.** The four append sites and their literal
arguments; `measuredd`'s 16-of-32 reconstruction and its discarded `w3`; the
8-of-32 reply; that `Digest32::of` appears nowhere in `task/image.rs`; and that
`GET_HEAD` has exactly two occurrences tree-wide. **Sweep for other placeholder
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

**R3 — Hashing cost is measured, not assumed.** Thirty-four images hashed at boot
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
Nothing can call `GET_HEAD`: it has no caller, and slot 4 is 0.39's. So the only
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
