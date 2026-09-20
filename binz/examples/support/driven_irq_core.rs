//! Record-only adapter for the REAL minz COMP ISR, not a sampled imitation.
//! The caller owns command epochs, COMP mux/EXTI setup, interval clock, and
//! serialization. Supply established HAL polarity, NOT raw COMP polarity.
//! No physical PWM/phase/COM-timer implementation can enter this bundle.
//! A returned acceptance is diagnostic evidence, never a Seed or gate permit.
use core::cell::Cell;
use minz_core::am32_hal::*;
use minz_core::am32_loop::{Drive, Sched};
use portable_atomic::Ordering::Relaxed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acceptance {
    pub sector: u8,
    /// Actual interval timer sample in the caller's timer units.
    pub interval: u16,
    /// Requested ARR only. NO physical timer is armed by this adapter.
    pub requested_arr: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Result {
    InvalidState,
    NoAcceptance,
    Accepted(Acceptance),
    /// Upstream code reached a seam outside the COMP-only contract.
    UnexpectedCall,
}

/// Execute precisely one production COMP ISR visit. Return the supplied
/// comparator/timer so ownership need not be duplicated with unsafe aliases.
/// A gate-closed pending edge remains pending exactly as in minz: callers
/// MUST retain an IRQ-rate guard. This function is not a storm mitigation.
pub fn visit<C: Comparator + CompExti, I: IntervalTimer, S: Cs>(
    sched: &Sched,
    drive: &Drive,
    comp: C,
    interval: I,
    cs: &S,
) -> (C, I, Result) {
    let sink = Sink::default();
    let mut hal = Motor {
        pwm: &sink,
        phase: &sink,
        comp,
        interval,
        com: &sink,
    };
    let step = drive.current_step.load(Relaxed);
    let filter = drive.filter_level.load(Relaxed);
    // Zero filter would accept without reading COMP; never allow it here.
    // 12 is core_bench's configured filter, not a new production tuning.
    if !(1..=6).contains(&step)
        || !(1..=12).contains(&filter)
        || sched.average_interval.load(Relaxed) == 0
    {
        return (hal.comp, hal.interval, Result::InvalidState);
    }
    let obs = Observer {
        bb: &sink,
        cs,
        adc: &sink,
        lt: &sink,
    };
    minz_core::am32_isr::comp_isr(sched, drive, &mut hal, &obs);
    let result = match (sink.event.get(), sink.arr.get(), sink.unexpected.get()) {
        (_, _, true) => Result::UnexpectedCall,
        (None, None, false) => Result::NoAcceptance,
        (Some((sector, interval)), Some(requested_arr), false) => Result::Accepted(Acceptance {
            sector,
            interval,
            requested_arr,
        }),
        _ => Result::UnexpectedCall,
    };
    (hal.comp, hal.interval, result)
}

#[derive(Default)]
struct Sink {
    event: Cell<Option<(u8, u16)>>,
    arr: Cell<Option<u16>>,
    unexpected: Cell<bool>,
}
impl Sink {
    fn unexpected(&self) {
        self.unexpected.set(true);
    }
}
impl Recorder for Sink {
    fn record(&self, ty: u8, sector: u8, data: u16) {
        if ty != minz_core::blackbox::EV_ACC
            || sector >= 6
            || self.event.replace(Some((sector, data))).is_some()
        {
            self.unexpected();
        }
    }
    fn freeze(&self) {
        self.unexpected();
    }
}
impl ComTimer for &Sink {
    fn set_and_enable(&mut self, arr: u16) {
        if self.arr.replace(Some(arr)).is_some() {
            self.unexpected();
        }
    }
    fn disable_interrupt(&mut self) {
        self.unexpected();
    }
    fn enable_interrupt(&mut self) {
        self.unexpected();
    }
}
impl PwmOutput for &Sink {
    fn set_duty_all(&mut self, _: u16) {
        self.unexpected();
    }
    fn set_auto_reload(&mut self, _: u16) {
        self.unexpected();
    }
    fn set_prescaler(&mut self, _: u16) {
        self.unexpected();
    }
    fn set_compare1(&mut self, _: u16) {
        self.unexpected();
    }
    fn set_compare2(&mut self, _: u16) {
        self.unexpected();
    }
    fn set_compare3(&mut self, _: u16) {
        self.unexpected();
    }
    fn generate_update_event(&mut self) {
        self.unexpected();
    }
    fn set_dead_time_override(&mut self, _: u16) {
        self.unexpected();
    }
}
impl PhaseOutput for &Sink {
    fn com_step(&mut self, _: u8) {
        self.unexpected();
    }
    fn all_off(&mut self) {
        self.unexpected();
    }
    fn full_brake(&mut self) {
        self.unexpected();
    }
    fn all_pwm(&mut self) {
        self.unexpected();
    }
    fn proportional_brake(&mut self) {
        self.unexpected();
    }
    fn pulse_toggle(&mut self, _: u8) {
        self.unexpected();
    }
}
impl InjAdc for Sink {
    fn inj_read(&self) -> (u16, u16, u16, u16) {
        self.unexpected();
        (0, 0, 0, 0)
    }
}
impl LoopTimer for Sink {
    fn clear_flag(&self) {
        self.unexpected();
    }
}
