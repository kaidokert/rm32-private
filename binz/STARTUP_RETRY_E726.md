# E726 — verified retry source and startup duty observations

Disabled source testing found SWIER18 did not assert the comparator pending
path. Three other checks passed, and final outputs were off. This rejects the
E725 software EXTI wake, without needing a motor run or speculative hardware
explanation. The replacement uses NVIC::pend(ADC_COMP) and a separate retry
flag in the owned comparator input adapter. A timer wake is not acceptance:
the ordinary ISR still validates gate, reference persistence and interval.
Command changes clear the retry flag; stop clears it and cancels the NVIC
request through comp_input::stop. The tightened disabled test calls this real
stop path rather than manually clearing the flag in the test itself.

726b and726c each passed four disabled source checks: no owner cannot wake,
canceled state remains canceled, native NVIC software request is pending,
and actual stop clears both request and adapter flag. Active-owner retry
timing is not proved by these disabled checks.

726c powered normal startup returned cleanly, with final-off readback, instead
of E724's silent timeout. It still expired the40ms driven seed window:29
accepted events/48commands, eight reanchors and only three fresh intervals,
handlermax33us/overrun0, averagefault0. No BEMF handoff or higher-duty ACK.
One clean return does not prove the cause of the earlier silent failure.

The old timed-startup6.2% cap was a first-bring-up restriction. Timed exploration
now allows4..10% driven startup, retaining10% sine and30% BEMF ceilings and
all average/rail/bus/fault/tracking/watchdog stops. Fixture startup/drive duty
arguments make this adjustable without a rebuild per operating point.

An8% startup on726d stopped on average-current refusal at0.693685s. One
intermediate7% startup stopped at1.568480s, residual3215>3185, nonrail cause3.
Both failed before handoff and produced no higher BEMF ACK. No average
threshold was raised. Physical current conversion is still nominal and
uncalibrated; these observations are not calibrated800mA measurements.

The first8% fixture invocation rejected its old host62 limit beforeRUN; the
second failed import with an indentation error before opening UART. Both were
corrected. Only startduty_726d_start80_direct300_03.txt is a powered8% attempt.
Completed powered captures decode with valid drive/coast counts and ordering
and verify final gates/ENABLE/MOE/CCRs off. Host sessions closed.

Installed image `captures/reference/startduty_726d/shell-pwm.elf` SHA256:
`8f72abe67ef8980a23f3d43a91bd01a680af016181138380029c5ba53045323d`.
Release-s/thinLTO and TIM16 math gate passed for all candidate builds.
Artifacts: deferred_check_726_source.txt (failed SWIER test),
nvic_retry_726b_source.txt, nvic_cancel_726c_source.txt,
nvic_cancel_726c_direct300.txt, startduty_726d_start80_direct300_03.txt,
startduty_726d_start70_direct300.txt. No new envelope qualification.

Next is normal startup waveform/mode architecture and current sampling
validity. A fresh-window seed under forced IRQ drive is still the bottleneck;
flying recovery remains outside the exploration gate. No further nominal
average-cap increase is planned.
