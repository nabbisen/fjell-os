OUTPUT_ARCH(riscv)
ENTRY(_start)

/* User-space service load address in Fjell's virtual address space. */
BASE_ADDRESS = 0x00040000;

SECTIONS
{
    . = BASE_ADDRESS;

    .text   : { KEEP(*(.text.init)) *(.text .text.*) }
    .rodata : ALIGN(4) { *(.rodata .rodata.*) }
    .data   : ALIGN(4) { *(.data .data.*) }
    .sdata  : ALIGN(4) { *(.sdata .sdata.*) }

    .bss : ALIGN(4) {
        __bss_start = .;
        *(.bss .bss.*) *(COMMON)
        __bss_end = .;
    }

    /* Stack starts at a FIXED address independent of binary size.
     * 0x80000 is safely above the largest expected service text+data+bss.
     * __stack_top is always 0x90000 so the kernel can use a single constant. */
    . = 0x00080000;
    __stack_bottom = .;
    . += 64K;
    __stack_top = .;         /* always 0x90000 */
}
