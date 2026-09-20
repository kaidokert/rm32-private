// Adapted from frozen minz/core/src/am32_hal.rs mock (2026-09-12).
// Storage/call recording only; controller logic remains in minz-core.
#![allow(dead_code)]
use minz_core::am32_hal::*;
    use minz_core::am32::{ZCT_REC, ZctRing};
    use minz_core::am32_loop::{Bench, Drive, Duty, Sched};
    use minz_core::zct_trace::ZctTrace;
    use core::cell::{Cell, RefCell};
    use portable_atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

    #[derive(Default)]
    pub(crate) struct MockHal {
        pub(crate) now: Cell<u64>,
        pub(crate) comp_enabled: Cell<bool>,
        pub(crate) com_deadline: Cell<Option<u64>>,
        pub(crate) com_preload: Cell<bool>,
        pub(crate) com_active_arr: Cell<u16>,
        pub(crate) com_shadow_arr: Cell<u16>,
        pub(crate) timed_roles: RefCell<Vec<(u64, u8)>>,
        pub(crate) calls: RefCell<Vec<&'static str>>,
        pub(crate) roles: RefCell<Vec<u8>>,
        /// `(step, rising)` pairs AS PASSED to `Comparator::set_step`
        /// (rung 3: the rm32 two-call shape â€” set_step stores, a
        /// following change_input applies).
        pub(crate) steps: RefCell<Vec<(u8, bool)>>,
        pub(crate) carrier_arrs: RefCell<Vec<u16>>,
        pub(crate) duties: RefCell<Vec<u16>>,
        pub(crate) com_arrs: RefCell<Vec<u16>>,
        pub(crate) events: RefCell<Vec<(u8, u8, u16)>>,
        pub(crate) timed_events: RefCell<Vec<(u64, u8, u8, u16)>>,
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
        /// Optional reference-polarity, sample-held all-phase input timeline.
        pub(crate) phase_samples: RefCell<Vec<(u64,usize,bool)>>,
        source_phase: Cell<usize>,
        source_rising: Cell<bool>,
    }

    impl MockHal {
        pub(crate) fn advance(&self, ticks: u32) {
            let before=self.now.get();
            let after=before+u64::from(ticks);
            for &(time,phase,level) in self.phase_samples.borrow().iter() {
                if time>before && time<=after && phase==self.source_phase.get() {
                    let previous=self.comp_value.replace(level);
                    if previous!=level && level==self.source_rising.get() && self.comp_enabled.get() {
                        self.pending.set(true);
                    }
                }
            }
            self.now.set(after);
            self.interval.set((self.interval.get() + ticks) & 0xffff);
        }
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
    // trait method names; `roles` records the step AS PASSED (1..6 â€”
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
            self.timed_roles.borrow_mut().push((self.now.get(), step));
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
    // `steps` records the `(step, rising)` pair AS PASSED (1..6 â€” the
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
            if !self.phase_samples.borrow().is_empty() {
                self.comp_enabled.set(false);
                self.pending.set(false);
                let (step,rising)=*self.steps.borrow().last().unwrap();
                let phase=[2,0,1,2,0,1][step as usize-1];
                self.source_phase.set(phase);self.source_rising.set(rising);
                if let Some(&(_,_,level))=self.phase_samples.borrow().iter().rev()
                    .find(|&&(time,p,_)|p==phase && time<=self.now.get()) {
                    self.comp_value.set(level);
                }
            }
        }
        fn enable_interrupts(&mut self) {
            self.comp_enabled.set(true);
            self.calls.borrow_mut().push("enable_interrupts");
        }
        fn mask_interrupts(&mut self) {
            self.comp_enabled.set(false);
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
    // strings kept IDENTICAL to the pre-rung-1 `ComTimers` names â€” the
    // test assertions depend on them.
    impl IntervalTimer for &MockHal {
        fn count(&self) -> u32 {
            let v = self.interval.get();
            self.advance(self.interval_step.get());
            v
        }
        fn set_count(&mut self, val: u32) {
            self.calls.borrow_mut().push("set_interval_cnt");
            self.interval.set(val);
        }
    }

    impl ComTimer for &MockHal {
        fn set_and_enable(&mut self, timeout: u16) {
            self.com_shadow_arr.set(timeout);
            if !self.com_preload.get() { self.com_active_arr.set(timeout); }
            self.com_deadline.set(Some(self.now.get() + u64::from(self.com_active_arr.get()) + 1));
            self.calls.borrow_mut().push("set_and_enable_com_int");
            self.com_arrs.borrow_mut().push(timeout);
        }
        fn disable_interrupt(&mut self) {
            self.com_deadline.set(None);
            self.calls.borrow_mut().push("disable_com_timer_int");
        }
        fn enable_interrupt(&mut self) {
            self.calls.borrow_mut().push("enable_com_timer_int");
        }
    }

    impl ComTimerExt for &MockHal {
        fn com_set_arr(&mut self, arr: u16) {
            self.com_shadow_arr.set(arr);
            if !self.com_preload.get() { self.com_active_arr.set(arr); }
            self.calls.borrow_mut().push("com_set_arr");
            self.com_arrs.borrow_mut().push(arr);
        }
        fn com_clear_flag(&mut self) {
            // Service follows overflow, which transfers ARR preload.
            self.com_active_arr.set(self.com_shadow_arr.get());
            self.calls.borrow_mut().push("com_clear_flag");
        }
    }

    impl Recorder for MockHal {
        fn record(&self, ty: u8, sector: u8, data: u16) {
            self.timed_events.borrow_mut().push((self.now.get(), ty, sector, data));
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
