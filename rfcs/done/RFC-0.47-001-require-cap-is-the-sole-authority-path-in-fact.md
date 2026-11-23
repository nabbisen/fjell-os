# RFC-0.47-001: `require_cap` becomes the sole authority path, in fact

**Status:** Implemented (0.47.0) — accepted 2026-11-23
**Evidence:** syscall:CapDelete, marker:capability:NEG:CAP:RECV_REFUSES_NON_ENDPOINT:PASS, marker:capability:NEG:CAP:RIGHT_BEFORE_LEASE:COPY:PASS, marker:capability:NEG:CAP:DELETE_RELEASES_REVOKED_LEASE:PASS, marker:capability:NEG:CAP:BIND_LEASE_WITHOUT_AUTHORITY_REJECTED:PASS, marker:capability:NEG:CAP:STALE_HANDLE_GENERATION_MISMATCH:PASS, file:tools/fjell-consistency-check/src/authority_funnel.rs, test:tools/fjell-consistency-check/src/authority_funnel.rs::an_inline_rights_check_is_red_and_named
**Milestone:** 0.47
**Tracks.** **E-126** — the funnel exists, declares itself sole, and is not. **E-129** — the receive path takes
no kind (§F5), filed as its own entry at the stage-1 review. Also carries, as findings the
handoff did not have, a latent hole in the receive path (§F5), a second `CapError`→`SysError` table (§F6) and
a dead capability kind (§F7). **E-126's disposition is the architect's:** this RFC's evidence for it is the gate in R9, green and shown red.
**Touches** *(indicative)*: `crates/fjell-cap/src/enforcement.rs`, `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-kernel/src/trap/syscall.rs`, `crates/fjell-syscall/src/lib.rs` (one stale doc),
`crates/services/fjell-neg-test/src/main.rs`, `tests/qemu/profiles/{capability,ipc}.toml`,
`tools/fjell-consistency-check/src/` (one new subcheck), `tests/abi/snapshot.json`, prebuilts and
`tests/repro/baseline-digests.txt`.
**Relates to:** **RFC 031 / RFC 032** (the funnel and its normative order), **RFC-0.41-002** (the object-0
invariant F5 breaks), **RFC-0.46-002** (its text records the premise E-126 corrects), **RFC-0.46-003**
(`revoke_permitted` is deliberately *not* a capability check and stays out of this line).
**Not in this line:** `DELEGATE_ONLY`, **E-093**, **E-073**, **E-095**, the network plane, and the 37
mismatched `SAFETY` justifications of the architect's proposed line 3 — beyond the seven this line's own
code makes true (R8).

*Every figure below was re-measured at `8cc6bcc`, not copied from the handoff. Where the handoff and the tree
differ, §"Where the handoff and the tree differ" says so.*

---

## What is true now

`crates/fjell-cap/src/enforcement.rs:56` defines `require_cap`. Its module doc calls it *"the **sole**
production authority-checking path"* and states a normative order (RFC 031 §2.6): lookup, generation,
slot-state, **kind (4)**, **rights (5)**, scope (6), **lease (7)**.

It is not sole. **Measured, by running the commands in §"Reproduce":**

| Path | Used by | Order | Error table |
|---|---|---|---|
| `require_cap` via `require_cap_on_ct` (`trap/syscall.rs:92`) | **11** call sites in `trap/syscall.rs`, **1** in `cap/syscall.rs` (`sys_cap_install`) | kind → rights → scope → lease | **a local one** (§F6) |
| `require_cap` called directly | **5** sites in `trap/syscall.rs` (`:1049 :1291 :1359 :1433 :1492`) | same | canonical `to_sys_error` |
| `check_right` (`cap/syscall.rs:491`) | `sys_ipc_send`, `_recv`, `_call`, `_try_recv` | rights → lease; **no kind, no scope** | `cs.get`'s own |
| four inline copies | `sys_cap_copy :30`, `_mint :77`, `_revoke :142`, `_inspect :185` | **lease → rights**; no kind, no scope | `cs.get`'s own |
| a sixth, inline | `sys_dma_alloc` (`trap/syscall.rs:1478`–`:1514`) | kind (two kinds) → rights → lease | mixed |
| authority by *scan*, no handle | `sys_cap_bind_lease` (`cap/syscall.rs:604`) | kind → state → rights → lease | — |
| nothing | `sys_cap_delete` (`:126`) | — | — |
| by design | `sys_cap_drop` (`:514`) | — | — |

### The findings

**F1 — the module doc is false.** Five production authority checks live outside the funnel (the four inline,
`check_right`) and two more (`sys_dma_alloc`, `sys_cap_bind_lease`) decide authority by their own text.

**F2 — the normative order is not followed by the inline four.** A capability that lacks the right **and**
carries a revoked lease gets `PermissionDenied` from `require_cap` and `check_right`, and `LeaseRevoked` from
`copy`/`mint`/`revoke`/`inspect`. One input, two answers, by which path serves it.

**F3 — seven `SAFETY` comments claim `require_cap` validated a handle it did not.** *Derived*, not
matched: every `SAFETY` comment under `crates/fjell-kernel/src` that says `require_cap` validated something,
paired with whether the enclosing function calls any funnel name. **Seven claims, seven functions, none calls
the funnel:** `sys_cap_copy :35`, `_mint :83`, `_revoke :146`, `_inspect :187`, `build_msg :467`,
`check_right :504`, `sys_ipc_reply :923`. The last is a different falsehood: `sys_ipc_reply` has no capability
handle at all — it checks a reply edge — so the claim is false however the funnel is arranged.

**F4 — `sys_cap_delete` checks no authority, and `cap_drop`'s documented difference does not exist.** Both
end in the same operation (`CapSlot::clear`: `Active → Empty`, generation++) and **both succeed on a revoked
lease.** What actually differs: `delete` answers `SlotEmpty` for an empty slot and `drop` answers
`InvalidCap` (`CapError::Dropped`); `delete` audits `CapDelete` on every attempt including failures, `drop`
audits `CapDrop` only on success. The doc comments (`enforcement.rs:28`–`:29`, `:105`–`:112`,
`cap/syscall.rs:513`) describe a lease-check difference that is not there.

**F5 — `sys_ipc_recv` and `sys_ipc_try_recv` never check the capability's kind. A latent hole,
*demonstrated*, not argued.** Both take `c.object_id` from whatever capability holds the `RECV` bit and use
it as an endpoint id (`cap/syscall.rs:811`, `:1039`). `CapRights::RECV` is bit 4 and is **in
`ALL_NON_META`**; the tree grants `ALL_NON_META` to non-endpoint capabilities — `LeaseAdmin`, `TaskCreate`,
`TaskControl`, `Reboot`, `DmaRegion`, `MmioRegion` (`spawn.rs:597 :735 :769 :830 :872 :889 :902`,
`main.rs:1380 :1394 :1408 :1644`) — and **every one of the `LeaseAdmin`/`TaskControl`/`Reboot`/`TaskCreate`
grants has `object_id: 0`**.

*Experiment, run on a real boot* (scratch worktree, one scenario added to `neg-test`, never committed,
removed after): `sys_ipc_try_recv` on `neg-test`'s own slot 4 (`LeaseAdmin`), 5 (`TaskCreate`), 6
(`TaskControl`) and 16 (`Reboot`). **All four returned `WouldBlock` (−20)** — not `WrongType`, not
`InvalidCap`: the kernel polled **endpoint 0**, an allocated endpoint with an empty queue, through a
capability that is not an endpoint. (A fifth, `DmaRegion`, was in the run; its number was lost in the
serial interleave with the audit stream and **is not claimed**.)

*Consequence.* RFC-0.41-002 R5 states, and `object-zero` gates, *"no task can receive on endpoint object 0"*.
That gate reads **`Endpoint` installs only**, so it is green while four other kinds can receive there. **It is
not a leak today** — nothing sends to endpoint 0, which is the only reason two receivers never took each
other's messages — **but the invariant is protected by that accident and not by the kernel.** `send` and
`call` are not affected: `build_msg` checks `kind == Endpoint`, after the rights and lease checks (so the
check order there is also not the funnel's).

**F6 — there are two `CapError`→`SysError` tables.** `fjell_cap::CapError::to_sys_error`
(`rights.rs:419`, "RFC 031 §2.7 error mapping table") and a second, local one inside `require_cap_on_ct`
(`trap/syscall.rs:111`–`:128`) that **collapses `InvalidHandle`, `GenerationMismatch` and `EmptySlot` into
`InvalidCap`** and ends in `_ => PermissionDenied`, so any error the arms do not name (`LeaseGenerationMismatch`,
`LeaseExpired`, `Dropped`, `Revoked`, `Internal`) becomes a permission failure. A comment in that very
function records that an earlier revision diverged on `WrongKind` and that *"no deliberate ABI reason
existed"*; the divergence was fixed for one arm and left for the rest. **Both maps are live in the same
scenario:** `neg-test` `:196`–`:232` asks `sys_cap_inspect` about an empty slot (inline path: `SlotEmpty`) and
`sys_clock_now` about the same slot (funnel wrapper: `InvalidCap`), and the test's expected tuple depends on
**both** answers. **No service branches on `GenerationMismatch` or `SlotEmpty`** (measured: the only
non-test mention is a comment), so the maps are observable to tests and to external ABI users, not to
the system's own services.

**F7 — `CapKind::DmaAlloc` is never granted.** `sys_dma_alloc` accepts `DmaRegion` *or* `DmaAlloc`
(`trap/syscall.rs:1483`–`:1488`) and carries the second inline path so that it can. `spawn.rs:583` records
that the grant was *"modernised from the legacy DmaAlloc alias"*; no code grants the alias. The branch accepts
a kind nothing can hold, and `fjell-syscall/src/lib.rs:590` still tells a caller to pass *"a
`CapKind::DmaAlloc` handle"*.

**F8 — `sys_cap_bind_lease` decides authority by scanning the CSpace**, not by a handle
(`cap/syscall.rs:604`–`:612`): *some* `Active` `LeaseAdmin` capability with `LEASE_CREATE` and `LEASE_BIND`
and a live lease. The funnel is handle-based, so it cannot serve this site as it stands.

**F9 — `CapRights::DROP` (bit 11) is enforced nowhere.** It is defined (`rights.rs:44`), named in the
capability manifest and in the broker's constants, and no kernel path requires it. It is another statement
about authority that nothing enforces — the subject of this milestone — and it bears directly on F4.

### Where the handoff and the tree differ

* The handoff counts **12** `require_cap_on_ct` call sites in `trap/syscall.rs`; there are **11** call sites
  and one definition (`:92`). Add the 5 direct `require_cap` calls and the funnel has **17** production callers.
* The handoff's "five production paths" is two short: `sys_dma_alloc` and `sys_cap_bind_lease` are authority
  checks the table omits (F7, F8).
* The handoff did not have F5, F6, F7, F9. F5 is the reason this line is more than hygiene.

---

## Requirements

**R1 — one body, entered by name.** `enforcement.rs` gets **one private implementation** taking
`expected_kind: Option<CapKind>` and **two public entries**:

* `require_cap(cs, handle, kind, rights, scope, checker)` — **unchanged signature**, kind mandatory. A caller
  of a kind-bound operation cannot forget the kind.
* `require_cap_any_kind(cs, handle, rights, scope, checker)` — for the operations that act on a capability as
  a slot-object whatever it names (`copy`, `mint`, `revoke`, `inspect`).

*What I concluded about the handoff's question.* **The signature is not wrong; the population has two
classes.** `Option<CapKind>` inside is not a sentinel — "no kind requirement" is the absence of a
requirement, exactly as `required_scope: Option<&ObjectScope>` already says for scope — and the two *public*
names mean a kind-agnostic call is **visible and greppable at the call site**, which a defaulted parameter
would not be. A sentinel "any kind" value in a mandatory parameter is what I would have recommended
against; this is not that.

**R2 — one order.** Every handler reaches the funnel and therefore gets kind → rights → scope → lease.
`copy`, `mint`, `revoke`, `inspect` change from lease → rights. **This is observable (D2).**

**R3 — the four IPC syscalls go through the funnel with `Endpoint`.** `check_right` is deleted and
`build_msg`'s own `kind != Endpoint` check dissolves into it. **`recv` and `try_recv` gain the kind check
they lacked, which closes F5** — a non-endpoint capability is refused with `WrongType` before its
`object_id` is read. **Observable (D2, D3).**

**R4 — `sys_cap_bind_lease`'s scan joins the funnel module** as `require_held(cs, kind, rights, checker)` —
*"the caller holds an `Active` capability of this kind with these rights and a live lease"* — one more
function in `enforcement.rs`, same private implementation, same order, no handle argument. The alternative
(leave it, classify it with a reason) is in D4.

**R5 — `sys_dma_alloc` takes one kind.** The dead `DmaAlloc` alias path is deleted, the handler calls
`require_cap(.., CapKind::DmaRegion, DMA_ALLOC, ..)` once, and the stale doc at `fjell-syscall/src/lib.rs:590`
is corrected. *Observable only for a capability of kind `DmaAlloc`, which nothing can hold.*

**R6 — `delete` and `drop` become one operation with two documented, true differences** (D5): the error code
for an empty slot and the audit behaviour — or one is retired. The doc comments stop describing a lease check
that is not there.

**R7 — the module doc is rewritten to be true**, listing the two entries, `require_held`, and the **named,
reasoned exemptions** (`cap_drop`/`cap_delete` — releasing authority you hold cannot add authority;
`sys_ipc_reply` — authority is the reply edge, not a capability; `revoke_permitted` — an ownership predicate
applied *after* the funnel).

**R8 — the seven false `SAFETY` comments become true or are reworded.** Six become true by R2/R3 (the
functions now call the funnel); `sys_ipc_reply`'s is reworded to say what it checks. **Only these seven.**
The wider set is the architect's proposed line 3 and is not started here.

**R9 — the gate: `authority-funnel`, the 26th `consistency-check` subcheck.**

* **Population is derived, not listed.** Every tracked Rust source under `crates/fjell-kernel/src`, through
  the shared `fjell-repo-scope` function. No syscall name appears in the check.
* **Half 1 — an authority check outside the funnel.** A source token of the shape *rights tested*
  (`.rights.contains(`, `.rights.is_subset_of(`), *lease tested* (`.check_lease(`) or *kind tested*
  (`.kind ==`/`!= CapKind::`, `matches!(…kind…)`) in kernel code **fails unless the enclosing function is on
  a classified list with a stated reason.** At `8cc6bcc` the population is **20 tokens in 3 files**
  (16 in `cap/syscall.rs`, 3 in `trap/syscall.rs`, 1 in `spawn.rs`, the last a comment). **The intended
  classified list after this line is empty or near it** — the check is judged by whether it is *short*, and
  each entry must name its reason, as `repo-scope`'s does.
* **Half 2 — a false `SAFETY` claim (the more valuable half, and the one `unsafe-audit` cannot make).** For
  every `SAFETY` comment whose text says `require_cap` validated something, the **enclosing function must
  itself call a funnel entry** (`require_cap`, `require_cap_on_ct`, `require_cap_any_kind`, `require_held`).
  A comment asserting a validation its function does not perform fails with the function and line named.
  Run at `8cc6bcc` this reports **seven**, which is the check's own first result and its evidence that it
  works.
* **An evaluator, not a name match.** Function boundaries come from brace matching with strings, comments and
  char literals handled; a function whose body the check cannot delimit is a **failure**, not a skip.

**R10 — one error table.** The local table in `require_cap_on_ct` is deleted and the wrapper uses the one
map (D2).

**R12 — an instruction to use a capability that cannot be obtained, corrected where it is published
(architect, stage-1 review).** `fjell-syscall/src/lib.rs:590` tells every caller of `sys_dma_alloc` that it
*"must pass … a `CapKind::DmaAlloc` handle"*, and `fjell-storaged/src/main.rs:97` says *"Slot 2: DmaAlloc
cap"*, while **nothing in the tree grants that kind** (R5/F7). R5 corrects the SDK doc; R12 adds the service
comment. The SDK sentence was **a published instruction to use an unobtainable capability** — a caller that
obeyed it could never have succeeded — which is the "an API must not mislead" half of the design bar, not
hygiene. (The service comment is edited **within its line**, so the prebuilt's bytes do not move for it.)

---

## The decisions this RFC asks for

**D1 — the funnel's shape (R1).** Two public entries over one `Option<CapKind>` body, plus `require_held`.
*Alternatives rejected:* a sentinel `CapKind::Any` (a value that must be remembered to be checked against);
one entry with `Option<CapKind>` public (kind-bound callers could omit it silently).

**D2 — the error-code changes, taken deliberately, and which map is the map.** Three classes of change are
observable and **none of them is visible to `abi-snapshot`; that gate covers declarations, not behaviour, and a reader should
not assume otherwise** — measured: the snapshot hashes the *declarations*
of public items (`SysError`'s definition, `CapError`'s), not which code a path returns, so it will not move for
any of these and **cannot gate them**; boot scenarios must (R11). (R1's two new public functions *do* move the
snapshot, once, and it is regenerated.)

| Class | Today | After | Syscalls |
|---|---|---|---|
| **order** — lacks the right **and** lease revoked | `LeaseRevoked` | `PermissionDenied` | `cap_copy`, `cap_mint`, `cap_revoke`, `cap_inspect` |
| **kind vs right** — a non-endpoint capability lacking the IPC right | `PermissionDenied` (rights first) | `WrongType` (kind first) | `ipc_send`, `ipc_call` |
| **kind** — a non-endpoint capability holding the IPC right | *accepted as endpoint `object_id`* | `WrongType` | `ipc_recv`, `ipc_try_recv` (**F5**) |
| **error map** — stale / empty handle | stale: `InvalidCap` everywhere; empty: `SlotEmpty` (inline 9) or `InvalidCap` (funnel 12) | per the chosen map | all |

*Which answer is right for the order class.* **`PermissionDenied`.** The funnel's order is the normative one
(RFC 031 §2.6) and every path except the inline four already follows it; but the stronger reason is the
one the handoff names: telling a caller that **lacks the right at all** that a lease was revoked discloses the
state of an object it has no authority over. `LeaseRevoked` is a fact about the lease table, and the right to
be told it is the right to use the capability. The inline order is therefore **a defect, not a difference**.
No existing scenario exercises the both-defects input — which is why no test is red at the order change, and
is itself the finding: the order was untested. R11 adds it.

*Which map.* I recommend the **canonical `to_sys_error`** (RFC 031 §2.7, already used by every `.into()`), and
deleting the wrapper's table. It keeps `GenerationMismatch` and `SlotEmpty` distinct from `InvalidCap` — a
stale handle and an empty one are different facts about *the caller's own* CSpace, so there is no disclosure —
and stops mapping unnamed errors to `PermissionDenied`. **The cost is the larger change:** the 12 wrapper sites
change stale `InvalidCap → GenerationMismatch` and empty `InvalidCap → SlotEmpty`, and `neg-test`'s `:230`
tuple changes. **The alternative is the minimal-churn one:** adopt the wrapper's collapsed table as *the* map,
which changes only the inline nine's empty-slot answer (`SlotEmpty → InvalidCap`) and nothing else — but it
contradicts RFC 031 §2.7's documented table and would need that table amended, trading a code change for a
document change. **Your ruling.** If you pick the alternative, R10 becomes "move the wrapper's table into
`fjell-cap` as `to_sys_error`".

**D3 — F5 is a hole, not a convention.** Recorded as such, with the experiment. It is fixed by R3. I am not
asking whether to fix it; I am asking you to confirm the classification, because it changes how RFC-0.41-002's
text should be read (its invariant held by accident for `Endpoint` installs and was never a kernel property).
**Filed as its own erratum, E-129** (the architect's ruling), carrying that `object-zero` is green while four
other kinds can receive on object 0.

**D4 — `sys_cap_bind_lease`'s scan (R4).** Recommended: `require_held`. The alternative — leave it as a
classified exemption with a recorded reason ("authority by possession; no handle is passed") — is honest but
leaves a hand-rolled copy of steps 3, 4, 5 and 7 whose order must be kept in step by hand, which is the defect.

**D5 — `sys_cap_delete`'s authority.** Four options; I recommend **(a)**.

* **(a) Release is always permitted, for both.** Releasing a capability you hold can only *reduce* authority,
  which is why `cap_drop` is already allowed to ignore a revoked lease; steps 1–3 (handle resolves to an
  `Active` slot) are all there is to check. `delete` and `drop` share one implementation; the doc comments say
  they differ in the **error for an empty slot** and the **audit-on-failure** behaviour, **which is true**, and
  the lease sentence is deleted. Observable change: none beyond D2's map.
* (b) Require `DROP` on both. **Rejected on a measurement, not a taste:** capabilities are routinely granted
  with narrow rights (a `SEND`-only endpoint), so every such holder would become unable to release its own
  slot. That is a behavioural regression for a safety gain of nothing.
* (c) Require `DROP` on `delete` only. Makes `delete` strictly less usable than `drop` for no stated reason.
* (d) Retire `delete`. An ABI removal; out of scope and nobody asked.

**F9 follows from (a):** `CapRights::DROP` would remain defined and enforced nowhere. I propose it is
**recorded as reserved — the bit is retained for ABI, releasing is always permitted, nothing may require it**
— in `rights.rs`'s doc and the module doc, and that **the gate's classified list carry no entry for it**
(nothing tests it). The alternative is to enforce it, which is (b).

**D6 — where the gate's classified list lives.** In the subcheck's source, one entry per `file::fn` with a
reason, and **a stale entry is a failure** (the way `repo-scope` retires its own). Not in a data file, so the
reason travels with the check that judges it.

---

## Rulings (architect, stage-1 review, 2026-11-22)

**All six decisions accepted; one addition (R12).** D1 as proposed. **D2: both halves, and the architect records
that my reason supersedes theirs — the governing argument for `PermissionDenied` is the *disclosure* one**
(telling a caller that lacks the right at all that a lease was revoked discloses the state of an object it has no
authority over; "the normative order says so" is only a consistency reason). **On the map: take the canonical
`to_sys_error` and pay the churn** — the minimal-churn alternative would amend RFC 031 §2.7's documented table
to fit the code, *"changing a normative document to fit an implementation is the inversion this whole
milestone exists to end"*. R10 is "delete the wrapper's table". D3 confirmed and **verified independently,
statically** (`cap/syscall.rs:811 :1039`; `ALL_NON_META` includes `RECV`; the grants) — filed as **E-129**.
D4 `require_held`. **D5 option (a)**; F9 as proposed — `DROP` recorded as reserved, retained for ABI, nothing
may require it, and the classified list carries no entry for it. D6 accepted, a stale entry a failure.

## Negative cases (R11) — and what I will show red

1. **The both-defects case, through every handle-taking syscall:** a capability that lacks the right **and**
   carries a revoked lease, via `cap_copy`, `cap_mint`, `cap_revoke`, `cap_inspect`, `ipc_send`, `ipc_recv`,
   `ipc_call`, `ipc_try_recv` — asserting `PermissionDenied` (or `WrongType` where D2 says) — **a new
   `neg-test` scenario set in the `capability` tier.**
2. **F5, closed:** `ipc_recv` and `ipc_try_recv` on a `LeaseAdmin`, a `TaskControl`, a `Reboot` capability each
   → `WrongType`; marker pinned in a profile.
3. **`cap_delete` on a revoked-lease capability** → whatever D5 decides, asserted (default: success and the
   slot is `Empty`).
4. **Host tests** for the funnel's two entries and `require_held`: every step's refusal, the order, and the
   property that the two entries agree except for the kind step.
5. **Mutations, each to show a named failure** (a compile error is INVALID, per the standing rule): restore
   the lease-before-rights order in one inline path → case 1 red; drop the kind check in `recv` → case 2
   red; **reintroduce an inline `rights.contains` in a handler → `authority-funnel` red, naming it**; plant a
   `SAFETY` comment claiming `require_cap` in a function that does not call it → `authority-funnel` red,
   naming it; plant a hand-rolled check in a *new* file under `crates/fjell-kernel/src` → red (the population
   is derived, so a new file cannot hide).

## What this RFC does not do, and why

* **It does not touch `DELEGATE_ONLY`.** It is the payoff and depends on this landing; a negative right is a
  design smell that deserves its own RFC against a `Delegable` kind or an install-source table. Out of scope is
  a decision.
* **It does not start E-093** (deferred to 0.48 on sequencing — every remedy rewrites authority decisions, and
  all are easier once there is one place where authority is decided).
* **It does not start line 3.** The seven comments R8 fixes are exactly those whose functions this line
  rewrites; the other 30 of the 37 are not touched, and the check in R9 half 2 deliberately judges only
  *`require_cap` claims*, not the whole safety-annotation population.
* **It does not claim F5 is exploitable today.** Nothing sends to endpoint 0. It claims the kernel does not
  enforce what the object-zero gate says it enforces, and that is the finding.
* **It does not change `sys_cap_install`'s order or error table** beyond D2's map: its one funnel call already
  follows the funnel.

## Reproduce

```
git ls-files -z '*.rs' | xargs -0 /usr/bin/grep -an 'require_cap_on_ct\|check_right(\|require_cap('
git ls-files -z '*.rs' | xargs -0 /usr/bin/grep -an 'rights\.contains(\|\.check_lease(\|\.kind [=!]= CapKind::'
git ls-files -z '*.rs' | xargs -0 /usr/bin/grep -an 'SysError::GenerationMismatch\|SysError::SlotEmpty'
```

F5 is reproduced by one added `neg-test` scenario calling `sys_ipc_try_recv` on slots 4, 5, 6, 16 under
`cargo xtask qemu-negative ipc` and reading `serial.log`; the committed scenario of R11 case 2 replaces it.
