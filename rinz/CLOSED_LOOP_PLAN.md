# Gentle closed-loop transition — the plan we hold ourselves to

The observation gate is passed (see CLAUDE.md "TRANSITION TO GENTLE CLOSED-LOOP").
This document is the leash: the stages, the single principle, and — the point of
writing it down — the **predefined checkpoints that tell us we have diverged**, either
technically (the approach is failing) or from our own discipline (we're fooling
ourselves). Predefining these *before* running is the discipline. We do not loosen a
checkpoint to make a run pass; if a checkpoint trips, we stop and diagnose.

## The one principle (non-negotiable)

**The offline multi-harmonic observer is the ORACLE, and it is the ONLY thing that
certifies the real-time detector.** Internal smoothness, in-window-ness, lock
duration, low phase error, agreement with the open-loop schedule — *none of these
certify correctness.* `skunk` had all of them while locked to a blank/demag artifact
(`cl_per ≈ 2·(blank+3)`, a self-consistent fixed point, not the rotor). Certification
means exactly one thing: **the real-time detector's zero-crossings match the oracle,
computed on the same captured frames.**

## Non-negotiables (from the CLAUDE.md guardrails)

1. Do **not** modify `scope1.rs`. Closed-loop work lives in `examples/scope_cl.rs`.
   The full capture / stream / analysis harness is preserved in it.
2. **Observe-only first.** Commutation stays on the open-loop schedule until the
   oracle gate passes. The detector/PLL steer nothing.
3. **Oracle agreement before any feedback.**
4. **Bounded authority + instant fallback** once feedback is engaged.

## Stages — each has an explicit entry gate and exit gate

| Stage | Entry gate | Work | Exit gate |
|---|---|---|---|
| **0 — Harness + logging** | now | `scope_cl.rs` = copy of scope1 + real-time ZC detector + shadow PLL, all observe-only; dump extended with a per-commutation `cl:` log | builds; runs open-loop identically to scope1; dumps BEMF frames **and** per-commutation detected-ZC / θ / ω / Δθ |
| **1 — Detector vs oracle** | Stage 0 exit | host tool overlays the firmware's real-time ZC against the offline multi-harmonic oracle across an open-loop sweep | real-time ZC matches the oracle to **≤ D1** across **≥ 3** `zc_map`-chosen in-window cells, with no stable systematic bias |
| **2 — Bounded closed-loop** | Stage 1 exit | engage feedback as an α-blend, slew-limited around the open-loop schedule, at an oracle-validated in-window operating point; ramp α 0→1 over serial, instant α→0 fallback | α=1 sustains lock with **no** governor rescue for **≥ T** s; load angle stays in-window; current per unit speed flat |
| **3 — Widen** | Stage 2 exit | expand operating range / reduce the open-loop crutch | each expansion individually oracle-gated |

The operating point for Stages 1–2 is **chosen from `zc_map.py`**, in a cell where
in-window ZC ground truth demonstrably exists — not "solid lock" by feel (deepest
lock has the ZC *out* of window).

## Divergence checkpoints

### A. The approach is failing (technical, falsifiable — trip ⇒ HALT + diagnose)

- **D1 — detector ≠ oracle.** Real-time ZC vs oracle median **> 5 % window (~3°)**,
  or a *stable* systematic offset **> 5 %**. The detector is tracking something other
  than the rotor. (Stage 1 gate.)
- **D2 — the `skunk` signature.** The closed-loop period locks to a value set by the
  detector's own constants rather than the rotor: `|cl_period − k·(blank+c)|` small
  **while** `|cl_period − oracle_period|` large. This is artifact lock; it can look
  perfectly smooth. HALT immediately.
- **D3 — load angle leaves the validated band.** The loop's ZC position drifts
  outside **[10 %, 90 %]** of the float window (i.e. toward a window edge / out). Fall
  back to open-loop.
- **D4 — current without work.** `iu_ma` per unit speed *rises* as α→1 → slip /
  mis-commutation, not lock. HALT.
- **D5 — can't stand on its own.** Lock holds only with the governor (α<1); at α=1 it
  diverges → the detector is not good enough. Drop to Stage 1, do not push α.

### B. We've gone off-discipline (process — the `skunk`-prevention rules)

- **P1 — more than one variable changed** between runs (drive vs detection vs
  control). We've lost cause attribution. STOP; revert to one-change-at-a-time.
- **P2 — citing internal evidence as proof.** The moment we justify a run by its
  smoothness / in-window-ness / lock duration / agreement-with-open-loop instead of
  the **oracle**, we've abandoned the principle. STOP; re-run the oracle overlay.
- **P3 — tuning to the loop, not the oracle.** If we find ourselves adjusting the
  detector / PLL gains to make the *loop* stable rather than to match the *oracle*,
  we are fitting the artifact. This is exactly how `skunk` "worked." STOP.
- **P4 — harness regression.** `scope1.rs` touched, or any capture/stream/analysis
  capability lost. STOP; restore before proceeding.
- **P5 — stage skipped.** Advancing a stage without its exit gate explicitly met and
  written down. STOP; return to the unmet gate.

## Rollback protocol

Any D or P trip: **α→0 (open-loop), capture a dump, run the oracle overlay, and write
down what tripped + the hypothesis _before_ changing anything.** Then one change,
re-test. No batch changes while chasing a divergence.

## Definition of done / definition of abandon

- **Done (this gentle phase):** closed-loop sustains lock at ≥1 oracle-validated
  operating point, α=1, no governor rescue, load angle in-window, for ≥ T s, with the
  dump + oracle confirming the loop tracks the **real** ZC (D2 explicitly not tripped).
  Speed range, robustness, startup-from-rest are all *future* — not in scope here.
- **Abandon (no shame):** if Stage 1 cannot make the real-time detector match the
  oracle in **any** in-window cell after honest effort, then real-time ADC-valley ZC
  detection is not viable on this hardware. Document it and stop — the validated
  open-loop observer stands as the deliverable, exactly as the dual-sampling negative
  result did.

## Thresholds are provisional — and lock-only

D1/D3/T and the D2 constants are provisional, to be calibrated in Stage 1 against the
oracle's *own* spread (the oracle matches direct in-window ground truth at ~1.5–5.7 %,
so the real-time detector should land in that band). Once calibrated, they are written
here and **only ratcheted tighter, never loosened to pass a run.**

---

## Stage 1b — shadow PLL, gated on tracking the oracle (added after D1 failed)

**Why:** D1 failed on the *raw* per-commutation detector — median 21.6 % vs oracle,
~40 % mis-lock rate (p90 ~19 %): it latches onto demag / noise / a second crossing.
That is `skunk`'s exact disease. A raw crossing was never meant to be trusted
unfiltered; a real loop filters them. Stage 1b asks the right question: **can a PLL,
fed this noisy detector, track the oracle's load-angle trajectory despite the
outliers?** This is *not* loosening D1 — it gates on the *filtered* estimate the loop
would actually use, and it stays oracle-anchored.

### Architecture — shared host/firmware Rust (already scaffolded in this crate)

- `rinz` is `#![cfg_attr(not(test), no_std)]`: pure modules build and `cargo test` on
  host; firmware-only modules are `cfg(target_arch="arm")`. Building blocks already
  present and unit-tested: `PLL` (PI loop filter, anti-windup), `Accumulator`
  (phase/VCO), `filter` / `ewma_pow2`, `signed_calc` (numeric abstraction),
  `algorithm_params` (live-tunable).
- **New portable `cl` module:** interpolated ZC detector (+ demag/outlier robustness)
  → phase error → existing `PLL` + `Accumulator` → per-commutation tracking state.
  Pure, deterministic, no_std, unit-tested. **STEERS NOTHING.**
- **Host harness** replays captured frames through `cl` exactly as the ISR would
  (causal, frame-by-frame), emits the tracker trajectory; Python compares it to the
  oracle. Iterate the detector/PLL on the **host** — no firmware flash per change.
- The firmware ISR calls the **identical** `cl` module. No host/firmware divergence
  (the rm32 `run_tick` lesson): the thing we validate on host *is* the firmware code.

### The Stage-1b gate (oracle-anchored)

- **G1 Tracking:** PLL filtered load-angle residual vs the oracle trajectory ≤ **5 %**
  median, after convergence.
- **G2 Outlier rejection:** PLL output variance ≪ raw-detector variance **and** the
  filtered value sits on the oracle (it filters toward truth, not an artifact).
- **G3 Anti-`skunk` (== D2):** the PLL's locked period equals the oracle's electrical
  period — the rotor — **not** a self-consistent multiple of the detector's
  blank/confirm constants. Smoothness alone is `skunk`; this is the check that we're
  locked to the rotor.

### Process

1. Build `cl` (detector + compose `PLL`/`Accumulator`), host unit tests.
2. Replay the existing sweep segments (12 commutations each) → iterate on host until
   G1–G3 hold. Then a **longer continuous capture** at solid lock for a definitive run
   (the 2-rev dump is short for PLL convergence; sweet spot ~350–450 Hz solid lock —
   clean BEMF and still ~7–9 samples/sector, vs ~5 at 600 Hz).
3. Firmware uses the identical `cl` (still shadow / observe-only); re-capture; confirm
   host == firmware on the same data.
4. Only then Stage 2 (let it steer, bounded α-blend).

Still observe-only throughout. The PLL must **track the oracle**, not merely be smooth.

### Replay ceiling, and the A→B path (chosen)

Building the host pipeline surfaced a hard limit: **observe-only replay cannot validate
the closed loop.** The captured BEMF was produced by *open-loop* commutation and won't
respond to *different* (closed-loop) timing. Replay validated the *observer* as far as
physics allows (the detector is ~3.5% within-cell where the ZC is in-window; it fires on
only ~15% of open-loop commutations because deep-lock pushes the ZC out of the window —
an open-loop artifact, not a closed-loop predictor). Testing the *loop* needs a model or
hardware. Chosen path: **A) cheap host motor-model sim to shake out the loop logic, then
B) bounded hardware.**

### Path A result — loop logic validated in simulation (`tests/cl_sim.rs`, `cargo cl-sim`)

The real `cl::ClLoop` controller (detector → ZC-to-ZC period PI filter → commutate ~30°
after the ZC, dead-reckon on the filtered period when a ZC is missed) runs against a
simple rotor+BEMF model whose BEMF *responds* to the loop's commutation. With demag
spikes, noise, and a trapezoidal flat-top:

- ✅ **`cl_sim_locks`** — acquires and holds lock, stays synced 1:1 with the rotor, and
  **tracks a mid-run load step** (rotor speed change) with post-step period error **1.7%**,
  coasting through ~36% missed detections.
- ✅ **`cl_sim_needs_zc`** (negative control) — starve the detector (no ZC) and the loop
  can only dead-reckon: it then **fails the same load step** (stalls, desyncs 3.3:1). This
  proves `cl_sim_locks` passes because of genuine ZC feedback, not a dead-reckonable
  constant-speed rotor.
- **Bug caught (the point of A):** the no-ZC fallback originally commutated at
  `period*1.4` (late → overshoot → miss-next-ZC coast spiral → stall). Fixed to
  dead-reckon at `period_est` (`coast`≈1.0).

**Honest caveats:** idealized model — sinusoid+trapezoid BEMF, simple torque, no
per-sector load-angle texture or Phase-A anomaly, demag is a clean exponential. It
catches gross logic bugs (and did); it is *not* a fidelity claim. The real anti-`skunk`
test (G3: locked to the rotor, not a demag artifact) is only decisive on hardware, where
real demag/artifacts exist. **Ready for path B (bounded hardware, α-blend + governor +
fallback).**
