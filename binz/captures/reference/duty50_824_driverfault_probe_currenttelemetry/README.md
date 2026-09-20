# E824 Driver7 release probe; sequential current is telemetry only

Bounded diagnostic image built because E822/E823 stopped on the asynchronous
sequential phase-current proxy at34/35%, before the retained Driver7 boundary.

- ELF SHA256: `DB07F8DEBFA7A7BCFE1BA574AC914338BBD21D21DE33BF537CBF315CBAC0072F`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121792, data1180, bss23160
- signed complete-block current trips are recorded but do not stop this image
- ADC invalidity/phase rails, complete-block bus sag, raw nFAULT, tracking,
  deadline and watchdog stops remain active; the physical PSU is capped at4 A
- on Driver7, all gate commands/MOE turn off immediately, nFAULT is observed
  for a bounded5.5 ms with ENABLE high, then ENABLE is forced low
- this image can classify the fault release behavior only; it cannot qualify
  any duty or current envelope point
- 34 policy tests per feature cohort PASS; four motor ISR-root M0 arithmetic
  reachability audits PASS
