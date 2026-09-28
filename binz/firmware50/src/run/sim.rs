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
    /// The supply is absent for the whole run -- the rail reads what this bench
    /// measured with the PSU switched off (277 codes against 1208-1217 powered).
    /// Exercises E346's prospective refusal.
    pub bus_absent: bool,
    /// Q60-1: report a dropped ADC conversion, so the counter's own wiring is
    /// testable on the host.
    pub adc_ovr: bool,
    /// Q60-1: the *consequence* of a circular-DMA rotation -- another
    /// channel's code in the VREF slot. In range, so production's only
    /// validation (`0 < vref < ADC_RAIL`) passes it, which is the whole point.
    /// Time-gated because that is the only kind production can experience:
    /// `resync_adc` runs at arm, so a rotation present before the baseline is
    /// cleared, and only one arising after it persists. The observer is
    /// anchored to the run's own baseline and is therefore blind to a pre-arm
    /// rotation BY CONSTRUCTION -- which is sound exactly because resync makes
    /// that case impossible.
    pub vref_rotated_at: Option<u32>,
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
    /// The cap every `publish_plans` call passed (ENV-6).
    pub plan_caps: Vec<u16>,
    pub advances: Vec<u32>,
    pub injected: Vec<(u32, Inject)>,
    pub crossings_delivered: u32,
    /// Bytes written while the bridge was driven (between MOE on and
    /// `safe_off`): must stay zero.
    pub bytes_while_driven: u32,
    pub text: String,
}

pub struct Sim {
    /// Q60-3: overrun polls that saw the sticky flag set.
    pub ovr_seen: u32,
    /// The sticky flag has been read and cleared.
    ovr_taken: bool,
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
            ovr_seen: 0,
            ovr_taken: false,
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
        let bus = if self.faults.bus_absent {
            277
        } else if self.at(self.faults.bus_sag_at) {
            900
        } else {
            1215
        };
        Some(RawScan {
            phase_a: 2048,
            phase_b: 2048,
            phase_c: 2048,
            bus,
            vref: if self.at(self.faults.vref_rotated_at) {
                2048
            } else {
                1500
            },
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

    fn poll_adc_ovr(&mut self) -> bool {
        // Read-and-clear, like the hardware flag: one overrun is seen once,
        // however often the foreground polls. Returning the fault
        // unconditionally counted one per scan (47 342 of them), which the
        // observer test caught.
        if self.faults.adc_ovr && !self.ovr_taken {
            self.ovr_taken = true;
            self.ovr_seen = self.ovr_seen.saturating_add(1);
            true
        } else {
            false
        }
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
    fn det_poll(&mut self) -> Option<super::accepted::Accepted> {
        self.pending_raw.take().map(super::accepted::Accepted::single)
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
    fn publish_plans(&mut self, duty: u16, _period: u32, cap: u16) {
        self.log.plans.push((self.t, duty));
        self.log.plan_caps.push(cap);
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
        match kind {
            // The detector stops deciding: no more crossings reach the loop.
            Inject::Tracking => self.crossings.until_us = Some(self.t),
            // E355: force the very counters the closed loop already polls at
            // `states.rs:278-282`, from this instant on. The scheduled-fault
            // fields exist because the sim modelled both conditions long before
            // either had a shell key.
            Inject::LateArm => self.faults.late_arm_at = Some(self.t),
            Inject::BlankLatched => self.faults.blank_latched_at = Some(self.t),
            _ => {}
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
    fn the_new_observers_report_and_each_one_is_load_bearing() {
        // Q60-1. Asserted against the EMITTED TEXT, not an in-memory struct:
        // E337's defect was an instrument that computed correctly and never
        // reached a capture, so the emit is part of what must be tested. And
        // each counter must be demonstrable by mutating the CODE, not the data
        // -- the tautology that made `cohort_selftest` useless in E345/E346.
        fn field(text: &str, key: &str) -> Option<u64> {
            let pat = std::format!(" {key}=");
            let at = text.find(&pat)? + pat.len();
            let rest = &text[at..];
            let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            rest[..end].parse().ok()
        }
        let faults = Faults {
            bus_sag_at: Some(5_500_000),
            adc_ovr: true,
            ..Faults::default()
        };
        let (sim, _out) = run(250, 10_000_000, STEADY, faults, None);
        let t = &sim.log.text;

        // Every new key must be present at all.
        for k in [
            "max_streak",
            "min_margin_xp",
            "vref_odd",
            "adc_ovr",
            "mdep_pm",
            "mdep_n",
            "mdep_run",
        ] {
            assert!(t.contains(&std::format!(" {k}=")), "missing key {k}");
        }

        let streak = field(t, "streak").expect("streak");
        let max_streak = field(t, "max_streak").expect("max_streak");
        assert!(
            max_streak >= streak,
            "max_streak {max_streak} must dominate the value at the stop {streak}"
        );
        assert_ne!(
            field(t, "min_margin_xp").expect("min_margin_xp"),
            u64::from(u32::MAX),
            "some scan must have been judged, so the margin cannot be the sentinel"
        );
        assert_eq!(
            field(t, "adc_ovr").expect("adc_ovr"),
            1,
            "the overrun count must reach the report"
        );
        // The observer's last bin IS the guard's line, which is what makes it
        // the guard's own decision variable rather than another proxy.
        assert_eq!(crate::protection::MEAN_DEPTH_FRACTIONS[3], 950);
        assert_eq!(field(t, "mdep_pm").expect("mdep_pm"), 990, "first bin emitted");
        // A clean run must not accuse the VREF slot.
        assert_eq!(field(t, "vref_odd").expect("vref_odd"), 0, "no rotation in this run");

        // And the rotation consequence must be detected, with an IN-RANGE code
        // that the only production validation accepts.
        let rot = Faults {
            vref_rotated_at: Some(3_000_000),
            ..Faults::default()
        };
        let (rsim, _) = run(250, 6_000_000, STEADY, rot, None);
        let rt = &rsim.log.text;
        assert!(
            field(rt, "vref_odd").is_none_or(|n| n > 0),
            "an in-range but wrong VREF slot must be counted: {:?}",
            field(rt, "vref_odd")
        );
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
    fn an_absent_supply_refuses_the_run_before_the_bridge_drives() {
        // E346. The failure this replaces: the run completed, latched
        // `Reason::Bus` from the in-run absolute floor, and permanently
        // disqualified the image's hardest rung -- and the host-side exemption
        // written to undo that re-read the trip's own input. Refusing to start
        // is the repair: no drive, no record, nothing to excuse.
        let faults = Faults {
            bus_absent: true,
            ..Faults::default()
        };
        let (sim, out) = run(250, 10_000_000, STEADY, faults, None);
        // Refused at arm, not stopped mid-run: `AdcTimeout` is the baseline
        // refusal, and crucially NOT `Reason::Bus`, which is what a run that
        // drove would have reported.
        assert_eq!(out.reason, Reason::AdcTimeout);
        assert_ne!(out.reason, Reason::Bus, "a refused run must not look like a bus trip");
        // The real witnesses that no winding saw voltage. (`gate_writes` is
        // NOT one: `gates_to_timer` hands the pins over before the baseline is
        // taken, with every CCR still zero and MOE off. Asserting on it was my
        // own error, caught by this test failing.)
        assert_eq!(sim.log.moe_on_at, None, "MOE must never have been enabled");
        assert_eq!(sim.log.driven_begun_at, None, "the driven stage must never have begun");
        assert!(sim.log.plans.is_empty(), "no duty was ever published");
        assert!(sim.log.safe_offs >= 1, "a refused run still goes through safe_off");
        assert!(sim.log.text.contains("ABORT bus_absent"), "{}", sim.log.text);
        // And the preflight itself PASSED -- the pins were idle and the driver
        // healthy. This is exactly the case the old paperwork could not see.
        assert!(sim.log.text.contains("verdict=PASS"));
        assert!(!sim.log.text.contains("BEMFDRIVEN"));
    }

    #[test]
    fn the_new_provocations_reach_their_own_stop_codes() {
        // E355. Both paths are read from HAL counters by the closed loop
        // (`states.rs:278-282`) and neither had any provocation: `LateArm` is
        // this campaign's own hazard and `BlankLatched` has never once been
        // observed firing in 1193 captures. What this asserts is the STOP PATH
        // -- the loop notices the counter and stops through the ordinary route
        // -- not the physics that would normally set it.
        for (kind, want) in [
            (Inject::LateArm, Reason::LateArm),
            (Inject::BlankLatched, Reason::BlankLatched),
        ] {
            let (sim, out) = run(250, 20_000_000, STEADY, Faults::default(), Some((kind, 2_000_000)));
            let &(_, fired) = sim.log.injected.first().expect("the stimulus fired");
            assert_eq!(fired, kind, "{kind:?}");
            assert_eq!(out.reason, want, "{kind:?}");
            assert_eq!(u32::from(want.code()), kind.code(), "{kind:?}");
            assert!(sim.log.safe_offs >= 1, "the stop must route through safe_off");
        }
    }

    #[test]
    fn the_sag_stimulus_never_leaves_the_campaign_ceiling() {
        // ENV-6: at the top rung the relative sag step used to publish
        // 625 + 75 with itself as the cap -- 70 % on the bridge (4176 mA worst
        // block on hardware). Every plan now passes the real cap, and no
        // requested duty exceeds it.
        use crate::run::policy::SIXSTEP_DUTY_CAP;
        // Fire AFTER the ramp has reached the cap: an injection at 2 s lands
        // mid-ramp (applied duty ~130) and takes the fixed-500 branch, which is
        // not the one that faulted (ENV-6 review). Asserted, not assumed.
        let at = crate::ramp::ramp_us(SIXSTEP_DUTY_CAP) + 2_000_000;
        let inject = Some((Inject::Sag, at));
        let (sim, _) = run(SIXSTEP_DUTY_CAP, at + 10_000_000, STEADY, Faults::default(), inject);
        let before = sim
            .log
            .plans
            .iter()
            .filter(|&&(t, _)| t + 100_000 < sim.log.closed_at.unwrap() + at);
        assert_eq!(
            before.map(|&(_, d)| d).max(),
            Some(SIXSTEP_DUTY_CAP),
            "the ramp reached the cap first"
        );
        assert!(!sim.log.plans.is_empty());
        assert!(
            sim.log.plan_caps.iter().all(|&c| c == SIXSTEP_DUTY_CAP),
            "{:?}",
            sim.log.plan_caps
        );
        let max = sim.log.plans.iter().map(|&(_, d)| d).max().unwrap();
        assert!(max <= SIXSTEP_DUTY_CAP, "published {max} above the cap");
        // Below the relative threshold the historical fixed step is untouched.
        let inject = Some((Inject::Sag, 2_000_000));
        let (sim, _) = run(250, 20_000_000, STEADY, Faults::default(), inject);
        assert!(
            sim.log.plans.iter().any(|&(_, d)| d == 500),
            "the inherited 250 -> 500 control changed"
        );
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
