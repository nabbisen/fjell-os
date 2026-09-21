# RFC-0.33-001 §A–§E, and six questions the RFC did not ask

**Governing RFC:** [../accepted/RFC-0.33-001-a-boot-control-that-controls-boot.md](../accepted/RFC-0.33-001-a-boot-control-that-controls-boot.md)

Written after R1 and before the kernel work, in the handoff's order. §A–§E are
answered. **§F is new**: R1 found that the RFC's shape — "the state machine, then
one syscall and one MMIO write" — is smaller than the work, and six of the
differences are design questions that are the architect's, not mine. They are
stated with a recommendation each, and **the work that depends on them has not
been started**; §G says exactly which.

---

## Two things first, because they change conclusions already reached

### R1's first probe was blind, and the same blindness reached the record

The handoff's probes for "no runtime `BootControlBlock`" and "nothing sends
`BOOT_*`" both pass, as written, through a `grep`. **In this environment `grep` is
a shell function over ugrep with `-I`, which silently skips any file containing a
NUL byte.** `crates/services/fjell-init/src/main.rs` has two (line 844, a byte
string written with raw NULs) and is the only tracked source file that does —
and `init` is where this system spawns services, formats its store and writes
its boot-control block.

With a NUL-safe `grep -a`, the same probes find `init` at every step (§R1 below).
**The same tool produced RFC-0.32-002's "0 raw-reinterpretation sites outside the
kernel"**, which I wrote into E-046's resolution, the 0.32.0 CHANGELOG and the
0.32.0 release record. It is **five** sites, all in `init`. Its control passed
because it ran on other files. Both are now corrected where they can be
(`5ef961d`), and the lesson is a saved rule.

### RFC-0.32-002 silently broke `init`'s semantic emissions — and I have fixed it

`init`'s `emit_envelope` was the one sender RFC-0.32-002 missed. It kept sending
raw `SemanticEnvelope` struct bytes after `semantic-stream` began decoding the
wire format, so **all 18 of `init`'s emissions were refused at `BEGIN` from
`201d191`**. Same command, three kernels, 25 s each:

| kernel | lines | `[STATE]` | `[EVENT]` | `[INTENT]` |
|---|---:|---:|---:|---:|
| 0.31.0 | 320 | 8 | 3 | 3 |
| HEAD before the fix | 227 | **0** | **0** | 1 |
| HEAD with the fix (`244c4ab`) | 320 | 8 | 3 | 3 |

No tier noticed: all four `semantic` markers come from the sender that *was*
converted. A fifth marker, `Verified boot status` (an `init` render), now guards
it; it fails on the regressed log and passes on the fixed one. **This is fixed
and committed; it is not part of this line's scope and I did not wait for a
ruling, because it is a regression I introduced.** Four `init` `from_raw_parts`
sites remain (three on-disk structs viewed as bytes to fill sectors) — named,
not fixed, in §F5.

---

## R1, re-derived

Every absence with a control **on the file the claim is about**, using
`/usr/bin/grep -a`.

| Claim (RFC / handoff) | Re-derived | |
|---|---|---|
| `fjell-bootctl` is 74 lines; its state is a 3-variant enum | 74 lines; `enum BootState { Pending, Confirmed, Rollback }` | ✔ |
| **No crate constructs, reads or writes a `BootControlBlock`** | **`fjell-init` does**: `BootControlBlock::new(1)`, `seal()`, then writes it to **both mirrors** (`LBA_BOOT_CTL_A_START`, `_B_START`) via `storaged` — `init/main.rs:583–600`. Nothing *reads* one. | ✘ |
| Two protocols, zero users | `READ_BCB`/`WRITE_BCB` are constants only; `BOOT_*` is named only by `bootctl` itself and by declarations; nothing sends any of it | ✔ |
| `syscall-surface`: 35 declared, 29 dispatched, 6 undispatched | exactly, and the six are `CapInstall`, `PlatformReboot`, `TaskKill`, `MmioUnmap`, `DmaShare`, `Reboot` | ✔ |
| **The kernel has no reset path** | none — searched `sbi`, `0x100000`, `syscon`, `finisher`, and my own additions (`sifive_test`, `poweroff`, `SRST`, `0x5555`, `0x7777`, `0x3333`). **Control: `0x1000_0000` finds 14 UART sites in the same directory.** | ✔ |
| `fjell-bootctl-model` has no dependents | only the workspace member lists name it | ✔ |
| `upgrade.toml` expects 4 markers; nothing emits them | exactly, `release_gated = false` | ✔ |
| **Nothing sends `storaged` a sector write today** (handoff §3) | **`init` sends six** (LBAs 193, 65, 193, 65, 1, 33). What is missing is the *read* side — §A | ✘ |
| **`HealthTarget` exists in `fjell-upgrade-format` with no evaluator** (RFC Finding 5; ADR-0009) | **There is no `HealthTarget` type anywhere in the tree.** `git`-wide, `struct HealthTarget` matches nothing (control: `struct SlotInfo` matches once). ADR-0009's prose is the only place it exists. | ✘ |
| **`service-manager` already tracks readiness and faults** (RFC D3, Finding 5) | Readiness, yes. **Its timeout and fault branches are unreachable**: entries are created only on receipt of a `READY`, with `ready: true` and `task_handle: 0`, so `!e.ready` and `e.task_handle != 0` can never hold. The `svc` profile's timeout and fault markers are emitted by `neg-test`, which spawns and inspects the tasks itself. | ✘ |
| **The kernel change is "one syscall and one MMIO write"** (RFC; handoff §2) | Also needs three more kernel touches — §F1 | ✘ |
| ADR-0009: health "is a fixed `true` constant" | It is — in **`init`** (`let health_ok = true`, line ~683), not in `bootctl`. `init` also prints `M7: health target passed`, `M7: slot confirmed after health`, `M6: boot confirmation simulated` and `M6: boot-control mirror valid` unconditionally. Nothing gates on any of them. | ✔ but misplaced |

Also verified: `bootctl` **does** run (`bootctl: started (RFC 057)` is in every
recent log) and then blocks in `ipc_recv`; it receives on **shared object 0**, the
endpoint RFC-0.28-001 found others racing for; and its Cargo.toml lists four
format crates it never uses.

---

## §D — how the kernel resets the board (answered first: the rest depends on it)

**The device.** QEMU `virt`'s own device tree, dumped with `-machine
dumpdtb`, contains:

```
test@100000 { reg = <0x00 0x100000 0x00 0x1000>;
              compatible = "sifive,test1", "sifive,test0", "syscon"; }
reboot   { compatible = "syscon-reboot";   regmap = <&test>; offset = <0>; value = <0x7777>; }
poweroff { compatible = "syscon-poweroff"; regmap = <&test>; offset = <0>; value = <0x5555>; }
```

So the reset is **a 32-bit store of `0x7777` to physical `0x100000`**.

**Where the address comes from: a constant in
`crates/fjell-kernel/src/platform/qemu_virt.rs`, beside `RAM_BASE` and
`MMIO_REGIONS`.** Not a `BoardProfile` field. Verified: `fjell-platform-format`
has no reset, reboot, poweroff or syscon entry (control: 25 UART/PLIC hits in the
same directory), and **the kernel does not consume a `BoardProfile` at all** — its
platform data is hard-coded (`dtb_pa: 0`, *"Full DTB parsing is deferred"*). A
profile field would be filled by nothing, which the RFC rules out.

**What that costs on real hardware, plainly.** The constant is QEMU-virt-specific.
A real RISC-V board resets through an SBI `SRST` call to firmware, or a
board-specific watchdog/PMU register — a different *mechanism*, not a different
address. The syscall's interface (a capability-checked reboot request) is
board-independent; its implementation is a platform hook, and `qemu_virt.rs` is
where the QEMU one belongs. The DTB's `syscon-reboot` node is the standard,
discoverable source and is what a DTB-parsing kernel would read; this one does not.

**The audit.** `// MMIO-ORDER: device_kick` — a store that triggers an action. The
justification is a real one: everything the kernel wants visible before the machine
goes (the UART bytes, the audit record of the request) must be ordered before the
store, so a full `fence` precedes it; **a write that never returns needs no fence
after it, because nothing follows it.** If it *does* return — the device absent or
ignoring the value — the handler returns an error and the call has not succeeded,
so `bootctl` can tell.

**A hazard that would have shown as a hang.** The kernel's own page table maps RAM,
one UART page and the eight virtio slots — **not `0x100000`** — and a syscall
handler runs on the *calling task's* page table, which gets its device pages
mapped at spawn. RFC-0.25-001 met exactly this: *"kmain's own `plic::init()` …
faulted with StorePageFault the first time it touched the PLIC — caught live
rather than assumed."* The reset store would fault in the kernel: a hang, the
outcome D5 exists to rule out. The fix has a precedent — `spawn.rs:69` maps
`plic::MAPPED_PAGES` into every task, kernel-only (`R|W`, no `U`) — and is one of
the touches in §F1.

### How the harness tells a reset from a hang (the handoff's first flag)

I built a scratch bare-metal binary outside the tree that prints `BOOT`, then stores
to `0x100000`, and ran it under the harness's own QEMU command shape. QEMU 11.1.1:

| probe | flags | exit | `BOOT` lines | QEMU's own account (QMP `SHUTDOWN`) |
|---|---|---:|---:|---|
| hang (no store) | — | 124 | 1 | — |
| hang | `-no-reboot` | 124 | 1 | `{guest: false, reason: "host-signal"}` |
| reset, `0x7777` | — | **124** | **32,086** | — |
| reset | `-no-reboot` | **0** | 1 | `{guest: true, reason: "guest-reset"}` |
| poweroff, `0x5555` | — | 0 | 1 | — |
| poweroff | `-no-reboot` | 0 | 1 | `{guest: true, reason: "guest-shutdown"}` |

Three consequences:

1. **Without `-no-reboot`, a real reset is a boot loop** — 32,086 boots in six
   seconds — and ends as a `timeout` kill: **exit 124, exactly like a hang.** This
   is the handoff's "a reset looks like a hang", measured.
2. **`-no-reboot` alone is not enough.** A reset and a *power-off* both exit 0. A
   kernel that wrote `0x5555` by mistake would look like success.
3. **QEMU itself distinguishes all three**, in the `SHUTDOWN` event on its QMP
   socket: `guest-reset` versus `guest-shutdown` versus `host-signal`.

**The design:** the harness runs a reset profile with `-no-reboot` and a QMP
socket, and passes it only if QEMU reports `SHUTDOWN` with `guest: true` and
`reason: "guest-reset"` *and* QEMU exited by itself rather than being killed. The
machine's behaviour, reported by the machine's host, is the evidence — no guest
marker asserts a reset. (The harness today ignores QEMU's exit status entirely:
`run_profile` reads only the serial output.) This is my tooling and in scope for
R6; I am building it.

---

## §A — persistence: **out of scope, and here is exactly why**

The handoff said to check first. I did. To make "the block survives the reset"
true, four things must hold, and **three do not**:

1. **Something can write a sector.** ✔ `init` does — six writes. *(The handoff's
   premise that nothing does was wrong; the conclusion below does not depend on it.)*
2. **Something can read one back.** ✘ `storaged`'s `READ_COMMIT` does the disk
   I/O into storaged's *private* buffer and replies `READ_OK`; **`READ_CHUNK` is a
   placeholder** (`chunk_off += 32; reply(READ_CHUNK)`, *"READ not used in M7"*).
   A client can never get a sector's contents.
3. **`bootctl` can reach `storaged`.** ✘ Its CSpace is its own endpoint and the
   shared readiness-send slot; no capability to storaged, and `CapInstall` — the
   dynamic way to grant one — is undispatched.
4. **Nothing overwrites the block first.** ✘ `init` writes **both boot-control
   mirrors unconditionally at every boot** with a fresh `BootControlBlock::new(1)`,
   so a block `bootctl` persisted would be clobbered by the next boot's `init`
   before anything could read it. And the harness recreates the disk as a
   zero-filled 16 MiB file before every run.

That is a store *client*, a store *server* fix, a capability grant and a change to
`init`'s boot — a line of its own, which the handoff says to stop and escalate
rather than write. **So: this line delivers the decision and the reset, and the
block is `bootctl`'s in-memory state. It is not durable, and nothing here says
otherwise.**

**The consequence a reader must not miss.** Without a persisted try counter, a
rollback that resets a machine whose health fails *again* resets it again,
forever. `remaining_tries` exists to bound that; it cannot be bounded across a
reset it does not survive. In the QEMU profile `-no-reboot` stops the loop. In a
shipped image the loop has no source **only because nothing in a normal boot can
fail health** — which is why §F6 asks for the evaluator to fail on positive
evidence only.

---

## §B — which protocol survives: `BOOT_*`, **without** the block on the wire

The RFC's lean is to keep `BOOT_*`, extend it to carry the block, and delete the
unimplemented pair. I agree with the first and last and **disagree with the
middle**, and the argument is Finding 2's own.

Carrying the 88-byte block over IPC would create a protocol with zero users — the
defect that produced two protocols. The block is `bootctl`'s private state (D1).
Nothing outside it needs the whole block until persistence exists, and when it
does, `bootctl` talks to `storaged` over *storaged's* sector protocol, not its own.

So: **keep `BOOT_*` as small commands that carry words**, and delete
`fjell_service_api::bootctl::{READY, READ_BCB, WRITE_BCB, READ_OK, WRITE_OK, ERR}`
— six names, `0x210`–`0x215`, and **none has a user anywhere** (`bootctl::` is
referenced nowhere). They are in the ABI snapshot, so Gate 4 will report six
`Removed` and the snapshot is re-recorded — retired through the process, not
dropped. Someone chose those names; the record of why is RFC 019, and the answer
is that the protocol they describe was never built.

The surviving surface, per §E: a **health report** from `service-manager`
(carrying whether, and which service failed), a **state query**, and `BOOT_SHUTDOWN`
kept as-is. `BOOT_CONFIRM` and `BOOT_ROLLBACK` as *commands* go: a reporter that
issues them is a reporter that decides, which §E rules out.

## §C — `PlatformReboot` (18) survives; `Reboot` (120) is retired

Agreed. 18's name says what it does and its doc comment already calls 120 an
"alias". `SyscallNumber` is one item in the ABI snapshot, so removing a variant
shows as **`Changed sig: 1`** (plus §B's six `Removed`) and is re-recorded.
`syscall-surface` afterwards: **34 declared, 30 dispatched, 4 undispatched** —
`CapInstall`, `TaskKill`, `MmioUnmap`, `DmaShare`, named, not fixed. `CapKind::Reboot`
(a *capability kind*, value 9) is unrelated to the number 120 and stays.

## §E — `service-manager` reports; `bootctl` decides

Agreed with the lean, for the RFC's reason. `service-manager` sends **one**
observation — *healthy*, or *unhealthy and which service* — and `bootctl` applies
the ADR's transitions and replies with what it decided. A `BOOT_CONFIRM` that
`service-manager` could also withhold is two mechanisms in one place.

---

## §F — what the RFC did not ask

Each has a recommendation; **each needs the architect**, because each changes the
line's shape or crosses a boundary the handoff drew.

**F1. The kernel change is four touches, not two.** The handoff: *"One syscall,
one MMIO write … Nothing else in the kernel changes"* and *"Do not touch the kernel
beyond the one dispatch arm and the reset site."* To make the syscall reachable and
safe:

| # | Touch | Why it is not optional | Precedent |
|---|---|---|---|
| 1 | `PlatformReboot` dispatch arm + handler (cap check, fence, store) | D4 | — |
| 2 | map the reset device's page **kernel-only into task address spaces** (`spawn.rs`, and `main.rs`'s `init` bootstrap) | the store faults otherwise — a hang | `plic::MAPPED_PAGES`, RFC-0.25-001 |
| 3 | install a `Reboot` cap in `bootctl`'s CSpace slot 1 (`spawn.rs`) | the capability check would fail closed for its **only** caller — no `CapKind::Reboot` is granted anywhere | `TaskCreate`/`LeaseAdmin` installs |
| 4 | a **dedicated endpoint** for `bootctl` and a send cap for `service-manager` (`spawn.rs` `ep_obj` table) | `bootctl` receives on shared object 0, which others race for (RFC-0.28-001), so `BOOT_*` could be received by a stranger | RFC-0.28-001 |

**Recommend: approve all four.** Each mirrors an existing pattern and is the same
shape as the first, and `CapInstall` being undispatched means there is no
non-kernel way to do 3. *Blocks:* D3's delivery, D4, D5, D6.

**F2. What does `service-manager` evaluate, and how does it learn what to watch?**
D3 says it decides "from readiness and faults it already tracks". It tracks
readiness. It tracks **no** faults and no timeouts (R1). Two honest readings:

- **(a) Readiness only.** A compile-time required set; unhealthy if any has not
  reported `READY` by the deadline. Needs no task handles. `sys_task_status` stays
  unused, and D3's "faults" is retired in the ADR.
- **(b) Readiness and faults.** `init` tells `service-manager` each required
  service's image id and **task handle** (init holds the handle from
  `sys_task_spawn`), and the dead `task_handle`/`fault_emitted` fields become live.
  A small registration message; `init` gains a few lines.

**Recommend (b)**, because D3 is settled as "readiness and faults" and (a) does
not deliver it — but (a) is a genuine, smaller alternative and the choice is the
architect's. `HealthTarget` is retired either way: it never existed, and the
required set is a `const` list in `service-manager`, not a type in a format crate.

**F3. What makes health fail in the demonstration and nowhere else?** D6 says
"health fails"; it does not say how. Every profile boots the *same* kernel image, so
the failure needs an input that only this profile supplies. The channels a profile
controls: the **console** (existing `inject_after_marker`, RFC-0.25-001 — the byte
reaches `init`) and **QEMU's command line**. The disk is unreadable (§A) and the
DTB unparsed. **Recommend: a console-injected trigger consumed by `init`, which
spawns `svc-fault` and registers it as required** — so the failure is *real*: a real
task really faults and `service-manager` really detects it through the same path a
production failure would use. Rejected: an always-failing required service (fails
every tier); a build flag (a second prebuilt set); a guest marker asserting failure
(the line's own prohibition). **Cost, stated:** test scaffolding in the shipped
image — `neg-test` is already one, and it is the project's existing pattern, but it
is a console byte that can cause a reset and that deserves a ruling.

**F4. Every boot starts unconfirmed; who confirms on the happy path?** `bootctl`
starts `Pending` and nothing ever sends `BOOT_CONFIRM`, so `bootctl: CONFIRMED` is
unreachable today. In the design: after health passes, `service-manager` reports
*healthy*, `bootctl` confirms and prints it — so **every smoke tier's happy path
observes a real confirmation**, replacing `init`'s unconditional
`M7: slot confirmed after health`. Those four `init` markers are claims with
nothing behind them and no tier depends on them; I propose to leave them alone and
name them, since removing serial output is not this line's job.

**F5. `init`'s four remaining `from_raw_parts` sites** (`StoreSuperblock` ×2,
`RecordHeader`, `BootControlBlock`) fill sector buffers from struct memory —
E-046's Finding 4 on the write side. Fixing them means explicit
`to_sector()` serialisation in the format crates and **changes on-disk bytes**, so
it is a decision and probably a version bump. **Recommend a small line of its own
(or E-046 re-opened)**; the `BootControlBlock` one would be the natural first
because this line needs an explicit block serialisation the moment persistence
arrives.

**F6. The false-positive risk is the line's real hazard.** A rollback that resets
the machine, with no persisted counter, means **a health evaluator that is ever
wrong in the direction of "unhealthy" turns every boot into a reset loop** — and
every QEMU tier with it. So: the evaluator fails on **positive evidence only** (a
required service *observed* faulted, or one that has not reported by a deadline set
well above today's boot), and the required set starts small and stable. The
`svc`, `smoke-*` and `uart-rx` tiers are the regression check; I will run all of
`test-all` before any kernel change is pushed.

---

## §G — what I will do, what I will not, and why

**Doing now, because it depends on nothing in §F:**

- **The harness** (R6): `-no-reboot` + QMP, a profile key for the expected shutdown
  reason, tested against the scratch probes above (reset passes; power-off and hang
  each fail, by different reasons).
- **D1/D2**: `BootControlBlock` transitions as safe code beside the type, `bootctl`
  holding a real block, the tests tying it to `fjell-bootctl-model` — with the
  rollback arm still ending in today's spin (handoff §1: land the part that cannot
  brick anything first).
- §B/§C's deletions **once §F1 is answered**, since they share the ABI re-record.

**Not started, pending §F:** the kernel touches (F1), `service-manager` health (F2),
the failure trigger (F3), the reset profile and its demonstration (D5/D6), and the
confirm-path observation (F4). All are downstream of F1 or F2 or both.

**Not doing:** persistence (§A), a store client, slot switching (D7), gating
`upgrade.toml`, touching `init`'s remaining store sites (F5), or adding a
`BoardProfile` field.

*A note on `5ef961d`'s message: it explains an earlier miscount ("six", in
`f447b90`) by a cause I did not verify. The true statement is only that `init`
held five sites; the message is left as committed, because history is fixed
forward.*
