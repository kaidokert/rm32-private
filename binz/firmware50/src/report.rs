//! Telemetry: the run's closing record as a value, and the one formatter
//! that writes it.
//!
//! A run's evidence is gathered into a [`RunReport`] **after `safe_off`**, and
//! only then written out. Nothing is formatted while the bridge is driven:
//! UART transmit edges couple into the zero-hysteresis comparator (E073), so
//! the qualified image prints nothing while powered, and neither does this
//! one.
//!
//! The text format is the fixture's contract (`scripts/bemf_run.py`,
//! `scripts/cohort.py` parse it), so it is reproduced byte for byte and
//! pinned by the tests below. Numbers are written by [`Sink::say_u32`], which
//! uses reciprocal multiplies, not `/`: at opt-level "s" a plain division is
//! an `__aeabi_uidiv` call.

use crate::fixed::{div_10, rem_10};
use crate::protection::Reason;

/// Somewhere the report's bytes go. `put` is the only required method.
pub trait Sink {
    fn put(&mut self, b: u8);

    /// Called between report sections so a bounded transmit ring can drain.
    fn flush(&mut self) {}

    fn say(&mut self, s: &str) {
        for b in s.as_bytes() {
            self.put(*b);
        }
    }

    fn say_u32(&mut self, mut v: u32) {
        if v == 0 {
            self.put(b'0');
            return;
        }
        // 16 entries and a masked index, so the bounds check is provably dead.
        let mut buf = [0u8; 16];
        let mut n = 0usize;
        while v > 0 && n < 10 {
            buf[n & 15] = b'0' + rem_10(v) as u8;
            v = div_10(v);
            n += 1;
        }
        while n > 0 {
            n -= 1;
            self.put(buf[n & 15]);
        }
    }

    fn say_i32(&mut self, v: i32) {
        if v < 0 {
            self.put(b'-');
            self.say_u32(v.unsigned_abs());
        } else {
            self.say_u32(v as u32);
        }
    }

    fn kv(&mut self, key: &str, v: u32) {
        self.say(key);
        self.say("=");
        self.say_u32(v);
        self.say(" ");
    }

    fn kvi(&mut self, key: &str, v: i32) {
        self.say(key);
        self.say("=");
        self.say_i32(v);
        self.say(" ");
    }
}

// ---------------------------------------------------------------------------
// The coast witness
// ---------------------------------------------------------------------------

/// Coast comparator edges above which the rotor is witnessed as turning.
///
/// **Derived from a measured control, not chosen** (notebook E040): a rotor
/// the bridge never energised gave `comp_edges` = 2, 2, 4; driven runs gave
/// 175 and 178. 20 sits five times above the stationary ceiling and nearly
/// nine times below the driven floor.
pub const COAST_SPUN_MIN_EDGES: u32 = 20;

/// The matched speed window, as measured (campaign 8 step 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TailWindow {
    /// Accepted crossings in the window, and its span in µs.
    pub accepts: u32,
    pub span_us: u32,
    /// The window's start and end, as µs before the stop stamp. Stated so a
    /// coast extrapolated back to the stop can be compared against a powered
    /// window whose position is known rather than assumed.
    pub start_before_stop_us: u32,
    pub end_before_stop_us: u32,
}

/// What the bridge-off coast window saw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoastStats {
    /// Peak-to-peak BEMF over the first and last quarter of the window.
    pub pp_first: i32,
    pub pp_last: i32,
    /// Decisive polarity alternations (ADC; the pins are no longer scanned,
    /// E041, so this reads 0).
    pub crossings: u32,
    pub scans: u32,
    /// Midpoint the crossings were counted against, in raw codes.
    pub midpoint: i32,
    /// Comparator output transitions, high samples and polls over the window.
    pub comp_edges: u32,
    pub comp_hi: u32,
    pub comp_polls: u32,
    /// Debounced comparator transitions, the time from the float to the
    /// first, and the first [`COAST_IV_LEN`] spacings (E065, widened E281).
    /// Two transitions per electrical cycle on one phase.
    pub trans_n: u32,
    /// µs from the stop stamp to the coast window's own origin: what the
    /// bridge spends being safed and the comparator re-pointed before the
    /// first transition can be seen. **Measured, not assumed** -- any
    /// back-extrapolation of the coast to the bridge-off instant crosses it
    /// (campaign 8 step 3).
    pub offset_us: u32,
    pub first_trans_us: u32,
    /// The retained half-period spacings. **32, widened from 8 in E281**, and
    /// the reason is the one defect behind every disputed reading in this
    /// campaign: `speed.coast_fit` pairs these into full cycles and
    /// least-squares an intercept back to the stop, so 8 gave **4 points and 2
    /// residual degrees of freedom**.
    ///
    /// The identity built on it has **93% of its scatter in this estimator**
    /// (`powered` sd 1.03 per mille against `coast@0` sd 3.76 over six runs of
    /// one image), and **10 of 318 runs produced a physically impossible
    /// positive slope** -- the rotor accelerating with the bridge off -- whose
    /// mean residual was +11.9 per mille against +0.1 for the rest. Both
    /// records that blocked rungs 400 and 550 on the rate gate were in that
    /// class (E273).
    ///
    /// Intercept standard error against length, at the measured spacing:
    /// 8 -> x1.000 (2.2 ms window), 24 -> x0.520, **32 -> x0.446 (8.3 ms)**.
    /// 32 more than halves it, and the rotor slows only **0.71%** across that
    /// window at the measured -1495 eHz/s, so the linear fit stays unbiased.
    /// There is data to fill it: `COASTTIMING trans=1351` at rung 450 means
    /// 1351 transitions were already seen and the first 8 kept.
    ///
    /// The capture loop (`run::measure`) and `emit` are both generic over this
    /// length, so widening it touches nothing else.
    pub trans_iv: [u32; COAST_IV_LEN],
}

/// Retained coast half-period spacings; see [`CoastStats::trans_iv`].
pub const COAST_IV_LEN: usize = 32;

impl CoastStats {
    /// A result for a run that never got to drive (`scans == 0`), so a
    /// refused run cannot be read as "measured, found nothing".
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            pp_first: 0,
            pp_last: 0,
            crossings: 0,
            scans: 0,
            midpoint: 0,
            comp_edges: 0,
            comp_hi: 0,
            comp_polls: 0,
            trans_n: 0,
            offset_us: 0,
            first_trans_us: 0,
            trans_iv: [0; COAST_IV_LEN],
        }
    }

    /// Did the coast witness rotation? Comparator edges against a threshold
    /// from a stationary-rotor control (checked in both directions, E040).
    #[must_use]
    pub const fn witnessed_rotation(&self) -> bool {
        self.comp_edges > COAST_SPUN_MIN_EDGES
    }

    /// `BEMFCOAST` and `COASTTIMING`.
    pub fn emit(&self, reason: Reason, out: &mut impl Sink) {
        out.say("BEMFCOAST ");
        out.kv("reason", u32::from(reason.code()));
        out.kvi("pp_first", self.pp_first);
        out.kvi("pp_last", self.pp_last);
        out.kv("crossings", self.crossings);
        out.kv("spun", u32::from(self.witnessed_rotation()));
        out.kv("comp_edges", self.comp_edges);
        out.kv("comp_hi", self.comp_hi);
        out.kv("comp_polls", self.comp_polls);
        out.say("\r\n");
        out.say("COASTTIMING ");
        out.kv("trans", self.trans_n);
        out.kv("offset_us", self.offset_us);
        out.kv("first_us", self.first_trans_us);
        out.say("iv_us=");
        let mut i = 0;
        while i < self.trans_iv.len() {
            if i > 0 {
                out.say(",");
            }
            out.say_u32(self.trans_iv[i]);
            i += 1;
        }
        out.say(" ");
        // Two transitions per cycle: eHz = 500_000 / spacing. After safe_off.
        out.kv("ehz_first", 500_000u32.checked_div(self.trans_iv[0]).unwrap_or(0));
        out.say("\r\n");
    }
}

// ---------------------------------------------------------------------------
// The run report
// ---------------------------------------------------------------------------

const SECTOR_ACC_KEYS: [&str; 6] = ["a1", "a2", "a3", "a4", "a5", "a6"];
const SECTOR_FRC_KEYS: [&str; 6] = ["f1", "f2", "f3", "f4", "f5", "f6"];
const REVISIT_TRY_KEYS: [&str; 6] = ["r1", "r2", "r3", "r4", "r5", "r6"];
const REVISIT_ACC_KEYS: [&str; 6] = ["v1", "v2", "v3", "v4", "v5", "v6"];
/// Where an accepted crossing landed relative to the believed interval.
const ACCEPT_PHASE_KEYS: [&str; 5] = ["lt075", "to100", "to125", "to150", "gt150"];

/// A gate-4 provocation's outcome (E080).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InjectOutcome {
    pub expected_reason: u32,
    pub guard_reason: u32,
    /// When the stimulus fired, if it did, and the loop-exit instant.
    pub fired_at: Option<u32>,
    pub stopped_at: u32,
}

/// The ISR detector's own counters and the commutation root's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Roots {
    /// `ZeroCross::counts()`: accepted, too early, unstable.
    pub zc_accepted: u32,
    pub too_early: u32,
    pub unstable: u32,
    /// COMP's entry-to-arm time and the arms that reached the wait (E083).
    pub spent_max_us: u32,
    pub late_arms: u32,
    /// The estimator's value and the measured spend at the FIRST late arm
    /// (E314), zero if none fired. Together they say which mechanism stopped
    /// the run: `ci_at_late` near the hold mean is a boundary failure, well
    /// below it is a depressed estimate.
    pub ci_at_late: u32,
    pub spent_at_late: u32,
    /// The smallest accepted average interval, µs: the causal variable, and
    /// invariant under the advance level.
    pub ci_min_us: u32,
    /// Acceptances with 2 µs of margin or less.
    pub thin_count: u32,
    /// `unstable` accumulated during the hold only (E212).
    pub hold_unstable: u32,
    /// Intervals beyond `ci_max` that were re-based and discarded -- counted
    /// and reset since the estimator was written, and **never reported** until
    /// E208. It is the counter that says whether a crossing was lost just
    /// before a stop (E205 SS1d).
    pub rebase: u32,
    /// **COM dispatches** (any phase, not only commutations) that preempted a
    /// zero-crossing decision, and the subset that landed inside `com_arm`'s
    /// own write sequence. Both read zero unless the image carries the chain
    /// recorder *and* is built `com-top`: the counters are gated on the
    /// recorder, so a `com-top` production image reads zero too, which is why
    /// they must not be read as "the mechanism did not fire" on their own
    /// (E170).
    pub com_preempts: u32,
    pub com_arm_preempts: u32,
    /// Driven-stage refusals (ISR).
    pub drv_early: u32,
    pub drv_unstable: u32,
    pub drv_defers: u32,
    /// COMP call rates: driven peak, closed-loop peak, the storm latch.
    pub drv_peak: u32,
    pub det_peak: u32,
    pub storm: bool,
    pub storm_step: u32,
    pub cap_armed: bool,
    pub comp_call_max_us: u32,
    pub overrun: bool,
    pub blank_arms: u32,
    pub blank_latched: u32,
    pub com_count: u32,
    pub com_late_max_us: u32,
}

/// The driven observation and the seed it produced (E058).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Driven {
    pub entered: bool,
    pub epochs: u32,
    pub accepts: u32,
    pub retries: u32,
    pub late_max_us: u32,
    pub qual_intervals: u32,
    pub qual_reanchors: u32,
    /// `seed::Fault` as its report code (0 = none).
    pub qual_fault: u32,
    /// Why the stage stopped, if it did (1-5; 0 = it handed over).
    pub fail: u32,
    /// The seed: step and interval.
    pub seed: Option<(u8, u32)>,
    pub commanded_us: u32,
    /// Every driven acceptance: (epoch, step, interval µs, position µs).
    pub rows: [(u16, u8, u16, u16); 64],
    pub rows_n: usize,
}

impl Default for Driven {
    fn default() -> Self {
        Self {
            entered: false,
            epochs: 0,
            accepts: 0,
            retries: 0,
            late_max_us: 0,
            qual_intervals: 0,
            qual_reanchors: 0,
            qual_fault: 0,
            fail: 0,
            seed: None,
            commanded_us: 0,
            rows: [(0, 0, 0, 0); 64],
            rows_n: 0,
        }
    }
}

/// The guard root and the tracking watch (E076), plus foreground health.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardRecord {
    pub reason: u32,
    pub ticks: u32,
    pub gap_max_us: u32,
    /// `tracking::Fault` as its report code (0 = none).
    pub track_fault: u32,
    pub track_max_us: u32,
    pub fast_events: u32,
    pub fast_min_us: u32,
    pub loop_iters_closed: u32,
    pub loop_gap_max_us: u32,
    pub acquire_us: u32,
}

/// The current proxy (E078/E093).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CurrentRecord {
    pub blocks: u32,
    /// **Scans the drive loop actually consumed.** Reported from E224.
    ///
    /// `adc_hz` in the banner is a hard-coded literal, so it is not evidence
    /// of anything; `loop_gap_max_us` exceeding the 101 us scan period says
    /// scans are missed. This field over `hold_ms` is the only direct
    /// measurement of the rate that `SAG_FILTER_SHIFT`'s "207 ms",
    /// `RAIL_MEAN_LEN`'s window and any droop persistence assume (E222 SS8).
    pub drive_scans: u32,
    /// The duty cap **in force**, tenths -- not the duty commanded.
    ///
    /// `sixstep::plan` clamps at this silently and `policy::sixstep_ccr_of`
    /// clamps again independently on the same constant, while the report's
    /// `target_duty_tenths` is the **pre-clamp request**. Nothing revealed a
    /// clamp before this field; it exists now rather than alongside the first
    /// cap raise, which is when it would become load-bearing (E222 SS11/SS14).
    pub applied_cap: u16,
    /// The compare value actually programmed for the commanded duty, TIM1
    /// ticks -- the arithmetic downstream of both clamps.
    pub applied_ccr: u32,
    /// Scans below each [`crate::protection::DEPTH_FRACTIONS`] fraction.
    ///
    /// **Observation, not a threshold**: no stop is attached. E218 tried to
    /// pick a slow-droop line from quantities never recorded and took both its
    /// constants from the wrong population; these eight numbers are the
    /// distribution such a line has to be chosen from (E223).
    pub depth_below: [u32; 4],
    /// The raw allowance the mA fields are scaled against (E287). Equal to
    /// `RAW_LIMIT` on a normal run; `RAW_LIMIT/100` under
    /// `Inject::AverageCurrent`, which makes those mA figures 100x inflated.
    pub current_allow: u32,
    /// The raw-scan observer's counts and longest runs (E284), with its own
    /// fractions so the capture stays self-describing.
    ///
    /// E287's field was inserted between this comment and its fields, so for
    /// two commits it documented `current_allow` and these two carried nothing
    /// — a mechanical patch landing a field at a matched line rather than in a
    /// place chosen for it.
    pub raw_depth_below: [u32; 4],
    pub raw_depth_longest: [u32; 4],
    /// Longest consecutive run below each fraction, in scans. A count cannot
    /// separate one dip from a droop and a streak cannot say how often, so a
    /// persistence is chosen from this and a fraction from the counts above.
    pub depth_longest: [u32; 4],
    pub mean_residual: i32,
    pub mean_ma: i32,
    pub hold_blocks: u32,
    pub hold_ma: i32,
    pub zero_blocks: u32,
    pub zero_start: u32,
    pub zero_end: Option<u32>,
    pub zero_drift_ma: i32,
    /// **The duty ceiling the foldback governor ended the run at**, tenths.
    ///
    /// The governor ratchets *down only*, and until E187 its new ceiling was
    /// thrown away at the call site (`let _ = self.governor.warn(..)`), so a
    /// run that touched the 4 A allowance once, got throttled, and then
    /// completed its window was reported as a clean run at the commanded duty
    /// -- with every current and speed figure measured at a *lower* duty. A
    /// run whose `ceiling_tenths` is below its `duty_tenths` did not hold the
    /// rung, whatever else the report says (E186 SS7.3).
    pub ceiling_tenths: u16,
    /// The **worst single block's** signed current residual, raw codes.
    ///
    /// `AverageCurrent` judges 100-scan (10.1 ms) block means, so every
    /// current number this bench has ever produced is an average. This is the
    /// worst of those blocks rather than the mean of them -- still not a peak
    /// (a desync surge inside one block is averaged), but the first figure
    /// here that is not a mean of means. It was computed every block and had
    /// no reader at all (E186 SS7.4).
    pub worst_residual: i32,
    /// The same, in mA through the block-mean scale.
    pub worst_ma: i32,
    /// The worst single block **of the hold**, mA -- the figure any claim about
    /// current at a rung actually wants.
    ///
    /// [`Self::worst_ma`] is the worst of the whole run, ramp included, and the
    /// two differ by more than an order of magnitude at low rungs (900 mA
    /// against a 34 mA hold at duty 150). E194 withdrew three worst-block
    /// claims over that and said a windowed worst had to land before the next
    /// current claim; E241 made the next claim without it and projected the
    /// ramp artefact three rungs forward (E242). This is that field.
    pub worst_hold_ma: i32,
}

/// The during-run rotation witness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WitnessRecord {
    pub samples: u32,
    pub bemf_min: i32,
    pub bemf_max: i32,
    pub mid: i32,
    pub mid_fixed: bool,
    pub alt: u32,
    pub hyst_codes: u32,
    pub rotated: bool,
    pub vsenc_min: u32,
    pub vsenc_max: u32,
    pub star_min: u32,
    pub star_max: u32,
}

/// Everything a run leaves behind, gathered after `safe_off`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunReport {
    pub reason: Option<Reason>,
    pub target_tenths: u16,
    pub inject: Option<InjectOutcome>,
    /// Commutations on accepted crossings, and forced ones (structurally 0).
    pub accepted: u32,
    pub forced: u32,
    /// The estimate at the stop (or the last one the foreground held).
    pub ci_us: u32,
    pub bus_ref: u32,
    pub bus_min: u32,
    /// Both of the sag guard's references at the stop (E146): the pre-run
    /// baseline, and the ~200 ms sharp reference it now judges against, with
    /// the streak and the latch.
    pub sag_ref: (u32, u32),
    pub sag_filt: (u32, u32),
    pub sag_streak: u32,
    pub sag_tripped: bool,
    /// Closed-loop and hold-at-target durations, from the MCU's own clock.
    pub closed_us: u32,
    pub hold_us: u32,
    /// Accepted crossings and their summed intervals over the hold.
    pub hold_acc: u32,
    pub hold_forced: u32,
    pub hold_ci_sum: u32,
    /// The matched speed window (campaign 8 step 3): the accepted crossings
    /// between the anchor and the last crossing before the stop, the window's
    /// own span in µs, and both endpoints' distance back from the stop stamp.
    /// The host divides these itself; nothing here is rounded to whole µs, as
    /// the printed loop frequency is (`mean_sector_us` below).
    pub tail: Option<TailWindow>,
    pub roots: Roots,
    pub acc_by_step: [u16; 6],
    pub forced_by_step: [u16; 6],
    pub acc_by_phase: [u16; 5],
    pub revisit_attempts: [u16; 6],
    pub revisit_accepts: [u16; 6],
    pub driven: Driven,
    pub guard: GuardRecord,
    pub current: CurrentRecord,
    pub witness: WitnessRecord,
}

/// The oracle's current at a duty (`DUTY_50_CAMPAIGN.md` rung table, E082).
#[must_use]
pub const fn oracle_ma(duty_tenths: u16) -> u32 {
    match duty_tenths {
        250.. => 326,
        200.. => 167,
        150.. => 58,
        _ => 45,
    }
}

impl RunReport {
    fn reason_code(&self) -> u32 {
        self.reason.map_or(0, |r| u32::from(r.code()))
    }

    /// The hold at target, ms.
    #[must_use]
    pub const fn hold_ms(&self) -> u32 {
        self.hold_us / 1_000
    }

    /// Write the report, `BEMFINJECT` through the rotation witness. Division
    /// is fine here: this runs once, after `safe_off`, on no motor path.
    pub fn emit(&self, out: &mut impl Sink) {
        self.emit_outcome(out);
        self.emit_rates(out);
        self.emit_histograms(out);
        self.emit_driven(out);
        self.emit_guard(out);
        self.emit_rows(out);
        self.emit_current(out);
        self.emit_witness(out);
    }

    fn emit_outcome(&self, out: &mut impl Sink) {
        if let Some(inj) = self.inject {
            out.say("BEMFINJECT ");
            out.kv("expected_reason", inj.expected_reason);
            out.kv("reason", self.reason_code());
            out.kv("guard_reason", inj.guard_reason);
            out.kv("fired", u32::from(inj.fired_at.is_some()));
            out.kv(
                "stop_after_inject_us",
                inj.fired_at.map_or(0, |t| inj.stopped_at.wrapping_sub(t)),
            );
            out.kv("provoked", u32::from(self.reason_code() == inj.expected_reason));
            out.say("\r\n");
        }
        out.say("BEMFDONE ");
        out.kv("reason", self.reason_code());
        out.kv("accepted", self.accepted);
        out.kv("forced", self.forced);
        out.kv("zc_acc", self.roots.zc_accepted);
        out.kv("too_early", self.roots.too_early);
        out.kv("unstable", self.roots.unstable);
        out.kv("ci_us", self.ci_us);
        out.kv("bus_ref", self.bus_ref);
        out.kv("bus_min", self.bus_min);
        out.say("\r\nBEMFSAG ");
        out.kv("ref_bus", self.sag_ref.0);
        out.kv("ref_vref", self.sag_ref.1);
        out.kv("filt_bus", self.sag_filt.0);
        out.kv("filt_vref", self.sag_filt.1);
        out.kv("streak", self.sag_streak);
        out.kv("tripped", u32::from(self.sag_tripped));
        out.say("\r\n");
    }

    /// Goal gates 1 and 2 on the MCU's own clock: the rate identity over the
    /// hold, against the mean sector (hold / every commutation).
    fn emit_rates(&self, out: &mut impl Sink) {
        let closed_ms = self.closed_us / 1_000;
        let ci = self.ci_us.max(1);
        let done = self.accepted + self.forced;
        out.say("BEMFGATE ");
        out.kv("closed_ms", closed_ms);
        out.kv("hold_ms", self.hold_ms());
        out.kv("target_tenths", u32::from(self.target_tenths));
        out.kv(
            "zc_per_s",
            self.accepted.saturating_mul(1_000).checked_div(closed_ms).unwrap_or(0),
        );
        out.kv(
            "comm_per_s",
            done.saturating_mul(1_000).checked_div(closed_ms).unwrap_or(0),
        );
        out.kv("ehz_from_ci_last", 1_000_000 / (6 * ci));
        out.kv(
            "forced_pct",
            self.forced.saturating_mul(100).checked_div(done).unwrap_or(0),
        );
        out.say("\r\n");
        let hold_ms = self.hold_ms();
        let mean_ci = self.hold_ci_sum.checked_div(self.hold_acc).unwrap_or(0).max(1);
        let zc_rate = self.hold_acc.saturating_mul(1_000).checked_div(hold_ms).unwrap_or(0);
        let all = self.hold_acc + self.hold_forced;
        let mean_sector = hold_ms.saturating_mul(1_000).checked_div(all).unwrap_or(0).max(1);
        let zc_expect = 1_000_000 / mean_sector;
        out.say("BEMFRATE ");
        out.kv("hold_ms", hold_ms);
        out.kv("hold_accepted", self.hold_acc);
        out.kv("hold_forced", self.hold_forced);
        out.kv("mean_ci_us", mean_ci);
        out.kv("mean_sector_us", mean_sector);
        out.kv("ehz_from_sector", 1_000_000 / (6 * mean_sector));
        out.kv("zc_per_s", zc_rate);
        out.kv("zc_expected_per_s", zc_expect);
        // **Circular, and it measures only truncation** (E172). With
        // `hold_forced = 0`, `zc_per_s` is `accepts / hold_ms` and
        // `zc_expected_per_s` is `1e6 / floor(hold_ms * 1000 / accepts)` --
        // the same two numbers on both sides -- so this ratio is identically
        // `floor(S) / S` for the unrounded mean sector S. At 47.5%,
        // S = 80.97 µs and it reads 988 permille: a 1.2% "failure" that is
        // pure quantisation, and worse at every higher speed. It is still
        // emitted because the fixture's format is a contract, but **nothing
        // gates on it** (`scripts/cohort.py`), and
        // `the_rate_identity_is_only_truncation` below pins why.
        out.kv(
            "zc_rate_permille_of_expected",
            zc_rate.saturating_mul(1_000).checked_div(zc_expect).unwrap_or(0),
        );
        out.kv(
            "hold_forced_pct",
            self.hold_forced.saturating_mul(100).checked_div(all).unwrap_or(0),
        );
        out.say("\r\n");
        // Raw counts and spans only: the host does the division, because a
        // whole-µs mean sector is what made the printed loop frequency
        // unusable as a speed (campaign 8 step 3).
        out.say("BEMFTAIL ");
        let t = self.tail.unwrap_or_default();
        out.kv("accepts", t.accepts);
        out.kv("span_us", t.span_us);
        out.kv("start_before_stop_us", t.start_before_stop_us);
        out.kv("end_before_stop_us", t.end_before_stop_us);
        out.kv("window_us", crate::run::policy::TAIL_WINDOW_US);
        out.say("\r\n");
        out.say("BEMFRCOMP ");
        out.kv("spent_max_us", self.roots.spent_max_us);
        out.kv("late_arms", self.roots.late_arms);
        out.kv("ci_at_late", self.roots.ci_at_late);
        out.kv("spent_at_late", self.roots.spent_at_late);
        // E208: the causal side of the late arm, per run.
        out.kv("ci_min_us", self.roots.ci_min_us);
        out.kv("thin_count", self.roots.thin_count);
        out.kv("hold_unstable", self.roots.hold_unstable);
        out.kv("rebase", self.roots.rebase);
        out.kv("com_preempts", self.roots.com_preempts);
        out.kv("com_arm_preempts", self.roots.com_arm_preempts);
        out.say("\r\n");
    }

    fn emit_histograms(&self, out: &mut impl Sink) {
        out.say("BEMFSECTOR ");
        let sectors = SECTOR_ACC_KEYS.iter().zip(SECTOR_FRC_KEYS.iter());
        for ((ka, kf), (a, f)) in sectors.zip(self.acc_by_step.iter().zip(self.forced_by_step.iter())) {
            out.kv(ka, u32::from(*a));
            out.kv(kf, u32::from(*f));
        }
        out.say("\r\n");
        out.say("BEMFPHASE ");
        for (k, n) in ACCEPT_PHASE_KEYS.iter().zip(self.acc_by_phase.iter()) {
            out.kv(k, u32::from(*n));
        }
        out.say("\r\n");
        out.say("BEMFREVISIT ");
        let keys = REVISIT_TRY_KEYS.iter().zip(REVISIT_ACC_KEYS.iter());
        for ((kr, kv), (r, v)) in keys.zip(self.revisit_attempts.iter().zip(self.revisit_accepts.iter())) {
            out.kv(kr, u32::from(*r));
            out.kv(kv, u32::from(*v));
        }
        out.say("\r\n");
    }

    fn emit_driven(&self, out: &mut impl Sink) {
        let d = &self.driven;
        let r = &self.roots;
        out.say("BEMFDRIVEN ");
        out.kv("entered", u32::from(d.entered));
        out.kv("epochs", d.epochs);
        out.kv("accepts", d.accepts);
        out.kv("early", r.drv_early);
        out.kv("unstable", r.drv_unstable);
        out.kv("defers", r.drv_defers);
        out.kv("retries", d.retries);
        out.kv("late_max_us", d.late_max_us);
        out.kv("qual_intervals", d.qual_intervals);
        out.kv("qual_reanchors", d.qual_reanchors);
        out.kv("qual_fault", d.qual_fault);
        out.kv("fail", d.fail);
        out.kv("seeded", u32::from(d.seed.is_some()));
        out.kv("seed_step", d.seed.map_or(0, |(s, _)| u32::from(s)));
        out.kv("seed_us", d.seed.map_or(0, |(_, us)| us));
        out.kv("commanded_us", d.commanded_us);
        out.kv("irq_peak_per_ms", r.drv_peak);
        out.kv("closed_irq_peak_per_ms", r.det_peak);
        out.kv("storm", u32::from(r.storm));
        out.kv("storm_step", r.storm_step);
        out.kv("cap_armed", u32::from(r.cap_armed));
        out.kv("comp_call_max_us", r.comp_call_max_us);
        out.kv("overrun", u32::from(r.overrun));
        out.kv("blank_arms", r.blank_arms);
        out.kv("blank_latched", r.blank_latched);
        out.kv("com_count", r.com_count);
        out.kv("com_late_max_us", r.com_late_max_us);
        out.say("\r\n");
    }

    fn emit_guard(&self, out: &mut impl Sink) {
        let g = &self.guard;
        out.say("BEMFGUARD ");
        out.kv("reason", g.reason);
        out.kv("ticks", g.ticks);
        out.kv("gap_max_us", g.gap_max_us);
        out.kv("track_fault", g.track_fault);
        out.kv("track_max_us", g.track_max_us);
        out.kv("fast_events", g.fast_events);
        out.kv("fast_min_us", g.fast_min_us);
        out.kv("loop_iters_closed", g.loop_iters_closed);
        out.kv("loop_gap_max_us", g.loop_gap_max_us);
        out.kv("acquire_us", g.acquire_us);
        out.say("\r\n");
        out.flush();
    }

    /// Driven accept rows, eight per line: `epoch:step:interval_us:position_us`.
    fn emit_rows(&self, out: &mut impl Sink) {
        let n = self.driven.rows_n.min(64);
        let mut r = 0;
        while r < n {
            out.say("BEMFDRVROWS");
            let mut k = 0;
            while k < 8 && r < n {
                let (ep, st, iv, pos) = self.driven.rows[r & 63];
                out.say(" ");
                out.say_u32(u32::from(ep));
                out.say(":");
                out.say_u32(u32::from(st));
                out.say(":");
                out.say_u32(u32::from(iv));
                out.say(":");
                out.say_u32(u32::from(pos));
                k += 1;
                r += 1;
            }
            out.say("\r\n");
            out.flush();
        }
    }

    fn emit_current(&self, out: &mut impl Sink) {
        let c = &self.current;
        out.say("BEMFCURRENT ");
        out.kv("blocks", c.blocks);
        out.kvi("mean_residual", c.mean_residual);
        out.kvi("mean_ma", c.mean_ma);
        out.kv("hold_blocks", c.hold_blocks);
        out.kvi("hold_ma", c.hold_ma);
        out.kv("zero_blocks", c.zero_blocks);
        // **The scale the mA fields are in** (E287). `block_milliamps` divides
        // by this, so the figures above are milliamps only when it equals
        // `RAW_LIMIT`; an `Inject::AverageCurrent` run rebuilds the accumulator
        // at `RAW_LIMIT/100` and every mA field is then 100x inflated.
        out.kv("ma_allow", c.current_allow);
        out.kv("ma_allow_ref", crate::protection::RAW_LIMIT);
        out.kv("zero_start", c.zero_start);
        out.kv("zero_end", c.zero_end.unwrap_or(0));
        out.kvi("zero_drift_ma", c.zero_drift_ma);
        out.kv("ref_ma", oracle_ma(self.target_tenths));
        out.kv("duty_tenths", u32::from(self.target_tenths));
        // E187: the two fields that decide whether the numbers above describe
        // the rung that was asked for.
        out.kv("ceiling_tenths", u32::from(c.ceiling_tenths));
        out.kvi("worst_residual", c.worst_residual);
        out.kvi("worst_ma", c.worst_ma);
        out.kvi("worst_hold_ma", c.worst_hold_ma);
        out.kv("drive_scans", c.drive_scans);
        out.kv("applied_cap", u32::from(c.applied_cap));
        out.kv("applied_ccr", c.applied_ccr);
        // **The threshold travels with the counts.** These keys were once
        // named after the fractions (`dep970_n` and friends), and when E244
        // moved the fractions to 995/990/985/980 the names kept saying
        // 970/950/920/900 -- a field whose name lied about what it measured,
        // caught on the first run of the ladder. Indexed keys cannot drift,
        // and each bin prints its own fraction beside its counts so a capture
        // is self-describing.
        out.kv("dep0_pm", crate::protection::DEPTH_FRACTIONS[0]);
        out.kv("dep0_n", c.depth_below[0]);
        out.kv("dep0_run", c.depth_longest[0]);
        out.kv("dep1_pm", crate::protection::DEPTH_FRACTIONS[1]);
        out.kv("dep1_n", c.depth_below[1]);
        out.kv("dep1_run", c.depth_longest[1]);
        out.kv("dep2_pm", crate::protection::DEPTH_FRACTIONS[2]);
        out.kv("dep2_n", c.depth_below[2]);
        out.kv("dep2_run", c.depth_longest[2]);
        out.kv("dep3_pm", crate::protection::DEPTH_FRACTIONS[3]);
        out.kv("dep3_n", c.depth_below[3]);
        out.kv("dep3_run", c.depth_longest[3]);
        // The raw-scan observer (E284). `raw1_run` is the discriminator: the
        // longest consecutive run of raw scans below the guard's own 950 trip
        // line. The guard sees an 8-scan mean, so a dip shorter than its window
        // is invisible to it and to `dep*` above -- and every run, pass or
        // fail, takes a raw scan past that line.
        out.kv("raw0_pm", crate::protection::RAW_DEPTH_FRACTIONS[0]);
        out.kv("raw0_n", c.raw_depth_below[0]);
        out.kv("raw0_run", c.raw_depth_longest[0]);
        out.kv("raw1_pm", crate::protection::RAW_DEPTH_FRACTIONS[1]);
        out.kv("raw1_n", c.raw_depth_below[1]);
        out.kv("raw1_run", c.raw_depth_longest[1]);
        out.kv("raw2_pm", crate::protection::RAW_DEPTH_FRACTIONS[2]);
        out.kv("raw2_n", c.raw_depth_below[2]);
        out.kv("raw2_run", c.raw_depth_longest[2]);
        out.kv("raw3_pm", crate::protection::RAW_DEPTH_FRACTIONS[3]);
        out.kv("raw3_n", c.raw_depth_below[3]);
        out.kv("raw3_run", c.raw_depth_longest[3]);
        out.say("\r\n");
        out.flush();
    }

    fn emit_witness(&self, out: &mut impl Sink) {
        let w = &self.witness;
        out.say("BEMFWITNESS ");
        out.kv("samples", w.samples);
        out.kvi("bemf_min", w.bemf_min);
        out.kvi("bemf_max", w.bemf_max);
        out.kvi("mid", w.mid);
        out.kv("mid_fixed", u32::from(w.mid_fixed));
        out.kv("alt", w.alt);
        out.kv("hyst_codes", w.hyst_codes);
        out.kv("driven_rotation", u32::from(w.rotated));
        // A bare LF, then the comparator-node ranges on their own line: the
        // exact bytes every capture since E041 carries.
        out.say("\n");
        out.flush();
        out.kv("vsenc_min", w.vsenc_min);
        out.kv("vsenc_max", w.vsenc_max);
        out.kv("star_min", w.star_min);
        out.kv("star_max", w.star_max);
        out.kv("bus_ref", self.bus_ref);
        out.say("\r\n");
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::String;
    use std::vec::Vec;

    struct Buf(Vec<u8>);
    impl Sink for Buf {
        fn put(&mut self, b: u8) {
            self.0.push(b);
        }
    }

    fn text(f: impl FnOnce(&mut Buf)) -> String {
        let mut b = Buf(Vec::new());
        f(&mut b);
        String::from_utf8(b.0).unwrap()
    }

    #[test]
    fn numbers_format_like_the_firmware_always_did() {
        assert_eq!(text(|b| b.say_u32(0)), "0");
        assert_eq!(text(|b| b.say_u32(4_294_967_295)), "4294967295");
        assert_eq!(text(|b| b.say_i32(-133)), "-133");
        assert_eq!(text(|b| b.kv("a", 7)), "a=7 ");
        assert_eq!(text(|b| b.kvi("d", i32::MIN)), "d=-2147483648 ");
    }

    #[test]
    fn coast_lines_keep_the_capture_format() {
        let c = CoastStats {
            comp_edges: 1356,
            comp_hi: 304_401,
            comp_polls: 443_146,
            trans_n: 866,
            offset_us: 1_204,
            first_trans_us: 91,
            trans_iv: {
                let mut v = [0u32; COAST_IV_LEN];
                let seed = [431, 440, 430, 443, 432, 443, 430, 446];
                let mut i = 0;
                while i < seed.len() {
                    v[i] = seed[i];
                    i += 1;
                }
                v
            },
            ..CoastStats::empty()
        };
        let got = text(|b| c.emit(Reason::SegmentDeadline, b));
        assert_eq!(
            got,
            "BEMFCOAST reason=2 pp_first=0 pp_last=0 crossings=0 spun=1 comp_edges=1356 comp_hi=304401 \
             comp_polls=443146 \r\nCOASTTIMING trans=866 offset_us=1204 first_us=91 \
             iv_us=431,440,430,443,432,443,430,446,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 \
             ehz_first=1160 \r\n"
        );
        // **The emitted length is part of the capture format** (E281 widened
        // `COAST_IV_LEN` 8 -> 32), so the count is asserted alongside the text:
        // a host pairs these into full cycles, and a silent change in how many
        // arrive is the unit-versioning class E265 exists to refuse.
        let n = got
            .split("iv_us=")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .split(',')
            .count();
        assert_eq!(n, COAST_IV_LEN);
    }

    /// `zc_rate_permille_of_expected` is `floor(S)/S`, nothing else: two
    /// reports whose only difference is a mean sector of 80.97 µs against
    /// 80.02 µs read 988 and 999 permille, and a run at exactly 80 µs reads
    /// 1000. The quantity is quantisation, not loop quality (E172).
    #[test]
    fn the_rate_identity_is_only_truncation() {
        fn permille(hold_ms: u32, hold_acc: u32) -> u32 {
            let r = RunReport {
                closed_us: hold_ms * 1_000,
                hold_us: hold_ms * 1_000,
                hold_acc,
                ..RunReport::default()
            };
            let t = text(|b| r.emit(b));
            let line = t.lines().find(|l| l.starts_with("BEMFRATE ")).unwrap();
            let tok = line
                .split_whitespace()
                .find(|k| k.starts_with("zc_rate_permille_of_expected="))
                .unwrap();
            tok.split('=').nth(1).unwrap().parse().unwrap()
        }
        // 20774 ms / 256577 accepts = 80.97 µs a sector: floor 80, 80/80.97.
        assert_eq!(permille(20_774, 256_577), 988);
        // 20774 ms / 259_573 accepts = 80.03 µs: the same loop, nearly no
        // truncation, and the metric jumps 11 permille.
        assert_eq!(permille(20_774, 259_573), 999);
        // An exact multiple reads perfect, whatever the loop did.
        assert_eq!(permille(20_000, 250_000), 1000);
    }

    /// The fixture's parse of a report this module wrote: every quantity the
    /// cohort judges must come back out of the text.
    #[test]
    fn report_carries_the_cohort_quantities() {
        let r = RunReport {
            reason: Some(Reason::SegmentDeadline),
            target_tenths: 250,
            accepted: 252_493,
            ci_us: 146,
            bus_ref: 1216,
            bus_min: 1101,
            closed_us: 38_776_000,
            hold_us: 31_276_000,
            hold_acc: 216_700,
            hold_ci_sum: 31_000_000,
            roots: Roots {
                zc_accepted: 252_494,
                too_early: 362_699,
                unstable: 972_842,
                ..Roots::default()
            },
            current: CurrentRecord {
                hold_ma: 366,
                // The mA scale this fixture's figures are in (E287).
                current_allow: crate::protection::RAW_LIMIT,
                zero_start: 617_714,
                zero_end: Some(616_029),
                ..CurrentRecord::default()
            },
            tail: Some(TailWindow {
                accepts: 14_382,
                span_us: 2_000_181,
                start_before_stop_us: 2_400_000,
                end_before_stop_us: 399_819,
            }),
            ..RunReport::default()
        };
        let t = text(|b| r.emit(b));
        assert!(t.starts_with("BEMFDONE reason=2 accepted=252493 forced=0 zc_acc=252494 too_early=362699 unstable=972842 ci_us=146 bus_ref=1216 bus_min=1101 \r\n"));
        assert!(t.contains("BEMFGATE closed_ms=38776 hold_ms=31276 target_tenths=250 zc_per_s=6511 "));
        assert!(t.contains("BEMFRATE hold_ms=31276 hold_accepted=216700 hold_forced=0 mean_ci_us=143 mean_sector_us=144 ehz_from_sector=1157 zc_per_s=6928 zc_expected_per_s=6944 zc_rate_permille_of_expected=997 hold_forced_pct=0 \r\n"));
        // Raw counts and spans: the host divides (campaign 8 step 3).
        assert!(t.contains(
            "BEMFTAIL accepts=14382 span_us=2000181 start_before_stop_us=2400000 end_before_stop_us=399819 window_us=2000000 \r\n"
        ));
        assert!(t.contains(" hold_ma=366 zero_blocks=0 "));
        // **The mA scale is emitted beside the figures it governs** (E287).
        // `block_milliamps` divides by `ma_allow`, so the mA fields are
        // milliamps only when it equals `ma_allow_ref` (`RAW_LIMIT`). An
        // `Inject::AverageCurrent` run rebuilds the accumulator at
        // `RAW_LIMIT/100`, which made every mA field in that capture 100x
        // inflated with nothing saying so -- `e280-prot500-i` reads
        // `worst_ma=184352` where the truth is ~1840 mA.
        assert!(t.contains(" ma_allow=31857 ma_allow_ref=31857 "));
        assert!(t.contains(" zero_start=617714 zero_end=616029 "));
        // E187: the ceiling and the worst block, on the same line. A run whose
        // ceiling fell below its commanded duty did not hold the rung, and
        // until E187 the report could not say so.
        // E208: the causal-side fields, on the line the fixture parses.
        assert!(t.contains(" ci_min_us="));
        assert!(t.contains(" thin_count="));
        assert!(t.contains(" hold_unstable="));
        assert!(t.contains(" rebase="));
        assert!(t.contains(" ref_ma=326 duty_tenths=250 ceiling_tenths="));
        assert!(t.contains(" worst_residual="));
        assert!(t.contains(" worst_ma="));
        assert!(t.contains(" worst_hold_ma="));
        assert!(t.contains(" drive_scans="));
        assert!(t.contains(" applied_cap="));
        assert!(t.contains(" applied_ccr="));
        assert!(t.contains(" dep0_pm="));
        assert!(t.contains(" dep0_n="));
        assert!(t.contains(" dep0_run="));
        assert!(t.contains(" dep3_pm="));
        assert!(t.contains(" dep3_n="));
        assert!(t.contains(" dep3_run="));
        assert!(t.contains("driven_rotation=0 \nvsenc_min=0 "));
        assert!(!t.contains("BEMFINJECT"));
    }

    #[test]
    fn an_injection_names_what_it_provoked() {
        let r = RunReport {
            reason: Some(Reason::Tracking),
            inject: Some(InjectOutcome {
                expected_reason: 8,
                guard_reason: 8,
                fired_at: Some(1_000),
                stopped_at: 1_541,
            }),
            ..RunReport::default()
        };
        let t = text(|b| r.emit(b));
        assert!(t.starts_with(
            "BEMFINJECT expected_reason=8 reason=8 guard_reason=8 fired=1 stop_after_inject_us=541 provoked=1 \r\n"
        ));
    }
}
