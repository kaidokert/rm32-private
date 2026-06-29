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

use crate::harmonic::{Harmonic, wrap_pm_pi};
use crate::pll_controller::{PLL, PLLParamsPlain};
use crate::pll_state::PLLState;
use micromath::F32Ext;

/// Decay for the streaming harmonic detector (≈1/e over ~12 float samples ≈ 1 rev/phase).
const CL_HARM_LAM: f32 = 0.92;
const PI_3: f32 = core::f32::consts::PI / 3.0; // 60° = one sector, in electrical radians
// cos/sin of each sector's start angle (sector*60°) -- a LUT so the per-commutation recurrence
// reseed needs no trig for the angle (only the per-tick-rotation cos_d/sin_d do, 2 trig/comm).
const SECTOR_COS: [f32; 6] = [1.0, 0.5, -0.5, -1.0, -0.5, 0.5];
const SECTOR_SIN: [f32; 6] = [
    0.0,
    0.866_025_4,
    0.866_025_4,
    0.0,
    -0.866_025_4,
    -0.866_025_4,
];

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
    // running least-squares of post-blank (frame, e): lets finish_linfit() SOLVE for the
    // zero by extrapolating the slope, even when the float window sits on a shallow part
    // of the BEMF and the raw signal never crosses (the common case -- see cl notes).
    fn_n: f32,
    fn_sx: f32,
    fn_sy: f32,
    fn_sxy: f32,
    fn_sxx: f32,
    // Run the least-squares accumulation (for finish_linfit) or skip it. Skipped when the
    // loop uses the sign-change crossing (finish_frame) -- the LSQ work is then dead weight,
    // and dropping it shaves the SAME few cycles off EVERY tick (uniform, no spike).
    lsq: bool,
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
            fn_n: 0.0,
            fn_sx: 0.0,
            fn_sy: 0.0,
            fn_sxy: 0.0,
            fn_sxx: 0.0,
            lsq: true,
        }
    }

    /// Enable/disable the least-squares accumulation. Off when the loop uses the sign-change
    /// detector (finish_frame), so the unused LSQ math is dropped uniformly from every tick.
    pub fn set_lsq(&mut self, on: bool) {
        self.lsq = on;
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.entry_sign = 0;
        self.prev_e = 0;
        self.prev_frame = 0;
        self.found = None;
        self.found_frame = None;
        self.fn_n = 0.0;
        self.fn_sx = 0.0;
        self.fn_sy = 0.0;
        self.fn_sxy = 0.0;
        self.fn_sxx = 0.0;
    }

    /// Push one frame's `float - neutral`. `fps` = frames per 60-degree window.
    pub fn push(&mut self, e: i32, fps: f32) {
        self.frame += 1;
        if self.frame <= self.blank {
            self.prev_e = e;
            self.prev_frame = self.frame;
            return;
        }
        // running least-squares accumulation of post-blank (frame, e) -- only when the linfit
        // detector is in use; skipped uniformly (every tick) under the sign-change detector.
        if self.lsq {
            let fx = self.frame as f32;
            let fy = e as f32;
            self.fn_n += 1.0;
            self.fn_sx += fx;
            self.fn_sy += fy;
            self.fn_sxy += fx * fy;
            self.fn_sxx += fx * fx;
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

    /// Crossing SOLVED from a least-squares line through the post-blank samples
    /// (ticks since sector start), extrapolating the slope so it works even when the
    /// float window sits on a shallow part of the BEMF and never actually crosses.
    /// None if too few points or a flat slope. The caller gates on plausible position.
    pub fn finish_linfit(&self) -> Option<f32> {
        if self.fn_n < 4.0 {
            return None;
        }
        let denom = self.fn_n * self.fn_sxx - self.fn_sx * self.fn_sx;
        if abs_f32(denom) < 1e-3 {
            return None;
        }
        let m = (self.fn_n * self.fn_sxy - self.fn_sx * self.fn_sy) / denom;
        if abs_f32(m) < 1e-3 {
            return None;
        }
        let b = (self.fn_sy - m * self.fn_sx) / self.fn_n;
        Some(-b / m) // zero-crossing frame (may lie outside [0, window])
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
    // Streaming harmonic detector -- OBSERVE-ONLY for now: it accumulates each tick and its
    // crossing is exposed via harm_zc() for telemetry/vetting on hardware, but it does NOT yet
    // drive commutation (the per-sector sign-change/linfit still does). Per the project's
    // observe-before-control rule: prove it recovers crossings on real BEMF before it steers.
    harmonic: Harmonic,
    harm_zc: Option<f32>, // last harmonic crossing, ticks since this sector's commutation
    // Incremental rotation for the electrical angle's cos/sin -- avoids 2 micromath trig calls
    // EVERY tick (which overran the ISR). cos_t/sin_t advance by (cos_d, sin_d) per tick (4 mul
    // + 2 add); the trig is recomputed only once per commutation (fire()).
    cos_t: f32,
    sin_t: f32,
    cos_d: f32,
    sin_d: f32,
    pll: PLL<f32, PLLParamsPlain<f32>>,
    state: PLLState<f32>, // state.frequency = period estimate (ticks per sector)
    sector: u8,
    ticks: f32, // ticks since the last commutation
    abs: f32,   // monotonic tick counter (for ZC-to-ZC period)
    last_zc_abs: Option<f32>,
    scheduled: Option<f32>, // commutate-at tick (since commutation) once a ZC is seen
    gate_frac: f32,         // reject a period measurement deviating more than this fraction
    coast: f32,             // force commutation at period_est*coast when no ZC is seen
    predict_coast: bool,    // on a miss, schedule from per-sector ZC memory vs generic timeout
    predict_gate: f32,      // engage predictive coast only once lock_fast exceeds this. Default
    // 0.5, but the sparse sign-change detector ceilings lock_fast at
    // ~2/6 = 0.33, so 0.5 never engages -> tunable to test lower gates.
    zc_filt: [Option<f32>; 6], // per-sector EMA of the ZC position (ticks since commutation)
    zc_beta: f32,              // EMA weight on history; 0.0 = no filtering (raw per-sector ZC)
    // Lock-quality telemetry (updated once per commutation). Two IIR time constants so the
    // host can SEE how marginal the lock is rather than relying on the ear: lock_* = ZC-hit
    // fraction (0..1), jit_* = |ZC - per-sector-smoothed| residual in ticks (jitter even
    // when the hit rate is 100%). k_fast ~ a few revs, k_slow ~ a couple seconds.
    last_residual: Option<f32>,
    lock_fast: f32,
    lock_slow: f32,
    jit_fast: f32,
    jit_slow: f32,
    k_fast: f32,
    k_slow: f32,
    // Long-running glitch monitor (cumulative since reset_glitch). The steady ~4% misses
    // are inaudible; the audible clicks are RARE (seconds apart) larger events. So count
    // two event classes that a single capture can't catch: coast BURSTS (>= glitch_run
    // consecutive missed ZCs = a momentary lock loss) and big-RESIDUAL hits (a found ZC
    // that jumped > glitch_resid ticks from its smoothed position = a phase glitch).
    comm_total: u32,       // commutations since reset
    coast_total: u32,      // missed-ZC commutations
    cur_run: u32,          // current consecutive-coast run length
    max_run: u32,          // longest consecutive-coast run seen
    coast_bursts: u32,     // runs that reached glitch_run (counted once per run)
    big_resid_events: u32, // hits with |residual| > glitch_resid
    last_event_comm: u32,  // comm_total at the last glitch event (for spacing)
    glitch_run: u32,       // burst length that counts as a glitch
    glitch_resid: f32,     // residual (ticks) that counts as a phase glitch
    // Histograms compress the high-rate distributions into shape. run_hist[i] = count of
    // completed coast runs of length i+1 (last bin = 8+): separates harmless single misses
    // from the rare deep bursts that are the audible clicks. resid_hist = |ZC residual|
    // buckets over every hit (<0.5, <1, <2, <4, <8, >=8 ticks): tight cluster vs heavy tail.
    run_hist: [u32; 8],
    resid_hist: [u32; 6],
    // Stall detector: latches once the loop has LOST a lock it previously held. Armed only
    // after lock_fast crosses `stall_arm` (so a failure-to-start or the steady ~half-out-of-
    // window per-sector wave -- max ~3 consecutive coasts -- can't trip it); fires when the
    // consecutive-coast run reaches `stall_run` (a sustained loss = rotor stopped / desync).
    stall_armed: bool,
    stalled: bool,
    stall_run: u32,
    stall_arm: f32,
    // Detector source for the loop: false = finish_linfit() (extrapolates a crossing even when
    // the window never crosses -- fabricates one from demag/commutation transients in the
    // out-of-window sectors, the source of the Phase-3 jitter), true = finish_frame() (the
    // straddle-gated sign-change: a crossing ONLY when the float window actually crosses
    // neutral; otherwise None -> the loop coasts cleanly on its per-sector memory). Default
    // false to preserve the sim-validated behaviour; the bench A/B toggles it on to measure.
    use_signchange: bool,
}

impl ClLoop {
    /// `period0` = warm-start sector period in ticks (= sample_hz/(hz*6)). `kp`/`ki`
    /// tune the period PI filter; `gate_frac` rejects wild period measurements;
    /// `coast` (e.g. 1.3) is the missed-ZC timeout; `blank` = demag skip frames.
    pub fn new(period0: f32, kp: f32, ki: f32, gate_frac: f32, coast: f32, blank: u32) -> Self {
        Self {
            detector: Detector::new(blank),
            harmonic: Harmonic::new(CL_HARM_LAM),
            harm_zc: None,
            cos_t: 1.0,
            sin_t: 0.0,
            cos_d: (PI_3 / period0.max(1.0)).cos(),
            sin_d: (PI_3 / period0.max(1.0)).sin(),
            pll: PLL::new(PLLParamsPlain::new(kp, ki, 2.0, 100_000.0)),
            state: PLLState::new(period0),
            sector: 0,
            ticks: 0.0,
            abs: 0.0,
            last_zc_abs: None,
            scheduled: None,
            gate_frac,
            coast,
            predict_coast: true,
            predict_gate: 0.5,
            zc_filt: [None; 6],
            zc_beta: 0.0,
            last_residual: None,
            lock_fast: 0.0,
            lock_slow: 0.0,
            jit_fast: 0.0,
            jit_slow: 0.0,
            k_fast: 0.08,   // ~ 2 electrical revs (12 commutations)
            k_slow: 0.0007, // ~ 2 s at 250 Hz (1500 commutations/s)
            comm_total: 0,
            coast_total: 0,
            cur_run: 0,
            max_run: 0,
            coast_bursts: 0,
            big_resid_events: 0,
            last_event_comm: 0,
            glitch_run: 2,
            glitch_resid: 2.0,
            run_hist: [0; 8],
            resid_hist: [0; 6],
            stall_armed: false,
            stalled: false,
            stall_run: 10,         // ~1.6 electrical revs of all-coast
            stall_arm: 0.30,       // arm once lock_fast exceeds this (open loop settles ~0.5)
            use_signchange: false, // default = linfit (sim-validated); bench toggles sign-change
        }
    }

    /// Detector source for the loop. false = extrapolating linfit (fabricates a crossing in
    /// out-of-window sectors), true = straddle-gated sign-change (clean crossing only when the
    /// window actually crosses; else coast). The bench A/B (cl_engage) toggles this to measure
    /// whether honest-sparse beats noisy-dense for closed-loop jitter.
    pub fn set_use_signchange(&mut self, on: bool) {
        self.use_signchange = on;
        self.detector.set_lsq(!on); // sign-change -> drop the unused LSQ math from every tick
    }

    /// Whether the loop is using the sign-change detector (true) or the linfit (false).
    pub fn use_signchange(&self) -> bool {
        self.use_signchange
    }

    /// Set the per-sector ZC EMA weight (0.0 = raw, no filtering; higher = smoother but
    /// laggier). Smooths each sector's rev-to-rev linfit noise -> less commutation jitter.
    pub fn set_zc_beta(&mut self, beta: f32) {
        self.zc_beta = beta.clamp(0.0, 0.95);
    }

    /// Predictive coast: on a missed detection, schedule from this sector's smoothed ZC
    /// memory instead of the generic open-loop timeout (true = on). Off restores the
    /// dead-reckon-at-period behavior, for an A/B of the residual commutation clips.
    /// Lock-quality threshold above which predictive coast engages (default 0.5). Lower it
    /// to let the per-sector ZC memory bridge gaps in the sparse-detection regime where
    /// lock_fast ceilings below 0.5; too low risks predicting off noisy memory during cold
    /// acquisition. Clamped to [0, 1].
    pub fn set_predict_gate(&mut self, gate: f32) {
        self.predict_gate = gate.clamp(0.0, 1.0);
    }

    pub fn set_predict_coast(&mut self, on: bool) {
        self.predict_coast = on;
    }

    /// Override the lock-quality IIR coefficients (per commutation). `k_fast` ~ few revs,
    /// `k_slow` ~ a couple seconds. Smaller k = longer averaging window.
    pub fn set_lock_filt(&mut self, k_fast: f32, k_slow: f32) {
        self.k_fast = k_fast;
        self.k_slow = k_slow;
    }

    /// Lock-hit fraction (0..1): fraction of recent commutations that found an in-window
    /// ZC (vs dead-reckoned coast). `fast` ~ few revs, slow ~ a couple seconds.
    pub fn lock_fast(&self) -> f32 {
        self.lock_fast
    }
    pub fn lock_slow(&self) -> f32 {
        self.lock_slow
    }
    /// ZC jitter (ticks): smoothed |ZC - per-sector-smoothed-ZC| residual. Nonzero even at
    /// 100% lock when the crossing bounces around -- the marginal-but-firing signature.
    pub fn jit_fast(&self) -> f32 {
        self.jit_fast
    }
    pub fn jit_slow(&self) -> f32 {
        self.jit_slow
    }

    /// Set the glitch-monitor thresholds: `run` = consecutive coasts that count as a burst
    /// event; `resid` = ZC jump (ticks) that counts as a phase-glitch event.
    pub fn set_glitch_thresholds(&mut self, run: u32, resid: f32) {
        self.glitch_run = run.max(1);
        self.glitch_resid = resid;
    }

    /// Zero the long-running glitch counters (start a fresh measurement window).
    pub fn reset_glitch(&mut self) {
        self.comm_total = 0;
        self.coast_total = 0;
        self.cur_run = 0;
        self.max_run = 0;
        self.coast_bursts = 0;
        self.big_resid_events = 0;
        self.last_event_comm = 0;
        self.run_hist = [0; 8];
        self.resid_hist = [0; 6];
    }

    /// Glitch-monitor snapshot: (commutations, coasts, coast-burst events, big-residual
    /// events, longest coast run, commutations since the last glitch event).
    pub fn glitch_stats(&self) -> (u32, u32, u32, u32, u32, u32) {
        (
            self.comm_total,
            self.coast_total,
            self.coast_bursts,
            self.big_resid_events,
            self.max_run,
            self.comm_total.wrapping_sub(self.last_event_comm),
        )
    }

    /// Coast-run-length histogram: index i = count of completed runs of length i+1
    /// (last bin = 8+). Separates harmless single misses from the rare deep bursts.
    pub fn run_hist(&self) -> [u32; 8] {
        self.run_hist
    }

    /// ZC-residual magnitude histogram over hits: buckets <0.5, <1, <2, <4, <8, >=8 ticks.
    pub fn resid_hist(&self) -> [u32; 6] {
        self.resid_hist
    }

    /// True once the loop has LOST a lock it previously held (sustained coast run). Latches
    /// until `reset_stall`. The firmware kills the motor on this edge.
    pub fn stalled(&self) -> bool {
        self.stalled
    }

    /// Set the stall-trip consecutive-coast threshold (commutations). Default 10.
    pub fn set_stall_run(&mut self, run: u32) {
        self.stall_run = run.max(1);
    }

    /// Clear the stall latch and re-disarm (re-arms on the next genuine lock). Call on a
    /// re-spin / restart so the detector starts fresh.
    pub fn reset_stall(&mut self) {
        self.stalled = false;
        self.stall_armed = false;
        self.cur_run = 0;
    }

    pub fn sector(&self) -> u8 {
        self.sector
    }
    pub fn period_est(&self) -> f32 {
        self.state.frequency
    }
    /// Last streaming-harmonic crossing for the current sector's float phase, in ticks since
    /// this sector's commutation (observe-only telemetry; None if the fit has no crossing yet).
    pub fn harm_zc(&self) -> Option<f32> {
        self.harm_zc
    }

    /// Detector + ZC-to-ZC period update for one frame. Returns `(zc_ticks, cl_target)`
    /// where `cl_target` is the loop's OWN desired commutation tick (since commutation).
    /// Does not commutate.
    fn observe(&mut self, bemf: [i32; 3], update_pll: bool) -> (Option<f32>, f32) {
        self.ticks += 1.0;
        self.abs += 1.0;
        let e = float_minus_neutral(bemf, self.sector as usize);
        let period = self.state.frequency.max(1.0);
        self.detector.push(e, period);

        // Streaming harmonic (OBSERVE-ONLY): accumulate this float sample at its electrical
        // angle. cos/sin advance by the per-tick rotation (cos_d, sin_d) -- 4 mul + 2 add, NO
        // per-tick trig (that overran the ISR). fire() reseeds cos_t/sin_t to the sector start
        // and cos_d/sin_d from the period each commutation.
        let fl =
            (3 - SIX_HIGH[self.sector as usize % 6] - SIX_LOW[self.sector as usize % 6]) as usize;
        let (c, s) = (self.cos_t, self.sin_t);
        self.cos_t = c * self.cos_d - s * self.sin_d;
        self.sin_t = s * self.cos_d + c * self.sin_d;
        self.harmonic.push(fl, self.cos_t, self.sin_t, e as f32);

        // Solve the crossing from the line fit (handles windows that never actually
        // cross). Wait until ~half the window so the fit is stable; gate on a plausible
        // position so a wild extrapolation can't hijack the schedule.
        let mut zc_ticks = None;
        if self.scheduled.is_none() && self.ticks >= 0.5 * self.state.frequency {
            // Sign-change (finish_frame) fires ONLY on a real in-window crossing -> None when
            // the window never crosses, so the loop coasts cleanly instead of chasing a
            // fabricated linfit extrapolation. linfit is the legacy default (sim-validated).
            let detected = if self.use_signchange {
                self.detector.finish_frame()
            } else {
                self.detector.finish_linfit()
            };
            if let Some(t_zc) = detected {
                let win = self.state.frequency;
                if t_zc >= -0.3 * win && t_zc <= 1.3 * win {
                    zc_ticks = Some(t_zc);
                    let zc_abs = self.abs - self.ticks + t_zc; // absolute time of the crossing
                    if let Some(last) = self.last_zc_abs {
                        let meas = zc_abs - last; // ZC-to-ZC = one sector period
                        let err = meas - self.state.frequency;
                        if update_pll && abs_f32(err) <= self.gate_frac * self.state.frequency {
                            self.pll.update(err, &mut self.state);
                        }
                    }
                    self.last_zc_abs = Some(zc_abs);
                    // Per-sector EMA on the ZC position: each sector has its own real ZC
                    // offset (the electrical-angle-locked per-sector wave), so we smooth
                    // each sector against ITS OWN history -- killing rev-to-rev linfit
                    // noise (the audible commutation jitter) without blurring the genuine
                    // sector-to-sector structure a global filter would average away.
                    let s = self.sector as usize;
                    let zc_f = match self.zc_filt[s] {
                        Some(p) => {
                            self.last_residual = Some(t_zc - p); // deviation from smoothed
                            self.zc_beta * p + (1.0 - self.zc_beta) * t_zc
                        }
                        None => t_zc,
                    };
                    self.zc_filt[s] = Some(zc_f);
                    self.scheduled = Some(zc_f + self.state.frequency * 0.5); // 30 deg after ZC
                }
            }
        }
        // No ZC scheduled yet (still searching, or this sector will miss). Prefer this
        // sector's smoothed ZC memory so a missed detection fires near its historically-
        // correct phase rather than a generic timeout (which is the right average period
        // but the wrong phase -> the audible per-miss clip). Fall back to the open-loop
        // coast only with no memory yet (acquisition) or when predictive coast is off.
        // Gate predictive coast on being locked (lock_fast > predict_gate): only then is the
        // per-sector memory trustworthy. During acquisition the memory is noise, and
        // predicting off it prevents lock -- so fall back to the open-loop coast there.
        let predict = self.predict_coast && self.lock_fast > self.predict_gate;
        let cl_target = match self.scheduled {
            Some(t) => t,
            None => match (predict, self.zc_filt[self.sector as usize]) {
                (true, Some(zc)) => zc + self.state.frequency * 0.5,
                _ => self.state.frequency * self.coast,
            },
        };
        (zc_ticks, cl_target)
    }

    /// Commutate if `ticks` reached `target`; build the step result.
    fn fire(&mut self, target: f32, zc_ticks: Option<f32>) -> ClStep {
        let coasted = self.scheduled.is_none();
        let mut commutate = false;
        if self.ticks >= target {
            commutate = true;
            // Lock-quality IIRs (once per commutation): hit rate + ZC jitter.
            let hit = if coasted { 0.0 } else { 1.0 };
            self.lock_fast += self.k_fast * (hit - self.lock_fast);
            self.lock_slow += self.k_slow * (hit - self.lock_slow);
            // Long-running glitch monitor (cumulative): coast bursts + big-residual hits.
            self.comm_total = self.comm_total.wrapping_add(1);
            if coasted {
                self.coast_total = self.coast_total.wrapping_add(1);
                self.cur_run += 1;
                if self.cur_run > self.max_run {
                    self.max_run = self.cur_run;
                }
                if self.cur_run == self.glitch_run {
                    self.coast_bursts = self.coast_bursts.wrapping_add(1);
                    self.last_event_comm = self.comm_total;
                }
            } else {
                if self.cur_run > 0 {
                    // a coast run just ended on this hit -- bin its length (1..8+)
                    self.run_hist[(self.cur_run as usize - 1).min(7)] += 1;
                }
                self.cur_run = 0;
            }
            if let Some(r) = self.last_residual.take() {
                let a = abs_f32(r);
                self.jit_fast += self.k_fast * (a - self.jit_fast);
                self.jit_slow += self.k_slow * (a - self.jit_slow);
                // bucket |residual| (ticks): <0.5, <1, <2, <4, <8, >=8
                let b = if a < 0.5 {
                    0
                } else if a < 1.0 {
                    1
                } else if a < 2.0 {
                    2
                } else if a < 4.0 {
                    3
                } else if a < 8.0 {
                    4
                } else {
                    5
                };
                self.resid_hist[b] += 1;
                if a > self.glitch_resid {
                    self.big_resid_events = self.big_resid_events.wrapping_add(1);
                    self.last_event_comm = self.comm_total;
                }
            }
            // Stall detector: arm once the loop has genuinely locked, then latch a stall
            // when it loses that lock for a sustained run (rotor stopped / hard desync).
            if self.lock_fast > self.stall_arm {
                self.stall_armed = true;
            }
            if self.stall_armed && self.cur_run >= self.stall_run {
                self.stalled = true;
            }
            // Harmonic crossing for the JUST-ENDED sector's float phase (observe-only), computed
            // ONCE per commutation -- the 3x3 solve + atan2/asin is far too costly per tick (it
            // overran the ISR budget when run every tick in the detection window). Crossing
            // nearest the sector centre, as ticks since this sector's commutation.
            let fl = (3 - SIX_HIGH[self.sector as usize % 6] - SIX_LOW[self.sector as usize % 6])
                as usize;
            let center = (self.sector as f32 + 0.5) * PI_3;
            self.harm_zc = self
                .harmonic
                .cross_near(fl, center)
                .map(|tc| wrap_pm_pi(tc - self.sector as f32 * PI_3) / PI_3 * self.state.frequency);
            self.sector = (self.sector + 1) % 6;
            self.ticks = 0.0;
            self.scheduled = None;
            self.detector.reset();
            // Reseed the rotation recurrence: cos_t/sin_t to the new sector's start angle from the
            // LUT (exact, no trig, kills accumulated drift); cos_d/sin_d from the current period
            // (2 trig, once per commutation -- not per tick).
            self.cos_t = SECTOR_COS[self.sector as usize];
            self.sin_t = SECTOR_SIN[self.sector as usize];
            let dt = PI_3 / self.state.frequency.max(1.0);
            self.cos_d = dt.cos();
            self.sin_d = dt.sin();
        }
        ClStep {
            commutate,
            sector: self.sector,
            period_est: self.state.frequency,
            zc_ticks,
            coasted: commutate && coasted,
        }
    }

    /// Pure closed-loop frame (host sim): commutate on the loop's own ZC-driven schedule,
    /// with period_est free-running (the PLL tracks the rotor speed).
    pub fn on_frame(&mut self, bemf: [i32; 3]) -> ClStep {
        let (zc, ct) = self.observe(bemf, true);
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
        // Governed mode: the frequency IS the commanded ol_period; the ZC only adjusts
        // PHASE. Pin period_est to ol_period (and don't run the period PLL) so a biased
        // ZC can't run the estimate away -- the runaway that broke the detector window
        // and desynced the loop as alpha rose (caught by cl_alpha_tune).
        self.state.frequency = ol_period;
        self.state.pi.integral = ol_period;
        let (zc, ct) = self.observe(bemf, false);
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
        // NOTE: do NOT reset the harmonic here -- set_period is called EVERY tick at alpha=0
        // (governed) to pin period_est, so resetting would wipe the accumulators each tick and
        // the fit could never build. The exponential decay handles stale data on a re-spin.
    }

    /// Hard-clamp the period estimate (ticks/sector) to [min, max] -- runaway protection for
    /// the full-drive path (`on_frame`). Caps the commanded speed regardless of what the ZC
    /// PLL tracks, and stops PI wind-up past the bound. Cheap: safe to call EVERY tick (uniform
    /// per-tick cost). `on_frame_blend` does not need it (it re-pins period_est = ol_period).
    pub fn clamp_period(&mut self, min: f32, max: f32) {
        self.state.frequency = self.state.frequency.clamp(min, max);
        self.state.pi.integral = self.state.pi.integral.clamp(min, max);
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
