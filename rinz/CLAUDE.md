# Claude Code working notes — rinz / B-G431B-ESC1 motor tester

## TRANSITION TO GENTLE CLOSED-LOOP (GATED & PASSED - June 2026)

The original hard constraint blocking closed-loop control has been successfully resolved. The prerequisite of a trustworthy open-loop observation has been met and verified:
* **Validated Results:** The multi-harmonic observer ($N=3$) matches direct in-window zero-crossing ground truth to within $\approx 4^\circ$ median error with 0 outliers $>20\%$ window across the operating plane.
* **Phase-A Anomaly Retired:** High-resolution sweep diagnostics (`scripts/zc_phase_a.py`) proved Phase A is physically matched to B/C (plateaus agree to 1.2%) and its low float amplitude is a dynamic load-angle effect, not a sense-path gain error.
* **Dual-Sampling Characterized:** Confirmed that peak-sampling collapses/distorts BEMF under low-side GND recirculation, locking in valley-only sampling (`scope1`) as our gold standard.

### Guardrails for Closed-Loop Transition:
1. **Preserve the Harness:** Do not modify `scope1.rs`. Create a new example file (e.g., `examples/scope_cl.rs`) that preserves all diagnostic logging, serial streaming, and host analysis tools.
2. **Observe-Only First (Stage 1):** The real-time detector runs in the background but *does not* control commutation. Commutation stays on the open-loop schedule.
3. **Oracle Agreement:** The real-time detector's ZCs must match the offline multi-harmonic fit (the oracle) under open-loop sweeps before any feedback is engaged.
4. **Bounded Authority & Fallbacks:** When closed-loop is engaged (Stage 2), the detector may only steer commutation timing within a narrow, slew-limited window around the open-loop schedule, with instant fallback to open-loop.

---

## What this crate is

`rinz` is a **standalone motor-tester firmware** for the ST B-G431B-ESC1 evaluation
board (STM32G431CB, 170 MHz Cortex-M4F, 32 KB SRAM, 128 KB flash). It is not part of
the main rm32/rm32_stm32 ESC project. Its sole purpose is to spin a 3-phase BLDC motor
in open-loop six-step mode while capturing and displaying BEMF waveforms.

The entire application lives in `examples/motor_tester.rs`.

---

## Hardware

| Item | Detail |
|------|--------|
| Board | ST B-G431B-ESC1 |
| MCU | STM32G431CB, 170 MHz, 32 KB SRAM |
| Motor phases | Phase A = TIM1 CH1/CH1N: PA8/PC13 (AF6/AF4) |
|              | Phase B = TIM1 CH2/CH2N: PA9/PA12 (AF6/AF6) |
|              | Phase C = TIM1 CH3/CH3N: PA10/PB15 (AF6/AF4) |
| BEMF inputs  | A = ADC2 ch17 / PA4; B = ADC2 ch5 / PC4; C = ADC2 ch14 / PB11 |
| BEMF enable  | PB5 driven LOW → enables resistor-divider network |
| Serial       | USART2 PA3=RX PA2=TX — NOT used. USART2 PB3=TX PB4=RX at 115200 8N1 is the actual terminal |
| Probe        | ST-LINK on-board (USB) |

## Build / flash

```
cd rinz
cargo run --example motor_tester --release
```

The `.cargo/config.toml` runner flashes via `probe-rs run` targeting STM32G431CBUx.

---

## ADC setup — critical numbers

| Parameter | Value | How set |
|-----------|-------|---------|
| ADC clock | **42.5 MHz** (HCLK/4) | `ClockMode::AdcHclkDiv4` in HAL `adc12_common.claim()` |
| ADC clock mode | Synchronous HCLK/4 | CKMODE=0b11 in ADC12_COMMON.CCR |
| Sample time | **6.5 cyc** (SMPR value 1) | Constant `ADC_BEMF_SAMP = SampleTime::Cycles_6_5` in `start_adc2_scan_dma` |
| Conversion total | 6.5 + 12.5 = **19 cyc / channel** = **447 ns/ch** | |
| 3-ch scan time | 3 × 447 = **1341 ns** | |
| 3rd ch sample start | 2 × 447 = **894 ns** after TRGO | |
| Trigger | TIM1_TRGO via CC4 (OC4REF, CCR4=1, PWM mode 1) | `configure_adc_capture(true)` |
| ADC scan order | **[ch17/A, ch14/C, ch5/B]** — A sampled first | `start_adc2_scan_dma`, `configure_adc_capture` |
| DMA layout | `ADC2_DMA_BUF[0]`=A, `[1]`=C, `[2]`=B | Follows scan order |

**PHASE_TO_DMA = [0, 2, 1]** — phase_idx (0=A, 1=B, 2=C) → DMA buffer slot.

Note: the design doc `BEMF_50_DESIGN_v2.md` specifies `PHASE_TO_DMA = [2, 0, 1]`
with scan order [B,C,A]. **This was changed in implementation** — see WIP doc.

### Amp% vs PWM duty

The `amp` knob (displayed as e.g. `amp=12%`) is NOT the same as PWM duty cycle.
Six-step CCR is computed as:

```rust
let duty = arr * amplitude * 2 / (100 * 3);
// arr = 4250 (ARR for 20 kHz center-aligned at 170 MHz)
// amp=12% → CCR = 340 → actual duty = 340/4250 = 8%
```

Conversion: `actual_duty% ≈ amp% × 2/3`.

This matters for ADC timing: the half-ON-window for TRGO center-aligned is
`CCR / 170 MHz = arr × amp × 2 / 300 / 170 MHz` nanoseconds.

At amp=8%: half-window = 1.33 µs. With 6.5-cycle sample time, 3rd channel sample
ends at 894 + 153 = 1047 ns < 1330 ns — fits.

### The scan-timing stagger (why A was sampled first)

The ADC scan fires sequentially after TRGO. Each channel takes 447 ns. At low
amplitude, the 3rd channel's sample phase can extend past the PWM ON-window, causing
it to read the OFF-state voltage (near zero for a hi-driven phase) rather than the
driven voltage.

Evidence (obs_test2, obs_test3): with old scan order [B,C,A], at amp=12%:
- B (1st): `0x6a` ✓
- A (3rd): `0x44–0x53` (reads ~65% of expected — outside ON-window)

After swap to [A,C,B]:
- A (1st): `0x6a` ✓
- B (3rd): `0x4c–0x51` (same effect, different channel)

Root cause confirmed: the 3rd channel at amp=12% (actual 8% duty, half-window=2.0µs)
with old SMPR=3 (24.5 cyc, 871ns/ch) placed the 3rd sample end at 2.318µs > 2.0µs.

Fix: `SampleTime::Cycles_6_5` (19 cyc, 447 ns/ch) → 3rd sample ends at 1.047µs < 1.33µs ✓.

---

## Ring / sampling architecture

| Layer | Rate | Description |
|-------|------|-------------|
| TIM7 ISR | 96 kHz | Writes one ADC sample per tick into `RAW_SAMPLE_BUF` + `RAW_NEUT_BUF`. Also advances 6-step commutation. |
| TIM1 | 20 kHz | Center-aligned complementary PWM. TRGO triggers ADC scan. |
| ADC2 | 20 kHz | DMA circular, 3-channel scan per TRGO. Writes `ADC2_DMA_BUF[3]`. TIM7 reads it each tick. |
| Ring sampling | 96 kHz | TIM7 reads `ADC2_DMA_BUF[PHASE_TO_DMA[chan]]` and writes to ring (~4.8 repeats per unique ADC value). |

`RAW_SAMPLE_BUF`: BEMF ring for selected `RING_CHAN_FIXED` phase.
`RAW_NEUT_BUF`: computed `(V_hi + V_lo)/2` per TIM7 tick (triggered mode only; 0 in async).

---

## Key constants in motor_tester.rs

```rust
const SIX_STEP_HIGH: [usize; 6] = [0, 0, 1, 1, 2, 2]; // phase idx driven high per sector
const SIX_STEP_LOW:  [usize; 6] = [1, 2, 2, 0, 0, 1]; // phase idx driven low per sector
const PHASE_TO_DMA:  [usize; 3] = [0, 2, 1];           // A→buf[0], B→buf[2], C→buf[1]
const RAMP_FALLING:  [bool; 6]  = [false, true, false, false, false, false]; // current state — under investigation, see PHASE_MAPPING.md
const ZC_BLANK:   usize = 2;   // ISR: skip first N triggered samples (~100 µs)
const ZC_CONFIRM: usize = 2;   // consecutive samples required to confirm ZC (ISR and 'e' matched)
const ADC_BEMF_SAMP: SampleTime = SampleTime::Cycles_6_5; // in start_adc2_scan_dma
```

### ZC detection — ISR vs 'e' command (as of June 2026)

Both now use the **same definition** of in-window ZC:
- `v_fl != 0` and `vn != 0`
- `ZC_CONFIRM = 2` consecutive samples satisfy the crossing condition
- Early (ZC<0): condition already met at samples [BLANK] **and** [BLANK+1] → suppressed, does not count toward lock or PLL
- In-window: condition first satisfied at sample > BLANK+1

The lock counter (`LOCK_COUNTS`, reported as `lock sN=` in `i` output) is driven by
`ZC_IN_WINDOW_S` — set only for genuine in-window crossings. Early ZCs do **not**
increment the lock counter. This makes `i` and `e` semantically consistent.

Three ISR statics per sector: `ZC_DETECTED_S` (any crossing seen, suppresses re-fire),
`ZC_IN_WINDOW_S` (genuine in-window, drives lock), `ZC_PREV_CROSSED_S` (previous
sample state, implements the 2-consecutive requirement).

---

## Active investigation (June 2026) — finding the missing ZC sectors

**Current state**: 2 out of 6 sectors show genuine in-window BEMF zero crossings:
- **s0** (A=hi, B=lo, C=float): C rises through neutral, ZC@~5/18. ✓
- **s4** (C=hi, A=lo, B=float): B rises through neutral, ZC@~10/18. ✓

The other 4 sectors all show ZC<0 (float phase enters the sector already on the crossed
side of neutral) or ZC>n (never crosses). See `PHASE_MAPPING.md` for the full
investigation log and the commutation-direction hypothesis.

**Next step**: try reversing the drive commutation direction (swap phase indices 1↔2 in
`SIX_STEP_HIGH`/`SIX_STEP_LOW`). This is a drive-only change; `PHASE_TO_DMA` stays
`[0,2,1]`.

---

## scope_l — peak-sampling BEMF capture (June 2026)

`examples/scope_l.rs` is a new capture firmware focused on clean BEMF waveform
observation. It differs from `scope8.rs` in one key way: it samples **at the PWM peak
(CNT=ARR)** rather than during the ON window.

### Why peak sampling

In center-aligned PWM mode 1, at CNT=ARR (peak):
- HIGH phase: high-side FET OFF, low-side FET ON → phase pulled to GND via low-side
- LOW  phase: low-side FET ON → GND
- FLOAT phase: both FETs OFF → reads pure BEMF with no switching noise

With both driven phases at GND, virtual neutral = GND by construction. Zero crossing
(e_C = 0) occurs at V_FLOAT = 0. No V_hi/V_lo estimation needed.

Clamping: negative BEMF reads as 0 (ADC hardware clamp). Only positive half visible.
ZC is detectable as the edge where the float phase transitions from/to 0.

### Trigger mechanism

`configure_adc_peak_trgo()` sets CCR4 = ARR-1 with PWM mode 1. OC4REF goes
LOW→HIGH on the downcount at CNT=ARR-1 (one count past the peak). This rising edge
feeds TIM1_TRGO → ADC2 EXTSEL. No TIM3 needed. ADC_FRAME_HZ = PWM_HZ = 20 kHz.

### 12-bit ADC (implemented, not yet captured)

scope_l uses `Resolution::Twelve` (0–4095), `u16` DMA buffer, 4-digit hex dump.
Previous 8-bit captures showed only 0–10 counts at the float phase peak — 16× too
coarse to see fine ZC structure. 12-bit gives 0–160 count range for the same signal.

**Dump format**: `dump3: N frames x 3 channels (ch17 ch5 ch14, 12-bit ADC, 20000 Hz)`
followed by 4-digit hex triples, e.g. `00a0 00b1 00c2`. `scope_common.py` detects
"12-bit" in the header and uses a `{4}`-char hex regex; 8-bit captures ("8-bit ADC")
still use the `{2}` regex. `Capture.full_scale` = 4095 or 255 accordingly.

**12-bit capture confirmed (June 2026)** — but via `scope1.rs` (valley sampling), not
scope_l. See the scope1 section below.

### ON-time sampling — next investigation

An LLM survey (5 models, with topology clarification addenda) converged 4/5 to the same
conclusion: for this topology (complementary center-aligned PWM, all phases resistor-
divided to ADC, 90–95% duty target), **sample at the counter valley (CNT=0)**, not the
peak. Reasoning:

- With complementary PWM the virtual neutral is stable in BOTH windows (no body-diode
  collapse as in asymmetric chopping). ON vs OFF choice is purely geometric.
- At 95% duty: ON window = 47.5 µs, OFF window = 2.5 µs split into two 1.25 µs
  slivers. The peak (CNT=ARR) sits in the middle of the OFF sliver; the valley (CNT=0)
  sits in the middle of the ON window.
- Valley sampling gives the **full bipolar BEMF** signal riding around Vbus/2.
  Peak sampling gives only the positive half (negative half clamps at ADC floor = 0).
  Zero crossing with valley sampling is a real bipolar crossing through Vbus/2; with
  peak sampling it is just the signal approaching zero.
- For ZC detection and future commutation timing, the full bipolar signal at valley is
  more useful.

Survey files: `notes/bemf_sampling_survey/` (original) and `addendum/` (after topology
clarification). Synthesis in each directory.

**Done — implemented as `scope1.rs`** (see next section). scope_l (peak sampling)
remains available as a diagnostic but valley sampling is the path forward.

---

## scope1 — valley-sampling 12-bit BEMF capture (June 2026, CURRENT)

`examples/scope1.rs` is the survey-recommended configuration: **one 12-bit 3-channel
scan per PWM period, triggered at the counter valley (CNT=0) — the exact middle of the
ON window**. CCR4=1 with PWM mode 1 in center-aligned mode pulses OC4REF at the valley;
CR2.MMS=0b111 routes OC4REF → TRGO → ADC2 EXTSEL. No TIM3.

### First good capture (window=2, 180 Hz, 223 frames = 2 elec revs)

`logs/latest_snapshot.png`. Each phase shows the textbook six-step terminal-voltage
staircase sampled mid-ON-window:

| Segment | Counts | Meaning |
|---|---|---|
| High plateau | ~1660–1680 | 2 sectors driven HIGH → Vbus via divider (1670/4095 ≈ 1.35 V at pin) |
| Low plateau | ~30–100 | 2 sectors driven LOW → GND |
| Mid-level ramps | ~550–1080 | 2 float sectors — bipolar BEMF riding around Vbus/2 ≈ 835 counts |

Timing: ~18.6 frames/sector at 180 Hz. BEMF excursion in the float windows is only
~±200 counts about neutral — this is why 12-bit was required (would be ~12 counts
in 8-bit).

**Filtering caveat**: snapshot moving-average window must be << sector length
(18 frames). `--snapshot-window 31` smears the staircase into fake sinusoidal humps —
that artifact was initially misread as a peak-sampling capture. Use window 2–5.
(`--lowpass-window` is a dead arg in scope_live_ui.py — parsed, never used;
`--snapshot-window` is the real one.)

### Observations to chase

- Float-window levels are asymmetric about Vbus/2 (phase A floats at ~900 and
  ~1010–1080, both above 835). Same "enters sector already crossed" signature as the
  motor_tester ZC investigation, now directly visible in voltage.
- Post-commutation demagnetization step visible (e.g. B: 1660 → 550 → settles 640).
  ZC logic must blank this.
- Virtual neutral for ZC should be computed as (Va+Vb+Vc)/3 per frame — all three
  phases are sampled in the same scan.

### scope1 serial commands (current)

Drive: `f`/`v` freq ±10 Hz, `g`/`b` freq ±1 Hz, `a`/`z` amp ±1.0 %, `+`/`-` amp
±0.1 %, `]`/`[` duty trim ±1 raw CCR count (signed `DUTY_TRIM`, added to the
computed six-step duty; finer than `+`/`-` which step ~2.8 counts), `w` kill,
`q` reset (also zeroes the trim). Capture: `d` single dump, `l`/`k` start/stop
continuous streaming. The debug line carries `hz= amp= trim=`; analysis scripts
read them via `capture.debug`. (`amp` is 0.1 % units; the true operating duty is
`amp`-derived base + `trim`, so log both.)

Host `scope_live_ui.py` mirrors all of these (`G/B`, `[`/`]` forwarded to the
firmware; `state.trim` tracked from the `trim=` echo) and adds `s` =
**log exploration point**: appends a record to `logs/exploration.json`
(`--exploration-path`) with ts, hz, amp_tenths/amp_pct, trim, frames, sample_hz,
zc_window, and the full per-sector ZC analysis (phase, status, zc_pct, zc_frame,
direction, d_start, d_end) plus raw `debug`/`regs`. Works after a single `d` or
mid-stream (uses `state.last_capture`).

### Rotor lock/stall detection (host, ZC-free)

`scope_common.classify_rotor_state(capture)` decides **locked / stalled /
uncertain WITHOUT needing any in-window zero crossing**. Two stages:

1. **Sensing gate (`plateau_spread`).** The three driven-high plateaus (92nd pct
   per channel) agree to <1% when the BEMF divider settles (duty high enough) but
   diverge to 25-30% at low duty where the ON pulse is too brief to settle — then
   the float reads and the (A+B+C)/3 neutral are not trustworthy. `spread >
   plateau_spread_max` (5%) → **`uncertain`**. This stops false "locked" calls in
   the <~6% duty range.
2. In the trustworthy regime, classify on **`late_swing`**: line-fit excursion of
   `float-neutral` over the LAST ~45% of each window (demag excluded), median over
   B,C (A skipped for its sense anomaly). Rotation keeps ramping to the window end;
   a frozen rotor goes flat once demag decays. `late_swing >= lock_swing` (30) →
   `locked`, else `stalled`.

**Hard-won calibration history (don't repeat the mistake):** the original version
classified on the drive-frequency sinusoid amplitude `R`. That **fails at low
duty** — on real ground-truth captures (cap076-079) a 4.3% *stalled* rotor read
`R=83` > a clean 8% *spinning* `R=57`, because energized-stall static offsets +
demag project onto the fundamental, and the divider doesn't settle. Six metrics
(R, swing, late_swing-on-(float-neutral), rev-to-rev RMS, raw-float late slope,
window-mean spread) were tested against labeled captures and **none separate
barely-spinning from stalled at 4.3-4.4%** — that regime is a genuine sensing +
mechanics limit, hence `uncertain`. `bemf_amp` (R) is kept as an advisory output
column only, NOT used to classify.

**Validated**: LOCKED (clean spin 82-359) and UNCERTAIN (the 4.3/4.4% captures,
spread 26-31%). **Provisional**: the STALLED `late_swing<30` cutoff — no clean
>6%-duty stalled capture yet; grab one (hold the rotor at ~8-10%, hit `d`) to nail
it. Calibration tool: `scripts/zc_rotor_calib.py label:rawlog ...`.

Shown in the rendered view: `render_zc_figure` suptitle reads
`rotor=LOCKED/STALLED/UNCERTAIN (late_swing= plateau_spread=% sensing=)`,
color-coded green/red/gray. The blessed UI Drive line shows the same; each `s`
exploration record embeds the full `rotor` dict.

### Automated (freq, amp) sweep (June 2026)

Two-pass characterization of the lock landscape (`scripts/scope_sweep.py` collector
+ `scripts/sweep_map.py` offline analysis). Brute max-res over the whole plane is
~8-12 h; the smart two-pass is <1 h: coarse (1% amp, 2 snaps, ~9 min) maps the
landscape, then re-run fine (0.1% amp, 5 snaps, `--freq-jitter 1`) **only** on the
transition bands `sweep_map` flags.

`scope_sweep.py` (standalone, owns serial): per frequency, `q`-reset → co-ramp
freq+amp up tracking just above the measured stall curve `0.035·hz+2.6` (keeps
current low in transit) → up to a frequency-dependent ceiling (default 25% @100 Hz
→ 30% @500 Hz, sized to keep supply current under the ~1.4 A/100 Hz limit) → sweep
amp DOWN to `stall-margin`. Self-bounds with the stall curve (no live stall
detector — we don't have a reliable one). Reads firmware `amp=`/`freq=` echoes for
setpoint tracking; **every dump is labeled by its own debug line**, so imperfect
ramps/desync don't corrupt the data (the offline grid uses actual reported hz/amp,
not the intended setpoint). Raw hex to `logs/sweep_<ts>/f<hz>.log` + `manifest.csv`;
NO images during capture.

`sweep_map.py` (offline): per-capture metrics (late_swing, bemf_amp, in-window ZC
count, sensing plateau_spread) → 6-panel (freq, amp) heatmaps. Crucially it also
maps the **snapshot-to-snapshot spread** of late_swing/bemf_amp — that's the
marginal/bistable zone ("a tiny amp change flips lock") quantified as variance
across repeats at the *same* setpoint. Flags top-N interesting points (high snap
spread + sharp amp-gradient) to `interesting.csv`; `--render` batch-generates ZC
images for just those. Does NOT assert binary lock (unreliable); maps the
observables and lets the structure show itself.

NOTE: `scope_sweep.py` is logic-validated (planning math, ramp arithmetic) but the
serial timing/echo handling is UNTESTED against live firmware — shake it out on a
short `--freq 180 --amp-max 14 --amp-min 8` run before a full sweep.

### ZC-vs-window investigation status (June 2026)

Tooling: `zc_chase.py` (per-hz amp search with repeats + fine sweep, `--dir`,
`--revs`), `zc_sector_stats.py` (per-sector-type offsets with linear
extrapolation from d_start/d_end; `--per-rev` drift matrix).

Findings so far, all at 120/180 Hz open loop:
1. **No amp equilibrium exists** — pooled mean ZC is bimodal across sector
   types; amp slides the whole pattern, can't compress it.
2. **Drive reversal helped materially** (best 18/36 in-window vs 10/36 fwd;
   3 consecutive good sector types) but did NOT collapse the spread → not a
   pure sequence/labeling mismatch.
3. **Dominant residual**: per-sector offsets follow a smooth ±0.8-window
   (≈±48° elec) wave, period exactly 1 electrical rev, repeatable across
   spin-ups, survives drive reversal.
4. **Elliptical-field (fixed drive asymmetry, e.g. PC13 weak CH1N) ruled
   out**: modulation amplitude unchanged from amp 13.7%→20%, and the wave's
   sector-phase flipped ~half a period between operating points.
5. **Rotor hunting also ruled out.** 6-rev capture (chase_20260612_200550)
   `--per-rev`: pattern is locked, not walking — s2 nailed at +0.38..+0.43
   (sd 0.05) across 6 consecutive electrical revs. Rev 0 is a startup transient
   (exclude it). Electrical-angle-locked, not slow oscillation.
6. **Divider mismatch measured and ruled out quantitatively.** Per-phase driven
   rail plateaus: high A=1660/B=1663/C=1645 (spread 1.1%), low A=32/B=87/C=84.
   A sector-dependent neutral bias of 18–55 counts against a BEMF slope of
   ~45 counts/frame → only 2–6% window shift. Order of magnitude too small.

### `zc_fit.py` — honest ZC angle by sinusoid fit (supersedes extrapolation)

The "spread 7.7 / structural divergence" verdict from `zc_sector_stats` was
largely an **artifact**: 4 of 6 sectors never cross in-window, and their offset
was a *linear* extrapolation of a *sine* from >1 window away (s0 read −8 logical
sectors — absurd). `zc_fit.py` instead fits each phase's floating-window samples
to `a·cos+b·sin+c` (note: `v_float − (Va+Vb+Vc)/3 == e_float` exactly, so this
is a direct BEMF fit) and solves for the true crossing angle whether or not it
lands in a window. Validated: agrees with the 2 in-window direct measurements.

**Decomposition of the per-sector "divergence" (the real finding):**
1. **Phase A sense anomaly.** Fitted BEMF amplitude R: A≈half of B,C
   (A 53/108, B 80/143, C 74/154 at 120/180 Hz). A's DC offset c/R = +2.4 at
   120 Hz (exceeds its own amplitude → no crossing). B, C healthy and
   reproducible across runs. A reads wrong ONLY when floating (driven plateaus
   fine), so it is the **sense path** (PA4 / ADC2 ch17), not drive. A floats in
   s2 and s5 — the perennially-pathological sectors. Motor/circuit symmetric as
   expected; phase A sense is the lone exception.
2. **Speed-dependent lag.** Healthy phases' crossings move with frequency
   (C-fall 212°→243°, B-fall 283°→315° over 120→180 Hz) → ordinary load angle,
   not a fixed geometric offset.

**Phase-A anomaly — RESOLVED (June 21 2026), no lead-swap needed.** The
sense-path gain hypothesis is refuted by three data lines (`scripts/zc_phase_a.py`,
1500+ locked mid-band captures; see the "Phase-A anomaly — resolved" section of
`BEMF_ZC_DETECTOR.md`): (1) driven-rail plateaus — same divider as the float read
— match to 1.2% (A=1658/B=1663/C=1643), so the divider gain is not mismatched;
(2) `R_A/R_C` swings 0.87→0.25→back with commanded amp, i.e. it is load-angle
dependent, not the constant a fixed gain would give; (3) the old `c/R ≈ +2.4`
offset was a `(A+B+C)/3` neutral artifact — against the driven-pair neutral the
per-bin median `|c/R| ≤ 0.21`. The residual A behaviour is the amplitude-domain
face of the electrical-angle-locked per-sector wave (A floats in s2/s5). A
host-side `g_A` would mask a real effect, not fix a calibration error. The
detector relies on B/C + the harmonic crossing and is unaffected. Book closed on
the observation phase.

### Live streaming capture (implemented June 2026)

Firmware `l`/`k` (scope1.rs): `l` sets a `STREAMING` flag, `k` clears it. The main
idle loop services **one back-to-back `d`-format dump per pass** while the flag is
set (`run_capture()` — flip front/back at electrical zero, dump the frozen buffer
while ADC→DMA keeps filling the alt buffer and the motor keeps spinning). All other
commands are unchanged: while streaming the loop fetches pending keys non-blocking
(blocking only when idle), so `f/v a/z +/-/q` adjust **live between dumps without
stopping the stream**. `w` clears the flag and kills the motor (must clear it, else
the next `run_capture` re-asserts RUNNING). This keeps command handling in one place
— the only change vs. single-shot is the flag-gated dump service in the idle loop.

Host (recommended, two terminals — keeps input in the snappy blessed UI and the
plot a passive, focus-free display):
- `scripts/scope_live_ui.py` (control): blessed text-UI owns the serial port and
  is the input path (responsive `term.inkey`). `l`/`k` toggle `state.streaming`;
  while streaming, `poll_serial` accumulates UART text, `split_complete_dumps()`
  isolates whole dumps on the `end` terminator, and a throttled (`stream_refresh`,
  ~1 s) background thread regenerates `latest_zc.png` via `plot_zc_snapshot` —
  written to a `.tmp.png` then atomically `replace()`d so a reader never sees a
  half-written file. `f/v a/z +/-` adjust live (sent straight to the firmware,
  applied between dumps); `w` kills + clears streaming; `d` is blocked while
  streaming. Input stays responsive because rendering is off-thread.
- `scripts/scope_view.py` (display only): a passive matplotlib window that reloads
  `latest_zc.png` on mtime change. Reads NO keyboard, never needs focus — keep the
  terminal focused for control.

`split_complete_dumps()` lives in scope_common.py (shared with scope_stream.py).
`render_zc_figure(capture, fig, ...)` was split out of `plot_zc_snapshot` so the
PNG path and any live figure share one renderer.

Standalone alternative `scripts/scope_stream.py`: single all-in-one window that
owns serial, resets+ramps (`--hz/--amp`), streams, renders live via
`render_zc_figure()`, and forwards control keys from the (focused) plot window.
Simpler to launch but input goes through the matplotlib window — laggier than the
blessed UI, which is why the two-terminal split above is preferred.

Throughput today: hex text ≈ 16 B/frame; a 2-rev dump (~333 frames at 120 Hz) is
~0.5 s at 115200 baud → ~2 dumps/s, redrawn at ~1 Hz (intermediate dumps dropped).
A binary/packed mode (3×u16 + sync ≈ 6–8 B/frame) would roughly triple that and
is the next step if higher refresh is wanted.

---

## Notes directory

| File | Content |
|------|---------|
| `notes/BEMF_50_DESIGN_v2.md` | Canonical design spec for virtual-neutral + ZC display |
| `notes/BEMF_50_DESIGN_WIP1.md` | WIP: what's implemented, deviations, current state |
| `notes/DUMP4/5/6_REVIEW*.md` | Per-dump analysis of `e` command output |
| `notes/UNFUCK1/2.md` | ADC/DMA register archaeology notes |
