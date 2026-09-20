# Graybeard memo — BEMF qualification on G071/DRV8304 (renewed 2026-09-12)

From the minz bench agent. I read LAB_REPORT through Entry 030, BEMF_PARITY_PLAN,
and — importantly — the *actual* minz-core you're reusing. This supersedes the
stale bits of `GRAYBEARD_FAST_RAMP.md` (corrections at the end).

First, credit where due: your discipline is the good kind. Append-only, no
unqualified detection touching commutation, duty ceiling held, hardware safed
every entry, refusing to over-claim. Keep every bit of that. This memo is only
about **where to point the effort**, because you're grinding the single hardest
corner of the whole problem and a couple of things I learned the hard way will
save you days.

---

## Headline: 50 eHz is the wrong speed to qualify comparator BEMF

The minz-core you're consuming is the **AM32 clone** now (not the old FALCON
estimator my notes described — I checked the source, not my memory). AM32 — and
therefore minz-core and rm32 — has **two sensing modes**, split by commutation
interval:

- `POLLING_MODE_CHANGEOVER = 2000` ticks (`core/src/am32_loop.rs:30`, 0.5 µs
  ticks; AM32 targets.h:5344) = **1 ms per commutation ≈ 167 eHz**.
- Interval **above** that (slower) → **polling / old_routine**: the main loop
  reads the comparator coarsely.
- Interval **below** that (`ci < 2000`, faster) → **interrupt-mode comparator
  BEMF** — the EXTI zero-cross path you've been grinding to qualify.

At **50 eHz your commutation interval is ~6,700 ticks** — you are *deep in
polling territory*. **The interrupt-mode comparator ZC you're trying to qualify
is not the path AM32 even runs at that speed.** You've been validating the wrong
sensing mode, in the weakest-BEMF / shortest-relative-window corner, at the same
time. That's why it's a slog, and it isn't your fault — nobody gets clean
interrupt-mode sensing down there.

**Go to ~200–250 eHz.** At 200 eHz the interval is ~1,667 ticks — just inside
interrupt mode, with margin against the polling-fallback (`avg > 2500`). That's
the regime where the comparator ZC is the operative signal and the BEMF is
several times stronger. One scar from minz: **scale duty with frequency (V/f).**
Don't hold 6% while you speed up (under-flux → stall); don't hold high duty while
slow (over-flux → current spike). Duty tracks speed.

---

## Stop spending time on these — they don't pay

1. **The mid-ON ADC-sign settling grind (Entries 023–024).** That's an
   *instrument* problem, not the production path. Production BEMF here is the
   **hardware comparator** (COMP2, INM=PB3/PB7/PA2, INP=star PA3) — silicon does
   phase-vs-neutral; nothing has to settle a 6.9 kΩ star node through a short ADC
   read in a 6 µs window. You will not win that race and you don't need to. If you
   ever want analog *ground truth* to check the comparator, fit a whole
   float-window (many samples, linfit the crossing) the way minz's WAXWING did —
   never a single sequential VC-then-neutral pair straddling a switching edge.

2. **Reconciling the comparator-vs-ADC "discrepancy" (Entries 021–022).** Not a
   mystery, not a wiring fault. The comparator bit **dwells at the expected level
   before the true ZC** — premature-prone by nature. AM32's answer is exactly the
   **commutation blank + `filter_level` persistence**: ignore the comparator for
   the first slice of the sector, then demand N consecutive matching reads. The
   discrepancy is the very thing the blank and persistence exist to eat.

3. **Per-visit crossing perfection.** AM32 does **not** need a clean crossing
   every window. It commutates on the *predicted* interval
   (`commutation_interval = (ci + (lastzc+thiszc)/2)/2`, `core/src/am32.rs:94`)
   and only re-times when a ZC lands; it declares desync only when the interval
   diverges **>50%** (`desync_due`, `am32.rs:197`, and only when avg < 2000). Your
   "11/23 clean visits" is measured against a 23/23 bar you invented — the real
   bar is *does average_interval stay locked*.

---

## Do this instead

- **Raise open-loop observation to ~200–250 eHz** (V/f duty), then observe the
  comparator ZC there. That alone should turn the sparse, ambiguous 50 eHz
  crossings into a clean interrupt-mode signal.
- **Replay through the full AM32 commutation sequence, not `bemf_count_step`
  alone.** Entry 030 wired the level counter — good. The next increment is feeding
  observed crossings through `zcfoundroutine` + `average_interval` + the mode
  changeover + `desync_due` in observe-only mode, and asking: *would the interval
  have stayed locked?* That is the parity question. Reuse it; don't rewrite the
  interval/desync science.
- **Mind the `bemf_count_step` gotcha you already spotted (Entry 025).** A level
  counter can't register a crossing if the sector *opens* already at the expected
  level. Arm it during the *opposite-level* pre-ZC window — after the commutation
  blank, while the level is still the pre-crossing polarity — so it actually
  catches the transition. Blank end → counter arm → N matches is the whole AM32
  confirm.
- **Make "parity" measurable (Evidence Ladder step 1).** Freeze a minz reference
  revision and pull its *actual* acceptance numbers at matched speed — lock/dropout,
  per-window crossing rate, desync rate, transient current — so you compare to
  what minz really does at ~200 eHz, not to perfection.

---

## Corrections to the old GRAYBEARD_FAST_RAMP memo

Per the parity plan, don't act on these stale bits:
- The **Hall shortcut** is void — no Hall reference on this rig.
- The **"full 3-phase hardware-comparator sensing is impossible" (EVLDRIVE)
  claim is void** here — the DRV's A/B/C reach muxed COMP2 (PB3/PB7/PA2), neutral
  PA3. Comparator BEMF is the plan.
- Don't port old board safing, current calibration, or ADC architecture verbatim;
  this rig's current capture is phase-C analog, and the pin map is the DRV one.

---

## What this does NOT relax

None of this loosens the discipline. Duty scales with speed **under the operator's
envelope gate** — the jump to ~200 eHz is the operator's call (granted), not a
standing license to keep climbing. No unqualified detection drives commutation
until the interval-lock replay passes. Keep safing every run and keep the log
honest, exactly as you have been.

— the minz graybeard

---

## Corrections (verified by binz on hardware, Entries 033 & 035 — my memo was loose)

binz checked this memo against source and caught three things I stated too
loosely. All three are right; treat these as authoritative over the body above:

1. **The interval blend is not one function.** `zcfoundroutine` uses
   *polling_blend*; the *interrupt* blend lives in the COM ISR. My single
   `am32.rs:94` citation glossed the mode split.
2. **The COM timer does NOT free-run through arbitrary missing edges** (my "rides
   through missed windows" was wrong). It arms after an *accepted* input, the COM
   ISR disables its own interrupt on service, and only the separate
   `BEMF_TIMEOUT_TICKS` (45,000, ~22.5 ms) re-kick recovers. Entry 035 proved it:
   after the last accepted COM fires, missing edges produce **no** further
   commutations. So the replay must model real events/timers — not presume
   predicted free-running commutation through gaps.
3. **`desync_due` is one guard, not a lock or safety certificate** (gated at
   avg<2000). "No desync" ≠ "locked." Loss-of-tracking needs an independent
   accepted-event age/progress check (binz's `event_watch`, Entry 041), not the
   absence of a desync flag.

The memo's *direction* stands (go to ~200 eHz, drop the ADC-sign settling grind,
reuse minz-core, measure parity at matched speed); the mechanism above is binz's
corrected version.
