//! binz's minz-core HAL trait implementations — the seam that lets us
//! consume minz-core's commutation brain instead of rewriting it.
//!
//! Register reference: rm32_stm32/src/mcu_g071 (G0 wiring) + minz/src/*.rs
//! (the trait shapes). binz differences vs minz's L431: TIM1 register names
//! (G0 PAC), and the Comparator is a SOFTWARE comparator over the VPH ADC
//! (minz uses hardware COMP2) — because PB0/phase-B reaches no G071 COMP.
//!
//! Convention: binz's six-step table (mzhal::COM) matches minz's sector
//! frame exactly (HIGH=[0,0,1,1,2,2], LOW=[1,2,2,0,0,1]); com_step takes
//! AM32 step 1..6 and subtracts 1 into the 0..5 sector, like minz.

use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering::Relaxed};
use stm32g0xx_hal::stm32;

// ---- shared TIM1 six-step drive (raw PAC, dead-time-protected) ----

pub const ARR: u32 = 3199; // 20 kHz carrier @ 64 MHz
const DTG: u8 = 26;
/// TIM1 CH4 as the ADC trigger source: OC4 in PWM mode 2 (OC4REF high while
/// CNT >= CCR4) + OC4PE preload. TRGO2 = OC4REF (CR2.MMS2 = 0b0111), so a
/// rising edge fires the ADC at CNT = ADC_TRIG_CNT — a FIXED mid-ON instant,
/// jitter-free regardless of ISR latency. set_roles must OR these into CCMR2
/// every commutation or it would wipe CH4 (CH3 and CH4 share CCMR2).
const OC4_TRGO2: u32 = (1 << 11) | (0b111 << 12); // OC4PE | OC4M=PWM2
/// Trigger ~1.25 us past the ON edge (CNT=0) — clears the switching transient
/// and leaves room for FOUR 19.5-cyc conversions (~8 us = 512 counts) inside
/// the ON window at >=20% duty (CCR1 >= 640).
pub const ADC_TRIG_CNT: u32 = 80;

/// (high, low, float) phase per 0..5 sector — matches minz set_roles_for_step.
const HIGH: [u8; 6] = [0, 0, 1, 1, 2, 2];
const LOW: [u8; 6] = [1, 2, 2, 0, 0, 1];
/// Float phase per sector = the remaining one (C,B,A,C,B,A).
pub const FLOAT: [u8; 6] = [2, 1, 0, 2, 1, 0];
/// VPH ADC channel per phase A/B/C = PB1/PB0/PB2 = IN9/8/10.
pub const VPH_CH: [u8; 3] = [9, 8, 10];

/// Current duty (CCR counts), shared by set_duty_all / com_step.
static DUTY: AtomicU8 = AtomicU8::new(0); // stored as 0..255 -> scaled

fn set_roles(sector: usize) {
    let hi = HIGH[sector];
    let lo = LOW[sector];
    let role = |ch: u8| -> (u32, u32, u32) {
        if ch == hi {
            (0b110, 1, 0) // PWM1: INH=PWM, INL off
        } else if ch == lo {
            (0b100, 1, 1) // force-inactive OCx (INH=0), OCxN(INL)=on
        } else {
            (0b100, 0, 0) // float: channel disabled -> INH=INL=0 (OSSR)
        }
    };
    let (m1, e1, n1) = role(0);
    let (m2, e2, n2) = role(1);
    let (m3, e3, n3) = role(2);
    let ccmr1 = (1 << 3) | (m1 << 4) | (1 << 11) | (m2 << 12);
    let ccmr2 = (1 << 3) | (m3 << 4) | OC4_TRGO2; // preserve CH4 ADC trigger
    let ccer = e1 | (n1 << 2) | (e2 << 4) | (n2 << 6) | (e3 << 8) | (n3 << 10);
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.ccmr1_output().write(|w| w.bits(ccmr1));
        tim.ccmr2_output().write(|w| w.bits(ccmr2));
        tim.ccer().write(|w| w.bits(ccer));
        tim.egr().write(|w| w.bits(1)); // latch preloaded regs
    }
}

/// One-time TIM1 base config (carrier, dead-time, OSSR/OSSI, ARPE). MOE
/// stays off; the caller enables it after precharge.
pub fn tim1_init() {
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 11))); // TIM1EN
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(ARR));
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.ccr4().write(|w| w.bits(ADC_TRIG_CNT)); // ADC trigger instant (mid-ON)
        tim.cr2().write(|w| w.bits(0b0111 << 20)); // MMS2 = OC4REF -> TRGO2
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32));
        tim.egr().write(|w| w.bits(1));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }
    set_roles(0);
}

/// Open-loop drive helper for the firmware startup ramp: energize `step`
/// (1..=6) at PWM compare `ccr`. Bypasses minz-core — used only to spin the
/// rotor into the BEMF-detectable band before the closed-loop handoff. Also
/// retargets the triggered ADC scan at the new floating phase.
pub fn drive_step(step: u8, ccr: u16) {
    let s = ((step.wrapping_sub(1)) % 6) as usize;
    write_duty(ccr);
    set_roles(s);
    SECTOR.store(s as u8, Relaxed);
}

/// Seed the comparator sector at the handoff, so sample_bemf reads the right
/// floating phase before the first minz commutation.
pub fn seed_step(step: u8) {
    SECTOR.store(((step.wrapping_sub(1)) % 6) as u8, Relaxed);
}

/// Open the software-comparator BEMF window (interrupt-mode emulation) at the
/// handoff, so the first ZC edge is caught before the first commutation
/// re-enables it.
pub fn comp_enable() {
    COMP_PENDING.store(false, Relaxed);
    COMP_ENABLED.store(true, Relaxed);
    BLANK_TICKS.store(BLANK_RELOAD, Relaxed); // demag settle after commutation
}

/// Diagnostic: is the software-comparator BEMF window currently open?
#[inline]
pub fn comp_enabled() -> bool {
    COMP_ENABLED.load(Relaxed)
}

/// The cached software-comparator level (floating > neutral). For the BEMF
/// ring instrument to record what the detector saw at each tick.
#[inline]
pub fn zc_level() -> bool {
    ZC_LEVEL.load(Relaxed)
}

/// Raw 12-bit floating-phase and 3-phase-mean-neutral for the current sector,
/// straight from the last DMA scan (no mV scaling — for the ring instrument).
#[inline]
pub fn float_neutral_raw() -> (u16, u16) {
    let sector = SECTOR.load(Relaxed) as usize % 6;
    let vf = dma_raw(PHASE_SLOT[FLOAT[sector] as usize]);
    let neu = (dma_raw(1) as u32 + dma_raw(2) as u32 + dma_raw(3) as u32) / 3;
    (vf, neu as u16)
}

/// Move the hardware ADC trigger point (TIM1 CC4) within the PWM period, to
/// characterize WHERE in the carrier the BEMF is visible (ON vs OFF window).
/// For the bemf-ring sweep instrument.
pub fn set_adc_trigger(cnt: u16) {
    unsafe {
        (&*stm32::TIM1::ptr()).ccr4().write(|w| w.bits(cnt as u32));
    }
}

#[inline]
pub fn moe(on: bool) {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        if on {
            tim.bdtr().modify(|r, w| w.bits(r.bits() | (1 << 15)));
        } else {
            tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15)));
        }
    }
}

fn write_duty(duty: u16) {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.ccr1().write(|w| w.bits(duty as u32));
        tim.ccr2().write(|w| w.bits(duty as u32));
        tim.ccr3().write(|w| w.bits(duty as u32));
    }
}

// ---- PwmOutput ----
pub struct Tim1Pwm;

impl minz_core::am32_hal::PwmOutput for Tim1Pwm {
    #[inline(always)]
    fn set_duty_all(&mut self, duty: u16) {
        write_duty(duty);
    }
    #[inline(always)]
    fn set_auto_reload(&mut self, arr: u16) {
        unsafe {
            (&*stm32::TIM1::ptr()).arr().write(|w| w.bits(arr as u32));
        }
    }
    #[inline(always)]
    fn set_prescaler(&mut self, psc: u16) {
        unsafe {
            (&*stm32::TIM1::ptr()).psc().write(|w| w.bits(psc as u32));
        }
    }
    #[inline(always)]
    fn set_compare1(&mut self, val: u16) {
        unsafe {
            (&*stm32::TIM1::ptr()).ccr1().write(|w| w.bits(val as u32));
        }
    }
    #[inline(always)]
    fn set_compare2(&mut self, val: u16) {
        unsafe {
            (&*stm32::TIM1::ptr()).ccr2().write(|w| w.bits(val as u32));
        }
    }
    #[inline(always)]
    fn set_compare3(&mut self, val: u16) {
        unsafe {
            (&*stm32::TIM1::ptr()).ccr3().write(|w| w.bits(val as u32));
        }
    }
    #[inline(always)]
    fn generate_update_event(&mut self) {
        unsafe {
            (&*stm32::TIM1::ptr()).egr().write(|w| w.bits(1));
        }
    }
    #[inline(always)]
    fn set_dead_time_override(&mut self, dtg: u16) {
        unsafe {
            (&*stm32::TIM1::ptr())
                .bdtr()
                .modify(|r, w| w.bits(r.bits() | dtg as u32));
        }
    }
}

// ---- PhaseOutput ----
impl minz_core::am32_hal::PhaseOutput for Tim1Pwm {
    /// AM32 step 1..6 -> minz sector 0..5.
    #[inline(always)]
    fn com_step(&mut self, step: u8) {
        set_roles(((step - 1) % 6) as usize);
    }
    #[inline(always)]
    fn all_off(&mut self) {
        crate::stage::force_safe();
    }
    #[inline(always)]
    fn full_brake(&mut self) {
        // All low FETs on = short brake. (UNCALLED in the basic path.)
        unsafe {
            let tim = &*stm32::TIM1::ptr();
            // force-inactive OCx + CCxNE on all three -> all INL on.
            tim.ccmr1_output()
                .write(|w| w.bits((1 << 3) | (0b100 << 4) | (1 << 11) | (0b100 << 12)));
            tim.ccmr2_output()
                .write(|w| w.bits((1 << 3) | (0b100 << 4)));
            tim.ccer()
                .write(|w| w.bits((1 << 2) | (1 << 6) | (1 << 10) | 1 | (1 << 4) | (1 << 8)));
            tim.egr().write(|w| w.bits(1));
        }
    }
    #[inline(always)]
    fn all_pwm(&mut self) {
        set_roles(0);
    }
    #[inline(always)]
    fn proportional_brake(&mut self) {
        crate::stage::force_safe();
    }
}

// ---- hardware-triggered ADC over DMA (TIM1 TRGO2 -> fixed mid-ON) ----
// A regular-group scan {VM(ch1), floating VPH, IS(ch15)} is HARDWARE-triggered
// by TIM1 TRGO2 at ADC_TRIG_CNT (a fixed mid-ON instant) and DMA'd into
// DMA_BUF every PWM period. No software timing -> the sample point never
// wanders with ISR latency (that ±µs jitter smeared the old software sampler
// and capped bemf_counter at 6). floating + VM land in the SAME triggered
// group, so the VM/2 neutral is COHERENT with the floating read (VM sags
// during ON pulses; sampling both at one instant keeps the comparison honest).
// sample_bemf just READS the buffer. (G0 has no injected group — that's the L4
// path — so this is the regular group + external trigger + DMA.)

// FIXED 4-channel triggered scan — CHSELR is written ONCE at init and never
// touched again (changing it needs an ADSTP/CCRDY dance that hung the
// commutation ISR). Ascending channel order 1,8,9,10 -> DMA slots
// [VM, B, A, C]. sample_bemf picks the floating phase's slot by sector. This
// needs the ON window to hold all 4 conversions (~8 us), i.e. duty >= ~19%,
// which the startup/handoff runs at (and 20% is on the path to the 30% goal;
// current stays bounded once synced).
const NBUF: usize = 4; // [VM(1), B(8), A(9), C(10)]
static mut DMA_BUF: [u16; NBUF] = [0; NBUF];
/// phase (0=A,1=B,2=C) -> DMA slot (VM=0, B=1, A=2, C=3).
const PHASE_SLOT: [usize; 3] = [2, 1, 3];

pub fn adc_init() {
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN
        rcc.ahbenr().modify(|r, w| w.bits(r.bits() | 1)); // DMA1EN
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30)); // CKMODE = PCLK/4 = 16 MHz
        adc.cr().write(|w| w.bits(1 << 28)); // ADVREGEN
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31))); // ADCAL
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        // 19.5 cyc @ 16 MHz = ~1.2 us — huge margin for the ~1.4 kOhm VPH
        // divider (needs <100 ns S/H).
        adc.smpr().write(|w| w.bits(0b011)); // 19.5 cyc
        adc.chselr0()
            .write(|w| w.bits((1 << 1) | (1 << 8) | (1 << 9) | (1 << 10))); // FIXED
        // EXTEN=01 rising | EXTSEL=000 (TIM1_TRGO2) | DMACFG circular | DMAEN
        adc.cfgr1()
            .write(|w| w.bits((0b01 << 10) | (0b000 << 6) | (1 << 1) | 1));

        // DMA1 CH1 <- ADC (DMAMUX req 5), circular over NBUF halfwords.
        let dmamux = &*stm32::DMAMUX::ptr();
        dmamux.ccr(0).modify(|r, w| w.bits((r.bits() & !0x3F) | 5));
        let dma = &*stm32::DMA1::ptr();
        let ch = dma.ch1();
        ch.cr().write(|w| w.bits(0)); // disable
        ch.par().write(|w| w.bits(adc.dr().as_ptr() as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of_mut!(DMA_BUF) as u32));
        ch.ndtr().write(|w| w.bits(NBUF as u32));
        // CIRC | MINC | PSIZE16 | MSIZE16 | PL high  (no TCIE — polled buffer)
        ch.cr()
            .write(|w| w.bits((1 << 5) | (1 << 7) | (0b01 << 8) | (0b01 << 10) | (0b10 << 12)));
        ch.cr().modify(|r, w| w.bits(r.bits() | 1)); // EN

        adc.isr().write(|w| w.bits(1)); // clear ADRDY
        adc.cr().modify(|r, w| w.bits(r.bits() | 1)); // ADEN
        while adc.isr().read().bits() & 1 == 0 {}
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2))); // ADSTART (waits for triggers)
    }
}

#[inline]
fn dma_raw(i: usize) -> u16 {
    unsafe { core::ptr::read_volatile((core::ptr::addr_of!(DMA_BUF) as *const u16).add(i)) }
}

/// Latest hardware-sampled bus voltage (pin mV). Valid once triggers run.
#[inline]
pub fn vm_mv() -> u16 {
    ((dma_raw(0) as u32 * 3300) >> 12) as u16
}

/// Latest hardware-sampled voltage of `phase` (0=A,1=B,2=C), pin mV. For
/// instrument validation (all three phases are in the fixed scan).
#[inline]
pub fn phase_mv(phase: u8) -> u16 {
    ((dma_raw(PHASE_SLOT[(phase % 3) as usize]) as u32 * 3300) >> 12) as u16
}

static SECTOR: AtomicU8 = AtomicU8::new(0);
/// Cached BEMF comparison (floating > neutral), refreshed by `sample_bemf()`.
/// minz calls `output_level()` in tight loops expecting a ~ns hardware
/// comparator, so it returns this cache.
static ZC_LEVEL: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// Interrupt-mode emulation: the software comparator stands in for the
/// hardware COMP+EXTI. A ZC_LEVEL edge, while comp "interrupts" are enabled,
/// latches COMP_PENDING — minz's comp_isr (driven from the TIM6 ISR) then
/// processes it exactly like the real EXTI, arming the com timer. This keeps
/// minz in INTERRUPT mode (old_routine=false) so the blocking polling
/// zcfr_spin_wait — which starves the 20 kHz ISR with no HW comparator to
/// hand off to — is never taken.
static COMP_PENDING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static COMP_ENABLED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// Post-commutation demag blanking: ignore ZC edges for this many control
/// ticks after a commutation (the freewheel/demag transient crosses the
/// neutral spuriously). ~10 ticks @ 20 kHz = 500 us. comp_enable (called on
/// every commutation) reloads it; sample_bemf counts it down.
static BLANK_TICKS: AtomicU8 = AtomicU8::new(0);
const BLANK_RELOAD: u8 = 1; // ~50 us — minimal demag blank, catch more crossings

pub static IS_CACHE: AtomicU16 = AtomicU16::new(1650);
pub static VM_CACHE: AtomicU16 = AtomicU16::new(600);

// Diagnostics (bring-up): latest floating-phase sample, the coherent neutral,
// a monotonic ZC_LEVEL transition count, and per-commutation-step crossing
// quality (transitions within the last completed step — synced rotor ~1;
// noise many; stalled/weak 0 — plus % of samples above neutral, ~50% for a
// mid-step crossing).
pub static VF_DIAG: AtomicU16 = AtomicU16::new(0);
pub static NEUTRAL_DIAG: AtomicU16 = AtomicU16::new(0);
pub static ZC_TRANS: AtomicU16 = AtomicU16::new(0);
pub static STEP_TRANS_LAST: AtomicU16 = AtomicU16::new(0);
pub static STEP_HI_PCT: AtomicU16 = AtomicU16::new(0);
/// PER-SECTOR self-calibrating neutral: a slow EWMA (K=256, Q8) of the
/// floating-phase value, one per step. Each float window has its own DC center
/// (odd steps sit ~15-30 mV below even ones — the bemf-ring plot: a single
/// global neutral leaves steps 1,3 stuck below it). Tracking each sector's own
/// center lets vf cross it on all 6 steps. Seeded ~280 mV.
static NEUTRAL_EWMA: [AtomicU32; 6] = [const { AtomicU32::new(280 << 8) }; 6];
/// Interrupt-mode diagnostics: COMP_PENDING latches (edges offered to
/// comp_isr) vs com-timer arms by interrupt_routine (ZCs actually ACCEPTED).
pub static COMP_PEND_N: AtomicU16 = AtomicU16::new(0);
pub static COM_ARM_N: AtomicU16 = AtomicU16::new(0);
static STEP_TRANS: AtomicU16 = AtomicU16::new(0);
static STEP_HI: AtomicU16 = AtomicU16::new(0);
static STEP_TOT: AtomicU16 = AtomicU16::new(0);
static PREV_SECTOR: AtomicU8 = AtomicU8::new(255);

/// Read the latest hardware-triggered ADC scan from the DMA buffer and update
/// the software comparator (ZC_LEVEL) + caches. NO ADC access — the DMA keeps
/// DMA_BUF fresh at the fixed mid-ON instant every PWM period. Cheap: a few
/// memory reads. Called once per control tick at the top of the TIM6 ISR.
pub fn sample_bemf() {
    let sector = SECTOR.load(Relaxed) as usize % 6;
    let slot = PHASE_SLOT[FLOAT[sector] as usize]; // this step's floating phase
    let vf = ((dma_raw(slot) as u32 * 3300) >> 12) as u16; // floating phase
    let vm = ((dma_raw(0) as u32 * 3300) >> 12) as u16; // bus, SAME trigger
    VM_CACHE.store(vm, Relaxed);
    // Neutral = per-sector slow EWMA of the floating phase (self-calibrating
    // to THIS window's DC center = its crossing point). K=256 (~13 ms tau)
    // tracks the center across revs without following the intra-step BEMF
    // swing; per-sector handles the odd/even DC offset that left steps 1,3
    // stuck under a single global neutral (bemf-ring plot).
    let e = NEUTRAL_EWMA[sector].load(Relaxed);
    let e2 = e - (e >> 8) + vf as u32; // += (vf - neutral)/256, Q8
    NEUTRAL_EWMA[sector].store(e2, Relaxed);
    let neutral = (e2 >> 8) as u16;
    VF_DIAG.store(vf, Relaxed);
    NEUTRAL_DIAG.store(neutral, Relaxed);
    // Hysteresis comparator: jitter is gone with the hardware trigger, so a
    // small band (15 mV) just rejects the last ~LSB of ADC noise at the
    // crossing without swallowing the BEMF swing.
    const HYST: u16 = 10; // reject ADC noise near the crossing — a rattling
    // rotor (operator's ear) means comp was firing on noise/demag; cleaner to
    // accept fewer, well-formed crossings than many jittery ones.
    let prev = ZC_LEVEL.load(Relaxed);
    let level = if vf > neutral + HYST {
        true
    } else if vf + HYST < neutral {
        false
    } else {
        prev
    };
    // Demag blanking counts down every tick.
    let blank = BLANK_TICKS.load(Relaxed);
    if blank > 0 {
        BLANK_TICKS.store(blank - 1, Relaxed);
    }
    if level != prev {
        ZC_TRANS.store(ZC_TRANS.load(Relaxed).wrapping_add(1), Relaxed);
        STEP_TRANS.store(STEP_TRANS.load(Relaxed).wrapping_add(1), Relaxed);
        // Latch a "COMP EXTI" edge for interrupt-mode processing, unless still
        // in the post-commutation demag blank. comp_isr filters direction and
        // the gate.
        if blank == 0 && COMP_ENABLED.load(Relaxed) {
            COMP_PENDING.store(true, Relaxed);
            COMP_PEND_N.store(COMP_PEND_N.load(Relaxed).wrapping_add(1), Relaxed);
        }
    }
    ZC_LEVEL.store(level, Relaxed);
    // Per-step crossing stats, finalized on sector change.
    let sc = SECTOR.load(Relaxed);
    if sc != PREV_SECTOR.load(Relaxed) {
        STEP_TRANS_LAST.store(STEP_TRANS.load(Relaxed), Relaxed);
        let tot = STEP_TOT.load(Relaxed).max(1) as u32;
        STEP_HI_PCT.store((STEP_HI.load(Relaxed) as u32 * 100 / tot) as u16, Relaxed);
        STEP_TRANS.store(0, Relaxed);
        STEP_HI.store(0, Relaxed);
        STEP_TOT.store(0, Relaxed);
        PREV_SECTOR.store(sc, Relaxed);
    }
    STEP_TOT.store(STEP_TOT.load(Relaxed).wrapping_add(1), Relaxed);
    if level {
        STEP_HI.store(STEP_HI.load(Relaxed).wrapping_add(1), Relaxed);
    }
}

pub struct AdcComp;

impl minz_core::am32_hal::Comparator for AdcComp {
    #[inline(always)]
    fn output_level(&self) -> bool {
        ZC_LEVEL.load(Relaxed)
    }
    #[inline(always)]
    fn set_step(&mut self, step: u8, _rising: bool) {
        // Just record the sector; the ADC scan is fixed (all phases), and
        // sample_bemf picks the floating slot. No ADC reconfig in the ISR.
        SECTOR.store(((step.wrapping_sub(1)) % 6) as u8, Relaxed);
    }
    #[inline(always)]
    fn change_input(&mut self) {}
    #[inline(always)]
    fn enable_interrupts(&mut self) {
        // Open the BEMF window: subsequent ZC_LEVEL edges latch COMP_PENDING.
        COMP_PENDING.store(false, Relaxed);
        COMP_ENABLED.store(true, Relaxed);
    }
    #[inline(always)]
    fn mask_interrupts(&mut self) {
        COMP_ENABLED.store(false, Relaxed);
        COMP_PENDING.store(false, Relaxed);
    }
}

impl minz_core::am32_hal::CompExti for AdcComp {
    #[inline(always)]
    fn exti_pending(&self) -> bool {
        COMP_PENDING.load(Relaxed)
    }
    #[inline(always)]
    fn clear_pending(&self) {
        COMP_PENDING.store(false, Relaxed);
    }
}

// ---- timers: TIM2 (interval, 32-bit free-run), TIM16 (com one-shot) ----

pub fn timers_init() {
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 0))); // TIM2EN
        rcc.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 17))); // TIM16EN
        // TIM2: 1 MHz free-running 32-bit interval timer.
        let t2 = &*stm32::TIM2::ptr();
        t2.psc().write(|w| w.bits(31)); // 2 MHz (0.5us) matches minz domain
        t2.arr().write(|w| w.bits(0xFFFF_FFFF));
        t2.egr().write(|w| w.bits(1));
        t2.cr1().write(|w| w.bits(1));
        // TIM16: 1 MHz one-shot (OPM), update IRQ = next-commutation tick.
        let t16 = &*stm32::TIM16::ptr();
        t16.psc().write(|w| w.bits(31)); // 2 MHz (0.5us)
        t16.arr().write(|w| w.bits(0xFFFF));
        t16.cr1().write(|w| w.bits(1 << 3)); // OPM
        t16.egr().write(|w| w.bits(1));
    }
}

pub struct Timers;

impl minz_core::am32_hal::IntervalTimer for Timers {
    #[inline(always)]
    fn count(&self) -> u32 {
        unsafe { (&*stm32::TIM2::ptr()).cnt().read().bits() }
    }
    #[inline(always)]
    fn set_count(&mut self, val: u32) {
        unsafe {
            (&*stm32::TIM2::ptr()).cnt().write(|w| w.bits(val));
        }
    }
}

impl minz_core::am32_hal::ComTimer for Timers {
    #[inline(always)]
    fn set_and_enable(&mut self, timeout: u16) {
        // Clamp the commutation interval. Floor 200 ticks (100 us): a
        // collapsed ci would fire TIM16 back-to-back and starve the ISR. Cap
        // 4000 ticks (2000 us = the forced floor): during acquisition ci/
        // wait_time inflate (rare accepts), and an inflated wait_time would
        // commutate milliseconds late -> desync -> even fewer accepts. Capping
        // keeps commutation at/under the forced rate so the loop can converge.
        let t = timeout.clamp(200, 8000);
        COM_ARM_N.store(COM_ARM_N.load(Relaxed).wrapping_add(1), Relaxed);
        unsafe {
            let t16 = &*stm32::TIM16::ptr();
            t16.arr().write(|w| w.bits(t as u32));
            t16.cnt().write(|w| w.bits(0));
            t16.dier().write(|w| w.bits(1)); // UIE
            t16.cr1().modify(|r, w| w.bits(r.bits() | 1)); // CEN (OPM auto-stops)
        }
    }
    #[inline(always)]
    fn disable_interrupt(&mut self) {
        unsafe {
            (&*stm32::TIM16::ptr()).dier().write(|w| w.bits(0));
        }
    }
    #[inline(always)]
    fn enable_interrupt(&mut self) {
        unsafe {
            (&*stm32::TIM16::ptr()).dier().write(|w| w.bits(1));
        }
    }
}

impl minz_core::am32_hal::ComTimerExt for Timers {
    #[inline(always)]
    fn com_set_arr(&mut self, arr: u16) {
        unsafe {
            (&*stm32::TIM16::ptr()).arr().write(|w| w.bits(arr as u32));
        }
    }
    #[inline(always)]
    fn com_clear_flag(&mut self) {
        unsafe {
            (&*stm32::TIM16::ptr()).sr().write(|w| w.bits(0));
        }
    }
}

// ---- Recorder over binz blackbox, Cs, InjAdc (safety) ----

pub struct Rec;
impl minz_core::am32_hal::Recorder for Rec {
    #[inline(always)]
    fn record(&self, ty: u8, sector: u8, data: u16) {
        // Fold sector into the value; timestamp from TIM17 (no DWT on M0).
        let t = crate::harvest::tim17_now() as u32;
        crate::blackbox::push_isr(
            t,
            crate::blackbox::KIND_INFO,
            ((sector as u16) << 12) | (data & 0x0FFF),
        );
        let _ = ty;
    }
    #[inline(always)]
    fn freeze(&self) {}
}

pub struct Cs;
impl minz_core::am32_hal::Cs for Cs {
    #[inline(always)]
    fn free<R>(&self, f: impl FnOnce() -> R) -> R {
        cortex_m::interrupt::free(|_| f())
    }
}

pub struct SafetyAdc;
impl minz_core::am32_hal::InjAdc for SafetyAdc {
    #[inline(always)]
    fn inj_read(&self) -> (u16, u16, u16, u16) {
        // (phase_a, phase_b, current, vbat) — safety kills only, NO ADC here
        // (ISR budget). IS_CACHE is the ON-window PULSE PEAK (BEMF sampling
        // dictates the window), not average current — feeding it to minz's
        // OC guard trips instantly on inrush. On this current-limited PSU the
        // unambiguous envelope edge is BUS-SAG (the supply-wall scar), which
        // minz's vbat-floor kill covers via `vm`. So report current = 0 (OC
        // guard off) and let bus-sag + the PSU limit + the STDRIVE102H's own
        // VDS/nFLT be the current backstops.
        let vm = VM_CACHE.load(Relaxed);
        (0, 0, 0, vm)
    }
}

pub struct Loop;
impl minz_core::am32_hal::LoopTimer for Loop {
    #[inline(always)]
    fn clear_flag(&self) {
        unsafe {
            (&*stm32::TIM6::ptr())
                .sr()
                .modify(|r, w| w.bits(r.bits() & !1));
        }
    }
}

// ---- bundle type aliases ----
pub type BinzMotor = minz_core::am32_hal::Motor<Tim1Pwm, AdcComp, Tim1Pwm, Timers, Timers>;
pub type BinzObserver<'a> = minz_core::am32_hal::Observer<'a, Rec, Cs, SafetyAdc, Loop>;
