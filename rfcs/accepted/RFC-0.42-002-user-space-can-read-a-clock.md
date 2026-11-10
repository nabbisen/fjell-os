# RFC-0.42-002: User space can read a clock, and the timer stays dormant

**Status:** Accepted — accepted 2026-11-10
**Milestone:** 0.42
**Tracks.** **E-106** — Fjell exposes no notion of time to user space, so no service can express a
duration.
**Touches** *(indicative)*: `crates/fjell-abi/src/syscall.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`, `crates/fjell-cap/src/rights.rs`,
`crates/fjell-syscall/src/lib.rs`, `crates/services/fjell-storaged/src/main.rs`,
`tests/abi/snapshot.json`, prebuilts and `tests/repro/baseline-digests.txt`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md` (the architect's).
**Relates to:** **RFC-0.41-006** / **E-104**, whose budget this makes describable and whose
behaviour it must not change; **RFC-0.42-001**, the other ABI-adding line this milestone; **E-110**,
untouched; **RFC 037**, whose dormant preemption path this line deliberately leaves dormant.

## What is wrong

No syscall returns a clock or a monotone counter, so a service waiting on a device cannot say *"wait
up to N milliseconds"*. It cost 0.41 three times: `fjell-storaged`'s budget is a count of 500,000
`sys_yield` rounds; the failure line had to stop calling that a timeout; and RFC-0.41-006's R7 was
withdrawn because **no service can read `tick`**.

**But the machinery exists and is wired. Nothing turns it on.** `read_mtime()`
(`arch/riscv64/timer.rs:25`), `set_mtimecmp()` (`:38`), `schedule_next_tick()` (`:49`),
`handle_timer()` (`trap/dispatch.rs:134`) and the `TIMER_PREEMPTED` flag (`:237`) are all written.
`schedule_next_tick`'s **only** caller is the timer-interrupt arm — it re-arms a timer nothing arms
first — and **`enable_interrupts()`, the one function that sets `STIE`, has zero callers.** Only
`enable_external_interrupt()` is called (`main.rs:1710`), and `csr.rs:150` says why in its own
words: *"This kernel has never enabled the timer interrupt before — `sie` has been `0` since boot on
every prior milestone."*

So this line is not "build a clock". It is **"read a counter that is already readable, and decide
what user space may see of it."**

## The risk that shapes the design

The same comment warns that turning `STIE` on *"would silently activate RFC 037's dormant
timer-preemption path kernel-wide, on every existing QEMU profile."* This project has just spent two
milestones on timing-sensitive behaviour — E-104's host-latency intermittent, the `svc-fault`
task-slot correlate, the audit ring's interleaving. **Activating preemption would move all of it at
once, and nothing in E-106 requires it.**

**Reading `mtime` does not need `STIE`.** It is a plain read; the interrupt enable is about
*delivery*, not *observation*. That separation is the whole design.

## Requirements

**R1 — one new syscall returning a monotone counter, capability-gated.** It takes the **next free
number, 113**. **`120` must not be reused**: it was `Reboot`, an undispatched duplicate of
`PlatformReboot` (18), retired deliberately (`fjell-abi/src/syscall.rs:102`), and reviving a retired
number would make old and new ABIs disagree silently.

**R2 — the unit is stated in the ABI, not left to the caller to guess.** `mtime` counts at a
platform-defined frequency, so a raw tick is meaningless without it. **Either** return nanoseconds,
**or** return ticks *and* expose the frequency through the same surface — the implementer argues
which, with the reason. **A counter whose unit is undocumented would recreate E-106 at one remove**:
a number that reads like a duration and is not one, which is exactly the defect this line closes.

**R3 — `STIE` stays masked and RFC 037's preemption path stays dormant.** Do **not** call
`enable_interrupts()`, do not set bit 5 of `sie`, and do not wire `schedule_next_tick`. If the
implementation appears to need the timer interrupt to read the counter, **stop** — that would mean
the separation above is wrong and the line's premise with it.

**R4 — capability-gated, with the channel named.** A monotone counter is a timing side channel: it
is what an attacker needs to measure anything. Gate it on a right, grant that right explicitly
rather than through an aggregate (as `LEASE_BIND` was in 0.42-001), and say in the RFC record which
services get it and why.

**R5 — one real consumer, and it must not change `do_io`'s behaviour.** An ABI nothing uses cannot
be shown to work — that is the `store` tier's shape, four markers nothing produces. So
`fjell-storaged` reads the clock across its poll and **reports** the elapsed duration in the
`io incomplete` line. **The budget remains a count of 500,000 rounds and the give-up condition does
not change.** The duration is an added field, not a new timeout. *This is deliberate: `do_io` is the
path E-104 lives on, and this line must not perturb it.*

**R6 — the duration appears on a real boot.** Run the throttled control and quote the
`io incomplete` line showing both the round count and a duration. That makes E-106's claim — *no
service can express a duration* — false by demonstration.

### Non-goals

- **Enabling timer preemption** (R3). It is a scheduling change across every profile and belongs to
  its own RFC with its own risk analysis.
- **Changing `do_io`'s budget, give-up condition, doorbell re-kick, or the two-line report's
  existing fields.** Only an added field.
- **A `sleep`/`wait-until` syscall.** Blocking on time needs the interrupt plane R3 keeps masked.
  Reading a clock and sleeping on one are different features; this is the first.
- **Fixing E-104.** Its behaviour is disclosed (limitations row 19) and unchanged here.

## Risks

- **Two ABI-adding lines in one milestone** (with 0.42-001). Expect `Added` movement and regenerate
  in this line's own commit — **and predict it in the review request**, as 0.42-001 did.
- **A clock makes timing measurable, including by `neg-test`.** If a negative test starts depending
  on wall time, it will be flaky on a loaded host — the lesson of E-104. **Do not add a
  time-dependent assertion to any tier.**
- **R5 touches `storaged`**, so its prebuilt and the baseline move; re-record in the same commit.
- **If the counter reads zero or does not advance**, that is a finding about the platform, not a
  reason to reach for `STIE`. Report it.
