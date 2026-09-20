//! ENABLE-low polled capture diagnostic. Never enables TIM2 or COMP interrupts.
use super::*;
fn delay(us: u16) {
    let start = t17();
    while t17().wrapping_sub(start) < us {}
}
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || powered_timer::owns()
        || core_bench::active()
        || !powered_timer::outputs_disabled()
    {
        let _ = writeln!(out, "FILTERCHECK refused=1 gate_authority=0");
        return;
    }
    gates_off();
    set_pin(3, 1, false);
    comp_input::stop();
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM2);
    let mut rows = [[0u32; 8]; 8];
    let mut timing = [[0u32; 4]; 8];
    let mut pairs = [[0u32; 3]; 8];
    let mut raw3 = [[0u32; 4]; 8];
    let mut ack = [[0u32; 3]; 8];
    let restored = unsafe {
        let r = &*stm32::RCC::ptr();
        let clock = r.apbenr1().read().bits();
        r.apbenr1().modify(|r, w| w.bits(r.bits() | 3));
        let t = &*stm32::TIM2::ptr();
        let t3 = &*stm32::TIM3::ptr();
        // Refuse another configured interrupt/DMA owner, even while outputs off.
        if t.dier().read().bits() != 0
            || t3.dier().read().bits() != 0
            || t3.cr1().read().bits() & 1 != 0
        {
            r.apbenr1().write(|w| w.bits(clock));
            let _ = writeln!(out, "FILTERCHECK refused=2 gate_authority=0");
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
            t.ccr2().read().bits(),
            t.smcr().read().bits(),
            t.ccr1().read().bits(),
        ];
        let csr = core::ptr::read_volatile(COMP2_CSR);
        let saved3 = [
            t3.cr1().read().bits(),
            t3.cr2().read().bits(),
            t3.psc().read().bits(),
            t3.arr().read().bits(),
            t3.cnt().read().bits(),
            t3.ccmr1_input().read().bits(),
            t3.ccer().read().bits(),
            t3.tisel().read().bits(),
            t3.ccr2().read().bits(),
            t3.smcr().read().bits(),
        ];
        t3.ccer().write(|w| w.bits(0));
        t3.cr1().write(|w| w.bits(0));
        // No ADC trigger/DMA/IRQ in this disabled diagnostic. Representative
        // 1MHz/201us counter; independent unfiltered TI2 on TIM3.
        t3.cr2().write(|w| w.bits(0));
        t3.smcr().write(|w| w.bits(0));
        t3.psc().write(|w| w.bits(63));
        t3.arr().write(|w| w.bits(200));
        t3.ccmr1_input().write(|w| w.bits(1 << 8));
        t3.tisel().write(|w| w.bits(1 << 8));
        t3.egr().write(|w| w.bits(1));
        t3.sr().write(|w| w.bits(0));
        t3.cr1().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.smcr().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(31));
        t.arr().write(|w| w.bits(65535));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        // Stable internal VREFINT INM; neutral INP unchanged, polarity forces edge.
        let base = (csr & !(15 << 4 | 1 << 15)) | (3 << 4) | 1;
        core::ptr::write_volatile(COMP2_CSR, base);
        delay(40);
        let low = base
            | if core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 {
                1 << 15
            } else {
                0
            };
        for (index, (filter, width)) in [
            (0, 2u16),
            (0, 40),
            (15, 2),
            (15, 40),
            (12, 2),
            (12, 40),
            (5, 2),
            (5, 40),
        ]
        .into_iter()
        .enumerate()
        {
            let p = capture_filter::plan(2, filter).unwrap();
            t.ccer().write(|w| w.bits(0));
            t.tisel().write(|w| w.bits(p.tisel));
            // Test indirect CH1<-TI2 with IC1F0 alongside direct CH2.
            // Do NOT assume indirect capture bypasses the TI2 filter.
            t.ccmr1_input().write(|w| w.bits(p.ccmr1 | 2));
            t.cr1().write(|w| w.bits(p.ckd | 1));
            core::ptr::write_volatile(COMP2_CSR, low);
            delay(40);
            t.ccer().write(|w| w.bits(1 << 4 | 1)); // CH1/2 rising, every edge
            t.sr().write(|w| w.bits(0));
            t3.ccer().write(|w| w.bits(1 << 4));
            t3.sr().write(|w| w.bits(0));
            let (elapsed, levels, before, after, before3) = cortex_m::interrupt::free(|_| {
                let pre = core::ptr::read_volatile(COMP2_CSR) >> 30 & 1;
                let start = t17();
                let before = t.cnt().read().bits();
                let before3 = t3.cnt().read().bits();
                core::ptr::write_volatile(COMP2_CSR, low ^ (1 << 15));
                delay(width);
                let high = core::ptr::read_volatile(COMP2_CSR) >> 30 & 1;
                core::ptr::write_volatile(COMP2_CSR, low);
                let after = t.cnt().read().bits();
                (
                    t17().wrapping_sub(start),
                    pre | (high << 1),
                    before,
                    after,
                    before3,
                )
            });
            delay(40);
            let sr = t.sr().read().bits();
            // Prove mirror CCR1 consumption cannot acknowledge authority CC2.
            let mirror = t.ccr1().read().bits();
            let after_mirror = t.sr().read().bits();
            let capture = t.ccr2().read().bits();
            let after_source = t.sr().read().bits();
            ack[index] = [sr & 6, after_mirror & 6, after_source & 6];
            pairs[index] = [(sr >> 1) & 1, (sr >> 9) & 1, mirror];
            let sr3 = t3.sr().read().bits();
            raw3[index] = [
                (sr3 >> 2) & 1,
                (sr3 >> 10) & 1,
                t3.ccr2().read().bits(),
                before3,
            ];
            rows[index] = [
                filter as u32,
                width as u32,
                elapsed as u32,
                levels,
                (sr >> 2) & 1,
                (sr >> 10) & 1,
                capture,
                (!get_idr(3, 1)) as u32,
            ];
            timing[index] = [
                before,
                after,
                elapsed as u32,
                core::ptr::read_volatile(COMP2_CSR) >> 30 & 1,
            ];
        }
        t.cr1().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        core::ptr::write_volatile(COMP2_CSR, csr);
        comp_input::stop();
        t.psc().write(|w| w.bits(saved[1]));
        t.arr().write(|w| w.bits(saved[2]));
        t.egr().write(|w| w.bits(1));
        t.ccmr1_input().write(|w| w.bits(saved[4]));
        t.ccr2().write(|w| w.bits(saved[7]));
        t.tisel().write(|w| w.bits(saved[6]));
        t.ccr1().write(|w| w.bits(saved[9]));
        t.smcr().write(|w| w.bits(saved[8]));
        t.cnt().write(|w| w.bits(saved[3]));
        t.sr().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(saved[5]));
        t.cr1().write(|w| w.bits(saved[0]));
        t3.cr1().write(|w| w.bits(0));
        t3.ccer().write(|w| w.bits(0));
        t3.psc().write(|w| w.bits(saved3[2]));
        t3.arr().write(|w| w.bits(saved3[3]));
        t3.egr().write(|w| w.bits(1));
        t3.ccmr1_input().write(|w| w.bits(saved3[5]));
        t3.tisel().write(|w| w.bits(saved3[7]));
        t3.ccr2().write(|w| w.bits(saved3[8]));
        t3.smcr().write(|w| w.bits(saved3[9]));
        t3.cnt().write(|w| w.bits(saved3[4]));
        t3.sr().write(|w| w.bits(0));
        t3.cr2().write(|w| w.bits(saved3[1]));
        t3.ccer().write(|w| w.bits(saved3[6]));
        t3.cr1().write(|w| w.bits(saved3[0]));
        let restored3 = [
            t3.cr1().read().bits(),
            t3.cr2().read().bits(),
            t3.psc().read().bits(),
            t3.arr().read().bits(),
            t3.cnt().read().bits(),
            t3.ccmr1_input().read().bits(),
            t3.ccer().read().bits(),
            t3.tisel().read().bits(),
            t3.ccr2().read().bits(),
            t3.smcr().read().bits(),
        ];
        let ok = restored3 == saved3
            && t3.dier().read().bits() == 0
            && t.psc().read().bits() == saved[1]
            && t.arr().read().bits() == saved[2]
            && t.tisel().read().bits() == saved[6]
            && t.ccmr1_input().read().bits() == saved[4]
            && t.cr1().read().bits() == saved[0]
            && t.ccer().read().bits() == saved[5]
            && t.smcr().read().bits() == saved[8]
            && (core::ptr::read_volatile(COMP2_CSR) & !(1 << 30)) == (csr & !(1 << 30))
            && t.dier().read().bits() == 0;
        r.apbenr1().write(|w| w.bits(clock));
        ok
    };
    gates_off();
    set_pin(3, 1, false);
    for r in rows {
        let _ = writeln!(
            out,
            "FILTERPART code={} requested_us={} measured_us={} levels={} captured={} overcapture={} ccr={} en_low={}",
            r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]
        );
    }
    for (i, r) in timing.into_iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERTIME index={} before={} after={} elapsed_us={} post={}",
            i, r[0], r[1], r[2], r[3]
        );
    }
    for (i, r) in pairs.into_iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERPAIR index={} indirect_captured={} indirect_over={} indirect_ccr={} ic1f=0 cc1s=2",
            i, r[0], r[1], r[2]
        );
    }
    for (i, r) in ack.into_iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERACK index={} before={} after_mirror={} after_source={} source_cc2=1 mirror_cc1=1",
            i, r[0], r[1], r[2]
        );
    }
    for (i, r) in raw3.into_iter().enumerate() {
        let _ = writeln!(
            out,
            "FILTERT3 index={} captured={} over={} ccr={} before={} period=201 tick_us=1 filter=0",
            i, r[0], r[1], r[2], r[3]
        );
    }
    let _ = writeln!(
        out,
        "FILTERCHECK restored={} disabled={} irq_enabled=0 gate_authority=0",
        restored as u8,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
