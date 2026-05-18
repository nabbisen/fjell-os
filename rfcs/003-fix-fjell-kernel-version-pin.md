# RFC 003: Fix fjell-kernel Cargo.toml version pin

**RFC ID:** 003  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/Cargo.toml`

---

## 1. Problem

`crates/fjell-kernel/Cargo.toml` has a hard-coded version:

```toml
# crates/fjell-kernel/Cargo.toml — current (incorrect)
[package]
name    = "fjell-kernel"
version = "0.0.3"          # ← pinned; workspace is at 0.0.7
```

All other 34 crates in the workspace use `version.workspace = true`.

**Observable symptom:** The build log shows:

```
Compiling fjell-kernel v0.0.3 (...)
```

while `cargo metadata` reports the workspace version as `0.0.7`.  This causes:

- `cargo publish` would publish `fjell-kernel` at `0.0.3` while the lockfile records
  cross-crate dependencies at `0.0.7`, making the published artifact unusable.
- Version tracking in audit trails and SBOM tooling will misreport the kernel version.
- `CHANGELOG.md` references `fjell-kernel` implicitly through workspace releases; the
  mismatch creates confusion during incident analysis.

---

## 2. Proposed fix

```toml
# crates/fjell-kernel/Cargo.toml — fixed
[package]
name              = "fjell-kernel"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

---

## 3. Rationale

Every crate in the workspace should track the workspace version unless there is a
deliberate reason to diverge (e.g., a stable library crate with an independent semver
commitment).  `fjell-kernel` is not independently published; there is no reason to pin
it.

The pin was introduced when the workspace was bootstrapped at M0/M1 and the kernel
crate was given a manual version that was never updated as workspace bumps were applied
to `Cargo.toml`.

---

## 4. Impact

| Crate | Change |
|---|---|
| `fjell-kernel/Cargo.toml` | `version = "0.0.3"` → `version.workspace = true` |

No source code change.  No kernel ABI change.  No smoke test impact.

---

## 5. Test plan

1. After the fix, `cargo metadata --format-version 1 | python3 -c "import sys,json; pkgs=[p for p in json.load(sys.stdin)['packages'] if p['name']=='fjell-kernel']; print(pkgs[0]['version'])"` must print `0.0.7` (or whatever the current workspace version is).
2. `cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release` build log must show `Compiling fjell-kernel v0.0.7`.
3. `cargo xtask qemu-test m7` must still pass.

---

## 6. Implementation notes

- No code change is required; this is purely a `Cargo.toml` metadata fix.
- Future workspace version bumps will automatically propagate to `fjell-kernel` after
  this fix, which is the desired behaviour.
