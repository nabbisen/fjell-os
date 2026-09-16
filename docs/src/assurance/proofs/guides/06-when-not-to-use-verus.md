# When Not to Use Verus

## 1. Principle

Verus is powerful, but using it in the wrong place harms productivity.

## 2. Do not use Verus when logic is unstable

If a module is still changing shape every few weeks, proof maintenance will dominate development.

Use ordinary tests until the invariant stabilizes.

## 3. Do not use Verus for hardware-effect-heavy code first

Avoid first-stage Verus use in:

```text
- MMIO ordering
- DMA cleanup
- trap entry
- page table mutation
- interrupt controller interaction
```

These need QEMU tests, hardware validation, unsafe audits, and careful review first.

## 4. Do not use Verus for low-risk tooling

Avoid Verus for:

```text
- doc generators
- CLI argument parsing
- output formatting
- markdown rendering
- benchmark harnesses
```

## 5. Do not use Verus when proof cannot connect to Rust

If no conformance test or generated vector can connect the proof to shipped code, defer.

## 6. Do not use Verus as a release badge

A proof target must have engineering value. It must not be added only because “formal verification” sounds impressive.
