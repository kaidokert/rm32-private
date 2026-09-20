# Reverse-only 32 kHz BEMF carrier A/B (2026-09-19)

Frozen ELF SHA256
`06B1C1D6940816D2BC4F804BE199944EE9E4BF8C8DCBE60189F1F7FF9C9BB988`.
`release-hybrid` opt-s/thin-LTO/codegen1; text124760/data1200/bss17144.
Exact corrected reverse lean feature closure plus `bench-reverse-32k`.
Startup remains at10kHz; only the BEMF powered carrier changes from
24006Hz/ARR2665 to32000Hz/ARR1999. `bench-current-carrier-24k` remains in
the dependency closure to retain226us ADC scanning but the explicit reverse
32k override takes precedence. Active4A signed-average current
foldback/terminal stop, 5%-three-coherent-scan fast bus stop, absolute bus,
nFAULT, tracking, IRQ-rate, watchdog and normal restart are retained. No
origin/interval/bus/CPU diagnostic recorder. Old shaft direction forbidden.

The 226us ADC cadence covers50 distinct phases of the 32k carrier per
current block; maximum uncovered arc80 timer ticks=1.25us. Carrier profile
host tests5/5, live role tests22/22, live-duty writer5/5, role/ADC host
checks6/6, and all four motor ISR-root soft-math checks PASS. At10% BEMF
entry, nominal ON after26 deadtime ticks is174/64=2.72us; startup/transfer
reliability must be measured rather than assumed. This is an exploratory
control A/B, not qualified at any new rung when frozen.

TI's DRV8304 datasheet recommends applied PWM up to200kHz and gives a45kHz
design example, so32k is inside the driver-level frequency range; this does
not establish the board/motor's thermal or control margin:
https://www.ti.com/lit/ds/symlink/drv8304.pdf
