# Reverse-only 32 kHz fault-frame diagnostic (2026-09-19)

Frozen ELF SHA256
`9C23657592678C2FCDBBFEDE049CDC4A72C622661DBF0085BE31C494A2927B11`.
Built `release-hybrid` opt-s/thinLTO/codegen1 with the exact32k lean feature
closure plus `bench-adc-latest-fault-frame`. The extra feature snapshots
only a rejected coherent ADC frame and emits it after shutdown. It does
not change ADC validity, current, bus, nFAULT, tracking or timer stops.
All four ISR-root soft-arithmetic forbids PASS on this exact ELF. Disabled
guard3/18, reverse role6/6 and duty6/6 PASS after flash on explicit G071.
This is a diagnostic image, not a lean qualification image.

One powered replay reached45% requested duty, then fast bus sag reason26
stopped at35.625135s. No ADC invalid frame occurred, so no ADCFRAME line.
See `../../reverse_direction_2026-09-19_32k_faultframe_45_fastbus.txt`.
The operator then paused bench work; board is this image, outputs OFF,
COM41 closed. No further powered test until they resume.
