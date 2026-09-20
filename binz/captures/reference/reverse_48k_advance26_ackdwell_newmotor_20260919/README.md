# Reverse 48 kHz, effective high-duty advance 26, target-dwell witness

Frozen release `shell-pwm.elf` SHA-256:
`79BDA4C57DCD8D097B25E084F96C1812582D38232768F25D5307332E36EC0CA9`.

This binz-only image keeps the protected replacement-motor reverse configuration:
4 A signed-average adaptive current foldback, three coherent 226 us scans
below 95% of segment-start bus for the fast shutdown, nFAULT, tracking and
watchdog stops. Physical PSU limit was 4.5 A. It uses low-duty advance 20 and
the post-COM binz override to effective 26 at duty >=35%; the override's first
powered call is witnessed post-run. `bench-target-ack-stamp` records the MCU
powered timestamp for guarded ACKs at 10%, 25%, and 50%, for actual target-dwell
qualification. All telemetry is dumped after shutdown, not over live UART.

Explicit STM32G071 device 0x460 flash/verify and reset passed. With the motor
disabled, guard 3/18, roledu100 6/6, duty 6/6 and output/FAULT readbacks passed.
The four motor ISR-root soft-arithmetic audit and target-dwell host tests passed.

Do not infer a target hold from `--ms` alone: it sets the total powered window,
including startup and live-duty ramp. Use `--qualify-hold --target-hold-ms` to
require the MCU ACK-to-stop duration. Individual retained captures and the
campaign report contain the measured results; no pre-stamp result is a verified
30-second target dwell.
