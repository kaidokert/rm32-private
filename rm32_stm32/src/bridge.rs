//! The port's `rm32::bridge::Bridge`: TIM1 (period, compares, dead time,
//! tone prescaler) plus the gate-pin driver, composed. The two halves stay
//! separate port types (each MCU has its own TIM1 wrapper; every MCU shares
//! the GPIO gate driver), described by the port-internal traits below.

/// TIM1 as the bridge's PWM timer (one implementation per MCU).
pub trait TimerPwm {
    fn set_duty_all(&mut self, duty: u16);
    fn set_auto_reload(&mut self, arr: u16);
    fn set_prescaler(&mut self, psc: u16);
    fn set_compare1(&mut self, val: u16);
    fn set_compare2(&mut self, val: u16);
    fn set_compare3(&mut self, val: u16);
    fn generate_update_event(&mut self);
    /// Override dead-time in TIM1 BDTR register (OR'd with existing DTG value).
    fn set_dead_time_override(&mut self, dtg: u16);
}

/// The six gate pins (`crate::phase::PhaseDriver`).
pub trait PhaseGates {
    fn com_step(&mut self, step: u8);
    fn all_off(&mut self);
    fn full_brake(&mut self);
    fn all_pwm(&mut self);
    fn proportional_brake(&mut self);
    /// Toggle pulse output on commutation step 1/4 (debug RPM measurement).
    /// Default no-op — override for boards with pulse output pin.
    fn pulse_toggle(&mut self, _step: u8) {}
}

/// TIM1 + gate pins as one `rm32::bridge::Bridge`.
pub struct StmBridge<P, PH> {
    pub pwm: P,
    pub phase: PH,
}

impl<P: TimerPwm, PH: PhaseGates> rm32::bridge::Bridge for StmBridge<P, PH> {
    #[inline(always)]
    fn com_step(&mut self, step: u8) {
        self.phase.com_step(step);
    }
    #[inline(always)]
    fn all_off(&mut self) {
        self.phase.all_off();
    }
    #[inline(always)]
    fn full_brake(&mut self) {
        self.phase.full_brake();
    }
    #[inline(always)]
    fn all_pwm(&mut self) {
        self.phase.all_pwm();
    }
    #[inline(always)]
    fn proportional_brake(&mut self) {
        self.phase.proportional_brake();
    }
    #[inline(always)]
    fn set_duty_all(&mut self, compare: u16) {
        self.pwm.set_duty_all(compare);
    }
    #[inline(always)]
    fn set_phase_duties(&mut self, a: u16, b: u16, c: u16) {
        self.pwm.set_compare1(a);
        self.pwm.set_compare2(b);
        self.pwm.set_compare3(c);
    }
    #[inline(always)]
    fn set_period(&mut self, period: u16) {
        self.pwm.set_auto_reload(period);
    }
    #[inline(always)]
    fn latch(&mut self) {
        self.pwm.generate_update_event();
    }
    #[inline(always)]
    fn set_dead_time_override(&mut self, dto: u16) {
        self.pwm.set_dead_time_override(dto);
    }
    #[inline(always)]
    fn set_tone_divider(&mut self, div: u16) {
        self.pwm.set_prescaler(div);
    }
    #[inline(always)]
    fn pulse_toggle(&mut self, step: u8) {
        self.phase.pulse_toggle(step);
    }
}
