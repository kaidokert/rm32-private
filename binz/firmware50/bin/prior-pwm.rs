//! Lean fixed-48k screen of preblend-estimate scheduling (E409).
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

mod board;

use cortex_m_rt::entry;
use firmware50::bemf::PreviousEstimate;
use firmware50::bridge::safe_off;
use firmware50::capture::NoLog;
use firmware50::chain::NoChain;
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::{Hal, Production};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn TIM16() {
    // SAFETY: configured TIM16 vector at Motor::NVIC.
    unsafe { roots::com_root::<NoChain>() }
}

#[interrupt]
fn ADC_COMP() {
    // SAFETY: configured ADC_COMP vector at CompPrio::NVIC.
    unsafe { roots::comp_root_timed::<NoLog, NoChain, PreviousEstimate>() }
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop { cortex_m::asm::nop(); }
    };
    safe_off(&mut Drv8304);
    let timer_ok = firmware50::hw::com_timer::crossing_selftest_off();
    board::banner(&mut board, adc_ok);
    if !adc_ok || !timer_ok {
        board.say("FATAL adc_or_deadline_timer_failed -- refusing to drive\r\n");
        loop { board.now(); board.drain(); }
    }
    board.say("PRIORPWM fixed48k; wait=preblend; entry-relative post-prepare arm\r\n");
    let mut controller = Production::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            controller.command(&mut board, b);
            if b == b'p' {
                board.say("PRIORPWM wait=preblend DEADLINESELFTEST boot_off=1 delays2_10_40=PASS\r\n");
            }
        }
    }
}
