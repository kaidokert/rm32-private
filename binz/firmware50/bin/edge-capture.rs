//! Diagnostic image: the production firmware with the COMP root's decisions
//! recorded, for the replay test (goal item 7; E121).
//!
//! Identical to `shell-pwm` except that `ADC_COMP` runs
//! `roots::comp_root::<Ring>()`, which records [`firmware50::capture`]'s
//! window of decisions once the loop is at target, and that after the 25%
//! rung (`5`) the capture is written out (`CAPSNAP`, `CAP` lines, `CAPEND`)
//! once the run's own report is done -- after `safe_off`, like every other
//! byte. Recording costs COMP time, so this image's runs are evidence of the
//! decision function, not of production's loop quality.

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
use firmware50::capture::{self, Ring};
use firmware50::chain::NoChain;
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::{Hal, Production};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

/// TIM16 one-shot commutation: `firmware50::roots::com_root`, unrecorded.
#[interrupt]
fn TIM16() {
    // SAFETY: the PAC's vector `TIM16 = 21` ("21 - TIM16 global interrupt"), at `COM_IRQ_PRIORITY`.
    unsafe { roots::com_root::<NoChain>() }
}

/// COMP2 zero-crossing edge, with the decision recorded (`Ring`).
#[interrupt]
fn ADC_COMP() {
    // SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.
    unsafe { roots::comp_root::<Ring, NoChain>() }
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    board.say("EDGECAPTURE diagnostic image: '5' runs 25% and dumps the decisions\r\n");
    board.tx_flush();
    let mut p = Production::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            // Only the 25% rung is captured; every other run leaves the ring
            // disarmed.
            // `5` keeps the first window (the replay capture, E121); `c`
            // keeps the last decisions before a stop (E138, the 30% dropout).
            if b == b'5' {
                Ring::arm_next_run();
            } else if b == b'c' || b == b'L' {
                // ENV-28: `L` (the climb rung, full window) also keeps the LAST
                // decisions, so a high-duty run's tail is captured.
                Ring::arm_next_run_circular();
            }
            p.command(&mut board, b);
            if b == b'5' || b == b'c' || b == b'L' {
                let _ = Ring::read(|c| capture::emit(c, &mut Echo(&mut board)));
            }
        }
    }
}

/// `capture::emit` writes through the board's ring and drains it per line.
struct Echo<'a>(&'a mut board::Board);

impl Sink for Echo<'_> {
    fn put(&mut self, b: u8) {
        self.0.put(b);
    }
    fn flush(&mut self) {
        self.0.tx_flush();
    }
}
