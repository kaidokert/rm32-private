//! Soft-UART RX example using the reusable `minz::softuart` module.
//!
//! Decoder state lives in `minz::softuart::SoftUart<...>` (pure state
//! machine — no FIFO). The RX byte queue is a `heapless::spsc::Queue`
//! split at boot: producer half lives in a `Mutex<RefCell<...>>` for
//! the TIM2 ISR, consumer half is a local in `main` so draining is
//! lock-free.
//!
//! Wire: USB-TTL **TX → PA0**, **GND ↔ GND**, 9600 8N1.

#![no_std]
#![no_main]

use core::cell::RefCell;

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::{entry, exception};
use fugit::HertzU32 as Hertz;
use heapless::spsc::{Consumer, Producer, Queue};
use minz::SYSTICK;
use minz::board_init::{BoardInit, configure_systick, init};
use minz::hal::gpio::gpioa::PA0;
use minz::hal::gpio::{Edge, ExtiPin, Input, PullUp};
use minz::hal::pac::{TIM2, interrupt};
use minz::hal::prelude::*;
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::hal::timer::{Event, Timer};
use minz::softuart::{IrqAck, RxHw, SoftUart};
use portable_atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

const BAUD: Hertz = Hertz::Hz(9600);
const OVERSAMPLE: usize = 4;
const SAMPLE: Hertz = Hertz::Hz(BAUD.raw() * OVERSAMPLE as u32); // 38400
const RX_BUF_LEN: usize = 16;

type Uart = SoftUart<{ BAUD.raw() }, { SYSTICK.raw() }, OVERSAMPLE>;
type RxQueue = Queue<u8, RX_BUF_LEN>;
// In heapless 0.9, `Producer`/`Consumer` are type-erased over the queue
// capacity (N is internal to the queue's storage).
type RxProducer = Producer<'static, u8>;
type RxConsumer = Consumer<'static, u8>;

static TICKS_10US: AtomicU32 = AtomicU32::new(0);

type RxPin = PA0<Input<PullUp>>;
type RxTimer = Timer<TIM2>;

static RX_HW: Mutex<RefCell<Option<RxHw<RxPin, RxTimer>>>> = Mutex::new(RefCell::new(None));
static UART: Mutex<RefCell<Uart>> = Mutex::new(RefCell::new(SoftUart::new()));
/// Producer half of the RX byte queue. Lives in a Mutex because the TIM2
/// ISR is the only writer and the cs already serializes it; the matching
/// Consumer half is local to `main` and accessed lock-free.
static RX_PRODUCER: Mutex<RefCell<Option<RxProducer>>> = Mutex::new(RefCell::new(None));

fn ticks_10us() -> u32 {
    TICKS_10US.load(Ordering::Relaxed)
}

/// Drain the SPSC queue without entering a critical section — the Consumer
/// is owned by main, the Producer by the TIM2 ISR, and `heapless::spsc`
/// makes that safe by design.
fn wait_until(deadline: u32, rx: &mut RxConsumer) {
    while ticks_10us().wrapping_sub(deadline) > u32::MAX / 2 {
        while let Some(b) = rx.dequeue() {
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

    configure_systick(cp.SYST, &clocks, SYSTICK);

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut rx_pin = gpioa
        .pa0
        .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr);

    rx_pin.make_interrupt_source(&mut dp.SYSCFG, &mut apb2);
    rx_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    rx_pin.enable_interrupt(&mut dp.EXTI);

    // TIM2 runs continuously at SAMPLE rate (no pause/resume). The ISR
    // is gated by RX_ACTIVE so it does nothing between frames — constant
    // WCET, no surprises when traffic starts/stops.
    let mut timer = Timer::tim2(dp.TIM2, SAMPLE, clocks, &mut apb1r1);
    timer.clear_update_interrupt_flag();
    timer.listen(Event::TimeOut);
    // Static SPSC RX queue. `Queue::new()` is const, but `split` needs
    // `&mut`, so the queue lives in a `static mut` and is split exactly
    // once here.
    static mut RX_QUEUE: RxQueue = Queue::new();
    let (producer, mut consumer) = unsafe { (&mut *core::ptr::addr_of_mut!(RX_QUEUE)).split() };

    free(|cs| {
        RX_HW.borrow(cs).replace(Some(RxHw::new(rx_pin, timer)));
        RX_PRODUCER.borrow(cs).replace(Some(producer));
    });

    unsafe {
        NVIC::unmask(Interrupt::TIM2);
        NVIC::unmask(Interrupt::EXTI0);
        cortex_m::interrupt::enable();
    }

    let mut loop_n: u32 = 0;
    let mut prev_frame: u32 = 0;
    let mut prev_err: u32 = 0;
    let mut prev_overrun: u32 = 0;
    let mut next_deadline = ticks_10us().wrapping_add(SYSTICK.raw());
    loop {
        loop_n = loop_n.wrapping_add(1);
        let (frame, errs, overrun) = free(|cs| {
            let uart = UART.borrow(cs).borrow();
            let hw = RX_HW.borrow(cs).borrow();
            let overrun = hw.as_ref().map_or(0, |h| h.overrun_count);
            (uart.frame_count(), uart.framing_errors(), overrun)
        });
        rprintln!(
            "loop={} frame_delta={} err_delta={} overrun_delta={}",
            loop_n,
            frame.wrapping_sub(prev_frame),
            errs.wrapping_sub(prev_err),
            overrun.wrapping_sub(prev_overrun),
        );
        prev_frame = frame;
        prev_err = errs;
        prev_overrun = overrun;

        wait_until(next_deadline, &mut consumer);
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
        // SoftUart flips its own `is_receiving` flag if this edge qualifies;
        // the TIM2 ISR doesn't need an external gate.
        UART.borrow(cs).borrow_mut().on_falling_edge(ticks_10us());
    });
}

#[interrupt]
fn TIM2() {
    free(|cs| {
        let mut hw_borrow = RX_HW.borrow(cs).borrow_mut();
        let hw = hw_borrow.as_mut().expect("RX_HW is not None");
        hw.timer.ack();
        if let Some(b) = UART.borrow(cs).borrow_mut().on_sample(hw.pin.is_high())
            && let Some(p) = RX_PRODUCER.borrow(cs).borrow_mut().as_mut()
            && p.enqueue(b).is_err()
        {
            hw.overrun_count = hw.overrun_count.wrapping_add(1);
        }
    });
}
