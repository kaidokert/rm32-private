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

## Host support (STEP 2 — BUILT)

Added to `scope_common.py`: `Capture.interleaved` (set when the header says
`interleaved`), `frame_is_valley()` (per-sector driven-high classification),
`deinterleave()` (→ clean 20 kHz valley + peak sub-captures), and
`render_envelope_figure()` / `plot_envelope_snapshot()` (the per-channel envelope
diagram). New `scripts/scope_pv.py` is the dual-aware analyzer/validator.

## Validation outcome (step 2) — valley validated, peak does NOT add a BEMF view

Bench capture `logs/dual.log` (300 Hz / 15%, locked): trigger/rate/alternation all
correct (268 = 2×134 frames; MMS=update; ~50/50 V/P; driven-pair neutral ≈ Vbus/2
on valley, ≈ 0 on peak). Then `scope_pv.py` tested the actual payoff:

- ✅ **Valley stream validated.** Valley-only float-window fit matches the direct
  in-window ZC to **median 1.5%** (~0.9° elec). scope2's valley path == scope1
  quality; `deinterleave(cap)[0]` is a clean 20 kHz scope1-equivalent that feeds the
  *entire* existing toolchain unchanged. So scope2 is a safe superset of scope1.
- ❌ **Peak stream is NOT a clean second BEMF on this topology.** The optimistic
  "neutral is free / pool for free / 2× density" premise (and the comparator
  cross-check) **failed in data**:
  - peak BEMF reads only **~0.36×** the valley amplitude (robust p10–p90, all three
    phases) with a spurious positive offset — forcing both driven terminals to GND
    in the OFF window destroys the star reference the valley's {Vbus, GND} pair
    provides, and the OFF window sits in post-commutation demag/freewheel;
  - the peak **clamp-edge sits ~75%** of the window regardless of the true ZC (which
    swings 34–97% with load angle) → not a usable ZC indicator;
  - **naive dual-pooling degrades the fit to ~9%** (vs valley-only 1.5%).
  So wins #1 (2× density) and #3 (comparator cross-check) do **not** materialize.

- ❌ **Win #2 (low-duty rescue) FALSIFIED** by a targeted probe (100 Hz, amp swept
  15%→4% into the valley-pinch zone; `logs/sweep_20260621_182510`, `rescue_test_100hz.png`).
  As duty falls the peak/valley ratio *falls* (0.27 → ~0.08), the **opposite** of a
  rescue; in the actual pinch zone (4–5%, where valley spread jumps to 22–55% and the
  rotor goes uncertain) the peak is either negligible (~0.03) or pure noise (>50).
  The peak never becomes usable at any duty. Bonus finding: the **valley is more robust
  than the premise assumed** — it stays well-sensed (spread ~2%, locked) down to ~5–6%,
  so the "pinch zone needing rescue" is tiny and the peak fails there too. Pooling peak
  always degrades the fit (valley-only median 4.0% vs +peak 6.1%, frequent blowups).

**Net (all three wins falsified):** the OFF-window peak scan adds nothing on this
topology — forcing both driven terminals to GND destroys the star reference the
valley's {Vbus,GND} pair provides. The firmware + infra are sound and backward-
compatible (valley sub-stream == scope1), and the envelope figure is a useful
diagnostic, but **for observation use `scope1`** (valley-only, half the data, same
BEMF). `scope2` is the diagnostic/experimental path. Book closed on dual sampling
as a BEMF-observation improvement.

⚠️ Do NOT feed raw interleaved captures to `zc_validate.py` / `zc_map.py` / `zc_fit.py`
(the alternating frames zigzag) — `deinterleave(cap)[0]` first, then analyze the valley
sub-stream as usual.

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
