# RFC-0.35-001: The capabilities the update path never had

**Status:** Implemented (0.35.0) — accepted 2026-10-14
**Milestone:** 0.35
**Tracks.** **E-072** — five services address their peers through eleven CSpace
slots the kernel never installs, and two of them busy-spin on the absence for the
lifetime of every boot — and **E-070**, the update-index fetch that has never
completed. One line because they are one defect seen from two ends: **E-070 is
what E-072 looks like from inside the service that needed the channel.** E-070
cannot be closed without E-072, and E-072 cannot be demonstrated honestly without
a path that is supposed to work.
**Touches** *(indicative)*: `crates/fjell-kernel/src/task/spawn.rs`,
`crates/services/fjell-secure-transportd/`, `crates/services/fjell-upgraded/`,
`crates/fjell-abi/src/service.rs`, `tools/fjell-consistency-check/`,
`tests/qemu/profiles/upgrade.toml`, `crates/services/fjell-neg-test/`,
`rfcs/ERRATA.md`, `docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.28-001**, which installed `SERVICE_READY_SEND_SLOT`
uniformly *"instead of a per-image declaration that has to be remembered to wire
up"* — the sentence `fjell-netd:50` quotes two lines above a per-image declaration
nobody wired up; **E-011**, capability rights validation on install;
**RFC-0.29-001 D5**, which gave a not-gated tier its honest failure.

## Summary

Derived 2026-10-14 at `9dc6454`, while scoping 0.35.

`crates/fjell-kernel/src/task/spawn.rs` installs, for **every** image, exactly two
capabilities: slot **0**, the task's own endpoint — whose object comes from a
per-image `match` that **falls through to the shared object `0`** for every service
not named in it — and `SERVICE_READY_SEND_SLOT`. Every further install sits inside
an `if image_id == …` block, and those blocks exist for nine images:
`BOOTCTL`, `SERVICE_MANAGER`, `DRIVER_UART`, `NEG_TEST`, `SAMPLE_SERVICE`,
`SEMANTIC_STREAM`, `SVC_PRESENTATION_LIAR`, `PROXY_TEXT`, `CAP_BROKER`, plus the
MMIO-region holders.

Five services declare a *"CSpace layout"* — `fjell-diagnosticsd`'s own words at
`main.rs:24` — and address their peers through it anyway:

| Service | Slots it names | Installed? |
|---|---|---|
| `fjell-secure-transportd` | 1 (smgr), **2 (upgraded, its only `recv`)**, 3 (diag), 4 (attest) | none |
| `fjell-diagnosticsd` | **1 (tools, its only `recv`)**, 2 (sxt), 3 (auditd), 4 (measuredd) | none |
| `fjell-upgraded` | 6 (sxt) | no |
| `fjell-attestd` | 5 (sxt) | no |
| `fjell-netd` | 1 (driver) | no |

**Eleven declared slots hold nothing.** `sys_ipc_recv`
(`crates/fjell-kernel/src/cap/syscall.rs:552`) resolves the handle with
`cs.get(ep_h)` and, on `Err`, **returns the error immediately — it does not
block**. `secure-transportd` and `diagnosticsd` both map `Err(_) => (0, 0, 0)`, and
tag `0` is in neither protocol's vocabulary (`SxtTag::from_u16(0)` → `None`;
`diagnosticsd` compares against `0x401`/`0x403`), so both fall out of the bottom of
their event loop and re-enter it at once. **Both busy-spin for the lifetime of
every boot**, each under a comment describing the work it is doing.

In **27 recorded boots**, `secure-transportd` prints `starting`, then `ready`, then
**none of its three per-outcome lines** — `channel opened`, `handshake failed`,
`channel table full` — ever.

And this is what E-070 stands on. The update-index fetch was recorded as failing
because a channel-kind word is never carried. That is true and it is not the first
cause: `upgraded` sends on slot 6 and `transportd` receives on slot 2, and neither
slot exists, so **the IPC never happens at all**.

## Why nothing saw it

The service-side slot layout and the kernel-side install are **two independent
declarations of one topology, and no gate compares them.** Gate 11's callsite
conformance checks LEASE / CAP / SYSCALL / BCB callsites, not this. Every affected
tier passes, because **nothing asserts that these services ever exchange a
message** — and the one tier that would have, `upgrade`, is one of the two that are
not release-gated and have no emitting scenario.

**The missing comparison is the defect. The eleven slots are its instances.** A
line that installs eleven capabilities and stops has fixed nothing that matters.

## Requirements

**R1 — Re-derive, at your tip, before changing anything.** The install set in
`spawn.rs` (which images, which slots); the eleven declared slots; that
`sys_ipc_recv` errors rather than blocks on an empty slot; that tag `0` is in
neither vocabulary. Each probe with a **control** (`/usr/bin/grep -a`), and the
controls stated. Two of mine, to reproduce or refute: the kernel's other install
site populates **`init`'s** CSpace, not the services'; and
`cap_broker`/`cap_install`/`sys_cap` appear **0** times in each of
`secure-transportd`, `diagnosticsd` and `upgraded`, so no slot is filled at
runtime. If either is wrong, **stop and escalate** — the line's premise moves.

**R2 — Demonstrate the spin before you remove it.** This failing case is live in
the tree today and can only be taken once. A transcript showing
`secure-transportd` (or `diagnosticsd`) re-entering its loop without blocking —
instrumented, in a scratch build, not committed — is the evidence that the loop
spins rather than idles. If you find it **blocks** instead, say so plainly and the
erratum is corrected: the claim is mine and it is the one I am least able to see
fail from here.

**R3 — One declaration of the topology.** A service's CSpace layout and the
kernel's install must agree **by construction**, not by two people remembering.
The mechanism is yours to design — a shared table in `fjell-abi` that both sides
read is the obvious candidate, as `SERVICE_READY_SEND_SLOT` already is for the one
slot that works — but whatever you choose must make a slot a service names and a
slot the kernel installs **the same fact**, written once.

**R4 — A gate that fails on the divergence.** A new subcheck (Gate 12's 19th) that
**fails when a service names a slot the kernel does not install for it.** It must
fail today, before R3 lands, on all eleven — show that run. A gate that passes on
first write against the state it was built to catch is the E-014 class, and this
register has a dozen of them.

**R5 — Install what the update path needs**, and only that. `upgraded` ↔
`transportd` for the metadata channel. The other eight slots are **not** this
line's business: whether `diagnosticsd`, `attestd` and `netd` get theirs is
RFC-0.35-002's decision, and installing authority a component does not yet
exercise is the least-privilege violation this project exists to refuse. Say in
writing which you installed and why each was necessary.

**R6 — The word, and the id space.** With the IPC real, the channel-kind word must
either be **carried** or **not claimed**: `transportd:139`'s comment describing
`kind (high 8 bits) | server_name_ptr (low 24 bits)` and its
`kind_raw = (w0 >> 24)` decode, and `upgraded:186`'s `kind = UpdateMetadata
(0x01)`, must each become true or go. And **channel id `0` must be invalid by
construction.** Today the fetch fails *closed* only because `ChannelTable::new`
starts `next_id` at `1` — arithmetic luck, with `wrapping_add(1)`, so the invalid
id sits inside the space the allocator will eventually hand out. State the
decision; do not inherit it from a counter's initial value.

**R7 — The `upgrade` tier emits and is gated.** Its four markers —
`UNSIGNED_RELEASE_REJECTED`, `INVALID_SIGNATURE_REJECTED`,
`ACTIVE_SLOT_WRITE_REJECTED`, `HEALTH_FAILURE_NOT_CONFIRMED` — get an emitting
scenario, and `release_gated` becomes `true` with `not_gated_reason` removed.
Each marker must assert the **real refusal** at the point the decision is made, not
a print beside it: `verifyd`'s `VERIFY_FAIL` arm, and `fjell-bootctl`'s slot and
health decisions. `test-all` then reports **30 of 31 gated**, and the doc comments
that speak for **both** not-gated tiers must be narrowed so they no longer describe
a tier that is now gated.

> **Amended by the architect, 2026-10-14, at the first review — two errors of mine
> in the paragraph above.**
>
> 1. It named **`upgraded`'s own `confirm`** as a mechanism for
>    `HEALTH_FAILURE_NOT_CONFIRMED`. `confirm` is the **anti-rollback counter** and
>    has nothing to do with a health check; asserting that marker on it would have
>    asserted the wrong thing under a right-sounding name. The implementer caught
>    this and used `fjell-bootctl`'s A/B health-decision path, flagging the
>    deviation in `d9f1947`'s own message. **They were right and I was wrong**, and
>    the sentence is corrected rather than left for the next reader to trip over.
> 2. It said the **`store` tier's own `not_gated_reason`** spoke for a tier that is
>    now gated. It does not and never did — `tests/qemu/profiles/store.toml`'s
>    reason is per-profile and names only `store`. What spoke for both tiers were
>    the doc comments at `crates/fjell-tools/src/qemu_run.rs:56` and
>    `crates/fjell-tools/src/test_all.rs:21`, which is what the requirement now
>    says and what the implementer in fact narrowed.
>
> The difference between the two is worth naming, because it is the whole of this
> project's method: error 1 would have produced a tier asserting a falsehood, and it
> was corrected out loud. Error 2 could only corrupt the record, and it was worked
> around silently — `d9f1947`'s message claims `store.toml` was narrowed, and that
> file is untouched by this line. A mis-specified requirement is to be **named**,
> not satisfied in words.

**R8 — E-070 and E-072 CLOSED**, or survivors named, with `v1-limitations.md` in
the same commit as the register. `attestd`'s and `netd`'s consequences are **not**
re-derived in E-072 and must be, individually, before either is called closed —
same class is not same outcome. If the honest answer is that they survive, name
them as survivors with their reasons; do not round them into this line.

**R9 — The gates**, each read by its **own** exit status and not through a pipe;
`test-all`; the repro baseline re-recorded in the same commit as any rebuild; a CI
run id. If any prebuilt is rebuilt, trap 2 and the baseline rule apply.

### Non-goals

- Serving `diagnosticsd`'s, `attestd`'s or `netd`'s endpoints (RFC-0.35-002).
- A multiplexed `recv` in the kernel. `transportd`'s comment dates one to
  "v0.4.0"; it does not exist, and this line does not build it.
- Making the update path **fetch anything real.** The metadata reply is a stub
  (`transportd:186`), and closing E-070 must not read as "updates work now".
- The `store` tier's four markers. Its own line, when it has one.

## Risks

- **The gate is the deliverable, and it is the part most likely to be cut** when
  the eleven installs turn out to be fiddly. If only one of R3/R4 can land, land
  **R4**: a divergence that fails loudly is better than eleven quiet repairs.
- **Installing eleven capabilities is easy and wrong.** The temptation is to fill
  every declared slot so everything "works". Five of those slots belong to
  protocols nobody serves (E-068, E-069) and installing them grants standing
  authority to services that will not use it.
- **Removing the spin can hide it.** If the loops are fixed to block or exit
  before R2's transcript is taken, the evidence that they spun is gone for good.
