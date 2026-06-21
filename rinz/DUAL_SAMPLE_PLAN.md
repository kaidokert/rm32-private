# Dual peak+valley BEMF sampling — implementation plan

Status: firmware implemented as `examples/scope2.rs` (copy of `scope1.rs` + the
changes below). Host analysis support is **step 2** (specified here, not yet built).

## Why

`BEMF_ZC_DETECTOR.md` "Honest limits" #3: *resolution falls with RPM —
frames/sector = sample_hz/(hz·6) → ~4 at 800 Hz; pushing higher needs more
samples per PWM period (a firmware change).* This is that change, and it's the
cheapest form of it: sample the BEMF **twice per PWM period** instead of once,
at the two duty-independent instants the center-aligned timer already gives us.

Three gains (from the design review):

1. **2× temporal resolution → pushes the clean/validated band from ~800 Hz toward
   ~1600 Hz.** The harmonic fit gets ~1.5–2× the points (the peak's clamped
   negative half is censored, so not a full 2×) exactly where it's sample-starved.
2. **Duty-robust observation — likely rescues the "uncertain" low-amp zone.** The
   tight-window problem *swaps* with duty: at low duty the valley (ON window) is
   pinched — today's `plateau_spread` 25–30% "uncertain" regime — but the OFF
   window is wide, so the **peak** sample is clean there. At high duty it's the
   reverse. At any duty at least one sample sits in a comfortable window.
3. **Built-in two-level self-cal + two ZC methods.** Valley neutral ≈ Vbus/2,
   peak neutral ≈ GND — two known references read on the BEMF channels every
   frame. Valley = full bipolar crossing (what we fit); peak = clamp-edge /
   comparator-style crossing (the classic AM32 view). They cross-check each other.

This is purely an **observation** upgrade. Nothing here closes a loop.

## Mechanism

Center-aligned PWM counts 0→ARR→0, so the **valley (CNT=0) and peak (CNT=ARR) are
fixed timer instants that occur regardless of duty** — duty only moves the CCR
edges (pulse width) between them. In center-aligned mode with **RCR=0 the update
event fires at BOTH underflow (valley) and overflow (peak)** → route update→TRGO
and the ADC scans twice per period (40 kHz), self-synchronised to the two anchors.

| | Valley (CNT=0, ON window) | Peak (CNT=ARR, OFF window) |
|---|---|---|
| Driven pair | {Vbus, GND} | both GND |
| Virtual neutral | ≈ Vbus/2 (~835 cts) | ≈ 0 |
| Float read | full bipolar BEMF | BEMF vs GND, **positive half only** (neg clamps at ADC floor) |
| Comfortable at | high duty | low duty |

The two are exactly half a period (25 µs) apart, so the interleaved stream is a
**uniform 40 kHz** stream — the `θ = i·Δt` angle map falls out at the doubled rate.

## Chosen path: Path A (update-triggered, single DMA)

Switch the existing TIM1_TRGO source from CC4 (valley only) to **update** (peak +
valley). Reuses *all* of scope1's DMA / double-buffer / capture plumbing — the
only firmware deltas are the trigger source, the rate constants, and the dump
header marker. One DMA, zero new ISR.

Rejected **Path B** (regular@valley DMA + injected@peak via JEOS ISR): it allows
independent sample times for the two windows, but adds an ISR and a second buffer
path for a benefit (long peak sample time) we don't need in this rig's duty range
(≤ ~30% actual duty → both windows are roomy). Revisit B only if we ever push duty
past ~85%, where the peak OFF-window shrinks below the 3-ch scan time.

**Peak vs valley are NOT tagged in the stream** (the DMA can't know which UEV it's
on, and capture start is phased by TIM7 commutation, not TIM1). The host tells
them apart by content — a valley frame has one driven phase ≈ Vbus; a peak frame
has both driven phases ≈ GND. This is robust and self-correcting against any
parity slip. (See "Host" below — the existing driven-pair neutral already produces
the correct neutral for each frame automatically because it reads actual driven
values.)

## Firmware changes (`scope2.rs` vs `scope1.rs`)

1. **Trigger.** Replace `configure_adc_valley_trgo()` with
   `configure_adc_peakvalley_trgo()`:
   - `TIM1.RCR = 0` (forced explicitly — don't depend on what the HAL left there;
     guarantees 2 UEV/period). Harmless to the PWM: ARR is constant and CCR is
     updated slowly from the TIM7 ISR, so reloading preload twice/period is fine.
   - `CR2.MMS = 0b010` (update → TRGO) instead of `0b111` (OC4REF). Drop the CC4 /
     CCMR2 OC4 setup — unused now.
2. **Rate.** `ADC_SAMPLES_PER_PWM: 1 → 2` ⇒ `ADC_FRAME_HZ = 40_000`.
3. **Buffer / SRAM (unchanged size).** Raise `CAPTURE_MIN_HZ: 60 → 120` so
   `ADC_FRAME_COUNT = 40000·2/120 + 1 = 667` ≈ scope1's 668 → **identical SRAM**
   (~18.7 KB of ADC buffers). Consequence: 2 full revs are captured down to
   ~120 Hz; below that the capture is buffer-bound (<2 revs — fine, dual-rate is
   about high RPM, and low-RPM already has abundant samples).
4. **Capture sizing (frame-first, overflow-safe).** Replace
   `two_rev_drive_tick_count`/`two_rev_adc_frame_count` with:
   - `capture_tick_count(hz)` = min(2-revs-of-ticks, `ADC_FRAME_COUNT/ADC_SAMPLES_PER_PWM`)
   - `capture_frame_count(ticks)` = `ticks · ADC_SAMPLES_PER_PWM`
   TIM1 (UEV @ 40 kHz) and TIM7 (@ 20 kHz) are both integer divides of the same
   170 MHz clock → exactly 2:1 locked, so `frames = ticks·2` is exact and can't
   overrun the buffer.
5. **Dump header marker.** Keep the `dump7:` / `cdump:` prefix and the `12-bit` /
   `b85` keywords (so capture/stream plumbing and `split_complete_dumps` keep
   working), bump the Hz to 40000, and add the literal token
   `interleaved peak+valley` so the host can branch on it.

Everything else (commands, watchdog, streaming, double-buffer flip, ADC1
current+VBUS ring, b85 codec) is unchanged. `scope1.rs` stays as the valley-only
baseline for A/B comparison (flash one or the other).

## Host changes (STEP 2 — specified, not yet built)

⚠️ **Until this is done, do NOT run `zc_validate.py` / `zc_map.py` / `zc_fit.py`
on scope2 captures** — they'd pool the clamped peak frames into the fit and bias
it toward zero. scope2 data is raw-inspectable (`d` dump) but not yet analysis-ready.

The good news (from the review): the host change is *small*, because our existing
machinery already does most of it:

- **Parser:** in `scope_common.parse_capture`, detect `interleaved` in the header
  and set a `Capture.interleaved` flag (additive, default False).
- **Neutral is free.** `_driven_pair_neutral` computes `(driven_hi+driven_lo)/2`
  from the *actual* driven-channel values per frame. On a valley frame that's
  ≈ Vbus/2; on a peak frame both driven channels read ≈ GND so it's ≈ 0 —
  **automatically correct per frame, no peak/valley special-casing.**
- **Angle is free.** Treat the capture as a uniform 40 kHz stream; `fps =
  sample_hz/(hz·6)` and `θ = i/fpr·2π` already double correctly.
- **The one real change — drop clamped points before fitting.** After computing
  `e = v_float − neutral` per frame, discard samples where the raw float channel
  is near the ADC floor (e.g. `< ~10–20` counts, the clamped peak negative half).
  Feed the rest (all valley points + the unclamped positive peak points) into the
  same linear `fit_harmonics`. This is plain truncation, not Tobit — the censored
  points carry no info the valley fit lacks.
- **Optional:** a `scope_pv.py` that splits a capture into valley-only and
  peak-only substreams (by driven-channel level) for side-by-side plots and the
  two-method ZC cross-check (bipolar fit vs clamp-edge).

## Validation plan (on the bench)

1. **Trigger sanity (raw eyeball).** Flash scope2, `d` dump at e.g. 300 Hz / 15%.
   Confirm the header reads `... interleaved peak+valley, 40000 Hz` and that
   frames **alternate** between a valley signature (one driven phase high ≈ Vbus,
   ~1650 cts) and a peak signature (both driven phases ≈ GND). If frames don't
   alternate, RCR≠0 took effect / update isn't firing twice — check `regs:` for
   `t1_cr2` MMS and that the frame count matches 40 kHz (≈2× scope1 for the same
   freq/revs).
2. **Frame-rate check.** At a fixed freq, scope2 should report ≈2× the frames of
   scope1 for the same capture (capped by buffer below 120 Hz).
3. **Host split + re-validate (after step 2 host work).** Run the dual-aware
   `zc_validate` and confirm: (a) residual vs in-window ground truth stays tight,
   (b) samples/sector ≈ doubled, (c) in-window ZC now appears in the low-duty
   captures that were "uncertain" before (the peak rescue), (d) the clean band
   extends past 800 Hz toward ~1600 Hz.

## Risks & mitigations

- **HAL may set RCR≠0** → forced `RCR=0` in `configure_adc_peakvalley_trgo`;
  step-1 validation (frames must alternate / 40 kHz) catches it if wrong.
- **High-duty peak sliver** (OFF window < scan time above ~85% duty) → out of this
  rig's range (≤ ~30%); Path B is the escape hatch if ever needed.
- **Unknown peak/valley parity** → host self-identifies by driven-channel level;
  no reliance on frame index parity.
- **Old analysis scripts mis-handle interleaved data** → the `interleaved` header
  token + the warning above; scripts gate on `Capture.interleaved` once updated.
