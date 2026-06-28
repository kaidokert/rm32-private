# rinz sensorless roadmap — detector → loop → full-range validation

_Written June 27 2026, after the §4 retraction (see `BEMF_ZC_DETECTOR.md`). Goal: a
trustworthy ZC detector, a closed loop (PLL + PI), and validated performance across the
operating range to 90–95 % duty (~1300–1400 elec Hz)._

## Progress log

- **Phase 0 — done.** Harmonic oracle + `zc_oracle_render` is the offline ground-truth tool.
- **Phase 1 — done & bench-validated.** `ClLoop` now consumes the straddle-gated sign-change
  ZC (`finish_frame()`) instead of the fabricating `finish_linfit()` (runtime toggle `j`,
  default still linfit so the sim is unchanged). Bench A/B @210 Hz: sign-change ~halved jitter
  (7.7k vs 14.8k), tripled the current drop (−20–28% vs −2%), and centred the crossings 2×
  better (offset 30 vs 62). Honest-sparse beats noisy-dense, as predicted.
- **Phase 3 — engaged & measured.** Fine alpha sweep (`cl_engage --detector signchange`,
  n/m steps) located a **sweet spot at alpha ≈ 0.5–0.7**: ~20–28% current reduction at the
  lowest engaged jitter; alpha=1.0 overshoots (worse on both). Residual jitter is still ~6×
  open-loop — the loop coasts ~half the cycles (only 2–3 of 6 sectors cross in-window).
- **Phase 2 — v1 done.** Always-on classified `FAULT:` line over UART (STALL / COAST_BURST,
  no monitor needed). **Remaining:** flight-recorder ring dump (last N commutations frozen on
  a fault), a continuous low-rate health heartbeat (always-on, not `i`-gated), and an
  overcurrent trigger (needs a continuous current proxy in the ISR).
- **Phase 4 — climb started (path 2).** `cl_climb.py` ramps frequency under closed loop and
  measures if lock holds. Key result (210→500 Hz, alpha 0.5): the 2/6 loop **scales UP** —
  lock_slow improves 0.16 (210 Hz) → **0.45 (400 Hz, best)** as the larger BEMF cleans the
  sparse detector; coast bursts drop to ~10. **Detection is not the limiter in the mid-band.**
  Hard stall at ~450–500 Hz, but that's the **open-loop spin envelope** (alpha=0.5 governs the
  frequency; only 25% amp at 500 Hz → rotor mechanically stalls), not a detection wall. The
  predict-gate A/B refuted lowering the gate (gate 0.25 → 100% coast, never acquires — the gate
  exists for good reason).
- **Next:** re-climb with **more amp** (push the open-loop spin envelope past ~450 Hz) to find
  the TRUE detection ceiling; then more loop authority (alpha→1, needs jitter work) to let the
  loop drive speed instead of the governor. The streaming harmonic detector (path 1) is the
  lever if/when detection — not the spin envelope — becomes the wall.

## Two facts that frame everything

**1. The loop is already half-built.** `cl.rs` uses the crate's tested `PLL` block as a
clamped **PI tracker on the ZC phase error** (`kp`/`ki`, `state.frequency` = period
estimate), with outlier gating, miss-coast and predict-coast, validated against a host
motor model (`tests/cl_sim.rs`). The "PLL + PI" is literally already there — it has just
never run as the control path on hardware, and it is fed by the linfit we discredited.

**2. There is a hard sampling wall.** At one 20 kHz ADC scan per PWM period:

| elec Hz | frames/sector |
|--------|---------------|
| 400 | 8.3 |
| 800 | 4.2 (linfit floor, needs ≥4) |
| 1300 | **2.6** |
| 1400 | **2.4** |

At the 1300–1400 Hz target there are only ~2.5 samples per sector — the current detection
cannot work there without raising the scan rate (~47–50 kHz for 6 frames/sector). This is
the single most important architectural fact for the high-speed goal, independent of which
detector is chosen. **(Detour in progress: a 48 kHz-PWM `scope_cl2` variant to lift the
scan rate.)**

## Strategic reframe

We have been fighting the **worst case**: open-loop, fixed schedule, where the ZC wanders
*out* of the float window (hence "2 of 6 sectors cross in-window" and the linfit
fabricating the rest). **Closing the loop makes detection easier** — a working loop steers
commutation so the ZC lands mid-window where it is sampled. The objective is therefore not
"perfect the open-loop detector across the plane," but "a detector good enough to bootstrap
lock, then let the loop center the ZC." Low Hz is explicitly not important, which removes
the hardest open-loop regime.

## Phases

**Phase 0 — Ground truth & review tooling (≈done).** The harmonic oracle + `zc_oracle_render`
is the *offline vetting* instrument, not the runtime detector. Lock it in: fix `zc_fit.py`'s
stale import (`SIX_STEP_HIGH_REV`), make the oracle render the standard host review. Every
later phase is vetted against it.

**Phase 1 — Trustworthy runtime detector (goal a).**
- Runtime detector = **sign-change, straddle-gated, primary**; the linfit demoted to a
  confidence-gated assist or dropped. The gate is the honesty fix: refuse to report a
  crossing unless the window actually straddles neutral — kills the fabricate-from-
  transients failure. Firmware-implementable.
- Vet on the saved captures against the oracle first (host), then fresh captures
  (the "oracle agreement" guardrail). Acceptance: where the ZC is in-window, runtime
  detector matches oracle within a few degrees; stays silent where it isn't.

**Phase 2 — Telemetry / flight recorder (must precede closing the loop).**
- Bandwidth reality: 115200 baud ≈ 11.5 KB/s. At 1300 Hz, per-commutation streaming is
  ~125 KB/s — 10× over budget. Two-tier telemetry:
  - **Continuous low-rate aggregates** (~20–50 Hz): mode, period_est, lock_fast/slow,
    jitter RMS, miss %, gate-reject %, coast %, Vbus, current. Always on.
  - **Triggered flight recorder**: ring of the last N per-commutation records (lf, bnd,
    zc, coast, err, period) that **freezes on a fault trigger** (desync, lock-loss,
    overcurrent, miss-burst) and dumps offline. Catches the precise failure mode without
    the bandwidth.
  - Per-failure-mode **counters** so a fault is classified: miss, gate-reject, false-ZC
    (sign disagreement), coast-run, desync events, overcurrent.
- Extends existing scaffolding (glitch monitor, run/resid histograms, DWT `isr_cyc`).
  Consider higher UART baud (G431 can) and/or binary packing for the continuous tier.

**Phase 3 — Close the loop at a SAFE mid speed.** Engage the existing PLL/PI at 300–400 Hz
(detection solid), **bounded authority** (loop only trims the open-loop schedule within a
narrow slew-limited window, instant open-loop fallback). Demonstrate lock + reduced current
vs open loop. De-risks the *loop* before touching the *sampling wall*.

**Phase 4 — Raise the sampling rate, then climb.** Bump ADC scan to ~48–50 kHz (multiple
scans per PWM period, or 48 kHz PWM — feasible on the G431 fast ADC; cost is ISR load,
watch via `isr_cyc`). Ramp speed in tiers, re-vetting the detector against the oracle at
each tier and against rev-to-rev jitter. **Fork:** if ADC sampling can't sustain clean
detection at the very top, the **comparator** path (analog ZC → EXTI, what the rm32 sister
project and production ESCs use, sample-rate-independent) is the proven high-speed fallback
— the board has the comparators.

**Phase 5 — Full characterization to 90–95 % duty (goal c).** Sweep speed/duty to the
ceiling; map lock range, jitter, current, efficiency, disturbance recovery, using the
Phase-2 telemetry to capture every edge-of-envelope failure.

## Critical path & open decisions

Spine: **Phase 1 (honest detector) → Phase 2 (failure telemetry) → Phase 3 (loop at safe
speed) → Phase 4 (sampling rate → climb) → Phase 5 (characterize).** Telemetry is
deliberately before the loop: control errors are inevitable, and the recorder must be
running the first time the loop misbehaves.

Two Phase-4 decisions, deferred until we know more:
1. **Scan-rate bump vs comparator** for the top end (ADC at ~50 kHz, or analog comparator
   → EXTI).
2. **UART strategy** (higher baud + binary packing vs the two-tier recorder being enough).

## Most practical immediate step

**Phase 1 on the host:** build the straddle-gated sign-change detector, run it against all
30 saved captures (`logs/zc_20260627_164752/`), and show in pictures (like the oracle
review) that it agrees with the oracle where crossings are in-window and stays silent where
they aren't. Pure host task, no reflash, produces the vetted detector everything depends on.

_(Current detour: a 48 kHz-PWM `scope_cl2` variant — the Phase-4 sampling-rate enabler,
pulled forward to characterize high-speed sampling early.)_
