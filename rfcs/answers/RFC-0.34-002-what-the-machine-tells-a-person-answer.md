# RFC-0.34-002 R1 and §A–§E: the eight bytes were not what E-062 said

**Governing RFC:** [../accepted/RFC-0.34-002-what-the-machine-tells-a-person.md](../accepted/RFC-0.34-002-what-the-machine-tells-a-person.md)
**Handoff:** [../handoffs/RFC-0.34-002-what-the-machine-tells-a-person/implementation-handoff.md](../handoffs/RFC-0.34-002-what-the-machine-tells-a-person/implementation-handoff.md)

Written after R1 and §A's measurement and before any change to the tree. Absences
probed with `/usr/bin/grep -a` and a control. Tip at R1: `0fc44ae`.

**The first thing to say, because it changes what this line can honestly close:**
**E-062's mechanism is not what produced its eight bytes.** The measurement §A asks
for shows the mechanism *as filed* — a dead task's bytes emitted in front of the next
task's line — **has never occurred in any tier**, cannot occur in today's kernel, and
the eight bytes are a live service writing its own diagnostic probes. §E has the
attribution; the consequence for D6, R7 and the prohibitions is at the end and is an
**escalation**, not a decision I take in code.

---

## R1 — what the tree says

### E-061 — thirteen sites, three conditions

`spawn.rs` returns `SysError::NoMemory` at **13** sites (41, 48, 62, 79, 100, 126,
143, 148, 163, 189, 198, 204, 220), confirmed by `grep -a -c` on the file. Read one by
one they are **three** conditions:

| Condition | Sites |
|---|---|
| **The task table has no free slot** — `next_free_index()` is `None`, or `table.insert` returns `TaskError::TableFull` | **41, 220** |
| **No physical frame** — root page table, user text ×2, user stack, kernel stack | 48, 126, 148, 189, 204 |
| **A `map_page` failed** — the page-table frames a mapping needs, or a mapping error; `map_err(\|_\| …)` **discards which** | 62, 79, 100, 143, 163, 198 |

`init`'s only report is `init: spawn error` (`fjell-init/src/main.rs:469`), which
discards the error and names neither the image nor the limit.

### E-062 — the buffer

`DBG_TASKS = MAX_TASKS` (**40**), `DBG_LINE = 160`; flush points: **a newline** and
**a full line** (`n + 1 == DBG_LINE`) — **nowhere on task exit or fault**. The index is
`current_task_idx() % DBG_TASKS`, and `current_task_idx()` is
`sched.current().map(..).unwrap_or(0)`: so an out-of-range index **and a missing
current task** both alias to a slot — the second is a case the RFC did not name.
**A dead task's slot is never handed on**: `TaskTable::remove` has **no caller**
(`grep -a -rn '\.remove('` over the kernel: 0 hits against the table; control: the
method's own definition), so `insert` only ever fills slots that were never used.

### E-062 — the eight bytes in a fresh run, and what wrote them

Fresh run, `semantic`: `serial.log` line 58 reads `90 90 90 90 90 90 90 92` then
`M6: storaged ready`. **Instrumented kernel** (a scratch worktree, discarded; probes
on every `sys_debug_write` of a byte ≥ 0x80, on every task spawn — index → image —
and on every task exit or fault, printing what the slot held):

- The bytes are written **one `ecall` at a time by task index 13**, interleaved in
  time with the braille presentation's own output (index 9, image 31, `e2 a0 xx`
  triples). **Index 13 is image 10, `STORAGED`.**
- They are written by **`fjell-storaged` itself, on purpose**:
  `crates/services/fjell-storaged/src/main.rs:276`,
  `sys_debug_write_byte(0x90 + (devid as u8 & 0xF)); // devid (0x90-0x9F)` — one byte
  per virtio-mmio slot it scans. QEMU `virt` has eight; seven are empty (`devid` 0 →
  `0x90`) and one is the block device (`devid` 2 → `0x92`). **Exactly `90×7, 92`.**
- Two more probes of the same kind sit in its service loop:
  `:352` (`0xB0 + lba`, "begin probe") and `:376` (`0xC0 + lba`, "lba probe"). Each
  appends a byte to storaged's *own* line buffer with no newline, so they surface at
  the next flush — as junk mid-line, or split off at 160 — in any tier that writes to
  the disk (the `store` and `upgrade` tiers, which are not release-gated).

### E-063 — two writers, and no specification

`M6: storaged ready` is written by `fjell-storaged/src/main.rs:334` (after
`send_ready()`) and `fjell-init/src/main.rs:573` (after `wait_relay_exact(READY)`).
**No specification asserts it**: of **45** files (`tests/qemu/profiles/*.toml` and
`tests/qemu/artifacts/*/expected-markers.txt`), **0** contain it; the control
`driver-uart: ready` is found in **12** (7 profiles, 5 artefact copies). **A third
figure is wrong, and I say so the way the handoff asks:** the RFC and the register say
the control is found in *four*; at this tip it is found in twelve. The absence is
unchanged and its control is stronger; the number was off. The only other places the
line appears are archived evidence logs (`tests/evidence/…`, 7 files), which are
recordings, and one paragraph of `v1-limitations.md`.

---

## §A — flush or clear? **The measurement says: it never mattered yet. Flush, with the cut marked.**

Across **24 tier logs** (every profile that commits its artefacts; run under the
instrumented kernel) there were **277 task departures — 250 exits and 27 faults — and
the task's line buffer was empty at every one.** Not one task, in any tier, has left
holding a partial line. So (i) the mechanism the register describes has never
occurred, and (ii) the choice between flush and clear costs nothing today — but it is
a real decision for the first task that does die mid-line, and there the lean is right:
a dying task's last words are what a person debugging a fault wants, so **flush, and
mark the cut** (D3's mechanism serves both). The lifetime defect is **latent and proven
by construction**: no path flushes on exit, so such a task's partial line is *lost
silently* (never emitted) today, and would be *handed to the next occupant of the slot*
the day `TaskTable::remove` gains a caller.

## §B — the bound. **Keep 160; mark the split; state it.**

The longest console line the committed logs carry is **153** bytes
(`verifyd: WARNING unprovisioned dev trust anchor …`); the longest braille line is
**135** (1,344 braille lines measured) — both under 160, which is why nothing has been
split. Sizing the buffer from `MAX_WIRE_BYTES` (4,624 × 40 = **185 KB** of kernel
`.bss`) buys nothing any tier produces and costs more than raising `MAX_TASKS` did
(34.6 KB). **Bound: 160, unchanged.** A longer line is split at 160 and *says so*: the
chunk ends `[cont]` and the next begins `[cont]`; a line cut by its writer's exit ends
`[cut]`. The bound goes in `v1-limitations.md` for the reader of a braille
presentation through the console, not only in a comment.

## §C — whose line goes? **`init`'s.**

`storaged`'s is the event; `init`'s marks its own wait returning. Checked for
dependents, with the control from R1 (the same searches find `driver-uart: ready` in
12 specifications, so the absence is a real absence): no profile, artefact
specification, document or procedure asserts it; the seven evidence logs record output
and do not assert a count. **The check found nothing that depends on two.**

## §D — a new `SysError` variant? **Yes, one: `TaskTableFull = -35`.**

Named for the condition, not the table; next in the resource block (`NoMemory = -30 …
NotSupported = -34`). **Effect on Gate 4:** since RFC-0.33-004 D5 the snapshot's enum
hash covers variants, so `--verify` reports `SysError` changed — the first ABI
addition caught by body; I run it *before* regenerating and paste it. **`syscall-surface`
(34/30/4) counts `SyscallNumber` variants, not `SysError`, so it is unchanged** —
stated because the RFC asks. Only sites **41 and 220** return it; the frame and mapping
conditions keep `NoMemory` (they are distinguishable from the table being full, and
splitting *those* is not what D1 asks).

## §E — is the eight-byte prefix explained by §A's measurement? **No — and it is explained by something else.**

Attribution, as the handoff requires: **the task whose buffer held them is
`storaged` (index 13, image 10), while it was alive**, and it put them there itself
(`main.rs:276`). They were not a dead task's leftovers: at every departure in every
tier the buffers were empty, and no slot is reused. **The mechanism E-062 describes is
proven defective by construction (nothing flushes on exit) but is not the cause of the
bytes.** I will not close E-062 as if it were.

---

## What this does to D6, R7 and the prohibitions — an escalation

1. **D6's first demonstration cannot be run as filed.** *"A task exiting mid-line and
   its bytes appearing on the next task's first line"* needs slot reuse, which does
   not exist. What *can* be shown is the defect's real present effect: **a task that
   exits mid-line has its words lost silently**; after the fix they are shown with the
   cut marked. To show it a task must die mid-line, so I add a scenario to a
   **test-only** service (`svc-fault`, which already exists to fault on the `F`
   trigger): it writes a partial line (and one over-long line for D6's second) before
   it faults. That changes what a *test* service prints; I flag it because the
   handoff prohibits changing what any service prints beyond D5's duplicate.
2. **R7's "the console is clean where it carried the prefix" cannot be asserted while
   `storaged` writes those bytes.** Making the console clean means removing (or
   making printable) three diagnostic-probe writes in `fjell-storaged` — which is
   changing what a service prints, and the RFC prohibits it. **That is a design
   conflict, so I do not resolve it in code.** The proposal, for a ruling: delete the
   three probe writes (they are debug residue — `// begin probe`, `// lba probe`),
   then assert cleanliness with a harness check that the serial log contains no
   control byte other than `\n`, `\r` and `\t`. Until ruled, R7 is met for what it *can*
   assert — a dying task's words, an over-long line's marked split, the marker once —
   and **E-062 closes with the prefix bytes named as a survivor: attributed, unfixed.**
3. **The RFC's premise about the register is worth correcting there:** E-062's
   "Tree, observed" paragraph attributes the bytes to the buffer's lifetime. The
   closure will say what was measured, dated, and leave the original text in place.

## Order

R1/§A–§E (this) → **R3** (the variant, `init`'s message, `--verify` before/after — its
own commit, it is an ABI change) → **R4** (`debug_leave` on exit and fault with the
marked cut; the marked split; the index fails instead of aliasing; the safety argument
re-made in the code) → **R5** (`init`'s duplicate removed; a harness `once_markers`
assertion so the marker's uniqueness is *checked*, not remembered) → **R6/R7** (the
`svc-fault` scenario and the tier assertions; the full-table demonstration in a scratch
tree) → **R8** → evidence.

## Decisions the RFC did not specify

1. The split/cut markers: `[cont]` / `[cut]`, ASCII, so a reader and a `grep` both see
   them.
2. `TaskTableFull` for sites 41 and 220 only.
3. A harness key `once_markers` (each must appear exactly once) — the generic form of
   D5's check, beside `per_boot_markers` (RFC-0.33-001 D23).
4. The test-service scenario (escalated above).
