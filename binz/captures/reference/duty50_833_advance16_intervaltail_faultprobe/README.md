# E833 fixed-advance-16 timing control

One-variable control for E831/E832: remove only `bench-advance-18`, returning
the minz-core reference default to fixed advance 16. The diagnostic interval
tail, bounded nFAULT release probe, 24 kHz carrier, live duty path, and all
non-diagnostic electrical/tracking/deadline/watchdog protections are unchanged.

- ELF SHA256: `58E5A048304323682188FE69E87774B045C14846CAB17E88495951D35E785619`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122116, data1180, bss23680
- four motor ISR-root M0 arithmetic reachability audits PASS
- diagnostic-only: sequential current and block-bus quantities are telemetry;
  raw feedback validity, nFAULT, tracking, deadlines, watchdogs, and the
  physical 4 A PSU cap still own shutdown
- not an envelope-qualification image; intended for a single matched smooth
  ramp comparison against E832's fixed-advance-18 collapse
