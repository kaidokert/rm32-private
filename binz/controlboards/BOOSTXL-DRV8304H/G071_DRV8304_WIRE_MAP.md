# NUCLEO-G071RB ↔ BOOSTXL-DRV8304HEVM — verified free-wire map (rm32/AM32, config-only)

**Wire by the Arduino silk label** (printed on the PCB) where a pin has one; the morpho
number is the backup.

Boards **not stacked**. Point-to-point jumpers. Common the grounds. G071 pins are rm32's
real ones (`phase.rs`, `pwm.rs`, `adc.rs`, `comp_init.rs`); DRV side verified vs MD008E2
schematic + DRV8304 datasheet (slvse39b). No firmware change bar two board-def constants (§H).

Phase mapping is consistent everywhere: rm32 A→DRV A→MOTA→VSENA, B→B, C→C.

## Rule zero
- [ ] Do **NOT** stack the morpho. Boards side by side.
- [ ] **Common the grounds:** several wires, Nucleo GND ↔ DRV GND (J1 screw PGND + a header GND).
- [ ] Do **NOT** jumper the 3.3 V rails — the EVM makes its own 3.3 V from VM. (ENABLE-high uses Nucleo 3V3, fine.)
- [x] **SB16 / SB18 already cut this session** → PA2/PA3 freed from ST-Link VCP. (Serial console now on USART3 PC10/PC11.)

## A. Gates + LED (G071 → DRV J4)
| rm32 signal | G071 pin | **Arduino silk** | morpho (backup) | → DRV | J4 pin |
|---|---|---|---|---|---|
| Phase A high | PA10 (TIM1_CH3) | **D2** | CN10-33 | INHA | J4-1 |
| Phase A low | PB1 | **A3** | CN7-34 | INLA | J4-3 |
| Phase B high | PA9 (TIM1_CH2) | **D8** | CN10-21 | INHB | J4-5 |
| Phase B low | PB0 | **D10** | CN10-17 | INLB | J4-7 |
| Phase C high | PA8 (TIM1_CH1) | **D7** | CN10-23 | INHC | J4-9 |
| Phase C low | PA7 | **D11** | CN10-15 | INLC | J4-11 |
| LED (status) | PB5 | **D4** | CN10-29 | LED | J4-15 |

## B. BEMF — 4 wires (DRV J5 → G071 comparator)
VSENA/B/C = the 82k/7.5k phase-voltage dividers = BEMF source.
| DRV pin | wire to G071 | Arduino silk | morpho |
|---|---|---|---|
| VSENA (J5-6) | PB3 (COMP2 INM A) | **D3** | CN10-31 |
| VSENB (J5-8) | PB7 (COMP2 INM B) | — | **CN7-21** |
| VSENC (J5-10) | PA2 (COMP2 INM C) | — | **CN10-34** |
| star common | PA3 (COMP2 INP neutral) | — | **CN10-6** |
- [ ] Star legs tap VSENA / VSENB / VSENC (same 3 J5 pins): each = one wire to its INM pin + one to a star resistor.

## C. Sense — VBUS + per-phase current (all ADC)

**2026-09-12 measured correction, LAB_REPORT Entry 016:** relative to the
qualified gate/VSEN phases, logical current A arrives on PA4/ADC4, B on
PA1/ADC1, C on PA0/ADC0. The table below is the original intended routing,
not the measured logical association. No wires moved; shell-pwm compensates
with CURRENT_ADC=[4,1,0]. A common permutation of physical phase labels is
not independently excluded, but the relative electrical mapping is tested.
- [x] **VSENVM (J5-3) → PA6** — Arduino **D12** (backup CN10-13), ADC IN6, ×11.94. **DONE 2026-09-08** (`examples/drv-vm.rs`, tracks bus, reads ~1% low).

All 3 DRV current-sense-amp outputs → the 3 free analog pins (per-phase low-side current, ~70 mV/A, ~1.65 V at 0 A):
| DRV | wire to G071 | ADC ch | Arduino silk | morpho |
|---|---|---|---|---|
| ISENA (J5-16) | **PA0** | IN0 | **A0** | CN7-28 |
| ISENB (J5-14) | **PA1** | IN1 | **A1** | CN7-30 |
| ISENC (J5-12) | **PA4** | IN4 | **A2** | CN7-32 |

## D. Logic pins → G071 GPIO
- [ ] **ENABLE (J5-9) → PD1** (CN7-10), driven HIGH at boot by firmware **OR** tie J5-9 → 3.3 V for zero-code always-on. Low = sleep.
- [ ] **nFAULT (J5-15) → PB14** — Arduino **D6** (backup CN10-25), GPIO input, open-drain (has its own pull-up on the EVM). Optional.

## D2. DRV8304H config straps — on the DRV board, NOT G071. Tie to fixed levels.
Power-up-LATCHED, multi-level analog pins — a binary GPIO can't set them and they latch at DRV power-up.
- [x] **MODE (J4-13) → GND (hard) = 6× PWM** — DONE via jumper **J4-13 → J4-2** (J4-2 IS GND; verified at full zoom). Required (rm32 drives 6 independent gates + GPIO-float; 3× would need a firmware rewrite). Board defaults to 3× (R19 47 kΩ→AGND on MODE = datasheet 3× strap); the hard GND wire overrides it → 6×. Zero firmware, latches correctly every power-up. (J4-2 also serves as a convenient header GND.)
- [ ] Verify by metering at power-up: MODE ≈ 0 V (after the GND tie).

## E. Motor + power
- [ ] Motor 3 leads → **J2 (MOTA / MOTB / MOTC)**. Backwards → swap any two leads (don't rewire gates).
- [ ] Motor bus → **J1** screw terminal. Separate PSU, 11.8 V, current-limited (0.3–0.5 A first).
- [ ] Nucleo on USB. Grounds common.

## F. Throttle / telemetry (straight to G071)
- [ ] Throttle/DShot → **PB4** — Arduino **D5** (backup CN10-27, TIM3_CH1). Not through the DRV board.
- [ ] Telemetry → **PB6** (CN10-24, USART1_TX) — optional.

## G. Meter check (power OFF) before first boot
- [ ] Each gate wire: G071 pin ↔ its J4 pin, no cross-wiring (verify by silk label)
- [ ] MODE (J4-13) continuity to GND; ENABLE (J5-9) to 3.3 V (or PD1)
- [ ] Grounds common; PA2/PA3 open to ST-Link (SB16/SB18 already cut)
- [ ] PA3 reads ~star/3 to each VSENx node
- [ ] No gate/sense pin shorted to GND or 3V3

## H. Board-def constants (config-only)
- **VBUS multiplier ≈ 11.94** (82k+7.5k / 7.5k).
- **Current: GAIN = 10 V/V (R27 = 47 kΩ→AGND strap) × 0.007 Ω = 70 mV/A**, bidirectional about ~1.65 V. Three per-phase low-side CSAs on PA0/PA1/PA4 (ISENA/B/C) — not a DC-link shunt.

## rm32 phase map (reference)
```
Phase A: PA10/PB1 → INHA/INLA → MOTA → VSENA → PB3
Phase B: PA9/PB0  → INHB/INLB → MOTB → VSENB → PB7
Phase C: PA8/PA7  → INHC/INLC → MOTC → VSENC → PA2
COMP2 INP (neutral)=PA3. VBUS=PA6. Current ISENA/B/C=PA0/PA1/PA4. Throttle=PB4.
```
