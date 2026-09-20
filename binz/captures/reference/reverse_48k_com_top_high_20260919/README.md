# Frozen qualified binz image — replacement motor, reverse48k, 50%

ELF: `shell-pwm.elf` SHA-256
`0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`.
Target STM32G071RBTx, explicit ST-Link SN `066CFF343433464757233430`.
Built with `--profile release-hybrid --example shell-pwm --no-default-features`
(binz opt-s, thin LTO, codegen-units=1); the four motor ISR-root
soft-arithmetic checks passed. The complete resolved feature closure at build:

```text
bench-adc-latest,bench-adc-tim15,bench-advance-scheduled,bench-average-current,bench-cached-comp,bench-capture-filter,bench-compact-qual,bench-com-peer,bench-com-top-high,bench-current-4000,bench-current-baseline,bench-current-carrier-24k,bench-current-epoch,bench-current-foldback-4000,bench-current-foldback-policy,bench-cycle360,bench-cycle400,bench-cycle450,bench-dma-201,bench-dma-226,bench-dma-fast-start,bench-dma-feedback,bench-dma-guard,bench-dma-peer,bench-driven-dma,bench-driven-entry,bench-driven-handoff,bench-driven-irq,bench-driven-power,bench-driven-reanchor,bench-duty-50,bench-fast-bus-sag,bench-fast-cycle-report,bench-filter-source,bench-guard-install,bench-inline-comp,bench-lean-core,bench-lean-irq,bench-live-control,bench-masked-seed-arm,bench-normal-restart,bench-prestart-dma,bench-pwm-20k,bench-pwm-24k,bench-pwm-roles,bench-quiet-irq-stamp,bench-range300,bench-range310,bench-range320,bench-range330,bench-range335,bench-range340,bench-range345,bench-range350,bench-reverse-48k,bench-reverse-advance22-high,bench-reverse-advance24-override,bench-reverse-advance26-override,bench-reverse-arm20,bench-reverse-blank,bench-reverse-irq-cap64,bench-reverse-phases,bench-running-level-revisit,bench-seed-div12,bench-seed-timing-reanchor,bench-single-core-atomics,bench-speed-event-watch,bench-startup-adc,bench-startup-staircase,bench-static-comp,bench-target-ack-stamp
```

The new control variable is `bench-com-top-high`: below ACKed48% duty,
COMP/COM remain priority0x40 peers; at or above48%, COM rises to priority0.
The comparator snapshots its accepted pre-arm sector before any COM
preemption, preserving event attribution. Fast three-scan5% bus-sag stop,
nominal4A current foldback, nFAULT, tracking and watchdog remain active.

Same-ELF evidence: protected true>=30s target dwells3/3 each at10/25/50%;
normal-start injected-loss recovery3/3 at50%, plus one exact-image recovery
regression each at10/25%; all final outputs off and nFAULT high. This image
contains no COM-lag, ADC-ring, rate-curve or CPU probe. See the top of
`../../../DUTY_50_CAMPAIGN.md` for exact results and capture paths. A later
opt-in source edit may change a rebuilt ELF hash; retain this binary and its
hash as the qualified artifact rather than silently substituting a rebuild.
