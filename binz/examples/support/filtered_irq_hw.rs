//! TIM2 CH2 interrupt-source backend; optional bench-filter-control integration.
//! Every state/register transaction is serialized. No CCR reads, CNT resets,
//! timer starts, output writes or software-generated commutation requests.
use super::*;
use filtered_irq_source::{CC2_ENABLE, CC2_IE, CLEAR_CC2, Source, Ticket, edge_bits};
#[cfg(all(feature = "bench-filter-one-us", feature = "bench-filter-bypass"))]
compile_error!("select one TIM2 filter experiment");
pub const FILTER_CODE: u8 = if cfg!(feature = "bench-filter-bypass") {
    0
} else if cfg!(feature = "bench-filter-one-us") {
    5
} else {
    12
};
static mut SOURCE: Source = Source::new();
static mut PREPARED: bool = false;
static mut ARMED: bool = false;
static mut CURRENT: Option<Ticket> = None;
// prepare refused, phase refused, arm refused, enable refused
static mut REFUSED: [u32; 4] = [0; 4];
// First source stop, before any backend masking/clearing. No CCR read.
static mut STOP_STATE: Option<[u32; 12]> = None;
unsafe fn source() -> &'static mut Source {
    unsafe { &mut *core::ptr::addr_of_mut!(SOURCE) }
}
unsafe fn refuse(index: usize) {
    unsafe {
        let p = core::ptr::addr_of_mut!(REFUSED).cast::<u32>().add(index);
        p.write(p.read().saturating_add(1));
    }
}
unsafe fn mask_locked() {
    unsafe {
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM2);
        (*stm32::TIM2::ptr())
            .dier()
            .modify(|r, w| w.bits(r.bits() & !CC2_IE));
        source().mask();
    }
}
unsafe fn clear_locked() {
    unsafe {
        // No SR read/modify/write: a concurrently arriving unrelated flag survives.
        (*stm32::TIM2::ptr()).sr().write(|w| w.bits(CLEAR_CC2));
        source().clear();
    }
}
unsafe fn stop_locked() {
    unsafe {
        if PREPARED && matches!(STOP_STATE, None) {
            let t = &*stm32::TIM2::ptr();
            STOP_STATE = Some([
                t.sr().read().bits(),
                t.dier().read().bits(),
                t.ccer().read().bits(),
                t.cr1().read().bits(),
                t.ccmr1_input().read().bits(),
                t.tisel().read().bits(),
                t.cnt().read().bits(),
                core::ptr::read_volatile(COMP2_CSR),
                ARMED as u32,
                source().enabled() as u32,
                cortex_m::peripheral::NVIC::is_enabled(stm32::Interrupt::TIM2) as u32,
                cortex_m::peripheral::NVIC::is_pending(stm32::Interrupt::TIM2) as u32,
            ]);
        }
        PREPARED = false;
        ARMED = false;
        CURRENT = None;
        source().stop();
        #[cfg(feature = "bench-filter-latency")]
        filter_latency::invalidate();
        mask_locked();
        (*stm32::TIM2::ptr())
            .ccer()
            .modify(|r, w| w.bits(r.bits() & !CC2_ENABLE));
        clear_locked();
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM2);
    }
}

/// Only after prior owner release and interval timer setup, with CEN/DIER zero.
/// A refusal does not mutate a timer that might still belong to another owner.
pub fn prepare() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM2::ptr();
        if !powered_timer::outputs_disabled()
            || driven_run::owns()
            || powered_timer::owns()
            || core_bench::active()
            || t.cr1().read().bits() & 1 != 0
            || t.dier().read().bits() != 0
        {
            refuse(0);
            return false;
        }
        stop_locked();
        STOP_STATE = None;
        let p = capture_filter::plan(2, FILTER_CODE).unwrap();
        t.ccer().modify(|r, w| w.bits(r.bits() & !0xf0));
        t.tisel()
            .modify(|r, w| w.bits((r.bits() & !0xf00) | p.tisel));
        t.ccmr1_input()
            .modify(|r, w| w.bits((r.bits() & !0xff00) | p.ccmr1));
        t.cr1().modify(|r, w| w.bits((r.bits() & !0x300) | p.ckd));
        #[cfg(feature = "bench-filter-latency")]
        filter_latency::prepare();
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::ADC_COMP);
        let e = &*stm32::EXTI::ptr();
        e.imr1().modify(|r, w| w.bits(r.bits() & !(1 << 18)));
        e.rpr1().write(|w| w.bits(1 << 18));
        e.fpr1().write(|w| w.bits(1 << 18));
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::ADC_COMP);
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(stm32::Interrupt::TIM2, 0x40);
        PREPARED = true;
        true
    })
}

/// Call before mux mutation. Returns a revocable one-phase capability.
pub fn before_mux() -> Option<Ticket> {
    cortex_m::interrupt::free(|_| unsafe {
        if !PREPARED {
            refuse(1);
            return None;
        }
        mask_locked();
        ARMED = false;
        CURRENT = None;
        #[cfg(feature = "bench-filter-latency")]
        filter_latency::invalidate();
        (*stm32::TIM2::ptr())
            .ccer()
            .modify(|r, w| w.bits(r.bits() & !CC2_ENABLE));
        clear_locked();
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM2);
        Some(source().phase())
    })
}
/// Only after the caller's mux/filter settling procedure. Does not enable IRQ.
pub fn after_mux(ticket: Ticket, rising: bool) -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        if !PREPARED || ARMED || !source().valid(ticket) {
            refuse(2);
            return false;
        }
        #[cfg(feature = "bench-filter-latency")]
        filter_latency::arm(rising);
        clear_locked();
        (*stm32::TIM2::ptr())
            .ccer()
            .modify(|r, w| w.bits((r.bits() & !0xf0) | edge_bits(rising)));
        ARMED = true;
        CURRENT = Some(ticket);
        true
    })
}
/// Keep peripheral pending intact: NVIC unpending is not event acknowledgement.
pub fn enable(ticket: Ticket) -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        if !PREPARED || !ARMED || !source().enable(ticket) {
            refuse(3);
            return false;
        }
        (*stm32::TIM2::ptr())
            .dier()
            .modify(|r, w| w.bits(r.bits() | CC2_IE));
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM2);
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM2);
        true
    })
}
pub fn mask() {
    cortex_m::interrupt::free(|_| unsafe { mask_locked() });
}
/// Controller callers already validate owner/status; recheck the phase locally.
pub fn enable_current() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        match CURRENT {
            Some(ticket) => enable(ticket),
            None => {
                refuse(3);
                false
            }
        }
    })
}
pub fn clear() {
    cortex_m::interrupt::free(|_| unsafe { clear_locked() });
}
pub fn pending() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        PREPARED && ARMED && (*stm32::TIM2::ptr()).sr().read().bits() & (1 << 2) != 0
    })
}
pub fn enabled() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        PREPARED
            && ARMED
            && source().enabled()
            && (*stm32::TIM2::ptr()).dier().read().bits() & CC2_IE != 0
    })
}
pub fn stop() {
    cortex_m::interrupt::free(|_| unsafe { stop_locked() });
}
pub fn refusals() -> [u32; 4] {
    cortex_m::interrupt::free(|_| unsafe { core::ptr::addr_of!(REFUSED).read() })
}
pub fn dump_stop<W: Write>(out: &mut W) {
    let row = cortex_m::interrupt::free(|_| unsafe { core::ptr::addr_of!(STOP_STATE).read() });
    if let Some(v) = row {
        let _ = writeln!(
            out,
            "FILTERSTOP sr={} dier={} ccer={} cr1={} ccmr1={} tisel={} cnt={} csr={} armed={} enabled={} nvic_enabled={} nvic_pending={} before_source_stop=1",
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11]
        );
    }
}
