//! Opt-in average protection. Raw threshold provision is not gain calibration.
use super::*;
#[path = "bus_sag.rs"]
mod bus_sag;
#[cfg(feature = "bench-fast-bus-sag")]
#[path = "fast_bus_sag.rs"]
mod fast_bus_sag;
#[path = "nominal_current.rs"]
pub mod nominal;
#[path = "average_current.rs"]
mod policy;
const _: () = assert!(policy::SCANS == nominal::SCANS);
static mut THRESHOLD: Option<i32> = None;
static mut LIMIT: Option<policy::Limit> = None;
static mut PHASE_ZERO: Option<[u16; 3]> = None;
static RAIL_CODES: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
static BUS_LOW_CODES: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-current-report-only")]
static CURRENT_ONLY_FAILURE: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
static mut BUS_SAG: bus_sag::Guard<{ policy::SCANS }> = bus_sag::Guard::new();
#[cfg(feature = "bench-fast-bus-sag")]
static mut FAST_BUS_SAG: Option<fast_bus_sag::Guard> = None;
#[cfg(feature = "bench-fast-bus-sag")]
static FAST_BUS_REF: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-fast-bus-sag")]
static FAST_BUS_TRIPPED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
#[cfg(feature = "bench-phase-peak-stop")]
static PHASE_PEAK_TRIPPED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
#[cfg(feature = "bench-current-bus-assisted-foldback")]
static mut BUS_EVIDENCE: bus_foldback::Evidence<{ policy::SCANS }, 3> =
    bus_foldback::Evidence::new();
#[cfg(feature = "bench-current-rolling-warning")]
static mut ROLLING: Option<rolling_current::Rolling<50>> = None;
#[cfg(feature = "bench-current-foldback-policy")]
static FOLDBACK_PENDING: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-current-foldback-policy")]
static FOLDBACK_COUNT: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-current-foldback-policy")]
static FOLDBACK_LAST: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-current-foldback-policy")]
static FOLDBACK_LAST_STEP: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-bus-recovery")]
static RECOVERY_COUNT: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-bus-recovery")]
static RECOVERY_LAST: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
#[cfg(feature = "bench-average-diagnostic")]
static mut DIAG: [i32; 8] = [0; 8]; // zero, allowance, last/max residual, cause, fault raw A/B/C

/// Idle only. Provision a threshold expressed as one complete-block sum. Its physical
/// accuracy depends on the source (measured calibration or explicit nominal).
/// No default: missing configuration must refuse the opt-in startup.
pub fn configure(raw: u32) -> bool {
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || powered_timer::owns()
        || core_bench::active()
    {
        return false;
    }
    #[cfg(feature = "bench-driven-handoff")]
    if driven_run::owns() {
        return false;
    }
    if raw == 0 || raw > 4095 * 3 * policy::SCANS {
        return false;
    }
    cortex_m::interrupt::free(|_| unsafe {
        THRESHOLD = Some(raw as i32);
        LIMIT = None;
    });
    #[cfg(feature = "bench-current-foldback-policy")]
    {
        FOLDBACK_PENDING.store(0, portable_atomic::Ordering::Relaxed);
        FOLDBACK_COUNT.store(0, portable_atomic::Ordering::Relaxed);
        FOLDBACK_LAST.store(0, portable_atomic::Ordering::Relaxed);
        FOLDBACK_LAST_STEP.store(0, portable_atomic::Ordering::Relaxed);
        #[cfg(feature = "bench-bus-recovery")]
        {
            RECOVERY_COUNT.store(0, portable_atomic::Ordering::Relaxed);
            RECOVERY_LAST.store(0, portable_atomic::Ordering::Relaxed);
        }
    }
    true
}
/// Called before first phase drive, after the complete same-wake zero capture.
pub fn install() -> bool {
    #[cfg(feature = "bench-rate-census")]
    super::rate_census::reset();
    #[cfg(feature = "bench-fast-sag-causal")]
    super::fast_sag_causal::reset();
    let Some((zero, phase_zero)) = prestart_baseline::current_zeros() else {
        return false;
    };
    #[cfg(feature = "bench-fast-bus-sag")]
    let Some((reference_bus, reference_vref)) = prestart_baseline::bus_reference() else {
        return false;
    };
    #[cfg(feature = "bench-fast-bus-sag")]
    let Some(fast_guard) = fast_bus_sag::Guard::new(reference_bus, reference_vref) else {
        return false;
    };
    #[cfg(feature = "bench-fast-bus-sag")]
    {
        FAST_BUS_REF.store(
            ((reference_bus as u32) << 16) | reference_vref as u32,
            portable_atomic::Ordering::Relaxed,
        );
        FAST_BUS_TRIPPED.store(false, portable_atomic::Ordering::Relaxed);
        #[cfg(feature = "bench-phase-peak-stop")]
        PHASE_PEAK_TRIPPED.store(false, portable_atomic::Ordering::Relaxed);
    }
    cortex_m::interrupt::free(|_| unsafe {
        let limit = THRESHOLD.and_then(|threshold| policy::Limit::new(zero, threshold));
        let ready = limit.is_some();
        LIMIT = limit;
        PHASE_ZERO = if ready { Some(phase_zero) } else { None };
        #[cfg(feature = "bench-current-rolling-warning")]
        {
            ROLLING = if ready {
                rolling_current::Rolling::new(zero, THRESHOLD.unwrap_or(0))
            } else {
                None
            };
        }
        #[cfg(feature = "bench-average-diagnostic")]
        {
            DIAG = [zero, THRESHOLD.unwrap_or(0), 0, 0, 0, 0, 0, 0];
        }
        #[cfg(feature = "bench-current-foldback-policy")]
        FOLDBACK_PENDING.store(0, portable_atomic::Ordering::Relaxed);
        RAIL_CODES.store(0, portable_atomic::Ordering::Relaxed);
        BUS_LOW_CODES.store(0, portable_atomic::Ordering::Relaxed);
        BUS_SAG = bus_sag::Guard::new();
        #[cfg(feature = "bench-fast-bus-sag")]
        {
            FAST_BUS_SAG = Some(fast_guard);
        }
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        {
            BUS_EVIDENCE = bus_foldback::Evidence::new();
        }
        ready
    })
}
/// ENABLE writes, startup foreground scans and DMA scans serialize with the
/// same IRQ mask. A complete block spanning a producer transition is a SAMPLE
/// mean, not a uniform time-average; no partial block is silently discarded.
pub fn revoke() {
    cortex_m::interrupt::free(|_| unsafe {
        LIMIT = None;
        PHASE_ZERO = None;
        BUS_SAG = bus_sag::Guard::new();
        #[cfg(feature = "bench-fast-bus-sag")]
        {
            FAST_BUS_SAG = None;
        }
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        {
            BUS_EVIDENCE = bus_foldback::Evidence::new();
        }
        #[cfg(feature = "bench-current-rolling-warning")]
        {
            ROLLING = None;
        }
    });
    #[cfg(feature = "bench-current-foldback-policy")]
    FOLDBACK_PENDING.store(0, portable_atomic::Ordering::Relaxed);
}
fn scan_inner(raw: [u16; 3], bus_low_block: Option<u32>) -> bool {
    unsafe {
        let Some(limit) = (&mut *core::ptr::addr_of_mut!(LIMIT)).as_mut() else {
            #[cfg(feature = "bench-average-diagnostic")]
            {
                if DIAG[4] == 0 {
                    DIAG[4] = 1;
                }
            }
            return false;
        };
        #[cfg(feature = "bench-current-rolling-warning")]
        if let Some(value) = (&mut *core::ptr::addr_of_mut!(ROLLING))
            .as_mut()
            .and_then(|rolling| rolling.scan(raw))
        {
            FOLDBACK_PENDING.fetch_max(value, portable_atomic::Ordering::Relaxed);
        }
        let residual = limit.scan(raw);
        #[cfg(feature = "bench-rate-curve")]
        if let Some(value) = residual {
            super::rate_census::current_block(value);
        }
        #[cfg(all(
            feature = "bench-current-foldback-policy",
            not(feature = "bench-current-bus-assisted-foldback"),
            not(feature = "bench-current-report-only")
        ))]
        if let Some(value) = residual.filter(|_| limit.over_streak() == 1) {
            // Latch until foreground consumes it. A later under-limit block
            // must not erase a warning that foreground has not observed yet.
            // Retain the worst pending complete-block excursion.
            FOLDBACK_PENDING.fetch_max(value as u32, portable_atomic::Ordering::Relaxed);
        }
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        if let Some(value) =
            residual.filter(|_| limit.over_streak() == 1 && bus_low_block.unwrap_or(0) != 0)
        {
            FOLDBACK_PENDING.fetch_max(value as u32, portable_atomic::Ordering::Relaxed);
        }
        #[cfg(feature = "bench-average-diagnostic")]
        {
            if let Some(value) = residual {
                DIAG[2] = value;
                DIAG[3] = DIAG[3].max(value);
            }
            if limit.tripped() && DIAG[4] == 0 {
                DIAG[4] = if raw.iter().any(|&v| v == 0 || v >= 4095) {
                    2
                } else {
                    3
                };
                DIAG[5] = raw[0] as i32;
                DIAG[6] = raw[1] as i32;
                DIAG[7] = raw[2] as i32;
            }
        }
        #[cfg(not(feature = "bench-average-diagnostic"))]
        let _ = residual;
        #[cfg(not(feature = "bench-current-bus-assisted-foldback"))]
        let _ = bus_low_block;
        !limit.tripped()
    }
}

/// Foreground consumes the first-over notification. This carries no threshold
/// or output authority; the caller must use the guarded live-duty transaction.
#[cfg(feature = "bench-current-foldback-policy")]
pub struct FoldbackWarning {
    pub residual: u32,
    pub allowance: u32,
}

#[cfg(feature = "bench-current-foldback-policy")]
pub fn take_foldback_warning() -> Option<FoldbackWarning> {
    let residual = FOLDBACK_PENDING.swap(0, portable_atomic::Ordering::Relaxed);
    if residual == 0 {
        return None;
    }
    let allowance = cortex_m::interrupt::free(|_| unsafe { THRESHOLD.unwrap_or(0) as u32 });
    (allowance != 0).then_some(FoldbackWarning {
        residual,
        allowance,
    })
}

#[cfg(feature = "bench-current-foldback-policy")]
pub fn record_foldback(duty: u32, step: u32) -> bool {
    if !super::duty_envelope::contains(duty) || !matches!(step, 10 | 20 | 30 | 40 | 50) {
        return false;
    }
    let acknowledged = cortex_m::interrupt::free(|_| unsafe {
        let acknowledged = (&mut *core::ptr::addr_of_mut!(LIMIT))
            .as_mut()
            .is_some_and(policy::Limit::acknowledge_reduction);
        #[cfg(feature = "bench-current-rolling-warning")]
        if acknowledged {
            if let Some(rolling) = (&mut *core::ptr::addr_of_mut!(ROLLING)).as_mut() {
                rolling.applied_reduction();
            }
        }
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        if acknowledged {
            (&mut *core::ptr::addr_of_mut!(BUS_EVIDENCE)).applied_reduction();
        }
        acknowledged
    });
    if !acknowledged {
        return false;
    }
    FOLDBACK_LAST.store(duty, portable_atomic::Ordering::Relaxed);
    FOLDBACK_LAST_STEP.store(step, portable_atomic::Ordering::Relaxed);
    let count = FOLDBACK_COUNT.load(portable_atomic::Ordering::Relaxed);
    FOLDBACK_COUNT.store(count.saturating_add(1), portable_atomic::Ordering::Relaxed);
    true
}

#[cfg(feature = "bench-bus-recovery")]
pub fn record_recovery(duty: u32) -> bool {
    if !super::duty_envelope::contains(duty) {
        return false;
    }
    cortex_m::interrupt::free(|_| unsafe {
        if let Some(limit) = (&mut *core::ptr::addr_of_mut!(LIMIT)).as_mut() {
            limit.discard_partial();
        }
        (&mut *core::ptr::addr_of_mut!(BUS_SAG)).discard_partial();
        (&mut *core::ptr::addr_of_mut!(BUS_EVIDENCE)).applied_reduction();
    });
    RECOVERY_LAST.store(duty, portable_atomic::Ordering::Relaxed);
    RECOVERY_COUNT.fetch_add(1, portable_atomic::Ordering::Relaxed);
    true
}

#[cfg(feature = "bench-bus-recovery")]
pub fn record_duty_change() {
    cortex_m::interrupt::free(|_| unsafe {
        if let Some(limit) = (&mut *core::ptr::addr_of_mut!(LIMIT)).as_mut() {
            limit.discard_partial();
        }
        (&mut *core::ptr::addr_of_mut!(BUS_SAG)).discard_partial();
        (&mut *core::ptr::addr_of_mut!(BUS_EVIDENCE)).applied_reduction();
    });
}

#[cfg(feature = "bench-current-foldback-policy")]
pub fn foldback_summary<W: Write>(out: &mut W) {
    #[cfg(feature = "bench-current-report-only")]
    let _ = writeln!(
        out,
        "CURRENTMODE average=report_only physical_psu_limit_ma=4000 bus_sag_stop=1 raw_validity_stop=1"
    );
    let _ = writeln!(
        out,
        "CURRENTFOLDBACK count={} last_duty_tenths={} last_step_tenths={} step_policy=severity10_50 release=none first_over_warning=1 unacknowledged_second_over_stop=1 foreground_writer=1",
        FOLDBACK_COUNT.load(portable_atomic::Ordering::Relaxed),
        FOLDBACK_LAST.load(portable_atomic::Ordering::Relaxed),
        FOLDBACK_LAST_STEP.load(portable_atomic::Ordering::Relaxed)
    );
    #[cfg(feature = "bench-bus-recovery")]
    let _ = writeln!(
        out,
        "BUSRECOVERY count={} last_duty_tenths={} hold_us=2000000 step_tenths=10 period_us=1000000 backoff_tenths=10 current_only_arms=0",
        RECOVERY_COUNT.load(portable_atomic::Ordering::Relaxed),
        RECOVERY_LAST.load(portable_atomic::Ordering::Relaxed)
    );
}
pub fn scan(raw: [u16; 3]) -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        let completed = (&mut *core::ptr::addr_of_mut!(BUS_EVIDENCE))
            .observe(false)
            .1;
        #[cfg(not(feature = "bench-current-bus-assisted-foldback"))]
        let completed = None;
        scan_inner(raw, completed)
    })
}
/// Feed one sequential, asynchronously triggered DMA scan to the signed block
/// average. Individual phase peaks are deliberately not a software stop: at
/// 201 us cadence the trigger walks through the 20 kHz PWM phase, so adjacent
/// scans can repeatedly sample the end-of-ON ripple. Treating that expected
/// ripple as DC-link current created a duty-dependent false wall. `Limit::scan`
/// includes exact phase rails in that average; it does NOT reject them. The
/// latest-frame cache accepts those phase codes too, while rejecting invalid
/// bus/VREF rails and impossible DMA words. nFAULT/VDS remains the hardware
/// backstop. The signed complete-block average owns its
/// own foldback/stop; `bench-phase-peak-stop` is a separate diagnostic veto.
pub fn scan_raw(raw: [u16; 3], bus: u16, vref: u16, vcal: u32) -> bool {
    #[cfg(feature = "bench-current-report-only")]
    CURRENT_ONLY_FAILURE.store(false, portable_atomic::Ordering::Relaxed);
    cortex_m::interrupt::free(|_| unsafe {
        if (*core::ptr::addr_of!(PHASE_ZERO)).is_none() {
            return false;
        }
        if vref == 0 || vref >= 4095 || vcal == 0 || vcal > 4095 {
            return false;
        }
        #[cfg(feature = "bench-phase-peak-stop")]
        let phase_peak;
        #[cfg(feature = "bench-rate-census")]
        {
            let reference = FAST_BUS_REF.load(portable_atomic::Ordering::Relaxed);
            super::rate_census::bus_scan(bus, vref, (reference >> 16) as u16, reference as u16);
            #[cfg(feature = "bench-phase-current-census")]
            let observed_peak = super::rate_census::phase_scan(
                raw,
                (*core::ptr::addr_of!(PHASE_ZERO)).unwrap_or([2048; 3]),
                bus,
                vref,
                (reference >> 16) as u16,
                reference as u16,
            );
            #[cfg(feature = "bench-phase-peak-stop")]
            {
                phase_peak = observed_peak;
            }
            #[cfg(all(
                feature = "bench-phase-current-census",
                not(feature = "bench-phase-peak-stop")
            ))]
            let _ = observed_peak;
        }
        #[cfg(feature = "bench-fast-bus-sag")]
        if !(&mut *core::ptr::addr_of_mut!(FAST_BUS_SAG))
            .as_mut()
            .is_some_and(|guard| guard.observe(bus, vref))
        {
            FAST_BUS_TRIPPED.store(true, portable_atomic::Ordering::Relaxed);
            #[cfg(feature = "bench-average-diagnostic")]
            {
                DIAG[4] = 6;
                DIAG[5] = bus as i32;
                DIAG[6] = vref as i32;
                DIAG[7] = vcal as i32;
            }
            return false;
        }
        #[cfg(feature = "bench-phase-peak-stop")]
        if phase_peak {
            PHASE_PEAK_TRIPPED.store(true, portable_atomic::Ordering::Relaxed);
            return false;
        }
        let bus_low = bus as u32 * vcal < 963u32 * vref as u32;
        if bus_low {
            let old = BUS_LOW_CODES.load(portable_atomic::Ordering::Relaxed);
            BUS_LOW_CODES.store(old.saturating_add(1), portable_atomic::Ordering::Relaxed);
        }
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        let (early_bus, completed_bus) =
            (&mut *core::ptr::addr_of_mut!(BUS_EVIDENCE)).observe(bus_low);
        #[cfg(feature = "bench-current-bus-assisted-foldback")]
        if early_bus {
            let allowance = THRESHOLD.unwrap_or(0).max(1) as u32;
            FOLDBACK_PENDING.fetch_max(
                allowance.saturating_mul(2),
                portable_atomic::Ordering::Relaxed,
            );
        }
        #[cfg(not(feature = "bench-current-bus-assisted-foldback"))]
        let completed_bus = None;
        if !(&mut *core::ptr::addr_of_mut!(BUS_SAG)).observe(bus, vref, vcal) {
            #[cfg(feature = "bench-average-diagnostic")]
            {
                DIAG[4] = 5;
                DIAG[5] = bus as i32;
                DIAG[6] = vref as i32;
                DIAG[7] = vcal as i32;
            }
            return false;
        }
        let rails = raw.iter().filter(|&&v| v == 0 || v >= 4095).count() as u32;
        if rails != 0 {
            let old = RAIL_CODES.load(portable_atomic::Ordering::Relaxed);
            RAIL_CODES.store(
                old.saturating_add(rails),
                portable_atomic::Ordering::Relaxed,
            );
        }
        let ok = scan_inner(raw, completed_bus);
        #[cfg(feature = "bench-current-report-only")]
        CURRENT_ONLY_FAILURE.store(!ok, portable_atomic::Ordering::Relaxed);
        ok
    })
}

#[cfg(feature = "bench-current-report-only")]
pub fn current_only_failure() -> bool {
    CURRENT_ONLY_FAILURE.load(portable_atomic::Ordering::Relaxed)
}
#[cfg(feature = "bench-fast-bus-sag")]
pub fn fast_bus_tripped() -> bool {
    FAST_BUS_TRIPPED.load(portable_atomic::Ordering::Relaxed)
}
#[cfg(feature = "bench-phase-peak-stop")]
pub fn phase_peak_tripped() -> bool {
    PHASE_PEAK_TRIPPED.load(portable_atomic::Ordering::Relaxed)
}
#[cfg(feature = "bench-fast-sag-causal")]
pub fn fast_bus_streak() -> u8 {
    // DMA is the sole writer while powered. Foreground only reads after stop.
    unsafe {
        (&*core::ptr::addr_of!(FAST_BUS_SAG))
            .as_ref()
            .map_or(0, |g| g.low_streak())
    }
}
#[cfg(feature = "bench-average-diagnostic")]
pub fn dump<W: Write>(out: &mut W) {
    let d = cortex_m::interrupt::free(|_| unsafe { DIAG });
    let _ = writeln!(
        out,
        "AVGDIAG zero={} allowance={} residual={} max_residual={} cause={} raw_a={} raw_b={} raw_c={} calibrated=0",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    );
}

pub fn quality_summary<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "CURRENTQUALITY phase_rail_codes={} phase_rail_stop=0 bus_low_codes={} bus_average_scans={} vref_validity_stop=1",
        RAIL_CODES.load(portable_atomic::Ordering::Relaxed),
        BUS_LOW_CODES.load(portable_atomic::Ordering::Relaxed),
        policy::SCANS
    );
    #[cfg(feature = "bench-fast-bus-sag")]
    {
        let reference = FAST_BUS_REF.load(portable_atomic::Ordering::Relaxed);
        let _ = writeln!(
            out,
            "FASTBUS baseline_bus={} baseline_vref={} threshold_pct=95 consecutive=3 scan_period_us={} tripped={} same_wake=1",
            reference >> 16,
            reference & 0xffff,
            adc_stream::PERIOD_US,
            FAST_BUS_TRIPPED.load(portable_atomic::Ordering::Relaxed) as u8,
        );
    }
}
