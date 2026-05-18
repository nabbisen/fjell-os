OUTPUT_ARCH(riscv)
ENTRY(_start)

/* User-space service load address in Fjell's virtual address space. */
BASE_ADDRESS = 0x00040000;

SECTIONS
{
    . = BASE_ADDRESS;

    .text : {
        KEEP(*(.text.init))
        *(.text .text.*)
    }

    .rodata : ALIGN(4) { *(.rodata .rodata.*) }
    .data   : ALIGN(4) { *(.data .data.*) }
    .sdata  : ALIGN(4) { *(.sdata .sdata.*) }

    .bss : ALIGN(4) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    . = ALIGN(4096);
    __stack_bottom = .;
    . += 64K;         /* 64 KiB service stack */
    __stack_top = .;
}
