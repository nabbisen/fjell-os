# Verus Proof Authoring Guide

## 1. Goal

The goal is to write proofs that clarify Fjell's critical invariants without turning development into proof maintenance.

## 2. Proof style

Prefer:

```text
- small pure functions
- finite state transitions
- simple inductive properties
- explicit preconditions and postconditions
- proof functions close to the model
```

Avoid:

```text
- large proofs spanning many modules
- proof code mirroring hardware effects
- proof-only abstractions nobody can map to Rust
- clever encodings that only one person understands
```

## 3. Model shape

A useful Verus model should have these parts:

```text
1. data model
2. transition function
3. invariant predicate
4. lemma/proof that transition preserves invariant
5. conformance cases for Rust tests
```

## 4. Example structure

```rust
verus! {
    pub struct ModelState { /* fields */ }

    pub open spec fn invariant(s: ModelState) -> bool {
        /* predicate */
    }

    pub open spec fn transition(s: ModelState, input: Input) -> ModelState {
        /* pure transition */
    }

    proof fn transition_preserves_invariant(s: ModelState, input: Input)
        requires invariant(s)
        ensures invariant(transition(s, input))
    {
        /* proof */
    }
}
```

## 5. Naming convention

Use names that map directly to Rust concepts.

```text
Good:
  cap_mint_does_not_amplify_rights
  revoked_lease_rejects_use
  mirror_selection_is_deterministic

Bad:
  theorem_1
  safe_state
  abstract_monotone_property
```

## 6. Proof acceptance rules

A proof is accepted only if:

```text
- it compiles under the pinned Verus toolchain
- it has an associated Rust conformance test or explicit reason why not
- it is listed in the proof target catalog
- it has a reviewer sign-off
- it is small enough to maintain
```

## 7. Documentation requirement

Every proof target must include a short Markdown note:

```text
- What invariant is proved?
- Which Rust module must conform?
- Which bug class is prevented?
- Which assumptions remain outside the proof?
```
