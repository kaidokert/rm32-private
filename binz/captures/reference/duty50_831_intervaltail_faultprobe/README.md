# E831 interval-tail Driver7 diagnostic

E826 fault-classifier control path plus a 128-commutation suffix requested by
`GRAYBEARD_TRANSIT_SURGE.md`.

- ELF SHA256: `01F7478E3EF48EEA9BE3780A7B1DECE2FC28C6B15736134308EB084C4C69A17E`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122136, data1180, bss23680
- every accepted powered event stores actual TIM17 gap in microseconds and the
  controller reference interval in half-microsecond ticks;128 rows cover about
  12 ms near1.7 kHz electrical and freeze on the existing stop path
- hot cost is one timer read, wrap subtraction, two u16 stores and ring-index
  bookkeeping; no formatting, division, critical section or UART in the ISR
- dump is post-stop and fixture/capture-only, four events per IT85 record
- diagnostic current/block-bus quantities remain telemetry as in E826; raw
  feedback, nFAULT, tracking, deadlines, watchdogs and physical4 A PSU remain
- stack plus BSS leaves about664 bytes by the latest painted-stack bound, so
  this image is diagnostic-only and must not be used for qualification
- four motor ISR-root M0 arithmetic reachability audits PASS
