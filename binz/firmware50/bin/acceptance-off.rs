//! Bridge-disabled acceptance/guard integration. No motor command parser.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
mod board;
use cortex_m_rt::entry;
use firmware50::{bridge::safe_off, hw, report::Sink, roots::{self, Drv8304}, run::Hal};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn ADC_COMP() {
    // SAFETY: actual COMP vector, configured at CompPrio by board initialization.
    unsafe { roots::acceptance::check::comp_interrupt::<roots::acceptance::check::ProbeWindow>() }
}
#[interrupt]
fn TIM16() { roots::acceptance::check::com_interrupt(); }

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop { cortex_m::asm::nop(); }
    };
    safe_off(&mut Drv8304);
    hw::fine::init();
    board::banner(&mut board, adc_ok);
    board.say("ACCEPTANCEOFF ready: t disabled suite, p readback; no drive commands\r\n");
    loop {
        board.now(); board.drain();
        if let Some(command) = board.rx() {
            if command == b't' && adc_ok {
                for case in 0..5 {
                    if !roots::acceptance::check::run(case, &mut board) { break; }
                    board.now(); board.drain();
                }
            }
            safe_off(&mut Drv8304);
            if command == b'p' {
                board.say("ACCEPTANCEOFF idle ");
                board.kv("enable", u32::from(hw::gpio::enable_is_high()));
                board.kv("moe", u32::from(hw::pwm::moe_is_set()));
                let (a, b, c) = hw::pwm::compares();
                board.kv("ccr1", a); board.kv("ccr2", b); board.kv("ccr3", c);
                board.kv("gates_low", u32::from(hw::gpio::gates_all_low()));
                board.kv("nfault", u32::from(hw::gpio::nfault_high()));
                board.say("\r\n");
            }
        }
    }
}
