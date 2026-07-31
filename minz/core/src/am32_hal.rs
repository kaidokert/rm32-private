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
//! | trait             | firmware impl                         |
//! |-------------------|---------------------------------------|
//! | [`PwmOutput`]     | `minz::tim1_motor_pwm::Tim1Pwm`       |
//! | [`PhaseOutput`]   | `minz::tim1_motor_pwm::Tim1Pwm`       |
//! | [`Comparator`]    | `minz::comp2::Comp2`                  |
//! | [`CompExti`]      | `minz::comp2::Comp2`                  |
//! | [`IntervalTimer`] | `minz::am32_timers::Am32Timers`       |
//! | [`ComTimer`]      | `minz::am32_timers::Am32Timers`       |
//! | [`ComTimerExt`]   | `minz::am32_timers::Am32Timers`       |
//! | [`Recorder`]      | `minz::bb::Bb`                        |
//! | [`Cs`]            | `minz::bb::CortexCs`                  |
//! | [`InjAdc`]        | `minz::adc_sync::InjAdc1`             |
//! | [`LoopTimer`]     | `minz::tim6_loop::Tim6Loop`           |

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 2;
// do not modify without syncing rm32.
/// PWM output interface for 3-phase motor control.
/// Goes beyond embedded-hal's single-channel SetDutyCycle.
pub trait PwmOutput {
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

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 2;
// do not modify without syncing rm32.
/// Motor phase output control (6-step commutation)
pub trait PhaseOutput {
    fn com_step(&mut self, step: u8);
    fn all_off(&mut self);
    fn full_brake(&mut self);
    fn all_pwm(&mut self);
    fn proportional_brake(&mut self);
    /// Toggle pulse output on commutation step 1/4 (debug RPM measurement).
    /// Default no-op — override for boards with pulse output pin.
    fn pulse_toggle(&mut self, _step: u8) {}
}

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 3;
// do not modify without syncing rm32.
/// Comparator (BEMF sensing) interface
pub trait Comparator {
    fn output_level(&self) -> bool;
    /// Set the commutation step and rising/falling edge for BEMF sensing.
    fn set_step(&mut self, step: u8, rising: bool);
    fn change_input(&mut self);
    fn enable_interrupts(&mut self);
    fn mask_interrupts(&mut self);
}

/// minz extension over [`Comparator`] — EXTI pending-bit control for the
/// AM32-verbatim camp-at-gate (stm32l4xx_it.c:278-286: a closed-gate post-ZC
/// edge is LEFT PENDING to re-fire). rm32's wrappers clear-and-mask at ISR
/// entry instead; reconciling the two policies is a flagged unification
/// decision, not a mechanical move.
pub trait CompExti {
    fn exti_pending(&self) -> bool;
    fn clear_pending(&self);
}

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 1;
// do not modify without syncing rm32.
/// Interval timer (commutation timing measurement)
pub trait IntervalTimer {
    fn count(&self) -> u32;
    fn set_count(&mut self, val: u32);
}

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 1;
// do not modify without syncing rm32.
/// Commutation timer (one-shot for next commutation event)
pub trait ComTimer {
    fn set_and_enable(&mut self, timeout: u16);
    fn disable_interrupt(&mut self);
    fn enable_interrupt(&mut self);
}

/// minz extension over [`ComTimer`] — the AM32-verbatim polling path needs a
/// bare ARR write (main.c:1884) and the shared-vector UIF ack that rm32 does
/// in its ISR wrapper instead. Kept OUT of the copied rm32 traits.
pub trait ComTimerExt {
    /// `COM_TIMER->ARR = time` (zcfoundroutine main.c:1884).
    fn com_set_arr(&mut self, arr: u16);
    /// Ack COM_TIMER's UIF (`COM_TIMER->SR = 0`) — first line of the
    /// COM ISR (TIM1.UIE is off, so the shared vector is the COM tick).
    fn com_clear_flag(&mut self);
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

/// The ADC injected-group observer seam (bench telemetry + the two
/// bench-safety kills ONLY — never modulates the loop). Realized by
/// `minz::adc_sync::InjAdc1`.
pub trait InjAdc {
    /// Newest completed injected burst: `(phase_a, phase_b, current,
    /// vbat)` from JDR1-4, all mid-ON at the TRGO2 sample point.
    fn inj_read(&self) -> (u16, u16, u16, u16);
}

/// The 20 kHz loop timer's IRQ-ack seam (AM32's tenKhz TIM6).
/// Realized by `minz::tim6_loop::Tim6Loop`.
pub trait LoopTimer {
    /// Ack the update flag — first line of the `TIM6_DACUNDER` ISR.
    fn clear_flag(&self);
}

// Copied verbatim from rm32/src/hal.rs — rm32-shape convergence rung 4;
// do not modify without syncing rm32.
/// Bundle of ISR-level motor peripherals for static dispatch.
///
/// Reduces generic parameter count from 5 to 1 in ISR function signatures.
/// Implementors provide the concrete MCU-specific types.
pub trait MotorHal {
    type Pwm: PwmOutput;
    type Comp: Comparator;
    type Phase: PhaseOutput;
    type Interval: IntervalTimer;
    type Com: ComTimer;

    fn pwm(&mut self) -> &mut Self::Pwm;
    fn comp(&mut self) -> &mut Self::Comp;
    fn phase(&mut self) -> &mut Self::Phase;
    fn interval(&mut self) -> &mut Self::Interval;
    fn com_timer(&mut self) -> &mut Self::Com;
}

/// The concrete motor bundle implementing [`MotorHal`] — minz-OWNED
/// (the trait above is the rm32 copy; this implementor is ours, like
/// rm32's per-MCU bundle structs). The five rm32 `&mut self` seams are
/// held BY VALUE (zero-sized in firmware — free). `pwm`/`phase` are two
/// slots per the rm32 shape; firmware wires `Tim1Pwm` into both. Fns
/// needing the minz extension seams bound the associated types:
/// `M: MotorHal<Com: ComTimerExt>` / `M: MotorHal<Comp: CompExti>`.
pub struct Motor<P, C, Ph, I, CT> {
    pub pwm: P,
    pub comp: C,
    pub phase: Ph,
    pub interval: I,
    pub com: CT,
}

impl<P: PwmOutput, C: Comparator, Ph: PhaseOutput, I: IntervalTimer, CT: ComTimer> MotorHal
    for Motor<P, C, Ph, I, CT>
{
    type Pwm = P;
    type Comp = C;
    type Phase = Ph;
    type Interval = I;
    type Com = CT;

    #[inline(always)]
    fn pwm(&mut self) -> &mut P {
        &mut self.pwm
    }
    #[inline(always)]
    fn comp(&mut self) -> &mut C {
        &mut self.comp
    }
    #[inline(always)]
    fn phase(&mut self) -> &mut Ph {
        &mut self.phase
    }
    #[inline(always)]
    fn interval(&mut self) -> &mut I {
        &mut self.interval
    }
    #[inline(always)]
    fn com_timer(&mut self) -> &mut CT {
        &mut self.com
    }
}

/// The minz-owned observer bundle — the seams rm32's [`MotorHal`] knows
/// nothing about (black box, critical sections, injected ADC, loop-timer
/// ack). All `&self`-only seams → plain `&` refs. Threaded alongside the
/// motor bundle as a separate parameter: `(state clusters.., hal, obs)`.
pub struct Observer<'a, B, S, A, L> {
    pub bb: &'a B,
    pub cs: &'a S,
    pub adc: &'a A,
    pub lt: &'a L,
}

// ===============================================================
// Shared host-test mock — ONE MockHal (all seven seams on one struct,
// with an ordered call log — the rm32 HAL-call-counter idiom) + the
// owned-atomics cluster fixtures, used by BOTH the `am32_control` and
// `am32_isr` test mods.
// ===============================================================
#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use crate::am32::{ZCT_REC, ZctRing};
    use crate::am32_loop::{Bench, Drive, Duty, Sched};
    use crate::zct_trace::ZctTrace;
    use core::cell::{Cell, RefCell};
    use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

    #[derive(Default)]
    pub(crate) struct MockHal {
        pub(crate) calls: RefCell<Vec<&'static str>>,
        pub(crate) roles: RefCell<Vec<u8>>,
        /// `(step, rising)` pairs AS PASSED to `Comparator::set_step`
        /// (rung 3: the rm32 two-call shape — set_step stores, a
        /// following change_input applies).
        pub(crate) steps: RefCell<Vec<(u8, bool)>>,
        pub(crate) carrier_arrs: RefCell<Vec<u16>>,
        pub(crate) duties: RefCell<Vec<u16>>,
        pub(crate) com_arrs: RefCell<Vec<u16>>,
        pub(crate) events: RefCell<Vec<(u8, u8, u16)>>,
        pub(crate) frozen: Cell<bool>,
        pub(crate) comp_value: Cell<bool>,
        /// Scripted comparator reads: while non-empty, each `value()`
        /// consumes the front; falls back to `comp_value` after. Lets
        /// the persistence filter see a mid-run flip.
        pub(crate) comp_seq: RefCell<Vec<bool>>,
        /// EXTI PR22 pending-bit model (COMP2's line 22).
        pub(crate) pending: Cell<bool>,
        /// Injected-burst model: `(phase_a, phase_b, current, vbat)`.
        pub(crate) inj: Cell<(u16, u16, u16, u16)>,
        /// INTERVAL_TIMER CNT model; advances by `interval_step` per
        /// read so the spin-wait can be driven deterministically.
        pub(crate) interval: Cell<u32>,
        pub(crate) interval_step: Cell<u32>,
    }

    impl MockHal {
        pub(crate) fn new() -> Self {
            Self::default()
        }
        pub(crate) fn motor(&self) -> Motor<&MockHal, &MockHal, &MockHal, &MockHal, &MockHal> {
            Motor {
                pwm: self,
                comp: self,
                phase: self,
                interval: self,
                com: self,
            }
        }
        pub(crate) fn observer(&self) -> Observer<'_, MockHal, MockHal, MockHal, MockHal> {
            Observer {
                bb: self,
                cs: self,
                adc: self,
                lt: self,
            }
        }
        pub(crate) fn called(&self, name: &'static str) -> bool {
            self.calls.borrow().iter().any(|c| *c == name)
        }
        pub(crate) fn clear_calls(&self) {
            self.calls.borrow_mut().clear();
        }
    }

    // rm32-verbatim seams (`&mut self` receivers) on `&MockHal`, like
    // IntervalTimer/ComTimer below. Call-log strings follow the rung-2
    // trait method names; `roles` records the step AS PASSED (1..6 —
    // the -1 sector conversion lives inside the firmware impl, not in
    // core, so the mock must not convert either).
    impl PwmOutput for &MockHal {
        fn set_duty_all(&mut self, duty: u16) {
            self.calls.borrow_mut().push("set_duty_all");
            self.duties.borrow_mut().push(duty);
        }
        fn set_auto_reload(&mut self, arr: u16) {
            self.calls.borrow_mut().push("set_auto_reload");
            self.carrier_arrs.borrow_mut().push(arr);
        }
        fn set_prescaler(&mut self, _psc: u16) {
            self.calls.borrow_mut().push("set_prescaler");
        }
        fn set_compare1(&mut self, _val: u16) {
            self.calls.borrow_mut().push("set_compare1");
        }
        fn set_compare2(&mut self, _val: u16) {
            self.calls.borrow_mut().push("set_compare2");
        }
        fn set_compare3(&mut self, _val: u16) {
            self.calls.borrow_mut().push("set_compare3");
        }
        fn generate_update_event(&mut self) {
            self.calls.borrow_mut().push("generate_update_event");
        }
        fn set_dead_time_override(&mut self, _dtg: u16) {
            self.calls.borrow_mut().push("set_dead_time_override");
        }
    }

    impl PhaseOutput for &MockHal {
        fn com_step(&mut self, step: u8) {
            self.calls.borrow_mut().push("com_step");
            self.roles.borrow_mut().push(step);
        }
        fn all_off(&mut self) {
            self.calls.borrow_mut().push("all_off");
        }
        fn full_brake(&mut self) {
            self.calls.borrow_mut().push("full_brake");
        }
        fn all_pwm(&mut self) {
            self.calls.borrow_mut().push("all_pwm");
        }
        fn proportional_brake(&mut self) {
            self.calls.borrow_mut().push("proportional_brake");
        }
    }

    // Comparator on `&MockHal` like the other rm32-verbatim `&mut self`
    // seams. Call-log strings follow the rung-3 trait method names;
    // `steps` records the `(step, rising)` pair AS PASSED (1..6 — the
    // -1 sector conversion lives inside the firmware impl's
    // change_input, not in core, so the mock must not convert either).
    impl Comparator for &MockHal {
        fn output_level(&self) -> bool {
            let mut seq = self.comp_seq.borrow_mut();
            if seq.is_empty() { self.comp_value.get() } else { seq.remove(0) }
        }
        fn set_step(&mut self, step: u8, rising: bool) {
            self.calls.borrow_mut().push("set_step");
            self.steps.borrow_mut().push((step, rising));
        }
        fn change_input(&mut self) {
            self.calls.borrow_mut().push("change_input");
        }
        fn enable_interrupts(&mut self) {
            self.calls.borrow_mut().push("enable_interrupts");
        }
        fn mask_interrupts(&mut self) {
            self.calls.borrow_mut().push("mask_interrupts");
        }
    }

    // The minz CompExti extension (EXTI pending model) on the same
    // receiver, so one `&MockHal` fills the bundle's comp slot.
    impl CompExti for &MockHal {
        fn exti_pending(&self) -> bool {
            self.pending.get()
        }
        fn clear_pending(&self) {
            self.calls.borrow_mut().push("clear_pending");
            self.pending.set(false);
        }
    }

    // The by-value bundle seams are implemented on `&MockHal` (interior
    // mutability via the Cells makes the `&mut &MockHal` receivers work),
    // so tests build bundles with `interval: &mock, com: &mock`. Call-log
    // strings kept IDENTICAL to the pre-rung-1 `ComTimers` names — the
    // test assertions depend on them.
    impl IntervalTimer for &MockHal {
        fn count(&self) -> u32 {
            let v = self.interval.get();
            self.interval.set(v + self.interval_step.get());
            v
        }
        fn set_count(&mut self, val: u32) {
            self.calls.borrow_mut().push("set_interval_cnt");
            self.interval.set(val);
        }
    }

    impl ComTimer for &MockHal {
        fn set_and_enable(&mut self, timeout: u16) {
            self.calls.borrow_mut().push("set_and_enable_com_int");
            self.com_arrs.borrow_mut().push(timeout);
        }
        fn disable_interrupt(&mut self) {
            self.calls.borrow_mut().push("disable_com_timer_int");
        }
        fn enable_interrupt(&mut self) {
            self.calls.borrow_mut().push("enable_com_timer_int");
        }
    }

    impl ComTimerExt for &MockHal {
        fn com_set_arr(&mut self, arr: u16) {
            self.calls.borrow_mut().push("com_set_arr");
            self.com_arrs.borrow_mut().push(arr);
        }
        fn com_clear_flag(&mut self) {
            self.calls.borrow_mut().push("com_clear_flag");
        }
    }

    impl Recorder for MockHal {
        fn record(&self, ty: u8, sector: u8, data: u16) {
            self.events.borrow_mut().push((ty, sector, data));
        }
        fn freeze(&self) {
            self.frozen.set(true);
        }
    }

    impl Cs for MockHal {
        fn free<R>(&self, f: impl FnOnce() -> R) -> R {
            f() // NullCs: host tests are single-threaded
        }
    }

    impl InjAdc for MockHal {
        fn inj_read(&self) -> (u16, u16, u16, u16) {
            self.inj.get()
        }
    }

    impl LoopTimer for MockHal {
        fn clear_flag(&self) {
            self.calls.borrow_mut().push("tim6_clear_flag");
        }
    }

    // --- Owned-atomics fixtures (the am32_loop test idiom: each store
    // --- owns statics-shaped storage; a method borrows the cluster).
    #[derive(Default)]
    pub(crate) struct SchedStore {
        pub(crate) commutation_interval: AtomicU32,
        pub(crate) interval_hist: [AtomicU32; 6],
        pub(crate) average_interval: AtomicU32,
        pub(crate) last_average_interval: AtomicU32,
        pub(crate) last_zc: AtomicU16,
        pub(crate) this_zc: AtomicU16,
        pub(crate) wait_time: AtomicU16,
    }
    impl SchedStore {
        pub(crate) fn sched(&self) -> Sched<'_> {
            Sched {
                commutation_interval: &self.commutation_interval,
                interval_hist: &self.interval_hist,
                average_interval: &self.average_interval,
                last_average_interval: &self.last_average_interval,
                last_zc: &self.last_zc,
                this_zc: &self.this_zc,
                wait_time: &self.wait_time,
            }
        }
    }

    #[derive(Default)]
    pub(crate) struct DriveStore {
        pub(crate) current_step: AtomicU16,
        pub(crate) rising: AtomicBool,
        pub(crate) old_routine: AtomicBool,
        pub(crate) running: AtomicBool,
        pub(crate) zcfound: AtomicBool,
        pub(crate) bemf_counter: AtomicU16,
        pub(crate) min_bemf_up: AtomicU16,
        pub(crate) min_bemf_down: AtomicU16,
        pub(crate) zero_crosses: AtomicU32,
        pub(crate) filter_level: AtomicU16,
        pub(crate) bad_count: AtomicU16,
        pub(crate) desync_check: AtomicBool,
        pub(crate) desync_happened: AtomicU32,
        pub(crate) bemf_timeout_happened: AtomicU32,
        pub(crate) tenkhz_counter: AtomicU16,
        pub(crate) zcfr_guard_hits: AtomicU32,
    }
    impl DriveStore {
        pub(crate) fn drive(&self) -> Drive<'_> {
            Drive {
                current_step: &self.current_step,
                rising: &self.rising,
                old_routine: &self.old_routine,
                running: &self.running,
                zcfound: &self.zcfound,
                bemf_counter: &self.bemf_counter,
                min_bemf_up: &self.min_bemf_up,
                min_bemf_down: &self.min_bemf_down,
                zero_crosses: &self.zero_crosses,
                filter_level: &self.filter_level,
                bad_count: &self.bad_count,
                desync_check: &self.desync_check,
                desync_happened: &self.desync_happened,
                bemf_timeout_happened: &self.bemf_timeout_happened,
                tenkhz_counter: &self.tenkhz_counter,
                zcfr_guard_hits: &self.zcfr_guard_hits,
            }
        }
    }

    #[derive(Default)]
    pub(crate) struct DutyStore {
        pub(crate) input: AtomicU16,
        pub(crate) adjusted_input: AtomicU16,
        pub(crate) uart_duty_input: AtomicU16,
        pub(crate) duty_cycle_setpoint: AtomicU16,
        pub(crate) duty_cycle: AtomicU16,
        pub(crate) last_duty_cycle: AtomicU16,
        pub(crate) duty_cycle_maximum: AtomicU16,
        pub(crate) ramp_count: AtomicU16,
        pub(crate) killed: AtomicBool,
        pub(crate) kill_reason: AtomicU16,
        pub(crate) tim1_arr: AtomicU16,
    }
    impl DutyStore {
        pub(crate) fn duty(&self) -> Duty<'_> {
            Duty {
                input: &self.input,
                adjusted_input: &self.adjusted_input,
                uart_duty_input: &self.uart_duty_input,
                duty_cycle_setpoint: &self.duty_cycle_setpoint,
                duty_cycle: &self.duty_cycle,
                last_duty_cycle: &self.last_duty_cycle,
                duty_cycle_maximum: &self.duty_cycle_maximum,
                ramp_count: &self.ramp_count,
                killed: &self.killed,
                kill_reason: &self.kill_reason,
                tim1_arr: &self.tim1_arr,
            }
        }
    }

    #[derive(Default)]
    pub(crate) struct BenchStore {
        pub(crate) uart_deadman_ticks: AtomicU32,
        pub(crate) i_raw: AtomicU16,
        pub(crate) vbat_raw: AtomicU16,
        pub(crate) oc_acc: AtomicU32,
        pub(crate) oc_cnt: AtomicU32,
        pub(crate) vbat_low_ticks: AtomicU32,
        pub(crate) vbat_floor_raw: AtomicU16,
        pub(crate) stop_req: AtomicBool,
        pub(crate) dump_req: AtomicBool,
        pub(crate) info_req: AtomicBool,
        pub(crate) gecko_req: AtomicBool,
        pub(crate) wax_req: AtomicBool,
        pub(crate) freerun_req: AtomicBool,
        pub(crate) hist_req: AtomicBool,
        pub(crate) zct_stream_on: AtomicBool,
        pub(crate) delay_in_free: AtomicU32,
        pub(crate) delay_out_free: AtomicU32,
    }
    impl BenchStore {
        pub(crate) fn bench(&self) -> Bench<'_> {
            Bench {
                uart_deadman_ticks: &self.uart_deadman_ticks,
                i_raw: &self.i_raw,
                vbat_raw: &self.vbat_raw,
                oc_acc: &self.oc_acc,
                oc_cnt: &self.oc_cnt,
                vbat_low_ticks: &self.vbat_low_ticks,
                vbat_floor_raw: &self.vbat_floor_raw,
                stop_req: &self.stop_req,
                dump_req: &self.dump_req,
                info_req: &self.info_req,
                gecko_req: &self.gecko_req,
                wax_req: &self.wax_req,
                freerun_req: &self.freerun_req,
                hist_req: &self.hist_req,
                zct_stream_on: &self.zct_stream_on,
                delay_in_free: &self.delay_in_free,
                delay_out_free: &self.delay_out_free,
            }
        }
    }

    pub(crate) struct ZctStore {
        pub(crate) ring: [[AtomicU16; ZCT_REC]; 8],
        pub(crate) head: AtomicUsize,
        pub(crate) tail: AtomicUsize,
        pub(crate) drop: AtomicU32,
        pub(crate) comm_n: AtomicU32,
        pub(crate) batching: AtomicBool,
    }
    impl ZctStore {
        pub(crate) fn new() -> Self {
            Self {
                ring: [const { [const { AtomicU16::new(0) }; ZCT_REC] }; 8],
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                drop: AtomicU32::new(0),
                comm_n: AtomicU32::new(0),
                batching: AtomicBool::new(false),
            }
        }
        pub(crate) fn zct(&self) -> ZctTrace<'_, 8> {
            ZctTrace {
                ring: ZctRing {
                    ring: &self.ring,
                    head: &self.head,
                    tail: &self.tail,
                    drop: &self.drop,
                },
                comm_n: &self.comm_n,
                batching: &self.batching,
            }
        }
        pub(crate) fn records(&self) -> usize {
            (self.head.load(Ordering::Relaxed) + 8 - self.tail.load(Ordering::Relaxed)) % 8
        }
    }
}
