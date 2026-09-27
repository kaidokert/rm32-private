//! COM-owned bounded level observations. No gates or synthetic commutations.
use super::*;
use crate::revisit_schedule::{Authority, Schedule};
use crate::shared::Seam;

pub mod check;

pub trait Policy { const ON: bool; }
pub struct Foreground;
impl Policy for Foreground { const ON: bool = false; }
pub struct Timed;
impl Policy for Timed { const ON: bool = true; }

pub trait Observer {
    fn inputs(now: u32, input: crate::revisit::Inputs, admitted: bool);
}
pub struct Quiet;
impl Observer for Quiet {
    #[inline(always)]
    fn inputs(_: u32, _: crate::revisit::Inputs, _: bool) {}
}

struct State {
    schedule: Option<Schedule>,
    requests: u32,
    observations: u32,
    retired: u32,
}
static STATE: Seam<State, Motor> = Seam::new(State {
    schedule: None, requests: 0, observations: 0, retired: 0,
});

/// Only an expired optional observation may park across masked COMP work.
/// Called before COM accounting: keep optional work outside decision-to-arm.
/// Refusal wakes the parked timer; acceptance replaces it.
#[inline(always)]
pub fn defer_for_comp() -> bool {
    crate::revisit_delivery::defer(S.com().phase.load(Ordering::Relaxed), hw::comp::line_live())
}

/// No schedule borrow here: COMP only wakes its higher-priority owner.
pub fn resume_after_refusal(comp_entry_raw: u16) {
    cortex_m::interrupt::free(|_| {
        let a = authority();
        if crate::revisit_delivery::resume(a.powered, a.phase,
            hw::comp::line_live(), hw::com_timer::is_running()) {
            hw::nvic::pend(stm32::Interrupt::TIM16);
        }
    });
    // Include the new refusal-tail work and any wake preemption in the existing
    // handler budget. The earlier check remains; this can only stop sooner.
    let elapsed = u32::from(hw::clock::raw().wrapping_sub(comp_entry_raw))
        .wrapping_add(S.comp().overrun_inject_us.load(Ordering::Relaxed));
    if elapsed > S.comp().call_max_us.load(Ordering::Relaxed) {
        S.comp().call_max_us.store(elapsed, Ordering::Relaxed);
    }
    if crate::rate::handler_overrun(elapsed) {
        S.comp().overrun.store(true, Ordering::Relaxed);
        guard_trip(Reason::HandlerOverrun);
    }
}

fn authority() -> Authority {
    Authority {
        powered: S.guard().reason.load(Ordering::Relaxed) == 0
            && S.det().active.load(Ordering::Relaxed)
            && crate::oneshot::arm_allowed(
                S.com().stopped.load(Ordering::Relaxed), S.com().active.load(Ordering::Relaxed)),
        phase: S.com().phase.load(Ordering::Relaxed),
        generation: S.det().accept_seq.load(Ordering::Relaxed),
        step: S.det().step.load(Ordering::Relaxed),
    }
}

/// Called under exclusion: one raw read maps both now and origin consistently.
fn clock_and_origin() -> (u32, u32) {
    let raw = hw::clock::raw();
    let base = S.guard().raw.load(Ordering::Relaxed) as u16;
    let now = S.guard().ext.load(Ordering::Relaxed).wrapping_add(u32::from(raw.wrapping_sub(base)));
    let accepted = S.det().accept_raw.load(Ordering::Relaxed) as u16;
    (now, now.wrapping_sub(u32::from(raw.wrapping_sub(accepted))))
}

/// Only COM touches the model. Guard never borrows it: the stop latch removes
/// authority. COMP publication invalidates its generation before atomic arm.
pub fn after_phase(at: &mut Root<Motor>, serviced: u32) {
    after_phase_observed::<Quiet>(at, serviced);
}

/// Typed diagnostic hook: the motor path instantiates the empty observer.
pub fn after_phase_observed<O: Observer>(at: &mut Root<Motor>, serviced: u32) {
    STATE.root(at, |state| cortex_m::interrupt::free(|_| {
        let a = authority();
        let (now, origin) = clock_and_origin();
        if !a.powered { state.schedule = None; return; }
        if serviced == 1 {
            state.schedule = Some(Schedule::new(a.generation, a.step, origin,
                S.det().accept_avg.load(Ordering::Relaxed)));
        }
        // A phase1 with a pending blank must not replace that timer.
        if a.phase != 0 && a.phase != 4 { return; }
        let Some(schedule) = state.schedule.as_mut() else { return; };
        let owns = schedule.owns(&a);
        let live = owns && {
            let input = crate::revisit::Inputs {
            closed_loop: true, line_live: hw::comp::line_live(), pending: hw::comp::pending(),
            average_us: schedule.average(), elapsed_us: schedule.elapsed(now),
            post_level: hw::comp::level() == edge_is_rising(Step::new_clamped(a.step as u8)),
            already_retried: false,
            };
            let admitted = crate::revisit::admit(input);
            O::inputs(now, input, admitted);
            admitted
        };
        let decision = schedule.observe(now, &a, live);
        state.observations = state.observations.wrapping_add(1);
        if !owns {
            state.schedule = None;
            state.retired = state.retired.wrapping_add(1);
            return; // May have interrupted COMP publication: never overwrite it.
        }
        // One bounded transaction: live admission, quota reservation and pend.
        // Comparator hardware can still change; COMP redoes live persistence.
        if decision.pend { hw::comp::pend(); state.requests = state.requests.wrapping_add(1); }
        S.com().phase.store(0, Ordering::Relaxed);
        if let Some(delay) = decision.delay_us { com_arm(delay, 4); }
        if decision.delay_us.is_none() {
            state.schedule = None;
        }
    }));
}

/// Boot-cumulative counters, explicit offline request only. Never live TX.
pub fn report(sink: &mut impl crate::report::Sink) {
    if S.guard().active.load(Ordering::Relaxed) || S.com().active.load(Ordering::Relaxed)
        || S.det().active.load(Ordering::Relaxed) || S.drv().active.load(Ordering::Relaxed)
        || hw::gpio::enable_is_high() || hw::pwm::moe_is_set() { return; }
    let row = STATE.lock(|s| (s.requests, s.observations, s.retired));
    if let Some((requests, observations, retired)) = row {
        sink.say("TIMEDRECHECK boot_cumulative=1 ");
        sink.kv("requests", requests); sink.kv("observations", observations); sink.kv("retired", retired);
        sink.say("\r\n");
    }
}
