//! Hardware seams for the AM32-clone control layer — the trait set
//! that `crate::am32_control` (and `crate::zct_trace`) reach hardware
//! through. **Static dispatch only** (no `dyn` anywhere): firmware
//! provides zero-sized register impls, host tests provide mocks, and
//! every call monomorphizes to the same direct register access the
//! pre-trait code had (zero runtime cost).
//!
//! The trait inventory is exactly what the control layer touches —
//! nothing speculative. Realizing impls live in the `minz` firmware
//! crate next to the registers they drive:
//!
//! | trait        | firmware impl                         |
//! |--------------|---------------------------------------|
//! | [`MotorPwm`] | `minz::tim1_motor_pwm::Tim1Pwm`       |
//! | [`CompCtl`]  | `minz::comp2::Comp2`                  |
//! | [`ComTimers`]| `minz::am32_timers::Am32Timers`       |
//! | [`Recorder`] | `minz::bb::Bb`                        |
//! | [`Cs`]       | `minz::bb::CortexCs`                  |

/// TIM1 motor PWM output stage. Realized by
/// `minz::tim1_motor_pwm::Tim1Pwm` (zero-sized, delegates to the
/// module's free fns); mirrors AM32 `phaseouts.c` + the
/// `SET_DUTY_CYCLE_ALL` / `SET_AUTO_RELOAD_PWM` macros.
pub trait MotorPwm {
    /// `SET_DUTY_CYCLE_ALL` (peripherals.h:25): same duty in all
    /// three CCRs, always.
    fn set_duty(&self, duty: u16);
    /// comStep role-only flip (`phaseouts.c` HIGH/LOW/FLOAT rotation),
    /// `step ∈ 0..5` (minz sector frame).
    fn set_roles_for_step(&self, step: u8);
    /// allOff (phaseouts.c): float all legs (gate-driver inputs
    /// actively driven low).
    fn all_off(&self);
    /// `SET_AUTO_RELOAD_PWM` — write the live carrier ARR (preloaded).
    fn set_carrier_arr(&self, arr: u16);
    /// The LIVE carrier ARR (`TIMER1_MAX_ARR` as currently ridden by
    /// variable_pwm) — the duty rescale denominator (main.c:1790).
    fn max_duty(&self) -> u16;
    /// The base carrier ARR — AM32 targets.h:5335 `TIM1_AUTORELOAD =
    /// CPU_FREQUENCY_MHZ*1e6/NOMINAL_PWM - 1` (3332 on L431 @ 24 kHz;
    /// the firmware value is carrier-feature-dependent, hence a
    /// method, not a core const).
    fn base_arr(&self) -> u16;
}

/// COMP2 BEMF comparator + its EXTI line. Realized by
/// `minz::comp2::Comp2`; mirrors AM32 `comparator.c`.
pub trait CompCtl {
    /// `comp2::value()` — the comparator output bit. NOTE: the minz
    /// polarity is the INVERSE of AM32's `getCompOutputLevel()` (see
    /// the am32_clone header comment).
    fn value(&self) -> bool;
    /// changeCompInput (comparator.c:18-35): mux the floating phase +
    /// select the sector's expected EXTI edge.
    fn change_comp_input(&self, sector: usize);
    /// enableCompInterrupts (comparator.c:14-16): unmask, KEEP pending.
    fn enable_comp_interrupts(&self);
    /// maskPhaseInterrupts (comparator.c:9-12): mask + clear pending.
    fn mask_phase_interrupts(&self);
}

/// INTERVAL_TIMER (TIM2) + COM_TIMER (TIM16) register macros
/// (peripherals.h:15-26, both 0.5 µs ticks). Realized by
/// `minz::am32_timers::Am32Timers`.
pub trait ComTimers {
    /// `INTERVAL_TIMER_COUNT` (peripherals.h:15) — 16-bit CNT as u32.
    fn interval_cnt(&self) -> u32;
    /// `SET_INTERVAL_TIMER_COUNT` (peripherals.h:22).
    fn set_interval_cnt(&self, v: u16);
    /// `COM_TIMER->ARR = time` (zcfoundroutine main.c:1884).
    fn com_set_arr(&self, arr: u16);
    /// `SET_AND_ENABLE_COM_INT(time)` (peripherals.h:19-21).
    fn set_and_enable_com_int(&self, arr: u16);
    /// `DISABLE_COM_TIMER_INT()` (peripherals.h:17).
    fn disable_com_timer_int(&self);
}

/// The black-box seam (observer-only). Realized by `minz::bb::Bb`
/// (critical-section cell around `crate::blackbox::BlackBox`, with
/// the hardware timestamp added platform-side).
pub trait Recorder {
    /// bb_record: one event, timestamped by the impl.
    fn record(&self, ty: u8, sector: u8, data: u16);
    /// Freeze the ring (a fault freezes so the dump shows the events
    /// LEADING TO the kill).
    fn freeze(&self);
}

/// Critical-section provider (the `cortex_m::interrupt::free`
/// envelope). Realized by `minz::bb::CortexCs`; host tests use a
/// pass-through. Generic method → not object-safe — irrelevant,
/// `dyn` is banned here anyway.
pub trait Cs {
    fn free<R>(&self, f: impl FnOnce() -> R) -> R;
}

/// The bundled HAL — one parameter threads all five seams through
/// `crate::am32_control` (call sites stay short; static dispatch —
/// the fields are plain `&` to zero-sized impls in firmware).
pub struct Hal<'a, P: MotorPwm, C: CompCtl, T: ComTimers, B: Recorder, S: Cs> {
    pub pwm: &'a P,
    pub comp: &'a C,
    pub tim: &'a T,
    pub bb: &'a B,
    pub cs: &'a S,
}
