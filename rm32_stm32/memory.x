/* STM32L431KC memory layout for AM32-compatible bootloader.
 *
 * FLASH: 256K total. App starts at 0x08001000 (4K bootloader at 0x08000000),
 * EEPROM at 0x0800F800 (2K). App region = 0x0800F800 - 0x08001000 = 58K.
 * Use 58K to avoid overlapping the EEPROM page.
 *
 * RAM: L431KC has 48K SRAM1 + 16K SRAM2 (parity-checked, separate addr at
 * 0x10000000). Use the contiguous 48K SRAM1 for app — that's 12K more than
 * the previous incorrect 36K value (inherited from G071 config), which was
 * causing stack to bump into BSS under deep ISR chains.
 */
MEMORY
{
  FLASH : ORIGIN = 0x08001000, LENGTH = 58K
  RAM   : ORIGIN = 0x20000000, LENGTH = 48K
}
