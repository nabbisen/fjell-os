# RFC 018: W^X three-region via 4 KiB aligned linker sections

**RFC ID:** 018  
**Status:** Implemented  
**Affects:** `crates/fjell-kernel/link.ld`, `crates/fjell-kernel/src/main.rs`

## Problem (H-02 from v0.0.10 review)

RFC 009 implemented a two-region W^X split (.text=R|X, rest=R|W).  A three-region
split (.text=R|X, .rodata=R, rest=R|W) was attempted but the kernel hung because
`.rodata` and `.data` shared the same 4 KiB page (`.rodata ALIGN(4)` is not a page
boundary).

The architect's diagnosis: `.rodata` mapped R-only also includes the start of `.data`
on the same page, making the first `.data` write fault.

## Proposed fix

### 1. 4 KiB page-align sections in link.ld

Add `. = ALIGN(4096);` between each section and include RISC-V orphan sections
(`.srodata`, `.sdata`, `.sbss`, `.got`):

```ld
.text   : { __text_start = .; KEEP(*(.text.init)) *(.text .text.*) __text_end = .; }
. = ALIGN(4096);
.rodata : { __rodata_start = .; *(.rodata .rodata.*) *(.srodata .srodata.*) __rodata_end = .; }
. = ALIGN(4096);
.data   : { __data_start = .; *(.data .data.*) *(.sdata .sdata.*) *(.got .got.*) __data_end = .; }
. = ALIGN(4096);
.bss    : { __bss_start = .; *(.bss .bss.*) *(.sbss .sbss.*) *(COMMON) __bss_end = .; }
```

### 2. Three-region map in kmain

```rust
let perms = if va < text_end   { VmPerms::R | VmPerms::X }  // .text
       else if va < rodata_end { VmPerms::R }               // .rodata: read-only
       else                    { VmPerms::R | VmPerms::W }; // .data/.bss/stack
```

## Test

`cargo xtask qemu-test m7` must pass.  Verify `.rodata` and `.data` are on separate
pages using `riscv64-unknown-elf-nm`.
