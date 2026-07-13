# Timing-margin analysis — the amp-68 / ~1850 Hz ceiling (2026-07-13)

Data gathered to analyse why the closed loop breaks at ~amp 67-68 /
~1850 Hz. Short version: **it is a commutation-timing-margin collapse
on the SCHEDULE side, not the supply.** The VBAT SAG KILL is the guard
that fires last; the amp draw (an 11 A ZC-miss monster) is the cause,
the bus sag is the effect.

Board: L431, 24 kHz PWM, 8 V bench, no caps. Build: TOPEND-gated
reacq-confirm + phase-C re-time, floor 8 µs, `AMP_MAX=75`.

## Cause-order (from the dropout, `night2_5070_t68.bin`)

`onset_probe` at the monster (window ~1130):

```
1129  s5  Y  int 80  qzc 39  i 2015   vbat 7.93   <- steady, clean
1130  s0  Y  int110  qzc 63  i 3035   vbat 6.86   <- phase-C window, LATE ZC, current rising
1131  s5  --         int130  ----    i 8462   vbat 5.41   <- first MISS, current explodes
1132+       cascade: ---- misses, interval thrashes 80<->160, i 7-11 A, vbat 5-6 V
```

Timing goes first (a late phase-C ZC → the next window misses), THEN
current runs away, THEN the bus sags. The bottom panel of
`night2_5070_dropout.png` shows the precursors clearly: small 3-5 A
single-window spikes scattered through the whole dwell (recovering),
growing, until one triggers the full cascade.

## The two margins at amp 66 (steady, 91 µs window, 1825 Hz)

### SCHEDULE side — EXHAUSTED

`gecko_load --bb` ACC commutation delays, and a temp pre-floor probe:

| quantity | value | note |
|---|---|---|
| delay budget = interval·(30−adv)/60 | **15.2 µs** | adv = 20° (auto_advance capped) |
| `elapsed` (ZC detect → schedule call) | **~10 µs** | ISR + persistence + estimator + confirm; a FIXED cost |
| ideal delay (pre-floor) = budget − elapsed | **~5 µs** | measured 4-6 µs, all 6 sectors |
| LPTIM2 floor (`LPTIM2_MIN_DELAY_US`) | **8 µs** | ideal 5 clamped UP to 8 |
| → commutation fires | **~3 µs / 12° LATE, zero slack** | |

ACC delay is **100 % floored at 8 µs from ~amp 58 through 66**
(at amp 48 it was still 9-13 µs; amp 50 ~8-9). So the schedule margin
runs out around amp 55 and is fully gone by 58.

The killer is two FIXED costs — **~10 µs elapsed latency + the 8 µs
floor** — against a delay budget (15 µs at amp 66) that keeps shrinking
with speed. Once the budget minus elapsed drops below the floor, every
commutation is clamped and there is no slack to absorb a perturbation.

### DETECTION side — still healthy (~3σ)

`owl_report` per-sector at amp 66:

- qzc_off **~48 µs** (53 % of the 91 µs window), σ 6-7 µs
- gate = interval·3/10 = **27 µs** → ~21 µs margin ≈ 3σ
- qzc **100 %** steady, all sectors even; pred_err bias −4 µs (the late
  commutations show up here)

So the ZC is NOT crowding the gate in steady state. But with 3σ ≈ the
whole margin, a perturbation that shifts the schedule (which the floored
late commutation does) pushes the next ZC toward the gate.

### Phase C is the shakiest predictor

`owl_report` on the monster transit (`t68`), err/win per sector:

| sector | phase | err/win |
|---|---|---|
| 0 | **C** | **3.4 %** |
| 3 | **C** | **3.6 %** |
| 4 | B | 3.8 % |
| 1 | B | 4.2 % |
| 2 | A | 1.5 % |
| 5 | A | 1.4 % |

Phase C (0/3) is re-timed from the comparator but has no ADC cross-check
(sectors 2/5 = phase A are the tightest). The monster **initiated at
sector 0** (phase C). C is where the zero-slack schedule gets its first
perturbation.

## Synthesis

At ~1850 Hz the commutation-timing margin is eaten by two fixed costs
(~10 µs `elapsed` + 8 µs floor ≈ 18 µs) against a ~15 µs delay budget
that shrinks with speed. The schedule is fully floored (no slack), the
detection margin is ~3σ, and phase C is the weak sector. A late phase-C
ZC → the next window's ZC drifts toward the gate → miss → ZC-miss
cascade → 11 A monster → bus sag. Same monster mechanism as amp-42, now
at 85-91 µs windows where the fixes (tuned at ~130 µs) can't catch it.

## Levers (for the analysis — not yet done)

1. **Cut the `elapsed` latency (~10 µs)** — the biggest fixed cost. Parts:
   the persistence loop, the estimator work before the elapsed read, the
   confirm, and the LPTIM2 setup (bounce + ARROK + delay(200), ~3 µs).
2. **Lower the delay floor (8 µs)** — GP-timer one-pulse (TIM7/TIM16)
   removes the LPTIM overhead; parked per operator.
3. **Advance vs floor tension**: advance 20° shrinks the delay budget to
   15 µs. Lower advance → bigger budget (less floored) but the ZC crowds
   the gate (worse detection). They trade against each other at the top.
4. **Phase-C robustness** — it initiates the cascade; a tighter C
   confirm or dead-reckon accuracy would raise the perturbation floor.

## FREQUENCY-DOMAIN model — the ceiling is an f_e limit, not a motor/throttle limit

The controller's limit is set by FIXED latencies vs the shrinking window,
so it lives in **electrical frequency**, independent of the motor Kv or
the supply voltage. A higher-Kv motor or more volts just reaches the same
f_e ceiling at LOWER throttle; this motor at 8 V happens to hit it at
~amp 66-68. Window `T = 1e6/(6·f_e)` µs.

### Fixed latencies (measured; do NOT scale with T)

| cost | value | what it is |
|---|---|---|
| `elapsed` | **~15 µs** (12-18) | ZC-ISR-entry → schedule-call: persistence loop + estimator + confirm + the reads |
| LPTIM2 commutation ISR | **~22 µs** | set_six_step + mux + close_float_window + reschedule; shares prio 1 with COMP, so it's BLIND time — a ZC landing here waits |
| delay floor | **8 µs** | `LPTIM2_MIN_DELAY_US` (bounce + ARROK + delay(200)) |
| ZC jitter σ | **~7 µs** | roughly constant in µs |

### T-dependent budgets

- window = `T`; gate = `0.30·T`; ZC→commutation delay budget =
  `T·(30−adv)/60`. Advance auto-ramps to a 20° cap, so at speed the
  budget is `T·10/60 = T/6`.

### Where each margin hits zero

| margin | condition | T | **f_e** |
|---|---|---|---|
| schedule delay starts flooring | `T/6 − elapsed < 8` | ~120 µs | ~1400 Hz (obs. amp 55-58) |
| **schedule budget = elapsed (goes negative)** | `T/6 = 15` | **90 µs** | **~1850 Hz** |
| detection margin < 3σ | `0.23·T < 21` | ~91 µs | **~1830 Hz** |

**Both walls land at ~1850 Hz** — and it's the SAME cause: the ~15 µs
`elapsed` is (a) the whole delay budget at T=90, and (b) via the
resulting late commutation, what pushes the next ZC toward the gate.
This is exactly the observed break (amp 68 ≈ 1850 Hz). The +22 µs
commutation-ISR blind time (24 % of a 90 µs window) is the aggravator
that turns a phase-C wobble into a miss.

### Implication for the 95 % target and other motors

- Projected 95 % on THIS motor/voltage ≈ **2000-2200 Hz** (user eyeball).
  That is ABOVE the ~1850 Hz timing ceiling → **the controller can't
  hold 95 % here today, and it would break at ~1850 Hz on ANY motor**
  (higher-Kv / more-volts just arrive there sooner).
- For robustness we want headroom well above 2200 — say a 2500-3000 Hz
  ceiling — so a hotter motor or a fresh battery doesn't walk into the
  wall.

### Levers, sized to a target ceiling

Ceiling ≈ where `T/6 ≈ elapsed` (+ the floor and detection track it):

| target f_e | T | need budget `T/6` > elapsed → elapsed ≤ | how |
|---|---|---|---|
| 1850 (now) | 90 | ~15 µs | current |
| 2200 (95 %) | 76 | ~11 µs | cut elapsed ~4 µs |
| 2500 | 67 | ~9 µs | cut elapsed ~6 µs + floor→~4 |
| 3000 (margin) | 56 | ~7 µs | cut elapsed ~8 µs + floor→~2 (TIM16) |

Reducible pieces of `elapsed` (~15 µs): the persistence loop (12 reads
at speed), the estimator work done before the elapsed read, and the
LPTIM2-ISR blind time bleeding into it (the "LPTIM diet" lever). Plus the
8 µs floor (GP-timer one-pulse, parked).

**One re-tuning lever that needs NO latency cut:** advance vs the two
margins. Advance at the 20° cap sets the budget to `T/6` (small) to buy
detection margin (ZC lands later, away from the gate). But at the ceiling
the SCHEDULE budget is the binding constraint. A lower advance at the
very top gives a bigger budget (`T·(30−adv)/60`) at the cost of detection
margin — there is an optimum advance-vs-f_e profile that balances the two
walls, and it currently over-weights detection. Worth a sweep: hold amp
~62-66 and step advance down, watch qzc% (detection) vs the ACC-delay
floor (schedule) — the crossover is the better cap.

## Data files (`captures/`)

- `night2_5070_map.png`, `_dropout.png`, `_a{50,56,62,66}.bin`, `_t68.bin`
- `marg{58,62,66}_*_bb.txt` (ACC delays floored at 8), `rawd66_*_bb.txt`
  (pre-floor ideal ~5 µs)
- `owl_report --infile` on the a/t bins for per-sector tables
