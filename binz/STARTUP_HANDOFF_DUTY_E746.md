# E745–746: reference pending and initial handoff duty

E745 restored the reference pending-edge startup behavior through optional
`bench-startup-reference-pending` (no timer defer, no synthetic level wake).
Frozen refpending_745b SHA256:
0D1594E8ACDA2B1AF26F839DD023817F202146953B08F6CD4B99A102975FFF17.
The phase0/forced10%/initialBEMF10% trial produced 13 accepts/13 commands,
measured seed1567, arm7us, then COM1/no powered accepts and tracking stop.
This is exploratory evidence, not repeatability qualification.

E746 removes a separate initial-BEMF10% cap for timed startup only. Acquisition
remains4–10%; initial BEMF supports4–30%, matching live throttle. Legacy
admission and optional flying-recovery limits remain unchanged. Equal CCRs
are calculated once in the noinline segment preparation, not per COM.
The host ramp no longer lowers an initial20% setting back to10%.

Installed/frozen captures/reference/initial30_746/shell-pwm.elf SHA256:
AC7F5E5F0AEB37E7864503D1CFA168268F955860720AA32BC1570E8E5D1E6962.
Release opt-s/thinLTO/codegen1 build0; TIM16 soft-helper audit PASS.
32 startup policy tests, 21 timed-startup role tests (all duties1..300,
three carriers, cached role transitions), eight capture tests PASS.
Download/reset0, own disabled guard3/18 PASS.

Initial host invocation refused the old helper's10% limit before motor;
second invocation had a Python indentation error before serial. Both fixed.
Actual motor captures:

- initial30_746c_start62_drive100_phase0_bemf200.txt: measuredseed1666,
  age202, arm6us; startup25accepts/25commands, handler28us.
  Powered reason9 at125us, zero commits/accepts. No300 ACK.
- initial30_746d_quiet_bemf200.txt: same setting with no live host requests;
  measuredseed1595, age262, arm7us; startup14accepts, handler28us.
  Powered reason9 at91us, zero commits/accepts.

Both final-off verified and UART closed. Average cause0. Neither is a20%
sustained run. Reason9 is named HostAbort, but core Recorder::all_off also
uses powered_timer::abort; the quiet trial disproves attributing it simply
to scheduled live queries. Next inspect that software abort/disable decision
and handoff initialization before blaming BEMF signal or hardware.
