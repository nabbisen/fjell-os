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

    .rodata : ALIGN(4) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    }

    .data : ALIGN(4) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    }

    .bss : ALIGN(4) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* 4 MiB kernel stack. */
    . = ALIGN(4096);
    __stack_bottom = .;
    . += 4M;
    __stack_top = .;
}
