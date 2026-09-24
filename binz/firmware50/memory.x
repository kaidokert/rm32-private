/* STM32G071RB on NUCLEO-G071RB: 128 KiB flash, 36 KiB RAM.
   Linked at the flash base — this image is SWD-flashed directly by probe-rs,
   there is no bootloader in front of it on this bench. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 128K
  RAM   : ORIGIN = 0x20000000, LENGTH = 36K
}
