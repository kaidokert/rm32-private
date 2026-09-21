# Low-duty catch/lock replication record

This is the reproducibility contract for the current G071/DRV8304 light-spin
campaign. It describes the known-good 30-second image and the evidence required
to call startup catch and closed-loop lock successful.

## Exact image

- Target: `STM32G071RBTx`, ST-Link serial `066CFF343433464757233430`.
- Build profile: `release-hybrid` (`opt-level = "s"`, thin LTO, one codegen unit).
- Base qualified image: SHA-256
  `0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`.
- 30-second hold image currently built from that closure plus
  `bench-hold-30s`; current local build SHA is recorded by the build command and
  must be treated as authoritative rather than inferred from the base hash.
- Feature closure: the comma-separated list in
  `captures/reference/reverse_48k_com_top_high_20260919/README.md`, plus
  `bench-hold-30s`. Do not substitute a default feature build.
- Current compact telemetry image additionally supports `bench-cap-summary`.
  It reports extrema/stage coverage after `cap1` without enabling the oversized
  A85 dump. The handoff/lock build additionally enables `bench-handoff-early`
  and currently hashes to
  `ED90A650BEA682A992418DF2B984124333851C686D772CD71E764E06C220125A`.

## Hardware and bench state

- DRV8304H EVM with the replacement motor, connected in the reverse direction
  mapping; airflow confirms the desired direction.
- Bus supply: approximately 11.7 V. Latest benign run used a 750 mA PSU limit
  and the operator observed approximately 550 mA.
- Motor must turn freely; inspect phase-to-phase equality and phase-to-case
  isolation before powering. Outputs must be off and `nFAULT=1` at the end.
- UART: USART3 PC10/PC11 to FTDI COM41, 115200 8N1, common ground. Host must
  remain silent during the powered interval; received bytes abort the run.

## Runtime sequence

After reset and the idle prompt:

```text
AVGNOMINAL
du60
run200
```

`AVGNOMINAL` must report `accepted=1`. `du60` means 6.0% duty and `run200`
selects 200 electrical Hz. The compiled startup profile is:

| stage | setting |
|---|---|
| align | 20 ms at 1.0% |
| catch | 50 Hz start, 980 ms, 7.0% |
| ramp | 2,000 ms toward target |
| target/hold | 200 eHz, 6.0%, 30,000 ms |
| carrier/control | 48 kHz PWM, 1 kHz control |

The firmware prints the authoritative `RUN: align=... catch=... ramp=...
target=... hold=...` banner. Record that line verbatim; it is the runtime
parameter source of truth if a feature closure changes.

For a handover/lock run, add `bench-handoff-early` to the feature closure and
send `driveobs1` before `run200`. This moves the one-shot transfer from the
end of the 30-second open-loop campaign to 4.7 s (20 ms align + 980 ms catch +
2,000 ms ramp + 1,700 ms target dwell), leaving the remainder under the real
BEMF controller. `drivex1` must also be sent: `driveobs1` schedules the
handoff callback, while `drivex1` authorizes transfer. Without both, `run200`
is intentionally an open-loop light-spin test and cannot prove handover.

## Catch and lock evidence

The existing lean summary proves elapsed powered time and protection outcome,
but **does not by itself prove BEMF handover or lock**. A qualifying capture
must include:

1. Startup stage coverage (`align`, `start/catch`, `ramp`, `hold`).
2. Accepted comparator-event count and commutation count, with a near 1:1
   relationship during the hold.
3. Event interval statistics (minimum, maximum, mean/spread) over the hold;
   no stale-event/watchdog or tracking fault.
4. Raw phase-current and VBUS/VREF extrema, so a quiet current result is not
   mistaken for a stalled or bus-collapsed rotor.
5. Final protection reason, outputs-off readback, and `nFAULT=1`.

With `bench-cap-summary`, arm `cap1` before `run200`. Post-run output includes
`CAPSUMMARY`, `CAPMIN`, `CAPMAX`, and `CAPSTAGE`; these summarize the final
256 ms capture ring. They are useful corroboration, but they do not replace a
full-run accepted-event counter. Until that counter is emitted, label the run
“light-spin demonstrated; handover/lock not independently quantified.”
In the current `bench-startup-adc` closure, `ia/ib/ic` are raw ADC counts and
`vsenc/neutral` are intentionally unavailable in those rows (reported as 0);
do not interpret them as zero voltage.

## Known-good result

At 6.0% / 200 eHz / 30 s / 750 mA limit: `energized_us=29998886`,
`DONE reason=1`, no current, bus-sag, nFAULT, tracking, or watchdog stop;
outputs were off afterward. This is a repeatable benign operating point, not
yet a complete catch/lock characterization.

The first real handoff run is retained as
`captures/handshake_6pct_200ehz_handoff_20260920.txt`. It transferred with
`DRIVETRANSFER result=1`, `fly_seeded=1`, and 29 commutations, but stopped on
`TRACKSTOP event_fault=1` after 23.041 ms (`BEMFSTOP ... lock_proven=0`). A
repeat with `LOCKSUMMARY` emitted `accepted=36`, `commutations=37`, then the
same tracking stop at 30.055 ms; its detailed `stats_events=0` field shows
that the existing event-moments recorder is not wired into the live path.
Therefore the open-loop light-spin result and BEMF lock result must not be
conflated: catch is now proven to execute, while sustained lock remains an
active defect to diagnose.

A matched 7.0% run behaved the same way: transfer succeeded with 34
commutations, then `TRACKSTOP event_fault=1` at 27.261 ms. Its transcript is
`captures/handshake_7pct_200ehz_handoff_20260920.txt`. The repeat across 6%
and 7% makes “insufficient BEMF amplitude at exactly 6%” an inadequate
explanation; the handoff/closed-loop timing or acceptance path needs diagnosis.
