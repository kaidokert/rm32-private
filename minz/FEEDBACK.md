# Outside agent feedback on `COMP2_DIAGNOSIS.md`

Three independent agents were given the diagnosis doc. Their verbatim
takes follow. Synthesis happens after all three are captured.

---

## Agent #1

### Initial round (before "AM32 works on this board" disclosure)

> My assessment is that the document's core conclusion is sound: this does
> not look like a real BEMF zero-cross signal being "buried in noise." It
> looks like the comparator is mostly reporting a PWM-synchronous
> artifact, with the actual BEMF information either absent at the
> comparator inputs or unusable without blanking.

**Top three guesses, in order:**

1. **PB4 is not tracking the motor star-point, so COMP2 is comparing a
   mostly static reference against a PWM-swinging floating phase.**
   Strongest fit to the evidence: the count is essentially fixed versus
   electrical frequency, sits in the same ballpark as the 24 kHz carrier,
   and hysteresis only reduces chatter density rather than revealing a
   speed-dependent event stream. Exactly what you'd expect if the
   comparator is flipping on carrier ripple/common-mode swing, not on
   rotor-position BEMF crossings.

2. **The board's "virtual neutral" on PB4 is wired differently than
   assumed.** Main alternative to #1. If PB4 is a rail divider, an
   unfiltered averaged node, or some other bias network rather than a
   true neutral-like tracking node, the comparator behavior can still
   be dominated by PWM-rate mismatch. Ranked below #1 because it's more
   a hardware-specific explanation of *why* #1 happens than a separate
   signal-theory explanation.

3. **Open-loop rotor slip/misalignment makes expected zero-cross timing
   meaningless.** Materially less likely as the primary cause. Slip can
   spoil BEMF timing but does not explain the nearly flat ~20k/s count
   across a large speed range nearly as well as PWM-coupled comparator
   toggling does. At most, slip may be making a bad setup worse.

> Short version: best guess is "wrong signal at the comparator during PWM
> edges," not "too much random noise" and not "bad sector mapping." The
> fastest discriminator is still the one your note ends with: scope PB4
> and one observed phase such as PB7 during a float window. If PB7 is
> swinging hard at the PWM rate relative to a near-static PB4, that
> basically closes the case and justifies implementing proper comparator
> blanking/sample timing.

### Revised round (after the disclosure that this board runs AM32 firmware fine)

> That materially strengthens H1 and weakens H3.
>
> If the same board runs AM32 sensorless commutation correctly, then the
> hardware topology is probably not fundamentally wrong for BEMF
> detection.

**Revised ranking:**

1. **COMP2 is seeing the right nodes, but without the timing treatment
   AM32 uses.** Most likely missing piece is PWM-edge blanking, sampling
   discipline, or comparator gating synchronized to TIM1. On these ESCs,
   "compare floating phase to virtual neutral" is usually only valid
   during the quiet part of the PWM cycle. If your implementation
   exposes the raw comparator continuously, it will happily fire on
   carrier-induced common-mode motion.

2. **Your analog reference is board-correct, but your use of it is not
   board-correct.** PB4 may still be the expected AM32 neutral/reference
   node, but AM32 likely assumes a specific filtering/blanking scheme
   around it. So the issue is less "PB4 is the wrong net" and more "PB4
   alone is not enough unless handled exactly the way production
   firmware handles it."

3. **Open-loop operating point is masking the usable zero-crosses.**
   Still possible, but now clearly secondary. Since AM32 works on this
   hardware, the board can produce usable BEMF information. Your
   current open-loop drive, duty, freewheel behavior, or commutation
   timing may simply be generating a waveform that requires the same
   blanking and timing windows AM32 already uses.

**What I'd conclude from "AM32 works":**

- The board is probably fine.
- The comparator concept is probably fine.
- The missing detail is probably in the measurement method, not the
  hardware map.

> Stop treating "is PB4 the right kind of node at all?" as the main
> question, and instead compare your `comp2.rs` and PWM timing behavior
> directly against AM32's comparator blanking/gating path. That is the
> highest-value next step.

---

## Agent #2

### Initial round (before "AM32 works" disclosure)

**Top three causes, ranked most-to-least probable:**

1. **H1: The comparator is measuring your 24 kHz PWM carrier, not the
   motor's BEMF.**
   - *Theory.* Motor's electrical star point swings ±Vbus/2 at PWM
     frequency. Floating phase input (e.g. PB7) sees the massive AC
     swing. If virtual neutral pin (PB4) is a static DC voltage (Vbus/2
     resistor divider), comparator triggers on every PWM cycle as the
     floating phase swings across the static reference.
   - *Why it fits.* Explains ~20 k/s (matching 24 kHz PWM with some
     missed cycles) and flat-vs-f across 5× speed range — carrier
     frequency is fixed, motor speed doesn't matter.
   - *Fix.* Production firmwares use PWM blanking. Must ignore
     comparator output during PWM switching transients.

2. **H3: The board's hardware for "virtual neutral" (PB4) isn't what we
   think it is.**
   - Blindly assuming PB4 is a standard averaged R-network or Vbus/2
     divider. If schematic reveals it's tied to full Vbus, an arbitrary
     bias, or lacks a necessary filter cap, comparator's trip point is
     garbage. A wrong reference level combined with PWM noise would
     cause constant, nonsensical triggering.

3. **H2: Rotor completely out of sync (slip).**
   - Open-loop drive → rotor might not be where the code thinks it is.
     When code opens float window expecting a zero-crossing, phase might
     actually be saturated at peak voltage. End up measuring PWM noise
     riding on a saturated DC offset.
   - *Why less likely.* Doesn't fully explain the perfectly flat rate
     across 60–310 Hz; if it were slip, beat frequencies or noise
     characteristics should change with motor speed.

> Next Step: definitive test is scope on PB4 and PB7 simultaneously. If
> PB4 is flat DC and PB7 is swinging at 24 kHz, H1 is your culprit and
> you need to implement TIM1 blanking windows.

### Revised round (after disclosure that AM32 works on this board, including RPM reporting)

> Hardware is correct, but the Software-Timer-Comparator integration in
> `minz` is incomplete. The 20 000 counts/sec is the comparator
> "measuring" your 24 kHz PWM carrier instead of the motor BEMF. Since
> AM32 reports RPM correctly, it must be successfully "blinding" the
> comparator to the carrier.

**Revised top three with concrete implementation pointers:**

1. **Missing blanking configuration (H1).** Most likely. In
   `src/comp2.rs`, blanking is currently explicitly disabled. AM32
   likely sets `COMP2_CSR.BLANKING` to `0b101` (**TIM1 OC4**) or `0b001`
   (TIM1 OC5). This forces the comparator output low during the PWM
   transitions and the "off" period when the star-point voltage
   collapses, preventing a false edge on every PWM cycle.

2. **Lack of digital filtering (Timer Input Capture).** Raw EXTI on a
   comparator is notoriously twitchy. Production firmwares often route
   the COMP output into a **Timer Input Capture (e.g., TIM1 CH4)**
   rather than EXTI. That allows the timer's built-in Digital Filter
   (ICFx bits) to ignore pulses shorter than a few microseconds —
   effectively deleting the 24 kHz PWM noise while still catching the
   "big" BEMF crossing.

3. **PWM alignment mismatch.** AM32 often uses **center-aligned PWM**.
   Transitions are symmetric around the middle of the cycle, allowing a
   much cleaner blanking window (single OC channel covers both turn-on
   and turn-off spikes) vs edge-aligned PWM, where the turn-off spike
   moves with duty cycle and is harder to blank statically.

> Recommended next step: since AM32 uses it, try enabling TIM1 OC4
> blanking in `src/comp2.rs` and setting `TIM1_CCR4_TRGO` to a value
> that covers the "off" time of the PWM (e.g. set OC4 to PWM mode 2
> so it's High whenever PWM is Off). If the 20 k/s count drops
> significantly, you've found the leak.

---

## Agent #3 (has access to AM32 source + rm32 firmware)

> The diagnostic doc on the minz bench (open-loop, no rm32 firmware
> involved) found ~20 000 COMP IRQs/sec flat across motor speed — pure
> PWM-edge ringing, no BEMF signal visible. Hysteresis alone (even at
> max 22 mV) cuts the count by ~5× but doesn't reveal real
> zero-crosses. The PWM-coupling noise is bigger than the BEMF signal
> at this duty/speed regime, by a factor of ~3000×.
>
> This directly explains rm32's 88 % rejection rate in
> `bemf_zero_cross`. COMP IRQ fires on PWM ringing → noise filter
> samples filter_level times → mostly rejects → occasional ringing
> happens to look like a valid direction → false accept → confused
> commutation timing → motor chops/stalls.

### Initial reasoning (followed the diagnosis doc's pointer toward blanking)

> The doc nails the structural fix: COMP2 `BLANKING` — bits 18:20 of
> `COMP2_CSR`. The comparator output is gated to a TIM1 OC channel;
> during PWM-on the comparator result is forced to a known value, only
> sampled in the steady portion of each PWM period when V_star ≈
> Vbus/2 ≈ PB4. AM32 uses this; the doc author identified that as the
> only thing that makes BEMF detection mathematically possible on a
> 6-step ESC.

### Verification step — and the surprise

> Wait — AM32 has `LL_COMP_BLANKINGSRC_NONE` for both COMP1 and COMP2
> init. Blanking is NOT used in AM32. Let me verify nothing else
> changes it later:

> Found it. The diagnostic doc's analysis is right about the noise
> problem, but **wrong about AM32's fix** — AM32 explicitly sets
> `OutputBlankingSource = LL_COMP_BLANKINGSRC_NONE` for both COMP1 and
> COMP2. No hardware blanking.

### What AM32 actually does

AM32's noise mitigation is a **software interval-timer guard in the
COMP ISR wrapper itself**, which rm32 is missing. From
`AM32/Mcu/l431/Src/stm32l4xx_it.c::COMP_IRQHandler`:

```c
void COMP_IRQHandler(void) {
    if (LL_EXTI_IsActiveFlag_0_31(EXTI_LINE) != RESET) {
        if ((INTERVAL_TIMER->CNT) > ((average_interval >> 1))) {
            LL_EXTI_ClearFlag_0_31(EXTI_LINE);
            interruptRoutine();                    // process zero-cross
        } else {
            if (getCompOutputLevel() == rising) {
                LL_EXTI_ClearFlag_0_31(EXTI_LINE);  // suppress, but ack
            }
            // else: leave flag set, refire on next edge
        }
    }
}
```

AM32 only runs the inner zero-cross logic if
`INTERVAL_TIMER_COUNT > average_interval / 2`. **That's a software
blanking window covering the first half of the expected commutation
period** — exactly where PWM-edge ringing dominates over BEMF. AM32
throws those IRQs away.

rm32's wrapper at `mcu_l431/interrupts.rs::COMP()` runs `handle_comp`
unconditionally — **no time-window guard**. Every PWM-coupling glitch
within the first half of the period gets processed and probably
accepted falsely (after some PWM cycles the comparator may have
settled to whatever direction matches the post-ZC expectation by
chance).

### The three-layer filter — what's actually present where

| Filter                                                | AM32                    | rm32                 |
|-------------------------------------------------------|-------------------------|----------------------|
| 1. Hardware hysteresis                                | none (set NONE in init) | none (set 0 in init) |
| 2. Software timing window (interval > avg/2)          | **yes — in wrapper**    | **no — missing**     |
| 3. Software noise filter (filter_level samples)       | yes                     | yes                  |

> The minz diagnostic measured layer (1)+(3) only — that's why it saw
> 20 k/sec of noise. AM32 firmware adds layer (2) on top, which would
> cut that further by the duty of "second half of expected commutation
> period" (~50 %) AND by suppressing IRQ fires that happen on PWM
> transients (which cluster around PWM edges, not zero-cross
> expectations).
>
> This explains:
>
> - Why minz bench saw 20 k/sec — no AM32-style timing guard
> - Why AM32 firmware on the same hardware is silky smooth — has the
>   guard
> - Why rm32 firmware chops — also missing the guard

### Proposed fix (for rm32 firmware, not minz directly)

> Modify `mcu_l431/interrupts.rs::COMP()` to mirror AM32's structure:

```rust
fn COMP() {
    let exti = unsafe { &*pac::EXTI::PTR };
    let interval_now = TIM2.cnt.read().bits();         // INTERVAL_TIMER->CNT
    let avg_half = shared.commutation_interval() >> 1; // average_interval >> 1

    if interval_now > avg_half {
        // Past the blanking window — process the zero-cross
        exti.pr1.write(|w| w.bits(1 << 22));
        exti.imr1.modify(|r, w| w.bits(r.bits() & !(1 << 22)));
        isr_handlers::handle_comp();
    } else {
        // Inside the blanking window — PWM noise. Suppress.
        // AM32: clear flag only if comp_level matches old direction
        // (else leave pending so ISR refires when edge settles).
        if comp_level_matches_rising() {
            exti.pr1.write(|w| w.bits(1 << 22));
        }
        // else: leave PR1 set → tail-chain refire
    }
}
```

### Subtleties to think about

> - `average_interval` in AM32 = `e_com_time / 3` (computed in main
>   loop, written every iter). rm32 has the same —
>   `self.timing.average_interval` in `main_state.tick`. Need to
>   expose to the ISR via shared.
> - `commutation_interval` (shared) is different — it's the per-phase
>   interval, ~6× smaller than `average_interval`. Need to make sure
>   I'm using the right one. Actually looking at the AM32 line again:
>   `INTERVAL_TIMER->CNT > average_interval >> 1`. `average_interval`
>   = `e_com_time / 3` (full electrical revolution / 3 ≈ 2 phases ≈ 2×
>   commutation_interval). So `average_interval / 2` ≈
>   `commutation_interval`. Different from `commutation_interval / 2`.
>   **Important to get right.**

---
