# E800 lean 3.5 A qualification candidate

Not flashed or motor-qualified. Board remains E797/2.5A/off pending operator
confirmation that the physical PSU current limit is3.5A.

- ELF SHA256: `3E6BD0C7217F95BFC4EBD1947DC7A33E6EAEDD142CAB66342B56A56A504C2CD9`
- release profile: opt-level=s, thin LTO, codegen-units=1
- current policy: explicit nominal3.5A, repeatable1% foreground ceiling
- control:24kHz carrier,226us ADC cadence,advance18,AM32 foreground filter map
- linked lean selectors: `bench-lean-core`, `bench-lean-irq`
- deliberately omitted: `bench-average-diagnostic`, `bench-fast-cycle-report`,
  `bench-event100` and all per-event/tail recorder features
- M0 audit: no forbidden helper reachable by direct emitted calls from
  DMA1_CHANNEL1, ADC_COMP, TIM16 or TIM6_DAC_LPTIM1
- size: text120396,data1180,bss23120 (diagnostic E799 is text121224,bss23152)

The generic post-run IRQTRACE header formatter remains linked, but lean IRQ
selection supplies no per-IRQ rows. Runtime qualification must still require
the existing strict lean verifier: zero populated recorder rows, valid compact
frame CRCs, electrical guards, deadline outcome, freshness, no veto and final
safing. This artifact is not evidence of those runtime properties until run.

