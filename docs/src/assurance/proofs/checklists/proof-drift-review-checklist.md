# Proof Drift Review Checklist

Run this when Rust implementation changes without corresponding proof changes.

```text
- [ ] Does the changed Rust module appear in verification/verus/verus-targets.toml?
- [ ] Does the change alter behavior modeled by Verus?
- [ ] If yes, was the model updated?
- [ ] If no, did reviewer document why behavior is unchanged?
- [ ] Were conformance tests rerun?
- [ ] Were new edge cases added if needed?
```

Drift outcome:

```text
No drift:
  Behavior unchanged or outside model scope.

Documented drift:
  Model intentionally does not cover changed behavior.

Invalid drift:
  Model must be updated before merge.
```
