# Reverse 32 kHz fast-sag PWM-phase diagnostic

Frozen `shell-pwm.elf` SHA256
`47ECC83AEA6B42D35388EDEEAE835C3502C086D101D17EBF9EA5155EEE26A756`.
Release-hybrid opt-s/thinLTO/codegen1; text128600/data1200/bss17660.
Exact reverse32k lean protection/control closure plus
`bench-fast-sag-causal`; four DMA1/ADC_COMP/TIM16/TIM6 interrupt-root
soft-arithmetic audits pass. This diagnostic is not a lean qualification
image.

The previous eight-scan coherent ADC ring and three low-bus causal
rows are unchanged. On the terminal third sub-95% bus scan only, a
`SAGPWM` row now latches sampled-clock time adjacent to TIM1 CNT,
ARR, CCR1/2/3 and CR1 before outputs are revoked. It adds no timer
reads to ordinary scans or isolated low-bus samples. Offline analysis
may back-project each retained scan's acquisition timestamp to a
PWM phase and then add the sequential ADC channel aperture offsets.
This is a derived alignment with clock/trigger/register-read
uncertainty, not a simultaneously sampled current measurement.

Disabled guard3/18, roledu1006/6 and duty6/6 passed on the G071.
The first configured30s control ended by deadline at30% because the
host had not yet ACKed35%; no motor fault and no terminal SAGPWM.
The next run used a90s window, ACKed40% and dwelled18s clean, then
climbed in0.5%-steps. The last ACK was44.5%; the unchanged three-scan
fast-bus guard stopped reason26 before45% was ACKed. `SAGPWM` captured
TIM1 CNT1284/ARR1999/CCR890 at sampled-clock48361796us. The eight
retained ADC scans and three causal rows are in
`captures/reverse_direction_2026-09-19_32k_sag_pwm445.txt`.

The three terminal bus sample apertures project to TIM1 counts
1570,34,498, spanning OFF, early ON and mid ON, with normalized bus
92.58/93.03/92.97% against the run's resting bus/VREF. They are not
one PWM-edge notch. Accepted events and COMs continue through the
dip; there is no immediately preceding missed-edge/held-sector
signature in the eight-event tail. This does not establish the
surge's electrical or control cause. Gates/ENABLE/MOE/CCRs0 and
nFAULT high were verified post-run, then exact known reverse32k lean
SHA06B1C1D6... was restored/verified/reset and p/i checked off with
COM41 closed. This observer image does not qualify44.5 or45% duty.
