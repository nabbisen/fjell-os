# Verus Adoption Anti-Patterns

## 1. Proving the wrong thing

A proof that does not match the real Rust behavior is worse than no proof.

## 2. Making Verus a badge

Do not add proof files just to claim formal verification.

## 3. Verifying unstable APIs

If the API is still changing, wait.

## 4. Replacing negative tests

Proofs do not replace QEMU negative tests.

## 5. Modeling hardware too early

Do not start with DMA, MMIO ordering, or trap assembly.

## 6. Large proof monoliths

A large proof across many modules is hard to review and easy to abandon.

## 7. Hidden assumptions

Every proof assumption must be written down.

## 8. Blocking all contributors

Most contributors should not need Verus knowledge to work on ordinary Fjell services.
