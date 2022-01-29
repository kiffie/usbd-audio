/*************************************************************************
 * Processor-specific object file.
 *************************************************************************/

/*************************************************************************
 * Symbols used for interrupt-vector table generation
 *************************************************************************/
PROVIDE(_vector_spacing = 0x0001);
PROVIDE(_ebase_address = 0x9D000000);  /* first 4 KiB of program flash */


MEMORY
{
    boot_flash          (rx)    : ORIGIN = 0xBFC00000, LENGTH = 0xc00
    program_flash       (rx)    : ORIGIN = 0x9D000000, LENGTH = 0x40000
    sram                (w!x)   : ORIGIN = 0x80000000, LENGTH = 0x8000
    configsfrs                  : ORIGIN = 0xBFC00BF0, LENGTH = 0x10
}

REGION_ALIAS("reset_mem", boot_flash)
REGION_ALIAS("exception_mem", program_flash)
REGION_ALIAS("program_mem", program_flash)
REGION_ALIAS("data_mem", sram)

/*SECTIONS
{
  /DISCARD/ : {
    *(.configsfrs)
  }
}*/

/*************************************************************************
 * common part for all PIC32 devices
 *************************************************************************/
INPUT("device.x") /* interrupt vector symbols from Peripheral Access Crate */
INPUT("pic32_common.ld")
