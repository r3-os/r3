MEMORY
{
  /* On-chip RAM. We are loading everything to this region. */
  RAM : ORIGIN = 0x20000000, LENGTH = 10240K
}

ENTRY(reset_handler);

SECTIONS
{
  /* The beginning of the on-chip RAM region (0x20000000) can be remapped to
     0x00000000, which is the low vector table address (SCTLR.V == 0).  */
  .vector_table :
  {
    KEEP(*(.vector_table));
  } > RAM

  /* ### .text */
  .text :
  {
    *(.text .text.*);
    . = ALIGN(4);
    __etext = .;
  } > RAM

  /* ### .rodata */
  .rodata __etext : ALIGN(4)
  {
    *(.rodata .rodata.*);

    /* 4-byte align the end (VMA) of this section.
       This is required by LLD to ensure the LMA of the following .data
       section will have the correct alignment. */
    . = ALIGN(4);
    __erodata = .;
  } > RAM

  /* ## Sections in RAM */
  /* ### .data */
  .data : AT(__erodata) ALIGN(4)
  {
    . = ALIGN(4);
    __sdata = .;
    *(.data .data.*);
    . = ALIGN(4); /* 4-byte align the end (VMA) of this section */
    __edata = .;
  } > RAM

  /* LMA of .data */
  __sidata = LOADADDR(.data);

  /* ### .bss */
  .bss : ALIGN(4)
  {
    . = ALIGN(4);
    __sbss = .;
    *(.bss .bss.*);
    . = ALIGN(4); /* 4-byte align the end (VMA) of this section */
    __ebss = .;
  } > RAM

  /* Initial, IRQ, and Abort stack */
  . += 4096;
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