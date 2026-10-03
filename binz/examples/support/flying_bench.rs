//! Gate-disabled measured seed probe. Default also disables ENABLE; explicit
//! powered-handoff acquisition keeps the previously awakened driver enabled.
//! Neither path has commutation authority.
use super::*;
#[path = "feedback_age.rs"]
mod feedback_age;
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
static SEED_SECTOR: AtomicU32 = AtomicU32::new(0);
static mut SELECTION: [u32; 2] = [0; 2];
static mut FAILURE: [u16; 13] = [0; 13];
// Bit16 marks a qualified edge; low16 is its onset tick. Separate from the
// legacy13-word F85 frame, whose candidate is cleared by confirmation.
static mut FAILURE_EDGE: u32 = 0;
pub fn select_sector(sector: u32) -> bool {
    if sector > 6 {
        return false;
    }
    SEED_SECTOR.store(sector, Relaxed);
    true
}
static ARMED: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
static TRACK: AtomicBool = AtomicBool::new(false);
static ADC: AtomicBool = AtomicBool::new(false);
static mut BASELINE: ([u16; 5], [u16; 5], u8) = ([0; 5], [0; 5], 0);
#[cfg(feature = "bench-range300")]
static mut BASELINE_BUS_MV: u32 = 0;
pub fn acquire_feedback() -> Option<(flying_acquire::Seed, u16)> {
    ADC.store(true, Relaxed);
    acquire()
}
pub fn acquire_awake_feedback() -> Option<(flying_acquire::Seed, u16)> {
    ADC.store(true, Relaxed);
    acquire_inner(true, false, 0)
}
fn safe_to_sense(awake: bool) -> bool {
    if awake {
        powered_timer::ready() && powered_timer::outputs_disabled()
    } else {
        disabled()
    }
}
pub fn baseline(vcal: u32) -> Option<(powered_guard::Feedback, u32)> {
    let (raw, times, valid) = unsafe { BASELINE };
    if valid != 31 {
        return None;
    }
    // One common observation AFTER the snapshot, not five volatile clock reads.
    // Original timestamps and the conservative 200us setup allowance remain.
    let age = feedback_age::age(times, t17())?;
    #[cfg(feature = "bench-range300")]
    {
        // Cache is updated with every raw update after all channels exist.
        // Original per-channel timestamps above remain the age authority.
        if vcal != unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32 {
            return None;
        }
        Some((
            powered_guard::Feedback {
                phase: [raw[0], raw[1], raw[2]],
                vref: raw[4],
                bus_mv: unsafe { BASELINE_BUS_MV },
            },
            age,
        ))
    }
    #[cfg(not(feature = "bench-range300"))]
    {
        Some((powered_timer::convert(raw, vcal), age))
    }
}
static mut REPORT: [u32; 16] = [0; 16];
static mut RECOVERY_REPORT: [u32; 16] = [0; 16];
static mut RECOVERY_FAILURE: [u16; 13] = [0; 13];
static mut RECOVERY_EDGE: u32 = 0;
static RECOVERY_DONE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-reentry-next-edge-live")]
struct FollowContext {
    next: flying_acquire::NextEdge<
        { flying_acquire::CYCLE_MIN_TICKS },
        { flying_acquire::INDIVIDUAL_MIN_TICKS },
    >,
    filters: [flying_acquire::EdgeFilter; 3],
    origin: u16,
    saved: u32,
    completed: Option<flying_acquire::Seed>,
    phase: usize,
}
#[cfg(feature = "bench-reentry-next-edge-live")]
static mut FOLLOW: Option<FollowContext> = None;
#[cfg(feature = "bench-reentry-next-edge-live")]
static mut FOLLOW_REPORT: [u32; 8] = [0; 8];
#[cfg(feature = "bench-reentry-next-edge-live")]
static FOLLOW_VALID: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-reentry-next-edge-live")]
static FOLLOW_STAGE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reentry-next-edge-live")]
static mut FOLLOW_READS: u32 = 0;
#[cfg(feature = "bench-reentry-next-edge-live")]
static mut FOLLOW_POLICY_TICKS: u32 = 0;
#[cfg(feature = "bench-reentry-next-edge-live")]
static mut FOLLOW_POLICY_DMA: u32 = 0;
// ISR shutdown revokes authority only. It MUST NOT mutate the foreground-owned
// context while setup_follow borrows its filters. Foreground alone replaces it.
#[cfg(feature = "bench-reentry-next-edge-live")]
pub fn cancel_follow() {
    FOLLOW_VALID.store(false, Relaxed);
}
#[cfg(feature = "bench-reentry-next-edge-live")]
pub fn setup_follow(stage: u32) -> bool {
    FOLLOW_STAGE.store(stage, Relaxed);
    unsafe {
        FOLLOW_READS = 0;
    }
    if !FOLLOW_VALID.load(Relaxed) || !powered_timer::ready() || !powered_timer::outputs_disabled()
    {
        let _ = follow_failure(4);
        return false;
    }
    let context = unsafe { (&mut *core::ptr::addr_of_mut!(FOLLOW)).as_mut() };
    let Some(c) = context else {
        let _ = follow_failure(2);
        return false;
    };
    // At the final prepared checkpoint, the qualified12-interval sequence has
    // already selected the sole next phase. Keep its continuous filter; other
    // phases will not be read by the following expected-only wait either.
    let final_only = cfg!(feature = "bench-follow-setup-phase")
        || (stage == 4
            && cfg!(feature = "bench-prepared-handoff")
            && cfg!(feature = "bench-follow-expected-phase"));
    let mut result = follow_sweep(c, final_only);
    // Bridge both sides of final setup: retain the original first successor
    // visit, then sample it again after the other two phases. Reordering alone
    // merely moves the gap upstream. No filter reset or timestamp refresh.
    if stage == 4 && !final_only && result.is_ok() {
        result = follow_sweep(c, true);
    }
    if let Err(code) = result {
        let _ = follow_failure(code);
        return false;
    }
    if !FOLLOW_VALID.load(Relaxed) || !powered_timer::ready() || !powered_timer::outputs_disabled()
    {
        let _ = follow_failure(4);
        return false;
    }
    true
}
/// Consume the exact retained acquisition/filters. ADC now belongs to DMA;
/// this loop only samples COMP and drains coherently timestamped feedback.
#[cfg(feature = "bench-reentry-next-edge-live")]
pub fn follow_prepared(
    prior: flying_acquire::Seed,
    origin: u16,
    vcal: u32,
    abort: &mut dyn FnMut() -> bool,
) -> Option<flying_acquire::Seed> {
    FOLLOW_STAGE.store(5, Relaxed);
    unsafe {
        FOLLOW_READS = 0;
    }
    let context = cortex_m::interrupt::free(|_| unsafe {
        if !FOLLOW_VALID.swap(false, Relaxed) {
            return None;
        }
        // Consume authority, not storage: copying the full acquisition/filter
        // context here ages the first sample. ISR cancellation only changes
        // FOLLOW_VALID; foreground exclusively owns this retained allocation.
        (&mut *core::ptr::addr_of_mut!(FOLLOW)).as_mut()
    });
    let Some(c) = context else {
        return follow_failure(2);
    };
    if c.origin != origin || c.next.prior() != prior {
        return follow_failure(2);
    }
    loop {
        let _ = clock_us();
        if abort() {
            return follow_failure(3);
        }
        if !powered_timer::owns() || !powered_timer::ready() || !powered_timer::outputs_disabled() {
            return follow_failure(4);
        }
        let seed = match follow_sweep(c, cfg!(feature = "bench-follow-expected-phase")) {
            Ok(seed) => seed,
            Err(code) => return follow_failure(code),
        };
        // DMAguard already publishes coherent safety feedback. Once an edge
        // is ready, leave statistics queued for the resumed foreground loop;
        // do not spend commutation slack draining them here. Waiting iterations
        // and non-DMAguard paths still service normally; no sample is dropped.
        let defer_stats = cfg!(feature = "bench-dma-guard") && seed.is_some();
        if (!defer_stats && !powered_timer::service_feedback(vcal))
            || !powered_timer::owns()
            || !powered_timer::outputs_disabled()
        {
            return follow_failure(4);
        }
        if let Some(seed) = seed {
            unsafe {
                core::ptr::write_volatile(COMP2_CSR, c.saved);
            }
            return Some(seed);
        }
    }
}
/// One bounded foreground sweep. A qualified edge is retained once, not
/// discarded if setup is unfinished and never relabelled with setup time.
#[cfg(feature = "bench-reentry-next-edge-live")]
fn follow_sweep(
    c: &mut FollowContext,
    expected_only: bool,
) -> Result<Option<flying_acquire::Seed>, u32> {
    let _ = clock_us();
    if let Some(seed) = c.completed {
        return Ok(Some(seed));
    }
    let origin = c.origin;
    // c.phase is the successor phase, with its original continuous filter.
    // All three remain serviced through setup. Only the final wait may
    // dedicate reads to the one expected edge, as the controller does.
    for offset in 0..if expected_only { 1 } else { 3 } {
        // Before the direct final wait, finish the all-phase checkpoint on
        // the phase that remains selected next. Preserve every sample and
        // its original gap checks; do not reset any filter timestamps.
        let offset = if cfg!(feature = "bench-follow-direct")
            && !expected_only
            && FOLLOW_STAGE.load(Relaxed) == 3
        {
            if offset == 2 { 0 } else { offset + 1 }
        } else {
            offset
        };
        let index = c.phase + offset;
        let phase = if index >= 3 { index - 3 } else { index };
        let tick = t17().wrapping_sub(origin) as u32 * 2;
        if c.next.poll_filtered(tick, &c.filters).is_err() {
            return Err(7);
        }
        unsafe {
            core::ptr::write_volatile(
                COMP2_CSR,
                (c.saved & !(15 << 4 | 3 << 8 | 1 << 15))
                    | ((6 + crate::phase_direction::physical_phase(phase as u8) as u32) << 4)
                    | (2 << 8),
            );
        }
        let switched = t17();
        while t17().wrapping_sub(switched) < 10 {}
        // One short observation: an IRQ must not separate the sampled
        // level from its timestamp/counter. Qualification stays unmasked.
        let (level, mut now, mut dma_before) = cortex_m::interrupt::free(|_| {
            let level = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0 };
            (
                level,
                t17().wrapping_sub(origin) as u32 * 2,
                adc_stream::completed_scans(),
            )
        });
        unsafe {
            FOLLOW_READS += 1;
        }
        #[cfg(feature = "bench-follow-persistence")]
        let sampled = c.filters[phase].sample_candidate(level, now);
        #[cfg(not(feature = "bench-follow-persistence"))]
        let sampled = c.filters[phase].sample(level, now);
        let mut edge = match sampled {
            Ok(e) => e,
            Err(()) => {
                unsafe {
                    FOLLOW_REPORT[7] = c.filters[phase].max_gap;
                }
                return Err(5);
            }
        };
        #[cfg(feature = "bench-follow-prevalidate")]
        let mut validated = None;
        if edge.is_none() && c.next.final_candidate(phase, &c.filters[phase]) {
            // Spend the existing physical dwell on policy work. The
            // exclusive transaction rolls back if this candidate cancels.
            #[cfg(all(
                feature = "bench-follow-prevalidate",
                not(feature = "bench-follow-persistence")
            ))]
            let pending = c.next.prepare_candidate(phase, &c.filters[phase]);
            #[cfg(feature = "bench-follow-persistence")]
            {
                let proof = flying_acquire::persistent_level(level, || unsafe {
                    core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0
                });
                now = t17().wrapping_sub(origin) as u32 * 2;
                if let Some(proof) = proof {
                    edge = match c.filters[phase].confirm_persistent(&proof, now) {
                        Ok(e) => e,
                        Err(()) => return Err(5),
                    };
                    if let Some((value, onset)) = edge {
                        validated =
                            Some(
                                c.next
                                    .edge_persistent(phase as u8, value, onset, now, proof),
                            );
                    }
                } // Rejected pass has not mutated acquisition state.
            }
            #[cfg(not(feature = "bench-follow-persistence"))]
            {
                while (t17().wrapping_sub(origin) as u32 * 2).wrapping_sub(now) < 40 {}
                let observation = cortex_m::interrupt::free(|_| {
                    let level = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0 };
                    (
                        level,
                        t17().wrapping_sub(origin) as u32 * 2,
                        adc_stream::completed_scans(),
                    )
                });
                let level = observation.0;
                now = observation.1;
                dma_before = observation.2;
                unsafe {
                    FOLLOW_READS += 1;
                }
                edge = match c.filters[phase].sample(level, now) {
                    Ok(e) => e,
                    Err(()) => {
                        unsafe {
                            FOLLOW_REPORT[7] = c.filters[phase].max_gap;
                        }
                        return Err(5);
                    }
                };
                #[cfg(feature = "bench-follow-prevalidate")]
                if let (Some(pending), Some((value, onset))) = (pending, edge) {
                    validated = Some(pending.finish(phase as u8, value, onset, now));
                }
            }
        }
        if let Some((level, onset)) = edge {
            #[cfg(feature = "bench-follow-prevalidate")]
            let result = validated.unwrap_or_else(|| c.next.edge(phase as u8, level, onset, now));
            #[cfg(feature = "bench-follow-prevalidate")]
            let Ok(result) = result else {
                return Err(6);
            };
            #[cfg(not(feature = "bench-follow-prevalidate"))]
            let Ok(result) = c.next.edge(phase as u8, level, onset, now) else {
                return Err(6);
            };
            let (finished, dma_after) = cortex_m::interrupt::free(|_| {
                (
                    t17().wrapping_sub(origin) as u32 * 2,
                    adc_stream::completed_scans(),
                )
            });
            unsafe {
                FOLLOW_POLICY_TICKS = finished.wrapping_sub(now);
                FOLLOW_POLICY_DMA = dma_after.wrapping_sub(dma_before);
                FOLLOW_REPORT[0] = 1;
                FOLLOW_REPORT[3] = result.seed.step as u32;
                FOLLOW_REPORT[4] = onset;
                FOLLOW_REPORT[5] = now;
                FOLLOW_REPORT[6] = result.follow_interval_ticks;
            }
            c.completed = Some(result.seed);
            return Ok(Some(result.seed));
        }
    }
    Ok(None)
}
#[cfg(feature = "bench-reentry-next-edge-live")]
fn follow_failure(code: u32) -> Option<flying_acquire::Seed> {
    unsafe {
        FOLLOW_REPORT[0] = code;
    }
    gates_off();
    set_pin(3, 1, false);
    None
}
/// Bounded continuation of the original scan, before promotion/cleanup.
/// Stop on the first real qualified edge; the caller must feed it to NextEdge.
#[cfg(feature = "bench-reentry-next-edge-live")]
fn continue_filters(
    filters: &mut [flying_acquire::EdgeFilter; 3],
    origin: u16,
    saved: u32,
    first: usize,
) -> Result<Option<(u8, bool, u32, u32, Option<flying_acquire::PersistentLevel>)>, u32> {
    for offset in 0..3 {
        if !powered_timer::ready() || !powered_timer::outputs_disabled() {
            return Err(4);
        }
        let index = first + offset;
        let phase = if index >= 3 { index - 3 } else { index };
        unsafe {
            core::ptr::write_volatile(
                COMP2_CSR,
                (saved & !(15 << 4 | 3 << 8 | 1 << 15))
                    | ((6 + crate::phase_direction::physical_phase(phase as u8) as u32) << 4)
                    | (2 << 8),
            );
        }
        let switched = t17();
        while t17().wrapping_sub(switched) < 10 {}
        let level = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0 };
        let mut now = t17().wrapping_sub(origin) as u32 * 2;
        unsafe {
            FOLLOW_READS += 1;
        }
        let mut proof = None;
        #[cfg(feature = "bench-follow-persistence")]
        let sampled = {
            if filters[phase].sample_candidate(level, now).is_err() {
                unsafe {
                    FOLLOW_REPORT[7] = filters[phase].max_gap;
                }
                return Err(5);
            }
            proof = flying_acquire::persistent_level(level, || unsafe {
                core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0
            });
            now = t17().wrapping_sub(origin) as u32 * 2;
            if let Some(ref token) = proof {
                filters[phase].confirm_persistent(token, now)
            } else {
                Ok(None)
            }
        };
        #[cfg(not(feature = "bench-follow-persistence"))]
        let sampled = filters[phase].sample(level, now);
        match sampled {
            Ok(Some((value, onset))) => return Ok(Some((phase as u8, value, onset, now, proof))),
            Ok(None) => {}
            Err(()) => {
                unsafe {
                    FOLLOW_REPORT[7] = filters[phase].max_gap;
                }
                return Err(5);
            }
        }
    }
    Ok(None)
}
// Recovery-only fixture evidence. These timestamps grant no authority and
// never replace the seed or feedback acquisition times.
#[cfg(feature = "bench-acquire-timing")]
static mut RECOVERY_LATENCY: [u32; 5] = [u32::MAX; 5];
pub fn recovery_reset() {
    RECOVERY_DONE.store(false, Relaxed);
    #[cfg(feature = "bench-reentry-next-edge-live")]
    {
        unsafe {
            FOLLOW_REPORT = [0; 8];
            FOLLOW_READS = 0;
            FOLLOW_POLICY_TICKS = 0;
            FOLLOW_POLICY_DMA = 0;
        }
        FOLLOW_STAGE.store(0, Relaxed);
        cancel_follow();
    }
}
/// Diagnostic only: preserves initial acquisition evidence and never grants
/// power. A returned measured seed is deliberately discarded at this stage.
pub fn reacquire_disabled() {
    let _ = reacquire(false, 0);
}
pub fn reacquire_awake() -> Option<(flying_acquire::Seed, u16)> {
    reacquire(true, 0)
}
#[cfg(feature = "bench-reentry-pwm-stage")]
pub fn reacquire_prepared(duty: u32) -> Option<(flying_acquire::Seed, u16)> {
    reacquire(true, duty)
}
#[inline(never)]
fn reacquire(awake: bool, duty: u32) -> Option<(flying_acquire::Seed, u16)> {
    #[cfg(feature = "bench-acquire-timing")]
    unsafe {
        RECOVERY_LATENCY = [u32::MAX; 5];
    }
    #[cfg(feature = "bench-range300")]
    let old_bus = unsafe { BASELINE_BUS_MV };
    let old = unsafe { (SELECTION, BASELINE) };
    let pending = PENDING.load(Relaxed);
    let sector = SEED_SECTOR.swap(0, Relaxed);
    TRACK.store(false, Relaxed);
    ADC.store(true, Relaxed);
    // Publish directly into the separate recovery archive. Copying both reports
    // out and restoring the originals here spends the fresh seed's arm margin.
    let result = acquire_inner(awake, true, duty);
    unsafe {
        SELECTION = old.0;
        if !awake {
            BASELINE = old.1;
        }
        #[cfg(feature = "bench-range300")]
        if !awake {
            BASELINE_BUS_MV = old_bus;
        }
    }
    SEED_SECTOR.store(sector, Relaxed);
    PENDING.store(pending, Relaxed);
    RECOVERY_DONE.store(true, Relaxed);
    #[cfg(feature = "bench-acquire-timing")]
    if awake {
        if let Some((_, origin)) = result {
            unsafe {
                RECOVERY_LATENCY[4] = t17().wrapping_sub(origin) as u32 * 2;
            }
        }
    }
    result
}
pub fn recovery_summary<W: Write>(out: &mut W, capture: bool) {
    if !RECOVERY_DONE.swap(false, Relaxed) {
        return;
    }
    #[cfg(feature = "bench-reentry-pwm-stage")]
    let _ = writeln!(
        out,
        "RECOVERPWMSTAGE enabled=1 before_sensing=1 physical_revalidate=1"
    );
    #[cfg(feature = "bench-reentry-guard-stage")]
    let _ = writeln!(out, "RECOVERGUARDSTAGE enabled=1 fresh_admission=1");
    #[cfg(feature = "bench-reentry-next-edge-live")]
    {
        let r = unsafe { FOLLOW_REPORT };
        let _ = writeln!(
            out,
            "FOLLOWREADS count={} policy_ticks={} dma={}",
            unsafe { FOLLOW_READS },
            unsafe { FOLLOW_POLICY_TICKS },
            unsafe { FOLLOW_POLICY_DMA }
        );
        #[cfg(feature = "bench-follow-expected-phase")]
        let _ = writeln!(out, "FOLLOWMODE expected_phase_only=1");
        let _ = writeln!(out, "FOLLOWSTAGE stage={}", FOLLOW_STAGE.load(Relaxed));
        let _ = writeln!(
            out,
            "FOLLOWEDGE result={} prior_step={} prior_tick={} step={} onset={} confirmed={} interval={} max_gap={} half_us=1",
            r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]
        );
    }
    let r = unsafe { RECOVERY_REPORT };
    let _ = writeln!(
        out,
        "RECOVERYACQ result={} step={} interval_ticks={} elapsed_us={} intervals={} max_gap_ticks={} disabled={} gate_authority=0",
        r[0], r[1], r[3], r[4], r[8], r[6], r[9]
    );
    if capture {
        cycle_summary(out, "RECOVERYCYCLE", &r);
    }
    #[cfg(feature = "bench-acquire-timing")]
    if capture && r[0] == 1 && unsafe { RECOVERY_LATENCY[0] != u32::MAX } {
        let a = unsafe { RECOVERY_LATENCY };
        let _ = writeln!(
            out,
            "ACQUIRELAT edge_ticks={} qualified_ticks={} cleared_ticks={} published_ticks={} returned_ticks={} half_us=1 recovery_only=1",
            a[0], a[1], a[2], a[3], a[4]
        );
    }
    if capture && ((2..=6).contains(&r[0]) || (14..=15).contains(&r[0])) {
        let f = unsafe { RECOVERY_FAILURE };
        let _ = snapshot::record(out, "RF85", &f);
        let edge = unsafe { RECOVERY_EDGE };
        if edge & 0x10000 != 0 {
            let _ = writeln!(out, "RECOVERYEDGE qualified_tick={}", edge & 65535);
        }
    }
}
pub fn arm(on: bool) {
    ARMED.store(on, Relaxed);
    TRACK.store(false, Relaxed);
}
pub fn arm_track(on: bool) {
    arm(on);
    TRACK.store(on, Relaxed);
}
pub fn take() -> bool {
    ARMED.swap(false, Relaxed)
}
/// Deferred diagnostics: 9=abnormal drive exit, 10=conflicting coast probes.
pub fn refuse(reason: u32) {
    unsafe {
        FAILURE_EDGE = 0;
    }
    unsafe {
        FAILURE = [0; 13];
    }
    unsafe {
        SELECTION = [SEED_SECTOR.load(Relaxed), 0];
    }
    TRACK.store(false, Relaxed);
    let mut report = [0; 16];
    report[0] = reason;
    report[9] = disabled() as u32;
    unsafe {
        REPORT = report;
    }
    PENDING.store(true, Relaxed);
}
fn disabled() -> bool {
    !get_idr(3, 1)
        && unsafe { (*stm32::TIM1::ptr()).bdtr().read().bits() & (1 << 15) == 0 }
        && [(0, 10), (0, 9), (0, 8), (1, 1), (1, 0), (0, 7)]
            .iter()
            .all(|&(p, b)| !get_idr(p, b))
}
pub fn run() {
    let _ = acquire();
}
pub fn acquire() -> Option<(flying_acquire::Seed, u16)> {
    acquire_inner(false, false, 0)
}
fn publish_report<const RECOVERY: bool>(report: [u32; 16]) {
    unsafe {
        if RECOVERY {
            RECOVERY_REPORT = report;
        } else {
            REPORT = report;
        }
    }
    if !RECOVERY {
        PENDING.store(true, Relaxed);
    }
}
fn publish_failure<const RECOVERY: bool>(failure: [u16; 13], edge: u32) {
    unsafe {
        if RECOVERY {
            RECOVERY_FAILURE = failure;
            RECOVERY_EDGE = edge;
        } else {
            FAILURE = failure;
            FAILURE_EDGE = edge;
        }
    }
}
/// Idle diagnostic: exercises the actual sinks and awake-refusal path without
/// waking the driver. Restore all diagnostic state before returning.
#[cfg(feature = "bench-reentry-staging")]
pub fn archive_check<W: Write>(out: &mut W) {
    if !disabled() || powered_timer::ready() {
        let _ = writeln!(out, "!archivecheck disabled_only");
        return;
    }
    let saved = unsafe {
        (
            REPORT,
            FAILURE,
            FAILURE_EDGE,
            RECOVERY_REPORT,
            RECOVERY_FAILURE,
            RECOVERY_EDGE,
            SELECTION,
            BASELINE,
        )
    };
    #[cfg(feature = "bench-range300")]
    let bus = unsafe { BASELINE_BUS_MV };
    let flags = (
        PENDING.load(Relaxed),
        RECOVERY_DONE.load(Relaxed),
        TRACK.load(Relaxed),
        ADC.load(Relaxed),
        SEED_SECTOR.load(Relaxed),
    );
    let mut passed = 0;
    publish_report::<false>([11; 16]);
    publish_failure::<false>([12; 13], 13);
    PENDING.store(false, Relaxed);
    publish_report::<true>([21; 16]);
    publish_failure::<true>([22; 13], 23);
    if unsafe {
        REPORT == [11; 16]
            && FAILURE == [12; 13]
            && FAILURE_EDGE == 13
            && RECOVERY_REPORT == [21; 16]
            && RECOVERY_FAILURE == [22; 13]
            && RECOVERY_EDGE == 23
    } && !PENDING.load(Relaxed)
    {
        passed += 1;
    }
    publish_report::<false>([31; 16]);
    publish_failure::<false>([32; 13], 33);
    if unsafe { RECOVERY_REPORT == [21; 16] && RECOVERY_FAILURE == [22; 13] && RECOVERY_EDGE == 23 }
        && PENDING.load(Relaxed)
    {
        passed += 1;
    }
    SEED_SECTOR.store(4, Relaxed);
    unsafe {
        SELECTION = [4, 7];
    }
    let refused = reacquire(true, 0).is_none();
    if refused
        && unsafe {
            REPORT == [31; 16]
                && FAILURE == [32; 13]
                && FAILURE_EDGE == 33
                && RECOVERY_REPORT[0] == 8
                && RECOVERY_FAILURE == [0; 13]
                && RECOVERY_EDGE == 0
                && SELECTION == [4, 7]
        }
        && PENDING.load(Relaxed)
        && RECOVERY_DONE.load(Relaxed)
        && SEED_SECTOR.load(Relaxed) == 4
        && disabled()
    {
        passed += 1;
    }
    unsafe {
        (
            REPORT,
            FAILURE,
            FAILURE_EDGE,
            RECOVERY_REPORT,
            RECOVERY_FAILURE,
            RECOVERY_EDGE,
            SELECTION,
            BASELINE,
        ) = saved;
    }
    #[cfg(feature = "bench-range300")]
    unsafe {
        BASELINE_BUS_MV = bus;
    }
    PENDING.store(flags.0, Relaxed);
    RECOVERY_DONE.store(flags.1, Relaxed);
    TRACK.store(flags.2, Relaxed);
    ADC.store(flags.3, Relaxed);
    SEED_SECTOR.store(flags.4, Relaxed);
    let _ = writeln!(
        out,
        "ARCHIVECHECK passed={} total=3 gate_authority=0 disabled={}",
        passed,
        disabled() as u8
    );
}
// Keep the acquisition loop out of the diagnostic wrapper's larger frame.
// Its sampling-gap budget must be qualified independently of archive setup.
#[inline(never)]
fn acquire_inner(awake: bool, recovery: bool, duty: u32) -> Option<(flying_acquire::Seed, u16)> {
    if recovery {
        publish_failure::<true>([0; 13], 0);
    } else {
        publish_failure::<false>([0; 13], 0);
    }
    unsafe {
        SELECTION = [SEED_SECTOR.load(Relaxed), 0];
    }
    let adc = ADC.swap(false, Relaxed);
    let mut adc_index = 0;
    unsafe {
        BASELINE = ([0; 5], [0; 5], 0);
    }
    #[cfg(feature = "bench-range300")]
    unsafe {
        BASELINE_BUS_MV = 0;
    }
    let track = TRACK.swap(false, Relaxed);
    let mut report = [0; 16];
    if !safe_to_sense(awake) {
        gates_off();
        set_pin(3, 1, false);
        report[0] = 8;
        publish_acquisition_report(report, recovery);
        return None;
    }
    gates_off();
    if !awake {
        set_pin(3, 1, false);
    }
    comp_input::stop();
    #[cfg(feature = "bench-reentry-staging")]
    if awake && powered_timer::reason() == 8 && !powered_timer::stage_reentry_statistics() {
        gates_off();
        set_pin(3, 1, false);
        report[0] = 8;
        publish_acquisition_report(report, recovery);
        return None;
    }
    let saved = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    #[cfg(feature = "bench-reentry-carrier")]
    if awake && recovery {
        phase_role_live::prepare_carrier();
    }
    #[cfg(feature = "bench-reentry-pwm-stage")]
    if duty != 0 && (!awake || !recovery || !phase_role_live::stage_recovery(duty)) {
        gates_off();
        set_pin(3, 1, false);
        report[0] = 8;
        publish_acquisition_report(report, recovery);
        return None;
    }
    let start = t17();
    let desired = SEED_SECTOR.load(Relaxed);
    let mut acquire = flying_acquire::RuntimeAcquire::with_sector(0, desired as u8).unwrap();
    let mut filters: [flying_acquire::EdgeFilter; 3] =
        core::array::from_fn(|_| flying_acquire::EdgeFilter::new());
    #[cfg(feature = "bench-reentry-next-edge-live")]
    let mut pending_follow = None;
    // Foreground owns this storage throughout sensing and finalization. ISR
    // shutdown only revokes FOLLOW_VALID, never moves/clears a borrowed context.
    // All history/filter construction and copying now precedes the first sample.
    #[cfg(feature = "bench-reentry-next-edge-live")]
    let (acquire, filters) = if recovery && awake && duty != 0 {
        unsafe {
            FOLLOW = Some(FollowContext {
                next: flying_acquire::NextEdge::unqualified(
                    flying_acquire::RuntimeAcquire::with_sector(0, desired as u8).unwrap(),
                ),
                filters: core::array::from_fn(|_| flying_acquire::EdgeFilter::new()),
                origin: start,
                saved,
                completed: None,
                phase: 0,
            });
            let c = (&mut *core::ptr::addr_of_mut!(FOLLOW)).as_mut().unwrap();
            (c.next.acquiring_mut().unwrap(), &mut c.filters)
        }
    } else {
        (&mut acquire, &mut filters)
    };
    #[cfg(feature = "bench-final-edge-prepare")]
    let mut cold_prepared = false;
    'scan: loop {
        for phase in 0..3 {
            if !safe_to_sense(awake) || !get_idr(1, 14) {
                report[0] = 8;
                break 'scan;
            }
            if t17().wrapping_sub(start) >= 20_000 {
                report[0] = 6;
                break 'scan;
            }
            unsafe {
                core::ptr::write_volatile(
                    COMP2_CSR,
                    (saved & !(15 << 4 | 3 << 8 | 1 << 15))
                        | ((6 + crate::phase_direction::physical_phase(phase as u8) as u32) << 4)
                        | (2 << 8),
                );
            }
            let switched = t17();
            while t17().wrapping_sub(switched) < 10 {} // experimental settle budget
            let level = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0 };
            let mut tick = t17().wrapping_sub(start) as u32 * 2;
            report[5] += 1;
            let mut edge = match filters[phase].sample(level, tick) {
                Ok(e) => e,
                Err(()) => {
                    report[0] = 7;
                    break 'scan;
                }
            };
            // A provisional seed does not need the legacy immediate-arm
            // shortcut. Keep round-robin visits running while its last edge
            // confirms; parking on this phase ages the two other filters.
            // EdgeFilter still enforces exactly the same minimum40tick dwell.
            if edge.is_none()
                && !(cfg!(feature = "bench-reentry-next-edge-live")
                    && recovery
                    && awake
                    && duty != 0)
                && acquire.final_candidate(phase, &filters[phase])
            {
                // Keep the already-settled mux selected. One bounded extra
                // visit meets the SAME20us dwell without waiting a full scan.
                // Dwell is measured from the comparator sample, not from
                // completion of filter/hint bookkeeping. EdgeFilter still
                // independently checks40ticks before qualifying the edge.
                while (t17().wrapping_sub(start) as u32 * 2).wrapping_sub(tick) < 40 {}
                if !safe_to_sense(awake) || !get_idr(1, 14) {
                    report[0] = 8;
                    break 'scan;
                }
                let level = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0 };
                tick = t17().wrapping_sub(start) as u32 * 2;
                report[5] += 1;
                report[15] += 1;
                edge = match filters[phase].sample(level, tick) {
                    Ok(e) => e,
                    Err(()) => {
                        report[0] = 7;
                        break 'scan;
                    }
                };
            }
            let result = if let Some((value, first)) = edge {
                acquire.edge(phase as u8, value, first)
            } else {
                acquire.poll_filtered(tick, &filters)
            };
            match result {
                Ok(Some(seed)) => {
                    report[0] = 1;
                    report[1] = seed.step as u32;
                    report[2] = seed.edge_tick;
                    report[3] = seed.interval_ticks;
                    snapshot_acquisition(&mut report, &acquire, &filters, start, desired);
                    #[cfg(feature = "bench-reentry-next-edge-live")]
                    if recovery && awake && duty != 0 {
                        FOLLOW_STAGE.store(6, Relaxed);
                        unsafe {
                            FOLLOW_READS = 0;
                        }
                        unsafe {
                            FOLLOW_REPORT = [0, report[1], report[2], 0, 0, 0, 0, 0];
                        }
                        let first = if phase == 2 { 0 } else { phase + 1 };
                        pending_follow = match continue_filters(filters, start, saved, first) {
                            Ok(edge) => edge,
                            Err(code) => {
                                let _ = follow_failure(code);
                                report[9] = disabled() as u32;
                                publish_acquisition_report(report, recovery);
                                return None;
                            }
                        };
                    }
                    break 'scan;
                }
                Ok(None) =>
                {
                    #[cfg(feature = "bench-final-edge-prepare")]
                    if awake && recovery && !cold_prepared {
                        if let Some(step) = acquire.preparation_step() {
                            if !core_bench::prepare_final_step(step) {
                                report[0] = 8;
                                break 'scan;
                            }
                            cold_prepared = true;
                        }
                    }
                }
                Err(f) => {
                    let (last_step, last_tick) = acquire.last_edge().unwrap_or((0, 0));
                    let mut failure = [0u16; 13];
                    failure[..4].copy_from_slice(&[
                        last_step as u16,
                        last_tick as u16,
                        tick as u16,
                        phase as u16,
                    ]);
                    for i in 0..3 {
                        failure[4 + i * 3..7 + i * 3].copy_from_slice(&filters[i].diagnostic());
                    }
                    let edge = edge.map_or(0, |(_, first)| 0x10000 | first);
                    if recovery {
                        publish_failure::<true>(failure, edge);
                    } else {
                        publish_failure::<false>(failure, edge);
                    }
                    report[0] = match f {
                        flying_acquire::Fault::InvalidPhase => 2,
                        flying_acquire::Fault::WrongOrder => 3,
                        flying_acquire::Fault::TooFast => 4,
                        flying_acquire::Fault::TooSlow => 5,
                        flying_acquire::Fault::Expired => 6,
                        flying_acquire::Fault::CycleTooFast => 14,
                        flying_acquire::Fault::CycleTooSlow => 15,
                    };
                    break 'scan;
                }
            }
        }
        if adc {
            let stamp = t17();
            let Some(value) = powered_timer::read_channel([4, 1, 0, 6, 13][adc_index]) else {
                report[0] = 12;
                break;
            };
            unsafe {
                BASELINE.0[adc_index] = value;
                BASELINE.1[adc_index] = stamp;
                BASELINE.2 |= 1 << adc_index;
            }
            #[cfg(feature = "bench-range300")]
            unsafe {
                if BASELINE.2 == 31 {
                    let vcal = core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) as u32;
                    BASELINE_BUS_MV = powered_timer::convert(BASELINE.0, vcal).bus_mv;
                }
            }
            adc_index = (adc_index + 1) % 5;
        }
    }
    #[cfg(feature = "bench-acquire-timing")]
    if recovery && awake && report[0] == 1 {
        unsafe {
            RECOVERY_LATENCY[0] = report[2];
            RECOVERY_LATENCY[1] = t17().wrapping_sub(start) as u32 * 2;
        }
    }
    if report[0] != 1 {
        snapshot_acquisition(&mut report, &acquire, &filters, start, desired);
    }
    #[cfg(feature = "bench-reentry-next-edge-live")]
    if report[0] == 1 && recovery && awake && duty != 0 {
        let c = unsafe { (&mut *core::ptr::addr_of_mut!(FOLLOW)).as_mut().unwrap() };
        let Ok(seed) = c.next.qualify(t17().wrapping_sub(start) as u32 * 2) else {
            let _ = follow_failure(2);
            publish_acquisition_report(report, recovery);
            return None;
        };
        if (seed.step as u32, seed.edge_tick, seed.interval_ticks)
            != (report[1], report[2], report[3])
        {
            let _ = follow_failure(2);
            publish_acquisition_report(report, recovery);
            return None;
        }
        let last_phase = [2usize, 0, 1, 2, 0, 1][report[1] as usize - 1];
        c.phase = if last_phase == 2 { 0 } else { last_phase + 1 };
        cortex_m::interrupt::free(|_| unsafe {
            FOLLOW_REPORT = [0, report[1], report[2], 0, 0, 0, 0, 0];
            FOLLOW_VALID.store(true, Relaxed);
        });
        if let Some((phase, level, onset, confirmed, proof)) = pending_follow {
            #[cfg(feature = "bench-follow-persistence")]
            let result = match proof {
                Some(proof) => c
                    .next
                    .edge_persistent(phase, level, onset, confirmed, proof),
                None => Err(flying_acquire::FollowFault::Confirmation),
            };
            #[cfg(not(feature = "bench-follow-persistence"))]
            let result = if let Some(proof) = proof {
                c.next
                    .prepare_edge(phase, level, onset)
                    .finish_persistent(phase, level, onset, confirmed, proof)
            } else {
                c.next.edge(phase, level, onset, confirmed)
            };
            let Ok(result) = result else {
                let _ = follow_failure(6);
                report[9] = disabled() as u32;
                publish_acquisition_report(report, recovery);
                return None;
            };
            c.completed = Some(result.seed);
            unsafe {
                FOLLOW_REPORT[0] = 1;
                FOLLOW_REPORT[3] = result.seed.step as u32;
                FOLLOW_REPORT[4] = onset;
                FOLLOW_REPORT[5] = confirmed;
                FOLLOW_REPORT[6] = result.follow_interval_ticks;
            }
        }
        // Snapshot above belongs to the original12 intervals. Service the
        // continuing stream before cleanup and publishing that snapshot.
        if !setup_follow(1) {
            report[9] = disabled() as u32;
            publish_acquisition_report(report, recovery);
            return None;
        }
    }
    comp_input::stop();
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, saved);
    }
    // acquire_inner stopped every scheduler before sensing; this loop never
    // grants output/timer authority. Successful awake return only needs the
    // physical clear, not another full scheduler shutdown on the fresh edge.
    // Fault/refusal paths retain full gates+ENABLE safing.
    if awake && report[0] == 1 {
        #[cfg(feature = "bench-reentry-pwm-stage")]
        let keep_pwm = duty != 0 && recovery && phase_role_live::recovery_prepared(duty);
        #[cfg(not(feature = "bench-reentry-pwm-stage"))]
        let keep_pwm = false;
        #[cfg(feature = "bench-reentry-pwm-stage")]
        if duty != 0 && !keep_pwm {
            report[0] = 8;
            gates_off();
            set_pin(3, 1, false);
        }
        if keep_pwm {
            // This sensing loop never enabled MOE or wrote a phase role. The
            // complete prepared-off state was just revalidated; retain it only
            // on this successful recovery return. Every failure still safes.
        } else if cfg!(feature = "bench-reentry-carrier") && recovery {
            bridge_clear_inner::<true>();
        } else {
            bridge_clear();
        }
    } else {
        gates_off();
        set_pin(3, 1, false);
    }
    #[cfg(feature = "bench-acquire-timing")]
    if recovery && awake && report[0] == 1 {
        unsafe {
            RECOVERY_LATENCY[2] = t17().wrapping_sub(start) as u32 * 2;
        }
    }
    report[9] = disabled() as u32;
    if track && report[0] == 1 {
        let seed = flying_acquire::Seed {
            step: report[1] as u8,
            edge_tick: report[2],
            interval_ticks: report[3],
        };
        if !core_bench::coast_flying_run(seed, start) {
            report[0] = 11;
        }
    }
    publish_acquisition_report(report, recovery);
    #[cfg(feature = "bench-acquire-timing")]
    if recovery && awake && report[0] == 1 {
        unsafe {
            RECOVERY_LATENCY[3] = t17().wrapping_sub(start) as u32 * 2;
        }
    }
    if report[0] == 1 {
        Some((
            flying_acquire::Seed {
                step: report[1] as u8,
                edge_tick: report[2],
                interval_ticks: report[3],
            },
            start,
        ))
    } else {
        None
    }
}
fn publish_acquisition_report(report: [u32; 16], recovery: bool) {
    if recovery {
        publish_report::<true>(report);
    } else {
        publish_report::<false>(report);
    }
}
#[inline(never)]
fn snapshot_acquisition(
    report: &mut [u32; 16],
    acquire: &flying_acquire::RuntimeAcquire,
    filters: &[flying_acquire::EdgeFilter; 3],
    start: u16,
    desired: u32,
) {
    report[4] = t17().wrapping_sub(start) as u32;
    report[6] = filters.iter().map(|f| f.max_gap).max().unwrap_or(0);
    report[7] = filters.iter().map(|f| f.cancelled).sum();
    report[8] = acquire.intervals() as u32;
    report[10] = acquire.qualification_waits;
    report[11..15].copy_from_slice(&acquire.cycles);
    unsafe {
        SELECTION = [desired, acquire.skipped];
    }
}
fn cycle_summary<W: Write>(out: &mut W, label: &str, r: &[u32; 16]) {
    let _ = writeln!(
        out,
        "{} checked={} min_ticks={} max_ticks={} rejected_ticks={} cycle_min_ticks={} cycle_max_ticks=12000 individual_min_ticks={}",
        label,
        r[11],
        r[12],
        r[13],
        r[14],
        flying_acquire::CYCLE_MIN_TICKS,
        flying_acquire::INDIVIDUAL_MIN_TICKS
    );
    let _ = writeln!(out, "{}CONFIRM final_visits={} dwell_us=20", label, r[15]);
}
pub fn summary<W: Write>(out: &mut W, capture: bool) {
    if !PENDING.swap(false, Relaxed) {
        return;
    }
    let s = unsafe { REPORT };
    let selection = unsafe { SELECTION };
    let _ = writeln!(
        out,
        "FLYSELECT desired={} skipped_initial_edges={} twelve_intervals_unchanged=1",
        selection[0], selection[1]
    );
    let _ = writeln!(
        out,
        "FLYWAIT polls={} candidate_only=1 max_candidate_age_ticks=240 physical_interval_max_ticks=2000",
        s[10]
    );
    let _ = writeln!(
        out,
        "FLY result={} step={} edge_ticks={} interval_ticks={} elapsed_us={} samples={} max_phase_gap_ticks={} cancelled={} intervals={} disabled={} settle_us=10 edge_dwell_ticks=40 gate_authority=0",
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], s[8], s[9]
    );
    if capture {
        cycle_summary(out, "FLYCYCLE", &s);
    }
    if capture && ((2..=6).contains(&s[0]) || (14..=15).contains(&s[0])) {
        let _ = writeln!(
            out,
            "FLYFAIL half_us=1 fields=last_step,last_edge,decision,phase,A_flags,A_onset,A_sample,B_flags,B_onset,B_sample,C_flags,C_onset,C_sample flags=stable_or_candidate_shift2 absent=2"
        );
        let failure = unsafe { FAILURE };
        let _ = snapshot::record(out, "F85", &failure);
        let edge = unsafe { FAILURE_EDGE };
        if edge & 0x10000 != 0 {
            let _ = writeln!(out, "FLYEDGE qualified_tick={}", edge & 65535);
        }
    }
}
