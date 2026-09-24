//! firmware50 bench bring-up: scripted open-loop drive with **live** protections.
//!
//! # Peripheral access policy
//!
//! Clocks, GPIO configuration, USART3, the timers and the watchdog all go
//! through `stm32g0xx-hal`. What does not lives in `firmware50::hw`, behind
//! safe functions, each justified at its definition there (TIM1, the gate-pin
//! mode/AF bits, the ADC and its DMA, the timebase, IWDG timeout, reset cause).
//!
//! **A correction worth recording.** An earlier version of this header claimed
//! the HAL's PWM macro "substitutes `cc1ne` for `cc1e`, so the main OC1-OC3
//! never pulse", inherited from a scar in the neighbouring `binz` package.
//! That is **false for this HAL revision** -- `timer/pwm.rs` sets `cc1e` and
//! `cc1ne` and `moe` together, correctly. The scar was against an older
//! revision and repeating it unverified was worse than saying nothing, because
//! it would have been cited again.
//!
//! The real reasons TIM1 cannot come from the HAL's PWM API are narrower and
//! checkable in the vendored source:
//!
//! * `BDTR` is written in exactly one place (`pwm.rs`, `moe().set_bit()`), so
//!   there is **no route to `DTG`, `OSSR` or `OSSI`**. A 6x gate driver with no
//!   dead-time insertion is a shoot-through generator; that alone forecloses it.
//! * `PwmPin::enable()` sets `MOE` in the same call that enables the channel,
//!   and `disable()` only clears `CCxE` -- there is no `moe_off()`. The whole
//!   preflight/`safe_off` design rests on "configured but MOE clear", which
//!   that API cannot express.
//! * `Pwm::set_freq` does `cr1().write(...)`, clobbering `ARPE`.
//!
//! And for the gate pins, the load-bearing reason is not that `AltFunction` is
//! `pub(crate)` (it is, but `bind_pin` would set AF2 for us). It is that
//! `bind_pin` takes the pin **by value and drops it**, so the HAL can make the
//! AF transition exactly once and never undo it -- while this firmware must
//! flip all six pins between AF2 and plain-low output at *every* arm and
//! safe-off boundary.
//!
//! Every remaining escape uses PAC *types* with named register fields and
//! `modify`: never a transcribed address, never a wholesale register write.
//! That distinction is not cosmetic. An earlier revision hand-wrote
//! `FLASH_ACR = 0x2` to set the wait states, silently cleared `DBG_SWEN`, and
//! disabled SWD from software -- the board looked bricked, survived a probe
//! replug and a power cycle, and needed `--connect-under-reset` to recover.
//! With the HAL, or with a PAC `modify`, that field is preserved by
//! construction.
//!
//! # Architecture (since E120)
//!
//! This binary is the hardware: `Board` (clocks, pins, USART3, timers, ADC and
//! DMA, TIM1, the watchdog) and its implementation of
//! `firmware50::run::Hal`; the four `#[interrupt]` shims whose logic is
//! `firmware50::roots`; and the panic handler. The run itself -- the typestate
//! `Idle -> Armed -> Startup -> Handover -> Locked -> Stopped`, every
//! protection, the restart campaign and the report -- is
//! `firmware50::run::Production`, which `main` constructs and serves.
//!
//! | rate | source | work |
//! |------|--------|------|
//! | edge | COMP2 -> EXTI18 | `roots::comp_root`: the zero-crossing decision |
//! | per commutation | TIM16 one-shot | `roots::com_root`: the next plan |
//! | ~9901 Hz | TIM6 TRGO + DMA | a five-channel scan, published under a seqlock |
//! | ~9901 Hz | TIM6 update | `roots::guard_root`: the powered guard |
//! | every pass | foreground | protections on each fresh scan, host bytes, one UART TX byte |
//!
//! **The ADC rate is deliberately 9901 Hz, not 10000 Hz**: equal periods with
//! a 10 kHz carrier would phase-lock the sampler to one instant of it.
//!
//! UART TX is a ring drained one byte per loop pass, never a blocking write,
//! and nothing is queued while the bridge is driven (E073): the ring is this
//! binary's, written only through `Board`'s `Sink`, which the driving states
//! do not hold.
//!
//! # Wiring (NUCLEO-G071RB + BOOSTXL-DRV8304H, 6x PWM mode)
//!
//! ```text
//! phase A  INHA PA10 = TIM1_CH3   INLA PB1 = TIM1_CH3N
//! phase B  INHB PA9  = TIM1_CH2   INLB PB0 = TIM1_CH2N
//! phase C  INHC PA8  = TIM1_CH1   INLC PA7 = TIM1_CH1N
//! ENABLE PD1   nFAULT PB14   LED PB5   USART3 TX PC10 / RX PC11
//! ISENA PA0    ISENB PA1     ISENC PA4   VBUS PA6   VREFINT internal
//! ```

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
// Unsafe-related lints, enforced on every clippy run (goal item 6): the
// pedantic pointer/cast lints, and every `unsafe` block documented, one
// operation each.
#![warn(
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::missing_safety_doc,
    clippy::multiple_unsafe_ops_per_block,
    clippy::ptr_as_ptr,
    clippy::ptr_cast_constness,
    clippy::transmute_ptr_to_ptr,
    clippy::undocumented_unsafe_blocks,
    clippy::unnecessary_safety_comment,
    clippy::unnecessary_safety_doc
)]

mod board;

use cortex_m_rt::entry;
use firmware50::bridge::safe_off;
use firmware50::capture::NoLog;
use firmware50::chain::NoChain;
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::Production;
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

/// TIM16 one-shot commutation: `firmware50::roots::com_root`, recording
/// nothing (`NoChain`).
#[interrupt]
fn TIM16() {
    // SAFETY: the PAC's vector `TIM16 = 21` ("21 - TIM16 global interrupt"), at `COM_IRQ_PRIORITY`.
    unsafe { roots::com_root::<NoChain>() }
}

/// COMP2 zero-crossing edge: `firmware50::roots::comp_root`, recording
/// nothing (`NoLog`, `NoChain`).
#[interrupt]
fn ADC_COMP() {
    // SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.
    unsafe { roots::comp_root::<NoLog, NoChain>() }
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    // Belt and braces: whatever the reset state was, the bridge is off now.
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    if !adc_ok {
        // Without feedback there are no protections, so there is no run. Keep
        // draining and feeding: a park loop that stops doing either looks
        // exactly like a hang.
        board.say("FATAL adc_init_failed -- refusing to drive\r\n");
        loop {
            board.tick_clock();
            board.tx_drain();
        }
    }
    Production::new().serve(&mut board)
}
