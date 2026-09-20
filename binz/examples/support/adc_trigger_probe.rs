//! Disabled-only TIM3 TRGO -> ADC13 -> DMA1 CH1 finite transfer probe.
//! No driver wake or gate authority. Optional metrology build, not motor-qualified.
use super::*;
use core::sync::atomic::{Ordering, compiler_fence};

#[inline(never)]
pub fn run<W: Write>(out: &mut W, mode: u8, capture: bool) {
    let scan = mode != 0;
    let cyclic = mode >= 2;
    let stall = mode == 3;
    gates_off();
    set_pin(3, 1, false);
    if powered_timer::owns() || get_idr(3, 1) || !powered_timer::outputs_disabled() {
        let _ = writeln!(out, "ADCTRIGGER result=1");
        return;
    }
    let width = if scan { 5 } else { 1 };
    let words = 32 * width;
    let mut buffer = [0xffffu16; 160];
    let mut ring = [0xffffu16; 10];
    let mut copied = 0usize;
    let mut copy_max = 0u16;
    let mut lease_fault = 0u8;
    let mut reason = 0u32;
    let mut elapsed = 0u16;
    let remaining;
    let stopped;
    unsafe {
        let r = &*stm32::RCC::ptr();
        r.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 1)));
        r.ahbenr().modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM3::ptr();
        let adc = &*stm32::ADC::ptr();
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch1();
        let mux = &*stm32::DMAMUX::ptr();
        if t.cr1().read().bits() & 1 != 0
            || ch.cr().read().bits() & 1 != 0
            || adc.cr().read().bits() & (1 << 2) != 0
        {
            let _ = writeln!(out, "ADCTRIGGER result=2 resource_busy=1");
            return;
        }
        let old_cfg = adc.cfgr1().read().bits();
        let old_channels = adc.chselr0().read().bits();
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(100));
        t.cr2().write(|w| w.bits(2 << 4));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        adc.isr()
            .write(|w| w.bits((1 << 13) | (1 << 4) | (1 << 3) | (1 << 2)));
        // Bitmask sequencer is ascending: C,B,A,bus,VREF (0,1,4,6,13),
        // NOT the logical A,B,C order of the foreground reader.
        adc.chselr0().write(|w| {
            w.bits(if scan {
                (1 << 0) | (1 << 1) | (1 << 4) | (1 << 6) | (1 << 13)
            } else {
                1 << 13
            })
        });
        let wait = t17();
        while adc.isr().read().bits() & (1 << 13) == 0 && t17().wrapping_sub(wait) < 100 {}
        if adc.isr().read().bits() & (1 << 13) == 0 {
            reason = 3;
        }
        // RM0444 Table73: EXTSEL011=TIM3_TRGO. PAC enum name is stale.
        // ADC requests remain enabled across trigger sequences; DMA itself is
        // finite/non-circular and bounds the destination to32 complete scans.
        adc.cfgr1().write(|w| w.bits((1 << 10) | (3 << 6) | 3));
        mux.ccr(0).write(|w| w.bits(5));
        dma.ifcr().write(|w| w.bits(15));
        ch.par().write(|w| w.bits(adc.dr().as_ptr() as u32));
        ch.mar().write(|w| {
            w.bits(if cyclic {
                ring.as_mut_ptr() as u32
            } else {
                buffer.as_mut_ptr() as u32
            })
        });
        ch.ndtr()
            .write(|w| w.bits(if cyclic { 10 } else { words as u32 }));
        compiler_fence(Ordering::SeqCst);
        ch.cr().write(|w| {
            w.bits(
                (1 << 7) | (1 << 8) | (1 << 10) | (2 << 12) | 1 | if cyclic { 1 << 5 } else { 0 },
            )
        });
        if reason == 0 {
            adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
            let began = t17();
            t.cr1().write(|w| w.bits(1));
            // Deliberately miss more than two completions: flag latches must
            // not be mistaken for a fresh coherent snapshot afterward.
            if stall {
                while t17().wrapping_sub(began) < 350 {}
            }
            loop {
                elapsed = t17().wrapping_sub(began);
                if dma.isr().read().bits() & (1 << 3) != 0 {
                    reason = 4;
                    break;
                }
                if !cyclic && dma.isr().read().bits() & (1 << 1) != 0 {
                    break;
                }
                if elapsed >= 10_000 {
                    reason = 5;
                    break;
                }
                if get_idr(3, 1) || !powered_timer::outputs_disabled() {
                    reason = 6;
                    break;
                }
                if cyclic {
                    let copy_start = t17();
                    let flags = dma.isr().read().bits();
                    if flags & (dma_snapshot::HT | dma_snapshot::TC) == 0 {
                        continue;
                    }
                    let lease = match dma_snapshot::Lease::begin(
                        flags,
                        ch.ndtr().read().bits() as u16,
                        0,
                    ) {
                        Ok(v) => v,
                        Err(f) => {
                            reason = 9;
                            lease_fault = fault_code(f);
                            break;
                        }
                    };
                    dma.ifcr().write(|w| w.bits(lease.acknowledge_flag()));
                    compiler_fence(Ordering::SeqCst);
                    let mut raw = [0u16; 5];
                    for j in 0..5 {
                        raw[j] = ring.as_ptr().add(lease.word_offset() + j).read_volatile();
                    }
                    compiler_fence(Ordering::SeqCst);
                    let after = ch.ndtr().read().bits() as u16;
                    let flags_after = dma.isr().read().bits();
                    let cost = t17().wrapping_sub(copy_start);
                    copy_max = copy_max.max(cost);
                    if let Err(f) = lease.finish(flags_after, after, cost as u32, 0) {
                        reason = 9;
                        lease_fault = fault_code(f);
                        break;
                    }
                    buffer[copied * 5..copied * 5 + 5].copy_from_slice(&raw);
                    copied += 1;
                    if copied == 32 {
                        break;
                    }
                }
            }
        }
        t.cr1().write(|w| w.bits(0));
        if adc.cr().read().bits() & (1 << 2) != 0 {
            adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 4)));
            let stop = t17();
            while adc.cr().read().bits() & ((1 << 2) | (1 << 4)) != 0 {
                if t17().wrapping_sub(stop) >= 100 {
                    panic!("ADC trigger stop timeout");
                }
            }
        }
        ch.cr().write(|w| w.bits(0));
        compiler_fence(Ordering::SeqCst);
        remaining = ch.ndtr().read().bits();
        stopped = t.cr1().read().bits() & 1 == 0
            && ch.cr().read().bits() & 1 == 0
            && adc.cr().read().bits() & ((1 << 2) | (1 << 4)) == 0;
        if reason == 0
            && (!stopped || (!cyclic && remaining != 0) || adc.isr().read().bits() & (1 << 4) != 0)
        {
            reason = 7;
        }
        dma.ifcr().write(|w| w.bits(15));
        adc.cfgr1().write(|w| w.bits(old_cfg));
        adc.chselr0().write(|w| w.bits(old_channels));
        adc.isr().write(|w| w.bits((1 << 4) | (1 << 3) | (1 << 2)));
        t.cr2().write(|w| w.bits(0));
    }
    let mut low = u16::MAX;
    let mut high = 0;
    for i in 0..words {
        let raw = unsafe { buffer.as_ptr().add(i).read_volatile() };
        low = low.min(raw);
        high = high.max(raw);
    }
    if reason == 0
        && (!(3000..=4000).contains(&elapsed)
            || high > 4095
            || (!scan && (low == 0 || high == 4095)))
    {
        reason = 8;
    }
    if cyclic {
        let _ = writeln!(
            out,
            "ADCCYCLIC result={} copied={} elapsed_us={} remaining={} stopped={} stall={} lease_fault={} copy_max_us={} order=0,1,4,6,13 trigger_us=101 gate_authority=0",
            reason, copied, elapsed, remaining, stopped as u8, stall as u8, lease_fault, copy_max
        );
        if capture {
            for i in 0..copied {
                let mut row = [0u16; 6];
                row[0] = i as u16;
                row[1..].copy_from_slice(&buffer[i * 5..i * 5 + 5]);
                let _ = snapshot::record(out, "CS85", &row);
            }
        }
    } else if scan {
        let _ = writeln!(
            out,
            "ADCSCAN result={} scans=32 words=160 elapsed_us={} remaining={} stopped={} order=0,1,4,6,13 trigger_us=101 gate_authority=0",
            reason, elapsed, remaining, stopped as u8
        );
        if capture {
            for i in 0..32 {
                let mut row = [0u16; 6];
                row[0] = i as u16;
                for j in 0..5 {
                    row[j + 1] = unsafe { buffer.as_ptr().add(i * 5 + j).read_volatile() };
                }
                let _ = snapshot::record(out, "AS85", &row);
            }
        }
    } else {
        let _ = writeln!(
            out,
            "ADCTRIGGER result={} n=32 elapsed_us={} min={} max={} trigger_us=101 ext_sel=3 gate_authority=0",
            reason, elapsed, low, high
        );
    }
    gates_off();
    set_pin(3, 1, false);
}

fn fault_code(f: dma_snapshot::Fault) -> u8 {
    use dma_snapshot::Fault::*;
    match f {
        Transfer => 1,
        AmbiguousFlags => 2,
        Position => 3,
        Deadline => 4,
        Restarted => 5,
    }
}
