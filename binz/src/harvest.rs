//! The 10 kHz ADC harvest + ISR-class guards + TIM17 timebase.
//!
//! TIM6 update (10 kHz) -> triggers one ADC scan of {VM, VPH2, VPH1, VPH3,
//! IS, NTC} (bitmask sequencer, ascending: ch 1,8,9,10,15,17) -> DMA1_CH1
//! circular -> transfer-complete ISR runs the guards and aggregation.
//! Kills fire IN THE ISR (stage::force_safe) within ~100 us of the sample
//! that tripped them. TIM17 free-runs at 1 MHz as the cycle timebase
//! (Cortex-M0+ has no DWT).
//!
//! All raw PAC (the HAL layers are scarred; see CLAUDE.md).

use crate::{blackbox, stage};
use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering::Relaxed};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::interrupt;

// ---- shared state (single-writer ISR, word-atomic loads elsewhere) ----
pub static TICKS_100US: AtomicU32 = AtomicU32::new(0);
pub static VM_MV: AtomicU16 = AtomicU16::new(0);
pub static IS_MV: AtomicU16 = AtomicU16::new(0);
pub static VPH1_MV: AtomicU16 = AtomicU16::new(0);
pub static VPH2_MV: AtomicU16 = AtomicU16::new(0);
pub static VPH3_MV: AtomicU16 = AtomicU16::new(0);
pub static NTC_MV: AtomicU16 = AtomicU16::new(0);

/// 0 = alive; else a blackbox KIND_KILL_* code.
pub static KILL: AtomicU8 = AtomicU8::new(0);

// Guard configuration (main writes while disarmed / to provoke).
pub static ARMED: AtomicU8 = AtomicU8::new(0);
pub static IS_BASE_MV: AtomicU16 = AtomicU16::new(1650);
// AVERAGE-current guard (EWMA of IS ~ DC-link mean ~ PSU current):
// ~60 mV/A, so 120 mV delta ~ 2 A average -- well above the ~0.56 A the
// proven 7% spin draws, below anything thermally dangerous.
pub static IS_KILL_DELTA_MV: AtomicU16 = AtomicU16::new(500); // hard backstop, above
// the dwell soft-abort (300). NOTE: the DC-link shunt AVERAGE overstates
// PSU current during slip transients (the bus cap supplies the surge), so
// bus-sag is the unambiguous envelope edge on a current-limited supply --
// this stays a coarse fault net, not the primary limit.
// Instantaneous PEAK ceiling: HIGH backstop only. Normal PWM pulse peak
// into this low-L motor is already ~6 A (360 mV) at 7% duty -- the peak
// current is NOT the overcurrent signal (the EWMA average is). Genuine
// shorts trip the STDRIVE's own VDS/overcurrent -> nFLT far faster than
// we could here, so this only catches a runaway the hardware missed.
pub static IS_PEAK_CEIL_MV: AtomicU16 = AtomicU16::new(750); // ~12.5 A
pub static IS_AVG_MV: AtomicU16 = AtomicU16::new(1650); // published EWMA
pub static VM_FLOOR_MV: AtomicU16 = AtomicU16::new(6000); // boot-relative, set at arm
pub static NTC_KILL_MV: AtomicU16 = AtomicU16::new(900); // ~60 C

// Rung aggregates (reset by main between rungs via reset_aggregates()).
pub static AGG_IS_MIN: AtomicU16 = AtomicU16::new(u16::MAX);
pub static AGG_IS_MAX: AtomicU16 = AtomicU16::new(0);
pub static AGG_IS_SUM: AtomicU32 = AtomicU32::new(0);
pub static AGG_VM_MIN: AtomicU16 = AtomicU16::new(u16::MAX);
pub static AGG_VM_MAX: AtomicU16 = AtomicU16::new(0);
pub static AGG_VM_SUM: AtomicU32 = AtomicU32::new(0);
pub static AGG_N: AtomicU32 = AtomicU32::new(0);

// Coast capture: MODE=1 -> ISR records VPH1 mV until the buffer fills.
pub static COAST_MODE: AtomicU8 = AtomicU8::new(0);
pub static COAST_IDX: AtomicU16 = AtomicU16::new(0);
pub const COAST_LEN: usize = 160; // ~16 ms @ 9.9 kHz -- a low-inertia prop
// coasts to a stop in a few ms, so the
// BEMF must be caught immediately.
static mut COAST_BUF: [u16; COAST_LEN] = [0; COAST_LEN];

const SEQ_LEN: usize = 6;
static mut DMA_BUF: [u16; SEQ_LEN] = [0; SEQ_LEN];

static SAG_STRIKES: AtomicU8 = AtomicU8::new(0);
static PEAK_STRIKES: AtomicU8 = AtomicU8::new(0);
static IS_EWMA_Q4: AtomicU32 = AtomicU32::new(0); // IS_avg << 4 fixed-point
const SAG_STRIKES_KILL: u8 = 8; // VM is continuous; consecutive is fine
const PEAK_STRIKES_KILL: u8 = 4; // 4 samples over the hard peak ceiling

/// Read the coast buffer after a capture (main context, COAST_MODE back
/// to 0 and index at COAST_LEN, i.e. capture finished).
pub fn coast_samples() -> &'static [u16; COAST_LEN] {
    unsafe { &*core::ptr::addr_of!(COAST_BUF) }
}

pub fn reset_aggregates() {
    cortex_m::interrupt::free(|_| {
        AGG_IS_MIN.store(u16::MAX, Relaxed);
        AGG_IS_MAX.store(0, Relaxed);
        AGG_IS_SUM.store(0, Relaxed);
        AGG_VM_MIN.store(u16::MAX, Relaxed);
        AGG_VM_MAX.store(0, Relaxed);
        AGG_VM_SUM.store(0, Relaxed);
        AGG_N.store(0, Relaxed);
    });
}

/// TIM17 CNT: free-running 1 MHz, 16-bit. For cycle-cost brackets.
#[inline]
pub fn tim17_now() -> u16 {
    unsafe { (&*stm32::TIM17::ptr()).cnt().read().bits() as u16 }
}

/// Bring up TIM17 (1 MHz), TIM6 (10 kHz TRGO), ADC scan + DMA, and the
/// guard ISR. Call once, after RCC freeze, before arming.
pub fn init() {
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20) | (1 << 18))); // ADCEN, TIM17EN
        rcc.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 4))); // TIM6EN
        rcc.ahbenr().modify(|r, w| w.bits(r.bits() | 1)); // DMA1EN

        // TIM17: 1 MHz free-run timebase.
        let t17 = &*stm32::TIM17::ptr();
        t17.psc().write(|w| w.bits(63));
        t17.arr().write(|w| w.bits(0xFFFF));
        t17.egr().write(|w| w.bits(1));
        t17.cr1().write(|w| w.bits(1));

        // ADC: PCLK/4 clock, regulator on, calibrate, scan set, DMA circ,
        // hardware-triggered by TIM6_TRGO (EXTSEL=101, EXTEN=rising).
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30)); // CKMODE = PCLK/4 = 16 MHz
        adc.cr().write(|w| w.bits(1 << 28)); // ADVREGEN
        cortex_m::asm::delay(64 * 30); // t_ADCVREG_SETUP
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31))); // ADCAL
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        // SMP1 = 160.5 cycles (max): the VPH dividers present ~22 kOhm
        // source impedance and need a long sample window for 12-bit
        // accuracy (minz's lesson: 3.2k vbat wanted 640 L4-cycles).
        adc.smpr().write(|w| w.bits(0b111));
        adc.chselr0()
            .write(|w| w.bits((1 << 1) | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 15) | (1 << 17)));
        adc.cfgr1()
            .write(|w| w.bits((0b01 << 10) | (0b101 << 6) | 0b11)); // EXTEN rise | EXTSEL TIM6 | DMACFG|DMAEN

        // DMA1 CH1 <- ADC (DMAMUX req 5), circular over the 6-slot buffer.
        let dmamux = &*stm32::DMAMUX::ptr();
        dmamux.ccr(0).modify(|r, w| w.bits((r.bits() & !0x3F) | 5));
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch1();
        ch.cr().write(|w| w.en().clear_bit());
        ch.par().write(|w| w.bits(adc.dr().as_ptr() as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of_mut!(DMA_BUF) as u32));
        ch.ndtr().write(|w| w.bits(SEQ_LEN as u32));
        // TCIE | CIRC | MINC | PSIZE16 | MSIZE16 | PL=high
        ch.cr().write(|w| {
            w.bits((1 << 1) | (1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10) | (0b10 << 12))
        });
        ch.cr().modify(|r, w| w.bits(r.bits() | 1)); // EN

        // ADC enable + start (waits for triggers).
        adc.isr().write(|w| w.bits(1)); // clear ADRDY
        adc.cr().modify(|r, w| w.bits(r.bits() | 1)); // ADEN
        while adc.isr().read().bits() & 1 == 0 {}
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2))); // ADSTART

        // TIM6: ~9901 Sa/s -- DELIBERATELY not 10.000 kHz. TIM1 PWM runs
        // at exactly 10 kHz from the same clock; equal periods phase-lock
        // the ADC to one fixed instant of the PWM cycle (measured: every
        // sample landed in the OFF region -> VPH/IS blind to the active
        // vector). 101 ticks makes the sampling point sweep the whole PWM
        // period ~99 times a second.
        let t6 = &*stm32::TIM6::ptr();
        t6.psc().write(|w| w.bits(63));
        t6.arr().write(|w| w.bits(100));
        t6.cr2().write(|w| w.bits(0b010 << 4)); // MMS = update
        t6.egr().write(|w| w.bits(1));
        t6.cr1().write(|w| w.bits(1)); // CEN

        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::DMA1_CHANNEL1);
    }
}

/// Arm the guards: IS baseline from the current reading, VM floor at 85%
/// of the current bus reading (boot-relative), then ARMED=1.
///
/// UNITS: everything in harvest is PIN millivolts (BUS divider is 1:17.59,
/// so 6 V bus = 341 mV at the pin). Call only with EN already high --
/// the nFLT guard reads the shared EN node.
pub fn arm() {
    let vm = VM_MV.load(Relaxed); // pin mV
    let is0 = IS_MV.load(Relaxed);
    IS_BASE_MV.store(is0, Relaxed);
    // Hard VM-floor kill at 75% of boot bus -- a real-collapse net only, set
    // well below the example's ~89% graceful bus-sag auto-stop so the ladder
    // ends gracefully (zero kills) at the PSU edge. (341 = ~6 V abs minimum.)
    VM_FLOOR_MV.store(core::cmp::max(341, vm * 3 / 4), Relaxed);
    IS_EWMA_Q4.store((is0 as u32) << 4, Relaxed);
    IS_AVG_MV.store(is0, Relaxed);
    SAG_STRIKES.store(0, Relaxed);
    PEAK_STRIKES.store(0, Relaxed);
    ARMED.store(1, Relaxed);
}

pub fn disarm() {
    ARMED.store(0, Relaxed);
}

#[inline]
fn mv(raw: u16) -> u16 {
    ((raw as u32 * 3300) >> 12) as u16
}

fn kill_from_isr(t: u32, kind: u8, val: u16) {
    stage::force_safe();
    ARMED.store(0, Relaxed);
    KILL.store(kind, Relaxed);
    blackbox::push_isr(t, kind, val);
}

#[interrupt]
fn DMA1_CHANNEL1() {
    unsafe {
        let dma = &*stm32::DMA1::ptr();
        dma.ifcr().write(|w| w.bits(0xF)); // clear CH1 flags unconditionally
    }
    let t = TICKS_100US.load(Relaxed).wrapping_add(1);
    TICKS_100US.store(t, Relaxed);

    let (vm, vph2, vph1, vph3, is, ntc) = unsafe {
        let b = &*core::ptr::addr_of!(DMA_BUF);
        (mv(b[0]), mv(b[1]), mv(b[2]), mv(b[3]), mv(b[4]), mv(b[5]))
    };
    VM_MV.store(vm, Relaxed);
    VPH2_MV.store(vph2, Relaxed);
    VPH1_MV.store(vph1, Relaxed);
    VPH3_MV.store(vph3, Relaxed);
    IS_MV.store(is, Relaxed);
    NTC_MV.store(ntc, Relaxed);

    // Coast capture.
    if COAST_MODE.load(Relaxed) == 1 {
        let idx = COAST_IDX.load(Relaxed) as usize;
        if idx < COAST_LEN {
            unsafe {
                COAST_BUF[idx] = vph1;
            }
            COAST_IDX.store(idx as u16 + 1, Relaxed);
        } else {
            COAST_MODE.store(2, Relaxed); // full
        }
    }

    // Aggregates.
    if AGG_IS_MIN.load(Relaxed) > is {
        AGG_IS_MIN.store(is, Relaxed);
    }
    if AGG_IS_MAX.load(Relaxed) < is {
        AGG_IS_MAX.store(is, Relaxed);
    }
    AGG_IS_SUM.store(AGG_IS_SUM.load(Relaxed).wrapping_add(is as u32), Relaxed);
    if AGG_VM_MIN.load(Relaxed) > vm {
        AGG_VM_MIN.store(vm, Relaxed);
    }
    if AGG_VM_MAX.load(Relaxed) < vm {
        AGG_VM_MAX.store(vm, Relaxed);
    }
    AGG_VM_SUM.store(AGG_VM_SUM.load(Relaxed).wrapping_add(vm as u32), Relaxed);
    AGG_N.store(AGG_N.load(Relaxed).wrapping_add(1), Relaxed);

    // EWMA of IS (alpha = 1/16, ~1.6 ms tau at 9.9 kSa/s): reconstructs
    // the DC-link AVERAGE current the PSU actually sources, since drifting
    // samples cover the whole PWM period uniformly.
    let e = IS_EWMA_Q4.load(Relaxed);
    let e2 = e - (e >> 4) + is as u32; // += (is - avg)/16, in Q4
    IS_EWMA_Q4.store(e2, Relaxed);
    let is_avg = (e2 >> 4) as u16;
    IS_AVG_MV.store(is_avg, Relaxed);

    // Guards.
    if ARMED.load(Relaxed) == 1 {
        // Average-current overcurrent (the real thermal/PSU guard).
        let avg_delta = is_avg.abs_diff(IS_BASE_MV.load(Relaxed));
        if avg_delta > IS_KILL_DELTA_MV.load(Relaxed) {
            kill_from_isr(t, blackbox::KIND_KILL_OC, avg_delta);
        }
        // Instantaneous peak ceiling (short/desync catcher).
        let peak_delta = is.abs_diff(IS_BASE_MV.load(Relaxed));
        if peak_delta > IS_PEAK_CEIL_MV.load(Relaxed) {
            let s = PEAK_STRIKES.load(Relaxed) + 1;
            PEAK_STRIKES.store(s, Relaxed);
            if s == 1 {
                blackbox::push_isr(t, blackbox::KIND_STRIKE_OC, peak_delta);
            }
            if s >= PEAK_STRIKES_KILL {
                kill_from_isr(t, blackbox::KIND_KILL_OC, peak_delta);
            }
        } else {
            PEAK_STRIKES.store(0, Relaxed);
        }
        if vm < VM_FLOOR_MV.load(Relaxed) {
            let s = SAG_STRIKES.load(Relaxed) + 1;
            SAG_STRIKES.store(s, Relaxed);
            if s == 1 {
                blackbox::push_isr(t, blackbox::KIND_STRIKE_SAG, vm);
            }
            if s >= SAG_STRIKES_KILL {
                kill_from_isr(t, blackbox::KIND_KILL_SAG, vm);
            }
        } else {
            SAG_STRIKES.store(0, Relaxed);
        }
        // nFLT: PA6 low = fault (shared EN node; low also if someone
        // yanked EN). Require 2 consecutive.
        let pa6_low = unsafe { (&*stm32::GPIOA::ptr()).idr().read().bits() & (1 << 6) == 0 };
        if pa6_low {
            kill_from_isr(t, blackbox::KIND_KILL_NFLT, 0);
        }
        if ntc > NTC_KILL_MV.load(Relaxed) {
            kill_from_isr(t, blackbox::KIND_KILL_NTC, ntc);
        }
    }
}
