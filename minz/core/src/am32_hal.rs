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
//! | [`InjAdc`]   | `minz::adc_sync::InjAdc1`             |
//! | [`LoopTimer`]| `minz::tim6_loop::Tim6Loop`           |

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
    /// COMP2's EXTI line pending read (`EXTI->PR1 & LINE`,
    /// stm32l4xx_it.c:278 `EXTI_GetITStatus`) — the COMP ISR entry gate.
    fn exti_pending(&self) -> bool;
    /// Ack the COMP2 EXTI pending bit (write-1-to-clear,
    /// stm32l4xx_it.c:281/285 `EXTI_ClearITPendingBit`).
    fn clear_pending(&self);
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
    /// Ack COM_TIMER's UIF (`COM_TIMER->SR = 0`) — first line of the
    /// COM ISR (TIM1.UIE is off, so the shared vector is the COM tick).
    fn com_clear_flag(&self);
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

/// The bundled HAL — one parameter threads all seven seams through
/// `crate::am32_control` / `crate::am32_isr` (call sites stay short;
/// static dispatch — the fields are plain `&` to zero-sized impls in
/// firmware).
pub struct Hal<'a, P: MotorPwm, C: CompCtl, T: ComTimers, B: Recorder, S: Cs, A: InjAdc, L: LoopTimer>
{
    pub pwm: &'a P,
    pub comp: &'a C,
    pub tim: &'a T,
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
        pub(crate) comp_inputs: RefCell<Vec<usize>>,
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
        pub(crate) max_duty: Cell<u16>,
        pub(crate) base_arr: Cell<u16>,
    }

    impl MockHal {
        pub(crate) fn new() -> Self {
            let m = Self::default();
            m.max_duty.set(3332);
            m.base_arr.set(3332);
            m
        }
        pub(crate) fn hal(
            &self,
        ) -> Hal<'_, MockHal, MockHal, MockHal, MockHal, MockHal, MockHal, MockHal> {
            Hal { pwm: self, comp: self, tim: self, bb: self, cs: self, adc: self, lt: self }
        }
        pub(crate) fn called(&self, name: &'static str) -> bool {
            self.calls.borrow().iter().any(|c| *c == name)
        }
        pub(crate) fn clear_calls(&self) {
            self.calls.borrow_mut().clear();
        }
    }

    impl MotorPwm for MockHal {
        fn set_duty(&self, duty: u16) {
            self.calls.borrow_mut().push("set_duty");
            self.duties.borrow_mut().push(duty);
        }
        fn set_roles_for_step(&self, step: u8) {
            self.calls.borrow_mut().push("set_roles_for_step");
            self.roles.borrow_mut().push(step);
        }
        fn all_off(&self) {
            self.calls.borrow_mut().push("all_off");
        }
        fn set_carrier_arr(&self, arr: u16) {
            self.calls.borrow_mut().push("set_carrier_arr");
            self.carrier_arrs.borrow_mut().push(arr);
        }
        fn max_duty(&self) -> u16 {
            self.max_duty.get()
        }
        fn base_arr(&self) -> u16 {
            self.base_arr.get()
        }
    }

    impl CompCtl for MockHal {
        fn value(&self) -> bool {
            let mut seq = self.comp_seq.borrow_mut();
            if seq.is_empty() { self.comp_value.get() } else { seq.remove(0) }
        }
        fn change_comp_input(&self, sector: usize) {
            self.calls.borrow_mut().push("change_comp_input");
            self.comp_inputs.borrow_mut().push(sector);
        }
        fn enable_comp_interrupts(&self) {
            self.calls.borrow_mut().push("enable_comp_interrupts");
        }
        fn mask_phase_interrupts(&self) {
            self.calls.borrow_mut().push("mask_phase_interrupts");
        }
        fn exti_pending(&self) -> bool {
            self.pending.get()
        }
        fn clear_pending(&self) {
            self.calls.borrow_mut().push("clear_pending");
            self.pending.set(false);
        }
    }

    impl ComTimers for MockHal {
        fn interval_cnt(&self) -> u32 {
            let v = self.interval.get();
            self.interval.set(v + self.interval_step.get());
            v
        }
        fn set_interval_cnt(&self, v: u16) {
            self.calls.borrow_mut().push("set_interval_cnt");
            self.interval.set(v as u32);
        }
        fn com_set_arr(&self, arr: u16) {
            self.calls.borrow_mut().push("com_set_arr");
            self.com_arrs.borrow_mut().push(arr);
        }
        fn set_and_enable_com_int(&self, arr: u16) {
            self.calls.borrow_mut().push("set_and_enable_com_int");
            self.com_arrs.borrow_mut().push(arr);
        }
        fn disable_com_timer_int(&self) {
            self.calls.borrow_mut().push("disable_com_timer_int");
        }
        fn com_clear_flag(&self) {
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
        pub(crate) stop_req: AtomicBool,
        pub(crate) dump_req: AtomicBool,
        pub(crate) info_req: AtomicBool,
        pub(crate) zct_stream_on: AtomicBool,
    }
    impl BenchStore {
        pub(crate) fn bench(&self) -> Bench<'_> {
            Bench {
                uart_deadman_ticks: &self.uart_deadman_ticks,
                i_raw: &self.i_raw,
                vbat_raw: &self.vbat_raw,
                oc_acc: &self.oc_acc,
                oc_cnt: &self.oc_cnt,
                stop_req: &self.stop_req,
                dump_req: &self.dump_req,
                info_req: &self.info_req,
                zct_stream_on: &self.zct_stream_on,
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
