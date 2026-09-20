# Reverse 32 kHz first-near-rail protective diagnostic

Frozen `shell-pwm.elf` SHA256
`7B2A67A1B78F9C1838182114EDB49BE3359F6DCFC3A810F152CA79BB08025BCC`.
Release-hybrid (`binz` opt-s, thin LTO, codegen1); text127732/data1200/
bss17280. Four exact motor ISR-root soft-arithmetic audits pass;
physical-phase classification host tests2/2 pass. This is not a lean
qualification image.

Exact reverse32k/fixed-advance20/current4A protected control closure
plus `bench-phase-peak-stop` and `bench-adc-latest-fault-frame`.
At a live 40% or45% census epoch, the first coherent physical phase
sample displaced >=1900 raw counts from its measured same-wake zero
causes same-DMA-wake `POWERPATH reason=27` and retains IA/IB/IC/bus/VREF.
The unchanged fast5%/three-scan bus guard is evaluated first on that
frame and retains stop precedence if simultaneously due. The existing
average-current actuator, nFAULT, tracking, watchdog and absolute-bus
stops remain. The latest-frame fault feature records the offending
frame and `Error` subtype if that path fires first. This probe stops
earlier than the historical exact-rail cache refusal; it is not a
calibrated FET SOA threshold or a valid envelope limit.

Build inputs: explicit feature closure from the prior phase-census ELF,
adding only `bench-phase-peak-stop,bench-adc-latest-fault-frame`.
Completed: explicit G071 device0x460 flash/verify/reset; disabled
guard3/18, roledu1006/6, duty6/6 and p/i off/nFAULT high PASS.
Ordinary10%/15s and40%/60s protected gates PASS, zero >=1500-count
phase excursions at40. The gradual climb stopped on reason27 at the
last ACKed43.0%, before the host's43.5% command was accepted.
`RATEPEAK` physical raw IA/IB/IC=2047/1094/4087, bus/VREF=1159/1505;
start bus/VREF=1210/1505, so sampled bus was~95.8% of reference.
No exact rail, fastbus, current foldback, tracking or nFAULT stop.
The snapshot is sequential/asynchronous, so it does not establish
simultaneous phase currents or prove a commutation-sector cause.
Capture: `captures/reverse_direction_2026-09-19_32k_peakstop43.txt`.
Exact lean SHA06B1C1D6... was then restored/verified/reset, p/i off/
nFAULT high, COM41 closed. This observer/protection image is retired;
its43% stop is NOT a lean envelope qualification.
