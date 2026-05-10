# rm32 STM32L431 Bring-up — Bug Journey and Servo PWM Misalignment

This doc captures the first hardware bring-up of `rm32` against an actual ESC
(Vimdrones L431, STM32L431KCU6). It walks through the bugs that surfaced,
ending with a non-trivial divergence from the AM32 C reference — a missing
piece of the input-capture state machine that the Python harness could not
catch by design.

## Hardware setup

- **Target**: Vimdrones L431 KCU6 ESC dev board (STM32L431KCU6, 256 KB flash,
  64 KB RAM, Cortex-M4F at 80 MHz)
- **Bootloader**: `AM32_L431_BOOTLOADER_PA2_V18` at `0x08000000` (4 KB region)
- **Application**: `rm32_firmware` at `0x08001000`
- **EEPROM**: at `0x0800F800` (2 KB)
- **Signal source**: Betaflight on STM32F411E-DISCOVERY, `motor_pwm_protocol = PWM`,
  `motor_pwm_rate = 480` (480 Hz fast-PWM), motor 1 on PB1
- **Debug probe**: ST-LINK V3 over SWD via `probe-rs`
- **Build**: `cargo build --release --features stm32l431 --no-default-features
  --target thumbv7em-none-eabihf`, `BOARD=boards/vimdrones_l431.yaml`

The bootloader gates that need to pass:
1. `CHECK_SOFTWARE_RESET` — last reset must be hardware (NRST or power cycle).
2. `CHECK_EEPROM_BEFORE_JUMP` — first byte at `0x0800F800` must equal `0x01`.

A lingering AM32 install satisfied (2) already; (1) requires power-cycle, since
`probe-rs reset` issues `SYSRESETREQ` (software reset) and the bootloader
deliberately stays put on those.

## Symptom

After flash + power-cycle: rm32 boots, plays the startup tones, then sits idle.
Throttle commands (Betaflight CLI `motor 1 1100`, or sweeping with the Arduino
PWM source) produce **no motor response at all**. No chirp loop, no panic
indicator, no smoke. Just silence.

## Pre-flight bugs — the path to "rm32 boots and runs"

Before the actual servo-decode bug, several distinct bugs had to be fixed just
to get rm32 to a steady-state main loop. Each one failed silently in a
different mode, so they're worth recording.

### 1. Linker script not applied to thumbv7em-none-eabihf

`rm32_stm32/.cargo/config.toml` set `rustflags = ["-C", "link-arg=-Tlink.x"]`
under `[target.thumbv6m-none-eabi]` only. Building for the L431
(`thumbv7em-none-eabihf`) silently linked **without the `cortex-m-rt` linker
script**, producing an ELF whose `.text` / `.data` / `.bss` sections were all
DCE'd to zero size. Symptom: `arm-none-eabi-size` reports `0 0 0`; the binary
is just debug info.

**Fix**: add target-specific blocks for `thumbv7em-none-eabi` and
`thumbv7em-none-eabihf` mirroring the M0 entry.

### 2. Interrupts enabled too early — DMA TC panic

The author of `main.rs:198` intended `cortex_m::interrupt::enable()` at the
end of init to be the *first* moment IRQs go live. But after a Cortex-M reset
**`PRIMASK = 0` (IRQs enabled by default)**, and `cortex-m-rt` doesn't disable
them before `main()`. So:

1. `mcu_l431::init::init` ran NVIC unmasks for `DMA1_CH5` etc. while IRQs
   were live.
2. `hal.input.receive_dshot_dma()` armed DMA against PA2.
3. The Arduino PWM sweep on PA2 fed the DMA buffer, transfer-complete fired.
4. `DMA1_CH5` ISR called `IsrCell::get`, found `ISR_STATE` empty (because
   `init_isr_state` hadn't run yet), `expect()` panicked.
5. Panic handler set FETs off and halted in a `nop` loop.

**Fix**: explicit `cortex_m::interrupt::disable()` as the very first line of
`main()`, paired with the existing enable at the end.

### 3. `Sounds::play_startup` unconditional `enable_irq` at tail

`rm32::sounds::Sounds::play_startup` did `disable_irq → play notes →
enable_irq` — assuming IRQs were *enabled* on entry, restoring on exit. With
the fix in (2) IRQs are now *disabled* on entry. The unconditional
`enable_irq` at tail re-enabled them mid-init, opening the same DMA TC panic
window as before. Even disabling IRQs again on the very next line in
`main.rs` was too slow — the ISR fires in the gap between the two
instructions.

**Fix**: remove the disable/enable bracket from `play_startup` (leave IRQs in
whatever state the caller had). Document the contract. `main.rs` now controls
the global IRQ state from start to finish.

### 4. `start_watchdog` busy-wait deadlock on L431

`mcu_l431::system::start_watchdog` mirrored a sequence common in HAL libraries
for chips where IWDG is hardware-started or LSI is always on:

```rust
KR = 0x5555;            // unlock
PR = prescaler;
RLR = reload;
while SR & 0x03 {}      // wait for PVU/RVU to clear
KR = 0xCCCC;            // start IWDG (also forces LSI on)
KR = 0xAAAA;            // refresh
```

On L431 the default option byte is `IWDG_SW = 1` (software-start), so LSI is
**not running at boot**. `SR.PVU` / `SR.RVU` only clear after the written
prescaler/reload values propagate at LSI rate — but with IWDG inactive and
LSI off, that never happens. The busy-wait deadlocks forever, no panic, no
log.

**Fix**: enable LSI explicitly via `RCC_CSR.LSION` and wait for `LSIRDY`
before the IWDG sequence; also reorder so `KR = 0xCCCC` activates IWDG
*before* the `SR` busy-wait, matching the STM32 HAL pattern.

### 5. DMA buffer relocation #1 — capture buffer moves with the struct

`GenericCapture<...>` has the DMA destination buffer **inline as a struct
field** (`dma_buf: [u32; 64]`). `receive_dshot_dma` writes the buffer's
*current* address to `DMA1_CMAR5`. `main.rs` originally called it *before*
`init_isr_state(isr_state)` — but that call moves `hal` (containing the
buffer) into `IsrState`, and `init_isr_state` moves `IsrState` again into a
`Mutex<RefCell<Option<...>>>` static. Each move is a `memcpy`. The buffer's
live address changes; DMA's `CMAR5` becomes stale.

Effect: DMA writes capture timestamps to a freed stack location while
`dma_buffer()` reads from the post-move struct. The Rust side sees an
all-zero buffer forever and never decodes anything.

**Fix #1 (partial)**: defer the `receive_dshot_dma()` call until *after*
`init_isr_state`, accessing the freshly installed state via `with_isr_state`.
This made one of the two moves harmless — but...

### 6. DMA buffer relocation #2 — `IsrCell` lazy-init takes the state

The architecture intentionally moves the state from `ISR_STATE`
(`Mutex<RefCell<Option<...>>>`) into `ISR_LOCAL` (`UnsafeCell<Option<...>>`)
on the first ISR call, via `IsrCell::get`'s `take_isr_state()`. That move
**also** changes the buffer's address, but it happens *after* main has armed
the DMA against the `ISR_STATE` location.

So even with fix #5, the very first DMA TC ISR moves the state into
`ISR_LOCAL`, and from that point on DMA writes to the now-stale `ISR_STATE`
address while every read goes via `ISR_LOCAL`. The buffer-zero symptom
persisted exactly as before.

**Fix #2**: re-arm DMA inside `IsrCell::get`, immediately after the move
into `ISR_LOCAL`. The state is now at its final permanent address and
subsequent moves do not happen.

After all six fixes, rm32 reaches its main loop, RTT logs flow,
and `transfer::process` reports `InputDetected(Servo)` against the live
PWM signal. The motor still does not spin.

## The current bug — servo PWM buffer misalignment

### What we see

With RTT instrumentation in `handle_exti_frame`, every captured frame logs:

```
[exti] frame#... pin_high=true input_set=true servo_pwm=true buf[0..4]=A B X Y
```

`pin_high=true` for **every single frame**. `input_set` and `servo_pwm` are
correctly latched. `signal_timeout` stays at `65535` (max), `newinput`
stays at `0`. The motor never sees a throttle command.

### Why pin_high=true forever blocks decoding

`rm32::transfer::process` (servo branch) is:

```rust
} else if servo_mode {
    if input_pin_high {
        // Rising edge — wait for falling to get pulse width
    } else if dma_buffer.len() >= 2 {
        let pulse = dma_buffer[1].wrapping_sub(dma_buffer[0]) as u16;
        action = match self.servo.compute(pulse, ...) { ... };
    }
}
```

The contract is:

- **`pin_high = false` at handler time** → the most recent captured edge was
  a falling edge. The 2-entry DMA buffer is `[rise, fall]`. Compute pulse
  width as `fall - rise`. Decode succeeds.
- **`pin_high = true` at handler time** → the most recent captured edge was
  a rising edge. The 2-entry DMA buffer is `[fall, rise]`, where `fall` is
  the falling edge of the *previous* PWM period. Don't decode this buffer
  (`buf[1] - buf[0]` would be the LOW gap, not the HIGH pulse). Wait for
  the next handler call, which by assumption will arrive aligned the right
  way.

The implicit assumption: alignment will eventually flip. **But it doesn't.**

The L431 `EXTI15_10` handler unconditionally re-arms DMA with `NDTR=2` after
each cycle:

```rust
let sz = if shared.servo_pwm() { 2u32 } else { 32 };
dma.cndtr5.write(|w| w.bits(sz));
dma.ccr5.modify(|r, w| w.bits(r.bits() | 1));
tim15.cr1.modify(|r, w| w.bits(r.bits() | 1));
```

DMA captures exactly 2 edges per cycle — that's exactly one PWM period
(rising + falling). The relative phase between the EXTI handler and the live
PWM signal stays roughly constant from cycle to cycle. If we land in the
`[fall, rise]` alignment once, we stay there. `pin_high=true` is the steady
state.

### Why this isn't aligned with AM32

AM32's `transfercomplete()` in `Src/signal.c:156-164` handles this
explicitly:

```c
if (servoPwm == 1) {
    if (getInputPinState()) {
        buffersize = 3;          // pin HIGH: arm DMA for 3 captures next time
    } else {
        buffersize = 2;          // pin LOW: buffer is [rise, fall], decode it
        computeServoInput();
    }
    receiveDshotDma();
}
```

When the pin is high at handler time (`[fall, rise]` alignment), AM32 sets
`buffersize = 3` so the next DMA cycle captures three edges:
`[fall, rise, fall]`. The next handler call sees `pin_high = false`, sets
`buffersize` back to 2, and decodes `dma_buffer[1] - dma_buffer[0]` —
which is now `rise - fall` of the **completed previous pulse**.
One-cycle re-alignment, then locked in.

The Rust port lost this. The `if input_pin_high { /* wait */ }` arm in
`transfer::process` describes the intent (wait for the next call), but
nothing in the HAL plumbing actually changes behavior to *cause* the next
call to be aligned. The dynamic buffer size is gone.

### Why the Python harness didn't catch it

The harness drives `rm32::transfer::process` directly with synthetic
`dma_buffer` arrays. Those arrays are constructed pre-aligned —
`[rise, fall]` for servo decode tests, full DShot frames for DShot tests.
The harness never feeds in a `[fall, rise]` buffer because that's a
*hardware-level* property: it's a function of when the DMA arm happens
relative to the live PWM signal, and is recovered (in AM32) by the
MCU-specific ISR layer dynamically adjusting `buffersize`.

The harness can validate the pure decode logic (`servo.compute`, DShot
frame parsing, etc.) but cannot validate the **HAL plumbing that controls
how many edges land in the buffer per cycle**. That plumbing has no
counterpart in the cross-platform tests because there's no portable
abstraction for "next NDTR value" — it lives in `mcu_xxx/interrupts.rs`
files that aren't part of the harness build.

In short: the C→Rust port faithfully copied `transfer::process` (the pure
decoder), but lost the dynamic-buffer-size feedback loop that lived in the
ISR layer. The harness would not detect this gap because the gap is
between the harness boundary and the chip.

### Status of fix-test parity going forward

This bug is the first of what's likely a class. Anywhere the AM32 C ISR
layer **mutates HAL state in response to runtime signals** (buffer size,
prescaler, edge polarity), the Rust port may have lost it if the ISR layer
was rewritten without close reference to AM32's `transfercomplete()` and
sibling ISRs. Worth grep-auditing AM32's `signal.c`, `dshot.c`, and the
per-MCU `IO.c` files for assignments to `buffersize`, `ic_timer_prescaler`,
`out_put`, etc., and confirming each assignment has a counterpart in the
corresponding `mcu_xxx/interrupts.rs`.

## Proposed resolutions

### Option A — Mirror AM32's dynamic buffer size (recommended)

Smallest patch, faithful to the C reference. In `handle_exti_frame`
(or in the L431 `EXTI15_10` ISR's re-arm code, depending on where we want
the policy to live), look at `input_pin_state()` after processing and emit
either `NDTR=2` (pin LOW, decode happened) or `NDTR=3` (pin HIGH, request
re-alignment).

This needs:
- A way to communicate the desired NDTR from `handle_exti_frame` back to the
  per-MCU EXTI re-arm code. Two options:
  - **A1**: read pin state directly inside the L431 EXTI re-arm code, after
    `handle_exti_frame` returns. Simplest. Slightly redundant (pin is read
    in the handler too) but cheap (one GPIO IDR read).
  - **A2**: add a `desired_ndtr: u8` field to `SharedState` (or to a
    new `CaptureControl` substruct). `handle_exti_frame` writes it; EXTI
    re-arm reads it. Cleaner abstraction; matches AM32's globals model.

Tradeoffs:
- ✅ Matches AM32 byte-for-byte semantics. Predictable behavior under any
  PWM rate, any duty cycle, any phase.
- ✅ Localized to the L431 (or any MCU) EXTI plumbing. `transfer::process`
  unchanged.
- ✅ No new state to test; the harness's existing decode tests stay valid.
- ⚠️  Still requires applying the same fix to every other MCU's EXTI handler
  (G071, F051, G431). Not a blocker — it's identical code at each site.
- ⚠️  Doesn't fix the underlying *architectural* issue: the HAL plumbing has
  decoder-aware logic. Future ports of new logic types (DShot bidir, EDT,
  etc.) will face the same harness-blindness.

### Option B — Capture full periods (4 edges)

Always arm DMA for `NDTR=4` in servo mode. Each cycle captures
`[X, Y, Z, W]` — definitely contains both edges of at least one full
pulse, regardless of starting alignment. Decoder does its own alignment
search on every call (look for `Y - X ≈ pulse_high` vs `Z - Y ≈ pulse_high`,
or use the pin state to disambiguate).

Tradeoffs:
- ✅ Robust to any signal phase / glitch / noise; one cycle always decodes.
- ✅ Could move *all* alignment logic into `transfer::process`, fully
  testable from the harness — no MCU plumbing involved.
- ⚠️  Doubles the per-cycle latency (each EXTI fires every full period
  instead of every half period). For 50 Hz PWM that's 20 ms per update vs
  10 ms. For 480 Hz it's 2 ms vs 1 ms. Probably fine, but it's a regression
  vs AM32's behavior.
- ⚠️  Doubles the buffer entries we have to copy in the harness for tests.
  Need to update existing servo decode test vectors.
- ❌  Diverges from AM32 reference. Future bugs will be harder to find by
  side-by-side comparison.

### Option C — Decode both alignments in `transfer::process`

Stay at `NDTR=2`. Push the alignment handling fully into the decoder: when
`pin_high=true`, the buffer is `[fall, rise]`; remember the previous
buffer's `rise` value across calls; compute high pulse as
`current.fall - previous.rise`. (Or: trust the period inferred during
detection and back-compute high pulse as `period - (current.rise - current.fall)`.)

Tradeoffs:
- ✅ Pure decoder change, fully harness-testable.
- ✅ No HAL-layer changes.
- ❌ Requires `TransferState` to carry alignment-state between calls
  (a new mutable field). This is exactly the kind of state the C version
  pushes into the HAL via `buffersize` instead — diverges philosophically.
- ❌ Edge cases get gnarly: signal loss, glitches, missed captures all need
  state-machine handling that AM32 sidesteps by just letting one cycle
  re-align with `buffersize=3`.
- ❌ Still leaves the broader concern of HAL-layer feedback loops uncovered.

### Recommendation

**Option A1** — mirror AM32's `getInputPinState() ? 3 : 2` decision in the
L431 (and other MCU) EXTI re-arm code, using a direct GPIO read at the
re-arm site. Smallest patch, exactly what AM32 does, leaves the harness
contract intact, and surfaces the missing piece in a way that future
audits against AM32 can compare against. We can revisit a more formal
abstraction (Option A2 with a shared field, or a generic
`CaptureControl` API in the rm32 lib) after we have the L431 actually
spinning a motor and can confirm the behavior matches AM32 end-to-end.
