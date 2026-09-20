# Graybeard memo — after the burn: the fast sag stop, and what E872 actually was (2026-09-18)

binz, three barriers proposed after E873. Taking them in the operator's order of
importance, with the operator's ruling on the physical side already in:
**the connector did not fail; it's fine.** Drop barrier 3's lead/connector
inspection from the plan. Barriers 1 and 2 stand, with the specifics below.

## Barrier 2 first: bus collapse must be caught in ~1 ms, not 11

Operator's spec, verbatim in substance: *"we don't want to crap out on 1-sample
ADC noise, but the moment you are getting a more than 5% sharp dip from the set
voltage we have a problem."* Turn that into code exactly, no interpretation:

- **Reference = the set voltage**, measured once at the start of the powered
  segment (the idle bus you already sample before handoff), **not** a running
  average. A running average tracks a slow collapse and never fires; the 50-scan
  block that stopped E872 is the slow version of that mistake.
- **Threshold = 5% below the set voltage.** 11.7 V → trip line 11.1 V. Store it
  as a raw ADC count computed once at segment start; the comparison in the DMA
  handler is one `cmp`.
- **Persistence = 3 consecutive coherent scans below the line** (~0.7 ms at
  226 µs). That is the "not one sample" rule: a switching notch or a phase-walk
  artefact hits one scan; a real collapse holds. Reset the count on any scan
  above the line.
- **Action = ENABLE low, gates off, latched for the run.** Same terminal path as
  reason 25; new reason code so the record says which guard fired.
- **Where = the DMA handler**, which already owns scan validation and the pulse
  clamp. No foreground hop — E623 already showed the foreground can lag 1 ms
  under load, and this guard's whole point is latency.
- **Record on trip:** the last 8 scans of bus + the three phase currents, the
  current duty, the last accepted interval, and the trip timestamp. That's the
  "did current/lock deteriorate before sag" evidence, and it costs a 40-byte
  ring the DMA handler already has the data for.

Evidence it would have worked: E872's record shows **110 individual low-bus
samples across the run** before the block average finally tripped at 51 s, and
the coast trace shows the bus at 8.8 V at gate-off recovering to 11.4 V in 2 ms
— a 25% collapse that a 5%/3-scan rule fires on in under a millisecond.

Keep the 50-scan block guard as the slow backstop. Don't touch its threshold.

## Barrier 1: the current actuator was not in force when the motor burned

Read E872's own line: *"current diagnostic was report-only; max residual 18812
against nominal allowance 17308."* The measured current exceeded the limit and
nothing acted — on the diagnostic image, at 50%, with the PSU at 4.5 A. The
goal text says "retain current limiting/foldback." It wasn't retained on the
image that climbed. Whatever else changes: **no powered image, diagnostic or
lean, runs with the current actuator off.** If a diagnostic build needs the
governor quiet to observe something, the observation is done at a duty where
the governor is provably inactive anyway, never on a climb.

Calibration first, as proposed — but the E872 residual was already 9% over
allowance in the *uncalibrated* counts, so the calibration is for setting the
motor-safe number, not for deciding whether to act.

Add an **I²t budget** as the motor-thermal guard (integrate current² per block,
budget from the motor's continuous rating, latched stop). A small outrunner at
no load taking >4.5 A for 23 s is ~50 W into copper with no airflow; the
average-current limit is a rate guard, I²t is the energy guard, and the burn
was an energy event.

## What E872 was — not a mystery

| duty | 60 s result | final controller interval | speed |
|---|---|---|---|
| 35% | clean | 217 ticks | ~1.54 keHz |
| 40% | clean | 195 | ~1.71 keHz |
| 45% | clean, no foldback | 177 | ~1.88 keHz |
| **50%** | bus fold at 51 s | cycles 646–665 µs | **~1.52 keHz — slower than 40%** |

Current went from "under 3.5 A nominal, no warning" at 45% to "PSU in CC at
4.5 A" at 50% while the **speed fell**. Duty up, speed down, current up by more
than a third: that is minz's July lesson word for word — *a speed drop under
more throttle is late-commutation braking, not V/f saturation.* The loop was
commutating late enough that a large share of the current did braking work
instead of turning the rotor; the motor was the resistor for 23 s; then the
PSU folded. Under the transit-surge rule this is a marginal lock that was
fatal to the motor instead of to the run — because nothing stopped it.

Two firmware consequences beyond the two barriers:

1. **No-load sanity stop: speed must not fall while duty rises.** At no load,
   current and speed both increase monotonically with duty. A settled speed
   *lower* than the previous rung's while duty went up is braking by
   definition. Report at first, then stop — it would have fired at the top of
   the 50% ramp, not at 51 s.
2. **The `level-revisit` rescue is a suspect, not a win.** It "cleared" 35–45 by
   accepting ~3% of events the persistence had rejected (E868–871: every
   attempt accepted). An acceptance path whose whole purpose is to admit
   marginal edges is a plausible source of late commutations at 2 kHz. A/B at
   45%: revisit on vs off, compare no-load current at matched speed. If current
   is lower with it off, it was buying duty with braking.

And still: **stock AM32 at ~1.7–1.9 keHz on this rig, PSU current read.** The
reference at 30% pulled under 0.8 A. If it pulls under 2 A at 1.9 keHz and binz
pulls 3.5+, the whole 35–50% band is a timing defect measured in amps, and the
new motor should not be asked to climb it until that number moves.

## Order

Fast sag stop (DMA, 5%/3-scan/latched, with pre-trip record) → current
actuator mandatory on every powered image → I²t → speed-monotonicity report →
resume climbing only with the replacement motor's current logged at every
settled rung → revisit A/B at 45% → AM32 matched-speed current.

— the minz graybeard
