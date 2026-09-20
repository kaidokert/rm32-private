# E812 fast-block adaptive 3.5 A diagnostic candidate

Staged only at freeze time. This is the exact E804 diagnostic feature set plus
`bench-current-fast-20`; it changes no nominal current, bus-voltage, nFAULT,
tracking, watchdog, or PWM thresholds.

- ELF SHA256: `04A111BD573DE5CA5B45712E5BF4528C9A311893727F2F68355143A0AF98FC3A`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121492,data1180,bss23156
- current and bus complete-block cadence:20 scans x226us =4.52ms, formerly50
  scans/~11.3ms
- carrier coverage:20 distinct24kHz phases, maximum uncovered arc278/2666
  timer ticks (4.35us)
- nominal threshold and prestart zero scale with the same20-scan block
- severity-scaled1..5% foreground foldback and second-unacknowledged-over stop
  are unchanged
- four motor ISR-root M0 arithmetic reachability audit: PASS
- host/Rust policy cohort:29 tests at each feature combination, including the
  fast20/3.5A build; all PASS

This candidate addresses the E810/E811 observation that the DRV hardware trip
can beat the old11.3ms warning. It is not itself evidence that45% is safe.
