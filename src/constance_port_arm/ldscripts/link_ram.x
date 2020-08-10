/* This will be provided by the user (see `memory.x`) or by a Board Support Crate */
INCLUDE memory.x
ENTRY(start);

SECTIONS
{
  .text :
  {
    *(.text .text.*);
    . = ALIGN(4);
    __etext = .;
  } > RAM

  .rodata __etext : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > RAM

  .data : ALIGN(4)
  {
    *(.data .data.*);
    . = ALIGN(4);
  } > RAM

  /* LMA of .data */
  __sidata = LOADADDR(.data);

  /* ### .bss */
  .bss : ALIGN(4)
  {
    __sbss = .;
    *(.bss .bss.*);
    . = ALIGN(4);
    __ebss = .;
  } > RAM

  /* Initial, IRQ, and Abort stack */
  . += 4096;
  . = ALIGN(8);
  _stack_start = .;

  /* Place the heap right after `.uninit` */
  . = ALIGN(4);
  __sheap = .;

  /* ## .got */
  /* Dynamic relocations are unsupported. This section is only used to detect relocatable code in
     the input files and raise an error if relocatable code is found */
  .got (NOLOAD) :
  {
    KEEP(*(.got .got.*));
  }

  /* ## Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx);
    *(.ARM.exidx.*);
    *(.ARM.extab.*);
  }
}