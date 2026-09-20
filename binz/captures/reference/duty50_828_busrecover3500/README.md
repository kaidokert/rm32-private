# E828 bus-corroborated foldback with quiet recovery

Exploration candidate after E827 identified Driver7 as a ~103 us self-releasing
fault correlated with hundreds of low-bus ADC samples, strongly favoring a
transient VM/charge-pump undervoltage over VDS auto-retry or GDF.

- ELF SHA256: `9438E875E2AAB7C928A46402660064F4E4B79B270DDD3ECF87B6794E7C5A166B`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122304, data1180, bss23176
- exact E817 bus-corroborated early foldback and all terminal guards retained
- after a foldback, hold the reduced ceiling2 s, then recover toward the latest
  valid host request by1% per second through the guarded foreground writer
- current-only evidence cannot arm recovery or foldback
- each recovery step discards partial mixed-duty current/bus evidence; completed
  prior blocks and all latched terminal faults remain authoritative
- 35 recovery-policy tests PASS in the new feature cohort; four motor ISR-root
  M0 soft-arithmetic reachability audits PASS

This is an exploration image, not a qualified50% image.
