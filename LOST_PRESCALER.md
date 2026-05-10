# rm32 L431 — Lost Dynamic Input-Capture Prescaler

This is a follow-on bug to the buffer-alignment issue documented in
`BRINGUP_NOTES_L431.md`. After the realignment fix landed (the
`CaptureSize::ServoRealign` patch), the buffer is correctly `[rise, fall]`,
`pin_high` consistently reports `false` at handler time, and `dma_buffer[1]
- dma_buffer[0]` produces a clean number for the pulse high-time. **The
motor still does not spin.** RTT consistently shows:

```
input_set=true servo_pwm=true newinput=0 armed=false sig_to=65535
```

The captured pulse looks fine on the wire, decoding does not propagate to
`newinput`, no motor command, no arming. This document explains why.

## Evidence on the chip

With Betaflight `motor_pwm_protocol = PWM`, `motor_pwm_rate = 480`, motor 1
on PB1 (idle PWM), live captures via probe-rs while rm32 is running:

```
[exti] frame#201  pin_high=false  buf[0..4]=56362 62504 4573 15930
[exti] frame#401  pin_high=false  buf[0..4]=18523 24664 4573 15930
[exti] frame#601  pin_high=false  buf[0..4]=28451 34591 31627 37769
```

`buf[0]` and `buf[1]` are the rising and falling edge timestamps of the most
recent PWM pulse:

```
buf[1] - buf[0] = 62504 - 56362 = 6142 ticks
```

Same pulse on every cycle, ~6142 ticks. That's the captured raw value handed
to `rm32::servo::ServoState::compute(pulse_width, ...)`.

## What `compute` does with it

`rm32/src/servo.rs:91-92`:

```rust
// Validate pulse range (800-2200µs)
if pulse_width <= 800 || pulse_width >= 2200 {
    return ServoResult::OutOfRange;
}
```

`6142 >= 2200` → `ServoResult::OutOfRange` → action `TransferAction::None` →
`zero_input_count = 0`, `signal_timeout` not reset, `newinput` not updated.
Forever.

The thresholds `800` and `2200` are **microseconds**, matching the AM32 C
reference (`Src/signal.c:51`):

```c
if (((dma_buffer[1] - dma_buffer[0]) > 800) && ((dma_buffer[1] - dma_buffer[0]) < 2200)) {
```

In AM32 those thresholds work because `dma_buffer` values arrive in µs.
In rm32 on the L431 they don't — the captures arrive in **timer ticks at
5.71 MHz**.

## Why the L431 timer ticks at 5.71 MHz

`rm32_stm32/src/mcu_l431/input_capture.rs:125`:

```rust
GenericCapture::new(L431Dma, L431Timer { prescaler: 80 / 6 }, L431Pin)
```

`80 / 6 = 13` (integer division). With L431 SYSCLK = 80 MHz and APB2 at
80 MHz (no division), the timer kernel clock is 80 MHz. The TIM15 internal
counter runs at:

```
80 MHz / (PSC + 1) = 80 MHz / 14 ≈ 5.714 MHz
```

So 1 timer tick ≈ 175 ns, and 1 ms ≈ 5714 ticks. Our captured pulse:

```
6142 ticks * (175 ns/tick) ≈ 1.075 ms = 1075 µs
```

That matches expected idle PWM (~1.0 ms pulse) well — **the wire signal is
fine; only the unit handed to the decoder is wrong.**

This prescaler is a sensible default for **DShot** decoding: at DShot300
each bit is 3.33 µs ≈ 19 ticks at 5.71 MHz, which gives the input capture
plenty of resolution to distinguish 0-bit (~25% high) vs 1-bit (~75% high).
At a 1 MHz timer that bit measures only 3 ticks — insufficient.

For **servo PWM**, where pulses are 1000–2000 µs and the decoder uses
1 µs-precision thresholds, the same prescaler is far too fast. The
captured numbers don't fit any unit-consistent interpretation against
the µs-based threshold table.

## What AM32 does — and the rm32 port doesn't

AM32's `signal.c` switches `ic_timer_prescaler` *based on the detected
protocol* and re-arms the timer with the new value:

`Src/signal.c:checkDshot` (lines 203–227, abridged):

```c
if (smallestnumber >= 1 && smallestnumber < 4 && ...) {
    ic_timer_prescaler = 0;            // tightest resolution for DShot600
    ...
    dshot = 1;
    buffersize = 32;
    inputSet = 1;
}
if (smallestnumber >= 4 && smallestnumber <= 8 && ...) {
    ic_timer_prescaler = 1;            // /2 for DShot300
    ...
}
```

`Src/signal.c:checkServo` (lines 229–237):

```c
if (smallestnumber > 200 && smallestnumber < 20000) {
    servoPwm = 1;
    ic_timer_prescaler = CPU_FREQUENCY_MHZ - 1;   // 79 at 80 MHz
    buffersize = 2;
    inputSet = 1;
}
```

The reconfiguration takes effect on the next `receiveDshotDma()` call,
which writes `ic_timer_prescaler` into the timer's PSC register. After
detection lands on servo, the timer reconfigures to **1 MHz / 1 µs per
tick** and from that point onward every captured pulse arrives in µs —
exactly what `computeServoInput()`'s `> 800 && < 2200` check expects.

`rm32::transfer::process` has no equivalent. It does emit a request via
`TransferActions.next_capture` for buffer-size adjustment (the recent
`CaptureSize` patch) — but **not** for prescaler. `L431Timer::prescaler`
is set once at construction (`80/6 = 13`) and never changes. The L431
EXTI re-arm path reads `next_capture.ndtr()` to set the next NDTR but
makes no PSC writes.

## Why the harness missed this

Same root cause as the buffer-alignment bug, deeper layer. Per
`BRINGUP_NOTES_L431.md`:

> the harness can validate the pure decode logic [...] but cannot validate
> the **HAL plumbing that controls how many edges land in the buffer per
> cycle**.

Now extend that: it also can't validate the HAL plumbing that controls
*at what rate* the captures land. The Python tests feed `dma_buffer`
arrays already-in-microseconds — implicitly assuming the timer is at
1 MHz. The L431 build runs the timer at 5.71 MHz. The decoder is correct.
The HAL is correct (for DShot resolution). The contract between them is
broken, and there is no test that exercises both halves together against
a real or modeled timer prescaler.

This is now the **second** instance of "AM32 ISR-side feedback into HAL
state, lost in port" — the first being the 2/3 buffer size, this being
the prescaler. The class of bug is real and worth a sweep:

> Anywhere AM32 mutates `buffersize`, `ic_timer_prescaler`,
> `output_timer_prescaler`, `out_put`, `buffer_padding`, or any other
> peripheral-config global from `signal.c` / `dshot.c` / per-MCU `IO.c`,
> rm32 may be missing the corresponding HAL feedback path.

## Resolution options

### A. Mirror AM32 — dynamic prescaler swap on detection

When the decoder detects a protocol, it requests both the next buffer
size **and** the next timer prescaler. The L431 (and other MCU) EXTI
re-arm path applies both before re-enabling DMA + TIM.

Implementation sketch:

- Replace `CaptureSize` (Dshot/Servo/ServoRealign → NDTR=32/2/3) with
  a `CaptureConfig` struct that carries both NDTR and PSC, *or* extend
  `CaptureSize` to carry the prescaler value implicit in its variant.
- `transfer::process` already knows servo vs DShot vs detection mode —
  it has all the information it needs to pick the right prescaler.
- Each MCU's EXTI handler writes the requested PSC into the input-capture
  timer's PSC register (and `EGR.UG` to apply immediately) before
  re-enabling DMA.

Pros:
- ✅ Faithful to AM32. Future audits against AM32's `signal.c` can
  pattern-match.
- ✅ Closes the second instance of the harness-blind class with the same
  shape of fix as the first. The pattern becomes generalizable: any
  HAL-mutation feedback can ride the `TransferActions` channel.
- ✅ Decoder thresholds stay unchanged. Same code that's been validated
  by the harness's microsecond-based test vectors.

Cons:
- ⚠️  Prescaler-swap semantics are subtler than buffer-size swap: an
  in-flight capture cycle running at the old prescaler must complete (or
  be discarded) before the new prescaler takes effect. AM32 handles this
  by deferring the apply to the next `receiveDshotDma()` call. rm32
  needs the same. One missed transient frame at the swap moment is
  fine; sustained drift is not.
- ⚠️  Each MCU's EXTI handler grows by a few register writes. Worth
  centralizing in a HAL trait method.

### B. Decode in ticks, pass `ticks_per_us` to the decoder

Keep the L431 timer at its DShot-friendly 5.71 MHz. Plumb the timer's
ticks-per-µs ratio (or the inverse) into `ServoState::compute` and the
calibration paths, so the µs thresholds are scaled to ticks at decode
time:

```rust
let pulse_us = (pulse_ticks * scale_num) / scale_den;
if pulse_us <= 800 || pulse_us >= 2200 { return OutOfRange; }
```

Pros:
- ✅ Pure decoder change. Fully harness-testable: tests pass `ticks_per_us`
  alongside synthetic buffers and the harness validates both µs-input and
  tick-input cases.
- ✅ HAL plumbing untouched. No risk of breaking DShot timing.
- ✅ Symmetric with how a clean reimplementation might do it from scratch:
  decoder is unit-aware, HAL just provides raw timestamps.

Cons:
- ❌ Diverges from AM32 structure. Future bugs will be harder to find by
  side-by-side comparison.
- ❌ Adds a multiply-divide (or tick-table lookup) to every servo decode
  call — negligible in practice but a real change.
- ❌ Doesn't fix the broader concern. The next AM32-feedback-loop missed
  by the port (output prescaler, telemetry timing, etc.) is unrelated to
  servo prescaler and won't be caught by this patch.

### C. Always run timer at 1 MHz

Set L431Timer prescaler to 79 unconditionally, matching AM32's servo
config. Captures land in µs.

Pros:
- ✅ Trivial change.

Cons:
- ❌ Catastrophically degrades DShot resolution. DShot300 bit time of
  3.33 µs becomes ~3 timer ticks. Differentiating 0-bit (1.25 µs high)
  from 1-bit (2.5 µs high) requires sub-tick precision the timer cannot
  provide. Don't do this.

## Recommendation

**Option A — extend the `next_capture` channel to carry both NDTR and
PSC** (or split into `CaptureConfig` with both). Apply the swap in the
per-MCU EXTI re-arm path with the same semantics as AM32: writes
`PSC = new_value`, `EGR.UG = 1` to latch, then re-arms DMA. The
buffer-alignment fix already established the precedent of "decoder
returns HAL directives via `TransferActions`"; this is the same shape,
one layer wider.

After Option A lands, audit AM32's `signal.c`, `dshot.c`, and per-MCU
`IO.c` files for any other writes to peripheral-config globals
(`buffer_padding`, `output_timer_prescaler`, `out_put`, etc.) and
confirm each has a Rust-side counterpart. Use the test
`servo_pin_high_requests_realign` as the template for harness-side
coverage of these directive returns — extend with cases that verify the
right `Prescaler` is requested for each detected protocol.
