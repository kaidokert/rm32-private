//! Finite TIM1_CH4 -> DMA1 CH2 capture. No PWM output or driver authority.
//! First qualify request-to-read latency using TIM1 CNT as the source.
use super::*;
use core::sync::atomic::{Ordering, compiler_fence};
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
const N: usize = 64;
static mut BUFFER: [u32; N] = [0; N];
const CN: usize = 256;
static mut COMP: [u32; CN] = [0; CN];
static ACTIVE: AtomicBool = AtomicBool::new(false);
static TARGET: AtomicU32 = AtomicU32::new(192);
static APPLIED: AtomicU32 = AtomicU32::new(192);
pub fn set_target(target: u32) -> bool {
    if !matches!(target, 192 | 320) || !disabled() || ACTIVE.load(Relaxed) {
        return false;
    }
    TARGET.store(target, Relaxed);
    true
}
pub fn stop() {
    if !ACTIVE.swap(false, Relaxed) {
        return;
    }
    unsafe {
        (*stm32::TIM1::ptr())
            .dier()
            .modify(|r, w| w.bits(r.bits() & !(1 << 12)));
        (*stm32::DMA1::ptr())
            .ch2()
            .cr()
            .modify(|r, w| w.bits(r.bits() & !1));
    }
    compiler_fence(Ordering::SeqCst);
}
fn disabled() -> bool {
    !get_idr(3, 1) && powered_timer::outputs_disabled()
}
pub fn prepare_comp(duty: u32) -> bool {
    unsafe {
        let target = TARGET.load(Relaxed);
        // Retain measured0.5us latency margin inside the commanded ON pulse.
        if target + 32 >= 6400 * duty.min(100) / 1000 {
            return false;
        }
        let tim = &*stm32::TIM1::ptr();
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch2();
        (*stm32::RCC::ptr())
            .ahbenr()
            .modify(|r, w| w.bits(r.bits() | 1));
        if ACTIVE.load(Relaxed)
            || ch.cr().read().bits() & 1 != 0
            || tim.dier().read().bits() & (1 << 12) != 0
        {
            return false;
        }
        tim.cr2().modify(|r, w| w.bits(r.bits() & !(1 << 3)));
        tim.ccmr2_output().modify(|r, w| w.bits(r.bits() & 0xff));
        tim.ccr4().write(|w| w.bits(target));
        APPLIED.store(target, Relaxed);
        (*stm32::DMAMUX::ptr()).ccr(1).write(|w| w.bits(23));
        ch.cr().write(|w| w.bits(0));
        dma.ifcr().write(|w| w.bits(15 << 4));
        ch.par().write(|w| w.bits(COMP2_CSR as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of_mut!(COMP) as u32));
        ch.ndtr().write(|w| w.bits(CN as u32));
        compiler_fence(Ordering::SeqCst);
        true
    }
}
pub fn start_comp() {
    unsafe {
        ACTIVE.store(true, Relaxed);
        (*stm32::DMA1::ptr())
            .ch2()
            .cr()
            .write(|w| w.bits((2 << 12) | (2 << 10) | (2 << 8) | (1 << 7) | 1));
        (*stm32::TIM1::ptr())
            .dier()
            .modify(|r, w| w.bits(r.bits() | (1 << 12)));
    }
}
pub fn count() -> u16 {
    unsafe { (CN as u32 - (*stm32::DMA1::ptr()).ch2().ndtr().read().bits()) as u16 }
}
pub fn healthy() -> bool {
    unsafe {
        ACTIVE.load(Relaxed)
            && (*stm32::DMA1::ptr()).isr().read().bits() & (1 << 7) == 0
            && count() < CN as u16
    }
}
pub fn dump_comp<W: Write>(out: &mut W) {
    let n = count().min(CN as u16);
    let flags = unsafe { ((*stm32::DMA1::ptr()).isr().read().bits() >> 4) & 15 };
    let _ = writeln!(
        out,
        "PWMCOMP n={} target={} flags={} stopped={} source_comp2_csr=1 handoff_authority=0",
        n,
        APPLIED.load(Relaxed),
        flags,
        (!ACTIVE.load(Relaxed)) as u8
    );
    for i in 0..n as usize {
        let v = unsafe {
            core::ptr::addr_of!(COMP)
                .cast::<u32>()
                .add(i)
                .read_volatile()
        };
        let _ = snapshot::record(out, "PC85", &[i as u16, v as u16, (v >> 16) as u16]);
    }
}
/// No public arbitrary-address DMA API until timing/ownership are qualified.
pub fn timing<W: Write>(out: &mut W, vcal: u32) {
    gates_off();
    set_pin(3, 1, false);
    if !disabled() {
        let _ = writeln!(out, "PWMDMA refused=1");
        return;
    }
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch2();
        (*stm32::RCC::ptr())
            .ahbenr()
            .modify(|r, w| w.bits(r.bits() | 1));
        if ch.cr().read().bits() & 1 != 0
            || tim.dier().read().bits() & (1 << 12) != 0
            || tim.cr1().read().bits() & 1 == 0
            || tim.arr().read().bits() != 6399
            || tim.psc().read().bits() != 0
        {
            let _ = writeln!(out, "PWMDMA refused=2");
            return;
        }
        let saved = (
            tim.ccr4().read().bits(),
            tim.ccmr2_output().read().bits(),
            tim.cr2().read().bits(),
            (*stm32::DMAMUX::ptr()).ccr(1).read().bits(),
        );
        // Output compare/frozen,CH4 external pin disabled. CCDS=0 selects CC4
        // requests,not update requests. Existing CH1..3 gate plans untouched.
        tim.ccmr2_output().modify(|r, w| w.bits(r.bits() & 0xff));
        tim.cr2().modify(|r, w| w.bits(r.bits() & !(1 << 3)));
        for target in [192u32, 320] {
            let mut reason = 0;
            let mut scans = 0;
            for i in 0..N {
                core::ptr::addr_of_mut!(BUFFER)
                    .cast::<u32>()
                    .add(i)
                    .write_volatile(u32::MAX);
            }
            tim.ccr4().write(|w| w.bits(target));
            // HAL's G071 DmaMuxIndex::TIM1_CH4=23. CH2 uses mux channel1.
            (*stm32::DMAMUX::ptr()).ccr(1).write(|w| w.bits(23));
            ch.cr().write(|w| w.bits(0));
            dma.ifcr().write(|w| w.bits(15 << 4));
            ch.par().write(|w| w.bits(tim.cnt().as_ptr() as u32));
            ch.mar()
                .write(|w| w.bits(core::ptr::addr_of_mut!(BUFFER) as u32));
            ch.ndtr().write(|w| w.bits(N as u32));
            compiler_fence(Ordering::SeqCst);
            // 32-bit peripheral/memory,increment memory,finite,no interrupt.
            ch.cr()
                .write(|w| w.bits((2 << 12) | (2 << 10) | (2 << 8) | (1 << 7) | 1));
            ACTIVE.store(true, Relaxed);
            let began = t17();
            tim.dier().modify(|r, w| w.bits(r.bits() | (1 << 12)));
            while ch.ndtr().read().bits() != 0 {
                if !disabled() {
                    reason = 1;
                    break;
                }
                if dma.isr().read().bits() & (1 << 7) != 0 {
                    reason = 3;
                    break;
                }
                if t17().wrapping_sub(began) > 7000 {
                    reason = 4;
                    break;
                }
                // Real foreground ADC traffic exercises APB/AHB contention.
                // This is NOT a qualification of concurrent motor ISRs.
                if powered_timer::sample_feedback(vcal).is_none() {
                    reason = 5;
                    break;
                }
                scans += 1;
            }
            let elapsed = t17().wrapping_sub(began);
            let flags = (dma.isr().read().bits() >> 4) & 15;
            stop();
            let n = N - ch.ndtr().read().bits() as usize;
            // Shared safing must also leave request/channel off,without changing
            // the finite capture. Later timer periods must not change NDTR.
            gates_off();
            set_pin(3, 1, false);
            let remaining = ch.ndtr().read().bits();
            let wait = t17();
            while t17().wrapping_sub(wait) < 250 {}
            let stopped = ch.cr().read().bits() & 1 == 0
                && tim.dier().read().bits() & (1 << 12) == 0
                && remaining == ch.ndtr().read().bits();
            let _ = writeln!(
                out,
                "PWMDMA reason={} target={} n={} elapsed_us={} adc_scans={} flags={} stopped={} disabled={} source_tim1_cnt=1 gate_authority=0",
                reason,
                target,
                n,
                elapsed,
                scans,
                flags,
                stopped as u8,
                disabled() as u8
            );
            for i in 0..n {
                let v = core::ptr::addr_of!(BUFFER)
                    .cast::<u32>()
                    .add(i)
                    .read_volatile();
                let _ = snapshot::record(out, "PM85", &[i as u16, v as u16, (v >> 16) as u16]);
            }
            let _ = writeln!(out, "PWMDMA END");
            if reason != 0 || !stopped || !disabled() {
                break;
            }
        }
        stop();
        tim.ccr4().write(|w| w.bits(saved.0));
        tim.ccmr2_output().write(|w| w.bits(saved.1));
        tim.cr2().write(|w| w.bits(saved.2));
        (*stm32::DMAMUX::ptr()).ccr(1).write(|w| w.bits(saved.3));
        dma.ifcr().write(|w| w.bits(15 << 4));
    }
    gates_off();
    set_pin(3, 1, false);
}
