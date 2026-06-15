# Observer before control — why the loop stays open (June 2026)

A standing note on a recurring temptation: the moment the open-loop data starts
looking good, someone (human or agent) wants to close a loop on it. This documents
*why we don't yet*, using evidence from the 100–600 Hz coarse sweep
(`logs/sweep_20260614_185850`) rather than the abstract argument alone.

The hard constraint (see `CLAUDE.md`): no closed-loop control until the open-loop
observation is trustworthy — correct ZC timing, stable neutral, repeatable
sector-to-sector behaviour. Closing a loop on an unvalidated signal does not fix the
signal; it hides the bugs behind feedback and makes them harder to diagnose. This
note is the concrete, data-backed version of that rule.

## The three things people ask for

1. **Lock detection** — locked / not-locked as a binary or probability. Natural
   representation: 6 float windows per electrical revolution, each either shows a
   clean in-window zero crossing or not → a 6-bit lock vector.
2. **Phase / rpm estimation** — for use as a feedback signal.
3. **A PI controller** driving throttle / rpm off (1) and (2).

(1) and (2) are **observers** — estimation, which is squarely the open-loop mission.
(3) is a **controller** — the forbidden step. The distinction matters: we can and
should build observers; we just don't wire them into feedback yet.

## Why (3) is still gated — and (1) shows it in the act

The coarse sweep gave us, for the first time, the lock boundary as data across the
whole 100–600 Hz plane (the phase-U current panel: lock-catch is a sharp current
jump). Cross-referencing the current panel against the in-window-ZC panel produced
the key finding:

> **In-window ZC count is *anti-correlated* with stable lock.**
> Deep-locked region (high current, upper-left of the map): ZC ≈ 0.
> Catch / slip boundary: ZC lights up to 8–10.

Consequence for the "count the good sectors" lock metric: a lock-probability built
from *fraction of sectors with a good in-window ZC* would read **highest right as the
rotor is about to slip out**, and **lowest when lock is strongest**. Wired into a
controller, it would back off throttle exactly when lock is solid and push when it is
marginal — and you would never see it, because the loop would appear to be "working".

That is precisely the bug-hiding the constraint exists to prevent, caught in the act
on real data. The obvious lock metric is inverted, and only feedback would hide it.

### What the data says the trustworthy lock signals actually are

Monotonic with lock in this sweep:

- **phase-U current** — the sharp catch jump (the lock boundary itself).
- **late_swing** — rotation-amplitude excursion in the float window.

NOT monotonic (inverted): **in-window ZC count.** This is why
`classify_rotor_state` (host) deliberately classifies on current/swing and keeps
`bemf_amp` / ZC count as advisory columns only.

So a trustworthy **lock observer is buildable today** — but it is the current/swing
one, not the ZC-count one the intuition reaches for first.

## Why (2) is the open-loop work, not a shortcut

In open loop we already **command** phase and rpm — the electrical frequency *is* the
forced rpm; the commutation phase *is* what we drive. The only estimate that carries
information is the **rotor's actual phase relative to the command** — the load angle /
slip.

That estimate is exactly what is not yet clean, for diagnosable reasons the map and
prior work name specifically:

- **Phase-A sense anomaly** — PA4 / ADC2 ch17 reads wrong *only when floating*
  (driven plateaus are fine). Fitted BEMF amplitude ≈ half of B/C; DC offset exceeds
  its own amplitude at 120 Hz. A's sense path, not the motor (symmetric otherwise).
  A floats in s2 and s5 — the perennially-pathological sectors.
- **Per-sector ZC offset wave** — a smooth ±0.8-window (≈±48° elec) modulation,
  period exactly one electrical rev, repeatable across spin-ups, electrical-angle-
  locked (not rotor hunting, ruled out via 6-rev `--per-rev`).
- **Crossings fall outside the sampled float window in deep lock** — load angle puts
  the genuine crossing past the window edge, so it is not "missing", it is geometric.

"The view of the data wasn't good enough yet" is therefore **correct, and correct for
specific reasons** — not vibes. Making the per-sector load angle trustworthy across
all six sectors *is* the work the constraint is pointing at.

## The synthesis

- The coarse data is enough to **build and validate an observer** (lock state now;
  load angle once the sense path is clean). That is the mission.
- It is **not** enough to close a loop. The most obvious lock metric runs backwards —
  fresh, concrete evidence for the constraint rather than a hypothetical.
- Build the observer, prove it tracks something physical and repeatable across all six
  sectors, *then* the controller question answers itself. Wiring feedback now would
  paint over the phase-A bug we already know is there.

## The gate (what unblocks the next step — still open-loop)

Neither of these is a controller:

1. **`zc_fit` across the locked region** → per-sector load angle as a number
   (fits the crossing whether or not it lands in a window).
2. **Phase-A lead-swap test** → swap motor leads so a different winding feeds the
   PA4-sensed terminal. Anomaly stays on channel A → sense divider (PA4 path); follows
   the winding → motor. Plus a bench ratiometric check of all three dividers at rest.

When the per-sector load angle is clean, repeatable, and sector-symmetric, the
observation is trustworthy and the closed-loop conversation can begin — not before.

## Provenance

- Coarse sweep: `logs/sweep_20260614_185850` (100–600 Hz × 1% amp, fixed driven-pair
  neutral + adaptive blank, iu_ma/vbus populated). Map: `sweep_map.png`.
- Lock classifier: `scripts/scope_common.py::classify_rotor_state`.
- ZC geometry: `scripts/zc_fit.py` (honest sinusoid-fit crossing angle).
- Related: `notes/ASYMMETRY_HYPOTHESIS_CX.md`, the ZC-vs-window investigation in
  `CLAUDE.md`.
