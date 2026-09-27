//! BEMF zero-cross acceptance.
//!
//! This is the decision half of the comparator interrupt: given the elapsed
//! time since the last commutation and a way to re-read the comparator output,
//! decide whether the edge is a real zero crossing, and if so how long to wait
//! before commutating.
//!
//! The hardware half (COMP2 `INMSEL` select, EXTI18 ack, TIM2 reset, TIM16
//! one-shot arm) lives in the target binary. Everything here is pure so the
//! accept/reject rules are host-testable, and it is division-free and
//! panic-free because it runs at interrupt priority.
//!
//! Two independent filters must both pass:
//!
//! 1. **Half-cycle gate.** An edge is ignored unless at least half of the
//!    current average interval has elapsed since the last commutation. This is
//!    what rejects the freewheel/demagnetization spike that immediately follows
//!    every commutation.
//! 2. **Persistence filter.** The comparator output is re-read `filter_level`
//!    times and every read must still show the expected polarity. A single
//!    dissenting read rejects the edge.

use crate::commutation::{advance_of, blend_interval, wait_time};

/// Which estimate schedules this acceptance's commutation. This is a control
/// policy, not a protection setting. Both choices publish the same new state.
pub trait WaitEstimate {
    const PREVIOUS: bool;
    /// None retains the runtime setting. A binary selecting Some must bind
    /// its controller's advance schedule to the same constant.
    const ADVANCE: Option<u32> = None;
}
/// Compile-time advance for a genuinely fixed controller schedule.
/// Keeps the selected estimate dependency and the exact existing rounding.
pub struct ConstantAdvance<T, const LEVEL: u32>(core::marker::PhantomData<T>);
impl<T: WaitEstimate, const LEVEL: u32> WaitEstimate for ConstantAdvance<T, LEVEL> {
    const PREVIOUS: bool = T::PREVIOUS;
    const ADVANCE: Option<u32> = {
        assert!(LEVEL <= 64, "constant advance outside supported range");
        Some(LEVEL)
    };
}
pub struct FreshEstimate;
impl WaitEstimate for FreshEstimate {
    const PREVIOUS: bool = false;
}
/// Reference-like wait dependency at fixed advance: the estimate formed from
/// the previous acceptance schedules this one. Not full AM32 update ordering.
pub struct PreviousEstimate;
impl WaitEstimate for PreviousEstimate {
    const PREVIOUS: bool = true;
}

#[cfg(test)]
mod timing_tests;
#[cfg(test)]
mod historical_persistence;

/// How many consecutive comparator reads must agree, as a policy type.
pub trait FilterPolicy {
    /// `average_interval` is in the same half-microsecond units as the
    /// commutation math.
    fn level(&self, average_interval: u32) -> u8;
}

/// A constant persistence depth.
pub struct FixedFilter<const N: u8>;

impl<const N: u8> FilterPolicy for FixedFilter<N> {
    #[inline]
    fn level(&self, _average_interval: u32) -> u8 {
        N
    }
}

/// The depth used by the qualified image in the 10% regime: at this speed the
/// interval is long, so the deepest filter is affordable and is what was
/// measured.
pub type DefaultFilter = FixedFilter<12>;

/// Speed-scheduled depth: shallow when fast (little time to spend re-reading),
/// deep when slow. Equivalent to `map(avg, 100, 500, 3, 12)`, clamped at both
/// ends.
pub struct MappedFilter;

/// `(x - 100) * 9 / 400 + 3`, evaluated as a reciprocal multiply so no
/// `__aeabi_uidiv` is emitted: `9/400 = 0.0225`, and `0.0225 * 2^16 = 1474.56`,
/// so `(x - 100) * 1475 >> 16` reproduces the truncating division exactly over
/// the clamped input range.
const MAP_RECIP: u32 = 1475;
const MAP_SHIFT: u32 = 16;
pub const MAP_IN_LOW: u32 = 100;
pub const MAP_IN_HIGH: u32 = 500;
/// The reference's floor is **3** reads. `deep-filter` raises it to **5**
/// (E324).
///
/// **Why.** The map floors at 3 for every `average_interval` at or below
/// 73 µs, and that boundary falls between rung 525 (ci 74, depth 4) and rung
/// 550 (ci 71, depth 3) — exactly where this campaign's failure appears:
///
/// ```text
/// depth 4 rungs (475, 500, 525):   6 events /  8183 s = 0.00073 /s
/// depth 3 rungs (550, 575):       11 events /   948 s = 0.01160 /s
///                                  rate ratio 15.8x, exact p = 1.0e-07
/// ```
///
/// The smooth duty trend accounts for only **2.4–4.0×** of that (the hazard
/// doubles every 1.25–1.95 % duty, and 525 → 550 is 2.5 %), leaving a **4–6.5×
/// excess at a single discrete step** — and the only discrete thing at ci 73 is
/// this constant.
///
/// E322 is what makes the argument admissible: it decoupled the depth step
/// from the `wait` boundary, which had been confounded with it to within one
/// microsecond since E314. Advance 20 moved the wait threshold and the latch
/// followed it down to ci 46 rather than disappearing, so the wait boundary is
/// *not* what separates 525 from 550. Depth is the surviving discrete
/// correlate.
///
/// **What it costs and what it risks.** Two extra live comparator reads per
/// accepted crossing, ~0.25 µs each by the static model, on the pre-arm path —
/// so `spent` rises by ~0.5 µs, against a `wait` of 9–11 at these rungs. And a
/// deeper filter refuses more edges: if it refuses *real* crossings the
/// commutation is missed and the tracking stop fires, which is a different and
/// equally informative failure. The audited loop bound is unchanged — the
/// clamp at `MAP_OUT_HIGH` still bounds it at 12.
pub const MAP_OUT_LOW: u8 = 3;
pub const MAP_OUT_HIGH: u8 = 12;

/// Applied as a **floor on the mapped result**, not as the map's base.
///
/// The first attempt at this raised `MAP_OUT_LOW`, which is the map's additive
/// base (`MAP_OUT_LOW + add`), so it shifted the entire curve up by two and
/// took the deepest schedule from 12 reads to 14 — past the audited loop
/// bound. `reference_filter_never_exceeds_twelve_reads` caught it on the first
/// run. A floor is `max(FILTER_FLOOR, mapped)`, which touches only the
/// intervals where the map is already at or below it.
#[cfg(not(any(feature = "deep-filter", feature = "deep-filter-6")))]
pub const FILTER_FLOOR: u8 = MAP_OUT_LOW;
#[cfg(all(feature = "deep-filter", not(feature = "deep-filter-6")))]
pub const FILTER_FLOOR: u8 = 5;
/// One level deeper, to test whether E324's floor-5 result is the start of a
/// monotone trend or its optimum (E325). Takes precedence over `deep-filter`
/// so enabling both is not a silent conflict.
#[cfg(feature = "deep-filter-6")]
pub const FILTER_FLOOR: u8 = 6;

#[inline]
pub const fn mapped_filter_level(average_interval: u32) -> u8 {
    if average_interval <= MAP_IN_LOW {
        return FILTER_FLOOR;
    }
    if average_interval >= MAP_IN_HIGH {
        return MAP_OUT_HIGH;
    }
    let span = average_interval - MAP_IN_LOW;
    let add = (span * MAP_RECIP) >> MAP_SHIFT;
    // `add` is bounded by 9 over the clamped range, so this cannot overflow.
    let mapped = MAP_OUT_LOW + add as u8;
    if mapped < FILTER_FLOOR { FILTER_FLOOR } else { mapped }
}

impl FilterPolicy for MappedFilter {
    #[inline]
    fn level(&self, average_interval: u32) -> u8 {
        mapped_filter_level(average_interval)
    }
}

/// Adapts a half-microsecond policy to an estimator that works in whole
/// microseconds (E083).
///
/// `FilterPolicy` is specified in the reference's half-µs units (minz's
/// `filter_and_duty_max` maps `average_interval`, which is half-µs,
/// `minz/core/src/am32_loop.rs:298-305`). firmware50's `ZeroCross` keeps its
/// interval in µs, so feeding it straight in would halve every threshold. This
/// doubles the input and nothing else; the wrapped policy's clamps still bound
/// the result, so a `MappedFilter` behind it never exceeds 12 reads.
pub struct FromMicros<P>(pub P);

impl<P: FilterPolicy> FilterPolicy for FromMicros<P> {
    #[inline]
    fn level(&self, average_interval_us: u32) -> u8 {
        self.0.level(average_interval_us.saturating_mul(2))
    }
}

/// The reference's persistence schedule for an estimator in µs: the mapped
/// depth with the very-fast floor, as minz computes it.
pub type ReferenceFilterUs = FromMicros<WithShallowFloor<MappedFilter>>;

/// Below this interval there is no time to spend on persistence reads at all.
pub const SHALLOW_INTERVAL: u32 = 50;
/// The depth used when the interval is below `SHALLOW_INTERVAL`.
pub const SHALLOW_LEVEL: u8 = 2;

/// Wraps any policy with the reference's very-fast-rotor floor.
pub struct WithShallowFloor<P>(pub P);

impl<P: FilterPolicy> FilterPolicy for WithShallowFloor<P> {
    #[inline]
    fn level(&self, average_interval: u32) -> u8 {
        if average_interval < SHALLOW_INTERVAL {
            SHALLOW_LEVEL
        } else {
            self.0.level(average_interval)
        }
    }
}

/// Where the commutation timing comes from.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Comparator edges drive commutation.
    Interrupt,
    /// Too slow for reliable edges; commutate on the timer and poll.
    Polling,
}

/// Interval at or below which the detector hands over to polling.
pub const POLLING_CHANGEOVER: u32 = 2_000;

#[inline]
pub const fn mode_for(average_interval: u32) -> Mode {
    if average_interval < POLLING_CHANGEOVER {
        Mode::Polling
    } else {
        Mode::Interrupt
    }
}

/// Why an edge was refused, or what an accepted edge scheduled.
///
/// Every refusal class gets its own variant so it can carry its own counter in
/// telemetry — an aggregate "rejected" count is blind to which filter is doing
/// the rejecting.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Inside the half-cycle blanking window.
    TooEarly,
    /// Persistence filter saw a dissenting read.
    Unstable,
    /// A real zero crossing.
    Accepted {
        /// Half-microseconds to load into the one-shot commutation timer.
        wait: u32,
        /// The new blended average interval.
        average_interval: u32,
        /// Advance actually applied, for telemetry.
        advance: u32,
    },
}

/// The zero-cross detector.
///
/// Not Copy: it carries the running interval estimate and the refusal
/// counters, and a silent copy would quietly discard both.
///
/// `BLANK_64` is the half-cycle gate's width in sixty-fourths of the estimate,
/// fixed per build (E130): production uses [`ZeroCross`], the reference's
/// half cycle, and a study build instantiates another fraction by type. As a constant the gate
/// compiles to a shift; as the runtime field it replaced it cost two loads,
/// two multiplies and a register spill on every COMP edge.
#[derive(Clone, Debug)]
pub struct ZeroCrossWith<const BLANK_64: u32> {
    average_interval: u32,
    last_zc: u32,
    prev_zc: u32,
    too_early: u32,
    unstable: u32,
    accepted: u32,
    /// Bounds on the interval estimate.
    ///
    /// **Without these the estimator runs away, measured on the bench.** The
    /// blanking window is half the current estimate, so an edge accepted early
    /// in a sector pulls the estimate down, which narrows the window, which
    /// admits an even earlier edge next time. A first closed-loop run handed
    /// off at 60 eHz -- a 2777 us sector -- and collapsed to `ci_us = 65`,
    /// implying about 2560 eHz, while still reporting 696 accepted crossings.
    /// The loop was confidently tracking nothing.
    ///
    /// Clamping the estimate is what the reference does (it admits a seed only
    /// within a validated interval band), and it breaks the feedback: the
    /// window can never narrow past the floor, so a physically impossible
    /// interval cannot be sustained.
    min_interval: u32,
    max_interval: u32,
    // The blanking fraction is the type parameter `BLANK_64`. The reference
    // blanks for half a cycle, which is `32` and remains the default. It is a
    // parameter because the bench measured a bias that
    // a half-cycle gate cannot reject: with the hardware edge detector the
    // mean *accepted* interval settled consistently below the mean *sector*
    // time (1737-1820 us against 2025-2107 us over three 35 s runs), and
    // 26-31% of accepts landed inside three quarters of an interval. Those
    // are chatter edges taken in place of the crossing, and each one pulls
    // the estimate down, which pulls the gate in.
    //
    // Widening the gate is self-correcting in the right direction: it refuses
    // the early band, the estimate rises toward the true sector time, and the
    // gate widens with it. It is bounded above by the fact that a genuine
    // crossing sits near `64`, so anything approaching that starts refusing
    // real crossings -- which is why this is stated per build and measured,
    // not tuned silently.
}

/// The reference's blanking window: half a cycle, i.e. 32 sixty-fourths.
pub const REFERENCE_BLANK_64: u32 = 32;

/// The production detector: the reference's half-cycle gate.
#[cfg(not(feature = "wide-blank"))]
pub type ZeroCross = ZeroCrossWith<REFERENCE_BLANK_64>;

/// **40/64 instead of the reference's 32/64** (`wide-blank`, E328).
///
/// # REFUTED ON THE BENCH — do not enable
///
/// One run at rung 600 (`captures/2026-09-25/e328-600-wideblank_01.txt`)
/// stopped on **`reason=26`, FastBusSag**, at 22.6 s with `hold_ms=0` and
/// `late_arms=0`. It removed the late arm and produced a supply event instead,
/// by the mechanism predicted below: `coast_ehz` fell to **2224** against the
/// usual 2450 (a 9 % slower rotor) and accepts ran at **8387/s against ~13344
/// expected**, so the loop was missing real crossings. `too_early` read only
/// **4**, which is the tell — the refusals were not counted at the gate,
/// they left `sector_start_raw` unmoved so the *following* accept measured
/// ~1.5× and jerked the estimate up, and the loop then commutated late. It
/// adds more jitter than it removes. Kept only so the result is reproducible.
///
/// The gate is what admits the edges that feed the estimator's descent, and it
/// admits everything above `0.5 · ci`. `lt075` — accepts arriving below
/// `0.75 · ci`, i.e. the descent's fuel — runs at **1.88 % of accepts at rung
/// 600**, and the blend turns each one into a step of up to
/// `1 - (0.75 + 0.25 · BLANK_64/64)`: **12.5 % at the reference 32, 9.4 % at
/// 40**. Widening the gate to 40 refuses the whole band between `0.500 · ci`
/// and `0.625 · ci` outright.
///
/// **Why this and not more filter depth.** E324's floor 3 → 5 attacked the same
/// fuel and bought 91× on the rung-600 hold, but it costs two extra live
/// comparator reads *on the pre-arm path*, which is why `thin` rose 40× and why
/// floor 6 was 6× worse than floor 5 (E325). The gate is a comparison: a
/// non-reference fraction costs the exact `interval * BLANK_64 / 64` —
/// a multiply and a shift instead of a single shift — so roughly four
/// instructions against two ~0.25 µs reads. **It buys descent resistance
/// without spending `spent`.**
///
/// **What it risks.** An edge from a rotor that genuinely accelerated more than
/// 37.5 % inside one commutation would now be refused — which no loaded rotor
/// can do. A refusal leaves `sector_start_raw` unmoved, so the *following*
/// accept measures a doubled count and the estimate moves **up**, which is the
/// safe direction and is what the tracking stop already covers.
#[cfg(feature = "wide-blank")]
pub type ZeroCross = ZeroCrossWith<40>;

impl<const BLANK_64: u32> ZeroCrossWith<BLANK_64> {
    /// `BLANK_64` must lie in `1..=56`: zero would disable the gate that stops
    /// the estimator running away, and a value at or above one whole interval
    /// would refuse every genuine crossing (56 leaves an eighth of an interval
    /// of margin). Checked when the type is instantiated -- a build error, not
    /// the runtime clamp it replaced:
    ///
    /// ```compile_fail
    /// let _ = firmware50::bemf::ZeroCrossWith::<0>::new(833);
    /// ```
    ///
    /// ```compile_fail
    /// let _ = firmware50::bemf::ZeroCrossWith::<57>::new(833);
    /// ```
    ///
    /// ```
    /// let _ = firmware50::bemf::ZeroCrossWith::<56>::new(833);
    /// ```
    const IN_BAND: () = assert!(BLANK_64 >= 1 && BLANK_64 <= 56, "BLANK_64 must be 1..=56");

    /// The same estimator state with another blanking fraction.
    #[inline]
    pub const fn with_blanking<const B: u32>(self) -> ZeroCrossWith<B> {
        ZeroCrossWith::<B>::from_state(self.state())
    }

    /// The blanking window in sixty-fourths.
    #[inline]
    pub const fn blanking_64(&self) -> u32 {
        BLANK_64
    }

    /// `seed_interval` is the handoff estimate from the open-loop ramp.
    ///
    /// Bounds the estimate to a quarter and four times the seed, which spans
    /// two octaves either side of the handoff speed -- wide enough for the
    /// closed loop to accelerate through, narrow enough that the estimator
    /// cannot collapse. Use [`Self::new_bounded`] to state the band directly.
    #[inline]
    pub const fn new(seed_interval: u32) -> Self {
        Self::new_bounded(seed_interval, seed_interval / 4, seed_interval.saturating_mul(4))
    }

    /// As [`Self::new`], with an explicit interval band.
    #[inline]
    pub const fn new_bounded(seed_interval: u32, min_interval: u32, max_interval: u32) -> Self {
        let () = Self::IN_BAND;
        Self {
            average_interval: seed_interval,
            last_zc: seed_interval,
            prev_zc: seed_interval,
            too_early: 0,
            unstable: 0,
            accepted: 0,
            min_interval,
            max_interval,
        }
    }

    /// The state `offer` depends on: `(average_interval, last_zc, min, max,
    /// blank_64)`. The refusal counters are not part of it.
    #[inline]
    pub const fn state(&self) -> [u32; 5] {
        [
            self.average_interval,
            self.last_zc,
            self.min_interval,
            self.max_interval,
            BLANK_64,
        ]
    }

    /// Rebuild a detector from [`Self::state`], counters at zero: for
    /// replaying a captured decision sequence from its first decision. The
    /// blanking fraction is this type's; `s[4]` records the capture's, and a
    /// replay compares the two.
    #[inline]
    pub const fn from_state(s: [u32; 5]) -> Self {
        let () = Self::IN_BAND;
        Self {
            average_interval: s[0],
            last_zc: s[1],
            prev_zc: s[1],
            too_early: 0,
            unstable: 0,
            accepted: 0,
            min_interval: s[2],
            max_interval: s[3],
        }
    }

    /// The interval band the estimate is held within.
    #[inline]
    pub const fn bounds(&self) -> (u32, u32) {
        (self.min_interval, self.max_interval)
    }

    /// Clamp an interval estimate into the band.
    #[inline]
    const fn clamp_interval(&self, v: u32) -> u32 {
        if v < self.min_interval {
            self.min_interval
        } else if v > self.max_interval {
            self.max_interval
        } else {
            v
        }
    }

    #[inline]
    pub const fn average_interval(&self) -> u32 {
        self.average_interval
    }
    #[inline]
    pub const fn mode(&self) -> Mode {
        mode_for(self.average_interval)
    }
    #[inline]
    pub const fn counts(&self) -> (u32, u32, u32) {
        (self.accepted, self.too_early, self.unstable)
    }

    /// The half-cycle blanking window, in the same units as `count`.
    #[inline]
    pub const fn blanking(&self) -> u32 {
        // The reference's half cycle, exactly (`(i >> 6) * 32 + ((i & 63) * 32
        // >> 6)` is `i >> 1`): one shift on every COMP edge.
        if BLANK_64 == REFERENCE_BLANK_64 {
            return self.average_interval >> 1;
        }
        // Other fractions: exact `interval * BLANK_64 / 64` in 32-bit
        // arithmetic, with no division and no overflow.
        //
        // The obvious `(interval * blank_64) >> 6` overflows: a first version
        // wrote exactly that, justified by "the estimate is bounded well below
        // `u32::MAX / 64`", and `extreme_intervals_do_not_wrap_or_panic`
        // rejected it on the spot with a `u32::MAX` seed. The bound is a
        // property of how this type is *used*, not of the type, and a
        // `const fn` cannot assume its caller. Splitting into whole and
        // fractional sixty-fourths keeps every intermediate small: the high
        // part is at most `interval`, and the low part at most `63 * 56`.
        //
        // `whole` cannot overflow: `BLANK_64` is 1..=56 by construction
        // (`IN_BAND`), and (2^26 - 1) * 56 < 2^32. It is a
        // plain multiply, not `saturating_mul`, because the saturating form
        // lowers to a 64-bit `__aeabi_lmul` on thumbv6m once this is inlined
        // into `ADC_COMP` (E116), which the math audit forbids in a root.
        let whole = (self.average_interval >> 6).wrapping_mul(BLANK_64);
        let frac = ((self.average_interval & 63) * BLANK_64) >> 6;
        whole.saturating_add(frac)
    }

    /// Offer a comparator edge.
    ///
    /// * `count` — elapsed since the last commutation (interval timer CNT).
    /// * `rising` — the polarity this sector expects.
    /// * `advance_level` — sixty-fourths of a half-cycle to advance by.
    /// * `read_level` — re-reads the live comparator output. Called at most
    ///   `filter_level` times, so the loop is bounded by the policy.
    pub fn offer<F: FilterPolicy, R: FnMut() -> bool>(
        &mut self,
        count: u32,
        rising: bool,
        advance_level: u32,
        policy: &F,
        read_level: R,
    ) -> Outcome {
        self.offer_timed::<FreshEstimate, F, R>(count, rising, advance_level, policy, read_level)
    }

    /// Same acceptance and state update; only the wait's estimate is selected
    /// by type. Snapshot under this exclusive borrow, never from foreground.
    pub fn offer_timed<T: WaitEstimate, F: FilterPolicy, R: FnMut() -> bool>(
        &mut self,
        count: u32,
        rising: bool,
        advance_level: u32,
        policy: &F,
        mut read_level: R,
    ) -> Outcome {
        self.offer_timed_matches::<T, F, _>(count, advance_level, policy, || read_level() == rising)
    }

    /// Each predicate call samples once and reports whether the level matches.
    /// Same digital persistence contract; a cheaper hardware predicate changes
    /// sample spacing and must be qualified as a distinct physical aperture.
    pub fn offer_timed_matches<T: WaitEstimate, F: FilterPolicy, R: FnMut() -> bool>(
        &mut self,
        count: u32,
        advance_level: u32,
        policy: &F,
        mut matches: R,
    ) -> Outcome {
        // 1. half-cycle gate — strictly greater, matching the reference.
        if count <= self.blanking() {
            self.too_early = self.too_early.wrapping_add(1);
            return Outcome::TooEarly;
        }

        // 2. persistence filter. Bounded loop: `filter_level` is a u8, so this
        //    is at most 255 reads and cannot run away.
        let depth = policy.level(self.average_interval);
        let mut i = 0u8;
        while i < depth {
            if !matches() {
                self.unstable = self.unstable.wrapping_add(1);
                return Outcome::Unstable;
            }
            i += 1;
        }

        // 3. Identical blend/publication for both policies; wait may use the
        // preblend estimate, including the seed on the very first acceptance.
        let previous = self.average_interval;
        self.prev_zc = self.last_zc;
        self.last_zc = count;
        self.average_interval = self.clamp_interval(blend_interval(self.average_interval, self.prev_zc, self.last_zc));
        let scheduled = if T::PREVIOUS { previous } else { self.average_interval };
        let advance_level = T::ADVANCE.unwrap_or(advance_level);
        let advance = advance_of(scheduled, advance_level);
        let wait = wait_time(scheduled, advance_level);
        self.accepted = self.accepted.wrapping_add(1);
        Outcome::Accepted {
            wait,
            average_interval: self.average_interval,
            advance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    const ADV: u32 = 16;

    /// The µs adapter reproduces the reference's half-µs schedule at the rungs
    /// this campaign runs (E083): 12 reads at 10% and at handover, 11 at the
    /// 15% target's 237 µs sector, 8 at 20%'s ~177 µs, 7 at 25%'s ~140 µs, 2
    /// below 25 µs.
    #[test]
    fn reference_filter_in_microseconds_matches_the_rungs() {
        let f: ReferenceFilterUs = FromMicros(WithShallowFloor(MappedFilter));
        assert_eq!(f.level(833), 12, "200 eHz handover");
        assert_eq!(f.level(419), 12, "10%: 838 half-us clamps at 12");
        assert_eq!(f.level(237), 11, "15%: 474 half-us");
        // 3 + floor((354 - 100) * 1475 / 2^16) = 3 + 5 = 8.
        assert_eq!(f.level(177), 8, "20% reference ~941 eHz: 354 half-us");
        // 3 + floor((280 - 100) * 1475 / 2^16) = 3 + 4 = 7.
        assert_eq!(f.level(140), 7, "25%: 280 half-us");
        assert_eq!(f.level(24), SHALLOW_LEVEL, "below 50 half-us");
        // It is exactly the half-us policy fed twice the input.
        for us in [0u32, 1, 25, 49, 50, 100, 199, 250, 251, 1_000, u32::MAX / 4] {
            assert_eq!(
                f.level(us),
                WithShallowFloor(MappedFilter).level(us.saturating_mul(2)),
                "{us} us"
            );
        }
    }

    /// Whatever the speed, the depth never exceeds the audited loop bound.
    #[test]
    fn reference_filter_never_exceeds_twelve_reads() {
        let f: ReferenceFilterUs = FromMicros(WithShallowFloor(MappedFilter));
        for us in (0..5_000u32).chain([u32::MAX / 2, u32::MAX]) {
            assert!(f.level(us) <= 12, "{us} us -> {}", f.level(us));
        }
    }

    /// The default is the reference's half cycle, and `blanking` reports it.
    /// **Scoped to the default gate.** `wide-blank` changes `ZeroCross`'s
    /// fraction deliberately (E328), so this pins the configuration it
    /// describes rather than being weakened for both.
    #[cfg(not(feature = "wide-blank"))]
    #[test]
    fn default_blanking_is_half_a_cycle() {
        let zc = ZeroCross::new(2777);
        assert_eq!(zc.blanking_64(), REFERENCE_BLANK_64);
        assert_eq!(zc.blanking(), 2777 / 2);
    }

    fn scales<const N: u32>() {
        let zc = ZeroCrossWith::<N>::new(2560);
        assert_eq!(zc.blanking_64(), N);
        assert_eq!(zc.blanking(), 2560 * N / 64);
    }

    /// A stated window scales with the estimate, in sixty-fourths.
    #[test]
    fn blanking_scales_with_the_configured_fraction() {
        scales::<1>();
        scales::<8>();
        scales::<32>();
        scales::<44>();
        scales::<56>();
    }

    /// The half-cycle shortcut is exactly the general formula at 32, for
    /// every interval including the extremes.
    /// **Scoped to the default gate.** `wide-blank` changes `ZeroCross`'s
    /// fraction deliberately (E328), so this pins the configuration it
    /// describes rather than being weakened for both.
    #[cfg(not(feature = "wide-blank"))]
    #[test]
    fn the_half_cycle_shortcut_equals_the_general_formula() {
        for i in (0..70_000u32).chain([u32::MAX / 2, u32::MAX - 1, u32::MAX]) {
            let general = (i >> 6).wrapping_mul(32) + (((i & 63) * 32) >> 6);
            assert_eq!(ZeroCross::new(i).blanking(), general, "{i}");
        }
    }

    /// Changing the fraction keeps the estimator's state; the fraction
    /// travels with the type (the band itself is a compile-time check, see
    /// the `IN_BAND` doctests).
    #[test]
    fn with_blanking_keeps_the_estimate_and_moves_the_gate() {
        let half = ZeroCross::new_bounded(2777, 40, 4000);
        let wide = half.clone().with_blanking::<44>();
        assert_eq!(wide.state()[..4], half.state()[..4]);
        assert_eq!(wide.blanking_64(), 44);
        assert_eq!(wide.blanking(), 2777 * 44 / 64);
    }

    /// A wider window refuses edges a half-cycle window would have accepted,
    /// and counts them as `too_early` rather than silently dropping them.
    /// **Scoped to the default gate.** `wide-blank` changes `ZeroCross`'s
    /// fraction deliberately (E328), so this pins the configuration it
    /// describes rather than being weakened for both.
    #[cfg(not(feature = "wide-blank"))]
    #[test]
    fn a_wider_window_refuses_the_early_band() {
        let ci = 2560;
        // 0.6 of an interval: past a half-cycle gate, inside a 44/64 gate.
        let count = ci * 6 / 10;

        let mut half = ZeroCross::new(ci);
        assert!(matches!(
            half.offer(count, true, ADV, &FixedFilter::<4>, steady(true)),
            Outcome::Accepted { .. }
        ));

        let mut wide = ZeroCrossWith::<44>::new(ci);
        assert!(matches!(
            wide.offer(count, true, ADV, &FixedFilter::<4>, steady(true)),
            Outcome::TooEarly
        ));
        assert_eq!(wide.counts().1, 1);
    }

    /// Widening the window must not refuse a crossing at a whole interval,
    /// which is where a genuine one sits.
    #[test]
    fn a_wider_window_still_accepts_a_whole_interval() {
        fn whole<const N: u32>() {
            let ci = 2560;
            let mut zc = ZeroCrossWith::<N>::new(ci);
            assert!(
                matches!(
                    zc.offer(ci, true, ADV, &FixedFilter::<4>, steady(true)),
                    Outcome::Accepted { .. }
                ),
                "blanking {N} refused a crossing at one whole interval"
            );
        }
        whole::<32>();
        whole::<40>();
        whole::<44>();
        whole::<50>();
        whole::<56>();
    }

    fn steady(level: bool) -> impl FnMut() -> bool {
        move || level
    }

    /// A reader that returns `first` once and then `rest` — the shape of a
    /// noise glitch that a single read would have accepted.
    fn flaky(first: bool, rest: bool) -> impl FnMut() -> bool {
        let n = Cell::new(0u32);
        move || {
            let i = n.get();
            n.set(i + 1);
            if i == 0 { first } else { rest }
        }
    }

    // --- filter policies ---------------------------------------------------

    #[test]
    fn fixed_filter_is_constant_and_default_is_twelve() {
        let p: DefaultFilter = FixedFilter::<12>;
        for ci in [0u32, 50, 2_000, 100_000] {
            assert_eq!(p.level(ci), 12);
        }
    }

    /// The floor is a floor: it lifts the shallow end and leaves the deep end
    /// exactly where the reference put it, including the audited 12 bound that
    /// the first attempt at this broke by raising the map's additive base.
    #[test]
    fn the_filter_floor_lifts_only_the_shallow_end() {
        // Half-µs inputs, as the map takes them. 500 and above is the deep clamp.
        assert_eq!(mapped_filter_level(500), MAP_OUT_HIGH, "deep clamp untouched");
        assert_eq!(mapped_filter_level(4000), MAP_OUT_HIGH, "and beyond it");
        assert!(mapped_filter_level(100) >= FILTER_FLOOR, "the short-interval return");
        let mut x = 100;
        while x <= 500 {
            let d = mapped_filter_level(x);
            assert!(d >= FILTER_FLOOR, "below the floor at {x}: {d}");
            assert!(d <= MAP_OUT_HIGH, "past the audited bound at {x}: {d}");
            x += 1;
        }
        // Monotone, so the floor cannot introduce a dip.
        let mut prev = 0;
        let mut x = 100;
        while x <= 500 {
            let d = mapped_filter_level(x);
            assert!(d >= prev, "not monotone at {x}");
            prev = d;
            x += 1;
        }
    }

    /// **Guards the DEFAULT build's reference parity.** `deep-filter` raises
    /// `FILTER_FLOOR` and therefore breaks this map's parity with the
    /// reference deliberately, which is the whole point of the feature, so the
    /// assertion is scoped to the configuration it describes rather than
    /// weakened for both.
    #[cfg(not(any(feature = "deep-filter", feature = "deep-filter-6")))]
    #[test]
    fn mapped_filter_reproduces_the_reference_map_exactly() {
        // The reciprocal multiply must equal the truncating integer division
        // (x-100)*9/400 + 3 across the whole clamped range.
        for x in 0u32..=600 {
            let want = if x <= MAP_IN_LOW {
                MAP_OUT_LOW as u32
            } else if x >= MAP_IN_HIGH {
                MAP_OUT_HIGH as u32
            } else {
                (x - MAP_IN_LOW) * 9 / 400 + MAP_OUT_LOW as u32
            };
            assert_eq!(mapped_filter_level(x) as u32, want, "average_interval {x}");
        }
    }

    /// **Guards the DEFAULT build's reference parity.** `deep-filter` raises
    /// `FILTER_FLOOR` and therefore breaks this map's parity with the
    /// reference deliberately, which is the whole point of the feature, so the
    /// assertion is scoped to the configuration it describes rather than
    /// weakened for both.
    #[cfg(not(any(feature = "deep-filter", feature = "deep-filter-6")))]
    #[test]
    fn mapped_filter_clamps_at_both_ends() {
        assert_eq!(mapped_filter_level(0), MAP_OUT_LOW);
        assert_eq!(mapped_filter_level(MAP_IN_LOW), MAP_OUT_LOW);
        assert_eq!(mapped_filter_level(MAP_IN_HIGH), MAP_OUT_HIGH);
        assert_eq!(mapped_filter_level(u32::MAX), MAP_OUT_HIGH);
    }

    #[test]
    fn shallow_floor_overrides_only_below_the_threshold() {
        let p = WithShallowFloor(FixedFilter::<12>);
        assert_eq!(p.level(SHALLOW_INTERVAL - 1), SHALLOW_LEVEL);
        assert_eq!(p.level(SHALLOW_INTERVAL), 12, "at the threshold the inner policy wins");
        assert_eq!(p.level(10_000), 12);
    }

    // --- mode changeover ---------------------------------------------------

    #[test]
    fn mode_changes_over_at_the_documented_interval() {
        assert_eq!(mode_for(POLLING_CHANGEOVER - 1), Mode::Polling);
        assert_eq!(mode_for(POLLING_CHANGEOVER), Mode::Interrupt);
        assert_eq!(mode_for(50_000), Mode::Interrupt);
        // at 10% the interval is far above the changeover
        assert_eq!(ZeroCross::new(20_000).mode(), Mode::Interrupt);
    }

    // --- half-cycle gate ---------------------------------------------------

    /// **Scoped to the default gate.** `wide-blank` changes `ZeroCross`'s
    /// fraction deliberately (E328), so this pins the configuration it
    /// describes rather than being weakened for both.
    #[cfg(not(feature = "wide-blank"))]
    #[test]
    fn edges_inside_the_blanking_window_are_refused() {
        let mut z = ZeroCross::new(20_000);
        assert_eq!(z.blanking(), 10_000);
        let p = FixedFilter::<12>;
        for count in [0u32, 1, 5_000, 10_000] {
            assert_eq!(
                z.offer(count, true, ADV, &p, steady(true)),
                Outcome::TooEarly,
                "count {count}"
            );
        }
        // strictly greater passes the gate
        assert!(matches!(
            z.offer(10_001, true, ADV, &p, steady(true)),
            Outcome::Accepted { .. }
        ));
    }

    /// **Scoped to the default gate.** `wide-blank` changes `ZeroCross`'s
    /// fraction deliberately (E328), so this pins the configuration it
    /// describes rather than being weakened for both.
    #[cfg(not(feature = "wide-blank"))]
    #[test]
    fn the_blanking_window_tracks_the_average_interval() {
        let mut z = ZeroCross::new(400);
        assert_eq!(z.blanking(), 200);
        let p = FixedFilter::<12>;
        assert_eq!(z.offer(150, true, ADV, &p, steady(true)), Outcome::TooEarly);
        assert!(matches!(
            z.offer(250, true, ADV, &p, steady(true)),
            Outcome::Accepted { .. }
        ));
    }

    // --- persistence filter ------------------------------------------------

    #[test]
    fn a_single_dissenting_read_refuses_the_edge() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        assert_eq!(z.offer(15_000, true, ADV, &p, steady(false)), Outcome::Unstable);
        // glitch on the very first read
        assert_eq!(z.offer(15_000, true, ADV, &p, flaky(false, true)), Outcome::Unstable);
        // glitch on a later read is caught too
        let n = Cell::new(0u32);
        let reader = || {
            let i = n.get();
            n.set(i + 1);
            i != 7
        };
        assert_eq!(z.offer(15_000, true, ADV, &p, reader), Outcome::Unstable);
    }

    #[test]
    fn falling_polarity_sectors_require_a_low_output() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        // expecting a falling edge: a steady-high output is the wrong polarity
        assert_eq!(z.offer(15_000, false, ADV, &p, steady(true)), Outcome::Unstable);
        assert!(matches!(
            z.offer(15_000, false, ADV, &p, steady(false)),
            Outcome::Accepted { .. }
        ));
    }

    #[test]
    fn the_filter_reads_exactly_the_policy_depth_and_no_more() {
        let mut z = ZeroCross::new(20_000);
        let reads = Cell::new(0u32);
        let p = FixedFilter::<5>;
        let _ = z.offer(15_000, true, ADV, &p, || {
            reads.set(reads.get() + 1);
            true
        });
        assert_eq!(reads.get(), 5, "bounded by the policy, not by the signal");
    }

    #[test]
    fn a_zero_depth_policy_still_requires_the_half_cycle_gate() {
        // Degenerate but must not be an unbounded loop or an accept-anything.
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<0>;
        assert_eq!(z.offer(100, true, ADV, &p, steady(false)), Outcome::TooEarly);
        // past the gate a zero-depth filter accepts without reading
        let reads = Cell::new(0u32);
        let out = z.offer(15_000, true, ADV, &p, || {
            reads.set(reads.get() + 1);
            false
        });
        assert!(matches!(out, Outcome::Accepted { .. }));
        assert_eq!(reads.get(), 0);
    }

    // --- accept path -------------------------------------------------------

    #[test]
    fn accept_schedules_half_a_cycle_minus_the_advance() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        match z.offer(20_000, true, ADV, &p, steady(true)) {
            Outcome::Accepted {
                wait,
                average_interval,
                advance,
            } => {
                assert_eq!(advance, advance_of(average_interval, ADV));
                assert_eq!(wait, wait_time(average_interval, ADV));
                assert_eq!(wait + advance, average_interval >> 1);
            }
            other => panic!("expected accept, got {other:?}"),
        }
    }

    #[test]
    fn a_zero_advance_level_waits_the_full_half_cycle() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        match z.offer(20_000, true, 0, &p, steady(true)) {
            Outcome::Accepted {
                wait,
                average_interval,
                advance,
            } => {
                assert_eq!(advance, 0);
                assert_eq!(wait, average_interval >> 1);
            }
            other => panic!("expected accept, got {other:?}"),
        }
    }

    #[test]
    fn the_average_interval_converges_toward_a_steady_speed() {
        // Seeded slow, fed a steady faster crossing: the blend must walk the
        // average toward it monotonically and settle, never oscillate.
        let mut z = ZeroCross::new(30_000);
        let p = FixedFilter::<12>;
        let mut prev = z.average_interval();
        for _ in 0..40 {
            let out = z.offer(20_000, true, ADV, &p, steady(true));
            assert!(matches!(out, Outcome::Accepted { .. }));
            let now = z.average_interval();
            assert!(now <= prev, "average must not increase: {prev} -> {now}");
            prev = now;
        }
        let settled = z.average_interval();
        assert!(
            (19_000..=21_000).contains(&settled),
            "should settle near the observed 20000, got {settled}"
        );
    }

    #[test]
    fn the_interval_estimate_cannot_collapse_however_early_the_edges() {
        // The bench failure this guards: every edge accepted as early as the
        // blanking gate allows drags the estimate down, narrowing the gate and
        // admitting a still-earlier edge. Unbounded, a 2777 us seed collapsed
        // to 65 us while reporting hundreds of accepted crossings.
        let seed = 2_777;
        let mut z = ZeroCross::new(seed);
        let (min, max) = z.bounds();
        let p = FixedFilter::<12>;
        for _ in 0..500 {
            // Offer at one tick past the gate every time: the worst case.
            let count = z.blanking() + 1;
            let out = z.offer(count, true, ADV, &p, steady(true));
            assert!(matches!(out, Outcome::Accepted { .. }));
            assert!(
                z.average_interval() >= min,
                "estimate fell through the floor: {} < {min}",
                z.average_interval()
            );
        }
        assert_eq!(z.average_interval(), min, "should rest on the floor, not below");
        assert!(max > min);
    }

    #[test]
    fn the_interval_estimate_cannot_run_away_upward() {
        let seed = 2_777;
        let mut z = ZeroCross::new(seed);
        let (_, max) = z.bounds();
        let p = FixedFilter::<12>;
        for _ in 0..500 {
            // Offer very late every time.
            let out = z.offer(max * 4, true, ADV, &p, steady(true));
            assert!(matches!(out, Outcome::Accepted { .. }));
            assert!(z.average_interval() <= max, "estimate exceeded the ceiling");
        }
        assert_eq!(z.average_interval(), max);
    }

    #[test]
    fn an_explicit_band_is_honoured_in_both_directions() {
        let mut z = ZeroCross::new_bounded(1_000, 800, 1_200);
        assert_eq!(z.bounds(), (800, 1_200));
        let p = FixedFilter::<2>;
        for _ in 0..50 {
            let _ = z.offer(z.blanking() + 1, true, ADV, &p, steady(true));
        }
        assert_eq!(z.average_interval(), 800);
        let mut z = ZeroCross::new_bounded(1_000, 800, 1_200);
        for _ in 0..50 {
            let _ = z.offer(10_000, true, ADV, &p, steady(true));
        }
        assert_eq!(z.average_interval(), 1_200);
    }

    #[test]
    fn the_blanking_window_can_never_narrow_past_half_the_floor() {
        // The feedback path in one assertion: blanking is half the estimate,
        // and the estimate cannot go below the floor, so the gate has a hard
        // minimum width no matter what the comparator does.
        let mut z = ZeroCross::new(2_777);
        let (min, _) = z.bounds();
        let p = FixedFilter::<4>;
        for _ in 0..200 {
            let _ = z.offer(z.blanking() + 1, true, ADV, &p, steady(true));
            assert!(z.blanking() >= min >> 1);
        }
    }

    #[test]
    fn refusals_do_not_disturb_the_average_or_the_history() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        let before = z.average_interval();
        assert_eq!(z.offer(10, true, ADV, &p, steady(true)), Outcome::TooEarly);
        assert_eq!(z.offer(15_000, true, ADV, &p, steady(false)), Outcome::Unstable);
        assert_eq!(z.average_interval(), before, "a refused edge changes nothing");
    }

    #[test]
    fn every_refusal_class_carries_its_own_counter() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        assert_eq!(z.counts(), (0, 0, 0));
        let _ = z.offer(10, true, ADV, &p, steady(true));
        assert_eq!(z.counts(), (0, 1, 0));
        let _ = z.offer(15_000, true, ADV, &p, steady(false));
        assert_eq!(z.counts(), (0, 1, 1));
        let _ = z.offer(15_000, true, ADV, &p, steady(true));
        assert_eq!(z.counts(), (1, 1, 1));
    }

    #[test]
    fn counters_saturate_by_wrapping_and_never_panic() {
        let mut z = ZeroCross::new(20_000);
        let p = FixedFilter::<12>;
        // drive the too-early counter hard; release builds have overflow
        // checks off but a debug host test would catch a plain `+= 1`
        for _ in 0..1000 {
            let _ = z.offer(0, true, ADV, &p, steady(true));
        }
        assert_eq!(z.counts().1, 1000);
    }

    #[test]
    fn extreme_intervals_do_not_wrap_or_panic() {
        let mut z = ZeroCross::new(u32::MAX);
        let p = FixedFilter::<2>;
        // blanking is MAX>>1; a count above it must be handled without overflow
        let out = z.offer(u32::MAX, true, 26, &p, steady(true));
        assert!(matches!(out, Outcome::Accepted { .. }));
        let mut z0 = ZeroCross::new(0);
        assert!(matches!(
            z0.offer(1, true, ADV, &p, steady(true)),
            Outcome::Accepted { .. }
        ));
    }
}
