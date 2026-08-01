//! Soft-UART RX on PA0 (header pin 4 / HSE_IN pad): host -> ESC bench
//! input while BF owns the signal wire and PB6 stays TX-only debug.
//!
//! Wire: USB-TTL TX -> PA0, GND common. 9600 8N1.
//!
//! Mechanism (proven in the minz bench era, git 8d2ff8c^): EXTI0
//! falling edge qualifies a start bit; LPTIM1 free-runs at
//! OVERSAMPLE x BAUD (38.4 kHz) and its CMP-match ISR feeds pin
//! samples to the pure decoder in [`crate::softuart`]. Between frames
//! the decoder's `is_receiving` flag makes the sample ISR a ~10-cycle
//! no-op — constant, negligible idle cost.
//!
//! Priorities: EXTI0 and LPTIM1 both run at NVIC level 3 (lowest,
//! with TIM6) — they can never preempt each other or the motor-
//! critical ISRs, which is also the soundness argument for the
//! shared `UnsafeCell` decoder state (single-writer: same-priority
//! ISRs serialize; main never touches the cell — it drains the SPSC
//! consumer half only).
//!
//! Poll law: soft-UART traffic DURING measured runs adds ISR load
//! (38.4 kHz while a byte is in flight) — same rule as PB6 prints,
//! don't send during measured holds.

use core::cell::UnsafeCell;

use heapless::spsc::{Consumer, Queue};

use crate::pac;
use crate::softuart::{RX_QUEUE_LEN, SoftUart};

pub const BAUD: u32 = 9_600;
pub const OVERSAMPLE: usize = 4;
/// Edge-timestamp tick rate fed to the decoder's frame-gap logic:
/// DWT.CYCCNT / 800 at 80 MHz = 100 kHz.
pub const TICK_HZ: u32 = 100_000;
const CYCLES_PER_TICK: u32 = 80_000_000 / TICK_HZ;
/// LPTIM1 runs from PCLK1 (80 MHz), CCIPR default source.
const LPTIM_ARR: u16 = (80_000_000 / (BAUD * OVERSAMPLE as u32) - 1) as u16;

pub type Uart = SoftUart<'static, BAUD, TICK_HZ, OVERSAMPLE>;
type RxQueue = Queue<u8, RX_QUEUE_LEN>;

/// Same-priority-ISR cell (see module doc for the soundness argument;
/// mirrors `isr_handlers::IsrCell`).
struct UartCell(UnsafeCell<Option<Uart>>);
// SAFETY: written only from EXTI0/LPTIM1 ISRs at one NVIC priority
// level on a single core; main drains the SPSC consumer, not this.
unsafe impl Sync for UartCell {}

static UART: UartCell = UartCell(UnsafeCell::new(None));
static mut RX_QUEUE: RxQueue = Queue::new();

fn cyccnt() -> u32 {
    unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() }
}

/// One-time init from main context, before interrupts matter.
/// Returns the consumer half — main drains decoded bytes from it
/// lock-free. Panics if called twice.
pub fn init() -> Consumer<'static, u8, RX_QUEUE_LEN> {
    let rcc = unsafe { &*pac::RCC::PTR };
    let gpioa = unsafe { &*pac::GPIOA::PTR };
    let exti = unsafe { &*pac::EXTI::PTR };
    let lptim = unsafe { &*pac::LPTIM1::PTR };

    // PA0: input + pull-up (idle-high UART line).
    gpioa.moder.modify(|_, w| w.moder0().input());
    gpioa.pupdr.modify(|_, w| w.pupdr0().pull_up());

    // EXTI line 0 <- PA0 (SYSCFG EXTICR1 reset default selects port A),
    // falling edge, unmasked.
    exti.ftsr1.modify(|r, w| unsafe { w.bits(r.bits() | 1) });
    exti.imr1.modify(|r, w| unsafe { w.bits(r.bits() | 1) });

    // LPTIM1 from PCLK1, continuous, CMP-match IRQ at mid-period.
    // LPTIM quirks: IER writes need ENABLE=0; ARR/CMP writes need
    // ENABLE=1 (sync'd via ARROK/CMPOK).
    rcc.apb1enr1.modify(|_, w| w.lptim1en().set_bit());
    lptim.ier.write(|w| w.cmpmie().set_bit());
    lptim.cr.modify(|_, w| w.enable().set_bit());
    lptim.arr.write(|w| unsafe { w.bits(LPTIM_ARR as u32) });
    while lptim.isr.read().arrok().bit_is_clear() {}
    lptim
        .cmp
        .write(|w| unsafe { w.bits((LPTIM_ARR / 2) as u32) });
    while lptim.isr.read().cmpok().bit_is_clear() {}
    lptim.cr.modify(|_, w| w.cntstrt().set_bit());

    // SAFETY: init runs once from main before the IRQs are unmasked.
    let (producer, consumer) = unsafe {
        let q = &mut *core::ptr::addr_of_mut!(RX_QUEUE);
        q.split()
    };
    let cell = unsafe { &mut *UART.0.get() };
    assert!(cell.is_none(), "softuart_rx::init called twice");
    *cell = Some(SoftUart::new(producer));
    consumer
}

/// EXTI0 ISR body: ack the line, qualify the falling edge.
pub fn on_exti0() {
    let exti = unsafe { &*pac::EXTI::PTR };
    exti.pr1.write(|w| unsafe { w.bits(1) });
    if let Some(uart) = unsafe { &mut *UART.0.get() } {
        uart.on_falling_edge(cyccnt() / CYCLES_PER_TICK);
    }
}

/// LPTIM1 ISR body: ack CMP-match, feed one pin sample.
pub fn on_lptim1() {
    let lptim = unsafe { &*pac::LPTIM1::PTR };
    lptim.icr.write(|w| w.cmpmcf().set_bit());
    if let Some(uart) = unsafe { &mut *UART.0.get() } {
        let gpioa = unsafe { &*pac::GPIOA::PTR };
        let high = gpioa.idr.read().idr0().bit_is_set();
        uart.on_sample(high);
    }
}

/// Decoder health counters (framing errors, frames, overruns) — read
/// from main for the debug heartbeat. Racy-read tolerable (u32 loads).
pub fn counters() -> (u32, u32, u32) {
    match unsafe { &*UART.0.get() } {
        Some(u) => (u.frame_count(), u.framing_errors(), u.overrun_count()),
        None => (0, 0, 0),
    }
}
