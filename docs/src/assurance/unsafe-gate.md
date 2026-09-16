# The Unsafe Gate

Every `unsafe` block in this workspace must say why it is sound, in a form a
tool can check. `fjell-unsafe-audit` is that tool, and it runs as **Gate 2** of
the release rehearsal and as a tier of `cargo xtask test-all`.

## What it checks

An `unsafe` site passes when a `// SAFETY:` comment appears within a few lines
above it **and** names a category:

```rust
// SAFETY: category=mmio-access; the UART's base address comes from the
// device tree and is mapped by the caller before this runs.
unsafe { core::ptr::write_volatile(base, byte) }
```

The categories are a closed set — `raw-pointer-deref`, `page-table-mutation`,
`csr-asm`, `mmio-access`, `phys-id-map-assumption`, `kernel-global-mutable`,
`user-copy`. A comment with no `category=` tag, or one naming a category that
does not exist, fails the same way a missing comment does.

## What it does not check

**It does not verify that the justification is true.** It checks that a
justification exists and is classified. A comment asserting something false
passes this gate; that is a known limitation of its family, not an oversight,
and it is why soundness work (for example RFC-0.32-002) removes `unsafe`
rather than annotating it.

## Running it

```bash
cargo run -p fjell-unsafe-audit -- --workspace . --check   # exit 0 or 1
cargo run -p fjell-unsafe-audit -- --workspace . --json    # the inventory
```

The `--check` form is what CI and Gate 2 run. The `--json` form prints the
current inventory — every site, its file, and its category.

**The counts live in the tool's output, not in this page.** A page that
records how many `unsafe` sites exist is wrong the next time one is added or
removed, and nothing regenerates it. The page this one replaces said "Total
unsafe sites: 0" while the audit reported 277; it was generated once, at
v0.6.0, and never again (RFC-0.32-003 D21, erratum E-050).
