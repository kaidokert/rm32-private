# rm32 First Motor Spin on STM32L431 — Bringup Complete

**Date:** 2026-05-10
**Hardware:** Vimdrones L431 KCU6 ESC dev board (STM32L431KCU6, Cortex-M4F @ 80 MHz)
**Signal source:** Betaflight on STM32F411E-DISCOVERY, `motor_pwm_protocol = PWM`,
`motor_pwm_rate = 480` (480 Hz fast-PWM)
**Result:** rm32 fully end-to-end — protocol detect → arm → throttle decode →
commutation → BEMF lock → sustained motor rotation.

```
13:14:12  [bf] HOLD motor 0 = 1400 for 4.0s
13:14:15  [loop] mode=Running newinput=744 adj=744 duty=703 zc=10000 bemf_to_hap=0
```

`mode=Running` + `zc=10000` (saturated) = closed-loop BEMF-synced rotation.

## The bugs, in order

Every entry: name, where it manifested, what was wrong, where the fix landed.
Three of the items have full deep-dive companion docs.

### 1. Linker script not applied to thumbv7em-none-eabihf

`rm32_stm32/.cargo/config.toml` only set `-Tlink.x` rustflag for
`thumbv6m-none-eabi`. Building for L431 (`thumbv7em-none-eabihf`)
silently produced an ELF with all .text/.data/.bss DCE'd to zero size.

**Fix:** add target-specific blocks for `thumbv7em-none-eabi` and
`thumbv7em-none-eabihf` mirroring the M0 entry.

### 2. PRIMASK enabled at main entry — DMA TC panic

After Cortex-M reset, **PRIMASK = 0 (IRQs enabled)**, and `cortex-m-rt`
doesn't disable them before `main()`. The author of `main.rs:198` intended
`cortex_m::interrupt::enable()` to be the *first* enable, but NVIC
unmasks during init were already live, and `receive_dshot_dma()` armed
DMA against PA2 with the Arduino PWM signal already feeding it. First
DMA TC fired before `init_isr_state()` ran, the ISR took an empty
`ISR_STATE`, and panicked.

**Fix:** explicit `cortex_m::interrupt::disable()` as the first line of
`main()`, paired with the existing enable at the end.

### 3. `Sounds::play_startup` race with the IRQ-disable

`play_startup` did `disable_irq → play notes → enable_irq` —
unconditionally. With fix #2 in place, IRQs were *disabled* on entry, and
the unconditional enable mid-init re-opened the same DMA TC panic window.
The instruction gap between `play_startup`'s tail enable and the next
instruction in main was enough for a pending ISR to fire and panic.

**Fix:** remove the `disable_irq`/`enable_irq` bracket from
`play_startup`. Document the contract that play methods leave global
IRQ state untouched. `main.rs` now controls global IRQs from start to
finish.

### 4. `start_watchdog` LSI / IWDG init order — silent deadlock

`mcu_l431::system::start_watchdog` followed a sequence that works on chips
where IWDG is hardware-started or LSI is always on:

```rust
KR = 0x5555;            // unlock
PR = prescaler;
RLR = reload;
while SR & 0x03 {}      // wait for PVU/RVU to clear ← deadlock
KR = 0xCCCC;            // start IWDG
KR = 0xAAAA;            // refresh
```

L431's default option byte is `IWDG_SW = 1` (software-start), so LSI is
not running at boot. `SR.PVU` / `SR.RVU` only clear after written values
propagate at LSI rate — but with IWDG inactive and LSI off, they never
clear. The busy-wait deadlocks forever, no panic, no log.

**Fix:** enable LSI explicitly via `RCC_CSR.LSION` and wait for `LSIRDY`
before the IWDG sequence; reorder so `KR = 0xCCCC` activates IWDG
*before* the `SR` busy-wait.

### 5. DMA buffer relocation #1 — capture buffer moves with the struct

`GenericCapture<...>` has its DMA destination buffer **inline as a struct
field** (`dma_buf: [u32; 64]`). `receive_dshot_dma` writes the buffer's
*current* address to `DMA1_CMAR5`. `main.rs` originally called it before
`init_isr_state(isr_state)` — but that call moves `hal` (containing the
buffer) into `IsrState`, then `init_isr_state` moves `IsrState` again into
the static. Each move is a memcpy. The buffer's live address changes;
DMA's `CMAR5` becomes stale. DMA writes capture timestamps to the freed
stack location while `dma_buffer()` reads from the post-move struct.

**Fix:** defer `receive_dshot_dma()` until after `init_isr_state()`,
accessing the freshly installed state via `with_isr_state`.

### 6. DMA buffer relocation #2 — IsrCell lazy-init takes the state again

The architecture intentionally moves the state from `ISR_STATE`
(`Mutex<RefCell<Option<...>>>`) into `ISR_LOCAL`
(`UnsafeCell<Option<...>>`) on the first ISR call, via `IsrCell::get`'s
`take_isr_state()`. That move *also* changes the buffer's address, but it
happens after main has armed the DMA against the `ISR_STATE` location.

**Fix:** re-arm DMA inside `IsrCell::get` immediately after the move
into `ISR_LOCAL`. The state is now at its final permanent address and
subsequent moves don't happen.

### 7. Servo PWM buffer alignment — missing AM32 buffer-size flip

> Full deep-dive: `BRINGUP_NOTES_L431.md`

The 2-edge DMA buffer can be either `[rise, fall]` (decodable) or
`[fall, rise]` (misaligned, decoder waits for next call). Once the L431
EXTI handler unconditionally re-armed with size 2, the alignment
relative to the live PWM signal stayed constant and we were locked in
the misaligned state forever. AM32 handles this by setting
`buffersize = 3` whenever the pin is high at handler time, capturing
3 edges next cycle and shifting the alignment by one. The Rust port
dropped the dynamic buffer size.

**Fix:** added `CaptureSize::ServoRealign` (NDTR=3) to the decoder's
return value, all 4 MCU EXTI handlers consume it via `next_capture.ndtr()`
when re-arming. First instance of "decoder returns HAL directives
through `TransferActions`" — re-used by every fix below.

### 8. Servo PWM timer prescaler — also lost on detect

> Full deep-dive: `LOST_PRESCALER.md`

After fix #7, the buffer was correctly aligned and `dma_buffer[1] -
dma_buffer[0]` produced clean numbers — but at 5.71 MHz timer ticks, not
microseconds. AM32's `signal.c:233` swaps the input-capture prescaler
when servo is detected (`ic_timer_prescaler = CPU_FREQUENCY_MHZ - 1` →
1 MHz timer → 1 tick = 1 µs), so subsequent captures match the
microsecond-based threshold check `> 800 && < 2200`. The Rust port
hardcoded prescaler at 13 (5.71 MHz) — fine for DShot resolution, way
too fast for servo µs-thresholds. Pulse 1.075 ms arrived as 6142 ticks,
got rejected as `>= 2200` → `OutOfRange`.

**Fix:** extended `CaptureSize` to `CaptureConfig { ndtr, prescaler:
Option<u16> }`. `process()` now takes `cpu_mhz: u8` so it can compute
the right servo prescaler. Each MCU's EXTI handler writes PSC + pulses
EGR.UG before re-arming DMA when a prescaler change is requested.

### 9. Wrong BEMF comparator pins — third instance of the same class

> Full deep-dive: `WRONG_BEMF_PINS_L431.md`

After fixes #7–#8, throttle decoded correctly, the state machine
transitioned `Armed → OldRoutine`, PWM duty was driven through the phase
outputs. Motor twitched but didn't spin — `zc = 0` for the entire run.
rm32's hardcoded INMSEL constants for L431 were:

```rust
phase_a: 0b0101,  // selects DAC ch2 — NOT PB7
phase_b: 0b0100,  // selects DAC ch1 — NOT PA5
phase_c: 0b0011,  // selects VrefInt — NOT PA4
```

Plus `set_inmsel` only wrote bits[7:4] of `COMP2_CSR` and never touched
the `INMESEL[1:0]` extension at bits[27:25] needed to disambiguate
IO3/IO4/IO5 on L431. The leading comment `"L431 NEUTRON"` revealed the
file was ported from a different board's pin map.

**Fix:** moved BEMF pins to board YAML files (`bemf_pins.phase_a: PB7`
etc.), `build.rs` resolves symbolic pin names to MCU-specific packed
register values via per-family mapping tables, `set_inmsel` was
extended to write both INMSEL + INMESEL fields. Affected all 14 board
configs.

### 10. SYSCFGEN — final blocker

After fix #9, `set_inmsel` had the right values to write, but
**writes to `COMP2_CSR` were silently dropped**. RTT log:

```
[comp_init] writing COMP2_CSR=0x00000071 (inmsel=7 inmesel=0)
[comp_init] readback COMP2_CSR=0x00000000   ← write didn't stick
```

On STM32L4, the COMP peripheral shares its register clock with SYSCFG.
Without `RCC_APB2ENR.SYSCFGEN`, the COMP_CSR register is unclocked and
all writes are dropped — nothing latches, comparator never enables, EXTI
line 22 never gets an event, `zc` never increments.

**Fix:** added `rcc.apb2enr.modify(|_, w| w.syscfgen().set_bit())` in
`init_comp2` before any COMP register access.

## The shape of the bug class

Six of the ten bugs (4, 7, 8, 9, 10 plus the relocation pair 5+6) are
the same shape: **MCU-specific peripheral configuration that the C→Rust
port either dropped, copied from the wrong board, or got the bit
positions wrong on**. None of them were caught by the Python integration
harness, because the harness is structured to test pure logic
(`servo.compute`, `transfer::process`, BEMF state machine) against
synthetic input vectors. It cannot see register configuration, can't
verify which physical pin a peripheral is wired to, and can't model a
peripheral that's silently unclocked.

Realistic next-pass audit list (also called out in the per-bug docs):

- **Other L431 peripheral clock enables.** The SYSCFGEN miss was the
  newest entry in this class; `init_comp2` is unlikely to be the only
  place a clock enable is missing. ADC, DAC, USART (if used outside
  what we exercised), TIM3/4 if they appear later — all worth a sweep.
- **Other AM32 ISR-side feedback into HAL state.** Anywhere `signal.c`
  or `dshot.c` writes a peripheral-config global (`buffersize`,
  `ic_timer_prescaler`, `out_put`, `buffer_padding`, etc.) and the
  Rust port doesn't have a counterpart in `mcu_xxx/interrupts.rs`.
- **The other three MCU port targets** (G071, F051, G431) likely have
  some subset of these issues too. The realignment + prescaler fixes
  were applied to all four MCUs as part of fixes #7 and #8, but
  per-board comparator pin maps and per-MCU SYSCFG-equivalents are not
  yet validated against hardware on those targets.

## Test strategy gap

The Python harness validates `transfer::process`, `servo::compute`,
DShot frame parsing, motor mode state transitions, and the like —
everything that runs in pure Rust against an in-memory simulator. None
of those bugs would have been caught by the harness because none of
them are in pure logic. The bug class is *register-level peripheral
configuration*, which the harness has no model for and no way to assert
against.

A realistic addition: a hardware-loop test target. Build the firmware,
flash it to a known board, run a fixed BF-driven motor sequence, scrape
RTT for the canonical state-machine progression
(`Disarmed → Armed → OldRoutine → Running` with `zc > 0`), assert the
trace matches expectations. That would catch the entire class going
forward, at the cost of needing an actual ESC + motor + battery on the
test rig. But none of these bugs would survive that gate.

## Patches landed locally vs. on `private`

Through the bringup, four fixes were applied via the
"private branch + other agent" cycle (the bug docs were written here,
the fixes came back through `git pull private`):

- Buffer-alignment fix (#7) → commit `5b795d9`
- Prescaler fix (#8) → commit `1f50229`
- BEMF pin map (#9) → commit `bda29c0`
- (Plus several unrelated cleanup commits in `priv_bringup`.)

Six fixes are local-only and may be worth pushing back to private:

1. `.cargo/config.toml` thumbv7em targets (#1)
2. `interrupt::disable()` at top of main (#2)
3. `Sounds::play_startup` no longer touches IRQs (#3)
4. `start_watchdog` LSI + reorder (#4)
5. Deferred `receive_dshot_dma` + `IsrCell::get` post-move re-arm (#5+#6)
6. **`SYSCFGEN` enable in `init_comp2` (#10)** — this is the latest one
   and the difference between "pin map fixed but writes dropped" and
   "motor actually spins"

## Acknowledgements

The other agent did the structurally cleaner work of fixes #7, #8, and
#9 — extending `TransferActions` into a directive channel for HAL
re-arm parameters, then YAML-izing per-board comparator pin maps. Each
of those was substantially better than what was proposed in the bug
docs, and each generalizes to future MCUs and boards. The pattern of
"this side identifies + documents the bug, that side designs the
broader fix" worked well across three iterations.
