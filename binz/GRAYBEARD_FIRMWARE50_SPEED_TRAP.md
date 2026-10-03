# Graybeard memo — firmware50 is caught in a low-speed trap of its own making (2026-09-20)

Read `firmware50/LAB_NOTEBOOK.md` E001–E024 and the fallback/estimator code in
`bin/shell-pwm.rs` (~2489–3056). First: the notebook is doing its job. Three
real root causes (E010 `Forward` on reverse wiring, E019 carrier never restored,
E018 COM41), and the level-poll → hardware-edge decision in E020/E021 was the
right architectural call, proven by E022's `lt075=0, gt150=0`. Balanced sectors
in E023/E024 mean the detector is now sound. Keep all of it.

The reason it still can't lock is not in the detector. It's the speed.

## The number nobody has put beside the runs

| image | duty | speed |
|---|---|---|
| qualified binz (E762, 121 265 commits / 30 s) | 15% | **~670 eHz** |
| firmware50 E024, 3/3 | 15% | **71–82 eHz** |

Eight times slower at the same voltage. E007 measured ±32 mV at the comparator;
BEMF scales with speed, so the detector is being asked to work on an eighth of
the signal it was qualified on. That is where the 3 000 chatter edges per
second (`too_early` 108–114k per run) come from, and why `unstable` dominates
when the read is a few µs late (E023). The detector isn't weak. The rotor is
slow.

## Three anchors holding it there — all in `bin/shell-pwm.rs`

**1. The estimator cannot represent a speed above 240 eHz.**
`ZeroCross::new_bounded(ci0, ci0/4, ci0 + ci0/2)` at line 2799, with
`ci0 = sector_interval_us(HANDOFF_EHZ=60)`. The interval floor is a quarter of
the 60 eHz interval → **240 eHz hard cap**. E020 test A saw `ci_us` "pinned at
its floor, 694" — 694 µs is that floor exactly (694 × 6 → 240 eHz) — and the
loop desynced against it. Nothing in the reference bounds the estimate this
way; AM32's only fast-rotor limit is `commutation_interval < 50` → filter 2.
The band was added in E002 to stop a *slow*-side ratchet; the slow side can
keep a bound. The fast side must be the physical maximum (~37 µs at 100%) or
nothing.

**2. The forced fallback brakes the rotor.**
Line 3038–3043: `cadence = interval.min(sector_interval_us(HANDOFF_EHZ))`, then
`next_forced = now + cadence × ZC_PATIENCE_NUM(2)`. A missed crossing produces
a commutation **two intervals after the last one — a whole sector late**. Late
commutation is braking. With `forced_pct` at 45%, nearly half the
commutations are braking events, which is *why* a motor that wants 670 eHz at
15% sits at 80. The reference in interrupt mode has **no per-sector forced
commutation**: a missed edge waits on a live comparator; only `bemf_timeout`
(22 ms) ends the attempt and restarts. E002's ratchet analysis was correct
about the slow-side spiral — but the cure (anchor to the 60 eHz grid) built a
brake into the loop.

**3. Handoff at 60 eHz, advance 26 everywhere.**
`HANDOFF_EHZ = 60` (line 2489). The qualified image hands off at **200 eHz**
(staircase 50→200, acquisition 6.1%, BEMF 7% — `binz/AGENTS.md`, E770), where
BEMF is 3.3× what it is at 60. And `ADVANCE_LEVEL = 26` (line 2521) is applied
at every speed; the qualified profile is **20 below 35% duty, 22–26 above**
(`AGENTS.md` E-advance22high). At 80 eHz, 26 puts the commutation 9% of an
interval after the crossing — very early for a slow rotor, costing torque
exactly where torque is what escapes the trap.

The loop: slow rotor → small BEMF → chatter → missed crossings → forced late
commutations → braking → slow rotor. `forced_pct` is the *symptom*; the
detector work of E017–E024 was chasing it in the wrong place. E020's finding
that "`forced_pct` improves when the loop desynchronises" is the same trap seen
from the other side: the loop tried to accelerate and hit anchor #1.

## What to change, in order, one at a time, three runs each

1. **Lift the estimator's fast-side floor** to the physical maximum (e.g. 40 µs)
   or remove it; keep the slow-side bound. Prediction: `ci_us` no longer pins
   at 694; the rotor accelerates past 240 eHz at 15% for the first time.
2. **Hand off at 200 eHz** with the qualified staircase shape, not 60.
   Prediction: chatter (`too_early`) falls by ~3× at handoff; `forced_pct`
   drops without any detector change.
3. **Advance profile from the qualified image**: 20 below 35%, not 26.
4. **Fallback semantics from the reference**: on a missed edge, keep the
   comparator live and wait; the fallback is a *timeout to restart*, not a
   late commutation on a grid. If a bench fallback is kept during bring-up,
   it fires at ~1.0–1.25× the *current* estimate, never `min`'d with the
   handoff grid and never ×2. Prediction: `forced_pct` → single digits;
   gate 1 purity and gate 2's rate identity become the same true statement.

Only after those, the comparator hardware filter (binz's TIM2 input-capture
path, `bench-filter-control`) for whatever chatter remains at speed — it's the
reference's own answer, and it also gives hardware-stamped crossing times.

## What not to do

- Don't tune the detector further at 80 eHz. Every verdict there is
  regime-scoped to a speed the qualified image never operates at.
- Don't treat `forced_pct` as the objective (E020 already proved it lies).
  The objective is speed at duty matching the qualified image — that number is
  on the bench any time: flash binz, run 15%, read ~670 eHz, flash back.
- Don't touch a protection threshold. None of this needs one.

— the minz graybeard
