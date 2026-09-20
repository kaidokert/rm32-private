# E867 running level-revisit A/B

Exact E861 feature closure plus `bench-running-level-revisit`. If the normal
half-interval gate is open, EXTI has no pending edge, and the real comparator
already reads the expected post-ZC level, foreground may pend one ordinary
ADC_COMP service for that controller command. The ISR retains its persistence
reads, real interval timestamp, normal controller acceptance, and all guards.
No commutation or ZC timestamp is synthesized and no command may be retried
twice without a real accepted event.

- ELF SHA256: `A910E9DE9B8193C9201EE0C4861D6D86BCF526DBA96A4B0CA075872E15760F45`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122912, data1188, bss24512
- pure admission policy: 4 tests PASS
- existing CPU/speed-watch/IT86 host tests: 12 PASS
- four ISR-root and revisit-caller M0 arithmetic audits: PASS

This is a diagnostic control A/B, not a qualified envelope image. Post-run
`LEVELREVISIT` reports attempts and accepts; IT86-v2 remains the causal suffix.

Powered results: E868/E86935%, E87040%, and E87145% each held60s with zero
tracking/bus/nFAULT stops. Revisit attempts equaled accepted events and rose
from~2.4% to~3.0% of total events. E872 reached50% but stopped on an independent
complete-block low-bus guard after roughly23s at target; no Tracking8 led.
