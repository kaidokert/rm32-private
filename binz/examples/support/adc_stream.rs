//! Experimental interrupt-consumed hardware ADC stream. No gate-enable authority.
//! Foreground configures/restores with DMA IRQ masked. While enabled only DMA
//! IRQ consumes DMA scans; foreground takes owned FIFO entries in short critical
//! sections. Guard ISR never accesses STATE or QUEUE.
use super::*;
use core::sync::atomic::{Ordering, compiler_fence};
// Separate trigger timer enables a future continuous startup/BEMF stream;
// this backend alone does not change admission or ADC ownership.
#[cfg(all(
    feature = "bench-adc-tim15",
    any(feature = "bench-filter-raw", feature = "bench-adc-phase")
))]
compile_error!("TIM15 backend needs separate qualification of timer-specific capture probes");
#[cfg(feature = "bench-adc-tim15")]
fn trigger() -> &'static stm32::tim15::RegisterBlock {
    unsafe { &*stm32::TIM15::ptr() }
}
#[cfg(not(feature = "bench-adc-tim15"))]
fn trigger() -> &'static stm32::tim3::RegisterBlock {
    unsafe { &*stm32::TIM3::ptr() }
}
const EXTSEL: u32 = if cfg!(feature = "bench-adc-tim15") {
    4
} else {
    3
};
pub const PERIOD_US: u32 = if cfg!(feature = "bench-dma-226") {
    226
} else if cfg!(feature = "bench-dma-209") {
    209
} else if cfg!(feature = "bench-dma-201") {
    201
} else {
    101
};
pub const FIRST_TRIGGER_US: u32 = if cfg!(feature = "bench-dma-fast-start") {
    1
} else {
    101
};
struct State {
    ring: [u16; 10],
    origin: u32,
    count: u32,
    cfg: u32,
    channels: u32,
    last_poll: u16,
    active: bool,
    max_us: u8,
}
static mut STATE: State = State {
    ring: [0; 10],
    origin: 0,
    count: 0,
    cfg: 0,
    channels: 0,
    last_poll: 0,
    active: false,
    max_us: 0,
};
static mut QUEUE: scan_queue::Queue = scan_queue::Queue::new();
#[cfg(feature = "bench-adc-latest")]
#[path = "startup_feedback.rs"]
mod latest_policy;
#[cfg(feature = "bench-adc-latest")]
static mut LATEST: latest_policy::Latest = latest_policy::Latest::new();
#[cfg(feature = "bench-adc-latest-fault-frame")]
static mut LATEST_FAULT_FRAME: [u32; 14] = [0; 14];
#[cfg(feature = "bench-adc-latest")]
static mut QUIESCED: bool = false;
#[cfg(feature = "bench-adc-latest")]
static mut ACQ_CLOCK: sampled_clock::Clock = sampled_clock::Clock::new(0);
#[cfg(feature = "bench-startup-adc")]
static mut STARTUP: bool = false;
#[cfg(feature = "bench-startup-adc")]
static mut STARTUP_REASON: u32 = 0;

/// Foreground only, before first drive. Callers must not perform software ADC
/// transactions until stop() restores exclusive foreground ADC ownership.
#[cfg(feature = "bench-startup-adc")]
pub fn startup_begin() -> bool {
    if !get_idr(3, 1)
        || !get_idr(1, 14)
        || !powered_timer::outputs_disabled()
        || powered_timer::owns()
        || core_bench::active()
    {
        return false;
    }
    if unsafe { core::ptr::addr_of!(STATE.active).read() } {
        return false;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
    match start_inner(true) {
        Ok(()) => {
            unsafe {
                STARTUP_REASON = 0;
                STARTUP = true;
            }
            compiler_fence(Ordering::SeqCst);
            unsafe {
                cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::DMA1_CHANNEL1);
            }
            true
        }
        Err(_) => {
            gates_off();
            set_pin(3, 1, false);
            stop();
            false
        }
    }
}
#[cfg(feature = "bench-startup-adc")]
fn startup_fail(code: u32) {
    unsafe {
        if STARTUP_REASON == 0 {
            STARTUP_REASON = code;
        }
    }
    gates_off();
    set_pin(3, 1, false);
}
#[cfg(feature = "bench-startup-adc")]
pub fn startup_reason() -> u32 {
    cortex_m::interrupt::free(|_| unsafe { STARTUP_REASON })
}
#[cfg(feature = "bench-adc-latest-fault-frame")]
pub fn fault_frame_dump<W: Write>(out: &mut W) {
    let f = unsafe { LATEST_FAULT_FRAME };
    if f[0] != 0 {
        let _ = writeln!(
            out,
            "ADCFRAME reject={} ia={} ib={} ic={} bus={} vref={} age_us={} acquired_us={} cache_now_us={} pwm_stamp_us={} pwm_cnt={} pwm_arr={} pwm_ccr={} pwm_cr1={} coherent=1 fault_only=1 postrun_only=1",
            f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7],
            f[8], f[9], f[10], f[11], f[12], f[13]
        );
    }
}
#[cfg(feature = "bench-startup-adc")]
pub fn startup_sample() -> Option<([u16; 5], u32)> {
    cortex_m::interrupt::free(|_| unsafe {
        if !STARTUP || QUIESCED || !STATE.active || STARTUP_REASON != 0 {
            return None;
        }
        let now = acquisition_now();
        (&*core::ptr::addr_of!(LATEST))
            .read(now)
            .ok()
            .map(|f| (f.raw, now.wrapping_sub(f.acquired)))
    })
}
/// Single-counter snapshot: retained acquisition time, not cache-read time.
#[cfg(feature = "bench-startup-adc")]
pub fn startup_feedback(vcal: u32) -> Option<(powered_guard::Feedback, u16)> {
    cortex_m::interrupt::free(|_| unsafe {
        if !STARTUP || QUIESCED || !STATE.active || STARTUP_REASON != 0 {
            return None;
        }
        let counter = t17();
        let now = (&mut *core::ptr::addr_of_mut!(ACQ_CLOCK)).sample(counter);
        let frame = (&*core::ptr::addr_of!(LATEST)).read(now).ok()?;
        Some((
            powered_timer::convert(frame.raw, vcal),
            counter.wrapping_sub(now.wrapping_sub(frame.acquired) as u16),
        ))
    })
}
#[cfg(feature = "bench-adc-latest")]
fn acquisition_now() -> u32 {
    cortex_m::interrupt::free(|_| unsafe {
        (&mut *core::ptr::addr_of_mut!(ACQ_CLOCK)).sample(t17())
    })
}

/// Immediate revocation only: no ADCSTOP wait in a fault ISR. Foreground
/// stop() must still finish the current conversion and restore configuration.
#[cfg(feature = "bench-adc-latest")]
pub fn quiesce() {
    cortex_m::interrupt::free(|_| unsafe {
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
        if core::ptr::addr_of!(STATE.active).read() {
            trigger().cr1().write(|w| w.bits(0));
            core::ptr::addr_of_mut!(QUIESCED).write(true);
        }
        core::ptr::addr_of_mut!(LATEST).write(latest_policy::Latest::new());
    });
}

/// Serialized original-age view, NOT a new scan for current averaging.
/// Powered ownership remains required until startup readers are ported.
#[cfg(feature = "bench-adc-latest")]
pub fn latest() -> Option<([u16; 5], u32)> {
    cortex_m::interrupt::free(|_| unsafe {
        if !powered_timer::owns()
            || !core::ptr::addr_of!(STATE.active).read()
            || core::ptr::addr_of!(QUIESCED).read()
        {
            return None;
        }
        (&*core::ptr::addr_of!(LATEST))
            .read(acquisition_now())
            .ok()
            .map(|frame| (frame.raw, frame.acquired))
    })
}

pub fn queue_peak() -> u8 {
    cortex_m::interrupt::free(|_| unsafe { (&*core::ptr::addr_of!(QUEUE)).peak() })
}
pub fn take() -> Option<scan_queue::Frame> {
    cortex_m::interrupt::free(|_| unsafe {
        if !powered_timer::owns() {
            return None;
        }
        (&mut *core::ptr::addr_of_mut!(QUEUE)).pop()
    })
}

pub fn max_us() -> u8 {
    unsafe { core::ptr::addr_of!(STATE.max_us).read_volatile() }
}
/// Foreground diagnostic: aligned load of the existing completed-scan counter.
/// No new ISR work; caller must not interpret zero as absence of all IRQs.
pub fn completed_scans() -> u32 {
    unsafe { core::ptr::addr_of!(STATE.count).read_volatile() }
}
/// Foreground only. ISR never changes active; start publishes only after setup.
#[inline(never)]
pub fn ensure_started() -> bool {
    if !powered_timer::owns() {
        return false;
    }
    #[cfg(feature = "bench-adc-latest")]
    if unsafe { core::ptr::addr_of!(QUIESCED).read() } {
        powered_timer::stream_fault(15);
        stop();
        return false;
    }
    if unsafe { core::ptr::addr_of!(STATE.active).read_volatile() } {
        return true;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
    match start() {
        Ok(()) => {
            compiler_fence(Ordering::SeqCst);
            unsafe {
                cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::DMA1_CHANNEL1);
            }
            powered_timer::owns()
        }
        Err(code) => {
            powered_timer::stream_fault(code);
            stop();
            false
        }
    }
}

/// One bounded scan per entry. Normally guard/DMA are peers above COMP.
/// bench-dma-peer keeps guard at0 and makes DMA a COMP/COM peer at64;
/// guard may interrupt a DMA copy, whose lease/age checks remain unchanged.
pub fn interrupt() {
    #[cfg(not(feature = "bench-lean-irq"))]
    let began = t17();
    if !powered_timer::owns() {
        #[cfg(feature = "bench-startup-adc")]
        if unsafe { STARTUP && !QUIESCED } {
            match poll() {
                Ok(Some((raw, acquired))) => {
                    if !get_idr(3, 1) || !get_idr(1, 14) {
                        startup_fail(7);
                        return;
                    }
                    let vcal =
                        unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;
                    if !average_current_live::scan_raw(
                        [raw[0], raw[1], raw[2]],
                        raw[3],
                        raw[4],
                        vcal,
                    ) {
                        #[cfg(feature = "bench-fast-bus-sag")]
                        let reason = if average_current_live::fast_bus_tripped() { 26 } else { 25 };
                        #[cfg(not(feature = "bench-fast-bus-sag"))]
                        let reason = 25;
                        startup_fail(reason);
                        return;
                    }
                    #[cfg(feature = "bench-lean-irq")]
                    let sample = match powered_guard::validate_raw_feedback(raw, vcal) {
                        Ok(sample) => sample,
                        Err(fault) => {
                            startup_fail(if fault == powered_guard::Fault::Bus {
                                6
                            } else {
                                5
                            });
                            return;
                        }
                    };
                    #[cfg(not(feature = "bench-lean-irq"))]
                    let sample = powered_timer::convert(raw, vcal);
                    if let Err(fault) = powered_guard::validate_feedback(sample) {
                        startup_fail(if fault == powered_guard::Fault::Bus {
                            6
                        } else {
                            5
                        });
                        return;
                    }
                    if driven_run::owns() {
                        let counter = t17();
                        let now =
                            unsafe { (&mut *core::ptr::addr_of_mut!(ACQ_CLOCK)).sample(counter) };
                        driven_run::stream_feedback(
                            sample,
                            counter.wrapping_sub(now.wrapping_sub(acquired) as u16),
                        );
                    }
                    if unsafe {
                        (&mut *core::ptr::addr_of_mut!(LATEST))
                            .publish(latest_policy::Frame { raw, acquired }, acquisition_now())
                            .is_err()
                    } {
                        startup_fail(11);
                    }
                }
                Ok(None) => {}
                Err(_) => startup_fail(11),
            }
            return;
        }
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
        return;
    }
    match poll() {
        Ok(Some((raw, acquired))) => {
            #[cfg(feature = "bench-adc-latest")]
            let guard_acquired = unsafe {
                powered_timer::stream_stamp(acquired, &mut *core::ptr::addr_of_mut!(ACQ_CLOCK))
            };
            #[cfg(not(feature = "bench-adc-latest"))]
            let guard_acquired = acquired;
            #[cfg(feature = "bench-dma-guard")]
            let valid = powered_timer::stream_feedback(raw, guard_acquired);
            #[cfg(not(feature = "bench-dma-guard"))]
            let valid = powered_timer::stream_current([raw[0], raw[1], raw[2]]);
            if valid {
                #[cfg(feature = "bench-adc-latest")]
                let cache_now = acquisition_now();
                let published = unsafe {
                    (&mut *core::ptr::addr_of_mut!(LATEST))
                        .publish(latest_policy::Frame { raw, acquired }, cache_now)
                };
                if let Err(error) = published {
                    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
                    // This diagnostic-only path samples the running PWM
                    // immediately before safing restores ARR/CCRs. It is
                    // never executed for an accepted ADC frame.
                    #[cfg(feature = "bench-adc-latest-fault-frame")]
                    let pwm_at_fault = {
                        let stamp = acquisition_now();
                        let pwm = unsafe { &*stm32::TIM1::ptr() };
                        [
                            stamp, pwm.cnt().read().bits(), pwm.arr().read().bits(),
                            pwm.ccr1().read().bits(), pwm.cr1().read().bits(),
                        ]
                    };
                    powered_timer::stream_fault(10);
                    #[cfg(feature = "bench-adc-latest-fault-frame")]
                    unsafe {
                        let code = match error {
                            latest_policy::Error::Missing => 1,
                            latest_policy::Error::Stale => 2,
                            latest_policy::Error::Reordered => 3,
                            latest_policy::Error::Invalid => 4,
                            latest_policy::Error::Latched => 5,
                        };
                        LATEST_FAULT_FRAME = [
                            code, raw[0] as u32, raw[1] as u32, raw[2] as u32,
                            raw[3] as u32, raw[4] as u32,
                            cache_now.wrapping_sub(acquired), acquired, cache_now,
                            pwm_at_fault[0], pwm_at_fault[1], pwm_at_fault[2],
                            pwm_at_fault[3], pwm_at_fault[4],
                        ];
                    }
                    #[cfg(not(feature = "bench-adc-latest-fault-frame"))]
                    let _ = error;
                    return;
                }
                let pushed = unsafe {
                    (&mut *core::ptr::addr_of_mut!(QUEUE)).push(scan_queue::Frame {
                        raw,
                        acquired: guard_acquired,
                    })
                };
                if !pushed {
                    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
                    powered_timer::stream_fault(9);
                }
            }
        }
        Ok(None) => {}
        Err(code) => {
            // Stop authority immediately; defer ADCSTOP/restore to foreground.
            cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
            powered_timer::stream_fault(code);
        }
    }
    #[cfg(not(feature = "bench-lean-irq"))]
    unsafe {
        let p = core::ptr::addr_of_mut!(STATE.max_us);
        p.write(p.read().max(t17().wrapping_sub(began).min(255) as u8));
    }
}

#[inline(never)]
pub fn stop() {
    unsafe {
        #[cfg(feature = "bench-filter-raw")]
        filter_observe::raw_stopped();
        // Foreground cannot overlap an executing ISR on this core. Mask before
        // borrowing STATE, then stop transfers before releasing/restoring memory.
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::DMA1_CHANNEL1);
        compiler_fence(Ordering::SeqCst);
        #[cfg(feature = "bench-startup-adc")]
        {
            STARTUP = false;
        }
        #[cfg(feature = "bench-adc-latest")]
        core::ptr::addr_of_mut!(LATEST).write(latest_policy::Latest::new());
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        if !s.active {
            return;
        }
        let t = trigger();
        let a = &*stm32::ADC::ptr();
        let d = &*stm32::DMA1::ptr();
        t.cr1().write(|w| w.bits(0));
        #[cfg(feature = "bench-adc-phase")]
        adc_phase_dma::stop();
        if a.cr().read().bits() & (1 << 2) != 0 {
            a.cr().modify(|r, w| w.bits(r.bits() | (1 << 4)));
            let began = t17();
            while a.cr().read().bits() & ((1 << 2) | (1 << 4)) != 0 {
                if t17().wrapping_sub(began) >= 100 {
                    panic!("ADC stream stop timeout");
                }
            }
        }
        d.ch1().cr().write(|w| w.bits(0));
        compiler_fence(Ordering::SeqCst);
        d.ifcr().write(|w| w.bits(15));
        a.cfgr1().write(|w| w.bits(s.cfg));
        a.chselr0().write(|w| w.bits(s.channels));
        a.isr().write(|w| w.bits((1 << 4) | (1 << 3) | (1 << 2)));
        t.cr2().write(|w| w.bits(0));
        s.active = false;
        #[cfg(feature = "bench-adc-latest")]
        core::ptr::addr_of_mut!(QUIESCED).write(false);
    }
}

#[inline(never)]
fn start() -> Result<(), u32> {
    start_inner(false)
}
#[inline(never)]
fn start_inner(idle_probe: bool) -> Result<(), u32> {
    #[cfg(feature = "bench-adc-latest-fault-frame")]
    unsafe {
        LATEST_FAULT_FRAME = [0; 14];
    }
    unsafe {
        if idle_probe {
            let baseline = (cfg!(feature = "bench-baseline-dma")
                || cfg!(feature = "bench-prestart-dma"))
                && get_idr(3, 1);
            let raw_disabled = cfg!(feature = "bench-filter-raw") && !get_idr(3, 1);
            if !(baseline || raw_disabled)
                || powered_timer::owns()
                || !powered_timer::outputs_disabled()
            {
                return Err(1);
            }
        } else if !powered_timer::owns() {
            return Err(1);
        }
        // The forced owner must have released TIM3 and its PWM capture DMA before
        // foreground takes the timer. No attempt to stop or steal a live owner.
        // These owners only start from foreground; IRQs can revoke, never acquire.
        #[cfg(feature = "bench-driven-dma")]
        if driven_run::owns()
            || driven_probe::owns()
            || (*stm32::TIM3::ptr()).cr1().read().bits() & 1 != 0
            || (*stm32::TIM3::ptr()).dier().read().bits() != 0
            || (*stm32::TIM1::ptr()).dier().read().bits() & (1 << 12) != 0
            || (*stm32::DMA1::ptr()).ch2().cr().read().bits() & 1 != 0
        {
            return Err(11);
        }
        let r = &*stm32::RCC::ptr();
        #[cfg(feature = "bench-adc-tim15")]
        r.apbenr2().modify(|_, w| w.tim15en().set_bit());
        #[cfg(not(feature = "bench-adc-tim15"))]
        r.apbenr1().modify(|r, w| w.bits(r.bits() | 2));
        r.ahbenr().modify(|r, w| w.bits(r.bits() | 1));
        let t = trigger();
        let a = &*stm32::ADC::ptr();
        let d = &*stm32::DMA1::ptr();
        let ch = d.ch1();
        if t.cr1().read().bits() & 1 != 0
            || ch.cr().read().bits() & 1 != 0
            || a.cr().read().bits() & (1 << 2) != 0
        {
            return Err(2);
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        s.cfg = a.cfgr1().read().bits();
        s.channels = a.chselr0().read().bits();
        s.active = true;
        s.count = 0;
        s.max_us = 0;
        (&mut *core::ptr::addr_of_mut!(QUEUE)).clear();
        #[cfg(feature = "bench-adc-latest")]
        core::ptr::addr_of_mut!(LATEST).write(latest_policy::Latest::new());
        cortex_m::Peripherals::steal().NVIC.set_priority(
            stm32::Interrupt::DMA1_CHANNEL1,
            if cfg!(feature = "bench-dma-below-comp") {
                0x80
            } else if cfg!(feature = "bench-dma-peer") {
                0x40
            } else {
                0
            },
        );
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::DMA1_CHANNEL1);
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        // A stale forced-scheduler pending IRQ must not service the reused timer.
        #[cfg(all(feature = "bench-driven-dma", not(feature = "bench-adc-tim15")))]
        {
            cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM3);
            cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM3);
        }
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(PERIOD_US - 1));
        t.cr2().write(|w| w.bits(2 << 4));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        a.isr()
            .write(|w| w.bits((1 << 13) | (1 << 4) | (1 << 3) | (1 << 2)));
        a.chselr0()
            .write(|w| w.bits((1 << 0) | (1 << 1) | (1 << 4) | (1 << 6) | (1 << 13)));
        let began = t17();
        while a.isr().read().bits() & (1 << 13) == 0 {
            if t17().wrapping_sub(began) >= 100 {
                return Err(3);
            }
        }
        a.cfgr1().write(|w| w.bits((1 << 10) | (EXTSEL << 6) | 3));
        (*stm32::DMAMUX::ptr()).ccr(0).write(|w| w.bits(5));
        d.ifcr().write(|w| w.bits(15));
        ch.par().write(|w| w.bits(a.dr().as_ptr() as u32));
        ch.mar().write(|w| w.bits(s.ring.as_mut_ptr() as u32));
        ch.ndtr().write(|w| w.bits(10));
        compiler_fence(Ordering::SeqCst);
        ch.cr()
            .write(|w| w.bits(15 | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 10) | (2 << 12)));
        a.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
        #[cfg(feature = "bench-adc-phase")]
        if !adc_phase_dma::prepare() {
            return Err(12);
        }
        #[cfg(feature = "bench-filter-raw")]
        if !filter_observe::raw_started() {
            return Err(14);
        }
        // Lower-bound trigger epoch, sampled before CEN. Any preemption here
        // makes age conservative, never artificially fresh. No timer reset later.
        // Explicit first delay independent of steady cadence. All ADC/DMA setup
        // is complete before CEN. No UG/extra ADC trigger and no age refresh.
        t.cnt().write(|w| w.bits(PERIOD_US - FIRST_TRIGGER_US));
        #[cfg(feature = "bench-adc-latest")]
        {
            core::ptr::addr_of_mut!(ACQ_CLOCK).write(sampled_clock::Clock::new(t17()));
            s.origin = 0;
        }
        #[cfg(not(feature = "bench-adc-latest"))]
        {
            s.origin = if idle_probe {
                0
            } else {
                powered_timer::stream_now()
            };
        }
        s.last_poll = t17();
        t.cr1().write(|w| w.bits(1));
        Ok(())
    }
}

/// Diagnostic foreground owns the stream only while every bridge output is
/// disabled. No DMA interrupt is unmasked; use the same coherent poll/restore
/// path, not a fabricated powered-timer owner. Output no data until CSA is off.
#[cfg(feature = "bench-filter-raw")]
pub fn raw_check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || powered_timer::owns()
        || core_bench::active()
        || unsafe { core::ptr::addr_of!(STATE.active).read_volatile() }
    {
        let _ = writeln!(out, "RAWADC refused=1");
        return;
    }
    comp_input::stop();
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
    let mut fault = 0;
    let mut scans = 0;
    let mut captures = 0;
    let mut vref_min = 4095u16;
    let began = t17();
    if let Err(code) = start_inner(true) {
        fault = 100 + code;
    }
    if fault == 0 && !raw_capture::prepare() {
        fault = 2;
    }
    while fault == 0 && scans < 128 {
        if t17().wrapping_sub(began) >= 30_000
            || get_idr(3, 1)
            || !powered_timer::outputs_disabled()
        {
            fault = 3;
            break;
        }
        match poll() {
            Ok(Some((raw, at))) => {
                if at != FIRST_TRIGGER_US + scans * PERIOD_US {
                    fault = 4;
                    break;
                }
                scans += 1;
                vref_min = vref_min.min(raw[4]);
                if !raw_capture::disabled_pulse() {
                    fault = 5;
                    break;
                }
                captures += 1;
                let t = unsafe { &*stm32::TIM3::ptr() };
                if t.psc().read().bits() != 63
                    || t.arr().read().bits() != 200
                    || t.cr1().read().bits() != 1
                    || t.cr2().read().bits() != 2 << 4
                    || t.dier().read().bits() != 1 << 8
                {
                    fault = 6;
                }
            }
            Ok(None) => {}
            Err(code) => {
                fault = 200 + code;
            }
        }
    }
    let elapsed = t17().wrapping_sub(began);
    raw_capture::stop();
    stop();
    set_pin(3, 1, false);
    let _ = writeln!(
        out,
        "RAWADC scans={} captures={} fault={} elapsed_us={} vref_min={} disabled={} period_us=201 adc_channels=5 dma_polled=1 authority=0",
        scans,
        captures,
        fault,
        elapsed,
        vref_min,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}

/// Pre-drive only; preserve the caller's wake on success, disable on failure.
/// Reuse powered scan configuration and lease checks, without powered ownership.
#[cfg(feature = "bench-prestart-dma")]
pub fn prestart_collect(
    valid: &mut dyn FnMut() -> bool,
    stats: &mut [zero_stats::Stats; 5],
) -> bool {
    if !valid()
        || powered_timer::owns()
        || core_bench::active()
        || !powered_timer::outputs_disabled()
        || unsafe { core::ptr::addr_of!(STATE.active).read_volatile() }
    {
        return false;
    }
    // Refuse existing producers before touching their interrupt state.
    unsafe {
        if (*stm32::TIM3::ptr()).cr1().read().bits() & 1 != 0
            || (*stm32::TIM3::ptr()).dier().read().bits() != 0
            || (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 != 0
            || (*stm32::DMA1::ptr()).ch2().cr().read().bits() & 1 != 0
        {
            return false;
        }
    }
    #[cfg(feature = "bench-driven-dma")]
    if driven_run::owns() || driven_probe::owns() {
        return false;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
    let began = t17();
    let mut good = start_inner(true).is_ok();
    let mut n = 0;
    while good && n < 128 {
        if t17().wrapping_sub(began) >= 50_000 || !valid() || !powered_timer::outputs_disabled() {
            good = false;
            break;
        }
        match poll() {
            Ok(Some((raw, at))) => {
                if at != FIRST_TRIGGER_US + n * PERIOD_US {
                    good = false;
                    break;
                }
                for i in 0..5 {
                    good &= stats[i].push(raw[i]);
                }
                n += 1;
            }
            Ok(None) => {}
            Err(_) => {
                good = false;
            }
        }
    }
    good = good && n == 128 && valid() && powered_timer::outputs_disabled();
    if !good {
        set_pin(3, 1, false);
    }
    stop();
    good && valid() && powered_timer::outputs_disabled()
}

#[cfg(feature = "bench-baseline-dma")]
pub fn baseline_check<W: Write>(out: &mut W, settle_us: u16) {
    if !matches!(settle_us, 1000 | 20000) {
        let _ = writeln!(out, "!basemode delay");
        return;
    }
    if get_idr(3, 1)
        || powered_timer::owns()
        || !powered_timer::outputs_disabled()
        || unsafe { core::ptr::addr_of!(STATE.active).read_volatile() }
    {
        let _ = writeln!(out, "!basemode busy");
        return;
    }
    #[cfg(feature = "bench-driven-dma")]
    if driven_run::owns() || driven_probe::owns() {
        let _ = writeln!(out, "!basemode owner");
        return;
    }
    if unsafe {
        (*stm32::TIM3::ptr()).cr1().read().bits() & 1 != 0
            || (*stm32::TIM3::ptr()).dier().read().bits() != 0
            || (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 != 0
            || (*stm32::DMA1::ptr()).ch2().cr().read().bits() & 1 != 0
    } {
        let _ = writeln!(out, "!basemode producer");
        return;
    }
    let mut stats = [[zero_stats::Stats::new(); 5]; 3];
    let mut elapsed = [0u16; 3];
    let mut fault = 0u32;
    set_pin(3, 1, true);
    let wake = t17();
    let mut wake_fault_low = false;
    while t17().wrapping_sub(wake) < settle_us {
        wake_fault_low |= !get_idr(1, 14);
        if !powered_timer::outputs_disabled() || !get_idr(3, 1) {
            fault = 4;
            break;
        }
    }
    let settled = t17().wrapping_sub(wake);
    // As in the existing startup path, acquire readiness provenance AFTER
    // the bounded wake interval. A fault still low now refuses acquisition.
    let token = calibration_live::begin();
    if token.is_none() {
        fault = 5;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::DMA1_CHANNEL1);
    'modes: for mode in 0..3 {
        if fault != 0 {
            break;
        }
        let began = t17();
        if mode == 1 {
            if let Err(code) = start_inner(true) {
                fault = 100 + code;
                break;
            }
        }
        let mut n = 0;
        while n < 128 {
            if t17().wrapping_sub(began) >= 50_000
                || !get_idr(1, 14)
                || !powered_timer::outputs_disabled()
                || !token.as_ref().is_some_and(calibration_live::matches)
            {
                fault = 1;
                break;
            }
            let raw = if mode == 1 {
                match poll() {
                    Ok(Some((raw, _))) => raw,
                    Ok(None) => continue,
                    Err(code) => {
                        fault = 200 + code;
                        break;
                    }
                }
            } else {
                let mut raw = [0; 5];
                for (i, ch) in [4, 1, 0, 6, 13].into_iter().enumerate() {
                    let Some(value) = powered_timer::read_channel(ch) else {
                        fault = 2;
                        break 'modes;
                    };
                    raw[i] = value;
                }
                raw
            };
            for i in 0..5 {
                if !stats[mode][i].push(raw[i]) {
                    fault = 3;
                    break 'modes;
                }
            }
            n += 1;
        }
        elapsed[mode] = t17().wrapping_sub(began);
        if fault != 0 {
            break;
        }
        if mode == 1 {
            stop();
        }
    }
    let same = token.as_ref().is_some_and(calibration_live::matches);
    // Physical disable precedes bounded peripheral cleanup even on failure.
    set_pin(3, 1, false);
    stop();
    let good = fault == 0 && same && !get_idr(3, 1) && powered_timer::outputs_disabled();
    let _ = writeln!(
        out,
        "BASESETTLE target_us={} measured_us={} timer_sampled=1 fault_low_seen={}",
        settle_us, settled, wake_fault_low as u8
    );
    let _ = writeln!(
        out,
        "BASEMODE complete={} fault={} same_wake={} gates_off={} n_target=128 period_us={} first_us={} stationary_verified=0 offsets_applied=0 dma_polled=1",
        good as u8,
        fault,
        same as u8,
        powered_timer::outputs_disabled() as u8,
        PERIOD_US,
        FIRST_TRIGGER_US
    );
    for mode in 0..3 {
        let _ = writeln!(
            out,
            "BASEMODEPART mode={} elapsed_us={}",
            mode, elapsed[mode]
        );
        for i in 0..5 {
            let mut words = [0u16; 11];
            words[0] = mode as u16;
            words[1..].copy_from_slice(&stats[mode][i].words([4, 1, 0, 6, 13][i]));
            let _ = snapshot::record(out, "BM85", &words);
        }
    }
    let _ = writeln!(out, "BASEMODE END");
}

/// None means no NEW completed scan. Never re-feed a cached frame.
#[inline(never)]
fn poll() -> Result<Option<([u16; 5], u32)>, u32> {
    unsafe {
        let began = t17();
        let d = &*stm32::DMA1::ptr();
        let ch = d.ch1();
        let a = &*stm32::ADC::ptr();
        // Diagnostic modular gap, not a blackout watchdog. Record even empty polls.
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let service_gap = began.wrapping_sub(s.last_poll) as u32;
        s.last_poll = began;
        let flags = d.isr().read().bits();
        if flags & dma_snapshot::TE != 0 || a.isr().read().bits() & (1 << 4) != 0 {
            return Err(4);
        }
        if flags & (dma_snapshot::HT | dma_snapshot::TC) == 0 {
            return Ok(None);
        }
        let position = ch.ndtr().read().bits() as u16;
        let lease = dma_snapshot::Lease::begin(flags, position, 0).map_err(|_| {
            5u32 | ((flags & 15) << 8) | ((position as u32) << 12) | (service_gap << 16)
        })?;
        if lease.word_offset() != ((s.count as usize & 1) * 5) {
            return Err(6);
        }
        d.ifcr().write(|w| w.bits(lease.acknowledge_flag()));
        compiler_fence(Ordering::SeqCst);
        let mut raw = [0u16; 5];
        for j in 0..5 {
            raw[j] = s.ring.as_ptr().add(lease.word_offset() + j).read_volatile();
        }
        compiler_fence(Ordering::SeqCst);
        let remaining = ch.ndtr().read().bits() as u16;
        let after = d.isr().read().bits();
        lease
            .finish(after, remaining, t17().wrapping_sub(began) as u32, 0)
            .map_err(|_| {
                7u32 | ((after & 15) << 8) | ((remaining as u32) << 12) | (service_gap << 16)
            })?;
        let logical = dma_snapshot::logical(raw).ok_or(8u32)?;
        #[cfg(feature = "bench-adc-phase")]
        if !adc_phase_dma::consume(s.count) {
            return Err(13);
        }
        s.count += 1;
        // First channel is acquired AFTER this trigger. Missing triggers cause
        // a conservative older timestamp and eventual stale refusal, not rescue.
        let acquired = s
            .origin
            .wrapping_add(FIRST_TRIGGER_US + (s.count - 1) * PERIOD_US);
        Ok(Some((logical, acquired)))
    }
}
