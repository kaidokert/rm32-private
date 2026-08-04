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
    CheckedOverflow, FixedBlockM4Bank, I32Q, NoOnlineFeature, OnlineBuilder, OnlineEwMoments,
    OnlineMinMax, SaturatingOverflow, SelectedOnlineBank, ShiftHorizon, WideQ32BlockMoments,
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

/// Wide-Q32 block-M4 shape bank (division-free; wide holds low-variance var²).
type ShapeBank = FixedBlockM4Bank<I32Q<16>, CheckedOverflow, WideQ32BlockMoments<EPOCH>>;

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

pub struct Monitor {
    interval: Chan,
    current: Chan,
    shape: ShapeBank,
    shape_fill: u32,
    last_seq: u32,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            interval: Chan::new(),
            current: Chan::new(),
            shape: ShapeBank::try_new().expect("monitor: shape bank config"),
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
