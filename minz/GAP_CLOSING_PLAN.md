# GAP-CLOSING PLAN — porting AM32's architecture, keeping our safety + telemetry
2026-07-17. Synthesis of three deep source studies (ZC chain, recovery
lifecycle, duty governance) + the live register diff. Goal: close the
timing-tail gap (their 0.00 spike events/s to 2293 Hz vs our 0.3-1.4/s
above 1650 Hz) without losing a single safety layer or data channel.

## The diagnosis in four sentences

1. **We inject disturbances AM32 never does**: host-side 1 % throttle
   steps hit CCR as instantaneous edges (no firmware di/dt clamp),
   and our blind-amp clamp adds a 2/3 torque step ~270 µs into any
   miss run (AM32 rides blind for 22.5 ms without touching duty).
2. **We amplify what they damp**: our acceptance gate references the
   fast single-interval estimate (walkable by one premature accept);
   theirs references a 6-commutation rolling average a single accept
   moves ~2 % (the two-timescale keystone). Our carrier is fixed;
   theirs rises 24→48 kHz with speed (`variable_pwm` — NOT vestigial;
   the empty block at main.c:1714 is a decoy, the real map lives at
   main.c:2131: `tim1_arr = map(interval, 48µs, 100µs, ARR/2, ARR)`).
3. **We kill where they re-seed**: four kill classes (starvation,
   desync, runaway, zombie) vs their zero — they cut duty to 3-6 %,
   drop to a slow re-seed, and re-ramp; only a stuck-rotor strike
   counter ever stops their motor.
4. **Our ISR fabric is louder**: TIM1 interrupts at 48 k/s (confirm/
   census/failsafe) vs their ZERO PWM-timer interrupts; LPTIM2's
   sync overhead + 0.8 µs grain vs their TIM16 one-shot at 0.5 µs.

## Standing constraints (every rung)

- All kill/clamp layers stay live until a rung's mechanism provably
  subsumes them — then they demote to COUNTED canaries, never deleted.
  (Zombie detector and harmonic guard are permanent.)
- Every silent decision keeps/gains a counter (rejection census
  discipline). New mechanisms ship WITH their instrumentation in the
  same commit.
- One rung = one unit: core tests + named regressions, 40-60 qualify,
  top-band ladders ×2-3 paired against the previous rung, spike census
  + rejection census + rung-specific counters in the meta CSV.
- Bench scripts: validated engage, finally-kill, current-vs-f_e
  plausibility. No unguarded probes, ever.

## The ladder

### R1 — Firmware di/dt limiter (the missing safety net)
**ATTEMPT 1 (2026-07-17): PARKED behind `R1_DUTY_SLEW=false`.** Core
limiter landed with AM32-exact parity tests (143 green). Three
integration lessons, two fixed en route, one structural:
1. multi-writer slew state (TIM7+LPTIM2) ping-ponged a ~1 % duty
   dither at ~300 Hz (clamp counter +350/s at steady lock) — fixed
   via single-writer;
2. the single update initially sat in TIM7's open-loop section =
   dead code under CL (duty froze at engage value) — fixed;
3. RESIDUAL REGRESSION even when correct (paired: a70 spikes
   4.9–9.1/s vs control 2.2/s; died at 74 twice vs control 76):
   the remaining difference is that AM32's COMMUTATION NEVER WRITES
   CCR — duty lives wholly in the control tick, commutation flips
   phase ROLES only. Our `set_six_step` couples duty+roles, so any
   tick/commutation interleaving perturbs duty. **Proper R1 requires
   the duty/role split in `tim1_motor_pwm`** (a `set_duty` written
   only by the tick; a `set_roles` for commutation) — do that split
   FIRST, then re-enable the shaper. The split is also R5-adjacent
   (it removes CCR traffic from the commutation ISR).
AM32: `max_duty_cycle_change` ∈ {2,6,16}/2000 per 50 µs tick, hard,
symmetric, applied to EVERY duty write regardless of source
(main.c:1688-1708). A 50→70 % step becomes a 1.25 ms linear ramp of
0.8 %/50 µs micro-steps with BEMF re-evaluated between each.
Ours: port as a core `duty_slew` step in the control tick; regimes
selected from the interval average (startup/low/high). All CCR paths
shaped — commands, re-arms, clamps releasing, everything.
Instrument: slew-engagement counter + max-step-clamped telemetry.
Expected: transit spike seeding drops; possibly the 66→78 transit
class alone. Effort ~1 day. Risk low.

### R2 — Two-timescale gate + speed-mapped persistence (unit B done right)
Keep scheduling on the fast estimate; add a 6-slot rolling
`average_interval`; the acceptance gate becomes
`elapsed-since-last-accepted-ZC > average_interval/2` (their exact
rule, stm32l4xx_it.c:280), with the interval timer semantics: reset
ONLY on accept. Persistence depth = `map(avg, 50µs, 250µs, 3, 12)`
(deep when slow, floor 2-3 at speed) replacing our fixed 5-under-lock.
The geometry estimator's no-bounds side then becomes safe as designed;
harmonic guard + zombie stay as canaries (their counters should read 0
— any count is a regression alarm).
Instrument: avg-vs-fast divergence in telemetry; gate-reject split
(pre-gate vs persistence-fail — closes the NOZ-anatomy blind spot).
Effort ~1-2 days. Risk medium (the phase-walk class must be re-tested
explicitly: replay = engage, induce a miss via `k` edge-mode flip).

### R3 — Kills become duty-clamped re-seeds
On starvation/desync triggers: instead of killing — cut commanded duty
to ~6 %, drop to RESEED state (interval seeded slow ~2.5 ms, fresh
¾-smoothing from the next ZCs, confirm depth 2, persistence 12),
re-ramp via R1's limiter. Keep exactly two hard stops: the 22.5 ms
no-accept backstop (forced stop, r/q re-arms) and a stuck-strike
counter (bemf_timeout_happened analog with the `zero_crosses>1000`
amnesty so a healthy lock never accrues strikes). Zombie stays.
Instrument: RESEED bb events + counter; kill history unchanged.
Expected: transits that die today become sub-100 ms dips.
Effort ~2 days. Risk medium — validate the reseed can't loop-cycle
(count reseeds/s; >2/s sustained = the old kill fires).

### R4 — Variable carrier — ATTEMPTED 2026-07-17, PARKED (revisit after R5)

**R4a LANDED**: sag debounce is now carrier-scaled at the proven
2.67 ms (`timing::sag_debounce_samples` + `guards::sag_step_scaled`,
live `SAG_DEBOUNCE_LIVE`); `max_duty()` reads the LIVE ARR
(`tim1_motor_pwm::LIVE_ARR`/`set_carrier_arr`); the TIM1_UP miss
detector's period follows live ARR. Other sample-counted windows
(OC 2^11, zombie 12000, burst 8/48) deliberately stay cycle-counted:
at higher carriers they shrink = STRICTER (safe direction); their
counters would expose any over-strictness.

**R4 PARKED behind `R4_VAR_CARRIER=false`** after 4 same-session
bench iterations, each fixing a real integration defect and dying
earlier than the R4-off control (deaths at amp 70/74/70/70 vs
control 76; all the known vbat_min 4.6-5.4 V transit-surge class):
1. r4top1/2: the update block was MIS-PLACED (a 12-space anchor
   substring-matched the 28-space i-echo handler line) — carrier
   hopped 6.8 % at echo cadence, steered by reseed-transient
   intervals, exactly during transits.
2. r4top3: correct placement (TIM7, stiff-avg-driven, dwell-gated
   via AMP_STABLE_RUN>=1800, frozen in reseed/burst) but the glide
   had no deadband — 6 kHz dither (chg=68118), duty floor-erosion.
3. r4top4, the honest verdict run: deadband 96 + rounding rescale —
   chg=5, clean settle at 3172 (25.2 kHz) through the amp-66 dwell,
   climb frozen — and the 66->70 climb STILL died while the
   same-session control climbed to 76. Climbing with an elevated
   carrier is net-negative for this loop as-is.
Sequencing lesson: the plan's own risk note was right — R5's ISR
headroom (and possibly a stiffer loop) must land first. The
machinery (map/glide/gates/rescale/restore-above-the-kill-return)
is correct, tested, and stays in the tree gated off; `carr:` i-line
telemetry ships live.

#### Original R4 design (kept for the revisit)
R4a: convert every carrier-relative constant to TIME (sag debounce,
OC window, census windows — the audit already flagged them as
sample-counted). Then: `tim1_arr = map(interval_us, 48, 100, ARR/2,
ARR)` — carrier rises 24→48 kHz through the danger band, duty ratio
preserved. We've proven both endpoints; this makes it proportional.
Check list: ADC trigger fits min ON-window at max carrier (recipe
known: 47.5-cycle phases, 12.5 current, trigger 1.25 µs); confirm
cadence changes with wrap rate (E2a logic reads interval, unaffected);
GECKO PERIOD_CYC uses ARR — make it read live ARR.
Instrument: live carrier in telemetry + meta CSV.
Effort ~1.5 days. Risk medium-high (the 48 kHz CPU lessons apply at
the top of the map — but R5 buys the headroom back).

### R5 — ISR diet — RESOLVED 2026-07-17 (R5a landed; R5b/R5c closed with verdicts)

**R5a LANDED (f425082)**: TIM1_CC retired — the COMP ISR derives
blank position from a direct TIM1.CNT read (edges at CNT=duty, plus
CNT=0 in open loop; exact parity with the old CC-stamp semantics in
both regimes). 17-72 k IRQ/s gone; bench r5a1 = no regression
(locked 60-70 identical, died at the same lottery-band 74 transit
as the same-session control).

**R5b (TIM15 one-shot retry) CLOSED — already answered by the
2026-07-13 one-variable A/B (6009c15)**: LPTIM2+floor2+reorder beat
TIM15 decisively (jitter 0.9-1.0 % vs 3.4 % @amp55, qzc 100 % vs
99 %, same amp-64 envelope). The jitter is TIM15's APB2 contention
with the injected ADC — structural, NOT the walkable-gate artifact
this plan hypothesized, so R2 does not reopen it. Do not retry
without an APB2 fix.

**R5c (TIM1_UP census thinning) CLOSED — deliberately skipped**:
CPU is not the wall (the 07-13 carrier A/B decoupled spike events
from CPU health; 45 % under load today), and half-rating the census
above the danger band doubles the burst responder's detection
latency (333->666 us) exactly where it matters. Trading live-fire-
proven safety cadence for a non-bottleneck fails the standing
constraint. Revisit only if a future rung is actually CPU-bound.

#### Original R5 design (kept for reference)
Toward AM32's fabric (COMP + commutation timer + control tick, zero
PWM-rate interrupts):
- Commutation timer → GP timer one-shot at 0.5 µs grain (TIM15
  retry, PSC=39): kills LPTIM2's ~3 µs full-path overhead + 0.8 µs
  grain + ARROK machinery. The May TIM15 conviction (pred jitter
  3.4 %) predates the two-timescale gate — plausibly the same
  walkable-gate artifact, not the timer; retry WITH R2 in place.
- TIM1_CC retired: blank timestamp from TIM1.CNT read in COMP (or
  drop blank above the speed where it's ≤1 µs anyway).
- TIM1_UP diet: confirm path is SWIFT-bypassed at speed already; move
  census/wax harvesting to half rate above the danger band.
Instrument: DUR counters (already per-ISR) prove each cut.
Effort ~2-3 days total, sub-rungs individually gated.

### R6 — Self-paced start — LANDED 2026-07-17 (start proven 9/10; full-ladder integration pending)

`minz_core::start::StartState` (7 named tests) + firmware `Y` key:
arm through the mode machine at pinned amp 6, rotor-paced sector
stepping in TIM7 (comparator LEVEL vs expected post-ZC, doubled
counts + wait-skip first 5 crossings, 50 ms blind backstop),
handoff after 4 consecutive sub-2.5 ms crossings fires the normal
`y` engage from main (target raised to amp 10 for the slew).
Live bench: FIRST-TRY engage; then n=8 validation = **8/8 CL
ACTIVE** (22-25 rotor-paced crossings, <=1 blind step each) vs the
documented ~50 % single-shot lottery; 1 later attempt died into a
clean ZC-STARVED guard kill (9/10 overall). Telemetry: `r6:` i-line
(on/cross/blind/handoff-interval). CAVEAT: a manual climb-to-40
after an R6 start pinned at 439 Hz / 3.5 A because the session ran
without the qualified D-geometry + SWIFT recipe (reflash resets
both; the pre-SWIFT confirm ceiling is ~500-650 Hz) - wire
`cl_lock_map --r6-start` and run the full paired ladder next.

#### Original R6 design (implemented as above)
AM32's start: polling mode, duty pinned ~6 %, commutate on
`min_bemf_counts` consecutive comparator-level agreements (doubled
for the first 5 crossings, wait-skip to kick the rotor), hand to
interrupt mode only when interval < 250 µs (~667 Hz). No scheduled
handoff instant to miss. Port as our engage path (our 20-24 kHz tick
supports polling below ~667 Hz — exactly the engage regime).
Instrument: start-phase bb events; crossing-count telemetry.
Effort ~3-4 days. Do last — the engage-lottery fix already made
starts reliable; this makes them principled.

## 2026-07-17 addendum: the transit-death autopsy (TRANSIT_AUTOPSY.md)

The 70-78 transit kill class is now root-caused: a survivable
ZC-miss run triggers burst/reseed correctly, but the RECOVERY
re-ramp runs at the at-speed slew class (16 %/ms) into a
dip-slowed rotor - a winding-limited multi-amp surge that sags the
bus, seeds the next miss (reseed storms, strikes=3 observed) and
eventually holds the sag kill. The fatal bb shows a PERFECT loop at
the kill - the energy transient kills, not the timing. Decision:
harden the recovery (startup-class slew until the duty catches its
target), NOT commutation latency. Full evidence + fix design in
TRANSIT_AUTOPSY.md.

## Validation milestones

- After R1+R2: top ladders should show the spike census dropping
  toward AM32's 0.00 in the 1650-1950 Hz band; rejection/canary
  counters near zero under lock.
- After R3: transit deaths become logged reseed dips; ladder-to-85
  attempt with the recovered envelope.
- After R4: the 1850+ band re-censused at the scaled carrier —
  expectation: matches AM32's tail at matched carrier.
- After R5: DUR/miss counters at AM32-like ISR load; 48 kHz top of
  the R4 map becomes affordable.
- Reference re-runs: the white-box AM32 rig (UART build) stays
  available for same-day A/B whenever a rung's result is ambiguous.

## Errata this plan supersedes

- CARRIER_REQUAL.md: "variable_pwm is an empty block/vestigial" is
  WRONG — the live map is main.c:2131; the AM32 reference curve was
  taken at a dynamic ~27 kHz (at 1900 Hz) rising toward 48 kHz. The
  24-vs-48 fixed-carrier comparison stands, but AM32's operating
  point is the interpolation — which is R4's whole thesis.
- "The 45 ms BEMF timeout" lore: TIM2 ticks at 0.5 µs → it is 22.5 ms.
