//! Protections. Every threshold here is transcribed from the qualified image
//! and none of them may be relaxed to make a run pass.
//!
//! All comparisons are **division-free** (cross-products and shifts only), and
//! all of them are evaluated from interrupt context, so nothing in this module
//! may panic: no indexing, no `unwrap`, no unchecked arithmetic.
//!
//! Timestamps are free-running microseconds in a `u32` that wraps every ~71
//! minutes. Ages are always computed with `wrapping_sub`, and a cross-ISR
//! reference is **loaded before** `now` so a late reference can never appear to
//! be in the future and manufacture a false trip.

/// Why a run stopped. Values are wire-compatible with the reference so
/// captures remain comparable.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Reason {
    /// Whole-campaign deadline reached.
    CampaignDeadline = 1,
    /// This run's requested window elapsed — the normal, expected stop.
    SegmentDeadline = 2,
    /// Control tick did not run within its allowance.
    TickGap = 3,
    /// No ADC feedback frame within its allowance.
    FeedbackStale = 4,
    /// A raw phase-current code was outside the admissible band.
    Current = 5,
    /// Bus or VREF code implausible / below the absolute floor.
    Bus = 6,
    /// Gate driver asserted nFAULT.
    Driver = 7,
    /// Accepted-commutation watchdog: an event was missing or misordered.
    Tracking = 8,
    /// Host asked for a stop.
    HostAbort = 9,
    /// Handoff seed failed its validity/reserve check.
    InvalidSeed = 10,
    /// ADC/DMA producer stalled or published an invalid frame.
    AdcTimeout = 11,
    /// Full electrical cycle outside its plausible duration band.
    CycleTiming = 12,
    /// Comparator dispatch storm.
    CompStorm = 13,
    /// One COMP handler call exceeded its 50 µs budget (binz's per-call
    /// handler-overrun stop, `driven_irq_live.rs:310-318`; E107).
    HandlerOverrun = 14,
    /// **An exhausted deadline: COMP armed a commutation with the wait
    /// already spent** (campaign 8). The one-shot then fires as soon as it can
    /// rather than when the crossing asked for, so that commutation's angle is
    /// whatever the latency happened to be. This was a *counter*
    /// (`late_arms`) through campaigns 5-7 and I read it as zero everywhere;
    /// it was non-zero in 4 of 588 captures, one of them a qualifying 45% run.
    /// A counter nobody stops on is a counter nobody checks, so it is a stop.
    ///
    /// Coverage: the **closed loop only**, from the instant the loop is closed
    /// to the stop. Not because the check is gated on the stage, but because
    /// the counter itself is: `late_arms` rises only inside
    /// `if S.com().active` (`roots.rs`), and the handover zeroes it
    /// (`bin/board.rs`, `com_handover`/`det_install`). So the earlier stages
    /// neither count nor stop -- an earlier draft of this comment claimed
    /// they count, and an independent review falsified it (E158).
    /// Shutdown: the ordinary protection route, so `safe_off` then the report
    /// with this code; the run is over, not degraded.
    LateArm = 15,
    /// **The blanking window latched a comparator edge** (campaign 8): phase 3
    /// found `EXTI` pending when it opened the line. A latched flag carries no
    /// timestamp, so serving it dates the crossing at the gate rather than at
    /// the rotor -- minz's FALCON self-lock class. E134 introduced the counter
    /// and it has read zero in every capture taken since (E138's probe and
    /// every rung run through campaign 8), which is exactly why a non-zero
    /// reading must end the run rather than be averaged into a report.
    ///
    /// Coverage and shutdown as [`Reason::LateArm`].
    BlankLatched = 16,
    /// Averaged over-current (second block) or absolute bus collapse.
    AverageCurrent = 25,
    /// Fast bus sag: three consecutive scans below the relative floor.
    FastBusSag = 26,
    /// Instantaneous phase peak. **NOT IMPLEMENTED AND NEVER RAISED.**
    ///
    /// There is no threshold, no accumulator and no call site: the variant and
    /// its wire code exist, and nothing in `src/` or `bin/` can produce it.
    /// Two reviews in a row have listed it among "the stops a loss-of-lock
    /// surge would arrive as" -- one of the three named stops can never fire
    /// (E186 SS6), and I had repeated that. **This firmware has no
    /// instantaneous or peak current protection of any kind**; the only
    /// current stop is [`Self::AverageCurrent`], which judges 10.1 ms block
    /// means at a 4 A allowance. The nearest thing to peak evidence is
    /// `AverageCurrent::worst_residual`, now reported (E187) -- the worst
    /// *block*, not a peak.
    ///
    /// Kept rather than deleted because the wire code is part of the capture
    /// format's history; do not cite it as coverage.
    PhasePeak = 27,
    /// **An unrecognised guard code.** The decode's fallback, so that an
    /// unknown stop can never be read as the success code (E186 SS6).
    UnknownGuard = 28,
}

impl Reason {
    #[inline]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ---------------------------------------------------------------------------
// Raw feedback validation (runs in the DMA interrupt; division-free)
// ---------------------------------------------------------------------------

/// Raw ADC codes of one coherent 5-channel scan, in logical order.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RawScan {
    pub phase_a: u16,
    pub phase_b: u16,
    pub phase_c: u16,
    pub bus: u16,
    pub vref: u16,
}

/// Absolute bus floor expressed as a raw cross-product constant: a bus code is
/// rejected when `bus * vcal < BUS_FLOOR_NUM * vref` (≈ 8400 mV).
pub const BUS_FLOOR_NUM: u32 = 963;

/// Admissible phase-current code band when the averaging current policy is
/// **not** installed.
pub const PHASE_CODE_LOW: u16 = 848;
pub const PHASE_CODE_HIGH: u16 = 3248;

/// Top of the ADC range; an exact rail is always implausible.
pub const ADC_RAIL: u16 = 4095;

/// How phase codes are judged.
///
/// The calibration datum lives *in* the variant that uses it. It used to be a
/// separate parameter, which meant every caller on the other path had to pass
/// a value that was silently ignored -- the live one passed `0`. Putting it
/// here makes the meaningless call unconstructible instead of merely harmless.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PhaseCodePolicy {
    /// No block averaging: require each phase code inside
    /// `[PHASE_CODE_LOW, PHASE_CODE_HIGH]`, and enforce the absolute bus floor
    /// against the VREFINT calibration.
    Band { vcal: u32 },
    /// Block averaging installed: phase codes are **not vetoed at all**.
    ///
    /// A rail is deliberately *retained* in the block average rather than
    /// rejecting the scan, so there is no per-code check to make and this
    /// performs only the VREF plausibility test.
    ///
    /// Named for what it does. The previous name, `RailOnly`, implied a rail
    /// check, and the implementation duly tested `code > ADC_RAIL` -- which is
    /// unsatisfiable for a 12-bit conversion, so it was three comparisons that
    /// could never fire dressed up as a protection.
    RetainRails,
}

/// Validate one raw scan. Division-free; returns the first fault found.
#[must_use = "a dropped validation verdict silently accepts an implausible frame"]
#[inline]
pub fn validate_raw_feedback(scan: &RawScan, policy: PhaseCodePolicy) -> Option<Reason> {
    // VREF must be plausible before it is used as a divisor-free reference.
    // This is the one check both policies share.
    if scan.vref == 0 || scan.vref >= ADC_RAIL {
        return Some(Reason::Bus);
    }
    if let PhaseCodePolicy::Band { vcal } = policy {
        let phases = [scan.phase_a, scan.phase_b, scan.phase_c];
        let mut i = 0;
        while i < phases.len() {
            let c = phases[i];
            // `contains` on an inclusive range is two `#[inline]`
            // comparisons — no division, no panic, no call left after LTO.
            if !(PHASE_CODE_LOW..=PHASE_CODE_HIGH).contains(&c) {
                return Some(Reason::Current);
            }
            i += 1;
        }
        // The absolute bus floor is only enforced on this path.
        if (scan.bus as u32) * vcal < BUS_FLOOR_NUM * (scan.vref as u32) {
            return Some(Reason::Bus);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Fast bus sag: three consecutive scans below 95% of a same-wake baseline
// ---------------------------------------------------------------------------

/// Baseline bus/VREF captured before drive, on the same wake.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BusReference {
    pub bus: u16,
    pub vref: u16,
}

/// Reporting fractions for [`BusDepth`], per mille of the pre-run reference.
///
/// **These are not thresholds and nothing stops on them.** They exist because
/// E218 tried to derive a slow-droop stop from quantities this firmware has
/// never recorded, and got both of them from the wrong population: its
/// "healthy 50%" figure was a 40% capture, and its 900 line would not have
/// fired on any of the ten confirmed current-limited runs this bench has
/// produced (E221 SS2, E222 SS1, verified in E223). A stop needs the
/// distribution first, at more than one rung.
pub const DEPTH_FRACTIONS: [u32; 4] = [995, 990, 985, 980];

/// Fractions for the **raw-scan** depth observer (E284), bracketing the sharp
/// guard's own trip line.
///
/// `FastBusSag` compares an **8-scan mean** against 95% of a 207 ms EWMA, and
/// the existing [`DEPTH_FRACTIONS`] observe that same mean -- so neither can
/// see a dip shorter than the mean's window, and neither has a bin at 950 where
/// the guard actually trips. Measured consequence: **every run takes a raw scan
/// past the line**, pass or fail. `bus_min` against `0.95*filt_bus` is 44 codes
/// past on the 575 injection run and **+17 / +36 / +59** on the three 550 runs,
/// the two that passed included.
///
/// So depth does not discriminate and duration is unmeasured. 950 is the trip
/// line; the rest bracket the measured minima (`bus_min/filt_bus` ran
/// 90.0-93.5% across that cohort).
pub const RAW_DEPTH_FRACTIONS: [u32; 4] = [970, 950, 930, 910];

/// Index of the trip line within [`RAW_DEPTH_FRACTIONS`]; its `longest` is the
/// quantity that discriminates a short transient from a sustained droop.
pub const RAW_DEPTH_TRIP_IX: usize = 1;

/// Fractions for the **mean-against-the-guard's-own-reference** observer
/// (Q60-1), bracketing the trip line from above.
///
/// Neither existing observer watches the quantity `FastBusSag` decides on:
/// [`DEPTH_FRACTIONS`] judges the mean against the **pre-run baseline**, and
/// [`RAW_DEPTH_FRACTIONS`] judges the **raw scan** against `filtered()`. The
/// guard judges the **mean** against `filtered()` at 950, and nothing counted
/// it -- so "how often, and for how long, did the deciding quantity approach
/// its line" had no whole-run answer at all. 950 is included as the last bin
/// so the observer straddles the guard exactly; the three above it show the
/// approach, including the EWMA-lag excursions that SAGQ4 measured at ~970.
pub const MEAN_DEPTH_FRACTIONS: [u32; 4] = [990, 970, 960, 950];

const _: () = assert!(
    MEAN_DEPTH_FRACTIONS[3] == 950,
    "the last bin must be the guard's own line"
);
const _: () = assert!(MEAN_DEPTH_FRACTIONS[0].is_multiple_of(5) && MEAN_DEPTH_FRACTIONS[1].is_multiple_of(5));
const _: () = assert!(MEAN_DEPTH_FRACTIONS[2].is_multiple_of(5) && MEAN_DEPTH_FRACTIONS[3].is_multiple_of(5));

const _: () = assert!(RAW_DEPTH_FRACTIONS[RAW_DEPTH_TRIP_IX] == 950);
// The raw bins must be strictly deeper than the mean bins, or the two observers
// are measuring the same thing twice.
const _: () = assert!(RAW_DEPTH_FRACTIONS[0] < DEPTH_FRACTIONS[3]);

// `BusDepth::with_fractions` precomputes `frac / 5` (E303), so every fraction
// must be an exact multiple of 5 or the scaled comparison silently shifts the
// line. E291 relied on this and only said so in a comment; it is checked now.
const _: () = assert!(DEPTH_FRACTIONS[0].is_multiple_of(5) && DEPTH_FRACTIONS[1].is_multiple_of(5));
const _: () = assert!(DEPTH_FRACTIONS[2].is_multiple_of(5) && DEPTH_FRACTIONS[3].is_multiple_of(5));
const _: () = assert!(RAW_DEPTH_FRACTIONS[0].is_multiple_of(5) && RAW_DEPTH_FRACTIONS[1].is_multiple_of(5));
const _: () = assert!(RAW_DEPTH_FRACTIONS[2].is_multiple_of(5) && RAW_DEPTH_FRACTIONS[3].is_multiple_of(5));

/// How deep, and for how long, the bus actually sits below its pre-run
/// reference -- the distribution a slow-droop stop would have to be chosen from.
///
/// For each fraction in [`DEPTH_FRACTIONS`] it keeps the **number of scans
/// below** and the **longest consecutive run below**. Both, because a count
/// alone cannot separate one deep dip from a sustained droop, and a streak
/// alone cannot say how often. A minimum -- which is what E218 proposed to
/// report -- answers neither, and is not comparable to a line defined by a
/// persistence (E222 SS3).
///
/// Judged as a **VREF-normalised cross-product**, the form
/// [`FastBusSag::observe`] and the absolute floor already use: an ADC code is a
/// ratio to VDDA, so a bare `bus/ref` conflates rail drift with bus droop, and
/// a division here would put `__aeabi_uidiv` in the foreground loop on every
/// scan (E222 SS4/SS5).
///
/// This runs in **thread mode**, not in any ISR -- every bus judgement in this
/// firmware does. `isr_diff.py` and `isr_audit.py` cannot see it, which is
/// stated here because E218 predeclared exactly those gates as its cohort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusDepth {
    below: [u32; 4],
    run: [u32; 4],
    longest: [u32; 4],
    /// Which fraction set this instance judges against: the mean observer uses
    /// [`DEPTH_FRACTIONS`], the raw one [`RAW_DEPTH_FRACTIONS`] (E284). Carried
    /// per instance so the two cannot be confused at the call site.
    fracs: [u32; 4],
    /// `fracs[i] / 5`, **precomputed at construction** (E303).
    ///
    /// E291 scaled both sides of the cross-product by 1/5 to fix a real
    /// overflow, and wrote `rhs * (self.fracs[i] / 5)` inline. `fracs` is a
    /// struct field, so the compiler cannot fold that division: on thumbv6m it
    /// lowered to `bl __aeabi_uidiv` **inside the four-iteration loop**, and
    /// `scan_pass` calls `observe` twice per scan -- eight soft divisions per
    /// ADC scan at ~9.9 kHz, in the foreground that hosts every bus judgement.
    ///
    /// Measured cost of that mistake, rung 150, matched window:
    /// `loop_iters_closed` **3 976 438 -> 2 058 998 (-48%)** and
    /// `loop_gap_max_us` **151 -> 225 (+49%)**. The module comment two hundred
    /// lines above this one warns against exactly it: "a division here would
    /// put `__aeabi_uidiv` in the foreground loop on every scan".
    ///
    /// Precomputing costs four const divisions per run and restores the
    /// multiply-only inner loop. Every fraction is asserted a multiple of 5, so
    /// this is exact.
    scaled: [u32; 4],
}

impl Default for BusDepth {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDepth {
    /// The observer for the guard's own input: the 8-scan rail mean.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_fractions(DEPTH_FRACTIONS)
    }

    /// The observer for the **raw scan** (E284), which is the only one that can
    /// see a dip shorter than the mean's 8-scan window.
    #[must_use]
    /// The observer for the guard's own decision variable (Q60-1): the 8-tap
    /// mean against `FastBusSag::filtered()`, straddling the 950 trip line.
    pub const fn new_mean_vs_filt() -> Self {
        Self::with_fractions(MEAN_DEPTH_FRACTIONS)
    }

    pub const fn new_raw() -> Self {
        Self::with_fractions(RAW_DEPTH_FRACTIONS)
    }

    #[must_use]
    pub const fn with_fractions(fracs: [u32; 4]) -> Self {
        Self {
            below: [0; 4],
            run: [0; 4],
            longest: [0; 4],
            scaled: [fracs[0] / 5, fracs[1] / 5, fracs[2] / 5, fracs[3] / 5],
            fracs,
        }
    }

    /// The fraction this index judges against, so a report can print the
    /// threshold beside the counts rather than leaving the reader to assume it
    /// (the `dep970_n` lesson, E246).
    #[must_use]
    pub const fn fraction(&self, i: usize) -> u32 {
        self.fracs[i]
    }

    /// Observe one scan's rail mean against the pre-run reference.
    ///
    /// Never returns a verdict: this is an observer. A zero reference is
    /// ignored rather than treated as a fault, because the fail-closed
    /// judgement belongs to the guards that actually stop the run.
    pub fn observe(&mut self, bus: u16, vref: u16, ref_bus: u16, ref_vref: u16) {
        if ref_bus == 0 || vref == 0 {
            return;
        }
        // bus/vref < (num/1000) * (ref_bus/ref_vref), cross-multiplied.
        //
        // **Both sides are scaled by 1/5** (E291). The natural form multiplies
        // by 1_000, and `4095 * 4095 * 1_000 = 16_769_025_000` is **3.9x past
        // `u32::MAX`** -- so the doc comment above, which borrowed
        // `FastBusSag`'s `* 100` bound of `1_676_902_500 < 2^31`, asserted a
        // guarantee this function does not have. It was not theoretical: the
        // largest `lhs` in the capture corpus is `2058 * 1508 * 1000 =
        // 3_103_464_000`, **72% of `u32::MAX`**, and it wraps at bus code 2849
        // -- about 27.7 V on this bench's scale, inside the board's 6-50 V
        // rating. A wrap here silently inverts the comparison and the counts
        // become garbage with no symptom.
        //
        // Scaling the two constants (not the products) is exact for any input,
        // because every fraction is a multiple of 5 -- asserted below. Worst
        // case becomes `4095 * 4095 * 200 = 3_353_805_000 < u32::MAX`, and the
        // right side is strictly smaller, so neither can wrap at 12 bits.
        const SCALE: u32 = 200; // 1_000 / 5
        let lhs = u32::from(bus) * u32::from(ref_vref) * SCALE;
        let rhs = u32::from(ref_bus) * u32::from(vref);
        let mut i = 0;
        while i < self.fracs.len() {
            if lhs < rhs * self.scaled[i] {
                self.below[i] = self.below[i].saturating_add(1);
                self.run[i] = self.run[i].saturating_add(1);
                if self.run[i] > self.longest[i] {
                    self.longest[i] = self.run[i];
                }
            } else {
                self.run[i] = 0;
            }
            i += 1;
        }
    }

    #[must_use]
    pub const fn below(&self, i: usize) -> u32 {
        self.below[i]
    }

    #[must_use]
    pub const fn longest(&self, i: usize) -> u32 {
        self.longest[i]
    }
}

/// Scans in the rail moving mean that feeds the sag stop (power of two).
pub const RAIL_MEAN_LEN: usize = 8;
const RAIL_MEAN_SHIFT: u32 = 3;
const _: () = assert!(1 << RAIL_MEAN_SHIFT == RAIL_MEAN_LEN);

/// Moving mean of the bus and VREF codes over the last `RAIL_MEAN_LEN` scans,
/// the input the closed-loop runs feed to [`FastBusSag`] (mean against the
/// bridge-off mean; the reasoning is at the binary's `feed_rail_mean`).
///
/// **It must be reset at the start of every powered run (E108).** Held once
/// per boot, it carried the previous run's last samples -- after a run that
/// ended in a genuine bus collapse, the next run's first means were that
/// collapse, and the sag stop fired within three scans of a healthy start.
#[derive(Clone, Debug)]
pub struct RailMean {
    bus: [u16; RAIL_MEAN_LEN],
    vref: [u16; RAIL_MEAN_LEN],
    bus_sum: u32,
    vref_sum: u32,
    ix: usize,
    filled: usize,
}

impl Default for RailMean {
    fn default() -> Self {
        Self::new()
    }
}

impl RailMean {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bus: [0; RAIL_MEAN_LEN],
            vref: [0; RAIL_MEAN_LEN],
            bus_sum: 0,
            vref_sum: 0,
            ix: 0,
            filled: 0,
        }
    }

    /// Forget every sample: the mean is not ready until `RAIL_MEAN_LEN` new ones.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[inline]
    pub fn feed(&mut self, bus: u16, vref: u16) {
        let i = self.ix & (RAIL_MEAN_LEN - 1);
        self.bus_sum = self.bus_sum + bus as u32 - self.bus[i] as u32;
        self.vref_sum = self.vref_sum + vref as u32 - self.vref[i] as u32;
        self.bus[i] = bus;
        self.vref[i] = vref;
        self.ix = self.ix.wrapping_add(1);
        if self.filled < RAIL_MEAN_LEN {
            self.filled += 1;
        }
    }

    #[inline]
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.filled >= RAIL_MEAN_LEN
    }

    #[inline]
    #[must_use]
    pub const fn bus_mean(&self) -> u16 {
        (self.bus_sum >> RAIL_MEAN_SHIFT) as u16
    }

    #[inline]
    #[must_use]
    pub const fn vref_mean(&self) -> u16 {
        (self.vref_sum >> RAIL_MEAN_SHIFT) as u16
    }
}

/// Relative floor numerator/denominator: trip below 95% of reference.
pub const SAG_NUM: u32 = 95;
pub const SAG_DEN: u32 = 100;
/// Consecutive low scans required to latch.
pub const SAG_STREAK: u8 = 3;

/// The fast bus-sag hard stop. This is a **hard stop**, never a report-only
/// observation, and it latches.
///
/// Deliberately NOT Copy: a latching state machine that is silently
/// duplicated loses its latch. With Copy, `let mut g = sag;` compiles,
/// mutates a throwaway copy, and drops the trip with no borrow error and no
/// warning.
#[derive(Clone, Debug)]
pub struct FastBusSag {
    /// The pre-run baseline. Kept for the record and to prime the filter; it
    /// is no longer what a scan is judged against (E146).
    reference: BusReference,
    /// The sharp reference: bus and VREF as ~200 ms exponential averages, in
    /// Q8. At the 9.901 kHz scan rate a shift of 11 is 2048 scans, 207 ms.
    filt_bus_q8: u32,
    filt_vref_q8: u32,
    lows: u8,
    tripped: bool,
    /// **High-water mark of `lows`** (Q60-1). `lows` resets to 0 on any block
    /// above the line, and the report only ever carried its value *at the
    /// stop*, so a run that reached 2 ten thousand times was indistinguishable
    /// from one that never left 0. This is the whole-run answer to "how close
    /// did the deciding quantity get", which no observer had.
    max_lows: u8,
    /// **Smallest `lhs - rhs` seen**, i.e. the closest the mean came to the
    /// line, in cross-product units. Deliberately NOT converted to codes: that
    /// needs a division, and E291 measured a division on this path costing 48%
    /// of `loop_iters_closed`. The host divides by `filt_vref * SAG_DEN`.
    ///
    /// `u32::MAX` means "no scan judged yet", so zero is unambiguous.
    min_margin: u32,
}

/// Shift of the sharp reference's exponential average: 2^11 scans at
/// 9.901 kHz, about 207 ms (E146).
pub const SAG_FILTER_SHIFT: u32 = 11;

/// One exponential-average step in Q8, shift-only: no division on any path
/// this runs on, and a sample can move the average by at least one Q8 step,
/// so the filter cannot stall against a slow drift.
#[inline]
const fn ewma(q8: u32, sample: u16) -> u32 {
    let x = (sample as u32) << 8;
    if x > q8 {
        let d = x - q8;
        let step = d >> SAG_FILTER_SHIFT;
        q8 + if step == 0 { 1 } else { step }
    } else if x < q8 {
        let d = q8 - x;
        let step = d >> SAG_FILTER_SHIFT;
        // `step <= d`, so this cannot pass `x`, let alone underflow.
        q8 - if step == 0 { 1 } else { step }
    } else {
        q8
    }
}

impl FastBusSag {
    #[inline]
    pub const fn new(reference: BusReference) -> Self {
        Self {
            reference,
            filt_bus_q8: (reference.bus as u32) << 8,
            filt_vref_q8: (reference.vref as u32) << 8,
            lows: 0,
            tripped: false,
            max_lows: 0,
            min_margin: u32::MAX,
        }
    }

    /// The sharp reference as plain codes, for the report.
    #[inline]
    #[must_use]
    pub const fn filtered(&self) -> (u16, u16) {
        // Rounded, not truncated: truncation throws away up to a whole code of
        // reference, which at the 95% line is worth about 0.08% of bus and
        // moves the threshold by a code. `exactly_at_ninety_five_percent_is_
        // not_low` fails on the truncating version.
        (
            ((self.filt_bus_q8 + 128) >> 8) as u16,
            ((self.filt_vref_q8 + 128) >> 8) as u16,
        )
    }

    /// The pre-run baseline, for the report.
    #[inline]
    #[must_use]
    pub const fn reference(&self) -> BusReference {
        self.reference
    }

    /// Feed one scan. Returns `Some(FastBusSag)` on the latching trip.
    ///
    /// Comparison is the reference cross-product form, which cannot overflow
    /// `u32` for 12-bit inputs (max 4095*4095*100 = 1_676_902_500 < 2^31):
    /// `bus * ref_vref * 100 < ref_bus * vref * 95`.
    #[must_use = "a dropped bus-sag verdict silently discards a hard stop"]
    #[inline]
    pub fn observe(&mut self, bus: u16, vref: u16) -> Option<Reason> {
        if self.tripped {
            return Some(Reason::FastBusSag);
        }
        // Fail closed: an implausible VREF makes the normalization meaningless.
        if vref == 0 || vref >= ADC_RAIL {
            self.tripped = true;
            self.lows = SAG_STREAK;
            return Some(Reason::FastBusSag);
        }
        // Judged against the **sharp** reference (E146): the ~200 ms average
        // of the bus, not the pre-run baseline. A load that draws the rail
        // down slowly takes the reference with it, so a droop is margin and
        // only a dip is a fault; the slow direction is the absolute floor's
        // job (`BUS_FLOOR_NUM`, `Reason::Bus`), which is untouched.
        //
        // The fraction, the streak and the latch are unchanged.
        let (fb, fv) = self.filtered();
        let lhs = (bus as u32) * (fv as u32) * SAG_DEN;
        let rhs = (fb as u32) * (vref as u32) * SAG_NUM;
        // Q60-1: the two whole-run observers, on the values this scan already
        // computed. `min_margin` is only meaningful when the scan is NOT low
        // (`lhs >= rhs`); a low scan has crossed and its margin is negative,
        // which `lows`/`max_lows` record instead.
        if lhs >= rhs {
            let m = lhs - rhs;
            if m < self.min_margin {
                self.min_margin = m;
            }
        }
        let verdict = if lhs < rhs {
            self.lows = self.lows.saturating_add(1);
            if self.lows > self.max_lows {
                self.max_lows = self.lows;
            }
            if self.lows >= SAG_STREAK {
                self.tripped = true;
                Some(Reason::FastBusSag)
            } else {
                None
            }
        } else {
            self.lows = 0;
            None
        };
        // Update **after** the test, so a collapsing sample cannot drag the
        // reference down onto itself and hide the collapse.
        self.filt_bus_q8 = ewma(self.filt_bus_q8, bus);
        self.filt_vref_q8 = ewma(self.filt_vref_q8, vref);
        verdict
    }

    #[inline]
    pub const fn streak(&self) -> u8 {
        self.lows
    }
    /// The whole-run high-water mark of the streak (Q60-1), as opposed to
    /// [`Self::streak`], which is its value at the stop.
    #[inline]
    #[must_use]
    pub const fn max_streak(&self) -> u8 {
        self.max_lows
    }
    /// Closest approach to the trip line over the whole run, in cross-product
    /// units; `u32::MAX` if no scan was judged (Q60-1).
    #[inline]
    #[must_use]
    pub const fn min_margin(&self) -> u32 {
        self.min_margin
    }
    #[inline]
    pub const fn tripped(&self) -> bool {
        self.tripped
    }
}

// ---------------------------------------------------------------------------
// Averaged signed current: foldback then hard stop
// ---------------------------------------------------------------------------

/// Scans accumulated per averaging block.
pub const BLOCK_SCANS: u32 = 100;

/// Raw-code residual equivalent to 4 A (three shunts, 7 mOhm, gain 10, VDDA
/// 3600, 100 scans). **This is the mA CALIBRATION** every reported current is
/// scaled by (E291), and it was also the production trip threshold until ENV-20.
pub const RAW_LIMIT: u32 = 31_857;

/// The production `AverageCurrent` trip threshold: **5000 mA** in the same raw
/// units (`RAW_LIMIT * 5 / 4`).
///
/// ENV-20, OPERATOR DECISION: the 4 A figure was never set by the operator. It
/// was sized by an agent against the bench's old 3 A PSU clamp and had never
/// fired. The operator raised the PSU limit to 5 A and directed that it be used, so
/// the allowance now matches the supply. The foldback-then-stop structure, the
/// streak and every other guard are unchanged; only the level moves.
pub const RAW_ALLOW: u32 = RAW_LIMIT * 5 / 4;
const _: () = assert!(RAW_ALLOW == 39_821);

/// Ceiling reduction, in tenths of a percent, chosen by how far the block
/// residual overshot the allowance.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Reduction(pub u16);

/// Classify overshoot severity using cross-products only (no division):
/// <=110% -> 10, <=125% -> 20, <=150% -> 30, <=175% -> 40, else 50.
#[inline]
pub const fn severity_reduction(residual: u32, allow: u32) -> Reduction {
    if residual * 10 <= allow * 11 {
        Reduction(10)
    } else if residual * 4 <= allow * 5 {
        Reduction(20)
    } else if residual * 2 <= allow * 3 {
        Reduction(30)
    } else if residual * 4 <= allow * 7 {
        Reduction(40)
    } else {
        Reduction(50)
    }
}

/// What an averaging block concluded.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BlockVerdict {
    /// Residual within allowance.
    Ok,
    /// First over-block: fold the ceiling back by this much.
    Foldback(Reduction),
    /// Second consecutive over-block: hard stop.
    Stop(Reason),
}

/// Signed three-shunt block accumulator.
///
/// The residual is `zero_block - sum(A+B+C)` and is **never** absolved into an
/// absolute value: motoring pushes the amplifiers below their rail so the
/// residual is positive, while regeneration makes it negative and must not be
/// able to trip an over-current.
/// Not Copy: see [`FastBusSag`] -- duplicating an accumulator loses both
/// the partial block and the over-block streak.
#[derive(Clone, Debug)]
pub struct AverageCurrent {
    zero_block: u32,
    allow: u32,
    sum: u32,
    scans: u32,
    over_streak: u8,
    worst: i32,
    /// The same running max, but reset at the hold mark (E244).
    ///
    /// `worst` spans the whole run -- sine startup, driven stage and ramp -- so
    /// at duty 150 it reads 900 mA against a 34 mA hold. Owed since E194, which
    /// withdrew three worst-block claims for exactly that reason.
    worst_hold: i32,
    /// Signed sum of completed-block residuals, and the block count.
    ///
    /// These exist to reproduce the reference's published current column
    /// exactly: *"the average signed three-shunt residual over completed
    /// 100-scan blocks, scaled by each run's nominal 4 A raw allowance"*
    /// (`binz/DUTY_50_CAMPAIGN.md`). That column is the only current figure on
    /// this bench that is comparable rung-for-rung against the qualified
    /// image -- 45 mA at 10% duty, 58 mA at 15% -- and it is a physical
    /// discriminator for a stalled rotor, which generates no back-EMF to
    /// oppose the applied voltage and therefore draws far more.
    total: i64,
    blocks: u32,
}

impl AverageCurrent {
    /// `zero_block` is the pre-drive zero sum over the same scan count.
    #[inline]
    pub const fn new(zero_block: u32, allow: u32) -> Self {
        Self {
            zero_block,
            allow,
            sum: 0,
            scans: 0,
            over_streak: 0,
            worst: 0,
            worst_hold: i32::MIN,
            total: 0,
            blocks: 0,
        }
    }

    /// Accumulate one scan; yields a verdict when a block completes.
    #[must_use = "a dropped block verdict silently discards a foldback or a hard stop"]
    #[inline]
    pub fn accumulate(&mut self, a: u16, b: u16, c: u16) -> Option<BlockVerdict> {
        self.sum = self
            .sum
            .saturating_add(a as u32)
            .saturating_add(b as u32)
            .saturating_add(c as u32);
        self.scans += 1;
        if self.scans < BLOCK_SCANS {
            return None;
        }
        let residual = (self.zero_block as i64) - (self.sum as i64);
        self.sum = 0;
        self.scans = 0;
        if residual > self.worst as i64 {
            self.worst = residual as i32;
        }
        // The same max, but only since `mark_hold` -- i.e. over the dwell, not
        // over the whole run. Owed since E194 and finally taken here: `worst`
        // spans the sine startup, the driven stage and the ramp, so at duty
        // 150 it reads 900 mA against a 34 mA hold, and E241 projected that
        // ramp artefact three rungs forward as if it were a hold figure.
        if residual > self.worst_hold as i64 {
            self.worst_hold = residual as i32;
        }
        // Running total for the mean. Signed and unclamped, because the
        // reference's published current column is a *signed* average and a
        // regenerating block is real information, not noise to discard.
        self.total = self.total.saturating_add(residual);
        self.blocks = self.blocks.saturating_add(1);
        // Negative (regenerating) residual can never be an over-current.
        if residual <= 0 {
            self.over_streak = 0;
            return Some(BlockVerdict::Ok);
        }
        let residual = residual as u32;
        if residual <= self.allow {
            self.over_streak = 0;
            return Some(BlockVerdict::Ok);
        }
        self.over_streak = self.over_streak.saturating_add(1);
        if self.over_streak >= 2 {
            Some(BlockVerdict::Stop(Reason::AverageCurrent))
        } else {
            Some(BlockVerdict::Foldback(severity_reduction(residual, self.allow)))
        }
    }

    /// Completed blocks contributing to the mean.
    #[inline]
    pub const fn blocks(&self) -> u32 {
        self.blocks
    }

    /// Mean signed block residual in raw codes, or 0 before any block.
    #[inline]
    pub const fn mean_residual(&self) -> i32 {
        if self.blocks == 0 {
            0
        } else {
            (self.total / self.blocks as i64) as i32
        }
    }

    /// Mean current in milliamps, scaled the reference's way: the mean signed
    /// residual as a fraction of the nominal allowance, times 4000 mA.
    ///
    /// This is not a calibrated DC-link measurement and the reference is
    /// explicit that its own column is not either -- it is "neither a
    /// calibrated DC-link/PSU current nor a PWM peak". Its value is that both
    /// sides compute it identically, so the numbers may be compared.
    #[inline]
    pub const fn mean_milliamps(&self) -> i32 {
        if self.blocks == 0 || self.allow == 0 {
            return 0;
        }
        // ENV-20: scaled by the calibration, not the threshold (E291's rule,
        // which `block_milliamps` already followed). Dividing by `allow` read
        // 20 % low once the allowance moved to 5 A.
        let mean = self.mean_residual() as i64;
        ((mean * 4_000) / RAW_LIMIT as i64) as i32
    }

    #[inline]
    pub const fn worst_residual(&self) -> i32 {
        self.worst
    }

    /// One block's signed residual in mA. Added in E187 because
    /// [`Self::worst_residual`] was computed every block and never read, so
    /// every current figure this bench had ever produced was a mean.
    ///
    /// **Scaled by `RAW_LIMIT`, not by `allow` (E291.)** The old form divided
    /// by `self.allow` and justified it as "the allowance is 4 A by
    /// construction (`RAW_LIMIT`)" -- which holds only while the two are equal.
    /// `RAW_LIMIT` raw units == 4000 mA is the **calibration**; `allow` is the
    /// **trip threshold**, and `Inject::AverageCurrent` deliberately rebuilds
    /// this accumulator at `RAW_LIMIT/100`. Dividing by the threshold conflated
    /// the two and inflated every mA field in an injected capture 100x:
    /// `e280-prot500-i` reads `worst_ma=184352` where the truth is 1840 mA.
    ///
    /// E287 reported the allowance beside the figures so a reader could divide.
    /// That was the wrong repair in two ways. It left the numbers wrong and
    /// pushed the correction onto every future reader; and it was not even
    /// uniform -- `zero_drift_ma` (`run/mod.rs`) always divided by `RAW_LIMIT`,
    /// so one `BEMFCURRENT` line carried **two different mA scales** under a
    /// single `ma_allow`, and a reader who followed E287's instruction would
    /// have corrupted `zero_drift_ma` and `ref_ma` by 100x. Fixing the scale at
    /// its root makes the whole line one scale and costs nothing.
    ///
    /// Captures where no injection ran are **unchanged**: `allow == RAW_LIMIT`
    /// there, so this is bit-identical for every qualifying run ever recorded.
    /// `allow()` is still emitted as `ma_allow`, now as what it actually is --
    /// the trip threshold, which tells a reader an injection was active -- and
    /// no longer as a scale anyone must apply.
    #[inline]
    #[must_use]
    pub const fn block_milliamps(&self, residual: i32) -> i32 {
        ((residual as i64 * 4_000) / RAW_LIMIT as i64) as i32
    }
    #[inline]
    /// The raw allowance the mA scale is relative to (E287).
    ///
    /// `block_milliamps` scales by `4000/allow`, so a capture's mA figures only
    /// mean milliamps when `allow == RAW_LIMIT`. `Inject::AverageCurrent`
    /// deliberately rebuilds this accumulator with `RAW_LIMIT/100` to trip the
    /// stop, which makes every mA field in that capture **100x inflated** --
    /// `e280-prot500-i` reads `worst_ma=184352` where the truth is ~1840 mA,
    /// and nothing in the report said so. A reader sees 184 A.
    ///
    /// So the allowance is now emitted beside the figures it scales.
    #[must_use]
    pub const fn allow(&self) -> u32 {
        self.allow
    }

    #[inline]
    pub const fn over_streak(&self) -> u8 {
        self.over_streak
    }

    /// Start the hold window for [`Self::hold_worst_residual`].
    ///
    /// Called at the same instant as [`Self::mark`], from the hold mark in the
    /// run's state machine. Separate from `mark` because a max cannot be
    /// recovered by subtraction the way a running total can: the worst block
    /// since an instant is not derivable from the worst block at it.
    #[inline]
    pub fn mark_hold(&mut self) {
        self.worst_hold = i32::MIN;
    }

    /// The worst single block **of the hold**, raw residual.
    ///
    /// [`Self::worst_residual`] is the worst of the *whole run*, ramp
    /// included; the two differ by more than an order of magnitude at low
    /// rungs. Any claim about current at a rung wants this one.
    ///
    /// Returns **0 before the hold is marked**, rather than the `i32::MIN`
    /// sentinel: a run that stops during the ramp has no hold, and scaling the
    /// sentinel would print a spectacular nonsense figure. The corpus already
    /// contains one such artefact from a different path (a provoked run
    /// reporting `worst_ma=172553`), so a sentinel that can reach the report is
    /// a defect, not a detail.
    #[inline]
    #[must_use]
    pub const fn hold_worst_residual(&self) -> i32 {
        if self.worst_hold == i32::MIN {
            0
        } else {
            self.worst_hold
        }
    }

    /// A mark of the completed-block totals, to average a later window.
    #[inline]
    pub const fn mark(&self) -> CurrentMark {
        CurrentMark {
            total: self.total,
            blocks: self.blocks,
        }
    }

    /// Mean current, scaled as [`Self::mean_milliamps`], over only the blocks
    /// completed since `from`.
    ///
    /// The reference's column is measured over the **final target dwell**
    /// ("measured ~15-19 s only after the final target ACK",
    /// `DUTY_50_CAMPAIGN.md`). The whole-run mean mixed in the sine startup and
    /// the driven stage, whose currents differ in both size and sign, and read
    /// -35..+85 mA across runs at the same duty (E073-E077). Averaging the hold
    /// window is the like-for-like figure.
    #[inline]
    pub const fn window_milliamps(&self, from: CurrentMark) -> i32 {
        let blocks = self.blocks.saturating_sub(from.blocks);
        if blocks == 0 || self.allow == 0 {
            return 0;
        }
        let total = self.total.saturating_sub(from.total);
        let mean = total / blocks as i64;
        // ENV-20: the calibration, not the threshold (see `mean_milliamps`).
        ((mean * 4_000) / RAW_LIMIT as i64) as i32
    }

    /// Blocks completed since `from`.
    #[inline]
    pub const fn window_blocks(&self, from: CurrentMark) -> u32 {
        self.blocks.saturating_sub(from.blocks)
    }
}

/// Completed-block totals at an instant (see [`AverageCurrent::mark`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CurrentMark {
    total: i64,
    blocks: u32,
}

/// Bridge-off zero blocks averaged into the per-block datum (E093).
///
/// One 100-scan block (10 ms) of bridge-off zero wanders by up to ~1350 raw
/// block to block on this board (E092, ~85 mA-equivalent), because the
/// amplifier outputs wander by a few codes on a 10 ms scale. The in-run
/// residual averages thousands of blocks, so the single-block datum was the
/// whole run-to-run scatter of the current figure. Fifty blocks is 0.5 s.
pub const ZERO_BLOCKS: u32 = 50;

/// Per-block zero from the sum of `blocks` bridge-off block sums, rounded
/// **up** as the reference rounds its 128-scan zero ("Round zero UP ... Upward
/// rounding cannot underestimate its positive residual",
/// `binz/examples/support/average_current.rs:18-27`). `None` for no blocks or
/// an out-of-range result.
#[must_use]
pub const fn zero_from_blocks(sum: u64, blocks: u32) -> Option<u32> {
    if blocks == 0 {
        return None;
    }
    let z = sum.div_ceil(blocks as u64);
    if z > u32::MAX as u64 {
        None
    } else {
        Some(z as u32)
    }
}

/// Duty ceiling governor. Foldback **ratchets down only** — there is no
/// automatic release, so a fault can never be walked back up by itself.
/// Not Copy: the whole point is that the ceiling ratchets down and never
/// back up, which a silent copy would undo.
#[derive(Clone, Debug)]
pub struct FoldbackGovernor {
    ceiling_tenths: u16,
    floor_tenths: u16,
}

impl FoldbackGovernor {
    #[inline]
    pub const fn new(ceiling_tenths: u16, floor_tenths: u16) -> Self {
        Self {
            ceiling_tenths,
            floor_tenths,
        }
    }

    /// Apply a reduction. Saturates at the floor; never increases.
    #[must_use = "returns the new ceiling; ignoring it is usually a bug"]
    #[inline]
    pub fn warn(&mut self, reduction: Reduction) -> u16 {
        let next = self.ceiling_tenths.saturating_sub(reduction.0);
        self.ceiling_tenths = if next < self.floor_tenths {
            self.floor_tenths
        } else {
            next
        };
        self.ceiling_tenths
    }

    #[inline]
    pub const fn ceiling(&self) -> u16 {
        self.ceiling_tenths
    }

    /// Clamp a requested duty to the current ceiling.
    #[inline]
    pub const fn clamp(&self, requested_tenths: u16) -> u16 {
        if requested_tenths > self.ceiling_tenths {
            self.ceiling_tenths
        } else {
            requested_tenths
        }
    }
}

// ---------------------------------------------------------------------------
// The control-tick guard: deadlines, tick gap, feedback staleness, tracking
// ---------------------------------------------------------------------------

/// Accepted-commutation watchdog allowance.
pub const EVENT_MAX_US: u32 = 1_000;
/// Plausible full-electrical-cycle band, used for same-sector revisits.
///
/// **Read from the qualified image on the bench, not chosen.** Its own
/// post-run line is
/// `RUNLIMIT cycle_min_us=2223 event_min_us=238 cycle_max_us=6000
/// event_max_us=1000` (notebook E032, capture
/// `captures/2026-09-20/oracle_8000ms.txt`). `CYCLE_MIN_US` was 4000 here,
/// which is mine: nearly twice the reference's floor, so a genuine full cycle
/// at running speed would be refused as implausibly fast. At the reference's
/// locked 392 eHz a full electrical cycle is 2551 us, which clears 2223 and
/// does *not* clear 4000.
pub const CYCLE_MIN_US: u32 = 2_223;
pub const CYCLE_MAX_US: u32 = 6_000;

/// Minimum interval between accepted commutation events.
///
/// From the same `RUNLIMIT` line: `event_min_us=238`. This was `333`, inlined
/// into the `Guard` alias below with no citation, and 333 is mine. The
/// difference is not cosmetic -- the reference's locked run has an average
/// event interval of 425 us (`average_half_us=850`), so an event arriving a
/// quarter early is **accepted by the reference and rejected by 333**. A
/// guard that refuses real events reads as a detector that cannot find them.
pub const EVENT_MIN_US: u32 = 238;
/// Guard-poll allowance for a **10 kHz** polling cadence: the reference polls
/// the guard from its 100 us timer interrupt, so 200 us means "missed at most
/// one of my own periods".
///
/// This is a property of the *poller*, not of the motor, so a build that polls
/// the guard at a different cadence needs the allowance that expresses the
/// same rule at that cadence -- see [`GAP_MAX_US_1KHZ`]. Reusing 200 us for a
/// slower poller does not make the firmware safer, it just makes the guard
/// report a fault every run.
pub const TICK_GAP_MAX_US: u32 = 200;

// A wider allowance for a slower poller was added here once, when the polled
// bring-up binary kept stopping with `reason=3`. It was removed: the real
// cause was a lying timebase (a 16-bit counter whose ARR was 64526 while the
// extension arithmetic masked with 0xFFFF, injecting 1010 us per wrap), and
// with that fixed the polled loop measures a worst-case gap of 164 us and
// passes the reference's 200 us unmodified.
//
// Kept as a comment deliberately: a relaxed constant left lying around is an
// invitation to widen a protection instead of finding the bug behind it.
/// ADC feedback must arrive at least this often.
pub const FEEDBACK_MAX_AGE_US: u32 = 1_000;

/// Inputs sampled once per control tick.
#[derive(Copy, Clone, Debug)]
pub struct TickInputs {
    pub now_us: u32,
    /// nFAULT pin level: true = high = healthy.
    pub nfault_high: bool,
    pub host_abort: bool,
}

/// Deadline/liveness guard, polled from the top-priority control tick.
///
/// `MIN_CYCLE_US`/`MIN_EVENT_US` are the reference's compile-time floors and
/// are carried as const generics because they are exactly the "small bounded
/// numeric parameters" that belong there.
/// Not Copy: faults latch first-wins, and a copy would un-latch them.
#[derive(Clone, Debug)]
pub struct RunGuard<const MIN_CYCLE_US: u32, const MIN_EVENT_US: u32, const GAP_MAX_US: u32> {
    campaign_limit_us: u32,
    segment_limit_us: u32,
    started_us: u32,
    last_poll_us: u32,
    last_feedback_us: u32,
    last_event_us: u32,
    first_poll_done: bool,
    /// Each liveness channel arms only once it has been primed. Startup
    /// (align/catch/ramp) legitimately runs for ~1 s before the first accepted
    /// commutation or ADC frame, so measuring the first age from the guard's
    /// construction time would trip Tracking/FeedbackStale on every run.
    feedback_primed: bool,
    event_primed: bool,
    fault: Option<Reason>,
}

impl<const MIN_CYCLE_US: u32, const MIN_EVENT_US: u32, const GAP_MAX_US: u32>
    RunGuard<MIN_CYCLE_US, MIN_EVENT_US, GAP_MAX_US>
{
    #[inline]
    pub const fn new(started_us: u32, campaign_limit_us: u32, segment_limit_us: u32) -> Self {
        Self {
            campaign_limit_us,
            segment_limit_us,
            started_us,
            last_poll_us: started_us,
            last_feedback_us: started_us,
            last_event_us: started_us,
            first_poll_done: false,
            feedback_primed: false,
            event_primed: false,
            fault: None,
        }
    }

    #[inline]
    pub const fn fault(&self) -> Option<Reason> {
        self.fault
    }

    #[inline]
    fn latch(&mut self, reason: Reason) -> Option<Reason> {
        if self.fault.is_none() {
            self.fault = Some(reason);
        }
        self.fault
    }

    /// Record an ADC feedback frame.
    #[inline]
    pub fn feedback(&mut self, now_us: u32) {
        self.last_feedback_us = now_us;
        self.feedback_primed = true;
    }

    /// Record an accepted commutation.
    ///
    /// Guards the event spacing from **both** sides:
    ///
    /// * too slow (`> EVENT_MAX_US`) is [`Reason::Tracking`] -- a commutation
    ///   went missing;
    /// * too fast (`< MIN_EVENT_US`) is [`Reason::CompStorm`] -- back-to-back
    ///   accepts far closer together than the rotor can physically turn, which
    ///   is the classic comparator-noise/desync signature.
    ///
    /// The fast side was missing until a review caught it: `MIN_EVENT_US` was
    /// declared as a const generic, documented as a floor, and never read, so
    /// `RunGuard<_, 333>` carried a 333 us bound that did nothing. That is
    /// latent for the open-loop binary (which never calls this) and exactly the
    /// protection the closed loop will depend on.
    ///
    /// The first accepted event is exempt from the fast check for the same
    /// reason it is exempt from the slow one: there is no previous event to
    /// measure against.
    #[must_use = "a dropped tracking verdict silently discards a fault"]
    #[inline]
    pub fn accepted(&mut self, now_us: u32) -> Option<Reason> {
        // Load the cross-ISR reference BEFORE deriving the age.
        let prev = self.last_event_us;
        let primed = self.event_primed;
        self.last_event_us = now_us;
        self.event_primed = true;
        if primed {
            let delta = now_us.wrapping_sub(prev);
            if delta > EVENT_MAX_US {
                return self.latch(Reason::Tracking);
            }
            if delta < MIN_EVENT_US {
                return self.latch(Reason::CompStorm);
            }
        }
        self.fault
    }

    /// Report a completed full electrical cycle (same sector revisited).
    #[must_use = "a dropped cycle verdict silently discards a fault"]
    #[inline]
    pub fn cycle(&mut self, duration_us: u32) -> Option<Reason> {
        if duration_us < MIN_CYCLE_US || duration_us > CYCLE_MAX_US {
            return self.latch(Reason::CycleTiming);
        }
        self.fault
    }

    /// The control-tick poll. Precedence matches the reference exactly.
    #[must_use = "a dropped guard verdict silently discards every deadline and watchdog"]
    pub fn poll(&mut self, input: TickInputs) -> Option<Reason> {
        if let Some(existing) = self.fault {
            return Some(existing);
        }
        let now = input.now_us;

        // 1. host abort
        if input.host_abort {
            return self.latch(Reason::HostAbort);
        }
        // 2. gate driver fault
        if !input.nfault_high {
            return self.latch(Reason::Driver);
        }
        // 3/4. deadlines
        let elapsed = now.wrapping_sub(self.started_us);
        if elapsed >= self.campaign_limit_us {
            return self.latch(Reason::CampaignDeadline);
        }
        if elapsed >= self.segment_limit_us {
            return self.latch(Reason::SegmentDeadline);
        }
        // 5. tick gap — skip on the very first poll, which has no predecessor.
        let prev_poll = self.last_poll_us;
        self.last_poll_us = now;
        if self.first_poll_done && now.wrapping_sub(prev_poll) > GAP_MAX_US {
            return self.latch(Reason::TickGap);
        }
        self.first_poll_done = true;
        // 6. feedback staleness (only once a frame has actually arrived)
        if self.feedback_primed && now.wrapping_sub(self.last_feedback_us) > FEEDBACK_MAX_AGE_US {
            return self.latch(Reason::FeedbackStale);
        }
        // 7. accepted-event watchdog (only once commutation has been accepted)
        if self.event_primed && now.wrapping_sub(self.last_event_us) > EVENT_MAX_US {
            return self.latch(Reason::Tracking);
        }
        None
    }
}

/// The guard shape the qualified image runs.
///
/// The gap allowance is a const generic because it is a property of whatever
/// polls the guard, not of the motor -- but every build in this crate, polled
/// foreground and ISR alike, uses the reference's value.
pub type Guard = RunGuard<CYCLE_MIN_US, EVENT_MIN_US, TICK_GAP_MAX_US>;

#[cfg(test)]
mod tests {
    use super::*;

    const VCAL: u32 = 1662;

    fn good_scan() -> RawScan {
        RawScan {
            phase_a: 2048,
            phase_b: 2048,
            phase_c: 2048,
            bus: 1212,
            vref: 1506,
        }
    }

    // --- raw feedback validation -----------------------------------------

    #[test]
    fn good_scan_passes_both_policies() {
        assert_eq!(
            validate_raw_feedback(&good_scan(), PhaseCodePolicy::Band { vcal: VCAL }),
            None
        );
        assert_eq!(validate_raw_feedback(&good_scan(), PhaseCodePolicy::RetainRails), None);
    }

    #[test]
    fn implausible_vref_is_a_bus_fault_on_every_policy() {
        for vref in [0u16, ADC_RAIL, ADC_RAIL + 1] {
            let mut s = good_scan();
            s.vref = vref;
            assert_eq!(
                validate_raw_feedback(&s, PhaseCodePolicy::Band { vcal: VCAL }),
                Some(Reason::Bus),
                "vref {vref}"
            );
            assert_eq!(
                validate_raw_feedback(&s, PhaseCodePolicy::RetainRails),
                Some(Reason::Bus),
                "vref {vref}"
            );
        }
    }

    #[test]
    fn band_policy_rejects_codes_outside_the_window() {
        for bad in [0u16, PHASE_CODE_LOW - 1, PHASE_CODE_HIGH + 1, 4000] {
            let mut s = good_scan();
            s.phase_b = bad;
            assert_eq!(
                validate_raw_feedback(&s, PhaseCodePolicy::Band { vcal: VCAL }),
                Some(Reason::Current),
                "code {bad}"
            );
        }
        // the inclusive edges are admissible
        for ok in [PHASE_CODE_LOW, PHASE_CODE_HIGH] {
            let mut s = good_scan();
            s.phase_c = ok;
            assert_eq!(validate_raw_feedback(&s, PhaseCodePolicy::Band { vcal: VCAL }), None);
        }
    }

    #[test]
    fn retain_rails_policy_does_not_veto_the_scan() {
        // With averaging installed an exact rail must NOT fault: rails are
        // retained in the block average instead.
        for code in [0u16, ADC_RAIL] {
            let mut s = good_scan();
            s.phase_a = code;
            assert_eq!(
                validate_raw_feedback(&s, PhaseCodePolicy::RetainRails),
                None,
                "code {code} must not veto"
            );
        }
    }

    #[test]
    fn absolute_bus_floor_only_applies_to_the_band_policy() {
        let mut s = good_scan();
        // drive bus below 963*vref/vcal
        s.bus = 100;
        assert_eq!(
            validate_raw_feedback(&s, PhaseCodePolicy::Band { vcal: VCAL }),
            Some(Reason::Bus)
        );
        assert_eq!(validate_raw_feedback(&s, PhaseCodePolicy::RetainRails), None);
    }

    // --- fast bus sag ------------------------------------------------------

    fn reference() -> BusReference {
        BusReference { bus: 1212, vref: 1506 }
    }

    #[test]
    fn healthy_bus_never_trips_and_keeps_streak_zero() {
        let mut g = FastBusSag::new(reference());
        for _ in 0..50 {
            assert_eq!(g.observe(1212, 1506), None);
            assert_eq!(g.streak(), 0);
        }
        assert!(!g.tripped());
    }

    #[test]
    fn three_consecutive_lows_latch_the_trip() {
        let mut g = FastBusSag::new(reference());
        // ~91% of reference
        assert_eq!(g.observe(1100, 1506), None);
        assert_eq!(g.streak(), 1);
        assert_eq!(g.observe(1100, 1506), None);
        assert_eq!(g.streak(), 2);
        assert_eq!(g.observe(1100, 1506), Some(Reason::FastBusSag));
        assert!(g.tripped());
        // stays latched forever, even on a healthy sample
        assert_eq!(g.observe(1212, 1506), Some(Reason::FastBusSag));
    }

    #[test]
    fn one_healthy_scan_breaks_the_streak() {
        let mut g = FastBusSag::new(reference());
        assert_eq!(g.observe(1100, 1506), None);
        assert_eq!(g.observe(1100, 1506), None);
        assert_eq!(g.streak(), 2);
        assert_eq!(g.observe(1212, 1506), None, "healthy sample resets");
        assert_eq!(g.streak(), 0);
        // two more lows must therefore not be enough
        assert_eq!(g.observe(1100, 1506), None);
        assert_eq!(g.observe(1100, 1506), None);
        assert!(!g.tripped());
    }

    #[test]
    fn exactly_at_ninety_five_percent_is_not_low() {
        // bus such that bus*ref_vref*100 == ref_bus*vref*95 -> not "<", so ok.
        // ref_bus*95 = 1212*95 = 115_140 ; /100 = 1151.4 -> 1152 is above.
        let mut g = FastBusSag::new(reference());
        assert_eq!(g.observe(1152, 1506), None);
        assert_eq!(g.streak(), 0);
        // 1151 is below the line
        assert_eq!(g.observe(1151, 1506), None);
        assert_eq!(g.streak(), 1);
    }

    #[test]
    fn bad_vref_trips_immediately_fail_closed() {
        for vref in [0u16, ADC_RAIL] {
            let mut g = FastBusSag::new(reference());
            assert_eq!(g.observe(1212, vref), Some(Reason::FastBusSag), "vref {vref}");
            assert!(g.tripped());
        }
    }

    /// E146: a load that pulls the rail down slowly is margin, not a dip.
    ///
    /// The pre-run reference would call a sustained 8% droop a fault three
    /// scans in. The sharp reference follows the load, so the droop passes and
    /// only a step below the *recent* bus trips. The absolute floor
    /// (`BUS_FLOOR_NUM`) still owns the slow direction and is untouched.
    #[test]
    fn a_slow_droop_is_not_a_dip_but_a_step_below_the_recent_bus_still_is() {
        let mut g = FastBusSag::new(reference());
        // 1212 -> 1110 over ~4 s of scans: more than 8% below the pre-run
        // reference at the end, and never a trip.
        let mut bus = 1212i32;
        for i in 0..40_000 {
            if i % 400 == 0 && bus > 1110 {
                bus -= 1;
            }
            assert_eq!(
                g.observe(bus as u16, 1506),
                None,
                "droop tripped at scan {i}, bus {bus}"
            );
        }
        // The reference has followed the load down.
        let (fb, _) = g.filtered();
        assert!(
            (1105..=1115).contains(&fb),
            "sharp reference {fb} did not follow the droop"
        );
        // A real dip, three scans 10% below *that*, still latches.
        let dip = (fb as u32 * 90 / 100) as u16;
        assert_eq!(g.observe(dip, 1506), None);
        assert_eq!(g.observe(dip, 1506), None);
        assert_eq!(g.observe(dip, 1506), Some(Reason::FastBusSag));
        assert!(g.tripped());
    }

    /// A collapse still latches, however fast the reference follows.
    ///
    /// This is the property the filtered reference must not cost: three
    /// consecutive scans deep below the recent bus trip, because one sample
    /// can move the reference by at most its 1/2048 share -- it cannot chase
    /// the collapse down and call it the new normal. A single deep sample also
    /// leaves the *rounded* reference where it was, since its share is far
    /// below one ADC code.
    #[test]
    fn a_collapse_still_latches_and_one_sample_cannot_move_the_reference() {
        let mut g = FastBusSag::new(reference());
        let (before, _) = g.filtered();
        assert_eq!(g.observe(600, 1506), None, "first deep scan: counted, not yet latched");
        let (after_one, _) = g.filtered();
        assert_eq!(
            after_one, before,
            "one sample must not move the reference by a whole code"
        );
        assert_eq!(g.observe(600, 1506), None);
        assert_eq!(
            g.observe(600, 1506),
            Some(Reason::FastBusSag),
            "three deep scans must latch"
        );
        assert!(g.tripped());
        // And the latch is sticky whatever comes next.
        assert_eq!(g.observe(1212, 1506), Some(Reason::FastBusSag));
    }

    #[test]
    fn sag_comparison_cannot_overflow_u32() {
        // worst case operands with 12-bit inputs
        let mut g = FastBusSag::new(BusReference { bus: 4095, vref: 4094 });
        let _ = g.observe(4095, 4094);
        let max = 4095u32 * 4095 * 100;
        assert!(max < i32::MAX as u32, "{max} must stay below 2^31");
    }

    #[test]
    fn a_normalizing_vref_rise_can_reveal_a_sag() {
        // Same bus code, but VREF rose: normalized bus fell.
        let mut g = FastBusSag::new(reference());
        assert_eq!(g.observe(1212, 1700), None);
        assert_eq!(g.streak(), 1, "vref rise must count as low");
    }

    // --- averaged current --------------------------------------------------

    /// Feed a whole block whose per-scan triple sums to `per_scan`.
    fn run_block(acc: &mut AverageCurrent, per_scan: u16) -> BlockVerdict {
        let mut last = None;
        for _ in 0..BLOCK_SCANS {
            last = acc.accumulate(per_scan, 0, 0);
        }
        last.expect("a verdict must land on the final scan of the block")
    }

    #[test]
    fn verdict_only_lands_on_block_boundaries() {
        let mut acc = AverageCurrent::new(0, RAW_LIMIT);
        for _ in 0..(BLOCK_SCANS - 1) {
            assert_eq!(acc.accumulate(1, 1, 1), None);
        }
        assert!(acc.accumulate(1, 1, 1).is_some(), "100th scan closes the block");
    }

    #[test]
    fn residual_within_allowance_is_ok() {
        // zero_block large, draw small -> residual small
        let mut acc = AverageCurrent::new(100 * 3 * 2048, RAW_LIMIT);
        // each scan sums to 3*2048 - a little
        let mut last = None;
        for _ in 0..BLOCK_SCANS {
            last = acc.accumulate(2048, 2048, 2048 - 100);
        }
        assert_eq!(last, Some(BlockVerdict::Ok));
        assert_eq!(acc.over_streak(), 0);
    }

    #[test]
    fn milliamps_are_milliamps_whatever_the_trip_threshold_is() {
        // E291. `RAW_LIMIT` raw units == 4000 mA is the calibration; `allow` is
        // the trip threshold, and `Inject::AverageCurrent` rebuilds the
        // accumulator at `RAW_LIMIT/100` to force a stop. The mA scale must not
        // follow the threshold: the same residual is the same current whatever
        // the run is willing to tolerate.
        //
        // This is the assertion that fails on the pre-E291 form, which divided
        // by `self.allow` and so read 100x high on every injected capture --
        // `e280-prot500-i`'s `worst_ma=184352` for a true 1840 mA.
        let normal = AverageCurrent::new(0, RAW_LIMIT);
        let injected = AverageCurrent::new(0, RAW_LIMIT / 100);
        for residual in [1_i32, 318, 14_656, RAW_LIMIT as i32, -14_656] {
            assert_eq!(
                normal.block_milliamps(residual),
                injected.block_milliamps(residual),
                "residual {residual} must read the same mA under either threshold",
            );
        }
        // And the calibration itself: the full allowance is 4 A, and the
        // capture that exposed the defect now reads its true current.
        assert_eq!(normal.block_milliamps(RAW_LIMIT as i32), 4_000);
        assert_eq!(injected.block_milliamps(14_656), 1_840);
        // Sign is preserved for a regenerating block.
        assert_eq!(normal.block_milliamps(-(RAW_LIMIT as i32)), -4_000);
    }

    #[test]
    fn regenerating_negative_residual_can_never_trip() {
        // sum exceeds the zero block -> residual negative
        let mut acc = AverageCurrent::new(0, RAW_LIMIT);
        let v = run_block(&mut acc, 4000);
        assert_eq!(v, BlockVerdict::Ok, "negative residual must be Ok");
        assert_eq!(acc.over_streak(), 0);
    }

    #[test]
    fn first_over_block_folds_back_and_second_stops() {
        // zero_block chosen so residual overshoots the allowance
        let zero = RAW_LIMIT * 2;
        let mut acc = AverageCurrent::new(zero, RAW_LIMIT);
        let first = run_block(&mut acc, 0); // residual == zero == 2x allowance
        match first {
            BlockVerdict::Foldback(r) => assert!(r.0 > 0),
            other => panic!("expected foldback, got {other:?}"),
        }
        assert_eq!(acc.over_streak(), 1);
        let second = run_block(&mut acc, 0);
        assert_eq!(second, BlockVerdict::Stop(Reason::AverageCurrent));
    }

    #[test]
    fn an_ok_block_clears_the_over_streak_so_it_must_be_consecutive() {
        let zero = RAW_LIMIT * 2;
        let mut acc = AverageCurrent::new(zero, RAW_LIMIT);
        assert!(matches!(run_block(&mut acc, 0), BlockVerdict::Foldback(_)));
        assert_eq!(acc.over_streak(), 1);
        // a block that draws enough to bring the residual under the allowance
        let ok = run_block(&mut acc, ((zero - 1) / BLOCK_SCANS) as u16);
        assert_eq!(ok, BlockVerdict::Ok);
        assert_eq!(acc.over_streak(), 0, "streak must clear");
        // so the next over-block is a foldback again, not a stop
        assert!(matches!(run_block(&mut acc, 0), BlockVerdict::Foldback(_)));
    }

    /// The window mean averages only the blocks after its mark, so a startup
    /// stage with a different current cannot bias the hold figure (E078).
    #[test]
    fn rail_mean_is_ready_only_after_a_full_window_and_averages_it() {
        let mut m = RailMean::new();
        for k in 0..RAIL_MEAN_LEN {
            assert!(!m.ready(), "ready after {k}");
            m.feed(1200, 1500);
        }
        assert!(m.ready());
        assert_eq!((m.bus_mean(), m.vref_mean()), (1200, 1500));
        m.feed(1208, 1500);
        assert_eq!(m.bus_mean(), 1201);
    }

    /// E108: a run that follows a collapsed bus must not inherit the collapse.
    /// Before the reset the sag stop tripped on the old samples; after it, the
    /// mean is not ready until the new run's own scans fill it.
    #[test]
    fn a_reset_forgets_the_previous_runs_collapse() {
        let reference = BusReference { bus: 1213, vref: 1500 };
        let mut m = RailMean::new();
        for _ in 0..RAIL_MEAN_LEN {
            m.feed(900, 1500); // the end of a run that collapsed the supply
        }
        // Without a reset: three healthy scans still see a collapsed mean.
        let mut stale = m.clone();
        let mut sag = FastBusSag::new(reference);
        let mut tripped = None;
        for _ in 0..3 {
            stale.feed(1213, 1500);
            tripped = tripped.or(sag.observe(stale.bus_mean(), stale.vref_mean()));
        }
        assert_eq!(tripped, Some(Reason::FastBusSag), "the defect E107 hit");
        // With the reset: no verdict until the window is this run's, then healthy.
        m.reset();
        let mut sag = FastBusSag::new(reference);
        for _ in 0..(RAIL_MEAN_LEN * 4) {
            m.feed(1213, 1500);
            if m.ready() {
                assert_eq!(sag.observe(m.bus_mean(), m.vref_mean()), None);
            }
        }
    }

    #[test]
    fn averaged_zero_rounds_up_like_the_reference() {
        assert_eq!(zero_from_blocks(616_000 * 50, 50), Some(616_000));
        // Any remainder rounds up: the zero never under-states, so the
        // positive (motoring) residual is never under-estimated.
        assert_eq!(zero_from_blocks(616_000 * 50 + 1, 50), Some(616_001));
        assert_eq!(zero_from_blocks(616_000 * 50 + 49, 50), Some(616_001));
        assert_eq!(zero_from_blocks(7, 1), Some(7));
    }

    #[test]
    fn averaged_zero_refuses_empty_or_out_of_range() {
        assert_eq!(zero_from_blocks(123, 0), None);
        assert_eq!(zero_from_blocks(u64::MAX / 2, 1), None);
        assert_eq!(zero_from_blocks(u32::MAX as u64, 1), Some(u32::MAX));
    }

    #[test]
    fn averaged_zero_leaves_the_allowance_alone() {
        // The datum only moves the residual's origin; a block that overshoots
        // the nominal allowance against the averaged zero still folds back.
        let zero = zero_from_blocks(RAW_LIMIT as u64 * 2 * ZERO_BLOCKS as u64, ZERO_BLOCKS).unwrap();
        let mut acc = AverageCurrent::new(zero, RAW_LIMIT);
        let mut v = None;
        for _ in 0..BLOCK_SCANS {
            v = acc.accumulate(0, 0, 0);
        }
        assert!(matches!(v, Some(BlockVerdict::Foldback(_))));
    }

    #[test]
    fn window_mean_excludes_blocks_before_the_mark() {
        let zero = 3000u32 * BLOCK_SCANS;
        let mut acc = AverageCurrent::new(zero, RAW_LIMIT);
        // Two "startup" blocks drawing hard, and one regenerating block.
        let _ = run_block(&mut acc, 2000);
        let _ = run_block(&mut acc, 2000);
        let _ = run_block(&mut acc, 3500);
        let mark = acc.mark();
        // Three "hold" blocks each with residual 100 per scan.
        for _ in 0..3 {
            let _ = run_block(&mut acc, 2900);
        }
        assert_eq!(acc.window_blocks(mark), 3);
        let expected = ((100 * BLOCK_SCANS) as i64 * 4_000 / RAW_LIMIT as i64) as i32;
        assert_eq!(acc.window_milliamps(mark), expected);
        // The whole-run mean is different, which is the point.
        assert_ne!(acc.mean_milliamps(), expected);
        // An empty window reads 0, never a division by zero.
        assert_eq!(acc.window_milliamps(acc.mark()), 0);
    }

    #[test]
    fn severity_bands_match_the_reference_cross_products() {
        let a = 1000u32;
        assert_eq!(severity_reduction(1050, a), Reduction(10)); // <=110%
        assert_eq!(severity_reduction(1100, a), Reduction(10)); // ==110%
        assert_eq!(severity_reduction(1200, a), Reduction(20)); // <=125%
        assert_eq!(severity_reduction(1250, a), Reduction(20)); // ==125%
        assert_eq!(severity_reduction(1400, a), Reduction(30)); // <=150%
        assert_eq!(severity_reduction(1500, a), Reduction(30)); // ==150%
        assert_eq!(severity_reduction(1600, a), Reduction(40)); // <=175%
        assert_eq!(severity_reduction(1750, a), Reduction(40)); // ==175%
        assert_eq!(severity_reduction(1751, a), Reduction(50)); // beyond
        assert_eq!(severity_reduction(9000, a), Reduction(50));
    }

    // --- foldback governor -------------------------------------------------

    #[test]
    fn governor_ratchets_down_only_and_saturates_at_the_floor() {
        let mut g = FoldbackGovernor::new(500, 40);
        assert_eq!(g.warn(Reduction(50)), 450);
        assert_eq!(g.warn(Reduction(10)), 440);
        // no API can raise it; repeated warnings only lower
        for _ in 0..100 {
            let _ = g.warn(Reduction(50));
        }
        assert_eq!(g.ceiling(), 40, "must saturate at the floor, not wrap");
    }

    #[test]
    fn governor_clamps_requested_duty() {
        let mut g = FoldbackGovernor::new(500, 40);
        assert_eq!(g.clamp(100), 100);
        let _ = g.warn(Reduction(50)); // ceiling 450
        assert_eq!(g.clamp(500), 450);
        assert_eq!(g.clamp(100), 100);
    }

    // --- run guard ---------------------------------------------------------

    fn ok_inputs(now: u32) -> TickInputs {
        TickInputs {
            now_us: now,
            nfault_high: true,
            host_abort: false,
        }
    }

    fn fresh_guard() -> Guard {
        Guard::new(0, 5_000_000, 1_000_000)
    }

    /// Realistic commutation spacing: comfortably inside both event bounds
    /// (`MIN_EVENT_US = 333`, `EVENT_MAX_US = 1000`).
    const EVENT_SPACING_US: u32 = 500;

    /// Poll frequently, keeping feedback and events fresh.
    ///
    /// Note the event cadence is *not* the poll cadence. An earlier version of
    /// this helper fed an accepted commutation on every 100 us poll, which is
    /// three times faster than the rotor can physically commutate -- once the
    /// fast-side `CompStorm` guard was implemented it correctly rejected that,
    /// and the helper was the thing that was wrong.
    fn healthy_poll(g: &mut Guard, now: u32) -> Option<Reason> {
        g.feedback(now);
        if now.is_multiple_of(EVENT_SPACING_US) {
            let _ = g.accepted(now);
        }
        g.poll(ok_inputs(now))
    }

    #[test]
    fn healthy_ticks_do_not_fault() {
        let mut g = fresh_guard();
        let mut t = 0;
        for _ in 0..1000 {
            t += 100;
            assert_eq!(healthy_poll(&mut g, t), None, "at {t} us");
        }
        assert_eq!(g.fault(), None);
    }

    #[test]
    fn host_abort_has_top_precedence() {
        let mut g = fresh_guard();
        // also assert nFAULT low and a huge elapsed: abort must still win
        let r = g.poll(TickInputs {
            now_us: 9_000_000,
            nfault_high: false,
            host_abort: true,
        });
        assert_eq!(r, Some(Reason::HostAbort));
    }

    #[test]
    fn nfault_outranks_the_deadlines() {
        let mut g = fresh_guard();
        let r = g.poll(TickInputs {
            now_us: 9_000_000,
            nfault_high: false,
            host_abort: false,
        });
        assert_eq!(r, Some(Reason::Driver));
    }

    #[test]
    fn campaign_deadline_outranks_segment_deadline() {
        // campaign shorter than segment so both are due
        let mut g: Guard = Guard::new(0, 1_000, 2_000);
        assert_eq!(g.poll(ok_inputs(5_000)), Some(Reason::CampaignDeadline));
    }

    #[test]
    fn segment_deadline_is_the_normal_stop() {
        let mut g: Guard = Guard::new(0, 5_000_000, 1_000);
        let mut t = 0;
        // stay healthy until the window elapses
        loop {
            t += 100;
            let r = healthy_poll(&mut g, t);
            if let Some(reason) = r {
                assert_eq!(reason, Reason::SegmentDeadline);
                assert!(t >= 1_000);
                break;
            }
            assert!(t < 10_000, "should have stopped by now");
        }
    }

    #[test]
    fn tick_gap_trips_only_beyond_the_allowance() {
        let mut g = fresh_guard();
        assert_eq!(healthy_poll(&mut g, 100), None); // establishes a predecessor
        assert_eq!(
            healthy_poll(&mut g, 100 + TICK_GAP_MAX_US),
            None,
            "exactly at limit is ok"
        );
        let t = 100 + TICK_GAP_MAX_US + TICK_GAP_MAX_US + 1;
        assert_eq!(healthy_poll(&mut g, t), Some(Reason::TickGap));
    }

    #[test]
    fn first_poll_is_exempt_from_the_tick_gap() {
        let mut g = fresh_guard();
        // A long time after start, but it is the first poll, and neither
        // liveness channel has been primed yet.
        assert_eq!(g.poll(ok_inputs(900_000)), None);
    }

    #[test]
    fn liveness_channels_are_exempt_until_primed_then_armed() {
        // Startup runs ~1 s before the first accepted commutation / ADC frame;
        // an unprimed channel must not trip, and must arm the moment it is fed.
        let mut g = fresh_guard();
        assert_eq!(g.poll(ok_inputs(900_000)), None, "unprimed: no trip");
        // first accepted event, very late relative to start, is legitimate
        assert_eq!(g.accepted(900_100), None, "first event is exempt");
        // now it is armed: a second event a long time later is a fault
        assert_eq!(g.accepted(900_100 + EVENT_MAX_US + 1), Some(Reason::Tracking));

        // and the same for feedback
        let mut g2 = fresh_guard();
        assert_eq!(g2.poll(ok_inputs(900_000)), None);
        g2.feedback(900_000);
        let _ = g2.accepted(900_000);
        let t = 900_000 + FEEDBACK_MAX_AGE_US + TICK_GAP_MAX_US;
        let mut hit = None;
        let mut now = 900_000;
        while now < t {
            now += TICK_GAP_MAX_US;
            if now.is_multiple_of(EVENT_SPACING_US) {
                let _ = g2.accepted(now);
            }
            if let Some(r) = g2.poll(ok_inputs(now)) {
                hit = Some(r);
                break;
            }
        }
        assert_eq!(hit, Some(Reason::FeedbackStale), "armed once primed");
    }

    #[test]
    fn a_backwards_event_timestamp_fails_closed() {
        // A timestamp older than the stored reference means the two ISRs
        // disagree about time; wrapping_sub turns it into a huge age, which
        // must fault rather than be silently treated as "fresh".
        let mut g = fresh_guard();
        let _ = g.accepted(500_000);
        assert_eq!(g.accepted(400_000), Some(Reason::Tracking));
    }

    #[test]
    fn stale_feedback_trips() {
        let mut g = fresh_guard();
        g.feedback(0);
        let _ = g.accepted(0);
        assert_eq!(g.poll(ok_inputs(100)), None);
        // advance in legal tick steps but never refresh feedback
        let mut t = 100;
        let mut hit = None;
        for _ in 0..40 {
            t += TICK_GAP_MAX_US;
            if t.is_multiple_of(EVENT_SPACING_US) {
                let _ = g.accepted(t); // keep events fresh, at a physical cadence
            }
            if let Some(r) = g.poll(ok_inputs(t)) {
                hit = Some(r);
                break;
            }
        }
        assert_eq!(hit, Some(Reason::FeedbackStale));
    }

    #[test]
    fn tracking_trips_when_accepted_events_stop() {
        let mut g = fresh_guard();
        g.feedback(0);
        let _ = g.accepted(0);
        let mut t = 0;
        let mut hit = None;
        for _ in 0..40 {
            t += TICK_GAP_MAX_US;
            g.feedback(t); // keep feedback fresh so staleness cannot fire first
            if let Some(r) = g.poll(ok_inputs(t)) {
                hit = Some(r);
                break;
            }
        }
        assert_eq!(hit, Some(Reason::Tracking));
    }

    #[test]
    fn a_late_accepted_event_is_a_tracking_fault() {
        let mut g = fresh_guard();
        let _ = g.accepted(0);
        assert_eq!(g.accepted(EVENT_MAX_US), None, "exactly at the limit is ok");
        assert_eq!(g.accepted(EVENT_MAX_US * 3), Some(Reason::Tracking));
    }

    #[test]
    fn cycle_outside_the_band_is_a_cycle_timing_fault() {
        let mut g = fresh_guard();
        assert_eq!(g.cycle(CYCLE_MIN_US), None);
        assert_eq!(g.cycle(CYCLE_MAX_US), None);
        assert_eq!(g.cycle(5_000), None);
        let mut g2 = fresh_guard();
        assert_eq!(g2.cycle(CYCLE_MIN_US - 1), Some(Reason::CycleTiming));
        let mut g3 = fresh_guard();
        assert_eq!(g3.cycle(CYCLE_MAX_US + 1), Some(Reason::CycleTiming));
    }

    #[test]
    fn a_latched_fault_is_sticky_and_never_downgrades() {
        let mut g = fresh_guard();
        assert_eq!(
            g.poll(TickInputs {
                now_us: 10,
                nfault_high: false,
                host_abort: false
            }),
            Some(Reason::Driver)
        );
        // later polls, even perfectly healthy ones, keep reporting the first fault
        assert_eq!(healthy_poll(&mut g, 200), Some(Reason::Driver));
        assert_eq!(g.fault(), Some(Reason::Driver));
    }

    #[test]
    fn timestamps_wrapping_past_u32_max_do_not_manufacture_faults() {
        let start = u32::MAX - 500;
        let mut g: Guard = Guard::new(start, 5_000_000, 1_000_000);
        let mut t = start;
        // Track the event cadence explicitly rather than with `t % N`: modular
        // alignment is meaningless across the wrap, which is the whole point
        // of this test.
        let mut last_event = start;
        for _ in 0..50 {
            t = t.wrapping_add(100);
            g.feedback(t);
            if t.wrapping_sub(last_event) >= EVENT_SPACING_US {
                assert_eq!(g.accepted(t), None, "wrapped event at {t}");
                last_event = t;
            }
            assert_eq!(g.poll(ok_inputs(t)), None, "wrapped tick at {t}");
        }
    }

    /// The mean current figure is the reference's quantity: mean signed block
    /// residual, scaled by the calibration (`RAW_LIMIT` == 4000 mA).
    #[test]
    fn mean_current_scales_like_the_reference() {
        const ALLOW: u32 = 4_000;
        const ZERO: u32 = 100 * 3 * 2_048;
        // A residual of 1% of the allowance should read 1% of 4000 mA.
        let mut c = AverageCurrent::new(ZERO, ALLOW);
        // Drive each block to a residual of exactly 40 codes below zero_block.
        let per_scan = (ZERO - 40) / (BLOCK_SCANS * 3);
        for _ in 0..(BLOCK_SCANS * 4) {
            let _ = c.accumulate(per_scan as u16, per_scan as u16, per_scan as u16);
        }
        assert!(c.blocks() >= 4, "blocks={}", c.blocks());
        // Residual is positive (draw) and the scaling is linear.
        assert!(c.mean_residual() > 0);
        // ENV-20: scaled by the calibration (`RAW_LIMIT` == 4000 mA), not by the
        // trip threshold -- E291's rule, now applied to the means as well.
        let expect = (c.mean_residual() as i64 * 4_000 / RAW_LIMIT as i64) as i32;
        assert_eq!(c.mean_milliamps(), expect);
        // The same drive under a different threshold reads the same current.
        let mut d = AverageCurrent::new(ZERO, RAW_ALLOW);
        for _ in 0..(BLOCK_SCANS * 4) {
            let _ = d.accumulate(per_scan as u16, per_scan as u16, per_scan as u16);
        }
        assert_eq!(
            d.mean_milliamps(),
            c.mean_milliamps(),
            "mean current must not follow the threshold"
        );
    }

    /// Before any block completes there is no mean to report, and it must read
    /// zero rather than divide.
    #[test]
    fn mean_current_is_zero_before_the_first_block() {
        let mut c = AverageCurrent::new(1_000, 4_000);
        assert_eq!(c.blocks(), 0);
        assert_eq!(c.mean_residual(), 0);
        assert_eq!(c.mean_milliamps(), 0);
        let _ = c.accumulate(1, 1, 1);
        assert_eq!(c.mean_milliamps(), 0);
    }

    /// A regenerating block is retained in the signed mean, not clamped away:
    /// the reference's column is explicitly signed.
    #[test]
    fn regenerating_blocks_pull_the_signed_mean_down() {
        const ALLOW: u32 = 4_000;
        // zero_block small so the accumulated sum exceeds it -> negative
        // residual, i.e. regeneration.
        let mut c = AverageCurrent::new(10, ALLOW);
        for _ in 0..BLOCK_SCANS {
            let _ = c.accumulate(100, 100, 100);
        }
        assert_eq!(c.blocks(), 1);
        assert!(c.mean_residual() < 0, "mean={}", c.mean_residual());
        assert!(c.mean_milliamps() < 0);
    }

    #[test]
    fn back_to_back_accepted_events_are_a_comparator_storm() {
        // The fast side of the event watchdog. Two accepts closer together
        // than `EVENT_MIN_US` cannot be real commutations at any speed this
        // firmware runs, so they are comparator noise.
        //
        // Driven by the constant rather than a literal: this test previously
        // hard-coded 332/333 and so silently asserted a floor of mine rather
        // than the reference's 238 (notebook E032). A test that pins a number
        // the reference disagrees with defends the wrong thing.
        let mut g = fresh_guard();
        assert_eq!(g.accepted(10_000), None, "first event is exempt");
        assert_eq!(g.accepted(10_000 + EVENT_MIN_US - 1), Some(Reason::CompStorm));

        // Exactly at the floor is admissible.
        let mut g2 = fresh_guard();
        assert_eq!(g2.accepted(10_000), None);
        assert_eq!(g2.accepted(10_000 + EVENT_MIN_US), None);

        // And the reference's own locked cadence must pass: its average
        // accepted-event interval is 425 us (`average_half_us=850` at 392
        // eHz), which the old 333 us floor also passed -- but a quarter-early
        // event at 318 us did not, and the reference accepts those.
        let mut g3 = fresh_guard();
        assert_eq!(g3.accepted(10_000), None);
        assert_eq!(g3.accepted(10_000 + 318), None, "reference accepts an early event here");
        assert_eq!(g3.accepted(10_000 + 318 + 425), None, "and the nominal cadence");
    }

    #[test]
    fn reason_codes_are_wire_stable() {
        assert_eq!(Reason::CampaignDeadline.code(), 1);
        assert_eq!(Reason::SegmentDeadline.code(), 2);
        assert_eq!(Reason::TickGap.code(), 3);
        assert_eq!(Reason::FeedbackStale.code(), 4);
        assert_eq!(Reason::Current.code(), 5);
        assert_eq!(Reason::Bus.code(), 6);
        assert_eq!(Reason::Driver.code(), 7);
        assert_eq!(Reason::Tracking.code(), 8);
        assert_eq!(Reason::HostAbort.code(), 9);
        assert_eq!(Reason::InvalidSeed.code(), 10);
        assert_eq!(Reason::AdcTimeout.code(), 11);
        assert_eq!(Reason::CycleTiming.code(), 12);
        assert_eq!(Reason::CompStorm.code(), 13);
        assert_eq!(Reason::HandlerOverrun.code(), 14);
        assert_eq!(Reason::AverageCurrent.code(), 25);
        assert_eq!(Reason::FastBusSag.code(), 26);
        assert_eq!(Reason::PhasePeak.code(), 27);
    }
}
