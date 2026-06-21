# Finding the BEMF zero-crossing across regimes — a validated multi-harmonic reconstruction

Capstone for the open-loop BEMF observation effort. Builds on
[`OBSERVER_BEFORE_CONTROL.md`](OBSERVER_BEFORE_CONTROL.md) (why we keep the loop open
until observation is trustworthy) and [`FINE_SWEEP_320_400.md`](FINE_SWEEP_320_400.md)
(the lock/slip structure). This documents the detector that makes the BEMF zero
crossing trustworthy in every regime where the motor locks, and the data that
validates it.

## The problem

In open-loop six-step, the BEMF zero crossing (ZC) is a physical rotor event — the
floating phase's back-EMF passing the motor neutral, twice per electrical rev per
phase. It is *directly* observable only when it falls inside the 60-degree window in
which that phase floats. Whether it does is set by the **load angle**: the rotor lags
the commanded commutation, and that lag moves the crossing within (or out of) the
window. At the catch/slip boundary the crossing sits in-window; in deep lock the load
angle pushes it past the window edge, so a comparator-style "watch for the crossing"
sees nothing — even though the crossing is still happening.

So "find the ZC in whatever regime it exists" is really: **recover the crossing angle
(equivalently, the load angle) reliably across the operating plane, including where it
falls outside the observation window** — and prove that recovery is trustworthy.

## The detector

**Acquisition.** One ADC scan per PWM period, triggered at the counter valley (CNT=0,
the middle of the ON window), 12-bit. Per frame: the three phase terminal voltages
(BEMF) plus the three phase shunt currents and VBUS (the `dump7` / `cdump` capture).
The capture is firmware-aligned to electrical zero, and because the frame rate and the
commutation rate share the timer clock, the frame→commutation-angle map
`θ = i / (sample_hz/hz) · 2π` is exact (clock tolerance cancels in the ratio).

**Virtual neutral — driven pair, not (A+B+C)/3.** The crossing is `v_float − neutral`.
The naive neutral `(A+B+C)/3` folds ~1/3 of the floating phase's *own* BEMF into its
reference (shrinking the measured BEMF to ~2/3 and adding that channel's noise). The
floating phase carries no current, so the true star point is the average of the two
*driven* terminals, ≈ Vbus/2 at the divider — computed per frame, it tracks bus ripple
and divider scale for free. The two neutrals agree exactly at the crossing but differ
in slope away from it, which matters for a fit.

**Settling blank.** At each commutation the just-switched driven terminal rings /
demagnetizes; its average (the neutral) departs sharply from its flat baseline. Those
frames are blanked by criterion (neutral out of band), not by a fixed count.

**The fit.** For each phase, pool the float-window samples (two 60-degree arcs 180
degrees apart per rev) and least-squares fit

```
v_float − neutral  =  c0 + Σ_{h=1..N} ( a_h·cos(hθ) + b_h·sin(hθ) )
```

The electrical frequency is *known* (we command it open-loop), so the fit is linear.
Solve for the zero crossing (closed form for N=1; dense-eval + interpolation for N>1).
The crossing's offset from the window centre is the **load angle**.

**Why N harmonics.** The BEMF is not a pure sinusoid (it is trapezoidal-ish), so a
fundamental-only fit (N=1) crosses zero at a *different* angle than the real waveform,
by an amount that grows where the waveform is most distorted. Adding harmonics captures
the distortion and recovers the true crossing. **N=3** is the validated choice (below).

## Validation methodology

Where the crossing falls in-window, the **direct crossing is ground truth**. So:
at every sector where an in-window ZC exists, compare it to the reconstruction's
crossing for that window. Metric: `|zc_in − zc_fit|` as a percentage of the 60-degree
window (1% = 0.6 degrees electrical). Tool: `scripts/zc_validate.py`.

## Results

### 1. The reconstruction matches ground truth — and N=3 is the right order

Controlled set (one amplitude, 200–310 Hz, 105 in-window crossings):

| fit | median | mean | worst | outliers (>20% window) |
|---|---|---|---|---|
| 1 harmonic (fundamental) | 7.0% (~4.2°) | 11.9% | 63.2% | 16 / 99 |
| 2 harmonics | 3.6% (~2.1°) | 6.8% | 40.9% | 8 / 85 |
| **3 harmonics** | 6.7% (~4.0°) | **6.7%** | **17.8% (~10°)** | **0 / 105** |

The fundamental-only fit has a systematic outlier tail (up to a full window off,
concentrated at the operating points where the BEMF is most distorted). **Three
harmonics eliminates the tail entirely** — every one of the 105 in-window crossings
agrees with the reconstruction to within ~10 degrees, median ~4. Two ruled-out
alternatives confirm the cause is harmonic distortion, not dynamics:

- **Constant-speed smear (ruled out):** fitting per electrical rev instead of pooling
  did *not* improve agreement (it slightly worsened it — fewer samples per fit). The
  disagreement is not a rotor-speed-vs-time artifact.
- **Rotor hunting (ruled out):** at the disagreeing points the in-window ZC is itself
  self-consistent (low scatter) while sitting offset from the fundamental fit — a
  systematic bias, not random scatter.

### 2. It holds across the operating plane, and ground truth reaches into deep lock

Comprehensive (Hz, amp) sweep, 320–800 Hz × 0.5% amp, 13k captures, `scripts/zc_map.py`:

- **Validation residual median 5.7%** across the whole plane (781 in-window crossings),
  with the residual concentrated at the catch-boundary transitions and marginal-lock
  points — the well-locked regime validates tightly.
- **The deep-lock gap closes in the mid-band.** In-window ground truth reaches up to
  **~52% amp at 400–499 Hz** (vs ~30% at 100 Hz): as frequency rises the crossing walks
  *into* the window at higher amplitudes, so the reconstruction is checked against real
  crossings *deep inside the locked region*, not only at the slip boundary. Above
  ~650 Hz the crossing exits the window again, leaving a reconstruction-only corner that
  is continuous with the validated band.
- **The clean-observation band extends to 800 Hz.** Per-sector spread (the residual
  asymmetry between the six sectors) collapses to 0–10 degrees throughout deep lock —
  the sectors behave symmetrically wherever the motor is solidly locked. The large
  per-sector asymmetry seen in earlier analysis is a **marginal-lock / catch-boundary
  artifact**, not a structural property; it vanishes in good lock.

### 3. The load-angle surface

The reconstruction yields a smooth, structured load-angle surface over the (Hz, amp)
plane: deep lock at roughly −15 to −25 degrees, swinging through zero and positive
along the catch-boundary diagonal. The surface is smooth at ±1 Hz frequency resolution
(jitter captures), i.e. it is the rotor genuinely sitting at a well-defined angle vs
commutation — not aliasing.

## What this gives us

A **trustworthy open-loop ZC / load-angle observation across every regime where the
motor locks** — ground-truth-validated against direct in-window crossings wherever they
exist (now including deep lock in the mid-band), with the honest reconstruction-only
corner explicitly flagged by the residual map. This is the prerequisite the project was
built around: the observation is now trustworthy.

## Phase-A anomaly — resolved (not a sense-path gain error)

Earlier per-sector analysis flagged phase A (PA4 / ADC2 ch17) as anomalous: its
fitted float-window BEMF amplitude read ~half of B/C and it carried a large DC
offset (`c/R ≈ +2.4`), so A never produced an in-window crossing. The standing
hypothesis was a linear sense-path error `V_meas = g_A·V_true + o_A`, to be
confirmed by adding a free gain `g_A` to the fit. The closeout (`scripts/zc_phase_a.py`,
1500+ solidly-locked mid-band captures) **refutes** that hypothesis on three
independent lines, all from existing data — no hardware lead-swap needed:

1. **The sense divider is matched.** The driven-rail high plateaus, which pass
   through the *same* resistor divider as the float read, agree to **1.2%**
   (A=1658, B=1663, C=1643). A per-channel divider gain error would scale the
   plateau and the BEMF identically; matched plateaus ⇒ matched sense gain ⇒ no
   gain error to correct.
2. **The low float amplitude is operating-point dependent, not fixed.** `R_A/R_C`
   is not constant: it swings **0.87 → 0.25 → back** as commanded amplitude
   (= duty = load angle) rises, while the fixed-gain prediction (divider gain
   A/C ≈ 1.0) is flat. A fixed gain is amplitude-independent by definition;
   this is a load-angle effect. It is the amplitude-domain face of the
   electrical-angle-locked per-sector wave (A floats in sectors s2/s5, whose
   60° windows land on the flatter trapezoid top at the deep-lock load angle).
3. **The offset half was a neutral-method artifact.** The `c/R ≈ +2.4` was
   measured against the old `(A+B+C)/3` neutral. Against the validated
   driven-pair neutral the per-bin median `|c/R| ≤ 0.21` for all three phases.

**Conclusion:** a host-side `g_A` would absorb a real load-angle-dependent
amplitude swing into a single constant — masking physics, not fixing a
calibration error. The validated detector is correct to rely on B/C and the
harmonic *crossing* (amplitude-robust) rather than A's amplitude. The Phase-A
question is closed. (Figure: `logs/.../phase_a_closeout.png`, regenerate with
`scripts/zc_phase_a.py <sweep_dir> --render`.)

## Honest limits

- **The crossing recovered is the fundamental-plus-harmonics crossing of the
  float-window BEMF.** It matches the direct terminal-voltage crossing to ~4 degrees
  median; it is not claimed to be exact to arbitrary precision.
- **The very-high-frequency deep-lock corner is reconstruction-only** — no in-window
  ground truth there (crossing outside the window). It is continuous with the validated
  band and clean (low sector spread), but unvalidated directly.
- **Resolution falls with RPM.** frames/sector = sample_hz/(hz·6): ~33 at 100 Hz, ~6.7
  at 500 Hz, ~4 at 800 Hz. The detector works to 800 Hz; pushing much higher needs more
  samples per PWM period (a firmware change), not a host-side change.
- **This is observation, not control.** Per the standing constraint, the detector is a
  measurement/diagnostic; nothing here closes a loop.

## Tools and provenance

- `scripts/zc_validate.py` — multi-harmonic fit vs direct in-window ZC; the validator.
- `scripts/zc_map.py` — the (Hz, amp) load-angle / residual surface.
- `scripts/zc_phase_a.py` — Phase-A sense-gain closeout (driven-rail gain vs
  amplitude-dependent float ratio vs offset); refutes the gain hypothesis.
- `scripts/scope_common.py` — driven-pair neutral, settling blank, rotor classifier.
- `examples/scope1.rs` — valley-sampled 7-channel capture (`dump7` / Ascii85 `cdump`).
- Controlled validation set and comprehensive sweep under `logs/` (gitignored).
