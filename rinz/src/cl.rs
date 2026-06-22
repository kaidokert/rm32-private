//! Portable, OBSERVE-ONLY closed-loop ZC tracker (Stage 1b of CLOSED_LOOP_PLAN.md).
//!
//! This is the *identical* code the firmware ISR runs and the host harness
//! (`tests/cl_replay.rs`) replays captured frames through — no host/firmware
//! divergence. It STEERS NOTHING: it observes the valley BEMF, detects the
//! zero-crossing in each float window, and runs a PI-filtered, outlier-gated tracker
//! that estimates the per-commutation ZC position (the load angle). The Stage-1b gate
//! asks whether that *filtered* estimate tracks the offline multi-harmonic oracle
//! despite the raw detector's ~40% mis-lock rate (the `skunk` failure mode).
//!
//! Raw per-commutation ZC detection is inherently noisy (it latches onto demag, a
//! second crossing, or noise). A real sensorless loop never trusts a single crossing;
//! it filters them. Here: an interpolated sign-change detector feeds a PI loop filter
//! (the crate's existing tested `PLL` block, used as a clamped PI tracker on the ZC
//! %-position) gated by an outlier check, so a single bad detection can't move the
//! estimate. Numeric type is f32 (the G431 has an FPU).

use crate::pll_controller::{PLL, PLLParamsPlain};
use crate::pll_state::PLLState;

/// Driven-high / driven-low channel index per physical six-step sector (channel idx
/// == phase idx: 0=A, 1=B, 2=C; the third is the floating phase). Matches scope1/
/// scope_cl's drive tables and scope_common.SIX_STEP_HIGH/LOW.
pub const SIX_HIGH: [usize; 6] = [0, 0, 1, 1, 2, 2];
pub const SIX_LOW: [usize; 6] = [1, 2, 2, 0, 0, 1];

/// `float - driven_pair_neutral` for the BEMF triple at the given physical sector --
/// the exact quantity the offline oracle fits. Shared so host and firmware agree.
#[inline]
pub fn float_minus_neutral(bemf: [i32; 3], phys: usize) -> i32 {
    let hi = SIX_HIGH[phys % 6];
    let lo = SIX_LOW[phys % 6];
    let fl = 3 - hi - lo;
    bemf[fl] - (bemf[hi] + bemf[lo]) / 2
}

/// Interpolated in-window ZC detector. Per float window: blank the first `blank`
/// frames (commutation/demag), establish the entry sign, then on the first opposite
/// sign linearly interpolate the sub-frame zero between the bracketing samples.
/// Reports the crossing as a percentage of the 60-degree window (0..100), or None.
pub struct Detector {
    blank: u32,
    frame: u32,
    entry_sign: i32,
    prev_e: i32,
    prev_frame: u32,
    found: Option<f32>,       // crossing as % of the window
    found_frame: Option<f32>, // crossing as a sub-frame index (ticks since sector start)
}

impl Detector {
    pub const fn new(blank: u32) -> Self {
        Self {
            blank,
            frame: 0,
            entry_sign: 0,
            prev_e: 0,
            prev_frame: 0,
            found: None,
            found_frame: None,
        }
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.entry_sign = 0;
        self.prev_e = 0;
        self.prev_frame = 0;
        self.found = None;
        self.found_frame = None;
    }

    /// Push one frame's `float - neutral`. `fps` = frames per 60-degree window.
    pub fn push(&mut self, e: i32, fps: f32) {
        self.frame += 1;
        if self.frame <= self.blank {
            self.prev_e = e;
            self.prev_frame = self.frame;
            return;
        }
        let s = e.signum();
        if self.entry_sign == 0 {
            if s != 0 {
                self.entry_sign = s;
            }
            self.prev_e = e;
            self.prev_frame = self.frame;
            return;
        }
        if self.found.is_none() && s != 0 && s != self.entry_sign {
            // linear interpolation of the zero between (prev_frame, prev_e) and (frame, e)
            let pe = self.prev_e.unsigned_abs() as f32;
            let ce = e.unsigned_abs() as f32;
            let denom = pe + ce;
            let sub = if denom > 0.0 { pe / denom } else { 0.0 };
            let zero_frame = self.prev_frame as f32 + sub * (self.frame - self.prev_frame) as f32;
            self.found_frame = Some(zero_frame);
            self.found = Some((zero_frame / fps * 100.0).clamp(0.0, 100.0));
        }
        self.prev_e = e;
        self.prev_frame = self.frame;
    }

    /// Crossing as a percentage of the 60-degree window (0..100), or None.
    pub fn finish(&self) -> Option<f32> {
        self.found
    }

    /// Crossing as a sub-frame index (ticks since the sector began), or None.
    /// Used by the closed loop for commutation timing (no %-of-window conversion).
    pub fn finish_frame(&self) -> Option<f32> {
        self.found_frame
    }
}

/// One per-commutation tracker output.
#[derive(Clone, Copy, Debug)]
pub struct ClSample {
    pub phys: u8,
    pub zc_raw: Option<f32>, // detector output, % window (None = no in-window crossing)
    pub zc_track: f32,       // PI-filtered estimate, % window (what control would use)
    pub accepted: bool,      // measurement passed the outlier gate and updated the filter
}

/// Observe-only ZC tracker: interpolated detector + outlier-gated PI filter on the
/// ZC %-position. Warm-started at window centre (50%). STEERS NOTHING.
pub struct ClTracker {
    pll: PLL<f32, PLLParamsPlain<f32>>,
    state: PLLState<f32>,
    detector: Detector,
    gate: f32, // reject a raw ZC further than this (% window) from the current estimate
    fps: f32,  // frames per 60-degree window (from the commanded electrical frequency)
}

impl ClTracker {
    /// `fps` = frames per window = sample_hz / (hz*6). `kp`/`ki` tune the PI filter;
    /// `gate` is the outlier rejection band (% window). `blank` = demag skip frames.
    pub fn new(fps: f32, kp: f32, ki: f32, gate: f32, blank: u32) -> Self {
        Self {
            pll: PLL::new(PLLParamsPlain::new(kp, ki, 0.0, 100.0)),
            state: PLLState::new(50.0), // warm-start at window centre
            detector: Detector::new(blank),
            gate,
            fps,
        }
    }

    /// Feed one valley frame (3 BEMF channels) for the current physical sector.
    pub fn push_frame(&mut self, bemf: [i32; 3], phys: usize) {
        let e = float_minus_neutral(bemf, phys);
        self.detector.push(e, self.fps);
    }

    /// Close the just-ended sector `phys`: finalize the detector, gate the raw ZC,
    /// update the PI filter, and return the tracker sample. Resets the detector.
    pub fn commutate(&mut self, phys: u8) -> ClSample {
        let zc_raw = self.detector.finish();
        let mut accepted = false;
        if let Some(z) = zc_raw {
            let err = z - self.state.frequency; // innovation vs current estimate
            if abs_f32(err) <= self.gate {
                self.pll.update(err, &mut self.state); // PI filter -> state.frequency
                accepted = true;
            }
        }
        self.detector.reset();
        ClSample {
            phys,
            zc_raw,
            zc_track: self.state.frequency,
            accepted,
        }
    }

    /// Current filtered ZC estimate (% window).
    pub fn zc_track(&self) -> f32 {
        self.state.frequency
    }
}

#[inline]
fn abs_f32(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

/// One closed-loop frame result.
#[derive(Clone, Copy, Debug)]
pub struct ClStep {
    pub commutate: bool, // commutation fired this frame (advance the drive sector)
    pub sector: u8,      // current drive sector (0..5)
    pub period_est: f32, // filtered sector period, ticks (omega proxy)
    pub zc_ticks: Option<f32>, // ZC detected this frame at this many ticks since commutation
    pub coasted: bool,   // commutation was forced (no ZC) -- coasting on period_est
}

/// Closed-loop six-step commutation controller -- the steering logic Stage 2 will run
/// on hardware, validated first against the host motor model (tests/cl_sim.rs).
///
/// Per valley frame it detects the BEMF zero crossing, PI-filters the period measured
/// **ZC-to-ZC** (load-angle-independent, unlike a `2*t_zc` estimate), and schedules
/// the next commutation ~30 deg after the ZC. If a ZC is missed it COASTS: commutate
/// at `period_est * coast` so a dropped detection can't stall the loop. The period is
/// warm-started (handover from open-loop at a known frequency).
pub struct ClLoop {
    detector: Detector,
    pll: PLL<f32, PLLParamsPlain<f32>>,
    state: PLLState<f32>, // state.frequency = period estimate (ticks per sector)
    sector: u8,
    ticks: f32, // ticks since the last commutation
    abs: f32,   // monotonic tick counter (for ZC-to-ZC period)
    last_zc_abs: Option<f32>,
    scheduled: Option<f32>, // commutate-at tick (since commutation) once a ZC is seen
    gate_frac: f32,         // reject a period measurement deviating more than this fraction
    coast: f32,             // force commutation at period_est*coast when no ZC is seen
}

impl ClLoop {
    /// `period0` = warm-start sector period in ticks (= sample_hz/(hz*6)). `kp`/`ki`
    /// tune the period PI filter; `gate_frac` rejects wild period measurements;
    /// `coast` (e.g. 1.3) is the missed-ZC timeout; `blank` = demag skip frames.
    pub fn new(period0: f32, kp: f32, ki: f32, gate_frac: f32, coast: f32, blank: u32) -> Self {
        Self {
            detector: Detector::new(blank),
            pll: PLL::new(PLLParamsPlain::new(kp, ki, 2.0, 100_000.0)),
            state: PLLState::new(period0),
            sector: 0,
            ticks: 0.0,
            abs: 0.0,
            last_zc_abs: None,
            scheduled: None,
            gate_frac,
            coast,
        }
    }

    pub fn sector(&self) -> u8 {
        self.sector
    }
    pub fn period_est(&self) -> f32 {
        self.state.frequency
    }

    /// Detector + ZC-to-ZC period update for one frame. Returns `(zc_ticks, cl_target)`
    /// where `cl_target` is the loop's OWN desired commutation tick (since commutation).
    /// Does not commutate.
    fn observe(&mut self, bemf: [i32; 3]) -> (Option<f32>, f32) {
        self.ticks += 1.0;
        self.abs += 1.0;
        let e = float_minus_neutral(bemf, self.sector as usize);
        self.detector.push(e, self.state.frequency.max(1.0));

        let mut zc_ticks = None;
        if self.scheduled.is_none() {
            if let Some(t_zc) = self.detector.finish_frame() {
                zc_ticks = Some(t_zc);
                let zc_abs = self.abs - self.ticks + t_zc; // absolute time of the crossing
                if let Some(last) = self.last_zc_abs {
                    let meas = zc_abs - last; // ZC-to-ZC = one sector period
                    let err = meas - self.state.frequency;
                    if abs_f32(err) <= self.gate_frac * self.state.frequency {
                        self.pll.update(err, &mut self.state);
                    }
                }
                self.last_zc_abs = Some(zc_abs);
                self.scheduled = Some(t_zc + self.state.frequency * 0.5); // 30 deg after ZC
            }
        }
        let cl_target = self.scheduled.unwrap_or(self.state.frequency * self.coast);
        (zc_ticks, cl_target)
    }

    /// Commutate if `ticks` reached `target`; build the step result.
    fn fire(&mut self, target: f32, zc_ticks: Option<f32>) -> ClStep {
        let coasted = self.scheduled.is_none();
        let mut commutate = false;
        if self.ticks >= target {
            commutate = true;
            self.sector = (self.sector + 1) % 6;
            self.ticks = 0.0;
            self.scheduled = None;
            self.detector.reset();
        }
        ClStep {
            commutate,
            sector: self.sector,
            period_est: self.state.frequency,
            zc_ticks,
            coasted: commutate && coasted,
        }
    }

    /// Pure closed-loop frame (host sim): commutate on the loop's own ZC-driven schedule.
    pub fn on_frame(&mut self, bemf: [i32; 3]) -> ClStep {
        let (zc, ct) = self.observe(bemf);
        self.fire(ct, zc)
    }

    /// alpha-BLENDED frame for bounded hardware (Stage 2). Commutate at the open-loop
    /// target `ol_period` (commanded ticks/sector) nudged toward the loop's own target
    /// by `alpha` in [0,1], the nudge clamped to +/- `slew_frac * ol_period`:
    ///   target = ol_period + alpha * clamp(cl_target - ol_period, +/- slew_frac*ol_period)
    /// alpha=0 == pure open-loop (the governor); alpha=1 == loop authority within the
    /// slew band. The frequency stays governed by `ol_period`, so it CANNOT run away --
    /// dropping alpha to 0 is the instant fallback to open-loop.
    pub fn on_frame_blend(
        &mut self,
        bemf: [i32; 3],
        ol_period: f32,
        alpha: f32,
        slew_frac: f32,
    ) -> ClStep {
        let (zc, ct) = self.observe(bemf);
        let lim = slew_frac * ol_period;
        let nudge = (ct - ol_period).clamp(-lim, lim);
        let target = ol_period + alpha.clamp(0.0, 1.0) * nudge;
        self.fire(target, zc)
    }

    /// Force the drive sector (used at handover so the loop's sector matches the
    /// open-loop sector before alpha is raised).
    pub fn set_sector(&mut self, sector: u8) {
        self.sector = sector % 6;
    }

    /// Set the period estimate directly (ticks/sector). Used at alpha=0 to keep the
    /// estimate pinned to the commanded open-loop period, so raising alpha later starts
    /// the loop from a correct period rather than a stale warm-start.
    pub fn set_period(&mut self, period: f32) {
        self.state = PLLState::new(period);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_driven_pair_average() {
        // sector 0: hi=A(0), lo=B(1), float=C(2). neutral=(A+B)/2.
        let e = float_minus_neutral([1600, 0, 900], 0);
        assert_eq!(e, 900 - 800); // float 900 - neutral 800 = 100
    }

    #[test]
    fn detector_interpolates_crossing() {
        // fps=10; blank=2. entry sign established at frame 3 (+). Monotonic down,
        // crosses between frame 5 (+10) and frame 6 (-10): zero at 5.5 -> 55%.
        let mut d = Detector::new(2);
        let seq = [50, 40, 30, 20, 10, -10]; // frames 1..6
        for &e in &seq {
            d.push(e, 10.0);
        }
        let zc = d.finish().unwrap();
        assert!((zc - 55.0).abs() < 0.01, "zc={zc}");
    }

    #[test]
    fn detector_none_when_no_crossing() {
        let mut d = Detector::new(2);
        for &e in &[30, 25, 20, 15, 10, 5] {
            d.push(e, 10.0);
        }
        assert!(d.finish().is_none());
    }

    #[test]
    fn tracker_converges_to_constant() {
        let mut t = ClTracker::new(10.0, 0.3, 0.3, 30.0, 0);
        // feed the same in-window crossing (65%) repeatedly; estimate should rise 50->~65
        for _ in 0..40 {
            // synth: entry + sign flip placing zero at 65%
            t.detector.reset();
            for f in 1..=10 {
                let e = if (f as f32) < 6.5 { 100 } else { -100 };
                t.detector.push(e, 10.0);
            }
            t.commutate(0);
        }
        assert!(
            (t.zc_track() - 65.0).abs() < 2.0,
            "zc_track={}",
            t.zc_track()
        );
    }

    #[test]
    fn tracker_rejects_outlier() {
        let mut t = ClTracker::new(10.0, 0.5, 0.5, 15.0, 0);
        // settle near 50
        for _ in 0..20 {
            t.detector.reset();
            for f in 1..=10 {
                let e = if (f as f32) < 5.0 { 100 } else { -100 };
                t.detector.push(e, 10.0);
            }
            t.commutate(0);
        }
        let before = t.zc_track();
        // inject a wild outlier crossing at ~95% (far beyond the 15% gate)
        t.detector.reset();
        for f in 1..=10 {
            let e = if (f as f32) < 9.5 { 100 } else { -100 };
            t.detector.push(e, 10.0);
        }
        let s = t.commutate(0);
        assert!(!s.accepted, "outlier should be rejected");
        assert_eq!(
            t.zc_track(),
            before,
            "estimate must not move on a rejected outlier"
        );
    }

    #[test]
    fn tracker_holds_on_no_detection() {
        let mut t = ClTracker::new(10.0, 0.5, 0.5, 30.0, 2);
        for _ in 0..10 {
            t.detector.reset();
            for f in 1..=10 {
                let e = if (f as f32) < 5.0 { 100 } else { -100 };
                t.detector.push(e, 10.0);
            }
            t.commutate(0);
        }
        let before = t.zc_track();
        // a window with no crossing -> None -> estimate holds
        t.detector.reset();
        for _ in 1..=10 {
            t.detector.push(50, 10.0);
        }
        let s = t.commutate(0);
        assert!(s.zc_raw.is_none());
        assert_eq!(t.zc_track(), before);
    }
}
