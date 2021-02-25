MEMORY
{
  /* NOTE K = KiBi = 1024 bytes */
  BOOT_LOADER : ORIGIN = 0x10000000, LENGTH = 0x100
  FLASH : ORIGIN = 0x10000100, LENGTH = 2048K
  RAM : ORIGIN = 0x20000000, LENGTH = 264K
}

SECTIONS {
  .boot_loader ORIGIN(BOOT_LOADER) :
  {
    KEEP(*(.boot_loader*));
  } > BOOT_LOADER
} INSERT BEFORE .text;

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
_core1_stack_start = _stack_start - 4096;
