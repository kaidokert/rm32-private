//! Disabled-driver reference execution benchmark: synthetic comparator levels,
//! real TIM2 interval timer + TIM16 IRQ, actual minz COM/COMP routines.
//! Output HAL is record-only by default. Explicit guarded_power_run grants
//! a bounded reference-output session through powered_timer, never raw gates.
use super::*;
use core_state::{DriveStore, DutyStore, SchedStore, ZctStore};
use minz_core::{am32_hal::*, am32_isr as isr, am32_loop as lp};
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
#[cfg(all(
    feature = "bench-lean-core",
    any(
        all(feature = "bench-cpu-timing", not(feature = "bench-cpu-aggregate")),
        feature = "bench-comp-paths",
        feature = "bench-qualification-direct",
        feature = "bench-qualification-sparse",
        feature = "bench-irq-tail",
        feature = "bench-comp-critical",
        feature = "bench-filter-control"
    )
))]
compile_error!("lean core and ISR diagnostic/filter experiment features must use separate images");
#[cfg(all(feature = "bench-reverse-comp-dma-peer", feature = "bench-dma-peer"))]
compile_error!("reverse COMP/DMA top-peer A/B requires DMA at priority zero");
static S: SchedStore = SchedStore::new();
static D: DriveStore = DriveStore::new();
static U: DutyStore = DutyStore::new();
static Z: ZctStore = ZctStore::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
static REAL_IRQ: AtomicBool = AtomicBool::new(false);
static LIVE_IRQ: AtomicBool = AtomicBool::new(false);
static COAST_REFERENCE: AtomicBool = AtomicBool::new(false);
// Retained through cleanup/reporting: a late ISR must still use the guarded
// writer (and be vetoed), never fall back to an unguarded output path.
static POWER_DUTY: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reverse-advance24-override")]
static ADV24_FIRST_CI: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reverse-advance24-override")]
static ADV24_FIRST_WAIT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-first-accept")]
static mut FIRST_ACCEPT: Option<[u16; 3]> = None;
#[cfg(feature = "bench-handoff-registers")]
static mut HANDOFF_REGISTERS: Option<[u32; 10]> = None;
static POWER_WINDOW_US: AtomicU32 = AtomicU32::new(1_000_000);
static mut FIRST_SEGMENT: segment_archive::Archive = segment_archive::Archive::new();
static FIRST_FREEZE_ERROR: AtomicU32 = AtomicU32::new(0);
static REENTRY_ARM: AtomicBool = AtomicBool::new(false);
static REENTRY_SESSION: AtomicBool = AtomicBool::new(false);
static mut REENTRY_REPORT: [u32; 6] = [0; 6];
static REENTRY_RESERVE: AtomicU32 = AtomicU32::new(200);
pub fn reentry_arm(on: bool) {
    REENTRY_ARM.store(on, Relaxed);
}
static DROP_ARM: AtomicBool = AtomicBool::new(false);
static DROP_AFTER_US: AtomicU32 = AtomicU32::new(2_000_000);
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_DROP_ARM: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_DROP_AFTER_US: AtomicU32 = AtomicU32::new(2_000_000);
#[cfg(feature = "bench-normal-restart")]
static DRIVEN_TRACK_ARM: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-normal-restart")]
static TRACK_ARM: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-normal-restart")]
static TRACK_AT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_REENTRY_ARM: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driven-handoff")]
static mut DRIVEN_FIRST: Option<[u32; 7]> = None;
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_reentry_arm(on: bool) {
    DRIVEN_REENTRY_ARM.store(on, Relaxed);
}
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_dropout_arm(on: bool) {
    DRIVEN_DROP_ARM.store(on, Relaxed);
}
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_dropout_after_ms(ms: u32) -> bool {
    if !(2_000..=10_000).contains(&ms) || active() || powered_timer::owns() {
        return false;
    }
    DRIVEN_DROP_AFTER_US.store(ms * 1000, Relaxed);
    true
}
#[cfg(feature = "bench-normal-restart")]
pub fn driven_tracking_arm(on: bool) {
    DRIVEN_TRACK_ARM.store(on, Relaxed);
}
static DROP_ACTIVE: AtomicBool = AtomicBool::new(false);
static DROP_AT: AtomicU32 = AtomicU32::new(0);
pub fn dropout_arm(on: bool) {
    DROP_ARM.store(on, Relaxed);
}
pub fn power_window_ms(ms: u32) -> bool {
    if !(20..=600_000).contains(&ms) || ACTIVE.load(Relaxed) {
        return false;
    }
    POWER_WINDOW_US.store(ms * 1000, Relaxed);
    true
}
fn window_us() -> u32 {
    if POWER_DUTY.load(Relaxed) != 0 {
        POWER_WINDOW_US.load(Relaxed)
    } else {
        20_000
    }
}
#[cfg(feature = "bench-normal-restart")]
pub fn configured_window_us() -> u32 {
    POWER_WINDOW_US.load(Relaxed)
}
static COAST_ARM: AtomicU32 = AtomicU32::new(0);
static COAST_DONE: AtomicBool = AtomicBool::new(false);
static FLY_SEEDED: AtomicBool = AtomicBool::new(false);
static FLY_ATTEMPT: AtomicBool = AtomicBool::new(false);
// Foreground-only provenance for the post-stop seed summary, never ISR work.
static SEED_COMMANDED: AtomicBool = AtomicBool::new(false);
static FLY_AGE: AtomicU32 = AtomicU32::new(0);
// Foreground-only stage ages from the real edge; fixture output after shutdown.
static mut FLY_LATENCY: [u32; 8] = [0; 8];
fn flying_mark(index: usize, flying: Option<(flying_acquire::Seed, u16)>) {
    if let Some((seed, origin)) = flying {
        let age = (t17().wrapping_sub(origin) as u32 * 2).wrapping_sub(seed.edge_tick);
        unsafe {
            FLY_LATENCY[index] = age;
        }
    }
}
static FLY_ARR: AtomicU32 = AtomicU32::new(0);
static FLY_ARM_US: AtomicU32 = AtomicU32::new(0);
static COAST_POLL_ARM: AtomicBool = AtomicBool::new(false);
static COAST_POLL: AtomicBool = AtomicBool::new(false);
static POLL_COMS: AtomicU32 = AtomicU32::new(0);
static MODE_CHANGES: AtomicU32 = AtomicU32::new(0);
static FIRST_IRQ_MODE_US: AtomicU32 = AtomicU32::new(0);
static DSY_CAPTURED: AtomicBool = AtomicBool::new(false);
static mut DSY_STATE: [u32; 4] = [0; 4];
static mut CYCLE_CORE: Option<[u32; 12]> = None;
/// Called only after a cycle refusal has physically safed the bridge, while
/// still in EV_ACC. Reference IRQs are stopped; capture no hardware timer age
/// here because interrupt_routine already reset TIM2 and scheduled COM.
pub fn cycle_refusal_after_safing() {
    if powered_timer::reason() != 12 || !bridge_disabled() {
        return;
    }
    cortex_m::interrupt::free(|_| unsafe {
        if matches!(CYCLE_CORE, None) {
            CYCLE_CORE = Some([
                D.current_step.load(Relaxed) as u32,
                D.rising.load(Relaxed) as u32,
                S.average_interval.load(Relaxed),
                S.last_average_interval.load(Relaxed),
                S.commutation_interval.load(Relaxed),
                S.this_zc.load(Relaxed) as u32,
                S.last_zc.load(Relaxed) as u32,
                S.wait_time.load(Relaxed) as u32,
                D.filter_level.load(Relaxed) as u32,
                D.zero_crosses.load(Relaxed) as u32,
                D.old_routine.load(Relaxed) as u32,
                D.running.load(Relaxed) as u32,
            ]);
        }
    });
}
static HISTORY_N: AtomicU32 = AtomicU32::new(0);
static HISTORY_DROP: AtomicU32 = AtomicU32::new(0);
static mut HISTORY: [[u16; 8]; 32] = [[0; 8]; 32];
pub fn coast_poll_arm(on: bool) {
    COAST_POLL_ARM.store(on, Relaxed);
}
static COAST_STOP: AtomicU32 = AtomicU32::new(0);
static LAST_IRQ_US: AtomicU32 = AtomicU32::new(0);
static mut COAST_END_STATE: [u32; 12] = [0; 12];
pub fn coast_arm(step: u32) {
    COAST_ARM.store(step, Relaxed);
}
pub fn coast_take() -> u32 {
    COAST_ARM.swap(0, Relaxed)
}
pub fn coast_report_pending() -> bool {
    COAST_DONE.swap(false, Relaxed)
}
static PHYSICAL_OBSERVATION: AtomicBool = AtomicBool::new(false);
static TRACE_READS: AtomicBool = AtomicBool::new(false);
static TRACE_ENABLED: AtomicBool = AtomicBool::new(true);
pub fn trace_enable(enabled: bool) {
    TRACE_ENABLED.store(enabled, Relaxed);
}
static LEVEL_N: AtomicU32 = AtomicU32::new(0);
static LEVEL_FIRST: AtomicU32 = AtomicU32::new(0);
static LEVEL_LAST: AtomicU32 = AtomicU32::new(0);
static GATE_COUNT: AtomicU32 = AtomicU32::new(u32::MAX);
static ACCEPTS: AtomicU32 = AtomicU32::new(0);
// Small acquisition log, independent of optional per-read IRQ tracing. The
// live prefix, powered reference or disabled coast writes it. Timestamp origin
// is observation start, extended across TIM17 wraps, never reset per event.
// Storage is still bounded: overflow is explicit and not a complete trace.
static ACCEPT_N: AtomicU32 = AtomicU32::new(0);
static ACCEPT_DROP: AtomicU32 = AtomicU32::new(0);
static PREFIX_END_US: AtomicU32 = AtomicU32::new(0);
static mut ACCEPT_LOG: [[u16; 4]; 32] = [[0; 4]; 32];
static mut ACCEPT_TAIL: event_tail::Tail = event_tail::Tail::new();
#[cfg(feature = "bench-interval-tail")]
static mut INTERVAL_TAIL: [[u16; 4]; 128] = [[0; 4]; 128];
#[cfg(feature = "bench-interval-tail")]
static mut INTERVAL_TOTAL: u32 = 0;
#[cfg(feature = "bench-interval-tail")]
static mut INTERVAL_LAST: u16 = 0;
#[cfg(feature = "bench-persistence-hist")]
static mut PERSIST_REJECT: [[u32; 12]; 6] = [[0; 12]; 6];
#[cfg(feature = "bench-persistence-hist")]
static mut PERSIST_ACCEPT: [u32; 6] = [0; 6];
#[cfg(feature = "bench-persistence-hist")]
static mut PERSIST_DUTY: u16 = 0;

#[cfg(feature = "bench-persistence-hist")]
#[inline(always)]
fn persistence_hist_reset(duty: u32) {
    unsafe {
        core::ptr::addr_of_mut!(PERSIST_REJECT).write([[0; 12]; 6]);
        core::ptr::addr_of_mut!(PERSIST_ACCEPT).write([0; 6]);
        core::ptr::addr_of_mut!(PERSIST_DUTY).write(duty as u16);
    }
}

#[cfg(feature = "bench-persistence-hist")]
#[inline(always)]
fn persistence_hist_reject(sector: usize, index: usize) {
    if sector < 6 && index < 12 {
        unsafe {
            let cell = core::ptr::addr_of_mut!(PERSIST_REJECT)
                .cast::<u32>()
                .add(sector * 12 + index);
            cell.write(cell.read().wrapping_add(1));
        }
    }
}

#[cfg(feature = "bench-persistence-hist")]
#[inline(always)]
fn persistence_hist_accept(sector: usize) {
    if sector < 6 {
        unsafe {
            let cell = core::ptr::addr_of_mut!(PERSIST_ACCEPT)
                .cast::<u32>()
                .add(sector);
            cell.write(cell.read().wrapping_add(1));
        }
    }
}
static mut ACCEPT_STATS: event_stats::Stats = event_stats::Stats::new();
static mut ACCEPT_TIMELINE: event_timeline::Timeline =
    event_timeline::Timeline::new(20_000).unwrap();
static TIMELINE_REFUSED: AtomicU32 = AtomicU32::new(0);
static TRACE_N: AtomicU32 = AtomicU32::new(0);
static TRACE_DROP: AtomicU32 = AtomicU32::new(0);
static TRACE_NEXT: AtomicU32 = AtomicU32::new(0);
// Experimental FIFO+raw sums trade eight optional IRQ-prefix rows for188bytes
// plus linker alignment headroom. Do not reduce the4096byte stack-span gate.
// Wire row format and all normal-build trace capacity unchanged.
const IRQ_TRACE_CAPACITY: usize =
    if cfg!(feature = "bench-dma-feedback") || cfg!(feature = "bench-driven-entry") {
        24
    } else {
        32
    };
const IRQ_TRACE_WORDS: usize = if cfg!(feature = "bench-irq-tail") {
    15
} else {
    14
};
static mut IRQ_TRACE: [[u16; IRQ_TRACE_WORDS]; IRQ_TRACE_CAPACITY] =
    [[0; IRQ_TRACE_WORDS]; IRQ_TRACE_CAPACITY];
pub fn trace_dump<W: Write>(out: &mut W) {
    if !LIVE_IRQ.load(Relaxed) {
        return;
    }
    if cfg!(feature = "bench-irq-tail") {
        let _ = writeln!(
            out,
            "IRQWINDOW mode=tail time_bits=32 capacity={} appended=us_hi",
            IRQ_TRACE_CAPACITY
        );
    }
    let _ = writeln!(
        out,
        "IRQTRACE n={} drop={} fields=seq,us,step,pwm_cnt,gate_count,avg,reads,first,last,pending,masked,accepts,cost_us,rising",
        TRACE_N.load(Relaxed),
        TRACE_DROP.load(Relaxed)
    );
    for i in 0..TRACE_N.load(Relaxed) as usize {
        let start = if cfg!(feature = "bench-irq-tail")
            && TRACE_N.load(Relaxed) as usize == IRQ_TRACE_CAPACITY
        {
            TRACE_NEXT.load(Relaxed) as usize
        } else {
            0
        };
        let index = (start + i) % IRQ_TRACE_CAPACITY;
        let row = unsafe {
            core::ptr::addr_of!(IRQ_TRACE)
                .cast::<[u16; IRQ_TRACE_WORDS]>()
                .add(index)
                .read()
        };
        let _ = snapshot::record(out, "I85", &row);
    }
    let _ = writeln!(out, "IRQTRACE END");
}
pub fn live_stop() {
    #[cfg(feature = "bench-reverse-blank")]
    REVERSE_BLANK_ACTIVE.store(false, Relaxed);
    #[cfg(any(
        feature = "bench-bemf-level-revisit",
        feature = "bench-running-level-revisit"
    ))]
    {
        LEVEL_REVISIT.store(false, Relaxed);
        LEVEL_REVISIT_INFLIGHT.store(false, Relaxed);
        LEVEL_REVISIT_STEP.store(0, Relaxed);
    }
    #[cfg(feature = "bench-final-edge-prepare")]
    FINAL_PREPARED_STEP.store(0, Relaxed);
    // Freeze the first stop and close the recording epoch atomically against
    // foreground/COMP records. A higher-priority guard may interrupt either.
    cortex_m::interrupt::free(|_| {
        if LIVE_IRQ.load(Relaxed) {
            // Preserve first stop only; later safing cannot extend observed age.
            if ACTIVE.load(Relaxed) {
                PREFIX_END_US.store(observation_elapsed(), Relaxed);
                if COAST_REFERENCE.load(Relaxed) {
                    cortex_m::interrupt::free(|_| unsafe {
                        core::ptr::addr_of_mut!(COAST_END_STATE).write([
                            D.current_step.load(Relaxed) as u32,
                            D.rising.load(Relaxed) as u32,
                            core::ptr::read_volatile(COMP2_CSR),
                            Comp.exti_pending() as u32,
                            (*stm32::EXTI::ptr()).imr1().read().bits(),
                            (*stm32::TIM2::ptr()).cnt().read().bits(),
                            (*stm32::TIM16::ptr()).dier().read().bits(),
                            LAST_IRQ_US.load(Relaxed),
                            GATE_COUNT.load(Relaxed),
                            LEVEL_N.load(Relaxed),
                            LEVEL_LAST.load(Relaxed),
                            MASKED.load(Relaxed) as u32,
                        ]);
                    });
                }
            }
            comp_input::stop();
            com_timer::Timer::stop();
            ACTIVE.store(false, Relaxed);
        }
    });
}
pub fn physical_change(step: u8) {
    if LIVE_IRQ.load(Relaxed)
        && OBS_STATUS.load(Relaxed) == 1
        && step as u16 != D.current_step.load(Relaxed)
    {
        OBS_STATUS.store(3, Relaxed);
        live_stop();
    }
}
pub fn observe_irq_start() {
    observe_irq_start_inner::<false>();
}
fn observe_irq_start_inner<const KEEP_COMP_MASKED: bool>() {
    if !LIVE_IRQ.load(Relaxed) || OBS_STATUS.load(Relaxed) != 1 {
        return;
    }
    cortex_m::interrupt::free(|_| {
        // rm32 gate sequence rotates opposite minz's physical gate sequence.
        // Keep raw captures unchanged; invert both HAL level and selected edge.
        comp_input::Input.set_step(D.current_step.load(Relaxed) as u8, !D.rising.load(Relaxed));
        comp_input::Input.change_input();
        comp_input::Input.clear_pending();
        // change_input masks the hardware source, but bypasses Comp's software
        // latch. Preserve both halves of the original enable-then-mask state.
        if KEEP_COMP_MASKED {
            MASKED.store(true, Relaxed);
        }
        ACTIVE.store(true, Relaxed);
        unsafe {
            let mut n = cortex_m::Peripherals::steal().NVIC;
            n.set_priority(
                stm32::Interrupt::ADC_COMP,
                if cfg!(feature = "bench-running-comp-top")
                    || cfg!(feature = "bench-reverse-comp-dma-peer") {
                    0
                } else {
                    0x40
                },
            );
            n.set_priority(
                stm32::Interrupt::TIM16,
                if cfg!(feature = "bench-com-peer") {
                    0x40
                } else {
                    0x80
                },
            );
            cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM16);
            if !KEEP_COMP_MASKED && !comp_input::filtered() {
                cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::ADC_COMP);
            }
        }
        if !KEEP_COMP_MASKED {
            Comp.enable_interrupts();
        }
    });
}
/// Disabled EXTI-path register probe, not a synthetic powered commutation.
#[cfg(feature = "bench-seedmask-check")]
#[inline(never)]
pub fn seedmaskcheck<W: Write>(out: &mut W) {
    if active()
        || powered_timer::owns()
        || driven_run::owns()
        || !bridge_disabled()
        || comp_input::filtered()
    {
        let _ = writeln!(out, "SEEDMASKCHECK refused=1");
        return;
    }
    let passed = cortex_m::interrupt::free(|_| unsafe {
        let saved = (
            LIVE_IRQ.load(Relaxed),
            REAL_IRQ.load(Relaxed),
            OBS_STATUS.load(Relaxed),
            POWER_DUTY.load(Relaxed),
            DROP_ACTIVE.load(Relaxed),
            D.current_step.load(Relaxed),
            D.rising.load(Relaxed),
        );
        let csr = core::ptr::read_volatile(COMP2_CSR);
        comp_input::stop();
        com_timer::Timer::init();
        LIVE_IRQ.store(true, Relaxed);
        REAL_IRQ.store(true, Relaxed);
        OBS_STATUS.store(1, Relaxed);
        POWER_DUTY.store(0, Relaxed);
        DROP_ACTIVE.store(false, Relaxed);
        let mut passed = 0;
        for step in 1..=6 {
            D.current_step.store(step, Relaxed);
            D.rising.store(step & 1 != 0, Relaxed);
            // Deliberately start with a stale software-unmasked latch and a
            // pending CPU request. Masked setup must not expose that request.
            MASKED.store(false, Relaxed);
            cortex_m::peripheral::NVIC::pend(stm32::Interrupt::ADC_COMP);
            observe_irq_start_inner::<true>();
            let masked = MASKED.load(Relaxed)
                && !comp_input::hardware_enabled()
                && !cortex_m::peripheral::NVIC::is_enabled(stm32::Interrupt::ADC_COMP);
            let no_com = (*stm32::TIM16::ptr()).dier().read().bits() == 0
                && (*stm32::TIM16::ptr()).cr1().read().bits() & 1 == 0
                && !cortex_m::peripheral::NVIC::is_pending(stm32::Interrupt::TIM16);
            // Exercise the enable primitive used by COM, not a COM ISR or gate write.
            Comp.enable_interrupts();
            let enabled = !MASKED.load(Relaxed)
                && comp_input::hardware_enabled()
                && cortex_m::peripheral::NVIC::is_enabled(stm32::Interrupt::ADC_COMP);
            Comp.mask_interrupts();
            comp_input::stop();
            com_timer::Timer::stop();
            ACTIVE.store(false, Relaxed);
            if masked && no_com && enabled && bridge_disabled() {
                passed += 1;
            }
        }
        core::ptr::write_volatile(COMP2_CSR, csr);
        LIVE_IRQ.store(saved.0, Relaxed);
        REAL_IRQ.store(saved.1, Relaxed);
        OBS_STATUS.store(saved.2, Relaxed);
        POWER_DUTY.store(saved.3, Relaxed);
        DROP_ACTIVE.store(saved.4, Relaxed);
        D.current_step.store(saved.5, Relaxed);
        D.rising.store(saved.6, Relaxed);
        passed
    });
    let _ = writeln!(
        out,
        "SEEDMASKCHECK passed={} total=6 pending_request_injected=1 enable_primitive_only=1 disabled={} gate_authority=0",
        passed,
        bridge_disabled() as u8
    );
}
static COMP_CALLS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-filter-control")]
static FILTER_PREPARED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-filter-control")]
static mut FILTER_REFUSAL_START: [u32; 4] = [0; 4];
static mut IRQ_RATE: irq_dispatch::Rate = irq_dispatch::Rate::new();
#[cfg(feature = "bench-reverse-irq-probe")]
static mut REVERSE_IRQ_DECISIONS: [u16; 4] = [0; 4];
#[cfg(feature = "bench-reverse-irq-probe")]
static mut REVERSE_IRQ_TRIP: [u32; 10] = [0; 10];
#[cfg(feature = "bench-reverse-blank")]
static REVERSE_BLANK_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-reverse-blank")]
static REVERSE_BLANK_START: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reverse-blank")]
static REVERSE_BLANK_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-reverse-blank")]
static REVERSE_BLANK_MAX_US: AtomicU32 = AtomicU32::new(0);
pub fn irq_rate_peak() -> u16 {
    unsafe { (*core::ptr::addr_of!(IRQ_RATE)).peak }
}
pub const fn reverse_rate_limit() -> u16 {
    if cfg!(feature = "bench-reverse-irq-cap64") {
        64
    } else if cfg!(feature = "bench-reverse-irq-cap24") {
        24
    } else if cfg!(feature = "bench-reverse-low-irq-cap") {
        12
    } else {
        64
    }
}
#[cfg(feature = "bench-reverse-irq-probe")]
pub fn reverse_irq_summary<W: Write>(out: &mut W) {
    let decisions = unsafe { core::ptr::addr_of!(REVERSE_IRQ_DECISIONS).read() };
    let trip = unsafe { core::ptr::addr_of!(REVERSE_IRQ_TRIP).read() };
    let _ = writeln!(out, "REVERSEIRQ dispatch={} masked={} hw_off={} not_pending={} limit={} postrun_only=1", decisions[0], decisions[1], decisions[2], decisions[3], reverse_rate_limit());
    let _ = writeln!(out, "REVERSEIRQTRIP step={} csr={} rpr={} fpr={} imr={} adc_isr={} adc_ier={} pwm_cnt={} t17={} powered={} one_shot=1", trip[0], trip[1], trip[2], trip[3], trip[4], trip[5], trip[6], trip[7], trip[8], trip[9]);
}
#[cfg(feature = "bench-reverse-blank")]
pub fn reverse_blank_summary<W: Write>(out: &mut W) {
    let _ = writeln!(out, "REVERSEBLANK arms={} max_mask_us={} still_active={} low_speed_only=1 postrun_only=1", REVERSE_BLANK_COUNT.load(Relaxed), REVERSE_BLANK_MAX_US.load(Relaxed), REVERSE_BLANK_ACTIVE.load(Relaxed) as u8);
}
#[cfg(feature = "bench-reverse-blank")]
fn reverse_blank_poll() {
    if !REVERSE_BLANK_ACTIVE.load(Relaxed) {
        return;
    }
    cortex_m::interrupt::free(|_| {
        if !REVERSE_BLANK_ACTIVE.load(Relaxed) {
            return;
        }
        if !powered_timer::owns() || OBS_STATUS.load(Relaxed) != 1 {
            REVERSE_BLANK_ACTIVE.store(false, Relaxed);
            return;
        }
        let elapsed = t17().wrapping_sub(REVERSE_BLANK_START.load(Relaxed) as u16) as u32;
        if elapsed < 280 {
            return;
        }
        REVERSE_BLANK_MAX_US.store(REVERSE_BLANK_MAX_US.load(Relaxed).max(elapsed), Relaxed);
        Comp.clear_pending();
        REVERSE_BLANK_ACTIVE.store(false, Relaxed);
        Comp.enable_interrupts();
    });
}
static DISPATCH_SKIP: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
static DISPATCH_SNAPSHOT_VALID: AtomicBool = AtomicBool::new(false);
static mut DISPATCH_SNAPSHOT: [u32; 8] = [0; 8];
static LEVEL: AtomicBool = AtomicBool::new(false);
static MASKED: AtomicBool = AtomicBool::new(true);
static PENDING: AtomicBool = AtomicBool::new(false);
static COMMUTATIONS: AtomicU32 = AtomicU32::new(0);
static COM_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_N: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_MIN: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_LATE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXCESS_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXIT_INDEX: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXIT_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXIT_MATCHED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXIT_OVER_WAIT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_EXIT_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-top-high")]
static COMP_RECORD_SECTOR: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_WAIT_MIN: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(feature = "bench-com-lag")]
static COM_LAG_WAIT_MAX: AtomicU32 = AtomicU32::new(0);
static COMP_MAX: AtomicU32 = AtomicU32::new(0);
static EVENTS: AtomicU32 = AtomicU32::new(0);
static LATE_ACCEPTS: AtomicU32 = AtomicU32::new(0);
static RECORD_MAX_US: AtomicU32 = AtomicU32::new(0);
static OBS_STATUS: AtomicU32 = AtomicU32::new(0); // 1 observing,2 request,3 mismatch,4 mode change
static OBS_POLLS: AtomicU32 = AtomicU32::new(0);
static OBS_COST: AtomicU32 = AtomicU32::new(0);
static OBS_REQUEST_US: AtomicU32 = AtomicU32::new(0);
static OBS_START: AtomicU32 = AtomicU32::new(0);
static OBS_PHYSICAL: AtomicU32 = AtomicU32::new(0);
static OBS_SEED_CI: AtomicU32 = AtomicU32::new(0);
static OBS_WAIT_UPDATES: AtomicU32 = AtomicU32::new(0);
static POLL_ARMED: AtomicBool = AtomicBool::new(false);
static POLL_TIMER: AtomicBool = AtomicBool::new(false);
static POLL_LAST: AtomicU32 = AtomicU32::new(0);
static POLL_GAP: AtomicU32 = AtomicU32::new(0);
// OBS_START16 remains only for legacy 16-bit prefix trace encoding.
static OBS_START16: AtomicU32 = AtomicU32::new(0);
static mut OBS_CLOCK: sampled_clock::Clock = sampled_clock::Clock::new(0);
fn observation_elapsed() -> u32 {
    cortex_m::interrupt::free(|_| unsafe {
        (&mut *core::ptr::addr_of_mut!(OBS_CLOCK)).sample(t17())
    })
}
#[cfg(feature = "bench-qualification-sparse")]
static SPARSE_OBSERVATION_EPOCH: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-qualification-direct")]
static DIRECT_OBSERVATION_EPOCH: AtomicU32 = AtomicU32::new(0);
fn observation_reset() {
    #[cfg(feature = "bench-qualification-window")]
    qualification_live::reset();
    #[cfg(feature = "bench-quiet-irq-stamp")]
    LAST_IRQ_US.store(0, Relaxed);
    #[cfg(feature = "bench-comp-critical")]
    {
        CRITICAL_MAX.store(0, Relaxed);
        CRITICAL_CALLS.store(0, Relaxed);
        CRITICAL_REFUSED.store(0, Relaxed);
    }
    cortex_m::interrupt::free(|_| unsafe {
        #[cfg(feature = "bench-comp-paths")]
        comp_path_live::reset();
        #[cfg(feature = "bench-qualification-direct")]
        {
            qualification_direct_live::reset();
            DIRECT_OBSERVATION_EPOCH.store(qualification_direct_live::epoch(), Relaxed);
        }
        #[cfg(feature = "bench-qualification-sparse")]
        {
            qualification_sparse_live::reset();
            SPARSE_OBSERVATION_EPOCH.store(qualification_sparse_live::epoch(), Relaxed);
        }
        let raw = t17();
        OBS_START16.store(raw as u32, Relaxed);
        core::ptr::addr_of_mut!(OBS_CLOCK).write(sampled_clock::Clock::new(raw));
    });
}
static BOUNDARY_DELAY: AtomicU32 = AtomicU32::new(0);
static NEXT_SECTOR: AtomicU32 = AtomicU32::new(0);
static SELECTED_SECTOR: AtomicU32 = AtomicU32::new(0);
pub fn select_sector(step: u32) {
    NEXT_SECTOR.store(step, Relaxed);
}
// Exactly one fresh-sector prefix; no reseeding after a request/divergence.
pub fn physical_applied(step: u8) {
    if !LIVE_IRQ.load(Relaxed) || OBS_STATUS.load(Relaxed) != 6 {
        return;
    }
    let selected = SELECTED_SECTOR.load(Relaxed);
    if selected != 0 && selected != step as u32 {
        return;
    }
    BOUNDARY_DELAY.store(observation_elapsed(), Relaxed);
    observation_reset();
    D.current_step.store(step as u16, Relaxed);
    D.rising.store(step & 1 != 0, Relaxed);
    OBS_PHYSICAL.store(step as u32, Relaxed);
    // Model a just-serviced COM: elapsed since the prior accepted input is
    // its programmed wait+1, not ci/2 (which falsely opens blanking at once).
    // Still an assumed reference trajectory, NOT a measured previous ZC.
    Interval.set_count(S.wait_time.load(Relaxed) as u32 + 1);
    OBS_STATUS.store(1, Relaxed);
    observe_irq_start();
}
pub fn polling_arm(on: bool) {
    POLL_ARMED.store(on, Relaxed);
}
pub fn polling_stop() {
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM7_LPTIM2);
    unsafe {
        (*stm32::TIM7::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM7::ptr()).cr1().write(|w| w.bits(0));
    }
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM7_LPTIM2);
}
pub fn polling_start() {
    if !POLL_TIMER.load(Relaxed) || OBS_STATUS.load(Relaxed) != 1 {
        return;
    }
    unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | (1 << 5)));
        let t = &*stm32::TIM7::ptr();
        t.cr1().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(49));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        // TIM6 forced waveform stays at priority0 and preempts this wait.
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(stm32::Interrupt::TIM7_LPTIM2, 0xc0);
        POLL_LAST.store(t17() as u32, Relaxed);
        POLL_GAP.store(0, Relaxed);
        t.dier().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(1));
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM7_LPTIM2);
    }
}
pub fn polling_interrupt() {
    unsafe {
        (*stm32::TIM7::ptr()).sr().write(|w| w.bits(0));
    }
    if OBS_STATUS.load(Relaxed) != 1 || !POLL_TIMER.load(Relaxed) {
        polling_stop();
        return;
    }
    let now = t17();
    let dt = now.wrapping_sub(POLL_LAST.load(Relaxed) as u16) as u32;
    POLL_LAST.store(now as u32, Relaxed);
    POLL_GAP.store(POLL_GAP.load(Relaxed).max(dt), Relaxed);
    if COAST_REFERENCE.load(Relaxed) && COAST_POLL.load(Relaxed) {
        if observation_elapsed() >= window_us() {
            polling_stop();
            return;
        }
        let before = COMMUTATIONS.load(Relaxed);
        let was_polling = D.old_routine.load(Relaxed);
        if !was_polling {
            return;
        }
        #[cfg(feature = "bench-startup-polling")]
        if SEED_COMMANDED.load(Relaxed) && powered_timer::owns() && before == 0 {
            // Bootstrap COM must finish before the polling acceptance band.
            // It is the sole armed COM until a qualified input schedules one.
            return;
        }
        #[cfg(not(feature = "bench-startup-polling"))]
        isr::polling_bemf_check(
            &S.sched(),
            &D.drive(),
            &U.duty(),
            &Z.zct(),
            &mut motor(),
            &observer(),
            D.running.load(Relaxed),
        );
        #[cfg(feature = "bench-startup-polling")]
        if D.running.load(Relaxed) {
            // Same reference polling band, with an operational acceptance seam
            // at the actual qualification decision. A post-call zero_crosses
            // delta could also come from a preempting COM, so is not a proof.
            let mut m = motor();
            m.comp().mask_interrupts();
            minz_core::am32_control::get_bemf_state(&D.drive(), &*m.comp());
            let threshold = if D.rising.load(Relaxed) {
                D.min_bemf_up.load(Relaxed)
            } else {
                D.min_bemf_down.load(Relaxed)
            };
            if !D.zcfound.load(Relaxed) && D.bemf_counter.load(Relaxed) > threshold {
                D.zcfound.store(true, Relaxed);
                if powered_timer::owns() {
                    powered_timer::accepted(
                        D.current_step.load(Relaxed) as u8,
                        S.average_interval.load(Relaxed),
                    );
                    ACCEPTS.store(ACCEPTS.load(Relaxed) + 1, Relaxed);
                }
                minz_core::am32_control::zcfoundroutine(
                    &S.sched(),
                    &D.drive(),
                    &Z.zct(),
                    &U.duty(),
                    &mut m,
                    &observer(),
                );
            }
        }
        POLL_COMS.store(
            POLL_COMS.load(Relaxed) + COMMUTATIONS.load(Relaxed).wrapping_sub(before),
            Relaxed,
        );
        OBS_POLLS.store(OBS_POLLS.load(Relaxed) + 1, Relaxed);
        OBS_COST.store(
            OBS_COST.load(Relaxed).max(t17().wrapping_sub(now) as u32),
            Relaxed,
        );
        if was_polling && !D.old_routine.load(Relaxed) {
            MODE_CHANGES.store(MODE_CHANGES.load(Relaxed) + 1, Relaxed);
            if FIRST_IRQ_MODE_US.load(Relaxed) == 0 {
                FIRST_IRQ_MODE_US.store(observation_elapsed(), Relaxed);
            }
        }
        return;
    }
    observe_sample_inner(wave_timer::physical().0, false, true);
    if OBS_STATUS.load(Relaxed) != 1 {
        polling_stop();
    }
}
pub fn active() -> bool {
    ACTIVE.load(Relaxed)
}

/// Live exploration does not silently restart at the original segment duty.
/// Reject updates in recovery campaigns; session deadlines remain untouched.
#[cfg(feature = "bench-live-control")]
pub fn live_duty_current() -> Option<u32> {
    if !real_irq_active() || REENTRY_SESSION.load(Relaxed) {
        return None;
    }
    let duty = POWER_DUTY.load(Relaxed);
    if duty == 0 { None } else { Some(duty) }
}
/// Read only after a tracking stop with the bridge disabled, before the next
/// observe_begin clears POWER_DUTY. This is the last live request actually
/// published with the coherent PWM update, not an unacknowledged host byte.
#[cfg(all(feature = "bench-live-control", feature = "bench-normal-restart"))]
pub fn stopped_live_duty() -> Option<u32> {
    if powered_timer::reason() != 8 || !bridge_disabled() {
        return None;
    }
    let duty = POWER_DUTY.load(Relaxed);
    duty_envelope::contains(duty).then_some(duty)
}
#[cfg(feature = "bench-live-control")]
pub fn live_interval() -> u32 {
    S.average_interval.load(Relaxed)
}
#[cfg(any(feature = "bench-fast-sag-causal", feature = "bench-phase-peak-stop"))]
pub fn sag_snapshot() -> [u32; 5] {
    [
        S.average_interval.load(Relaxed),
        S.this_zc.load(Relaxed) as u32,
        unsafe { (*stm32::TIM2::ptr()).cnt().read().bits() },
        D.current_step.load(Relaxed) as u32,
        POWER_DUTY.load(Relaxed),
    ]
}
#[cfg(feature = "bench-live-control")]
pub fn publish_live_duty(duty: u32, _cs: &cortex_m::interrupt::CriticalSection) {
    #[cfg(feature = "bench-revisit-origin")]
    if POWER_DUTY.load(Relaxed) != duty {
        // Caller holds the IRQ mask, so no accepted ISR can race this epoch.
        unsafe { core::ptr::addr_of_mut!(REVISIT_ORIGIN_COUNTS).write([[0; 4]; 2]); }
        REVISIT_ORIGIN_DUTY.store(duty, Relaxed);
    }
    POWER_DUTY.store(duty, Relaxed);
    #[cfg(feature = "bench-cpu-target-epoch")]
    if matches!(duty, 350 | 400 | 450 | 500) {
        // Foreground live publication already holds PRIMASK. Discard the
        // ramp's samples so the next IRQ starts a target-duty-only epoch.
        super::cpu_sparse::reset();
    }
    #[cfg(feature = "bench-com-top-high")]
    unsafe {
        // Foreground live publication holds PRIMASK. Keep startup and lower
        // rungs at their qualified peer priority; only the high-duty A/B
        // lets COM preempt a comparator ISR after its timer arm.
        cortex_m::Peripherals::steal().NVIC.set_priority(
            stm32::Interrupt::TIM16,
            if duty >= 480 { 0 } else { 0x40 },
        );
    }
    #[cfg(feature = "bench-persistence-hist")]
    persistence_hist_reset(duty);
    #[cfg(feature = "bench-advance-scheduled")]
    U.duty_cycle.store(
        #[cfg(feature = "bench-reverse-advance22-high")]
        reverse_advance::level(duty),
        #[cfg(not(feature = "bench-reverse-advance22-high"))]
        if duty >= 350 {
            22
        } else if duty >= 300 {
            20
        } else {
            18
        },
        Relaxed,
    );
}

fn live_advance_level() -> u32 {
    #[cfg(feature = "bench-advance-scheduled")]
    {
        return lp::interrupt_advance_level(&U.duty());
    }
    #[cfg(not(feature = "bench-advance-scheduled"))]
    {
        lp::TEMP_ADVANCE
    }
}
#[cfg(feature = "bench-reverse-advance22-high")]
pub fn reverse_advance_summary<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "ADVANCEPROFILE below350=20 at_or_above350=22 retained_level={} postrun_only=1",
        U.duty_cycle.load(Relaxed)
    );
    #[cfg(feature = "bench-reverse-advance24-override")]
    {
        let ci = S.commutation_interval.load(Relaxed);
        let stored_wait = S.wait_time.load(Relaxed) as u32;
        let high = POWER_DUTY.load(Relaxed) >= 350;
        let first_ci = ADV24_FIRST_CI.load(Relaxed);
        let first_wait = ADV24_FIRST_WAIT.load(Relaxed);
        let _ = writeln!(
            out,
            "ADVANCEOVERRIDE high_active={} published=22 effective_high={} first_ci={} first_wait={} first_expected={} witnessed={} terminal_ci={} terminal_wait={} terminal_expected={} terminal_match={} postrun_only=1",
            high as u8, reverse_advance24::LEVEL, first_ci, first_wait, reverse_advance24::wait(first_ci),
            (first_ci != 0 && first_wait == reverse_advance24::wait(first_ci)) as u8,
            ci, stored_wait, reverse_advance24::wait(ci),
            (stored_wait == reverse_advance24::wait(ci)) as u8
        );
    }
}

struct Output;
impl PwmOutput for Output {
    fn set_duty_all(&mut self, _: u16) {}
    fn set_auto_reload(&mut self, _: u16) {}
    fn set_prescaler(&mut self, _: u16) {}
    fn set_compare1(&mut self, _: u16) {}
    fn set_compare2(&mut self, _: u16) {}
    fn set_compare3(&mut self, _: u16) {}
    fn generate_update_event(&mut self) {}
    fn set_dead_time_override(&mut self, _: u16) {}
}
impl PhaseOutput for Output {
    fn com_step(&mut self, step: u8) {
        COMMUTATIONS.store(COMMUTATIONS.load(Relaxed) + 1, Relaxed);
        let duty = POWER_DUTY.load(Relaxed);
        if duty != 0 {
            if powered_timer::commit(step, duty) {
                OBS_PHYSICAL.store(step as u32, Relaxed);
            } else {
                powered_timer::abort();
            }
            return;
        }
        if OBS_STATUS.load(Relaxed) == 1 && !COAST_REFERENCE.load(Relaxed) {
            OBS_REQUEST_US.store(
                if POLL_TIMER.load(Relaxed) || LIVE_IRQ.load(Relaxed) {
                    observation_elapsed()
                } else {
                    clock_us().wrapping_sub(OBS_START.load(Relaxed))
                },
                Relaxed,
            );
            OBS_STATUS.store(2, Relaxed);
        }
    }
    fn all_off(&mut self) {
        if POWER_DUTY.load(Relaxed) != 0 {
            powered_timer::abort();
        }
    }
    fn full_brake(&mut self) {}
    fn all_pwm(&mut self) {}
    fn proportional_brake(&mut self) {}
}
struct Comp;
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
static LEVEL_REVISIT: AtomicBool = AtomicBool::new(false);
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
static LEVEL_REVISIT_INFLIGHT: AtomicBool = AtomicBool::new(false);
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
static LEVEL_REVISIT_STEP: AtomicU32 = AtomicU32::new(0);
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
static LEVEL_REVISIT_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
static LEVEL_REVISIT_ACCEPTS: AtomicU32 = AtomicU32::new(0);
// Diagnostic-only acceptance-time buckets for the final live-duty epoch.
// Rows: all accepted, then measured interval > prior average * 1.25.
// Columns: physical-only, software-only, both flags, neither flag.
#[cfg(feature = "bench-revisit-origin")]
static mut REVISIT_ORIGIN_COUNTS: [[u32; 4]; 2] = [[0; 4]; 2];
#[cfg(feature = "bench-revisit-origin")]
static REVISIT_ORIGIN_DUTY: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-revisit-origin")]
static REVISIT_DISPATCH_FLAGS: AtomicU32 = AtomicU32::new(0);
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
fn revisit_low_speed_level() {
    cortex_m::interrupt::free(|_| {
        // One-variable reverse high-duty A/B. Serialize this decision with
        // foreground duty publication so an ACKed >=35% epoch cannot pend a
        // synthetic service. The ordinary COMP ISR and every guard are intact.
        #[cfg(feature = "bench-running-revisit-off35")]
        if !level_revisit::below_high_duty_cutoff(POWER_DUTY.load(Relaxed)) {
            return;
        }
        #[cfg(feature = "bench-running-revisit-off50")]
        if !level_revisit::below_duty_cutoff(POWER_DUTY.load(Relaxed), 500) {
            return;
        }
        #[cfg(feature = "bench-running-revisit-off48")]
        if !level_revisit::below_duty_cutoff(POWER_DUTY.load(Relaxed), 480) {
            return;
        }
        let average = S.average_interval.load(Relaxed);
        let step = D.current_step.load(Relaxed) as u32;
        let ready = level_revisit::admit(level_revisit::Inputs {
            owner: powered_timer::owns(),
            active: active(),
            coast_reference: COAST_REFERENCE.load(Relaxed),
            software_masked: MASKED.load(Relaxed),
            hardware_enabled: comp_input::hardware_enabled(),
            average_half_us: average,
            interval_half_us: Interval.count(),
            pending: Comp.exti_pending(),
            post_level: Comp.output_level() == D.rising.load(Relaxed),
            same_step_retried: LEVEL_REVISIT_STEP.load(Relaxed) == step,
            allow_high_speed: cfg!(feature = "bench-running-level-revisit"),
        });
        if !ready {
            return;
        }
        LEVEL_REVISIT_STEP.store(step, Relaxed);
        LEVEL_REVISIT_INFLIGHT.store(true, Relaxed);
        LEVEL_REVISIT.store(true, Relaxed);
        LEVEL_REVISIT_ATTEMPTS.store(LEVEL_REVISIT_ATTEMPTS.load(Relaxed) + 1, Relaxed);
        cortex_m::peripheral::NVIC::pend(stm32::Interrupt::ADC_COMP);
    });
}
#[cfg(any(
    feature = "bench-bemf-level-revisit",
    feature = "bench-running-level-revisit"
))]
#[inline(always)]
fn level_revisit_accepted() {
    if LEVEL_REVISIT_INFLIGHT.swap(false, Relaxed) {
        LEVEL_REVISIT_ACCEPTS.store(LEVEL_REVISIT_ACCEPTS.load(Relaxed) + 1, Relaxed);
    }
    // A real acceptance advances the command. Permit one retry for the next
    // command even when its numeric step wraps after six commutations.
    LEVEL_REVISIT_STEP.store(0, Relaxed);
}
#[cfg_attr(feature = "bench-inline-comp", inline(always))]
#[cfg_attr(not(feature = "bench-inline-comp"), inline)]
fn read_comp_level(real: bool, inverted: bool, trace: bool) -> bool {
    let read = || {
        if real {
            comp_input::Input.output_level()
        } else {
            LEVEL.load(Relaxed)
        }
    };
    #[cfg(feature = "bench-filter-latency")]
    let raw = if real && trace && LEVEL_N.load(Relaxed) == 0 && comp_input::filtered() {
        filter_latency::read_first(
            COMP_CALLS.load(Relaxed),
            D.current_step.load(Relaxed) as u32,
            read,
        )
    } else {
        read()
    };
    #[cfg(not(feature = "bench-filter-latency"))]
    let raw = read();
    let value = if inverted { !raw } else { raw };
    if trace {
        let n = LEVEL_N.load(Relaxed);
        if n == 0 {
            LEVEL_FIRST.store(value as u32, Relaxed);
        }
        LEVEL_LAST.store(value as u32, Relaxed);
        LEVEL_N.store(n + 1, Relaxed);
    }
    value
}
impl Comparator for Comp {
    fn output_level(&self) -> bool {
        read_comp_level(
            REAL_IRQ.load(Relaxed) || POLL_TIMER.load(Relaxed),
            PHYSICAL_OBSERVATION.load(Relaxed),
            TRACE_READS.load(Relaxed),
        )
    }
    fn set_step(&mut self, step: u8, rising: bool) {
        if REAL_IRQ.load(Relaxed) {
            comp_input::Input.set_step(
                step,
                if LIVE_IRQ.load(Relaxed) {
                    !rising
                } else {
                    rising
                },
            );
        }
    }
    fn change_input(&mut self) {
        if REAL_IRQ.load(Relaxed) && COAST_REFERENCE.load(Relaxed) {
            comp_input::Input.change_input();
            return; // actual phase/neutral selection, no synthetic reference
        }
        if REAL_IRQ.load(Relaxed) && !LIVE_IRQ.load(Relaxed) {
            comp_input::Input.change_input();
            // Diagnostic-only stable internal reference. Output before each
            // crossing is opposite expected. NOT production phase sensing.
            unsafe {
                let old = core::ptr::read_volatile(COMP2_CSR);
                let pol = if D.rising.load(Relaxed) { 0 } else { 1 << 15 };
                core::ptr::write_volatile(COMP2_CSR, (old & !(15 << 4 | 1 << 15)) | (3 << 4) | pol);
            }
            comp_input::Input.clear_pending();
        }
    }
    fn enable_interrupts(&mut self) {
        if DROP_ACTIVE.load(Relaxed) {
            return;
        }
        if POWER_DUTY.load(Relaxed) != 0 && (!ACTIVE.load(Relaxed) || !powered_timer::owns()) {
            return;
        }
        if LIVE_IRQ.load(Relaxed) && OBS_STATUS.load(Relaxed) != 1 {
            return;
        }
        MASKED.store(false, Relaxed);
        if REAL_IRQ.load(Relaxed) {
            comp_input::Input.enable_interrupts();
        }
    }
    fn mask_interrupts(&mut self) {
        MASKED.store(true, Relaxed);
        if REAL_IRQ.load(Relaxed) {
            comp_input::Input.mask_interrupts();
        }
    }
}
impl CompExti for Comp {
    fn exti_pending(&self) -> bool {
        #[cfg(feature = "bench-revisit-origin")]
        {
            // minz-core clears EXTI *before* EV_ACC. Capture both sources at
            // the dispatch gate; every later dispatch overwrites a rejected
            // pass, while an accepted pass reads its own entry snapshot.
            let software = LEVEL_REVISIT.load(Relaxed);
            let physical = if REAL_IRQ.load(Relaxed) {
                comp_input::Input.exti_pending()
            } else {
                PENDING.load(Relaxed)
            };
            REVISIT_DISPATCH_FLAGS.store((software as u32) | ((physical as u32) << 1), Relaxed);
            return software || physical;
        }
        #[cfg(not(feature = "bench-revisit-origin"))]
        {
        #[cfg(any(
            feature = "bench-bemf-level-revisit",
            feature = "bench-running-level-revisit"
        ))]
        if LEVEL_REVISIT.load(Relaxed) {
            return true;
        }
        if REAL_IRQ.load(Relaxed) {
            return comp_input::Input.exti_pending();
        }
        PENDING.load(Relaxed)
        }
    }
    fn clear_pending(&self) {
        #[cfg(any(
            feature = "bench-bemf-level-revisit",
            feature = "bench-running-level-revisit"
        ))]
        LEVEL_REVISIT.store(false, Relaxed);
        PENDING.store(false, Relaxed);
        if REAL_IRQ.load(Relaxed) {
            comp_input::Input.clear_pending();
        }
    }
}
struct Interval;
impl IntervalTimer for Interval {
    fn count(&self) -> u32 {
        let n = unsafe { (*stm32::TIM2::ptr()).cnt().read().bits() & 65535 };
        #[cfg(feature = "bench-comp-paths")]
        comp_path_live::first_count(n);
        #[cfg(feature = "bench-qualification-direct")]
        qualification_direct_live::first_count(n as u16);
        if TRACE_READS.load(Relaxed) && GATE_COUNT.load(Relaxed) == u32::MAX {
            GATE_COUNT.store(n, Relaxed);
        }
        n
    }
    fn set_count(&mut self, n: u32) {
        #[cfg(feature = "bench-filter-latency")]
        filter_latency::invalidate();
        #[cfg(feature = "bench-filter-observe")]
        filter_observe::before_counter_reset(n == 0);
        unsafe {
            (*stm32::TIM2::ptr()).cnt().write(|w| w.bits(n & 65535));
        }
    }
}
struct Obs;
impl Recorder for Obs {
    #[cfg(any(
        feature = "bench-qualification-reject",
        feature = "bench-persistence-hist"
    ))]
    #[inline(always)]
    fn persistence_rejected(&self, read_index: u16) {
        #[cfg(feature = "bench-persistence-hist")]
        persistence_hist_reject(
            D.current_step.load(Relaxed).wrapping_sub(1) as usize,
            read_index as usize,
        );
        #[cfg(feature = "bench-qualification-reject")]
        qualification_direct_live::persistence_rejected(read_index);
    }
    fn record(&self, kind: u8, sector: u8, interval: u16) {
        #[cfg(feature = "bench-lean-core")]
        {
            // Accepted progress feeds the operational watchdog. Statistics,
            // tail rings, traces and timing maxima are not control state.
            if kind == minz_core::blackbox::EV_ACC {
                #[cfg(feature = "bench-com-top-high")]
                let sector = if POWER_DUTY.load(Relaxed) >= 480 {
                    // COM may have preempted after the arm and advanced
                    // current_step; the edge belongs to the pre-arm sector.
                    COMP_RECORD_SECTOR.load(Relaxed) as u8
                } else {
                    sector
                };
                #[cfg(any(
                    feature = "bench-bemf-level-revisit",
                    feature = "bench-running-level-revisit"
                ))]
                level_revisit_accepted();
                #[cfg(feature = "bench-persistence-hist")]
                persistence_hist_accept(sector as usize);
                #[cfg(feature = "bench-first-accept")]
                unsafe {
                    if core::ptr::addr_of!(FIRST_ACCEPT).read().is_none() {
                        FIRST_ACCEPT = Some([
                            sector as u16 + 1,
                            interval,
                            OBS_SEED_CI.load(Relaxed) as u16,
                        ]);
                    }
                }
                if powered_timer::owns() {
                    let average = S.average_interval.load(Relaxed);
                    #[cfg(feature = "bench-rate-census")]
                    super::rate_census::accepted(interval, average);
                    #[cfg(feature = "bench-revisit-origin")]
                    // minz clears EXTI before EV_ACC; use the entry snapshot.
                    // "Both" may include a software flag from an earlier
                    // rejected dispatch and remains ambiguous.
                    let source = {
                        let flags = REVISIT_DISPATCH_FLAGS.load(Relaxed);
                        revisit_origin::classify(flags & 1 != 0, flags & 2 != 0)
                    };
                    #[cfg(feature = "bench-revisit-origin")]
                    {
                        let counts = unsafe { &mut *core::ptr::addr_of_mut!(REVISIT_ORIGIN_COUNTS) };
                        counts[0][source as usize] = counts[0][source as usize].wrapping_add(1);
                        if revisit_origin::late(interval, average) {
                            counts[1][source as usize] = counts[1][source as usize].wrapping_add(1);
                        }
                    }
                    #[cfg(feature = "bench-interval-tail")]
                    unsafe {
                        let at = t17();
                        let total = INTERVAL_TOTAL;
                        let gap = if total == 0 {
                            0
                        } else {
                            at.wrapping_sub(INTERVAL_LAST)
                        };
                        let event_limit = powered_timer::speed_event_limit_us();
                        #[cfg(feature = "bench-revisit-origin-tail")]
                        let event_limit = revisit_origin::pack_limit_origin(event_limit, source);
                        INTERVAL_TAIL[(total as usize) & 127] = [
                            gap,
                            interval,
                            average as u16,
                            event_limit,
                        ];
                        INTERVAL_LAST = at;
                        INTERVAL_TOTAL = total.wrapping_add(1);
                    }
                    powered_timer::accepted(sector + 1, average);
                }
                if POWER_DUTY.load(Relaxed) != 0 && !powered_timer::owns() {
                    return;
                }
                if !ACTIVE.load(Relaxed) || OBS_STATUS.load(Relaxed) != 1 {
                    return;
                }
                ACCEPTS.store(ACCEPTS.load(Relaxed) + 1, Relaxed);
            }
            return;
        }
        #[cfg(not(feature = "bench-lean-core"))]
        {
            if COAST_REFERENCE.load(Relaxed)
                && COAST_POLL.load(Relaxed)
                && (kind == minz_core::blackbox::EV_REF || kind == minz_core::blackbox::EV_DSY)
            {
                // Serialize foreground desync and ISR commutation records. No
                // formatting or gate access; bounded eight-word snapshot only.
                cortex_m::interrupt::free(|_| {
                    let n = HISTORY_N.load(Relaxed) as usize;
                    if n < 32 {
                        let row = [
                            t17().wrapping_sub(OBS_START16.load(Relaxed) as u16),
                            kind as u16,
                            sector as u16 + 1,
                            interval,
                            S.average_interval.load(Relaxed) as u16,
                            S.last_average_interval.load(Relaxed) as u16,
                            D.old_routine.load(Relaxed) as u16,
                            D.zero_crosses.load(Relaxed) as u16,
                        ];
                        unsafe {
                            core::ptr::addr_of_mut!(HISTORY)
                                .cast::<[u16; 8]>()
                                .add(n)
                                .write(row);
                        }
                        HISTORY_N.store(n as u32 + 1, Relaxed);
                    } else {
                        HISTORY_DROP.store(HISTORY_DROP.load(Relaxed) + 1, Relaxed);
                    }
                });
            }
            if kind == minz_core::blackbox::EV_DSY
                && COAST_REFERENCE.load(Relaxed)
                && !DSY_CAPTURED.load(Relaxed)
            {
                // Reference emits EV_DSY before replacing last_average_interval.
                unsafe {
                    core::ptr::addr_of_mut!(DSY_STATE).write([
                        observation_elapsed(),
                        S.last_average_interval.load(Relaxed),
                        interval as u32,
                        S.commutation_interval.load(Relaxed),
                    ]);
                }
                DSY_CAPTURED.store(true, Relaxed);
            }
            if kind == minz_core::blackbox::EV_ACC {
                #[cfg(any(
                    feature = "bench-bemf-level-revisit",
                    feature = "bench-running-level-revisit"
                ))]
                level_revisit_accepted();
                if powered_timer::owns() {
                    let average = S.average_interval.load(Relaxed);
                    #[cfg(feature = "bench-interval-tail")]
                    unsafe {
                        let at = t17();
                        let total = INTERVAL_TOTAL;
                        let gap = if total == 0 {
                            0
                        } else {
                            at.wrapping_sub(INTERVAL_LAST)
                        };
                        INTERVAL_TAIL[(total as usize) & 127] = [
                            gap,
                            interval,
                            average as u16,
                            powered_timer::speed_event_limit_us(),
                        ];
                        INTERVAL_LAST = at;
                        INTERVAL_TOTAL = total.wrapping_add(1);
                    }
                    powered_timer::accepted(sector + 1, average);
                }
                let began = t17();
                let recorded = cortex_m::interrupt::free(|_| {
                    if LIVE_IRQ.load(Relaxed) && OBS_STATUS.load(Relaxed) == 1 {
                        if !ACTIVE.load(Relaxed)
                            || (POWER_DUTY.load(Relaxed) != 0 && !powered_timer::owns())
                        {
                            LATE_ACCEPTS.store(LATE_ACCEPTS.load(Relaxed) + 1, Relaxed);
                            return false;
                        }
                        let us = observation_elapsed();
                        cortex_m::interrupt::free(|_| unsafe {
                            (&mut *core::ptr::addr_of_mut!(ACCEPT_STATS)).push(us, sector + 1);
                            if !(&mut *core::ptr::addr_of_mut!(ACCEPT_TIMELINE)).push(us) {
                                TIMELINE_REFUSED.store(TIMELINE_REFUSED.load(Relaxed) + 1, Relaxed);
                            }
                        });
                        let n = ACCEPT_N.load(Relaxed) as usize;
                        if n < 32 {
                            unsafe {
                                core::ptr::addr_of_mut!(ACCEPT_LOG)
                                    .cast::<[u16; 4]>()
                                    .add(n)
                                    .write([
                                        us as u16,
                                        (us >> 16) as u16,
                                        sector as u16 + 1,
                                        interval,
                                    ]);
                            }
                            ACCEPT_N.store(n as u32 + 1, Relaxed);
                        } else {
                            ACCEPT_DROP.store(ACCEPT_DROP.load(Relaxed) + 1, Relaxed);
                            cortex_m::interrupt::free(|_| unsafe {
                                (&mut *core::ptr::addr_of_mut!(ACCEPT_TAIL)).push([
                                    us as u16,
                                    (us >> 16) as u16,
                                    sector as u16 + 1,
                                    interval,
                                ]);
                            });
                        }
                    }
                    #[cfg(feature = "bench-qualification-direct")]
                    qualification_direct_live::accepted(ACCEPTS.load(Relaxed), sector + 1);
                    ACCEPTS.store(ACCEPTS.load(Relaxed) + 1, Relaxed);
                    true
                });
                RECORD_MAX_US.store(
                    RECORD_MAX_US
                        .load(Relaxed)
                        .max(t17().wrapping_sub(began) as u32),
                    Relaxed,
                );
                if !recorded {
                    return;
                }
            }
            EVENTS.store(EVENTS.load(Relaxed) + 1, Relaxed);
        }
    }
    fn freeze(&self) {}
}
impl Cs for Obs {
    fn free<R>(&self, f: impl FnOnce() -> R) -> R {
        cortex_m::interrupt::free(|_| f())
    }
}
impl InjAdc for Obs {
    fn inj_read(&self) -> (u16, u16, u16, u16) {
        (0, 0, 0, 0)
    }
}
impl LoopTimer for Obs {
    fn clear_flag(&self) {}
}
// These mode flags are configured outside each controller invocation. Safety
// interrupts can mask/stop the motor but do not reconfigure read polarity/source
// or tracing. NEVER cache the comparator value, pending bits, or safety state.
#[cfg(feature = "bench-cached-comp")]
struct CachedComp {
    real: bool,
    inverted: bool,
    trace: bool,
}
// Only selected after the same per-invocation mode snapshot used by CachedComp.
// No comparator signal, pending flag, ownership or safety result is cached.
#[cfg(feature = "bench-static-comp")]
struct StaticComp;
#[cfg(feature = "bench-static-comp")]
pub fn readcadencecheck<W: Write>(out: &mut W) {
    if !bridge_disabled() || ACTIVE.load(Relaxed) || powered_timer::owns() {
        let _ = writeln!(out, "READCADENCE refused=1");
        return;
    }
    let mut maximum = 0u16;
    let mut minimum = u16::MAX;
    for _ in 0..64 {
        let mut remaining = core::hint::black_box(12u32);
        let start = t17();
        while remaining != 0 {
            core::hint::black_box(StaticComp.output_level());
            remaining -= 1;
        }
        let elapsed = t17().wrapping_sub(start);
        maximum = maximum.max(elapsed);
        minimum = minimum.min(elapsed);
    }
    let _ = writeln!(
        out,
        "READCADENCE call={} trials=64 reads=12 min_us={} max_us={} disabled={} actual_isr=0 gate_authority=0",
        cfg!(feature = "bench-comp-read-call") as u8,
        minimum,
        maximum,
        bridge_disabled() as u8
    );
}
#[cfg(feature = "bench-comp-critical")]
static CRITICAL_MAX: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-comp-critical")]
static CRITICAL_CALLS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-comp-critical")]
static CRITICAL_REFUSED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-comp-critical")]
#[inline(always)]
fn critical_service(filter: u16, body: impl FnOnce()) -> Option<u32> {
    if filter != 12 {
        return None;
    }
    Some(cortex_m::interrupt::free(|_| {
        let start = t17();
        body();
        t17().wrapping_sub(start) as u32
    }))
}

/// Tests the same wrapper, NOT a powered reference/recorder WCET certificate.
#[cfg(feature = "bench-comp-critical")]
pub fn criticalcheck<W: Write>(out: &mut W) {
    if powered_timer::owns()
        || !bridge_disabled()
        || ACTIVE.load(Relaxed)
        || !cortex_m::register::primask::read().is_active()
    {
        let _ = writeln!(out, "CRITICALCHECK refused=1");
        return;
    }
    let invalid = critical_service(0, || panic!("invalid filter executed")).is_none()
        && critical_service(13, || panic!("invalid filter executed")).is_none();
    for mode in 0..4 {
        let mut passed = 0;
        let mut maximum = 0;
        let mut restored = true;
        for _ in 0..16 {
            let mut reads = 0;
            let mut masked = true;
            let body = || {
                masked &= !cortex_m::register::primask::read().is_active();
                for _ in 0..12 {
                    core::hint::black_box(unsafe { core::ptr::read_volatile(COMP2_CSR) });
                    reads += 1;
                    if mode == 0 {
                        return;
                    }
                }
                if mode == 3 {
                    let start = t17();
                    while t17().wrapping_sub(start) < 61 {}
                }
            };
            let (elapsed, nested_restored) = if mode == 2 {
                cortex_m::interrupt::free(|_| {
                    let elapsed = critical_service(12, body).unwrap();
                    (elapsed, !cortex_m::register::primask::read().is_active())
                })
            } else {
                (critical_service(12, body).unwrap(), true)
            };
            maximum = maximum.max(elapsed);
            restored &= nested_restored && cortex_m::register::primask::read().is_active();
            if invalid
                && masked
                && nested_restored
                && cortex_m::register::primask::read().is_active()
                && reads == if mode == 0 { 1 } else { 12 }
                && if mode == 3 {
                    elapsed > 60 && elapsed <= 100
                } else {
                    elapsed <= 60
                }
            {
                passed += 1;
            }
        }
        let _ = writeln!(
            out,
            "CRITICALCHECK mode={} passed={} total=16 max_us={} invalid_filters_refused={} restored={} disabled={} wrapper_only=1 gate_authority=0",
            mode,
            passed,
            maximum,
            invalid as u8,
            restored as u8,
            bridge_disabled() as u8
        );
    }
}
#[cfg(feature = "bench-comp-decisions")]
pub fn decisioncheck<W: Write>(out: &mut W) {
    if powered_timer::owns() || !bridge_disabled() || ACTIVE.load(Relaxed) {
        let _ = writeln!(out, "DECISIONCHECK refused=1");
        return;
    }
    comp_path_live::check(out);
    let _ = writeln!(
        out,
        "DECISIONCHECK END disabled={}",
        bridge_disabled() as u8
    );
}
#[cfg(feature = "bench-static-comp")]
impl Comparator for StaticComp {
    #[cfg_attr(not(feature = "bench-comp-read-call"), inline(always))]
    #[cfg_attr(feature = "bench-comp-read-call", inline(never))]
    fn output_level(&self) -> bool {
        read_comp_level(true, true, false)
    }
    fn set_step(&mut self, step: u8, rising: bool) {
        Comp.set_step(step, rising);
    }
    fn change_input(&mut self) {
        Comp.change_input();
    }
    fn enable_interrupts(&mut self) {
        Comp.enable_interrupts();
    }
    fn mask_interrupts(&mut self) {
        Comp.mask_interrupts();
    }
}
#[cfg(feature = "bench-static-comp")]
impl CompExti for StaticComp {
    fn exti_pending(&self) -> bool {
        Comp.exti_pending()
    }
    fn clear_pending(&self) {
        Comp.clear_pending();
    }
}
#[cfg(feature = "bench-cached-comp")]
impl Comparator for CachedComp {
    #[cfg_attr(feature = "bench-inline-comp", inline(always))]
    fn output_level(&self) -> bool {
        read_comp_level(self.real, self.inverted, self.trace)
    }
    fn set_step(&mut self, step: u8, rising: bool) {
        Comp.set_step(step, rising);
    }
    fn change_input(&mut self) {
        Comp.change_input();
    }
    fn enable_interrupts(&mut self) {
        Comp.enable_interrupts();
    }
    fn mask_interrupts(&mut self) {
        Comp.mask_interrupts();
    }
}
#[cfg(feature = "bench-cached-comp")]
impl CompExti for CachedComp {
    fn exti_pending(&self) -> bool {
        Comp.exti_pending()
    }
    fn clear_pending(&self) {
        Comp.clear_pending();
    }
}
#[cfg(feature = "bench-cached-comp")]
type MotorComp = CachedComp;
#[cfg(not(feature = "bench-cached-comp"))]
type MotorComp = Comp;
fn motor() -> Motor<Output, MotorComp, Output, Interval, com_timer::Timer> {
    Motor {
        pwm: Output,
        #[cfg(not(feature = "bench-cached-comp"))]
        comp: Comp,
        #[cfg(feature = "bench-cached-comp")]
        comp: CachedComp {
            real: REAL_IRQ.load(Relaxed) || POLL_TIMER.load(Relaxed),
            inverted: PHYSICAL_OBSERVATION.load(Relaxed),
            trace: TRACE_READS.load(Relaxed),
        },
        phase: Output,
        interval: Interval,
        com: com_timer::Timer,
    }
}
fn observer() -> Observer<'static, Obs, Obs, Obs, Obs> {
    Observer {
        bb: &Obs,
        cs: &Obs,
        adc: &Obs,
        lt: &Obs,
    }
}

pub fn interrupt() {
    if LIVE_IRQ.load(Relaxed)
        && (OBS_STATUS.load(Relaxed) != 1
            || (!COAST_REFERENCE.load(Relaxed)
                && wave_timer::physical().0 as u16 != D.current_step.load(Relaxed)))
    {
        if OBS_STATUS.load(Relaxed) == 1 {
            OBS_STATUS.store(3, Relaxed);
        }
        live_stop();
        return;
    }
    let t = unsafe { &*stm32::TIM16::ptr() };
    if t.sr().read().bits() & 1 == 0 || t.dier().read().bits() & 1 == 0 {
        return;
    }
    #[cfg(feature = "bench-com-lag")]
    if POWER_DUTY.load(Relaxed) >= 480 && (COMMUTATIONS.load(Relaxed) & 15) == 0 {
        // TIM2 resets at accepted ZC, before TIM16 is armed. Sample 1/16
        // COMs to bound observer cost without changing control or protection.
        let actual = unsafe { (*stm32::TIM2::ptr()).cnt().read().bits() & 65535 };
        let wait = S.wait_time.load(Relaxed) as u32 + 1;
        COM_LAG_N.store(COM_LAG_N.load(Relaxed) + 1, Relaxed);
        COM_LAG_MIN.store(COM_LAG_MIN.load(Relaxed).min(actual), Relaxed);
        COM_LAG_MAX.store(COM_LAG_MAX.load(Relaxed).max(actual), Relaxed);
        COM_LAG_WAIT_MIN.store(COM_LAG_WAIT_MIN.load(Relaxed).min(wait), Relaxed);
        COM_LAG_WAIT_MAX.store(COM_LAG_WAIT_MAX.load(Relaxed).max(wait), Relaxed);
        COM_LAG_EXCESS_MAX.store(COM_LAG_EXCESS_MAX.load(Relaxed).max(actual.saturating_sub(wait)), Relaxed);
        if actual > wait + 2 {
            COM_LAG_LATE.store(COM_LAG_LATE.load(Relaxed) + 1, Relaxed);
        }
        if COM_LAG_EXIT_INDEX.load(Relaxed) == COMMUTATIONS.load(Relaxed) {
            let exit = COM_LAG_EXIT_COUNT.load(Relaxed);
            COM_LAG_EXIT_MATCHED.store(COM_LAG_EXIT_MATCHED.load(Relaxed) + 1, Relaxed);
            COM_LAG_EXIT_MAX.store(COM_LAG_EXIT_MAX.load(Relaxed).max(exit), Relaxed);
            if exit >= wait {
                COM_LAG_EXIT_OVER_WAIT.store(COM_LAG_EXIT_OVER_WAIT.load(Relaxed) + 1, Relaxed);
            }
        }
    }
    #[cfg(not(feature = "bench-lean-core"))]
    let start = t17();
    isr::tim1_up_tim16_isr_policy::<{ !cfg!(feature = "bench-lean-core") }, _, _>(
        &S.sched(),
        &D.drive(),
        &Z.zct(),
        &U.duty(),
        &mut motor(),
        &observer(),
    );
    #[cfg(feature = "bench-reverse-advance24-override")]
    if POWER_DUTY.load(Relaxed) >= 350
        && powered_timer::owns()
        && OBS_STATUS.load(Relaxed) == 1
        && !D.old_routine.load(Relaxed)
    {
        // minz-core just computed the reference COM interval but rejects a
        // published level24. Replace only the next-edge wait; no gate,
        // comparator, estimator, guard, or minz-core state is bypassed.
        let ci = S.commutation_interval.load(Relaxed);
        let wait = reverse_advance24::wait(ci);
        S.wait_time.store(wait as u16, Relaxed);
        if ADV24_FIRST_CI.load(Relaxed) == 0 {
            ADV24_FIRST_WAIT.store(wait, Relaxed);
            ADV24_FIRST_CI.store(ci, Relaxed);
        }
    }
    #[cfg(feature = "bench-reverse-blank")]
    if POWER_DUTY.load(Relaxed) != 0
        && powered_timer::owns()
        && OBS_STATUS.load(Relaxed) == 1
        && S.average_interval.load(Relaxed) >= 1500
    {
        // At this low speed, the earliest useful crossing is later than
        // 280us. Ignore PWM-coupled EXTI traffic in that known blank window.
        Comp.mask_interrupts();
        REVERSE_BLANK_START.store(t17() as u32, Relaxed);
        REVERSE_BLANK_ACTIVE.store(true, Relaxed);
        REVERSE_BLANK_COUNT.store(REVERSE_BLANK_COUNT.load(Relaxed) + 1, Relaxed);
    }
    #[cfg(not(feature = "bench-lean-core"))]
    {
        let elapsed = t17().wrapping_sub(start) as u32;
        COM_MAX.store(COM_MAX.load(Relaxed).max(elapsed), Relaxed);
    }
    if LIVE_IRQ.load(Relaxed) && OBS_STATUS.load(Relaxed) != 1 {
        live_stop();
    }
}

pub fn real_irq_active() -> bool {
    active() && REAL_IRQ.load(Relaxed)
}

#[cfg(feature = "bench-com-lag")]
pub fn com_lag_summary<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "COMLAG n={} actual_min_halfus={} actual_max_halfus={} wait_min_halfus={} wait_max_halfus={} excess_max_halfus={} late_gt1us={} exit_matched={} exit_over_wait={} exit_max_halfus={} sample_divisor=16 postrun_only=1",
        COM_LAG_N.load(Relaxed),
        COM_LAG_MIN.load(Relaxed),
        COM_LAG_MAX.load(Relaxed),
        COM_LAG_WAIT_MIN.load(Relaxed),
        COM_LAG_WAIT_MAX.load(Relaxed),
        COM_LAG_EXCESS_MAX.load(Relaxed),
        COM_LAG_LATE.load(Relaxed),
        COM_LAG_EXIT_MATCHED.load(Relaxed),
        COM_LAG_EXIT_OVER_WAIT.load(Relaxed),
        COM_LAG_EXIT_MAX.load(Relaxed),
    );
}

/// Observe one causally valid prefix of the physical forced-drive trace.
/// The record-only HAL cannot change gates or comparator mux. Sampling cadence
/// is the microscope cadence, not claimed to be production20kHz polling.
pub fn observe_begin(step: u8, hz: u32, started: u32) {
    #[cfg(feature = "bench-reverse-advance24-override")]
    {
        ADV24_FIRST_CI.store(0, Relaxed);
        ADV24_FIRST_WAIT.store(0, Relaxed);
    }
    #[cfg(feature = "bench-revisit-origin")]
    {
        unsafe { core::ptr::addr_of_mut!(REVISIT_ORIGIN_COUNTS).write([[0; 4]; 2]); }
        REVISIT_ORIGIN_DUTY.store(0, Relaxed);
    }
    #[cfg(feature = "bench-final-edge-prepare")]
    FINAL_PREPARED_USED.store(false, Relaxed);
    unsafe {
        CYCLE_CORE = None;
    }
    POWER_DUTY.store(0, Relaxed);
    DROP_ACTIVE.store(false, Relaxed);
    DROP_AT.store(0, Relaxed);
    flying_bench::recovery_reset();
    FLY_SEEDED.store(false, Relaxed);
    FLY_ATTEMPT.store(false, Relaxed);
    COAST_REFERENCE.store(false, Relaxed);
    SELECTED_SECTOR.store(NEXT_SECTOR.swap(0, Relaxed), Relaxed);
    PHYSICAL_OBSERVATION.store(true, Relaxed);
    BOUNDARY_DELAY.store(0, Relaxed);
    TRACE_N.store(0, Relaxed);
    TRACE_DROP.store(0, Relaxed);
    ACCEPTS.store(0, Relaxed);
    TRACE_NEXT.store(0, Relaxed);
    LAST_IRQ_US.store(0, Relaxed);
    GATE_COUNT.store(u32::MAX, Relaxed);
    LEVEL_N.store(0, Relaxed);
    LEVEL_LAST.store(2, Relaxed);
    for counter in &DISPATCH_SKIP {
        counter.store(0, Relaxed);
    }
    DISPATCH_SNAPSHOT_VALID.store(false, Relaxed);
    DSY_CAPTURED.store(false, Relaxed);
    HISTORY_N.store(0, Relaxed);
    HISTORY_DROP.store(0, Relaxed);
    live_stop();
    LIVE_IRQ.store(hz >= 167 && hz <= 250, Relaxed);
    unsafe {
        core::ptr::addr_of_mut!(IRQ_RATE).write(irq_dispatch::Rate::new());
        #[cfg(feature = "bench-reverse-irq-probe")]
        {
            core::ptr::addr_of_mut!(REVERSE_IRQ_DECISIONS).write([0; 4]);
            core::ptr::addr_of_mut!(REVERSE_IRQ_TRIP).write([0; 10]);
        }
    }
    #[cfg(feature = "bench-reverse-blank")]
    {
        REVERSE_BLANK_ACTIVE.store(false, Relaxed);
        REVERSE_BLANK_START.store(0, Relaxed);
        REVERSE_BLANK_COUNT.store(0, Relaxed);
        REVERSE_BLANK_MAX_US.store(0, Relaxed);
    }
    ACCEPT_N.store(0, Relaxed);
    ACCEPT_DROP.store(0, Relaxed);
    PREFIX_END_US.store(0, Relaxed);
    LATE_ACCEPTS.store(0, Relaxed);
    RECORD_MAX_US.store(0, Relaxed);
    cortex_m::interrupt::free(|_| unsafe {
        core::ptr::addr_of_mut!(ACCEPT_TAIL).write(event_tail::Tail::new());
        core::ptr::addr_of_mut!(ACCEPT_STATS).write(event_stats::Stats::new());
        core::ptr::addr_of_mut!(ACCEPT_TIMELINE)
            .write(event_timeline::Timeline::new(window_us()).unwrap());
        #[cfg(feature = "bench-interval-tail")]
        {
            INTERVAL_TAIL = [[0; 4]; 128];
            INTERVAL_TOTAL = 0;
            INTERVAL_LAST = 0;
        }
        #[cfg(any(
            feature = "bench-bemf-level-revisit",
            feature = "bench-running-level-revisit"
        ))]
        {
            LEVEL_REVISIT.store(false, Relaxed);
            LEVEL_REVISIT_INFLIGHT.store(false, Relaxed);
            LEVEL_REVISIT_STEP.store(0, Relaxed);
            LEVEL_REVISIT_ATTEMPTS.store(0, Relaxed);
            LEVEL_REVISIT_ACCEPTS.store(0, Relaxed);
        }
        #[cfg(feature = "bench-persistence-hist")]
        persistence_hist_reset(0);
    });
    polling_stop();
    POLL_TIMER.store(POLL_ARMED.swap(false, Relaxed), Relaxed);
    observation_reset();
    OBS_STATUS.store(0, Relaxed);
    OBS_POLLS.store(0, Relaxed);
    OBS_COST.store(0, Relaxed);
    OBS_REQUEST_US.store(0, Relaxed);
    OBS_START.store(started, Relaxed);
    OBS_WAIT_UPDATES.store(0, Relaxed);
    // Bound the reference blocking polling wait to ~1ms at this initial stage.
    if !(50..=250).contains(&hz) {
        return;
    }
    REAL_IRQ.store(LIVE_IRQ.load(Relaxed), Relaxed);
    ACTIVE.store(false, Relaxed);
    if LIVE_IRQ.load(Relaxed) {
        POLL_TIMER.store(false, Relaxed);
    }
    COMP_CALLS.store(0, Relaxed);
    COMP_MAX.store(0, Relaxed);
    COM_MAX.store(0, Relaxed);
    #[cfg(feature = "bench-com-lag")]
    {
        COM_LAG_N.store(0, Relaxed);
        COM_LAG_MIN.store(u32::MAX, Relaxed);
        COM_LAG_MAX.store(0, Relaxed);
        COM_LAG_LATE.store(0, Relaxed);
        COM_LAG_EXCESS_MAX.store(0, Relaxed);
        COM_LAG_EXIT_INDEX.store(u32::MAX, Relaxed);
        COM_LAG_EXIT_COUNT.store(0, Relaxed);
        COM_LAG_EXIT_MATCHED.store(0, Relaxed);
        COM_LAG_EXIT_OVER_WAIT.store(0, Relaxed);
        COM_LAG_EXIT_MAX.store(0, Relaxed);
        COM_LAG_WAIT_MIN.store(u32::MAX, Relaxed);
        COM_LAG_WAIT_MAX.store(0, Relaxed);
    }
    EVENTS.store(0, Relaxed);
    com_timer::Timer::init();
    comp_input::stop();
    let ci = 2_000_000 / (6 * hz);
    OBS_SEED_CI.store(ci, Relaxed);
    S.commutation_interval.store(ci, Relaxed);
    S.average_interval.store(ci, Relaxed);
    S.last_average_interval.store(ci, Relaxed);
    for v in &S.interval_hist {
        v.store(ci, Relaxed);
    }
    S.last_zc.store(ci as u16, Relaxed);
    S.this_zc.store(ci as u16, Relaxed);
    let advance = minz_core::am32::advance_of(ci, lp::TEMP_ADVANCE);
    S.wait_time
        .store(minz_core::am32::wait_time(ci, advance) as u16, Relaxed);
    D.current_step.store(step as u16, Relaxed);
    D.rising.store(step & 1 != 0, Relaxed);
    D.old_routine.store(!LIVE_IRQ.load(Relaxed), Relaxed);
    D.running.store(true, Relaxed);
    D.zcfound.store(false, Relaxed);
    D.bemf_counter.store(0, Relaxed);
    D.bad_count.store(0, Relaxed);
    D.zero_crosses.store(20, Relaxed);
    D.desync_check.store(false, Relaxed);
    D.desync_happened.store(0, Relaxed);
    D.bemf_timeout_happened.store(0, Relaxed);
    U.input.store(300, Relaxed);
    U.adjusted_input.store(300, Relaxed);
    #[cfg(feature = "bench-reverse-advance22-high")]
    U.duty_cycle.store(reverse_advance::level(100), Relaxed);
    Z.head.store(0, Relaxed);
    Z.tail.store(0, Relaxed);
    Z.drop.store(0, Relaxed);
    Z.comm_n.store(0, Relaxed);
    COMMUTATIONS.store(0, Relaxed);
    unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM2::ptr();
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(31));
        t.arr().write(|w| w.bits(65535));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        // Explicit assumed prior ZC, not a measured rotor timestamp.
        let seed = if LIVE_IRQ.load(Relaxed) {
            S.wait_time.load(Relaxed) as u32 + 1
        } else {
            ci / 2
        };
        t.cnt().write(|w| w.bits(seed));
        t.cr1().write(|w| w.bits(1));
    }
    OBS_PHYSICAL.store(step as u32, Relaxed);
    OBS_STATUS.store(1, Relaxed);
    observe_bands();
    if LIVE_IRQ.load(Relaxed) {
        OBS_STATUS.store(6, Relaxed);
    }
}
pub fn observe_sample(step: u8, level: bool, valid: bool) {
    if LIVE_IRQ.load(Relaxed) {
        physical_change(wave_timer::physical().0);
        return;
    }
    if POLL_TIMER.load(Relaxed) {
        if OBS_STATUS.load(Relaxed) == 1 {
            observe_bands();
        }
        return;
    }
    observe_sample_inner(step, level, valid);
}
fn observe_sample_inner(step: u8, level: bool, valid: bool) {
    if OBS_STATUS.load(Relaxed) != 1 {
        return;
    }
    if D.current_step.load(Relaxed) != step as u16 {
        OBS_STATUS.store(3, Relaxed);
        return;
    }
    if !D.old_routine.load(Relaxed) {
        OBS_STATUS.store(4, Relaxed);
        return;
    }
    if !valid {
        return;
    }
    let before = t17();
    let wave_before = wave_timer::stats().0;
    LEVEL.store(level, Relaxed);
    isr::polling_bemf_check(
        &S.sched(),
        &D.drive(),
        &U.duty(),
        &Z.zct(),
        &mut motor(),
        &observer(),
        true,
    );
    if !POLL_TIMER.load(Relaxed) {
        observe_bands();
    }
    OBS_POLLS.store(OBS_POLLS.load(Relaxed) + 1, Relaxed);
    OBS_COST.store(
        OBS_COST
            .load(Relaxed)
            .max(t17().wrapping_sub(before) as u32),
        Relaxed,
    );
    if OBS_STATUS.load(Relaxed) == 2 {
        OBS_WAIT_UPDATES.store(wave_timer::stats().0.wrapping_sub(wave_before), Relaxed);
    }
    // No future comparator reads after the first counterfactual request.
    if OBS_STATUS.load(Relaxed) != 1 {
        com_timer::Timer::stop();
    }
}
fn observe_bands() {
    lp::min_bemf_schedule(&D.drive());
    let ss = S.sched();
    let ect = ss.intervals().e_com_time();
    let avg = lp::store_average_interval(&ss, ect);
    minz_core::am32_control::desync_check_band(&ss, &D.drive(), &U.duty(), &observer(), avg);
    let (running, zc) = lp::filter_and_duty_max(&ss, &D.drive(), &U.duty(), ect, avg);
    lp::bemf_timeout_resets(&D.drive(), &U.duty(), zc);
    minz_core::am32_control::bemf_timeout_rekick(
        &ss,
        &D.drive(),
        &U.duty(),
        &Z.zct(),
        &mut motor(),
        &observer(),
        running,
    );
}
pub fn observe_end() {
    live_stop();
    polling_stop();
    com_timer::Timer::stop();
    unsafe {
        (*stm32::TIM2::ptr()).cr1().write(|w| w.bits(0));
    }
}
pub(super) fn bridge_disabled() -> bool {
    !get_idr(3, 1)
        && unsafe { (*stm32::TIM1::ptr()).bdtr().read().bits() & (1 << 15) == 0 }
        && [(0, 10), (0, 9), (0, 8), (1, 1), (1, 0), (0, 7)]
            .iter()
            .all(|&(p, b)| !get_idr(p, b))
}
/// Actual reference IRQ sequence with sense-mux authority ONLY. Called after
/// an already-disabled normal drive exit, before ordinary coast sampling.
/// Default stops on polling changeover; explicit coastpoll enables the
/// reference blocking polling band on TIM7, with record-only output HAL.
pub fn coast_run(step: u8, hz: u32) -> bool {
    coast_run_inner(step, hz, None, None, None, None)
}
pub fn coast_flying_run(seed: flying_acquire::Seed, origin: u16) -> bool {
    if !(1..=6).contains(&seed.step)
        || !(flying_acquire::SEED_MIN_TICKS..=2000).contains(&seed.interval_ticks)
    {
        return false;
    }
    coast_run_inner(
        seed.step,
        2_000_000 / (6 * seed.interval_ticks),
        Some((seed, origin)),
        None,
        None,
        None,
    )
}
pub fn guarded_dry_run(
    seed: flying_acquire::Seed,
    origin: u16,
    run_start: u32,
    vcal: u32,
    abort: &mut dyn FnMut() -> bool,
) -> bool {
    coast_run_inner(
        seed.step,
        200,
        Some((seed, origin)),
        Some((run_start, vcal, abort)),
        None,
        None,
    )
}
pub fn guarded_power_run(
    seed: flying_acquire::Seed,
    origin: u16,
    run_start: u32,
    vcal: u32,
    duty: u32,
    abort: &mut dyn FnMut() -> bool,
) -> bool {
    if duty == 0 || duty > MAX_DUTY_TENTHS {
        return false;
    }
    let retry = REENTRY_ARM.swap(false, Relaxed);
    REENTRY_SESSION.store(retry, Relaxed);
    let first_elapsed = clock_us().wrapping_sub(run_start);
    if retry {
        unsafe {
            REENTRY_REPORT[5] = first_elapsed + POWER_WINDOW_US.load(Relaxed);
        }
    }
    let budget =
        powered_guard::SessionBudget::initial(first_elapsed, POWER_WINDOW_US.load(Relaxed));
    POWER_DUTY.store(duty, Relaxed);
    let result = guarded_dry_run(seed, origin, run_start, vcal, abort);
    REENTRY_SESSION.store(false, Relaxed);
    if retry {
        let code = resume_once::<200>(run_start, vcal, abort, budget.map(|b| b.0));
        unsafe {
            REENTRY_REPORT[0] = code;
            REENTRY_REPORT[4] = clock_us().wrapping_sub(run_start);
        }
    }
    result
}
#[inline(never)]
fn resume_once<const RESERVE: u32>(
    run_start: u32,
    vcal: u32,
    abort: &mut dyn FnMut() -> bool,
    budget: Option<powered_guard::SessionBudget>,
) -> u32 {
    REENTRY_RESERVE.store(RESERVE, Relaxed);
    if powered_timer::reason() != 8
        || !bridge_disabled()
        || unsafe { (&*core::ptr::addr_of!(FIRST_SEGMENT)).get().is_none() }
    {
        return 1;
    }
    // Snapshot the stopped segment's actual requested duty BEFORE observe_begin
    // resets POWER_DUTY. Never reuse the caller's original pre-update setting.
    // The existing fixed preparation path still supports only40..100. Refuse
    // unsupported recovery targets instead of clamping them or silently falling
    // back. Live changes in retry campaigns remain disabled until the complete
    // high-duty preparation/arm path is validated.
    let duty = POWER_DUTY.load(Relaxed);
    if !(40..=duty_split::MAX).contains(&duty) {
        return 8;
    }
    if abort() {
        return 2;
    }
    let Some(mut budget) = budget else {
        return 3;
    };
    unsafe {
        REENTRY_REPORT[1] = DROP_AT.load(Relaxed);
    }
    powered_timer::prepare();
    observe_begin(1, 200, clock_us());
    if !powered_timer::wake(abort) {
        return 4;
    }
    #[cfg(feature = "bench-current-baseline")]
    prestart_baseline::recovery_check();
    #[cfg(feature = "bench-reentry-pwm-stage")]
    let acquired = flying_bench::reacquire_prepared(duty);
    #[cfg(not(feature = "bench-reentry-pwm-stage"))]
    let acquired = flying_bench::reacquire_awake();
    let Some((seed, origin)) = acquired else {
        gates_off();
        set_pin(3, 1, false);
        return 5;
    };
    #[cfg(feature = "bench-reentry-next-edge-live")]
    if !flying_bench::setup_follow(2) {
        return 5;
    }
    let elapsed = clock_us().wrapping_sub(run_start);
    let Ok(limits) = budget.reentry_reserved::<RESERVE>(elapsed, powered_guard::Fault::Tracking)
    else {
        gates_off();
        set_pin(3, 1, false);
        return 6;
    };
    unsafe {
        REENTRY_REPORT[2] = elapsed;
        REENTRY_REPORT[3] = limits.segment;
    }
    POWER_WINDOW_US.store(limits.segment, Relaxed);
    POWER_DUTY.store(duty, Relaxed);
    let _ = coast_run_inner(
        seed.step,
        200,
        Some((seed, origin)),
        Some((run_start, vcal, abort)),
        Some(limits),
        None,
    );
    7 // second-segment path entered; its guard/result proves success or failure
}
/// Perform timer/counter initialization before acquisition produces a fresh
/// edge. No comparator IRQ or output authority is enabled by observe_begin.
pub fn prepare_early(hz: u32) {
    unsafe {
        REENTRY_REPORT = [0; 6];
    }
    unsafe {
        core::ptr::addr_of_mut!(FIRST_SEGMENT).write(segment_archive::Archive::new());
    }
    FIRST_FREEZE_ERROR.store(0, Relaxed);
    powered_timer::prepare();
    observe_begin(1, hz, clock_us());
}
/// Stage reference state while startup gates are stopped, BEFORE driven
/// acquisition owns TIM2/COMP/TIM6. Does not drop the already-awake ENABLE.
#[cfg(feature = "bench-driven-handoff")]
pub fn prepare_driven(hz: u32) {
    powered_timer::stage_driven();
    unsafe {
        REENTRY_REPORT = [0; 6];
        core::ptr::addr_of_mut!(FIRST_SEGMENT).write(segment_archive::Archive::new());
    }
    FIRST_FREEZE_ERROR.store(0, Relaxed);
    unsafe {
        DRIVEN_FIRST = None;
    }
    REENTRY_ARM.store(DRIVEN_REENTRY_ARM.swap(false, Relaxed), Relaxed);
    DROP_ARM.store(DRIVEN_DROP_ARM.swap(false, Relaxed), Relaxed);
    DROP_AFTER_US.store(DRIVEN_DROP_AFTER_US.load(Relaxed), Relaxed);
    #[cfg(feature = "bench-normal-restart")]
    {
        TRACK_ARM.store(DRIVEN_TRACK_ARM.swap(false, Relaxed), Relaxed);
        TRACK_AT.store(0, Relaxed);
    }
    observe_begin(1, hz, clock_us());
}
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_power_run(
    transfer: driven_run::Transfer,
    run_start: u32,
    vcal: u32,
    duty: u32,
    abort: &mut dyn FnMut() -> bool,
) -> bool {
    DRIVEN_ENTRY_REFUSAL.store(0, Relaxed);
    let (seed, origin) = transfer.seed();
    let maximum = duty_split::segment_max(cfg!(feature = "bench-startup-adc"));
    let refused = if abort() {
        1
    } else if !(40..=maximum).contains(&duty) {
        2
    } else if !powered_timer::adopt_driven(&transfer) {
        3
    } else {
        0
    };
    if refused != 0 {
        DRIVEN_ENTRY_REFUSAL.store(refused, Relaxed);
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    let retry = REENTRY_ARM.swap(false, Relaxed);
    REENTRY_SESSION.store(retry, Relaxed);
    POWER_DUTY.store(duty, Relaxed);
    let first_elapsed = clock_us().wrapping_sub(run_start);
    let window = POWER_WINDOW_US.load(Relaxed);
    let budget = powered_guard::SessionBudget::initial(first_elapsed, window);
    if retry {
        unsafe {
            REENTRY_REPORT[5] = first_elapsed + window;
        }
    }
    let result = coast_run_inner(
        seed.step,
        200,
        Some((seed, origin)),
        Some((run_start, vcal, abort)),
        None,
        Some(transfer.feedback()),
    );
    if !result {
        DRIVEN_ENTRY_REFUSAL.store(4, Relaxed);
    }
    REENTRY_SESSION.store(false, Relaxed);
    if retry {
        // Preserve the first actual arm before resume's observe_begin resets it.
        unsafe {
            DRIVEN_FIRST = Some([
                OBS_SEED_CI.load(Relaxed),
                FLY_AGE.load(Relaxed),
                FLY_ARR.load(Relaxed),
                FLY_ARM_US.load(Relaxed),
                FLY_SEEDED.load(Relaxed) as u32,
                first_elapsed,
                window,
            ]);
        }
        let code = resume_once::<300>(run_start, vcal, abort, budget.map(|b| b.0));
        unsafe {
            REENTRY_REPORT[0] = code;
            REENTRY_REPORT[4] = clock_us().wrapping_sub(run_start);
        }
    }
    result
}
#[cfg(feature = "bench-driven-handoff")]
static DRIVEN_ENTRY_REFUSAL: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_entry_summary<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "DRIVENENTRY refusal={} adopt_refusal={} coast_stop={} fly_age={} fly_arr={} fly_arm_us={} fly_seeded={} postrun_only=1",
        DRIVEN_ENTRY_REFUSAL.load(Relaxed),
        powered_timer::adopt_refusal(),
        COAST_STOP.load(Relaxed),
        FLY_AGE.load(Relaxed),
        FLY_ARR.load(Relaxed),
        FLY_ARM_US.load(Relaxed),
        FLY_SEEDED.load(Relaxed) as u8
    );
    #[cfg(feature = "bench-reverse-seed-stage")]
    if DRIVEN_ENTRY_REFUSAL.load(Relaxed) == 4 {
        let a = unsafe { FLY_LATENCY };
        let _ = writeln!(
            out,
            "REVERSESEEDSTAGE entry={} reset={} feedback={} guard={} reference={} half_us=1 postrun_only=1",
            a[0], a[1], a[2], a[3], a[4]
        );
        let _ = writeln!(
            out,
            "REVERSESEEDBUDGET interval={} wait={} age={} reserve=40 half_us=1 postrun_only=1",
            OBS_SEED_CI.load(Relaxed),
            S.wait_time.load(Relaxed),
            FLY_AGE.load(Relaxed)
        );
        let _ = writeln!(
            out,
            "REVERSESEEDPREFIX after_clear={} after_filter={} after_cold={} half_us=1 postrun_only=1",
            a[5], a[6], a[7]
        );
    }
}
#[cfg(feature = "bench-driven-handoff")]
pub fn driven_first_dump<W: Write>(out: &mut W) {
    if let Some(values) = unsafe { DRIVEN_FIRST } {
        let _ = writeln!(
            out,
            "DRIVENFIRST fields=interval_ticks,edge_age_ticks,remaining_arr,arm_us,armed,first_elapsed_us,requested_us u32_le_pairs=1 before_reentry=1"
        );
        let mut words = [0u16; 14];
        for (i, v) in values.iter().enumerate() {
            words[2 * i] = *v as u16;
            words[2 * i + 1] = (*v >> 16) as u16;
        }
        let _ = snapshot::record(out, "DFA85", &words);
    }
}
// Keep the bulk-copy temporary OUT of the powered run's stack frame. The M0
// has only a few KiB above static capture buffers; active COMP/COM/guard IRQs
// must not nest on top of a segment-sized temporary reserved for later use.
#[inline(never)]
fn freeze_first_segment() -> bool {
    let record = unsafe {
        segment_archive::StoppedSegment {
            power: powered_timer::stopped_snapshot(),
            end_us: PREFIX_END_US.load(Relaxed),
            stats: ACCEPT_STATS,
            timeline: ACCEPT_TIMELINE,
            prefix: ACCEPT_LOG,
            prefix_len: ACCEPT_N.load(Relaxed) as usize,
            tail: ACCEPT_TAIL,
        }
    };
    let frozen = unsafe {
        (&mut *core::ptr::addr_of_mut!(FIRST_SEGMENT)).freeze(
            record,
            !ACTIVE.load(Relaxed) && !powered_timer::owns(),
            bridge_disabled(),
        )
    };
    if let Err(code) = frozen {
        FIRST_FREEZE_ERROR.store(code as u32, Relaxed);
    }
    frozen.is_ok()
}
// No timers, clock origins, mux changes, or output authority in this reset.
#[inline(never)]
fn cold_coast_state(step: u8, polling: bool) {
    D.current_step.store(step as u16, Relaxed);
    D.rising.store(step & 1 != 0, Relaxed);
    OBS_PHYSICAL.store(step as u32, Relaxed);
    COAST_REFERENCE.store(true, Relaxed);
    COAST_STOP.store(0, Relaxed);
    COAST_POLL.store(polling, Relaxed);
    POLL_COMS.store(0, Relaxed);
    MODE_CHANGES.store(0, Relaxed);
    FIRST_IRQ_MODE_US.store(0, Relaxed);
}
#[cfg(feature = "bench-final-edge-prepare")]
static FINAL_PREPARED_STEP: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-final-edge-prepare")]
static FINAL_PREPARED_USED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-final-edge-prepare")]
pub fn prepare_final_step(step: u8) -> bool {
    FINAL_PREPARED_USED.store(false, Relaxed);
    if !(1..=6).contains(&step)
        || active()
        || powered_timer::owns()
        || driven_run::owns()
        || !powered_timer::ready()
        || !powered_timer::outputs_disabled()
        || FINAL_PREPARED_STEP.load(Relaxed) != 0
    {
        return false;
    }
    cold_coast_state(step, false);
    FINAL_PREPARED_STEP.store(step as u32, Relaxed);
    true
}
#[cfg(feature = "bench-final-edge-prepare")]
fn take_final_step(step: u8, polling: bool) -> bool {
    let prepared = FINAL_PREPARED_STEP.swap(0, Relaxed);
    prepared == step as u32 && (1..=6).contains(&step) && !polling
}
/// ENABLE-low test. Injects prepared metadata, never driver readiness.
/// Exercises the actual reset/consumption/revocation functions without power.
#[cfg(feature = "bench-final-edge-check")]
pub fn finalprepcheck<W: Write>(out: &mut W) {
    if active() || powered_timer::owns() || driven_run::owns() || !bridge_disabled() {
        let _ = writeln!(out, "FINALPREPCHECK refused=1");
        return;
    }
    let passed = cortex_m::interrupt::free(|_| {
        let saved = (
            D.current_step.load(Relaxed),
            D.rising.load(Relaxed),
            OBS_PHYSICAL.load(Relaxed),
            COAST_REFERENCE.load(Relaxed),
            COAST_STOP.load(Relaxed),
            COAST_POLL.load(Relaxed),
            POLL_COMS.load(Relaxed),
            MODE_CHANGES.load(Relaxed),
            FIRST_IRQ_MODE_US.load(Relaxed),
            FINAL_PREPARED_USED.load(Relaxed),
        );
        let mut passed = 0;
        FINAL_PREPARED_STEP.store(0, Relaxed);
        if !prepare_final_step(1) && FINAL_PREPARED_STEP.load(Relaxed) == 0 {
            passed += 1;
        }
        for step in 1..=6 {
            cold_coast_state(step, false);
            if D.current_step.load(Relaxed) == step as u16
                && D.rising.load(Relaxed) == (step & 1 != 0)
                && OBS_PHYSICAL.load(Relaxed) == step as u32
                && !active()
                && bridge_disabled()
            {
                passed += 1;
            }
            FINAL_PREPARED_STEP.store(step as u32, Relaxed);
            if take_final_step(step, false) && !take_final_step(step, false) {
                passed += 1;
            }
            FINAL_PREPARED_STEP.store(step as u32, Relaxed);
            if !take_final_step(if step == 6 { 1 } else { step + 1 }, false)
                && FINAL_PREPARED_STEP.load(Relaxed) == 0
            {
                passed += 1;
            }
            FINAL_PREPARED_STEP.store(step as u32, Relaxed);
            if !take_final_step(step, true) && FINAL_PREPARED_STEP.load(Relaxed) == 0 {
                passed += 1;
            }
            FINAL_PREPARED_STEP.store(step as u32, Relaxed);
            live_stop();
            if !take_final_step(step, false) && bridge_disabled() {
                passed += 1;
            }
        }
        D.current_step.store(saved.0, Relaxed);
        D.rising.store(saved.1, Relaxed);
        OBS_PHYSICAL.store(saved.2, Relaxed);
        COAST_REFERENCE.store(saved.3, Relaxed);
        COAST_STOP.store(saved.4, Relaxed);
        COAST_POLL.store(saved.5, Relaxed);
        POLL_COMS.store(saved.6, Relaxed);
        MODE_CHANGES.store(saved.7, Relaxed);
        FIRST_IRQ_MODE_US.store(saved.8, Relaxed);
        FINAL_PREPARED_USED.store(saved.9, Relaxed);
        passed
    });
    let _ = writeln!(
        out,
        "FINALPREPCHECK passed={} expected=31 injected_metadata=1 enable=0 gate_authority=0",
        passed
    );
}
fn coast_run_inner(
    step: u8,
    hz: u32,
    flying: Option<(flying_acquire::Seed, u16)>,
    dry: Option<(u32, u32, &mut dyn FnMut() -> bool)>,
    resume: Option<powered_guard::Limits>,
    initial: Option<(powered_guard::Feedback, u16)>,
) -> bool {
    let result = coast_run_body(step, hz, flying, dry, resume, initial);
    // All returns, including a refused arm after DMA startup, must release
    // the ADC before later foreground/coast reads. Never do bounded ADCSTOP
    // cleanup in an ISR: physical safing precedes this foreground restoration.
    gates_off();
    set_pin(3, 1, false);
    #[cfg(feature = "bench-dma-feedback")]
    adc_stream::stop();
    result
}
fn coast_run_body(
    step: u8,
    hz: u32,
    flying: Option<(flying_acquire::Seed, u16)>,
    mut dry: Option<(u32, u32, &mut dyn FnMut() -> bool)>,
    resume: Option<powered_guard::Limits>,
    initial: Option<(powered_guard::Feedback, u16)>,
) -> bool {
    #[cfg(feature = "bench-filter-control")]
    {
        FILTER_PREPARED.store(false, Relaxed);
        unsafe {
            FILTER_REFUSAL_START = filtered_irq_hw::refusals();
        }
    }
    unsafe {
        FLY_LATENCY = [u32::MAX; 8];
    }
    flying_mark(0, flying);
    let powered = dry.is_some() && POWER_DUTY.load(Relaxed) != 0;
    let inject = DROP_ARM.swap(false, Relaxed) && powered;
    let inject_at = DROP_AFTER_US.load(Relaxed);
    #[cfg(feature = "bench-normal-restart")]
    let track_inject = TRACK_ARM.swap(false, Relaxed) && powered;
    DROP_ACTIVE.store(false, Relaxed);
    DROP_AT.store(0, Relaxed);
    let safe = if powered {
        powered_timer::ready() && powered_timer::outputs_disabled()
    } else {
        bridge_disabled()
    };
    if !(1..=6).contains(&step) || !(167..=250).contains(&hz) || !safe {
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    let saved = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    // Explicit driven baseline comes only through adopt_driven: old forced
    // timers/COMP/DMA are already stopped, bridge verified clear, new guard
    // configured but not started. Repeating the full shutdown here spends
    // the fresh seed's arm margin on stopping those same owners again.
    // resume_once likewise stopped every owner in prepare before awake
    // acquisition; its explicit remaining-budget token identifies that path.
    if initial.is_some() || resume.is_some() {
        // Private resume_once reaches here only after successful awake
        // acquisition's bridge_clear. Its intervening work checks the original
        // budget and stores metadata: no gate/timer output writes. The live
        // ready/output checks above and fresh guard admission below remain.
        // The opt-in driven A/B also relies on successful acquisition's
        // bridge_clear, after adopt_driven and the physical off checks above.
        // Default and every refusal keep the historical second clear.
        if !((cfg!(feature = "bench-reentry-clear-once") && resume.is_some())
            || (cfg!(feature = "bench-driven-clear-once") && initial.is_some()))
        {
            bridge_clear();
        }
    } else {
        gates_off();
    }
    #[cfg(feature = "bench-reverse-seed-stage")]
    flying_mark(5, flying);
    #[cfg(feature = "bench-pwm-24k")]
    if powered {
        #[cfg(feature = "bench-reentry-pwm-stage")]
        if resume.is_some() && !phase_role_live::recovery_prepared(POWER_DUTY.load(Relaxed)) {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        #[cfg(feature = "bench-reentry-carrier")]
        if resume.is_some() && !phase_role_live::recovery_carrier_ready() {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        if !((cfg!(feature = "bench-reentry-carrier") || cfg!(feature = "bench-reentry-pwm-stage"))
            && resume.is_some())
        {
            if cfg!(feature = "bench-startup-adc") && initial.is_some() {
                if !phase_role_live::adopt_startup_carrier() {
                    observe_end();
                    gates_off();
                    set_pin(3, 1, false);
                    return false;
                }
            } else {
                phase_role_live::prepare_carrier();
            }
        }
    }
    if !powered {
        set_pin(3, 1, false);
    }
    // First driven handoff only: released timer is stopped, all output owners
    // inactive. Resume/other modes do not silently inherit this observation.
    #[cfg(feature = "bench-filter-observe")]
    if powered && initial.is_some() && !filter_observe::prepare() {
        observe_end();
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    #[cfg(feature = "bench-filter-control")]
    if powered {
        // Acquisition is finished and output owners are inactive. Resume may
        // leave the interval timer running; preserve CEN across configuration.
        // The actual measured seed is still applied below, never refreshed.
        let ok = cortex_m::interrupt::free(|_| unsafe {
            if powered_timer::owns()
                || driven_run::owns()
                || core_bench::active()
                || !powered_timer::outputs_disabled()
            {
                return false;
            }
            let t = &*stm32::TIM2::ptr();
            let cen = t.cr1().read().bits() & 1;
            t.cr1().modify(|r, w| w.bits(r.bits() & !1));
            let ok = comp_input::prepare_filtered();
            t.cr1().modify(|r, w| w.bits(r.bits() | cen));
            ok
        });
        if !ok {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        FILTER_PREPARED.store(true, Relaxed);
    }
    #[cfg(feature = "bench-reverse-seed-stage")]
    flying_mark(6, flying);
    if dry.is_none() {
        observe_begin(step, hz, clock_us());
    }
    let requested_polling = COAST_POLL_ARM.swap(false, Relaxed);
    let polling = (requested_polling && flying.is_none())
        || (cfg!(feature = "bench-startup-polling") && initial.is_some() && flying.is_some());
    #[cfg(feature = "bench-final-edge-prepare")]
    let cold_prepared = {
        let prepared = take_final_step(step, polling);
        if resume.is_some() && !prepared {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        FINAL_PREPARED_USED.store(resume.is_some(), Relaxed);
        resume.is_some()
    };
    #[cfg(not(feature = "bench-final-edge-prepare"))]
    let cold_prepared = false;
    if !cold_prepared {
        cold_coast_state(step, polling);
    }
    #[cfg(feature = "bench-reverse-seed-stage")]
    flying_mark(7, flying);
    observation_reset();
    OBS_STATUS.store(1, Relaxed);
    #[cfg(feature = "bench-first-accept")]
    unsafe {
        FIRST_ACCEPT = None;
    }
    #[cfg(feature = "bench-handoff-registers")]
    unsafe {
        HANDOFF_REGISTERS = None;
    }
    SEED_COMMANDED.store(
        cfg!(feature = "bench-startup-bootstrap") && initial.is_some() && flying.is_some(),
        Relaxed,
    );
    // Seed-only work belongs before the final sensing/setup checkpoints, not
    // after they may have captured the next real edge. No timer is armed here.
    let seeded_wait = if let Some((seed, _)) = flying {
        let ci = seed.interval_ticks;
        S.commutation_interval.store(ci, Relaxed);
        S.average_interval.store(ci, Relaxed);
        S.last_average_interval.store(ci, Relaxed);
        for v in &S.interval_hist {
            v.store(ci, Relaxed);
        }
        S.last_zc.store(ci as u16, Relaxed);
        S.this_zc.store(ci as u16, Relaxed);
        let wait =
            minz_core::am32::wait_time(ci, minz_core::am32::advance_of(ci, lp::TEMP_ADVANCE));
        S.wait_time.store(wait as u16, Relaxed);
        OBS_SEED_CI.store(ci, Relaxed);
        // Use the interval count of the selected measured seed, not twelve
        // fabricated crossings: this path now uses twelve measured intervals.
        D.zero_crosses.store(12, Relaxed);
        wait
    } else {
        0
    };
    #[cfg(feature = "bench-reentry-next-edge-live")]
    if resume.is_some() && !flying_bench::setup_follow(3) {
        observe_end();
        gates_off();
        set_pin(3, 1, false);
        return false;
    }
    if flying.is_none() {
        unsafe {
            ACCEPT_TIMELINE = event_timeline::Timeline::new(window_us()).unwrap();
        }
    }
    TIMELINE_REFUSED.store(0, Relaxed);
    flying_mark(1, flying);
    if let Some((run_start, vcal, abort)) = dry.as_mut() {
        if abort() {
            powered_timer::abort();
            observe_end();
            return false;
        }
        let baseline = match initial {
            Some((sample, acquired)) => Some((sample, t17().wrapping_sub(acquired) as u32)),
            None => flying_bench::baseline(*vcal),
        };
        let Some((sample, age)) = baseline else {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        };
        flying_mark(2, flying);
        let started = if let Some(limits) = resume {
            powered_timer::start_reentry(step, sample, age, limits)
        } else {
            powered_timer::start_prepared(
                clock_us().wrapping_sub(*run_start),
                step,
                sample,
                age,
                window_us(),
            )
        };
        if !started {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        #[cfg(feature = "bench-reentry-next-edge-live")]
        if resume.is_some() {
            // E643 admitted an893us-old baseline, then spent its remaining
            // freshness slack in comparator sweeps before starting DMA. Start
            // the sole ADC owner now; never refresh the baseline timestamp.
            if !powered_timer::service_feedback(*vcal) {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                return false;
            }
            #[cfg(feature = "bench-prepared-handoff")]
            if !com_timer::prepare_recovery_counter() {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                return false;
            }
            // Direct mode performs this final sensing in follow_prepared below,
            // after all setup. Keep the same continuous filters and first edge;
            // do not confirm it here and return through another setup frame.
            #[cfg(not(feature = "bench-follow-direct"))]
            if !flying_bench::setup_follow(4) {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                return false;
            }
        }
        #[cfg(feature = "bench-filter-control")]
        if !comp_input::filtered() {
            // A legacy start path that shut down the source may not silently
            // fall back to EXTI and masquerade as a filtered-control result.
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            return false;
        }
        flying_mark(3, flying);
    }
    if let Some((mut seed, origin)) = flying {
        let wait = seeded_wait;
        flying_mark(4, flying);
        #[cfg(feature = "bench-reentry-next-edge-live")]
        if resume.is_some() {
            let Some((_, vcal, abort)) = dry.as_mut() else {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                return false;
            };
            let followed = flying_bench::follow_prepared(seed, origin, *vcal, *abort);
            let Some(next) = followed else {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                COAST_STOP.store(8, Relaxed);
                COAST_DONE.store(true, Relaxed);
                return false;
            };
            if !powered_timer::follow_seed(seed.step, next.step, abort()) {
                observe_end();
                gates_off();
                set_pin(3, 1, false);
                COAST_STOP.store(8, Relaxed);
                COAST_DONE.store(true, Relaxed);
                return false;
            }
            seed = next;
            D.current_step.store(seed.step as u16, Relaxed);
            D.rising.store(seed.step & 1 != 0, Relaxed);
            OBS_PHYSICAL.store(seed.step as u32, Relaxed);
        }
        let armed = cortex_m::interrupt::free(|_| {
            // A guard can expire after follow_seed returns. Do not re-arm a
            // timer/vector after its shutdown, even though commit also refuses.
            #[cfg(feature = "bench-reentry-next-edge-live")]
            if resume.is_some()
                && (!powered_timer::owns()
                    || !powered_timer::ready()
                    || powered_timer::reason() != 0
                    || !powered_timer::outputs_disabled())
            {
                return false;
            }
            // Configure sense mux and vector priorities with dispatch held off.
            #[cfg(feature = "bench-masked-seed-arm")]
            observe_irq_start_inner::<true>();
            #[cfg(not(feature = "bench-masked-seed-arm"))]
            {
                observe_irq_start();
                Comp.mask_interrupts();
            }
            Comp.clear_pending();
            let arm_start = t17();
            let now = arm_start.wrapping_sub(origin) as u32 * 2;
            FLY_ATTEMPT.store(true, Relaxed);
            FLY_AGE.store(now.wrapping_sub(seed.edge_tick), Relaxed);
            FLY_ARR.store(u32::MAX, Relaxed);
            FLY_ARM_US.store(0, Relaxed);
            let handoff = if cfg!(feature = "bench-reverse-arm20") {
                seed.handoff_with_budget::<{ flying_acquire::SEED_MIN_TICKS }, 40>(now, wait)
            } else {
                seed.handoff_with_min::<{ flying_acquire::SEED_MIN_TICKS }>(now, wait)
            };
            let Some((age, arr)) = handoff
            else {
                return false;
            };
            Interval.set_count(age);
            // Driven release stops TIM2 after prepare_driven configured it.
            // Adopt the same PSC31/ARR65535 counter at measured edge age;
            // without CEN, every COMP visit sees a permanently closed gate.
            if initial.is_some() {
                unsafe {
                    (*stm32::TIM2::ptr())
                        .cr1()
                        .modify(|r, w| w.bits(r.bits() | 1));
                }
            }
            FLY_AGE.store(age, Relaxed);
            FLY_ARR.store(arr as u32, Relaxed);
            #[cfg(feature = "bench-prepared-handoff")]
            if resume.is_some() {
                if !com_timer::publish_recovery_deadline(origin, seed.edge_tick.wrapping_add(wait))
                {
                    return false;
                }
            } else {
                com_timer::Timer.set_and_enable(arr);
            }
            #[cfg(not(feature = "bench-prepared-handoff"))]
            com_timer::Timer.set_and_enable(arr);
            // Recorder remains masked until this critical section exits.
            // Put non-sensing setup after ARR programming; include its cost
            // in the unchanged16us arm budget, never refresh the edge age.
            unsafe {
                ACCEPT_TIMELINE = event_timeline::Timeline::new(window_us()).unwrap();
            }
            let arm_us = t17().wrapping_sub(arm_start) as u32;
            FLY_ARM_US.store(arm_us, Relaxed);
            if arm_us > 16 {
                com_timer::Timer::stop();
                return false;
            }
            FLY_SEEDED.store(true, Relaxed);
            #[cfg(feature = "bench-prepared-handoff")]
            if resume.is_some() {
                unsafe {
                    cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM16);
                }
            }
            true
        });
        if !armed {
            observe_end();
            gates_off();
            set_pin(3, 1, false);
            unsafe {
                core::ptr::write_volatile(COMP2_CSR, saved);
            }
            COAST_STOP.store(8, Relaxed);
            COAST_DONE.store(true, Relaxed);
            return false;
        }
        #[cfg(feature = "bench-startup-polling")]
        if polling {
            cortex_m::interrupt::free(|_| {
                Comp.mask_interrupts();
                Comp.clear_pending();
                D.old_routine.store(true, Relaxed);
                D.zcfound.store(false, Relaxed);
                D.bemf_counter.store(0, Relaxed);
                POLL_TIMER.store(true, Relaxed);
            });
            polling_start();
        }
    } else if polling {
        // No interrupts may observe partially reset startup state. Output HAL
        // remains record-only; start_motor owns step advance and interval seed.
        cortex_m::interrupt::free(|_| {
            observe_irq_start();
            Comp.mask_interrupts();
            Comp.clear_pending();
            D.running.store(false, Relaxed);
            D.old_routine.store(true, Relaxed);
            D.zero_crosses.store(0, Relaxed);
            D.zcfound.store(false, Relaxed);
            D.zcfr_guard_hits.store(0, Relaxed);
            minz_core::am32_control::start_motor(&S.sched(), &D.drive(), &mut motor(), &observer());
            POLL_TIMER.store(true, Relaxed);
        });
        polling_start();
    } else {
        observe_irq_start();
    }
    loop {
        // Keep the foreground campaign clock extended during a long handoff.
        // Never call this foreground-owned clock from COMP/COM/guard ISRs.
        let _ = clock_us();
        if let Some((_, vcal, abort)) = dry.as_mut() {
            if abort() {
                powered_timer::abort();
            }
            if !powered_timer::owns() {
                COAST_STOP.store(7, Relaxed);
                break;
            }
            // One foreground-only snapshot after the first COM. No ISR hook,
            // UART write, interrupt mask, or continuously running recorder.
            #[cfg(feature = "bench-handoff-registers")]
            unsafe {
                if core::ptr::addr_of!(HANDOFF_REGISTERS).read().is_none()
                    && COMMUTATIONS.load(Relaxed) != 0
                {
                    let t = &*stm32::TIM2::ptr();
                    let e = &*stm32::EXTI::ptr();
                    HANDOFF_REGISTERS = Some([
                        t.cnt().read().bits(),
                        t.cr1().read().bits(),
                        core::ptr::read_volatile(COMP2_CSR),
                        e.imr1().read().bits(),
                        e.rpr1().read().bits(),
                        e.fpr1().read().bits(),
                        core::ptr::read_volatile(0xE000E100 as *const u32),
                        MASKED.load(Relaxed) as u32,
                        D.old_routine.load(Relaxed) as u32,
                        D.current_step.load(Relaxed) as u32,
                    ]);
                }
            }
            if !powered_timer::service_feedback(*vcal) {
                COAST_STOP.store(7, Relaxed);
                break;
            }
            #[cfg(feature = "bench-reverse-blank")]
            reverse_blank_poll();
            #[cfg(any(
                feature = "bench-bemf-level-revisit",
                feature = "bench-running-level-revisit"
            ))]
            revisit_low_speed_level();
        }
        let elapsed = observation_elapsed();
        #[cfg(feature = "bench-normal-restart")]
        if track_inject && elapsed >= inject_at {
            TRACK_AT.store(elapsed, Relaxed);
            if !powered_timer::inject_tracking() {
                COAST_STOP.store(8, Relaxed);
            } else {
                COAST_STOP.store(7, Relaxed);
            }
            break;
        }
        if inject && elapsed >= inject_at && !DROP_ACTIVE.load(Relaxed) {
            cortex_m::interrupt::free(|_| {
                // Suppress sensing only. Pending one-shot COM may still fire;
                // its enable_interrupts cannot undo the injection. TIM6 guard
                // remains live, with unchanged accepted-event/feedback ages.
                DROP_ACTIVE.store(true, Relaxed);
                Comp.mask_interrupts();
                Comp.clear_pending();
                DROP_AT.store(observation_elapsed(), Relaxed);
            });
        }
        let reason = if POWER_DUTY.load(Relaxed) == 0 && !bridge_disabled() {
            4
        } else if !get_idr(1, 14) {
            5
        } else if OBS_STATUS.load(Relaxed) != 1 {
            6
        } else if !D.running.load(Relaxed) {
            3
        } else if D.old_routine.load(Relaxed) && !polling {
            2
        } else if elapsed >= window_us() {
            1
        } else {
            0
        };
        if reason != 0 {
            COAST_STOP.store(reason, Relaxed);
            break;
        }
        observe_bands();
    }
    observe_end();
    gates_off();
    set_pin(3, 1, false);
    #[cfg(feature = "bench-dma-feedback")]
    adc_stream::stop();
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, saved);
    }
    if inject && DROP_AT.load(Relaxed) != 0 && powered_timer::reason() == 8 {
        // COM/COMP execution has ended and the driver is disabled. Read actual
        // comparator levels directly; no synthetic events or new authority.
        if freeze_first_segment() && !REENTRY_SESSION.load(Relaxed) {
            flying_bench::reacquire_disabled();
        }
    }
    COAST_DONE.store(true, Relaxed);
    true
}
/// Compact post-stop controller estimate, not independent rotor/lock proof.
pub fn terminal_summary<W: Write>(out: &mut W) {
    let interval = S.average_interval.load(Relaxed);
    let ehz = if interval > 0 && interval <= 65535 {
        2_000_000 / (6 * interval)
    } else {
        0
    };
    let _ = writeln!(
        out,
        "BEMFSTOP core_stop={} average_half_us={} estimated_ehz={} com={} lock_proven=0",
        COAST_STOP.load(Relaxed),
        interval,
        ehz,
        COMMUTATIONS.load(Relaxed)
    );
    #[cfg(feature = "bench-revisit-origin")]
    {
        let counts = unsafe { core::ptr::addr_of!(REVISIT_ORIGIN_COUNTS).read() };
        let _ = writeln!(
            out,
            "REVISITORIGIN duty_tenths={} physical_only={} software_only={} both={} neither={} late_physical={} late_software={} late_both={} late_neither={} late_rule=measured_gt_prior_average_plus_quarter flags_at_dispatch=1 both_ambiguous=1 postrun_only=1",
            REVISIT_ORIGIN_DUTY.load(Relaxed),
            counts[0][0], counts[0][1], counts[0][2], counts[0][3],
            counts[1][0], counts[1][1], counts[1][2], counts[1][3],
        );
    }
}
/// Compile-time lean-path witness, printed only after outputs are safe.
#[cfg(feature = "bench-lean-core")]
pub fn lean_marker<W: Write>(out: &mut W) {
    let _ = writeln!(
        out,
        "LEANCORE r1 recorder=0 comp_max=0 com_max=0 control_progress=1 lean_irq={}",
        cfg!(feature = "bench-lean-irq") as u8
    );
}
#[cfg(feature = "bench-running-level-revisit")]
pub fn running_revisit_counts() -> (u32, u32) {
    (LEVEL_REVISIT_ATTEMPTS.load(Relaxed), LEVEL_REVISIT_ACCEPTS.load(Relaxed))
}
#[cfg(feature = "bench-reverse-comp-dma-peer")]
pub fn reverse_priority_summary<W: Write>(out: &mut W) {
    // Post-stop only. Verify the actual NVIC map, not just the cfg label.
    let _ = writeln!(
        out,
        "COREPRIORITY comp={} com={} guard={} dma={} postrun_only=1",
        cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::ADC_COMP),
        cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM16),
        cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM6_DAC_LPTIM1),
        cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::DMA1_CHANNEL1),
    );
}
#[cfg(feature = "bench-interval-tail")]
pub fn interval_tail_dump<W: Write>(out: &mut W) {
    let total = unsafe { INTERVAL_TOTAL };
    let len = total.min(128);
    let first = total.saturating_sub(len);
    #[cfg(feature = "bench-revisit-origin-tail")]
    let _ = writeln!(
        out,
        "INTERVALTAIL n={} skipped={} total={} fields=ordinal_lo,ordinal_hi,gap_us,measured_interval_half_us,average_interval_half_us,event_limit_us+origin2 two_per_row=1 wire=it87-v3 diagnostic_only=1",
        len, first, total
    );
    #[cfg(not(feature = "bench-revisit-origin-tail"))]
    let _ = writeln!(
        out,
        "INTERVALTAIL n={} skipped={} total={} fields=ordinal_lo,ordinal_hi,gap_us,measured_interval_half_us,average_interval_half_us,event_limit_us two_per_row=1 wire=it86-v2 diagnostic_only=1",
        len, first, total
    );
    let mut offset = 0u32;
    while offset < len {
        let mut row = [0u16; 10];
        let ordinal = first + offset;
        row[0] = ordinal as u16;
        row[1] = (ordinal >> 16) as u16;
        for j in 0..2u32 {
            if offset + j >= len { break; }
            let source = unsafe { INTERVAL_TAIL[((first + offset + j) as usize) & 127] };
            for k in 0..4usize { row[2 + j as usize * 4 + k] = source[k]; }
        }
        let _ = snapshot::record(
            out,
            if cfg!(feature = "bench-revisit-origin-tail") { "IT87" } else { "IT86" },
            &row,
        );
        offset += 2;
    }
}
pub fn observe_summary<W: Write>(out: &mut W, capture: bool) {
    #[cfg(feature = "bench-persistence-hist")]
    if capture {
        let duty = unsafe { core::ptr::addr_of!(PERSIST_DUTY).read() };
        let accept = unsafe { core::ptr::addr_of!(PERSIST_ACCEPT).read() };
        let reject = unsafe { core::ptr::addr_of!(PERSIST_REJECT).read() };
        let _ = writeln!(
            out,
            "PERSISTHIST duty_tenths={} fields=sector,accepted,reject_index0..11 diagnostic_only=1",
            duty
        );
        for sector in 0..6 {
            let _ = write!(out, "PH {} {}", sector + 1, accept[sector]);
            for count in reject[sector] {
                let _ = write!(out, " {}", count);
            }
            let _ = out.write_char('\n');
        }
    }
    #[cfg(feature = "bench-interval-tail")]
    if capture { interval_tail_dump(out); }
    #[cfg(any(
        feature = "bench-bemf-level-revisit",
        feature = "bench-running-level-revisit"
    ))]
    if capture {
        let _ = writeln!(
            out,
            "LEVELREVISIT attempts={} accepts={} one_per_command=1 half_interval_gate=1 real_level=1 persistence_retained=1 high_speed={}",
            LEVEL_REVISIT_ATTEMPTS.load(Relaxed),
            LEVEL_REVISIT_ACCEPTS.load(Relaxed),
            cfg!(feature = "bench-running-level-revisit") as u8
        );
    }
    #[cfg(feature = "bench-startup-bootstrap")]
    if capture {
        let a = unsafe { FLY_LATENCY };
        let _ = writeln!(
            out,
            "STARTUPSETUP entry={} reset={} feedback={} guard={} reference={} half_us=1",
            a[0], a[1], a[2], a[3], a[4]
        );
    }
    #[cfg(feature = "bench-lean-core")]
    lean_marker(out);
    #[cfg(feature = "bench-comp-hyst-low")]
    if capture {
        let _ = writeln!(
            out,
            "COMPHYST code={} startup_and_bemf=1 raw_reads_unchanged=1",
            unsafe { (core::ptr::read_volatile(COMP2_CSR) >> 16) & 3 }
        );
    }
    #[cfg(feature = "bench-com-peer")]
    if capture {
        let _ = writeln!(
            out,
            "COREPRIORITY comp={} com={} guard={} dma={}",
            cortex_m::peripheral::NVIC::get_priority(if cfg!(feature = "bench-filter-control") {
                stm32::Interrupt::TIM2
            } else {
                stm32::Interrupt::ADC_COMP
            }),
            cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM16),
            cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM6_DAC_LPTIM1),
            cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::DMA1_CHANNEL1)
        );
    }
    #[cfg(feature = "bench-filter-latency")]
    if capture {
        filter_latency::dump(out);
    }
    #[cfg(feature = "bench-filter-control")]
    if capture {
        filtered_irq_hw::dump_stop(out);
        let now = filtered_irq_hw::refusals();
        let before = unsafe { FILTER_REFUSAL_START };
        let _ = writeln!(
            out,
            "FILTERCONTROL prepared={} code={} settle_us={} active={} refusals={},{},{},{}",
            FILTER_PREPARED.load(Relaxed) as u8,
            filtered_irq_hw::FILTER_CODE,
            comp_input::FILTER_SETTLE_US,
            comp_input::filtered() as u8,
            now[0].saturating_sub(before[0]),
            now[1].saturating_sub(before[1]),
            now[2].saturating_sub(before[2]),
            now[3].saturating_sub(before[3])
        );
    }
    #[cfg(feature = "bench-filter-observe")]
    if capture {
        filter_observe::dump(out);
    }
    #[cfg(feature = "bench-cpu-timing")]
    if capture {
        cpu_meter::dump(out);
    }
    if capture {
        timeline_summary(out, "ET85", unsafe {
            &*core::ptr::addr_of!(ACCEPT_TIMELINE)
        });
        let _ = writeln!(out, "TIMELINEREFUSED n={}", TIMELINE_REFUSED.load(Relaxed));
    }
    if capture {
        let _ = writeln!(
            out,
            "RECORDGUARD late_accepts={} max_us={} epoch_closed_at_first_stop=1",
            LATE_ACCEPTS.load(Relaxed),
            RECORD_MAX_US.load(Relaxed)
        );
    }
    let retry = unsafe { REENTRY_REPORT };
    if retry[0] != 0 {
        let _ = writeln!(
            out,
            "REENTRYRESERVE us={} included_in_original_deadline=1",
            REENTRY_RESERVE.load(Relaxed)
        );
    }
    if retry[0] != 0 {
        let _ = writeln!(
            out,
            "REENTRY result={} first_injection_us={} resume_elapsed_us={} remaining_us={} final_elapsed_us={} original_end_elapsed_us={} attempts_max=1",
            retry[0], retry[1], retry[2], retry[3], retry[4], retry[5]
        );
    }
    if FIRST_FREEZE_ERROR.load(Relaxed) != 0 {
        let _ = writeln!(out, "FIRSTSEG error={}", FIRST_FREEZE_ERROR.load(Relaxed));
    }
    if capture {
        if let Some(first) = unsafe { (&*core::ptr::addr_of!(FIRST_SEGMENT)).get() } {
            timeline_summary(out, "FT85", &first.timeline);
            let _ = writeln!(
                out,
                "FIRSTSEG fault={} stop_us={} commits={} scans={} peak_raw={} bus_min={} end_us={} events={} prefix={} tail={} skipped={}",
                first.power[0],
                first.power[1],
                first.power[2],
                first.power[3],
                first.power[4],
                first.power[5],
                first.end_us,
                first.stats.events,
                first.prefix_len,
                first.tail.len(),
                first.tail.skipped()
            );
            for row in &first.prefix[..first.prefix_len] {
                let _ = snapshot::record(out, "P185", row);
            }
            for i in 0..first.tail.len() {
                let _ = snapshot::record(out, "T185", &first.tail.row(i));
            }
        }
    }
    flying_bench::recovery_summary(out, capture);
    if DROP_AT.load(Relaxed) != 0 {
        let _ = writeln!(
            out,
            "DROPOUT applied=1 at_us={} scheduled_us={} suppress_until_stop=1 automatic_restart=0",
            DROP_AT.load(Relaxed),
            DROP_AFTER_US.load(Relaxed)
        );
    }
    #[cfg(feature = "bench-normal-restart")]
    if TRACK_AT.load(Relaxed) != 0 {
        let _ = writeln!(
            out,
            "TRACKINJECT applied=1 at_us={} scheduled_us={} immediate_tracking_trip=1 stalled_sector_omitted=1",
            TRACK_AT.load(Relaxed),
            DROP_AFTER_US.load(Relaxed)
        );
    }
    if capture && COAST_REFERENCE.load(Relaxed) && COAST_POLL.load(Relaxed) {
        let _ = writeln!(
            out,
            "COREHISTORY n={} drop={} fields=us,kind,step,event_interval,average,previous_average,polling,zero_crosses wire=h85-v1",
            HISTORY_N.load(Relaxed),
            HISTORY_DROP.load(Relaxed)
        );
        for i in 0..HISTORY_N.load(Relaxed) as usize {
            let row = unsafe {
                core::ptr::addr_of!(HISTORY)
                    .cast::<[u16; 8]>()
                    .add(i)
                    .read()
            };
            let _ = snapshot::record(out, "H85", &row);
        }
        let _ = writeln!(out, "COREHISTORY END");
    }
    #[cfg(feature = "bench-first-accept")]
    if let Some(row) = unsafe { FIRST_ACCEPT } {
        let _ = writeln!(
            out,
            "FIRSTACCEPT step={} interval_ticks={} seed_ticks={} half_us=1 diagnostic=1 after_persistence=1",
            row[0], row[1], row[2]
        );
    }
    #[cfg(feature = "bench-handoff-registers")]
    if let Some(row) = unsafe { HANDOFF_REGISTERS } {
        let _ = writeln!(
            out,
            "HANDOFFREG cnt={} cr1={:08X} csr={:08X} imr={:08X} rpr={:08X} fpr={:08X} nvic={:08X} masked={} polling={} step={} foreground_once=1",
            row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7], row[8], row[9]
        );
    }
    if capture && DSY_CAPTURED.load(Relaxed) {
        let s = unsafe { core::ptr::addr_of!(DSY_STATE).read() };
        let _ = writeln!(
            out,
            "COASTDSY us={} previous_average_ticks={} average_ticks={} interval_ticks={} acquisition=EV_DSY before_previous_update=1",
            s[0], s[1], s[2], s[3]
        );
    }
    if capture {
        if let Some(s) = unsafe { CYCLE_CORE } {
            let _ = writeln!(
                out,
                "CYCLECORE step={} rising={} average_ticks={} previous_average_ticks={} interval_ticks={} this_zc_ticks={} last_zc_ticks={} wait_ticks={} filter={} zero_crosses={} polling={} running={} after_safing=1 before_ev_acc_return=1",
                s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11]
            );
        }
    }
    if COAST_REFERENCE.load(Relaxed) && COAST_POLL.load(Relaxed) {
        let _ = writeln!(
            out,
            "COASTPOLL startup_reference=1 startup_interval_ticks=10000 startup_counter_ticks=5000 polling_coms={} to_irq={} first_irq_mode_us={} wait_guard_hits={} recovery={} desync={} polling_ticks={} max_call_us={} max_gap_us={} gate_authority=0",
            POLL_COMS.load(Relaxed),
            MODE_CHANGES.load(Relaxed),
            FIRST_IRQ_MODE_US.load(Relaxed),
            D.zcfr_guard_hits.load(Relaxed),
            D.bemf_timeout_happened.load(Relaxed),
            D.desync_happened.load(Relaxed),
            OBS_POLLS.load(Relaxed),
            OBS_COST.load(Relaxed),
            POLL_GAP.load(Relaxed)
        );
    }
    let _ = writeln!(
        out,
        "COREDISPATCH software_masked={} hardware_masked={} not_pending={} source_guard=1",
        DISPATCH_SKIP[0].load(Relaxed),
        DISPATCH_SKIP[1].load(Relaxed),
        DISPATCH_SKIP[2].load(Relaxed)
    );
    if capture && DISPATCH_SNAPSHOT_VALID.load(Relaxed) {
        let s = unsafe { core::ptr::addr_of!(DISPATCH_SNAPSHOT).read() };
        let _ = writeln!(
            out,
            "DISPATCHSTATE first_skip_us={} exti_imr={} rising_pending={} falling_pending={} adc_isr={} adc_ier={} comp1_csr={} comp2_csr={} before_backstop=1",
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]
        );
    }
    if COAST_REFERENCE.load(Relaxed) {
        if capture {
            let s = unsafe { core::ptr::addr_of!(COAST_END_STATE).read() };
            let _ = writeln!(
                out,
                "COASTSTATE step={} expected={} csr={} pending={} exti_imr={} interval_cnt={} com_dier={} last_irq_us={} last_gate={} last_reads={} last_level={} masked={} before_irq_shutdown=1",
                s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], s[8], s[9], s[10], s[11]
            );
        }
        let _ = writeln!(
            out,
            "COASTREF stop={} max_us={} gates_disabled={} sense_mux_only={} desync={} polling={} running={} later_coast_samples_delayed=1",
            COAST_STOP.load(Relaxed),
            window_us(),
            bridge_disabled() as u8,
            (POWER_DUTY.load(Relaxed) == 0) as u8,
            D.desync_happened.load(Relaxed),
            D.old_routine.load(Relaxed) as u8,
            D.running.load(Relaxed) as u8
        );
    }
    if LIVE_IRQ.load(Relaxed) {
        // Report-only test envelope, NOT a motor abort or qualified lock band.
        let mut monitor = accepted_timing::Monitor::new(0, 1333, 2000);
        let n = ACCEPT_N.load(Relaxed) as usize;
        let mut fault = None;
        if capture {
            let _ = writeln!(
                out,
                "ACCEPTLOG n={} drop={} fields=us_lo,us_hi,step,reference_interval_ticks acquisition=EV_ACC prefix_only={}",
                n,
                ACCEPT_DROP.load(Relaxed),
                (!COAST_REFERENCE.load(Relaxed)) as u8
            );
        }
        for i in 0..n {
            let row = unsafe {
                core::ptr::addr_of!(ACCEPT_LOG)
                    .cast::<[u16; 4]>()
                    .add(i)
                    .read()
            };
            fault = monitor.event(
                ((row[0] as u32) | ((row[1] as u32) << 16)) * 2,
                row[2] as u8,
            );
            if capture {
                let _ = snapshot::record(out, "A85", &row);
            }
        }
        let end = PREFIX_END_US.load(Relaxed);
        if capture {
            // Prefix overflow stays explicit for old replay tools. The suffix
            // is a separate stream; never join across an omitted middle gap.
            let tail = unsafe { &*core::ptr::addr_of!(ACCEPT_TAIL) };
            let _ = writeln!(
                out,
                "ACCEPTTAIL n={} skipped={} total={} fields=us_lo,us_hi,step,reference_interval_ticks wire=t85-v1 windows_only=1",
                tail.len(),
                tail.skipped(),
                n as u32 + ACCEPT_DROP.load(Relaxed)
            );
            for i in 0..tail.len() {
                let _ = snapshot::record(out, "T85", &tail.row(i));
            }
        }
        let stats = unsafe { &*core::ptr::addr_of!(ACCEPT_STATS) };
        let _ = writeln!(
            out,
            "ACCEPTQUALITY events={} first_us={} last_us={} order_bad={} end_us={} lock_proven=0",
            stats.events, stats.first, stats.last, stats.order_bad, end
        );
        for (name, m) in [("gap", stats.gaps), ("cycle", stats.cycles)] {
            let _ = writeln!(
                out,
                "ACCEPTMOMENTS kind={} n={} min_us={} max_us={} sum_us={} squares_us={} excluded={}",
                name,
                m.n,
                if m.n == 0 { 0 } else { m.min },
                m.max,
                m.sum,
                m.squares,
                m.excluded
            );
        }
        if end != 0 {
            fault = monitor.poll(end * 2);
        }
        let code = match fault {
            None => 0,
            Some(accepted_timing::Fault::Stale) => 1,
            Some(accepted_timing::Fault::TooFast) => 2,
            Some(accepted_timing::Fault::SectorOrder) => 3,
        };
        let _ = writeln!(
            out,
            "ACCEPTTIMING fault={} intervals={} observed_end_us={} min_ticks=1333 max_ticks=2000 report_only=1 continuous={} lock_proven=0",
            code,
            n.saturating_sub(1),
            end,
            COAST_REFERENCE.load(Relaxed) as u8
        );
    }
    let _ = writeln!(
        out,
        "COREPOL live_raw_inverted={} physical_raw_inverted={} irq_trace_levels=reference_hal raw_observation_unchanged=1",
        LIVE_IRQ.load(Relaxed) as u8,
        PHYSICAL_OBSERVATION.load(Relaxed) as u8
    );
    if FLY_ATTEMPT.load(Relaxed) {
        if capture {
            let a = unsafe { FLY_LATENCY };
            let _ = writeln!(
                out,
                "SEEDLAT entry_ticks={} reset_ticks={} feedback_ticks={} guard_ticks={} reference_ticks={} half_us=1",
                a[0], a[1], a[2], a[3], a[4]
            );
        }
        let commanded = SEED_COMMANDED.load(Relaxed);
        let _ = writeln!(
            out,
            "CORESEED assumed={} source={} interval_ticks={} edge_age_ticks={} remaining_arr={} arm_us={} bootstrap_com=1 synthetic_accept=0",
            commanded as u8,
            if commanded {
                "commanded_startup"
            } else {
                "measured_flying"
            },
            OBS_SEED_CI.load(Relaxed),
            FLY_AGE.load(Relaxed),
            FLY_ARR.load(Relaxed),
            FLY_ARM_US.load(Relaxed)
        );
        let _ = writeln!(
            out,
            "CORESEED armed={} refusal_stop_code=8",
            FLY_SEEDED.load(Relaxed) as u8
        );
    } else if LIVE_IRQ.load(Relaxed) && !(COAST_REFERENCE.load(Relaxed) && COAST_POLL.load(Relaxed))
    {
        let ci = OBS_SEED_CI.load(Relaxed);
        let wait =
            minz_core::am32::wait_time(ci, minz_core::am32::advance_of(ci, lp::TEMP_ADVANCE));
        let _ = writeln!(
            out,
            "CORESEED assumed=1 initial_wait_ticks={} initial_elapsed_ticks={} source=reference_com_wait_plus_one",
            wait,
            wait + 1
        );
    }
    let _ = writeln!(
        out,
        "CORETRACE enabled={} per_read_bookkeeping={}",
        TRACE_ENABLED.load(Relaxed) as u8,
        TRACE_ENABLED.load(Relaxed) as u8
    );
    #[cfg(feature = "bench-quiet-irq-stamp")]
    let _ = writeln!(
        out,
        "IRQSTAMP omitted={} report_only=1 accepted_clock_unchanged=1 safety_clocks_unchanged=1",
        (POWER_DUTY.load(Relaxed) != 0 && !TRACE_ENABLED.load(Relaxed)) as u8
    );
    atomic_check::marker(out);
    #[cfg(feature = "bench-comp-paths")]
    comp_path_live::dump(out, capture, ACCEPTS.load(Relaxed));
    #[cfg(feature = "bench-qualification-direct")]
    {
        if capture {
            let _ = writeln!(
                out,
                "DIRECTBIND epoch={} accepted={} final_observation=1 ram={}",
                DIRECT_OBSERVATION_EPOCH.load(Relaxed),
                ACCEPTS.load(Relaxed),
                cfg!(feature = "bench-direct-ram") as u8
            );
        }
        qualification_direct_live::dump(out, capture, ACCEPTS.load(Relaxed));
    }
    #[cfg(feature = "bench-qualification-window")]
    qualification_live::dump(out, capture);
    #[cfg(feature = "bench-qualification-sparse")]
    {
        if capture {
            let _ = writeln!(
                out,
                "SPARSEBIND epoch={} accepted={} final_observation=1 ram={}",
                SPARSE_OBSERVATION_EPOCH.load(Relaxed),
                ACCEPTS.load(Relaxed),
                cfg!(feature = "bench-sparse-ram") as u8
            );
        }
        qualification_sparse_live::dump(out, capture);
    }
    #[cfg(feature = "bench-pwm-roles")]
    let _ = writeln!(
        out,
        "PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz={}",
        phase_role_live::CARRIER.hz_floor()
    );
    if cfg!(feature = "bench-cached-comp") {
        let _ = writeln!(
            out,
            "COMPMODE cached_per_call=1 signal_cached=0 safety_cached=0"
        );
    }
    #[cfg(feature = "bench-inline-comp")]
    let _ = writeln!(
        out,
        "COMPREAD inline_adapter=1 sample_count_unchanged=1 signal_cached=0"
    );
    #[cfg(feature = "bench-static-comp")]
    let _ = writeln!(
        out,
        "COMPSTATIC real_inverted_traceoff=1 live_reads=1 reference_core=1 diagnostic_fallback=1"
    );
    #[cfg(feature = "bench-comp-read-call")]
    let _ = writeln!(
        out,
        "COMPREADCALL noninline=1 live_read=1 read_count_unchanged=1 timing_equivalence_proven=0"
    );
    #[cfg(feature = "bench-com-keep-running")]
    let _ = writeln!(
        out,
        "COMARM running_counter=1 stale_pending_cleared=1 stopped_start_retained=1 experimental=1"
    );
    #[cfg(feature = "bench-comp-critical")]
    let _ = writeln!(
        out,
        "COMPCRITICAL calls={} max_us={} refused={} filter_required=12 limit_us=60 restores_between_calls=1 recorder_inside=1",
        CRITICAL_CALLS.load(Relaxed),
        CRITICAL_MAX.load(Relaxed),
        CRITICAL_REFUSED.load(Relaxed)
    );
    #[cfg(feature = "bench-inline-guard")]
    let _ = writeln!(
        out,
        "GUARDCODE inline_constructor=1 admission_unchanged=1 prestaged=0"
    );
    #[cfg(feature = "bench-guard-install")]
    let _ = writeln!(
        out,
        "GUARDINSTALL admission_token=1 fresh_checks=1 in_place=1 prestaged=0"
    );
    #[cfg(feature = "bench-seed-div12")]
    let _ = writeln!(
        out,
        "SEEDMATH div12_bound=24000 exact=1 fallback=1 qualification_unchanged=1"
    );
    #[cfg(all(feature = "bench-seed400", not(feature = "bench-seed450")))]
    {
        const _: () = assert!(
            flying_acquire::SEED_MIN_TICKS == 834
                && flying_acquire::CYCLE_MIN_TICKS == 5000
                && flying_acquire::INDIVIDUAL_MIN_TICKS == 476
        );
        let _ = writeln!(
            out,
            "SEEDPROFILE min_ticks=834 cycle_ticks=5000 individual_ticks=476 remaining_ticks=64 arm_max_us=16 shared_acquisition=1"
        );
    }
    #[cfg(feature = "bench-final-edge-prepare")]
    let _ = writeln!(
        out,
        "FINALPREP used={} interval=11 output_authority=0 final_edge_checks=1",
        FINAL_PREPARED_USED.load(Relaxed) as u8
    );
    #[cfg(feature = "bench-reentry-clear-once")]
    let _ = writeln!(
        out,
        "REENTRYCLEAR acquisition_clear=1 duplicate_omitted=1 live_checks=1 initial_unchanged=1"
    );
    #[cfg(feature = "bench-masked-seed-arm")]
    let _ = writeln!(
        out,
        "SEEDMASK setup_masked=1 software_latch=1 bootstrap_enable_unchanged=1 age_floor_unchanged=1"
    );
    #[cfg(feature = "bench-prepared-handoff")]
    let _ = writeln!(out, "PREPAREDHANDOFF floor=64 arm=16 priority=0");
    #[cfg(feature = "bench-follow-prevalidate")]
    let _ = writeln!(out, "FOLLOWPREVALIDATE v1");
    #[cfg(feature = "bench-follow-direct")]
    let _ = writeln!(out, "FOLLOWDIRECT v1");
    #[cfg(feature = "bench-follow-setup-phase")]
    let _ = writeln!(out, "FOLLOWSETUP expected");
    #[cfg(feature = "bench-follow-persistence")]
    let _ = writeln!(out, "FOLLOWPERSIST reads=12 sampled_dwell=0");
    #[cfg(all(feature = "bench-seed450", not(feature = "bench-seed500")))]
    let _ = writeln!(
        out,
        "SEEDPROFILE min_ticks=741 cycle_ticks=4445 individual_ticks=476 remaining_ticks=64 arm_max_us=16 shared_acquisition=1"
    );
    #[cfg(feature = "bench-seed500")]
    let _ = writeln!(
        out,
        "SEEDPROFILE min_ticks=667 cycle_ticks=4000 individual_ticks=476 remaining_ticks=64 arm_max_us=16 shared_acquisition=1"
    );
    #[cfg(feature = "bench-reentry-carrier")]
    let _ = writeln!(
        out,
        "REENTRYCARRIER prepared_before_edge=1 live_period_check=1 output_authority_retained=0 failure_reset_unchanged=1"
    );
    #[cfg(feature = "bench-pwm-roles")]
    let _ = writeln!(
        out,
        "CARRIERMATH compare_at_prepare=1 steady_divide=0 exact_compare=1 duty_change_refused=1"
    );
    #[cfg(feature = "bench-adc-phase")]
    let _ = writeln!(
        out,
        "BINMATH adc_phase_divide=0 timeline_index_divide=0 exact=1 timestamps_unchanged=1"
    );
    let _ = writeln!(
        out,
        "COREBOUNDARY wait_us={} request_time_origin=fresh_sector selected_sector={}",
        BOUNDARY_DELAY.load(Relaxed),
        SELECTED_SECTOR.load(Relaxed)
    );
    let _ = writeln!(
        out,
        "COREEXTI live={} irq_calls={} comp_max_us={} com_max_us={} events={} synthetic=0",
        LIVE_IRQ.load(Relaxed) as u8,
        COMP_CALLS.load(Relaxed),
        COMP_MAX.load(Relaxed) & 65535,
        COM_MAX.load(Relaxed),
        EVENTS.load(Relaxed)
    );
    if POWER_DUTY.load(Relaxed) != 0 {
        let limit = reverse_rate_limit();
        let _ = writeln!(out, "IRQRATE peak={} limit={} bucket_us=1000", unsafe {
            (*core::ptr::addr_of!(IRQ_RATE)).peak
        }, limit);
    }
    let _ = writeln!(
        out,
        "COREPOLL timer={} target_period_us=50 max_gap_us={} direct_comp={}",
        POLL_TIMER.load(Relaxed) as u8,
        POLL_GAP.load(Relaxed),
        POLL_TIMER.load(Relaxed) as u8
    );
    let _ = writeln!(
        out,
        "COREOBS status={} polls={} max_call_us={} seed_ci={} physical_step={} requested_step={} request_us={} average={} com={} gate_authority={} sample_cadence_only={} wait_wave_updates={}",
        OBS_STATUS.load(Relaxed),
        OBS_POLLS.load(Relaxed),
        OBS_COST.load(Relaxed),
        OBS_SEED_CI.load(Relaxed),
        OBS_PHYSICAL.load(Relaxed),
        D.current_step.load(Relaxed),
        OBS_REQUEST_US.load(Relaxed),
        S.average_interval.load(Relaxed),
        COMMUTATIONS.load(Relaxed),
        (POWER_DUTY.load(Relaxed) != 0) as u8,
        (!POLL_TIMER.load(Relaxed) && !COAST_REFERENCE.load(Relaxed)) as u8,
        OBS_WAIT_UPDATES.load(Relaxed)
    );
    #[cfg(feature = "bench-average-diagnostic")]
    let _ = writeln!(
        out,
        "LIVEPARAM filter={} advance={} filter_source=am32_foreground advance_source={}",
        D.filter_level.load(Relaxed),
        live_advance_level(),
        if cfg!(feature = "bench-advance-scheduled") {
            "scheduled"
        } else {
            "fixed"
        }
    );
}

fn timeline_summary<W: Write>(out: &mut W, label: &str, t: &event_timeline::Timeline) {
    let _ = writeln!(
        out,
        "TIMELINE label={} bins=6 window_us={} bin_us={} events={} overrun_events={} accepted_only=1",
        label,
        t.window_us,
        t.bin_us,
        t.total(),
        t.overrun_events
    );
    for (i, b) in t.bins.iter().enumerate() {
        let _ = snapshot::record(
            out,
            label,
            &[
                i as u16,
                b.events as u16,
                (b.events >> 16) as u16,
                b.min_gap,
                b.max_gap,
            ],
        );
    }
}
/// Exercise the real recorder with the stopped flags a preempted callback sees.
/// No NVIC unmask, output authority or synthetic controller acceptance.
#[inline(never)]
pub fn recordcheck<W: Write>(out: &mut W) {
    if ACTIVE.load(Relaxed) || powered_timer::owns() || !bridge_disabled() {
        let _ = writeln!(out, "RECORDCHECK refused=1 gate_authority=0");
        return;
    }
    comp_input::stop();
    com_timer::Timer::stop();
    let (passed, rejected) = cortex_m::interrupt::free(|_| {
        let saved = (
            LIVE_IRQ.load(Relaxed),
            OBS_STATUS.load(Relaxed),
            POWER_DUTY.load(Relaxed),
            LATE_ACCEPTS.load(Relaxed),
            RECORD_MAX_US.load(Relaxed),
        );
        let before = (
            PREFIX_END_US.load(Relaxed),
            ACCEPT_N.load(Relaxed),
            ACCEPT_DROP.load(Relaxed),
            ACCEPTS.load(Relaxed),
            EVENTS.load(Relaxed),
            unsafe { ACCEPT_STATS.events },
        );
        LIVE_IRQ.store(true, Relaxed);
        OBS_STATUS.store(1, Relaxed);
        for duty in [0, 45] {
            POWER_DUTY.store(duty, Relaxed);
            for sector in 0..6 {
                Obs.record(minz_core::blackbox::EV_ACC, sector, 1400);
            }
        }
        let after = (
            PREFIX_END_US.load(Relaxed),
            ACCEPT_N.load(Relaxed),
            ACCEPT_DROP.load(Relaxed),
            ACCEPTS.load(Relaxed),
            EVENTS.load(Relaxed),
            unsafe { ACCEPT_STATS.events },
        );
        let rejected = LATE_ACCEPTS.load(Relaxed).wrapping_sub(saved.3);
        LIVE_IRQ.store(saved.0, Relaxed);
        OBS_STATUS.store(saved.1, Relaxed);
        POWER_DUTY.store(saved.2, Relaxed);
        LATE_ACCEPTS.store(saved.3, Relaxed);
        RECORD_MAX_US.store(saved.4, Relaxed);
        (before == after && rejected == 12, rejected)
    });
    let _ = writeln!(
        out,
        "RECORDCHECK pass={} rejected={} expected=12 unchanged_log_and_end={} disabled={} gate_authority=0",
        passed as u8,
        rejected,
        passed as u8,
        bridge_disabled() as u8
    );
}
pub fn comp_interrupt() {
    if DROP_ACTIVE.load(Relaxed) {
        Comp.mask_interrupts();
        Comp.clear_pending();
        return;
    }
    if LIVE_IRQ.load(Relaxed)
        && (OBS_STATUS.load(Relaxed) != 1
            || (!COAST_REFERENCE.load(Relaxed)
                && wave_timer::physical().0 as u16 != D.current_step.load(Relaxed)))
    {
        if OBS_STATUS.load(Relaxed) == 1 {
            OBS_STATUS.store(3, Relaxed);
        }
        live_stop();
        return;
    }
    #[cfg(feature = "bench-lean-core")]
    {
        if !REAL_IRQ.load(Relaxed) || !LIVE_IRQ.load(Relaxed) || !COAST_REFERENCE.load(Relaxed) {
            Comp.mask_interrupts();
            Comp.clear_pending();
            return;
        }
        // Lean operation still needs the powered source-rate safety cutoff.
        // Without it a comparator storm can starve foreground UART/cleanup
        // after the independent TIM6 guard has already disabled the bridge.
        let rate_limit = reverse_rate_limit();
        let decision = irq_dispatch::qualify(
            MASKED.load(Relaxed),
            comp_input::hardware_enabled(),
            Comp.exti_pending(),
        );
        #[cfg(feature = "bench-reverse-irq-probe")]
        unsafe {
            let index = match decision {
                irq_dispatch::Decision::Dispatch => 0,
                irq_dispatch::Decision::SoftwareMasked => 1,
                irq_dispatch::Decision::HardwareMasked => 2,
                irq_dispatch::Decision::NotPending => 3,
            };
            let count = core::ptr::addr_of_mut!(REVERSE_IRQ_DECISIONS)
                .cast::<u16>()
                .add(index);
            count.write(count.read().saturating_add(1));
        }
        if POWER_DUTY.load(Relaxed) != 0
            && unsafe {
                !(&mut *core::ptr::addr_of_mut!(IRQ_RATE)).hit_limit(t17(), rate_limit)
            }
        {
            #[cfg(feature = "bench-reverse-irq-probe")]
            unsafe {
                let exti = &*stm32::EXTI::ptr();
                let adc = &*stm32::ADC::ptr();
                let tim = &*stm32::TIM1::ptr();
                core::ptr::addr_of_mut!(REVERSE_IRQ_TRIP).write([
                    D.current_step.load(Relaxed) as u32,
                    core::ptr::read_volatile(COMP2_CSR),
                    exti.rpr1().read().bits(),
                    exti.fpr1().read().bits(),
                    exti.imr1().read().bits(),
                    adc.isr().read().bits(),
                    adc.ier().read().bits(),
                    tim.cnt().read().bits(),
                    t17() as u32,
                    powered_timer::owns() as u32,
                ]);
            }
            Comp.mask_interrupts();
            Comp.clear_pending();
            live_stop();
            powered_timer::irq_storm();
            return;
        }
        if decision != irq_dispatch::Decision::Dispatch {
            return;
        }
        let mut specialized = Motor {
            pwm: Output,
            comp: StaticComp,
            phase: Output,
            interval: Interval,
            com: com_timer::Timer,
        };
        #[cfg(feature = "bench-com-top-high")]
        if POWER_DUTY.load(Relaxed) >= 480 {
            COMP_RECORD_SECTOR.store(D.current_step.load(Relaxed).wrapping_sub(1) as u32, Relaxed);
        }
        #[cfg(feature = "bench-com-lag")]
        let before_accepts = ACCEPTS.load(Relaxed);
        isr::comp_isr(&S.sched(), &D.drive(), &mut specialized, &observer());
        #[cfg(feature = "bench-com-lag")]
        if POWER_DUTY.load(Relaxed) >= 480 && ACCEPTS.load(Relaxed) != before_accepts {
            let index = COMMUTATIONS.load(Relaxed);
            if index & 15 == 0 {
                COM_LAG_EXIT_COUNT.store(unsafe { (*stm32::TIM2::ptr()).cnt().read().bits() & 65535 }, Relaxed);
                COM_LAG_EXIT_INDEX.store(index, Relaxed);
            }
        }
        return;
    }
    #[cfg(not(feature = "bench-lean-core"))]
    {
        let calls = COMP_CALLS.load(Relaxed) + 1;
        COMP_CALLS.store(calls, Relaxed);
        let powered = POWER_DUTY.load(Relaxed) != 0;
        let exceeded = if powered {
            unsafe { !(&mut *core::ptr::addr_of_mut!(IRQ_RATE)).hit(t17()) }
        } else {
            calls > 256
        };
        if exceeded {
            Comp.mask_interrupts();
            Comp.clear_pending();
            if LIVE_IRQ.load(Relaxed) {
                OBS_STATUS.store(5, Relaxed);
                live_stop();
            }
            if powered {
                powered_timer::irq_storm();
            } // immediate bridge kill, not foreground-only
            return;
        }
        if REAL_IRQ.load(Relaxed) {
            let enabled = comp_input::hardware_enabled();
            let decision =
                irq_dispatch::qualify(MASKED.load(Relaxed), enabled, Comp.exti_pending());
            let skipped = match decision {
                irq_dispatch::Decision::Dispatch => None,
                irq_dispatch::Decision::SoftwareMasked => Some(0),
                irq_dispatch::Decision::HardwareMasked => Some(1),
                irq_dispatch::Decision::NotPending => Some(2),
            };
            if let Some(index) = skipped {
                // Capture the source BEFORE the IRQ cap clears pending/masks NVIC.
                // First skip only; no UART, flag clearing or gate access in this ISR.
                if COAST_REFERENCE.load(Relaxed) && !DISPATCH_SNAPSHOT_VALID.load(Relaxed) {
                    unsafe {
                        let e = &*stm32::EXTI::ptr();
                        let a = &*stm32::ADC::ptr();
                        core::ptr::addr_of_mut!(DISPATCH_SNAPSHOT).write([
                            observation_elapsed(),
                            e.imr1().read().bits(),
                            e.rpr1().read().bits(),
                            e.fpr1().read().bits(),
                            a.isr().read().bits(),
                            a.ier().read().bits(),
                            core::ptr::read_volatile((COMP2_CSR as usize - 4) as *const u32),
                            core::ptr::read_volatile(COMP2_CSR),
                        ]);
                    }
                    DISPATCH_SNAPSHOT_VALID.store(true, Relaxed);
                }
                DISPATCH_SKIP[index].store(DISPATCH_SKIP[index].load(Relaxed) + 1, Relaxed);
                return; // preserve pending COM deadline and reference interval
            }
        }
        let before = t17();
        #[cfg(feature = "bench-qualification-direct")]
        let _direct_dispatch =
            qualification_direct_live::begin(powered, S.average_interval.load(Relaxed));
        let trace = LIVE_IRQ.load(Relaxed) && TRACE_ENABLED.load(Relaxed);
        // Optional removal of report-only per-dispatch clock work. The accepted
        // event recorder and stop path still sample OBS_CLOCK; safety clocks are
        // separate and unchanged. No missing timestamp may masquerade as fresh.
        if COAST_REFERENCE.load(Relaxed)
            && !(cfg!(feature = "bench-quiet-irq-stamp") && powered && !trace)
        {
            LAST_IRQ_US.store(observation_elapsed(), Relaxed);
        }
        #[cfg(feature = "bench-comp-paths")]
        let path_scope =
            comp_path_live::begin(S.average_interval.load(Relaxed), ACCEPTS.load(Relaxed));
        #[cfg(feature = "bench-qualification-event")]
        let path_scope = path_scope.with_event_context(powered, D.current_step.load(Relaxed) as u8);
        #[cfg(feature = "bench-comp-decisions")]
        let path_scope =
            path_scope.with_context(observation_elapsed(), D.current_step.load(Relaxed) as u8);
        if !trace {
            #[cfg(feature = "bench-qualification-sparse")]
            let sparse = if powered {
                Some(qualification_sparse_live::begin(
                    before,
                    ACCEPTS.load(Relaxed),
                    D.current_step.load(Relaxed) as u8,
                ))
            } else {
                None
            };
            #[cfg(feature = "bench-qualification-window")]
            let qualification = if powered {
                Some(qualification_live::begin(
                    ACCEPTS.load(Relaxed),
                    D.current_step.load(Relaxed) as u8,
                ))
            } else {
                None
            };
            // Preserve actual minz filtering and all prefix/IRQ-count guards.
            // No trace metadata collection or per-read counter updates.
            TRACE_READS.store(false, Relaxed);
            #[cfg(feature = "bench-static-comp")]
            {
                let mut m = motor();
                if m.comp.real && m.comp.inverted && !m.comp.trace {
                    let (sched, drive) = (&S.sched(), &D.drive());
                    let mut specialized = Motor {
                        pwm: Output,
                        comp: StaticComp,
                        phase: Output,
                        interval: Interval,
                        com: com_timer::Timer,
                    };
                    #[cfg(feature = "bench-comp-critical")]
                    if powered {
                        // One known bounded reference service, not a campaign mask.
                        // Include acceptance/recording: do not reopen an interrupt
                        // gap between successful qualification and its timestamp.
                        let Some(elapsed) = critical_service(D.filter_level.load(Relaxed), || {
                            isr::comp_isr(sched, drive, &mut specialized, &observer());
                        }) else {
                            CRITICAL_REFUSED.store(CRITICAL_REFUSED.load(Relaxed) + 1, Relaxed);
                            powered_timer::abort();
                            return;
                        };
                        CRITICAL_CALLS.store(CRITICAL_CALLS.load(Relaxed) + 1, Relaxed);
                        CRITICAL_MAX.store(CRITICAL_MAX.load(Relaxed).max(elapsed), Relaxed);
                        // Additional probe stop, NOT a replacement for any guard.
                        if elapsed > 60 {
                            CRITICAL_REFUSED.store(CRITICAL_REFUSED.load(Relaxed) + 1, Relaxed);
                            powered_timer::abort();
                        }
                    } else {
                        isr::comp_isr(sched, drive, &mut specialized, &observer());
                    }
                    #[cfg(not(feature = "bench-comp-critical"))]
                    isr::comp_isr(sched, drive, &mut specialized, &observer());
                } else {
                    isr::comp_isr(&S.sched(), &D.drive(), &mut m, &observer());
                }
            }
            #[cfg(not(feature = "bench-static-comp"))]
            isr::comp_isr(&S.sched(), &D.drive(), &mut motor(), &observer());
            #[cfg(feature = "bench-comp-paths")]
            path_scope.finish(
                ACCEPTS.load(Relaxed),
                OBS_STATUS.load(Relaxed) != 1 || (powered && !powered_timer::owns()),
            );
            #[cfg(feature = "bench-qualification-window")]
            if let Some(scope) = qualification {
                scope.finish(
                    ACCEPTS.load(Relaxed),
                    OBS_STATUS.load(Relaxed) != 1 || !powered_timer::owns(),
                );
            }
            #[cfg(feature = "bench-qualification-direct")]
            drop(_direct_dispatch); // include normal cleanup in COMP_MAX bracket
            let completed = t17();
            #[cfg(feature = "bench-qualification-sparse")]
            if let Some(scope) = sparse {
                scope.finish(
                    completed.wrapping_sub(before),
                    ACCEPTS.load(Relaxed),
                    OBS_STATUS.load(Relaxed) != 1 || !powered_timer::owns(),
                );
            }
            // Sparse selection uses the pre-publication bracket; include its cost
            // in the existing whole-dispatch maximum and outer IRQ accounting.
            #[cfg(feature = "bench-qualification-sparse")]
            let completed = t17();
            COMP_MAX.store(
                COMP_MAX
                    .load(Relaxed)
                    .max(completed.wrapping_sub(before) as u32),
                Relaxed,
            );
            return;
        }
        let pwm = unsafe { (*stm32::TIM1::ptr()).cnt().read().bits() };
        // Full-width timestamp sampled beside PWM CNT, not inferred across wraps.
        let trace_us = if cfg!(feature = "bench-irq-tail") {
            observation_elapsed()
        } else {
            before.wrapping_sub(OBS_START16.load(Relaxed) as u16) as u32
        };
        let accepted = ACCEPTS.load(Relaxed);
        let step = D.current_step.load(Relaxed);
        let avg = S.average_interval.load(Relaxed);
        let rising = D.rising.load(Relaxed);
        LEVEL_N.store(0, Relaxed);
        LEVEL_FIRST.store(2, Relaxed);
        LEVEL_LAST.store(2, Relaxed);
        GATE_COUNT.store(u32::MAX, Relaxed);
        TRACE_READS.store(trace, Relaxed);
        isr::comp_isr(&S.sched(), &D.drive(), &mut motor(), &observer());
        #[cfg(feature = "bench-comp-paths")]
        path_scope.finish(
            ACCEPTS.load(Relaxed),
            OBS_STATUS.load(Relaxed) != 1 || (powered && !powered_timer::owns()),
        );
        TRACE_READS.store(false, Relaxed);
        if trace {
            let n = TRACE_N.load(Relaxed);
            if (n as usize) < IRQ_TRACE_CAPACITY || cfg!(feature = "bench-irq-tail") {
                let mut row = [0u16; IRQ_TRACE_WORDS];
                row[..14].copy_from_slice(&[
                    calls as u16,
                    trace_us as u16,
                    step,
                    pwm as u16,
                    GATE_COUNT.load(Relaxed) as u16,
                    avg as u16,
                    LEVEL_N.load(Relaxed) as u16,
                    LEVEL_FIRST.load(Relaxed) as u16,
                    LEVEL_LAST.load(Relaxed) as u16,
                    Comp.exti_pending() as u16,
                    MASKED.load(Relaxed) as u16,
                    ACCEPTS.load(Relaxed).wrapping_sub(accepted) as u16,
                    t17().wrapping_sub(before),
                    rising as u16,
                ]);
                if cfg!(feature = "bench-irq-tail") {
                    row[IRQ_TRACE_WORDS - 1] = (trace_us >> 16) as u16;
                }
                let index = if cfg!(feature = "bench-irq-tail") {
                    TRACE_NEXT.load(Relaxed) as usize
                } else {
                    n as usize
                };
                unsafe {
                    core::ptr::addr_of_mut!(IRQ_TRACE)
                        .cast::<[u16; IRQ_TRACE_WORDS]>()
                        .add(index)
                        .write(row);
                }
                if cfg!(feature = "bench-irq-tail") {
                    TRACE_NEXT.store(
                        if index + 1 == IRQ_TRACE_CAPACITY {
                            0
                        } else {
                            index as u32 + 1
                        },
                        Relaxed,
                    );
                }
                if (n as usize) < IRQ_TRACE_CAPACITY {
                    TRACE_N.store(n + 1, Relaxed);
                } else {
                    TRACE_DROP.store(TRACE_DROP.load(Relaxed) + 1, Relaxed);
                }
            } else {
                TRACE_DROP.store(TRACE_DROP.load(Relaxed) + 1, Relaxed);
            }
        }
        COMP_MAX.store(
            COMP_MAX
                .load(Relaxed)
                .max(t17().wrapping_sub(before) as u32),
            Relaxed,
        );
    }
}

pub fn run<W: Write>(out: &mut W, real_irq: bool, dump: bool) {
    COAST_REFERENCE.store(false, Relaxed);
    PHYSICAL_OBSERVATION.store(false, Relaxed);
    live_stop();
    LIVE_IRQ.store(false, Relaxed);
    polling_stop();
    POLL_TIMER.store(false, Relaxed);
    gates_off();
    set_pin(3, 1, false);
    com_timer::Timer::init();
    comp_input::stop();
    let saved_csr = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    REAL_IRQ.store(real_irq, Relaxed);
    unsafe {
        let r = &*stm32::RCC::ptr();
        r.apbenr1().modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM2::ptr();
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(31));
        t.arr().write(|w| w.bits(65535));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        t.cr1().write(|w| w.bits(1));
    }
    for ci in [1667u32, 1333] {
        S.commutation_interval.store(ci, Relaxed);
        S.average_interval.store(ci, Relaxed);
        S.last_average_interval.store(ci, Relaxed);
        for v in &S.interval_hist {
            v.store(ci, Relaxed);
        }
        S.last_zc.store(ci as u16, Relaxed);
        S.this_zc.store(ci as u16, Relaxed);
        S.wait_time.store((ci / 4) as u16, Relaxed);
        D.current_step.store(1, Relaxed);
        D.rising.store(true, Relaxed);
        D.old_routine.store(false, Relaxed);
        D.running.store(true, Relaxed);
        D.zero_crosses.store(20, Relaxed);
        D.filter_level.store(12, Relaxed);
        Z.head.store(0, Relaxed);
        Z.tail.store(0, Relaxed);
        Z.drop.store(0, Relaxed);
        Z.comm_n.store(0, Relaxed);
        COMMUTATIONS.store(0, Relaxed);
        COM_MAX.store(0, Relaxed);
        COMP_MAX.store(0, Relaxed);
        EVENTS.store(0, Relaxed);
        COMP_CALLS.store(0, Relaxed);
        MASKED.store(false, Relaxed);
        PENDING.store(false, Relaxed);
        ACTIVE.store(true, Relaxed);
        if real_irq {
            Comp.set_step(1, true);
            Comp.change_input();
            Comp.enable_interrupts();
            unsafe {
                cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::ADC_COMP);
            }
        }
        unsafe {
            cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM16);
        }
        Interval.set_count(0);
        let start = clock_us();
        let mut sent = 0;
        let mut failures = 0;
        for n in 1..=32u32 {
            let deadline = n * ci / 2;
            while clock_us().wrapping_sub(start) < deadline {}
            if MASKED.load(Relaxed) {
                failures += 1;
                continue;
            }
            if real_irq {
                unsafe {
                    core::ptr::write_volatile(
                        COMP2_CSR,
                        core::ptr::read_volatile(COMP2_CSR) ^ (1 << 15),
                    );
                }
            } else {
                LEVEL.store(D.rising.load(Relaxed), Relaxed);
                PENDING.store(true, Relaxed);
                let before = t17();
                isr::comp_isr(&S.sched(), &D.drive(), &mut motor(), &observer());
                COMP_MAX.store(
                    COMP_MAX
                        .load(Relaxed)
                        .max(t17().wrapping_sub(before) as u32),
                    Relaxed,
                );
            }
            sent += 1;
            let s = S.sched();
            lp::store_average_interval(&s, s.intervals().e_com_time());
        }
        let wait = t17();
        while COMMUTATIONS.load(Relaxed) < sent && t17().wrapping_sub(wait) < 2000 {}
        com_timer::Timer::stop();
        comp_input::stop();
        ACTIVE.store(false, Relaxed);
        let _ = writeln!(
            out,
            "COREBENCH ci={} sent={} com={} masked={} comp_max_us={} com_max_us={} avg={} trace_drop={} synthetic=1 real_irq={} irq_calls={}",
            ci,
            sent,
            COMMUTATIONS.load(Relaxed),
            failures,
            COMP_MAX.load(Relaxed) & 65535,
            COM_MAX.load(Relaxed),
            S.average_interval.load(Relaxed),
            Z.drop.load(Relaxed),
            real_irq as u8,
            COMP_CALLS.load(Relaxed)
        );
        if dump {
            let _ = writeln!(
                out,
                "CORETRACE ci={} records={} format=zct_bytes_as_u16_a85_crc",
                ci,
                Z.records()
            );
            let mut row = [0u16; minz_core::am32::ZCT_REC];
            let mut ix = 0;
            while Z.records() != 0 {
                Z.zct().drain(|b| {
                    row[ix] = b as u16;
                    ix += 1;
                    if ix == row.len() {
                        let _ = snapshot::record(out, "Z85", &row);
                        ix = 0;
                    }
                });
            }
            let _ = writeln!(out, "CORETRACE END");
        }
    }
    unsafe {
        (*stm32::TIM2::ptr()).cr1().write(|w| w.bits(0));
        core::ptr::write_volatile(COMP2_CSR, saved_csr);
    }
    REAL_IRQ.store(false, Relaxed);
    gates_off();
    set_pin(3, 1, false);
}
