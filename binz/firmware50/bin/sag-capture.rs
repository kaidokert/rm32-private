//! Diagnostic image: the production firmware with the **sharp-sag guard's own
//! inputs** recorded per judged block, in a bounded pre-trip ring that freezes
//! on the fault (campaign 9 step 3).
//!
//! Identical to `shell-pwm` except that the controller's sag-recorder slot is
//! [`firmware50::sagtrace::SagRing`] instead of `NoSagLog`. Every block the
//! guard judges is recorded with the four quantities it actually compares --
//! the bus and VREF block means against their ~207 ms filtered references, as
//! used for *that* test -- plus the streak it holds afterwards, the duty, the
//! sector and the microseconds since the last accepted crossing.
//!
//! On a trip the ring freezes, so the dump is the window that led there. It is
//! written after the run's report, i.e. after `safe_off`: never a byte while
//! the bridge is live.
//!
//! Recording costs foreground time, so this image's runs are evidence about
//! the guard's inputs, not about production's loop quality.

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
use firmware50::run::policy;
use firmware50::run::{Controller, Hal};
use firmware50::sagtrace::{self, SagRing};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

/// TIM16 one-shot commutation: the production root, unrecorded -- this image
/// instruments the foreground's sag judgement, not the ISRs.
#[interrupt]
fn TIM16() {
    // SAFETY: the PAC's vector `TIM16 = 21` ("21 - TIM16 global interrupt"), at `COM_IRQ_PRIORITY`.
    unsafe { roots::com_root::<NoChain>() }
}

/// COMP2 zero-crossing edge: the production root, unrecorded.
#[interrupt]
fn ADC_COMP() {
    // SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.
    unsafe { roots::comp_root::<NoLog, NoChain>() }
}

/// Production's composition with the sag recorder installed.
type SagProduction = Controller<
    policy::Wiring,
    policy::BemfPolicy,
    policy::AdvancePolicy,
    policy::CurrentProtection,
    policy::BusSagProtection,
    policy::Restart,
    policy::Telemetry,
    SagRing,
>;

/// Keys that arm the ring, and so are dumped when their run ends: the rung
/// keys the fixture drives plus the climb keys.
///
/// **`b` (15%) is deliberately absent.** It is the unjudged warm-up every
/// diagnostic runner drives first, and a dump after it would leave two rings
/// in the link for the *next* capture to read as part of its own.
fn arms_a_run(b: u8) -> bool {
    matches!(
        b,
        b'2' | b'5'
            | b'a'
            | b'A'
            | b'y'
            | b'Y'
            | b'c'
            | b'C'
            | b'd'
            | b'D'
            | b'm'
            | b'M'
            | b'e'
            | b'E'
            | b'j'
            | b'J'
            | b'l'
            | b'L'
            | b'Z'
    )
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
    board.say("SAGCAPTURE diagnostic image: every run dumps the sag guard inputs\r\n");
    board.tx_flush();
    let mut p = SagProduction::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            if arms_a_run(b) {
                SagRing::arm_next_run();
            }
            p.command(&mut board, b);
            if arms_a_run(b) {
                // The run is over and the bridge is off: stop recording, then
                // write the ring out.
                SagRing::disarm();
                SagRing::read(|t| sagtrace::emit(t, &mut Echo(&mut board)));
            }
        }
    }
}

/// `sagtrace::emit` writes through the board's ring and drains it per line.
struct Echo<'a>(&'a mut board::Board);

impl Sink for Echo<'_> {
    fn put(&mut self, b: u8) {
        self.0.put(b);
    }
    fn flush(&mut self) {
        self.0.tx_flush();
    }
}
