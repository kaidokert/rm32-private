//! Actual COMP ISR observation alongside forced sectors. Caller serializes
//! EVERY method with interrupts disabled. No COM timer or output authority.
use super::*;
use minz_core::am32_hal::{CompExti, Comparator, Cs, IntervalTimer};
use portable_atomic::{AtomicBool, Ordering::Relaxed};
static MASKED: AtomicBool = AtomicBool::new(true);
static RETRY_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-startup-bootstrap")]
static mut BOOTSTRAP: Option<flying_acquire::Seed> = None;
// Terminal-only diagnostic. No reads/stores on successful rate checks.
// Captured under the caller's interrupt mask BEFORE stop clears EXTI.
#[cfg(feature = "bench-driven-rate-snapshot")]
static mut RATE_SNAPSHOT: Option<[u16; 8]> = None;
static S: core_state::SchedStore = core_state::SchedStore::new();
static D: core_state::DriveStore = core_state::DriveStore::new();
struct State {
    origin: u16,
    epoch: u16,
    step: u8,
    commands: u16,
    calls: u32,
    max_us: u16,
    n: usize,
    rate: irq_dispatch::Rate,
    rows: [[u16; 7]; 26],
    qualification: Option<driven_seed::Qualification>,
    overruns: u16,
    deferred: bool,
    sector_accepted: bool,
}
static mut STATE: State = State {
    origin: 0,
    epoch: 0,
    step: 0,
    commands: 0,
    calls: 0,
    max_us: 0,
    n: 0,
    rate: irq_dispatch::Rate::new(),
    rows: [[0; 7]; 26],
    qualification: None,
    overruns: 0,
    deferred: false,
    sector_accepted: false,
};
struct Input;
impl Comparator for Input {
    fn output_level(&self) -> bool {
        !comp_input::Input.output_level()
    }
    fn set_step(&mut self, step: u8, rising: bool) {
        comp_input::Input.set_step(step, !rising);
    }
    fn change_input(&mut self) {
        comp_input::Input.change_input();
    }
    fn enable_interrupts(&mut self) {
        if driven_run::owns() {
            MASKED.store(false, Relaxed);
            comp_input::Input.enable_interrupts();
        }
    }
    fn mask_interrupts(&mut self) {
        MASKED.store(true, Relaxed);
        comp_input::Input.mask_interrupts();
    }
}
impl CompExti for Input {
    fn exti_pending(&self) -> bool {
        RETRY_PENDING.load(Relaxed) || comp_input::Input.exti_pending()
    }
    fn clear_pending(&self) {
        RETRY_PENDING.store(false, Relaxed);
        comp_input::Input.clear_pending();
    }
}
struct Interval;
impl IntervalTimer for Interval {
    fn count(&self) -> u32 {
        unsafe { (*stm32::TIM2::ptr()).cnt().read().bits() & 65535 }
    }
    fn set_count(&mut self, n: u32) {
        unsafe {
            (*stm32::TIM2::ptr()).cnt().write(|w| w.bits(n & 65535));
        }
    }
}
struct Critical;
impl Cs for Critical {
    fn free<R>(&self, f: impl FnOnce() -> R) -> R {
        cortex_m::interrupt::free(|_| f())
    }
}

pub fn reset(origin: u16) {
    unsafe {
        stop();
        #[cfg(feature = "bench-startup-bootstrap")]
        {
            BOOTSTRAP = None;
        }
        #[cfg(feature = "bench-driven-rate-snapshot")]
        {
            RATE_SNAPSHOT = None;
        }
        core::ptr::addr_of_mut!(STATE).write(State {
            origin,
            epoch: 0,
            step: 0,
            commands: 0,
            calls: 0,
            max_us: 0,
            n: 0,
            rate: irq_dispatch::Rate::new(),
            rows: [[0; 7]; 26],
            overruns: 0,
            deferred: false,
            sector_accepted: false,
            qualification: Some({
                #[cfg(feature = "bench-startup-adc")]
                {
                    driven_seed::Qualification::with_startup_estimator(0)
                }
                #[cfg(all(
                    feature = "bench-seed-timing-reanchor",
                    not(feature = "bench-startup-adc")
                ))]
                {
                    driven_seed::Qualification::with_timing_reanchor(0)
                }
                #[cfg(all(
                    not(feature = "bench-seed-timing-reanchor"),
                    not(feature = "bench-startup-adc")
                ))]
                {
                    if cfg!(feature = "bench-driven-reanchor") {
                        driven_seed::Qualification::with_reanchor(0)
                    } else {
                        driven_seed::Qualification::new(0)
                    }
                }
            }),
        });
    }
}
pub fn prepare() {
    unsafe {
        // Fixed200eHz diagnostic interval gate. Not an acquired estimator or seed.
        S.average_interval.store(1666, Relaxed);
        S.wait_time.store(416, Relaxed);
        S.this_zc.store(0, Relaxed);
        S.last_zc.store(0, Relaxed);
        D.filter_level.store(12, Relaxed);
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 1));
        let t = &*stm32::TIM2::ptr();
        t.cr1().write(|w| w.bits(0));
        t.dier().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(31));
        t.arr().write(|w| w.bits(65535));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        t.cnt().write(|w| w.bits(0));
        t.cr1().write(|w| w.bits(1));
        // Forced sector scheduler owns physical deadlines during this observer.
        // A production pending camp must not starve that independent scheduler.
        cortex_m::Peripherals::steal().NVIC.set_priority(
            stm32::Interrupt::ADC_COMP,
            if cfg!(feature = "bench-startup-comp-top") {
                0
            } else {
                0x80
            },
        );
    }
}
pub fn command(epoch: u16, step: u8) -> bool {
    unsafe {
        if !driven_run::owns() {
            return false;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        if !(1..=6).contains(&step)
            || (s.commands == 0 && epoch != 0)
            || (s.commands != 0 && (epoch != s.epoch + 1 || step != s.step % 6 + 1))
        {
            return false;
        }
        Input.mask_interrupts();
        RETRY_PENDING.store(false, Relaxed);
        s.deferred = false;
        s.sector_accepted = false;
        s.epoch = epoch;
        s.step = step;
        s.commands += 1;
        D.current_step.store(step as u16, Relaxed);
        D.rising.store(step & 1 != 0, Relaxed);
        Input.set_step(step, step & 1 != 0);
        Input.change_input();
        Input.enable_interrupts();
        true
    }
}
/// false requires immediate shared physical shutdown. Called only by the
/// driven owner; already-masked/pending-less callbacks never re-enable COMP.
pub fn interrupt() -> bool {
    unsafe {
        let began = t17();
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        if MASKED.load(Relaxed) {
            Input.mask_interrupts();
            Input.clear_pending();
            return true;
        }
        s.calls += 1;
        // Dispatch count is telemetry, not a safety boundary. A short burst at
        // 65/ms previously killed an otherwise bounded normal restart. Actual
        // runaway/latency protection is the measured 50us handler budget below,
        // plus the independent command/feedback/watchdog deadlines.
        #[cfg(not(feature = "bench-driven-rate-snapshot"))]
        s.rate.observe(began);
        #[cfg(feature = "bench-driven-rate-snapshot")]
        if !s.rate.hit(began) {
            {
                let e = &*stm32::EXTI::ptr();
                RATE_SNAPSHOT = Some([
                    Interval.count() as u16,
                    S.average_interval.load(Relaxed) as u16, // fixed1666 in this observer
                    ((e.rpr1().read().bits() >> 18) & 1) as u16,
                    ((e.fpr1().read().bits() >> 18) & 1) as u16,
                    Input.output_level() as u16,
                    D.rising.load(Relaxed) as u16,
                    s.epoch,
                    t17().wrapping_sub(began),
                ]);
            }
            stop();
            return false;
        }
        let before = t17().wrapping_sub(s.origin);
        #[cfg(feature = "bench-startup-adc")]
        if !cfg!(feature = "bench-startup-reference-pending")
            && Input.exti_pending()
            && Interval.count() <= (S.average_interval.load(Relaxed) >> 1)
            && Input.output_level() == D.rising.load(Relaxed)
        {
            // No busy pending camp: preserve a same-command revisit obligation,
            // revoke the IRQ source, and let the independent timer open the gate.
            s.deferred = true;
            Input.mask_interrupts();
            Input.clear_pending();
            return finish_duration(s, t17().wrapping_sub(began));
        }
        let result = driven_irq_core::visit(&S.sched(), &D.drive(), Input, Interval, &Critical).2;
        let after = t17().wrapping_sub(s.origin);
        match result {
            driven_irq_core::Result::Accepted(a) => {
                if (!cfg!(feature = "bench-lean-irq") && s.n == 26) || a.sector + 1 != s.step {
                    stop();
                    return false;
                }
                // First interval starts at prepare(), NOT a previous accepted edge.
                // Retain its flag; none of these records is a handoff permit.
                if s.n < 26 {
                    s.rows[s.n] = [
                        s.epoch,
                        s.step as u16,
                        before,
                        after,
                        a.interval,
                        a.requested_arr,
                        (s.n != 0) as u16,
                    ];
                }
                s.n += 1;
                s.sector_accepted = true;
                #[cfg(feature = "bench-startup-bootstrap")]
                {
                    BOOTSTRAP = driven_seed::startup_bootstrap(
                        driven_seed::Edge {
                            epoch: s.epoch,
                            step: s.step,
                            before: before as u32 * 2,
                            after: after as u32 * 2,
                            interval: a.interval,
                        },
                        s.n,
                    );
                }
                s.qualification.as_mut().unwrap().accept(driven_seed::Edge {
                    epoch: s.epoch,
                    step: s.step,
                    before: before as u32 * 2,
                    after: after as u32 * 2,
                    interval: a.interval,
                });
            }
            driven_irq_core::Result::NoAcceptance => {}
            _ => {
                stop();
                return false;
            }
        }
        let elapsed = t17().wrapping_sub(began);
        finish_duration(s, elapsed)
    }
}
#[inline(always)]
fn finish_duration(s: &mut State, elapsed: u16) -> bool {
    s.max_us = s.max_us.max(elapsed);
    // Promote the existing host acceptance bound to an immediate refusal.
    // Caller safes outputs on false BEFORE it can consume a seed/handoff.
    if elapsed > 50 {
        s.overruns += 1;
        stop();
        return false;
    }
    true
}
/// Serialized timer callback, no comparator qualification or acceptance here.
/// A mux/command change cancels deferred state. Real persistence is performed
/// later by the normal COMP ISR; a software wake is not a fabricated crossing.
#[cfg(feature = "bench-startup-adc")]
pub fn resume_deferred() {
    unsafe {
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let revisit = s.deferred
            || (cfg!(feature = "bench-startup-level")
                && !s.sector_accepted
                && !MASKED.load(Relaxed)
                && s.commands != 0);
        if !irq_dispatch::deferred_ready(
            driven_run::owns(),
            revisit,
            Interval.count(),
            S.average_interval.load(Relaxed),
        ) {
            return;
        }
        s.deferred = false;
        let post = Input.output_level() == D.rising.load(Relaxed);
        Input.clear_pending();
        Input.enable_interrupts();
        if post {
            RETRY_PENDING.store(true, Relaxed);
            cortex_m::peripheral::NVIC::pend(stm32::Interrupt::ADC_COMP);
        }
    }
}
/// Disabled-only source test; never invent a live driven owner or gate permit.
#[cfg(feature = "bench-startup-adc")]
pub fn deferred_check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || driven_run::owns()
        || powered_timer::owns()
        || core_bench::active()
    {
        let _ = writeln!(out, "DEFERREDCHECK refused=1");
        return;
    }
    let passed = cortex_m::interrupt::free(|_| unsafe {
        stop();
        STATE.deferred = true;
        // No owner: actual retry callback must not unmask or generate a wake.
        resume_deferred();
        let no_owner =
            MASKED.load(Relaxed) && !comp_input::hardware_enabled() && !Input.exti_pending();
        // Cancel state without granting hardware/output ownership.
        STATE.deferred = false;
        resume_deferred();
        let canceled = !STATE.deferred && MASKED.load(Relaxed);
        // Exercise the exact software source while NVIC stays masked.
        RETRY_PENDING.store(true, Relaxed);
        cortex_m::peripheral::NVIC::pend(stm32::Interrupt::ADC_COMP);
        let pending = Input.exti_pending()
            && cortex_m::peripheral::NVIC::is_pending(stm32::Interrupt::ADC_COMP);
        stop();
        let cleared = !Input.exti_pending()
            && !cortex_m::peripheral::NVIC::is_pending(stm32::Interrupt::ADC_COMP);
        stop();
        [no_owner, canceled, pending, cleared]
    });
    let _ = writeln!(
        out,
        "DEFERREDCHECK cases=4 passed={} no_owner={} canceled={} software_pending={} cleared={} disabled=1 gate_authority=0 active_owner_retry_tested=0",
        passed.iter().all(|&v| v) as u8,
        passed[0] as u8,
        passed[1] as u8,
        passed[2] as u8,
        passed[3] as u8
    );
}
/// Exercise actual duration refusal and timer/mask cleanup, bridge disabled.
/// Synthetic elapsed values, NOT a measured full ISR or physical safing test.
#[cfg(feature = "bench-seed-timing-reanchor")]
pub fn budget_check<W: Write>(out: &mut W) {
    if core_bench::active()
        || powered_timer::owns()
        || driven_run::owns()
        || !core_bench::bridge_disabled()
    {
        let _ = writeln!(out, "IRQBUDGETCHECK refused=1");
        return;
    }
    let mut failed = 0u32;
    #[cfg(feature = "bench-driven-rate-snapshot")]
    let mut rate_us = 0u16;
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 1));
        #[cfg(not(feature = "bench-driven-rate-snapshot"))]
        {
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            s.overruns = 0;
            s.max_us = 0;
            for elapsed in [0u16, 49, 50, 51, 65535] {
                let before = s.overruns;
                (*stm32::TIM2::ptr()).cr1().write(|w| w.bits(1));
                MASKED.store(false, Relaxed); // logical state only; no EXTI unmask
                let ok = finish_duration(s, core::hint::black_box(elapsed));
                let stopped = (*stm32::TIM2::ptr()).cr1().read().bits() & 1 == 0;
                if ok != (elapsed <= 50)
                    || stopped != (elapsed > 50)
                    || MASKED.load(Relaxed) != (elapsed > 50)
                    || s.overruns - before != (elapsed > 50) as u16
                {
                    failed += 1;
                }
            }
        }
        stop();
        #[cfg(feature = "bench-driven-rate-snapshot")]
        {
            // Fail the real rate latch without enabling NVIC or any output.
            // Then time the real interrupt refusal through stop(), including
            // snapshot stores. This excludes driven_run::end and exception entry.
            for _ in 0..16 {
                let mut rate = irq_dispatch::Rate::new();
                for _ in 0..65 {
                    rate.hit(0);
                }
                (*core::ptr::addr_of_mut!(STATE)).rate = rate;
                MASKED.store(false, Relaxed);
                let began = t17();
                let ok = interrupt();
                rate_us = rate_us.max(t17().wrapping_sub(began));
                if ok
                    || !MASKED.load(Relaxed)
                    || core::ptr::addr_of!(RATE_SNAPSHOT).read().is_none()
                    || (*stm32::TIM2::ptr()).cr1().read().bits() & 1 != 0
                {
                    failed += 1;
                }
            }
            RATE_SNAPSHOT = None;
        }
    });
    #[cfg(feature = "bench-driven-rate-snapshot")]
    {
        let _ = writeln!(
            out,
            "RATESTOP max_us={} cases=16 failed={}",
            rate_us, failed
        );
    }
    #[cfg(not(feature = "bench-driven-rate-snapshot"))]
    let _ = writeln!(
        out,
        "IRQBUDGETCHECK cases=5 failed={} boundary_us=50 stopped_above=1 synthetic_elapsed=1 disabled={} gate_authority=0",
        failed,
        core_bench::bridge_disabled() as u8
    );
}
pub fn stop() {
    Input.mask_interrupts();
    RETRY_PENDING.store(false, Relaxed);
    comp_input::stop();
    unsafe {
        (*stm32::TIM2::ptr()).cr1().write(|w| w.bits(0));
    }
}
#[cfg(feature = "bench-driven-handoff")]
pub fn seed() -> Option<flying_acquire::Seed> {
    unsafe {
        #[cfg(feature = "bench-startup-bootstrap")]
        {
            return BOOTSTRAP;
        }
        #[cfg(not(feature = "bench-startup-bootstrap"))]
        (*core::ptr::addr_of!(STATE))
            .qualification
            .as_ref()
            .and_then(|q| q.ready())
    }
}
pub fn dump<W: Write>(out: &mut W) {
    unsafe {
        #[cfg(feature = "bench-startup-bootstrap")]
        let _ = writeln!(
            out,
            "STARTUPBOOTSTRAP commanded_interval=1666 measured_seed=0 accepted_gate=6 lock_proven=0"
        );
        let s = &*core::ptr::addr_of!(STATE);
        #[cfg(feature = "bench-driven-rate-snapshot")]
        {
            let _ = writeln!(
                out,
                "RATEGATE fields=count,average,rpr18,fpr18,hal_level,rising,epoch,bracket_us terminal_only=1 sequential_reads=1"
            );
            if let Some(row) = RATE_SNAPSHOT {
                let _ = snapshot::record(out, "RG85", &row);
            }
        }
        let _ = writeln!(
            out,
            "DRIVENIRQBUDGET limit_us=50 overruns={} immediate_stop=1 before_handoff=1 excludes_release=1",
            s.overruns
        );
        let _ = writeln!(
            out,
            "DRIVENIRQ calls={} accepts={} max_us={} rate_peak={} rate_limit=64 fixed_average_ticks=1666 filter_reads=12 raw_inverted=1 handoff_authority=0 masked={}",
            s.calls,
            s.n,
            s.max_us,
            s.rate.peak,
            MASKED.load(Relaxed) as u8
        );
        let _ = writeln!(
            out,
            "STARTUPRATE report_only=1 tick_watchdog=1 handler_overrun_stop=1"
        );
        let _ = writeln!(
            out,
            "DI85FIELDS epoch,step,before_us,after_us,interval_half_us,requested_arr,previous_accept_exists"
        );
        for row in &s.rows[..s.n.min(26)] {
            let _ = snapshot::record(out, "DI85", row);
        }
        if let Some(q) = s.qualification.as_ref() {
            #[cfg(all(
                feature = "bench-seed-timing-reanchor",
                not(feature = "bench-startup-adc")
            ))]
            {
                let _ = writeln!(
                    out,
                    "SEEDTIMING discarded_fault={} max_restarts=1 fresh_intervals=12 original_deadline=1 experimental=1",
                    q.discarded_fault()
                );
            }
            #[cfg(feature = "bench-driven-reanchor")]
            {
                let (count, epoch) = q.reanchors();
                #[cfg(feature = "bench-startup-adc")]
                let _ = writeln!(
                    out,
                    "STARTUPSEED window_us=40000 rolling_missing_epoch=1 count={} anchor_epoch={} fresh_intervals=12 gaps_not_averaged=1 individual_spacing_report=1",
                    count, epoch
                );
                #[cfg(not(feature = "bench-startup-adc"))]
                let _ = writeln!(
                    out,
                    "DRIVENSEEDRESTART count={} anchor_epoch={} max_restarts=1 fresh_intervals=12 original_deadline=1",
                    count, epoch
                );
            }
            let seed = q.ready();
            let cycles = q.cycles();
            let _ = writeln!(
                out,
                "DRIVENSEED fields=ready,step,edge_lo,edge_hi,average,intervals,cycles,cycle_min,cycle_max,fault conservative_bracket_start=1 handoff_authority=0 partial_epoch_excluded=1"
            );
            let _ = snapshot::record(
                out,
                "DS85",
                &[
                    seed.is_some() as u16,
                    seed.map_or(0, |v| v.step as u16),
                    seed.map_or(0, |v| v.edge_tick as u16),
                    seed.map_or(0, |v| (v.edge_tick >> 16) as u16),
                    seed.map_or(0, |v| v.interval_ticks as u16),
                    q.intervals() as u16,
                    cycles[0] as u16,
                    cycles[1] as u16,
                    cycles[2] as u16,
                    q.fault(),
                ],
            );
        }
    }
}
