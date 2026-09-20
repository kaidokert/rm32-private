//! Opt-in, post-run event-rate census at the 40% live-duty epoch.
//! Single-producer counters only: DMA owns bus fields, ADC_COMP owns accepted
//! fields. No clock reads, formatting, division, or guard changes in either ISR.
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
#[cfg(feature = "bench-rate-curve")]
use portable_atomic::AtomicI32;
use core::fmt::Write;
#[cfg(feature = "bench-phase-current-census")]
#[path = "phase_current_census_policy.rs"]
mod phase_policy;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static START_US: AtomicU32 = AtomicU32::new(0);
static TARGET_TENTHS: AtomicU32 = AtomicU32::new(0);
static SCANS: AtomicU32 = AtomicU32::new(0);
static LOW97: AtomicU32 = AtomicU32::new(0);
static LOW95: AtomicU32 = AtomicU32::new(0);
static LOW95_RUNS: AtomicU32 = AtomicU32::new(0);
static LOW95_STREAK: AtomicU32 = AtomicU32::new(0);
static LOW95_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-rate-curve")]
static BUS_SUM: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-rate-curve")]
static VREF_SUM: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-rate-curve")]
static CURRENT_SUM: AtomicI32 = AtomicI32::new(0);
#[cfg(feature = "bench-rate-curve")]
static CURRENT_BLOCKS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-rate-curve")]
static CURRENT_MIN: AtomicI32 = AtomicI32::new(i32::MAX);
#[cfg(feature = "bench-rate-curve")]
static CURRENT_MAX: AtomicI32 = AtomicI32::new(i32::MIN);
static ACCEPTS: AtomicU32 = AtomicU32::new(0);
static LATE: AtomicU32 = AtomicU32::new(0);
static LATE_RUNS: AtomicU32 = AtomicU32::new(0);
static LATE_STREAK: AtomicU32 = AtomicU32::new(0);
static LATE_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-phase-current-census")]
static mut PHASE_COUNTS: [u32; 8] = [0; 8];
#[cfg(feature = "bench-phase-peak-stop")]
static mut FIRST_PEAK: [u16; 5] = [0; 5];
// Written only by DMA on the first terminal peak, read only after shutdown.
// Service-time PWM/role state is context, not the state at each sequential ADC aperture.
#[cfg(feature = "bench-phase-peak-stop")]
static mut PEAK_TIMING: [u32; 18] = [0; 18];

pub fn reset() {
    cortex_m::interrupt::free(|_| {
        ACTIVE.store(false, Relaxed);
        START_US.store(0, Relaxed);
        TARGET_TENTHS.store(0, Relaxed);
        #[cfg(feature = "bench-phase-current-census")]
        unsafe { core::ptr::addr_of_mut!(PHASE_COUNTS).write([0; 8]); }
        #[cfg(feature = "bench-phase-peak-stop")]
        unsafe {
            core::ptr::addr_of_mut!(FIRST_PEAK).write([0; 5]);
            core::ptr::addr_of_mut!(PEAK_TIMING).write([0; 18]);
        }
        for counter in [
            &SCANS, &LOW97, &LOW95, &LOW95_RUNS, &LOW95_STREAK, &LOW95_MAX,
            &ACCEPTS, &LATE, &LATE_RUNS, &LATE_STREAK, &LATE_MAX,
        ] {
            counter.store(0, Relaxed);
        }
        #[cfg(feature = "bench-rate-curve")]
        {
            BUS_SUM.store(0, Relaxed);
            VREF_SUM.store(0, Relaxed);
            CURRENT_SUM.store(0, Relaxed);
            CURRENT_BLOCKS.store(0, Relaxed);
            CURRENT_MIN.store(i32::MAX, Relaxed);
            CURRENT_MAX.store(i32::MIN, Relaxed);
        }
    });
}

#[cfg(feature = "bench-phase-peak-stop")]
pub fn peak_timing(row: [u32; 18]) {
    unsafe { core::ptr::addr_of_mut!(PEAK_TIMING).write(row); }
}

pub fn begin(now_us: u32, duty_tenths: u32) {
    reset();
    START_US.store(now_us, Relaxed);
    TARGET_TENTHS.store(duty_tenths, Relaxed);
    ACTIVE.store(true, Relaxed);
}

#[inline(always)]
const fn below_pct(bus: u16, vref: u16, ref_bus: u16, ref_vref: u16, pct: u32) -> bool {
    // Largest product is 4095^2*100 = 1_676_902_500, safely u32.
    bus as u32 * ref_vref as u32 * 100 < ref_bus as u32 * vref as u32 * pct
}

#[inline(always)]
fn count(counter: &AtomicU32) {
    counter.store(counter.load(Relaxed) + 1, Relaxed);
}

#[inline(always)]
pub fn bus_scan(bus: u16, vref: u16, ref_bus: u16, ref_vref: u16) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    count(&SCANS);
    #[cfg(feature = "bench-rate-curve")]
    {
        BUS_SUM.store(BUS_SUM.load(Relaxed).saturating_add(bus as u32), Relaxed);
        VREF_SUM.store(VREF_SUM.load(Relaxed).saturating_add(vref as u32), Relaxed);
    }
    if below_pct(bus, vref, ref_bus, ref_vref, 97) {
        count(&LOW97);
    }
    if below_pct(bus, vref, ref_bus, ref_vref, 95) {
        count(&LOW95);
        let n = LOW95_STREAK.load(Relaxed) + 1;
        if n == 1 { count(&LOW95_RUNS); }
        LOW95_STREAK.store(n, Relaxed);
        if n > LOW95_MAX.load(Relaxed) { LOW95_MAX.store(n, Relaxed); }
    } else {
        LOW95_STREAK.store(0, Relaxed);
    }
}

#[cfg(feature = "bench-rate-curve")]
#[inline(always)]
pub fn current_block(residual: i32) {
    if !ACTIVE.load(Relaxed) { return; }
    CURRENT_BLOCKS.store(CURRENT_BLOCKS.load(Relaxed).saturating_add(1), Relaxed);
    CURRENT_SUM.store(CURRENT_SUM.load(Relaxed).saturating_add(residual), Relaxed);
    CURRENT_MIN.store(CURRENT_MIN.load(Relaxed).min(residual), Relaxed);
    CURRENT_MAX.store(CURRENT_MAX.load(Relaxed).max(residual), Relaxed);
}

/// Complete coherent physical IA/IB/IC frame, already remapped by
/// `dma_snapshot::logical`. These are asynchronous shunt samples, not a
/// calibrated peak-current or DC-link-current protection decision.
#[cfg(feature = "bench-phase-current-census")]
#[inline(always)]
pub fn phase_scan(
    phase: [u16; 3], zero: [u16; 3],
    bus: u16, vref: u16, ref_bus: u16, ref_vref: u16,
) -> bool {
    if !ACTIVE.load(Relaxed) { return false; }
    let flags = phase_policy::classify(phase, zero);
    let any1900 = flags & phase_policy::ANY_1900 != 0;
    unsafe {
        let counts = &mut *core::ptr::addr_of_mut!(PHASE_COUNTS);
        if flags & phase_policy::ANY_1500 != 0 { counts[0] += 1; }
        if any1900 { counts[1] += 1; }
        if flags & phase_policy::IA_1900 != 0 { counts[2] += 1; }
        if flags & phase_policy::IB_1900 != 0 { counts[3] += 1; }
        if flags & phase_policy::IC_1900 != 0 { counts[4] += 1; }
        if flags & phase_policy::EXACT_RAIL != 0 { counts[7] += 1; }
        if any1900 {
            counts[5] += below_pct(bus, vref, ref_bus, ref_vref, 97) as u32;
            counts[6] += below_pct(bus, vref, ref_bus, ref_vref, 95) as u32;
        }
        #[cfg(feature = "bench-phase-peak-stop")]
        if any1900 && core::ptr::addr_of!(FIRST_PEAK).read()[3] == 0 {
            core::ptr::addr_of_mut!(FIRST_PEAK)
                .write([phase[0], phase[1], phase[2], bus, vref]);
        }
    }
    any1900
}

#[inline(always)]
pub fn accepted(measured_half_us: u16, prior_average_half_us: u32) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    count(&ACCEPTS);
    let late = prior_average_half_us >= 64
        && measured_half_us as u32
            > prior_average_half_us + (prior_average_half_us >> 2);
    if late {
        count(&LATE);
        let n = LATE_STREAK.load(Relaxed) + 1;
        if n == 1 { count(&LATE_RUNS); }
        LATE_STREAK.store(n, Relaxed);
        if n > LATE_MAX.load(Relaxed) { LATE_MAX.store(n, Relaxed); }
    } else {
        LATE_STREAK.store(0, Relaxed);
    }
}

pub fn dump<W: Write>(out: &mut W, stop_us: u32) {
    let start = START_US.load(Relaxed);
    let _ = writeln!(out,
        "RATECENSUS active={} start_us={} elapsed_us={} target_tenths={} below_pct_strict=1 diagnostic_only=1",
        ACTIVE.load(Relaxed) as u8, start, stop_us.saturating_sub(start), TARGET_TENTHS.load(Relaxed));
    let _ = writeln!(out,
        "RATEBUS scans={} low97={} low95={} low95_runs={} low95_max_streak={} complete_dma_only=1",
        SCANS.load(Relaxed), LOW97.load(Relaxed), LOW95.load(Relaxed),
        LOW95_RUNS.load(Relaxed), LOW95_MAX.load(Relaxed));
    let _ = writeln!(out,
        "RATEACCEPT accepted={} late125={} late_runs={} late_max_streak={} comparator_events_only=1",
        ACCEPTS.load(Relaxed), LATE.load(Relaxed), LATE_RUNS.load(Relaxed), LATE_MAX.load(Relaxed));
    #[cfg(feature = "bench-rate-curve")]
    let _ = writeln!(out,
        "RATECURVE duty_tenths={} bus_sum={} vref_sum={} scans={} current_sum={} current_blocks={} current_min={} current_max={} raw_units=1 diagnostic_only=1",
        TARGET_TENTHS.load(Relaxed), BUS_SUM.load(Relaxed), VREF_SUM.load(Relaxed), SCANS.load(Relaxed),
        CURRENT_SUM.load(Relaxed), CURRENT_BLOCKS.load(Relaxed), CURRENT_MIN.load(Relaxed), CURRENT_MAX.load(Relaxed));
    #[cfg(feature = "bench-phase-current-census")]
    {
        let p = unsafe { core::ptr::addr_of!(PHASE_COUNTS).read() };
        let _ = writeln!(out,
            "RATEPHASE any1500={} any1900={} ia1900={} ib1900={} ic1900={} any1900_bus97={} any1900_bus95={} exact_rail={} zero=same_wake units=raw_displacement complete_dma_only=1 report_only=1",
            p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
    }
    #[cfg(feature = "bench-phase-peak-stop")]
    {
        let p = unsafe { core::ptr::addr_of!(FIRST_PEAK).read() };
        let _ = writeln!(out,
            "RATEPEAK seen={} ia={} ib={} ic={} bus={} vref={} threshold_raw=1900 protective_stop=1",
            (p[3] != 0) as u8, p[0], p[1], p[2], p[3], p[4]);
        let t = unsafe { core::ptr::addr_of!(PEAK_TIMING).read() };
        let _ = writeln!(out,
            "PEAKTIME acquired_us={} service_us={} pwm_cnt={} pwm_arr={} ccr1={} ccr2={} ccr3={} last_event_us={} sector={} average_half_us={} this_zc_half_us={} tim2_cnt={} core_step={} duty_tenths={} commits={} tim17_cnt={} guard_last_poll_us={} guard_feedback_us={} service_context_only=1 adc_channels_sequential=1",
            t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8],
            t[9], t[10], t[11], t[12], t[13], t[14], t[15], t[16], t[17]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_thresholds_track_vref_and_fit_u32() {
        assert!(!below_pct(1140, 1500, 1200, 1500, 95));
        assert!(below_pct(1139, 1500, 1200, 1500, 95));
        assert!(!below_pct(1164, 1500, 1200, 1500, 97));
        assert!(below_pct(1163, 1500, 1200, 1500, 97));
        assert!(!below_pct(2280, 3000, 1200, 1500, 95));
        assert!(below_pct(0, 4095, 4095, 4095, 95));
    }
}
