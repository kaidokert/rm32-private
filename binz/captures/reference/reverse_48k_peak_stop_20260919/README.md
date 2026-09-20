# Reverse 48.012 kHz carrier / first-near-rail protective A/B (lower gate refused)

Frozen `shell-pwm.elf` SHA256
`C6252F0FC400B129B2089171178011BA42DBFA3017F4AB312CD4E7542361CB0B`.
Release-hybrid (`binz` opt-s, thin LTO, codegen1); text125172/data1200/
bss17236. Four exact motor ISR-root soft-arithmetic audits pass.
`carrier_profile` host tests7/7 pass, including exact 48,012Hz timer
geometry, all1333 phase ticks into32 exact bins, and 50 unique
226us-ADC sample phases with <=41 timer-tick uncovered arc.

Only functional difference from frozen reverse32k peak-stop image is
`bench-reverse-48k` in place of `bench-reverse-32k`; fingerprint
feature diff confirms that pair alone. Startup remains10kHz. At
fixed duty the nominal PWM high interval is about one-third shorter
than32k, not half. The same 4A signed-average actuator, fast5%/
three-scan bus stop, nFAULT, tracking/watchdog, first >=1900-count
phase stop and latest-frame fault snapshot remain. This is a carrier
A/B diagnostic, not qualification or a proven ripple remedy.

Motor gates: explicit G071 flash/verify/reset, disabled guard3/18,
roledu1006/6 at ARR1332, duty6/6, p/i off/nFAULT high PASS;
ordinary10%/15s protected hold PASS. Both attempts to begin the live
40% ramp stopped `POWERPATH9 HostAbort` on the first host command,
including one after a five-second quiet interval. No 40% ACK and no
upper-duty/carrier-effect evidence. Exact byte/IRQ ownership was not
retained; see `captures/reverse_direction_2026-09-19_48k_hostabort.txt`.
Exact32k lean SHA06B1C1D6... restored/verified/reset, p/i off/
nFAULT high, UART closed. This image is retired pending a targeted
host-abort state probe; do not treat the two refusals as a motor wall.
