# Status checkpoint — 2026-07-04

Snapshot of where we are against the three roadmap objectives, and the data gates that
prove (or disprove) we're on track. Re-read this before resuming work; if what we're doing
doesn't serve a gate below, we're drifting.

## Roadmap objectives (original)

a) A ZC detector reliably working across the operating range, trustworthy and host-vetted.
b) A control loop (PLL + PI).
c) Full validation up to ~1300–1400 elec Hz (90–95% duty).

## Where we stand

### (a) Detector — partly done, trust not yet earned
- Three detectors exist: **sign-change** (honest, sparse: ~2–4/12 in open loop),
  **linfit** (fabricates crossings from transients — retired from trust),
  **harmonic observer** (81% coverage, matches offline oracle to ~2.7°, fits ISR budget
  after amortization: full pipeline 3272 cyc / 92% of the 3541 budget).
- BUT: the harmonic is validated **observe-only, open-loop, vs the oracle**. Never verified
  against the raw waveform on a confirmed-spinning **closed-loop** capture.
- CPU: 92–99% of budget; `FAULT: CPU_HIGH` during high-Hz ramps. Usable, not comfortable.

### (b) Control loop — half done
- **Governed mode works** (commanded rate + P-only phase trim, `on_frame_blend`):
  fresh motor, eyes-verified: open-loop stalls **540 Hz**, closed-loop **700**,
  + timing advance **780**. The loop genuinely earns +240 Hz over open loop.
- **Free-running PLL+PI** (`on_frame`, rotor sets its own speed — what (b) actually means):
  desyncs and stalls immediately at the hard governor→PI handoff. **Unsolved.**
- Commutation timing advance shipped (`<`/`>` knob + `/` speed-proportional schedule):
  fixed 3–6° lifts 700→780 and cleans lock; 9° over-advances the low end (540).
  Speed-proportional also caps at 780 → the 780 wall is NOT an advance problem.

### (c) Range validation — at 780 of 1300, blocked on measurement
- Ceiling walked 540 → 700 → 780 and stopped. 20-Hz fine steps don't get past it either.
- Structural note: at 1300 Hz there are only ~6 valley samples/sector. Sign-change
  (2 blank + 2 confirm) barely fits — the top of the range was always going to need the
  harmonic's pooled fit. Objectives (a) and (c) are coupled.

### The meta-problem that cost the most time: WE CANNOT MEASURE
- `lockS` reads high (0.6–1.0) on a **stalled** rotor. `late_swing` reads high (898) on a
  **stopped** rotor. `classify_rotor_state` is advisory-only in closed loop
  (plateau_spread/sensing gates mis-fire; no captured scalar separates spinning from stalled).
- Every "held to X Hz" telemetry claim was wrong until human eyes corrected it.
- The old motor was partly damaged (repeated hard stalls) and skewed early results;
  fresh motor confirmed the ~700–780 ceiling is control, not hardware.
- User observation, currently uncontradicted by any data: **"doesn't sound like a clean
  lock even at low frequency."** We have NEVER examined a raw waveform captured while the
  closed loop verifiably drove a spinning motor.

## Data gates (in order — each one provable/falsifiable)

- [x] **Gate 1 — grounding datum. DONE 2026-07-04, MAJOR FINDING.** Verified-spinning
  420 Hz capture (`logs/spin_look_420.log`, vbus 6.7 V, iu 1458 mA): **phases B/C show clean
  six-step staircases; phase A NEVER participates** — rides 2700–3100 counts (ABOVE the
  ~1700-count driven rail) the whole capture, never pulls low. Signature of an undriven/
  floating phase A → motor torques in only 2 of 6 sectors → "runs but never sounds locked"
  CONFIRMED as a real drive fault, not loop tuning. Oracle found crossings only on C;
  sign-change 0/12; A float-minus-neutral +1600 in every A window.
  **DIAGNOSIS COMPLETE (2026-07-04), all from existing capture data — no scope needed:**
  motor coils identical (user-measured) → wiring fine. Firmware verified symmetric (pins,
  enables, CCR writes, float logic, DRIVE tables). Currents in the same capture: I_A (ch13)
  flat at bias in ALL 12 sectors; I_B (ch16) saturating rail-to-rail. Voltage: A never
  driven either direction. → **BOARD FAULT: phase-A half-bridge (gate driver / FETs / leg)
  is electrically dead.** Dated via archived raw logs: June 28 captures show A with a full
  healthy staircase (min 0, max 1910) — **A died during this week's stall-heavy sessions**
  (scripts ran stall_kill OFF; every seizure cooked energized windings until manual power
  cut). Death likely LATE (after the 540/700/780 envelope runs — those had 0.85–0.98 locks;
  the erratic 20-Hz-step run afterwards is the plausible break) — but unprovable because
  cl_gov_sweep didn't save raw captures per plateau.
  **Consequences: (1) board must be replaced/repaired before ANY further envelope work;
  (2) keep stall-kill ON in all scripts from now on; (3) cl_gov_sweep must save a raw
  capture per plateau so faults can be dated post-hoc; (4) treat 540/700/780 as probable
  but re-verify on the new board.**
- [ ] **Gate 2 — loop correctness.** Same capture: oracle (`zc_fit`) crossing angles vs
  where the loop actually commutated → real phase-error distribution incl. the ~60%
  coasted commutations. If commutations land tens of degrees off, that IS the chop.
- [ ] **Gate 3 — detector truth-rate in closed loop.** Fraction of harmonic / sign-change
  firings that coincide with a real waveform crossing (vs artifact). This is what
  "trustworthy detector" (objective a) actually requires.
- [ ] **Gate 4 — identity of the 780 wall.** Capture pair at 700 and 780: what degrades
  (samples/sector, detection rate, phase error, current signature). Turns the
  "detection-resolution limit" hypothesis into a diagnosis.
- [ ] **Gate 5 — machine-readable rotation check.** Derive from verified captures (real
  float-window crossings at the commanded rate). Until then every sweep is anecdotal;
  eyes don't scale.
- [ ] **Gate 6 — free-running PI holds at one speed** (objective b proper). Gentle
  governor→PI handover instead of the cold switch. Only attack after Gates 1–4.

## Discipline rules learned the hard way (do not re-learn)

1. **Trust order: human eyes > raw waveform capture > any aggregate metric.**
   `lockS`, `late_swing`, coast counters all lie in closed-loop drive.
2. Check `vbus_mv` / `iu_ma` before interpreting ANY capture (first `cl_spin_look` run
   analyzed a powered-down bench).
3. Reflash between heavy experiment runs — toggled state accumulates and produces
   non-catching spin-ups that look like control failures.
4. One variable at a time; re-run once before believing any single sweep (run-to-run
   variation at identical setpoints is large).
5. Amp is tuned out: sweet spot ~31–36% (24% → torque floor; 44% → mistimed-current
   braking). Don't re-sweep amp.

## Tools (all committed, branch `bisect_init_changes`)

- `scripts/cl_spin_look.py` — THE observation tool: one spin, waveform PNG + per-commutation
  log + debug line. Gates 1–4 run through this.
- `scripts/cl_gov_sweep.py` — eyes-in-loop Hz sweep (announces each plateau; classifier
  column is ADVISORY). `--advance`, `--adv-sched`, `--signchange`.
- `scripts/cl_handoff_probe.py` — governor→PI handoff test (frozen-counter = STALL).
- `scripts/cl_harm_check.py` — harmonic vs sign-change coverage + ISR cost breakdown.
- Firmware knobs (scope_cl2_48k): alpha `0/1/2/3,n/m`, zc_beta `,/.`, advance `</>`,
  adv-sched `/`, harm mode `;`, harm-drive `'`, PI drive `o`, monitor `i`.

## Next action (when bench is powered + COM41 free)

Run Gate 1: `python scripts/cl_spin_look.py COM41 --hz 420 --amp 31 --alpha 0.4`
— check `vbus_mv`/`iu_ma` first, confirm rotation by eye, then read the PNG + cl-log.
