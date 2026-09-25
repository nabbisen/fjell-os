# RFC-0.34-002: What the machine tells a person

**Status:** Proposed
**Milestone:** 0.34
**Tracks.** **E-061** (a failed spawn cannot name the limit it hit), **E-062** (the
console line buffer emits a dead task's bytes in front of a live task's line, and
splits long lines silently), **E-063** (one marker, two writers). They are one line
because they are one surface: **the console is the only thing this system says to a
person**, and each of the three makes it say something a reader cannot rely on.
**Touches** *(indicative)*: `crates/fjell-kernel/src/trap/syscall.rs`,
`crates/fjell-kernel/src/task/spawn.rs`, `crates/fjell-abi/src/error.rs`,
`crates/services/fjell-init/src/main.rs`,
`crates/services/fjell-storaged/src/main.rs`, `tests/qemu/profiles/`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** E-014 (instruments deciding by fixed-string match — E-063 is its
class, waiting); RFC-0.34-001 (whose braille presentation writes the longest lines
the console carries); RFC-0.33-004 D5 (the ABI hash now sees an enum's variants,
which is what makes D3's new error value visible to Gate 4).

## Summary

Re-derived 2026-09-25, at `728d79c`. **Two figures in the register were wrong and
are corrected there, dated**; this RFC uses the corrected ones.

### Finding 1 — thirteen sites, one error value (E-061)

`spawn.rs` returns `SysError::NoMemory` from **thirteen** sites (41, 48, 62, 79,
100, 126, 143, 148, 163, 189, 198, 204, 220). Behind them are at least three
different conditions: **no free task slot** (41), **no frame** — for the kernel page
table, user text, the user stack, the kernel stack — and **the task table rejected
the insert** (220). `init`'s only report is one line, `init: spawn error`
(`fjell-init/src/main.rs:469`), which names neither the image nor the limit. When
RFC-0.34-001's two new services overflowed a 32-entry table, that line was the
whole diagnosis.

*(The register said four sites. It counted the first four occurrences.)*

### Finding 2 — the console buffer outlives the task it belongs to (E-062)

`DBG_TASKS = MAX_TASKS`, `DBG_LINE = 160`. Bytes accumulate per task and are
flushed on a newline or when the line fills — **never when a task leaves**. So a
task that exits or faults mid-line leaves its bytes in its slot, and the next task
to take that index has them emitted **in front of its first line**. In every
profile's `serial.log`, and in archived runs back to **2026-09-02**: eight
non-printing bytes (`90 90 90 90 90 90 90 92`; `…94` under `reboot`) between
`devmgr: profiles verified` and the next line. Two more defects in the same path:
a line longer than 160 bytes is split with **nothing marking the split** — a
braille presentation line is already 136 bytes at the sizes RFC-0.34-001 tested —
and `current_task_idx() % DBG_TASKS` turns an out-of-range index into an **alias**
instead of a failure.

### Finding 3 — one marker, two writers (E-063)

`M6: storaged ready` is printed by `storaged` (`main.rs:334`) **and** by `init`
after its relay wait (`main.rs:573`). Both lines appear in every serial log.

**What is not true, and the register now says so:** no committed marker
specification asserts this marker. All 45 `tests/qemu/profiles/*.toml` and
`tests/qemu/artifacts/*/expected-markers.txt` files were searched with the control
`driver-uart: ready`, found in four of them. **So this is E-014's class waiting,
not an instance**: the ambiguity is the defect, and the trap is for whoever adds
the assertion.

## The settled part

**D1 — an error a person reads names what failed.** A full task table gets its own
value, distinct from *out of frames*; `init` names **the image** it could not spawn
and the value it got. Two conditions that a caller must tell apart may not share an
error.

**D2 — a console line buffer belongs to its task's lifetime.** When a task leaves —
exit or fault — its slot is resolved, and a slot is never handed on carrying
another task's bytes.

**D3 — a split line says it was split.** A reader must not be shown two lines where
the writer wrote one, with nothing saying so.

**D4 — an index out of range fails.** The `% DBG_TASKS` aliasing goes; if the index
can exceed the table, that is a bug to surface, not to fold.

**D5 — one writer per marker.** `M6: storaged ready` has one, and the duplicate is
removed rather than renamed.

**D6 — demonstrated failing, each:** a task exiting mid-line and its bytes
appearing on the next task's first line (the case E-062 was filed from); a line
longer than the buffer; a full table reported distinctly from an exhausted
allocator; and, for D5, the marker printed exactly once in a real run.

**D7 — the per-byte `sys_debug_write` syscall is not redesigned here.** One byte per
`ecall` is why the buffering exists at all, and replacing it is an ABI change with
its own line. It is named as a survivor, not fixed.

## The open questions

**§A — on task exit, flush or clear?** Flushing emits an unterminated line;
clearing loses a dying task's last words, which is exactly what a person debugging
a fault wants. **Lean: flush, with the cut marked** (D3's mechanism serves both).
Measure what the buffers actually hold at exit across the tiers before choosing —
if they are always empty in practice, say so, because that changes what E-062's
eight bytes actually were.

**§B — what is the longest line the system can produce?** `MAX_WIRE_BYTES` is
4,624, so a braille rendering can exceed 160 bytes by a lot. Sizing the buffer from
the wire format costs `DBG_LINE × MAX_TASKS` of kernel `.bss` — at 4,624 × 40 that
is 185 KB, against the 34.6 KB that raising `MAX_TASKS` to 40 already cost. **Lean:
keep a bounded buffer and mark the split**, and have presentations wrap
deliberately rather than discover the limit. Say what bound you chose and why.

**§C — E-063: whose line goes?** `storaged`'s is the event; `init`'s marks its own
wait returning. **Lean: `init`'s goes**, but check first whether any evidence log,
document or procedure depends on seeing two — with a control, since the answer
"nothing" is what this RFC's own corrected figure got wrong once.

**§D — does D1 need a new `SysError` variant?** Almost certainly, and that is an
ABI change: `syscall-surface` and the snapshot both see it, and since RFC-0.33-004
D5 the snapshot sees **enum variants**, so this will be the first ABI addition Gate
4 catches by body rather than by declaration. **Lean: yes, one variant**, named for
the condition and not for the table. Say what it does to the syscall-surface
figures.

**§E — is the eight-byte prefix explained by §A's measurement?** The bytes are
`0x90`-ish, which is not ASCII and not UTF-8 lead bytes. **Do not close E-062 on a
fix whose mechanism you have not tied to those bytes**: either show the task whose
buffer held them, or say plainly that the mechanism is proven and the specific
bytes are not.

**Answer all five in writing before implementing.**

## Requirements

**R1 — Re-derive** all three findings at your tip: the thirteen sites and their
conditions; the buffer's constants and flush points; the eight bytes in a fresh
run; the two writers; and **the absence of any marker specification asserting the
M6 line, with a control** (`driver-uart: ready` is found in four). `/usr/bin/grep
-a` throughout.

**R2 — §A–§E answered in writing.**

**R3 — D1/§D:** the error value, `init`'s message naming the image, and the ABI and
syscall-surface figures that move.

**R4 — D2/D3/D4:** the buffer owned by its task's lifetime, the marked split, the
aliasing gone.

**R5 — D5/§C:** one writer, with the check that nothing depended on two.

**R6 — D6's four demonstrations**, each with a transcript, each reverted.

**R7 — A tier asserts what a person sees.** At least one profile must assert that
the console is clean where it used to carry the prefix — not only that the fix
compiles. This is the line where *the console is a surface* becomes a checked
claim.

**R8 — E-061, E-062, E-063 CLOSED** or survivors named, with `v1-limitations.md` in
the same commit as the register. The corrections dated 2026-09-25 in both entries
stay; do not tidy them away.

**R9 — The gates**, each by its own exit status, `test-all`, the repro baseline
re-recorded in the same commit as any rebuild, and a CI run id.

### Non-goals

- Redesigning `sys_debug_write` (D7).
- A logging framework, levels, or timestamps.
- Changing what any service prints, beyond D5's duplicate.
- The four `SysError` variants that remain undispatched (E-044's survivors).

## Risks

**Flushing on exit can itself corrupt a line** if the leaving task's bytes are
emitted while another task's line is open. The single-hart argument that makes the
current buffer safe must be re-made for the exit path, in the fix's own comment.

**A bounded buffer with a marked split is still a truncation** to whoever reads the
console for braille output. §B's answer belongs in `v1-limitations.md`, not only in
a comment: a person reading a presentation through the console needs to know the
line can be cut.
