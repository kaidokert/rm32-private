# E748: low-duty entry and explicit startup ramp

## Correction discovered E749

The two early host ramps below were NOT executed as requested. Firmware
allows ehz steps only after its initial3s startup, and rejected early commands
with !step. The fixture previously ignored this. Both raw average stops are
real, but they do not demonstrate behavior under the requested frequency
trajectory. The catch-duty setting did apply. See STARTUP_ACK_E749.md.

Goal read; previous turn progress fixed first-COM admission and retained
hardware-mask evidence. Removed bench-handoff-registers from this build.
Shared const segment_max refactor now rebuilt, frozen and installed:
captures/reference/practical_748/shell-pwm.elf SHA256
E36C2BA93CFC4F7F86BFBC0B76DB8738633DAF73CFE5A8C4950AC418866BB09D.
Release-s/thinLTO/build0/TIM16audit/download/reset0/own guard3/18 PASS.
No qualification claim: startup acquisition diagnostics remain in this image.

Three exploration captures, all final-off verified/UART closed/no300 ACK:

- practical_748_initial70_live300.txt: direct100->200 startup62,
  forced100/phase60/BEMF70. Measuredseed1667/age204/arm7us;
  three poweredCOMs, latestaccepted806us/sector4, tracking stop1905us,
  ADC1779us fresh, averagecause0. Low entry alone does not restore lock.
- practical_748b_early_slow_ramp.txt: host50->200 begins0.2s and spans3s,
  catch/target62, forced61/phase60/BEMF70. Average stop during sine1.52749s,
  residual7183 vsnominalallowance6936, cause3; no handoff.
- practical_748c_vf_ramp.txt: host50->200 begins0.2s/spans2s,
  catch40->target62 through existing firmware duty ramp,
  forced61/phase60/BEMF70. Average stop during sine2.673456s,
  residual7180 vsallowance6913, cause3; no handoff.

The host now exposes --startup-ramp-start, --startup-ramp-span and
--catch-duty; historical defaults unchanged. Invalid ramp timing rejects
before UART and must finish<=4.5s ahead of4.7s handoff. Two mocked validation
tests (eight cases) and eight capture tests PASS. No firmware threshold,
watchdog, current gain or protection change during these host experiments.

Average current is still NOMINAL gain10/shunt7mOhm, not calibrated. Do not
equate raw-average refusals to a PSU measurement, or call this a motor/supply
wall. Earlier/softer startup delays the refusal but does not solve it. Need
resolve actual startup tracking/current estimator behavior, not continue
tiny-duty certification cohorts. Normal-startup restart and calibration
remain unfinished; goal active.
