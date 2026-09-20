# X-NUCLEO-IHM08M1 ↔ NUCLEO-G071RB — pin mapping & AM32 fitness

*Verified from the ST schematic v3, page 3 "Figure 6: MCU pinout assignment"
(rendered, not text-extracted — the multi-MCU columns scramble under pdftotext).
2026-09-06.*

## Confirmed pin map (G071 column)

| IHM08M1 signal | G071 pin | function used | AM32 role | status |
|---|---|---|---|---|
| UH / VH / WH (high gate in) | **PA8 / PA9 / PA10** | TIM1_CH1 / CH2 / CH3 (AF2) | motor PWM high | ✅ native |
| UL / VL / WL (low gate in) | **PA7 / PB0 / PB1** | TIM1_CH1N / CH2N / CH3N (AF2) | motor PWM low | ✅ native |
| Curr_fdbk1/2/3 | **PA0 / PC1 / PC0** | ADC_IN | current sense (single-shunt default) | ✅ |
| VBUS | **PA1** | ADC_IN | bus voltage | ✅ |
| Temperature | **PC2** | ADC_IN | temp (optional) | ✅ |
| CPOUT (LMV331 overcurrent comp) | **PA12** | TIM1_ETR / BKIN | overcurrent break | ✅ |
| **BEMF1** | **PC3** | **ADC_IN** | BEMF sense phase | ⚠️ ADC, not COMP |
| **BEMF2** | **PB11** | **ADC_IN** | BEMF sense phase | ⚠️ ADC, not COMP |
| **BEMF3** | **PB13** | **ADC_IN** | BEMF sense phase | ⚠️ ADC, not COMP |
| GPIO_BEMF (network enable) | **PC9** | GPIO | BEMF divider enable | ✅ (GPIO) |
| Hall/Enc A / B / Z | PA15 / PB3 / PB10 | GPIO/EXTI | (sensored only) | free for sensorless |
| DAC/current-ref / SPEED | PA4 / PA5 / PB2 | DAC/ADC | current threshold | — |
| USER button | PC13 | GPIO | — | — |

## The headline finding

**The IHM08M1 is an ADC-BEMF board, not a comparator-BEMF board.** All three
BEMF taps (`BEMF1/2/3`) go to **ADC pins (PC3 / PB11 / PB13)** via 0 Ω resistors,
gated by `GPIO_BEMF` (PC9), referenced to the divider-network neutral. The board's
*only* comparator is the external **LMV331** used for **overcurrent** protection
(CPOUT → PA12/TIM1_ETR). Phases never reach an MCU comparator. This is ST's
AN4220 scheme (sample the floating phase at end-of-PWM-off, compare in software).

## Consequence for AM32

- **Drive side: fully AM32-compatible.** TIM1 6-PWM complementary on
  PA8/9/10 + PA7/PB0/PB1 — exactly what AM32 emits. This is the big fix vs the
  EVLDRIVE102H (whose lows sat on non-TIM1 PD3/PD4).
- **BEMF side: unmodified AM32 can't sense it.** AM32 reads BEMF through the
  MCU's analog comparator (G071 target = COMP2, INM muxed across the 3 phases,
  INP = neutral). On the IHM08M1 the phases only reach ADC pins, so unmodified
  AM32 would drive the motor but be **blind to BEMF → no sensorless lock.**
- **Net:** run **binz's ADC-BEMF Rust stack** on it (ideal — correct drive +
  clean ADC-BEMF network), *or* modify AM32 to add ADC-BEMF, *or* hardware-mod
  the board to route BEMF to the G071 comparator (see below) and run stock AM32.

All four candidate boards (IHM08M1, EVLDRIVE102BH, BOOSTXL-DRV8304H,
STEVAL-IHM043V1) are ADC-BEMF designs — none run unmodified comparator-AM32
without a hardware reroute.

## Hardware reroute — make it a comparator board for stock AM32

**Target (fixed, non-negotiable):** AM32/rm32 on G071 always uses **COMP2**,
INM muxed across **PB3 (phase A / IO1), PB7 (phase B / IO2), PA2 (phase C / IO3)**,
INP = **PA3 (neutral / IO3)**. Those are the *only* COMP2 GPIO inputs on the
G071, so any reroute must land the three phases on PB3/PB7/PA2. (Verified in
`rm32_stm32/src/mcu_g071/comp_init.rs` + every `boards/*g071*.yaml`.)

**IHM08M1 BEMF network (per phase):** phase `OUT` → 10k (R39/40/41) → `BEMFn`
node → 2.2k (R36/37/38) → `GPIO_BEMF` gate; `BEMFn` → **0Ω** → ADC pin.
There is **no neutral star** — you must add one.

### Common surgery (all options)
1. **Lift three 0Ω** to disconnect BEMF from the ADC pins (confirm which
   variant is populated on your board; the others are for F302/other Nucleos):
   - `R59` : BEMF1 ↔ **PC3**
   - `R60` : BEMF2 ↔ **PB11**  (alt `R61`→PC4)
   - `R62` : BEMF3 ↔ **PB13**  (alt `R63`→PB13-F302, `R65`→PC5)
2. **Hotwire the divider-side pad of each lifted 0Ω → the COMP2 pin** (keep
   phase order aligned with the TIM1 phase order):
   - BEMF1 → **PB3**  · BEMF2 → **PB7**  · BEMF3 → **PA2**
3. **Free PB3 from the Hall header:** lift `R81` (0Ω, PB3↔Enc.B/H2); JP3 open
   (default) so no Hall pull-up loads it.
4. **Enable the dividers via PC9 (GPIO_BEMF):** PC9 is **free** — zero
   references in AM32/rm32-G071, and the IHM08M1 uses it only for GPIO_BEMF.
   Add an ifdef'd board-def GPIO: init PC9 push-pull output, **drive LOW at
   boot** (low = dividers enabled, per the 2.2k + BAT30 diode network). No GND
   jumper needed. (Toggle it per sensing window only if you want the power
   saving; for bench, permanent-low is fine.)
5. **Free the VCP pins:** PA2 (and PA3 if used for neutral) are the Nucleo
   ST-Link VCP. Open the NUCLEO-G071RB solder bridges that tie PA2/PA3 to the
   ST-Link, or just don't use the VCP. AM32 doesn't need it (telemetry rides
   the signal pin).

### The neutral — pick one (this is the axis the options vary on)

**Option A — exact rm32 config, neutral → PA3 (recommended, zero firmware change).**
Add a 3-resistor virtual star: **10k from each of BEMF1/BEMF2/BEMF3 to a common
node**, common → **PA3**. Match the three to 1%. Runs *any* stock rm32 g071
board yaml (phase_a=PB3, phase_b=PB7, phase_c=PA2, INP=PA3) — no code, no config.
Cost: VCP fully gone (PA2+PA3). Wires: 3 phase jumpers + 3 star resistors + 1
neutral wire + 1 GPIO_BEMF-to-GND; lifts: R59/R60/R62 + R81.

**Option B — neutral → PB6, one-line board-def tweak (keeps VCP-RX).**
Same phase reroute; star common → **PB6** (an unused COMP2_INP pin — no VCP-RX
sacrifice). Set `INPSEL = IO2 (PB6)` in the board def (one register field, not a
code rewrite). Only PA2 (phase C) still costs VCP-TX. Verify PB6 is the IO2
INPSEL code for COMP2 on G071 before committing.

**Option C — neutral → PB4 (fallback only).**
PB4 is the third COMP2_INP option but the IHM08M1 uses it (`PB4-PWM`, the
DAC/current-ref PWM). Only viable if you don't use that current-reference path;
otherwise prefer A or B.

### The no-surgery alternative (recommended if you're not wedded to stock AM32)
Leave the board stock and run **binz's ADC-BEMF Rust stack** (or a modified AM32
with ADC-BEMF). Zero hardware mod — the board is *designed* for ADC-BEMF, and
the correct 6-PWM drive + clean divider network make it the ideal ADC-BEMF
platform. This trades "stock AM32" for "no soldering."

### Verify before cutting
- Confirm which BEMF 0Ω variants are actually populated (R59/R60/R62 vs the
  F302/PC5 alternates) — probe continuity from each ADC pin to its BEMF node.
- Confirm PB7 and PB6 are truly unrouted on your board (no hidden load).
- Confirm the G071 COMP2 INPSEL code for PB6 (Option B) against RM0444.
- After the mod, re-scale the BEMF divider expectation in the board def to the
  10k/2.2k ratio (≈0.18) if AM32's default assumes a different divider.

