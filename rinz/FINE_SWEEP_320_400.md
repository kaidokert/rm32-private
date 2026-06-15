# Fine sweep 320–400 Hz — the lock transition is one structure seen three ways

Follow-up to [`OBSERVER_BEFORE_CONTROL.md`](OBSERVER_BEFORE_CONTROL.md). That note
argued — from the 100–600 Hz coarse map — that in-window ZC count is anti-correlated
with stable lock, so a "count the good sectors" lock metric reads backwards. This note
records the **fine sweep that confirms it at 0.5 % amp resolution** and proposes the
next step (which is *not* more sweeping).

## What was run

```
scope_sweep.py COM41 --freq 320 400 --freq-step 20 --amp-step 0.5 \
    --amp-min 13 --amp-max 30 --snaps 5 --freq-jitter 1
```

5 setpoint freqs × ±1 Hz jitter × 35 amps × 5 snaps = 2625 captures, ~29 min.
Output: `logs/sweep_20260614_191405/` (`sweep_map.png`, `interesting.csv`,
`flagged_img/`). Fixed driven-pair neutral + adaptive blank; iu_ma/vbus populated.

Targeted the 300–400 Hz bistability hotspot the coarse run
(`sweep_20260614_185850`) flagged — 360 Hz appeared 6× there.

## Finding: catch boundary, ZC band, and bistability are the same diagonal

At fine resolution the three observables collapse onto **one line** — the lock/slip
transition:

1. **Catch boundary is a sharp current *step*, not a gradient** (phase-U current
   panel). Below it (~14–18 % amp at 340–400 Hz) current is ~400–600 mA = slipping;
   above it jumps to ~1500–2800 mA = locked. The step rises ~15 % @320 Hz → ~20 %
   @400 Hz. VBUS is the mirror image (sags where current is high).

2. **In-window ZC sits exactly on that boundary; deep lock is empty.** ZC ≈ 0 in the
   locked upper-left; lights up to 4–7 in a diagonal band that traces the current step
   precisely. The in-window-ZC band **is** the catch line. This is the
   `OBSERVER_BEFORE_CONTROL` anti-correlation, now at 0.5 % resolution: ZC count marks
   the marginal transition, *not* stable lock.

3. **Bistability peaks at the boundary**, strongest at 399–401 Hz / 17–20 %
   (snap-spread ~280–300). The rendered hotspot `flagged_img/f399_a19.0.png` shows
   why: that single snap reads *marginally* locked (`late_swing=30.9`, a hair over the
   30 threshold, 0 in-window ZC), yet across its 5 snaps `late_swing` swung by ~300.
   The rotor is flipping between locked and dropping out at one fixed setpoint —
   bistability made visible.

4. **Sensing trustworthy throughout** (plateau_spread 0.6–2.2 %). The waveforms at the
   knife-edge are clean six-step staircases with a clean driven-pair neutral — the
   observation quality is good even where the *lock* is marginal.

## Honest caveat — the hotspot is at the window edge

The bistability peak landed at **400 Hz, the edge of the swept window**. Partly real
(the catch step is steepest at the high-freq end of this window), partly a windowing
artifact (the amp-gradient interest score sees a one-sided neighbour at the boundary).
The coarse run flagged 360 Hz; the fine run flagged 400 Hz. To bracket the bistable
knife-edge properly the fine window would need to extend up to ~440–460 Hz so 400 is
no longer the boundary.

## Proposal — the next step is geometry, not marginality

The sweep has done its job: it mapped the lock landscape, pinned the catch boundary in
current, confirmed in-window-ZC-tracks-the-boundary at fine resolution, and showed
sensing is clean. More sweeping characterises *lock marginality* in finer detail —
interesting, but **not the gate**.

The gate from `OBSERVER_BEFORE_CONTROL.md` is **observation geometry** — a trustworthy
per-sector load angle. Two moves, neither a controller, neither needing a new sweep:

1. **`zc_fit` across the locked captures** in this fine sweep → per-sector load angle
   as a number (fits the crossing whether or not it lands in a window, so deep-lock
   captures with ZC=0 still yield an angle). This turns "ZC band tracks the boundary"
   into a quantitative load-angle-vs-(freq,amp) surface.

2. **Phase-A lead-swap test** → swap motor leads so a different winding feeds the
   PA4-sensed terminal. Anomaly stays on channel A → sense divider (PA4 path); follows
   the winding → motor. Plus a bench ratiometric check of the three dividers at rest.

Extending the bistability window to 460 Hz (finishing the marginality map) is a valid
*alternative*, but it answers the lock-stability question, not the observation-quality
gate. Geometry first.

## Provenance

- This sweep: `logs/sweep_20260614_191405` (320–400 Hz × 0.5 % amp, ±1 Hz jitter,
  5 snaps).
- Coarse precursor: `logs/sweep_20260614_185850` (100–600 Hz × 1 % amp).
- Prior note: `OBSERVER_BEFORE_CONTROL.md`.
- Tools: `scripts/scope_sweep.py`, `scripts/sweep_map.py`, `scripts/zc_fit.py`,
  `scripts/scope_common.py::classify_rotor_state`.
