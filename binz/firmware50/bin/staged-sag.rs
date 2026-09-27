//! Staged64 with only the existing foreground sag ring; motor roots unrecorded.
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

mod board;

use cortex_m_rt::entry;
use firmware50::bridge::safe_off;
use firmware50::capture::NoLog;
use firmware50::chain::NoChain;
use firmware50::report::Sink;
use firmware50::roots::{self, Drv8304};
use firmware50::run::{Controller, Hal, policy};
use firmware50::sagtrace::{self, SagRing};
use stm32g0xx_hal::stm32::{self, interrupt};

#[interrupt]
fn TIM16() {
    // SAFETY: this is the TIM16 vector, configured at Motor::NVIC.
    unsafe { roots::com_root::<NoChain>() }
}

#[interrupt]
fn ADC_COMP() {
    // SAFETY: this is ADC_COMP, configured at CompPrio::NVIC.
    unsafe { roots::comp_root::<NoLog, NoChain>() }
}

type StagedSag = Controller<
    policy::Wiring,
    policy::BemfPolicy,
    policy::AdvancePolicy,
    policy::CurrentProtection,
    policy::BusSagProtection,
    policy::Restart,
    policy::Telemetry,
    SagRing,
    policy::FasterAbove<350, 1000>,
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
    let carrier_ok = firmware50::hw::pwm::carrier_selftest_off();
    board::banner(&mut board, adc_ok);
    if !adc_ok || !carrier_ok {
        board.say("FATAL adc_or_carrier_check_failed -- refusing to drive\r\n");
        loop {
            board.now();
            board.drain();
        }
    }
    board.say("STAGEDSAG diagnostic; foreground guard operands only\r\n");
    board.tx_flush();
    let mut p = StagedSag::new();
    loop {
        board.now();
        board.drain();
        if let Some(b) = board.rx() {
            let record = firmware50::run::drives_a_run(b) && b != b'b';
            if record {
                SagRing::arm_next_run();
            }
            p.command(&mut board, b);
            if b == b'p' {
                board.say("CARRIERSELFTEST boot_off=1 native64k=PASS startup_restore_requested=1\r\n");
            }
            if record {
                SagRing::disarm();
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
