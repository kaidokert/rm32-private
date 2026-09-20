# Reverse32k fixed20 rate-census diagnostic — 2026-09-19

ELF SHA256: `DBE640973129E056AD76AAF3C62B3FBB70E111969252C1A958F2E58F80EC89AC`.
Frozen from `target/thumbv6m-none-eabi/release-hybrid/examples/shell-pwm`.
Release-hybrid is opt `s`, thin LTO, one codegen unit. Text125908,
data1200, bss17192. Four motor ISR-root soft-arithmetic audits passed.

Feature closure: `bench-advance-20,bench-compact-qual,bench-current-foldback-4000,bench-dma-peer,bench-dma-fast-start,bench-driven-dma,bench-fast-bus-sag,bench-fast-cycle-report,bench-guard-install,bench-masked-seed-arm,bench-quiet-irq-stamp,bench-reverse-32k,bench-reverse-arm20,bench-reverse-flat-start,bench-running-level-revisit,bench-seed-div12,bench-seed-timing-reanchor,bench-single-core-atomics,bench-speed-event-watch,bench-startup-adc,bench-rate-census`.

This differs from the frozen lean control only by opt-in counters. The
rate census activates after a live 40% duty publication and reports
post-stop; it does not change the fast bus/current/nFAULT/tracking/watchdog
protection decisions. Its ISR cost is not qualified as zero, so this ELF
cannot qualify the lean operating envelope. The 10% low gate and 40%
70-second hold passed; a stepped 42.5% run passed and a stepped 45% run
stopped on fast bus sag. See `DUTY_50_CAMPAIGN.md` and the three exact
`captures/reverse_direction_2026-09-19_32k_rate*.txt` transcripts.

The G071 was restored afterward to exact fixed20 lean SHA `06B1C1D6...`;
this diagnostic is not installed.
