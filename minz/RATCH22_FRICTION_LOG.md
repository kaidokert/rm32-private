# ratch22 onboard-analytics friction log

First real firmware consumer of ratch22's online families, wired into the
am32_clone monitor (`minz/src/monitor.rs`, `feature = "monitor"`, full tier) on
the STM32L431 bench: short/long **trend** (`I32OrdinalSlope`), **P² median +
p90** (`P2Median`/`P2Quantile`), and a **window-retuned histogram**
(`WindowRetunedHistogram`) whose config epoch is published through a `.frame()`.
Fed per-commutation from main context. Validated spinning a real BLDC at
20/35/50% throttle.

This is the API-hardening milestone's input: what fought us, what a real
budget-bearing consumer needs, captured before names freeze.

## Blocking / design gaps

1. **No range→Q envelope calculator.** ~~`calculate_online_i32_envelope` does
   not exist yet.~~ **RESOLVED by PR#20** (`fixed_config`), named exactly. We
   ADOPTED it: the interval-shape block-M4 bank is now configured by
   `calculate_block_moment_envelope_for` from a declared `PhysicalRange(-2,2)`
   instead of a hand-picked `WideQ32BlockMoments` + guessed `SHAPE_GAIN` bound.
   Bench-validated identical (skew/kurt match the hand-picked version).
   **Calculator ergonomics verdict (the milestone claim — survives contact):**
   - POSITIVE: it's `const fn`, so a bad range fails the BUILD, not a runtime
     `try_new` — exactly the compile-time proof we wanted. Const-generics from
     `PLAN.sample.maximum_absolute_raw` / `PLAN.power_fractional_bits` /
     `PLAN.maximum_samples` work on edition-2024 / rustc 1.96. Covers both
     personalities we use (`BlockScaledI32Math`, `FixedBlockMoments`).
   - Minor: the `const` call site is verbose — `match PhysicalRange::try_new(..)
     { Ok=>, Err=>panic!() }` twice (const fn can't use `?`). A const
     `*_or_panic` / unwrap helper would tidy every declaration.
   - Gap: the plan derives sample-Q + power-scale + raw-bound + accumulator
     width, but NOT the OUTPUT Q (`I64OutputQ<32>` still hand-chosen) — fine,
     but the plan doesn't close the whole type.

2. **Histogram initial bounds are unusable without the calculator.** First cut
   used `ExpandOnly` with a guessed `(0, 512)`; the ~38-count current all landed
   in bin 0 (`[256,0,0,0,0,0,0,0]`) and `ExpandOnly` can only *widen*, never
   recover. Switching to `Margin<4>` fixed it (tracks observed extrema → a clean
   bell), but that is a workaround for not knowing the range. Either the
   calculator, or a "seed bounds from the first window then lock" mode, would be
   the ergonomic answer. **Secondary:** `Margin` alone retunes almost every
   window (epoch 145 over ~184 windows) → the epoch is noisy; `Hysteresis<M,D>`
   is presumably the intended damper but that's a second knob to discover.
   **RESOLVED by PR#21** (`calculate_histogram_plan`, const histogram planner).
   ADOPTED: the current histogram is now planned from a declared operational
   `PhysicalRange(0,96)` + margin + deadband, constructed via
   `PLAN.try_window_retuned_histogram()`, and switched to `Hysteresis`. Bench
   A/B confirms the damper: over a 628-window 35→50→35 run the epoch stepped
   only ep4→ep9→ep14 (real regime changes) vs `Margin`'s ~1-per-window churn
   (145→305), while the histogram still resolves the current distribution.
   Planner ergonomics: `const fn`, compile-time, `PhysicalMagnitude::integer`
   avoids fallible margin/deadband construction; the plan's `.margin_raw` /
   `.hysteresis_deadband_raw` feed the `Hysteresis<M,D>` type params directly.
   Cost +~120 cyc (deadband bookkeeping). Clean; compiled first try.

3. **No frame serializer / wire codec.** `.frame()` cleanly surfaced the config
   epoch, but `FrameContext` fields are private (read-only via const accessors)
   and `SchemaId` is a `&'static str`, not wire-friendly. To publish over UART
   we hand-packed the epoch + counts and mapped the schema to our own numeric
   code. Every embedded consumer will reinvent this. A compact
   `no_std` frame codec (or at least numeric schema IDs) would pay off.

## Ergonomic friction

4. **Constructor naming is inconsistent.** `I32OrdinalSlope::new()` (infallible)
   vs `P2Quantile::try_new()` / `WindowRetunedHistogram::try_new()` /
   `OnlineBank::try_new()` (fallible). Tripped on it; pick one convention.

5. **`.frame()` requires `window.sample_count() == snapshot.samples()`** exactly
   (else `SampleCountMismatch`). We used `snap.histogram().samples()` so it
   matches by construction, but a monitor that computes the count elsewhere has
   a footgun. Consider a `snapshot.frame_auto(config)` that fills the count.

6. **Multi-window trend is DIY.** Short/long = two `I32OrdinalSlope` instances +
   `clear()` on each boundary. Works, but a built-in short/long or sliding
   trend would remove boilerplate (and the boundary-bookkeeping bugs it invites).

7. **The `2*OPERAND_SHIFT == FRAC_BITS` coupling on `BlockScaledI32Math` fails
   only at runtime** (`InvalidConfiguration` from `try_new`/`build`), never at
   compile time. A `const` assertion or a type-level derivation would catch it.

## Family notes (F0-portability relevant)

8. **P² is f32-only** — no integer variant. Fine in main context on the M4F
   here; but it bars P² from the prio-0 ISR and from FPU-less F0/G0 without
   softfloat. If streaming quantiles matter on M0, an integer P² (or a
   documented "P² stays host/main-side on M0") is needed.

9. **`BlockScaledI32Math` only helps `OnlineBank`, and expects normalized
   inputs.** Raw 12-bit ADC at Q8 overflows its i32 product (65520² > i32::MAX),
   so the float-free block personality can't be dropped onto a raw-ADC channel
   without normalizing first — which again wants the range calculator. NOT used
   in this slice.

10. **Positive: trend / histogram / P² are envelope-free at the input** — they
    take raw `i32` (trend, hist) or `f32` (P²) directly, no Q wrangling. Only
    the `OnlineBank` block-scaled path needs the (missing) calculator. So the
    three families we wired were the *easy* ones; the calculator gap bites the
    bank, not these.

## What worked cleanly

- Every `update()` is O(1), allocation-free, and returns `Result` (no panics) —
  clean to feed from a hot path; checked policies are transactional (a rejected
  sample corrupts nothing).
- Compiled first try once the exact signatures were known.
- State is small: trend ×2 + P² ×2 + retuned histogram ≈ 210 B.

## Policy choices made (deliberate, per the ask)

- **Trend → `CheckedOverflow`:** the i128 sufficient-stat accumulator can't
  overflow at these ranges/windows, so checked is free insurance and
  transactional.
- **Histogram → `SaturatingOverflow`:** a full bin should saturate + flag
  quality, never *reject* the sample (a monitor must not silently drop a count).
- **P²:** no policy knob (internal reject-on-overflow).

## Demonstrated demand: continuous spectral tachometer → Goertzel

The app-side spectral tacho (peak-pick over ratch22's windowed Welch spectrum,
which the crate correctly does NOT own) is **unreliable on the raw
commutation-interval series**: reliable mean-derived e-rate 904/1353/1651 Hz vs
spectral peak-pick +57% / +102% / +1% at 20/35/50% — it only matched when the
sector ripple was strong. Root cause is the exact thing catch24 reported: the
signal is drift-dominated (f1ecac ~110, low-freq power ~0.96), so a search-based
peak-pick loses the weak fundamental. The reliable tach here is just the mean.

A robust CONTINUOUS spectral tach therefore wants a Goertzel evaluated AT the
known electrical rate (cadence/6) — evaluate the spectrum at a target frequency
rather than search for a peak — which is exactly the **Goertzel selected-
frequency detector on ratch22's optional-branch list**. This is the
demonstrated-demand evidence for pulling it forward (or confirming a
time-uniform current/BEMF capture + detrend is the app's job first).

## Multi-window `WindowPlan` (PR#18/#22) — consumer note

Exercised on a real engage capture: `pre / post / full` catch24 in one
allocation-free plan, one shared workspace. POSITIVE: `WindowPlan::try_new` +
`.and()` chaining compiled first try; mixed `Range`+`Suffix` selection is
ergonomic; and `requirements()` reports the exact footprint (windows,
sample-visits, workspace/output/config bytes) *before* running — precisely what
the on-chip escalation rung needs to budget an on-demand deep-dive.
`WindowOutOfBounds` / `TooShort` / `TooLong` separation is clear.

FRICTION: the output is a nested tuple `(((a,b),c)…)` that grows with each
`.and()`, so a K-window *tiling* (a settle-curve or changepoint sweep over many
uniform windows) is awkward to consume — you can't index or iterate it. A
same-profile "evaluate over N uniform windows → `[Output; N]`" helper would fit
the localization/tiling use case (worked around it here by rebuilding a
two-window plan per candidate split in a loop). For a fixed small K
(pre/post/full) the tuple is fine.

## Measured cost (L431, 80 MHz, main context, per commutation)

- Full tier WITH analytics: DWT floor **~1291 cyc** (≈16 µs), vs ~798 cyc for
  the shape-only full tier → analytics ≈ **+500 cyc**. Deep-debug tier; lite
  stays ~120 cyc. Bench: 20/35/50% throttle, histogram resolves the current
  distribution, epoch tracks throttle changes, P² tracks (p50 39→68 raw).
