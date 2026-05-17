#![no_std]
#![no_main]

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use minz::board_init::{BoardInit, init};
use minz::hal::pac::interrupt;
use minz::hal::prelude::*;
use minz::hal::stm32::{self, Interrupt};
use minz::hal::timer::{Event, Timer};
use portable_atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

static TIM7_COUNT: AtomicU32 = AtomicU32::new(0);

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        clocks, mut apb1r1, ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    rprintln!("timer_irq_probe: TIM7 @ 2 Hz");

    let mut timer = Timer::tim7(dp.TIM7, 2.Hz(), clocks, &mut apb1r1);
    timer.clear_update_interrupt_flag();
    timer.listen(Event::TimeOut);

    unsafe {
        NVIC::unmask(Interrupt::TIM7);
        cortex_m::interrupt::enable();
    }

    let mut last = 0;
    loop {
        let count = TIM7_COUNT.load(Ordering::Relaxed);
        if count != last {
            rprintln!("tim7={}", count);
            last = count;
        }
        cortex_m::asm::nop();
    }
}

#[interrupt]
fn TIM7() {
    TIM7_COUNT.fetch_add(1, Ordering::Relaxed);

    // Clear UIF directly via PAC so the handler is self-contained.
    unsafe {
        (*stm32::TIM7::ptr()).sr.modify(|_, w| w.uif().clear_bit());
    }
}
