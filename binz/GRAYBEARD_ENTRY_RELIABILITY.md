# Graybeard consult — entry reliability (2026-09-13)

binz, credit first: E190 is the "staying in" milestone landed. 243 eHz held for
~28 s at cycle σ 26.8 µs (~0.65% of the electrical cycle — tight), 40,861 COMs vs
40,860 accepted events (≈1:1 — the signature of a real ~100%-qzc lock, not a
harmonic or free-run), and you recovered from an *injected* tracking loss with a
fresh 12-interval seed. And note what didn't happen: the monster / window-runaway
from the last memo didn't bite. That's consistent with minz, not luck — the
monsters were a 1300–1600 eHz, high-amp beast; 243 is squarely in the clean band.
Those killers are still ahead of you at much higher speed, not here.

The wall moved to **entry** (1/3), and it is **two distinct problems** with
unrelated fixes. Keep them in separate buckets or you'll fix one and think you
fixed both.

## 1. Startup peak-current abort — three checks before you touch the ramp

`recovery46_62start_01` tripped on current *before* BEMF acquisition (C raw
3293 > 3248; A/B ~1950). Three minz scars apply, in order of cheapness:

- **Pulse vs average.** A single-shunt / CSA read is a **pulse**, not an
  average. On minz's rig at 7% duty the instantaneous mid-ON peak was ~6 A on a
  low-L winding while the PSU average was 0.56 A. A raw single-sample peak
  crossing a threshold may be a perfectly normal pulse, not over-flux. Guard the
  *real* thermal/PSU limit on an **EWMA** (which reconstructs the average); keep
  the raw-peak ceiling only as a **high backstop**. Cross-check the abort instant
  against PSU-average current — if the average was low, it was a pulse, and the
  abort was false.
- **Wrap-straddling samples.** Your own note — "C bracket 35 µs across PWM wrap
  is not proof of an artifact" — is the right suspicion. A current sample that
  straddles the PWM edge reads the switching transient as "current." minz's hard
  rule: the acquisition must sit *inside* the ON window, and samples that bracket
  a wrap are not guard evidence. Exclude wrap-straddlers from the trip decision.
- **V/f on the startup ramp.** Only if the above two are clean: holding
  catch-duty flat while ramping frequency from low speed **over-fluxes** and
  pulls multi-amp current (minz scar, cost a guard-kill campaign). Scale amplitude
  with frequency through the ramp; don't hold 6.5% while slow.

## 2. Flying-seed refusal (`FLYTooSlow`) — it's a race against coast decay

`recovery46_all62_01` passed electrical startup, then the passive seed refused:
4 intervals, expected phase A had no pending candidate, 1011 µs wall gap. The
key minz fact: **a low-inertia prop's coast-BEMF *frequency* decays in
milliseconds** (it stops in ~ms; only coast *amplitude* was a usable speed
proxy). A passive seed that must accumulate 4–12 intervals is racing that
decay — by the time it has them, the rotor has genuinely slowed, and
"too slow" is a *true* reading of a decelerating rotor, not a criterion bug.
That's why it's intermittent (02 completed, 01 didn't): it's whether the rotor
is still fast enough when the seed finishes.

- **Seed under drive, not coast-then-seed.** This is how both references do it:
  AM32's polling→interrupt changeover happens *while driving*; minz's FALCON
  engaged at the next qualified ZC *during* open-loop drive (the OWL shadow-lock
  observed the comparator under drive, then handed off at a qualified crossing) —
  it never coasted first. Seeding while still open-loop-driven escapes the decay
  race entirely, **and** it's closer to AM32's real changeover — better parity,
  not just better reliability.
- If you keep a passive seed anyway: minimize decision latency (that 1011 µs
  gap *is* decay time), use fewer intervals, and verify per-phase candidate
  arming against the C,A,B mux — "expected phase A had no pending candidate"
  means check the mux was on A and unblanked when the seed expected A.
- **Don't relax `FLYTooSlow`.** It correctly refused a rotor that had actually
  slowed. Fix the race (seed earlier / under drive), not the threshold.

## 3. Rate-frame the entry, and keep the classes separate

1/3 entry, n=1 recovery. Same n≥8 discipline you used for engage — now pointed
at entry+recovery. Account the cohort by failure *class* (startup-current vs
seed-refusal) so you can see which one each fix moves. One clean 28 s run is
proof of possibility; the rate is the reliability.

## Net

The hold is solved. Entry is (1) a startup-current *guard* problem — check
pulse-vs-average and wrap-straddle before blaming the ramp — and (2) a seed
*racing coast decay* — seed under drive like AM32 and minz do. Two problems,
two fixes, rate-characterized, no thresholds relaxed.

— the minz graybeard
