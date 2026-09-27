//! E456: bounded COM-owned level observations; no foreground retry budget.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]
mod board;
use cortex_m_rt::entry;
use firmware50::{bemf::{ConstantAdvance, FreshEstimate}, bridge::safe_off, capture::NoLog, chain::NoChain};
use firmware50::{hw, report::Sink, roots::{self, Drv8304}, run::{Hal, Controller, policy}};
use stm32g0xx_hal::stm32::{self, interrupt};

const _: () = {
    assert!(firmware50::run::policy::ADVANCE_LOW == 16);
    assert!(firmware50::run::policy::ADVANCE_HIGH == 16);
};

#[interrupt]
fn TIM16() {
    // SAFETY: configured TIM16 vector at Motor::NVIC.
    unsafe { roots::com_root_scheduled::<NoChain, hw::pwm::DiodeLatched, roots::recheck::Timed>() }
}
#[interrupt]
fn ADC_COMP() {
    // SAFETY: configured ADC_COMP vector at CompPrio::NVIC.
    unsafe { roots::comp_root_timed::<NoLog, NoChain, ConstantAdvance<FreshEstimate, 16>>() }
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
    board.say("DIODETIMED fixed48k fresh-wait source-low-off bounded-recheck\r\n");
    type Candidate = Controller<policy::Wiring, policy::ExternalRevisit<policy::BemfPolicy>, policy::AdvancePolicy, policy::CurrentProtection, policy::BusSagProtection, policy::Restart, policy::Telemetry>;
    let mut controller = Candidate::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            controller.command(&mut board, b);
            if b == b'p' {
                roots::recheck::report(&mut board);
                board.say("DIODESELFTEST en0 six_sectors_pwm=PASS latch=PASS deadline=PASS\r\n");
            }
        }
    }
}
