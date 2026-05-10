# rm32 L431 — BEMF Comparator Selects Wrong Pins

Third in the series of "ISR-side HAL feedback / chip-specific config that the
port dropped or got wrong" — after the buffer-alignment bug
(`BRINGUP_NOTES_L431.md`) and the prescaler bug (`LOST_PRESCALER.md`). With
those two fixed, rm32 actually attempts to start the motor: arming works,
throttle is decoded, the state machine transitions `Armed → OldRoutine`,
PWM duty is driven through the phase outputs. On the bench, the motor
**twitches once at the moment of throttle-up** then sits dead. RTT trace
shows `zc=0` (zero zero-crosses) for the entire run — open-loop
commutation with no BEMF feedback.

## Why we know this is BEMF detection

With AM32 flashed on the same ESC, same wiring, same BF/F411 input source,
the motor spins normally under the same `bf_motor_sequence.py --motor 0`
test. So:

- F411 → PWM signal on PB1 → ESC PA2: ✓ works
- ESC PA2 → AM32/rm32 input capture / decode: ✓ works (we see correct
  µs-scale throttle values once the prescaler bug was fixed)
- ESC FETs + phase outputs PA10/PB1/PA9/PB0/PA8/PA7: ✓ works (motor
  twitches on commutation step, AM32 spins via these same pins)
- Motor wiring to ESC phases: ✓ works (AM32 spins)
- **BEMF zero-cross detection on rm32: ✗ never fires** — `zc=0` for the
  whole run, even with the stuck-rotor latch disabled

## Root cause — wrong COMP2 INMSEL values

The Vimdrones L431 board uses AM32's `HARDWARE_GROUP_L4_B` pin map, which
specifies (from `AM32/Inc/targets.h:4788-4792`):

```c
#define USE_COMP_2
#define PHASE_A_COMP LL_COMP_INPUT_MINUS_IO2  // PB7
#define PHASE_B_COMP LL_COMP_INPUT_MINUS_IO5  // PA5
#define PHASE_C_COMP LL_COMP_INPUT_MINUS_IO4  // PA4
#define COMMON_COMP  LL_COMP_INPUT_PLUS_IO1   // PB4
```

Decoding the LL_COMP constants against `stm32l4xx_ll_comp.h:161-166`:

| Pin | LL constant | INMSEL[2:0] (bits 6-4) | INMESEL[1:0] (bits 27-26) |
|---|---|---|---|
| PB7 (IO2) | `LL_COMP_INPUT_MINUS_IO2` | `0b111` | `0b00` |
| PA4 (IO4) | `LL_COMP_INPUT_MINUS_IO4` | `0b111` | `0b10` |
| PA5 (IO5) | `LL_COMP_INPUT_MINUS_IO5` | `0b111` | `0b11` |

Both bit fields (the legacy 3-bit `INMSEL` and the 2-bit extension
`INMESEL`) must be written together to select IO3/IO4/IO5. The INMSEL
field alone can only encode IO1/IO2 plus the on-chip references — for the
extended I/O selections the chip uses INMESEL as the high bits.

**rm32's `mcu_l431/comparator.rs`** does neither:

```rust
pub const INMSEL: InmselMap = InmselMap {
    phase_a: 0b0101,
    phase_b: 0b0100,
    phase_c: 0b0011,
};

fn set_inmsel(&self, phase: u32) {
    let comp = unsafe { &*COMP::ptr() };
    let v = comp.comp2_csr.read().bits();
    comp.comp2_csr
        .write(|w| unsafe { w.bits((v & !(0xF << 4 | 0x3 << 8)) | (phase << 4)) });
}
```

Issues:

1. **Wrong values.** Decoded as INMSEL bit patterns: `0b0101` selects
   DAC ch2 (not PB7), `0b0100` selects DAC ch1 (not PA5), `0b0011` selects
   VrefInt (not PA4). The comparator is reading the wrong input pins for
   every phase.
2. **INMESEL never touched.** The write mask clears `0xF << 4 | 0x3 << 8`
   (bits 4-7 and 8-9), but the L431 INMESEL bits are at **27-26**. Even
   if the INMSEL pattern were corrected, IO4 and IO5 selections would
   never work without writing INMESEL too.
3. **Boot-time `comp_init.rs` has the same problem.** Initial COMP2
   configuration writes `comp2_inmsel().bits(0b101)` with the comment
   `// PB7 = IO2`. `0b101` is *not* IO2 — it's DAC ch2.

The board comment at the top of `comp_init.rs:1-6` says "L431 NEUTRON",
which is a different board (`HARDWARE_GROUP_L4_N` in AM32, with different
INM pin assignments). Strong evidence the Rust file was ported from
NEUTRON pin maps and never updated for VIMDRONES.

## Why the harness didn't catch this

Same pattern as the previous two bugs. The Python harness exercises pure
decode logic (`servo.compute`, DShot frame parsing, BEMF zero-cross
*state machine*), but the **register-level configuration that selects
which physical pin the comparator listens to** has no harness counterpart.
The harness can feed the BEMF state machine a stream of "rising edge
detected" / "falling edge detected" events directly — it cannot test
whether the chip's COMP2 peripheral is actually wired to the right pins.

This bug is invisible at the harness layer and only manifests as silent
nothingness on hardware — `zc=0` forever, no commutation lock, motor sits
dead.

## Resolution options

### A. Fix the L431 INMSEL values for the Vimdrones board only (recommended)

Update `InmselMap` for `mcu_l431` to encode both INMSEL[2:0] and
INMESEL[1:0]. Either:

- **A1**: extend `InmselMap` from `u32` slots into a struct holding both
  fields:
  ```rust
  pub struct InmselEntry { pub inmsel: u8, pub inmesel: u8 }
  pub struct InmselMap { pub phase_a: InmselEntry, pub phase_b: InmselEntry, pub phase_c: InmselEntry }
  ```
  Update `CompOps::set_inmsel` signature to take both fields.
- **A2**: pack both fields into a single `u32` (e.g., low 3 bits = INMSEL,
  bits 8-9 = INMESEL), update `set_inmsel` to decode and write both
  register fields.
- **A3** (smallest patch): just rewrite `mcu_l431/comparator.rs` and
  `mcu_l431/comp_init.rs` directly for the Vimdrones L4_B pin map,
  bypassing the `InmselMap` abstraction. Hardcode the three needed
  configurations. Lose generality but minimize risk.

For Vimdrones L431:
- Phase A floating (steps 1,4): comparator reads **PB7** → INMSEL=`0b111`,
  INMESEL=`0b00`
- Phase B floating (steps 2,5): comparator reads **PA5** → INMSEL=`0b111`,
  INMESEL=`0b11`
- Phase C floating (steps 3,6): comparator reads **PA4** → INMSEL=`0b111`,
  INMESEL=`0b10`

The `comp_init.rs` initial INMSEL needs to point at one of these (the
default starting step). Either of the three works for first power-up
since the first commutation step will set it correctly.

### B. Make INMSEL board-configurable from the YAML

Add a `bemf_pins` block to `boards/*.yaml` carrying the three INMSEL
patterns per phase. `build.rs` emits them as constants. Each L4 board
gets its own correct pin map, no per-MCU code changes needed.

Pros:
- ✅ Future-proof: NEUTRON, S50, Nano, and other L431 variants pick up
  their own pin maps from their YAML without touching Rust source.
- ✅ Captures the chip-pin-mapping concept in a single place that the
  harness can lint (e.g., assert pin numbers match phase output pins).

Cons:
- ⚠️ Bigger change. Affects the YAML schema, build.rs, the comparator
  abstraction in `rm32/src/`, and each per-MCU comparator module.
- ⚠️ Doesn't actually fix the harness gap — the per-board values still
  aren't checked against hardware.

### C. Quick patch + add harness coverage

Apply A3 (smallest patch) to unblock the L431 hardware, *and* in parallel
add a harness test that exercises the comparator pin-mapping code at the
register level — e.g., a mock that records all writes to `COMP2_CSR`
during a simulated commutation cycle and asserts each step writes the
expected INMSEL+INMESEL pattern.

This is the only resolution that closes the "register config has no
harness coverage" gap. It's also the largest commitment.

## Recommendation

**A3 (direct rewrite for Vimdrones L4_B)** to unblock the bench *now*.
Pattern matches what the other agent did with the realignment and
prescaler fixes — quick, AM32-faithful, ships behavior. Schedule
B (YAML-driven per-board pin maps) once we know rm32 actually drives
the motor end-to-end on at least one board, and have a second L431
board (e.g., NEUTRON) to validate the abstraction against.

After A3 lands, the expected behavior on next bench run:
- `zc` should increment from 0 to non-zero within a few commutation
  steps after `mode` enters `OldRoutine`
- `mode` should transition `OldRoutine → Running` on `MotorEvent::BemfLocked`
- The motor should actually rotate, not just twitch

If `zc` still stays at 0 after the fix, the next culprit is likely
either the EXTI line / NVIC priority for the COMP interrupt, or the
`changeCompInput()`-equivalent step-to-phase mapping in
`mcu_l431/comparator.rs:set_inmsel` getting called with wrong `phase`
values (off-by-one rotation).

## Worth a broader audit

This is the third bug in this class. Worth pattern-grepping AM32's
`HARDWARE_GROUP_L4_B` (and L4_A, L4_N, L4_C) defines and confirming
that every chip-specific constant has a 1:1 counterpart in
`mcu_l431/` with matching values. Candidates to check now:

- `PHASE_*_GPIO_LOW/HIGH` pin mapping (we currently believe these match —
  the motor twitches, so the FETs are switching the right pins; but
  verify against `phase.rs`)
- `CURRENT_ADC_CHANNEL`, `VOLTAGE_ADC_CHANNEL`, `ADC_CHANNEL_TEMP` — if
  these are off, ADC readings are garbage; not obviously broken yet
  because we're not using them for arming gates, but they'll bite later
- `IC_TIMER_*`, `INPUT_DMA_CHANNEL` (already verified during input
  capture bring-up)
