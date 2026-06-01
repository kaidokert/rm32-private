# Claude Code working notes — rinz / B-G431B-ESC1 motor tester

## HARD CONSTRAINT — READ THIS FIRST

**Do not propose closed-loop control. Do not suggest it. Do not hint at it.**

The entire point of this project is to get open-loop six-step BEMF observation working
correctly and reliably *first*. Until the data coming out of the open-loop system makes
clear physical sense — correct ZC timing, stable neutral, repeatable sector-to-sector
behavior — there is nothing to close a loop *on*. Closing the loop on a broken or
unvalidated observation signal does not fix the signal; it hides the bugs behind
feedback and makes them harder to diagnose.

If a reviewer or agent suggests "move to closed-loop", "use the ZC as a feedback
signal", "implement commutation based on detected ZC", or any variant thereof: **reject
it immediately**. The prerequisite is a trustworthy open-loop observation. We are not
there yet. That is what we are working on.

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
const RAMP_FALLING:  [bool; 6]  = [true, false, true, false, true, false]; // s-indexed
const BLANK:      usize = 5;   // skip first N de-staircased points before ZC scan
const ZC_CONFIRM: usize = 2;   // consecutive samples required to confirm ZC
const ADC_BEMF_SAMP: SampleTime = SampleTime::Cycles_6_5; // in start_adc2_scan_dma
```

---

## Active investigation (May 2026)

We are implementing `BEMF_50_DESIGN_v2.md`. The `e` command shows per-sector BEMF
vs virtual neutral with ZC annotations. See `notes/BEMF_50_DESIGN_WIP1.md` for
implementation state.

### Immediate pending work

1. **SMPR register write cleanup**: currently raw bit manipulation in
   `start_adc2_scan_dma`. Should use PAC typed calls:
   ```rust
   adc2.smpr1().modify(|_, w| w.smp5().cycles6_5());
   adc2.smpr2().modify(|_, w| w.smp14().cycles6_5().smp17().cycles6_5());
   ```

2. **Startup ADC info print**: add after `configure_adc_capture(true)` + TIM1 enable:
   ```rust
   let adc_clk = rcc.clocks.ahb_clk.raw() / 4; // AdcHclkDiv4
   // compute per-channel ns and min_amp, print once
   ```

3. **Update `BEMF_50_DESIGN_v2.md`**: fix PHASE_TO_DMA table and scan order to match
   current implementation [A, C, B].

4. **obs_test4**: flash current firmware (SMPR=Cycles_6_5, scan=[A,C,B]) and verify
   all phases read correctly at amp=8%.

---

## Notes directory

| File | Content |
|------|---------|
| `notes/BEMF_50_DESIGN_v2.md` | Canonical design spec for virtual-neutral + ZC display |
| `notes/BEMF_50_DESIGN_WIP1.md` | WIP: what's implemented, deviations, current state |
| `notes/DUMP4/5/6_REVIEW*.md` | Per-dump analysis of `e` command output |
| `notes/UNFUCK1/2.md` | ADC/DMA register archaeology notes |
