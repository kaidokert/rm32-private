//! Diagnostic image: the production firmware with the **commutation timing
//! chain** recorded, one row per accepted crossing and one per COM service
//! (campaign 8 step 2, E154).
//!
//! Identical to `shell-pwm` except that both motor roots carry
//! [`firmware50::chain::ChainRing`]: `ADC_COMP` records the crossing stamp,
//! the arm instant, the wait asked for and what the handler had spent, and
//! `TIM16` records when it ran, when the bridge actually changed, what instant
//! it was scheduled for, the lateness the firmware would report and -- the
//! quantity E153 showed is missing -- the timer's **purpose**.
//!
//! The ring is armed for one run by the keys the fixture drives, and dumped
//! after the run's own report, i.e. after `safe_off`: never a byte while the
//! bridge is live (UART edges couple into the comparator).
//!
//! Recording costs both roots time, so this image's runs are evidence about
//! *timing structure*, not about production's loop quality. The quantity
//! compared across images is the derived effective angle, computed on the host
//! by `scripts/chain.py`.

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
use firmware50::chain::{self, ChainRing};
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::{Hal, Production};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

/// TIM16 one-shot commutation, with its service recorded (`ChainRing`).
#[interrupt]
fn TIM16() {
    // SAFETY: the PAC's vector `TIM16 = 21` ("21 - TIM16 global interrupt"), at `COM_IRQ_PRIORITY`.
    unsafe { roots::com_root::<ChainRing>() }
}

/// COMP2 zero-crossing edge, with each acceptance's arm recorded
/// (`ChainRing`); the decision log itself stays off (`NoLog`).
#[interrupt]
fn ADC_COMP() {
    // SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.
    unsafe { roots::comp_root::<NoLog, ChainRing>() }
}

/// Keys that arm the ring, and so are dumped when their run ends: the rung
/// keys the fixture drives plus the climb keys.
///
/// **`b` (15%) is deliberately absent.** It is the unjudged warm-up every
/// diagnostic runner drives first, and a dump after it would leave 1024 rows
/// in the link for the *next* capture to read as part of its own.
fn arms_a_run(b: u8) -> bool {
    // **One list, in the library** (E186 SS3): `run::drives_a_run` is what the
    // shell actually dispatches on, so this image cannot drift from it -- which
    // is exactly what a local allowlist did, silently, in E183.
    //
    // `b` (15%) is excluded here: it is the unjudged warm-up every diagnostic
    // runner drives first, and a dump after it would leave two rings in the
    // link for the *next* capture to read as part of its own.
    firmware50::run::drives_a_run(b) && b != b'b'
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    // The fine clock the chain stamps from (E180): TIM2 free-running at
    // 64 MHz, 15.6 ns a tick. Only this image enables it; production never
    // touches TIM2, which is what keeps its roots identical.
    firmware50::hw::fine::init();
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    board.say("CHAINCAPTURE diagnostic image: every run dumps its timing chain\r\n");
    board.tx_flush();
    let mut p = Production::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            if arms_a_run(b) {
                ChainRing::arm_next_run();
            }
            p.command(&mut board, b);
            if arms_a_run(b) {
                // The run is over and the bridge is off: stop recording, then
                // write the ring out.
                ChainRing::disarm();
                let _ = ChainRing::read(|acc, svc| chain::emit(acc, svc, &mut Echo(&mut board)));
            }
        }
    }
}

/// `chain::emit` writes through the board's ring and drains it per line.
struct Echo<'a>(&'a mut board::Board);

impl Sink for Echo<'_> {
    fn put(&mut self, b: u8) {
        self.0.put(b);
    }
    fn flush(&mut self) {
        self.0.tx_flush();
    }
}
