# Reverse 48 kHz, high-duty advance 22 — replacement motor

Frozen `shell-pwm.elf` SHA256: `B9B5F6B06570B459DAEA54FEFB1C28E714916CF1AE2DE9745571ACCD462E8A23`.

Built `--profile release-hybrid --target thumbv6m-none-eabi --example shell-pwm --no-default-features` with features:
`bench-reverse-advance22-high,bench-compact-qual,bench-current-foldback-4000,bench-dma-peer,bench-dma-fast-start,bench-driven-dma,bench-fast-bus-sag,bench-fast-cycle-report,bench-guard-install,bench-masked-seed-arm,bench-quiet-irq-stamp,bench-reverse-48k,bench-reverse-arm20,bench-normal-restart,bench-running-level-revisit,bench-seed-div12,bench-seed-timing-reanchor,bench-single-core-atomics,bench-speed-event-watch,bench-startup-adc`.

Four motor ISR-root soft-arithmetic audits passed. Installed on explicit G071, disabled guard/role/duty preflights passed. Same image: 48% holds 3×30s + 1×60s clean; ordinary restart to48% 3/3; 49% and50% each ACKed but fast-sag stopped before deadline. The image is currently installed, outputs OFF, COM41 closed. See `DUTY_50_CAMPAIGN.md` and `captures/reverse_direction_2026-09-19_advance22high_*`.
