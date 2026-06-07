# Verus Reviewer Guide

## 1. Review mindset

Review Verus proofs as security design artifacts, not as decoration.

A proof should make a security boundary easier to trust.

## 2. Reviewer checklist

```text
- Is the target approved for Verus use?
- Does the proof correspond to a real Fjell invariant?
- Is the invariant documented in docs/verification?
- Is there Rust conformance coverage?
- Are assumptions explicit?
- Is the proof small enough to maintain?
- Does the proof avoid modeling unrelated implementation detail?
```

## 3. Red flags

```text
- proof target is low security value
- model does not resemble Rust behavior
- proof removes or weakens QEMU negative tests
- proof requires broad code rewrites
- proof introduces unstable toolchain requirements into normal builds
- proof is accepted without conformance tests
```

## 4. Approval outcomes

A review should end with one of:

```text
Approved:
  proof is useful and connected

Approved with follow-up:
  proof is useful but documentation or conformance must improve

Rejected:
  proof target or model is not worth the productivity cost

Deferred:
  target may be valuable later but not now
```
