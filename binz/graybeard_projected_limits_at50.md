# Graybeard — projected limits above 50%, written at 47.5% (2026-09-24)

**Status of this document: a prediction, made before the evidence.** It is
kept so it can be vetted rung by rung. Every claim below is either *measured*
(with its source) or *projected* (marked as such). When the bench gets there,
strike what was wrong and leave the strike visible — the value of this file is
the diff between what the graybeard expected and what happened.

## Frame

The question was "what is the barrier to 100% throttle on this MCU." The
operator's reminder frames the answer: **AM32 ships in production on
STM32F050 (Cortex-M0, 48 MHz, no prefetch to speak of) and on this exact G071
at 100% throttle on 4S packs.** Whatever the barrier is, it is not the core.
A 64 MHz M0+ running a ~5 µs comparator ISR has been doing this job in
quadcopters for years. So the prediction is not "where the MCU runs out" but
"which of *our* choices runs out first, and what the bench can physically
supply."

## Measured today (firmware50 image 63EC7EFE, 47.5% qualified)

| quantity | value | source |
|---|---|---|
| speed at 47.5% | ~2.0 keHz, +3.3% vs oracle | E147–E151 |
| edge→arm (`spent_max_us`, partial) | 11 µs | WCET_ESTIMATES.md |
| one-shot firing late (`com_late_max_us`) | 8–11 µs | E152 |
| wait at level 22, ci ≈ 80 µs | ~15 µs | arithmetic |
| late arms | 4 of 588 captures (one in a qualifying 45% run) | E152 |
| supply | 1.75 A cap; 0.89 A metered at 37.5% | operator, 2026-09-22 |
| current law | I ∝ duty², 0.399 A @ 25% → 0.89 A @ 37.5% (−1%) | two metered points |
| sharp-sag trips at 50% | 3 of 6 runs, cause unresolved | E151 |

## Projections (each marked P#, to be vetted)

**P1 — Speed at 100%, no load: ~3.4–3.5 keHz, ≈ 48 µs per commutation.**
From the crate's own speed-vs-duty fit (WCET_ESTIMATES.md). Vet: coast BEMF
frequency at the top rung.

**P2 — Current at 100%, no load: ~6.4 A.** `0.399 × (100/25)²`. The duty²
law is really a speed² law for iron/eddy loss, so this holds only while the
loss is iron-dominated; copper heating will bend it. Vet: PSU display at
60 / 75 / 88%. Consequences if P2 holds: 1.75 A → ~48%; the 5 A bench supply
→ ~88%; 100% needs the battery. **The bench supply, not the MCU, is the first
wall above 50%.**

**P3 — The motor is the second wall.** ~75 W of mostly iron loss into a small
outrunner with no airflow. 100% no-load on this motor is a heater run measured
in seconds, and a prop raises the current further. This is a property of the
bench motor at 11.7 V, not of the firmware; stock AM32 would draw the same.
Vet: motor case temperature vs dwell time at 75%+; expect a hand-hot case in
tens of seconds.

**P4 — The timing chain is the only code wall, and it arrives between 50% and
60%.** At 3.5 keHz a wait at level 22 is ~7.5 µs; the chain today is
11 µs + 8–11 µs. The commutation lands after its intended instant on every
crossing and the advance is eaten by latency. Two exits, both legitimate:

- *AM32's exit*: accept it. When the wait is shorter than the ISR, the one-shot
  fires at ISR exit and the commutation is late by the latency; the reference
  keeps that loss small with a ~5 µs body (F050 included). Copying it means a
  COMP root of roughly: timestamp, 2–3 persistence reads (the mapped filter at
  this speed), one subtract, one timer write. Predicted achievable: ≤5 µs.
- *The hardware exit*, which the G071 supports and neither AM32 nor rm32 uses:
  COMP2 output → TIM2 input capture (hardware-stamped crossing, digital filter
  for chatter) and TIM1's commutation event (`CCPC`, `COMG`/TRGI) latching the
  preloaded next-sector CCER/CCMR from the one-shot. Software computes
  schedules; nothing in software touches the commutation instant. Deletes
  `com_late` and the dispatch term outright.

Vet: `com_late` and edge→commutation per rung. If the chain is unchanged and
50–60% still qualifies, P4 was too pessimistic.

**P5 — CPU is not the wall.** At 3.5 keHz on today's bodies (COMP 1026 cycles,
COM 356, DMA 78, guard 421) the ISR load is ~55%. Tight, but the F050 at 48 MHz
runs the reference at this speed with room to spare. Vet: aggregate IRQ union
at 75%+ on the lean image.

**P6 — The small things, all from `GRAYBEARD_ROAD_TO_100.md`:** persistence
depth 2–3 by `map(avg, 100, 500, 3, 12)` (already implemented as
`MappedFilter`); duty capped at 2000/2047 ≈ 97.7% for bootstrap refresh —
there is no true 100% in the reference either; guards keyed in time, not
commutations (the 100 µs tick and 226 µs scan become slower than a
commutation); ADC rail samples at high duty as reports; no UART while locked
(done). Vet: none should be a rung-stopper if done before the climb.

**P7 — The sharp-sag trips at 50% are a timing event, not a supply event.**
Dips are flat −7 to −8.7% from 37.5 to 50% with no current trend (E152), so
they are transients. Prediction: they correlate with `com_late` extremes or
late arms per commutation, and shrink when the chain does. Vet: the
per-commutation `com_late` ring against the sharp-sag scans — the instrument
the three-way review converged on.

**P8 — "Advance level" will stop being the comparison quantity.** The oracle
at level 26 with a ~32 µs COMP body and firmware50 at level 22 with an 11 µs
body may already have the *same effective commutation angle*. Prediction: when
`(commutation − crossing)/interval` is measured on both images, they agree
within a few percent at 37.5% and the "refused advance 26" divergence
dissolves. Vet: the like-for-like angle, both images, one afternoon.

## What the graybeard expects the vetting to show

- P2/P3 hold and are the first things that actually stop the climb: the supply
  at ~48% on 1.75 A, then motor heat well before 100%.
- P4 is right in mechanism but the crate reaches 60% on level 20–22 with
  today's chain before it bites hard.
- P5 holds.
- P7 is the one I'd bet against myself on — this bench has surprised every
  timing hypothesis so far.

## What this file must not become

Not a wall. Every earlier "wall" on this bench — 6.9%, 45%, 342 eHz, the
supply, the resistors, "lock-limited" — was code, a guard, or an instrument.
The reference runs 100% on weaker silicon. If a rung fails, the first question
is which of *our* terms failed, and the second is what the oracle does at that
speed — flash it, read it, flash back.

— the minz graybeard
