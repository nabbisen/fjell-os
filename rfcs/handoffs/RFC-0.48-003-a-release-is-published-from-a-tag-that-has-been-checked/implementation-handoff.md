# Developer Handoff — RFC-0.48-003

**Governing RFC:** [RFC-0.48-003](../../done/RFC-0.48-003-a-release-is-published-from-a-tag-that-has-been-checked.md)
**Milestone:** 0.48 · third line
**Status:** inherited from the governing RFC (Implemented, 0.48.0)
**Audience:** implementation model

---

## 1. The owner's ruling, and the one the architect got wrong

**The owner authorized an automatic Release on tag push and overruled the architect's recommendation
that it create a draft:** the tag is already the owner's authorization signal, so publishing from it
bypasses nothing. Their constraint — *"the trigger should be designed and managed carefully"* — makes
**the trigger the deliverable.**

**The architect also asked for something impossible:** attach the archive, its signature and digest,
and verify them. **Step 12b signs the archive *after* the tag, with the offline key**, so at the moment
the push fires the signature does not exist. Notes only; the archive and signature attach as a cut step
verified by the `verify-bundle-sig` the checklist already prescribes. **Automating assets would need a
CI-held signing key and a different trust model**, which trades away the property Step 12b exists to
keep.

## 2. Two things the architect's design could not have

**The correspondence it would gate does not hold for history.** Of **105** bare tags, only **35** have
a CHANGELOG section and **27** a release record; it is total **from `0.21.3` on, with zero exceptions**,
and absent before. **The floor is derived** — the lowest version with a record — never written, or the
gate fossilises today's history.

**The tag-dependent checks cannot be a `consistency-check` subcheck.** That check is *a pure function of
the committed tree* (`release-handoff.md:298`), tags are not in the tree, and when Gate 12 runs in the
cut **the new tag does not exist yet**. Split: a tree-pure `release-records` subcheck, and a tested
`release-preflight` subcommand for the tag facts, run by the workflow and by the owner beforehand.

## 3. The trigger

`tags: ['[0-9]+.[0-9]+.[0-9]+']` — the bare convention, so a `v`-prefixed tag cannot fire it.
`permissions: {}` at the top; two jobs; **`contents: write` in exactly one**; every action pinned by
commit SHA **resolved from the API, not typed**. `actions: read` on `verify` is one permission beyond
the handoff, because `gh run list` needs it; `verify` still cannot write.

Each precondition **fails the job** rather than publishing, because deleting a Release leaves the tag
behind. CI success waits `RELEASE_CI_WAIT_MINUTES` (30) and **a timeout fails**.

## 4. What was demonstrated, and the one part that cannot be

Every precondition is a tested pure function shown failing alone, gathered from a scratch repository
with throwaway keys: unsigned, lightweight, wrong key, **tampered**, tag ≠ version, `v`-prefixed, off
`main`, wrong fingerprint, no key committed — **and a tag whose own tree carries the key that pins it,
which must not verify against itself.** The tests **fail if `gpg` is missing**, because a skipped
signature test reads as a pass.

**`release-preflight 0.47.0` fails on exactly precondition 5 — no signing key committed — and passes
everything else.** That is the owner's act, and **the first real fire is the only part not demonstrated
in advance.** The release record says what happened rather than claiming it was rehearsed.
