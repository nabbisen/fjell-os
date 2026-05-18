# RFC 009: W^X enforcement for kernel page table

**RFC ID:** 009  
**Status:** Implemented  
_(was: Accepted, deferred to M7.1)  
**Affects:** `crates/fjell-kernel/src/main.rs` (kernel identity map setup)

## Problem (H-03)

Kernel text, rodata, data, BSS, stack are all mapped `R | W | X`.
This violates the W^X (Write XOR Execute) principle:
- Kernel text is writable (exploit can patch it).
- BSS/stack is executable (shellcode injection).

## Proposed fix

Map kernel sections with appropriate permissions:

```text
.text    → R | X
.rodata  → R
.data    → R | W
.bss     → R | W  (includes DMA_BUF, statics)
stack    → R | W
MMIO     → R | W  (no X)
```

Requires linker script to export section boundaries:
```
PROVIDE(__text_start  = ADDR(.text));
PROVIDE(__text_end    = ADDR(.text) + SIZEOF(.text));
PROVIDE(__rodata_end  = ADDR(.rodata) + SIZEOF(.rodata));
PROVIDE(__data_end    = ADDR(.data)   + SIZEOF(.data));
```

## Defer condition

Requires linker script changes and section boundary constants.
Large but low-risk mechanical change.  Defer to M7.1 hardening sprint.
