# Reverse-only 40 kHz BEMF carrier A/B (2026-09-19)

Frozen ELF SHA256
`22AEFD4CCAB2BC42971409A6740F751A103DF4626ED28B4B7FBDE5BAD7EE776A`.
Same corrected reverse lean control/protection feature closure as the32k
reference, changing only `bench-reverse-32k` to `bench-reverse-40k`.
Forced startup remains10kHz; BEMF carrier ARR1599/40000Hz. The226us
ADC scan cadence covers25 distinct positions twice in a50-scan block;
maximum uncovered phase arc64 timer ticks=1us. Active4A signed-average
current foldback/terminal, fast5%-three-scan bus stop, absolute bus,
nFAULT, tracking, IRQ-rate, watchdog and normal restart remain. No ISR
diagnostic recorder. At10% BEMF duty, ideal post-deadtime pulse is
134/64=2.09us, NOT a proven gate conduction width; initial startup/
handoff is therefore a hard test gate before a high-duty climb.

Release-hybrid opt-s/thinLTO/codegen1 text123304/data1200/bss17136.
All four motor ISR-root soft-arithmetic forbids PASS. Actual carrier/role
host tests24/24, guarded live writer5/5, role/ADC host checks11/11 PASS.
This is an exploratory A/B, not motor-qualified when frozen.

Powered results: separate15s10% startup/BEMF window passed; a later
smooth ramp ACKed45%, then fast bus sag reason26 stopped at powered
39.998459s (approximately3s at45 by host wall clock). Current foldback0,
phase rails0, tracking0, nFAULT high, outputs off. 40k did not remove
the upper transient. This image was retired; exact reverse32k lean image
was restored and disabled preflights passed. Retained captures live two
directories up as `reverse_direction_2026-09-19_40k_10_gate.txt` and
`reverse_direction_2026-09-19_40k_45_fastbus.txt`.
