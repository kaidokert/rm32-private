# Reverse 32 kHz first-peak timing probe

Frozen `shell-pwm.elf` SHA256
`3769F5AC6202FD0B0872BC14383B457D735F262B838CD63DED9C6C71EF01D260`.
Release-hybrid opt-s/thinLTO/codegen1; text129104/data1200/bss17352.
Four ISR-root M0 soft-arithmetic audits pass. Exact prior reverse32k
protected feature closure plus a first-terminal-peak `PEAKTIME` row.
It records DMA trigger/service time, TIM1 CNT/ARR/CCRs, TIM17 CNT,
guard event/sector, controller interval/step/duty and COM count before
reason27 revokes outputs. It adds no work to ordinary DMA scans beyond
the existing first-peak test. PWM/role values are service-time context,
not direct timestamps of the sequential IA/IB/IC ADC apertures.

Flashed/verified on explicit G071 device0x460. Disabled guard3/18,
roledu1006/6 and duty6/6 passed; initial p/i outputs off, nFAULT high.
No motor result is qualified. One `run200` refused before drive because
`avgnominal` was omitted. After configuration, a second attempt was
incorrectly armed via `engage1` plus `live1`: `engage1` clears the
`driveobs1` path and does not provide live UART duty control. A normal
startup banner appeared, then UART gave no response while active; an
`off` write had no ACK. Explicit SWD hard reset restored shell response,
with p/i all gates/ENABLE/MOE/CCRs off and nFAULT high. The exact known
reverse32k lean SHA06B1C1D6... was restored/verified/reset, p/i again
safe, and COM41 closed. No `PEAKTIME` row was captured. This image is
staged, not a 43% pass/fail or a carrier verdict.

The correct live-transfer arming path in this firmware is idle-only
`drivex1`, `driveobs1`, and `live1`, with `avgnominal` and bounded
`engagems` configured before `run200`; `engage1` is a different
powered-acquisition path whose abort callback treats *any* UART byte
as stop. The corrected path was then exercised. A protected12s low-duty
run accepted `du110` and ended by deadline without electrical fault.
One bounded climb ACKed40%, dwelled9s, then stopped on the first
near-rail phase sample at last ACKed42.5%: reason27, raw physical
IA/IB/IC=2846/2056/24 and bus/VREF=1136/1510. `PEAKTIME` trigger/
service47780262/47780337us; PWM CNT1011 ARR1999 CCR850 at service.
At64MHz timer rate the75us backward projection gives CNT~211 at
trigger. The first ADC channel is physical IC; its160.5-cycle sampling
at16MHz nominally ends near CNT853, close to the PWM compare850.
This is a derived edge-alignment, not a precise aperture measurement
or proof of the45% bus-collapse cause. The bus channel's aperture
nominally ends near CNT929, ~1.2us after the following PWM compare,
so its one low code could be a switching-phase notch. Full raw output:
`captures/reverse_direction_2026-09-19_32k_peak_timing_425.txt`.
Known lean image was again restored/verified/reset with gates/ENABLE/
MOE/CCRs0, nFAULT high and COM41 closed. Diagnostic42.5% stop does not
qualify a lean ceiling.
