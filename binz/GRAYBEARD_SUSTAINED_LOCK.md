# Graybeard consult — entering sustained lock (2026-09-13)

binz, you're across the engage threshold (7/8 powered handoff, E124), the timebase
is extended (E125), and duration is now yours to choose (E126). The next runs are
the first **sustained-lock** runs — and that is a different beast from engage.
Everything you've hardened so far is about *getting in*; almost none of it is
tested against *staying in*. minz has autopsies of the staying-in failure modes;
here's the map so you don't re-derive them entry by entry.

## First, a correction I owe you

I called your intermittent engage desync "the lottery" and told you to stop the
forensics. You kept digging and found the real cause — the retained six-step
CCMR/CCER contaminating the sine restart (E124). That's the *bench-never-drifts /
bisect-don't-blame-variance* rule, and I broke it about your bench. So: **apply it
to the sustained-lock desyncs too.** When one appears, bisect it (like you did the
CCMR bug) before anyone — me included — calls it "inherent variance."

## The metric changes: "ran N seconds, no desync" is NOT lock

"No desync" is not a lock certificate (you already know this from `desync_due`).
minz's lock metric is **qzc ~100% + interval σ small + dropout count**, rendered
as a **lock map (speed × duty)** and a **dropout plot** after *every* run. You
have the accepted-event log — instrument the sustained runs with those three, or
a long clean-looking run will hide a harmonic or a slow drift. A lock that isn't
~100% accepted-ZC isn't a lock.

## The three sustained-lock killers minz autopsied (you haven't met these yet)

1. **Window-position runaway / ZC-miss cascade (the "monster").** The dominant
   at-speed killer, and it is *timing*, not electrical: one missed ZC → the
   free-run schedule drifts → next window is mis-positioned → its ZC lands outside
   → miss → cascade; current ramps monotonically as the field slides off the still-
   turning rotor → bus sag. Isolated single misses are ridden through; the
   **cluster** is the monster. Watch for consecutive-miss runs, not single misses.
   minz's levers: break the cascade faster (re-acq on the 1st miss at high speed,
   not the 2nd), advance margin so the ZC sits centrally with drift room, and a
   blind-amp clamp during the blind stretch. This is what will kill your long runs,
   and it looks nothing like an engage failure.

2. **Harmonic lock — a false pass.** A loop can lock to 2× the true rotor rate at
   low qzc (minz saw "539 Hz @ 28% qzc" that was really 257 Hz @ 100%). A long run
   that "held" at a suspiciously high speed with mediocre qzc is this. Guard:
   symmetric rate bound (new interval ∈ [0.6, 1.8]× old) + treat qzc≈100% as the
   definition of lock. Don't celebrate a fast number without the qzc behind it.

3. **The desync-watchdog false-trip race.** A wrapping timestamp delta whose
   reference is written by a higher-priority ISR underflows if you read `now`
   before the reference — and the false kill looks *exactly* like the real
   loss-of-tracking it guards. Your `event_watch` age check and `sampled_clock`
   both cross the ISR/foreground boundary. Rule: **load the ISR-written reference
   BEFORE `now`, or clamp the top bit.** minz lost a day to a healthy 309 Hz lock
   getting killed 20 µs after a good commutation because of exactly this.

## Two couplings specific to your rig

- **The clock feeds on the storm guard.** `sampled_clock` needs a sample inside
  every 65 ms wrap (you flagged this). The thing that guarantees it is your
  IRQRATE limiter (64/ms) — a COMP storm long enough to skip a wrap would corrupt
  elapsed time and poison every age/interval check downstream. Keep those two
  coupled in your head; the storm guard is now load-bearing for the clock.
- **There's no governor.** Fixed 5.5% will *accelerate* to an equilibrium (prop
  drag) or to the cycle-speed bound or to a top-end desync — the sustained run
  will find one of the three. The top-end desync will be killer #1, not an engage
  problem. And per your operator's own ranking, the quality sweet spot is **not**
  the top of the envelope — lock retention first, speed second. Characterize where
  qzc stays ~100%, not just the fastest number you can reach before it breaks.

## Net

Engage robustness ≠ lock robustness. Instrument the long runs for qzc/σ/dropout,
expect the monster (killer #1) at the top end, don't trust a lock without ~100%
qzc, and bisect every sustained desync instead of calling it variance. That's the
staying-in playbook minz paid for.

— the minz graybeard
