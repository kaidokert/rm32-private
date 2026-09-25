//! Six-step commutation: sector table, direction mapping, floating-phase
//! comparator select, and the advance/wait arithmetic.
//!
//! Every computation here is reachable from the COM and COMP interrupt roots,
//! so it is **division-free by construction** — only shifts, adds and
//! multiplies. Indices are produced by bounded types, never by arithmetic that
//! could panic, so no bounds-check panic path is reachable either.
//!
//! Timebase: the reference feeds `ci` (commutation interval) and takes the
//! returned wait in TIM2 **half-microsecond** ticks. **firmware50 feeds
//! microseconds** -- `bemf::ZeroCross` keeps its intervals in µs from the
//! TIM17 1 MHz clock and passes them straight in, so every `ci`, advance and
//! wait in this module is µs here. The arithmetic is unit-agnostic (shifts of
//! a proportion), which is why the divergence is safe, but the header said
//! only the reference's half of it until the pre-run review of E159 pointed
//! at the contradiction.

/// A motor phase. Ordering is the logical A/B/C the controller reasons about;
/// the physical mapping is applied by [`Direction`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Phase {
    A = 0,
    B = 1,
    C = 2,
}

impl Phase {
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A commutation sector, 1..=6. Construction is checked once; afterwards the
/// value indexes the sector table without any possibility of a panic.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Step(u8);

impl Step {
    /// The first sector.
    pub const FIRST: Step = Step(1);

    /// Build a sector, returning `None` outside 1..=6 (no panic path).
    #[inline]
    pub const fn new(raw: u8) -> Option<Step> {
        if raw >= 1 && raw <= 6 { Some(Step(raw)) } else { None }
    }

    /// Build a sector, saturating into range. Infallible, panic-free.
    #[inline]
    pub const fn new_clamped(raw: u8) -> Step {
        if raw < 1 {
            Step(1)
        } else if raw > 6 {
            Step(6)
        } else {
            Step(raw)
        }
    }

    #[inline]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Next sector, wrapping 6 -> 1.
    #[inline]
    pub const fn next(self) -> Step {
        if self.0 >= 6 { Step(1) } else { Step(self.0 + 1) }
    }

    /// Zero-based table index, always 0..=5.
    #[inline]
    const fn idx(self) -> usize {
        (self.0 - 1) as usize
    }

    /// BEMF edge polarity for this sector: odd sectors look for a rising
    /// crossing, even sectors a falling one (reference `rising = step & 1`).
    #[inline]
    pub const fn rising(self) -> bool {
        self.0 & 1 == 1
    }
}

/// Which phases are driven and which floats, for one sector.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Sector {
    /// High-side, PWM-chopped.
    pub source: Phase,
    /// Low-side, held on.
    pub sink: Phase,
    /// Both FETs off — the BEMF sense phase.
    pub floating: Phase,
}

/// The reference six-step table, indexed by sector 1..=6.
/// Floating sequence is C, A, B, C, A, B.
///
/// Sized 8 and indexed with `& 7` (E131), like the COM root's plan table: a
/// `Step` is always 1..=6, but once the ISR roots stopped masking interrupts
/// around every atomic access the optimiser no longer proved that, and a
/// 6-entry index brought `panic_bounds_check` into TIM16, which the
/// fail-closed audit refuses. Entries 6 and 7 are unreachable copies of 1 and 2.
const SECTORS: [Sector; 8] = [
    Sector {
        source: Phase::A,
        sink: Phase::B,
        floating: Phase::C,
    }, // 1
    Sector {
        source: Phase::C,
        sink: Phase::B,
        floating: Phase::A,
    }, // 2
    Sector {
        source: Phase::C,
        sink: Phase::A,
        floating: Phase::B,
    }, // 3
    Sector {
        source: Phase::B,
        sink: Phase::A,
        floating: Phase::C,
    }, // 4
    Sector {
        source: Phase::B,
        sink: Phase::C,
        floating: Phase::A,
    }, // 5
    Sector {
        source: Phase::A,
        sink: Phase::C,
        floating: Phase::B,
    }, // 6
    Sector {
        source: Phase::A,
        sink: Phase::B,
        floating: Phase::C,
    }, // unreachable
    Sector {
        source: Phase::C,
        sink: Phase::B,
        floating: Phase::A,
    }, // unreachable
];

/// Look up a sector. Total function — no panic path: the masked index is
/// provably inside the 8-entry table.
#[inline]
pub const fn sector(step: Step) -> Sector {
    SECTORS[step.idx() & 7]
}

/// Physical wiring orientation. This is a *type-level* policy choice, not a
/// runtime flag or a cargo feature.
pub trait Direction {
    /// Map a logical phase to the physically wired one.
    fn phase(logical: Phase) -> Phase;
    /// Map a logical sector to the physically applied one.
    fn step(logical: Step) -> Step;
}

/// Phases and sector order as wired.
#[derive(Copy, Clone, Debug)]
pub struct Forward;

impl Direction for Forward {
    #[inline]
    fn phase(logical: Phase) -> Phase {
        logical
    }
    #[inline]
    fn step(logical: Step) -> Step {
        logical
    }
}

/// The qualified bench orientation: physical A and B are swapped and the
/// sector order is reversed (1->4, 2->3, 3->2, 4->1, 5->6, 6->5), giving the
/// physical sequence 4,3,2,1,6,5. The controller keeps reasoning in logical
/// A/B/C; only the output mapping changes.
#[derive(Copy, Clone, Debug)]
pub struct Reverse;

impl Direction for Reverse {
    #[inline]
    fn phase(logical: Phase) -> Phase {
        match logical {
            Phase::A => Phase::B,
            Phase::B => Phase::A,
            Phase::C => Phase::C,
        }
    }
    #[inline]
    fn step(logical: Step) -> Step {
        // Table-free, branch-only: no indexing, no arithmetic that can panic.
        match logical.get() {
            1 => Step(4),
            2 => Step(3),
            3 => Step(2),
            4 => Step(1),
            5 => Step(6),
            _ => Step(5),
        }
    }
}

/// COMP2 `INMSEL` field value selecting the floating phase's input.
/// Reference: `6 + physical_phase(floating)`, i.e. A->6, B->7, C->8.
#[inline]
pub fn comparator_inmsel<D: Direction>(step: Step) -> u8 {
    6 + D::phase(sector(step).floating) as u8
}

/// How far ahead of the half-cycle point to commutate.
///
/// `advance = (ci * level) >> 6`, so `level` is the numerator of a /64
/// fraction: 16 -> 25% of `ci`, 26 -> ~40.6%.
pub trait AdvancePolicy {
    /// Advance level for the currently applied duty (tenths of a percent).
    fn level(&self, duty_tenths: u16) -> u32;
}

/// A single fixed advance level — what the qualified image uses below the
/// high-duty band (default level 16).
#[derive(Copy, Clone, Debug)]
pub struct FixedAdvance<const LEVEL: u32>;

impl<const LEVEL: u32> AdvancePolicy for FixedAdvance<LEVEL> {
    #[inline]
    fn level(&self, _duty_tenths: u16) -> u32 {
        LEVEL
    }
}

/// The reference default: 25% advance.
pub type DefaultAdvance = FixedAdvance<16>;

/// `advance = (ci * level) >> 6` — shift only, no division.
///
/// **Split into whole and fractional sixty-fourths, not `saturating_mul`.**
/// On the M0 a `saturating_mul` needs the high word of the product to detect
/// overflow, and once the compiler can see `level` it lowers that to the
/// 64-bit helper `__aeabi_lmul` -- which the fail-closed ISR audit forbids in
/// `ADC_COMP` and refused at link time (E058). `ci = 64q + r` gives
/// `ci * level >> 6 = q * level + (r * level >> 6)` exactly, and with `level`
/// clamped to 64 neither term can overflow (`q * 64 <= ci`). The clamp changes
/// no wait: any level of 64 or more makes `advance >= ci`, so `wait_time`
/// saturates to 0 either way. `advance_of_split_matches_the_wide_product`
/// checks the equality against 64-bit arithmetic.
#[inline]
pub const fn advance_of(ci: u32, level: u32) -> u32 {
    let level = if level > 64 { 64 } else { level };
    (ci >> 6) * level + (((ci & 63) * level) >> 6)
}

/// `wait = (ci >> 1) - advance`, saturating so it can never wrap negative.
#[inline]
pub const fn wait_time(ci: u32, level: u32) -> u32 {
    (ci >> 1).saturating_sub(advance_of(ci, level))
}

/// What is left of the blanking window, `blanking` µs after a crossing that
/// was `since_zc` µs ago (E134). Saturating: a commutation that fired late
/// shortens the blank instead of pushing the floor out past it.
///
/// The COM root keeps the comparator line masked for this long instead of
/// arming it at the commutation. Nothing inside the window can be accepted --
/// the blanking gate refuses it -- so those dispatches are pure cost, and at
/// 25% they were a quarter of them.
#[inline]
pub const fn blank_remaining(blanking: u32, since_zc: u32) -> u32 {
    blanking.saturating_sub(since_zc)
}

/// Shortest blank worth arming the one-shot for, µs (E134). Below it the root
/// arms the line at once: a timer round trip costs more than the few edges
/// such a sliver could carry.
pub const BLANK_ARM_MIN_US: u32 = 16;

/// Blend a fresh zero-cross pair into the running interval estimate:
/// `(ci + ((last + this) >> 1)) >> 1`.
///
/// Both adds saturate. The release profile builds with `overflow-checks =
/// false`, so a plain `+` would wrap silently here and hand the commutation
/// one-shot a garbage interval; saturating fails in the slow direction, which
/// the COM clamp and the tracking watchdog both catch. The saturation point is
/// far outside any physically reachable interval — at 10% the interval is
/// ~20000 half-microseconds — so this changes no real behavior.
#[inline]
pub const fn blend_interval(ci: u32, last_zc: u32, this_zc: u32) -> u32 {
    let pair = last_zc.saturating_add(this_zc) >> 1;
    (ci.saturating_add(pair)) >> 1
}

/// The COM one-shot accepts only this range of half-microsecond timeouts.
pub const COM_TIMEOUT_MIN: u32 = 200;
pub const COM_TIMEOUT_MAX: u32 = 8000;

/// Clamp a computed wait into the COM timer's accepted window.
#[inline]
pub const fn clamp_com_timeout(wait: u32) -> u32 {
    if wait < COM_TIMEOUT_MIN {
        COM_TIMEOUT_MIN
    } else if wait > COM_TIMEOUT_MAX {
        COM_TIMEOUT_MAX
    } else {
        wait
    }
}

/// The reference's six-slot commutation-interval ring and the average it
/// derives (E100).
///
/// AM32 stores the blended `commutation_interval` into
/// `commutation_intervals[step - 1]` at every commutation (`main.c:887`,
/// transcribed as `minz/core/src/am32_control.rs:77-80`), and after each
/// commutation computes `e_com_time = (sum + 4) >> 1` and
/// `average_interval = e_com_time / 3` (`main.c:2159`, `:2283`;
/// `minz/core/src/am32_loop.rs:267-279`), in half-µs. The qualified image's
/// reverse blank arms on **that** average, `>= 1500` half-µs
/// (`binz/examples/support/core_bench.rs:1829-1841`), and seeds all six slots
/// with the seed interval at handover (`core_bench.rs:2805-2812`). Because the
/// ring lags an accelerating rotor, the blank keeps arming for several
/// commutations after transfer (`REVERSEBLANK arms=4` at every rate-curve
/// rung); firmware50 had gated it on the per-crossing blend, which drops below
/// the threshold after one commutation (`blank_arms=1`, E099).
///
/// Division-free: with intervals in µs the reference's condition
/// `((2*sum_us + 4) >> 1) / 3 >= 1500` is exactly `sum_us >= 4498`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SixSlot {
    slots: [u32; 6],
    sum: u32,
}

/// Six-slot sum (µs) at or above which the reverse blank arms: the
/// reference's `average_interval >= 1500` half-µs.
pub const REVERSE_BLANK_SUM_US: u32 = 4_498;

impl SixSlot {
    /// All six slots at the handover seed interval, as the reference seeds.
    #[must_use]
    pub const fn seeded(interval_us: u32) -> Self {
        let i = if interval_us > 1_000_000 {
            1_000_000
        } else {
            interval_us
        };
        Self {
            slots: [i; 6],
            sum: i * 6,
        }
    }

    /// Store `interval_us` in the slot for `step` (1..=6), as
    /// `commutation_intervals[step - 1] = commutation_interval`.
    #[inline]
    pub fn push(&mut self, step: u8, interval_us: u32) {
        let i = if interval_us > 1_000_000 {
            1_000_000
        } else {
            interval_us
        };
        // No `%`: this runs in the COM ISR root, where a modulo would be a
        // soft division on the M0. Out-of-range steps use slot 0.
        let k = match step {
            1..=6 => (step - 1) as usize,
            _ => 0,
        };
        self.sum = self.sum - self.slots[k] + i;
        self.slots[k] = i;
    }

    #[must_use]
    pub const fn sum(&self) -> u32 {
        self.sum
    }

    /// Does the reference's blank condition hold on this ring?
    #[inline]
    #[must_use]
    pub const fn reverse_blank_due(&self) -> bool {
        self.sum >= REVERSE_BLANK_SUM_US
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference's own arithmetic, in half-µs, for comparison.
    fn reference_average_half_us(sum_us: u32) -> u32 {
        let sum_half = sum_us * 2;
        ((sum_half + 4) >> 1) / 3
    }

    #[test]
    fn blank_condition_equals_the_references_for_every_sum() {
        for sum in 0..20_000u32 {
            let mut r = SixSlot::seeded(0);
            r.sum = sum;
            assert_eq!(
                r.reverse_blank_due(),
                reference_average_half_us(sum) >= 1500,
                "sum {sum}"
            );
        }
    }

    #[test]
    fn a_seeded_ring_lags_an_accelerating_rotor_for_several_commutations() {
        // Handover at a ~770 us seed, then the rotor speeds up toward ~700 us
        // blended intervals: the reference's ring stays at or above the blank
        // threshold for several commutations, not one.
        let mut r = SixSlot::seeded(770);
        assert!(r.reverse_blank_due());
        let mut arms = 0;
        let mut step = 5u8;
        for ci in [745u32, 730, 718, 708, 700, 695, 690, 688] {
            if r.reverse_blank_due() {
                arms += 1;
            }
            r.push(step, ci);
            step = if step == 6 { 1 } else { step + 1 };
        }
        assert!(arms >= 3, "arms {arms}");
        assert!(!r.reverse_blank_due(), "settles below once the ring has turned over");
    }

    #[test]
    fn push_replaces_one_slot_and_keeps_the_sum_exact() {
        let mut r = SixSlot::seeded(800);
        assert_eq!(r.sum(), 4_800);
        r.push(1, 500);
        assert_eq!(r.sum(), 4_500);
        r.push(1, 600);
        assert_eq!(r.sum(), 4_600, "same step overwrites the same slot");
        r.push(7, 800);
        assert_eq!(r.sum(), 4_800, "an out-of-range step uses slot 0 (600 -> 800)");
        r.push(6, u32::MAX);
        assert_eq!(r.sum(), 4_800 - 800 + 1_000_000, "clamped to 1 s");
    }

    #[test]
    fn step_construction_is_bounded() {
        assert!(Step::new(0).is_none());
        assert!(Step::new(7).is_none());
        for raw in 1..=6u8 {
            assert_eq!(Step::new(raw).unwrap().get(), raw);
        }
        assert_eq!(Step::new_clamped(0).get(), 1);
        assert_eq!(Step::new_clamped(9).get(), 6);
    }

    #[test]
    fn step_wraps_six_to_one() {
        let mut s = Step::FIRST;
        let mut seen = alloc_seq();
        for _ in 0..6 {
            seen.push(s.get());
            s = s.next();
        }
        assert_eq!(seen, [1, 2, 3, 4, 5, 6]);
        assert_eq!(s, Step::FIRST, "sixth next() must wrap to 1");
    }

    fn alloc_seq() -> std::vec::Vec<u8> {
        std::vec::Vec::new()
    }

    #[test]
    fn sector_table_matches_reference() {
        let expect = [
            (Phase::A, Phase::B, Phase::C),
            (Phase::C, Phase::B, Phase::A),
            (Phase::C, Phase::A, Phase::B),
            (Phase::B, Phase::A, Phase::C),
            (Phase::B, Phase::C, Phase::A),
            (Phase::A, Phase::C, Phase::B),
        ];
        for (i, (src, snk, flt)) in expect.iter().enumerate() {
            let s = sector(Step::new((i + 1) as u8).unwrap());
            assert_eq!(s.source, *src, "sector {} source", i + 1);
            assert_eq!(s.sink, *snk, "sector {} sink", i + 1);
            assert_eq!(s.floating, *flt, "sector {} floating", i + 1);
            assert_ne!(s.source, s.sink);
            assert_ne!(s.source, s.floating);
            assert_ne!(s.sink, s.floating);
        }
    }

    #[test]
    fn floating_sequence_is_c_a_b_repeating() {
        let got: std::vec::Vec<Phase> = (1..=6u8).map(|s| sector(Step::new(s).unwrap()).floating).collect();
        assert_eq!(got, [Phase::C, Phase::A, Phase::B, Phase::C, Phase::A, Phase::B]);
    }

    #[test]
    fn rising_alternates_by_sector_parity() {
        for s in 1..=6u8 {
            let step = Step::new(s).unwrap();
            assert_eq!(step.rising(), s % 2 == 1, "sector {s}");
        }
    }

    #[test]
    fn reverse_step_map_is_the_documented_permutation() {
        let pairs = [(1, 4), (2, 3), (3, 2), (4, 1), (5, 6), (6, 5)];
        for (logical, physical) in pairs {
            assert_eq!(
                Reverse::step(Step::new(logical).unwrap()).get(),
                physical,
                "logical {logical}"
            );
        }
    }

    #[test]
    fn reverse_step_map_is_an_involution_and_a_bijection() {
        let mut hit = [false; 7];
        for l in 1..=6u8 {
            let p = Reverse::step(Step::new(l).unwrap());
            assert!(!hit[p.get() as usize], "physical {} produced twice", p.get());
            hit[p.get() as usize] = true;
            // applying it twice returns the original
            assert_eq!(Reverse::step(p).get(), l);
        }
    }

    #[test]
    fn reverse_swaps_a_and_b_only() {
        assert_eq!(Reverse::phase(Phase::A), Phase::B);
        assert_eq!(Reverse::phase(Phase::B), Phase::A);
        assert_eq!(Reverse::phase(Phase::C), Phase::C);
        // involution
        for p in [Phase::A, Phase::B, Phase::C] {
            assert_eq!(Reverse::phase(Reverse::phase(p)), p);
        }
    }

    #[test]
    fn forward_is_identity() {
        for s in 1..=6u8 {
            let step = Step::new(s).unwrap();
            assert_eq!(Forward::step(step), step);
        }
        for p in [Phase::A, Phase::B, Phase::C] {
            assert_eq!(Forward::phase(p), p);
        }
    }

    #[test]
    fn inmsel_is_six_plus_physical_floating_phase() {
        // Forward: floating C,A,B,... -> 8,6,7,8,6,7
        let fwd: std::vec::Vec<u8> = (1..=6u8)
            .map(|s| comparator_inmsel::<Forward>(Step::new(s).unwrap()))
            .collect();
        assert_eq!(fwd, [8, 6, 7, 8, 6, 7]);
        // Reverse swaps A<->B, so 6<->7; C (8) unchanged.
        let rev: std::vec::Vec<u8> = (1..=6u8)
            .map(|s| comparator_inmsel::<Reverse>(Step::new(s).unwrap()))
            .collect();
        assert_eq!(rev, [8, 7, 6, 8, 7, 6]);
        for v in fwd.iter().chain(rev.iter()) {
            assert!((6..=8).contains(v), "INMSEL {v} out of range");
        }
    }

    #[test]
    fn advance_is_the_sixty_fourth_fraction() {
        // level 16 -> ci/4 ; level 26 -> 26*ci/64
        assert_eq!(advance_of(1024, 16), 256);
        assert_eq!(advance_of(1024, 26), 416);
        assert_eq!(advance_of(0, 16), 0);
    }

    /// The split form equals the exact 64-bit product for every level the
    /// firmware uses and far beyond, over a sweep of intervals up to `u32::MAX`
    /// (where the old `saturating_mul` saturated and this does not need to).
    #[test]
    fn advance_of_split_matches_the_wide_product() {
        let cis = (0..200_000u32).chain([u32::MAX, u32::MAX - 1, u32::MAX / 64, u32::MAX / 64 + 63, 1 << 31]);
        for ci in cis {
            for level in 0..=64u32 {
                let wide = ((ci as u64 * level as u64) >> 6) as u32;
                assert_eq!(advance_of(ci, level), wide, "ci={ci} level={level}");
            }
        }
        // Levels above 64 clamp, and the wait they produce is unchanged: zero.
        for ci in [0u32, 1, 833, 20_000, u32::MAX] {
            assert_eq!(wait_time(ci, 65), wait_time(ci, 64));
            assert_eq!(wait_time(ci, 1000), 0);
        }
    }

    /// The floor sits `advance` after the commutation, which is what the COM
    /// root holds the line masked for (E134), and a late commutation eats into
    /// it rather than moving it.
    #[test]
    fn the_blank_left_after_a_commutation_is_the_advance() {
        for ci in [141u32, 235, 174, 80, 48] {
            let wait = wait_time(ci, 20);
            let left = blank_remaining(ci >> 1, wait);
            assert_eq!(left, advance_of(ci, 20), "ci={ci}");
            // 5 µs late: the blank is 5 µs shorter, and the floor does not move.
            assert_eq!(blank_remaining(ci >> 1, wait + 5), left - 5, "ci={ci}");
        }
        // Past the floor already: no blank, never a wrap.
        assert_eq!(blank_remaining(70, 70), 0);
        assert_eq!(blank_remaining(70, 5_000), 0);
        assert_eq!(blank_remaining(0, 0), 0);
    }

    #[test]
    fn wait_is_half_cycle_minus_advance_and_never_wraps() {
        // level 16: wait = ci/2 - ci/4 = ci/4
        assert_eq!(wait_time(1024, 16), 256);
        // level 26: 512 - 416 = 96
        assert_eq!(wait_time(1024, 26), 96);
        // A level >= 32 would make advance exceed the half cycle: must saturate
        // to zero, not underflow.
        assert_eq!(wait_time(1024, 32), 0);
        assert_eq!(wait_time(1024, 64), 0);
        assert_eq!(wait_time(0, 16), 0);
        // huge ci must not panic or wrap
        let _ = wait_time(u32::MAX, 26);
    }

    #[test]
    fn default_advance_policy_is_level_sixteen_at_every_duty() {
        let p: DefaultAdvance = FixedAdvance::<16>;
        for duty in [0u16, 40, 100, 250, 500] {
            assert_eq!(p.level(duty), 16);
        }
    }

    #[test]
    fn blend_averages_toward_the_new_pair() {
        // equal inputs are a fixed point
        assert_eq!(blend_interval(1000, 1000, 1000), 1000);
        // a faster pair pulls the estimate down, by half the difference
        assert_eq!(blend_interval(1000, 800, 800), 900);
        assert_eq!(blend_interval(1000, 700, 900), 900);
    }

    #[test]
    fn com_timeout_clamps_to_the_timer_window() {
        assert_eq!(clamp_com_timeout(0), COM_TIMEOUT_MIN);
        assert_eq!(clamp_com_timeout(199), COM_TIMEOUT_MIN);
        assert_eq!(clamp_com_timeout(200), 200);
        assert_eq!(clamp_com_timeout(4000), 4000);
        assert_eq!(clamp_com_timeout(8000), 8000);
        assert_eq!(clamp_com_timeout(8001), COM_TIMEOUT_MAX);
        assert_eq!(clamp_com_timeout(u32::MAX), COM_TIMEOUT_MAX);
    }
}
