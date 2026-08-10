//! Firmware self-monitoring (feature = "monitor").
//!
//! THE PRINCIPLE: key control values are streamed through ratch22 online
//! banks and continuously checked for "surprises" — samples outside the
//! value's own learned distribution. The board carries its own verdict
//! instead of shipping a fat high-rate stream to a host.
//!
//! COST POLICY: runs in MAIN context, fed from the loop — zero work added to
//! the prio-0 commutation/COMP ISRs. Every update is DWT-bracketed so the
//! in-situ cost is a measured number, not a guess.
//!
//! TWO RUNTIME DIALS (set live over UART, no reflash):
//!   * TIER  — 0=off, 1=lite (min/max + EW mean/var + surprise band on the
//!             interval AND current channels), 2=full (adds per-epoch block-M4
//!             shape on the interval). This is the CPU-budget dial. Key 'm'.
//!   * K     — surprise band width (|x−mean| > K·σ). Sensitivity dial. 'k'/'K'.
//!
//! ratch22 is float-free here and cross-compiles for thumbv6m, so this lifts
//! unchanged to the M0 rm32 targets (F0/G0).

use core::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, Ordering};
use cortex_m::peripheral::DWT;
use ratch22::{
    BlockAccumulatorWidth, BlockMomentEnvelope, BlockMomentEnvelopeRequest, BlockPowerScale,
    BoundedBlockMoments, CheckedOverflow, ConfigurationIdentity, FixedBlockM4Bank, FractionalBits,
    HistogramPlan, HistogramPlanRequest, Hysteresis, I32OrdinalSlope, I32Q, I64OutputQ, Nearest,
    NoOnlineFeature, OnlineBuilder, OnlineEwMoments, OnlineMinMax, P2Median, P2Quantile,
    PhysicalMagnitude, PhysicalRange, SaturatingOverflow, SelectedOnlineBank, ShiftHorizon,
    WideBlockI128, WindowContext, WindowIdentity, WindowRetunedHistogram,
    calculate_block_moment_envelope_for, calculate_histogram_plan,
};

/// Skip surprise checks until the EW estimate has warmed up (per channel).
const WARMUP: u32 = 128;
/// Fractional bits for the minimal-bank channel scaling.
const F: u32 = 8;
/// Block-shape epoch length (samples per skew/kurtosis emission).
const EPOCH: u32 = 256;
/// Shape input gain: (ci−mean) ticks → Q16.16, ~0.01/tick (shape is
/// scale-invariant; this just lands typical deviations inside the ±2.0 bound).
const SHAPE_GAIN: i32 = 655;

// --- onboard analytics (full tier, current channel) -------------------------
/// Short trend window (commutations) — the fast drift-vs-ripple split.
const WIN_SHORT: u32 = 64;
/// Long trend / P² / histogram window (commutations).
const WIN_LONG: u32 = 256;
/// Current histogram bins.
const HIST_BINS: usize = 8;

type Scale = I32Q<F>;
type Horizon = ShiftHorizon<5>; // alpha = 1/32

/// Minimal per-channel bank: extrema + EW mean & variance.
type MinBank = SelectedOnlineBank<
    Scale,
    SaturatingOverflow,
    OnlineMinMax,
    OnlineEwMoments<Horizon>,
    NoOnlineFeature,
    NoOnlineFeature,
    NoOnlineFeature,
>;

// Interval-shape block-M4 config DERIVED by the ratch22 range→Q calculator
// (PR#20 calculate_block_moment_envelope_for) from a declared ±2.0 physical
// range — replaces the earlier hand-picked WideQ32BlockMoments + guessed
// bounds. const-eval PROVES arithmetic representability at COMPILE time (the
// exact thing friction-log #1 asked for; a bad range now fails the build, not
// a runtime try_new). Wide accumulator: low-variance shape needs var² headroom.
const SHAPE_RANGE: PhysicalRange = match PhysicalRange::try_new(-2, 2, 1) {
    Ok(r) => r,
    Err(_) => panic!("shape physical range"),
};
const SHAPE_PLAN: BlockMomentEnvelope = match calculate_block_moment_envelope_for(
    BlockMomentEnvelopeRequest {
        physical_range: SHAPE_RANGE,
        sample_fractional_bits: FractionalBits::exact(16),
        power_fractional_bits: FractionalBits::between(16, 30),
        maximum_samples: EPOCH,
        moment_order: 4,
    },
    BlockAccumulatorWidth::I64TermsI128Sums,
) {
    Ok(p) => p,
    Err(_) => panic!("shape block envelope must fit"),
};
type ShapeConfig = BoundedBlockMoments<
    WideBlockI128,
    BlockPowerScale<{ SHAPE_PLAN.power_fractional_bits }>,
    Nearest,
    I64OutputQ<32>,
    { SHAPE_PLAN.sample.maximum_absolute_raw },
    { SHAPE_PLAN.maximum_samples },
>;
/// Wide-Q32 block-M4 shape bank (division-free; wide holds low-variance var²).
type ShapeBank = FixedBlockM4Bank<I32Q<16>, CheckedOverflow, ShapeConfig>;

// ---- runtime dials ---------------------------------------------------------
/// Surprise sensitivity K (band = |x−mean| > K·σ). 'k' down / 'K' up.
pub static K: AtomicU32 = AtomicU32::new(3);
/// Tier: 0=off, 1=lite, 2=full. 'm' cycles.
pub static TIER: AtomicU8 = AtomicU8::new(1);

// ---- published readouts (main writes, print_info reads) --------------------
pub static SAMPLES: AtomicU32 = AtomicU32::new(0);
pub static LAST_CYC: AtomicU32 = AtomicU32::new(0);
pub static MIN_CYC: AtomicU32 = AtomicU32::new(u32::MAX);
// interval channel
pub static CI_SURP: AtomicU32 = AtomicU32::new(0);
pub static CI_MEAN_TICKS: AtomicI32 = AtomicI32::new(0);
pub static CI_VAR_TICKS2: AtomicI32 = AtomicI32::new(0);
// current channel
pub static CUR_SURP: AtomicU32 = AtomicU32::new(0);
pub static CUR_MEAN: AtomicI32 = AtomicI32::new(0);
pub static CUR_VAR: AtomicI32 = AtomicI32::new(0);
// full-tier interval shape (milli-units: value×1000)
pub static SKEW_M: AtomicI32 = AtomicI32::new(0);
pub static KURT_M: AtomicI32 = AtomicI32::new(0);
pub static EPOCHS: AtomicU32 = AtomicU32::new(0);
// onboard analytics readouts (current channel)
pub static AN_WIN: AtomicU32 = AtomicU32::new(0);
pub static AN_TREND_SHORT_M: AtomicI32 = AtomicI32::new(0); // slope×1000 (cnt/samp)
pub static AN_TREND_LONG_M: AtomicI32 = AtomicI32::new(0);
pub static AN_P50: AtomicI32 = AtomicI32::new(0); // current median (raw counts)
pub static AN_P90: AtomicI32 = AtomicI32::new(0); // current p90
pub static AN_HIST_EPOCH: AtomicU32 = AtomicU32::new(0); // window-retune config epoch
pub static AN_HIST: [AtomicU32; HIST_BINS] = [const { AtomicU32::new(0) }; HIST_BINS];

/// One minimal-bank channel: update + surprise band.
struct Chan {
    bank: MinBank,
    n: u32,
}

impl Chan {
    fn new() -> Self {
        let bank = OnlineBuilder::<Scale, SaturatingOverflow>::new()
            .min_max()
            .ewma::<Horizon>()
            .variance()
            .build()
            .expect("monitor: channel bank config");
        Self { bank, n: 0 }
    }

    /// Feed `x` (Q(F)); returns (mean Q(F), var Q(F), surprised).
    fn update(&mut self, x: i32, k2: i64) -> (i32, i32, bool) {
        let _ = self.bank.update(x);
        self.n += 1;
        if let Ok(s) = self.bank.snapshot() {
            let (mean, var) = (s.moments.mean, s.moments.variance);
            // |x−mean|² > K²·σ²  ⇔  (x−mean)² > K²·var·2^F   (all Q(F))
            let surprised = self.n > WARMUP && var > 0 && {
                let d = (x - mean) as i64;
                d * d > (k2 * var as i64) << F
            };
            (mean, var, surprised)
        } else {
            (0, 0, false)
        }
    }
}

// Per-channel policy choices (written down per the API-hardening ask):
//  * Trend: CheckedOverflow — the i128 sufficient-stat accumulator can't
//    overflow at these ranges/windows, so "checked" is free insurance and
//    transactional (a surprise sample is dropped, not corrupting the fit).
//  * Histogram: SaturatingOverflow — a full bin should saturate + flag quality,
//    never REJECT the sample (a monitor must not silently drop a count).
//  * P²: no policy knob (internally reject-on-overflow); f32 update, so it runs
//    in MAIN, never the prio-0 ISR (no-integer-variant friction, F0-relevant).
// Q-envelope: trend/hist take RAW i32, P² takes f32 — none need the (missing)
// range→Q calculator; that gap only bites the OnlineBank block-scaled path.
type Trend = I32OrdinalSlope<f32, CheckedOverflow>;
type P50 = P2Median<f32>;
type P90 = P2Quantile<f32, 9, 10>;
// Current histogram configured by ratch22's const histogram planner (PR#21 —
// friction #2 resolved). Operational range declared in raw ADC counts;
// Hysteresis damps the per-window epoch churn Margin<4> showed (145→305 every
// window) — the deadband is the damper the friction log asked for.
const CUR_OP_RANGE: PhysicalRange = match PhysicalRange::try_new(0, 96, 1) {
    Ok(r) => r,
    Err(_) => panic!("current op range"),
};
const CUR_HIST_PLAN: HistogramPlan<HIST_BINS> =
    match calculate_histogram_plan(HistogramPlanRequest {
        operational_range: CUR_OP_RANGE,
        fractional_bits: 0, // raw ADC counts, no Q
        margin: PhysicalMagnitude::integer(4),
        hysteresis_deadband: PhysicalMagnitude::integer(8),
    }) {
        Ok(p) => p,
        Err(_) => panic!("current histogram plan must fit"),
    };
type CurHist = WindowRetunedHistogram<
    Hysteresis<{ CUR_HIST_PLAN.margin_raw }, { CUR_HIST_PLAN.hysteresis_deadband_raw }>,
    SaturatingOverflow,
    HIST_BINS,
>;

/// Onboard streaming analytics on the current channel: short/long trend, P²
/// median+p90, and a window-retuned histogram whose config epoch is published
/// through a frame. Fed per-commutation from main.
struct Analytics {
    trend_short: Trend,
    trend_long: Trend,
    p50: P50,
    p90: P90,
    hist: CurHist,
    short_fill: u32,
    long_fill: u32,
    win_id: u64,
}

impl Analytics {
    fn new() -> Self {
        Self {
            trend_short: Trend::new(),
            trend_long: Trend::new(),
            p50: P50::try_new().expect("monitor: p50"),
            p90: P90::try_new().expect("monitor: p90"),
            hist: CUR_HIST_PLAN
                .try_window_retuned_histogram()
                .expect("monitor: current hist"),
            short_fill: 0,
            long_fill: 0,
            win_id: 0,
        }
    }

    fn feed(&mut self, cur: u16) {
        let x = cur as i32;
        let _ = self.trend_short.update(x);
        let _ = self.trend_long.update(x);
        let _ = self.hist.update(x);
        let f = cur as f32;
        let _ = self.p50.update(f);
        let _ = self.p90.update(f);

        self.short_fill += 1;
        if self.short_fill >= WIN_SHORT {
            if let Ok(s) = self.trend_short.snapshot() {
                AN_TREND_SHORT_M.store((s.output().slope * 1000.0) as i32, Ordering::Relaxed);
            }
            self.trend_short.clear();
            self.short_fill = 0;
        }

        self.long_fill += 1;
        if self.long_fill >= WIN_LONG {
            if let Ok(s) = self.trend_long.snapshot() {
                AN_TREND_LONG_M.store((s.output().slope * 1000.0) as i32, Ordering::Relaxed);
            }
            if let Ok(s) = self.p50.snapshot() {
                AN_P50.store(s.estimate() as i32, Ordering::Relaxed);
            }
            if let Ok(s) = self.p90.snapshot() {
                AN_P90.store(s.estimate() as i32, Ordering::Relaxed);
            }
            // Publish the histogram counts + its config epoch VIA THE FRAME path
            // (epochs surfaced in frames). Copy counts out before retune mutates.
            let snap = self.hist.snapshot();
            let counts = *snap.histogram().counts();
            let samples = snap.histogram().samples();
            let epoch = match WindowContext::new(WindowIdentity(self.win_id), samples) {
                Some(wc) => match snap.frame(wc, ConfigurationIdentity(0)) {
                    Ok(fr) => fr.context().configuration_epoch().map(|e| e.0).unwrap_or(0),
                    Err(_) => snap.configuration_epoch().0,
                },
                None => snap.configuration_epoch().0,
            };
            for (i, c) in counts.iter().enumerate() {
                AN_HIST[i].store(*c, Ordering::Relaxed);
            }
            AN_HIST_EPOCH.store(epoch as u32, Ordering::Relaxed);
            AN_WIN.fetch_add(1, Ordering::Relaxed);
            // roll the window
            let _ = self.hist.retune();
            self.p50.clear();
            self.p90.clear();
            self.trend_long.clear();
            self.long_fill = 0;
            self.win_id = self.win_id.wrapping_add(1);
        }
    }
}

pub struct Monitor {
    interval: Chan,
    current: Chan,
    shape: ShapeBank,
    analytics: Analytics,
    shape_fill: u32,
    last_seq: u32,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            interval: Chan::new(),
            current: Chan::new(),
            shape: ShapeBank::try_new().expect("monitor: shape bank config"),
            analytics: Analytics::new(),
            shape_fill: 0,
            last_seq: u32::MAX,
        }
    }

    /// Call every main iteration. `seq` must be MONOTONIC per commutation
    /// (`zct.comm_n`). Feeds both channels on each new sample; the block-M4
    /// interval shape runs only at TIER≥2.
    pub fn poll(&mut self, seq: u32, commutation_interval: u32, current_raw: u16) {
        if seq == self.last_seq {
            return;
        }
        self.last_seq = seq;
        let tier = TIER.load(Ordering::Relaxed);
        if tier == 0 {
            return;
        }
        let start = DWT::cycle_count();
        let k = K.load(Ordering::Relaxed) as i64;
        let k2 = k * k;

        // interval channel
        let ci_x = (commutation_interval as i32) << F;
        let (im, iv, isurp) = self.interval.update(ci_x, k2);
        CI_MEAN_TICKS.store(im >> F, Ordering::Relaxed);
        CI_VAR_TICKS2.store(iv >> F, Ordering::Relaxed);
        if isurp {
            CI_SURP.fetch_add(1, Ordering::Relaxed);
        }

        // current channel
        let (cm, cv, csurp) = self.current.update((current_raw as i32) << F, k2);
        CUR_MEAN.store(cm >> F, Ordering::Relaxed);
        CUR_VAR.store(cv >> F, Ordering::Relaxed);
        if csurp {
            CUR_SURP.fetch_add(1, Ordering::Relaxed);
        }

        SAMPLES.fetch_add(1, Ordering::Relaxed);

        // full tier: per-epoch interval shape (division-free block-M4).
        if tier >= 2 {
            let dev = (commutation_interval as i32) - (im >> F);
            let xs = dev.saturating_mul(SHAPE_GAIN).clamp(-131_071, 131_071);
            let _ = self.shape.update(xs);
            self.shape_fill += 1;
            if self.shape_fill >= EPOCH {
                match self.shape.snapshot() {
                    Ok(sh) => {
                        SKEW_M.store(q32_milli(sh.population_skewness), Ordering::Relaxed);
                        KURT_M.store(q32_milli(sh.population_excess_kurtosis), Ordering::Relaxed);
                    }
                    Err(_) => {
                        SKEW_M.store(0, Ordering::Relaxed);
                        KURT_M.store(0, Ordering::Relaxed);
                    }
                }
                EPOCHS.fetch_add(1, Ordering::Relaxed);
                self.shape.reset();
                self.shape_fill = 0;
            }
            // onboard streaming analytics on the current channel
            self.analytics.feed(current_raw);
        }

        let cost = DWT::cycle_count().wrapping_sub(start);
        LAST_CYC.store(cost, Ordering::Relaxed);
        MIN_CYC.fetch_min(cost, Ordering::Relaxed);
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Q32 i64 -> milli-units i32 (value×1000).
fn q32_milli(v: i64) -> i32 {
    (v.saturating_mul(1000) >> 32) as i32
}

/// Handle a monitor control key ('k'/'K'/'m'). Any change resets MIN_CYC so
/// the cost floor reflects the new setting.
pub fn key(b: u8) {
    match b {
        b'k' => {
            let v = K.load(Ordering::Relaxed);
            if v > 1 {
                K.store(v - 1, Ordering::Relaxed);
            }
        }
        b'K' => {
            let v = K.load(Ordering::Relaxed);
            if v < 12 {
                K.store(v + 1, Ordering::Relaxed);
            }
        }
        b'm' => {
            let t = TIER.load(Ordering::Relaxed);
            let next = match t {
                1 => 2,
                2 => 0,
                _ => 1,
            };
            TIER.store(next, Ordering::Relaxed);
        }
        _ => return,
    }
    MIN_CYC.store(u32::MAX, Ordering::Relaxed);
}
