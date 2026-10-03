//! TIM6-owned independent guard. Guarded commit is the only powered writer.
//! Reference COM uses this writer only for explicitly armed powered handoff.
//! guardcheck exercises post-stop writer refusal with bridge disabled.
//! Mutations serialized
//! against COMP/COM with short critical sections. No ADC or UART in its ISR.
use super::*;
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-reentry-staging")]
static REENTRY_STATS_STAGED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-reentry-staging")]
static REENTRY_STATS_USED: AtomicBool = AtomicBool::new(false);
static AWAKE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_STAGED: AtomicBool = AtomicBool::new(false);
static ADOPT_REFUSAL: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_ADOPTED: AtomicBool = AtomicBool::new(false);
static WAKE_US: AtomicU32 = AtomicU32::new(0);
static mut CLOCK: sampled_clock::Clock = sampled_clock::Clock::new(0);
static REASON: AtomicU32 = AtomicU32::new(0);
static STOP_US: AtomicU32 = AtomicU32::new(0);
static MAX_US: AtomicU32 = AtomicU32::new(0);
static VETO: AtomicU32 = AtomicU32::new(0);
static COMMIT_MAX_US: AtomicU32 = AtomicU32::new(0);
static COMMITS: AtomicU32 = AtomicU32::new(0);
static FEEDBACK_N: AtomicU32 = AtomicU32::new(0);
static PEAK_RAW: AtomicU32 = AtomicU32::new(0);
static BUS_MIN: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(feature = "bench-fast-bus-tail")]
static mut BUS_TAIL: [[u16; 8]; 16] = [[0; 8]; 16];
#[cfg(feature = "bench-fast-bus-tail")]
static mut BUS_TAIL_N: u32 = 0;
#[cfg(feature = "bench-fast-bus-inject")]
static FAST_BUS_INJECT_SCANS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-fast-bus-inject")]
static FAST_BUS_INJECT_OFF_AGE_US: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-fast-bus-inject")]
static FAST_BUS_INJECT_REAL_BUS: AtomicU32 = AtomicU32::new(0);
// Diagnostic mirror of the guard-owned accepted-event deadline. The guard is
// authoritative; this lets the post-run interval ring record the exact value
// seen before each accepted event without another critical section.
#[cfg(feature = "bench-speed-event-watch")]
static SPEED_EVENT_LIMIT_US: AtomicU32 = AtomicU32::new(1000);
#[cfg(feature = "bench-driver-fault-probe")]
static DRIVER_FAULT_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driver-fault-probe")]
static DRIVER_FAULT_FIRST_HIGH_US: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driver-fault-probe")]
static DRIVER_FAULT_SAMPLES: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driver-fault-probe")]
static DRIVER_FAULT_HELD_US: AtomicU32 = AtomicU32::new(0);
static mut ADC_FAULT: [u32; 6] = [0; 6];
#[cfg(feature = "bench-dma-feedback")]
static mut FIRST_DELIVERY: [u32; 6] = [0; 6]; // seen,initial_age,decision,previous,acquired,fault
#[cfg(feature = "bench-dma-feedback")]
static mut CURRENT_SUMS: current_sums::Sums = current_sums::Sums::new();
#[cfg(feature = "bench-dma-stall")]
static mut FIFO_STALL: scan_queue::StallOnce = scan_queue::StallOnce::new();
#[cfg(feature = "bench-dma-feedback")]
pub fn clear_sums() {
    unsafe {
        (&mut *core::ptr::addr_of_mut!(CURRENT_SUMS)).clear();
    }
}
// Foreground-only: bounded first-window launch diagnostics, not current sums.
#[cfg(not(feature = "bench-dma-feedback"))]
static mut ADC_COVERAGE: adc_occupancy::Coverage = adc_occupancy::Coverage::new();
#[cfg(feature = "bench-dma-feedback")]
pub fn coverage_dump<W: Write>(out: &mut W) {
    #[cfg(feature = "bench-adc-phase")]
    adc_phase_dma::dump(out);
    let first = unsafe { FIRST_DELIVERY };
    let _ = writeln!(
        out,
        "DMASTART first_trigger_us={} steady_trigger_us={} preloaded_counter=1",
        adc_stream::FIRST_TRIGGER_US,
        adc_stream::PERIOD_US
    );
    let _ = writeln!(
        out,
        "FEEDBACKFIRST seen={} initial_age_us={} decision_us={} previous_us={} acquired_us={} fault={}",
        first[0], first[1], first[2], first[3], first[4], first[5]
    );
    let _ = writeln!(
        out,
        "DMAFEEDBACK trigger_us={} acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8={} queue_peak={} guard_irq={}",
        adc_stream::PERIOD_US,
        adc_stream::max_us(),
        adc_stream::queue_peak(),
        cfg!(feature = "bench-dma-guard") as u8
    );
    #[cfg(feature = "bench-driven-dma")]
    let _ = writeln!(
        out,
        "DMAOWNER sequential_tim3=1 forced_release_required=1 refusal_code=11"
    );
    let _ = writeln!(
        out,
        "ADCSTATS uncalibrated=1 segment_only=1 current_center=2048"
    );
    #[cfg(feature = "bench-dma-stall")]
    {
        let state = unsafe { (&*core::ptr::addr_of!(FIFO_STALL)).state() };
        let _ = writeln!(
            out,
            "FIFOFAULT state={} after_scans=20000 once_per_boot=1",
            state
        );
    }
    for ch in 0..5 {
        let words = unsafe { (&*core::ptr::addr_of!(CURRENT_SUMS)).words(ch) };
        let _ = snapshot::record(out, "S85", &words);
    }
}
#[cfg(feature = "bench-fast-bus-tail")]
pub fn bus_tail_dump<W: Write>(out: &mut W) {
    // Called only after powered ownership and ADC DMA have stopped.
    let total = unsafe { core::ptr::addr_of!(BUS_TAIL_N).read() };
    let count = total.min(16);
    let last_event_us = cortex_m::interrupt::free(|_| unsafe {
        (&*core::ptr::addr_of!(GUARD))
            .as_ref()
            .map_or(0, |guard| guard.stopped_tracking()[1])
    });
    let _ = writeln!(
        out,
        "BUSSCAN n={} total={} fields=stamp_lo,stamp_hi,bus,vref,ia,ib,ic,duty chronological=1 last_event_us={} stop_reason={} stop_us={} diagnostic_only=1",
        count,
        total,
        last_event_us,
        reason(),
        STOP_US.load(Relaxed)
    );
    for ordinal in total - count..total {
        let row = unsafe {
            core::ptr::addr_of!(BUS_TAIL)
                .cast::<[u16; 8]>()
                .add((ordinal as usize) & 15)
                .read()
        };
        let _ = snapshot::record(out, "BS85", &row);
    }
}
#[cfg(not(feature = "bench-dma-feedback"))]
pub fn coverage_dump<W: Write>(out: &mut W) {
    let c = unsafe { &*core::ptr::addr_of!(ADC_COVERAGE) };
    let _ = writeln!(
        out,
        "ADCLAUNCH bins=8 period_ticks=6400 limit=192 early_window=1 aperture_known=0 joint_sector_coverage=0"
    );
    for i in 0..3 {
        let mut row = [0u16; 11];
        row[0] = i as u16;
        row[1] = c.attempted[i] as u16;
        row[2] = c.rejected[i] as u16;
        for j in 0..8 {
            row[j + 3] = c.bins[i][j] as u16;
        }
        let _ = snapshot::record(out, "AL85", &row);
    }
}
#[cfg(feature = "bench-cycle450")]
type RuntimeGuard = powered_guard::RunGuard<
    2223,
    {
        if cfg!(feature = "bench-event100") {
            100
        } else {
            238
        }
    },
    { cfg!(feature = "bench-fast-cycle-report") },
>;
#[cfg(all(feature = "bench-cycle400", not(feature = "bench-cycle450")))]
type RuntimeGuard = powered_guard::RunGuard<2500, 238>;
#[cfg(all(feature = "bench-cycle360", not(feature = "bench-cycle400")))]
type RuntimeGuard = powered_guard::RunGuard<2778, 238>;
#[cfg(all(feature = "bench-range350", not(feature = "bench-cycle360")))]
type RuntimeGuard = powered_guard::RunGuard<2858, 238>;
#[cfg(all(feature = "bench-range345", not(feature = "bench-range350")))]
type RuntimeGuard = powered_guard::RunGuard<2899, 241>;
#[cfg(all(feature = "bench-range340", not(feature = "bench-range345")))]
type RuntimeGuard = powered_guard::RunGuard<2942, 245>;
#[cfg(all(feature = "bench-range335", not(feature = "bench-range340")))]
type RuntimeGuard = powered_guard::RunGuard<2986, 248>;
#[cfg(all(feature = "bench-range330", not(feature = "bench-range335")))]
type RuntimeGuard = powered_guard::RunGuard<3031, 252>;
#[cfg(all(feature = "bench-range320", not(feature = "bench-range330")))]
type RuntimeGuard = powered_guard::RunGuard<3125, 260>;
#[cfg(all(feature = "bench-range310", not(feature = "bench-range320")))]
type RuntimeGuard = powered_guard::RunGuard<3226, 268>;
#[cfg(all(feature = "bench-range300", not(feature = "bench-range310")))]
type RuntimeGuard = powered_guard::RunGuard<3333, 277>;
#[cfg(not(feature = "bench-range300"))]
type RuntimeGuard = powered_guard::Guard;
static mut GUARD: Option<RuntimeGuard> = None;
// Written only on a rejected accepted-event cycle, retained through safing.
static mut CYCLE_FAULT: [u32; 4] = [0; 4];
pub fn owns() -> bool {
    ACTIVE.load(Relaxed)
}
pub fn reason() -> u32 {
    REASON.load(Relaxed)
}
#[cfg(feature = "bench-speed-event-watch")]
pub fn speed_event_limit_us() -> u16 {
    SPEED_EVENT_LIMIT_US.load(Relaxed) as u16
}
#[cfg(not(feature = "bench-speed-event-watch"))]
pub fn speed_event_limit_us() -> u16 {
    1000
}
pub fn stopped_snapshot() -> [u32; 9] {
    [
        reason(),
        STOP_US.load(Relaxed),
        COMMITS.load(Relaxed),
        FEEDBACK_N.load(Relaxed),
        PEAK_RAW.load(Relaxed),
        BUS_MIN.load(Relaxed),
        MAX_US.load(Relaxed),
        COMMIT_MAX_US.load(Relaxed),
        VETO.load(Relaxed),
    ]
}
pub fn abort() {
    trip(powered_guard::Fault::HostAbort);
}
/// Comparator dispatch storm, distinct from a host-requested stop.
pub fn irq_storm() {
    trip_reason(13);
}
/// Explicit bench fault injection after powered ownership is established.
/// This exercises the same tracking trip/safing path as the missing-event
/// guard without first holding a commutation sector energized.
#[cfg(feature = "bench-normal-restart")]
pub fn inject_tracking() -> bool {
    if !owns() || reason() != 0 {
        return false;
    }
    trip(powered_guard::Fault::Tracking);
    true
}
pub fn summary<W: Write>(out: &mut W) {
    let state = cortex_m::interrupt::free(|_| unsafe {
        (&*core::ptr::addr_of!(GUARD))
            .as_ref()
            .map(|g| g.stopped_tracking())
    });
    if let Some(s) = state {
        let _ = writeln!(
            out,
            "TRACKSTOP event_fault={} last_event_us={} sector={} last_poll_us={} feedback_acquired_us={} retained_operational_state=1",
            s[0], s[1], s[2], s[3], s[4]
        );
    }
    #[cfg(feature = "bench-speed-event-watch")]
    {
        let limit = cortex_m::interrupt::free(|_| unsafe {
            (&*core::ptr::addr_of!(GUARD))
                .as_ref()
                .map_or(0, |g| g.event_stale_limit())
        });
        let _ = writeln!(
            out,
            "SPEEDEVENTWATCH max_us={} tighten_only=1 periods=3 poll_us=100 diagnostic=1",
            limit
        );
    }
    let _ = writeln!(
        out,
        "ADOPTREFUSAL reason={} zero_pass=1 foreground_only=1",
        ADOPT_REFUSAL.load(Relaxed)
    );
    #[cfg(feature = "bench-fast-cycle-report")]
    {
        let (count, minimum) = cortex_m::interrupt::free(|_| unsafe {
            (&*core::ptr::addr_of!(GUARD))
                .as_ref()
                .map_or((0, 0), |g| g.fast_cycles())
        });
        // r1: report-only2223us floor, segment-local count/minimum in us.
        let _ = writeln!(out, "FASTCYCLE r1 {} {}", count, minimum);
        let (count, minimum) = cortex_m::interrupt::free(|_| unsafe {
            (&*core::ptr::addr_of!(GUARD))
                .as_ref()
                .map_or((0, 0), |g| g.fast_events())
        });
        let _ = writeln!(
            out,
            "FASTEVENT r1 {} {} report_only=1 stale_max_us=1000 order_kill=1",
            count, minimum
        );
    }
    #[cfg(feature = "bench-reentry-staging")]
    let _ = writeln!(
        out,
        "REENTRYSTATS used={} preparation_before_acquisition=1 gate_authority=0",
        REENTRY_STATS_USED.load(Relaxed) as u8
    );
    let _ = writeln!(
        out,
        "RUNLIMIT cycle_min_us={} event_min_us={} cycle_max_us=6000 event_max_us=1000 experimental={}",
        if cfg!(feature = "bench-cycle450") {
            2223
        } else if cfg!(feature = "bench-cycle400") {
            2500
        } else if cfg!(feature = "bench-cycle360") {
            2778
        } else if cfg!(feature = "bench-range350") {
            2858
        } else if cfg!(feature = "bench-range345") {
            2899
        } else if cfg!(feature = "bench-range340") {
            2942
        } else if cfg!(feature = "bench-range335") {
            2986
        } else if cfg!(feature = "bench-range330") {
            3031
        } else if cfg!(feature = "bench-range320") {
            3125
        } else if cfg!(feature = "bench-range310") {
            3226
        } else if cfg!(feature = "bench-range300") {
            3333
        } else {
            4000
        },
        if cfg!(feature = "bench-event100") {
            100
        } else if cfg!(feature = "bench-range350") {
            238
        } else if cfg!(feature = "bench-range345") {
            241
        } else if cfg!(feature = "bench-range340") {
            245
        } else if cfg!(feature = "bench-range335") {
            248
        } else if cfg!(feature = "bench-range330") {
            252
        } else if cfg!(feature = "bench-range320") {
            260
        } else if cfg!(feature = "bench-range310") {
            268
        } else if cfg!(feature = "bench-range300") {
            277
        } else {
            333
        },
        cfg!(feature = "bench-range300") as u8
    );
    let _ = writeln!(
        out,
        "POWERPATH reason={} stop_us={} isr_max_us={} commit_max_us={} veto={} active={} disabled={}",
        reason(),
        STOP_US.load(Relaxed),
        MAX_US.load(Relaxed),
        COMMIT_MAX_US.load(Relaxed),
        VETO.load(Relaxed),
        owns() as u8,
        disabled() as u8
    );
    #[cfg(feature = "bench-fast-bus-inject")]
    let _ = writeln!(
        out,
        "FASTBUSINJECT powered_scans={} injected_at=9000,9001,9002 real_bus={} off_age_us={} disabled={} diagnostic_only=1",
        FAST_BUS_INJECT_SCANS.load(Relaxed),
        FAST_BUS_INJECT_REAL_BUS.load(Relaxed),
        FAST_BUS_INJECT_OFF_AGE_US.load(Relaxed),
        disabled() as u8
    );
    let _ = writeln!(out, "POWERCOMMITS applied={}", COMMITS.load(Relaxed));
    let _ = writeln!(
        out,
        "POWERFEEDBACK scans={} peak_abs_raw={} bus_min_mv={}",
        FEEDBACK_N.load(Relaxed),
        PEAK_RAW.load(Relaxed),
        BUS_MIN.load(Relaxed)
    );
    #[cfg(feature = "bench-driver-fault-probe")]
    {
        let samples = DRIVER_FAULT_SAMPLES.load(Relaxed);
        let _ = writeln!(
            out,
            "DRIVERFAULTPROBE attempted={} initial_low={} low_100us={} low_1000us={} low_4000us={} low_5000us={} first_high_us={} held_us={} outputs_fault={} final_enable={} diagnostic_only=1",
            (samples & (1 << 8) != 0) as u8,
            (samples & 1 != 0) as u8,
            (samples & (1 << 1) != 0) as u8,
            (samples & (1 << 2) != 0) as u8,
            (samples & (1 << 3) != 0) as u8,
            (samples & (1 << 4) != 0) as u8,
            DRIVER_FAULT_FIRST_HIGH_US.load(Relaxed),
            DRIVER_FAULT_HELD_US.load(Relaxed),
            (samples & (1 << 9) != 0) as u8,
            get_idr(3, 1) as u8
        );
    }
    if reason() == 12 {
        let s = unsafe { CYCLE_FAULT };
        let _ = writeln!(
            out,
            "CYCLEFAULT step={} previous_us={} decision_us={} delta_us={} guard_timestamp=1",
            s[0], s[1], s[2], s[3]
        );
    }
    if reason() == 11 {
        let s = unsafe { ADC_FAULT };
        let _ = writeln!(
            out,
            "ADCFAULT stage={} channel={} elapsed_us={} cr={} isr={} launch_us={}",
            s[0], s[1], s[2], s[3], s[4], s[5]
        );
        #[cfg(feature = "bench-dma-feedback")]
        if s[0] >= 20 {
            let _ = writeln!(
                out,
                "DMAFAULT code={} flags={} remaining={} service_gap_us={}",
                s[0] - 20,
                s[5] & 15,
                (s[5] >> 4) & 15,
                s[2]
            );
        }
    }
}
fn now() -> u32 {
    // Read hardware inside the same critical section as the state update:
    // a preempted foreground read must never follow a newer ISR sample.
    cortex_m::interrupt::free(|_| unsafe { (&mut *core::ptr::addr_of_mut!(CLOCK)).sample(t17()) })
}
#[cfg(feature = "bench-dma-feedback")]
pub fn stream_now() -> u32 {
    now()
}
/// Correlate the independent ADC acquisition clock with this segment clock.
/// Sample BOTH from the same hardware read under one IRQ mask; preserve age.
#[cfg(feature = "bench-adc-latest")]
pub fn stream_stamp(acquired: u32, clock: &mut sampled_clock::Clock) -> u32 {
    cortex_m::interrupt::free(|_| unsafe {
        let counter = t17();
        clock.map_stamp(&mut *core::ptr::addr_of_mut!(CLOCK), counter, acquired)
    })
}
pub fn stop() {
    #[cfg(feature = "bench-reentry-staging")]
    REENTRY_STATS_STAGED.store(false, Relaxed);
    if !ACTIVE.swap(false, Relaxed) {
        return;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    unsafe {
        let t = &*stm32::TIM6::ptr();
        t.dier().write(|w| w.bits(0));
        t.cr1().write(|w| w.bits(0));
        t.sr().write(|w| w.bits(0));
    }
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
    #[cfg(feature = "bench-cpu-timing")]
    cpu_meter::finish();
}
fn fault_code(f: powered_guard::Fault) -> u32 {
    use powered_guard::Fault::*;
    match f {
        CampaignDeadline => 1,
        SegmentDeadline => 2,
        TickGap => 3,
        FeedbackStale => 4,
        Current => 5,
        Bus => 6,
        Driver => 7,
        Tracking => 8,
        HostAbort => 9,
        InvalidSeed => 10,
        AdcTimeout => 11,
        CycleTiming => 12,
    }
}
fn trip(f: powered_guard::Fault) {
    trip_reason(fault_code(f));
}
fn trip_reason(code: u32) {
    if REASON.load(Relaxed) == 0 {
        REASON.store(code, Relaxed);
        STOP_US.store(now(), Relaxed);
    }
    stop();
    gates_off();
    #[cfg(feature = "bench-driver-fault-probe")]
    if code == 7 {
        // The powered owner has already lost all gate authority. Keeping the
        // DRV awake briefly is safe because all six inputs and TIM1 MOE are
        // low, and is necessary to observe its documented 4 ms auto-retry.
        let initial_low = !get_idr(1, 14);
        DRIVER_FAULT_SAMPLES.store((1 << 8) | initial_low as u32, Relaxed);
        DRIVER_FAULT_PENDING.store(true, Relaxed);
        return;
    }
    set_pin(3, 1, false);
}

/// Finish the diagnostic-only nFAULT observation in foreground. Never wait in
/// an ISR and never retain ENABLE if any gate-off invariant is lost.
#[cfg(feature = "bench-driver-fault-probe")]
#[inline(never)]
pub fn finish_driver_fault_probe() {
    if !DRIVER_FAULT_PENDING.swap(false, Relaxed) {
        return;
    }
    let began = STOP_US.load(Relaxed);
    let mut sampled = 0u32;
    let mut bits = DRIVER_FAULT_SAMPLES.load(Relaxed);
    loop {
        let elapsed = now().wrapping_sub(began);
        if !outputs_disabled() {
            bits |= 1 << 9;
            break;
        }
        let low = !get_idr(1, 14);
        if !low && DRIVER_FAULT_FIRST_HIGH_US.load(Relaxed) == 0 {
            DRIVER_FAULT_FIRST_HIGH_US.store(elapsed.max(1), Relaxed);
        }
        for (index, threshold) in [100u32, 1_000, 4_000, 5_000].iter().enumerate() {
            let marker = 1u32 << index;
            if sampled & marker == 0 && elapsed >= *threshold {
                sampled |= marker;
                if low {
                    bits |= 1 << (index + 1);
                }
            }
        }
        if elapsed >= 5_500 {
            DRIVER_FAULT_HELD_US.store(elapsed, Relaxed);
            break;
        }
    }
    DRIVER_FAULT_SAMPLES.store(bits, Relaxed);
    gates_off();
    set_pin(3, 1, false);
}
#[cfg(not(feature = "bench-driver-fault-probe"))]
#[inline(always)]
pub fn finish_driver_fault_probe() {}
fn disabled() -> bool {
    !get_idr(3, 1) && outputs_disabled()
}
pub fn outputs_disabled() -> bool {
    // Same six DRV gate inputs, sampled once per GPIO port. This check is on
    // every acquisition phase visit; six generic pin reads waste its gap budget.
    unsafe {
        (*stm32::TIM1::ptr()).bdtr().read().bits() & (1 << 15) == 0
            && (*stm32::GPIOA::ptr()).idr().read().bits()
                & ((1 << 10) | (1 << 9) | (1 << 8) | (1 << 7))
                == 0
            && (*stm32::GPIOB::ptr()).idr().read().bits() & ((1 << 1) | (1 << 0)) == 0
    }
}
pub fn ready() -> bool {
    AWAKE.load(Relaxed) && get_idr(3, 1) && get_idr(1, 14)
}
/// Adopt only an opaque release token from a healthy driven owner. No wake
/// pulse, ADC timestamp refresh, or output authority is granted here.
#[cfg(feature = "bench-driven-handoff")]
pub fn adopt_driven(transfer: &driven_run::Transfer) -> bool {
    AWAKE.store(false, Relaxed);
    DRIVEN_ADOPTED.store(false, Relaxed);
    let refusal = if owns() {
        1
    } else if driven_run::owns() {
        2
    } else if !outputs_disabled() {
        3
    } else if !get_idr(3, 1) {
        4
    } else if !get_idr(1, 14) {
        5
    } else if !transfer.fresh() {
        6
    } else if !DRIVEN_STAGED.swap(false, Relaxed) {
        7
    } else {
        0
    };
    ADOPT_REFUSAL.store(refusal, Relaxed);
    if refusal != 0 {
        return false;
    }
    cortex_m::interrupt::free(|_| unsafe {
        configure_timer();
    });
    DRIVEN_ADOPTED.store(true, Relaxed);
    AWAKE.store(true, Relaxed);
    true
}
#[cfg(feature = "bench-driven-handoff")]
pub fn adopt_refusal() -> u32 {
    ADOPT_REFUSAL.load(Relaxed)
}
/// Non-time-critical recorder reset before forced acquisition; no timer/gates.
#[cfg(feature = "bench-driven-handoff")]
pub fn stage_driven() {
    revoke_driven();
    if owns() || driven_run::owns() || !outputs_disabled() {
        return;
    }
    reset_run_statistics();
    DRIVEN_STAGED.store(true, Relaxed);
}
#[cfg(feature = "bench-driven-handoff")]
pub fn revoke_driven() {
    DRIVEN_STAGED.store(false, Relaxed);
    DRIVEN_ADOPTED.store(false, Relaxed);
}
/// Idle metrology only. Same bounded ADC reader as powered feedback; no gates.
#[inline(never)]
pub fn zero_check<W: Write>(out: &mut W, capture: bool) {
    if owns() || !disabled() {
        let _ = writeln!(out, "ZEROCHK result=1 disabled=0");
        return;
    }
    gates_off();
    let mut stats = [[zero_stats::Stats::new(); 5]; 2];
    let channels = [4, 1, 0, 6, 13];
    let mut reason = 0;
    if !wake(&mut || false) {
        reason = 2;
    }
    let mut clock = sampled_clock::Clock::new(t17());
    if reason == 0 {
        'scan: for block in &mut stats {
            for _ in 0..512 {
                for i in 0..5 {
                    if clock.sample(t17()) >= 200_000 {
                        reason = 3;
                        break 'scan;
                    }
                    if !ready() || !outputs_disabled() {
                        reason = 4;
                        break 'scan;
                    }
                    let Some(raw) = read_channel(channels[i]) else {
                        reason = 5;
                        break 'scan;
                    };
                    if !block[i].push(raw) {
                        reason = 6;
                        break 'scan;
                    }
                }
            }
        }
    }
    let elapsed = clock.sample(t17());
    gates_off();
    set_pin(3, 1, false);
    // GPIO IDR may lag the write; verify actual shutdown within a bounded
    // interval instead of evaluating IDR in the very next instruction.
    let stopped = t17();
    while !disabled() && t17().wrapping_sub(stopped) < 100 {}
    if !disabled() {
        reason = 7;
    }
    let _ = writeln!(
        out,
        "ZEROCHK result={} elapsed_us={} disabled={} same_powered_reader=1 applied_calibration=0 windows=2 same_wake=1",
        reason,
        elapsed,
        disabled() as u8
    );
    if capture {
        for (block, label) in stats.iter().zip(["Z85", "ZB85"]) {
            for i in 0..5 {
                let _ = snapshot::record(out, label, &block[i].words(channels[i] as u16));
            }
        }
    }
}
/// TI SLVSE39B pp6/35: up to1ms wake with nFAULT low is expected.
/// Only this all-gates-low, bounded preparation tolerates that transition.
pub fn wake(abort: &mut dyn FnMut() -> bool) -> bool {
    AWAKE.store(false, Relaxed);
    WAKE_US.store(0, Relaxed);
    if !disabled() {
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    set_pin(3, 1, true);
    let began = t17();
    while t17().wrapping_sub(began) < 1100 {
        if abort() || !outputs_disabled() || !get_idr(3, 1) {
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
    }
    WAKE_US.store(t17().wrapping_sub(began) as u32, Relaxed);
    if !get_idr(1, 14) {
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    AWAKE.store(true, Relaxed);
    true
}
pub fn wake_summary<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "DRVWAKE ready={} elapsed_us={} pwm_blocked_during_wake=1",
        AWAKE.load(Relaxed) as u8,
        WAKE_US.load(Relaxed)
    );
}
pub fn start(elapsed: u32, step: u8, sample: powered_guard::Feedback) -> bool {
    start_inner::<false, false>(elapsed, step, sample, 0, 20_000, UNUSED_LIMITS)
}
pub fn start_prepared(
    elapsed: u32,
    step: u8,
    sample: powered_guard::Feedback,
    age: u32,
    window_us: u32,
) -> bool {
    start_inner::<true, false>(elapsed, step, sample, age, window_us, UNUSED_LIMITS)
}
pub fn start_reentry(
    step: u8,
    sample: powered_guard::Feedback,
    age: u32,
    limits: powered_guard::Limits,
) -> bool {
    if !ready() || reason() != 8 {
        #[cfg(feature = "bench-reentry-staging")]
        REENTRY_STATS_STAGED.store(false, Relaxed);
        return false;
    }
    start_inner::<true, true>(limits.elapsed, step, sample, age, limits.segment, limits)
}
/// Foreground, after first-segment archive and acquisition's final shared
/// shutdown, before its first measurement. No output or timer authority.
#[cfg(feature = "bench-reentry-staging")]
pub fn stage_reentry_statistics() -> bool {
    REENTRY_STATS_STAGED.store(false, Relaxed);
    if owns() || reason() != 8 || !outputs_disabled() || (!disabled() && !ready()) {
        return false;
    }
    let stopped = STOP_US.load(Relaxed);
    reset_run_statistics();
    // Keep tracking-stop provenance until start_reentry admits a fresh seed.
    REASON.store(8, Relaxed);
    STOP_US.store(stopped, Relaxed);
    #[cfg(feature = "bench-reentry-guard-stage")]
    cortex_m::interrupt::free(|_| unsafe {
        RuntimeGuard::stage_install(&mut *core::ptr::addr_of_mut!(GUARD));
    });
    REENTRY_STATS_STAGED.store(true, Relaxed);
    true
}
#[cfg(feature = "bench-reentry-staging")]
pub fn reentry_stats_check<W: Write>(out: &mut W) {
    if owns() || !disabled() {
        let _ = writeln!(out, "REENTRYSTATSCHECK refused=1 gate_authority=0");
        return;
    }
    let saved = (reason(), STOP_US.load(Relaxed));
    REASON.store(9, Relaxed);
    let wrong = !stage_reentry_statistics() && !REENTRY_STATS_STAGED.load(Relaxed);
    REASON.store(8, Relaxed);
    STOP_US.store(123, Relaxed);
    let staged = stage_reentry_statistics()
        && reason() == 8
        && STOP_US.load(Relaxed) == 123
        && REENTRY_STATS_STAGED.load(Relaxed)
        && !REENTRY_STATS_USED.load(Relaxed);
    #[cfg(feature = "bench-reentry-guard-stage")]
    let staged = staged
        && cortex_m::interrupt::free(|_| unsafe {
            let slot = &mut *core::ptr::addr_of_mut!(GUARD);
            let dormant = slot.as_ref().is_some_and(|g| !g.healthy());
            let sample = powered_guard::Feedback {
                phase: [2048; 3],
                bus_mv: 12000,
                vref: 1500,
            };
            let stale = RuntimeGuard::admit(0, 1000, 1, sample, 100000, 20000, 1001).is_err();
            let Ok(admission) = RuntimeGuard::admit(0, 1000, 1, sample, 100000, 20000, 0) else {
                return false;
            };
            dormant
                && stale
                && RuntimeGuard::install_staged(slot, admission)
                && slot.as_ref().is_some_and(|g| g.healthy())
                && !owns()
                && disabled()
        });
    stop(); // Inactive stop must also revoke the preparation optimization.
    let revoked = !REENTRY_STATS_STAGED.load(Relaxed) && !owns() && disabled();
    REASON.store(saved.0, Relaxed);
    STOP_US.store(saved.1, Relaxed);
    let _ = writeln!(
        out,
        "REENTRYSTATSCHECK passed={} expected=3 disabled={} gate_authority=0",
        wrong as u8 + staged as u8 + revoked as u8,
        disabled() as u8
    );
}
/// Caller stops all drive before this; register setup may precede acquisition.
pub fn prepare() {
    #[cfg(feature = "bench-dma-stall")]
    unsafe {
        (&mut *core::ptr::addr_of_mut!(FIFO_STALL)).prepare();
    }
    #[cfg(feature = "bench-dma-feedback")]
    clear_sums(); // Also clears stale evidence if fresh acquisition later refuses.
    AWAKE.store(false, Relaxed);
    gates_off();
    set_pin(3, 1, false);
    cortex_m::interrupt::free(|_| unsafe {
        configure_timer();
    });
}
unsafe fn configure_timer() {
    unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | (1 << 4)));
        let t = &*stm32::TIM6::ptr();
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(99));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(stm32::Interrupt::TIM6_DAC_LPTIM1, 0);
    }
}
fn reset_run_statistics() {
    #[cfg(feature = "bench-dma-feedback")]
    unsafe {
        FIRST_DELIVERY = [0; 6];
    }
    // Staged driven entry bypasses prepare(); reset segment-local DMA evidence
    // here as well, before acquisition, not on the fresh-seed arm path.
    #[cfg(feature = "bench-dma-feedback")]
    clear_sums();
    unsafe {
        CYCLE_FAULT = [0; 4];
    }
    #[cfg(feature = "bench-reentry-staging")]
    REENTRY_STATS_USED.store(false, Relaxed);
    #[cfg(feature = "bench-cpu-timing")]
    cpu_meter::reset();
    #[cfg(feature = "bench-cpu-sparse")]
    cpu_sparse::reset();
    REASON.store(0, Relaxed);
    STOP_US.store(0, Relaxed);
    MAX_US.store(0, Relaxed);
    COMMITS.store(0, Relaxed);
    COMMIT_MAX_US.store(0, Relaxed);
    VETO.store(0, Relaxed);
    FEEDBACK_N.store(0, Relaxed);
    PEAK_RAW.store(0, Relaxed);
    BUS_MIN.store(u32::MAX, Relaxed);
    #[cfg(feature = "bench-driver-fault-probe")]
    {
        DRIVER_FAULT_PENDING.store(false, Relaxed);
        DRIVER_FAULT_FIRST_HIGH_US.store(0, Relaxed);
        DRIVER_FAULT_SAMPLES.store(0, Relaxed);
        DRIVER_FAULT_HELD_US.store(0, Relaxed);
    }
    #[cfg(not(feature = "bench-dma-feedback"))]
    unsafe {
        core::ptr::addr_of_mut!(ADC_COVERAGE).write(adc_occupancy::Coverage::new());
    }
}
// Only the private non-reentry callers pass this unused value. It cannot
// grant time: their specialization computes the original startup budgets.
const UNUSED_LIMITS: powered_guard::Limits = powered_guard::Limits {
    elapsed: 0,
    campaign: 0,
    segment: 0,
};
fn start_inner<const PREPARED: bool, const REENTRY: bool>(
    elapsed: u32,
    step: u8,
    sample: powered_guard::Feedback,
    age: u32,
    window_us: u32,
    limits: powered_guard::Limits,
) -> bool {
    #[cfg(feature = "bench-reentry-staging")]
    let staged = REENTRY_STATS_STAGED.swap(false, Relaxed);
    #[cfg(not(feature = "bench-reentry-staging"))]
    let staged = false;
    // Consume the one-shot adoption even on refusal. Shared shutdown revokes it.
    #[cfg(feature = "bench-driven-handoff")]
    let adopted = DRIVEN_ADOPTED.swap(false, Relaxed);
    #[cfg(not(feature = "bench-driven-handoff"))]
    let adopted = false;
    // Call only before enabling bridge, never replace a live scheduler.
    let awake = PREPARED && ready();
    if owns() || !outputs_disabled() || (!awake && !disabled()) {
        return false;
    }
    if adopted && (!awake || REENTRY) {
        return false;
    }
    if staged && (!awake || !REENTRY || adopted) {
        return false;
    }
    if staged {
        REASON.store(0, Relaxed);
        STOP_US.store(0, Relaxed);
        #[cfg(feature = "bench-reentry-staging")]
        REENTRY_STATS_USED.store(true, Relaxed);
    } else if !adopted {
        gates_off();
        reset_run_statistics();
    }
    if !awake {
        set_pin(3, 1, false);
    }
    BUS_MIN.store(sample.bus_mv, Relaxed);
    let Some(reserved) = (if REENTRY {
        Some(limits.elapsed)
    } else {
        powered_guard::reserved_elapsed(elapsed)
    }) else {
        REASON.store(1, Relaxed);
        return false;
    };
    if !(20_000..=600_000_000).contains(&window_us) {
        REASON.store(2, Relaxed);
        return false;
    }
    #[cfg(feature = "bench-guard-install")]
    let result = {
        let (campaign, segment) = if REENTRY {
            (limits.campaign, limits.segment)
        } else if PREPARED {
            (powered_guard::CAMPAIGN_BASE_US + window_us, window_us)
        } else {
            (powered_guard::CAMPAIGN_BASE_US, 20_000)
        };
        RuntimeGuard::admit(0, reserved, step, sample, campaign, segment, age)
    };
    #[cfg(not(feature = "bench-guard-install"))]
    let result = if REENTRY {
        RuntimeGuard::with_limits(
            0,
            limits.elapsed,
            step,
            sample,
            limits.campaign,
            limits.segment,
        )
    } else if PREPARED {
        RuntimeGuard::with_limits(
            0,
            reserved,
            step,
            sample,
            powered_guard::CAMPAIGN_BASE_US + window_us,
            window_us,
        )
    } else {
        RuntimeGuard::new(0, reserved, step, sample)
    };
    let mut guard = match result {
        Ok(g) => g,
        Err(f) => {
            REASON.store(fault_code(f), Relaxed);
            return false;
        }
    };
    #[cfg(not(feature = "bench-guard-install"))]
    if !guard.age_initial_feedback(age) {
        REASON.store(4, Relaxed);
        return false;
    }
    #[cfg(feature = "bench-dma-feedback")]
    unsafe {
        FIRST_DELIVERY[1] = age;
    }
    #[cfg(feature = "bench-speed-event-watch")]
    SPEED_EVENT_LIMIT_US.store(1000, Relaxed);
    cortex_m::interrupt::free(|_| unsafe {
        #[cfg(feature = "bench-reentry-guard-stage")]
        if REENTRY {
            if !staged || !RuntimeGuard::install_staged(&mut *core::ptr::addr_of_mut!(GUARD), guard)
            {
                REASON.store(10, Relaxed);
                return false;
            }
        } else {
            RuntimeGuard::install(&mut *core::ptr::addr_of_mut!(GUARD), guard);
        }
        #[cfg(all(
            feature = "bench-guard-install",
            not(feature = "bench-reentry-guard-stage")
        ))]
        RuntimeGuard::install(&mut *core::ptr::addr_of_mut!(GUARD), guard);
        #[cfg(not(feature = "bench-guard-install"))]
        core::ptr::addr_of_mut!(GUARD).write(Some(guard));
        if !PREPARED {
            configure_timer();
        }
        let t = &*stm32::TIM6::ptr();
        t.cnt().write(|w| w.bits(0));
        t.sr().write(|w| w.bits(0));
        core::ptr::addr_of_mut!(CLOCK).write(sampled_clock::Clock::new(t17()));
        #[cfg(feature = "bench-fast-bus-tail")]
        core::ptr::addr_of_mut!(BUS_TAIL_N).write(0);
        #[cfg(feature = "bench-fast-bus-inject")]
        {
            FAST_BUS_INJECT_SCANS.store(0, Relaxed);
            FAST_BUS_INJECT_OFF_AGE_US.store(0, Relaxed);
            FAST_BUS_INJECT_REAL_BUS.store(0, Relaxed);
        }
        ACTIVE.store(true, Relaxed);
        t.dier().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(1));
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        true
    })
}
/// Prospective next-edge bridge-off transition. No timer or gate writes on
/// success; every refusal safes. Not called by the live recovery path yet.
#[cfg(feature = "bench-reentry-next-edge")]
pub fn follow_seed(prior: u8, next: u8, host_abort: bool) -> bool {
    let mut fault = None;
    let ok = cortex_m::interrupt::free(|_| unsafe {
        if !owns() || reason() != 0 || !outputs_disabled() || COMMITS.load(Relaxed) != 0 {
            return false;
        }
        let Some(g) = (&mut *core::ptr::addr_of_mut!(GUARD)).as_mut() else {
            return false;
        };
        fault = g.follow_seed(now(), prior, next, get_idr(1, 14), host_abort);
        fault.is_none()
    });
    if !ok {
        trip(fault.unwrap_or(powered_guard::Fault::InvalidSeed));
    }
    ok
}
/// Check and apply in ONE critical section: an interrupted COM must not
/// resume after a guard trip and re-enable MOE. Check the policy latch too,
/// since an event/feedback fault can precede trip()'s peripheral cleanup.
/// Caller must account for measured critical-section cost in shutdown margin.
pub fn commit(step: u8, duty: u32) -> bool {
    let start = t17();
    let mut fault = None;
    let ok = cortex_m::interrupt::free(|_| unsafe {
        #[cfg(feature = "bench-live-control")]
        let live = phase_role_live::live_matches(duty) && duty_envelope::contains(duty);
        #[cfg(not(feature = "bench-live-control"))]
        let live = false;
        // Timed startup may initialize its first segment at the same ceiling
        // as live updates. No preload transaction exists before that first COM.
        let initial_max = duty_split::segment_max(cfg!(feature = "bench-startup-adc"));
        if !owns()
            || REASON.load(Relaxed) != 0
            || !(1..=6).contains(&step)
            || duty == 0
            || (!live && duty > initial_max)
        {
            return false;
        }
        let Some(guard) = (&mut *core::ptr::addr_of_mut!(GUARD)).as_mut() else {
            return false;
        };
        // Check deadlines/freshness at the write, not only on the preceding tick.
        fault = guard.poll(now(), get_idr(1, 14), false);
        if fault.is_some() {
            return false;
        }
        #[cfg(not(feature = "bench-pwm-roles"))]
        sixstep_write(step, duty);
        #[cfg(feature = "bench-pwm-roles")]
        phase_role_live::apply(step, duty);
        set_pin(3, 1, true);
        COMMITS.store(COMMITS.load(Relaxed) + 1, Relaxed);
        true
    });
    #[cfg(not(feature = "bench-lean-irq"))]
    {
        COMMIT_MAX_US.store(
            COMMIT_MAX_US
                .load(Relaxed)
                .max(t17().wrapping_sub(start) as u32),
            Relaxed,
        );
        if !ok {
            VETO.store(VETO.load(Relaxed) + 1, Relaxed);
        }
    }
    if let Some(f) = fault {
        trip(f);
    }
    if t17().wrapping_sub(start) > 100 {
        trip(powered_guard::Fault::TickGap);
    }
    ok
}
/// Foreground request, already prepared outside the interrupt mask. Not a COM:
/// no sector/mux/MOE write, commit count, guard reset or deadline extension.
/// Every refusal stops; the first existing electrical/tracking reason wins.
#[cfg(feature = "bench-live-control")]
pub fn update_live_duty(request: live_duty::Prepared) -> bool {
    let mut elapsed = 0;
    let mut fault = None;
    let ok = cortex_m::interrupt::free(|cs| unsafe {
        // Measure this masked transaction, not unrelated ISRs that can run
        // just before entry or immediately after PRIMASK is restored.
        let start = t17();
        if !owns() || REASON.load(Relaxed) != 0 || !get_idr(3, 1) {
            return false;
        }
        let Some(old) = core_bench::live_duty_current() else {
            return false;
        };
        if !phase_role_live::prepared_matches(old) {
            return false;
        }
        let Some(guard) = (&mut *core::ptr::addr_of_mut!(GUARD)).as_mut() else {
            return false;
        };
        fault = guard.poll(now(), get_idr(1, 14), false);
        if fault.is_some() || !live_duty_hw::update(request) {
            return false;
        }
        phase_role_live::publish_live(request.duty(), cs);
        core_bench::publish_live_duty(request.duty(), cs);
        elapsed = t17().wrapping_sub(start) as u32;
        true
    });
    COMMIT_MAX_US.store(COMMIT_MAX_US.load(Relaxed).max(elapsed), Relaxed);
    if let Some(f) = fault {
        trip(f);
    }
    if elapsed > 100 {
        trip(powered_guard::Fault::TickGap);
        return false;
    }
    if !ok {
        VETO.store(VETO.load(Relaxed) + 1, Relaxed);
        abort();
    }
    ok
}
pub fn feedback(sample: powered_guard::Feedback) {
    feedback_inner(sample, None);
}
fn feedback_inner(sample: powered_guard::Feedback, acquired: Option<u32>) -> bool {
    let recorded = owns();
    #[cfg(not(feature = "bench-lean-irq"))]
    if recorded {
        FEEDBACK_N.store(FEEDBACK_N.load(Relaxed) + 1, Relaxed);
        for raw in sample.phase {
            PEAK_RAW.store(
                PEAK_RAW
                    .load(Relaxed)
                    .max((raw as i32 - 2048).unsigned_abs()),
                Relaxed,
            );
        }
        BUS_MIN.store(BUS_MIN.load(Relaxed).min(sample.bus_mv), Relaxed);
    }
    let fault = cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            return None;
        }
        let stamp = now();
        (&mut *core::ptr::addr_of_mut!(GUARD))
            .as_mut()
            .and_then(|g| {
                #[cfg(all(feature = "bench-dma-feedback", not(feature = "bench-lean-irq")))]
                let previous = g.feedback_timestamp();
                let fault = if let Some(acquired) = acquired {
                    g.feedback_aged(stamp, stamp.wrapping_sub(acquired), sample)
                } else {
                    g.feedback(stamp, sample)
                };
                #[cfg(all(feature = "bench-dma-feedback", not(feature = "bench-lean-irq")))]
                if let Some(acquired) = acquired {
                    if FIRST_DELIVERY[0] == 0 {
                        FIRST_DELIVERY = [
                            1,
                            FIRST_DELIVERY[1],
                            stamp,
                            previous,
                            acquired,
                            fault.map(fault_code).unwrap_or(0),
                        ];
                    }
                }
                fault
            })
    });
    if let Some(f) = fault {
        trip(f);
    }
    recorded
}
pub fn service_feedback(vcal: u32) -> bool {
    #[cfg(not(feature = "bench-dma-feedback"))]
    {
        sample_feedback(vcal).is_some()
    }
    #[cfg(feature = "bench-dma-feedback")]
    {
        if !adc_stream::ensure_started() {
            return false;
        }
        // Bounded drain: retain main-loop/host-abort progress even if producer
        // keeps filling. No timestamp refresh and no latest-only shortcut.
        for _ in 0..8 {
            #[cfg(feature = "bench-dma-stall")]
            if unsafe {
                (&mut *core::ptr::addr_of_mut!(FIFO_STALL))
                    .hold((*core::ptr::addr_of!(CURRENT_SUMS)).n)
            } {
                return owns(); // Keep main/reference/host handling and all IRQs live.
            }
            let Some(frame) = adc_stream::take() else {
                break;
            };
            // Match the exact delivery decision, including a guard stop during
            // conversion. Aggregation is foreground-owned and outside IRQ masks.
            #[cfg(not(feature = "bench-dma-guard"))]
            let recorded = feedback_inner(convert(frame.raw, vcal), Some(frame.acquired));
            // DMA already validated this exact frame. Replaying its older
            // timestamp here would roll back the guard's acquisition clock.
            #[cfg(feature = "bench-dma-guard")]
            let recorded = owns();
            if recorded
                && !unsafe {
                    (&mut *core::ptr::addr_of_mut!(CURRENT_SUMS))
                        .push_period::<{ adc_stream::PERIOD_US }>(frame.raw, frame.acquired)
                }
            {
                stream_fault(10);
                return false;
            }
            if !owns() {
                return false;
            }
        }
        owns()
    }
}
#[cfg(feature = "bench-dma-feedback")]
pub fn stream_current(phase: [u16; 3]) -> bool {
    if !powered_guard::phase_valid(phase) {
        trip(powered_guard::Fault::Current);
        false
    } else {
        true
    }
}
/// Complete coherent DMA scan only. Keep immediate current precedence, then
/// the SAME conversion, bus/VREF checks and original acquisition-age policy.
/// Conversion is preemptible; guard publication stays in feedback_inner's
/// short critical section. No cached scan or delivery-time timestamp substitute.
#[cfg(feature = "bench-dma-guard")]
pub fn stream_feedback(raw: [u16; 5], acquired: u32) -> bool {
    #[cfg(feature = "bench-fast-bus-tail")]
    unsafe {
        let ordinal = core::ptr::addr_of!(BUS_TAIL_N).read();
        let duty = core_bench::live_duty_current().unwrap_or(0) as u16;
        core::ptr::addr_of_mut!(BUS_TAIL)
            .cast::<[u16; 8]>()
            .add((ordinal as usize) & 15)
            .write([
                acquired as u16,
                (acquired >> 16) as u16,
                raw[3],
                raw[4],
                raw[0],
                raw[1],
                raw[2],
                duty,
            ]);
        core::ptr::addr_of_mut!(BUS_TAIL_N).write(ordinal.wrapping_add(1));
    }
    #[cfg(feature = "bench-fast-bus-inject")]
    let raw = {
        let scan = FAST_BUS_INJECT_SCANS.fetch_add(1, Relaxed);
        if (9000..9003).contains(&scan) {
            FAST_BUS_INJECT_REAL_BUS.store(raw[3] as u32, Relaxed);
            let mut injected = raw;
            injected[3] -= injected[3] >> 3;
            injected
        } else {
            raw
        }
    };
    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;
    #[cfg(feature = "bench-average-current")]
    let mut average_ok =
        average_current_live::scan_raw([raw[0], raw[1], raw[2]], raw[3], raw[4], vcal);
    #[cfg(feature = "bench-fast-sag-causal")]
    {
        let streak = average_current_live::fast_bus_streak();
        super::fast_sag_causal::scan(acquired, raw, streak);
        if streak != 0 {
            if streak == 3 {
                let stamp = now();
                let pwm = unsafe { &*stm32::TIM1::ptr() };
                super::fast_sag_causal::pwm_stop([
                    stamp,
                    pwm.cnt().read().bits(),
                    pwm.arr().read().bits(),
                    pwm.ccr1().read().bits(),
                    pwm.ccr2().read().bits(),
                    pwm.ccr3().read().bits(),
                    pwm.cr1().read().bits(),
                ]);
            }
            let event = unsafe {
                (&*core::ptr::addr_of!(GUARD))
                    .as_ref()
                    .map_or([0; 5], |g| g.stopped_tracking())
            };
            super::fast_sag_causal::record(
                streak,
                acquired,
                now(),
                raw,
                event,
                core_bench::sag_snapshot(),
                COMMITS.load(Relaxed),
            );
        }
    }
    #[cfg(feature = "bench-current-report-only")]
    if !average_ok && average_current_live::current_only_failure() {
        average_ok = true;
    }
    #[cfg(feature = "bench-driver-fault-probe")]
    let average_ok = {
        // Preserve the full sequential-current/bus diagnostic state, but let
        // the independent raw-feedback validator and DRV nFAULT own shutdown
        // in the one image whose purpose is to classify Driver7.
        let _ = average_ok;
        true
    };
    #[cfg(feature = "bench-average-current")]
    if owns() && !average_ok {
        #[cfg(feature = "bench-phase-peak-stop")]
        if average_current_live::phase_peak_tripped() {
            // First terminal peak only: snapshot *before* trip_reason revokes
            // CCRs and roles. All timing is service-time context; the ADC
            // channels were converted sequentially after `acquired`.
            let service = now();
            let pwm = unsafe { &*stm32::TIM1::ptr() };
            let event = unsafe {
                (&*core::ptr::addr_of!(GUARD))
                    .as_ref()
                    .map_or([0; 5], |g| g.stopped_tracking())
            };
            let core = core_bench::sag_snapshot();
            super::rate_census::peak_timing([
                acquired,
                service,
                pwm.cnt().read().bits(),
                pwm.arr().read().bits(),
                pwm.ccr1().read().bits(),
                pwm.ccr2().read().bits(),
                pwm.ccr3().read().bits(),
                event[1],
                event[2],
                core[0],
                core[1],
                core[2],
                core[3],
                core[4],
                COMMITS.load(Relaxed),
                unsafe { (*stm32::TIM17::ptr()).cnt().read().bits() },
                event[3],
                event[4],
            ]);
            trip_reason(27);
            return false;
        }
        #[cfg(feature = "bench-fast-bus-sag")]
        let reason = if average_current_live::fast_bus_tripped() {
            26
        } else {
            25
        };
        #[cfg(not(feature = "bench-fast-bus-sag"))]
        let reason = 25;
        trip_reason(reason);
        #[cfg(feature = "bench-fast-bus-inject")]
        if reason == 26 {
            // Called after trip_reason has revoked gates and ENABLE. The frame
            // stamp predates DMA service, so this is a conservative upper
            // bound from the third scan's trigger to completed shutdown.
            FAST_BUS_INJECT_OFF_AGE_US.store(now().wrapping_sub(acquired), Relaxed);
        }
        return false;
    }
    if !owns() || !stream_current([raw[0], raw[1], raw[2]]) {
        return false;
    }
    #[cfg(feature = "bench-lean-irq")]
    let sample = match powered_guard::validate_raw_feedback(raw, vcal) {
        Ok(sample) => sample,
        Err(fault) => {
            trip(fault);
            return false;
        }
    };
    #[cfg(not(feature = "bench-lean-irq"))]
    let sample = convert(raw, vcal);
    feedback_inner(sample, Some(acquired));
    owns()
}
#[cfg(feature = "bench-dma-feedback")]
pub fn stream_fault(code: u32) {
    unsafe {
        ADC_FAULT = [
            20 + (code & 255),
            0,
            code >> 16,
            (*stm32::ADC::ptr()).cr().read().bits(),
            (*stm32::ADC::ptr()).isr().read().bits(),
            code >> 8,
        ];
    }
    trip(powered_guard::Fault::AdcTimeout);
}
/// Foreground only, with exclusive ADC ownership. Timed waits keep a wedged
/// conversion from also wedging the shell. The guard timer remains preemptive.
pub fn sample_feedback(vcal: u32) -> Option<powered_guard::Feedback> {
    let mut raw = [0u16; 5];
    let channels = [4, 1, 0, 6, 13]; // measured logical A/B/C, bus, VREFINT
    for i in 0..5 {
        raw[i] = read_channel(channels[i])?;
    }
    let sample = convert(raw, vcal);
    feedback(sample);
    Some(sample)
}
/// Driven acquisition only: finish the current bounded single conversion,
/// then abandon the partial scan if IRQ ownership has been released. Never
/// hand a partial sample to either guard or launch another channel afterward.
#[cfg(feature = "bench-driven-handoff")]
pub fn sample_driven_feedback(vcal: u32) -> Option<powered_guard::Feedback> {
    let mut raw = [0u16; 5];
    let channels = [4, 1, 0, 6, 13];
    for i in 0..5 {
        if !driven_run::owns() {
            driven_run::scan_yield(i as u32);
            return None;
        }
        raw[i] = read_channel(channels[i])?;
    }
    if !driven_run::owns() {
        driven_run::scan_yield(5);
        return None;
    }
    Some(convert(raw, vcal))
}
#[path = "bus_scale.rs"]
mod bus_scale;
pub fn convert(raw: [u16; 5], vcal: u32) -> powered_guard::Feedback {
    let vdda = if raw[4] != 0 {
        3000 * vcal / raw[4] as u32
    } else {
        0
    };
    powered_guard::Feedback {
        phase: [raw[0], raw[1], raw[2]],
        bus_mv: bus_scale::scale(raw[3] as u32 * vdda / 4096),
        vref: raw[4],
    }
}
pub fn read_channel(channel: u8) -> Option<u16> {
    let mut stage = 1;
    let mut launch_us = u32::MAX;
    let begin = t17();
    let value = unsafe {
        let adc = &*stm32::ADC::ptr();
        let start = t17();
        if adc.cr().read().bits() & (1 << 2) != 0 {
            None
        } else {
            stage = 2;
            adc.isr().write(|w| w.bits(1 << 13));
            adc.chselr0().write(|w| w.bits(1 << channel));
            while adc.isr().read().bits() & (1 << 13) == 0 && t17().wrapping_sub(start) < 100 {}
            if adc.isr().read().bits() & (1 << 13) == 0 {
                None
            } else {
                stage = 3;
                launch_us = t17().wrapping_sub(begin) as u32;
                #[cfg(not(feature = "bench-dma-feedback"))]
                {
                    let phase = match channel {
                        4 => Some(0),
                        1 => Some(1),
                        0 => Some(2),
                        _ => None,
                    };
                    let observe = owns()
                        && phase
                            .map(|i| {
                                (*core::ptr::addr_of!(ADC_COVERAGE)).attempted[i]
                                    < adc_occupancy::LIMIT
                            })
                            .unwrap_or(false);
                    if observe {
                        // COM resets TIM1 via EGR even if its counter looks nearby.
                        let generation = COMMITS.load(Relaxed);
                        let launch_start = t17();
                        let before = (*stm32::TIM1::ptr()).cnt().read().bits() as u16;
                        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
                        let after = (*stm32::TIM1::ptr()).cnt().read().bits() as u16;
                        let elapsed = t17().wrapping_sub(launch_start);
                        let end_generation = COMMITS.load(Relaxed);
                        (&mut *core::ptr::addr_of_mut!(ADC_COVERAGE)).push(
                            phase.unwrap(),
                            before,
                            after,
                            elapsed,
                            generation,
                            end_generation,
                        );
                    } else {
                        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
                    }
                }
                #[cfg(feature = "bench-dma-feedback")]
                adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
                // Channel selection and conversion are separate operations.
                // COMP/COM can preempt selection; do not start a conversion
                // then reject it against a deadline that expired pre-launch.
                // Each wait is bounded at 100 us; TIM6 independently enforces
                // the unchanged 1 ms age limit for the complete feedback scan.
                let conversion_start = t17();
                while adc.isr().read().bits() & (1 << 2) == 0
                    && t17().wrapping_sub(conversion_start) < 100
                {}
                if adc.isr().read().bits() & (1 << 2) == 0 {
                    None
                } else {
                    Some(adc.dr().read().bits() as u16)
                }
            }
        }
    };
    let Some(value) = value else {
        unsafe {
            let adc = &*stm32::ADC::ptr();
            ADC_FAULT = [
                stage,
                channel as u32,
                t17().wrapping_sub(begin) as u32,
                adc.cr().read().bits(),
                adc.isr().read().bits(),
                launch_us,
            ];
        }
        trip(powered_guard::Fault::AdcTimeout);
        // Single-conversion mode: after safing and stopping reference IRQs,
        // let any in-flight conversion finish before legacy coast ADC use.
        // Never return to that reader with an outstanding transaction.
        unsafe {
            let adc = &*stm32::ADC::ptr();
            let cleanup = t17();
            while adc.cr().read().bits() & (1 << 2) != 0 {
                if t17().wrapping_sub(cleanup) >= 100 {
                    panic!("ADC cleanup timeout");
                }
            }
            let _ = adc.dr().read().bits();
            adc.isr().write(|w| w.bits((1 << 2) | (1 << 3) | (1 << 4)));
        }
        return None;
    };
    Some(value)
}
pub fn accepted(step: u8, reference_half_us: u32) {
    let fault = cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            return None;
        }
        (&mut *core::ptr::addr_of_mut!(GUARD))
            .as_mut()
            .and_then(|g| {
                let at = now();
                #[cfg(feature = "bench-speed-event-watch")]
                let fault = {
                    let fault = g.accepted_speed_scaled(at, step, reference_half_us);
                    SPEED_EVENT_LIMIT_US.store(g.event_stale_limit(), Relaxed);
                    fault
                };
                #[cfg(not(feature = "bench-speed-event-watch"))]
                let fault = {
                    let _ = reference_half_us;
                    g.accepted(at, step)
                };
                if fault == Some(powered_guard::Fault::CycleTiming) {
                    if let Some(snapshot) = g.refused_cycle(at, step) {
                        CYCLE_FAULT = snapshot;
                    }
                }
                #[cfg(feature = "bench-fast-sag-causal")]
                if fault.is_none() {
                    super::fast_sag_causal::accepted(at);
                }
                fault
            })
    });
    if let Some(f) = fault {
        trip(f);
        if f == powered_guard::Fault::CycleTiming {
            core_bench::cycle_refusal_after_safing();
        }
    }
}
pub fn interrupt() {
    #[cfg(feature = "bench-qualification-sparse")]
    qualification_sparse_live::guard_enter();
    #[cfg(feature = "bench-qualification-window")]
    qualification_live::guard_enter();
    #[cfg(not(feature = "bench-lean-irq"))]
    let before = t17();
    unsafe {
        (*stm32::TIM6::ptr()).sr().write(|w| w.bits(0));
    }
    let fault = cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            return None;
        }
        (&mut *core::ptr::addr_of_mut!(GUARD))
            .as_mut()
            .and_then(|g| g.poll(now(), get_idr(1, 14), false))
    });
    if let Some(f) = fault {
        trip(f);
    }
    #[cfg(not(feature = "bench-lean-irq"))]
    MAX_US.store(
        MAX_US.load(Relaxed).max(t17().wrapping_sub(before) as u32),
        Relaxed,
    );
}
pub fn diagnostic<W: Write>(out: &mut W) {
    if !disabled() {
        let _ = writeln!(out, "POWERGUARD refused bridge_must_be_disabled");
        return;
    }
    #[cfg(feature = "bench-lean-irq")]
    let _ = writeln!(
        out,
        "GUARDPROFILE diagnostics=0 deadline_enforced=1 timing_measured=0"
    );
    let sample = powered_guard::Feedback {
        phase: [2048; 3],
        bus_mv: 11800,
        vref: 1500,
    };
    // Synthetic inputs test the timer/safing path, NOT ADC calibration or
    // physical nFAULT/current trip thresholds. No gate or ENABLE write high.
    for mode in 0..3 {
        let elapsed = if mode == 2 { 4_999_500 } else { 4_700_000 };
        if !start(elapsed, 1, sample) {
            let _ = writeln!(
                out,
                "POWERGUARD start_refused code={}",
                REASON.load(Relaxed)
            );
            break;
        }
        let begin = t17();
        let mut fed = 0;
        while owns() && t17().wrapping_sub(begin) < 3000 {
            if mode == 1 && now().wrapping_sub(fed) >= 100 {
                fed = now();
                feedback(sample);
            }
        }
        let watchdog = owns();
        stop();
        gates_off();
        set_pin(3, 1, false);
        // Exercise the real writer after shutdown, including every sector.
        // A stale reference COM must not revive a stopped session.
        let mut refused = 0;
        for step in 1..=6 {
            if commit(step, 65) || !disabled() {
                panic!("post-stop commit");
            }
            refused += 1;
        }
        let _ = writeln!(
            out,
            "POWERGUARD mode={} reason={} stop_us={} isr_max_us={} host_backstop={} disabled={} synthetic_feedback=1 gate_authority=0",
            mode,
            REASON.load(Relaxed),
            STOP_US.load(Relaxed),
            MAX_US.load(Relaxed),
            watchdog as u8,
            disabled() as u8
        );
        let _ = writeln!(
            out,
            "POSTSTOP refused={} expected=6 disabled={}",
            refused,
            disabled() as u8
        );
    }
}
