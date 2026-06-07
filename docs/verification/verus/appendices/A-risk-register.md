# Verus Adoption Risk Register

## R1: Proof maintenance slows development

Mitigation:

```text
- keep proof targets small
- use tiers
- do not gate ordinary PRs
```

## R2: Proofs drift from Rust implementation

Mitigation:

```text
- require conformance tests
- use drift review checklist
- record modeled Rust modules
```

## R3: CI becomes fragile

Mitigation:

```text
- pin Verus toolchain
- start with optional CI
- isolate experimental targets
```

## R4: Proof theater

Mitigation:

```text
- every proof must connect to Rust tests or release gates
- reviewers may reject low-value targets
```

## R5: Developers avoid verified modules

Mitigation:

```text
- keep verified modules small
- write clear documentation
- allow demotion if proof cost becomes excessive
```

## R6: Verus toolchain limitations block important fixes

Mitigation:

```text
- retain Rust implementation as source of truth
- allow temporary proof gate suspension with architect approval
- never make Verus a kernel build dependency initially
```
