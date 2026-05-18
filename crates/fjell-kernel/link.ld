OUTPUT_ARCH(riscv)
ENTRY(_start)

/*
 * QEMU virt machine RAM start address.
 * The kernel image is loaded here by QEMU when passed with -kernel.
 */
BASE_ADDRESS = 0x80000000;

SECTIONS
{
    . = BASE_ADDRESS;

    /* Boot code must be at the very beginning of the image. */
    .text : {
        __text_start = .;
        KEEP(*(.text.init))
        *(.text .text.*)
        __text_end = .;
    }

    /*
     * RFC 018: align to 4 KiB page boundary before .rodata so that the
     * .text / .rodata boundary coincides with a page boundary.  This allows
     * W^X three-region mapping (.text=R|X, .rodata=R, rest=R|W) without a
     * page containing both executable code and writable data.
     */
    . = ALIGN(4096);
    .rodata : {
        __rodata_start = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)   /* RISC-V small-data read-only */
        __rodata_end = .;
    }

    . = ALIGN(4096);
    .data : {
        __data_start = .;
        *(.data .data.*)
        *(.sdata .sdata.*)       /* RISC-V small-data */
        *(.got .got.*)           /* GOT (position-independent data) */
        __data_end = .;
    }

    . = ALIGN(4096);
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)         /* RISC-V small BSS */
        *(COMMON)
        __bss_end = .;
    }

    /* 4 MiB kernel stack. */
    . = ALIGN(4096);
    __stack_bottom = .;
    . += 4M;
    __stack_top = .;
}
