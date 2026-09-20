# Reverse 48 kHz fast-sag PWM-phase A/B

Frozen `shell-pwm.elf` SHA256
`32683823EA380500449C2C22AD7EFAFA532891B5311D4D6DFE2E32B06C96F415`.
Release-hybrid opt-s/thinLTO/codegen1; text128648/data1200/bss17660.
Build inputs match the frozen reverse32k sag-PWM diagnostic SHA
`47ECC83A...` except `bench-reverse-32k` is replaced by
`bench-reverse-48k`. All four motor ISR-root soft-arithmetic audits
pass. TIM1 nominal ARR1332/1333 ticks =48012Hz; ADC stream remains
226us, 16MHz PCLK/4, 160.5-cycle sample. The independent fast5%/
three-scan bus stop, active4A signed-current foldback/stop, nFAULT,
tracking and watchdog protections are unchanged. `SAGPWM` occurs
only on a terminal third low-bus scan. This is diagnostic, not lean
qualification.

Explicit G071 flash/verify/reset and disabled guard3/18,
roledu1006/6 (period1333/compare133), duty6/6 passed. Correctly
armed protected run ACKed40%, dwelled18s, reached45.0% and held
about18s; intentional host `off` was reason9, with no fast-bus,
current, tracking or nFAULT stop and no foldback. Capture:
`captures/reverse_direction_2026-09-19_48k_hold45.txt`.

A second run ACKed40%, dwelled9s, ACKed45%, dwelled5s, then
climbed toward50. The unchanged three-scan fast-bus stop reason26
fired at last ACKed46.5%, before47/50. Frozen eight-scan capture:
`captures/reverse_direction_2026-09-19_48k_sag_pwm465.txt`.
Its terminal `SAGPWM` stamp43500632us CNT966 ARR1332 CCR619;
the final three bus apertures project to CNT807,608,409, spanning
OFF, near compare, and mid ON. Their normalized bus values were
93.24/94.21/92.58%; not one edge notch. No current foldback,
tracking or nFAULT stop. Post-run p/i all gates/ENABLE/MOE/CCRs0,
nFAULT high. Exact known reverse32k lean SHA06B1C1D6... was
restored/verified/reset, p/i off and COM41 closed.

These are one45% exploratory pass and one46.5% protected stop, not
a 45% lean qualification, a quantified carrier advantage or a50%
result. Never transfer the32k timer counts directly to this1333-
tick carrier.
