# E826 Driver7 release probe; sequential current/bus block is telemetry

E825 reached35% but the50-scan8.4 V software bus-average stopped first, so the
DRV nFAULT discriminator still did not execute. This bounded diagnostic image
lets the independent raw-feedback validator and DRV protection own shutdown.

- ELF SHA256: `07464329B3271C5C153390F6871F657C9F088FF17EDFA94E9A1C72E3C8CB6D71`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121728, data1180, bss23160
- asynchronous sequential current and complete-block8.4 V bus results are
  retained as telemetry but do not stop this diagnostic image
- raw ADC validity/phase rails and the independent raw-feedback bus validator,
  nFAULT, tracking, deadlines and watchdogs remain active; PSU is capped at4 A
- Driver7 immediately revokes all gate commands/MOE, samples nFAULT for5.5 ms
  with ENABLE high, then unconditionally drives ENABLE low
- cannot qualify an envelope operating point
- 34 policy tests per cohort PASS; four motor ISR-root M0 arithmetic audits PASS
