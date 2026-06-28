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

### 4. The per-sector wave — load angle vs geometry — RETRACTED & RE-DERIVED (June 27 2026)

> **This section previously claimed "the per-sector wave tracks load angle, not geometry —
> SUPPORTED across 250–400 Hz." That claim was wrong, built on a detector artifact. It is
> retracted below and replaced with the validated-oracle re-derivation. Kept in full as a
> worked example of trusting a detector's plausible numbers before validating the rig.**

**What was claimed.** A `cl_wave_sweep.py` amp×freq sweep reported a per-sector ZC "wave"
whose peak-to-trough spread **collapsed dramatically with amplitude** (e.g. 250 Hz 179→24,
350 Hz 37→10) and **offset upward with frequency** — read as the two orthogonal knobs of
a load-angle effect, and promoted to "SUPPORTED."

**Why it was wrong.** The sweep measured each sector with an **ungated least-squares linfit
over the whole float window**. Raw captures (preserved only after the user insisted —
`logs/zc_<ts>/zc_<hz>_<amp>.log`) show that at these operating points the float-window BEMF
is usually a **one-sided plateau** — the real zero crossing falls at or outside the 60°
window edge — and the only excursions through neutral are the **demag transient** at the
window start and the **next-commutation edge** at its end. The whole-window linfit fit
*those two transients*, not a BEMF ramp, and fabricated a "crossing." Direct check on
`zc_250_12` s4: clean middle rises through zero at ~20 % (rising), but the linfit reported
**slope −24.8, z = 84 %** because the `−335` demag and `−390` commutation endpoints flipped
the least-squares slope. The directly-observed sign-change detector (`analyze_zero_crossings`,
`status=='zc'`) confirms the problem: across the whole 250–400 Hz sweep it finds only
**0–2 genuine in-window crossings per capture** (≈12 windows each) — consistent with the
long-known "**2 of 6 sectors cross in-window**" (`motor_tester`). The "clean flat wave at
high amp" was the *artifact* being consistent (the linfit fits the same two transient shapes
in every window), not crossings converging — e.g. `400/22`, the supposed flattest/best
point, has **no in-window crossing in any sector**.

**Re-derivation with the validated oracle (`zc_fit.py` sinusoid fit).** The oracle fits each
phase's pooled float-arc BEMF to `a·cos+b·sin+c` (transients blanked) and solves
analytically for the crossing — recovering out-of-window crossings *legitimately* (it was
validated to ~4° vs in-window ground truth). Run per capture on the same 20 setpoints
(`oracle_wave_20260627.json`), all fits clean (`|c/R| ≤ 0.23`, i.e. real crossings):

```
per-sector crossing-offset spread (% of window)    LS-linfit (artifact) vs ORACLE (validated)
  250 Hz, amp 12→20:   LS 171 → 25                 ORACLE  28 22 22 22 19   (≈flat)
  300 Hz, amp 14→22:   LS  70 → 24                 ORACLE  35 32 37 30 38   (≈flat)
  350 Hz, amp 16→24:   LS  37 → 10                 ORACLE  24 23 31 33 42   (INCREASES)
  400 Hz, amp 18→26:   LS  17 →  8                 ORACLE  53 50 52 55 51   (≈flat)
```

**The amp-flatten does not exist.** The validated per-sector spread is **roughly constant
with amplitude** (at 350 Hz it grows). The collapse, the "minimum that moves with frequency,"
and the clean amp-flatten were all LS-linfit transient artifacts.

**What the evidence actually supports now:**
1. **A stable per-sector offset pattern** (~±0.3 window; `s0` consistently low, `s5` high)
   that **does not collapse with torque margin** — i.e. it behaves like a **fixed
   geometric/sequence offset**, the opposite of the retracted claim. The "geometry vs load
   angle" question is **re-opened, leaning geometry** for the amplitude axis.
2. **Load angle is real but minor here** — it shows mainly on the **frequency** axis (the
   earlier 120–180 Hz oracle run moved C-fall 212→243°, B-fall 283→315°), with only a small
   amp-dependent slide at 350 Hz. Over 250–400 Hz the amplitude knob barely moves the true
   crossings.
3. **In-window ZC coverage is intrinsically sparse** at the open-loop schedule (commutation
   isn't timed to land ZC inside the float window). A trustworthy detector must combine the
   **observed** sign-change crossings with the **harmonic-oracle** out-of-window recovery —
   never a whole-window linfit.

**Method consequences (carried forward):**
- `cl_wave_sweep.py`'s linfit wave is **not a measurement** of the crossing; do not quote its
  magnitudes. Its lock gate also must not depend on crossing count (there often are none) —
  judge lock on BEMF *structure* (plateau spread, ramp presence) instead.
- The Phase-A "amplitude-domain face of the load-angle wave" reading (§"Phase-A anomaly")
  leaned on this section; revisit it under the oracle.

> **STATUS: the load-angle *wave* claim is RETRACTED.** Validated re-derivation shows the
> per-sector crossing pattern is largely **amplitude-invariant** (geometry-like) with only a
> small, mainly-frequency-axis load-angle component. Root cause of the error: a whole-window
> LS linfit fitting commutation/demag transients in out-of-window sectors, trusted before the
> raw captures were preserved and the rig validated. Lesson logged in
> `feedback_dont_declare_walls_prematurely`.
> Evidence: `oracle_wave_20260627.json` (validated), raw captures `logs/zc_20260627_164752/`,
> the discredited LS wave `wave_sweep_multifreq_20260627.json`.

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
- **The "per-sector wave = load angle" result (§4) is RETRACTED.** The amp-flatten /
  freq-offset "wave" was a whole-window LS-linfit artifact (fitting demag + commutation
  transients in out-of-window sectors). The validated harmonic oracle shows the per-sector
  crossing pattern is largely **amplitude-invariant** (geometry-like), with only a small,
  mainly-frequency-axis load-angle component. Real in-window crossings are sparse (0–2 per
  capture). See §4 for the full retraction and re-derivation.

## Tools and provenance

- `scripts/zc_validate.py` — multi-harmonic fit vs direct in-window ZC; the validator.
- `scripts/zc_map.py` — the (Hz, amp) load-angle / residual surface.
- `scripts/zc_phase_a.py` — Phase-A sense-gain closeout (driven-rail gain vs
  amplitude-dependent float ratio vs offset); refutes the gain hypothesis.
- `scripts/cl_wave_sweep.py` — per-sector ZC wave vs amp (§4): the load-angle-vs-geometry
  test. Open loop = clean probe; ungated python linfit for full coverage. Raw run:
  `wave_sweep_250hz_20260626.json`.
- `scripts/scope_common.py` — driven-pair neutral, settling blank, rotor classifier.
- `examples/scope1.rs` — valley-sampled 7-channel capture (`dump7` / Ascii85 `cdump`).
- Controlled validation set and comprehensive sweep under `logs/` (gitignored).
