# E835 fixed-advance-20 timing experiment

Second one-variable timing point after E832 (advance18) and E834 (advance16).
Only the compile-time commutation advance changes to20. The 24 kHz carrier,
dynamic AM32 foreground persistence map, interval suffix, bounded nFAULT
release probe, live duty path, and all non-diagnostic protections are retained.

- ELF SHA256: `EF76A2D62BE6FBD301715290232B11C48A89353207FB86ECA73040E43AFC4199`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122140, data1180, bss23680
- minz-core default stays16; advance20 is an explicit opt-in feature and is
  compile-time mutually exclusive with the existing advance18 feature
- default and advance20 minz-core cohorts: 86/86 PASS each
- four motor ISR-root M0 arithmetic reachability audits PASS
- diagnostic-only; not transferable as envelope qualification
