# Review Record — RFC-v0.23-001 Slices 1–4, and the v0.7-sync blocker

**Reviewer:** architect
**Reviewing:** `review-request-rfc-v0.23-001-slices-1-4.md`
**Date:** 2026-07-31

## Outcome

**Approved, with one missing deliverable to add before committing (§4).**

The blocker is dispositioned as **neither of the two options offered** — see §3.
It is not a cost of this RFC and it is not an exception to grant. It is a
pre-existing gate-integrity defect that this RFC's latency exposed, and it gets
its own RFC.

Stopping at the boundary of `dispatch.rs` rather than reaching into it was
correct, and the scope discipline throughout this submission is the best in the
project's recent history.

## 1. Verified independently

| Claim | Result |
|---|---|
| Slice 1 committed (`d67cbfc`), Slices 2–4 uncommitted | **Confirmed** |
| `init` no longer depends on `fjell-proxy-text` | **Confirmed** — removed from `Cargo.toml`. The violation the RFC exists to remove is gone |
| `semantic.toml` exists with 4 markers incl. the refusal | **Confirmed** — `action accepted` **and** `action DENIED (capability not held)` |
| Hardcoded task indices in `dispatch.rs` | **Confirmed** — `exited_ok(10/14/19/21/1)` |
| v0.7-sync now FAILs | **Confirmed** — artifact flipped `PASS` → `FAIL` |
| Scope held | **Confirmed** — see §2 |

## 2. Scope compliance — checked, and clean

I checked the two files that looked like they might sit outside the handoff's
in-scope list. Both are legitimate:

- `crates/fjell-tools/src/test_all.rs` — **one line**, `"semantic",`, registering
  the new profile. Exactly the "profile plumbing needed to run them" the handoff
  allows.
- `crates/fjell-service-api/src/lib.rs` — 81 lines defining the chunked-transfer
  protocol, **with the documented divergence written into the code comment**:
  that this is not `ipc.md`'s capability-granted shared region, because that is
  `DmaShare` (111), an undispatched syscall. That is what I asked for, in the
  place a future implementer will actually encounter it.

No kernel change beyond the authorised `ep_obj` arms. The nine undispatched
syscalls untouched. The frozen catalog untouched.

## 3. The blocker — dispositioned

### 3.1 What it actually is

`dispatch.rs` emits the milestone markers **from the kernel, keyed on hardcoded
task-table indices**, with the spawn order recorded in a *comment*:

```rust
// Spawn order: devmgr(10) upgraded(14) syncd(19) netd(21)
if exited_ok(19) { kprintln!("TEST:V0.7-SYNC:PASS"); }
```

So `TEST:V0.7-SYNC:PASS` does not mean *"syncd succeeded."* It means *"whatever
task occupies index 19 exited cleanly."* Those coincided until this RFC changed
boot timing.

**This covers the entire smoke suite.** All four smoke profiles key on markers
emitted this way (`crates/fjell-tools/src/smoke.rs`):

| Profile | Marker | Emitted on |
|---|---|---|
| `m8` | `TEST:M8:PASS` | `exited_ok(14)` |
| `v0.4-net` | `TEST:V0.4-NET:PASS` | `exited_ok(21)` |
| `v0.5-platform` | `TEST:V0.5-PLATFORM:PASS` | `exited_ok(10)` |
| `v0.7-sync` | `TEST:V0.7-SYNC:PASS` | `exited_ok(19)` |

None of them verifies that the service it names succeeded.

### 3.2 Your finding 8 is the same defect, and proves the point

You reported, as an unrelated aside, that init's M8 section passes endpoint IDs
where CSpace slots are expected, so the M8 narration never runs — *"masked only
because `TEST:M8:PASS` is emitted independently by the kernel based on a
different task's exit."*

That is not an aside. **It is this same defect, already having caused harm.** The
`m8` profile has been green while the M8 section it claims to exercise never
ran. The marker attested a different task's exit and nobody could tell.

So this is not a latent fragility that might one day mislead. It has already
misled, in the profile named after the project's flagship milestone.

### 3.3 Why neither offered option is right

**Not option (a) — "accept as a known cost of this RFC."** That would put a false
statement in the record. This RFC did not break v0.7-sync; it revealed that the
marker was resting on a coincidence. Recording a pre-existing defect as a cost of
the ABDD line would misattribute it permanently.

**Not option (b) — a narrow exception to touch `dispatch.rs` here.** The fix is
probably small, but burying a whole-smoke-suite gate-integrity finding inside an
unrelated feature line would make both unreviewable — and this deserves its own
evidence, not a paragraph in someone else's release record.

### 3.4 Ruling: RFC-v0.23-002

**Milestone markers by identity, not index.** Its own RFC, its own slices, its
own evidence. Required before `0.23.0` is cut, since release-cycle exit
criterion 5 needs the QEMU tiers green.

Scope, provisionally — I will write it properly:

- Markers keyed on service identity (`ImageId`) rather than table index.
- A marker must attest the thing it names. `TEST:M8:PASS` should mean the M8
  path completed, not that task 14 exited.
- Finding 8's slot/endpoint-ID confusion in init's M8 waits, since it is the
  same defect's first victim.

**The v0.22 governing principle applies**: every marker fixed must be
demonstrated *failing* when the named service fails, and demonstrated **not**
firing when a different task happens to occupy that slot. Otherwise we will have
replaced one unverified check with another.

### 3.5 Sequencing

1. Add the missing `ipc.md` note (§4), then **commit Slices 2–4**. They work; the
   regression is not theirs.
2. RFC-v0.23-002 fixes marker identity.
3. Cut `0.23.0` with both.

`test-all` stays red between (1) and (2). That is honest mid-line state — the
release cycle gates the tag, not every commit.

## 4. Required before committing Slices 2–4

**The `ipc.md` note is missing.** Review record §3.2 and handoff §3 both required
a note in `docs/src/external-design/ipc.md` recording that the documented
shared-region bulk-transfer path is unavailable because `DmaShare` is
undispatched.

You documented it in `fjell-service-api`, which is good and was not instead-of —
but a reader of the *architecture* looks at `ipc.md`, and `ipc.md:76` currently
still states bulk transfer "uses a shared region granted by capability" with no
indication that this is not currently true. That is a doc asserting behaviour the
tree does not have — the exact thing RFC-v0.21.3-002 exit criterion 8 forbids.

One paragraph. Then commit.

## 5. On the eight findings

Every one was caught by **booting QEMU and reading the serial log**, not by
static review. Three deserve specific note:

- **Finding 3 — services cannot write to *any* static.** `spawn.rs` maps a
  service's whole image `R | X | U` with no `W`. The handoff's design decision #4
  covered `static mut`; you found it applies to `static UnsafeCell` too, and that
  nothing had ever exercised it enough to notice. That is a real constraint on
  every future service author and belongs in the SDK docs. **Carried as a v0.23
  candidate.**
- **Finding 4 — the deadlock.** Replying only after calling back into the peer
  that is blocked awaiting your reply. Only live testing finds that.
- **Finding 2 — 470 KiB of dead BSS made reachable.** A good illustration that
  "nothing called it" was load-bearing for more than documentation.

Finding 7 (the shared TOML parser closing an array on a `]` inside a string) is
correctly left alone and worked around with bracket-free marker text. It is a
real defect in shared tooling. **Carried as a v0.23 candidate** — it silently
loaded 2 of 4 markers, which is a fail-open in the harness itself.

## 6. Next

1. Add the `ipc.md` paragraph; commit Slices 2–4.
2. I will write RFC-v0.23-002 and its handoff.
3. Then `0.23.0`.

Report the chunk round-trip count per node in the Slices 2–4 commit message —
you measured ~300 for one emission plus one forward, and that number should be
recoverable later without re-deriving it.
