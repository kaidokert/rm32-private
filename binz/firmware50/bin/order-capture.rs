//! E381: compact acceptance/phase1 bridge tails plus unchanged sag inputs.
//! Diagnostic only; the observer may change timing. No streaming under power.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

mod board;

use cortex_m_rt::entry;
use firmware50::bridge::safe_off;
use firmware50::capture::NoLog;
use firmware50::ordertrace::{OrderRing, OrderSag};
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::{Controller, Hal, policy};
use firmware50::sagtrace::{self, SagRing};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn TIM16() {
    // SAFETY: this is the TIM16 vector, configured at Motor::NVIC.
    unsafe { roots::com_root::<OrderRing>() }
}
#[interrupt]
fn ADC_COMP() {
    // SAFETY: this is ADC_COMP, configured at CompPrio::NVIC.
    unsafe { roots::comp_root::<NoLog, OrderRing>() }
}

type Diagnostic = Controller<
    policy::Wiring,
    policy::BemfPolicy,
    policy::ScheduledAdvance<16, 18>,
    policy::CurrentProtection,
    policy::BusSagProtection,
    policy::Restart,
    policy::Telemetry,
    OrderSag,
>;

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    firmware50::hw::fine::init();
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    if !adc_ok {
        board.say("FATAL adc_init_failed -- refusing to drive\r\n");
        loop {
            board.now();
            board.drain();
        }
    }
    board.say("ORDERCAPTURE diagnostic; service timestamps, not physical edges\r\n");
    board.tx_flush();
    let mut p = Diagnostic::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            let record = firmware50::run::drives_a_run(b) && b != b'b';
            if record {
                OrderRing::arm_next_run();
                SagRing::arm_next_run();
            }
            p.command(&mut board, b);
            if record {
                OrderRing::disarm();
                SagRing::disarm();
                OrderRing::emit(&mut Echo(&mut board));
                SagRing::read(|t| sagtrace::emit(t, &mut Echo(&mut board)));
            }
        }
    }
}

struct Echo<'a>(&'a mut board::Board);
impl Sink for Echo<'_> {
    fn put(&mut self, b: u8) {
        self.0.put(b);
    }
    fn flush(&mut self) {
        self.0.tx_flush();
    }
}
