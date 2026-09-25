//! A simulated board for the controller's host tests (goal item 1): a
//! scripted crossing sequence in, the controller's commutation and stop
//! decisions out.
//!
//! Time advances a fixed quantum on every `now()`. Scans arrive every 101 µs
//! with a biased, current-free shunt and a steady bus. The driven stage sees
//! one acceptance per driven sector (mid-sector, tagged with the controller's
//! own epoch and step); after the handover the script supplies the closed
//! loop's crossings, and the simulated COM root commutates on each. Every
//! decision the controller takes is logged in [`Log`].

extern crate std;

use std::string::String;
use std::vec::Vec;

use crate::bemf::ZeroCross;
use crate::commutation::Step;
use crate::protection::RawScan;
use crate::report::{GuardRecord, Roots, Sink};
use crate::sixstep::Plan;

use super::hal::{DrivenAccept, Drives, Gates, Hal, Inject, Preflight, Stopped};

/// The closed-loop script: crossings every `interval_us` from the seed edge,
/// until `until_us` (absolute sim time), if set.
#[derive(Clone, Copy, Debug)]
pub struct Crossings {
    pub interval_us: u32,
    pub until_us: Option<u32>,
}

/// Faults a test can schedule (absolute sim time).
#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    pub preflight_fails: bool,
    pub nfault_low_at: Option<u32>,
    pub bus_sag_at: Option<u32>,
    pub stop_key_at: Option<u32>,
    /// The simulated guard latches `Tracking` this long after the last
    /// crossing once the loop is closed.
    pub guard_stale_us: Option<u32>,
    /// Campaign 8's hard stops: the roots' counters read non-zero from this
    /// instant, so the stop path itself is exercised on the host.
    pub late_arm_at: Option<u32>,
    pub blank_latched_at: Option<u32>,
}

/// Everything the controller decided.
#[derive(Debug, Default)]
pub struct Log {
    pub gate_writes: u32,
    pub safe_offs: u32,
    pub moe_on_at: Option<u32>,
    pub driven_begun_at: Option<u32>,
    /// Seed interval, step, and the seed edge on the extended clock.
    pub seed: Option<(u32, u8, u32)>,
    pub handover: Option<(u16, u32)>,
    /// When the loop closed (the estimator was installed).
    pub closed_at: Option<u32>,
    pub plans: Vec<(u32, u16)>,
    pub advances: Vec<u32>,
    pub injected: Vec<(u32, Inject)>,
    pub crossings_delivered: u32,
    /// Bytes written while the bridge was driven (between MOE on and
    /// `safe_off`): must stay zero.
    pub bytes_while_driven: u32,
    pub text: String,
}

pub struct Sim {
    /// Level-revisit polls the loop asked for (E140's rescue test).
    pub revisit_polls: u32,
    pub t: u32,
    pub quantum_us: u32,
    last_scan: u32,
    scan_seq: u32,
    pub crossings: Crossings,
    pub faults: Faults,
    pub log: Log,
    driven: bool,
    drv_step: Step,
    drv_epoch: u32,
    drv_sector_at: u32,
    drv_emitted_epoch: Option<u32>,
    drv_last_accept: Option<u32>,
    closed: bool,
    next_crossing: u32,
    last_crossing: u32,
    pending_raw: Option<u16>,
    com_step: Step,
    com_count: u32,
    zc: Option<ZeroCross>,
    guard_reason: u32,
    driving: bool,
}

impl Sim {
    pub fn new(crossings: Crossings, faults: Faults) -> Self {
        Self {
            revisit_polls: 0,
            t: 1_000,
            quantum_us: 7,
            last_scan: 0,
            scan_seq: 0,
            crossings,
            faults,
            log: Log::default(),
            driven: false,
            drv_step: Step::new_clamped(1),
            drv_epoch: 0,
            drv_sector_at: 0,
            drv_emitted_epoch: None,
            drv_last_accept: None,
            closed: false,
            next_crossing: 0,
            last_crossing: 0,
            pending_raw: None,
            com_step: Step::new_clamped(1),
            com_count: 0,
            zc: None,
            guard_reason: 0,
            driving: false,
        }
    }

    fn at(&self, t: Option<u32>) -> bool {
        t.is_some_and(|x| self.t >= x)
    }

    /// The simulated roots: deliver the next closed-loop crossing when due,
    /// and latch the guard's tracking stop if crossings stop.
    fn roots(&mut self) {
        if !self.closed {
            return;
        }
        let live = self.crossings.until_us.is_none_or(|u| self.next_crossing < u);
        if live && self.t >= self.next_crossing && self.pending_raw.is_none() {
            self.pending_raw = Some(self.next_crossing as u16);
            self.last_crossing = self.next_crossing;
            self.next_crossing += self.crossings.interval_us;
            self.com_step = self.com_step.next();
            self.com_count += 1;
            self.log.crossings_delivered += 1;
        }
        if let Some(stale) = self.faults.guard_stale_us {
            if self.guard_reason == 0 && self.t.wrapping_sub(self.last_crossing) > stale {
                self.guard_reason = 8;
            }
        }
    }
}

impl Sink for Sim {
    fn put(&mut self, b: u8) {
        if self.driving {
            self.log.bytes_while_driven += 1;
        }
        self.log.text.push(b as char);
    }
}

impl Hal for Sim {
    fn now(&mut self) -> u32 {
        self.t = self.t.wrapping_add(self.quantum_us);
        self.roots();
        self.t
    }
    fn raw(&self) -> u16 {
        self.t as u16
    }
    fn stamp_from_raw(&mut self, raw: u16) -> u32 {
        let now = self.now();
        now.wrapping_sub(u32::from((now as u16).wrapping_sub(raw)))
    }
    fn drain(&mut self) {}
    fn flush_link(&mut self) {}
    fn rx(&mut self) -> Option<u8> {
        if self.at(self.faults.stop_key_at) {
            self.faults.stop_key_at = None;
            return Some(b'o');
        }
        None
    }
    fn led(&mut self, _on: bool) {}
    fn nfault_high(&self) -> bool {
        !self.at(self.faults.nfault_low_at)
    }
    fn enable(&mut self, _on: bool) {}
    fn preflight(&self) -> Preflight {
        Preflight {
            moe: false,
            ccr: [0; 3],
            gates_low: true,
            enable: false,
            nfault_high: !self.faults.preflight_fails,
        }
    }
    fn adc_due(&mut self) -> bool {
        self.t.wrapping_sub(self.last_scan) >= 101
    }
    fn adc_stale(&self, _now: u32) -> bool {
        false
    }
    fn scan(&mut self) -> Option<RawScan> {
        self.last_scan = self.t;
        self.scan_seq += 2;
        let bus = if self.at(self.faults.bus_sag_at) { 900 } else { 1215 };
        Some(RawScan {
            phase_a: 2048,
            phase_b: 2048,
            phase_c: 2048,
            bus,
            vref: 1500,
        })
    }
    fn resync_adc(&mut self) {}
    fn vdda_mv(&self, _vref: u16) -> u32 {
        3_300
    }
    fn comp_inputs(&self) -> (u16, u16) {
        (0, 0)
    }
    fn pwm_counter(&self) -> u32 {
        0
    }
    fn gates_to_timer<S: Drives>(&mut self, _g: &mut Gates<S>) {
        self.log.gate_writes += 1;
    }
    fn all_phases_pwm<S: Drives>(&mut self, _g: &mut Gates<S>) {
        self.log.gate_writes += 1;
    }
    fn set_compares<S: Drives>(&mut self, _g: &mut Gates<S>, _logical: [u32; 3]) {
        self.log.gate_writes += 1;
    }
    fn apply_plan<S: Drives>(&mut self, _g: &mut Gates<S>, _plan: &Plan) {
        self.log.gate_writes += 1;
    }
    fn moe_on<S: Drives>(&mut self, _g: &mut Gates<S>) {
        self.log.gate_writes += 1;
        self.log.moe_on_at = Some(self.t);
        self.driving = true;
    }
    fn set_period(&mut self, _period: u32) {}
    fn float_all(&mut self) {}
    fn safe_off<S>(&mut self, g: Gates<S>) -> Gates<Stopped> {
        self.log.safe_offs += 1;
        self.driving = false;
        g.into_stopped()
    }
    fn comp_mask(&mut self) {}
    fn comp_arm(&mut self, _step: Step) {}
    fn comp_select(&mut self, _step: Step) {}
    fn comp_level(&self) -> bool {
        false
    }
    fn comp_hysteresis(&mut self, _hyst: u8) {}
    fn comp_run_hysteresis(&mut self) {}
    fn edge_rising(&self, step: Step) -> bool {
        step.rising()
    }
    fn reset_roots(&mut self) {}
    fn guard_arm(&mut self) {}
    fn guard_reason(&self) -> u32 {
        self.guard_reason
    }
    fn storm(&self) -> bool {
        false
    }
    fn unstable_count(&self) -> u32 {
        0
    }
    fn wait_hist(&self) -> [u32; 8] {
        [0; 8]
    }
    fn left_hist(&self) -> [u32; 8] {
        [0; 8]
    }

    fn late_arms(&self) -> u32 {
        u32::from(self.faults.late_arm_at.is_some_and(|t| self.t >= t))
    }
    fn blank_latched(&self) -> u32 {
        u32::from(self.faults.blank_latched_at.is_some_and(|t| self.t >= t))
    }
    fn overrun(&self) -> bool {
        false
    }
    fn cap_armed(&self) -> bool {
        true
    }
    fn arm_cap(&mut self) {}
    fn drv_begin(&mut self, step: Step) {
        self.driven = true;
        self.drv_step = step;
        self.drv_epoch = 0;
        self.drv_sector_at = self.t;
        self.log.driven_begun_at = Some(self.t);
    }
    fn drv_advance(&mut self, step: Step, epoch: u32) {
        self.drv_step = step;
        self.drv_epoch = epoch;
        self.drv_sector_at = self.t;
    }
    /// One acceptance per driven sector, 400 µs in.
    fn drv_poll(&mut self) -> Option<DrivenAccept> {
        if !self.driven || self.drv_emitted_epoch == Some(self.drv_epoch) {
            return None;
        }
        let at = self.drv_sector_at + 400;
        if self.t < at {
            return None;
        }
        self.drv_emitted_epoch = Some(self.drv_epoch);
        let interval = self.drv_last_accept.map_or(833, |p| at.wrapping_sub(p));
        self.drv_last_accept = Some(at);
        Some(DrivenAccept {
            raw: at as u16,
            epoch: self.drv_epoch as u16,
            step: self.drv_step,
            interval_us: interval,
            position_us: 400,
        })
    }
    fn drv_resume_deferred(&mut self, _step: Step) -> bool {
        false
    }
    fn drv_end(&mut self) {
        self.driven = false;
    }
    fn det_install(&mut self, zc: ZeroCross, seed_us: u32, raw: u16, step: Step, _advance: u32) {
        self.zc = Some(zc);
        let edge = self.t.wrapping_sub(u32::from((self.t as u16).wrapping_sub(raw)));
        self.log.seed = Some((seed_us, step.get(), edge));
        self.log.closed_at = Some(self.t);
        self.com_step = step;
        self.last_crossing = edge;
        self.next_crossing = edge + self.crossings.interval_us;
        self.closed = true;
    }
    fn com_handover(&mut self, duty: u16, _period: u32, _step: Step, commit_us: u32) {
        self.log.handover = Some((duty, commit_us));
    }
    fn det_poll(&mut self) -> Option<u16> {
        self.pending_raw.take()
    }
    fn det_average(&self) -> Option<u32> {
        self.zc.as_ref().map(|_| self.crossings.interval_us)
    }
    fn com_step(&self) -> Step {
        self.com_step
    }
    fn com_idle(&self) -> bool {
        // Idle between a commutation and the next crossing, which is when the
        // level revisit is allowed to poll. Hard-coded `false` until E140,
        // which left the revisit path with no host coverage at all.
        self.pending_raw.is_none()
    }
    fn com_count(&self) -> u32 {
        self.com_count
    }
    fn revisit(&mut self, _step: Step) -> bool {
        // Counted so a test can see the rescue attempts E140 added; the poll
        // itself never finds a held level in the sim.
        self.revisit_polls = self.revisit_polls.wrapping_add(1);
        false
    }
    fn publish_plans(&mut self, duty: u16, _period: u32, _cap: u16) {
        self.log.plans.push((self.t, duty));
    }
    fn set_advance(&mut self, advance: u32) {
        self.log.advances.push(advance);
    }
    fn det_release(&mut self) {
        self.closed = false;
    }
    fn take_back(&mut self) {
        self.driven = false;
    }
    fn inject(&mut self, kind: Inject) {
        self.log.injected.push((self.t, kind));
        if kind == Inject::Tracking {
            // The detector stops deciding: no more crossings reach the loop.
            self.crossings.until_us = Some(self.t);
        }
    }
    fn undo_inject(&mut self, _kind: Inject) {}
    fn roots_record(&mut self) -> Roots {
        Roots {
            zc_accepted: self.log.crossings_delivered,
            ..Roots::default()
        }
    }
    fn guard_record(&mut self, loop_iters_closed: u32, loop_gap_max_us: u32, acquire_us: u32) -> GuardRecord {
        GuardRecord {
            reason: self.guard_reason,
            loop_iters_closed,
            loop_gap_max_us,
            acquire_us,
            ..GuardRecord::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commutation::wait_time;
    use crate::protection::Reason;
    use crate::run::{Production, Request, Window};

    /// Pull `key=value` out of the report line starting with `head`.
    fn field(text: &str, head: &str, key: &str) -> u32 {
        let line = text
            .lines()
            .rev()
            .find(|l| l.starts_with(head))
            .unwrap_or_else(|| panic!("no {head} line"));
        line.split_whitespace()
            .find_map(|t| t.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
            .unwrap_or_else(|| panic!("no {key} in {line}"))
            .parse()
            .unwrap()
    }

    fn run(
        target: u16,
        window_us: u32,
        crossings: Crossings,
        faults: Faults,
        inject: Option<(Inject, u32)>,
    ) -> (Sim, crate::run::Outcome) {
        let mut sim = Sim::new(crossings, faults);
        let out = Production::new().run(
            &mut sim,
            Request {
                target_tenths: target,
                inject,
                window: Window::ForUs(window_us),
            },
        );
        (sim, out)
    }

    const STEADY: Crossings = Crossings {
        interval_us: 144,
        until_us: None,
    };

    #[test]
    fn a_scripted_crossing_train_locks_ramps_and_stops_at_the_window() {
        // 4.7 s of startup and 7.5 s of ramp before the 25% hold.
        let (sim, out) = run(250, 14_000_000, STEADY, Faults::default(), None);
        let log = &sim.log;
        assert_eq!(out.reason, Reason::SegmentDeadline);
        assert_eq!(log.safe_offs, 1, "one safe_off, at the stop");
        // The seed: twelve driven intervals of 833 µs, at the driven step.
        let (seed_us, _, _) = log.seed.expect("the driven stage must seed");
        assert_eq!(seed_us, 833);
        // The first commutation lands `wait` after the seed edge, at 10% duty.
        let (duty, _) = log.handover.unwrap();
        assert_eq!(duty, 100, "transfer at the reference's bemfdu100");
        // The ramp publishes each 1% step up to 25%, at the below-the-step
        // advance throughout. Asserted against `ADVANCE_LOW` rather than the
        // literal 20 (E330): the ramp runs below `ADVANCE_STEP_TENTHS`, so the
        // quantity under test is that constant, and `advance-ref` moves it to
        // the reference 16 at both ends.
        let duties: Vec<u16> = log.plans.iter().map(|&(_, d)| d).collect();
        assert!(duties.windows(2).all(|w| w[1] > w[0]), "ramp only rises: {duties:?}");
        assert_eq!(duties.last(), Some(&250));
        assert!(log.advances.iter().all(|&a| a == crate::run::policy::ADVANCE_LOW));
        // Every crossing the script delivered was counted as an acceptance.
        assert_eq!(field(&log.text, "BEMFDONE", "accepted"), log.crossings_delivered);
        assert_eq!(field(&log.text, "BEMFDONE", "reason"), 2);
        assert!(
            field(&log.text, "BEMFGATE", "hold_ms") > 0,
            "the ramp reached target and held"
        );
        // Nothing was written while the bridge was driven (E073).
        assert_eq!(log.bytes_while_driven, 0);
    }

    /// The matched speed window (campaign 8 step 3): it ends at the last
    /// accepted crossing, is one to two `TAIL_WINDOW_US` long, and its accept
    /// count matches its own span at the scripted interval. Nothing here is
    /// rounded, which is the point of reporting counts and spans rather than a
    /// frequency.
    #[test]
    fn the_matched_window_ends_at_the_last_crossing_and_states_its_own_span() {
        use super::super::policy::TAIL_WINDOW_US;
        let (sim, _) = run(250, 20_000_000, STEADY, Faults::default(), None);
        let t = &sim.log.text;
        let accepts = field(t, "BEMFTAIL", "accepts");
        let span = field(t, "BEMFTAIL", "span_us");
        let start_back = field(t, "BEMFTAIL", "start_before_stop_us");
        let end_back = field(t, "BEMFTAIL", "end_before_stop_us");
        assert!(
            span >= TAIL_WINDOW_US,
            "the window is at least one anchor apart: {span}"
        );
        assert!(span < 2 * TAIL_WINDOW_US, "and at most two: {span}");
        assert_eq!(start_back - end_back, span, "the endpoints bracket the span");
        assert!(
            end_back < TAIL_WINDOW_US,
            "the window ends at the stop, not mid-hold: {end_back}"
        );
        // At a 144 µs scripted interval the count is the span over 144, and
        // the window's own arithmetic is exact to one crossing.
        let expect = span / STEADY.interval_us;
        assert!(
            accepts.abs_diff(expect) <= 1,
            "accepts {accepts} against span/interval {expect}"
        );
    }

    /// Campaign 8's first hard stop: one commutation armed with its wait
    /// already spent ends the run with its own code, through `safe_off`.
    #[test]
    fn an_exhausted_commutation_deadline_ends_the_run() {
        let faults = Faults {
            late_arm_at: Some(13_000_000),
            ..Faults::default()
        };
        let (sim, out) = run(250, 20_000_000, STEADY, faults, None);
        assert_eq!(out.reason, Reason::LateArm);
        assert_eq!(Reason::LateArm.code(), 15);
        assert_eq!(sim.log.safe_offs, 1, "the stop is the ordinary protection route");
        assert_eq!(sim.log.bytes_while_driven, 0);
    }

    /// The second: a comparator edge the blanking window latched.
    #[test]
    fn a_latched_blanking_edge_ends_the_run() {
        let faults = Faults {
            blank_latched_at: Some(13_000_000),
            ..Faults::default()
        };
        let (sim, out) = run(250, 20_000_000, STEADY, faults, None);
        assert_eq!(out.reason, Reason::BlankLatched);
        assert_eq!(Reason::BlankLatched.code(), 16);
        assert_eq!(sim.log.safe_offs, 1);
    }

    /// Neither stop is armed before the loop closes: the driven and
    /// acquisition stages count both quantities and run through them.
    #[test]
    fn the_hard_stops_are_not_armed_before_the_loop_closes() {
        let faults = Faults {
            late_arm_at: Some(0),
            ..Faults::default()
        };
        let (sim, out) = run(250, 6_000_000, STEADY, faults, None);
        assert!(sim.log.closed_at.is_some(), "the loop still closed");
        assert_eq!(out.reason, Reason::LateArm, "and then stopped, once closed");
        let closed_at = sim.log.closed_at.unwrap();
        assert!(closed_at > 0, "the stop did not pre-empt the startup stages");
    }

    #[test]
    fn the_first_commutation_is_wait_after_the_seed_edge() {
        let (sim, _) = run(150, 6_000_000, STEADY, Faults::default(), None);
        let (_, commit_us) = sim.log.handover.unwrap();
        let (seed_us, _, edge) = sim.log.seed.unwrap();
        // The seed edge is this sector's crossing: its commutation is due
        // `wait` after it, at the transfer duty's advance -- which is
        // `ADVANCE_LOW`, since the handover duty is below the schedule's step.
        // Named rather than written as 20 (E330), so the assertion follows the
        // configuration instead of pinning one build's value.
        assert_eq!(
            commit_us,
            edge + wait_time(seed_us, crate::run::policy::ADVANCE_LOW).max(1)
        );
        // Thirteen driven acceptances (epoch 0 never anchors), 833 µs apart.
        let begun = sim.log.driven_begun_at.unwrap();
        assert!(edge > begun + 12 * 833);
    }

    #[test]
    fn crossings_that_stop_end_on_the_guard_s_tracking_stop() {
        let crossings = Crossings {
            interval_us: 144,
            until_us: Some(6_000_000),
        };
        let faults = Faults {
            guard_stale_us: Some(3 * 144),
            ..Faults::default()
        };
        let (sim, out) = run(250, 10_000_000, crossings, faults, None);
        assert_eq!(out.reason, Reason::Tracking);
        assert_eq!(sim.log.safe_offs, 1);
        assert_eq!(field(&sim.log.text, "BEMFGUARD", "reason"), 8);
    }

    /// E140: a sector that never accepts still gets looked at again.
    ///
    /// The revisit is once per sector, cleared by an accept, so before E140 a
    /// sector whose crossing was swallowed was never revisited -- the chain
    /// that desynced the drive at 30% (E138). With the rescue, the loop keeps
    /// polling that sector while it is overdue, up to `REVISIT_RESCUE_MAX`.
    #[test]
    fn a_sector_that_never_accepts_is_revisited_again_while_it_is_overdue() {
        // Crossings stop mid-run; the guard is given a long fuse so the loop
        // sits in the dead sector instead of being stopped at once.
        let crossings = Crossings {
            interval_us: 144,
            until_us: Some(6_000_000),
        };
        let faults = Faults {
            guard_stale_us: Some(50_000),
            ..Faults::default()
        };
        let (stalled, _) = run(250, 10_000_000, crossings, faults, None);
        let (steady, _) = run(250, 6_100_000, STEADY, Faults::default(), None);
        // Per sector the steady run polls about once; the stalled one adds the
        // rescues on top, so its polls per delivered crossing are higher.
        let rate = |s: &Sim| f64::from(s.revisit_polls) / f64::from(s.log.crossings_delivered.max(1));
        assert!(
            rate(&stalled) > rate(&steady),
            "stalled {:.3} polls/crossing vs steady {:.3}",
            rate(&stalled),
            rate(&steady)
        );
        assert!(stalled.revisit_polls > 0);
    }

    #[test]
    fn nfault_stops_the_run_on_driver() {
        let faults = Faults {
            nfault_low_at: Some(5_500_000),
            ..Faults::default()
        };
        let (sim, out) = run(250, 10_000_000, STEADY, faults, None);
        assert_eq!(out.reason, Reason::Driver);
        assert_eq!(sim.log.safe_offs, 1);
    }

    #[test]
    fn a_bus_sag_stops_the_run() {
        let faults = Faults {
            bus_sag_at: Some(5_500_000),
            ..Faults::default()
        };
        let (_, out) = run(250, 10_000_000, STEADY, faults, None);
        assert!(
            matches!(out.reason, Reason::FastBusSag | Reason::Bus),
            "{:?}",
            out.reason
        );
    }

    #[test]
    fn the_host_stop_key_aborts() {
        let faults = Faults {
            stop_key_at: Some(5_500_000),
            ..Faults::default()
        };
        let (_, out) = run(250, 10_000_000, STEADY, faults, None);
        assert_eq!(out.reason, Reason::HostAbort);
    }

    #[test]
    fn a_failed_preflight_writes_no_gate_and_reports_nothing() {
        let faults = Faults {
            preflight_fails: true,
            ..Faults::default()
        };
        let (sim, out) = run(250, 10_000_000, STEADY, faults, None);
        assert_eq!(out.reason, Reason::Driver);
        assert_eq!(sim.log.gate_writes, 0);
        assert_eq!(sim.log.safe_offs, 1, "refused runs still go through safe_off");
        assert!(sim.log.text.contains("verdict=FAIL"));
        assert!(!sim.log.text.contains("BEMFDONE"));
    }

    #[test]
    fn a_lowercase_key_provokes_from_the_locked_25_percent_loop() {
        let faults = Faults {
            guard_stale_us: Some(3 * 144),
            ..Faults::default()
        };
        let mut sim = Sim::new(STEADY, faults);
        Production::new().command(&mut sim, b't');
        let &(at, kind) = sim.log.injected.first().expect("the stimulus fired");
        assert_eq!(kind, Inject::Tracking);
        let closed_at = sim.log.closed_at.unwrap();
        let after = crate::ramp::ramp_us(250) + 2_000_000;
        assert!(at.abs_diff(closed_at + after) < 1_000, "{at} vs {closed_at}");
        // At target when it fired: the last published duty is 25%.
        assert_eq!(sim.log.plans.last().map(|&(_, d)| d), Some(250));
        assert!(sim.log.text.contains("BEMFRUN handoff_ehz=200 target_duty_tenths=250"));
        assert!(sim.log.text.contains("BEMFINJECT expected_reason=8 reason=8"));
    }

    #[test]
    fn an_injection_fires_three_seconds_into_the_closed_loop() {
        let faults = Faults {
            guard_stale_us: Some(3 * 144),
            ..Faults::default()
        };
        let (sim, out) = run(150, 12_000_000, STEADY, faults, Some((Inject::Tracking, 3_000_000)));
        assert_eq!(out.reason, Reason::Tracking);
        let &(at, kind) = sim.log.injected.first().expect("the stimulus fired");
        assert_eq!(kind, Inject::Tracking);
        let closed_at = sim.log.closed_at.unwrap();
        // Fired on the first pass 3 s after the loop closed (the controller
        // stamps the close at its pass time, a few quanta before the install).
        assert!(at.abs_diff(closed_at + 3_000_000) < 1_000, "{at} vs {closed_at}");
        assert!(sim.log.text.contains("BEMFINJECT expected_reason=8 reason=8"));
    }
}
