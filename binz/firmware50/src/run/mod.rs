//! The production controller: `Controller<Wiring, BemfPolicy, AdvancePolicy,
//! CurrentProtection, BusSagProtection, Restart, Telemetry>`, running the
//! typestate run in [`states`] against any [`Hal`].
//!
//! `main` constructs [`Production`] and hands it the board; everything the
//! shell does -- the rungs, the restart campaign, the provocations, the
//! preflight -- is a method here. The binary keeps the hardware: `Board`'s
//! `Hal` impl, the interrupt shims and the panic handler.

use core::marker::PhantomData;

use crate::commutation::Direction;
use crate::protection::{RAW_LIMIT, Reason, ZERO_BLOCKS};
use crate::report::{CoastStats, CurrentRecord, InjectOutcome, RunReport, Sink, WitnessRecord};

pub mod accepted;
pub mod hal;
pub mod measure;
pub mod policy;
#[cfg(test)]
mod replay;
#[cfg(test)]
mod sim;
pub mod states;

pub use hal::{Gates, Hal, Inject, Preflight};
pub use states::{Refused, Request, StartupNext, Stopped, Window};

use policy::{Advance, BEMF_DUTY_TENTHS, BEMF_TOTAL_MS, Bemf, Carrier, CurrentLimit, Reporting, RestartRule, SagLimit};

/// The seven policy slots, as one bundle the states are generic over.
pub trait Policies {
    type W: Direction;
    type B: Bemf;
    type A: Advance;
    type C: CurrentLimit;
    type S: SagLimit;
    type R: RestartRule;
    type T: Reporting;
    /// Where the sharp-sag guard's own inputs go, scan by scan (campaign 9
    /// step 3). Production's `NoSagLog` folds every call away; only the
    /// `sag-capture` image installs the ring.
    type G: crate::sagtrace::SagLog;
    type H: Carrier;
}

/// The run's controller, one type parameter per policy decision.
pub struct Controller<W, B, A, C, S, R, T, G = crate::sagtrace::NoSagLog, H = policy::FixedCarrier> {
    /// Duty the lowercase provocations and `Z` run at, tenths (E141): 250 as
    /// through campaign 5, or 375 for the goal's item 5. Shell state only --
    /// no run reads it except when it starts, and every capture records the
    /// duty it actually ran.
    provoke_tenths: u16,
    /// The climb's duty, tenths (E147): 40% at boot, stepped by `+`/`-` in
    /// 2.5% increments between 37.5% and the clamp at 50%.
    climb_tenths: u16,
    // Two markers rather than one eight-tuple: clippy's `type_complexity`
    // counts through aliases, and a warning is not waived in this crate.
    _p: PhantomData<(W, B, A, C)>,
    _q: PhantomData<(S, R, T, G, H)>,
}

impl<
    W: Direction,
    B: Bemf,
    A: Advance,
    C: CurrentLimit,
    S: SagLimit,
    R: RestartRule,
    T: Reporting,
    G: crate::sagtrace::SagLog,
    H: Carrier,
> Policies for Controller<W, B, A, C, S, R, T, G, H>
{
    type W = W;
    type B = B;
    type A = A;
    type C = C;
    type S = S;
    type R = R;
    type T = T;
    type G = G;
    type H = H;
}

/// The one composition this firmware runs. The sag recorder slot defaults to
/// `NoSagLog`, so production records nothing.
pub type Production = Controller<
    policy::Wiring,
    policy::BemfPolicy,
    policy::AdvancePolicy,
    policy::CurrentProtection,
    policy::BusSagProtection,
    policy::Restart,
    policy::Telemetry,
    ProductionSagLog,
>;

/// The recorder `Production` carries. `NoSagLog` by default, so production
/// records nothing and every call folds away.
///
/// The `sag-ring` feature swaps in the real ring **without changing anything
/// else** (E304). That matters because the campaign's evidence gap is a
/// recorded 55% trip: all four rung-550 captures ran on production with
/// `NoSagLog` and carry `sagrows=0`, so `e253-550_03` -- the only un-injected
/// hard stop above rung 525, a `FastBusSag` latch -- has **no recorded
/// sequence at all**.
///
/// The existing `bin/sag-capture.rs` cannot close that gap: it has caught the
/// event **0 of 7** times where production caught it 3 of 6
/// ([[feedback-check-the-instrument-transfer-function]]), and it differs from
/// production by more than the recorder -- it runs its own serve loop instead
/// of `Production::serve`, and inits the fine clock. A diagnostic image that
/// never sees the event is not an instrument, and one that differs in two ways
/// cannot say which difference is responsible.
///
/// So this feature changes exactly one type parameter, leaving the loop,
/// policies, guards and thresholds identical to the image that does trip.
#[cfg(not(feature = "sag-ring"))]
pub type ProductionSagLog = crate::sagtrace::NoSagLog;
#[cfg(feature = "sag-ring")]
pub type ProductionSagLog = crate::sagtrace::SagRing;

/// True if this byte is one of the unconditional stop keys.
#[must_use]
pub const fn parser_stop(b: u8) -> bool {
    matches!(b, b'o' | b's' | b'!' | 0x03 | 0x1B)
}

/// A finished run: why it stopped, when, and what the coast saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub reason: Reason,
    pub entry: u32,
    pub stopped_at: u32,
    pub hold_ms: u32,
    pub coast: CoastStats,
}

impl<
    W: Direction,
    B: Bemf,
    A: Advance,
    C: CurrentLimit,
    S: SagLimit,
    R: RestartRule,
    T: Reporting,
    G: crate::sagtrace::SagLog,
    H: Carrier,
> Controller<W, B, A, C, S, R, T, G, H>
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            provoke_tenths: 250,
            climb_tenths: 400,
            _p: PhantomData,
            _q: PhantomData,
        }
    }

    /// Drive one run through the typestate and report it, after `safe_off`.
    pub fn run<IO: Hal + Sink>(&mut self, io: &mut IO, req: Request) -> Outcome {
        let stopped = match states::Idle::new(req).arm::<Self, IO>(io) {
            Ok(armed) => Self::drive(armed.start::<Self>(io), io),
            Err(refused) => refused.into(),
        };
        self.finish(io, stopped)
    }

    /// The driving states' loop. Generic over `Hal` alone: no text can be
    /// written from here.
    fn drive(mut startup: states::Startup, hal: &mut impl Hal) -> Stopped {
        let handover = loop {
            match startup.poll::<Self>(hal) {
                StartupNext::Continue => {}
                StartupNext::Seeded(seed, raw, now) => break startup.handover(seed, raw, now),
                StartupNext::Stop(r) => return startup.stop(hal, r),
            }
        };
        let mut locked = handover.lock::<Self>(hal);
        loop {
            if let Some(r) = locked.poll::<Self>(hal) {
                return locked.stop(hal, r);
            }
        }
    }

    /// Bridge off: coast, re-take the zero, undo a stimulus, verify all-off,
    /// then build the report and write it.
    fn finish<IO: Hal + Sink>(&mut self, io: &mut IO, stopped: Stopped) -> Outcome {
        let Stopped {
            reason,
            entry,
            stopped_at,
            ctx,
            ..
        } = stopped;
        let Some(ctx) = ctx else {
            return Outcome {
                reason,
                entry,
                stopped_at,
                hold_ms: 0,
                coast: CoastStats::empty(),
            };
        };
        // The coast first, before a single report byte (E065).
        let coast = measure::coast_capture(io, stopped_at);
        // Post-run zero (E079): gates reclaimed low with MOE clear, so waking
        // the driver cannot switch a FET; the amplifiers need ENABLE to bias.
        io.enable(true);
        let wake = io.now();
        while io.now().wrapping_sub(wake) < 2_000 {
            io.drain();
        }
        let zero_end = measure::capture_baseline(io).and_then(|bl| measure::averaged_zero(io, bl.zero_block));
        io.enable(false);
        if let Some((kind, _)) = ctx.req.inject {
            io.undo_inject(kind);
        }
        io.say("POSTSTOP\r\n");
        let p = io.preflight();
        measure::say_preflight(&p, io);
        let report = Self::report(io, &ctx, reason, stopped_at, zero_end);
        T::emit(&report, io);
        Outcome {
            reason,
            entry,
            stopped_at,
            hold_ms: report.hold_ms(),
            coast,
        }
    }

    /// The current proxy's half of the report.
    ///
    /// Lifted out of [`Self::report`] to bring that function back under the
    /// 100-line structure limit, which it had been over since E187 added the
    /// three fields below -- printed in the archived gate output and missed
    /// because `structure_report.py` returned only the RAM ceiling's verdict
    /// (E198). Both are fixed in this candidate.
    ///
    /// **Every number here is a 10.1 ms block quantity on an uncalibrated
    /// signed three-shunt residual**, not a metered current: the scale rests on
    /// one operator-metered point, and `zero_drift_ma` on the same line is the
    /// systematic that comes with it. `worst_*` is the worst block of the whole
    /// run, ramp included, and **not** of the hold -- a `CurrentMark`-windowed
    /// worst is still owed (E194).
    fn current_record(ctx: &states::Ctx, zero_end: Option<u32>) -> CurrentRecord {
        CurrentRecord {
            drive_scans: ctx.stats.drive_scans,
            applied_cap: crate::run::policy::SIXSTEP_DUTY_CAP,
            applied_ccr: crate::run::policy::sixstep_ccr_of(ctx.applied_duty, ctx.period),
            applied_period: ctx.period,
            depth_below: [
                ctx.depth.below(0),
                ctx.depth.below(1),
                ctx.depth.below(2),
                ctx.depth.below(3),
            ],
            depth_longest: [
                ctx.depth.longest(0),
                ctx.depth.longest(1),
                ctx.depth.longest(2),
                ctx.depth.longest(3),
            ],
            // The raw-scan observer (E284): the guard and `depth` above both
            // judge an 8-scan mean, so a dip shorter than that window is
            // invisible to both -- and every run, pass or fail, takes a raw
            // scan past the 5% line. `raw_depth_longest[RAW_DEPTH_TRIP_IX]` is
            // the longest consecutive run of raw scans below the guard's own
            // trip line, which is the quantity that discriminates.
            raw_depth_below: [
                ctx.raw_depth.below(0),
                ctx.raw_depth.below(1),
                ctx.raw_depth.below(2),
                ctx.raw_depth.below(3),
            ],
            raw_depth_longest: [
                ctx.raw_depth.longest(0),
                ctx.raw_depth.longest(1),
                ctx.raw_depth.longest(2),
                ctx.raw_depth.longest(3),
            ],
            blocks: ctx.current.blocks(),
            mean_residual: ctx.current.mean_residual(),
            mean_ma: ctx.current.mean_milliamps(),
            hold_blocks: ctx.hold_current.map_or(0, |m| ctx.current.window_blocks(m)),
            hold_ma: ctx.hold_current.map_or(0, |m| ctx.current.window_milliamps(m)),
            zero_blocks: ZERO_BLOCKS,
            current_allow: ctx.current.allow(),
            zero_start: ctx.base.zero_block,
            zero_end,
            ceiling_tenths: ctx.governor.ceiling(),
            worst_residual: ctx.current.worst_residual(),
            worst_ma: ctx.current.block_milliamps(ctx.current.worst_residual()),
            worst_hold_ma: ctx.current.block_milliamps(ctx.current.hold_worst_residual()),
            zero_drift_ma: zero_end.map_or(0, |z| {
                ((i64::from(z) - i64::from(ctx.base.zero_block)) * 4_000 / i64::from(RAW_LIMIT)) as i32
            }),
        }
    }

    /// Post-stop subtraction of foreground hold marks from IRQ totals.
    fn report_roots(io: &mut impl Hal, s: &states::Stats) -> crate::report::Roots {
        let mut r = io.roots_record();
        r.hold_unstable = r.unstable.saturating_sub(s.unstable_at_hold);
        for i in 0..8 {
            r.wait_hist_hold[i] = r.wait_hist[i].saturating_sub(s.wait_hist_at_hold[i]);
            r.left_hist_hold[i] = r.left_hist[i].saturating_sub(s.left_hist_at_hold[i]);
        }
        if !s.held {
            r.wait_hist_hold = [0; 8];
            r.left_hist_hold = [0; 8];
        }
        r
    }

    fn report(
        io: &mut impl Hal,
        ctx: &states::Ctx,
        reason: Reason,
        stopped_at: u32,
        zero_end: Option<u32>,
    ) -> RunReport {
        let s = &ctx.stats;
        let d = &ctx.drv;
        let (bemf_min, bemf_max) = s.wit.range();
        RunReport {
            reason: Some(reason),
            target_tenths: ctx.req.target_tenths,
            inject: ctx.req.inject.map(|(kind, _)| InjectOutcome {
                expected_reason: kind.code(),
                guard_reason: io.guard_reason(),
                fired_at: ctx.injected_at,
                stopped_at,
            }),
            accepted: s.accepted,
            coalesced_accepts: s.coalesced_accepts,
            forced: 0,
            ci_us: io.det_average().unwrap_or(ctx.last_ci),
            bus_ref: u32::from(ctx.base.bus_ref.bus),
            bus_min: u32::from(s.bus_min),
            sag_ref: (u32::from(ctx.sag.reference().bus), u32::from(ctx.sag.reference().vref)),
            sag_filt: (u32::from(ctx.sag.filtered().0), u32::from(ctx.sag.filtered().1)),
            sag_streak: u32::from(ctx.sag.streak()),
            sag_tripped: ctx.sag.tripped(),
            closed_us: ctx.closed_at.map_or(0, |t| stopped_at.wrapping_sub(t)),
            hold_us: ctx.hold_start.map_or(0, |t| stopped_at.wrapping_sub(t)),
            hold_acc: s.hold_acc,
            hold_forced: 0,
            hold_ci_sum: s.hold_ci_sum,
            tail: match (s.tail_prev.or(s.tail_mark), s.tail_last) {
                (Some((t0, a0)), Some((t1, a1))) if t1.wrapping_sub(t0) > 0 => Some(crate::report::TailWindow {
                    accepts: a1.wrapping_sub(a0),
                    span_us: t1.wrapping_sub(t0),
                    start_before_stop_us: stopped_at.wrapping_sub(t0),
                    end_before_stop_us: stopped_at.wrapping_sub(t1),
                }),
                _ => None,
            },
            roots: Self::report_roots(io, s),
            acc_by_step: core::array::from_fn(|i| s.acc_by_step[i]),
            forced_by_step: [0; 6],
            acc_by_phase: core::array::from_fn(|i| s.acc_by_phase[i]),
            revisit_attempts: core::array::from_fn(|i| s.revisit_attempts[i]),
            revisit_accepts: core::array::from_fn(|i| s.revisit_accepts[i]),
            driven: crate::report::Driven {
                entered: d.at.is_some(),
                epochs: d.epoch,
                accepts: d.accepts,
                retries: d.retries,
                late_max_us: d.late_max_us,
                qual_intervals: u32::from(d.qual.intervals()),
                qual_reanchors: u32::from(d.qual.reanchors()),
                qual_fault: d.qual.fault().map_or(0, fault_code),
                fail: d.fail,
                seed: d.seed.map(|sd| (sd.step.get(), sd.interval_us)),
                commanded_us: policy::sector_interval_us(policy::DRIVEN_EHZ),
                rows: d.rows,
                rows_n: d.rows_n,
            },
            guard: io.guard_record(
                s.loop_iters_closed,
                s.loop_gap_max_us,
                match (d.at, d.seed) {
                    (Some(t), Some(sd)) => sd.edge_us.wrapping_sub(t),
                    _ => 0,
                },
            ),
            current: Self::current_record(ctx, zero_end),
            witness: WitnessRecord {
                samples: s.wit.samples(),
                bemf_min,
                bemf_max,
                mid: s.wit.midpoint(),
                mid_fixed: s.wit.midpoint_fixed(),
                alt: s.wit.alternations(),
                hyst_codes: policy::WITNESS_HYST_CODES as u32,
                rotated: s.wit.rotated(),
                vsenc_min: u32::from(s.vsenc.0),
                vsenc_max: u32::from(s.vsenc.1),
                star_min: u32::from(s.star.0),
                star_max: u32::from(s.star.1),
            },
        }
    }

    /// A run at a rung, announced (`BEMFRUN`) and closed with its coast lines.
    /// `announce` is the `inject` code the header carries.
    pub fn rung<IO: Hal + Sink>(
        &mut self,
        io: &mut IO,
        target: u16,
        inject: Option<(Inject, u32)>,
        announce: u32,
        window_ms: u32,
        until: Option<u32>,
    ) -> Outcome {
        io.say("BEMFRUN ");
        io.kv("handoff_ehz", policy::HANDOFF_EHZ);
        io.kv("target_duty_tenths", u32::from(target));
        io.kv("advance_level", A::level(target));
        io.kv("total_ms", window_ms);
        io.kv("inject", announce);
        io.kv("run_period_ticks", H::period(target, H::ENTRY_TICKS));
        io.kv("entry_period_ticks", H::ENTRY_TICKS);
        io.kv("revisit_at_target", u32::from(B::allow_revisit(target)));
        io.kv("startup_ticks", crate::duty::STARTUP_TICKS);
        io.say("\r\n");
        io.flush_link();
        let window = until.map_or(Window::ForUs(window_ms.saturating_mul(1_000)), Window::Until);
        let out = self.run(
            io,
            Request {
                target_tenths: target,
                inject,
                window,
            },
        );
        out.coast.emit(out.reason, io);
        io.flush_link();
        out
    }

    /// Goal item 5, normal-start recovery (E089): a locked run loses tracking
    /// by injection, the guard stops it, and after a full second off the
    /// ordinary startup runs again and completes the original window.
    pub fn restart_campaign<IO: Hal + Sink>(&mut self, io: &mut IO, target: u16) {
        let window_us = BEMF_TOTAL_MS * 1_000;
        let need_us = R::OFF_US + R::STARTUP_US + crate::ramp::ramp_us(target) + R::MIN_HOLD_US;
        io.say("BEMFRESTARTRUN ");
        io.kv("target_duty_tenths", u32::from(target));
        io.kv("window_ms", BEMF_TOTAL_MS);
        io.kv("need_ms", need_us / 1_000);
        io.kv("off_ms", R::OFF_US / 1_000);
        io.say("\r\n");
        io.flush_link();
        let mut policy = R::policy();
        let first = self.restart_first(io, target);
        let t0 = first.entry;
        let first_stop_ms = first.stopped_at.wrapping_sub(t0) / 1_000;
        let first_ms = io.now().wrapping_sub(t0) / 1_000;
        let decision = policy.admit(
            first.reason,
            io.now().wrapping_sub(t0).saturating_add(R::OFF_US),
            window_us,
            need_us - R::OFF_US,
        );
        let (second, aborted) = if decision.is_ok() {
            self.restart_second(io, target, t0, window_us)
        } else {
            (None, false)
        };
        let campaign_ms = io.now().wrapping_sub(t0) / 1_000;
        io.say("BEMFRESTART ");
        io.kv("first_reason", u32::from(first.reason.code()));
        io.kv("first_hold_ms", first.hold_ms);
        io.kv("first_stop_ms", first_stop_ms);
        io.kv("first_report_end_ms", first_ms);
        io.kv("admitted", u32::from(decision.is_ok()));
        io.kv("refusal", decision.err().map_or(0, |r| r.code()));
        io.kv("remaining_ms", decision.map_or(0, |r| r / 1_000));
        io.kv("aborted", u32::from(aborted));
        io.kv(
            "second_reason",
            second.map_or(0, |o: Outcome| u32::from(o.reason.code())),
        );
        io.kv("second_hold_ms", second.map_or(0, |o| o.hold_ms));
        io.kv(
            "drive_end_ms",
            second.map_or(first_stop_ms, |o| o.stopped_at.wrapping_sub(t0) / 1_000),
        );
        io.kv("campaign_ms", campaign_ms);
        io.kv("window_ms", BEMF_TOTAL_MS);
        let recovered = first.reason == Reason::Tracking
            && second.is_some_and(|o| o.reason == Reason::SegmentDeadline && o.hold_ms > 0);
        io.kv("recovered", u32::from(recovered));
        io.say("\r\n");
        io.flush_link();
    }

    /// Segment 1: the full chain to target, then the injected loss
    /// `INJECT_AT_TARGET_US` after the ramp reached target.
    fn restart_first<IO: Hal + Sink>(&mut self, io: &mut IO, target: u16) -> Outcome {
        let after = crate::ramp::ramp_us(target) + R::INJECT_AT_TARGET_US;
        self.rung(
            io,
            target,
            Some((Inject::Tracking, after)),
            Inject::Tracking.code(),
            BEMF_TOTAL_MS,
            None,
        )
    }

    /// A full second off (a stop byte ends the campaign), then segment 2 to
    /// the end of the original window.
    fn restart_second<IO: Hal + Sink>(
        &mut self,
        io: &mut IO,
        target: u16,
        t0: u32,
        window_us: u32,
    ) -> (Option<Outcome>, bool) {
        let off_from = io.now();
        while io.now().wrapping_sub(off_from) < R::OFF_US {
            io.drain();
            if io.rx().is_some_and(parser_stop) {
                return (None, true);
            }
        }
        let left_ms = window_us.saturating_sub(io.now().wrapping_sub(t0)) / 1_000;
        let out = self.rung(io, target, None, 0, left_ms, Some(t0.wrapping_add(window_us)));
        (Some(out), false)
    }

    /// Serve the shell forever: the qualification path only (E110).
    pub fn serve<IO: Hal + Sink>(&mut self, io: &mut IO) -> ! {
        loop {
            io.now();
            io.drain();
            if let Some(b) = io.rx() {
                self.command(io, b);
            }
        }
    }

    /// The climb's rung selector and its two runs (E147).
    fn climb_key<IO: Hal + Sink>(&mut self, io: &mut IO, b: u8) {
        if b == b'+' || b == b'-' {
            let step = 25;
            self.climb_tenths = if b == b'+' {
                (self.climb_tenths + step).min(policy::SIXSTEP_DUTY_CAP)
            } else {
                self.climb_tenths.saturating_sub(step).max(375)
            };
            io.say("CLIMBAT ");
            io.kv("duty_tenths", u32::from(self.climb_tenths));
            io.say("\r\n");
            io.flush_link();
            return;
        }
        let target = self.climb_tenths;
        let window = if b == b'L' {
            BEMF_TOTAL_MS
        } else {
            policy::BEMF_EXPLORE_MS
        };
        let _ = self.rung(io, target, None, 0, window, None);
    }

    /// One shell command.
    ///
    /// Key notes, kept here rather than in the body so the function stays
    /// inside the structure limit (E154):
    ///
    /// * The short holds (`3`/`4`/`6`…) run ~15 s at target for metered
    ///   current points. They are not fixture rungs: the 30 s hold gate does
    ///   not apply to them.
    /// * Campaign 6's rungs (E137): lowercase explores with ~10 s at target,
    ///   uppercase qualifies on the full window. `SIXSTEP_DUTY_CAP` refuses
    ///   anything above the cap whatever is asked for here. E139 added the
    ///   bisect point between 27.5% (qualified) and 30% (failed twice);
    ///   28.75% is not representable in tenths, hence 288.
    /// * `x` sets the duty the lowercase provocations and `Z` use (E141).
    /// * The climb's rung selector (E147): five rungs need ten keys and the
    ///   shell has three lowercase letters left, so one duty is held in
    ///   `climb_tenths` and stepped in the goal's 2.5% increments -- `+` up,
    ///   `-` down, `l` explores it, `L` qualifies it.
    ///
    /// Every capture records the duty it actually ran
    /// (`BEMFRUN target_duty_tenths`), so the record is unambiguous whatever
    /// the shell state, and the fixture is told which rung it asked for.
    pub fn command<IO: Hal + Sink>(&mut self, io: &mut IO, b: u8) {
        let full = |c: &mut Self, io: &mut IO, target: u16| {
            let _ = c.rung(io, target, None, 0, BEMF_TOTAL_MS, None);
        };
        match b {
            b'p' => {
                let p = io.preflight();
                measure::say_preflight(&p, io);
                io.flush_link();
            }
            b'b' => full(self, io, BEMF_DUTY_TENTHS),
            b'2' => full(self, io, 200),
            b'5' => full(self, io, 250),
            b'm' | b'M' | b'y' | b'Y' | b'a' | b'c' | b'd' | b'e' | b'j' | b'A' | b'C' | b'D' | b'E' | b'J' => {
                let target = match b {
                    b'm' | b'M' => 338,
                    b'y' | b'Y' => 288,
                    b'a' | b'A' => 275,
                    b'c' | b'C' => 300,
                    b'd' | b'D' => 325,
                    b'e' | b'E' => 350,
                    _ => 375,
                };
                let window = if b.is_ascii_uppercase() {
                    BEMF_TOTAL_MS
                } else {
                    policy::BEMF_EXPLORE_MS
                };
                let _ = self.rung(io, target, None, 0, window, None);
            }
            b'3' | b'4' | b'6' | b'7' | b'8' | b'9' => {
                let target = match b {
                    b'8' => 250,
                    b'3' => 230,
                    b'4' => 210,
                    b'6' => 190,
                    b'7' => 170,
                    _ => 150,
                };
                let _ = self.rung(io, target, None, 0, 28_000, None);
            }
            b'R' => self.restart_campaign(io, 250),
            b'+' | b'-' | b'l' | b'L' => self.climb_key(io, b),
            b'x' => {
                // 25% (campaign 5), 37.5% (campaign 6), then 47.5% -- campaign
                // 7's highest qualified rung, where its item 5 runs (E149).
                self.provoke_tenths = match self.provoke_tenths {
                    250 => 375,
                    375 => 475,
                    // 500 and 600 added in E244. The goal requires 3/3 restart
                    // at 50% AND 60%, and until now the cycle stopped at 475 so
                    // NEITHER was commandable -- the criterion was structurally
                    // unreachable and E240 wrongly said it was not (E242).
                    475 => 500,
                    500 => 600,
                    _ => 250,
                };
                io.say("PROVOKEAT ");
                io.kv("duty_tenths", u32::from(self.provoke_tenths));
                io.say("\r\n");
                io.flush_link();
            }
            b'Z' => {
                let at = self.provoke_tenths;
                self.restart_campaign(io, at);
            }
            b'?' => {
                io.say("b/2/5=15/20/25% 8/3/4/6/7/9=25/23/21/19/17/15% short acdej/ACDEJ=27.5/30/32.5/35/37.5% explore/qual y/Y=28.8% m/M=33.8% x=provoke-duty 25/37.5% Z=restart at it +/-=climb duty l/L=explore/qual it R=restart T G F N U H V I W K Q=protections at 15%, t g f n u h v i w k q at 25% p=preflight o=stop\r\n");
                io.flush_link();
            }
            other => {
                if let Some(k) = inject_for(other) {
                    let _ = self.rung(
                        io,
                        BEMF_DUTY_TENTHS,
                        Some((k, policy::INJECT_AFTER_US)),
                        k.code(),
                        BEMF_TOTAL_MS,
                        None,
                    );
                } else if let Some(k) = inject_for(other.to_ascii_uppercase()).filter(|_| other.is_ascii_lowercase()) {
                    // The same stimulus from a locked 25% loop (E125): fired
                    // two seconds after the ramp reached target.
                    let at = self.provoke_tenths;
                    let after = crate::ramp::ramp_us(at) + R::INJECT_AT_TARGET_US;
                    let _ = self.rung(io, at, Some((k, after)), k.code(), BEMF_TOTAL_MS, None);
                }
            }
        }
    }
}

impl<
    W: Direction,
    B: Bemf,
    A: Advance,
    C: CurrentLimit,
    S: SagLimit,
    R: RestartRule,
    T: Reporting,
    G: crate::sagtrace::SagLog,
    H: Carrier,
> Default for Controller<W, B, A, C, S, R, T, G, H>
{
    fn default() -> Self {
        Self::new()
    }
}

/// `seed::Fault` as the report's code.
const fn fault_code(f: crate::seed::Fault) -> u32 {
    match f {
        crate::seed::Fault::Window => 6,
        crate::seed::Fault::Sequence => 2,
        crate::seed::Fault::Spacing => 3,
        crate::seed::Fault::Corroboration => 4,
    }
}

/// **Does this key drive a powered run?**
///
/// The one authoritative list. The recording images have to arm and dump their
/// rings around exactly the keys that drive a run, and they each kept their own
/// hand-maintained allowlist — which is how a gate-4 provocation on the sag
/// recorder ran, latched the guard, froze the rings and emitted *nothing*
/// (E183): `v` was missing from the copy. E186 asked for a test tying the two
/// together; deleting one of the two lists is better than testing that they
/// agree, so both images now ask this.
///
/// `b` (the 15% warm-up) is included here because it *is* a run; the images
/// exclude it themselves, because a dump after the unjudged warm-up would
/// leave a second ring in the link for the next capture to read as its own.
#[must_use]
pub const fn drives_a_run(b: u8) -> bool {
    matches!(
        b,
        // the fixed rungs and the short metered holds
        b'b' | b'2' | b'5' | b'3' | b'4' | b'6' | b'7' | b'8' | b'9'
        // the lettered rungs, explore and qualify
        | b'm' | b'M' | b'y' | b'Y' | b'a' | b'A' | b'c' | b'C'
        | b'd' | b'D' | b'e' | b'E' | b'j' | b'J'
        // the climb's two run keys ('+'/'-' only move the duty)
        | b'l' | b'L'
        // the restart campaigns
        | b'R' | b'Z'
    ) || inject_for(b).is_some()
        // The lowercase provocations: the same stimulus from a locked loop at
        // the provoke duty (E125). `command` reaches them by upper-casing, so
        // this must too -- the first version of this function missed them, and
        // the key-set test caught it.
        || (b.is_ascii_lowercase() && inject_for(b.to_ascii_uppercase()).is_some())
}

/// The provocation a shell key asks for.
#[must_use]
pub const fn inject_for(b: u8) -> Option<Inject> {
    Some(match b {
        b'T' => Inject::Tracking,
        b'G' => Inject::TickGap,
        b'F' => Inject::FeedbackStale,
        b'N' => Inject::Driver,
        b'U' => Inject::Storm,
        b'H' => Inject::Overrun,
        b'V' => Inject::Sag,
        b'I' => Inject::AverageCurrent,
        b'W' => Inject::Watchdog,
        // E355: the last two free keys in the shell's space. `LateArm` first
        // because it is this campaign's own hazard and had no provocation at
        // all; `BlankLatched` because it is live and never once observed.
        b'K' => Inject::LateArm,
        b'Q' => Inject::BlankLatched,
        _ => return None,
    })
}

#[cfg(test)]
mod key_tests {
    /// The recorder images arm their rings on exactly the keys that drive a
    /// run, so this list is safety-relevant to the *evidence*: a key that
    /// drives a run and is missing records nothing (E183's silent no-dump), and
    /// a key that does not drive a run but is listed dumps a stale ring into
    /// the next capture.
    #[test]
    fn every_run_key_is_named_and_nothing_else_is() {
        for b in b'!'..=b'~' {
            let drives = super::drives_a_run(b);
            let dispatched = matches!(
                b,
                b'b' | b'2'
                    | b'5'
                    | b'3'
                    | b'4'
                    | b'6'
                    | b'7'
                    | b'8'
                    | b'9'
                    | b'm'
                    | b'M'
                    | b'y'
                    | b'Y'
                    | b'a'
                    | b'A'
                    | b'c'
                    | b'C'
                    | b'd'
                    | b'D'
                    | b'e'
                    | b'E'
                    | b'j'
                    | b'J'
                    | b'l'
                    | b'L'
                    | b'R'
                    | b'Z'
                    | b'T'
                    | b'G'
                    | b'F'
                    | b'N'
                    | b'U'
                    | b'H'
                    | b'V'
                    | b'I'
                    | b'W'
                    | b't'
                    | b'g'
                    | b'f'
                    | b'n'
                    | b'u'
                    | b'h'
                    | b'v'
                    | b'i'
                    | b'w'
                    // E355: the two new provocations and their 25% variants.
                    | b'K'
                    | b'Q'
                    | b'k'
                    | b'q'
            );
            assert_eq!(drives, dispatched, "key {:?}", b as char);
        }
        // Keys that must NOT arm a ring: they change state or print, and a dump
        // after one would corrupt the next capture.
        for b in [b'+', b'-', b'x', b'p', b'?', b'o', b's'] {
            assert!(!super::drives_a_run(b), "key {:?} must not drive a run", b as char);
        }
    }
}
