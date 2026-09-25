//! The four motor ISR roots' logic and the board's motor wiring, as the
//! production image runs them (moved from the binary in E118; the binary
//! keeps one-line `#[interrupt]` shims).
//!
//! Target-only (`#[cfg(target_os = "none")]` in `lib.rs`): every function
//! here drives `hw` and the [`crate::shared`] seam. The decision logic each
//! root runs is the library's host-tested code (`bemf::ZeroCross::offer`,
//! `rate::Rate`, `tracking::EventWatch`, `commutation::SixSlot`,
//! `sixstep::plan`); what lives here is the order in which a root calls it.

use portable_atomic::Ordering;
use stm32g0xx_hal::rcc::Rcc;
use stm32g0xx_hal::stm32;

use crate::bridge::Bridge;
use crate::capture::{Decision, EdgeLog};
use crate::chain::ChainLog;
use crate::commutation::{self, Direction, Step};
use crate::hw;
use crate::protection::Reason;
use crate::shared::{CompPrio, Guard, Motor, Priority, Root, SHARED as S};
use crate::sixstep;

/// Reverse blank (`bench-reverse-blank`): post-commutation mask length. It
/// arms while the reference's six-slot average is at least 1500 half-µs
/// (`commutation::REVERSE_BLANK_SUM_US`, E100).
pub const REVERSE_BLANK_US: u32 = 280;

/// Put the six gate pins into AF2 so TIM1 drives them.
pub fn gates_to_timer() {
    hw::gpio::gates_to_timer();
}

/// Drive all six gate pins low as plain outputs, reclaiming them from TIM1.
pub fn gates_low_now() {
    hw::gpio::gates_low_now();
}

pub fn gates_all_low() -> bool {
    hw::gpio::gates_all_low()
}

/// Dead-time generator setting, in TIM1 clock ticks.
pub const DTG: u8 = 26;

// ---------------------------------------------------------------------------
// Six-step commutation output
// ---------------------------------------------------------------------------

/// How this bench is physically wired, as one named type.
///
/// **This was `Forward`, and that was the defect behind every symptom in
/// entries E004–E009 of the notebook.** The qualified image on this rig is
/// `reverse48k` (`binz/AGENTS.md:18`, frozen ELF at
/// `captures/reference/reverse_48k_com_top_high_20260919/shell-pwm.elf`), and
/// the replacement motor runs *"in the operator-confirmed desired reverse
/// direction"* (`binz/AGENTS.md:130`). `Reverse` in
/// `firmware50/src/commutation.rs` already implemented exactly the reference's
/// `bench-reverse-phases` permutation -- physical A and B swapped, sectors
/// 1→4, 2→3, 3→2, 4→1, 5→6, 6→5 -- and its own doc comment already called it
/// *"the qualified bench orientation"*. The binary simply never used it.
///
/// Why it broke BEMF specifically: `comparator_inmsel` selects
/// `6 + D::phase(floating)`. Under `Forward` on reverse-wired hardware the mux
/// pointed at the wrong physical pin in every sector that floats A or B --
/// four of six -- so the comparator was reading a driven phase, not the
/// floating one, most of the time. Hence a dead-flat level map, and hence the
/// immunity to hysteresis and polarity changes: the wire was wrong, not the
/// threshold.
///
/// `binz/examples/support/phase_direction.rs` proves the step permutation and
/// the phase swap are the same operation -- its test asserts
/// `ROLES[physical_step(s)] == phase_swapped(ROLES[s])` -- so mapping the step
/// through `Direction::step` before building the register plan is sufficient
/// for the gates, and `comparator_inmsel::<Wiring>` covers the mux.
pub type Wiring = crate::run::policy::Wiring;

/// The physical sector to drive for a logical step.
#[inline]
pub fn physical(step: Step) -> Step {
    <Wiring as Direction>::step(step)
}

/// The register plan for a logical step at the running duty cap.
#[inline]
pub fn plan_for(step: Step, duty_tenths: u16, period: u32) -> Option<sixstep::Plan> {
    sixstep::plan(physical(step), duty_tenths, period, SIXSTEP_DUTY_CAP)
}

/// `INPSEL` value selecting PA3, the star point / virtual neutral.
pub const COMP2_INPSEL_PA3: u8 = 0b10;

/// Hysteresis band for COMP2. 0 = none, 1 = low, 2 = medium, 3 = high.
///
/// **Zero, because that is what the qualified reference runs.**
/// `binz/examples/shell-pwm.rs:1450` writes `COMP2_CSR` as
/// `(0b1000 << 4) | (0b10 << 8) | <hyst> | 1`, where the hysteresis term is
/// `1 << 16` only under its `bench-comp-hyst-low` feature and `0` otherwise --
/// and the default build is the one qualified to 50%.
///
/// I briefly set this to medium after measuring 93792 comparator edges in a
/// 1500 ms coast window (62.5 kHz, against the ~120/s a 60 eHz rotor can
/// produce). That measurement was real but the inference was not: two
/// consecutive coasts from the *same* build then gave 91742 and 1013 edges, a
/// 90x swing, so the quantity was never stable enough to attribute to a
/// setting. Reverted to match the reference rather than to keep a change the
/// data does not support.
// **Re-opened in E038, and set to 1 (low) as a stated divergence.**
//
// The reference runs 0 -- but the reference also runs a *hardware* capture
// filter on the comparator path (`bench-capture-filter`, `filtered_irq_hw`,
// TIM2 input capture), which firmware50 does not have. E037 measured what its
// absence costs: `unstable=92236-103550`, the comparator crossing and crossing
// straight back at the zero crossing where back-EMF is smallest, with 63-64%
// of commutations forced as a result. Hysteresis is the nearest in-peripheral
// substitute for a missing input filter.
//
// E006's earlier revert of hysteresis does not stand as evidence: it ran while
// `Wiring` was `Forward` (E010), with `INMSEL` on a *driven* phase in four
// sectors of six, so it measured chatter on the wrong wire. It was never
// re-measured on the corrected build until now.
// **Back to 0, the reference's value (E050).** Hysteresis was raised to 1 then
// 2 in E039-E040 to suppress chatter, and it did. But E049 established that
// COMP2 here is DC-biased *high* -- 100% high at rest -- and on a comparator
// biased high, hysteresis is asymmetric in effect: rising transitions stay
// easy while falling ones must overcome the offset *plus* the band. That
// raised the bar on exactly the falling-sector transitions the reference's
// alternating polarity depends on, which is a large part of why E046 saw even
// sectors fail -- and I then "fixed" that by arming rising everywhere, which
// produced the floor-paced loop E049 exposed.
// **0 again in E061, now with the reference's own storm control.** E058-E060
// showed the driven observer, at HYST 1, locking onto post-commutation
// transients at a fixed 15-100 us into every sector, where the reference's
// driven accepts (`captures/duty50_885...`, decoded by `decode_di85.py`) are
// real crossings with 715-864 us spread that drift ahead of the drive. The
// reference also takes ~5x the COMP calls (`DRIVENIRQ calls=283` in ~11 ms)
// -- the transitions HYST 1 was suppressing. E051's storm at HYST 0 is what
// the reference bounds with `bench-reverse-irq-cap64` (`crate::rate`),
// not with hysteresis, and that cutoff is now installed.
pub const COMP2_HYST: u8 = 0;

// `COMP_MUX_SETTLE_US` (10 us) is gone, and the reason it was added was a
// misreading of the reference. `comp_input.rs` holds `FILTER_SETTLE_US = 10`
// only inside its `bench-filter-control` branch -- the TIM2 filtered path that
// the qualified image does not compile in (notebook E043). The qualified
// image's actual path masks, rewrites `INMSEL`, selects the edge and clears
// pending, with no settle wait at all. What rejects the mux transient there,
// and now here, is that the in-ISR decision refuses anything closer than the
// reference's 238 us minimum accepted-event interval to the last crossing,
// which every post-commutation transient is.

/// Does this board's sense chain invert the comparator polarity the sector
/// expects?
///
/// The reference treats this as a per-board degree of freedom, not a constant:
/// AM32 carries `#ifdef INVERTED_EXTI` to flip `rising` globally for boards
/// whose signal chain inverts, and binz warns about the same thing in
/// `examples/support/comp_input.rs:91` -- *"minz's rising means raw comparator
/// rises (not rm32 generic inverted HAL)"* -- because the two trees disagree on
/// the sense.
///
/// **Tested and left at `false`: the inversion hypothesis was falsified.**
/// The comparator-level map (fraction of polls showing the expected polarity,
/// binned by time into the sector) came out monotonically *decreasing* --
/// `.40 .34 .32 .31 .28 .29 .28 .23 .23 .29 .28 .27` -- where a genuine
/// crossing must make it increase, so an inverted sense looked likely.
///
/// Inverting it predicts the ratio complements to about `.70`. Measured with
/// the flip applied: `.34 .35 .32 .31 .29 .25 .26 .32 .27 .29 .30 .31` --
/// unchanged at about `.30`. A deterministic level cannot read `.30` under
/// both conventions, so the comparator output is not a fixed function of
/// sector position in *either* sense, and polarity is not the defect. Kept as
/// a named constant because the reference treats it as a real per-board knob
/// and the next person will wonder; the answer is that it was tried.
// **True, transcribed from the reference (E053).** `binz/AGENTS.md:8338`:
// "E209 corrects E208 inference: sustained powered path DOES invert raw COMP
// ... COREPOL physical_raw_inverted=1 confirms." This is AM32's per-board
// `INVERTED_EXTI`, set for this board. E009's rejection of it is void: it ran
// under the `Forward` wiring bug with a polled level detector.
pub const COMP_POLARITY_INVERTED: bool = true;

// The polling-era `expected_level` is gone with the polled detector. Its
// finding -- that a constant expectation made all six sectors accept while
// each alternating phasing killed half -- is recorded in LAB_NOTEBOOK E013-E015
// and E020, which also explains why it was level-matching rather than edge
// detection. The edge path uses `edge_is_rising`.

/// Bring COMP2 up: neutral on the positive input, a floating phase on the
/// negative, and [`COMP2_HYST`] of hardware hysteresis.
pub fn comp2_init() {
    // COMP shares the SYSCFG clock gate on this part. The HAL's `Rcc` does not
    // expose it (`rcc::Enable` is implemented for peripherals the HAL drives,
    // and it drives neither SYSCFG nor COMP), so this is one named field
    // through the PAC.
    hw::comp::init(
        COMP2_INPSEL_PA3,
        commutation::comparator_inmsel::<Wiring>(Step::new_clamped(1)),
        COMP2_HYST,
    );

    // COMP is the highest-priority motor root.
    //
    // Priority, not just enablement, because gate 5 asks for it and because
    // `rm32/CLAUDE.md` records what happens without it: with every IRQ left at
    // level 0, same-priority tail-chaining serialises the comparator behind
    // the housekeeping tick and commutation timing decays under load. On
    // Cortex-M0+ only the **upper two bits** of each priority byte are
    // implemented, and `cortex_m`'s `set_priority` writes the raw byte, so a
    // level is written pre-shifted -- the same trap that made every rm32
    // priority collapse to 0 on M4 ("cortex-m's `NVIC::set_priority(irq, n)`
    // writes the raw byte `n`"). 0x00 is the highest of the four levels.
    //
    // The line itself stays masked here; `comp_exti_arm` opens it at handoff.
    hw::comp::nvic_init(COMP_IRQ_PRIORITY);
    comp_exti_mask();
}

/// NVIC priority byte for the COMP root, pre-shifted for M0+'s two
/// implemented priority bits. 0x00 is the highest of levels 0..3.
///
/// **0x40 by default since E076**, a peer of the COM root and below the guard
/// at 0x00, so the guard can always preempt a comparator storm. The only
/// quotation this campaign has from the reference is *"COMP/COM remain
/// priority 0x40 peers"*, and it carries no file:line here; **the claim that
/// the reference raises COM above COMP above 48% duty comes from the
/// operator's goal statement, not from anything verified in this tree** -- the
/// "48%" itself is firmware50's own arithmetic, and the same notebook later
/// revised the crossing point to about 75% duty (E167 records both).
///
/// **0x80 under the `com-top` feature** (step 6b): below the COM root, so a
/// commutation preempts a zero-crossing decision. Diagnostic only.
pub const COMP_IRQ_PRIORITY: u8 = CompPrio::NVIC;

// ---------------------------------------------------------------------------
// COMP ISR root: COMP2 output -> EXTI line 18 -> ADC_COMP.
//
// This is the first of goal gate 5's four ISR roots, and it is also the fix
// for gates 1 and 2. Four variants of foreground level polling were measured
// (notebook E013-E021) and the failure is structural: a level poll cannot
// resolve a per-sector *edge*, so half the sectors either accept without a
// crossing behind them or never accept at all, and `forced_pct` stalls near
// 40%. The reference takes the edge in hardware -- AM32 through EXTI, and
// binz's qualified path through `COMP2/EXTI18` in
// `examples/support/comp_input.rs` -- so that is what this does.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ISR-owned closed-loop detector (notebook E044).
//
// E043 established that the qualified image locks with the same COMP2 -> EXTI18
// detector firmware50 has, and that the structural difference is *where the
// decision is made*. The reference decides inside its COMP handler -- half-cycle
// gate, persistence reads of the live comparator, estimator update -- and on a
// refusal it leaves the line live so the next edge fires immediately. firmware50
// masked on every edge and deferred the decision to the foreground polling loop,
// then re-armed when the foreground got round to it: a dead window after every
// refused edge, during which the real crossing could pass unseen.
//
// Ownership, because this is shared between an ISR and the foreground:
//
// * The estimator is `S.det().zc`, a `shared::Seam` at the motor roots' ceiling:
//   `ADC_COMP` and `TIM16` borrow it through their `Root<Motor>` token, the
//   foreground only inside a critical section (`crate::shared` states the
//   rule). The foreground seeds it before setting `S.det().active`.
// * Everything the ISR needs from the foreground (step, advance) and everything
//   it publishes back (accepted edge, wait, counters) is an atomic.
// * The ISR works entirely in the **16-bit raw TIM17 timeline**: a sector is
//   at most a few ms and the counter wraps every 65.5 ms, so a wrapping 16-bit
//   difference is an exact interval without the foreground's 32-bit extension.
// ---------------------------------------------------------------------------

// **No acceptance floor in the detector (E074).** `DET_ACCEPT_FLOOR_US` (238 us)
// was the reference's *guard* constant `event_min_us` transcribed into the
// COMP decision. The reference's detector (`minz_core::am32_isr::comp_isr`) has
// no such floor -- only the half-sector gate -- and its guard keeps the event
// and cycle minima report-only at speed (`bench-fast-cycle-report`;
// `DUTY_50_CAMPAIGN.md:1117-1119`), which is how it ran 2.1 keHz at 50%. At the
// 15% target's 704 eHz a sector is 237 us, so a 238 us detector floor would
// refuse every real crossing. Its removal is also the goal's discriminating
// test: a genuine lock is independent of the floor (E049's floor-paced
// artefact tracked it exactly).

/// Persistence depth for the in-ISR filter: the reference's 12 reads
/// (`binz/LOW_DUTY_REPLICATION.md`, "persistence depth | 12 reads").
///
/// **Speed-scheduled since E083**, as the reference schedules it
/// (`map(average_interval, 100, 500, 3, 12)` with the very-fast floor,
/// `minz/core/src/am32_loop.rs:298-305`, in its half-µs units, adapted to this
/// estimator's µs by `FromMicros`). 12 reads through 15%, 8 at 20%, 7 at 25%.
/// Clamped at 12, so the audited loop bound and COMP's worst case are
/// unchanged.
pub use crate::run::policy::DET_FILTER;

/// The ISR-owned estimate, for foreground reads.
///
/// A single `u32` field read, which is atomic on this core, so it cannot tear
/// even while the ISR is updating the estimator.
#[inline]
pub fn det_average_interval() -> Option<u32> {
    S.det()
        .zc
        .lock(|z| z.as_ref().map(crate::bemf::ZeroCross::average_interval))
        .flatten()
}

/// The ISR-owned refusal counters `(accepted, too_early, unstable)`.
///
/// Three independent `u32` reads; each is consistent, the triple may straddle
/// one ISR update, which is acceptable for a report.
#[inline]
pub fn det_counts() -> (u32, u32, u32) {
    S.det()
        .zc
        .lock(|z| z.as_ref().map_or((0, 0, 0), crate::bemf::ZeroCross::counts))
        .unwrap_or((0, 0, 0))
}

/// The zero-crossing decision, made inside the interrupt.
///
/// Returns true if the edge was accepted (and the line should stay masked until
/// the next commutation), false if it was refused (and the line should stay
/// live). Bounded: at most `DET_FILTER`'s 12 comparator reads, no division, no
/// allocation, no loop other than that bounded one.
///
/// Generic over the decision log (`crate::capture`). Production's `NoLog`
/// (`ON == false`) runs [`det_decide_plain`], the decision exactly as it was;
/// the diagnostic image's recorder runs [`det_decide_logged`], its twin with
/// the recording added. Two bodies, not one with dead branches: a closure
/// that merely *captures* the log changed COMP's machine code (E121).
#[inline(always)]
pub fn det_decide<L: EdgeLog, C: ChainLog>(raw: u16, fine0: u16, at: &mut Root<CompPrio>) -> bool {
    if L::ON {
        det_decide_logged::<L, C>(raw, fine0, at)
    } else {
        det_decide_plain::<C>(raw, fine0, at)
    }
}

/// What the COMP root can say about the run's stage from the flags it already
/// owns, for a chain row: bit 0 the driven observer, bit 1 the closed-loop
/// detector, bit 2 the guard's tracking flag. **Ramp and hold are not
/// distinguishable here** -- no root sees the commanded duty -- so that split
/// is made on the host from the row's own clock and accept ordinal, and is
/// labelled there as a host classification (E154).
#[inline(always)]
fn stage_code() -> u8 {
    u8::from(S.drv().active.load(Ordering::Relaxed))
        | (u8::from(S.det().active.load(Ordering::Relaxed)) << 1)
        | (u8::from(S.guard().tracking.load(Ordering::Relaxed)) << 2)
}

/// One acceptance's chain row: coarse µs for pairing, fine ticks for every
/// measured delta (E180). `None` in production, where `C::ON` is false.
#[inline(always)]
#[allow(clippy::type_complexity)]
fn beat_row<C: ChainLog>(
    raw: u16,
    fine0: u16,
    wait: u32,
    step: u8,
    late: bool,
) -> Option<(u16, u16, u16, u32, u16, u8, bool)> {
    if !C::ON {
        return None;
    }
    let fine_now = hw::fine::raw() as u16;
    Some((raw, fine0, fine_now, wait, fine_now.wrapping_sub(fine0), step, late))
}

/// Arm the commutation with the hazard's own window marked, so COM can count
/// a dispatch that lands inside `com_arm`'s write sequence (E170). Folds to a
/// bare `com_arm` in production, where `C::ON` is false.
#[inline(always)]
fn arm_marked<C: ChainLog>(left: u32) {
    if C::ON {
        S.det().in_arm.store(true, Ordering::Relaxed);
    }
    com_arm(left, 1);
    if C::ON {
        S.det().in_arm.store(false, Ordering::Relaxed);
    }
}

/// **Record the causal variable and the thin-margin count** (E208/E212).
///
/// One function, called by both acceptance paths, because E204 found the
/// previous candidate edited only the diagnostic twin and therefore did not
/// run: the production arm is inline in [`det_decide_plain`] and the twin is
/// [`accept`], and anything that must hold of "an acceptance" belongs here
/// rather than in two places.
///
/// `left` is `wait - spent` saturating, i.e. what the arm had left; a late arm
/// is exactly `left == 0`. Both minima are stored **plus one** so that 0 means
/// "no acceptance seen" without a sentinel, which is what `margin_seen`
/// reports. `spent_max_us` cannot answer this question: it is a saturated
/// whole-run maximum that reads 11 µs in 490 captures, including every run
/// that ever latched a late arm.
/// **Both histograms are behind `margin-hist`** and both are incremented here,
/// which is called *after* `arm_marked`/`com_arm` on both acceptance paths. So
/// the instrument's own cost lands after the commutation is already scheduled
/// and **cannot enter the `spent` it is helping to measure** -- the observer
/// effect that would otherwise invalidate the whole exercise, since
/// `P(spent >= 9)` is one of the two factors in the model. It does lengthen the
/// handler, so it can still delay the *next* entry; `comp_call_max_us` bounds
/// that and is reported alongside.
///
/// Bucket maps, both eight wide and both provably in range (the `& 7` is a
/// no-op the compiler drops once it has seen the bound, and a guarantee if it
/// has not -- no panic path, so no branch and no helper call in the prio-0
/// root):
///
/// | `wait_hist` | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
/// |---|---|---|---|---|---|---|---|---|
/// | wait µs | ≤7 | 8 | **9** | 10 | 11 | 12 | 13 | ≥14 |
///
/// `left_hist` is the identity for 0..=6 with 7 meaning "7 or more", so bucket
/// 0 is the latch itself and buckets 1-2 are `thin`'s two components split out.
/// Bucket 2 of `wait_hist` plus everything below it is `P(wait <= 9)`, the
/// quantity E315 needs.
#[inline(always)]
fn note_margin(wait: u32, left: u32) {
    // The accepted average interval, stored by the estimator's borrow just
    // above: the causal variable, and level-invariant (E210 SS1).
    let ci_p1 = S.det().accept_avg.load(Ordering::Relaxed).saturating_add(1);
    let prevc = S.det().ci_min_p1.load(Ordering::Relaxed);
    if prevc == 0 || ci_p1 < prevc {
        S.det().ci_min_p1.store(ci_p1, Ordering::Relaxed);
    }
    if left <= 2 {
        S.det()
            .thin
            .store(S.det().thin.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
    }
    #[cfg(feature = "margin-hist")]
    {
        let wi = if wait >= 14 {
            7
        } else if wait <= 7 {
            0
        } else {
            (wait - 7) as usize
        } & 7;
        let li = if left >= 7 { 7 } else { left as usize } & 7;
        let w = &S.det().wait_hist[wi];
        w.store(w.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
        let l = &S.det().left_hist[li];
        l.store(l.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
    }
    #[cfg(not(feature = "margin-hist"))]
    let _ = wait;
}

/// Production's decision. Keep in step with [`det_decide_logged`].
#[inline(always)]
pub fn det_decide_plain<C: ChainLog>(raw: u16, fine0: u16, at: &mut Root<CompPrio>) -> bool {
    let start = S.det().sector_start_raw.load(Ordering::Relaxed) as u16;
    let count = raw.wrapping_sub(start) as u32;
    // The chain row is filled inside the borrow and pushed after it, because
    // the root token is borrowed by the estimator's closure (E154).
    // (crossing µs, crossing fine, arm fine, wait µs, spent fine, sector, late)
    let mut beat: Option<(u16, u16, u16, u32, u16, u8, bool)> = None;

    // The estimator's borrow returns the accepted crossing's (step, average)
    // so the watch is fed after it ends: the watch borrow needs the token.
    let accepted = S.det().zc.root(at, |zc| {
        let zc = zc.as_mut()?;

        // A crossing lost beyond any plausible interval: re-base on this edge and
        // wait for the next, rather than feeding a meaningless count in.
        let (_, ci_max) = zc.bounds();
        if count > ci_max {
            S.det().sector_start_raw.store(raw as u32, Ordering::Relaxed);
            S.det().rebase.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let step = Step::new_clamped(S.det().step.load(Ordering::Relaxed) as u8);
        let advance = S.det().advance.load(Ordering::Relaxed);
        // Persistence reads the **live** comparator, microseconds after the edge --
        // exactly what AM32's handler does, and what the foreground could never do.
        match zc.offer(count, edge_is_rising(step), advance, &DET_FILTER, hw::comp::level) {
            crate::bemf::Outcome::Accepted { wait, .. } => {
                // The accepted crossing's bookkeeping, then the arm. (A stale
                // comment describing the reverted E142 order stood here until
                // E153 found it; the order below is the qualified one.)
                S.det().sector_start_raw.store(raw as u32, Ordering::Relaxed);
                S.det().accept_raw.store(raw as u32, Ordering::Relaxed);
                S.det().accept_wait.store(wait, Ordering::Relaxed);
                // The estimate this acceptance hands to the commutation, taken
                // here and published before the arm (step 6a). COM reads these
                // instead of borrowing `det.zc`, which is what made the
                // estimator the one value two motor roots touched.
                S.det().accept_avg.store(zc.average_interval(), Ordering::Relaxed);
                S.det().accept_blank.store(zc.blanking(), Ordering::Relaxed);
                S.det().accept_seq.fetch_add(1, Ordering::Relaxed);
                // Arm the commutation, as AM32's `interruptRoutine` arms its COM
                // timer. The line stays masked until the COM root re-arms it.
                // The wait runs from the edge, not from here: subtract what this
                // handler has already spent since its entry stamp `raw`, so the
                // commutation lands at edge + wait whenever the response time is
                // below the wait (E083, R_COMP < wait_time(ci)).
                //
                // **Arming before these stores was tried and rejected** (E142,
                // E144): it moved the reported response time from 11 µs to 10
                // and cost about 1% of rotor speed at 37.5%, where the
                // remaining wait is only 5-6 µs. Below that rung it changed
                // nothing measurable. The order here is the qualified one.
                if S.com().active.load(Ordering::Relaxed) {
                    let spent = (hw::clock::raw()).wrapping_sub(raw) as u32;
                    let left = wait.saturating_sub(spent);
                    arm_marked::<C>(left.max(1));
                    note_margin(wait, left);
                    beat = beat_row::<C>(raw, fine0, wait, step.get(), left == 0);
                    if left == 0 {
                        let n = S.det().late_arms.load(Ordering::Relaxed);
                        S.det().late_arms.store(n.wrapping_add(1), Ordering::Relaxed);
                        // **The two numbers the whole mechanism argument turns
                        // on, captured where they are true** (E314). Only for
                        // the FIRST late arm, so a second cannot overwrite the
                        // one the run stopped on.
                        if n == 0 {
                            S.det().ci_at_late.store(zc.average_interval(), Ordering::Relaxed);
                            S.det().spent_at_late.store(spent, Ordering::Relaxed);
                        }
                    }
                    if spent > S.det().spent_max.load(Ordering::Relaxed) {
                        S.det().spent_max.store(spent, Ordering::Relaxed);
                    }
                }
                Some((step.get(), zc.average_interval()))
            }
            _ => None,
        }
    });
    if C::ON {
        if let Some((crossing_us, crossing_fine, arm_fine, wait_us, spent_fine, sector, late)) = beat {
            C::accept(
                at,
                &crate::chain::Arm {
                    crossing_us,
                    crossing_fine,
                    arm_fine,
                    wait_us,
                    spent_fine,
                    step: sector,
                    late,
                    stage: stage_code(),
                },
            );
        }
    }
    match accepted {
        Some((step, average)) => {
            // The accepted-event envelope (E076).
            guard_event(at, step, average);
            true
        }
        None => false,
    }
}

/// [`det_decide_plain`]'s acceptance arm, for the diagnostic twin (the
/// production body keeps it inline: factoring it out moved COMP's code, E121).
#[inline(always)]
fn accept<C: ChainLog>(
    raw: u16,
    fine0: u16,
    wait: u32,
    avg: u32,
    blank: u32,
) -> Option<(u16, u16, u16, u32, u16, bool)> {
    let mut beat = None;
    S.det().sector_start_raw.store(raw as u32, Ordering::Relaxed);
    S.det().accept_raw.store(raw as u32, Ordering::Relaxed);
    S.det().accept_wait.store(wait, Ordering::Relaxed);
    S.det().accept_avg.store(avg, Ordering::Relaxed);
    S.det().accept_blank.store(blank, Ordering::Relaxed);
    S.det().accept_seq.fetch_add(1, Ordering::Relaxed);
    if S.com().active.load(Ordering::Relaxed) {
        let spent = (hw::clock::raw()).wrapping_sub(raw) as u32;
        let left = wait.saturating_sub(spent);
        com_arm(left.max(1), 1);
        // Gated like `beat_row`'s read (E269): `edge-capture` reaches this
        // path and never calls `hw::fine::init()`, so this was a live read of
        // an unclocked peripheral inside the prio-0 COMP root of the very
        // image whose purpose is measuring that root's cost.
        let fine_now = if C::ON { hw::fine::raw() as u16 } else { 0 };
        beat = Some((raw, fine0, fine_now, wait, fine_now.wrapping_sub(fine0), left == 0));
        if left == 0 {
            let n = S.det().late_arms.load(Ordering::Relaxed);
            S.det().late_arms.store(n.wrapping_add(1), Ordering::Relaxed);
            // **The twin site (E314).** `late_arms` is incremented in TWO
            // places -- here and in `det_decide_plain` -- and an instrument
            // that covers one of them is the "present but inert" failure this
            // campaign has now hit four times. Both capture, first-latch-only.
            if n == 0 {
                S.det().ci_at_late.store(avg, Ordering::Relaxed);
                S.det().spent_at_late.store(spent, Ordering::Relaxed);
            }
        }
        note_margin(wait, left);
        if spent > S.det().spent_max.load(Ordering::Relaxed) {
            S.det().spent_max.store(spent, Ordering::Relaxed);
        }
    }
    beat
}

/// The diagnostic image's decision: [`det_decide_plain`] with every offer's
/// inputs, live reads and outcome recorded. Keep in step with it.
#[inline(always)]
pub fn det_decide_logged<L: EdgeLog, C: ChainLog>(raw: u16, fine0: u16, at: &mut Root<CompPrio>) -> bool {
    let start = S.det().sector_start_raw.load(Ordering::Relaxed) as u16;
    let count = raw.wrapping_sub(start) as u32;
    let rec = L::arm(at);
    let mut log: Option<Decision> = None;
    // Both recorders fill inside the estimator's borrow and push after it: the
    // closure holds the root token (E154).
    let mut beat = None;
    let mut sector = 0u8;
    let accepted = S.det().zc.root(at, |zc| {
        let zc = zc.as_mut()?;
        let (_, ci_max) = zc.bounds();
        if count > ci_max {
            S.det().sector_start_raw.store(raw as u32, Ordering::Relaxed);
            S.det().rebase.fetch_add(1, Ordering::Relaxed);
            log = rec.then(|| Decision::rebase(count));
            return None;
        }
        let step = Step::new_clamped(S.det().step.load(Ordering::Relaxed) as u8);
        let advance = S.det().advance.load(Ordering::Relaxed);
        let rising = edge_is_rising(step);
        let (mut reads, mut n) = (0u16, 0u16);
        let outcome = zc.offer(count, rising, advance, &DET_FILTER, || {
            let l = hw::comp::level();
            reads |= u16::from(l) << (n & 15);
            n += 1;
            l
        });
        log = rec.then(|| Decision::of(count, rising, advance, reads, n, &outcome));
        match outcome {
            crate::bemf::Outcome::Accepted { wait, .. } => {
                beat = accept::<C>(raw, fine0, wait, zc.average_interval(), zc.blanking());
                sector = step.get();
                Some((step.get(), zc.average_interval()))
            }
            _ => None,
        }
    });
    if let Some(d) = log {
        L::push(at, d);
    }
    if C::ON {
        if let Some((crossing_us, crossing_fine, arm_fine, wait_us, spent_fine, late)) = beat {
            C::accept(
                at,
                &crate::chain::Arm {
                    crossing_us,
                    crossing_fine,
                    arm_fine,
                    wait_us,
                    spent_fine,
                    step: sector,
                    late,
                    stage: stage_code(),
                },
            );
        }
    }
    match accepted {
        Some((step, average)) => {
            guard_event(at, step, average);
            true
        }
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Driven-stage observer (E058): the reference's `driven_irq_live::interrupt`.
//
// While the driven six-step runs (sectors scheduled from the sine phase, not
// from the comparator), the ISR runs the ordinary AM32 decision against a
// **fixed** 200 eHz average -- `prepare()` stores 1666 half-µs, so the gate is
// `count > 833` half-µs, i.e. more than 416 µs after the last acceptance (or
// after the stage began). An acceptance commutates nothing: it is recorded for
// `crate::seed` and the line stays masked until the next driven sector.
//
// Gate closed with the level already post-crossing: the reference (under
// `bench-startup-adc`, which the oracle carries) **defers** -- masks, clears,
// and lets its 20 kHz timer re-pend the ISR once the gate opens
// (`resume_deferred`). Here the foreground does that re-pend.
// ---------------------------------------------------------------------------

/// Fixed driven-stage gate: half of the reference's 1666 half-µs average.
pub const DRV_GATE_US: u32 = 416;

// The closed-loop IRQ ring (E064/E071) was removed in E083: it was COMP's
// largest optional cost, and gate 7 needs R_COMP < wait_time(ci) at 25%.

/// The driven-stage decision. Returns true to keep the line masked.
///
/// Bounded: at most 12 comparator reads, no division, no other loop.
#[inline(always)]
pub fn drv_decide(raw: u16) -> bool {
    let count = raw.wrapping_sub(S.drv().last_raw.load(Ordering::Relaxed) as u16) as u32;
    let step = Step::new_clamped(S.drv().step.load(Ordering::Relaxed) as u8);
    let rising = edge_is_rising(step);
    if count <= DRV_GATE_US {
        if hw::comp::level() == rising {
            S.drv().deferred.store(true, Ordering::Relaxed);
            S.drv().defers.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        S.drv().early.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let mut i = 0u8;
    while i < 12 {
        if hw::comp::level() != rising {
            S.drv().unstable.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        i += 1;
    }
    S.drv().acc_interval.store(count, Ordering::Relaxed);
    S.drv().acc_pos.store(
        raw.wrapping_sub(S.drv().sector_raw.load(Ordering::Relaxed) as u16) as u32,
        Ordering::Relaxed,
    );
    S.drv().acc_raw.store(raw as u32, Ordering::Relaxed);
    S.drv()
        .acc_epoch
        .store(S.drv().epoch.load(Ordering::Relaxed), Ordering::Relaxed);
    S.drv().acc_step.store(step.get() as u32, Ordering::Relaxed);
    S.drv().last_raw.store(raw as u32, Ordering::Relaxed);
    S.drv().acc_seq.fetch_add(1, Ordering::Relaxed);
    true
}

// ---------------------------------------------------------------------------
// Guard ISR root: TIM6 update at 101 us (E076; goal gate 5's fourth root).
//
// The reference's powered guard is its TIM6 tick at priority 0
// (`driven_run.rs::tick`, `powered_timer`): every period it checks the
// driver, the tick cadence, feedback freshness and the accepted-event
// envelope, and a fault removes the bridge *from the interrupt*, not from a
// foreground that may itself be the thing that stalled. firmware50 had these
// checks only in its foreground. TIM6 already paces the ADC (TRGO), so its
// update interrupt is the tick; `Timer::listen` (HAL) enables it, and the
// handler clears the flag directly because an ISR cannot own the `Timer`.
// ---------------------------------------------------------------------------

/// NVIC priority byte for the guard: the highest, above COMP/COM (0x40).
pub const GUARD_IRQ_PRIORITY: u8 = Guard::NVIC;
/// Backstop on the whole powered run, µs: the last stop if every other one
/// fails to fire. The foreground ends a run at `policy::BEMF_TOTAL_MS`; this
/// only catches a foreground that never does.
///
/// It must sit **above** [`crate::run::policy::BEMF_TOTAL_MS`] so that an
/// ordinary run ends on its own window rather than on this, and close enough
/// above it that a run which loses its foreground still stops.
///
/// **Derived, not written down** (E283). This was a literal 84 s against an
/// 80 s window, and the comment beside it recorded that it had *already* gone
/// stale once ("used to compare 56 s against 54 s -- both stale by a campaign,
/// in the one place that bounds how long the bridge can stay energised",
/// E186 S1). I then moved `BEMF_TOTAL_MS` to 90 s and left this at 84, so the
/// backstop fired **6 s early on every run** and three rung-150 runs stopped
/// with `CampaignDeadline` instead of `SegmentDeadline` -- the third time this
/// coupling has gone stale, in a constant whose own doc comment warns about it.
///
/// Now it follows the window by construction, with the same 4 s of margin the
/// literal encoded, and the assertions below make a violation a build error
/// rather than a bench result.
pub const GUARD_CAMPAIGN_MARGIN_US: u32 = 4_000_000;
pub const GUARD_CAMPAIGN_US: u32 = crate::run::policy::BEMF_TOTAL_MS * 1_000 + GUARD_CAMPAIGN_MARGIN_US;

// **The first pair of assertions here were vacuous and I caught it by trying
// to break them** (E283). They compared `GUARD_CAMPAIGN_US` against
// `BEMF_TOTAL_MS`, but the former is now *derived from* the latter, so both
// held for any window -- including a deliberately absurd 200 s. An assertion
// that checks an identity is the can't-fail class this campaign keeps finding.
//
// What actually needs bounding is the window itself, absolutely: it is the
// longest the bridge is ever energised on this bench, and nothing else in the
// firmware caps it. 120 s catches a typo (200_000, 900_000) while leaving room
// above the 90 s the restart campaign needs for a 30 s post-restart dwell.
const _: () = assert!(crate::run::policy::BEMF_TOTAL_MS <= 120_000);
// And the margin must be a real interval, not zero or negative-by-wrap.
const _: () = assert!(GUARD_CAMPAIGN_MARGIN_US >= 1_000_000);
const _: () = assert!(GUARD_CAMPAIGN_US > crate::run::policy::BEMF_TOTAL_MS * 1_000);

/// The guard clock as of now. Call from the guard or inside a critical
/// section, so the (ext, raw) pair cannot change underneath.
#[inline(always)]
pub fn guard_now() -> u32 {
    let raw = hw::clock::raw();
    let base = S.guard().raw.load(Ordering::Relaxed) as u16;
    S.guard()
        .ext
        .load(Ordering::Relaxed)
        .wrapping_add(raw.wrapping_sub(base) as u32)
}

/// Latch a fault and remove the bridge, from interrupt context.
#[inline(always)]
pub fn guard_trip(reason: Reason) {
    if S.guard().reason.load(Ordering::Relaxed) == 0 {
        S.guard().reason.store(reason.code() as u32, Ordering::Relaxed);
    }
    S.guard().active.store(false, Ordering::Relaxed);
    S.guard().tracking.store(false, Ordering::Relaxed);
    S.com().active.store(false, Ordering::Relaxed);
    S.det().active.store(false, Ordering::Relaxed);
    S.drv().active.store(false, Ordering::Relaxed);
    comp_exti_mask();
    com_stop();
    // The interrupt-context subset of `safe_off`, in its order: MOE off (with
    // OSSI set the timer then drives all six gate pins to their idle-low level
    // in hardware), compares to zero, ENABLE low. The full `safe_off`, whose
    // `gates_low` step reclaims the pins through the PAC's indexed `moder(p)`
    // -- a loop with a bounds-check panic path the fail-closed audit refuses in
    // an ISR root -- follows from the foreground as soon as it sees the reason.
    let mut d = Drv8304;
    d.moe_off();
    d.zero_compares();
    d.enable_low();
}

/// Has the guard already latched a stop reason?
///
/// [`guard_trip`] stores the reason *before* it clears any `active` flag, so a
/// context that reads this as non-zero knows a shutdown is in progress even if
/// it has not yet reached the flag it was about to write. The foreground's
/// handover consults it before releasing anything, because releasing is the one
/// operation that can *undo* a stop (E181 SS3.2).
#[inline(always)]
#[must_use]
pub fn guard_latched() -> bool {
    S.guard().reason.load(Ordering::Relaxed) != 0
}

/// Arm the guard for a powered run: clock, tick and feedback baselines.
pub fn guard_arm() {
    let raw = hw::clock::raw() as u32;
    S.guard().raw.store(raw, Ordering::Relaxed);
    S.guard().ext.store(0, Ordering::Relaxed);
    S.guard().last_tick.store(0, Ordering::Relaxed);
    S.guard().start.store(0, Ordering::Relaxed);
    S.guard().ticks.store(0, Ordering::Relaxed);
    S.guard().gap_max.store(0, Ordering::Relaxed);
    S.guard()
        .adc_seen
        .store(S.scan().sequence(Ordering::Relaxed), Ordering::Relaxed);
    S.guard().adc_at.store(0, Ordering::Relaxed);
    S.guard().reason.store(0, Ordering::Relaxed);
    S.guard().tracking.store(false, Ordering::Relaxed);
    hw::nvic::set_priority(stm32::Interrupt::TIM6_DAC_LPTIM1, GUARD_IRQ_PRIORITY);
    hw::nvic::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
    S.guard().active.store(true, Ordering::Release);
    hw::nvic::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
}

/// Arm the tracking watch at the transfer instant.
pub fn guard_arm_tracking() {
    cortex_m::interrupt::free(|_| {
        let now = guard_now();
        let _ = S.guard().watch.lock(|w| {
            *w = crate::tracking::EventWatch::new(now, crate::protection::EVENT_MIN_US, crate::tracking::EVENT_MAX_US);
        });
        S.guard().tracking.store(true, Ordering::Relaxed);
    });
}

/// Disarm the guard at the end of a run.
pub fn guard_disarm() {
    S.guard().active.store(false, Ordering::Relaxed);
    S.guard().tracking.store(false, Ordering::Relaxed);
    hw::nvic::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    hw::nvic::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
}

/// Feed one accepted crossing to the tracking watch, and tighten its deadline
/// to three controller periods (reference `accepted(step, reference_half_us)`).
/// Called from the COMP root, below the watch's ceiling (the guard), so the
/// borrow masks interrupts and keeps the guard out.
#[inline(always)]
pub fn guard_event(at: &mut Root<CompPrio>, step: u8, average_us: u32) {
    if !S.guard().tracking.load(Ordering::Relaxed) {
        return;
    }
    S.guard().watch.masked(at, |w| {
        let now = guard_now();
        let _ = w.tighten_max_interval(crate::tracking::speed_event_limit_us(average_us.saturating_mul(2)));
        if w.event(now, step).is_some() {
            guard_trip(Reason::Tracking);
        }
    });
}

/// The guard root.
///
/// Straight-line and bounded: no loop, no division. Its checks follow the
/// reference's precedence for the parts firmware50 runs here -- driver fault,
/// tick gap, feedback age, campaign backstop, then the accepted-event watch.
/// # Safety
///
/// Call only from the `TIM6_DAC_LPTIM1` interrupt handler, whose NVIC priority is
/// `GUARD_IRQ_PRIORITY` (`Guard::NVIC`, set by `guard_arm`), once per invocation: it takes that root's `Root` token.
#[inline(always)]
pub unsafe fn guard_root() {
    // SAFETY: the caller is the guard handler (this fn's contract).
    let mut at = unsafe { Root::<Guard>::enter() };
    // TIM6's status register: the ISR cannot own the HAL `Timer`.
    hw::pace::clear_update();
    // Extend the clock every tick, active or not.
    let raw = hw::clock::raw();
    let base = S.guard().raw.load(Ordering::Relaxed) as u16;
    let now = S
        .guard()
        .ext
        .load(Ordering::Relaxed)
        .wrapping_add(raw.wrapping_sub(base) as u32);
    S.guard().ext.store(now, Ordering::Relaxed);
    S.guard().raw.store(raw as u32, Ordering::Relaxed);
    if !S.guard().active.load(Ordering::Relaxed) {
        return;
    }
    S.guard().ticks.fetch_add(1, Ordering::Relaxed);
    let gap = now.wrapping_sub(S.guard().last_tick.load(Ordering::Relaxed));
    S.guard().last_tick.store(now, Ordering::Relaxed);
    if gap > S.guard().gap_max.load(Ordering::Relaxed) {
        S.guard().gap_max.store(gap, Ordering::Relaxed);
    }
    if !nfault_high() {
        guard_trip(Reason::Driver);
        return;
    }
    if S.guard().ticks.load(Ordering::Relaxed) > 1 && gap > crate::protection::TICK_GAP_MAX_US {
        guard_trip(Reason::TickGap);
        return;
    }
    let seq = S.scan().sequence(Ordering::Relaxed);
    if seq != S.guard().adc_seen.load(Ordering::Relaxed) {
        S.guard().adc_seen.store(seq, Ordering::Relaxed);
        S.guard().adc_at.store(now, Ordering::Relaxed);
    } else if now.wrapping_sub(S.guard().adc_at.load(Ordering::Relaxed)) > crate::protection::FEEDBACK_MAX_AGE_US {
        guard_trip(Reason::FeedbackStale);
        return;
    }
    if now.wrapping_sub(S.guard().start.load(Ordering::Relaxed)) > GUARD_CAMPAIGN_US {
        guard_trip(Reason::CampaignDeadline);
        return;
    }
    if S.guard().tracking.load(Ordering::Relaxed) && S.guard().watch.root(&mut at, |w| w.poll(now).is_some()) {
        guard_trip(Reason::Tracking);
    }
}

/// `Reason` back from its wire code, for the guard's latched fault.
pub fn reason_from_code(code: u32) -> Reason {
    match code {
        1 => Reason::CampaignDeadline,
        3 => Reason::TickGap,
        4 => Reason::FeedbackStale,
        7 => Reason::Driver,
        8 => Reason::Tracking,
        // **Not `SegmentDeadline`** (E269). That is code 2, the *success*
        // code every gate treats as a completed window, so an unrecognised
        // guard code decoded a stop into a pass. `run::states` fixed exactly
        // this in E186 SS6 and this duplicate was left behind -- the copy that
        // *reports* kept the defect while the copy that *gates* was corrected,
        // which is the worst way round.
        _ => Reason::UnknownGuard,
    }
}

// ---------------------------------------------------------------------------
// COM ISR root: TIM16 one-shot commutation (E070; goal gate 5's second root).
//
// The reference commutates from a hardware one-shot armed by the accepted
// crossing -- AM32's `interruptRoutine` arms the COM timer with `waitTime`, and
// its `PeriodElapsedCallback` commutates (`minz/core/src/am32_isr.rs:108-118`,
// `tim1_up_tim16_isr_policy`). firmware50 commutated from the foreground loop
// at `commit_at`, and the E067/E068 loop monitor measured gaps of 165-321 us
// between passes: on the 400-800 us sectors after transfer a commutation could
// land a large fraction of a sector late, which costs torque, moves the next
// crossing, and feeds the comparator chatter that then starves the foreground
// further.
//
// **Seventh PAC escape.** The HAL's `Timer::start` computes its prescaler and
// reload with runtime division -- `let psc = cycles / 0xffff; let arr =
// cycles / (psc + 1);` (`ref/stm32g0xx-hal/src/timer/mod.rs:154-155`) -- which
// the fail-closed audit forbids in an ISR root, re-derives PSC on every call,
// and the general `Timer` offers no one-pulse mode. TIM16 is therefore set up
// once at PSC=63 (exactly 1 MHz at 64 MHz) with OPM, and re-armed by writing
// only ARR/CNT/EGR/DIER/CR1.
// ---------------------------------------------------------------------------

/// NVIC priority byte for the COM root: a peer of COMP (0x40), so neither
/// preempts the other and both may touch the estimator's state -- the
/// reference's "COMP/COM remain priority 0x40 peers" below 48% duty.
pub const COM_IRQ_PRIORITY: u8 = Motor::NVIC;

/// One-time TIM16 setup: 1 MHz, one-pulse, update-only-on-overflow, stopped.
pub fn tim16_init(rcc: &mut Rcc) {
    hw::com_timer::init(rcc);
    hw::nvic::set_priority(stm32::Interrupt::TIM16, COM_IRQ_PRIORITY);
    hw::nvic::mask(stm32::Interrupt::TIM16);
}

/// Arm the one-shot to fire `us` µs from now with the given phase. Bounded,
/// division-free, callable from either ISR root or the foreground (with the
/// line masked or inside a critical section).
#[inline(always)]
pub fn com_arm(us: u32, phase: u32) {
    // Pure arithmetic, deliberately outside the critical section below.
    let arr = if us < 2 {
        1
    } else if us > 0xFFFF {
        0xFFFE
    } else {
        us - 1
    };
    // **The decision and every write are one critical section** -- the arm is
    // atomic with respect to every root that could interleave with it
    // (`oneshot::FIRMWARE_ARM_IS_ATOMIC`, host-tested). Three findings, one
    // structure:
    //
    // * E179's masked *recheck* covered only the enable, and only `stopped`,
    //   while the decision is two flags (`arm_allowed`). The pair was not read
    //   atomically, so the safety still rested on every stop path happening to
    //   clear `active` before it latched -- caller ordering, which is the thing
    //   step 2 set out to stop resting on (E181 SS3.3).
    // * A guard trip anywhere inside the sequence could land between the
    //   decision and the enable, and the arm would re-create timer activity
    //   after the bridge was de-energised (the binz reviewer, against
    //   E173/E174).
    // * `com_arm` has **two** callers that can interleave: COMP's acceptance
    //   and the foreground's handover, which arms while the detector is
    //   already live. A preemption between the stamp and the enable left
    //   `sched_raw`/`phase` from one context and the reload from the other
    //   (E181 SS3.4).
    //
    // The cost is a masked window of a few dozen cycles containing no loop and
    // no division: the flags, a clock read, two stores and the timer writes.
    // `oneshot::ARM_ORDER` is the order inside it -- **disarm, stamp, purpose,
    // enable** -- kept because the disarm-first ordering is still what makes a
    // dispatch from a *previous* arm harmless if one is already pending.
    cortex_m::interrupt::free(|_| {
        // **The one place that decides whether the one-shot may be armed**
        // (`oneshot::arm_allowed`): a latched stop refuses, so work already in
        // flight cannot re-create timer activity after a shutdown, and an
        // inactive loop refuses without latching. Callers do not have to
        // remember.
        if !crate::oneshot::arm_allowed(
            S.com().stopped.load(Ordering::Relaxed),
            S.com().active.load(Ordering::Relaxed),
        ) {
            return;
        }
        hw::com_timer::disable_interrupt();
        let now_raw = hw::clock::raw() as u32;
        S.com()
            .sched_raw
            .store(now_raw.wrapping_add(us) & 0xFFFF, Ordering::Relaxed);
        S.com().phase.store(phase, Ordering::Relaxed);
        hw::com_timer::arm(arr as u16);
    });
}

/// Stop the one-shot and forget any armed event.
pub fn com_stop() {
    // Latch first: from here every `com_arm` is refused, including one already
    // in flight below this context (campaign 9 step 2).
    S.com().stopped.store(true, Ordering::Relaxed);
    hw::com_timer::stop();
    S.com().phase.store(0, Ordering::Relaxed);
    hw::nvic::unpend(stm32::Interrupt::TIM16);
}

/// Fill the inactive plan buffer for `duty` on `period`, then publish it.
pub fn com_publish_plans(duty: u16, period: u32) {
    com_publish_plans_capped(duty, period, SIXSTEP_DUTY_CAP);
}

/// As [`com_publish_plans`] with an explicit duty ceiling. Only the gate-4
/// fast-sag provocation passes anything but `SIXSTEP_DUTY_CAP` (E080).
pub fn com_publish_plans_capped(duty: u16, period: u32, cap: u16) {
    let mut table = [None; 8];
    let mut s = 1u8;
    while s <= 6 {
        table[((s - 1) & 7) as usize] = sixstep::plan(physical(Step::new_clamped(s)), duty, period, cap);
        s += 1;
    }
    let _ = S.com().plans.lock(|t| *t = table);
}

/// Count a commutation that preempted COMP's decision (E167; see
/// [`com_root`]'s docs). Folds away entirely in production, where `C::ON` is
/// false.
#[inline(always)]
fn count_preempt<C: ChainLog>() -> bool {
    if !C::ON {
        return false;
    }
    let in_decide = S.det().in_decide.load(Ordering::Relaxed);
    if in_decide {
        S.com().preempts.fetch_add(1, Ordering::Relaxed);
    }
    // The hazard's own window, counted separately: a dispatch inside
    // `com_arm`'s write sequence is the case that could commutate immediately
    // (E167's reading, E170's instrument). `in_decide` spans the whole
    // decision, so it cannot answer this on its own.
    if S.det().in_arm.load(Ordering::Relaxed) {
        S.com().arm_preempts.fetch_add(1, Ordering::Relaxed);
    }
    in_decide
}

/// The COM root.
///
/// Phase 1: advance the step, apply its precomputed plan, move the mux, then
/// either start the reverse blank (phase 2, 280 µs) or re-arm the line.
/// Phase 2: the blank has ended, re-arm the line. Bounded and straight-line:
/// no loop, no division, no allocation.
///
/// **The preemption counter** (`com_preempts`, E167): if this handler runs
/// while COMP's `in_decide` flag is set, COM has preempted a zero-crossing
/// decision — only possible with COM above COMP (`com-top`). It is counted
/// where it happens rather than inferred from an outcome maximum, and it is
/// carried by the recording images on *both* sides of the A/B so the
/// comparison does not measure the instrument; production folds it away with
/// the rest of the recorder.
///
/// Two long-standing notes on phase 1 and phase 3, moved up here from the body
/// so the root stays inside the structure limit with E141's code untouched
/// (E154 -- the one alternative, extracting phase 1, changed two of TIM16's
/// instructions):
///
/// * **Priming before the blanking floor.** The edge is primed while the line
///   is masked, so the pending flag the blank may latch is a genuine post-mux
///   transition (EXTI latches with IMR clear -- binz Entry 094). Phase 3 then
///   only unmasks, and a crossing that happened inside the window fires at
///   once instead of waiting for the next transition or the foreground poll.
/// * **Why phase 3 counts what it latched.** `blank_latched` is the one
///   quantity that separates "keep the pending flag" from "clear it": a
///   latched flag carries no timestamp, so accepting it dates the crossing at
///   the gate, not at the rotor -- minz's FALCON self-lock class. If the
///   counter stays at zero the question is empty; if it does not, the flag
///   must be cleared and the estimate left to correct itself over the next
///   sectors.
/// # Safety
///
/// Call only from the `TIM16` interrupt handler, whose NVIC priority is
/// `COM_IRQ_PRIORITY` (`Motor::NVIC`, set by `tim16_init`), once per invocation: it takes that root's `Root` token.
#[inline(always)]
pub unsafe fn com_root<C: ChainLog>() {
    // SAFETY: the caller is the TIM16 handler (this fn's contract).
    let mut at = unsafe { Root::<Motor>::enter() };
    hw::com_timer::ack();
    if !S.com().active.load(Ordering::Relaxed) {
        hw::com_timer::disable_interrupt();
        return;
    }
    let preempted = count_preempt::<C>();
    let now_raw = hw::clock::raw();
    let late = now_raw.wrapping_sub(S.com().sched_raw.load(Ordering::Relaxed) as u16) as u32;
    if late < 0x8000 && late > S.com().late_max.load(Ordering::Relaxed) {
        S.com().late_max.store(late, Ordering::Relaxed);
    }
    // The chain's service row (E154), pushed at the end; see `crate::chain`.
    let phase = S.com().phase.load(Ordering::Relaxed);
    let mut bridge = 0u16;
    match phase {
        1 => {
            let step = Step::new_clamped(S.com().step.load(Ordering::Relaxed) as u8).next();
            S.com().step.store(step.get() as u32, Ordering::Relaxed);
            S.det().step.store(step.get() as u32, Ordering::Relaxed);
            let plan = S.com().plans.root(&mut at, |t| t[((step.get() - 1) & 7) as usize]);
            if let Some(pl) = plan {
                hw::pwm::apply_plan(&pl);
            }
            if C::ON {
                bridge = hw::clock::raw();
            }
            comp2_select_floating(step);
            S.com().count.fetch_add(1, Ordering::Relaxed);
            // The blank decision uses the ring as of the previous commutation,
            // then this commutation stores its interval -- the reference's
            // order (`commutate` pushes; `observe_bands` recomputes the
            // average after the COM, `core_bench.rs:2105-2137`).
            //
            // **Read from the accepted crossing's published pair, not from the
            // estimator** (step 6a). COMP writes `accept_avg`/`accept_blank`
            // inside the acceptance and before the arm, so what this reads is
            // the estimate belonging to the crossing that scheduled this
            // commutation -- the same value the borrow returned, by
            // construction, and now without `det.zc` being touched by two
            // roots. That overlap was the reason raising COM above COMP would
            // have been undefined behaviour rather than a scheduling change
            // (E153), and removing it is what makes the step-6b A/B possible.
            let average = S.det().accept_avg.load(Ordering::Relaxed);
            let blanking = S.det().accept_blank.load(Ordering::Relaxed);
            let blank = S.com().six.root(&mut at, |six| {
                let blank = six.reverse_blank_due();
                six.push(step.get(), average);
                blank
            });
            // The reverse blank, or else the blanking floor (E134): the line
            // stays masked for whatever is left of the half-cycle window, since
            // the gate refuses every edge inside it anyway. Phase 2 arms the
            // line when the one-shot fires.
            let hold = if blank {
                REVERSE_BLANK_US
            } else {
                let since = now_raw.wrapping_sub(S.det().accept_raw.load(Ordering::Relaxed) as u16) as u32;
                crate::commutation::blank_remaining(blanking, since)
            };
            if blank {
                comp_exti_mask();
                S.com().blank_arms.fetch_add(1, Ordering::Relaxed);
                com_arm(hold, 2);
            } else if hold >= crate::commutation::BLANK_ARM_MIN_US {
                // Prime the edge while the line is masked (see this fn's docs).
                comp_exti_prime(step);
                S.com().blank_arms.fetch_add(1, Ordering::Relaxed);
                com_arm(hold, 3);
            } else {
                S.com().phase.store(0, Ordering::Relaxed);
                comp_exti_arm(step);
            }
        }
        2 => {
            S.com().phase.store(0, Ordering::Relaxed);
            comp_exti_arm(Step::new_clamped(S.com().step.load(Ordering::Relaxed) as u8));
        }
        // The blanking floor (E134). The edge was primed and the pending flags
        // cleared at the commutation, so anything latched since is real: open
        // the line and keep it.
        3 => {
            S.com().phase.store(0, Ordering::Relaxed);
            // Count the edges the blank latched (see this fn's docs).
            if hw::comp::pending() {
                S.com().blank_latched.fetch_add(1, Ordering::Relaxed);
            }
            hw::comp::line_enable();
        }
        _ => {}
    }
    log_service::<C>(&mut at, now_raw, bridge, late, phase, preempted);
}

/// The chain's service row (E154), pushed after the dispatch so it carries the
/// timer's purpose and the instant the bridge was written. Folds away in
/// production.
#[inline(always)]
fn log_service<C: ChainLog>(at: &mut Root<Motor>, now_raw: u16, bridge: u16, late: u32, phase: u32, preempted: bool) {
    if C::ON {
        // The row carries whether *this* service preempted a decision, in
        // flag bit 7, so the host can split the sampled events instead of
        // comparing a whole-run maximum against an unlabelled tail (E170).
        C::service(
            at,
            now_raw,
            bridge,
            S.com().sched_raw.load(Ordering::Relaxed) as u16,
            late,
            phase | if preempted { 0x80 } else { 0 },
            S.com().step.load(Ordering::Relaxed) as u8,
        );
    }
}

/// COMP2 zero-crossing edge.
///
/// **Masks before it acks.** `EXTI.IMR1[18]` is cleared at entry and only the
/// next commutation re-arms it. That ordering is the fix for a documented
/// latent bug class on this family: `rm32/CLAUDE.md` records that the COMP
/// handler's early-return path could leave the pending bit set and re-fire
/// forever, and explicitly lists G071 as still carrying it ("F051 / G071 /
/// G431 -- same `bemf_zero_cross` early-return path exists; their COMP ISR
/// wrappers don't pre-ack the EXTI line. Latent bug -- same fix needed").
/// Masking at entry also gives the half-cycle blanking gate its teeth: one
/// edge per sector is served, and comparator chatter after it cannot storm the
/// core.
///
/// The handler does no arithmetic beyond a wrapping add and no division, so it
/// is trivially inside gate 5's budget; the estimator still runs in the
/// foreground, which keeps this root short and bounded.
/// # Safety
///
/// Call only from the `ADC_COMP` interrupt handler, whose NVIC priority is
/// `COMP_IRQ_PRIORITY` (`CompPrio::NVIC`, set by `comp2_init`: `Motor`'s 0x40
/// by default, `CompLow`'s 0x80 under `com-top`), once per invocation: it
/// takes that root's `Root` token. The token's type follows the feature, so
/// the contract and the configuration cannot drift apart.
#[inline(always)]
pub unsafe fn comp_root<L: EdgeLog, C: ChainLog>() {
    // Mask and ack first, always. This is the storm guard, and it keeps the
    // documented G071 early-return bug class closed (`rm32/CLAUDE.md`): whatever
    // path is taken below, the pending bit is already clear.
    // NVIC first, then IMR (binz Entry 094, E103): an IMR mask alone does not
    // stop a latched edge from dispatching on the G071 (E102 measured 639).
    hw::comp::line_disable();
    hw::comp::clear_pending();
    let raw = hw::clock::raw();
    // The fine stamp for the chain's own measurements (E180): 125 ns ticks
    // from the free-running TIM2, taken beside the coarse one so both name the
    // same instant. Production runs `NoChain`, so this folds away and TIM2 is
    // never even enabled there.
    let fine0 = if C::ON { hw::fine::raw() as u16 } else { 0 };
    // SAFETY: the caller is the ADC_COMP handler (this fn's contract).
    let mut at = unsafe { Root::<CompPrio>::enter() };

    if S.det().active.load(Ordering::Relaxed) {
        // Storm cutoff (reference `hit_limit` before dispatch). On a trip the
        // line stays masked -- it was masked at entry -- and the foreground
        // removes the bridge.
        // binz's startup policy (E107): during the handover window the count
        // is telemetry; after it, the unchanged 64/ms cutoff.
        let within = S.det().rate.root(&mut at, |rate| {
            if S.det().cap_armed.load(Ordering::Relaxed) {
                rate.hit(raw)
            } else {
                rate.observe(raw);
                true
            }
        });
        if !within {
            S.comp().storm.store(true, Ordering::Relaxed);
            S.comp()
                .storm_step
                .store(S.det().step.load(Ordering::Relaxed), Ordering::Relaxed);
            return;
        }
        // Closed loop: decide here, microseconds after the edge.
        // Mark the decision so COM can count its own preemptions of it.
        // Gated on the **chain recorder**, not on the `com-top` feature: both
        // sides of the A/B must carry the same instrumentation or the
        // comparison measures the instrument as well as the priority (the
        // feature-gated first version made the two images' COMP roots differ
        // by more than the priority byte, which `isr_diff.py` caught, E167).
        // Production runs `NoChain`, so both stores fold away there.
        if C::ON {
            S.det().in_decide.store(true, Ordering::Relaxed);
        }
        let accepted = det_decide::<L, C>(raw, fine0, &mut at);
        if C::ON {
            S.det().in_decide.store(false, Ordering::Relaxed);
        }
        // binz's per-call handler budget (E107), always enforced. On an
        // overrun the line stays masked and the foreground stops the run.
        let elapsed = ((hw::clock::raw()).wrapping_sub(raw) as u32)
            .wrapping_add(S.comp().overrun_inject_us.load(Ordering::Relaxed));
        if elapsed > S.comp().call_max_us.load(Ordering::Relaxed) {
            S.comp().call_max_us.store(elapsed, Ordering::Relaxed);
        }
        if crate::rate::handler_overrun(elapsed) {
            S.comp().overrun.store(true, Ordering::Relaxed);
            return;
        }
        if !accepted {
            // Refused. **Leave the line live** so the real crossing, if it has
            // not happened yet, still fires this sector. This is the reference's
            // behaviour and the point of the change: under the foreground
            // design every refusal masked the line until the foreground next
            // polled, and the crossing could pass inside that dead window.
            //
            // The pending flags were cleared above, so re-enabling cannot
            // re-fire on the same event. binz Entry 094 order (E103).
            hw::comp::line_enable();
        }
        // Accepted: stay masked until the foreground commutates and re-arms for
        // the next sector -- one crossing per sector, as the reference serves.
        return;
    }
    if S.drv().active.load(Ordering::Relaxed) {
        S.drv().rate.root(&mut at, |r| r.observe(raw));
        if !drv_decide(raw) {
            hw::comp::line_enable();
        }
        return;
    }

    // Open loop / acquisition: record the edge for the foreground, as before.
    let level = hw::comp::level();
    S.edge().raw.store(raw as u32, Ordering::Relaxed);
    S.edge().level.store(level as u32, Ordering::Relaxed);
    S.edge().seq.fetch_add(1, Ordering::Relaxed);
}

/// Which comparator edge this sector's crossing produces.
///
/// **Alternating, per the reference -- restored in E050.** AM32 uses
/// `rising = step % 2` and binz drives `RTSR1`/`FTSR1` from the same
/// alternation (`binz/examples/support/comp_input.rs:91-99`).
///
/// E046-E048 replaced this with "rising in every sector" and E049 proved that
/// wrong. On a comparator biased high, a rising edge is always available within
/// about one carrier period -- every PWM dip supplies one -- so arming rising
/// everywhere accepted the first edge after the acceptance floor opened, in
/// every sector, and the loop paced itself off the floor. The genuine signal on
/// a high-biased comparator is the rarer excursion **low**, which is exactly
/// what the falling sectors of the alternation look for, and exactly what
/// arming rising everywhere discarded.
///
/// E046's "post-crossing level is high in every sector" was true but misread:
/// the comparator is high almost all the time, crossing or not, and with
/// `COMP2_HYST = 2` the falling transitions had to clear the offset plus the
/// band. With hysteresis back at the reference's 0 as well, this build's
/// detector matches the reference in edge selection, hysteresis, persistence
/// depth, persistence source (live reads) and decision site (in the ISR).
#[inline]
pub fn edge_is_rising(step: Step) -> bool {
    step.rising() != COMP_POLARITY_INVERTED
}

/// Select this sector's edge and re-arm the line.
///
/// Order matters and follows the reference: mask, select the edge, clear both
/// pending flags, then unmask. Clearing *after* selecting is what discards the
/// transition the mux change itself produced --
/// `binz/examples/support/comp_input.rs` calls `clear_pending()` right after
/// the mux write for exactly this reason, noting that a newly selected phase
/// already past its crossing would otherwise latch a stale event.
pub fn comp_exti_arm(step: Step) {
    hw::comp::line_disable();
    hw::comp::select_edge(edge_is_rising(step));
    hw::comp::clear_pending();
    hw::comp::line_enable();
}

/// Select this sector's edge and clear both pending flags, leaving the line
/// masked (E134). The reference's order without the final unmask: the blanking
/// floor's one-shot does that, and any flag set in between is a real
/// transition on the already-selected phase and edge.
pub fn comp_exti_prime(step: Step) {
    hw::comp::line_disable();
    hw::comp::select_edge(edge_is_rising(step));
    hw::comp::clear_pending();
    hw::nvic::unpend(stm32::Interrupt::ADC_COMP);
}

/// Mask the comparator line and drop any pending edge.
pub fn comp_exti_mask() {
    hw::comp::line_disable();
    hw::comp::clear_pending();
    hw::comp::nvic_unpend();
}

/// Point the comparator's negative input at the sector's floating phase.
#[inline]
pub fn comp2_select_floating(step: Step) {
    hw::comp::select_negative(commutation::comparator_inmsel::<Wiring>(step));
}

// The PWM sample gate and its settling allowance are gone with the polled
// detector: an edge cannot be deferred to a convenient window. Both
// experiments that bracketed it -- removing the gate (desynchronised the loop
// while improving `forced_pct`) and arming only after blanking (killed two
// sectors outright) -- are recorded in LAB_NOTEBOOK E020.

/// Duty ceiling applied to every six-step plan, in tenths of a percent.
///
/// The reference plan clamps at 100 (10.0%), which was its qualified envelope.
/// This target is 15.0%, so the cap is raised deliberately and named here
/// rather than edited into the plan -- the difference from the reference stays
/// visible instead of becoming an unremarked divergence.
///
/// **250 since E083**, for the 20% and 25% rungs. This is the drive envelope,
/// not a protection: every protection limit is unchanged.
pub use crate::run::policy::SIXSTEP_DUTY_CAP;

/// Write *logical* sine compares through the wiring map.
///
/// `set_compares` writes logical A/B/C straight onto the physical channels,
/// which under `Reverse` turns the rotor the opposite way from the six-step
/// loop -- the loop maps every sector through `physical()`, the sine did not.
/// The reference relabels its sine with the same map it uses for the gates
/// (`binz/examples/shell-pwm.rs:1350-1354`: CCR3, physical A, takes
/// `ccr[physical_phase(0)]`), so its sine and its six-step agree. Any sine that
/// hands over to the closed loop must go through here (E058).
#[inline]
pub fn set_compares_wired(logical: [u32; 3]) {
    let mut physical_order = [0u32; 3];
    for p in [commutation::Phase::A, commutation::Phase::B, commutation::Phase::C] {
        physical_order[<Wiring as Direction>::phase(p).index()] = logical[p.index()];
    }
    hw::pwm::set_phase_compares(physical_order);
}

// ---------------------------------------------------------------------------
// ENABLE (PD1) -- also reached from the panic handler, so via the PAC block.
// ---------------------------------------------------------------------------

#[inline]
pub fn enable_set(high: bool) {
    hw::gpio::enable_set(high);
}

#[inline]
pub fn enable_is_high() -> bool {
    hw::gpio::enable_is_high()
}

#[inline]
pub fn nfault_high() -> bool {
    hw::gpio::nfault_high()
}

pub struct Drv8304;

impl Bridge for Drv8304 {
    #[inline]
    fn moe_off(&mut self) {
        hw::pwm::moe_off();
    }

    #[inline]
    fn zero_compares(&mut self) {
        hw::pwm::set_phase_compares([0, 0, 0]);
    }

    #[inline]
    fn gates_low(&mut self) {
        gates_low_now();
    }

    #[inline]
    fn enable_low(&mut self) {
        enable_set(false);
    }
}
