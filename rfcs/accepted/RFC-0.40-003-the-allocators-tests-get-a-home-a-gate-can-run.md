# RFC-0.40-003: The allocator's tests get a home a gate can run

**Status:** Accepted (0.40) — accepted 2026-10-29
**Milestone:** 0.40
**Tracks.** **E-100** — the frame allocator's unit tests are `#[cfg(test)]` inside
`fjell-kernel`, a `no_std` binary that cannot be host-built, so no gate runs them.
**Touches** *(indicative)*: a new crate, `crates/fjell-kernel/src/mm/`,
`crates/fjell-kernel/Cargo.toml`, `Cargo.toml`, `Cargo.lock`,
`.github/workflows/ci.yml` *(possibly not — see R4)*, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **E-097**, whose thirty-five-milestone life is this line's entire
argument; **E-027** and the instrument audit's `UNAUDITED` row, the same family one level
up.

## What is wrong, and why it is worth a line

The tests exist and **nothing can build them**. They are `#[cfg(test)]` in a `no_std`
binary crate, and they are **already written against `std`** — ten uses of `Box::leak` and
`vec![]` in `mm/frame_alloc.rs`. So they are not merely ungated: there is no target that
compiles them, which is why the implementer of RFC-0.39-003 had to run them in a scratch
harness crate to use them at all.

**The argument is E-097.** A frame-allocator reservation defect — a 4 MiB kernel stack half
outside the reserved range — lived for **thirty-five milestones**, corrupting a live
service's text intermittently, and was found only because a tier went red for reasons
nobody could explain. The code with the weakest automated coverage in this tree turned out
to be the code that held that defect, and the tests that would speak to it had never
executed. RFC-0.39-003 added an owner map and a boot invariant; this line makes the tests
that exercise them real.

This project's standard is *a gate that cannot run is not a passing gate*. Tests no gate
runs are the same shape one level down: coverage that exists as source and never as
evidence.

## The extraction is clean, and the architect measured it

| | |
|---|---|
| `mm/frame_alloc.rs` | 309 lines, **zero** `crate::` references |
| Its imports | `super::address::PhysFrame`, `super::error::MmError`, `fjell_abi::task::TaskId` |
| `mm/address.rs` | 80 lines, no `crate::` references |
| `mm/error.rs` | 26 lines, no `crate::` references |
| Kernel files using `PhysFrame`/`MmError` | 10 |

So the allocator plus the two types it needs is **415 lines with no kernel coupling**, and
`TaskId` already comes from a published crate. A `no_std` crate holding those three files,
re-exported by the kernel's `mm` module, should leave the other ten files compiling
unchanged. **Confirm that rather than taking it from me** (R1).

## Requirements

**R1 — Re-derive the coupling before moving anything.** That the three files reference no
`crate::` path; that `frame_alloc.rs` needs exactly `PhysFrame`, `MmError` and `TaskId`;
and which kernel files use the two types. **If any of the three reaches back into the
kernel, stop and escalate** — the shape of this line depends on them not doing so.

**R2 — The allocator moves to a crate the host can build and test.** A `no_std` library
crate, with the tests travelling with it, so `cargo test` reaches them.

**State what you moved and why.** If `PhysFrame` and `MmError` move too, the kernel
re-exports them from `mm` so the other ten files are untouched; if you judge a narrower cut
better, say which and what it costs. **`forbid(unsafe_code)` on the new crate if the
allocator's logic allows it** — and if it does not, say which function needs `unsafe` and
why, because a bitmap allocator that needs none is worth having on the record.

**R3 — The tests run, and the count is stated.** They already pass by hand: **6**, per
RFC-0.39-003's scratch harness, including `a_dma_frame_is_not_labelled_a_stack`. After this
line the number a gate runs must be stated, and it must be at least those six.

**R4 — A gate runs them, and say which.** `cargo test` on the workspace is the obvious
answer and may need nothing new: check whether the existing host-test tier picks the crate
up automatically, as `ci-negative-matrix` picks up a new profile. **If it does, say so and
add nothing**; if it does not, wire the smallest thing that works and name it.

**Report the tier's test count before and after.** A gate whose number does not move did
not pick the crate up.

**R5 — Add the test E-097 would have failed.** The six existing tests do not cover the
defect that motivated this line. Add one that asserts the property RFC-0.39-003's boot
invariant enforces: **a reserved range contains no free frame** — in particular that a
range reserved across two calls, or reserved after allocation has begun, leaves nothing
free inside it.

This is the requirement that makes the line worth more than a build-system change. A
host-runnable allocator with tests that would still miss E-097 is coverage theatre.
**Demonstrate it failing** against a deliberately mis-reserved allocator in the test
itself, which is cheap now that the allocator is host-constructible.

**R6 — E-100 closes in this line's commit**, with `v1-limitations.md` row 16 removed or
rewritten — **and nothing else may claim these tests as coverage** until it is true, which
after this line it is.

**R7 — Gates by exit status.** `release-rehearsal`, `consistency-check --all`,
`readiness-check`, `abi-snapshot --verify`, `fjell-repro-check`, `test-all` to completion,
`callsite-audit`, `cargo fmt --check`.

**Watch three in particular.** `unsafe-audit`'s count may move if the files move — state
the new figure and update `README.md`'s two places. `abi-snapshot` should not move unless
the new crate is published, which it should not be. And the kernel's prebuilts should
**not** move at all if the extraction is a pure re-home: if they do, something changed
beyond moving code, and that is a finding worth reporting rather than absorbing into the
baseline.

### Non-goals

- **Changing the allocator's behaviour.** This is a re-home plus tests. If the tests find a
  defect, **file it and escalate** — do not fix it in the line that moved the code, or
  nobody will be able to tell which change did what.
- **Extracting any other kernel module.** `page_table`, `vspace` and the rest are adjacent
  and out of scope.
- **E-093**, reclaiming task memory, and **E-091**. Unrelated.
- **Publishing the new crate.** It is internal; `publish = false`.

## Risks

- **A re-home that quietly changes behaviour.** R7's prebuilt check is the detector: a pure
  move should leave the binaries byte-identical. If they move, find out why before
  committing a baseline.
- **Six tests passing and E-097 still uncovered.** R5 exists for this. The point of the line
  is not that the tests run; it is that the tests that run would have caught something.
- **Scope drift into the memory manager.** The three files are self-contained. The next
  three are not.
- **`unsafe` migrating with the code without being noticed.** The audit counts sites, not
  crates; if the figure moves, say why.
