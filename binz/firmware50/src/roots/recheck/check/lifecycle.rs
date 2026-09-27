//! Disabled integration tests: autonomous expiry, obsolete pending, stop latch.
use super::*;

static CALLS: AtomicU32 = AtomicU32::new(0);
static MAX_TICKS: AtomicU32 = AtomicU32::new(0);
static FLAGS: AtomicU32 = AtomicU32::new(0);
static EXPIRED: AtomicU32 = AtomicU32::new(0);
static CROSS_RAW: AtomicU32 = AtomicU32::new(0);
static TIMES: [AtomicU32; 5] = [const { AtomicU32::new(0) }; 5];

/// Called only by prepare under exclusion, with outputs off and owners synthetic.
pub(super) fn arm(case: u32) {
    CALLS.store(0, Ordering::Relaxed); MAX_TICKS.store(0, Ordering::Relaxed);
    FLAGS.store(0, Ordering::Relaxed); EXPIRED.store(0, Ordering::Relaxed);
    if case == 6 { com_arm(501, 4); return; }
    com_arm(4000, 4);
    hw::nvic::pend(stm32::Interrupt::TIM16);
    hw::comp::pend();
    let old_pending = hw::nvic::is_pending(stm32::Interrupt::TIM16);
    let raw = hw::clock::raw(); CROSS_RAW.store(u32::from(raw), Ordering::Relaxed);
    if case == 8 { guard_trip(Reason::HostAbort); }
    let armed = com_arm_crossing(raw, 1000).is_some();
    let cleared = !hw::nvic::is_pending(stm32::Interrupt::TIM16);
    let comp_clear = !hw::nvic::is_pending(stm32::Interrupt::ADC_COMP);
    FLAGS.store(u32::from(old_pending) | (u32::from(armed) << 1)
        | (u32::from(cleared) << 2) | (u32::from(comp_clear) << 3), Ordering::Relaxed);
    // Stop already cleared the old pending bit. Inject a new stale dispatch
    // only after witnessing cancellation and the refused crossing arm.
    if case == 8 { hw::nvic::pend(stm32::Interrupt::TIM16); }
}

/// # Safety
/// Real TIM16 vector at Motor::NVIC; bridge remains disabled throughout.
pub(super) unsafe fn interrupt() {
    let expired = hw::com_timer::update_pending();
    hw::com_timer::ack(); hw::nvic::mask(stm32::Interrupt::TIM16);
    let n = CALLS.load(Ordering::Relaxed);
    if !off() || n >= 6 { finish(2); return; }
    CALLS.store(n + 1, Ordering::Relaxed);
    if expired { EXPIRED.store(EXPIRED.load(Ordering::Relaxed) + 1, Ordering::Relaxed); }
    let case = CASE.load(Ordering::Relaxed);
    let elapsed = if case == 6 { guard_now() } else {
        u32::from(hw::clock::raw().wrapping_sub(CROSS_RAW.load(Ordering::Relaxed) as u16))
    };
    if let Some(row) = TIMES.get(n as usize) { row.store(elapsed, Ordering::Relaxed); }
    // SAFETY: reached only from the actual vector at the resource ceiling.
    let mut at = unsafe { Root::<Motor>::enter() };
    let start = hw::fine::raw();
    if case != 7 { after_phase(&mut at, 4); }
    let ticks = hw::fine::raw().wrapping_sub(start);
    MAX_TICKS.store(MAX_TICKS.load(Ordering::Relaxed).max(ticks), Ordering::Relaxed);
    PHASE.store(S.com().phase.load(Ordering::Relaxed), Ordering::Relaxed);
    ARMED.store(u32::from(hw::com_timer::is_running()), Ordering::Relaxed);
    if case == 6 && hw::com_timer::is_running() {
        hw::nvic::unmask(stm32::Interrupt::TIM16);
    } else { finish(1); }
}

fn finish(done: u32) {
    guard_trip(Reason::HostAbort);
    hw::nvic::mask(stm32::Interrupt::TIM16);
    hw::nvic::unpend(stm32::Interrupt::TIM16);
    DONE.store(if off() { done } else { 2 }, Ordering::Release);
}

pub fn run(case: u32, sink: &mut impl crate::report::Sink) -> bool {
    if !(6..=8).contains(&case) || !prepare(case) { return false; }
    let start = hw::clock::raw();
    for _ in 0..100_000 {
        if DONE.load(Ordering::Acquire) != 0 || hw::clock::raw().wrapping_sub(start) >= 8000 { break; }
        cortex_m::asm::nop();
    }
    let done = DONE.load(Ordering::Acquire);
    cortex_m::interrupt::free(|_| finish(if done == 1 { 1 } else { 2 }));
    let calls = CALLS.load(Ordering::Relaxed);
    let expired = EXPIRED.load(Ordering::Relaxed);
    let flags = FLAGS.load(Ordering::Relaxed);
    let phase = PHASE.load(Ordering::Relaxed);
    let armed = ARMED.load(Ordering::Relaxed);
    let counts = STATE.lock(|s| (s.requests, s.observations, s.retired)).unwrap_or((u32::MAX, 0, 0));
    let elapsed = TIMES[0].load(Ordering::Relaxed);
    let expected = match case {
        6 => calls == 5 && expired == 5 && phase == 0 && armed == 0 && counts.1 == 5 && counts.0 <= 5
            && slot_times_ok(),
        7 => calls == 1 && expired == 1 && flags & 7 == 7 && phase == 1 && armed == 0
            && (1000..=1100).contains(&elapsed),
        _ => calls == 1 && expired == 0 && flags == 13 && phase == 0 && armed == 0 && counts.0 == 0,
    };
    let ok = done == 1 && off() && expected;
    sink.say("RECHECKLIFE "); sink.kv("case", case); sink.kv("calls", calls);
    sink.kv("expired", expired); sink.kv("flags", flags); sink.kv("phase", phase);
    sink.kv("armed", armed); sink.kv("requests", counts.0); sink.kv("observations", counts.1);
    sink.kv("max_ticks125ns", MAX_TICKS.load(Ordering::Relaxed));
    sink.kv("off", u32::from(off())); sink.kv("pass", u32::from(ok));
    sink.say(" times_us=");
    for row in TIMES.iter().take(calls.min(5) as usize) { sink.say_u32(row.load(Ordering::Relaxed)); sink.say(","); }
    sink.say("\r\n");
    ok
}

fn slot_times_ok() -> bool {
    TIMES.iter().zip([501, 1501, 2001, 2501, 3001]).all(|(row, due)| {
        let at = row.load(Ordering::Relaxed);
        (due..=due + 100).contains(&at)
    })
}
