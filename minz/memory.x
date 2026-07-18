MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 256K  /* L431KC = 256K; the old 64K was the AM32-64K-part-era assumption */
  RAM   : ORIGIN = 0x20000000, LENGTH = 48K
  /* SRAM2: the stack lives here alone. Statics keep all 48K of RAM,
     and a stack overflow walks off the BOTTOM of SRAM2 into unmapped
     space = a clean bus fault the HardFault handler PRINTS - instead
     of silently corrupting .bss (the J-armed IWDG-reboot class:
     RCC_CSR read 0x3C000602, IWDGRSTF set, both handlers mute =
     LOCKUP from a fault inside the fault handler). */
  RAM2  : ORIGIN = 0x10000000, LENGTH = 16K
}

_stack_start = ORIGIN(RAM2) + LENGTH(RAM2);
_stack_end = ORIGIN(RAM2);
