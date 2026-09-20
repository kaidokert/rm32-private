# E722 — timed startup feedback integration

The startup ADC producer is now connected, not just staged as an unused API.
`bench-startup-adc` selects TIM15 and the latest-frame cache. Sine capture and
driven feedback consume that cache without software ADC transactions. DMA
consumes every coherent scan exactly once for average-current protection.
Repeated cache reads never add average samples. Driven reads retain original
acquisition timestamps and skip duplicate frames. The validated sine/driven
transfer preserves the independent ADC producer; faults revoke it immediately.
Foreground coast processing finishes bounded ADC restoration before software
reads. Startup VSENC and neutral are unavailable, explicitly marked validity0.

Installed image: `captures/reference/startup_dma_722b/shell-pwm.elf`, SHA256
`524226caaa8c6eb2c899a40f3ca4cc01151f071f84d3bfb718c50f9c5b138b54`.
Release size optimization/thin LTO build and TIM16 arithmetic-helper gate pass.
25 pure cache/current/clock tests pass; these are not full ISR timing evidence.
Predecessor 722 (8d360990...) passed its own four baseline cases, three timer
faults, eighteen post-stop refusals, and missing-average-configuration refusal.
722b adds retained operational startup fault code and correct ADC-order metadata.
Both flash and OpenOCD reset commands completed successfully.

## Powered evidence

- `captures/startup_dma_722_ramp200.txt`: stopped before BEMF at 1.276699s.
  Legacy reason4 was ambiguous on this first image.
- `captures/startup_dma_722b_ramp200.txt`: stopped before BEMF at 1.276783s,
  retained startup fault25 after 6350 DMA scans at 201us cadence. No higher
  throttle acknowledgment. Capture SHA256
  `cb2d8956dc82202553ad693b84a8bb1573eb2bf0097006a80b569dfd4e4a85c5`.
- `captures/startup_dma_722b_direct200.txt`: same image, firmware ramp from
  100 to 200Hz instead of the host-paced 50-to-200 startup. Stopped at 0.824843s,
  fault25 after 4100 DMA scans, before BEMF or higher throttle acknowledgment.

All three saved drive/coast snapshot streams decode with valid record counts
and ordering. Final gate/ENABLE/MOE/CCR readbacks are off; host sessions closed.
These are failed exploratory starts, not sustained lock or envelope passes.

Fault25 identifies average-monitor refusal, not a calibrated overcurrent event.
The explicit nominal threshold remains 1991 raw sum counts, using assumed gain10
and shunt7mohm for a nominal500mA target. Gain, sampling accuracy and physical
uncertainty remain unverified. No threshold was raised and no hardware-limit
claim follows from these stops. The next useful evidence is the actual block
residual and channel calibration, not a flying-recovery cohort. The old raw
pulse clamp also remains to be reconciled with the practical goal.
