# E862 fixed-advance20 aggregate CPU diagnostic

Built from the exact E861 feature closure with `bench-interval-tail` and
`bench-persistence-hist` removed and `bench-cpu-aggregate` added. This keeps
the E861 speed-scaled missing-event watchdog, fixed advance20, 24 kHz carrier,
4.0 A nominal measurement, and report-only signed-current authority. Bus sag,
nFAULT, raw-validity, tracking, deadline, and watchdog stops remain active.

- ELF SHA256: `E522191FB7017EBAF9939818E6D193962D1EDF1E3F88D88AAA3118E5063BD637`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text125180, data1180, bss23212
- four motor ISR-root arithmetic reachability audits: PASS
- CPU/speed-watch/wire host tests: 12 PASS

Aggregate entry/exit clock reads and critical sections are observer effects.
Any powered result from this image characterizes CPU scheduling only and does
not qualify a lean image or expand the duty envelope.

E863 ramped through31% before Tracking8. Aggregate IRQ union was53.98%:
guard3.58%, COMP24.04%, COM15.43%, DMA10.94%. The target35% was not reached.
This rules out CPU saturation as the immediate event-dropout cause at31%, but
does not establish exclusive WCET or predict occupancy at50%.
