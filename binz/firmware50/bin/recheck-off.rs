//! Standalone disabled recheck exercise. No motor command parser.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
mod board;
use cortex_m_rt::entry;
use firmware50::{bridge::safe_off, hw, report::Sink, roots::{self, Drv8304}, run::Hal};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn TIM16() {
    // SAFETY: actual TIM16 vector, initialized at Motor::NVIC by board init.
    unsafe { roots::recheck::check::interrupt() }
}
#[interrupt]
fn ADC_COMP() { hw::comp::line_disable(); hw::comp::clear_pending(); }

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop { cortex_m::asm::nop(); }
    };
    safe_off(&mut Drv8304);
    hw::fine::init();
    board::banner(&mut board, adc_ok);
    board.say("RECHECKOFF ready: t tests disabled callback, p off readback; no drive commands\r\n");
    loop {
        board.now(); board.drain();
        if let Some(b) = board.rx() {
            if b == b't' {
                for case in 0..6 {
                    if !roots::recheck::check::run(case, &mut board) { break; }
                    board.now(); board.drain();
                }
            }
            safe_off(&mut Drv8304);
            if b == b'p' { board.say("RECHECKOFF idle ");
                board.kv("enable", u32::from(hw::gpio::enable_is_high()));
                board.kv("moe", u32::from(hw::pwm::moe_is_set())); board.say("\r\n"); }
        }
    }
}
