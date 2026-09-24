//! The run as a typestate: `Idle -> Armed -> Startup -> Handover -> Locked ->
//! Stopped(Reason)`. Every transition consumes the state it leaves; the only
//! way into [`Stopped`] is [`stop`], which calls [`Hal::safe_off`]; and the
//! gate capability each state holds is typed so that only `Armed`, `Startup`
//! and `Locked` can write the gates (see [`super::hal`]).
//!
//! The pass structure is `bemf_run`'s as of E118, statement for statement:
//! the common checks ([`Ctx::pass`]) and then the state's own step.

use crate::commutation::{self, Direction, Phase, Step};
use crate::driven;
use crate::duty::{RUN_PERIOD_TICKS, STARTUP_TICKS};
use crate::protection::{
    validate_raw_feedback, AverageCurrent, BlockVerdict, CurrentMark, FastBusSag, FoldbackGovernor, PhaseCodePolicy,
    RailMean, Reason, RAW_LIMIT,
};
use crate::ramp::duty_at;
use crate::seed::{Edge, Qualification, Seed};
use crate::sine;
use crate::sixstep;
use crate::startup::{Script, StaircaseScript};
use crate::witness::RotationWitness;

use super::hal::{self, Gates, Hal, Inject};
use super::measure::Baseline;
use super::policy::{
    in_off_window, sector_interval_us, sixstep_ccr_of, Advance, Bemf, CurrentLimit, SagLimit, CATCH_DUTY_TENTHS,
    CATCH_EHZ, DRIVEN_DUTY_TENTHS, DRIVEN_PHASE_DEG, DRIVEN_RATE, HANDOFF_DUTY_TENTHS, INJECT_SAG_DUTY_TENTHS,
    REVISIT_RESCUE_MAX, SIXSTEP_DUTY_CAP, TAIL_WINDOW_US, WITNESS_HYST_CODES, WITNESS_MID_SAMPLES,
};
use super::Policies;

/// How long a run may last: from its own entry, or to an absolute instant a
/// campaign supplies (E090), so a restarted segment ends exactly where the
/// original window does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    ForUs(u32),
    Until(u32),
}

/// What a run was asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub target_tenths: u16,
    /// A gate-4 provocation, and how long after the loop closes it fires.
    pub inject: Option<(Inject, u32)>,
    pub window: Window,
}

/// Counters the report is built from.
pub(crate) struct Stats {
    pub accepted: u32,
    pub acc_by_step: [u16; 8],
    pub acc_by_phase: [u16; 8],
    pub revisit_attempts: [u16; 8],
    pub revisit_accepts: [u16; 8],
    pub loop_iters_closed: u32,
    pub loop_gap_max_us: u32,
    pub loop_prev_raw: Option<u16>,
    pub vsenc: (u16, u16),
    pub star: (u16, u16),
    pub wit: RotationWitness,
    pub hold_acc: u32,
    pub hold_ci_sum: u32,
    /// The matched speed window (campaign 8 step 3): the last accepted
    /// crossing's stamp with the hold-accept count as of it, plus two marks
    /// laid down one `TAIL_WINDOW_US` apart. The window reported is
    /// `tail_prev .. tail_last`, so it is between one and two windows long
    /// and never collapses to nothing the way a single resetting anchor does.
    /// Every stamp is on the same extended-µs clock as the stop.
    pub tail_last: Option<(u32, u32)>,
    pub tail_mark: Option<(u32, u32)>,
    pub tail_prev: Option<(u32, u32)>,
    pub bus_min: u16,
    pub drive_scans: u32,
}

/// The driven observation's record.
pub(crate) struct DrivenLog {
    pub at: Option<u32>,
    pub epoch: u32,
    pub accepts: u32,
    pub retries: u32,
    pub late_max_us: u32,
    /// 1 no boundary, 2 sequence mismatch, 3 qualification fault, 4 window
    /// elapsed, 5 seed/step mismatch.
    pub fail: u32,
    pub qual: Qualification,
    pub rows: [(u16, u8, u16, u16); 64],
    pub rows_n: usize,
    pub seed: Option<Seed>,
}

/// Everything the stages share, carried through every state.
pub(crate) struct Ctx {
    pub req: Request,
    pub entry: u32,
    /// The window, µs, resolved against `entry`.
    pub window_us: u32,
    pub start: u32,
    pub injected_at: Option<u32>,
    pub hold_plans: bool,
    pub base: Baseline,
    pub sag: FastBusSag,
    pub current: AverageCurrent,
    pub governor: FoldbackGovernor,
    pub rail: RailMean,
    pub period: u32,
    pub step: Step,
    pub applied_duty: u16,
    pub closed_at: Option<u32>,
    pub hold_start: Option<u32>,
    pub hold_current: Option<CurrentMark>,
    pub last_ci: u32,
    pub stats: Stats,
    pub drv: DrivenLog,
}

impl Ctx {
    fn new<P: Policies>(req: Request, entry: u32, base: Baseline) -> Self {
        Self {
            req,
            entry,
            window_us: match req.window {
                Window::ForUs(us) => us,
                Window::Until(t) => t.wrapping_sub(entry),
            },
            start: entry,
            injected_at: None,
            hold_plans: false,
            base,
            sag: P::S::watch(base.bus_ref),
            current: P::C::meter(base.zero_block),
            governor: P::C::governor(req.target_tenths),
            rail: RailMean::new(),
            period: STARTUP_TICKS,
            step: Step::new_clamped(1),
            applied_duty: HANDOFF_DUTY_TENTHS,
            closed_at: None,
            hold_start: None,
            hold_current: None,
            last_ci: sector_interval_us(CATCH_EHZ),
            stats: Stats {
                accepted: 0,
                acc_by_step: [0; 8],
                acc_by_phase: [0; 8],
                revisit_attempts: [0; 8],
                revisit_accepts: [0; 8],
                loop_iters_closed: 0,
                loop_gap_max_us: 0,
                loop_prev_raw: None,
                vsenc: (u16::MAX, 0),
                star: (u16::MAX, 0),
                wit: RotationWitness::new(WITNESS_MID_SAMPLES, WITNESS_HYST_CODES),
                hold_acc: 0,
                tail_last: None,
                tail_mark: None,
                tail_prev: None,
                hold_ci_sum: 0,
                bus_min: u16::MAX,
                drive_scans: 0,
            },
            drv: DrivenLog {
                at: None,
                epoch: 0,
                accepts: 0,
                retries: 0,
                late_max_us: 0,
                fail: 0,
                qual: Qualification::new(entry),
                rows: [(0, 0, 0, 0); 64],
                rows_n: 0,
                seed: None,
            },
        }
    }

    /// The physical plan for a logical step at `duty` on the current period.
    fn plan<P: Policies>(&self, step: Step, duty: u16) -> Option<sixstep::Plan> {
        sixstep::plan(P::W::step(step), duty, self.period, SIXSTEP_DUTY_CAP)
    }

    /// The checks every pass makes, in `bemf_run`'s order: clock, link,
    /// driver, the closed loop's health, the injection, the roots' latches,
    /// the host, feedback age, the window, and one scan's protections.
    /// Returns the pass's timestamp, or why the run stops.
    fn pass<P: Policies>(&mut self, hal: &mut impl Hal, closed: bool) -> Result<u32, Reason> {
        let now = hal.now();
        hal.drain();
        if !hal.nfault_high() {
            return Err(Reason::Driver);
        }
        if closed {
            self.step = hal.com_step();
            let raw_now = hal.raw();
            if let Some(p) = self.stats.loop_prev_raw {
                self.stats.loop_gap_max_us = self.stats.loop_gap_max_us.max(u32::from(raw_now.wrapping_sub(p)));
            }
            self.stats.loop_prev_raw = Some(raw_now);
            self.stats.loop_iters_closed = self.stats.loop_iters_closed.wrapping_add(1);
        }
        self.maybe_inject(hal, closed, now);
        let guard_code = hal.guard_reason();
        if guard_code != 0 {
            return Err(reason_from_code(guard_code));
        }
        if hal.storm() {
            return Err(Reason::CompStorm);
        }
        if hal.overrun() {
            return Err(Reason::HandlerOverrun);
        }
        // Campaign 8's two hard stops, armed on the closed loop only (see
        // `Reason::LateArm`): an exhausted commutation deadline, and a
        // comparator edge the blanking window latched. Both counters only
        // rise, so one reading is enough, and both take the ordinary
        // protection route -- `safe_off`, then the report carrying the code.
        if closed && hal.late_arms() != 0 {
            return Err(Reason::LateArm);
        }
        if closed && hal.blank_latched() != 0 {
            return Err(Reason::BlankLatched);
        }
        if closed
            && !hal.cap_armed()
            && self
                .closed_at
                .is_some_and(|t| crate::rate::cap_enforced(now.wrapping_sub(t)))
        {
            hal.arm_cap();
        }
        if hal.rx().is_some_and(super::parser_stop) {
            return Err(Reason::HostAbort);
        }
        if hal.adc_stale(now) {
            return Err(Reason::AdcTimeout);
        }
        if now.wrapping_sub(self.entry) >= self.window_us {
            return Err(Reason::SegmentDeadline);
        }
        if hal.adc_due() {
            self.scan_pass::<P>(hal, closed)?;
        }
        Ok(now)
    }

    /// One fresh scan: the during-run witness, then every scan protection.
    fn scan_pass<P: Policies>(&mut self, hal: &mut impl Hal, closed: bool) -> Result<(), Reason> {
        if closed {
            let (vc, sr) = hal.comp_inputs();
            let s = &mut self.stats;
            s.vsenc = (s.vsenc.0.min(vc), s.vsenc.1.max(vc));
            s.star = (s.star.0.min(sr), s.star.1.max(sr));
            if commutation::sector(P::W::step(self.step)).floating == Phase::C
                && in_off_window(
                    hal.pwm_counter(),
                    sixstep_ccr_of(self.applied_duty, self.period),
                    self.period,
                    STARTUP_TICKS,
                )
            {
                s.wit.sample(i32::from(vc) - i32::from(sr));
            }
        }
        let scan = hal.scan().ok_or(Reason::AdcTimeout)?;
        self.stats.drive_scans += 1;
        self.stats.bus_min = self.stats.bus_min.min(scan.bus);
        self.rail.feed(scan.bus, scan.vref);
        if let Some(r) = validate_raw_feedback(&scan, PhaseCodePolicy::RetainRails) {
            return Err(r);
        }
        if scan.bus < self.base.bus_floor_code {
            return Err(Reason::Bus);
        }
        if self.rail.ready() {
            if let Some(r) = self.sag.observe(self.rail.bus_mean(), self.rail.vref_mean()) {
                return Err(r);
            }
        }
        match self.current.accumulate(scan.phase_a, scan.phase_b, scan.phase_c) {
            Some(BlockVerdict::Stop(r)) => Err(r),
            Some(BlockVerdict::Foldback(red)) => {
                let _ = self.governor.warn(red);
                Ok(())
            }
            Some(BlockVerdict::Ok) | None => Ok(()),
        }
    }

    /// Gate 4 (E080): fire the planned stimulus once, `after` into the loop.
    fn maybe_inject(&mut self, hal: &mut impl Hal, closed: bool, now: u32) {
        let Some((kind, after)) = self.req.inject else {
            return;
        };
        if !closed || self.injected_at.is_some() || self.closed_at.is_none_or(|t| now.wrapping_sub(t) < after) {
            return;
        }
        self.injected_at = Some(now);
        match kind {
            Inject::Sag => {
                hal.publish_plans(INJECT_SAG_DUTY_TENTHS, self.period, INJECT_SAG_DUTY_TENTHS);
                self.hold_plans = true;
            }
            Inject::AverageCurrent => {
                // A *stricter* allowance with the plans held, so the first
                // foldback goes unacknowledged (reference
                // `unacknowledged_second_over_stop=1`).
                self.current = AverageCurrent::new(self.base.zero_block, RAW_LIMIT / 100);
                self.hold_plans = true;
            }
            other => hal.inject(other),
        }
    }
}

/// `Reason` back from the guard's latched wire code.
#[must_use]
pub const fn reason_from_code(code: u32) -> Reason {
    match code {
        1 => Reason::CampaignDeadline,
        3 => Reason::TickGap,
        4 => Reason::FeedbackStale,
        7 => Reason::Driver,
        8 => Reason::Tracking,
        _ => Reason::SegmentDeadline,
    }
}

// ---------------------------------------------------------------------------
// Idle and Armed
// ---------------------------------------------------------------------------

/// Nothing energized. Holds no gate capability that can write.
pub struct Idle {
    req: Request,
    gates: Gates<hal::Idle>,
}

/// Preflight passed, driver awake, baseline taken; the gates are ours.
pub struct Armed {
    ctx: Ctx,
    gates: Gates<hal::Armed>,
}

impl Idle {
    pub(crate) fn new(req: Request) -> Self {
        Self {
            req,
            gates: Gates::idle(),
        }
    }

    /// Preflight (printed on `io`), hand the pins to TIM1, wake the driver,
    /// check nFAULT, and take the baseline and zero -- or stop, bridge off.
    pub fn arm<P: Policies, IO: Hal + crate::report::Sink>(self, io: &mut IO) -> Result<Armed, Refused> {
        let entry = io.now();
        let p = io.preflight();
        super::measure::say_preflight(&p, io);
        let mut gates = match self.gates.after_preflight(&p) {
            Ok(g) => g,
            Err(g) => return Err(Refused::new(io, g, Reason::Driver, entry)),
        };
        io.gates_to_timer(&mut gates);
        io.resync_adc();
        io.enable(true);
        io.led(true);
        let wake = io.now();
        while io.now().wrapping_sub(wake) < 2_000 {
            io.drain();
        }
        if !io.nfault_high() {
            return Err(Refused::new(io, gates, Reason::Driver, entry));
        }
        let Some(mut base) = super::measure::capture_baseline(io) else {
            return Err(Refused::new(io, gates, Reason::AdcTimeout, entry));
        };
        let Some(zero) = super::measure::averaged_zero(io, base.zero_block) else {
            return Err(Refused::new(io, gates, Reason::AdcTimeout, entry));
        };
        base.zero_block = zero;
        Ok(Armed {
            ctx: Ctx::new::<P>(self.req, entry, base),
            gates,
        })
    }
}

impl Armed {
    /// Set the startup carrier, open all three phases at zero, hand the run to
    /// the guard root and enable MOE.
    pub fn start<P: Policies>(mut self, hal: &mut impl Hal) -> Startup {
        let c = &mut self.ctx;
        hal.set_period(c.period);
        hal.all_phases_pwm(&mut self.gates);
        hal.set_compares(&mut self.gates, sine::compares(c.period - 1, 0, 0));
        hal.comp_mask();
        hal.drv_end();
        hal.guard_arm();
        hal.moe_on(&mut self.gates);
        c.start = hal.now();
        c.drv.qual = Qualification::new(c.start);
        hal.reset_roots();
        Startup {
            sine: SineStage {
                script: StaircaseScript::new(CATCH_DUTY_TENTHS, CATCH_DUTY_TENTHS),
                last: c.start,
                tick: 0,
                theta: 0,
            },
            driven: None,
            ctx: self.ctx,
            gates: self.gates.pass(),
        }
    }
}

// ---------------------------------------------------------------------------
// Startup: the sine, then the driven observation
// ---------------------------------------------------------------------------

struct SineStage {
    script: StaircaseScript,
    last: u32,
    tick: u32,
    theta: u32,
}

struct DrivenStage {
    theta0: u32,
    t0: u32,
    next_rel: u32,
}

/// The reference's startup chain (E058): a sine drags the rotor to the
/// handover speed, then a driven six-step observes real crossings.
pub struct Startup {
    ctx: Ctx,
    gates: Gates<hal::Startup>,
    sine: SineStage,
    driven: Option<DrivenStage>,
}

/// What one startup pass decided. A pass works on the state in place; only
/// a transition consumes it ([`Startup::stop`], [`Startup::handover`]) -- the
/// state is ~1 KB, and moving it through every pass slowed the loop 3.6%
/// (E120).
pub enum StartupNext {
    Continue,
    /// A qualified seed: its edge, the raw stamp, and the pass's time.
    Seeded(Seed, u16, u32),
    Stop(Reason),
}

impl Startup {
    /// One pass, in place.
    pub fn poll<P: Policies>(&mut self, hal: &mut impl Hal) -> StartupNext {
        let now = match self.ctx.pass::<P>(hal, false) {
            Ok(t) => t,
            Err(r) => return StartupNext::Stop(r),
        };
        let flow = if self.driven.is_none() {
            self.sine_pass::<P>(hal, now)
        } else {
            self.driven_pass::<P>(hal, now)
        };
        match flow {
            Ok(None) => StartupNext::Continue,
            Ok(Some((seed, raw))) => StartupNext::Seeded(seed, raw, now),
            Err(r) => StartupNext::Stop(r),
        }
    }

    /// The seed qualified: on to the handover (no gate writes there).
    #[must_use]
    pub fn handover(self, seed: Seed, raw: u16, now: u32) -> Handover {
        Handover {
            ctx: self.ctx,
            gates: self.gates.pass(),
            seed,
            raw,
            now,
        }
    }

    /// Stop, through `safe_off`.
    pub fn stop(self, hal: &mut impl Hal, reason: Reason) -> Stopped {
        stop(hal, self.ctx, self.gates, reason)
    }

    /// The sine on its 1 kHz schedule, then entry to the driven stage.
    fn sine_pass<P: Policies>(&mut self, hal: &mut impl Hal, now: u32) -> Result<Option<(Seed, u16)>, Reason> {
        let s = &mut self.sine;
        if now.wrapping_sub(s.last) >= 1_000 {
            s.last = s.last.wrapping_add(1_000);
            if let Some(sp) = s.script.at(s.tick) {
                s.theta = s.theta.wrapping_add(sine::theta_increment(sp.freq_chz));
                let duty = self.ctx.governor.clamp(sp.duty_tenths);
                hal.set_compares(
                    &mut self.gates,
                    sine::compares(self.ctx.period - 1, u32::from(duty), s.theta),
                );
            }
            s.tick += 1;
        }
        if Script::handoff_due(now.wrapping_sub(self.ctx.start)) {
            self.enter_driven::<P>(hal)?;
        }
        Ok(None)
    }

    /// Enter the driven stage on the sine's own phase, advanced by the
    /// reference's `drivephase60`, turning at the sine's rate.
    fn enter_driven<P: Policies>(&mut self, hal: &mut impl Hal) -> Result<(), Reason> {
        let t_now = hal.now();
        let turned = DRIVEN_RATE.wrapping_mul(t_now.wrapping_sub(self.sine.last));
        let mut theta_d = driven::phase_shift(self.sine.theta.wrapping_add(turned), DRIVEN_PHASE_DEG);
        let mut bnd = self.boundary(driven::next(theta_d, DRIVEN_RATE))?;
        let mut t0 = t_now;
        let w = bnd.initial_wait();
        if w != 0 {
            // A sliver of a first sector is waited out floating, never driven.
            hal.float_all();
            while hal.now().wrapping_sub(t_now) < u32::from(w) {
                hal.drain();
            }
            t0 = hal.now();
            theta_d = theta_d.wrapping_add(DRIVEN_RATE.wrapping_mul(t0.wrapping_sub(t_now)));
            bnd = self.boundary(driven::next(theta_d, DRIVEN_RATE))?;
        }
        let c = &mut self.ctx;
        self.driven = Some(DrivenStage {
            theta0: theta_d,
            t0,
            next_rel: u32::from(bnd.delay_us),
        });
        c.step = bnd.step;
        if let Some(pl) = c.plan::<P>(c.step, DRIVEN_DUTY_TENTHS) {
            hal.apply_plan(&mut self.gates, &pl);
            c.applied_duty = DRIVEN_DUTY_TENTHS;
        }
        c.drv.epoch = 0;
        c.drv.qual = Qualification::new(t0);
        c.drv.at = Some(t0);
        hal.drv_begin(c.step);
        Ok(())
    }

    fn boundary(&mut self, b: Option<driven::Boundary>) -> Result<driven::Boundary, Reason> {
        b.ok_or_else(|| {
            self.ctx.drv.fail = 1;
            Reason::InvalidSeed
        })
    }

    /// The driven stage: consume an acceptance (maybe the seed), resume a
    /// deferred level, check the qualification, and commutate on schedule.
    fn driven_pass<P: Policies>(&mut self, hal: &mut impl Hal, now: u32) -> Result<Option<(Seed, u16)>, Reason> {
        if let Some(a) = hal.drv_poll() {
            let c = &mut self.ctx;
            c.drv.accepts += 1;
            let e = Edge {
                epoch: a.epoch,
                step: a.step,
                at_us: hal.stamp_from_raw(a.raw),
                interval_us: a.interval_us,
            };
            if c.drv.rows_n < c.drv.rows.len() {
                c.drv.rows[c.drv.rows_n & 63] = (e.epoch, e.step.get(), e.interval_us as u16, a.position_us);
                c.drv.rows_n += 1;
            }
            if let Some(sd) = c.drv.qual.accept(e) {
                if sd.step != c.step {
                    c.drv.fail = 5;
                    return Err(Reason::InvalidSeed);
                }
                return Ok(Some((sd, a.raw)));
            }
        }
        if hal.drv_resume_deferred(self.ctx.step) {
            self.ctx.drv.retries += 1;
        }
        if self.ctx.drv.qual.fault().is_some() {
            self.ctx.drv.fail = 3;
            return Err(Reason::InvalidSeed);
        }
        if self.ctx.drv.qual.expired(now) {
            self.ctx.drv.fail = 4;
            return Err(Reason::InvalidSeed);
        }
        self.driven_boundary::<P>(hal, now)?;
        Ok(None)
    }

    /// The next driven boundary, recomputed from the stage's origin phase every
    /// time so rounding never accumulates.
    fn driven_boundary<P: Policies>(&mut self, hal: &mut impl Hal, now: u32) -> Result<(), Reason> {
        let Some(d) = self.driven.as_ref() else {
            return Ok(());
        };
        let (theta0, t0, next_rel) = (d.theta0, d.t0, d.next_rel);
        let deadline = t0.wrapping_add(next_rel);
        if now.wrapping_sub(deadline) >= u32::MAX / 2 {
            return Ok(());
        }
        self.ctx.drv.late_max_us = self.ctx.drv.late_max_us.max(now.wrapping_sub(deadline));
        let bnd = self.boundary(driven::next(
            theta0.wrapping_add(DRIVEN_RATE.wrapping_mul(next_rel)),
            DRIVEN_RATE,
        ))?;
        let c = &mut self.ctx;
        if bnd.step != c.step.next() {
            c.drv.fail = 2;
            return Err(Reason::InvalidSeed);
        }
        c.step = bnd.step;
        c.drv.epoch += 1;
        hal.drv_advance(c.step, c.drv.epoch);
        if let Some(pl) = c.plan::<P>(c.step, DRIVEN_DUTY_TENTHS) {
            hal.apply_plan(&mut self.gates, &pl);
        }
        hal.comp_select(c.step);
        hal.comp_arm(c.step);
        if let Some(d) = self.driven.as_mut() {
            d.next_rel = d.next_rel.wrapping_add(u32::from(bnd.delay_us));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Handover and Locked
// ---------------------------------------------------------------------------

/// A qualified seed, between the driven stage and the closed loop. Its gate
/// capability cannot write: nothing is driven here.
pub struct Handover {
    ctx: Ctx,
    gates: Gates<hal::Handover>,
    seed: Seed,
    raw: u16,
    now: u32,
}

/// The closed loop: COMP decides, the COM root commutates, the foreground
/// consumes, retries, and follows the duty. Holds no serial handle.
pub struct Locked {
    ctx: Ctx,
    gates: Gates<hal::Locked>,
    sector_start: u32,
    revisit_step: u8,
    revisit_inflight: bool,
    /// Rescue attempts spent in this sector (E140), reset by an accept.
    rescues: u8,
    last_com_count: u32,
}

impl Handover {
    /// Take the driven stage back, install the estimator from the seed and
    /// hand the loop to COMP; then, as `Locked`, apply the transfer duty and
    /// hand commutation to the COM root, first commutation `wait` after the
    /// seed edge.
    pub fn lock<P: Policies>(self, hal: &mut impl Hal) -> Locked {
        let Handover {
            mut ctx,
            gates,
            seed: sd,
            raw,
            now,
        } = self;
        hal.drv_end();
        let adv = P::A::level(duty_at(ctx.req.target_tenths, 0));
        hal.det_install(P::B::estimator(sd.interval_us), sd.interval_us, raw, ctx.step, adv);
        ctx.last_ci = sd.interval_us;
        let commit_us = sd
            .edge_us
            .wrapping_add(commutation::wait_time(sd.interval_us, adv).max(1));
        ctx.closed_at = Some(now);
        ctx.drv.seed = Some(sd);
        ctx.period = RUN_PERIOD_TICKS;
        hal.set_period(ctx.period);
        let mut gates: Gates<hal::Locked> = gates.pass();
        let bemf_duty = ctx.governor.clamp(duty_at(ctx.req.target_tenths, 0));
        if let Some(pl) = ctx.plan::<P>(ctx.step, bemf_duty) {
            hal.apply_plan(&mut gates, &pl);
            ctx.applied_duty = bemf_duty;
        }
        hal.com_handover(bemf_duty, ctx.period, ctx.step, commit_us);
        Locked {
            ctx,
            gates,
            sector_start: sd.edge_us,
            revisit_step: 0,
            revisit_inflight: false,
            rescues: 0,
            last_com_count: 0,
        }
    }
}

impl Locked {
    /// Stop, through `safe_off`.
    pub fn stop(self, hal: &mut impl Hal, reason: Reason) -> Stopped {
        stop(hal, self.ctx, self.gates, reason)
    }

    /// One pass of the closed loop, in place; `Some` is why it must stop.
    pub fn poll<P: Policies>(&mut self, hal: &mut impl Hal) -> Option<Reason> {
        let now = match self.ctx.pass::<P>(hal, true) {
            Ok(t) => t,
            Err(r) => return Some(r),
        };
        let c = &mut self.ctx;
        let duty = c.governor.clamp(duty_at(
            c.req.target_tenths,
            c.closed_at.map_or(0, |t| now.wrapping_sub(t)),
        ));
        // The first instant the *applied* duty reaches target is the hold.
        if c.hold_start.is_none() && duty >= c.req.target_tenths {
            c.hold_start = Some(now);
            c.hold_current = Some(c.current.mark());
        }
        if let Some(raw) = hal.det_poll() {
            self.consume(hal, raw);
        }
        self.revisit(hal);
        let cc = hal.com_count();
        if cc != self.last_com_count {
            self.last_com_count = cc;
            // A retry not accepted before this commutation failed.
            self.revisit_inflight = false;
        }
        let c = &mut self.ctx;
        if duty != c.applied_duty && !c.hold_plans {
            hal.publish_plans(duty, c.period, SIXSTEP_DUTY_CAP);
            c.applied_duty = duty;
            hal.set_advance(P::A::level(duty));
        }
        None
    }

    /// An accepted crossing: statistics, and the estimate as of it.
    fn consume(&mut self, hal: &mut impl Hal, raw: u16) {
        let edge_us = hal.stamp_from_raw(raw);
        let count = edge_us.wrapping_sub(self.sector_start);
        let c = &mut self.ctx;
        let ci_before = c.last_ci;
        self.sector_start = edge_us;
        c.stats.accepted += 1;
        if c.hold_start.is_some() {
            c.stats.hold_acc += 1;
            c.stats.hold_ci_sum = c.stats.hold_ci_sum.saturating_add(count);
            // Roll the matched window's anchor forward once it is older than
            // `TAIL_WINDOW_US`, so the window always ends at the newest
            // crossing and is one to two windows long (step 3).
            let here = (edge_us, c.stats.hold_acc);
            match c.stats.tail_mark {
                Some((t, _)) if edge_us.wrapping_sub(t) < TAIL_WINDOW_US => {}
                _ => {
                    c.stats.tail_prev = c.stats.tail_mark;
                    c.stats.tail_mark = Some(here);
                }
            }
            c.stats.tail_last = Some(here);
        }
        let bin = (c.step.get() as usize - 1) & 7;
        c.stats.acc_by_step[bin] = c.stats.acc_by_step[bin].saturating_add(1);
        if self.revisit_inflight {
            c.stats.revisit_accepts[bin] = c.stats.revisit_accepts[bin].saturating_add(1);
        }
        self.revisit_inflight = false;
        self.revisit_step = 0;
        self.rescues = 0;
        // Shift-only thresholds, no division on a motor path.
        let half = ci_before >> 1;
        let t = if count <= half + (ci_before >> 2) {
            0
        } else if count <= ci_before {
            1
        } else if count <= ci_before + (ci_before >> 2) {
            2
        } else if count <= ci_before + half {
            3
        } else {
            4
        };
        c.stats.acc_by_phase[t] = c.stats.acc_by_phase[t].saturating_add(1);
        c.last_ci = hal.det_average().unwrap_or(c.last_ci);
    }

    /// The level revisit: retry a crossing the half-cycle gate refused and the
    /// comparator now simply holds. Once per sector, plus up to
    /// `REVISIT_RESCUE_MAX` rescues when the sector is overdue (E140).
    ///
    /// The rescue is what E138 asked for. Without it the single attempt is
    /// spent on the first pass of the sector and `revisit_step` blocks every
    /// later one until an accepted crossing clears it -- so the one sector
    /// that most needs another look, the one whose crossing was swallowed,
    /// never gets one. Each rescue needs another half-interval of overdue, so
    /// the count is bounded and the poll cannot hammer.
    fn revisit(&mut self, hal: &mut impl Hal) {
        let step = self.ctx.step;
        let fresh = self.revisit_step != step.get();
        let overdue = !fresh && self.rescue_due(hal);
        if hal.com_idle() && (fresh || overdue) && hal.revisit(step) {
            self.revisit_step = step.get();
            self.revisit_inflight = true;
            if overdue {
                self.rescues = self.rescues.saturating_add(1);
            }
            let bin = (step.get() as usize - 1) & 7;
            self.ctx.stats.revisit_attempts[bin] = self.ctx.stats.revisit_attempts[bin].saturating_add(1);
        }
    }

    /// Is this sector overdue by another half-interval, with a rescue left?
    fn rescue_due(&mut self, hal: &mut impl Hal) -> bool {
        if self.rescues >= REVISIT_RESCUE_MAX {
            return false;
        }
        let ci = self.ctx.last_ci;
        if ci == 0 {
            return false;
        }
        let elapsed = hal.now().wrapping_sub(self.sector_start);
        // The first rescue at 1.5 intervals, then one per further half.
        elapsed > ci + (ci >> 1) + (ci >> 1) * u32::from(self.rescues)
    }
}

// ---------------------------------------------------------------------------
// Stopped
// ---------------------------------------------------------------------------

/// The bridge is off. Built only by [`stop`] and from a [`Refused`], both of
/// which consumed the gate capability through [`Hal::safe_off`].
pub struct Stopped {
    pub reason: Reason,
    pub entry: u32,
    pub stopped_at: u32,
    /// `None` when the run never drove (refused before `Armed::start`).
    pub(crate) ctx: Option<Ctx>,
    _gates: Gates<hal::Stopped>,
}

/// A run refused before it drove (preflight, nFAULT, baseline): the bridge
/// is off -- the capability went through `safe_off` -- and there is no report.
pub struct Refused {
    pub reason: Reason,
    pub entry: u32,
    gates: Gates<hal::Stopped>,
}

impl Refused {
    fn new<S>(hal: &mut impl Hal, gates: Gates<S>, reason: Reason, entry: u32) -> Self {
        Self {
            reason,
            entry,
            gates: hal.safe_off(gates),
        }
    }
}

impl From<Refused> for Stopped {
    fn from(r: Refused) -> Self {
        Self {
            reason: r.reason,
            entry: r.entry,
            stopped_at: r.entry,
            ctx: None,
            _gates: r.gates,
        }
    }
}

/// The one route into [`Stopped`] for a run that drove: take the estimator
/// back, stamp the stop, take everything else back, `safe_off`, mask the
/// comparator, restore the startup carrier.
pub(crate) fn stop<S>(hal: &mut impl Hal, ctx: Ctx, gates: Gates<S>, reason: Reason) -> Stopped {
    hal.det_release();
    let stopped_at = hal.now();
    hal.take_back();
    let g = hal.safe_off(gates);
    hal.comp_mask();
    hal.set_period(STARTUP_TICKS);
    hal.led(false);
    Stopped {
        reason,
        entry: ctx.entry,
        stopped_at,
        ctx: Some(ctx),
        _gates: g,
    }
}
