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

### Telemetry interpretation

The phase labels in compact captures are logical labels, not raw ADC order:
`CURRENT_ADC=[4,1,0]`, so logical A/B/C are physical ADC4/ADC1/ADC0. The
three current channels are bidirectional low-side CSA outputs, approximately
70 mV/A around a zero-current code near 1.65 V; instantaneous samples can show
PWM ripple and must not be equated with PSU average current. VBUS is ADC6
(`VSENVM`) with an approximately 11.94:1 divider. BEMF phase inputs are
VSENA→PB3, VSENB→PB7, VSENC→PA2, with neutral on PA3. In the compact
`bench-startup-adc` rows, `vsenc` and `neutral` are intentionally unavailable
and print as zero; that is an instrumentation omission, not a zero-voltage
measurement. Use `CAPMIN`/`CAPMAX` only for the channels they actually expose.

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

### Source-derived handoff constants

For the frozen reverse/48 kHz closure, the less-visible runtime values are:

| parameter | value | meaning |
|---|---:|---|
| `START_HZ` | 50 eHz | first open-loop catch frequency |
| `CATCH_DUTY_TENTHS` | 70 | 7.0% during catch |
| `SEED_MIN_TICKS` | 952 half-µs ticks | minimum accepted flying seed (`bench-range350`) |
| `CYCLE_MIN_TICKS` | 5716 half-µs ticks | minimum six-event acquisition cycle |
| `INDIVIDUAL_MIN_TICKS` | 476 half-µs ticks | minimum individual acquisition interval |
| persistence depth | 12 reads | every candidate comparator level |
| runtime cycle guard | 2223 µs | `bench-cycle450` profile |
| runtime event guard | 238 µs | minimum accepted-event interval |
| stale-event timeout | 1000 µs effective | `bench-speed-event-watch` is enabled with a three-period tighten-only policy; at this speed it does not reduce the 1000 µs ceiling |
| low-duty advance | 20 | scheduled/reverse profile below 35.0% |
| high-duty advance override | 26 | only active at the high-duty threshold, not in this run |
| reverse blanking | enabled | arms when average interval is at least 1500 half-µs; post-run summary reports actual mask |

The acquisition tick unit is 0.5 µs; the powered guard values are already in
microseconds. Do not mix those units when comparing seed/interval telemetry to
`TRACKSTOP` timestamps.

The 48 kHz carrier is TIM1 at 64 MHz (`ARR=1332`, edge-aligned); the sine/control
update is 1 kHz, and the configured complementary dead-time is 26 timer ticks
(406.25 ns). These are part of the electrical timing regime even though the
UART `RUN:` banner only reports the campaign rates.

For a handover/lock run, add `bench-handoff-early` to the feature closure and
send `driveobs1` before `run200`. This moves the one-shot transfer from the
end of the 30-second open-loop campaign to 4.7 s (20 ms align + 980 ms catch +
2,000 ms ramp + 1,700 ms target dwell), leaving the remainder under the real
BEMF controller. `drivex1` must also be sent: `driveobs1` schedules the
handoff callback, while `drivex1` authorizes transfer. Without both, `run200`
is intentionally an open-loop light-spin test and cannot prove handover.

### Known-good BEMF-controlled recipe

The earlier 10--50% locked campaign used additional driven-path overrides that
were missing from the minimal low-duty recipe. To reproduce that behavior, use
the following sequence while idle (CR-terminated, with no extra UART traffic):

```text
off
engage0
obs0
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
run200
```

`drivephase60` and `drivedu61` establish the driven startup vector;
`bemfdu100` sets the post-transfer BEMF duty override to 10.0%, even though
the commanded campaign duty is 6.2%; `drivepwm192` selects the validated
driven ADC/PWM sampling target; `engagems35000` gives the powered handoff
window 35 seconds; and `drivex1` authorizes the one-shot transfer. The 50%
cohort used the same setup with `engagems55000`. These are not cosmetic
commands.

The retained 10% and 50% transcripts show the distinction: both transferred
with `DRIVETRANSFER result=1`, then ran to a normal deadline with
`TRACKSTOP event_fault=0`; the 50% run recorded 561,869 power commits. By
contrast, the recent minimal 6%/7% handoff sequence omitted these overrides
and lost tracking after 20--30 ms. Those results are not an apples-to-apples
counterexample to the earlier locked runs.

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

### Lock verdict rule

`BEMFSTOP lock_proven=0` is currently a hard-coded conservative label in the
compact firmware; it is not evidence that lock failed or succeeded. The
independent verdict must therefore be derived from the run:

| Check | Required for a lock pass |
|---|---|
| transfer | exactly one `DRIVETRANSFER result=1` |
| runtime | the real BEMF-controlled segment reaches the requested target dwell |
| events | accepted-event and commutation counters remain populated throughout; no final-event gap |
| timing | no `TRACKSTOP event_fault=1`, stale-event/watchdog, order, or deadline fault |
| protection | no current, fast-sag, nFAULT, or tracking stop |
| shutdown | normal deadline completion, outputs disabled, `nFAULT=1` |

For a 30-second target dwell, a short post-handoff burst of accepted events is
only **catch evidence**. It is not lock evidence. The current 6% and 7% runs
meet transfer/catch but fail the runtime and timing rows after roughly 20--30
ms, so they must be reported as `catch=pass, sustained_lock=fail`.

The executable checker is `scripts/verify_bemf_lock.py`; run it against the
complete transcript. It intentionally rejects the retained handoff capture
because that capture lacks a live lock summary, and it will reject the compact
captures on `TRACKSTOP event_fault=1`. A future successful transcript must make
this checker pass rather than relying on visual spinning or the hard-coded
`BEMFSTOP` label.

## Known-good result

## Replication audit (2026-09-20)

The recipe above is sufficient to reproduce the **open-loop** 6%/200 eHz
light-spin. It is not yet sufficient to reproduce a successful closed-loop
lock, because no such result exists. The exact handoff experiment is therefore
the thing to replicate, not a claimed pass.

Parameters that must be copied verbatim:

- Use the frozen ELF from `captures/reference/reverse_48k_com_top_high_20260919`
  for the qualified baseline, or record the SHA-256 of every diagnostic rebuild;
  the mutable source tree alone is not an image identity.
- Use the complete feature closure in that README. For the first handoff run,
  add `bench-hold-30s`, `bench-cap-summary`, and `bench-handoff-early`.
  `bench-running-level-revisit` is an explicit A/B variable: do not silently
  add or remove it when comparing runs.
- Preserve the compiled `RUN:` banner, `HANDOFF_US`, event-watch limit, carrier,
  advance policy, current/bus thresholds, and the actual PSU voltage/current
  limit. These are firmware parameters, not operator folklore.
- Preserve the physical phase identity from
  `controlboards/BOOSTXL-DRV8304H/G071_DRV8304_WIRE_MAP.md`: logical A/B/C
  gate, VSEN, and ISEN associations, reverse phase order, ENABLE/MODE straps,
  UART pins, and motor connector orientation.
- Send the six commands in order, with CR terminators and no other UART traffic:
  `cap1`, `avgnominal`, `du60`, `driveobs1`, `drivex1`, `run200`.
  `driveobs1` arms the early handoff; `drivex1` authorizes it. Omitting either
  changes the experiment to open loop.
- Capture the complete post-run lines, including `DRIVETRANSFER`,
  `LOCKSUMMARY`, `LIVEGAPS`, `TRACKSTOP`, `BEMFSTOP`, protection summaries,
  and final output/nFAULT readback. A motor that audibly spins is not lock
  evidence.

The latest revisit-disabled A/B used image SHA
`4D2C1262D753232886DA4984C9B5809D5936FDD385949685738CDA34CBDC8448`
(text=128176/data=1208/bss=17160), with the same sequence and
`bench-running-level-revisit` removed. It produced `accepted=28`,
`commutations=29`, `LIVEGAPS n=27 min_us=482 max_us=956`, then
`TRACKSTOP event_fault=1` at 22.790 ms. The earlier full-closure run produced
32 accepted / 33 commutations and the same stale-event stop. Thus the A/B does
not show that level-revisit is the root cause; both variants lose the final
comparator event. It is useful replication data, not a lock pass.

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
A feature-scoped live-gap census now fills that gap: a repeat reported
`accepted=32`, `commutations=33`, `LIVEGAPS n=31 min_us=513 max_us=964`, then
tracking loss at 26.684 ms. The final missing event crossed the 1 ms watch
boundary; current evidence points to a BEMF-event/closed-loop timing loss,
not an electrical protection trip.
Therefore the open-loop light-spin result and BEMF lock result must not be
conflated: catch is now proven to execute, while sustained lock remains an
active defect to diagnose.

A matched 7.0% run behaved the same way: transfer succeeded with 34
commutations, then `TRACKSTOP event_fault=1` at 27.261 ms. Its transcript is
`captures/handshake_7pct_200ehz_handoff_20260920.txt`. The repeat across 6%
and 7% makes “insufficient BEMF amplitude at exactly 6%” an inadequate
explanation; the handoff/closed-loop timing or acceptance path needs diagnosis.
