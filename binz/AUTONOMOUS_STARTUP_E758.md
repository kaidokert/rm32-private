# E757–758 — startup no longer requires host frequency steps

The operator accepts the corrected current and bus channels as rough bench
estimates for routine runs. Independent 15% anchor: 230 mA, 11.8 V without
PSU voltage limiting; final firmware block approximately 255 mA nominal.
Do not require another display observation for each operating point.

## Autonomous schedule

The straight firmware 100-to-200 eHz startup on the restored software ADC
configuration failed before BEMF handoff (E757, acquisition reason21).
The previously rotating configuration used host run50 then ACK-paced
60 through200 steps at3.2–4.4s.

Optional `bench-startup-staircase` reproduces that trajectory in the existing
1kHz foreground envelope:100Hz catch, ramp down to50Hz, then fifteen steps
60..200 at ticks3200..4400. Existing waveform ISR is unchanged. Deadlines are
constant table values with compile-time assertions on all boundaries; the
new selector uses only u32 comparisons/additions, no runtime division.
This is envelope-tick timing, not a promise of identical host wall timing.
5 campaign tests pass. Host frequency commands are absent in direct tests.

E757 first staircase trial was refused during forced acquisition by the
corrected nominal800mA average guard:3771>3458, ~872mA nominal. This was not
the straight startup's earlier acquisition21 failure. Both are retained.

## Operator 1A bench setting

E758 aligns the nominal firmware physical current target with the operator's
confirmed exact1A PSU setting, announced before changing code. Gain10,
shunt7mohm, VDDA calculation and corrected signed polarity remain unchanged.
The threshold is computed from that target, not fitted to a failing count.
`--nominal-target-ma 1000` requires the fixture ACK to name1000mA and verifies
its converted raw threshold. Older800mA images remain explicitly selectable.
34 startup policy tests pass.

Frozen installed image: `captures/reference/avg1a_758/shell-pwm.elf`.
SHA256 `756adf564f065438dd02ef47c3fd1d024535500075caf8de8f457ffc46fdafaa`.
Release opt-s/thinLTO/codegen1; build, directTIM16 arithmetic audit, download,
OpenOCD reset and own disabled guard3/18 check succeed.

## Autonomous startup and live ramp

`captures/avg1a_758_direct200_ramp30.txt`: autonomous startup enters BEMF.
Live10/15/20/25% acknowledged;30% never acknowledged. At powered4,853,057us
average-current reason25 stops: residual5125>4323 (~1185.5mA nominal).
13,376 applied commutations, minimum bus10,937mV, peakraw750,
last accepted4,853,038us and feedback4,852,775us. Acquisition handler23us,
zero overruns. Final outputoff verified; UART closed.

This proves one autonomous startup into BEMF and live25% response, not startup
repeatability or sustained25% qualification. Nominal current is a bridge
return sample mean, not synchronized PSU current; load transients, gain and
sampling bias remain unresolved. The target is not increased again to erase
this result. A same-image autonomous20% hold follows.

`captures/avg1a_758_direct200_ramp20.txt`: second autonomous startup reaches
BEMF and live20% ACK. Runs22,239,545us, then average-current reason25 stops:
4352>4332, approximately1004.6mA nominal.111,943 applied commutations,
busminimum10,722mV, rawpeak748, lastaccept22,239,443us, feedback22,239,275us.
Finaloff verified, UART closed. This is not a completed30s20% hold. A single
10.05ms block crossing the nominal1A target is the stop mechanism; long-term
current mean or an excursion's physical cause is not proven by the last block.

Normal tracking-loss shutdown-and-startup restart still needs implementation.
Qualification remains separate, with no in-ISR diagnostics. Legacy raw phase
peak cutoff remains linked and must be reconciled with the goal's report-only
uncalibrated peaks. No hardware modification or flying-reacquisition gate.
