//! Driver-disabled peripheral exercise. Linked only by recheck-off binary.
use super::*;
use portable_atomic::AtomicU32;

pub mod lifecycle;

static CASE: AtomicU32 = AtomicU32::new(0);
static DONE: AtomicU32 = AtomicU32::new(0);
static TICKS: AtomicU32 = AtomicU32::new(0);
static PHASE: AtomicU32 = AtomicU32::new(0);
static ARMED: AtomicU32 = AtomicU32::new(0);

static INPUT_NOW: AtomicU32 = AtomicU32::new(0);
static INPUT_AGE: AtomicU32 = AtomicU32::new(0);
static INPUT_AVG: AtomicU32 = AtomicU32::new(0);
static INPUT_BITS: AtomicU32 = AtomicU32::new(0);

struct Capture;
impl Observer for Capture {
    fn inputs(now: u32, v: crate::revisit::Inputs, admitted: bool) {
        INPUT_NOW.store(now, Ordering::Relaxed);
        INPUT_AGE.store(v.elapsed_us, Ordering::Relaxed);
        INPUT_AVG.store(v.average_us, Ordering::Relaxed);
        // present/line/pending/post/admitted, low five bits respectively.
        let bits = 1 | (u32::from(v.line_live) << 1) | (u32::from(v.pending) << 2)
            | (u32::from(v.post_level) << 3) | (u32::from(admitted) << 4);
        INPUT_BITS.store(bits, Ordering::Relaxed);
    }
}

fn off() -> bool {
    !hw::gpio::enable_is_high() && !hw::pwm::moe_is_set()
        && hw::pwm::compares() == (0, 0, 0)
}

fn inactive() -> bool {
    off() && !S.guard().active.load(Ordering::Relaxed)
        && !S.det().active.load(Ordering::Relaxed) && !S.com().active.load(Ordering::Relaxed)
        && !S.drv().active.load(Ordering::Relaxed)
}

/// Test vector: never calls the commutating COM root or changes a gate role.
/// # Safety
/// Only TIM16 at Motor::NVIC may call this, in the driver-disabled probe.
pub unsafe fn interrupt() {
    if CASE.load(Ordering::Relaxed) >= 6 {
        // SAFETY: this function's vector/priority contract is unchanged.
        unsafe { lifecycle::interrupt() };
        return;
    }
    hw::com_timer::ack();
    hw::nvic::mask(stm32::Interrupt::TIM16);
    if !off() { guard_trip(Reason::HostAbort); DONE.store(2, Ordering::Release); return; }
    // SAFETY: caller is the unique TIM16 vector at the declared priority.
    let mut at = unsafe { Root::<Motor>::enter() };
    let start = hw::fine::raw();
    after_phase_observed::<Capture>(&mut at, if CASE.load(Ordering::Relaxed) == 0 { 1 } else { 4 });
    TICKS.store(hw::fine::raw().wrapping_sub(start), Ordering::Relaxed);
    PHASE.store(S.com().phase.load(Ordering::Relaxed), Ordering::Relaxed);
    ARMED.store(u32::from(hw::com_timer::is_running()), Ordering::Relaxed);
    guard_trip(Reason::HostAbort);
    hw::nvic::unpend(stm32::Interrupt::TIM16);
    DONE.store(if off() { 1 } else { 2 }, Ordering::Release);
}

/// Synthetic authority only with all real drive owners inactive and bridge off.
fn prepare(case: u32) -> bool {
    if !inactive() { return false; }
    cortex_m::interrupt::free(|_| {
        if !inactive() { return false; }
        comp_exti_mask();
        hw::nvic::mask(stm32::Interrupt::TIM16);
        hw::com_timer::stop();
        hw::nvic::unpend(stm32::Interrupt::TIM16);
        let raw = hw::clock::raw();
        S.guard().raw.store(u32::from(raw), Ordering::Relaxed);
        S.guard().ext.store(0, Ordering::Relaxed);
        let now = 0u32;
        let age = if case == 0 || case >= 6 { 0 } else if case == 2 { 400 } else { 60 };
        let avg = if case == 0 || case >= 6 { 1000 } else { 80 };
        let step = if hw::comp::level() == edge_is_rising(Step::new_clamped(1)) { 1 } else { 2 };
        let generation = S.det().accept_seq.load(Ordering::Relaxed);
        if STATE.lock(|s| {
            s.schedule = Some(Schedule::new(if case == 3 { generation.wrapping_sub(1) } else { generation },
                step, now.wrapping_sub(age), avg));
            s.requests = 0; s.observations = 0; s.retired = 0;
        }).is_none() { return false; }
        S.det().accept_raw.store(u32::from(raw.wrapping_sub(age as u16)), Ordering::Relaxed);
        S.det().accept_avg.store(avg, Ordering::Relaxed);
        S.det().step.store(step, Ordering::Relaxed);
        S.guard().reason.store(0, Ordering::Relaxed);
        S.com().active.store(true, Ordering::Relaxed);
        S.com().stopped.store(case == 4, Ordering::Relaxed);
        S.com().phase.store(if case == 0 { 0 } else if case == 5 { 3 } else { 4 }, Ordering::Relaxed);
        S.det().active.store(true, Ordering::Relaxed);
        hw::comp::line_enable();
        hw::nvic::mask(stm32::Interrupt::ADC_COMP);
        hw::comp::clear_pending();
        INPUT_BITS.store(0, Ordering::Relaxed);
        CASE.store(case, Ordering::Relaxed); DONE.store(0, Ordering::Relaxed);
        if case >= 6 { lifecycle::arm(case); } else { hw::nvic::pend(stm32::Interrupt::TIM16); }
        hw::nvic::unmask(stm32::Interrupt::TIM16);
        true
    })
}

/// One bounded off-only test. No enables or live control commands exist here.
pub fn run(case: u32, sink: &mut impl crate::report::Sink) -> bool {
    if case > 5 || !prepare(case) { return false; }
    let start = hw::clock::raw();
    for _ in 0..100_000 {
        if DONE.load(Ordering::Acquire) != 0 || hw::clock::raw().wrapping_sub(start) >= 2000 { break; }
        cortex_m::asm::nop();
    }
    cortex_m::interrupt::free(|_| {
        guard_trip(Reason::HostAbort);
        hw::nvic::mask(stm32::Interrupt::TIM16);
        hw::nvic::unpend(stm32::Interrupt::TIM16);
    });
    let counts = STATE.lock(|s| (s.requests, s.observations, s.retired)).unwrap_or((u32::MAX, 0, 0));
    let phase = PHASE.load(Ordering::Relaxed);
    let armed = ARMED.load(Ordering::Relaxed);
    let expected = match case { 0 => (0, 4, 1), 1 => (1, 4, 1), 2 => (1, 0, 0),
        3 => (0, 4, 0), 4 => (0, 4, 0), _ => (0, 3, 0) };
    let ok = DONE.load(Ordering::Acquire) == 1 && off() && (counts.0, phase, armed) == expected;
    sink.say("RECHECKOFF "); sink.kv("case", case); sink.kv("ticks125ns", TICKS.load(Ordering::Relaxed));
    sink.kv("requests", counts.0); sink.kv("observations", counts.1); sink.kv("retired", counts.2);
    sink.kv("phase", phase); sink.kv("armed", armed); sink.kv("off", u32::from(off()));
    sink.kv("pass", u32::from(ok)); sink.say("\r\n");
    sink.say("RECHECKINPUT "); sink.kv("case", case);
    sink.kv("now", INPUT_NOW.load(Ordering::Relaxed)); sink.kv("age", INPUT_AGE.load(Ordering::Relaxed));
    sink.kv("avg", INPUT_AVG.load(Ordering::Relaxed)); sink.kv("bits", INPUT_BITS.load(Ordering::Relaxed));
    sink.say("\r\n");
    ok
}
