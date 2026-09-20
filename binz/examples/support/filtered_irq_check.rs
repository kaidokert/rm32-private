//! Disabled-only actual TIM2 source IRQ contract. No control or gate authority.
use super::*;
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ACK: AtomicBool = AtomicBool::new(false);
static VISITS: AtomicU32 = AtomicU32::new(0);
pub fn interrupt() {
    if !ACTIVE.load(Relaxed) {
        filtered_irq_hw::stop();
        return;
    }
    // One-shot even when intentionally retaining peripheral pending.
    let seen = filtered_irq_hw::pending() && filtered_irq_hw::enabled();
    filtered_irq_hw::mask();
    if ACK.load(Relaxed) {
        filtered_irq_hw::clear();
    }
    if seen {
        VISITS.store(VISITS.load(Relaxed) + 1, Relaxed);
    }
}
fn delay(us: u16) {
    let start = t17();
    while t17().wrapping_sub(start) < us {}
}
fn wait(n: u32) {
    let start = t17();
    while VISITS.load(Relaxed) < n && t17().wrapping_sub(start) < 500 {}
}
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || core_bench::active()
        || powered_timer::owns()
        || driven_run::owns()
    {
        let _ = writeln!(out, "FILTERSOURCE refused=1");
        return;
    }
    comp_input::stop();
    let mut bits = 0u32;
    let mut visits = 0;
    let mut restore_detail = [0u32; 3];
    let restored = unsafe {
        let r = &*stm32::RCC::ptr();
        let clock = r.apbenr1().read().bits();
        r.apbenr1().modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM2::ptr();
        if t.dier().read().bits() != 0 || t.cr1().read().bits() & 1 != 0 {
            r.apbenr1().write(|w| w.bits(clock));
            let _ = writeln!(out, "FILTERSOURCE refused=2");
            return;
        }
        let saved = [
            t.cr1().read().bits(),
            t.psc().read().bits(),
            t.arr().read().bits(),
            t.cnt().read().bits(),
            t.ccmr1_input().read().bits(),
            t.ccer().read().bits(),
            t.tisel().read().bits(),
            t.smcr().read().bits(),
            t.cr2().read().bits(),
        ];
        let csr = core::ptr::read_volatile(COMP2_CSR);
        t.cr2().write(|w| w.bits(0));
        t.smcr().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(31));
        t.arr().write(|w| w.bits(65535));
        t.egr().write(|w| w.bits(1));
        t.cnt().write(|w| w.bits(123));
        let before = filtered_irq_hw::refusals();
        if filtered_irq_hw::prepare() {
            bits |= 1;
            if t.psc().read().bits() == 31
                && t.arr().read().bits() == 65535
                && t.cnt().read().bits() == 123
                && t.smcr().read().bits() == 0
            {
                bits |= 2;
            }
            let base = (csr & !(15 << 4 | 1 << 15)) | (3 << 4) | 1;
            core::ptr::write_volatile(COMP2_CSR, base);
            delay(40);
            let low = base
                | if core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 {
                    1 << 15
                } else {
                    0
                };
            core::ptr::write_volatile(COMP2_CSR, low);
            delay(40);
            let ticket = filtered_irq_hw::before_mux().unwrap();
            delay(40);
            let armed = filtered_irq_hw::after_mux(ticket, true);
            t.cr1().modify(|r, w| w.bits(r.bits() | 1));
            delay(40);
            filtered_irq_hw::clear();
            VISITS.store(0, Relaxed);
            ACK.store(false, Relaxed);
            ACTIVE.store(true, Relaxed);
            core::ptr::write_volatile(COMP2_CSR, low ^ (1 << 15));
            delay(40);
            if armed
                && filtered_irq_hw::pending()
                && !filtered_irq_hw::enabled()
                && VISITS.load(Relaxed) == 0
            {
                bits |= 4;
            }
            let en = filtered_irq_hw::enable(ticket);
            wait(1);
            if en
                && VISITS.load(Relaxed) == 1
                && filtered_irq_hw::pending()
                && !filtered_irq_hw::enabled()
            {
                bits |= 8;
            }
            ACK.store(true, Relaxed);
            let en = filtered_irq_hw::enable(ticket);
            wait(2);
            if en && VISITS.load(Relaxed) == 2 && !filtered_irq_hw::pending() {
                bits |= 16;
            }
            // UIF from disabled setup UG must survive the backend's CC2 clears.
            if t.sr().read().bits() & 1 != 0 {
                bits |= 32;
            }
            let replacement = filtered_irq_hw::before_mux().unwrap();
            delay(40);
            if !filtered_irq_hw::after_mux(ticket, true)
                && !filtered_irq_hw::enable(ticket)
                && filtered_irq_hw::after_mux(replacement, true)
            {
                bits |= 64;
            }
            filtered_irq_hw::stop();
            if !filtered_irq_hw::enable(replacement)
                && !filtered_irq_hw::pending()
                && !filtered_irq_hw::enabled()
                && t.ccer().read().bits() & 16 == 0
            {
                bits |= 128;
            }
            ACTIVE.store(false, Relaxed);
            // Directly exercise the late-vector fallback after revocation.
            interrupt();
            delay(40);
            visits = VISITS.load(Relaxed);
            if visits == 2 && t.dier().read().bits() == 0 && t.sr().read().bits() & 4 == 0 {
                bits |= 256;
            }
            let after = filtered_irq_hw::refusals();
            if after[0] == before[0]
                && after[1] == before[1]
                && after[2] == before[2] + 1
                && after[3] == before[3] + 2
            {
                bits |= 512;
            }
        }
        ACTIVE.store(false, Relaxed);
        filtered_irq_hw::stop();
        t.cr1().write(|w| w.bits(0));
        core::ptr::write_volatile(COMP2_CSR, csr);
        t.psc().write(|w| w.bits(saved[1]));
        t.arr().write(|w| w.bits(saved[2]));
        t.egr().write(|w| w.bits(1));
        t.cnt().write(|w| w.bits(saved[3]));
        t.ccmr1_input().write(|w| w.bits(saved[4]));
        t.ccer().write(|w| w.bits(saved[5]));
        t.tisel().write(|w| w.bits(saved[6]));
        t.smcr().write(|w| w.bits(saved[7]));
        t.cr2().write(|w| w.bits(saved[8]));
        t.sr().write(|w| w.bits(0));
        t.cr1().write(|w| w.bits(saved[0]));
        let actual = [
            t.cr1().read().bits(),
            t.psc().read().bits(),
            t.arr().read().bits(),
            t.cnt().read().bits(),
            t.ccmr1_input().read().bits(),
            t.ccer().read().bits(),
            t.tisel().read().bits(),
            t.smcr().read().bits(),
            t.cr2().read().bits(),
        ];
        let csr_after = core::ptr::read_volatile(COMP2_CSR);
        let mut diff = 0;
        for i in 0..9 {
            if actual[i] != saved[i] {
                diff |= 1 << i;
            }
        }
        restore_detail = [diff, csr, csr_after];
        let ok = actual == saved
            && t.dier().read().bits() == 0
            && filtered_irq_source::same_comp_config(csr, csr_after);
        r.apbenr1().write(|w| w.bits(clock));
        ok
    };
    let _ = writeln!(
        out,
        "FILTERRESTORE timer_diff={} csr_saved={} csr_after={}",
        restore_detail[0], restore_detail[1], restore_detail[2]
    );
    let _ = writeln!(
        out,
        "FILTERSOURCE bits={} visits={} restored={} disabled={} gate_authority=0",
        bits,
        visits,
        restored as u8,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
