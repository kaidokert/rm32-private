//! Soft-UART RX example using the reusable `minz::softuart` module.
//!
//! Only `unsafe` in this file is the `NVIC::unmask` block. The pure decoder
//! state machine + RX FIFO live in `minz::softuart::SoftUart<...>`; this
//! binary just wires hardware (pin, EXTI, TIM2, SysTick) and feeds the
//! library two callbacks: a falling-edge event with timestamp, and a
//! bit-sample-time pin level.
//!
//! Wire: USB-TTL **TX → PA0**, **GND ↔ GND**, 9600 8N1.

#![no_std]
#![no_main]

use core::cell::RefCell;

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::{NVIC, syst::SystClkSource};
use cortex_m_rt::{entry, exception};
use fugit::HertzU32 as Hertz;
use minz::board_init::{BoardInit, init};
use minz::hal::gpio::gpioa::PA0;
use minz::hal::gpio::{Edge, ExtiPin, Input, PullUp};
use minz::hal::pac::{TIM2, interrupt};
use minz::hal::prelude::*;
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::hal::timer::{Event, Timer};
use minz::softuart::{BitSampleTimer, FrameStep, RxHw, SoftUart};
use minz::{SYSCLK, SYSTICK};
use portable_atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

const BAUD: Hertz = Hertz::Hz(9600);
const OVERSAMPLE: usize = 4;
const SAMPLE: Hertz = Hertz::Hz(BAUD.raw() * OVERSAMPLE as u32); // 38400
const RX_BUF_LEN: usize = 16;

type Uart = SoftUart<{ BAUD.raw() }, { SYSTICK.raw() }, OVERSAMPLE, RX_BUF_LEN>;

static TICKS_10US: AtomicU32 = AtomicU32::new(0);

type RxPin = PA0<Input<PullUp>>;
type RxTimer = Timer<TIM2>;

static RX_HW: Mutex<RefCell<Option<RxHw<RxPin, RxTimer>>>> = Mutex::new(RefCell::new(None));
static UART: Mutex<RefCell<Uart>> = Mutex::new(RefCell::new(SoftUart::new()));

fn ticks_10us() -> u32 {
    TICKS_10US.load(Ordering::Relaxed)
}

fn wait_until(deadline: u32) {
    while ticks_10us().wrapping_sub(deadline) > u32::MAX / 2 {
        // Drain the RX FIFO opportunistically.
        while let Some(b) = free(|cs| UART.borrow(cs).borrow_mut().pop()) {
            let ch = if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                '.'
            };
            rprintln!("rx 0x{:02X} '{}'", b, ch);
        }
        cortex_m::asm::nop();
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        cp,
        clocks,
        mut ahb2,
        mut apb1r1,
        mut apb2,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);
    rprintln!(
        "bitbang_uart_decode_hal: PA0 RX, TIM2 @ {} Hz ({}x {} baud), FRAME_GAP_TICKS={}",
        SAMPLE.raw(),
        OVERSAMPLE,
        BAUD.raw(),
        Uart::FRAME_GAP_TICKS,
    );
    rprintln!(
        "clocks: sysclk={} hclk={} pclk1={}",
        clocks.sysclk().raw(),
        clocks.hclk().raw(),
        clocks.pclk1().raw(),
    );

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut rx_pin = gpioa
        .pa0
        .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr);

    rx_pin.make_interrupt_source(&mut dp.SYSCFG, &mut apb2);
    rx_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    rx_pin.enable_interrupt(&mut dp.EXTI);

    let mut timer = Timer::tim2(dp.TIM2, SAMPLE, clocks, &mut apb1r1);
    timer.clear_update_interrupt_flag();
    timer.listen(Event::TimeOut);
    timer.pause(); // EXTI ISR turns it back on at the right phase.

    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(clocks.hclk().raw() / SYSTICK.raw() - 1);
    syst.clear_current();
    syst.enable_interrupt();
    syst.enable_counter();

    free(|cs| {
        RX_HW.borrow(cs).replace(Some(RxHw::new(rx_pin, timer)));
    });

    unsafe {
        NVIC::unmask(Interrupt::TIM2);
        NVIC::unmask(Interrupt::EXTI0);
        cortex_m::interrupt::enable();
    }

    let mut loop_n: u32 = 0;
    let mut prev_frame: u32 = 0;
    let mut prev_err: u32 = 0;
    let mut next_deadline = ticks_10us().wrapping_add(SYSTICK.raw());
    loop {
        loop_n = loop_n.wrapping_add(1);
        let (frame, errs) = free(|cs| {
            let uart = UART.borrow(cs).borrow();
            (uart.frame_count(), uart.framing_errors())
        });
        rprintln!(
            "loop={} frame_delta={} err_delta={}",
            loop_n,
            frame.wrapping_sub(prev_frame),
            errs.wrapping_sub(prev_err),
        );
        prev_frame = frame;
        prev_err = errs;

        wait_until(next_deadline);
        next_deadline = next_deadline.wrapping_add(SYSTICK.raw());
    }
}

#[exception]
fn SysTick() {
    TICKS_10US.fetch_add(1, Ordering::Relaxed);
}

#[interrupt]
fn EXTI0() {
    free(|cs| {
        let mut hw_borrow = RX_HW.borrow(cs).borrow_mut();
        let hw = hw_borrow.as_mut().expect("RX_HW is not None");
        hw.pin.clear_interrupt_pending_bit();
        // Falling-edge-only trigger: pin is low by construction.
        if UART.borrow(cs).borrow_mut().on_falling_edge(ticks_10us()) {
            // Half-period seed for TIM2.CNT so the first update lands
            // ~½ sample-period after this start edge — bit-center.
            hw.timer.reload(SYSCLK.raw() / SAMPLE.raw() / 2);
        }
    });
}

#[interrupt]
fn TIM2() {
    free(|cs| {
        let mut hw_borrow = RX_HW.borrow(cs).borrow_mut();
        let hw = hw_borrow.as_mut().expect("RX_HW is not None");
        hw.timer.clear_update_interrupt_flag();
        let step = UART.borrow(cs).borrow_mut().on_sample(hw.pin.is_high());
        if step == FrameStep::Complete {
            hw.timer.pause();
        }
    });
}
