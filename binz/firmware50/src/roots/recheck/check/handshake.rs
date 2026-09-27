//! Real IRQ handshake with synthetic COMP outcomes and bridge disabled.
use super::*;

static ENTERED: AtomicU32 = AtomicU32::new(0);
static PARKED: AtomicU32 = AtomicU32::new(0);
static CALLS: AtomicU32 = AtomicU32::new(0);
static REASON: AtomicU32 = AtomicU32::new(0);
static CROSS: AtomicU32 = AtomicU32::new(0);
static ELAPSED: AtomicU32 = AtomicU32::new(0);
static BEFORE: AtomicU32 = AtomicU32::new(0);
static CANCELLED: AtomicU32 = AtomicU32::new(0);

pub(super) fn start() {
    for flag in [&ENTERED,&PARKED,&CALLS,&REASON,&ELAPSED,&BEFORE,&CANCELLED] { flag.store(0,Ordering::Relaxed); }
    S.comp().overrun_inject_us.store(0,Ordering::Relaxed);
    hw::nvic::unmask(stm32::Interrupt::ADC_COMP);
    hw::comp::pend();
}

/// Used only by the no-drive binary's actual ADC_COMP vector.
pub fn comp_interrupt() {
    hw::comp::line_disable(); hw::comp::clear_pending();
    let case=CASE.load(Ordering::Relaxed);
    if !(9..=12).contains(&case) || ENTERED.load(Ordering::Relaxed)!=0 { return; }
    ENTERED.store(1,Ordering::Relaxed);
    if !off() { finish(2); return; }
    let raw=hw::clock::raw(); CROSS.store(u32::from(raw),Ordering::Relaxed);
    com_arm(10,4);
    for _ in 0..10_000 {
        if PARKED.load(Ordering::Acquire)!=0 || hw::clock::raw().wrapping_sub(raw)>200 { break; }
        cortex_m::asm::nop();
    }
    if PARKED.load(Ordering::Acquire)!=1 { finish(2); return; }
    match case {
        10 | 11 => cortex_m::interrupt::free(|_| {
            hw::nvic::pend(stm32::Interrupt::TIM16);
            let pending=hw::nvic::is_pending(stm32::Interrupt::TIM16);
            if case==10 { publish_accept_sequence(); let _=com_arm_crossing(raw,1000); }
            else { guard_trip(Reason::HostAbort); }
            CANCELLED.store(u32::from(pending && !hw::nvic::is_pending(stm32::Interrupt::TIM16)),Ordering::Relaxed);
            BEFORE.store(snapshot(),Ordering::Relaxed);
            if case==11 { hw::nvic::pend(stm32::Interrupt::TIM16); }
        }),
        _ => {
            if case==12 { S.comp().overrun_inject_us.store(100,Ordering::Relaxed); }
            if comp_resume_powered() { resume_after_refusal(raw); }
            REASON.store(S.guard().reason.load(Ordering::Relaxed),Ordering::Relaxed);
            BEFORE.store(snapshot(),Ordering::Relaxed);
            finish(1);
        }
    }
}

fn snapshot()->u32 {
    u32::from(S.com().stopped.load(Ordering::Relaxed))
        | (u32::from(S.com().active.load(Ordering::Relaxed))<<1)
        | (u32::from(hw::com_timer::is_running())<<2)
        | (u32::from(hw::nvic::is_pending(stm32::Interrupt::TIM16))<<3)
        | (u32::from(hw::com_timer::update_pending())<<4)
        | (u32::from(off())<<5)
}

/// # Safety
/// Actual TIM16 vector at Motor priority, higher than this probe's COMP.
pub(super) unsafe fn interrupt() {
    let expired=hw::com_timer::update_pending(); hw::com_timer::ack();
    let n=CALLS.load(Ordering::Relaxed); CALLS.store(n+1,Ordering::Relaxed);
    if !off() || n>=2 { finish(2); return; }
    if defer_for_comp() {
        let valid=expired && !hw::com_timer::is_running() && n==0;
        PARKED.store(if valid {1} else {2},Ordering::Release);
        return;
    }
    let case=CASE.load(Ordering::Relaxed);
    // Refusal must wake by software while the parked one-shot is stopped.
    if (case==9 || case==12) && (expired || hw::com_timer::is_running()) {
        finish(2); return;
    }
    if S.com().active.load(Ordering::Relaxed) && case!=10 {
        // SAFETY: actual TIM16 owns the scheduler at its resource ceiling.
        let mut at=unsafe { Root::<Motor>::enter() };
        after_phase(&mut at,4);
    }
    PHASE.store(S.com().phase.load(Ordering::Relaxed),Ordering::Relaxed);
    ARMED.store(u32::from(hw::com_timer::is_running()),Ordering::Relaxed);
    ELAPSED.store(u32::from(hw::clock::raw().wrapping_sub(CROSS.load(Ordering::Relaxed) as u16)),Ordering::Relaxed);
    hw::nvic::mask(stm32::Interrupt::TIM16);
    if case==10 || case==11 {
        REASON.store(S.guard().reason.load(Ordering::Relaxed),Ordering::Relaxed);
        finish(if (case==10 && expired) || (case==11 && !expired) {1} else {2});
    }
}

fn finish(done:u32) {
    guard_trip(Reason::HostAbort);
    hw::nvic::mask(stm32::Interrupt::TIM16);
    hw::nvic::unpend(stm32::Interrupt::TIM16);
    DONE.store(if off() {done} else {2},Ordering::Release);
}

pub fn run(case:u32,sink:&mut impl crate::report::Sink)->bool {
    if !(9..=12).contains(&case) || !prepare(case) { return false; }
    let raw=hw::clock::raw();
    for _ in 0..100_000 {
        if DONE.load(Ordering::Acquire)!=0 || hw::clock::raw().wrapping_sub(raw)>3000 { break; }
        cortex_m::asm::nop();
    }
    let done=DONE.load(Ordering::Acquire);
    cortex_m::interrupt::free(|_| finish(if done==1 {1} else {2}));
    let observations=STATE.lock(|s|s.observations).unwrap_or(u32::MAX);
    let phase=PHASE.load(Ordering::Relaxed); let armed=ARMED.load(Ordering::Relaxed);
    let reason=REASON.load(Ordering::Relaxed); let elapsed=ELAPSED.load(Ordering::Relaxed);
    let expected=match case {
        9 => observations==1 && phase==4 && armed==1 && reason==0,
        10 => observations==0 && phase==1 && armed==0 && reason==0 && (1000..=1100).contains(&elapsed)
            && CANCELLED.load(Ordering::Relaxed)==1 && BEFORE.load(Ordering::Relaxed)==38,
        11 => observations==0 && phase==0 && armed==0 && reason==9
            && CANCELLED.load(Ordering::Relaxed)==1 && BEFORE.load(Ordering::Relaxed)==33,
        _ => observations==1 && reason==14 && BEFORE.load(Ordering::Relaxed)==33,
    };
    let ok=done==1 && PARKED.load(Ordering::Relaxed)==1 && CALLS.load(Ordering::Relaxed)==2 && off() && expected;
    sink.say("RECHECKHAND "); sink.kv("case",case); sink.kv("calls",CALLS.load(Ordering::Relaxed));
    sink.kv("parked",PARKED.load(Ordering::Relaxed)); sink.kv("observations",observations);
    sink.kv("phase",phase); sink.kv("armed",armed); sink.kv("reason",reason); sink.kv("elapsed_us",elapsed);
    sink.kv("before",BEFORE.load(Ordering::Relaxed)); sink.kv("cancelled",CANCELLED.load(Ordering::Relaxed));
    sink.kv("off",u32::from(off())); sink.kv("pass",u32::from(ok)); sink.say("\r\n");
    S.comp().overrun_inject_us.store(0,Ordering::Relaxed);
    ok
}
