//! What the run needs from the board, as one trait, and the gate capability.
//!
//! [`Hal`] is everything the typestate run does to hardware and to the ISR
//! seam, one method per thing it does. The production implementation is the
//! binary's `Board` (over `hw` and `shared::SHARED`); host tests implement it
//! with a simulated motor (`super::sim`).
//!
//! **Gate writes need [`Gates`].** Every call that can make a gate pin switch
//! -- applying a six-step plan, programming compares, enabling MOE, handing
//! the pins to TIM1 -- takes `&mut Gates<S>` with `S: Drives`, and only the
//! states that drive the bridge (`Armed`, `Startup`, `Locked`) implement
//! [`Drives`]. A gate write from `Idle`, `Handover` or `Stopped` does not
//! compile. [`Hal::safe_off`] consumes the capability: it is the only way a
//! run reaches `Stopped`, and the bridge is off before the stopped state
//! exists. Safing calls (MOE off, all channels off) need no capability: they
//! can only remove drive.
//!
//! **Text output is not here.** Writing goes through [`crate::report::Sink`],
//! which the `Idle` and `Stopped` transitions take and the driving states do
//! not: the `Locked` state owns no serial handle, so the "no UART during
//! lock" rule (E073) is a compile-time property. `Hal` can still drain bytes
//! queued before the run and read the host's stop key; it cannot queue any.
//!
//! # The claims, checked by the compiler
//!
//! A driving state may write the gates:
//!
//! ```
//! use firmware50::run::hal::{Gates, Hal, Locked};
//! use firmware50::sixstep::Plan;
//! fn commutate(hal: &mut impl Hal, g: &mut Gates<Locked>, p: &Plan) {
//!     hal.apply_plan(g, p);
//! }
//! ```
//!
//! The handover may not (nor may `Idle` or `Stopped`: no `Drives`):
//!
//! ```compile_fail,E0277
//! use firmware50::run::hal::{Gates, Hal, Handover};
//! use firmware50::sixstep::Plan;
//! fn commutate(hal: &mut impl Hal, g: &mut Gates<Handover>, p: &Plan) {
//!     hal.apply_plan(g, p);
//! }
//! ```
//!
//! ```compile_fail,E0277
//! use firmware50::run::hal::{Gates, Hal, Idle};
//! fn arm(hal: &mut impl Hal, g: &mut Gates<Idle>) {
//!     hal.moe_on(g);
//! }
//! ```
//!
//! ```compile_fail,E0277
//! use firmware50::run::hal::{Gates, Hal, Stopped};
//! fn after(hal: &mut impl Hal, g: &mut Gates<Stopped>) {
//!     hal.gates_to_timer(g);
//! }
//! ```
//!
//! Code generic over `Hal` alone -- every driving state's -- cannot write text:
//!
//! ```compile_fail,E0599
//! use firmware50::report::Sink;
//! fn locked_pass(hal: &mut impl firmware50::run::Hal) {
//!     hal.say("no UART while the comparator is in charge");
//! }
//! ```
//!
//! And `Stopped` cannot be made except through `safe_off`:
//!
//! ```compile_fail,E0451
//! use firmware50::run::Stopped;
//! use firmware50::protection::Reason;
//! fn skip_safe_off(r: Reason) -> Stopped {
//!     Stopped { reason: r, entry: 0, stopped_at: 0, ctx: None, _gates: todo!() }
//! }
//! ```

use core::marker::PhantomData;

use crate::bemf::ZeroCross;
use crate::commutation::Step;
use crate::protection::RawScan;
use crate::report::{GuardRecord, Roots};
use crate::sixstep::Plan;

/// A state that drives the bridge, and so may write the gates.
pub trait Drives {}

/// Run states, as capability markers.
pub struct Idle;
pub struct Armed;
pub struct Startup;
pub struct Handover;
pub struct Locked;
pub struct Stopped;
impl Drives for Armed {}
impl Drives for Startup {}
impl Drives for Locked {}

/// The capability to drive the gates, held by the state `S`. Not `Clone`. A run
/// starts with `Gates<Idle>`, which cannot write; only a passed preflight
/// ([`Gates::after_preflight`]) turns it into `Gates<Armed>`; it moves from state
/// to state and is consumed by [`Hal::safe_off`].
pub struct Gates<S> {
    _s: PhantomData<S>,
}

impl Gates<Idle> {
    /// A run's starting capability. Crate-internal: only the run makes one.
    pub(crate) fn idle() -> Self {
        Self { _s: PhantomData }
    }

    /// The gates are handed to the run only by a preflight that passed; a
    /// failed one hands the idle capability back (to be safed).
    pub fn after_preflight(self, p: &Preflight) -> Result<Gates<Armed>, Self> {
        if p.passed() {
            Ok(Gates { _s: PhantomData })
        } else {
            Err(self)
        }
    }
}

impl<S> Gates<S> {
    /// Move the capability to the next state. Crate-internal: only the run's
    /// transitions do this.
    pub(crate) fn pass<N>(self) -> Gates<N> {
        Gates { _s: PhantomData }
    }

    /// End the capability. For [`Hal::safe_off`] implementations, after the
    /// bridge is off: `Gates<Stopped>` can write nothing.
    #[must_use]
    pub fn into_stopped(self) -> Gates<Stopped> {
        Gates { _s: PhantomData }
    }
}

/// The disabled-bridge readback taken before energizing and after every stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preflight {
    pub moe: bool,
    pub ccr: [u32; 3],
    pub gates_low: bool,
    pub enable: bool,
    pub nfault_high: bool,
}

impl Preflight {
    /// Fail closed: MOE clear, compares zero, gates low, ENABLE low, and
    /// nFAULT already healthy *before* enabling.
    #[must_use]
    pub const fn passed(&self) -> bool {
        !self.moe
            && self.ccr[0] == 0
            && self.ccr[1] == 0
            && self.ccr[2] == 0
            && self.gates_low
            && !self.enable
            && self.nfault_high
    }
}

/// One driven-stage acceptance, as the COMP root published it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrivenAccept {
    pub raw: u16,
    pub epoch: u16,
    pub step: Step,
    pub interval_us: u32,
    pub position_us: u16,
}

/// Gate-4 stimuli (E080): which protection a run provokes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Inject {
    /// The detector stops deciding. Expect `Tracking` (8).
    Tracking,
    /// Interrupts masked for `INJECT_STALL_US`. Expect `TickGap` (3).
    TickGap,
    /// The ADC's DMA channel stalled. Expect `FeedbackStale` (4).
    FeedbackStale,
    /// nFAULT pulled low from the MCU side. Expect `Driver` (7).
    Driver,
    /// 80 software-pended COMP entries in one 1 ms bucket. Expect `CompStorm` (13).
    Storm,
    /// A step to the 50% rung into the 1 A supply. Expect `FastBusSag` (26).
    Sag,
    /// 100 ms without feeding the watchdog. Expect an IWDG reset.
    Watchdog,
    /// 60 µs added to the measured COMP call. Expect `HandlerOverrun` (14).
    Overrun,
    /// A 1/100 allowance with the plans held. Expect `AverageCurrent` (25).
    AverageCurrent,
}

impl Inject {
    /// The reason code the provocation expects (0: the watchdog's evidence is
    /// the next boot's reset flag).
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Inject::Tracking => 8,
            Inject::TickGap => 3,
            Inject::FeedbackStale => 4,
            Inject::Driver => 7,
            Inject::Storm => 13,
            Inject::Overrun => 14,
            Inject::Sag => 26,
            Inject::AverageCurrent => 25,
            Inject::Watchdog => 0,
        }
    }
}

/// Everything the run does to the board and to the ISR seam.
pub trait Hal {
    // ---- time and the link ----
    /// The extended µs clock (feeds the watchdog).
    fn now(&mut self) -> u32;
    /// The raw 16-bit TIM17 count.
    fn raw(&self) -> u16;
    /// A raw count from the recent past on the extended timeline.
    fn stamp_from_raw(&mut self, raw: u16) -> u32;
    /// Push at most one already-queued byte to the UART.
    fn drain(&mut self);
    /// Drain the queue to completion (outside a run).
    fn flush_link(&mut self);
    /// One received byte, if any (non-blocking).
    fn rx(&mut self) -> Option<u8>;
    /// The board LED.
    fn led(&mut self, on: bool);

    // ---- driver and feedback ----
    fn nfault_high(&self) -> bool;
    /// DRV8304 ENABLE: wakes the driver and biases its amplifiers.
    fn enable(&mut self, on: bool);
    fn preflight(&self) -> Preflight;
    /// A DMA scan the foreground has not consumed yet?
    fn adc_due(&mut self) -> bool;
    /// No fresh scan for longer than the stale limit?
    fn adc_stale(&self, now: u32) -> bool;
    /// The latest scan, read consistently.
    fn scan(&mut self) -> Option<RawScan>;
    /// Restart the DMA scan aligned (every run starts from one; E080).
    fn resync_adc(&mut self);
    /// VDDA, mV, from an averaged VREFINT code.
    fn vdda_mv(&self, vref: u16) -> u32;
    /// The ADC-BEMF pair `(vsenc, star)` (unscanned since E041: zeros).
    fn comp_inputs(&self) -> (u16, u16);
    /// TIM1's counter, for the witness's OFF-window gate.
    fn pwm_counter(&self) -> u32;

    // ---- the bridge: gate writes need the capability ----
    fn gates_to_timer<S: Drives>(&mut self, g: &mut Gates<S>);
    fn all_phases_pwm<S: Drives>(&mut self, g: &mut Gates<S>);
    /// Logical per-phase compares, mapped through the wiring.
    fn set_compares<S: Drives>(&mut self, g: &mut Gates<S>, logical: [u32; 3]);
    fn apply_plan<S: Drives>(&mut self, g: &mut Gates<S>, plan: &Plan);
    fn moe_on<S: Drives>(&mut self, g: &mut Gates<S>);
    /// The PWM period (no switching by itself).
    fn set_period(&mut self, period: u32);
    /// Every channel off: the rotor floats. Safing, so no capability.
    fn float_all(&mut self);
    /// MOE off, compares zero, gates low, ENABLE low -- and the capability
    /// ends here.
    fn safe_off<S>(&mut self, g: Gates<S>) -> Gates<Stopped>;

    // ---- the comparator ----
    fn comp_mask(&mut self);
    fn comp_arm(&mut self, step: Step);
    fn comp_select(&mut self, step: Step);
    fn comp_level(&self) -> bool;
    fn comp_hysteresis(&mut self, hyst: u8);
    /// The run's own hysteresis (restored after the coast).
    fn comp_run_hysteresis(&mut self);
    /// Does this sector's crossing produce a rising edge?
    fn edge_rising(&self, step: Step) -> bool;

    // ---- the ISR seam: guard ----
    /// Reset the roots' per-run counters and latches.
    fn reset_roots(&mut self);
    /// The guard root takes the powered run (and the TIM6 interrupt).
    fn guard_arm(&mut self);
    fn guard_reason(&self) -> u32;
    fn storm(&self) -> bool;
    /// Commutations armed with the wait already spent, and comparator edges
    /// the blanking window latched: campaign 8's two hard stops. Both are
    /// read from the roots' counters, which only ever rise.
    fn late_arms(&self) -> u32;

    /// The detector's cumulative `unstable` count, for the hold-window mark
    /// (E212): the ratio against accepted crossings is only meaningful on a
    /// single stage, and whole-run it mixes the ramp with the hold.
    fn unstable_count(&self) -> u32;
    fn blank_latched(&self) -> u32;
    fn overrun(&self) -> bool;
    fn cap_armed(&self) -> bool;
    fn arm_cap(&mut self);

    // ---- the ISR seam: driven stage ----
    /// Hand the driven stage to COMP at `step` (epoch 0), line armed.
    fn drv_begin(&mut self, step: Step);
    /// The next driven boundary: mask, advance the step and epoch, re-arm.
    fn drv_advance(&mut self, step: Step, epoch: u32);
    /// A new driven acceptance, if the root made one.
    fn drv_poll(&mut self) -> Option<DrivenAccept>;
    /// Re-pend a deferred post-crossing level once its gate opens (the
    /// reference's `resume_deferred`). `true` if it pended a retry.
    fn drv_resume_deferred(&mut self, step: Step) -> bool;
    /// Take the driven stage back from COMP.
    fn drv_end(&mut self);

    // ---- the ISR seam: handover and the closed loop ----
    /// Install the estimator and hand the closed loop to COMP: the seed
    /// crossing at `raw`, logical `step`, `advance`.
    fn det_install(&mut self, zc: ZeroCross, seed_us: u32, raw: u16, step: Step, advance: u32);
    /// Hand commutation to the COM root at `duty` on `period`, and arm the
    /// first commutation to land at `commit_us` on the extended clock.
    fn com_handover(&mut self, duty: u16, period: u32, step: Step, commit_us: u32);
    /// A new accepted crossing's raw stamp, if COMP made one.
    fn det_poll(&mut self) -> Option<u16>;
    /// The estimator's interval, µs.
    fn det_average(&self) -> Option<u32>;
    /// The step the COM root last applied.
    fn com_step(&self) -> Step;
    /// No commutation or blank is armed.
    fn com_idle(&self) -> bool;
    fn com_count(&self) -> u32;
    /// The level revisit: sample the line state and pend one retry inside one
    /// critical section. `true` if admitted.
    fn revisit(&mut self, step: Step) -> bool;
    /// Rebuild the COM root's plan table at `duty` (ceiling `cap`) and swap
    /// it in whole.
    fn publish_plans(&mut self, duty: u16, period: u32, cap: u16);
    /// The advance COMP schedules with.
    fn set_advance(&mut self, advance: u32);
    /// Take the estimator back from COMP (first, before the stop is stamped).
    fn det_release(&mut self);
    /// Take everything else back from the roots: driven stage, COM root,
    /// guard, pace interrupt.
    fn take_back(&mut self);

    // ---- stimuli and the record ----
    fn inject(&mut self, kind: Inject);
    fn undo_inject(&mut self, kind: Inject);
    fn roots_record(&mut self) -> Roots;
    fn guard_record(&mut self, loop_iters_closed: u32, loop_gap_max_us: u32, acquire_us: u32) -> GuardRecord;
}
