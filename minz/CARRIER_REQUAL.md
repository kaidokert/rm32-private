# CARRIER RE-QUALIFICATION — 24 vs 48 kHz after the audit era (2026-07-15)

> **ERRATUM (2026-07-17, deep source study):** the claim below that
> `variable_pwm` is an empty/vestigial block is WRONG. The empty
> `if(variable_pwm){}` at main.c:1714 is a decoy; the live logic at
> main.c:2131 scales the carrier 24→48 kHz with commutation interval
> (`tim1_arr = map(interval, 48us, 100us, ARR/2, ARR)`). The AM32
> reference runs therefore used a DYNAMIC carrier (~27 kHz at
> 1900 Hz, rising with speed). The fixed-24-vs-fixed-48 comparison
> below stands as measured, but AM32's operating point is the
> interpolation between them — see GAP_CLOSING_PLAN.md rung R4.

Goal: re-instance the 48 kHz build (cargo feature `pwm48`, default
24 kHz), qualify BOTH carriers in the amp 40–60 band, count artifacts,
measure timing, and recompute the margins — because every prior
carrier verdict predates the INT_MIN fix, the stale-CCR fix, the
monster fixes, the constants-audit E-ladder, and the engage-lottery
fix.

## What re-instancing took (all committed, 673355c)

1. **`pwm48` cargo feature** — one constant; everything timing-critical
   derives from `TIM1_AUTORELOAD` (trigger is absolute ticks: 1.25 µs
   both carriers).
2. **The 48 kHz build crash-looped at arm** (5 boot banners; IWDG at
   +1.06 s; main fully starved — could not even echo `i`). Two causes,
   both fixed:
   - **TIM1_CC** ran `ticks_1us()` (4 loads + u64 divide; 81 cyc idle,
     550+ under load) once per PWM cycle → ~33 % of the core at
     48 kHz. Now stores one raw `DWT.CYCCNT`; COMP compares the blank
     in cycles. cc dur 81→**27 cyc**. Benefits 24 kHz identically
     (post-fix 24 k control ladder: clean, no regression).
   - **blank 8 µs (the July-8 recipe) no longer survives arm** — the
     ISR set has grown since (hybrid 4-ch ADC, DWT extender+miss
     detector, CTX rings, EMA) and every non-blanked noise edge pays
     the pre-lock 12-read persistence spin. blank 8 = crash, blank
     **16** = arms and engages cleanly. `cl_lock_map --blank` added
     (echo-driven).
3. The engage-lottery fix carried over: 48 kHz engages on demand.

## Qualification data (40–60, 4 % rungs, 5 s dwells; spike = single-
## window i_max > median+1.5 A, clustered)

24 kHz (post-CC-fix control run `q24_ccfix`; two pre-fix runs agree):

| amp | f_e | qzc | spikes/s | window-σ | current |
|---|---|---|---|---|---|
| 40 | 1193 | 100 % | 0.2 | 4.0 % | 0.57 A |
| 44 | 1294 | 100 % | 0.0 | 4.4 % | 0.90 A |
| 48 | 1400 | 100 % | 0.0 | 4.9 % | 1.10 A |
| 52 | 1493 | 100 % | 0.0 | 5.9 % | 1.23 A |
| 56 | 1580 | 100 % | 0.0 | 6.5 % | 1.40 A |
| 60 | 1661 | 100 % | 0.0 | 6.4 % | 1.70 A |

Ladder robustness: 3 of 4 runs fully clean; one mid-dwell break at 44.

48 kHz (runs `q48_2`, `q48_4`):

| amp | f_e | qzc | spikes/s | window-σ | current |
|---|---|---|---|---|---|
| 40 | 1112–1115 | 100 % | 0.0–0.4 | 5.1 % | 0.54 A |
| 44 | 1194–1196 | 100 % | 0.0–0.2 | 6.7 % | 0.65 A |
| 48 | 1293–1296 | 100 % | 0.0–0.8 | 9.7 % | 0.78 A |
| 52 | 1378–1391 | ~100 % | 0.0–1.0 | 7.7–11.3 % | 0.94 A |
| 56 | 1482–1485 | 100 % | 0.0 | 8.4 % | 1.13 A |
| 60 | 1520–1568 | 99–100 % | 0.8–3.9 | 6.3–15.9 % | 1.34 A |

Ladder robustness: 1 of 3 fully clean; one broke during the 60 dwell,
one died on the 48→52 transit; the amp-50 timing-probe dwell also died
(sag to 5.97 V) between its two samples.

## Timing probes @ amp 50 under SWIFT lock

| metric | 24 kHz | 48 kHz |
|---|---|---|
| f_e / interval | 1455 Hz / 114 µs | 1366 Hz / 122 µs |
| comp IRQ rate | 33–37 k/s | **65 k/s** |
| tim1_up delivered | 17.7–21.2 k (of 24 k) | 33–42 k (of 48 k) |
| t1u cycles LOST | 5–9.5 k/s (~21–40 %) | 9.2–19.4 k/s (~19–40 %) |
| dur: t1u / cc / comp / lp2 | 355 / 27* / 413–483 / 1724 | 337 / 25 / 465–504 / 1637 |

*post-fix value.

## Margins, recomputed at interval ≈ 120 µs / amp 50

- **CPU** (rate × dur/80 MHz): 24 kHz ≈ comp 21 % + t1u 11 % + lp2
  18 % + t7 5 % + cc 1 % ≈ **56 %**. 48 kHz ≈ comp 37 % + t1u 20 % +
  lp2 17 % + t7 5 % + cc 1.5 % ≈ **81 %**. The doubled comparator
  noise-edge density (65 k/s) is the dominant new cost — each edge
  costs ~6 µs at priority 1, ahead of everything but LPTIM2.
- **Wrap quantum** (the old pro-48 k argument): 20.8 vs 41.7 µs. Now
  nearly moot: SWIFT accepts are continuous-time and E2a removed the
  only in-band 2-wrap dependency. The confirm paths that remain
  wrap-quantized (engage, low speed) are not speed-limiting.
- **OFF-gap ZC tax** (the other pro-48 k argument): halves at 48 kHz —
  but it also shrinks with duty at 24 kHz (1.7 µs at 96 %), and the
  measured window-σ says the 48 kHz build is 1.5–2× WORSE despite the
  smaller gap: the CPU-side losses (t1u misses, comp preemption of the
  close/confirm machinery) outweigh the analog gain.
- **Dead-time fraction**: 2×562 ns per cycle = 5.4 % of the 48 kHz
  period vs 2.7 % at 24 kHz → measured ~7 % lower f_e and ~20 % lower
  current per amp at 48 kHz (less effective drive per commanded duty).
- **Commutation ISR blind time**: ~20 µs, carrier-independent — the
  ~2.3–2.6 kHz wall is identical on both.

## Verdict — the July-13 carrier picture is OBSOLETE, inverted by the fixes

- 48 kHz's one decisive advantage was **ripple-seeded spike
  suppression** (4.5× fewer events, July-13 A/B). The spike root
  causes are now FIXED at the source (INT_MIN, stale-CCR, monster
  trio, E-ladder): 24 kHz in this band is **essentially spike-free
  (0.0–0.2/s)**. There is nothing left for the smaller ripple to
  suppress.
- 48 kHz's costs remain and grew: 2× comparator noise density (37 %
  of the core at prio 1), 2× t1u/cc rates against an ISR set that got
  heavier since July, double dead-time fraction (slower + weaker per
  amp), and a fragile arm (blank ≥16 required).
- Measured outcome: 24 kHz is better on **every** qualification axis
  in 40–60 — spikes, window-σ, speed per amp, current per amp, ladder
  survival.

**Recommendation:** 24 kHz stays the default and the production
recipe. Keep `pwm48` buildable (it now arms/engages/locks — a real
option again) but treat it as a contingency that only pays if (a) a
future regime shows ripple-seeded artifacts 24 kHz can't fix in
timing, AND (b) the CPU backlog lands first (MAGPIE serialize in
main, gate ADC-confirm under SWIFT, half-rate vbat/sag — worth ~15–20
points of core at 48 kHz). The runtime carrier switch drops in
priority accordingly.

Artifacts: `captures/q24_*`, `captures/q48_*`, probes in session
logs. Census/jitter tables generated by the magpie parser (spike =
median+1.5 A single-window clusters).
