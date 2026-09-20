//! Guarded, commanded-phase-continuous20ms drive/COMP observation.
//! Fixture-only records. Normal observer has no handoff authority; the separate
//! bench-driven-handoff feature plus drivex1 permits a validated fresh transfer.
use super::*;
use portable_atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_DUTY: AtomicU32 = AtomicU32::new(0);
static APPLIED_DUTY: AtomicU32 = AtomicU32::new(0);
static NEXT_BEMF_DUTY: AtomicU32 = AtomicU32::new(0);
static APPLIED_BEMF_DUTY: AtomicU32 = AtomicU32::new(0);
const ACQUISITION_DUTY_MAX: u32 = if cfg!(feature = "bench-startup-adc") {
    100
} else {
    62
};
pub fn clear_bemf_duty() {
    NEXT_BEMF_DUTY.store(0, Relaxed);
}
fn take_bemf_duty(acquisition: u32) -> u32 {
    let requested = NEXT_BEMF_DUTY.swap(0, Relaxed);
    if cfg!(feature = "bench-startup-adc") {
        duty_split::select_startup(acquisition, requested).unwrap_or(0)
    } else {
        duty_split::select(acquisition, requested).unwrap_or(0)
    }
}
pub fn set_bemf_duty(duty: u32) -> bool {
    let selected = if cfg!(feature = "bench-startup-adc") {
        duty_split::select_startup(62, duty)
    } else {
        duty_split::select(62, duty)
    };
    if selected.is_none()
        || owns()
        || powered_timer::owns()
        || core_bench::active()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        return false;
    }
    NEXT_BEMF_DUTY.store(duty, Relaxed);
    true
}
pub fn duty_check<W: Write>(out: &mut W) {
    if owns()
        || powered_timer::owns()
        || core_bench::active()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        let _ = writeln!(out, "DUTYCHECK refused=1 gate_authority=0");
        return;
    }
    clear_bemf_duty();
    let mut passed = 0;
    passed += (set_bemf_duty(70) && take_bemf_duty(62) == 70) as u32;
    passed += (take_bemf_duty(62) == 62) as u32;
    let maximum = if cfg!(feature = "bench-startup-adc") {
        duty_split::STARTUP_MAX
    } else {
        duty_split::MAX
    };
    passed += (set_bemf_duty(maximum)
        && !set_bemf_duty(maximum + 1)
        && take_bemf_duty(62) == maximum) as u32;
    passed += (set_bemf_duty(40) && !set_bemf_duty(39) && take_bemf_duty(62) == 40) as u32;
    let staged = set_bemf_duty(70);
    clear_bemf_duty();
    passed += (staged && take_bemf_duty(62) == 62) as u32;
    passed += (set_bemf_duty(0) && take_bemf_duty(62) == 62) as u32;
    clear_bemf_duty();
    let _ = writeln!(
        out,
        "DUTYCHECK passed={} expected=6 pending=0 disabled={} gate_authority=0",
        passed,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
static NEXT_PHASE: AtomicI32 = AtomicI32::new(0);
static APPLIED_PHASE: AtomicI32 = AtomicI32::new(0);
static mut ALIGNMENT: [u16; 3] = [0; 3]; // initial remainder,requested wait,actual wait
#[cfg(feature = "bench-startup-adc")]
static mut ADC_STOP_STATE: [u16; 12] = [0; 12];
#[cfg(feature = "bench-driven-handoff")]
static NEXT_TRANSFER: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driven-handoff")]
static TRANSFER_ARMED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "bench-driven-handoff")]
static TRANSFER_RESULT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
static TRANSFER_EPOCH: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
static SCAN_YIELD: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-driven-handoff")]
pub fn scan_yield(completed: u32) {
    SCAN_YIELD.store(completed + 1, Relaxed);
}
#[cfg(feature = "bench-driven-handoff")]
static mut RELEASED: Option<Transfer> = None;
#[cfg(feature = "bench-driven-handoff")]
static mut LAST_FEEDBACK: Option<(powered_guard::Feedback, u16)> = None;
#[cfg(feature = "bench-driven-handoff")]
static mut TRANSFER_TIMES: [u16; 7] = [0; 7];
/// Constructed only by the healthy driven IRQ release; consumed once by core.
#[cfg(feature = "bench-driven-handoff")]
pub struct Transfer {
    seed: flying_acquire::Seed,
    origin: u16,
    sample: powered_guard::Feedback,
    acquired: u16,
    began: u16,
    released: u16,
    epoch: u32,
}
#[cfg(feature = "bench-driven-handoff")]
impl Transfer {
    pub fn seed(&self) -> (flying_acquire::Seed, u16) {
        (self.seed, self.origin)
    }
    pub fn feedback(&self) -> (powered_guard::Feedback, u16) {
        (self.sample, self.acquired)
    }
    pub fn fresh(&self) -> bool {
        self.fresh_at(t17())
    }
    fn fresh_at(&self, now: u16) -> bool {
        let ci = self.seed.interval_ticks;
        let wait = minz_core::am32::wait_time(
            ci,
            minz_core::am32::advance_of(ci, minz_core::am32_loop::TEMP_ADVANCE),
        );
        self.epoch == TRANSFER_EPOCH.load(Relaxed)
            && self.released.wrapping_sub(self.began) >= 1100
            && now.wrapping_sub(self.released) < 1000
            && now.wrapping_sub(self.acquired) <= 1000
            && self
                .seed
                .handoff_with_min::<{ flying_acquire::SEED_MIN_TICKS }>(
                    now.wrapping_sub(self.origin) as u32 * 2,
                    wait,
                )
                .is_some()
    }
}
#[cfg(feature = "bench-driven-handoff")]
pub fn transfer_arm(on: bool) -> bool {
    // Disarming must also work during host-off from a powered startup.
    if !on {
        NEXT_TRANSFER.store(false, Relaxed);
        return true;
    }
    if owns() || get_idr(3, 1) || !powered_timer::outputs_disabled() {
        return false;
    }
    NEXT_TRANSFER.store(on, Relaxed);
    true
}
/// Synthetic token checks at idle; never raises ENABLE or writes a gate high.
/// Production tokens are still constructed only at qualified IRQ release.
#[cfg(feature = "bench-driven-handoff")]
pub fn transfer_check<W: Write>(out: &mut W) {
    if owns()
        || powered_timer::owns()
        || core_bench::active()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        let _ = writeln!(out, "TRANSFERCHK refused idle_disabled_required=1");
        return;
    }
    gates_off();
    NEXT_TRANSFER.store(false, Relaxed);
    let origin = t17().wrapping_sub(10020);
    let make = || Transfer {
        seed: flying_acquire::Seed {
            step: 2,
            edge_tick: 20000,
            interval_ticks: 1600,
        },
        origin,
        sample: powered_guard::Feedback {
            phase: [2048; 3],
            bus_mv: 11700,
            vref: 1500,
        },
        acquired: origin.wrapping_add(9900),
        began: origin,
        released: origin.wrapping_add(10010),
        epoch: TRANSFER_EPOCH.load(Relaxed),
    };
    let now = origin.wrapping_add(10020);
    let mut passed = 0u8;
    let token = make();
    passed += token.fresh_at(now) as u8;
    // ci1600 -> wait400 ticks; age338 leaves62,below required64.
    passed += (!token.fresh_at(origin.wrapping_add(10169))) as u8;
    let mut stale = make();
    stale.acquired = now.wrapping_sub(1001);
    passed += (!stale.fresh_at(now)) as u8;
    let mut unwoken = make();
    unwoken.began = unwoken.released.wrapping_sub(1099);
    passed += (!unwoken.fresh_at(now)) as u8;
    passed += (!powered_timer::adopt_driven(&token)) as u8; // ENABLE is off
    unsafe {
        core::ptr::addr_of_mut!(RELEASED).write(Some(make()));
    }
    gates_off(); // actual shared revocation,including when forced owner stopped
    passed += (!token.fresh_at(now)) as u8;
    passed += unsafe { (&*core::ptr::addr_of!(RELEASED)).is_none() } as u8;
    sector();
    tick();
    comp_irq(); // real late callbacks,all owners stopped
    let disabled = !get_idr(3, 1) && powered_timer::outputs_disabled();
    passed += (disabled && !owns() && !powered_timer::owns()) as u8;
    set_pin(3, 1, false);
    let _ = writeln!(
        out,
        "TRANSFERCHK passed={} expected=8 disabled={} synthetic_token=1 gate_authority=0",
        passed, disabled as u8
    );
}
pub fn set_phase(degrees: i32) -> bool {
    if !matches!(degrees, -30 | 0 | 30 | 60)
        || owns()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        return false;
    }
    NEXT_PHASE.store(degrees, Relaxed);
    true
}
pub fn set_duty(duty: u32) -> bool {
    if !(duty == 0
        || (40..=ACQUISITION_DUTY_MAX).contains(&duty))
        || owns()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        return false;
    }
    NEXT_DUTY.store(duty, Relaxed);
    true
}
/// Replay the exact successful first-start handoff configuration for an
/// ordinary restart. The shell controls are intentionally one-shot, but a
/// tracking restart must not silently change acquisition duty, BEMF duty or
/// sine-to-six-step phase. Outputs and ENABLE remain off throughout.
#[cfg(feature = "bench-normal-restart")]
pub fn rearm_applied() -> bool {
    let duty = APPLIED_DUTY.load(Relaxed);
    let bemf = APPLIED_BEMF_DUTY.load(Relaxed);
    let phase = APPLIED_PHASE.load(Relaxed);
    if !(40..=ACQUISITION_DUTY_MAX).contains(&duty)
        || !(40..=100).contains(&bemf)
        || !matches!(phase, -30 | 0 | 30 | 60)
        || owns()
        || powered_timer::owns()
        || core_bench::active()
        || get_idr(3, 1)
        || !powered_timer::outputs_disabled()
    {
        return false;
    }
    NEXT_DUTY.store(duty, Relaxed);
    NEXT_BEMF_DUTY.store(bemf, Relaxed);
    NEXT_PHASE.store(phase, Relaxed);
    true
}
const N: usize = 416;
const A: usize = 256;
static mut READS: [[u16; 4]; N] = [[0; 4]; N];
static mut ADC: [[u16; 7]; A] = [[0; 7]; A];
static mut COMMANDS: [[u16; 3]; 26] = [[0; 3]; 26];
static mut DMA_BOUNDS: [[u16; 3]; 26] = [[0; 3]; 26];
static DMA_USED: AtomicBool = AtomicBool::new(false);
static mut COAST_ORIGIN: [u16; 3] = [0, 0, 0];
/// Called after shutdown with TIM17 reads bracketing the coast clock origin.
pub fn coast_origin(before: u16, after: u16) {
    unsafe {
        let s = &*core::ptr::addr_of!(STATE);
        // Acquisition release is not the final powered stop. Do not modulo-wrap
        // a long powered segment into an apparently valid coast-origin bracket.
        if s.reason == 22 {
            core::ptr::addr_of_mut!(COAST_ORIGIN).write([0, 0, 0]);
            return;
        }
        let stop = s.origin.wrapping_add(s.stop as u16);
        let low = before.wrapping_sub(stop);
        let high = after.wrapping_sub(stop);
        let valid = !owns() && s.reason != 0 && low <= high && high <= 1000 && high - low <= 20;
        core::ptr::addr_of_mut!(COAST_ORIGIN).write([valid as u16, low, high]);
    }
}
struct State {
    guard: Option<driven_guard::Guard>,
    observer: Option<driven_observer::Observer>,
    origin: u16,
    theta: u32,
    rate: u32,
    duty: u32,
    deadline: u32,
    first: u32,
    start: u32,
    step: u8,
    epoch: u32,
    reason: u32,
    stop: u32,
    reads: usize,
    scans: usize,
    tick_max: u16,
    sector_max: u16,
    late_max: u32,
    age_max: u32,
}
static mut STATE: State = State {
    guard: None,
    observer: None,
    origin: 0,
    theta: 0,
    rate: 0,
    duty: 0,
    deadline: 0,
    first: 0,
    start: 0,
    step: 0,
    epoch: 0,
    reason: 0,
    stop: 0,
    reads: 0,
    scans: 0,
    tick_max: 0,
    sector_max: 0,
    late_max: 0,
    age_max: 0,
};
pub fn owns() -> bool {
    ACTIVE.load(Relaxed)
}
#[cfg(feature = "bench-startup-adc")]
pub fn stream_feedback(sample: powered_guard::Feedback, acquired: u16) {
    cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let counter = t17();
        let now = counter.wrapping_sub(s.origin) as u32;
        if let Some(g) = s.guard.as_mut() {
            if let Err(f) = g.feedback(now, counter.wrapping_sub(acquired) as u32, sample) {
                end(s, code(f), now);
            }
        }
    });
}
fn code(f: powered_guard::Fault) -> u32 {
    use powered_guard::Fault::*;
    match f {
        SegmentDeadline => 2,
        TickGap => 3,
        FeedbackStale => 4,
        Current => 5,
        Bus => 6,
        Driver => 7,
        HostAbort => 9,
        _ => 10,
    }
}
// Every caller is serialized with both timers. Stop never calls gates_off:
// shared gates_off calls cancel,so recursion must be impossible.
unsafe fn end(s: &mut State, reason: u32, _decision_us: u32) {
    unsafe {
        let first = s.reason == 0;
        if first {
            s.reason = reason;
        }
        if let Some(g) = s.guard.as_mut() {
            g.stop();
        }
        ACTIVE.store(false, Relaxed);
        #[cfg(feature = "bench-driven-irq")]
        driven_irq_live::stop();
        pwm_sample_dma::stop();
        (*stm32::TIM3::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM3::ptr()).cr1().write(|w| w.bits(0));
        (*stm32::TIM6::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM6::ptr()).cr1().write(|w| w.bits(0));
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM3);
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        bridge_clear();
        // Only the explicit, validated ownership release keeps the driver awake.
        #[cfg(feature = "bench-startup-adc")]
        if first && reason == 4 {
            let t = &*stm32::TIM15::ptr();
            let d = &*stm32::DMA1::ptr();
            let a = &*stm32::ADC::ptr();
            ADC_STOP_STATE = [
                1,
                t.cr1().read().bits() as u16,
                t.cnt().read().bits() as u16,
                t.arr().read().bits() as u16,
                d.isr().read().bits() as u16,
                d.ch1().ndtr().read().bits() as u16,
                d.ch1().cr().read().bits() as u16,
                cortex_m::peripheral::NVIC::is_enabled(stm32::Interrupt::DMA1_CHANNEL1) as u16,
                a.cr().read().bits() as u16,
                a.cfgr1().read().bits() as u16,
                (a.cfgr1().read().bits() >> 16) as u16,
                a.isr().read().bits() as u16,
            ];
        }
        if !(cfg!(feature = "bench-driven-handoff") && reason == 22) {
            set_pin(3, 1, false);
        }
        // Physical shutdown timestamp,not the earlier guard decision. In particular
        // feedback bookkeeping may finish after that decision but before this stop.
        if first {
            s.stop = t17().wrapping_sub(s.origin) as u32;
        }
    }
}
pub fn cancel() {
    cortex_m::interrupt::free(|_| unsafe {
        #[cfg(feature = "bench-driven-handoff")]
        {
            powered_timer::revoke_driven();
            TRANSFER_EPOCH.fetch_add(1, Relaxed);
            TRANSFER_ARMED.store(false, Relaxed);
            core::ptr::addr_of_mut!(RELEASED).write(None);
        }
        if owns() {
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            end(s, 9, t17().wrapping_sub(s.origin) as u32);
        }
    });
}
pub fn tick() {
    #[cfg(not(feature = "bench-lean-irq"))]
    let began = t17();
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::TIM6::ptr()).sr().write(|w| w.bits(0));
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let now = t17().wrapping_sub(s.origin) as u32;
        if !get_idr(3, 1) {
            end(s, 11, now);
            return;
        }
        if let Err(f) = s.guard.as_mut().unwrap().tick(now, get_idr(1, 14), false) {
            end(s, code(f), now);
            return;
        }
        #[cfg(feature = "bench-startup-adc")]
        driven_irq_live::resume_deferred();
        #[cfg(not(feature = "bench-lean-irq"))]
        if !pwm_sample_dma::healthy() {
            end(s, 20, now);
            return;
        }
        #[cfg(not(feature = "bench-lean-irq"))]
        if s.reads == N {
            end(s, 17, now);
            return;
        }
        #[cfg(not(feature = "bench-lean-irq"))]
        {
            // This polling microscope has no handoff authority. The live seed
            // comes from driven_irq_live in comp_irq, not from this observer.
            let before = t17().wrapping_sub(s.origin) as u32;
            let pwm = (*stm32::TIM1::ptr()).cnt().read().bits() as u16;
            let level = core::ptr::read_volatile(COMP2_CSR) & (1 << 30) == 0;
            let after = t17().wrapping_sub(s.origin) as u32;
            let result = s
                .observer
                .as_mut()
                .unwrap()
                .sample(s.epoch, s.epoch, before, after, level);
            let status = match result {
                Ok(Some(_)) => 1,
                Ok(None) => 0,
                Err(_) => 2,
            };
            core::ptr::addr_of_mut!(READS)
                .cast::<[u16; 4]>()
                .add(s.reads)
                .write([
                    before as u16,
                    pwm,
                    s.step as u16
                        | ((level as u16) << 3)
                        | (status << 4)
                        | ((after.wrapping_sub(before).min(255) as u16) << 8),
                    s.epoch as u16,
                ]);
            s.reads += 1;
            s.tick_max = s.tick_max.max(t17().wrapping_sub(began));
        }
    });
}
#[cfg(feature = "bench-driven-irq")]
pub fn comp_irq() {
    cortex_m::interrupt::free(|_| unsafe {
        if !owns() {
            driven_irq_live::stop();
            return;
        }
        if !driven_irq_live::interrupt() {
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            end(s, 21, t17().wrapping_sub(s.origin) as u32);
        }
        #[cfg(feature = "bench-driven-handoff")]
        if owns() && TRANSFER_ARMED.load(Relaxed) {
            if let Some(seed) = driven_irq_live::seed() {
                let s = &mut *core::ptr::addr_of_mut!(STATE);
                let now = t17().wrapping_sub(s.origin) as u32;
                let valid = get_idr(3, 1)
                    && get_idr(1, 14)
                    && seed.step == s.step
                    && s.guard.as_mut().unwrap().authorize(now, s.step).is_ok();
                let feedback = core::ptr::addr_of!(LAST_FEEDBACK).read();
                if !valid || feedback.is_none() {
                    end(s, 23, now);
                    return;
                }
                let (sample, acquired) = feedback.unwrap();
                let token = Transfer {
                    seed,
                    origin: s.origin,
                    sample,
                    acquired,
                    began: s.origin.wrapping_add(s.start as u16),
                    released: t17(),
                    epoch: TRANSFER_EPOCH.load(Relaxed),
                };
                if !token.fresh() {
                    end(s, 23, now);
                    return;
                }
                TRANSFER_TIMES = [
                    seed.step as u16,
                    seed.edge_tick as u16,
                    seed.interval_ticks as u16,
                    token.released.wrapping_sub(s.origin),
                    0,
                    0,
                    0,
                ];
                TRANSFER_ARMED.store(false, Relaxed);
                end(s, 22, now);
                core::ptr::addr_of_mut!(RELEASED).write(Some(token));
            }
        }
    });
}
pub fn sector() {
    let began = t17();
    cortex_m::interrupt::free(|_| unsafe {
        (*stm32::TIM3::ptr()).sr().write(|w| w.bits(0));
        if !owns() {
            return;
        }
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        let now = t17().wrapping_sub(s.origin) as u32;
        let late = now.wrapping_sub(s.deadline);
        if !cfg!(feature = "bench-startup-adc") && late > 50 {
            end(s, 15, now);
            return;
        }
        s.late_max = s.late_max.max(late);
        if !get_idr(3, 1) || !get_idr(1, 14) {
            end(s, 7, now);
            return;
        }
        let next = s.step % 6 + 1;
        if let Err(f) = s.guard.as_mut().unwrap().authorize(now, next) {
            end(s, code(f), now);
            return;
        }
        let theta = s.theta.wrapping_add(s.rate.wrapping_mul(s.deadline));
        let Some(plan) = phase_schedule::next(theta, s.rate) else {
            end(s, 16, now);
            return;
        };
        if plan.step != next {
            end(s, 16, now);
            return;
        }
        // The old50us margin is a report for practical startup. Do not issue
        // this role if its entire following sector has already elapsed.
        // Original ideal deadlines are retained, never refreshed to hide lag.
        if cfg!(feature = "bench-startup-adc") && late >= plan.delay_us as u32 {
            end(s, 15, now);
            return;
        }
        // Serialized authorize+write+observer epoch: stop cannot race a commit.
        if !cfg!(feature = "bench-lean-irq") && s.epoch >= 25 {
            end(s, 17, now);
            return;
        }
        let dma_before = pwm_sample_dma::count();
        sixstep_write(next, s.duty);
        s.step = next;
        s.epoch += 1;
        let changed = t17().wrapping_sub(s.origin) as u32;
        let dma_after = pwm_sample_dma::count();
        if s.epoch < 26 {
            core::ptr::addr_of_mut!(DMA_BOUNDS)
                .cast::<[u16; 3]>()
                .add(s.epoch as usize)
                .write([s.epoch as u16, dma_before, dma_after]);
            core::ptr::addr_of_mut!(COMMANDS)
                .cast::<[u16; 3]>()
                .add(s.epoch as usize)
                .write([s.epoch as u16, next as u16, changed as u16]);
        }
        #[cfg(not(feature = "bench-lean-irq"))]
        if !s.observer.as_mut().unwrap().command(s.epoch, next, changed) {
            end(s, 14, changed);
            return;
        }
        #[cfg(feature = "bench-driven-irq")]
        if !driven_irq_live::command(s.epoch as u16, next) {
            end(s, 21, changed);
            return;
        }
        (*stm32::TIM3::ptr())
            .arr()
            .write(|w| w.bits(plan.delay_us as u32 - 1));
        s.deadline += plan.delay_us as u32;
        s.sector_max = s.sector_max.max(t17().wrapping_sub(began));
    });
}
pub fn run(
    hz: u32,
    duty: u32,
    campaign_start: u32,
    vcal: u32,
    abort: &mut dyn FnMut() -> bool,
) -> (usize, u8, u32) {
    let selected = NEXT_DUTY.swap(0, Relaxed);
    let duty = if selected == 0 { duty } else { selected };
    APPLIED_DUTY.store(duty, Relaxed);
    let bemf_duty = take_bemf_duty(duty);
    APPLIED_BEMF_DUTY.store(bemf_duty, Relaxed);
    let shift = NEXT_PHASE.swap(0, Relaxed);
    APPLIED_PHASE.store(shift, Relaxed);
    // Restrict FIRST powered qualification to already-tested200eHz/<=6.2%.
    // This is an experiment setting,not a new goal ceiling.
    let allowed = hz == 200
        && (40..=ACQUISITION_DUTY_MAX).contains(&duty)
        && get_idr(3, 1)
        && !owns()
        && !driven_probe::owns()
        && !powered_timer::owns()
        && !core_bench::active()
        && unsafe { (*stm32::TIM3::ptr()).cr1().read().bits() & 1 == 0 }
        && clock_us().wrapping_sub(campaign_start) < 4_950_000;
    #[cfg(not(feature = "bench-startup-adc"))]
    gates_off(); // captures the startup phase/time once,keeps ENABLE awake
    #[cfg(feature = "bench-startup-adc")]
    if allowed {
        gates_off_keep_adc();
    } else {
        gates_off();
    }
    DMA_USED.store(false, Relaxed);
    unsafe {
        core::ptr::addr_of_mut!(COAST_ORIGIN).write([0, 0, 0]);
    }
    unsafe {
        core::ptr::addr_of_mut!(ALIGNMENT).write([0; 3]);
    }
    #[cfg(feature = "bench-startup-adc")]
    unsafe {
        ADC_STOP_STATE = [0; 12];
    }
    let anchor = wave_timer::stop_anchor();
    #[cfg(feature = "bench-driven-handoff")]
    {
        let armed = NEXT_TRANSFER.swap(false, Relaxed);
        TRANSFER_ARMED.store(armed, Relaxed);
        TRANSFER_RESULT.store(0, Relaxed);
        SCAN_YIELD.store(0, Relaxed);
        unsafe {
            RELEASED = None;
            LAST_FEEDBACK = None;
            TRANSFER_TIMES = [0; 7];
        }
        if armed && allowed {
            #[cfg(feature = "bench-current-baseline")]
            prestart_baseline::entry_check();
            core_bench::prepare_driven(hz);
        }
    }
    #[cfg(feature = "bench-driven-irq")]
    driven_irq_live::reset(anchor.2);
    #[cfg(not(feature = "bench-startup-adc"))]
    let baseline_at = t17();
    #[cfg(not(feature = "bench-startup-adc"))]
    let baseline = powered_timer::sample_feedback(vcal);
    #[cfg(feature = "bench-startup-adc")]
    let (baseline, baseline_at) = match adc_stream::startup_feedback(vcal) {
        Some((sample, at)) => (Some(sample), at),
        None => (None, t17()),
    };
    #[cfg(feature = "bench-driven-handoff")]
    unsafe {
        LAST_FEEDBACK = baseline.map(|v| (v, baseline_at));
    }
    let saved_comp = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    comp_input::stop();
    cortex_m::interrupt::free(|_| unsafe {
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        *s = State {
            guard: None,
            observer: Some(driven_observer::Observer::new()),
            origin: anchor.2,
            theta: campaign::phase_shift(anchor.1, shift),
            rate: campaign::phase_rate(hz * 100),
            duty,
            deadline: 0,
            first: 0,
            start: 0,
            step: 0,
            epoch: 0,
            reason: 0,
            stop: 0,
            reads: 0,
            scans: 0,
            tick_max: 0,
            sector_max: 0,
            late_max: 0,
            age_max: 0,
        };
        let now = t17().wrapping_sub(s.origin) as u32;
        if !allowed || !anchor.0 || anchor.3 || now > 500 || !get_idr(3, 1) || !get_idr(1, 14) {
            end(s, 18, now);
            return;
        }
        let Some(sample) = baseline else {
            end(s, 13, now);
            return;
        };
        #[cfg(not(feature = "bench-lean-irq"))]
        if !pwm_sample_dma::prepare_comp(duty) {
            end(s, 20, now);
            return;
        }
        #[cfg(feature = "bench-driven-irq")]
        driven_irq_live::prepare();
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | 2 | (1 << 4)));
        let t = &*stm32::TIM3::ptr();
        t.cr1().write(|w| w.bits(4));
        t.dier().write(|w| w.bits(0));
        t.cr2().write(|w| w.bits(0));
        t.ccer().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        let t6 = &*stm32::TIM6::ptr();
        t6.cr1().write(|w| w.bits(4));
        t6.dier().write(|w| w.bits(0));
        t6.psc().write(|w| w.bits(63));
        t6.arr().write(|w| w.bits(49));
        t6.egr().write(|w| w.bits(1));
        t6.sr().write(|w| w.bits(0));
        let mut p = cortex_m::Peripherals::steal();
        p.NVIC.set_priority(stm32::Interrupt::TIM6_DAC_LPTIM1, 0);
        p.NVIC.set_priority(stm32::Interrupt::TIM3, 0x80);
        #[cfg(feature = "bench-driven-irq")]
        p.NVIC.set_priority(stm32::Interrupt::TIM3, 0x40);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM3);
        cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
        let mut now = t17().wrapping_sub(s.origin) as u32;
        let Some(mut plan) =
            phase_schedule::next(s.theta.wrapping_add(s.rate.wrapping_mul(now)), s.rate)
        else {
            end(s, 16, now);
            return;
        };
        let wait = plan.initial_wait();
        ALIGNMENT = [plan.delay_us, wait, 0];
        if wait != 0 {
            if !powered_timer::outputs_disabled() {
                end(s, 19, now);
                return;
            }
            let boundary = now + wait as u32;
            let began = t17();
            while (t17().wrapping_sub(s.origin) as u32) < boundary {}
            ALIGNMENT[2] = t17().wrapping_sub(began);
            now = t17().wrapping_sub(s.origin) as u32;
            if !powered_timer::outputs_disabled() || !get_idr(3, 1) || !get_idr(1, 14) {
                end(s, 19, now);
                return;
            }
            let Some(updated) =
                phase_schedule::next(s.theta.wrapping_add(s.rate.wrapping_mul(now)), s.rate)
            else {
                end(s, 16, now);
                return;
            };
            plan = updated;
        }
        if plan.delay_us < 100 || now > 500 {
            end(s, 19, now);
            return;
        }
        s.first = now + plan.delay_us as u32;
        s.deadline = s.first;
        s.step = plan.step;
        s.start = now;
        s.guard = driven_guard::Guard::new(
            now,
            if cfg!(feature = "bench-startup-adc") {
                40_000
            } else {
                20_000
            },
            s.step,
            sample,
        )
        .ok();
        let Some(g) = s.guard.as_mut() else {
            end(s, 10, now);
            return;
        };
        if g.age_initial_feedback(t17().wrapping_sub(baseline_at) as u32)
            .is_err()
        {
            end(s, 4, now);
            return;
        }
        let commit = t17().wrapping_sub(s.origin) as u32;
        if let Err(f) = g.authorize(commit, s.step) {
            end(s, code(f), commit);
            return;
        }
        ACTIVE.store(true, Relaxed);
        sixstep_write(s.step, duty);
        let changed = t17().wrapping_sub(s.origin) as u32;
        core::ptr::addr_of_mut!(COMMANDS).cast::<[u16; 3]>().write([
            0,
            s.step as u16,
            changed as u16,
        ]);
        s.observer.as_mut().unwrap().command(0, s.step, changed);
        #[cfg(feature = "bench-driven-irq")]
        if !driven_irq_live::command(0, s.step) {
            end(s, 21, changed);
            return;
        }
        core::ptr::addr_of_mut!(DMA_BOUNDS)
            .cast::<[u16; 3]>()
            .write([0, 0, 0]);
        #[cfg(not(feature = "bench-lean-irq"))]
        {
            pwm_sample_dma::start_comp();
            DMA_USED.store(true, Relaxed);
        }
        let arm = t17().wrapping_sub(s.origin) as u32;
        if arm >= s.first || s.first - arm < 32 {
            end(s, 19, arm);
            return;
        }
        t.arr().write(|w| w.bits(s.first - arm - 1));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        t6.dier().write(|w| w.bits(1));
        t6.cr1().write(|w| w.bits(5));
        t.dier().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(5));
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM3);
    });
    #[cfg(feature = "bench-startup-adc")]
    let mut last_scan = baseline_at;
    while owns() {
        if abort() || clock_us().wrapping_sub(campaign_start) >= 4_999_000 {
            cancel();
            break;
        }
        #[cfg(not(feature = "bench-startup-adc"))]
        let acquired = t17();
        #[cfg(all(feature = "bench-driven-handoff", not(feature = "bench-startup-adc")))]
        let sample = powered_timer::sample_driven_feedback(vcal);
        #[cfg(all(
            not(feature = "bench-driven-handoff"),
            not(feature = "bench-startup-adc")
        ))]
        let sample = powered_timer::sample_feedback(vcal);
        #[cfg(feature = "bench-startup-adc")]
        let (sample, acquired) = match adc_stream::startup_feedback(vcal) {
            Some((_, at)) if at == last_scan => continue,
            Some((sample, at)) => {
                last_scan = at;
                (Some(sample), at)
            }
            None => (None, t17()),
        };
        #[cfg(all(feature = "bench-startup-adc", feature = "bench-lean-irq"))]
        cortex_m::interrupt::free(|_| unsafe {
            if !owns() {
                return;
            }
            let Some(sample) = sample else {
                let s = &mut *core::ptr::addr_of_mut!(STATE);
                let now = t17().wrapping_sub(s.origin) as u32;
                end(s, 13, now);
                return;
            };
            // DMA already publishes validated guard feedback once per frame.
            // Retain the original-age handoff token, not a masked diagnostic
            // row/stats update on every frame or a recorder-capacity stop.
            LAST_FEEDBACK = Some((sample, acquired));
        });
        #[cfg(not(all(feature = "bench-startup-adc", feature = "bench-lean-irq")))]
        cortex_m::interrupt::free(|_| unsafe {
            if !owns() {
                return;
            }
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            let now = t17().wrapping_sub(s.origin) as u32;
            let Some(sample) = sample else {
                end(s, 13, now);
                return;
            };
            let age = t17().wrapping_sub(acquired) as u32;
            if s.scans == A {
                end(s, 17, now);
                return;
            }
            core::ptr::addr_of_mut!(ADC)
                .cast::<[u16; 7]>()
                .add(s.scans)
                .write([
                    acquired.wrapping_sub(s.origin),
                    age as u16,
                    sample.phase[0],
                    sample.phase[1],
                    sample.phase[2],
                    sample.bus_mv.min(65535) as u16,
                    sample.vref,
                ]);
            s.scans += 1;
            s.age_max = s.age_max.max(age);
            // Preserve the offending foreground sample before either guard.
            // Average-current refusal is distinct from legacy pulse Current5.
            #[cfg(all(feature = "bench-average-current", not(feature = "bench-startup-adc")))]
            if !average_current_live::scan(sample.phase) {
                end(s, 25, now);
                return;
            }
            // Keep the offending raw feedback row as well as successful scans.
            #[cfg(not(feature = "bench-startup-adc"))]
            if let Err(f) = s.guard.as_mut().unwrap().feedback(now, age, sample) {
                end(s, code(f), now);
                return;
            }
            #[cfg(feature = "bench-driven-handoff")]
            {
                LAST_FEEDBACK = Some((sample, acquired));
            }
        });
    }
    #[cfg(feature = "bench-driven-handoff")]
    {
        let released = cortex_m::interrupt::free(|_| unsafe {
            (&mut *core::ptr::addr_of_mut!(RELEASED)).take()
        });
        if let Some(token) = released {
            unsafe {
                TRANSFER_TIMES[4] = t17().wrapping_sub(token.origin);
                TRANSFER_TIMES[5] = t17().wrapping_sub(token.acquired);
                TRANSFER_TIMES[6] = 1;
            }
            let ran = core_bench::driven_power_run(token, campaign_start, vcal, bemf_duty, abort);
            TRANSFER_RESULT.store(if ran { 1 } else { 2 }, Relaxed);
        }
    }
    gates_off();
    set_pin(3, 1, false);
    comp_input::stop();
    // Shared stop has revoked ownership; pending callbacks cannot write gates.
    sector();
    tick();
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, saved_comp);
    }
    let s = unsafe { &*core::ptr::addr_of!(STATE) };
    // Nonzero marker ensures refusal diagnostics also reach fixture-only dump.
    (1, if s.reason == 2 { 1 } else { 8 }, s.stop)
}
/// Foreground, after coast and safing only. Never reuse a previous BEMF
/// snapshot when this attempt failed before transfer. Raw current is not amps.
pub fn terminal_summary<W: Write>(out: &mut W) {
    if owns() || powered_timer::owns() || get_idr(3, 1) || !powered_timer::outputs_disabled() {
        return;
    }
    let s = unsafe { &*core::ptr::addr_of!(STATE) };
    let _ = writeln!(
        out,
        "DRIVESTOP acquisition_reason={} acquisition_us={} acquisition_duty_tenths={} bemf_requested_tenths={} disabled=1",
        s.reason,
        s.stop.saturating_sub(s.start),
        s.duty,
        APPLIED_BEMF_DUTY.load(Relaxed)
    );
    // Walk the existing retained acquisition records AFTER shutdown; no extra
    // work, arithmetic, or instrumentation in the powered path.
    let mut peak = 0u16;
    let mut bus = u16::MAX;
    for i in 0..s.scans {
        let row = unsafe { core::ptr::addr_of!(ADC).cast::<[u16; 7]>().add(i).read() };
        for raw in &row[2..5] {
            peak = peak.max(raw.abs_diff(2048));
        }
        bus = bus.min(row[5]);
    }
    let _ = writeln!(
        out,
        "DRIVEADC scans={} peak_abs_raw={} bus_min_mv={} uncalibrated=1 zero_scans_unavailable=1",
        s.scans,
        peak,
        if s.scans == 0 { 0 } else { bus }
    );
    #[cfg(feature = "bench-driven-handoff")]
    {
        let transfer = TRANSFER_RESULT.load(Relaxed);
        let _ = writeln!(
            out,
            "DRIVETRANSFER result={} zero_not_attempted=1 two_refused=1",
            transfer
        );
        core_bench::driven_entry_summary(out);
        if transfer == 1 {
            powered_timer::summary(out);
            core_bench::terminal_summary(out);
        }
    }
}
pub fn dump<W: Write>(out: &mut W) {
    #[cfg(feature = "bench-startup-adc")]
    if unsafe { ADC_STOP_STATE[0] != 0 } {
        let _ = snapshot::record(out, "AS85", unsafe {
            &*core::ptr::addr_of!(ADC_STOP_STATE)
        });
    }
    let s = unsafe { &*core::ptr::addr_of!(STATE) };
    let o = s.observer.as_ref().unwrap();
    let _ = writeln!(
        out,
        "DRIVEPHASE applied_degrees={} commanded_only=1",
        APPLIED_PHASE.load(Relaxed)
    );
    let alignment = unsafe { core::ptr::addr_of!(ALIGNMENT).read() };
    let _ = writeln!(
        out,
        "DRIVEALIGN initial_remaining_us={} requested_wait_us={} actual_wait_us={} gates_disabled_during_wait=1 elapsed_phase=1",
        alignment[0], alignment[1], alignment[2]
    );
    if s.reason == 22 {
        let _ = writeln!(
            out,
            "DRIVENCOAST unavailable=1 acquisition_release_not_final_stop=1"
        );
    } else {
        let _ = writeln!(
            out,
            "DRIVENCOAST fields=valid,delay_min_us,delay_max_us t17_brackets_coast_origin=1"
        );
        let origin = unsafe { core::ptr::addr_of!(COAST_ORIGIN).read() };
        let _ = snapshot::record(out, "CB85", &origin);
    }
    let _ = writeln!(
        out,
        "DRIVEOBS reason={} start_us={} stop_us={} theta={} rate={} duty_tenths={} first_us={} commands={} reads={} scans={} tick_max_us={} sector_max_us={} late_max_us={} age_max_us={} candidates={} missed={} handoff_authority=0 disabled={}",
        s.reason,
        s.start,
        s.stop,
        s.theta,
        s.rate,
        s.duty,
        s.first,
        s.epoch,
        s.reads,
        s.scans,
        s.tick_max,
        s.sector_max,
        s.late_max,
        s.age_max,
        o.candidates,
        o.missed,
        (powered_timer::outputs_disabled() && !get_idr(3, 1)) as u8
    );
    for i in 0..s.reads {
        let row = unsafe { core::ptr::addr_of!(READS).cast::<[u16; 4]>().add(i).read() };
        let _ = snapshot::record(out, "DQ85", &row);
    }
    for i in 0..s.scans {
        let row = unsafe { core::ptr::addr_of!(ADC).cast::<[u16; 7]>().add(i).read() };
        let _ = snapshot::record(out, "DA85", &row);
    }
    if s.reads > 0 {
        for i in 0..=s.epoch.min(25) as usize {
            let row = unsafe {
                core::ptr::addr_of!(COMMANDS)
                    .cast::<[u16; 3]>()
                    .add(i)
                    .read()
            };
            let _ = snapshot::record(out, "DC85", &row);
        }
    }
    if DMA_USED.load(Relaxed) {
        pwm_sample_dma::dump_comp(out);
        for i in 0..=s.epoch.min(25) as usize {
            let row = unsafe {
                core::ptr::addr_of!(DMA_BOUNDS)
                    .cast::<[u16; 3]>()
                    .add(i)
                    .read()
            };
            let _ = snapshot::record(out, "PD85", &row);
        }
    }
    let _ = writeln!(out, "DRIVEOBS END");
    #[cfg(feature = "bench-driven-irq")]
    driven_irq_live::dump(out);
    #[cfg(feature = "bench-driven-handoff")]
    if s.reason == 22 || TRANSFER_RESULT.load(Relaxed) != 0 {
        let yielded = SCAN_YIELD.load(Relaxed);
        let _ = writeln!(
            out,
            "DRIVENYIELD abandoned={} completed_channels={} partial_feedback_published=0",
            (yielded != 0) as u8,
            yielded.saturating_sub(1)
        );
        let _ = writeln!(
            out,
            "DUTYSPLIT acquisition={} bemf={} units=tenths_percent segment_fixed=1",
            s.duty,
            APPLIED_BEMF_DUTY.load(Relaxed)
        );
        let _ = writeln!(
            out,
            "DRIVEX result={} power_reason={} fresh_transfer=1",
            TRANSFER_RESULT.load(Relaxed),
            powered_timer::reason()
        );
        let _ = writeln!(
            out,
            "DX85FIELDS step,edge_half_us,mean_half_us,release_us,foreground_us,feedback_age_us,foreground_entered"
        );
        let _ = snapshot::record(out, "DX85", unsafe {
            &*core::ptr::addr_of!(TRANSFER_TIMES)
        });
        // The shell emits the unique core summary before this driven dump.
        powered_timer::summary(out);
        // dump() is fixture-only and runs after shutdown. The same coverage
        // collected by the powered reader was previously omitted on this path.
        powered_timer::coverage_dump(out);
        #[cfg(feature = "bench-current-baseline")]
        prestart_baseline::dump(out);
        core_bench::driven_first_dump(out);
    }
}
