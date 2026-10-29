# RFC-0.40-002 R1–R7: a revocation is audited under a code the decoder knows

**Governing RFC:** [../accepted/RFC-0.40-002-a-revocation-is-audited-under-a-code-the-decoder-knows.md](../accepted/RFC-0.40-002-a-revocation-is-audited-under-a-code-the-decoder-knows.md)
**Handoff:** [../handoffs/RFC-0.40-002-a-revocation-is-audited-under-a-code-the-decoder-knows/implementation-handoff.md](../handoffs/RFC-0.40-002-a-revocation-is-audited-under-a-code-the-decoder-knows/implementation-handoff.md)

Written against base `0555d76`. **E-092 closes in this line.**

## R1 — the wholesale comparison, and whether the table held

Method: a script (kept with the evidence, not committed) that **parses both enums** and diffs the
value sets, rather than reading them.

```
variants: kernel=22 userspace=22
same name, DIFFERENT value: [('LeaseRevoked', 40, 16)]
kernel-only names: [('CapDenied', 41)]
userspace-only names: [('Internal', 255)]
values the kernel can emit that userspace cannot name: [40, 41]
duplicate values within an enum: kernel=[] userspace=[]
```

**The architect's table held exactly.** Control: the same script on a copy where I changed an
*unrelated* value (`CapDrop` 15 → 14) reported both `CapDrop` and `LeaseRevoked` without being told
where to look. After the fix the script reports no mismatch and no kernel-only name.

## R2 — the direction: (a), the kernel emits 16

Because `AuditKind` is the published ABI and the kernel is the implementation, and because (b) would
leave two values meaning one event with the published one never emitted. (b) was the answer *if*
something depended on 40, so I looked for it: no evidence file, expected-markers artefact, tier or
decoder contains the wire value; `auditd:51` is the only decode site; the only other `40` near audit
code is `SysError::LeaseRevoked`'s unrelated syscall error code `-40`. The handoff's warning (four
consecutive lines found a dependency) was a live possibility and I did not find one.

## R3 — `CapDenied`

Deleted. Emitted by nothing, decoded by nothing. **No emitter added.** A comment where it stood says
capability denials are *not* audited. **My proposal, as asked for, not a placeholder:** a capability
denial is an event a person investigating an incident would want, and the kernel has no trace of it
today, so it is probably worth a line of its own — but it carries the same question as E-091's
unbuilt `AUDIT_LEASE_BIND_REJECTED` and the 0.38-004 note that a refusal leaves no trace: a denial is
reachable by exactly the caller doing something it should not, so a record per denial is
attacker-controlled log volume. It needs a rate or a ring-slot policy decided first. I would sequence
it with E-091.

## R4 — the tier, red then green, asserting the decoded name

`neg-test` creates and revokes a lease, drains the ring, finds that revocation's record, and runs it
through `AuditRecordBin::from_bytes` → `AuditKind::from_u16` → `label()` — the decode `auditd` does on
each record. It asserts the **label**, not the raw word. (The raw words 16 and 40 are used only to
*pick the record out of the ring*, so the same scenario finds it before and after the fix.)

* **Pre-change, red (exit 1):** `neg-test: audit: the record for the revoked lease decodes as 'internal'`;
  `missing marker NEG:AUDIT:LEASE_REVOCATION_DECODES_AS_REVOKED:PASS`.
* **Post-change, green:** `… decodes as 'lease.revoked'`, marker present; the `audit` tier passes 2 markers.

### Where I could not do what the RFC asked, and why

R4 says *"in `auditd`'s decoded output"*. That is **not reachable**: `auditd` drains the kernel ring
**once, at start-up**, and then only when a message arrives on its endpoint, and nothing sends one
(its trigger is the one RFC-0.28-001 and RFC-0.33-001 recorded as "never fixed"). In a normal boot
its entire output is **9 early-boot records** (`boot`, `task.*`, `ipc.*`) and no revocation, which
happens later. So the assertion runs the real kernel record through the same decode inside `neg-test`
(which also holds the AuditDrain capability and drains the same ring). Whether `auditd` should get a
working trigger — a topology and protocol change — is the architect's, and is noted in the E-092
closure. I did not build it.

### Fix-forward: the tier must not need the ring empty

As first written the tier flushed the ring and required it to come back empty. That held on the 0.40-002
tree and failed most runs once 0.40-001 was in: while services start, `TaskSwitch` records refill the
256-slot ring faster than a 64-drain loop empties it. The assertion never needed an empty ring (the record is
picked out by lease id), so `drain_audit_all` now fails only if a drain fails, the pre-drain is best effort,
and the last match wins. Red re-shown (kernel emitting 40 → `internal`), green 8/8. Commit `c37dbdc`.

## R5 — the ABI snapshot

`AuditKind` did not move: **Removed 0, Changed sig 0**, and the kernel enum is not tracked. The one
change was **+1, the tier's marker constant** (`AUDIT_LEASE_REVOCATION_DECODES`), 495 → 496,
regenerated — the same as 0.39-004. So the RFC's "Added 0" holds for the *fix* and not for a fix
*with a tier*.

## R6 — E-092 closed

Register closure note (original left as written) and Summary row. `v1-limitations.md` **row 11**
rewritten as closed rather than deleted (it sits mid-table and other documents cite rows by number;
row 9 is the precedent). It says what stands: a refused operation still leaves no trace, and `auditd`
does not print a revocation after start-up.

## R7

Gates are in the review request.
