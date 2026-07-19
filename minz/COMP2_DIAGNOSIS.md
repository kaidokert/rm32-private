# COMP2 BEMF detection — fundamentals out of whack

We are firing the EXTI line on COMP2 ~20 000 times per second when we should be
firing it ~2 times per electrical revolution. At f=60 Hz electrical that's a
**~200× excess**. The signal we want isn't anywhere visible in the output —
something deeper than "noise floor" is going on. This document captures what
we set up, what we measured (and didn't), and the three most likely root
causes to investigate next.

## a) Experiment setup

### Hardware

- Board: Vimdrones L431 ESC, STM32L431KCU6, motor-attached, open-air prop
- Bench supply: ~5.4 V to VBat. Confirmed visibly spinning.
- USB-TTL @ 9600 8N1 for host comms (PB6 half-duplex)

### Code under test

| What                              | File                                        |
|-----------------------------------|---------------------------------------------|
| Bench tool / main loop            | `minz/examples/motor_tester.rs`             |
| COMP2 init + EXTI wiring          | `minz/src/comp2.rs`                         |
| TIM1 motor PWM + 6-step CCER      | `minz/src/tim1_motor_pwm.rs`                |
| Sector mapping helper             | `minz/src/open_loop.rs::six_step_sector`    |
| BEMF pin map (consulted)          | `rm32_stm32/boards/vimdrones_l431.yaml`     |
| Reference impl (cross-checked)    | `rm32_stm32/src/mcu_l431/comparator.rs`     |
| Reference 6-step table            | `rm32_stm32/src/phase.rs::com_step`         |

### COMP2 configuration (`comp2.rs::init`)

- **INP+** = `PB4` (IO1) — "virtual neutral" / common reference from the
  board's R-network
- **INM−** = one of `PB7` / `PA5` / `PA4` (IO2/IO5/IO4) → phase A / B / C
  BEMF sense pin; switched live via `set_observed_phase()`
- `EN=1`, `PWRMODE=00` (high-speed), `POLARITY=0` (non-inverted),
  `HYST=0b11` (**maximum**, ~22 mV typ deadband)
- `SCALEN=0`, `BRGEN=0`, `WINMODE=0`, `BLANKING=0` — internal VREFINT
  scaler and TIM blanking are both OFF (i.e. the comparator output is the
  raw analog comparison, no internal filtering or gating).

### EXTI line 22 (`comp2.rs::configure_exti_both_edges` + `set_exti_enabled`)

- Both rising and falling edges armed in `RTSR1.TR22` / `FTSR1.TR22`
- `IMR1.MR22` masked at boot; unmasked only by the main loop during the
  observed phase's float sectors (with the recently-added 5-step guard
  band at each sector boundary)
- ISR (`motor_tester.rs::COMP`): clears `PR1.PR22`, increments
  `COMP_COUNT` atomic
- Rate computed in the outer loop over rolling ~1 s DWT-cycle windows,
  stored in `COMP_RATE` for the `b` key

### Drive (`tim1_motor_pwm.rs::set_six_step`)

- TIM1 PWM at 24 kHz, dead-time 45 cycles, complementary outputs
- 6-step commutation table (HIGH/LOW arrays at top of `set_six_step`):

  | Sector | Hi | Lo | Float |
  |--------|----|----|-------|
  | 0      | A  | B  | C     |
  | 1      | A  | C  | B     |
  | 2      | B  | C  | **A** |
  | 3      | B  | A  | C     |
  | 4      | C  | A  | B     |
  | 5      | C  | B  | **A** |

  Phase A floats in sectors 2 and 5. Matches `rm32_stm32/src/phase.rs::com_step`
  steps 2 and 5 — independently verified consistent.

- Float is achieved by clearing `CCxE` and `CCxNE` for the floating
  phase's channel. With `BDTR.OSSR=0` (set by `tim1_motor_pwm::init`),
  the AF block releases the pad → pin goes Hi-Z, gate driver outputs both
  FETs off.

### Open-loop drive parameters at the moment of measurement

- `mode = SixStep`, `amplitude = 15 %`, `electrical_hz` varied 60..310 Hz
- Motor visibly spins for the whole range
- Open-loop V/f — no rotor sync, no current limit, no BEMF closed loop

## b) What we measured (and what's missing)

### What the comparator does say

| Observed phase | Pin  | Rate (steady)  |
|----------------|------|----------------|
| A              | PB7  | ~20 000 /s     |
| B              | PA5  | ~13 300 /s     |
| C              | PA4  | ~13 300 /s     |

Each entry is **flat from f=60 to f=310** — the count does **not change**
with motor frequency, despite the rotor visibly speeding up.

The three phases give different counts but the ratios are stable:
A : B : C ≈ 24 : 16 : 16 (before guard band) → 20 : 13.3 : 13.3 (after).
The guard band reduced all three by the same 17 % — exactly the duty-cycle
clip factor — confirming the asymmetry is not from sector-boundary
transition glitches.

### What is conspicuously absent

- **No frequency dependence.** Real BEMF would scale linearly with motor
  speed; we see zero change across a 5× speed range.
- **No "near-1-event-per-window" floor.** Expected behaviour: in 6-step
  with phase A floating during sectors 2 and 5, the BEMF on phase A
  crosses neutral exactly once per float window, twice per electrical rev,
  ≈ 120 /s at f=60 Hz. We see **20 000 /s**, ~200× too many, and
  unrelated to speed.
- **No measurable effect from hysteresis.** Going from 0 mV to 22 mV
  (max) hysteresis dropped the count from ~400 k/s to ~20 k/s, but the
  remaining 20 k is still pure flat-vs-f. Hysteresis cut the bouncing
  per PWM edge from ~8 to ~1, but didn't reveal the underlying signal.
- **No effect from sector gating.** Gating to the float-only window only
  reduced the count proportionally with the gate duty cycle (33 % → 28 %),
  meaning the noise *density* during the float window is the same as
  during driven windows.

### The verified non-causes (from prior sessions)

These are already ruled out and should not be re-litigated:
- Phase-to-channel mapping (cross-verified against rm32 firmware)
- Sector gating logic (confirmed `rate=0` when motor killed)
- HAL ADC / VDDA calibration (only relevant to the `i` printout, not
  COMP2 inputs)
- Hysteresis ceiling (already at max)
- Transition-glitch boundary noise (guard band gave purely proportional
  reduction → no glitch-specific signal)

## c) Top three hypotheses for the 20 000/s

Ordered from "most likely root cause" to "long-shot but testable."

### H1: PB4 is a STATIC reference and floating-phase voltage swings massively at PWM rate around it

This is the strongest candidate. In a 6-step BLDC drive with two phases
PWM-active and one phase floating, the **motor's electrical star point**
swings between Vbus/2 (during PWM-on, current flowing through the high
side of one driven phase and low side of the other) and ~0 (during
PWM-off, current freewheeling through both low-side FETs / body diodes).
That swing is ±Vbus/2 = ±2.7 V at 24 kHz.

A floating motor terminal sits at `V_star + BEMF_phase`. So PB7 sees the
**full PWM-rate swing** of the star point — divided through the BEMF
sense network (presumably ~9× similar to vbat), that's still ~300 mV at
24 kHz on PB7.

Meanwhile `PB4` (the "virtual neutral") on this kind of board is
typically a static R-divider on the supply rail (`Vbus/2` set by two
matched resistors). It does **not** track the star-point swing.

Result: the comparator sees PB4 = constant ~Vbus/2 while PB7 swings
±300 mV around that level at 24 kHz. The comparator output flips on
every PWM cycle, completely independently of BEMF. **Hysteresis at 22 mV
is irrelevant** because the AC swing is an order of magnitude bigger.

This explains everything we observe:
- 20 k/s ≈ 24 kHz with ~80 % survival through hysteresis → matches a
  comparator clocked by the PWM carrier, not by motor speed.
- Frequency-independence → carrier is fixed, motor speed doesn't matter.
- Phase asymmetry → small differences in trace routing / divider
  tolerance change the AC coupling magnitude per phase.

**Test.** Scope PB4 directly. Hypothesis predicts a near-DC level at
Vbus/2. If instead PB4 also swings at PWM rate (because it's tied into
the same three-phase R-network or has insufficient bypass), the picture
is more complex but the **mismatch** between PB4 and PB7 still ends up
oscillating across the trip point. Either way, **scoping PB4 + PB7
simultaneously during a float window** tells us exactly what the
comparator is seeing.

**Fix if confirmed.** Blanking. The reference implementations (AM32 and
rm32 production firmware) hook the comparator output through TIM1's
blanking input so the comparator result is **forced to a defined value
during the PWM-edge ringing**, and only sampled in the steady portion of
each PWM cycle when V_star ≈ Vbus/2 ≈ PB4. That's why blanking is the
production fix — it's not a "noise reduction nicety", it's structurally
required for this measurement to make sense.

### H2: PB4 is the right reference but rotor is grossly out of sync with the commanded sectors

Open-loop V/f drive at amp=15 % has no rotor-position feedback. The rotor
might be slipping by significant phase angles relative to the commanded
field — i.e., when our code thinks "phase A is in its float window and
BEMF should be crossing neutral", the rotor is actually somewhere else
entirely and BEMF is saturated at peak +V or peak −V rather than crossing
zero.

In that case the BEMF signal looks like a **square wave at electrical
frequency** rather than a clean zero-crossing waveform during the gate
window, and the count we see is **PWM coupling on a slowly-shifting DC
offset** — same character as H1 but a different mechanism.

This is a weaker candidate because:
- It would predict the count to vary as the rotor slips into and out of
  alignment with the commanded field, which we don't observe.
- It doesn't explain why the count is so cleanly flat across a 5×
  frequency range.
- Visibly the motor *does* spin — slip exists but the rotor is following.

But it's testable.

**Test.** Slow the motor way down (f=1..5 Hz) where the rotor *must* keep
up with the field, and the BEMF crossings should walk through the gate
window slowly enough to count discretely. If we still see ~20 k/s, slip
is not the cause. If the count drops dramatically and starts scaling with
f, slip is at least partially in play.

### H3: The board's "virtual neutral" on PB4 is connected to a different reference than we think

We assumed PB4 is the AM32-standard "virtual neutral" — a passive
R-network averaging the three motor terminals (or a supply-rail divider
estimating Vbus/2). It might be:

- A **single resistor to Vbus** with no balance to ground (so it sits at
  full Vbus, not Vbus/2) — would give a constant high reference and the
  comparator would almost always read 1.
- A **scaled reference unrelated to the motor terminals** (e.g. tied to
  a regulator output or a fixed bias) — would also give a static value
  but at an arbitrary level.
- A **three-phase R-network with no filter cap**, picking up the same
  PWM swing as the BEMF inputs but with different phase shift — would
  give a beat pattern across the comparator at PWM rate.

We have **not actually inspected the schematic for PB4**. The yaml
(`bemf_pins.common: PB4`) labels it but doesn't tell us what's on the
other side. The user has the schematic — we should verify before
assuming.

**Test.** Pull up the Vimdrones L431 schematic and confirm what PB4 is
wired to. Specifically:
- Are there three resistors of equal value tying PB4 to phases A/B/C?
- Is there a filter capacitor on PB4 (and how big)?
- Are the BEMF sense dividers on PB7/PA5/PA4 the same topology as the
  common net?

If PB4 turns out to be wired weirdly, all three observations (flat
20k/s rate, ratio asymmetry, hysteresis ineffectiveness) get a different
explanation than H1 — but the cure is similar (blanking, or a different
reference choice).

## Next concrete step

The cheapest discriminator is **scope-probing PB4 and PB7 during a float
window**. If H1 is right we'll see PB4 near-flat, PB7 swinging hugely
at 24 kHz, and the comparator trip threshold sitting right in the middle
of that swing. That single scope shot rules in H1 and rules out the
others — and tells us whether blanking is required or whether we missed
something simpler.
