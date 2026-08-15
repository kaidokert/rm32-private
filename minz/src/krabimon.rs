//! krabilorean qualification consumer (feature = "krabimon").
//!
//! Consumes the git-pinned sibling TSFE crate `krabilorean` (annotated tag
//! v0.1.0-alpha.1 = commit 78eac27, resolved from a real remote so this build
//! is reproducible — unlike the local-path `ratch22` monitor) in the SAME
//! onboard role as src/monitor.rs: key control values are streamed through the
//! crate's feature extractors in MAIN context, DWT-bracketed so the in-situ
//! cost is a measured number, not a guess. Zero work is added to the prio-0
//! commutation/COMP ISRs. This is a direct A/B against the ratch22 monitor on
//! the same interval + current channels.
//!
//! TWO PATHS are exercised, so the qualification covers both of krabilorean's
//! duality halves and prices each separately:
//!
//!   * ONLINE `core_merge_profile` (Extrema + RunningMoments +
//!     SuccessiveDifference) on the interval AND current channels — the
//!     streaming O(1)/sample path. It is a CUMULATIVE-EXACT profile (whole-run
//!     min/max/mean/var/mean-abs-diff from exact sufficient statistics), NOT an
//!     EW/windowed surprise detector like the ratch22 monitor's band — so no
//!     surprise counter here by design; the crate's EW equivalent lives in its
//!     ServoAdaptiveProfile (EwVariance/TransientIndicator) if that role is
//!     wanted. core_merge's stats stay u64, so this hot path is u128-FREE
//!     (krabilorean's u128 wide-variance ratios are only on the windowed/rolling
//!     paths — see the windowed variance below, which we DO price).
//!
//!   * WINDOWED `BasicProfile` epoch (fixed histogram + DIRECT normalized
//!     AUTOCORRELATION) on the interval — the BATCH path the online-only ratch22
//!     monitor never had. The autocorrelation zero-crossing / first-local-min
//!     lag is a commutation-REGULARITY marker: a periodic per-sector interval
//!     bias (the thing the parity campaign chased) shows up as structure at the
//!     matching lag. Runs once per 256-sample epoch in main; DWT-bracketed so
//!     the batch cost (incl. the ~len*LAGS autocorrelation products and the
//!     u128 variance ratio) is measured on-target.
//!
//! krabilorean is i16-input, alloc-free, #![forbid(unsafe_code)], and every op
//! is checked/no-panic (Result). It cross-compiles for thumbv7em here; the M0
//! (F0/G0) portability of the u128 windowed path is a cost question, answered by
//! the `wcyc` readout below.

use core::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, Ordering};
use cortex_m::peripheral::DWT;
use krabilorean::histogram::HistogramBounds;
use krabilorean::policy::Checked;
use krabilorean::profiles::{CoreMergeProfile, core_merge_profile};
use krabilorean::windowed::{Autocorrelation, BasicProfile, Workspace};

/// Windowed epoch length (samples per BasicProfile evaluation). Capped at the
/// crate's `MAX_PROVEN_BASIC_SAMPLES` (256).
const WIN_LEN: usize = 256;
/// Histogram bins for the interval window.
const BINS: usize = 8;
/// Autocorrelation lags (incl. lag 0). 16 spans the 6-sector periodicity with
/// margin, so a per-sector interval bias is visible at lag ~6/12.
const LAGS: usize = 16;
/// Interval histogram bounds (0.5 µs ticks). Locked intervals fall well inside
/// [0, 2047]; engage/slow intervals overflow into the `above` counter.
const HIST_LO: i16 = 0;
const HIST_HI: i16 = 2047;

/// Windowed workspace + sample ring. Touched ONLY from `poll` (main context,
/// single consumer, never an ISR), so a `static mut` is sound. Kept out of the
/// `Krabimon` struct so the ~1.6 KB lands in .bss, not on main's stack (the
/// match-arm-stack-frame lesson).
static mut WIN_WS: Workspace<WIN_LEN, BINS, LAGS> = Workspace::new();
static mut WIN_BUF: [i16; WIN_LEN] = [0; WIN_LEN];

/// Tier: 0=off, 1=online only, 2=online + windowed epochs. Key 'm' cycles.
pub static TIER: AtomicU8 = AtomicU8::new(1);

// ---- published readouts (main writes, print_info reads) --------------------
// online path
pub static SAMPLES: AtomicU32 = AtomicU32::new(0);
pub static LAST_CYC: AtomicU32 = AtomicU32::new(0); // online update+snapshot cost
pub static MIN_CYC: AtomicU32 = AtomicU32::new(u32::MAX);
// interval channel (core_merge)
pub static I_MIN: AtomicI32 = AtomicI32::new(0);
pub static I_MAX: AtomicI32 = AtomicI32::new(0);
pub static I_MEAN: AtomicI32 = AtomicI32::new(0); // sum/samples (ticks)
pub static I_VAR: AtomicI32 = AtomicI32::new(0); // pop var from suff. stats
pub static I_MAD: AtomicI32 = AtomicI32::new(0); // mean abs successive diff
// current channel (core_merge)
pub static C_MIN: AtomicI32 = AtomicI32::new(0);
pub static C_MAX: AtomicI32 = AtomicI32::new(0);
pub static C_MEAN: AtomicI32 = AtomicI32::new(0);
pub static C_MAD: AtomicI32 = AtomicI32::new(0);
// windowed path
pub static WIN_N: AtomicU32 = AtomicU32::new(0); // epochs evaluated
pub static WIN_CYC: AtomicU32 = AtomicU32::new(0); // batch evaluate cost
pub static WIN_MIN_CYC: AtomicU32 = AtomicU32::new(u32::MAX);
pub static W_VAR: AtomicI32 = AtomicI32::new(0); // window pop variance (ticks²)
pub static W_MODE: AtomicI32 = AtomicI32::new(-1); // histogram mode bin (-1 none)
pub static W_ACF1: AtomicI32 = AtomicI32::new(0); // lag-1 autocorr, milli (Q30→×1000)
pub static W_ZC: AtomicI32 = AtomicI32::new(-1); // first non-positive lag
pub static W_LMIN: AtomicI32 = AtomicI32::new(-1); // first local-minimum lag

/// One cumulative-exact channel: Extrema + RunningMoments + SuccessiveDifference.
struct Chan {
    stats: CoreMergeProfile<Checked>,
}

impl Chan {
    fn new() -> Self {
        Self {
            stats: core_merge_profile::<Checked>(),
        }
    }

    /// Feed one i16 sample; publish min/max/mean/var/mad into the given statics.
    fn update(
        &mut self,
        x: i16,
        min: &AtomicI32,
        max: &AtomicI32,
        mean: &AtomicI32,
        var: Option<&AtomicI32>,
        mad: &AtomicI32,
    ) {
        let _ = self.stats.update(x);
        let s = self.stats.snapshot();
        if let Some(e) = s.extrema {
            min.store(e.minimum as i32, Ordering::Relaxed);
            max.store(e.maximum as i32, Ordering::Relaxed);
        }
        if let Some(m) = s.moments {
            let n = m.samples as i64;
            if n > 0 {
                let mn = m.sum / n;
                mean.store(mn as i32, Ordering::Relaxed);
                if let Some(v) = var {
                    // pop var = E[x²] − mean². core_merge keeps sum_of_squares as
                    // u64 (Σx² ≤ n·32767² never overflows u64 at any realistic n),
                    // so this stays u128-free — the online-path cost claim above.
                    let ex2 = (m.sum_of_squares / n as u64) as i64;
                    v.store((ex2 - mn * mn) as i32, Ordering::Relaxed);
                }
            }
        }
        if let Some(d) = s.difference {
            if d.differences > 0 {
                mad.store(
                    (d.absolute_sum / d.differences as u64) as i32,
                    Ordering::Relaxed,
                );
            }
        }
    }
}

pub struct Krabimon {
    interval: Chan,
    current: Chan,
    win: BasicProfile<WIN_LEN, BINS, LAGS>,
    fill: usize,
    last_seq: u32,
}

impl Krabimon {
    pub fn new() -> Self {
        // const-validated capacities + bounds; a bad config fails here, not
        // silently at runtime.
        let bounds = match HistogramBounds::new(HIST_LO, HIST_HI) {
            Ok(b) => b,
            Err(_) => panic!("krabimon: histogram bounds"),
        };
        let win = match BasicProfile::new(bounds) {
            Ok(p) => p,
            Err(_) => panic!("krabimon: basic profile capacities"),
        };
        Self {
            interval: Chan::new(),
            current: Chan::new(),
            win,
            fill: 0,
            last_seq: u32::MAX,
        }
    }

    /// Call every main iteration. `seq` MUST be monotonic per commutation
    /// (`zct.comm_n`) so each interval is consumed exactly once.
    pub fn poll(&mut self, seq: u32, commutation_interval: u32, current_raw: u16) {
        if seq == self.last_seq {
            return;
        }
        self.last_seq = seq;
        let tier = TIER.load(Ordering::Relaxed);
        if tier == 0 {
            return;
        }

        // krabilorean is i16-input. Interval ticks and raw current both fit
        // i16 across the operating range; clamp defensively at engage extremes.
        let ci = commutation_interval.min(i16::MAX as u32) as i16;
        let cur = current_raw.min(i16::MAX as u16) as i16;

        // ---- ONLINE path (streaming, per sample) ----
        let start = DWT::cycle_count();
        self.interval
            .update(ci, &I_MIN, &I_MAX, &I_MEAN, Some(&I_VAR), &I_MAD);
        self.current
            .update(cur, &C_MIN, &C_MAX, &C_MEAN, None, &C_MAD);
        let cost = DWT::cycle_count().wrapping_sub(start);
        LAST_CYC.store(cost, Ordering::Relaxed);
        MIN_CYC.fetch_min(cost, Ordering::Relaxed);
        SAMPLES.fetch_add(1, Ordering::Relaxed);

        // ---- WINDOWED path (batch, per 256-sample epoch) ----
        if tier >= 2 {
            // SAFETY: main-only, single consumer; WIN_BUF/WIN_WS are never
            // touched from any ISR.
            let buf = unsafe { &mut *core::ptr::addr_of_mut!(WIN_BUF) };
            buf[self.fill] = ci;
            self.fill += 1;
            if self.fill >= WIN_LEN {
                self.fill = 0;
                let ws = unsafe { &mut *core::ptr::addr_of_mut!(WIN_WS) };
                let wstart = DWT::cycle_count();
                if let Ok(res) = self.win.evaluate(&*buf, ws) {
                    // exact population variance (u64/u64 ratio → integer ticks²)
                    let v = res.summary.population_variance;
                    if v.denominator > 0 {
                        W_VAR.store((v.numerator / v.denominator) as i32, Ordering::Relaxed);
                    }
                    W_MODE.store(
                        res.histogram.mode_index.map(|i| i as i32).unwrap_or(-1),
                        Ordering::Relaxed,
                    );
                    match res.autocorrelation {
                        Autocorrelation::Defined(a) => {
                            // lag-1 normalized autocorr, Q30 → milli (×1000).
                            let acf1 = a.values_q30.get(1).copied().unwrap_or(0) as i64;
                            W_ACF1.store(((acf1 * 1000) >> 30) as i32, Ordering::Relaxed);
                            W_ZC.store(
                                a.first_non_positive.map(|i| i as i32).unwrap_or(-1),
                                Ordering::Relaxed,
                            );
                            W_LMIN.store(
                                a.first_local_minimum.map(|i| i as i32).unwrap_or(-1),
                                Ordering::Relaxed,
                            );
                        }
                        Autocorrelation::Undefined(_) => {
                            // zero-variance window (rotor dead-still): mark absent.
                            W_ACF1.store(0, Ordering::Relaxed);
                            W_ZC.store(-1, Ordering::Relaxed);
                            W_LMIN.store(-1, Ordering::Relaxed);
                        }
                    }
                }
                let wcost = DWT::cycle_count().wrapping_sub(wstart);
                WIN_CYC.store(wcost, Ordering::Relaxed);
                WIN_MIN_CYC.fetch_min(wcost, Ordering::Relaxed);
                WIN_N.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl Default for Krabimon {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a krabimon control key ('m' cycles tier). Resets the cost floors so
/// they reflect the new tier.
pub fn key(b: u8) {
    if b == b'm' {
        let t = TIER.load(Ordering::Relaxed);
        let next = match t {
            1 => 2,
            2 => 0,
            _ => 1,
        };
        TIER.store(next, Ordering::Relaxed);
        MIN_CYC.store(u32::MAX, Ordering::Relaxed);
        WIN_MIN_CYC.store(u32::MAX, Ordering::Relaxed);
    }
}
