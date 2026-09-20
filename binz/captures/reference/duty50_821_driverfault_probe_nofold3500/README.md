# E821 Driver7 release probe with non-actuating first current warning

Diagnostic-only successor to E819 after E820's two uncorroborated current
warnings folded 36% down to31% with zero low-bus samples and prevented the
known Driver7 boundary from being reached.

- ELF SHA256: `56201F8E76E01569F7A9F035384E9341B0EB63BBA319575F4CB91B43E11F7701`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121792, data1180, bss23160
- exact E819 nFAULT release observation: gates/MOE off immediately, bounded
  5.5 ms ENABLE-high foreground sampling, then unconditional ENABLE-low
- first complete current-over warning is consumed but neither actuated nor
  acknowledged; the unchanged second-consecutive-over DMA guard remains a
  terminal stop
- bus, nFAULT, tracking, deadline and watchdog stops are unchanged
- this image is diagnostic evidence only and cannot qualify an envelope point
- 34 policy tests in each feature cohort PASS; four motor ISR-root M0 soft-
  arithmetic reachability audits PASS
