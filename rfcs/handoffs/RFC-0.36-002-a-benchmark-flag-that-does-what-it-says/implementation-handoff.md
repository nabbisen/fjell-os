# Developer Handoff — RFC-0.36-002

**Governing RFC:** [RFC-0.36-002](../../accepted/RFC-0.36-002-a-benchmark-flag-that-does-what-it-says.md)
**Milestone:** 0.36
**Status:** inherited from the governing RFC (Accepted, 0.36)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does `cargo xtask bench --basline` tell you it does not understand you, instead
of succeeding and recording nothing?** That, and `--help` answering without running
a benchmark. Small line, one new wrinkle.

## 0.1 The one thing that is not a repeat of 0.35

Three tools got this fix already (`fjell-repro-check`, `fjell-unsafe-audit`,
`fjell-mmio-audit`) and all three are **standalone binaries** whose `main` returns
the `ExitCode`. **`cmd_bench` returns its `ExitCode` to a dispatcher.** So
returning `ExitCode::from(2)` proves nothing by itself — **run
`cargo xtask bench --nonsense` and read `$?` at the shell.** If `xtask` flattens it
to 1, say so plainly; that is a finding about the dispatcher, not a reason to
pretend the contract holds. Check `$?` directly, never through a pipe.

## 1. Order

1. **R1** — re-derive the flag reads and the default arm, with a control.
2. **R4 first half** — the transcript of a typo'd flag running the benches and
   exiting success. Take it now; after the fix it is gone.
3. **R2/R3** — reject unknown with usage, exit **2**; `--help` exits 0; neither
   benchmarks. Verify the exit code **at the shell**.
4. **R4 second half** — the same flag refusing.
5. **R5** — `--help`'s real output, pasted, every flag it names confirmed to exist.
6. **R6** — E-077 closed, limitations in the same commit.
7. **R7** — the gates, as the required set.

## 2. Traps specific to this line

1. **`1` is taken.** It means a metric exceeded tolerance and a failed baseline
   write. A usage error reported as `1` tells a caller the benchmark regressed.
2. **Run every command you write into a document**, `--help` included. The last
   line in this family shipped a `--help` documenting `--artefacts <dir>`, which
   had never been implemented.
3. **Do not widen.** If the same `.any()` shape exists in other `xtask`
   subcommands, **report it**; RFC-0.35-003's sweep judged the remaining sites safe
   because the toggle only narrows an already-chosen action, and that judgement
   stands until something disproves it.
4. **No guest code changes here**, so no prebuilt should move. If one does, find
   out why before re-recording anything.

## 3. What to put in the review request

- R1's re-derivation and control; any fourth flag.
- The typo'd flag running benches and exiting success, then refusing.
- **`$?` from a real shell run of `cargo xtask bench --nonsense`**, and whether the
  dispatcher preserved 2.
- `--help`'s pasted output, with every named flag confirmed.
- E-077 closed; limitations in the same commit.
- Each gate by its own exit status, as the required set.

Hand back one path: your review request. Do not tag anything.
