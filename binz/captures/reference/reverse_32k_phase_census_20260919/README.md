# Reverse 32 kHz physical-current/bus rate diagnostic (retired)

Frozen `shell-pwm.elf` SHA256:
`E62E4D4E2F7145081305578DD91488A43E022CF7A913B19AD20E495F66263E00`

Release `opt-level=s`, thin LTO, codegen-units=1; text 126856,
data 1200, bss 17236 bytes. Exact fixed20 reverse32k lean feature
closure plus `bench-phase-current-census` (which includes the existing
`bench-rate-census`); current4A, fast5%/three-scan bus, absolute-bus,
nFAULT, tracking and watchdog protections remain active. Four exact
motor ISR-root soft-arithmetic audits pass. Pure current-bin policy
host tests 2/2 pass. This image was flashed/verified on the explicit
G071 and run diagnostically, then replaced by the exact lean image.
It is not an envelope-qualification build.

The diagnostic starts/restarts its report-only epoch when live duty
400 or450 is ACKed. At each complete coherent ADC DMA scan it counts
physical IA/IB/IC displacements from the measured same-wake zero at
raw thresholds1500 and1900; `RATEPHASE` prints per-phase >=1900
counts, any-phase counts, near-rail/bus-dip co-occurrence, and exact
ADC rails. The fast-bus return is AFTER this census, so its terminal
frame is included. These are asynchronous PWM samples, not calibrated
amps or a protection decision. The existing late-accept and bus-rate
counters remain. No UART is emitted during a powered hold.

Bench sequence performed: explicit G071 flash verify/reset; disabled
guard3/18, roledu1006/6, duty6/6, p/i off; protected ordinary10%/15s
gate PASS; protected40%/60s PASS; gradual comparison stopped at
ACKed43.5% with `POWERPATH11`/`DMAFAULT10`, exact rail observed but
fault subtype not retained. It did **not** reach45%. Full terminal
transcripts are in
`captures/reverse_direction_2026-09-19_32k_phasecensus.txt`.
Exact lean SHA `06B1C1D6...` was then restored/verified, outputs
OFF/nFAULT high, UART closed. Do not transfer this observer-bearing
fault or control pass to the lean envelope by itself.
