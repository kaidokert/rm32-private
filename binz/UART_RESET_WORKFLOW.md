# UART reset race investigation (E468–470)

Installed probe-rs identifies as0.28.0, commit3c10cd38. Its exact
[STM32 ARMv6 sequence](https://github.com/probe-rs/probe-rs/blob/3c10cd38/probe-rs/src/vendor/st/sequences/stm32_armv6.rs)
uses APBENR1 offset0x3c/bit27 for G0 debug enable. debug_core_stop reads the
whole register, clears bit27 in the saved value, then writes the whole word.
debug_device_unlock similarly sets bit27 using read-modify-write. Neither
operation shown in that sequence synchronizes with firmware register updates.

E469 trace shows the teardown read/write after core run operations. E468 MCU
breadcrumbs showed UART bit18 present after HAL then absent after ADC, with
no emitted APBENR1 store in that interval. A stale debugger RMW can overwrite
a concurrent firmware UART-enable update. This is a concrete source-grounded
race mechanism; the exact failed transaction payload was not captured.
Do not claim a UART electrical fault or patch HAL based on this evidence.

## Tested reset alternative

Only after verified outputs-off and with no UART fixture/SWD process active:

```powershell
E:/m/xpack-openocd-0.12.0-6/bin/openocd.exe -s E:/m/xpack-openocd-0.12.0-6/openocd/scripts -f interface/stlink.cfg -c 'transport select dapdirect_swd' -c 'adapter serial 066CFF343433464757233430' -f target/stm32g0x.cfg -c 'init' -c 'reset run' -c 'shutdown'
```

This resets only; it does not flash. The inspected local target config has no
APBENR1 teardown RMW. reset run does not request its reset-init PLL setup.
Afterward run the normal disabled guard fixture with an exclusive output path.
Do not use this command to interrupt a powered campaign or another probe.

E470 fixed three-reset cohort on unchanged A3BD firmware: captures
openocd_reset_guard01.txt,02.txt,03.txt all PASS3 timer faults/18 refusals,
UART responsive without repair, final outputs-off verified. OpenOCD exits
after each reset; serial fixtures close. No motor command or flash in E470.
This is3/3 at this build, not proof of universal reset reliability. It provides
a tested next-reset workflow without more firmware clock workarounds.

Current recorder timing failure remains independent and unresolved. Return
to that work; retain breadcrumbs until the startup workflow is sufficiently
qualified. Never silently delete prior UART failures or clocksnapshots.
