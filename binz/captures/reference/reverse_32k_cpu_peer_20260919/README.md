# Reverse32k matched-control aggregate CPU diagnostic (retired)

ELF SHA256 `C2237838979EF5D24F57F4D63225B4C76C064B7B811AF82143417618A4D795FC`.
Feature-fingerprint comparison to the original proven reverse control
fingerprint confirms only `bench-reverse-32k` plus the expected CPU
aggregate closure (`bench-cpu-aggregate/roots/timing/union`), minus
the two old postrun diagnostics (`bench-adc-latest-fault-frame` and
`bench-reverse-seed-stage`). DMA remains a COMP/COM peer at priority64;
TIM6 stays0. No controller/electrical threshold change from frozen
reverse32k fixed20 lean. The CPU meter itself is observer-affected
and cannot qualify the lean envelope.

`release-hybrid` opt-s/thinLTO/codegen1, text128748/data1200/
bss17212; flash headroom1124 bytes. Linker audit exits0 for all
four motor ISR roots. Exact lean-current rebuild has identical `.text`
and `.vector_table` to frozen SHA06B1C1D6...; ELF/rodata hashes differ,
so this remains a diagnostic comparison, not binary identity.
It was flashed/verified and passed disabled `cpucheck`, guard3/18,
reverse role/duty/off preflights. Its first ordinary10% startup
stopped during handoff (DRIVENENTRY refusal4/coast_stop8) before any
running BEMF window. CPUUNION elapsed28us is not an occupancy result.
No upper-duty run and no retry. See
`../../reverse_direction_2026-09-19_32k_cpu_peer_low_refusal.txt`.
The meter ran from the first powered guard tick, so observer effect is
plausible but not proven. A running-only probe supersedes this image.
