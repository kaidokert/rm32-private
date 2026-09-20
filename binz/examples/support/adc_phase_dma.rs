//! TIM3 update -> DMAch3 reads TIM1 CNT. Trigger-phase only, not ADC aperture.
//! Foreground configures/stops; existing ADC IRQ consumes one corresponding
//! phase word per scan. No extra interrupt or motor-output authority.
use super::*;
use core::sync::atomic::{Ordering, compiler_fence};
static mut RING: [u16; 2] = [0; 2];
static mut BINS: [u32; 32] = [0; 32];
static mut COUNT: u32 = 0;
static mut ACTIVE: bool = false;
// Retained with the histogram, even after bridge safing restores startup ARR.
static mut PERIOD: u32 = 6400;

pub fn prepare() -> bool {
    prepare_source(false)
}
fn prepare_source(self_counter: bool) -> bool {
    unsafe {
        let t = &*stm32::TIM3::ptr();
        let d = &*stm32::DMA1::ptr();
        let ch = d.ch3();
        if ACTIVE
            || t.cr1().read().bits() & 1 != 0
            || t.dier().read().bits() != 0
            || ch.cr().read().bits() & 1 != 0
        {
            return false;
        }
        let period = if self_counter {
            6400
        } else {
            (*stm32::TIM1::ptr()).arr().read().bits() + 1
        };
        if period != 6400
            && !(cfg!(feature = "bench-pwm-24k") && period == phase_role_live::CARRIER.ticks())
        {
            return false;
        }
        PERIOD = period;
        BINS = [0; 32];
        COUNT = 0;
        RING = [0; 2];
        // Cached stm32g0xx-hal/src/dmamux.rs: TIM3_UP=37.
        (*stm32::DMAMUX::ptr()).ccr(2).write(|w| w.bits(37));
        d.ifcr().write(|w| w.bits(15 << 8));
        let source = if self_counter {
            t.cnt().as_ptr()
        } else {
            (*stm32::TIM1::ptr()).cnt().as_ptr()
        };
        ch.par().write(|w| w.bits(source as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of_mut!(RING) as u32));
        ch.ndtr().write(|w| w.bits(2));
        compiler_fence(Ordering::SeqCst);
        // Halfword widths, circular/increment, high priority; NO DMA interrupts.
        ch.cr()
            .write(|w| w.bits(1 | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 10) | (2 << 12)));
        ACTIVE = true;
        t.dier().write(|w| w.bits(1 << 8));
        true
    }
}

/// Idle-only route/slot/latency check, no ADC or motor-load timing claim.
pub fn check<W: Write>(out: &mut W) {
    unsafe {
        if get_idr(3, 1) || !powered_timer::outputs_disabled() || powered_timer::owns() || ACTIVE {
            let _ = writeln!(out, "PHASECHECK refused=1");
            return;
        }
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 2));
        (*stm32::RCC::ptr())
            .ahbenr()
            .modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM3::ptr();
        if t.cr1().read().bits() & 1 != 0 || t.dier().read().bits() != 0 {
            let _ = writeln!(out, "PHASECHECK refused=2");
            return;
        }
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM3);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM3);
        t.cr2().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(200));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        if !prepare_source(true) {
            let _ = writeln!(out, "PHASECHECK refused=3");
            return;
        }
        #[cfg(feature = "bench-filter-raw")]
        let mut raw_ok = raw_capture::prepare();
        #[cfg(feature = "bench-filter-raw")]
        let mut raw_count = 0u32;
        let mut n = 0u32;
        let mut maximum = 0u16;
        let began = t17();
        t.cr1().write(|w| w.bits(1));
        while n < 32 && t17().wrapping_sub(began) < 10_000 {
            if get_idr(3, 1) || !powered_timer::outputs_disabled() {
                break;
            }
            if ((*stm32::DMA1::ptr()).isr().read().bits() >> 8) & 14 == 0 {
                continue;
            }
            if !consume(n) {
                break;
            }
            maximum = maximum.max(
                core::ptr::addr_of!(RING)
                    .cast::<u16>()
                    .add((n & 1) as usize)
                    .read_volatile(),
            );
            n += 1;
            #[cfg(feature = "bench-filter-raw")]
            {
                if !raw_ok || !raw_capture::disabled_pulse() {
                    raw_ok = false;
                    break;
                }
                raw_count += 1;
                raw_ok = t.psc().read().bits() == 63
                    && t.arr().read().bits() == 200
                    && t.cr1().read().bits() == 1
                    && t.cr2().read().bits() == 0
                    && t.dier().read().bits() == 1 << 8;
            }
        }
        t.cr1().write(|w| w.bits(0));
        stop();
        #[cfg(feature = "bench-filter-raw")]
        {
            raw_capture::stop();
            let _ = writeln!(
                out,
                "PHASERAW captures={} config_preserved={} adc_load=0 authority=0",
                raw_count, raw_ok as u8
            );
        }
        let disabled = !get_idr(3, 1) && powered_timer::outputs_disabled();
        let _ = writeln!(
            out,
            "PHASECHECK n={} max_counter_us={} passed={} disabled={} adc_load=0 gate_authority=0",
            n,
            maximum,
            (n == 32 && maximum <= 2 && disabled) as u8,
            disabled as u8
        );
    }
}

/// Called only after coherent ADC scan copy; index is zero-based ADC scan.
/// Refuse a mismatched phase stream, never silently associate another trigger.
pub fn consume(index: u32) -> bool {
    unsafe {
        let d = &*stm32::DMA1::ptr();
        let ch = d.ch3();
        if !ACTIVE || COUNT != index {
            return false;
        }
        let expected = if index & 1 == 0 { 4 } else { 2 };
        let remaining = if index & 1 == 0 { 1 } else { 2 };
        let flags = (d.isr().read().bits() >> 8) & 15;
        if flags & 14 != expected || ch.ndtr().read().bits() != remaining {
            return false;
        }
        d.ifcr().write(|w| w.bits(expected << 8));
        compiler_fence(Ordering::SeqCst);
        let phase = core::ptr::addr_of!(RING)
            .cast::<u16>()
            .add((index & 1) as usize)
            .read_volatile();
        compiler_fence(Ordering::SeqCst);
        if ch.ndtr().read().bits() != remaining
            || ((d.isr().read().bits() >> 8) & 14) != 0
            || phase as u32 >= PERIOD
        {
            return false;
        }
        let bin = match PERIOD {
            6400 => carrier_profile::Carrier::Khz10.phase_bin(phase as u32),
            3200 => carrier_profile::Carrier::Khz20.phase_bin(phase as u32),
            2666 => carrier_profile::Carrier::Khz24.phase_bin(phase as u32),
            2000 => carrier_profile::Carrier::Khz32.phase_bin(phase as u32),
            1600 => carrier_profile::Carrier::Khz40.phase_bin(phase as u32),
            1333 => carrier_profile::Carrier::Khz48.phase_bin(phase as u32),
            _ => return false,
        };
        let Some(bin) = bin else {
            return false;
        };
        (&mut *core::ptr::addr_of_mut!(BINS))[bin] += 1;
        COUNT += 1;
        true
    }
}

pub fn stop() {
    unsafe {
        if !ACTIVE {
            return;
        }
        (*stm32::TIM3::ptr())
            .dier()
            .modify(|r, w| w.bits(r.bits() & !(1 << 8)));
        (*stm32::DMA1::ptr()).ch3().cr().write(|w| w.bits(0));
        compiler_fence(Ordering::SeqCst);
        ACTIVE = false;
    }
}
pub fn dump<W: Write>(out: &mut W) {
    unsafe {
        let _ = writeln!(
            out,
            "ADCPHASE count={} bins=32 period_ticks={} trigger_only=1 aperture_known=0 sector_known=0 latency_qualified=0 stopped={}",
            COUNT as u32,
            PERIOD as u32,
            (!ACTIVE) as u8
        );
        for i in 0..32 {
            let n = (&*core::ptr::addr_of!(BINS))[i];
            let _ = snapshot::record(out, "AP85", &[i as u16, n as u16, (n >> 16) as u16]);
        }
    }
}
