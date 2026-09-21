# Proper low-duty BEMF lock evidence — 2026-09-20

This is the first repeat of the richer handoff procedure that previously
qualified the 10–50% campaign, run at no more than 10% post-transfer duty.

## Image and setup

- ELF: `captures/reference/reverse_48k_com_top_high_20260919/shell-pwm.elf`
- SHA-256: `0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`
- target: STM32G071RBTx, reverse replacement motor, 48 kHz carrier
- supply: approximately 11.7 V; low-duty bench limit unchanged
- UART: COM41, 115200 8N1

## Exact command sequence

```text
off
avgnominal
engage0
drivephase60
drivedu61
bemfdu100
drivepwm192
driveobs1
engagems35000
drivex1
cap1
du62
catchdu62
live1
run200
```

`du62` is the open-loop startup command (6.2%). `bemfdu100` is the effective
post-transfer BEMF duty override (10.0%), so the run remains below the 20%
ceiling while providing the same torque margin as the known-good campaign.
`engagems35000` sets the powered deadline; the shell banner's nominal hold is
still 2 s.

## Terminal telemetry

```text
RUN: align=20ms@1.0% catch=50Hz/980ms@6.2% ramp=2000ms target=200Hz/6.2% hold=2000ms
DRIVESTOP acquisition_reason=22 acquisition_us=10649 acquisition_duty_tenths=61 bemf_requested_tenths=100 disabled=1
DRIVETRANSFER result=1 zero_not_attempted=1 two_refused=1
DRIVENENTRY refusal=0 adopt_refusal=0 coast_stop=7 fly_age=196 fly_arr=142 fly_arm_us=7 fly_seeded=1 postrun_only=1
TRACKSTOP event_fault=0 last_event_us=34999847 last_poll_us=35000002 feedback_acquired_us=34999730 retained_operational_state=1
SPEEDEVENTWATCH max_us=1000 tighten_only=1 periods=3 poll_us=100 diagnostic=1
POWERPATH reason=2 stop_us=35000005 isr_max_us=0 commit_max_us=0 veto=0 active=0 disabled=1
POWERCOMMITS applied=81157
BEMFSTOP core_stop=7 average_half_us=844 estimated_ehz=394 com=81157 lock_proven=0
RUNNINGREVISIT attempts=7266 accepts=7266 postrun_only=1
REVERSEBLANK arms=4 max_mask_us=301 still_active=0 low_speed_only=1 postrun_only=1
CURRENTQUALITY phase_rail_codes=0 phase_rail_stop=0 bus_low_codes=0 bus_average_scans=100 vref_validity_stop=1
FASTBUS baseline_bus=1213 baseline_vref=1507 threshold_pct=95 consecutive=3 scan_period_us=226 tripped=0 same_wake=1
CURRENTFOLDBACK count=0 last_duty_tenths=0 last_step_tenths=0 step_policy=severity10_50 release=none first_over_warning=1 unacknowledged_second_over_stop=1 foreground_writer=1
DONE reason=8 drive_records=256 coast_records=500 gates=off en=off
```

## Verdict

This is a proper low-duty BEMF-controlled lock run:

- transfer accepted and flying seed valid;
- 35.000005 s powered deadline completed;
- `TRACKSTOP event_fault=0` for the entire interval;
- 81,157 BEMF commutations, with `POWERCOMMITS` and `BEMFSTOP com` agreeing;
- no bus, current, phase-rail, fast-sag, nFAULT, or tracking fault;
- outputs disabled at shutdown.

`BEMFSTOP lock_proven=0` remains a hard-coded compact-image label and is not
used for this verdict. The production-format fallback in
`scripts/verify_bemf_lock.py` cross-checks `POWERCOMMITS` against `BEMFSTOP`.
