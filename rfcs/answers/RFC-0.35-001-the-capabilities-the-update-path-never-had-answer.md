# RFC-0.35-001 R1–R9, the two live transcripts, and the two re-derived survivors

**Governing RFC:** [../accepted/RFC-0.35-001-the-capabilities-the-update-path-never-had.md](../accepted/RFC-0.35-001-the-capabilities-the-update-path-never-had.md)
**Handoff:** [../handoffs/RFC-0.35-001-the-capabilities-the-update-path-never-had/implementation-handoff.md](../handoffs/RFC-0.35-001-the-capabilities-the-update-path-never-had/implementation-handoff.md)

Written in the order the handoff requires: R1 and both transcripts before any
fix, the gate's failing run before R3 landed, everything after R5 ordinary.
This document contains **measurements** (marked as such) and **design**
(marked as such). An Afterword, added after implementation, says what
survived contact.

---

## R1 — Re-derivation at `9dc6454`→`607ab49`, with controls, including the RFC's own two

| Finding | RFC's claim | What the tree says |
|---|---|---|
| Install set | `spawn.rs` installs, for every image, only slot 0 (per-image `match`, falling to shared object `0`) and `SERVICE_READY_SEND_SLOT`; further installs sit inside nine `if image_id == …` blocks | **Confirmed.** `BOOTCTL`, `SERVICE_MANAGER`, `DRIVER_UART`, `NEG_TEST`, `SAMPLE_SERVICE`, `SEMANTIC_STREAM`, `SVC_PRESENTATION_LIAR`, `PROXY_TEXT`, `CAP_BROKER`, plus the MMIO-region holders — no block for `SECURE_TRANSPORTD`, `DIAGNOSTICSD`, `UPGRADED`, `ATTESTD`, or `NETD`. |
| Eleven declared slots | Five services declare named capability slots the kernel never installs | **Confirmed**, re-enumerated directly from each service's own `CapHandle` constants: `secure-transportd` (1, 2, 3, 4), `diagnosticsd` (1, 2, 3, 4), `upgraded` (6), `attestd` (5), `netd` (1) — eleven, matching the RFC's count exactly. |
| `sys_ipc_recv` errors, does not block | `crates/fjell-kernel/src/cap/syscall.rs:552` resolves `cs.get(ep_h)` and returns `Err` immediately on an empty slot | **Confirmed** by reading the function; no blocking path exists for an unresolved capability. |
| Tag `0` is in neither vocabulary | `SxtTag::from_u16(0)` → `None`; `diagnosticsd` compares against `0x401`/`0x403` | **Confirmed**, both by reading the match arms. |
| **RFC's control 1** | The kernel's other install site populates `init`'s CSpace, not the services' | **Confirmed.** `main.rs`'s install block is a different site entirely, keyed by `init`'s own task index, never touching `spawn.rs`'s per-image table. |
| **RFC's control 2** | `cap_broker`/`cap_install`/`sys_cap` appear 0 times in `secure-transportd`, `diagnosticsd`, `upgraded` | **Confirmed** (`/usr/bin/grep -a -c`, each file, each term: 0). Control on the control: the same three terms appear in `fjell-cap-broker` itself (non-zero), so the search methodology finds real occurrences where they exist. |

Both of the RFC's own controls held. No escalation needed.

## R2 — Two transcripts taken before any fix, in a scratch worktree

**The spin, not a freeze.** `target/scratch/rfc-0.35-001-live-fail` (`git
worktree add --detach`, pre-fix tip), one instrumentation edit:
`secure-transportd`'s `recv_msg` `Err` arm gains a counter, printing
`secure-transportd: SPIN-PROBE iter=<n>` every 10,000 iterations (the console
is one byte per `ecall`; sparse by design). Built with `cargo xtask
build-services`, run under `cargo xtask qemu-run` with a 60-second timeout.
Result: **4,380,000+ iterations in a single boot**, counting still climbing
when the timeout killed the VM — not blocking, not a single error, a tight
re-entrant loop exactly as E-072 claimed. Confirms the kernel's trap-return
path does not freeze the whole machine: `trap_dispatch`
(`crates/fjell-kernel/src/trap/dispatch.rs`) calls `schedule_next(tf)` after
*every* trap, including a failed syscall, so other runnable tasks still get
turns between iterations — the cost is wasted cycles and a service that never
does its job, not a hang. Full transcript (not committed, scratch, cited by
path): `target/scratch/rfc-0.35-001-evidence/secure-transportd-spin-transcript.log`
(32,718 bytes, 818 lines).

**The gate, failing on all eleven.** R4's subcheck, written before R3's
fix, run against the pre-fix tree:

```
capability-topology: FAIL — SECURE_TRANSPORTD slot 2 (upgraded — the update path's metadata channel) is declared but spawn.rs installs nothing there
capability-topology: FAIL — UPGRADED slot 6 (secure-transportd — the update path's metadata channel) is declared but spawn.rs installs nothing there
capability-topology: FAIL
```

— plus nine `note` lines, one per remaining slot, each naming the line
responsible (`RFC-0.35-002`, or `secure-transportd`'s own slot 1, waived as
"not the update path"). All eleven printed in one run, satisfying R4's "show
that run" literally; two FAIL, nine waived-but-disclosed, exit non-zero. Full
output: `target/scratch/rfc-0.35-001-evidence/gate-before-fix.log`.

## R3 — One declaration of the topology

**Design.** Not a copy of `SERVICE_READY_SEND_SLOT`'s one-fixed-slot model —
the handoff's own trap 3 rules that out, since this is a per-service table,
not one slot shared by every image. The declaration is the pair of facts
already closest to true: a service's own `CapHandle` constant (unchanged,
each service still names its own slot number) and a single shared endpoint
object constant in `fjell_abi::service`
(`crates/fjell-abi/src/service.rs:274`, `UPDATE_CHANNEL_EP_OBJECT: u32 = 17`),
installed by `spawn.rs` at the exact slot number the service declares. The
object id is the one fact both sides read: `spawn.rs:954`/`:973` name the same
constant for both installs, and `secure-transportd`'s slot 2 / `upgraded`'s
slot 6 now resolve to the same kernel object. A new consistency-check gate
(below) is the mechanism that makes "names the same slot" an enforced fact
rather than a hoped-for one — the actual R3/R4 pairing the handoff asks for:
R3 is the shared constant, R4 is what makes drifting from it a build failure.

## R4 — `capability-topology`, Gate 12's 19th subcheck

**Design.** `tools/fjell-consistency-check/src/capability_topology.rs`, a
pure text-scanning checker (no dependency on the kernel or service crates,
matching this tool's own established convention). A `DECLARED_SLOTS` table
enumerates all eleven slots the RFC names, each with a `waived_by:
Option<&'static str>` — `None` for the two the update path needs, `Some(...)`
for the other nine, naming the line responsible for each. For each slot,
`check()` confirms the service file still declares the `CapHandle` (so the
table cannot silently go stale against a renamed or removed constant),
searches `spawn.rs`'s source text for the literal `image_id ==
fjell_abi::service::ImageId::{image}` comparison, and — only if found —
brace-matches that block's body for an `install_raw(\n<slot>,` call. Every
slot prints a line (`FAIL` or `note`, the latter naming its waiver) on every
run, so one invocation discloses the full topology; only non-waived misses
flip the exit code.

**A bug found and fixed inside this gate before it was trusted.** The first
version matched a bare `ImageId::ATTESTD` and walked backward
(`rfind("if ")`) to find the enclosing block — and false-positived on a
*match arm* (`ImageId::ATTESTD => 3,` in the unrelated `ep_obj` match further
up the file), walking into a different `if` block and finding an unrelated
`install_raw(5, ...)`, reporting `ATTESTD` slot 5 as installed when it is not.
Fixed by requiring the full `image_id == …` comparison (never a bare
type-path mention) and scanning **forward**, never backward, from the match.
A regression test (`image_block_installs_slot_ignores_a_match_arm_mention`)
reproduces the exact false-positive shape so this class of bug cannot return
silently. Six unit tests total, all passing, including one
(`declared_slots_are_still_real_on_the_current_tree`) that reads the actual
current source files, not a fixture.

Before/after runs, both cited above and in R2/R7.

## R5 — Installs: two slots, each justified; the other nine, named why not

**Installed, and only these:**

- **`secure-transportd` slot 2** (`spawn.rs:946`–`:962`) — SEND|RECV on
  `UPDATE_CHANNEL_EP_OBJECT`. Necessary because this is the service's *only*
  `recv` call site (the RFC's own table) and the update path's one real
  caller is `upgraded`; without it, E-070 cannot be closed regardless of any
  word-carrying fix, because the IPC never reaches a listener.
- **`upgraded` slot 6** (`spawn.rs:965`–`:981`) — the same rights, the same
  object. Necessary for the symmetric reason: `upgraded` is the sender, and
  without its own install the send itself fails the capability check before
  any message leaves the task.

**Not installed, and why each of the other nine is not this line's
business:**

| Slot | Why not |
|---|---|
| `secure-transportd` slot 1 (smgr readiness) | Superseded by `SERVICE_READY_SEND_SLOT`, which every other service already uses for this purpose; installing a second, parallel readiness path is not a fix, it is a second wrong mechanism next to the first. |
| `secure-transportd` slots 3, 4 (diagnosticsd, attestd) | Nobody serves these protocols' other end in any committed caller the update path needs; RFC-0.35-002's decision, named in E-069. |
| `diagnosticsd` slots 1–4 | None are the update path; `diagnosticsd`'s own request paths are tracked under E-068/E-069, RFC-0.35-002. |
| `attestd` slot 5 | Not the update path; also, re-derived below, not a live hazard the way the first two were. |
| `netd` slot 1 | Not the update path; also, re-derived below, not a live hazard. |

Installing any of the nine would grant standing authority to a component
that does not yet exercise it — the least-privilege violation the RFC
itself names as the temptation to resist.

## R6 — The word, and the id space

**Decision: carried, not unclaimed — and both words, not one.** The RFC
named the channel-kind word (`transportd:139`'s `kind (high 8 bits) |
server_name_ptr (low 24 bits)` comment, `upgraded:186`'s `kind =
UpdateMetadata`). Tracing the protocol past the open call showed the
channel-**id** word is equally load-bearing: `UpdateMetadataFetch`/`Close`
address a channel by the id the `Opened` reply carries, and carrying only the
kind word would make `OpenChannel` succeed while every subsequent step still
addressed channel `0`. Both are now carried for real via
`sys_ipc_send_words(ep_slot, tag, w0, w1)`
(`crates/fjell-syscall/src/lib.rs:366`) — the exact ABI surface RFC-0.34-004
§C deferred as "its own line": this is that line.

**Channel id `0` invalid by construction**, not inherited from a counter.
`crates/services/fjell-secure-transportd/src/channel.rs:106`:
`pub const INVALID_CHANNEL_ID: u32 = 0`, checked at the top of `find_mut`
(`:146`) regardless of table state, and `open` (`:129`) skips assigning it on
wraparound. Previously the invariant held only because `ChannelTable::new`
happened to start `next_id` at `1` with `wrapping_add(1)` — arithmetic luck
sitting inside the space the allocator would eventually hand back out. Now
it is a named constant checked explicitly, independent of the counter's
starting value.

No host unit test exists for `ChannelTable`'s invariants: confirmed live that
`cargo test -p fjell-secure-transportd` fails with `E0152` (duplicate lang
item `panic_impl`) — this is a `[[bin]]`-only `#![no_std] #![no_main]` crate
with its own `#[panic_handler]`, structurally incompatible with the
std-linked test harness. The `upgrade` QEMU tier exercises the invariant
instead (R7).

## R7 — The `upgrade` tier: four markers, each asserted at the deciding service

Each marker follows RFC-0.34-004's `semantic-liar` pattern: the deciding
service's own output *and* the instigator's own independent read, both
required.

| Marker | Deciding service's own output | Instigator |
|---|---|---|
| `UNSIGNED_RELEASE_REJECTED` | `verifyd: signature verification failed, rejecting` (new `sys_debug_writeln` in the `VERIFY_FAIL` arm) | `fjell-neg-test`'s `test_upgrade_verify_rejections`, `sys_ipc_call_words` to `verifyd`'s own dedicated endpoint, checks the reply tag itself |
| `INVALID_SIGNATURE_REJECTED` | Same `verifyd` line, second call with a forged signature word | Same function, second call |
| `ACTIVE_SLOT_WRITE_REJECTED` | `bootctl: candidate-set into the active slot REFUSED` (new dispatch arm, `BootError::ActiveSlot`) | `fjell-neg-test`'s `test_upgrade_active_slot_write_rejected`, `sys_ipc_call_words` to `bootctl`'s probe tag |
| `HEALTH_FAILURE_NOT_CONFIRMED` | `bootctl: health FAILED on the last confirmed slot; no fallback, not resetting` (already-existing line, reused from the working `health-fail` tier) | `bootctl`'s own `HealthVerdict::NoFallback` arm, which now also prints the marker |

All four verified live: `cargo xtask qemu-run --profile upgrade` → **PASS (7
marker(s) matched)** (the above four plus `verifyd`'s and `bootctl`'s own
prose lines, counted separately by the profile). Full serial log:
`target/scratch/rfc-0.35-001-evidence/upgrade-tier-passing.log`.

**`verifyd` given its own endpoint, not the shared default.** Before this
line, `verifyd`'s `EP_SLOT = 0` fell to the same shared object-0 default many
untouched services use, and nothing had ever sent to it. Making `neg-test`
its first real caller risked a silent cross-service collision on that shared
object. Fixed by giving `verifyd` its own `VERIFYD_EP_OBJECT`
(`fjell-abi/src/service.rs:283`), mirroring the precedent RFC-0.33-001 D8 set
for `bootctl`. `neg-test` is granted `CapRights::CALL`-only capabilities to
both endpoints (`spawn.rs:589`–`:611`), least-privilege: `sys_ipc_call`/
`sys_ipc_call_words` check `CapRights::CALL` alone
(`crates/fjell-kernel/src/cap/syscall.rs:629`), and `neg-test` has no other
business with either endpoint.

**Deviation from the amendment note's "`upgraded`'s own confirm."** The
architect's E-070 amendment mentioned `upgraded`'s own confirm as part of the
reachable machinery; I used `bootctl`'s A/B health-decision path instead,
because `upgraded::confirm` is the anti-rollback counter mechanism, unrelated
to a health check — reusing it for `HEALTH_FAILURE_NOT_CONFIRMED` would have
asserted the wrong thing under a right-sounding name. `bootctl`'s
already-working `health-fail` scenario needed only one new print at the
already-correct decision point.

**`release_gated: true`, proven for the right reason first.** The run cited
above is the one proving it: all seven markers present, the gate's own exit
status 0, before the flip was made permanent in `upgrade.toml`.

**`store`'s reason narrowed.** `not_gated_reason` previously spoke for both
`store` and `upgrade`; now it names only `store` (`fjell-storaged` has no
CRC, superblock, mirror, or recovery code to exercise, so its four markers
need a store that can reject, not a test — a separate, still-open line).
Narrowed in `tests/qemu/profiles/store.toml`, `crates/fjell-tools/src/
qemu_run.rs`'s and `test_all.rs`'s doc comments, in the same commit as the
`upgrade` flip.

## R8 — E-070 and E-072: closed, with two survivors re-derived individually

**E-070, CLOSED.** The uncarried word was a symptom, not the root cause: the
capability did not exist until E-072's two slots were installed, so no
word — carried or not — was ever going anywhere. With both installed and
carried, `fetch_update_index` reaches a real `secure-transportd` and the
handshake **correctly fails closed** (no simulated trust; real certificate
verification is v0.7.3's own, separate, already-documented stub) —
`fetch_update_index` still returns `None`, now for that honest reason, not a
silent IPC failure. Demonstrated live:
`target/scratch/rfc-0.35-001-evidence/update-channel-exchange-after-fix.log`
— `upgraded: fetching update index` → `secure-transportd: starting` →
`secure-transportd: ready` → `secure-transportd: handshake failed` →
`upgraded: update index fetch returned no index`.

**E-072, CLOSED for the update path, with `attestd` and `netd` named as
survivors — re-derived individually, not rounded in, per the RFC's own R8
instruction that same class is not same outcome.** Re-deriving both:

- **`fjell-attestd`'s slot 5 (`CAP_SXT_EP`)** is addressed only by
  `push_attestation`, which has **zero call sites anywhere in the tree**
  (`/usr/bin/grep -rn "push_attestation" --include="*.rs" .`, checked against
  the live tree, excluding old scratch clones under `.git-exclude/`) — a
  dead `pub fn`, not a live loop. `attestd`'s own `service_main` loop
  receives on slot 0 (`EP_SLOT`), never slot 5. The missing capability's
  `Err(_) => (0, 0)` path in `sxt_recv` is therefore never reached at
  runtime at all, let alone in a tight loop.
- **`fjell-netd`'s slot 1 (`CAP_DRV_EP`)** has **no reference anywhere
  outside its own `const` declaration** (same grep, same scope) —
  `recv_msg` hardcodes `sys_ipc_recv_msg(0)`, never `CAP_DRV_EP.0`.

Both are unreachable dead declarations, not busy-spins — a materially
different and better outcome than `secure-transportd`'s and
`diagnosticsd`'s, found by checking rather than assumed from "same class."
Named as survivors in `rfcs/ERRATA.md`'s E-072 entry and
`docs/src/releasing/v1-limitations.md`, both in this commit set, with the
re-derivation's specifics in each.

**Also not touched, named rather than silently left:** `secure-transportd`'s
and `diagnosticsd`'s wrong readiness-notification slots (one sends into an
empty slot, one into the service's own shared endpoint instead of
`service-manager`'s) — unrelated to this line's scope, still wrong.
`diagnosticsd`'s own `CAP_TOOLS_EP` (slot 1) remains uninstalled
(RFC-0.35-002's decision); its `recv_msg` now yields instead of spinning
tightly on the resulting error, but the error itself still fires every
iteration, every boot, until that line lands.

## R9 — Gates, `test-all`, baseline, CI

- `cargo xtask consistency-check capability-topology` (its own exit status):
  before-fix `FAIL` (2 of 11), after-fix `PASS (2 installed, 9 waived to a
  named line, 11 checked)`.
- `cargo xtask qemu-run --profile upgrade`: `PASS (7 marker(s) matched)`.
- Neighbouring tiers re-run and confirmed unaffected: `health-fail`,
  `store`, `svc`, `semantic-liar`.
- `cargo xtask test-all`, clean, solo (no other build in flight): pending
  this document's own final pass — see the review request for the run this
  line is handed back on.
- Repro baseline: re-recorded in the same commit as any rebuilt prebuilt
  (trap 2 / R9), after confirming which of
  `fjell-secure-transportd`/`fjell-upgraded`/`fjell-bootctl`/`fjell-verifyd`/
  `fjell-neg-test`/`fjell-diagnosticsd` actually changed bytes.
- CI run id: none — nothing pushed, matching precedent from
  RFC-0.34-004/005's own local-only submissions.

---

## Decisions I am making that nothing above the line specified

1. The shared endpoint constant's name, `UPDATE_CHANNEL_EP_OBJECT`, and its
   home in `fjell_abi::service` alongside every other named endpoint object.
2. `verifyd`'s new dedicated endpoint's name, `VERIFYD_EP_OBJECT`, and
   giving it one at all rather than leaving `neg-test` to collide with the
   shared object-0 default.
3. `capability_topology.rs`'s `waived_by: Option<&'static str>` design — a
   gate that prints every known divergence but fails only the unwaived
   subset — rather than either failing until RFC-0.35-002 lands or silently
   ignoring the other nine.
4. Driving all four `upgrade.toml` scenarios through `fjell-neg-test`
   (explicitly named in the RFC's own *Touches* list) rather than through
   `fjell-init`'s single-byte-per-boot console trigger.
5. Reusing `bootctl`'s A/B health-decision path for
   `HEALTH_FAILURE_NOT_CONFIRMED` instead of `upgraded::confirm` (justified
   in R7, above).
6. `sys_yield()` as the general-case fix for a `recv` error, in both
   `secure-transportd` and `diagnosticsd`, independent of which specific
   slot caused it — not scoped only to the two slots this line installs.

I will not touch `diagnosticsd`'s, `attestd`'s, or `netd`'s uninstalled
slots, the wrong readiness-notification sends, `fjell-auditd`'s missing
`AUDIT_QUERY` implementation, `secure-transportd`'s round-robin polling gap,
or real TLS certificate verification.

---

## Afterword — what survived contact with the code (written after it)

Everything above this line was written before the commits that implement it
and is left as it was. This is what the running system did to it.

**Kept, and shown to work:** the shared `UPDATE_CHANNEL_EP_OBJECT`, installed
at `secure-transportd` slot 2 and `upgraded` slot 6; `verifyd`'s own
dedicated endpoint; both channel-kind and channel-id words carried via
`sys_ipc_send_words`; `INVALID_CHANNEL_ID` checked explicitly in `find_mut`;
the `capability-topology` gate's waiver design; all four `upgrade.toml`
markers, each asserted at its deciding service and independently read by
`neg-test`; the general-case `sys_yield()` fix in both spinning services.

**A requirement under-scoped on first pass, caught by re-reading the
erratum, not by a review.** Installing the two capabilities stops the
specific busy-spin E-072 measured, but `diagnosticsd`'s own main loop
receives on its own, still-uninstalled `CAP_TOOLS_EP` — re-reading E-072's
own resolution text ("a recv error must not be a tight loop," stated as
separate and not optional) caught that this needed a general fix, not a
fix scoped to the two slots R5 installs. `sys_yield()` was added to both
services' `recv_msg` `Err` arms as the general case.

**A gate bug found and fixed before the gate was trusted**, not after:
`image_block_installs_slot`'s backward-`rfind` search false-positived on a
match-arm mention of `ImageId::ATTESTD`, reporting an uninstalled slot as
installed. Found by inspecting the gate's own output against the source by
hand before relying on a green run, fixed by requiring the full comparison
pattern and searching forward only, and pinned with a regression test
reproducing the exact shape.

**R8's instruction to re-derive, not round in, produced a materially
different finding than "named survivor" alone would have implied.**
Checking `attestd`'s and `netd`'s actual call sites — not assuming they
share `secure-transportd`'s and `diagnosticsd`'s busy-spin because they share
the "declared slot, nothing installed" shape — found that neither is a live
hazard at all: `push_attestation` has no caller, `CAP_DRV_EP` has no
reference beyond its own declaration. Reporting them merely as "same class,
not re-derived" would have been honest about the gap but would have left a
reader assuming the worse outcome by default; re-deriving them found the
better, true one instead.

**Found while in the affected files, fixed in passing, disclosed here
rather than folded into E-070/E-072's own claims:** `secure-transportd`'s
top-of-file comment falsely claimed it "signals readiness and exits before
its main loop by design" — it never did, and does so even less now that it
genuinely exchanges messages; `v1-limitations.md`'s "smoke-test stubs" list
incorrectly included `secure-transportd` (removed) and also, separately,
incorrectly includes `fjell-proxy-text` (noted, left uncorrected — out of
this line's scope); `qemu_run.rs`/`test_all.rs` had a duplicate `#[test]`
attribute on an unrelated test, pre-existing, removed on sight.

**Not verified, and said so:** whether `attestd`'s or `netd`'s dead
declarations are themselves symptoms of a larger gap in those services'
design (an attestation-push path that was never wired to anything, a
driver-query path that was never built) — that is a question for whoever
takes RFC-0.35-002, not answered here.
