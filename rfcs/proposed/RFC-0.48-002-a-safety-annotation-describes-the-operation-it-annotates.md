# RFC-0.48-002: A safety annotation describes the operation it is attached to — and the audit says what it cannot know

**Status:** Proposed
**Milestone:** 0.48
**Tracks.** The finding the 0.47 handoff named *proposed line 3*, now the owner's line: **`unsafe-audit` reports
291 sites, 291 annotated, 0 missing, and Gate 2 passes, because it checks that a comment *exists*.** No erratum is
filed for it yet; this RFC proposes one (D7) rather than filing it, because its disposition depends on the rulings
below. The E-014 class (an instrument reporting coverage it does not have), in the safety annotations of a kernel.
**Touches** *(indicative)*: `tools/fjell-unsafe-audit/src/main.rs`, a shared scanner (D5), the `SAFETY` comments of
roughly 73 sites across `crates/fjell-kernel/src/` and `crates/drivers/`, `crates/fjell-tools/src/trust_report.rs`
(what Gate 2's line says), `docs/src/` where the audit is described.
**Relates to:** **RFC-0.47-001** (`authority-funnel` already pairs `SAFETY` claims with the function beneath them —
this generalises half of it), **E-014**, **E-125** (instruments that report scope they do not have), **RFC-0.48-001**
(this line's sweep follows it, per the handoff's sequencing).
**Not in this line:** judging whether a justification's *prose* is true (D1); `unsafe` in code the audit does not
scan (host tests, `fjell-fuzz`, vendored code).

*Figures re-measured at `587778d`. This is the fourth time this population's figure has moved (37/50 → 27/40 → the
numbers below); the RFC states how each is derived so the next move is a re-run, not a surprise.*

---

## What is true now

The handoff's command reproduces exactly (re-run): of **40** `unsafe` sites that take a static-mut reference through
`get_lease_table()` / `get_kernel_state()`, **27 carry a justification that does not describe that operation**, **16 of
those with the wrong `category=`** (13 `user-copy`, 2 `csr-asm`, 1 `page-table-mutation`, each reading *"all
user-supplied pointers are checked against the task address space before dereferencing"*, on an access to a
`static mut`). Three sites read, not matched — `trap/syscall.rs` `sys_task_spawn` (`&mut *fa`, a frame-allocator
pointer), `sys_audit_drain` (`get_kernel_state()`) and `dispatch_task_spawn` (`fa_static_ptr()`) — **none dereferences a
user-supplied pointer**, and the audit records all three as annotated.

**That population is the part of the problem someone already looked at.** Measuring the whole tree, three more
findings change the line's size and its first requirement:

### F1 — the audit does not see 49 of the tree's 346 `unsafe` tokens

`fjell-unsafe-audit` finds sites by substring on one line (`unsafe {`, `unsafe fn `, `unsafe impl `, `unsafe trait `).
Counting every `unsafe` token in code (comments and strings blanked) gives **346**; the audit records **291**. The
**55** it does not record are **49 real, 6 prose** (strings in other tools' source):

| Shape | Unrecorded | Why the audit misses it |
|---|---|---|
| `#[unsafe(no_mangle)]` | **42** | an attribute, not a block; edition-2024 requires the `unsafe(…)` wrapper because the exported symbol name is an unchecked claim |
| `unsafe extern "C" { … }` block and `pub unsafe extern "C" fn` | **6** | the `extern "C"` between `unsafe` and `fn` defeats the `unsafe fn ` substring |
| `unsafe impl<T> Sync for KS<T> {}` | **1** | the generic list defeats `unsafe impl ` — and a `SAFETY` comment sits right above it, unattached to anything |

So **"291 sites, 0 missing" is a count of the sites the scanner can see.** Whether `#[unsafe(no_mangle)]` is an
*operation* needing a justification is a ruling (D4); that **the `unsafe impl<T> Sync` — a soundness claim about a
static shared across the whole kernel — is invisible to the audit** is not.

### F2 — the surplus of comments over sites is accounted for (310 vs 291)

The handoff's "310 category-tagged comments against 291 sites" was unexplained. Measured: **311 `SAFETY` lines carry
`category=`; they form 299 comment blocks; 291 are attached (nearest preceding, within 12 lines) to exactly one site
each.** Every one of the 291 sites has *its own* comment — **no site borrows a neighbour's**, which was the failure
mode worth ruling out. The 8 unattached blocks are: 4 in `fjell-unsafe-audit`'s own source (its documentation and
fixtures), 1 above the undetected `unsafe impl<T>` (F1), and 3 shadowed by a nearer comment (by difference). **The
surplus is not comments annotating things that are not unsafe.**

### F3 — a purely lexical rule is wrong in 40% of cases, and the reason is the design

A first rule — *"an `unsafe` body must contain a token characteristic of its `category=`"* (`asm!` for `csr-asm`,
`read_volatile`/`write_volatile` for `mmio-access`, a static-mut accessor or a `static mut` name for
`kernel-global-mutable`, a pointer deref/cast for `raw-pointer-deref`, `page_table`/`map_page` for
`page-table-mutation`, a user-pointer type for `user-copy`, a physical/frame token for `phys-id-map-assumption`) —
was prototyped over all **291** sites and flags **117 (41%)**. Reading the failures, most are **correct tags that the
rule cannot see through a call**: `unsafe { write_sie(0x222) }` is `csr-asm` because `write_sie` is an `unsafe fn`
whose body is `asm!`. The tree has **56 `unsafe fn`s**, each carrying its own category.

Refined — *the body contains the category's shape **or calls an `unsafe fn` carrying the same category*** — over the
283 sites with a body (8 `unsafe impl`s have none):

| Verdict | Sites |
|---|---|
| shape present directly | **166** |
| delegated to a callee of the same category | **44** |
| **flagged: calls an `unsafe fn` of a *different* category only** | **35** |
| **flagged: no shape and no unsafe callee** | **38** |

**210 pass; 73 (26%) are flagged and each needs a human decision.** Read, the flagged set is not uniform:

* **Genuine mismatches** — `trap/dispatch.rs:111` is tagged `csr-asm` on `let tf_ref = unsafe { &mut *tf }`, a
  raw-pointer deref; `main.rs:1714` is `csr-asm` on a call to `first_entry` (`raw-pointer-deref`); `main.rs:906` is
  `csr-asm` on a call to `map_page` (`page-table-mutation`); the 16 wrong-category static-mut sites of the handoff.
* **Overlapping categories, legitimately** — `page_table.rs:105` is `page-table-mutation` on pointer arithmetic over
  a page table: both tags are true. A site can be two things.
* **A gap in the taxonomy** — about six sites tag `phys-id-map-assumption` on `&__stack_top as *const u8 as usize`, the
  address of a linker symbol. That is none of the seven categories, and the tag is the nearest one that was
  available.
* **Call-through to a different category by design** — `unsafe { init() }` tagged `kernel-global-mutable` where
  `init` is `mmio-access`: the *block* touches a static; the *callee* touches a device.

**So the check cannot decide a category; it can only refuse one the evidence does not support.** That is the whole
design, and it is why a green must say what it is (D1).

---

## Requirements

**R1 — the population is derived by a real scan.** The scanner finds every `unsafe` in code (comments, strings and
character literals blanked — `authority_funnel::code_only` already does this and is tested against raw strings and
nested comments), in every tracked `.rs` through the shared `fjell-repo-scope`; **no site is recorded by substring on
a line.** F1's 49 are in the population or classified out by name (D4).
**R2 — the rule.** A site's `category=` is *supported* when its body contains that category's shape **or** calls an
`unsafe fn` (resolved by name within the workspace) that carries the same category; it is *unsupported* otherwise.
`unsafe impl` and `unsafe trait` have no body and are judged on their own list (D3). A call that resolves to several
`unsafe fn`s of one name is supported if **any** has the category — the check refuses only what no reading supports.
**R3 — an unsupported site fails Gate 2** naming the file, line, tag, and what the body *does* contain (the shapes
found, the unsafe callees and their categories), so the fix is read off the failure.
**R4 — a named exemption, with a reason, is the only other way through.** A classified list keyed by `file::fn#n` (the
n-th unsafe site in it), each entry a **reason**; a stale entry — one whose site is now supported or gone — **fails**,
the way `repo-scope` retires its own. A site whose true justification cannot be determined is **exempted with that
sentence**, not given a plausible one: *a guessed comment is false in a new way*.
**R5 — the output says what it proves.** Gate 2's line becomes, verbatim:
`unsafe-audit: PASS (N sites; every one has a SAFETY comment and a category the body or its callees support; this does
NOT check that the comment's prose is true, and an exempted site (M) is unjudged)`. The release record and the
trust report quote the line, not "291 annotated".
**R6 — the sweep.** Every flagged site is re-tagged, given a corrected comment, or exempted. **Where a comment's
text describes an operation the site does not perform, it is rewritten to describe the operation** — the 27 handoff
sites are the first batch, not the definition of the work. **After RFC-0.48-001 lands** (D6).

## The decisions this RFC asks for

**D1 — gate the category; leave the text to review; say so in the output.** Recommended, as the handoff did. A check
that parsed prose would pass any sentence containing the right words — *"all user-supplied pointers are checked"*
passes for a user pointer and fails for a static, but **the same words pasted onto a static are exactly the defect**,
so a text rule would have to be told which sentences are boilerplate, which is the E-014 shape. R5's sentence is the
guard: **a green means the tag is not contradicted, not that the justification is true.** The RFC also proposes the
audit **print the count of distinct comment texts** (**161 distinct of 311** at this tip, exact text after trimming) so a reader can see how
boilerplate-heavy the tree is without a rule judging it.
**D2 — the taxonomy.** Add `linker-symbol` (address-of an `extern` static / linker-provided symbol) for the ~6 sites
that have no honest tag; do **not** add a catch-all `other`, which would be the exemption list with no reason column.
Whether a site may carry **two** tags (`category=page-table-mutation,raw-pointer-deref`) is the real taxonomy question:
**recommended yes**, because the alternative forces an arbitrary choice at every overlapping site — `page_table.rs:105`
is both — and arbitrary choices are how the tags drifted. The check then requires *each* named tag to be supported.
**D3 — `unsafe impl` / `unsafe trait`.** 8 recorded (+1 invisible). They assert a type may cross a thread boundary or
that a trait's contract holds; there is no operation to match. **Recommended: they carry `category=` as today and are
judged only for *having a comment and a valid tag*, listed in the output as a separate count**, since pretending a
shape rule applies would be the overstatement this line is about.
**D4 — what counts as a site.** Rule on F1: `#[unsafe(no_mangle)]` (42), `unsafe extern` (6), `unsafe impl<T>` (1).
**Recommended: all three are sites.** `unsafe extern` and `unsafe impl<T>` plainly are. `#[unsafe(no_mangle)]` is an
unchecked claim that the symbol name collides with nothing, which is exactly what `SAFETY` comments exist to state;
the 42 are boilerplate-shaped and a new category `symbol-export` would let one sentence be correct for all of them.
If the architect rules attributes are not sites, the output says *"N unsafe attributes are not counted"*.
**D5 — where it lives.** The audit needs a **workspace-wide** pass (callee categories resolve across files), a
tokenizer that is not line-substring, and the `code_only` scanner. `fjell-unsafe-audit` is a standalone tool with no
dependencies, by design. **Recommended: a small shared crate (`tools/fjell-rust-scan`) exporting the blanker and the
brace-matched function/block extractor**, used by `fjell-unsafe-audit` and `authority_funnel`, which today each
contain a version of it; the alternative is a third copy. (This is the `repo-scope` pattern one level up.)
**D6 — sequencing.** The handoff is right and the measurements support it: RFC-0.48-001 changes `spawn()`, `main.rs` and
`lease/mod.rs`, which together hold **53 of the 291 sites** (`main.rs` 43, `spawn.rs` 9, `lease/mod.rs` 1) — and **25 of the 73
flagged sites are in `main.rs` alone**, the boot code that line 1's init-exemption and bind step sit next to;
re-annotating them first annotates code that is about to move. (Of the handoff's 40 static-mut sites, 7 are in
`spawn.rs` and none in `main.rs`: that population understates the exposure, which is why it is not the one measured
here.) **R1–R5 (the rule and its gate) do not
depend on line 1; R6 (the sweep) does.** So the **gate can land first with a seeded exemption list** — every currently
flagged site, reason *"awaiting RFC-0.48-001's sweep"* — and the sweep then **shrinks that list to zero**, which is a
checkable end state: *the exemption list's length is the sweep's progress*, and an entry cannot silently outlive its
site.
**D7 — an erratum.** The finding stands regardless of the rulings: *the audit's "0 missing" is a count of sites it
can see, and its annotations are checked for existence only.* Proposed **E-131** (found 2026-11-24), `RFC:` none, ACCEPTED
tracking 0.48; closed when R1–R5 land and the exemption list is empty.

## Negative cases — what I will show red

1. **Each category's rule, planted:** a `csr-asm` tag on a block that contains no `asm!` and calls no `csr-asm` fn;
   likewise `mmio-access`, `kernel-global-mutable`, `raw-pointer-deref`, `page-table-mutation`, `user-copy`,
   `phys-id-map-assumption` — **seven failures, each naming the tag and what the body does contain.**
2. **The real defect:** run on the tree as it stands, the gate reports the 73 and the 16 wrong-category static-mut
   sites among them — **the three read sites named in the handoff each appear by file and line.**
3. **Call-through is not a free pass:** `unsafe { write_sie(..) }` tagged `kernel-global-mutable` is red; tagged
   `csr-asm` is green.
4. **A site the scanner used to miss:** a planted `unsafe impl<T> Sync` and a `pub unsafe extern "C" fn` with no
   comment turn Gate 2 red as *missing a comment* (they were invisible before).
5. **Exemption hygiene:** an entry for a site that is now supported fails as stale; a site with no comment cannot be
   exempted into a pass; an exemption with an empty reason fails.
6. **The output sentence (R5) is itself pinned:** a test fails if the PASS line stops saying it does not check the
   prose — so the overstatement the line exists to remove cannot be edited back in.
7. **Mutations, each a named failure** (compile error is INVALID): rule loosened to accept any category; callee
   delegation accepts *any* category; the exemption list not consulted; stale-entry check removed.

## What this RFC does not do, and why

* **It does not claim any justification is true.** Not before, not after: the tree has 311 category-tagged `SAFETY`
  lines of which **the count of distinct texts is the honest measure of how much review the prose still needs**.
* **It does not change a single `unsafe` block's behaviour.** The sweep rewrites comments and tags; the build must
  be byte-identical (`fjell-repro-check`: **0 changed digests**) — **a comment-only line that moves a binary has done
  something else**, and that is a mutation I will run.
* **It does not scan code the audit never scanned** (host tests under `#[cfg(test)]` stay in the population as they
  are today; `fjell-fuzz` and vendored crates are out).
* **It does not decide D4 for the architect**, and says what each answer does to the number.

## Reproduce

```
git ls-files -z '*.rs' | xargs -0 /usr/bin/grep -aB1 \
  'unsafe { crate::get_lease_table() }\|unsafe { crate::get_kernel_state() }' \
  | /usr/bin/grep -ao 'category=[a-z-]* .*' | sort | uniq -c | sort -rn
cargo run -q -p fjell-unsafe-audit -- --root . --check
cargo run -q -p fjell-unsafe-audit -- --root . --json      # the 291 sites the prototype was run over
```

The 346 / 49 count, the 299-block accounting and the 166 / 44 / 35 / 38 table come from throwaway scripts over the
`--json` inventory and the blanked source; the implementation commits them as the gate's own tests over the real tree.
