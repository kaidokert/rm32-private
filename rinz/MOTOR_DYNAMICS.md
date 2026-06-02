# Motor Dynamics — Open-Loop Six-Step Lock-In Observations

Observations from fine-resolution amplitude sweeps using `bemf_sweep.py` on a 3-phase
BLDC motor driven by B-G431B-ESC1 (STM32G431CB). All sweeps: open-loop six-step,
ceiling-first then descending, 11 samples per step, 0.1% amplitude resolution.

Motor supply: ~6 V bench PSU. Amp values are the internal `amp%` knob (actual PWM
duty ≈ amp × 2/3).

---

## Key Physical Observation: Discrete Stable Lock-In States

The motor phase-locks into discrete stable equilibrium angles relative to the
electrical cycle. These produce reproducible BEMF ZC positions within specific sectors.
Between stable states are dead zones where no valid ZC is detected.

Within a single lock-in state, the ZC index drifts continuously with amplitude: higher
amplitude → ZC earlier in the sector window (more phase lead); lower amplitude → ZC
later in the window (phase lag, approaching the window edge). The state itself
(which sector, which direction) does not change until the motor transitions to a
different equilibrium.

The stable state the motor settles into depends on initial conditions — particularly
the phase angle at the moment of frequency ramp-up. The same (freq, amp) target
reached via different paths can land in different sectors. Sweeping ceiling→floor
stays within a single equilibrium; re-ramping from another frequency can land
elsewhere.

---

## 130 Hz

Sweep range: 13.0% → 8.0%, step 0.1%, 11 samples/step.

### State A — s0, 12.9%–11.5%

| amp range   | sector | consistency | ZC index (of ~26) | ctr   |
|-------------|--------|-------------|-------------------|-------|
| 12.9%       | s0     | 11/11       | 7–9               | 0.59  |
| 12.8%–12.5% | s0     | 11/11       | 5–9 → sliding     | 0.48–0.54 |
| 12.3%–11.5% | s0     | 10–11/11    | 21–24             | 0.16–0.36 |

ZC drifts from ~8/26 (well centered) at 12.9% to ~23/26 (late, near window end) at
11.5% as amplitude falls. RAMP_FALLING=false for s0 (rising crossing).

Best in this state: **12.9%** — s0:11/11, ctr=0.59, ZC ≈ 7–9/26.

### Dead Zone — 10.4%–11.4%

Sporadic hits only: s0 fading, s2 not yet stable. 0–0.9 average in-window, no
sector reliable.

### State B — s2, 9.3%–8.0%

| amp range  | sector | consistency | ZC index (of ~26) | ctr   |
|------------|--------|-------------|-------------------|-------|
| 9.3%–8.0%  | s2     | 11/11       | 5–9               | 0.39–0.61 |

Stable across a wide amp range. ZC index stays tightly clustered (5–9/26) with no
significant drift. RAMP_FALLING=false for s2 (rising crossing).

Best in this state: **9.0%–9.3%** — s2:11/11, ctr=0.52–0.61, ZC ≈ 6–8/26.

### Summary — 130 Hz

Two stable states, one transition dead zone. State B is preferable: more centered,
consistent over a wider amp range, no index drift.

```
amp:   8.0──────── 9.3%   [State B: s2, stable]
       9.4──────── 11.4%  [Dead zone]
      11.5──────── 12.9%  [State A: s0, drifting]
      13.0%               [Sporadic, no stable state]
```

---

## 180 Hz

Sweep range: 16.0% → 10.0%, step 0.1%, 11 samples/step.

### State A — s4, 14.8%–16.0%

| amp range   | sector | consistency | ZC index (of ~18–19) | ctr   |
|-------------|--------|-------------|----------------------|-------|
| 16.0%       | s4     | 11/11       | 7–9                  | 0.83  |
| 15.5%–15.9% | s4     | 11/11       | 5–8                  | 0.56–0.80 |
| 15.1%–15.4% | s4     | 10–11/11    | 3–6                  | 0.41–0.59 |
| 14.8%–15.0% | s4     | 9–11/11     | 3–5                  | 0.36–0.42 |

ZC drifts from ~8/19 (centered) at 16.0% to ~3/18 (early, near window start) at
14.8% as amplitude falls. RAMP_FALLING=true for s4 (falling crossing).

Best in this state: **15.5%–15.8%** — s4:11/11, ctr=0.56–0.76, ZC ≈ 5–7/18.

### Dead Zone — 10.0%–14.0%

Complete dropout. Zero in-window detections across 40+ consecutive 0.1% steps. The
occasional single-sample hits (1/11, index 3/18) at scattered points are boundary
noise — index 3 is the minimum reportable and likely false detections at the window
edge.

No secondary stable state exists in this range.

### Summary — 180 Hz

One stable state, hard cliff at ~14.5%, then complete dead zone down to 10%.

```
amp:  10.0────────14.4%  [Dead zone — no valid BEMF]
      14.5────────14.7%  [Transition: 6/11 consistency]
      14.8────────16.0%  [State A: s4, stable]
```

**Note on sector variability:** The previous coarse scan (3 samples, 1% steps) found
s1+s2 at 14% for this frequency. This sweep found s4 at 14.8%–16.0%. Both are valid —
different approach conditions landed the motor in a different equilibrium state. The
coarse scan ended the 130 Hz sweep at its floor (8%–9%), then set amp to 16% and
ramped; this sweep did the same but the motor settled differently. The sector observed
is path-dependent.

---

## 210 Hz

Sweep range: 17.0% → 11.0%, step 0.1%, 11 samples/step.

### Upper Dead Zone — 16.2%–17.0%

Zero detections. ZC exits the window from below (ZC<0) — too much phase lead at high
amplitude pushes the crossing before the observation window opens.

### State A — s4, 15.0%–16.0%

| amp range   | sector | consistency | ZC index (of ~16) | ctr   |
|-------------|--------|-------------|-------------------|-------|
| 16.0%       | s4     | 11/11       | 7–10              | 0.92  |
| 15.9%       | s4     | 11/11       | 7–9               | 0.93  |
| 15.5%–15.8% | s4     | 11/11       | 5–8               | 0.67–0.88 |
| 15.0%–15.4% | s4     | 11/11       | 3–6               | 0.55–0.70 |

ZC drifts from ~8/16 (center) at 16.0% to ~4/16 (early) at 15.0%. Peak centeredness
is at **15.9%** (ctr=0.93). RAMP_FALLING=true for s4 (falling crossing).

Best in this state: **15.9%** — s4:11/11, ctr=0.93, ZC ≈ 7–9/16.

Same sector (s4) as the 180 Hz sweep — the motor carried its phase equilibrium
unchanged through the frequency ramp from 180 Hz.

### Transition — 14.0%–14.9%

Consistency degrades to 4–10/11, ZC stuck at index 3/16 (window start). Below ~14.5%
the ZC can no longer reliably be detected before the window ends.

### Lower Dead Zone — 11.0%–13.7%

Complete dropout. Isolated 1/11 hits at index 3 are boundary noise. One s0:1/11 hit
at 11.9% (index 14/16) is a stray detection in a different sector — not a stable state.

No secondary state was found below the dead zone within the sweep range.

### Summary — 210 Hz

```
amp:  11.0────────13.7%  [Dead zone]
      13.8────────14.9%  [Transition, degraded]
      15.0────────16.0%  [State A: s4, stable — peak at 15.9%]
      16.2────────17.0%  [Upper dead zone: ZC<0, too much phase lead]
```

**Note on sector variability:** The coarse scan (different run) found s1+s2 at 14% for
this frequency. This fine sweep found s4 (carried from the 180 Hz state). Both reflect
real equilibria — the state reached depends on approach conditions.

---

## 270 Hz

Sweep range: 19.0% → 13.0%, step 0.1%, 11 samples/step.

### Upper Dead Zone — 17.0%–19.0%

Zero detections across 20 steps. ZC<0 (excessive phase lead). Same mechanism as 210 Hz
upper dead zone; at 270 Hz it is wider.

### State A — s4, 14.9%–16.6%

| amp range   | sector | consistency | ZC index (of ~12–13) | ctr   |
|-------------|--------|-------------|----------------------|-------|
| 16.6%       | s4     | 11/11       | 7–8                  | 0.83  |
| 16.0%–16.5% | s4     | 11/11       | 5–8                  | 0.87–0.97 |
| 15.5%–15.9% | s4     | 11/11       | 4–7                  | 0.82–0.94 |
| 15.0%–15.4% | s4     | 11/11       | 3–6                  | 0.69–0.84 |
| 14.9%–14.6% | s4     | 9–11/11     | 3–5                  | 0.56–0.67 |

Peak centeredness at **16.0%** — ctr=0.97, ZC at 6-7/12-13 (essentially the geometric
center of the window). RAMP_FALLING=true for s4 (falling crossing).

Same sector (s4) as 180 Hz and 210 Hz. The motor maintained its phase equilibrium
through two consecutive frequency ramp-ups (180→210→270 Hz).

Best in this state: **16.0%** — s4:11/11, ctr=0.97, ZC ≈ 6–7/12.

### Degraded Region — 13.0%–14.5%

No complete dead zone. Sector remains s4, but consistency falls to 1–10/11 and ZC
index is stuck at 3/12-13 (window start). This is a gradual degradation rather than
the hard cliff seen at 180 Hz and 210 Hz. Below 13.0% the sweep floor was reached
without finding a clean dead zone.

### Summary — 270 Hz

```
amp:  13.0────────14.5%  [Degraded: s4 inconsistent, ZC at window start]
      14.6────────16.6%  [State A: s4, stable — peak at 16.0%]
      16.7────────17.0%  [Transition]
      17.0────────19.0%  [Upper dead zone: ZC<0]
```

**Note on sector variability:** The coarse scan (different run) found s1+s2 at 13%
for this frequency. This fine sweep found s4 throughout (carried from the 210→180 Hz
chain). The s1+s2 result would have been reached via a different approach sequence.

---

## Cross-Frequency Patterns

### ZC index drift direction

Within any lock-in state, decreasing amplitude moves the ZC toward the end (high
index) of the window. Increasing amplitude moves it toward the start (low index). This
is consistent with the motor's phase lag increasing at lower drive levels.

At 130 Hz State A: ZC moves from index 8 to 23 (of 26) as amp drops 12.9→11.5%.
At 180 Hz State A: ZC moves from index 8 to 3 (of 19) as amp drops 16.0→14.8%.

Note the opposite direction: at 130 Hz the ZC drifts toward window end; at 180 Hz
toward window start. This reflects which stable equilibrium the motor has locked into
(s0/s2 vs s4) and which edge of the window it's approaching as drive weakens.

### Window boundaries: upper and lower dead zones

At sufficient amplitude the ZC occurs before the observation window opens (ZC<0 →
upper dead zone). At insufficient amplitude the ZC occurs after the window closes
(ZC> → lower dead zone). The stable operating band sits between these two limits.

| freq   | lower dead/degraded | stable band     | upper dead zone | peak ctr |
|--------|---------------------|-----------------|-----------------|----------|
| 130 Hz | 10.4%–11.4%         | 11.5%–12.9% (s0), 8.0%–9.3% (s2) | none seen | 0.61 |
| 180 Hz | 10.0%–14.4%         | 14.8%–16.0% (s4) | none seen      | 0.83     |
| 210 Hz | 11.0%–13.7%         | 15.0%–16.0% (s4) | 16.2%–17.0%   | 0.93     |
| 270 Hz | 13.0%–14.5% (soft)  | 14.9%–16.6% (s4) | 17.0%–19.0%   | 0.97     |

Peak centeredness improves with frequency — at 270 Hz the ZC lands almost exactly at
window center. The upper dead zone (ZC<0) appears at 210 Hz and widens at 270 Hz,
consistent with the shorter electrical period leaving less room for phase lead.

Dead zones appear to widen with frequency, and lower-frequency points tend to have a
second stable state below the dead zone.

### Sector rotation with amplitude

At 130 Hz, the two stable states occupy s0 (high amp) and s2 (low amp) — two sectors
apart. The motor physically shifts equilibrium angle by ~60° electrical between states.
At 180 Hz only one state was found. Whether this reflects the motor/load combination or
the sweep range needs further investigation.

---

## Operational Recommendations

For reliable open-loop BEMF observation at each frequency:

| freq   | amp    | sector | ctr  | notes                              |
|--------|--------|--------|------|------------------------------------|
| 130 Hz | 9.1%   | s2     | 0.55 | State B — most stable, single sector |
| 180 Hz | 15.6%  | s4     | 0.66 | widest stable band                 |
| 210 Hz | 15.9%  | s4     | 0.93 | tight band, excellent centering    |
| 270 Hz | 16.0%  | s4     | 0.97 | peak centering, approaching stall below 14% |

The script's automatic best-pick chose 9.5% for 130 Hz (avg_iw=1.7 from two partially
overlapping sectors during state transition), but 9.1% is the better operating point:
s2:11/11, single sector, no ambiguity.
