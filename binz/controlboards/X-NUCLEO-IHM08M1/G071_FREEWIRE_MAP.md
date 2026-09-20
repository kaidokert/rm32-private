# IHM08M1 ↔ NUCLEO-G071RB — free-wire map (run rm32/AM32, config-only)

Boards side by side, **NOT stacked on the morpho.** Point-to-point jumpers only.
Common the grounds. Pins below are rm32's real G071 pins (from `phase.rs`,
`pwm.rs`, `adc.rs`, `comp_init.rs`, `input_capture.rs`) — no firmware change.

## Rule zero
- [ ] Do **NOT** mate the morpho headers. Side by side only.
- [ ] Run several **GND** jumpers (G071 GND ↔ IHM08M1 GND).

## A. Gates (6 wires)
Tap the IHM08M1 at the listed morpho position (gate is live there via its 0 Ω bridge).

- [ ] **PA10 → UH** (IHM08M1 CN10-23)
- [ ] **PB1 → UL** (IHM08M1 CN10-15)
- [ ] **PA9 → VH** (IHM08M1 CN10-21)
- [ ] **PB0 → VL** (IHM08M1 CN7-34)
- [ ] **PA8 → WH** (IHM08M1 CN10-33)
- [ ] **PA7 → WL** (IHM08M1 CN10-24)

Motor 3 leads → OUT1 / OUT2 / OUT3. Spins backwards → swap any two motor leads
(do NOT rewire gates).

## B. BEMF (4 wires) — verify existing taps
Confirm each tap is on the TRUE BEMF node (old doc named these by wrong G071 pins).

- [ ] **BEMF1 node → PB3**  (IHM08M1 CN7-37 / R59 pad)
- [ ] **BEMF2 node → PB7**  (IHM08M1 CN10-18 / R60 pad)
- [ ] **BEMF3 node → PA2**  (IHM08M1 CN10-6 / R65 pad)
- [ ] **47 k star common → PA3**

## C. Sense / throttle / enable / power
- [ ] **Single-shunt current out → PA4**  (confirm which Curr_fdbk pin carries the shunt in 1-Sh mode)
- [ ] **VBUS_sensing → PA6**  (IHM08M1 CN7-30)
- [ ] **throttle/DShot source → PB4**  (straight to G071, not through IHM08M1)
- [ ] **free G071 GPIO → GPIO_BEMF**  (IHM08M1 CN10-1; drive to enable dividers — confirm polarity)
- [ ] **G071 3V3 → IHM08M1 3V3**
- [ ] **PB6 → telemetry**  (optional)
- [ ] Motor bus: IHM08M1 **J1** screw terminal, separate PSU, 11.8 V, current-limited

## D. Meter check (power OFF) before first boot
- [ ] Each gate wire: G071 pin ↔ its IHM08M1 gate pin, no cross-wiring
- [ ] Grounds common between boards
- [ ] No gate pin shorted to GND or 3V3
- [ ] BEMF: PA3 reads ~star/3 to each BEMF node

## rm32 phase map (reference)
```
Phase A: high PA10, low PB1   (leg U / OUT1 / BEMF1)
Phase B: high PA9,  low PB0   (leg V / OUT2 / BEMF2)
Phase C: high PA8,  low PA7   (leg W / OUT3 / BEMF3)
COMP2 INM = PB3/PB7/PA2, INP(neutral) = PA3
Current = PA4 (ADC_IN4), VBUS = PA6 (ADC_IN6), throttle = PB4 (TIM3_CH1)
```

## Why not stacked
On the G071 morpho the IHM08M1's gate holes route to the wrong pins
(PA7→BKIN, PA8→VH, PA10→Hall, …). Stacking forces a dozen bridge lifts.
Free-wiring sidesteps all of it.
