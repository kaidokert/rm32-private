# Reverse-only 32 kHz DMA-below-COMP/COM A/B (2026-09-19)

Frozen ELF SHA256
`11777BAC0970BCDA01CD27ED3C495A0D21B8DCCFE7431986D18651D47D9B6FD9`.
Same32k lean control/protection feature closure as
`../reverse_32k_20260919/` plus only `bench-dma-below-comp`.
TIM6 guard remains priority0; COMP/COM64; DMA128 instead of0. Full
current, relative/absolute bus, nFAULT, tracking and watchdog stops
retained. Release-hybrid opt-s/thinLTO/codegen1, text124760/data1200/
bss17144, all four ISR-root soft-arithmetic forbids PASS.

Flashed/verified on G071, disabled guard3/18, reverse role6/6,
duty6/6 and off-state/nFAULT1 PASS. One ordinary-start10% attempt
stopped at powered4453us on ADC reason11, stage27, DMA code7:
`Lease::finish` rejected the DMA half-buffer snapshot, service_gap237us
against226us scan period, remaining2. Only7 COMs; no high-duty comparison.
Outputs off/nFAULT1. Candidate retired after this single lower-gate failure;
prior exact32k lean SHA06B1C1D6... restored, reverified, reset and passed
disabled preflights. Capture:
`../../reverse_direction_2026-09-19_32k_dma_below_startup_fail.txt`.
