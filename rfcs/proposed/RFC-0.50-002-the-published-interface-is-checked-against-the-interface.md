# RFC-0.50-002: The published interface is checked against the interface — `syscall-doc`, and the check that was named for this and aimed at three source files

**Status:** Proposed — drafted by the implementer for the architect's review; **nothing in this line is implemented**
**Milestone:** 0.50
**Tracks.** Handoff line 2 (`.git-exclude/handoff/handoff-0.50.md`). Builds on RFC-0.49-002 (the ABI declares only syscalls the kernel dispatches) and **RFC-v0.22-001 Slice 1** (the check that first claimed this question).
**Errata:** none filed by this RFC. The cause is the E-014 class (an instrument reporting coverage it does not have); whether it warrants its own entry is the architect's.
**Touches (proposed):** `tools/fjell-consistency-check` (one library module, one gate half, `ALL_SUBCHECKS`, `SUBCHECK_NAMES`, `doc_counts.rs`'s scope note); `docs/src/api/syscalls.md`, `docs/src/external-design/{kernel.md, capability-lease.md, services.md}`.
**Not in this RFC:** a general "verify every number in every document" linter (`doc-counts`' own design note rules it out, and is right to); `capability-lease.md`'s rights list (it declares its own incompleteness; **the lattice has 33 single-bit rights**, re-derived below, if the sentence is touched); any syscall addition or retirement (the surface is 35 / 35 / 0 and stays there).

---

## What is true now

Re-measured at `b5d488d` by the handoff's own commands, run verbatim (the symmetric control included, so an asymmetric result would have meant my parser and not the documents):

```
declared 35 | documented 28
declared, undocumented: ['CapInstall', 'IrqAck', 'IrqBind', 'IrqWait', 'MmioUnmap', 'PlatformReboot', 'TaskKill']
documented, not declared: []
```

| # | Where | What it says | What is true |
|---|---|---|---|
| 1 | `docs/src/api/syscalls.md` header | *"dispatches **26** of them. The remaining 9 … are not dispatched … This page lists only the 26 that are live."* | `syscall-surface`: **35 / 35 / 0**. The table has **28** rows — wrong about its own contents by two — and **seven live syscalls appear nowhere on it**. 28 + 7 = 35 |
| 1b | the same page's stamp, *"as-built catalog at v0.21.3"* | a date | **`ClockNow` (113) is a row and has a return-value note; it arrived at 0.42.** The body was maintained ≥ 20 milestones past the date in its own header, while the header kept saying 26 live. A reader who notices the stamp and discounts the page is misled the other way |
| 2 | `external-design/kernel.md:27` | *"Dispatched groups (as-built at v0.21.2, **26 syscalls**)"* | the table names **27** distinct syscalls (30 code spans; three are wrapper aliases) and omits **8**: `clock_now`, `cap_install`, `task_kill`, `mmio_unmap`, `platform_reboot`, `irq_bind`, `irq_ack`, `irq_wait`. 27 + 8 = 35 |
| 3 | `kernel.md:123` | *"**Interrupt syscalls are not dispatched** … `trap/syscall.rs` has no dispatch arm for any of them (see §2) … `driver-virtio-net` calls `sys_irq_bind`"* | **every clause false.** The arms are at `trap/syscall.rs:57`–`:59`; `driver-virtio-net`'s import line is `{sys_debug_write, sys_debug_writeln, sys_exit, sys_mmio_map}` (it calls no IRQ syscall); `driver-uart` does; `uart-rx` and `uart-rx-unbound` are two of the 27 gated categories over that path. False since 0.25. **It cites §2 of its own file as authority, which 0.49 made read *"none: 35 declared, 35 dispatched"*** |
| 4 | `task/spawn.rs:1393`, `fjell-syscall/src/lib.rs:312` | `MmioUnmap` "declared and never dispatched" | — small item B (RFC-0.50-004); the sweep for the *shape* is reported there |
| 5 | `capability-lease.md:34` | *"Capability operations (syscalls)"* lists seven | omits `cap_install` (dispatched since 0.45) |
| 6 | `services.md:17` | *"The service plane (as-built, **29 programs**)"* over a table | the table has **27** rows; the tree has **31** services + **3** drivers = **34** programs; the kernel embeds **33** prebuilt images. Three figures, no two agreeing |

### Findings beyond the handoff's six (the handoff asks that a seventh be a finding in the RFC, not a quiet fix)

**7. `kernel.md:21`–`:22`, the same file's opening paragraph:** *"`fjell-abi` declares 35 syscall numbers; `trap/syscall.rs` dispatches **26** of them."* It is the same stale count as #2 four lines above the table that repeats it, and it sits outside the section the handoff named. The figure that is right (35) and the figure that is wrong (26) are in one sentence.

**8. The prose counts are the common cause, not the tables.** Both pages state a count in prose (*"26"*, *"the remaining 9"*) and carry a table that is the real catalogue; **the table was kept up to date and the sentence beside it was not**, on two pages, for ≥ 20 milestones. A gate that compares tables to the enum and leaves a prose number free would leave the *kind* of drift that was observed in place. D3 is about this.

**9. `kernel.md`'s table groups by snake-case wrapper name, not by `SyscallNumber` variant, and has no numbers.** `ipc_call` ↔ `IpcCall`, `platform_info_get` ↔ `PlatformInfoGet`; three entries carry an *"(also reached via `ipc_call_words`)"* parenthetical that names a **different wrapper of the same syscall**. So a comparison against the enum needs a name mapping (snake ↔ Pascal) **and** an explicit rule for the three aliases — the handoff's "treat the wrapper aliases explicitly rather than by a count that happens to come out right". D1 gives the rule.

**10. `syscalls.md` row `12 | CapDelete | — (dispatched; no wrapper)`, and the note under the table (*"`fjell-syscall` does not currently expose a `sys_cap_delete` wrapper; a caller would need to issue the raw `ecall`"*), are false: `pub fn sys_cap_delete` exists at `crates/fjell-syscall/src/lib.rs:743` and issues `SyscallNumber::CapDelete`.** Its own doc-comment says *"Until 0.47 this had no wrapper"* — **so the page has said the opposite of the crate since 0.47**, a date-stamped negative ("at v0.21.3") that went false and stayed. Found by checking the claim the D4 wrapper rule would check (the handoff did not list it). It is a defect in the SDK reference, which is the page a service author reads.

**11. `kernel.md:32` names a wrapper that does not exist.** *"`ipc_send` (also reached via `ipc_try_send`)"* — **`sys_ipc_try_send` was the old name**, renamed `sys_ipc_send` at RFC-0.27-002 (closes E-022) precisely because it did not try; no function of that name exists in `fjell-syscall` (`grep -n 'pub fn sys_ipc_try_send'` returns nothing; the name survives only in history prose). So of the three aliases the handoff said to treat explicitly, **one is not an alias of anything**, and the D1 alias rule fails it at `b5d488d`: the gate's first real red is on a claim nobody had reported. (`ipc_call_words` and `ipc_recv_msg` do exist, at `lib.rs:804` and `:100`.)

### The finding that makes this a line (re-derived, and the correction it carries)

`doc_counts.rs:29`–`:31` excludes, in writing, *"every other numeric claim anywhere else in the tree (service counts, **syscall counts already covered by `syscall-surface`**, …)"*. `syscall_surface.rs` reads four paths — `crates/fjell-abi/src/syscall.rs`, `crates/fjell-kernel/src/trap/syscall.rs`, `tests/syscall/expected.toml`, `Cargo.toml` — **and no document**: `grep -n '\.md' tools/fjell-consistency-check/src/syscall_surface.rs` returns nothing. **A check declines a question because a sibling owns it; the sibling does not; the disclaimer is why nobody looked again.**

**And it is better than a deferral, because the sibling was built to own exactly this and aimed at the wrong files.** `ROADMAP.md:194`, 0.22's own scope item 3: *"Mechanical syscall-count check, **to stop documented-surface drift recurring**."* It was built — it is `syscall-surface`, whose handoff (RFC-v0.22-001) says what it does: *"parse the `SyscallNumber` enum … parse the dispatch arms … and compare both against a committed expectations file."* **Three source files and a TOML. A check named for documented-surface drift reads no documented surface.** And that handoff recorded the ground truth it was built against on 2026-07-30 — *"35 declared, 26 dispatched, 9 undispatched — `CapInstall(17)`, `PlatformReboot(18)`, `TaskKill(43)`, `MmioUnmap(91)`, `IrqBind(100)`, `IrqAck(101)`, `IrqWait(102)`, `DmaShare(111)`, `Reboot(120)`"* — **which is, to the number and the name, what `syscalls.md`'s header still says.** The page was true when the check meant to keep it true was written; it has drifted against that check ever since without being read by it. The sharpest turn: 0.49 retired the `undispatched` key in `expected.toml`, so the one committed file that recorded that set no longer does — while the page quoting it still does.

*The wrong version of this story, which I am not telling:* that the check was deferred and never built. It was built, to the letter of its description, and given the wrong inputs.

---

## The decisions this RFC asks for

**D1 — The gate: `syscall-doc`.** A library function `syscall_doc::evaluate` (pure, host-tested, like `syscall_surface`'s) over the enum text, the two tables and the wrapper source, and a thin gate half that reads the four files. Fails on:

* **a row naming no variant** — in either table;
* **a variant no row names** — in *both* tables (so after the correction `kernel.md` must list all 35, the aliases not counted as extra syscalls);
* **a row whose number disagrees with the enum** — `syscalls.md` only (its first column is the number; `kernel.md` has none);
* **an alias that is not a wrapper:** the three *"(also reached via `x`)"* spans are parsed **as aliases, never as syscalls**, and each must name an existing `pub fn sys_<x>` in `crates/fjell-syscall/src/lib.rs`, **and that wrapper must issue the row's own syscall number** (`ipc_call_words` → `IpcCall`; a wrapper that issues a *different* syscall is the failure). That is the explicit rule, and it is stronger than a count: a count that came out right could not tell `ipc_call_words` from `ipc_recv_msg` swapped.
* **duplicates** — the same variant in two rows of one table; the same number twice.
* **the table is not found at all** — a doc edited so the header the parser anchors on disappears is a **FAIL**, not a pass over nothing.

The PASS line prints what it read (rows, distinct syscalls, aliases resolved, the two documents) and says what it does **not** check: the prose around the tables and the *Group* column's grouping.

**D2 — The red run is captured first, and it is the strongest evidence this line produces.** In stage 2, **in this order**: (1) write the gate and its host tests; (2) run it against `b5d488d`'s documents **before any document is touched** and save the output into the RFC's *As built* section — expected, from the measurements above: `syscalls.md` has 7 variants no row names (the seven) and a `CapDelete` row whose wrapper claim is false (finding 10); `kernel.md` has 8 variants no entry names and an alias that is not a wrapper (finding 11); no row names a non-variant (the symmetric control); (3) only then correct the documents; (4) the gate goes green on the corrected set. **Three deliberately broken inputs, one per direction** (a row naming a retired variant, a variant with its row removed, a row with the wrong number), shown red by name as 0.22's governing principle asks; an alias pointing at the wrong wrapper is a fourth.

**D3 — What happens to the numbers in prose.** *Recommended: the two pages state no count at all.* The sentence *"dispatches 26 … the remaining 9"* is replaced by what is true and cannot drift: *"every declared syscall is dispatched, or it is retired (RFC-0.49-002); the table below is kept equal to `SyscallNumber` by `syscall-doc`, and the dispatch state by `syscall-surface`."* The reason is observation 8: the table was maintained and the sentence beside it was not, so the unmaintained thing is the number a person types. *Alternative:* keep the counts and have the gate parse them (`dispatches **N**`, `declares N`); it is fragile (it parses prose) and it keeps a hand-maintained figure, which is the E-125 family. **I recommend removal over checking.**

**D4 — Whether the gate also checks the wrapper column.** `syscalls.md`'s third column names `fjell-syscall` wrappers (`sys_cap_copy`, …). *Recommended: yes, narrowly* — every `sys_*` named in that column must exist as a `pub fn` in `fjell-syscall`, and a row that says *"no wrapper"* (`CapDelete`) must not have one. It is the same parse the alias rule already needs, costs ~15 lines, and closes #10. It is **not** a check that the wrapper is *correct* (which number it issues is the alias rule's job and is applied to the aliases only, because for a primary row the wrapper's name is the canonical one). *Alternative:* leave the column unchecked and say so in the PASS line. **Finding 10 is the argument for yes:** the one row that makes a claim about a wrapper's absence is the one that is false.

**D5 — The two documents the handoff counted as defects and the gate does not read.** `services.md:17` (#6) and `capability-lease.md:34` (#5) are not syscall tables. *Recommended:* correct both by hand now (the service table is reconciled to the tree; the capability list gains `cap_install`), **and** replace `services.md`'s hand-typed program count with the derivation's statement of what it counts, because *three figures, no two agreeing* is the symptom of a number a person maintains. *Whether a small `service-table` check* — every `crates/services/*` and `crates/drivers/*` is a row, no row is a ghost — *belongs in this line* is the architect's call: it is the same shape and the parser exists, and it is exactly the "general linter" the handoff rules out if it grows. **I recommend not building it in 0.50 and saying in `doc_counts.rs`'s corrected note that service counts are *uncovered*** (D6), so the next reader is not told they are somebody else's.

**D6 — `doc_counts.rs`'s scope note, corrected in the same commit.** The sentence *"syscall counts already covered by `syscall-surface`"* becomes true or goes: *"syscall **tables** in the documents are `syscall-doc`'s; the dispatch state is `syscall-surface`'s; service and driver counts are covered by nothing (named in `services.md`, uncovered)."* **Correcting six documents and leaving the sentence that explains why nobody checked them is closing the instance and keeping the cause — E-125's error.** A test in the library pins that the note names `syscall-doc` (the failure the note itself had: a claim about coverage with no instrument behind it).

**D7 — The staleness stamps.** `syscalls.md`'s *"as-built at v0.21.3"* and `kernel.md:27`'s *"as-built at v0.21.2"* are replaced, **not re-dated**: a date a person must remember to move is the thing that lied for twenty milestones. They are replaced by the sentence *"kept equal to `SyscallNumber` by `syscall-doc`"*, which is true of the table and which a gate keeps true.

**D8 — Correcting the paragraph at `kernel.md:123`** (#3) *rewrites it to what is true* — `irq_bind`/`irq_ack`/`irq_wait` are dispatched; `driver-uart` binds, waits and acks in its loop; `uart-rx` and `uart-rx-unbound` exercise the path — **and does not preserve the false clauses with a strike-through**: it is a present-tense claim in a live document. A dated one-line note records that it read otherwise until 0.50 (the project's correction-block convention).

---

## Requirements

**R1** `syscall_doc` library module + gate half + registration in `ALL_SUBCHECKS` and `SUBCHECK_NAMES` (the dispatcher test asserts they agree). **R2** the red run against `b5d488d`'s documents, captured **before** the corrections, in the RFC. **R3** four broken-input tests (three directions + wrong alias) and one for *table not found*. **R4** the document corrections (#1, 1b, 2, 3, 5, 6, 7, 10, 11, and the two stamps), each with the failing sentence quoted in the commit message. **R5** `doc_counts.rs`'s note, with its pin test. **R6** a mutation per rule against the real gate (`>` for the number comparison; alias check removed; the "table not found" branch turned into a pass), each RED by a named failure, backups checked, sha-exact restores.

## Negative cases — what I will show

* **N1.** The gate **fails on `b5d488d`'s documents** (R2) — a gate demonstrated only after the fix has no demonstrated discriminating power.
* **N2.** It **passes on the corrected set** and **fails again when one row is deleted**, both on the real files.
* **N3.** A document that **renames its table header** so the parser finds nothing: FAIL, not PASS (a check that passes over no input is the E-014 shape again).
* **N4.** **`kernel.md`'s aliases do not inflate the count:** 30 code spans, 27 distinct canonical names + 3 aliases, and after the correction 35 + 3.
* **N5.** The corrected pages **contain no hand-typed syscall count** (a grep, with a control that finds the old sentence at `b5d488d`).

## What this RFC does not do, and why

Does not check prose beyond what D3/D7 remove; does not build the service-table check (D5) unless ruled; does not touch `v1-limitations.md` (the architect's, and it already carries the 0.49 correction block), `standards-mapping.md` row CRA-I-2j (*"`syscall-surface` … mechanically holds declared, dispatched, and committed-expectation syscall sets equal"* — **true as written, and now incomplete**; the row is the architect's to extend to `syscall-doc`), or the history documents (`docs/src/history/`, `docs/src/audit/`, `docs/src/adr/0009…`), whose past tense is legitimate.

## Reproduce

```
cargo run -q -p fjell-consistency-check -- syscall-surface
python3 -I -c "…the handoff's pipeline, verbatim…"        # declared 35 | documented 28, the seven, and the empty reverse
/usr/bin/grep -n 'already covered by' tools/fjell-consistency-check/src/doc_counts.rs
/usr/bin/grep -n '\.md' tools/fjell-consistency-check/src/syscall_surface.rs                 # nothing
sed -n '57,59p' crates/fjell-kernel/src/trap/syscall.rs ; sed -n '39p' crates/drivers/fjell-driver-virtio-net/src/main.rs
/usr/bin/grep -n 'ALL_NON_META\|pub const [A-Z_]*: CapRights' crates/fjell-abi/src/*.rs | head      # the 33-bit figure: count single-bit consts
```

---

## As built (stage 2) — what the implementation changed in this text, and why

* **Gate built first, run red against the uncorrected documents, then the documents corrected** (D2's order, which the architect called the whole evidential value of the line). At `9b5a662^` the two documents are byte-identical to `b5d488d`'s (`git diff b5d488d HEAD -- <both>` empty). `syscall-doc` against them: **FAIL, 17 problems — exactly the predicted set and nothing else:** `syscalls.md` — the **seven** variants no row names (`CapInstall`, `PlatformReboot`, `TaskKill`, `MmioUnmap`, `IrqBind`, `IrqAck`, `IrqWait`) and `CapDelete`'s *"no wrapper"* with `pub fn sys_cap_delete` present; `kernel.md` — the **eight** (`cap_install`, `platform_reboot`, `task_kill`, `mmio_unmap`, `irq_bind`, `irq_ack`, `irq_wait`, `clock_now`) and `IpcSend`'s alias `sys_ipc_try_send`, **no `pub fn` of that name** (finding 11). **No row named a non-variant** — the symmetric control, so the parse is the parser's and not the documents'. The captured output, verbatim (the same red is the host test `the_real_documents_agree_with_the_enum` before the corrections):

```
--- consistency-check: syscall-doc ---
syscall-doc: FAIL
  docs/src/api/syscalls.md: row `CapDelete` says it has no wrapper, and `pub fn sys_cap_delete` exists in crates/fjell-syscall/src/lib.rs
  docs/src/api/syscalls.md: `CapInstall` (17) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `PlatformReboot` (18) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `TaskKill` (43) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `MmioUnmap` (91) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `IrqBind` (100) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `IrqAck` (101) is declared in the ABI and no row names it
  docs/src/api/syscalls.md: `IrqWait` (102) is declared in the ABI and no row names it
  docs/src/external-design/kernel.md: `IpcSend` lists `sys_ipc_try_send` as an alias, and no `pub fn sys_ipc_try_send` exists in crates/fjell-syscall/src/lib.rs (an alias that is not a wrapper)
  docs/src/external-design/kernel.md: `cap_install` (`CapInstall`, 17) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `platform_reboot` (`PlatformReboot`, 18) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `task_kill` (`TaskKill`, 43) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `mmio_unmap` (`MmioUnmap`, 91) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `irq_bind` (`IrqBind`, 100) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `irq_ack` (`IrqAck`, 101) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `irq_wait` (`IrqWait`, 102) is declared in the ABI and no group-table entry names it
  docs/src/external-design/kernel.md: `clock_now` (`ClockNow`, 113) is declared in the ABI and no group-table entry names it
```

* **D1 as built, with one rule the RFC did not foresee:** an alias must *reach* its syscall — `SyscallNumber::<Name>` in its body, **or the literal `li a7, <number>`, or a call through another `sys_*` wrapper** (depth ≤ 4). `sys_ipc_recv_msg` issues the call with `li a7, 21`, not `SyscallNumber::IpcRecv`; a rule that required the enum path would have failed a correct alias. Six aliases resolve (2 in `kernel.md`, 4 in `syscalls.md`, the `debug_write` family resolving through `sys_debug_write_byte`).
* **D4 built** (the architect approved D1–D8 as recommended): the wrapper column is checked narrowly — 35 wrapper names, each must be a `pub fn`; a row that says *"no wrapper"* must not have one. `PlatformReboot`'s wrapper is `sys_reboot` (the name differs from the variant), so the new row says so.
* **D3 and D7 as ruled:** neither page states a count and the staleness stamps are replaced, not re-dated. **Absence probe, with its control:** the pattern (`dispatches **N** | declares N | N syscalls | remaining N | only the N`) finds **6 hits at `b5d488d`** and **one at the tip — my own quotation of the retired sentence** in `syscalls.md`'s header (*"dispatches 26 of them … the remaining 9"*, inside the sentence explaining why it was removed).
* **D5:** `capability-lease.md` gains `cap_install`. **`services.md` — the handoff's three figures were each right about something different, and the real defect is that the *table* was five programs short of the tree:** the heading said 29 and the table held 29 programs (27 rows, one of them a row of three); the tree has 34 (31 services + 3 drivers); 33 prebuilt images (everything but `summaryd`, which has none). Missing from the table: `driver-uart`, `proxy-braille`, `summaryd`, `svc-presentation-fault`, `svc-presentation-liar`. All five added; the hand-typed count is replaced by the two commands that derive it, **run verbatim at the tip** (`ls -d crates/services/*/ crates/drivers/*/ | wc -l` → 34; `git ls-files 'crates/fjell-kernel/prebuilt/*.bin' | wc -l` → 33), and the page says plainly that nothing checks the table. **No service-table subcheck built** (D5's recommendation, and it is the architect's call).
* **D6 as built:** `doc_counts.rs`'s scope note rewritten (it now names `syscall-doc`, `syscall-surface` and what **nothing** covers) and a pin test fails if the false sentence returns; **shown red** by putting it back. **D8:** `kernel.md` §6's interrupt paragraph rewritten to what is true, with a dated correction note stating all three clauses it got wrong.
* **Mutations (R6), each RED by a named test, restored sha-exact:** SD1 number comparison off (`a_row_with_the_wrong_number_fails`); SD2 alias-exists check off and SD3 alias-reaches check always true (`an_alias_that_is_not_a_wrapper_or_reaches_another_syscall_fails`); SD4 *rows not found* turned into a pass (`a_table_that_cannot_be_found_is_a_failure_not_a_pass`); SD5 variant-no-row check off (`a_variant_no_row_names_fails_in_both_tables`); DC1 the false sentence restored in `doc_counts.rs`.
* **Findings beyond the RFC's eleven:** none new in the documents; **`syscalls.md` row 18 and the `PlatformReboot` wrapper name** (above) is the only place the table and the code disagree on a *name* rather than a presence. The `standards-mapping.md` row CRA-I-2j (*"`syscall-surface` … holds declared, dispatched, and committed-expectation syscall sets equal"*) is **true as written and now incomplete**; it is the architect's to extend to `syscall-doc`.
