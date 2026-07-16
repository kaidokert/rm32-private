# CONSTANTS AUDIT — 5-agent review of the push75 build (2026-07-14)

Five independent Opus reviewers swept every file in the flashed build
(core/src/*, examples/motor_tester2.rs, src/ hardware modules) for
constants, delays, and guards that inhibit top performance. Reference
regime: interval 84–95 µs (1750–1980 Hz), 2.1 A, 24 kHz carrier,
~2.2 PWM cycles per window; failure signature = NOZ-window bursts →
blind free-run → 11–14 A ramp. Precedent template: AMP_MAX, the 24 µs
LPTIM floor, INT_MIN=100.

## TIER 1 — the transit-death mechanism (independent convergence)

### 1. The reacq trap: recovery state structurally cannot accept a ZC
(agents: core-timing + guards, independently)
- `window.rs:170,382` — reacq trips on the FIRST isolated A/B miss
  when interval < HIGH_SPEED_US(160): always, in-band. But isolated
  misses are documented-normal (0.2–2.2 %/window) and are supposed to
  be ridden through.
- `zc.rs:147-155` — in reacq below TOPEND_US(125), SWIFT AcceptNow is
  disabled (the 2026-07-12 fix) → candidates go to the wrap-confirm
  path.
- `timing.rs:109` — `confirm_need(reacq)=2` wraps = 83.3 µs. A
  candidate armed mid-window (~40 µs in) has ≤1 wrap before
  `close_float_window` clears it (`window.rs:228`). **Two confirms
  do not fit inside an 84–95 µs window.** The reacq state therefore
  produces consecutive NOZ windows by construction — the exact
  cascade signature. The recovery mechanism IS the wall.
- Compounding: the reacq re-seed (`estimator.rs:100`) demands
  spans==1, but dead-reckoned C windows force spans==2 every third
  window; and the reacq gate `(interval*2/25).max(10)` is FLOORED at
  10 µs in-band = 11–12 % (never the designed 8 %).
- Experiments: (a) keep AcceptNow in reacq at top end, or
  confirm_need=1 when reacq && interval<TOPEND; (b) trip=2
  unconditionally (or trip=1 only below ~70 µs).

### 2. Comparator deaf-span discards the recovery edge
(agent: firmware-ISR)
- `motor_tester2.rs:2890` — `comp2::set_exti_enabled(true)` is the
  LAST statement of the LPTIM2 ISR, after `close_float_window`
  (~7.2 µs in `free`) — deaf span ≈ 9–10 µs from FET switch to
  ear-open, and the re-arm CLEARS PENDING, so an early ZC latched
  during the deaf span is DISCARDED, not delayed.
- The reacq gate floor is 10 µs — the deaf span consumes essentially
  the entire early-catch region that reacq widening exists to admit.
- Experiment: hoist the re-arm above `close_float_window` (same
  priority level ⇒ COMP tail-chains, cannot preempt — safe).

### 3. ~5 µs of the 7 µs ZC→shot latency is two fixed busy-waits
(agents: firmware-ISR + hardware, independently)
- `lptim2_oneshot.rs:73` — `asm::delay(200)` (2.5 µs) + bounce +
  ARROK poll ≈ 3 µs on the ZC-REFINE path only (`schedule_us`, called
  from the accept at motor_tester2.rs:3359). The count starts AFTER
  the delay is computed → ~3 µs uncompensated lateness on every
  refined commutation, while free-run steps (`reschedule_light`) have
  no such wait → alternating late/on-time = divergence seed.
- `motor_tester2.rs:3258` — persistence loop 12 × `delay(8)` + CSR
  read ≈ 2.4 µs in-band (blank<5 forces 12 reads; the 5-read fast
  regime never engages below 375 µs intervals).
- `lptim2_oneshot.rs:67,102` — `us*5/4` truncation: 0..−0.8 µs
  ALWAYS-SHORT bias on every free-run re-arm (accumulates during
  blind runs). Also hw clamp min=4 makes the logic floor
  LPTIM2_MIN_DELAY_US=2 dead (and its TIM15 narrative comment is
  stale — the TIM15 swap was deliberately reverted; docs still tell
  the TIM15 story).
- Experiments: subtract a measured SCHEDULE_LATENCY_US (~3) inside
  the refine path; cap persistence at 5 under CL lock; round the
  tick conversion.

## TIER 2 — guard collisions

### 4. Runaway floor 60 µs vs the estimator's own legal bounds
(agents: guards + numerology, independently)
- `guards.rs:55` kills interval<60 µs. The accept bound permits
  0.6×old and the reacq re-seed 0.5×old: at 84 µs one LEGAL fast
  accept = 50 µs, one legal re-seed = 42 µs → instant Runaway kill.
  The collision exists for ANY interval < 100 µs, i.e. the whole
  operating band. This is the transit-death endgame: misses → reacq
  re-seed from aliased ZCs → sub-60 → kill.
- Also the true ceiling for any future push: est INT_MIN=40 headroom
  is dead code above 2778 Hz.
- Experiment: log interval at every Runaway kill in a transit; if
  40–58 µs, lower floor to ~45 and align the bounds.

### 5. Blind-amp clamp cannot tell a transit from a cascade
(agent: guards)
- `guards.rs:244` cuts amp ×2/3 at noz_run≥3, ×1/3 at ≥6 — during a
  throttle transit this removes torque exactly while acceleration
  needs it → BEMF drops → ZCs harder → deeper clamp. Positive
  feedback against the climb.
- Experiment: disable the clamp for one ladder; if transits complete,
  it is fighting them.

### 6. Carrier-relative failsafe timing silently rescaled 48→24 kHz
(agents: guards + numerology)
- SAG_DEBOUNCE=64 samples: doc says 1.3 ms, reality 2.67 ms at
  24 kHz. OC window 2^11: 85 ms — blind to 10–18 ms bursts entirely
  (a 14 A/15 ms burst averages under the trip). Net: the sag guard
  owns ALL transit kills, at double its designed latency; nothing
  clamps a burst at onset.
- Experiment: size both in ms not samples; consider a short-window
  (~2 ms) peak-current trip to catch the monster at onset.

## TIER 3 — smaller / cleanup

- `timing.rs:84` blank `interval/75` = 1 µs everywhere in-band,
  truncates to 0 (silently OFF) below 75 µs. 48 kHz-era divisor.
- `guards.rs:52` starvation `.max(500)*12` = 6 ms ≈ 66 windows at
  90 µs (doc says "12 intervals"); moot at speed (sag fires first).
- `motor_tester2.rs:2894` TIM1_CC at up to 72 kHz does a u64/80
  division per fire (`ticks_1us`) ≈ 7–8 % of the core, to maintain a
  blank timestamp that in-band gates a 1 µs window. Store raw CYCCNT
  instead, or kill TIM1_CC (lever #3).
- Per-commutation duty recompute + AMP_MAX=75 clamp on the LPTIM2
  critical path (~25 cyc; precompute).
- Stale comments: TIM15 narrative in timing.rs:40-50, "48 kHz"
  vintage comments on SAG/OC/blank, lptim2 doc clamp range.

## Explicitly cleared

Slew 1 %/50 ms is NOT too sharp (a 5 % transit = ~2750 commutations;
~550 per 1 % step) — the operator's gradient hunch is quantifiably
not the killer, per the guards agent. DWT 64-bit timebase audited
clean (extender, wrap, truncation, compose — no silent timebase
fault). ADC injected sequence fits the ON window with margin at
70–75 % duty. TIM1 stale-CCR fix airtight. Advance ramp healthy
in-band (17°), delay not floored on the fast path. Priorities
consistent; decimation coprime; counters saturate safely.

## Suggested experiment order (each cheap, one variable)

1. E1: hoist comp re-arm above close_float_window (one-line move).
   **TRIED 2026-07-15 — REGRESSED, REVERTED.** Interleaved vs control:
   E1 0/3 transits to amp 70 (one death even climbing to 60) vs
   control 2/2 (one full 60→72 pass). Mechanism: EXTI has no latch
   timestamp — a preserved edge is stamped when the COMP handler
   finally runs (~10-20 µs post-commutation), late enough to clear
   the 10 µs gate floor, so E1 converted DISCARDED glitches into
   ACCEPTABLE premature edges. The end-of-ISR pending-clear is
   protective, not a bug. Any fix here needs a hardware timestamp
   (TIM input capture), not an ordering change.
2. E2a: confirm_need=1 in reacq below TOPEND (trip left at 1 — with
   the confirm achievable, first-miss reacq entry is benign).
   **LANDED (049a848) — WIN: 3/3 full ladders to 72 vs control 1/2.**
3. E3: LPTIM2_FULL_SCHEDULE_OVERHEAD_US=3 folded into refine elapsed.
   **LANDED (c2e9404) — bench-neutral at n=3, kept for correctness.**
4. E4: persistence 12→5 under established CL lock.
   **LANDED (5337e60) — WIN: first-ever amp-74 rungs, 2/2.**
5. E5: runaway floor 60→45.
   **LANDED (f96bd98) — WIN: full ladders to 75 = AMP_MAX, 3/3,
   qzc 100 %, 1931 Hz, jitter 4.8 % at top. Envelope = the clamp.**
6. E6: REFRAMED as doc-truth (3308eec) — the 2.67 ms sag debounce is
   the bench-proven behavior every 24 kHz result was earned on;
   comments corrected, behavior kept. The short-window peak trip
   stays on the backlog (OC 85 ms is blind to 10-18 ms bursts).
7. E7: blind-amp clamp off for one ladder (diagnostic) — NOT RUN,
   moot at the current envelope (nothing left to climb below
   AMP_MAX); revisit if a raised clamp reopens transit deaths.

### Post-audit lever verdicts (2026-07-15 night, the 85 campaign)

- **Advance-boost extension (finding 5 / lever #3): TRIED — REGRESSED,
  REVERTED.** Boost cap 6→10 (17°→20-21° in the 83-95 µs band): 3/3
  ladders died BELOW the baseline's proven rungs (76→74), with the
  over-advance signature — current UP 0.2-0.3 A at the same rung
  (field ahead of rotor). The +6 saturation is correct for this
  motor at bench load; the jitter rise past amp 72 is the current
  saturation, not gate-margin erosion. "Advance is not a lever until
  real load" survives another test.
- **Commutation-ISR split stage A (lever #2): TRIED — NO WIN,
  REVERTED.** Deferred the only cs-requiring step (shared PEND
  enqueue) to a TSC software interrupt at prio 3; control kept at
  prio 1 (the E1 conviction + reacq-trip latency both forbid
  deferring control). Result: lp2 dur UNCHANGED (~1663 vs 1637-1724)
  — the deferred work only ran on 1-in-5 closes (telemetry
  decimation), so the average win was noise; 40-60 qualification
  clean but top-band paired runs showed split ≤ control. The v1
  lesson again: measure the segment before plumbing around it. A
  REAL blind-time cut requires deferring control (window_control_step
  + reacq trips) — E1-class risk, only worth it with a hardware
  timestamp (TIM input capture) for the COMP edge.
- **The 80/85 wall is not software.** Every death at 78+ across
  burst/boost/split builds is current-domain: the 85 ms OC-AVERAGE
  trip fired at a sustained 78 dwell (2.5 A steady, over the
  continuous envelope) and the burst responder logged 48
  engagements/run there. 85 needs bus stiffening / supply headroom /
  drive efficiency, not control changes. Campaign stopped per
  operator directive.

Final state: tag `checkpoint-24k-audit75`. The binding limit is the
AMP_MAX=75 duty clamp (operator's bench-current call). Remaining
CPU/protection backlog: TIM1_CC u64 division at 72 kHz (~7 % core),
short-window current trip, blank divisor /75 re-tune.
