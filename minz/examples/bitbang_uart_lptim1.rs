//! Soft-UART RX example using **LPTIM1** as the bit-sample timer.
//!
//! Mirrors `bitbang_uart.rs` (which uses TIM2) — only the timer
//! differs. TIM1/2/6/15/16 are off-limits in this project (reserved for
//! the rm32 firmware), so prototypes that need a periodic timer pick
//! from SysTick / TIM7 / LPTIM1 / LPTIM2.
//!
//! Key behavioural differences vs TIM2 (handled inside
//! `softuart_lptim1`'s `BitSampleTimer` impl; the example itself looks
//! the same): LPTIM has no public `set_cnt`, and disabling LPTIM also
//! wipes ARR/CMP. So we leave the timer running continuously and stop
//! the sample ISR between frames by masking `NVIC::LPTIM1`. Phase sync
//! to the start edge is done by writing `CMP = (CNT + cnt) % (ARR+1)`.
//!
//! Wire: USB-TTL **TX → PA0**, **GND ↔ GND**, 9600 8N1.

#![no_std]
#![no_main]

use core::cell::RefCell;

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::{entry, exception};
use fugit::HertzU32 as Hertz;
use minz::SYSTICK;
use minz::board_init::{BoardInit, configure_systick, init};
use minz::hal::gpio::gpioa::PA0;
use minz::hal::gpio::{Edge, ExtiPin, Input, PullUp};
use minz::hal::lptimer::{ClockSource, Event, LowPowerTimer, LowPowerTimerConfig, PreScaler};
use minz::hal::pac::{LPTIM1, interrupt};
use minz::hal::prelude::*;
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::softuart::{FrameStep, IrqAck, RxHw, SoftUart};
use portable_atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

const BAUD: Hertz = Hertz::Hz(9600);
const OVERSAMPLE: usize = 4;
const SAMPLE: Hertz = Hertz::Hz(BAUD.raw() * OVERSAMPLE as u32); // 38400
const RX_BUF_LEN: usize = 16;

type Uart = SoftUart<{ BAUD.raw() }, { SYSTICK.raw() }, OVERSAMPLE, RX_BUF_LEN>;

static TICKS_10US: AtomicU32 = AtomicU32::new(0);

type RxPin = PA0<Input<PullUp>>;
type RxTimer = LowPowerTimer<LPTIM1>;

static RX_HW: Mutex<RefCell<Option<RxHw<RxPin, RxTimer>>>> = Mutex::new(RefCell::new(None));
static UART: Mutex<RefCell<Uart>> = Mutex::new(RefCell::new(SoftUart::new()));

fn ticks_10us() -> u32 {
    TICKS_10US.load(Ordering::Relaxed)
}

fn wait_until(deadline: u32) {
    while ticks_10us().wrapping_sub(deadline) > u32::MAX / 2 {
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
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    // LPTIM1 from PCLK1 (= sysclk = 80 MHz with our cfgr).
    // Sample period in LPTIM ticks = pclk1 / SAMPLE. CMP = ARR/2 puts the
    // CompareMatch event at the bit centre; updated per-frame by reload().
    let lptim_ticks_per_sample = clocks.pclk1().raw() / SAMPLE.raw();
    let arr = (lptim_ticks_per_sample - 1) as u16;
    let cmp = arr / 2;

    rprintln!(
        "bitbang_uart_lptim1: PA0 RX, LPTIM1 @ {} Hz ({}x {} baud), ARR={} CMP={} FRAME_GAP_TICKS={}",
        SAMPLE.raw(),
        OVERSAMPLE,
        BAUD.raw(),
        arr,
        cmp,
        Uart::FRAME_GAP_TICKS,
    );
    rprintln!(
        "clocks: sysclk={} hclk={} pclk1={}",
        clocks.sysclk().raw(),
        clocks.hclk().raw(),
        clocks.pclk1().raw(),
    );

    configure_systick(cp.SYST, &clocks, SYSTICK);

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut rx_pin = gpioa
        .pa0
        .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr);

    rx_pin.make_interrupt_source(&mut dp.SYSCFG, &mut apb2);
    rx_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    rx_pin.enable_interrupt(&mut dp.EXTI);
    let lptim_cfg = LowPowerTimerConfig::default()
        .clock_source(ClockSource::PCLK)
        .prescaler(PreScaler::U1)
        .arr_value(arr)
        .compare_value(cmp);
    let mut timer = LowPowerTimer::lptim1(dp.LPTIM1, lptim_cfg, &mut apb1r1, &mut ccipr, clocks);
    // `listen` internally toggles ENABLE to write IER, which wipes ARR/CMP
    // back to 0. Re-program them so the timer ticks at the right rate.
    timer.listen(Event::CompareMatch);
    timer.set_autoreload(arr);
    timer.set_compare_match(cmp);
    free(|cs| {
        RX_HW.borrow(cs).replace(Some(RxHw::new(rx_pin, timer)));
    });

    // LPTIM1 runs continuously; the ISR also fires continuously but the
    // `RX_ACTIVE` flag gates whether it actually advances the decoder.
    unsafe {
        NVIC::unmask(Interrupt::LPTIM1);
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
        if UART.borrow(cs).borrow_mut().on_falling_edge(ticks_10us()) {
            // LPTIM is free-running; just open the gate so the LPTIM ISR
            // starts feeding samples to the decoder. Phase relative to the
            // start edge is uncontrolled (0..1 sample period), but at
            // OVERSAMPLE=4 all four samples per bit still land inside the
            // bit so majority voting is unaffected.
            hw.rx_active = true;
        }
    });
}

#[interrupt]
fn LPTIM1() {
    free(|cs| {
        let mut hw_borrow = RX_HW.borrow(cs).borrow_mut();
        let hw = hw_borrow.as_mut().expect("RX_HW is not None");
        hw.timer.ack();
        if !hw.rx_active {
            return;
        }
        let step = UART.borrow(cs).borrow_mut().on_sample(hw.pin.is_high());
        if step == FrameStep::Complete {
            hw.rx_active = false;
        }
    });
}
