//! Disabled integration only: real COMP body and board's real guard ISR.
use super::super::*;
use super::Window;
use portable_atomic::AtomicU32;

static CASE: AtomicU32 = AtomicU32::new(0);
static DONE: AtomicU32 = AtomicU32::new(0);
static WINDOW: AtomicU32 = AtomicU32::new(0);
static RESUME: AtomicU32 = AtomicU32::new(0);
static REASON: AtomicU32 = AtomicU32::new(0);
static BEFORE: AtomicU32 = AtomicU32::new(0);
static AFTER: AtomicU32 = AtomicU32::new(0);
static COM_CALLS: AtomicU32 = AtomicU32::new(0);
static MASKED: AtomicU32 = AtomicU32::new(0);

fn off() -> bool {
    !hw::gpio::enable_is_high() && !hw::pwm::moe_is_set()
        && hw::pwm::compares() == (0, 0, 0)
}

fn snapshot() -> u32 {
    u32::from(S.com().stopped.load(Ordering::Relaxed))
        | (u32::from(S.com().active.load(Ordering::Relaxed)) << 1)
        | (u32::from(hw::com_timer::is_running()) << 2)
        | (u32::from(hw::nvic::is_pending(stm32::Interrupt::TIM16)) << 3)
        | (u32::from(off()) << 4)
}

pub struct ProbeWindow;
impl Window for ProbeWindow {
    const MASKED: bool = true;
    #[inline(always)]
    fn run<T>(f: impl FnOnce() -> T) -> T {
        let mut request = 0;
        let result = cortex_m::interrupt::free(|_| {
            let begin = hw::fine::raw();
            let case = CASE.load(Ordering::Relaxed);
            if (1..=3).contains(&case) {
                S.guard().last_tick.store(S.guard().ext.load(Ordering::Relaxed).wrapping_sub(300), Ordering::Relaxed);
                S.guard().ticks.store(2, Ordering::Relaxed);
                S.guard().active.store(true, Ordering::Relaxed);
                request = hw::fine::raw();
                hw::nvic::pend(stm32::Interrupt::TIM6_DAC_LPTIM1);
            }
            if case == 3 {
                let raw = hw::clock::raw();
                for _ in 0..1000 {
                    if hw::clock::raw().wrapping_sub(raw) >= 20 { break; }
                    cortex_m::asm::nop();
                }
            }
            if case == 4 { hw::nvic::pend(stm32::Interrupt::TIM16); }
            let value = f();
            WINDOW.store(hw::fine::raw().wrapping_sub(begin), Ordering::Relaxed);
            MASKED.store(u32::from(cortex_m::register::primask::read().is_inactive()), Ordering::Relaxed);
            BEFORE.store(snapshot(), Ordering::Relaxed);
            value
        });
        if request != 0 { RESUME.store(hw::fine::raw().wrapping_sub(request), Ordering::Relaxed); }
        result
    }
}

/// # Safety
/// Actual ADC_COMP vector at CompPrio, with bridge disabled by this binary.
/// Keep the entry generic: changing a concrete library entry to this form
/// restored archived motor bytes in E478. This is artifact-scoped evidence,
/// not a guarantee about future optimizer decisions. Audit each binary.
pub unsafe fn comp_interrupt<W: Window>() {
    if DONE.load(Ordering::Relaxed) != 0 || !off() {
        guard_trip(Reason::HostAbort); return;
    }
    // SAFETY: same unique vector and priority as this function's contract.
    unsafe { comp_root_window::<crate::capture::NoLog, crate::chain::NoChain,
        crate::bemf::ConstantAdvance<crate::bemf::FreshEstimate, 16>,
        recheck::Timed, W>() };
    REASON.store(S.guard().reason.load(Ordering::Relaxed), Ordering::Relaxed);
    AFTER.store(snapshot(), Ordering::Relaxed);
    comp_exti_mask();
    DONE.store(1, Ordering::Release);
}

/// No role writes or enables: a pending COM is counted, then stopped.
pub fn com_interrupt() {
    hw::com_timer::ack();
    COM_CALLS.fetch_add(1, Ordering::Relaxed);
    com_stop();
}

fn prepare(case: u32) -> bool {
    if !off() || S.guard().active.load(Ordering::Relaxed) || S.det().active.load(Ordering::Relaxed) { return false; }
    cortex_m::interrupt::free(|_| {
        comp_exti_mask(); com_stop();
        S.guard().reason.store(0, Ordering::Relaxed);
        S.guard().tracking.store(false, Ordering::Relaxed);
        hw::nvic::set_priority(stm32::Interrupt::TIM6_DAC_LPTIM1, GUARD_IRQ_PRIORITY);
        hw::nvic::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
        hw::nvic::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        // Disabled fixture only: HAL RefintInput::VRefint is INMSEL=3.
        // Phase-vs-neutral at all-off did not provide a stable test level.
        hw::comp::select_negative(3);
        cortex_m::asm::delay(640);
        let seed = if case == 3 { 40 } else { 1000 };
        if S.det().zc.lock(|zc| *zc = Some(crate::bemf::ZeroCross::new_bounded(seed, 1, 4000))).is_none() { return false; }
        let level = hw::comp::level() ^ (case == 2);
        let step = if edge_is_rising(Step::new_clamped(1)) == level { 1 } else { 2 };
        S.det().step.store(step, Ordering::Relaxed);
        S.det().filter_depth.store(12, Ordering::Relaxed);
        S.det().sector_start_raw.store(u32::from(hw::clock::raw().wrapping_sub(seed as u16)), Ordering::Relaxed);
        S.com().stopped.store(false, Ordering::Relaxed);
        S.com().active.store(true, Ordering::Relaxed);
        S.com().phase.store(4, Ordering::Relaxed);
        S.det().active.store(true, Ordering::Relaxed);
        CASE.store(case, Ordering::Relaxed);
        for item in [&DONE, &WINDOW, &RESUME, &BEFORE, &AFTER, &REASON, &COM_CALLS, &MASKED] { item.store(0, Ordering::Relaxed); }
        hw::nvic::unmask(stm32::Interrupt::TIM16);
        hw::nvic::unmask(stm32::Interrupt::ADC_COMP);
        hw::comp::pend();
        true
    })
}

pub fn run(case: u32, sink: &mut impl crate::report::Sink) -> bool {
    if case > 4 || !prepare(case) { return false; }
    let raw = hw::clock::raw();
    for _ in 0..100_000 {
        if DONE.load(Ordering::Acquire) != 0 || hw::clock::raw().wrapping_sub(raw) > 2000 { break; }
        cortex_m::asm::nop();
    }
    let reason = REASON.load(Ordering::Relaxed);
    let before = BEFORE.load(Ordering::Relaxed);
    let after = AFTER.load(Ordering::Relaxed);
    let counts = S.det().zc.lock(|zc| zc.as_ref().map(|v| v.counts())).flatten();
    let expected_counts = if case == 2 { (0, 0, 1) } else { (1, 0, 0) };
    let off_before = off();
    let gates_before = hw::gpio::gates_all_low();
    let gp = cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM6_DAC_LPTIM1);
    let cp = cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::ADC_COMP);
    let tp = cortex_m::peripheral::NVIC::get_priority(stm32::Interrupt::TIM16);
    let expected = match case { 0 | 4 => reason == 0 && before == 22 && after == 22,
        1 => reason == 3 && before == 22 && after == 17,
        2 => reason == 3 && before == 18 && after == 17,
        _ => reason == 15 && before == 17 && after == 17 };
    let ok = DONE.load(Ordering::Acquire) == 1 && MASKED.load(Ordering::Relaxed) == 1
        && WINDOW.load(Ordering::Relaxed) > 0 && expected && counts == Some(expected_counts)
        && COM_CALLS.load(Ordering::Relaxed) == 0 && off_before && gates_before
        && (gp, tp, cp) == (0, 0x40, 0x80);
    guard_trip(Reason::HostAbort);
    hw::nvic::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    hw::nvic::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
    let counts = counts.unwrap_or((u32::MAX, u32::MAX, u32::MAX));
    sink.say("ACCEPTWINDOW "); sink.kv("case", case); sink.kv("window_body125ns", WINDOW.load(Ordering::Relaxed));
    sink.kv("resume125ns", RESUME.load(Ordering::Relaxed)); sink.kv("reason", reason);
    sink.kv("before", before); sink.kv("after", after); sink.kv("com_calls", COM_CALLS.load(Ordering::Relaxed));
    sink.kv("masked", MASKED.load(Ordering::Relaxed)); sink.kv("off", u32::from(off()));
    sink.kv("accepted", counts.0); sink.kv("early", counts.1); sink.kv("unstable", counts.2);
    sink.kv("off_before", u32::from(off_before)); sink.kv("gates_before", u32::from(gates_before));
    sink.kv("guard_prio", u32::from(gp)); sink.kv("com_prio", u32::from(tp)); sink.kv("comp_prio", u32::from(cp));
    sink.kv("pass", u32::from(ok)); sink.say("\r\n");
    ok
}
