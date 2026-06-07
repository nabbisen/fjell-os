# Release Proof Gate Checklist

Before a release that includes proof-gated targets:

```text
- [ ] cargo xtask verus-check --release-required passes.
- [ ] Rust conformance tests pass.
- [ ] Proof target catalog is current.
- [ ] No proof-gated target has undocumented drift.
- [ ] Toolchain version is pinned.
- [ ] Proof outputs are attached to release artifacts.
- [ ] Demoted proof targets are documented with rationale.
```

Proof gates supplement, but do not replace:

```text
- QEMU negative tests
- host unit tests
- fuzz/property tests
- unsafe audit
- release validation drills
```
