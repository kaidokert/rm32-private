//! E426: same diode order diagnostic with fresh-estimate scheduling.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]
mod board;
use cortex_m_rt::entry;
use firmware50::{bemf::FreshEstimate, bridge::safe_off, capture::NoLog, ordertrace::OrderRing};
use firmware50::{hw, report::Sink, roots::{self, Drv8304}, run::{Hal, Production}};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn TIM16() {
    // SAFETY: configured TIM16 vector at Motor::NVIC.
    unsafe { roots::com_root_with::<OrderRing, hw::pwm::DiodeLatched>() }
}
#[interrupt]
fn ADC_COMP() {
    // SAFETY: configured ADC_COMP vector at CompPrio::NVIC.
    unsafe { roots::comp_root_timed::<NoLog, OrderRing, FreshEstimate>() }
}
#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop { cortex_m::asm::nop(); }
    };
    hw::fine::init();
    safe_off(&mut Drv8304);
    let latch_ok = hw::pwm::latch_check::run() == Some((24, 24, 9, 0));
    safe_off(&mut Drv8304);
    let diode_ok = hw::pwm::diode_check::run();
    safe_off(&mut Drv8304);
    let timer_ok = hw::com_timer::crossing_selftest_off();
    board::banner(&mut board, adc_ok);
    if !adc_ok || !timer_ok || !latch_ok || !diode_ok {
        board.say("FATAL adc_timer_diode_check -- refusing to drive\r\n");
        loop { board.now(); board.drain(); }
    }
    board.say("DIODEFRESH diagnostic fixed48k fresh-wait source-low-off latched\r\n");
    let mut controller = Production::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            let record = firmware50::run::drives_a_run(b) && b != b'b';
            if record { OrderRing::arm_next_run(); }
            controller.command(&mut board, b);
            if record {
                OrderRing::disarm();
                OrderRing::emit(&mut board);
            }
            if b == b'p' {
                board.say("DIODESELFTEST en0 six_sectors_pwm=PASS latch=PASS deadline=PASS\r\n");
            }
        }
    }
}
