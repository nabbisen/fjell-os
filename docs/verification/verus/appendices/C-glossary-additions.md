# Glossary Additions

## Verus target

A Fjell module or invariant selected for Verus modeling or proof.

## Proof gate

A CI or release condition requiring a Verus proof to pass.

## Conformance test

A Rust test that checks production implementation behavior against a Verus-modeled invariant or generated vector.

## Proof drift

A mismatch between a Verus model/proof and the Rust implementation behavior.

## Tier 1 target

Verus model only; production Rust is tested against model cases.

## Tier 2 target

Rust implementation is shaped to closely match the Verus model.

## Tier 3 target

Release-critical proof-gated target.

## Proof theater

A formal artifact that looks impressive but does not constrain production behavior, review, or release decisions.
