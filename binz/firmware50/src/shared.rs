//! The one seam between the interrupt roots and the foreground.
//!
//! Every value an ISR root and the foreground both touch lives in [`SHARED`],
//! grouped by the root that owns it. There are two kinds of member:
//!
//! * **typed atomics** for scalars. Ordering is `Relaxed` unless a member's
//!   doc says otherwise, and that is enough on this core for a stated reason:
//!   the G071 is a single in-order Cortex-M0+, and a root runs to completion
//!   over the code it preempts, so the foreground sees all of a root's stores
//!   at once, in the order the root's program made them. The one hazard
//!   `Relaxed` leaves is the *compiler* sinking a foreground store past the
//!   flag that hands a stage to a root. Each activation flag is therefore
//!   stored with `Release`, and its doc says so. Read-modify-write on thumbv6m
//!   is portable-atomic's interrupt-masked sequence.
//! * [`Seam`] for a struct that crosses: a `Mutex<RefCell<T>>` with a
//!   **priority ceiling** `P`, the highest NVIC priority of any context that
//!   touches it -- the stack resource policy RTIC uses. Three ways in:
//!   - [`Seam::root`], for a root running *at* the ceiling: no masking, no
//!     flag. Every other root that touches the value runs at `P` too and
//!     cannot preempt it, nothing above `P` touches it, and every context
//!     below `P` borrows only with interrupts masked.
//!   - [`Seam::masked`], for a root *below* the ceiling (`ADC_COMP` feeding
//!     the guard's watch): interrupts masked, no flag.
//!   - [`Seam::lock`], for the foreground: interrupts masked, and the
//!     `RefCell` flag checked, so a nested foreground borrow is refused
//!     (`None`) rather than aliased. Refused outright in handler mode.
//!
//!   Both root paths take the handler's [`Root`] token by `&mut`, so while a
//!   root holds one value its closure cannot reach a root path again, and
//!   `lock` refuses it. The compiler checks the ceiling: `root` wants a token
//!   of exactly `P`, `masked` one at or below `P` ([`AtOrBelow`]).
//!
//! Why no `interrupt::free` and no flag at the ceiling (E116): masking inside
//! `ADC_COMP` grew its audited worst path from 1188 to 1315 cycles and pulled
//! a panic path into the root; the flag's round-trips on COMP's refusal path
//! lengthened the handler enough to move the loop out of the cohort.

use core::cell::RefCell;
use core::marker::PhantomData;

use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::{AtomicU32 as CoreU32, AtomicU8, AtomicUsize};
use cortex_m::interrupt::{CriticalSection, Mutex};

use portable_atomic::{AtomicBool, AtomicU32, Ordering};

use crate::bemf::ZeroCross;
use crate::commutation::SixSlot;
use crate::protection::RawScan;
use crate::rate::Rate;
use crate::sixstep::Plan;
use crate::tracking::EventWatch;

// ---------------------------------------------------------------------------
// Priority ceilings and root tokens
// ---------------------------------------------------------------------------

/// An NVIC priority class (the byte written to IPR; the M0+ keeps the top
/// two bits).
pub trait Priority {
    const NVIC: u8;
}

/// The motor roots: `ADC_COMP`, `TIM16` and `DMA1_CHANNEL1`, peers at 0x40.
pub struct Motor;
impl Priority for Motor {
    const NVIC: u8 = 0x40;
}

/// The guard root, `TIM6_DAC_LPTIM1`, alone at 0x00 (the highest).
pub struct Guard;
impl Priority for Guard {
    const NVIC: u8 = 0x00;
}

/// **COMP's class in the COM-above-COMP A/B** (step 6b, `com-top` feature):
/// 0x80, i.e. below the COM root's 0x40 and below the guard's 0x00, so COM
/// may preempt COMP. Without the feature this is [`Motor`] and every motor
/// root stays a peer, which is the qualified arrangement.
///
/// **That the reference runs COM above COMP above 48% duty is the operator's
/// goal statement, not a verified reading of it** (E167): the only quotation
/// this campaign has is *"COMP/COM remain priority 0x40 peers"*, the 48% is
/// firmware50's own arithmetic, and the tree's later measurement moves the
/// crossing to about 75% duty -- i.e. above every rung this A/B ran at.
///
/// The class exists as a *type* because the seam's exclusivity argument is
/// typed: COMP's own values (the estimator, the two rate meters, the
/// diagnostic rings COMP writes) get this ceiling, and the compiler then
/// refuses any attempt to take them from a root that is not COMP. Step 6a
/// moved the last value COM shared with COMP out of the way, which is what
/// makes the split possible at all.
#[cfg(feature = "com-top")]
pub struct CompLow;
#[cfg(feature = "com-top")]
impl Priority for CompLow {
    const NVIC: u8 = 0x80;
}

/// COMP's priority class: its own in the A/B image, [`Motor`] otherwise.
#[cfg(feature = "com-top")]
pub type CompPrio = CompLow;
/// COMP's priority class: its own in the A/B image, [`Motor`] otherwise.
#[cfg(not(feature = "com-top"))]
pub type CompPrio = Motor;

/// A root at priority `Self` may borrow, with interrupts masked, a value
/// whose ceiling is `P`: `Self` is at or below `P`.
pub trait AtOrBelow<P> {}
impl AtOrBelow<Motor> for Motor {}
impl AtOrBelow<Guard> for Motor {}
impl AtOrBelow<Guard> for Guard {}
#[cfg(feature = "com-top")]
impl AtOrBelow<CompLow> for CompLow {}
// **No `AtOrBelow<Motor> for CompLow`.** COMP could then masked-borrow COM's
// `plans` and `six`, and nothing in review would notice; nothing needs it. The
// pre-run review of step 6b flagged it as dead permission (E167).
#[cfg(feature = "com-top")]
impl AtOrBelow<Guard> for CompLow {}

/// Proof, held for the duration of one handler, that the code running is a
/// root at priority `P`. Neither `Send` nor `Sync`, so it cannot be stored in
/// a static or outlive the handler that made it.
pub struct Root<P> {
    _p: PhantomData<(P, *const ())>,
}

impl<P: Priority> Root<P> {
    /// # Safety
    ///
    /// Call only in an interrupt handler whose NVIC priority is `P::NVIC`,
    /// at most once per invocation of that handler, and keep the token inside
    /// it.
    #[inline(always)]
    #[must_use]
    pub const unsafe fn enter() -> Self {
        Self { _p: PhantomData }
    }
}

/// A struct that crosses between contexts: `Mutex<RefCell<T>>` with ceiling
/// `P`. See the module docs for the rule.
pub struct Seam<T, P> {
    cell: Mutex<RefCell<T>>,
    _p: PhantomData<P>,
}

// SAFETY: `Mutex<RefCell<T>>` is `Sync` for `T: Send`; the marker `P` holds no
// data.
unsafe impl<T: Send, P> Sync for Seam<T, P> {}

impl<T, P> Seam<T, P> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            cell: Mutex::new(RefCell::new(value)),
            _p: PhantomData,
        }
    }

    /// Borrow from the foreground: masks interrupts for the duration of `f`
    /// and checks the `RefCell` flag, refusing (`None`) rather than aliasing
    /// if the foreground is already inside a borrow of this value. `None` in
    /// handler mode: roots use [`Seam::root`] or [`Seam::masked`].
    #[inline]
    pub fn lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        if !thread_mode() {
            return None;
        }
        cortex_m::interrupt::free(|cs| match self.cell.borrow(cs).try_borrow_mut() {
            Ok(mut v) => Some(f(&mut v)),
            Err(_) => None,
        })
    }

    /// Borrow from a root below the ceiling, with interrupts masked.
    #[inline(always)]
    pub fn masked<Q: Priority + AtOrBelow<P>, R>(&self, _at: &mut Root<Q>, f: impl FnOnce(&mut T) -> R) -> R {
        cortex_m::interrupt::free(|cs| {
            // SAFETY: interrupts are masked, so nothing preempts this; every
            // root at the ceiling has run to completion (none can be below
            // this one, and those above cannot be interrupted by it); the
            // foreground borrows only inside its own masked section, which
            // this root did not interrupt; and the `&mut` token keeps `f`
            // from reaching any root path again (module docs).
            f(unsafe { &mut *self.cell.borrow(cs).as_ptr() })
        })
    }
}

/// True in thread mode (the foreground). Off-target (host tests) there is no
/// handler mode.
#[inline(always)]
fn thread_mode() -> bool {
    #[cfg(target_os = "none")]
    {
        matches!(
            cortex_m::peripheral::SCB::vect_active(),
            cortex_m::peripheral::scb::VectActive::ThreadMode
        )
    }
    #[cfg(not(target_os = "none"))]
    {
        true
    }
}

impl<T, P: Priority> Seam<T, P> {
    /// Borrow from a root running at the ceiling, without masking and
    /// without the `RefCell` flag.
    ///
    /// The flag is not what makes this exclusive -- the ceiling is -- and on
    /// the COMP root's refusal path its two round-trips were not free: E116
    /// measured the lengthened handler as ~4k more level revisits per 25% run
    /// and an accepted count below the cohort. So a root takes the value
    /// directly, as RTIC's lock-free resource access does.
    #[inline(always)]
    pub fn root<R>(&self, _at: &mut Root<P>, f: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY: the token proves the caller runs at priority `P`, this
        // value's ceiling. Every other root that touches the value runs at
        // `P` too, so it cannot preempt this one; nothing above `P` touches
        // it; every context below `P` borrows only with interrupts masked,
        // which this root cannot interrupt; and the `&mut` token keeps `f`
        // from reaching a root path again -- so no other reference to the
        // value is live while `f` runs (module docs; RTIC's stack resource
        // policy).
        let cs = unsafe { CriticalSection::new() };
        // SAFETY: as above -- no other reference to the value is live, so the
        // `&mut` is unique for `f`'s duration.
        f(unsafe { &mut *self.cell.borrow(&cs).as_ptr() })
    }
}

// ---------------------------------------------------------------------------
// The groups
// ---------------------------------------------------------------------------

/// Open-loop / acquisition comparator edges, recorded by `ADC_COMP` for the
/// foreground.
#[repr(C)]
pub struct Edge {
    /// Raw TIM17 count at the last edge.
    pub raw: AtomicU32,
    /// Incremented once per edge, so the foreground can tell a new edge from
    /// a repeat without a "consumed" flag.
    pub seq: AtomicU32,
    /// Comparator level sampled at the edge.
    pub level: AtomicU32,
}

/// The ISR-owned closed-loop detector (E044). The foreground seeds `zc`,
/// `rate` and the scalars, then hands the stage over by setting `active`
/// (`Release`); `ADC_COMP` decides only while `active` is set.
#[repr(C)]
pub struct Det {
    /// Stage handover flag. Stored `Release` by the foreground.
    pub active: AtomicBool,
    /// Raw TIM17 of the last accepted crossing: the interval reference.
    pub sector_start_raw: AtomicU32,
    /// Current logical sector, published after each commutation.
    pub step: AtomicU32,
    /// Advance level in force.
    pub advance: AtomicU32,
    /// **The accepted crossing's estimate, published for the COM root**
    /// (campaign 8 step 6a): the estimator's blended interval and the
    /// blanking window as of that acceptance.
    ///
    /// COM used to read these by borrowing `zc` itself, which made `det.zc`
    /// the one value two motor roots touch -- and therefore the reason
    /// raising COM above COMP would have been undefined behaviour rather
    /// than a scheduling change (E153). Publishing them here is the
    /// reference's own shape: snapshot what the next stage needs *before*
    /// handing over, rather than sharing the estimator.
    ///
    /// Written by COMP inside the acceptance, read by COM at the
    /// commutation. Both are plain 32-bit loads and stores on this core.
    pub accept_avg: AtomicU32,
    pub accept_blank: AtomicU32,
    /// Incremented on each accepted crossing; `accept_raw`/`accept_wait`
    /// describe that crossing.
    pub accept_seq: AtomicU32,
    pub accept_raw: AtomicU32,
    pub accept_wait: AtomicU32,
    /// Longest entry-stamp-to-arm time, and arms that reached the wait (E083).
    pub spent_max: AtomicU32,
    pub late_arms: AtomicU32,
    /// Refusals by the re-base rule.
    pub rebase: AtomicU32,
    /// The storm count is enforced only once set (E107).
    pub cap_armed: AtomicBool,
    /// The estimator. Ceiling: the motor roots (`ADC_COMP` writes it and
    /// `TIM16` reads its average).
    pub zc: Seam<Option<ZeroCross>, CompPrio>,
    /// The closed loop's COMP call rate: the storm cutoff.
    pub rate: Seam<Rate, CompPrio>,
    /// **Set while COMP is inside the zero-crossing decision** (`com-top`
    /// only): the flag COM reads to count its own preemptions of COMP. Zero
    /// and never written without the feature.
    ///
    /// **Last field on purpose.** This struct is `#[repr(C)]` and the roots
    /// address it by offset, so inserting a field in the middle moves every
    /// later offset and changes the roots' machine code for no behavioural
    /// reason -- which is exactly what happened when this went in above
    /// `zc`, and what `isr_diff.py` caught (E167).
    pub in_decide: AtomicBool,
    /// **Set across `com_arm` itself** -- the ten non-atomic writes that arm
    /// the one-shot. This is the window the COM-top hazard actually lives in
    /// (a dispatch between `phase.store(1)` and `CR1|CEN` could commutate at
    /// once instead of at `crossing + wait`), and `in_decide` spans the whole
    /// decision, so it cannot show whether a single preemption ever landed
    /// here. The step-6 review made exactly that point (E170).
    pub in_arm: AtomicBool,
}

/// Closed-loop COMP health: the storm and handler-budget stops (E107).
#[repr(C)]
pub struct CompHealth {
    pub storm: AtomicBool,
    /// Logical step in force at the storm trip (0 = none), E099.
    pub storm_step: AtomicU32,
    pub overrun: AtomicBool,
    /// Longest closed-loop COMP call this run, µs.
    pub call_max_us: AtomicU32,
    /// Gate-4 stimulus: µs added to the measured call (key `H` only).
    pub overrun_inject_us: AtomicU32,
}

/// The driven-stage observer (E058). Handover as for [`Det`].
#[repr(C)]
pub struct Drv {
    /// Stage handover flag. Stored `Release` by the foreground.
    pub active: AtomicBool,
    /// A gate-closed post-crossing level awaiting the foreground's re-pend.
    pub deferred: AtomicBool,
    /// Raw TIM17 of the last acceptance, or of the stage start.
    pub last_raw: AtomicU32,
    /// Current driven sector and its command epoch.
    pub step: AtomicU32,
    pub epoch: AtomicU32,
    /// The last acceptance, published under `acc_seq`.
    pub acc_seq: AtomicU32,
    pub acc_raw: AtomicU32,
    pub acc_epoch: AtomicU32,
    pub acc_step: AtomicU32,
    pub acc_interval: AtomicU32,
    /// Raw TIM17 at the current sector's commutation, and the acceptance's
    /// position in its sector (diagnostic; E059).
    pub sector_raw: AtomicU32,
    pub acc_pos: AtomicU32,
    /// Refusals by kind.
    pub early: AtomicU32,
    pub unstable: AtomicU32,
    pub defers: AtomicU32,
    /// COMP call rate during the driven stage (report-only).
    pub rate: Seam<Rate, CompPrio>,
}

/// The guard root's state (E076).
#[repr(C)]
pub struct GuardState {
    /// Set while the guard owns the powered run. Stored `Release` on arming.
    pub active: AtomicBool,
    /// Set once the closed loop is in charge and the watch is armed.
    pub tracking: AtomicBool,
    /// First fault (a `Reason` code), 0 while healthy. First wins.
    pub reason: AtomicU32,
    /// The guard's extended µs clock and the raw count it was extended at.
    pub ext: AtomicU32,
    pub raw: AtomicU32,
    pub last_tick: AtomicU32,
    pub start: AtomicU32,
    pub ticks: AtomicU32,
    pub gap_max: AtomicU32,
    /// Feedback freshness: the last scan sequence seen, and when.
    pub adc_seen: AtomicU32,
    pub adc_at: AtomicU32,
    /// The accepted-event envelope. Ceiling: the guard. `ADC_COMP` (below
    /// it) feeds events inside a critical section.
    pub watch: Seam<EventWatch<true>, Guard>,
}

/// The COM root's state (E070).
#[repr(C)]
pub struct Com {
    /// Set while the closed loop commutates through TIM16. Stored `Release`
    /// by the foreground.
    pub active: AtomicBool,
    /// What the armed one-shot will do: **0 idle, 1 commutate, 2 end the
    /// reverse blank, 3 open the line at the blanking floor** (E134).
    ///
    /// Phase 3 was missing from this list until the independent review of
    /// E153 pointed out that the stale comment sat on the very field that
    /// entry's finding turned on -- that `com_late_max_us` is stamped before
    /// this value is dispatched, so it mixes all of these purposes (E169).
    pub phase: AtomicU32,
    /// The logical step applied by the COM root.
    pub step: AtomicU32,
    pub count: AtomicU32,
    /// Raw TIM17 time the armed event is due, and the worst lateness.
    pub sched_raw: AtomicU32,
    pub late_max: AtomicU32,
    pub blank_arms: AtomicU32,
    /// Edges the blanking blank latched before the line opened (E134).
    pub blank_latched: AtomicU32,
    /// The six-slot interval ring gating the reverse blank (E100).
    pub six: Seam<SixSlot, Motor>,
    /// Register plans for every logical step at the current duty, indexed
    /// `step - 1` (sized 8, indexed `& 7`, so no bounds check reaches the
    /// root). The foreground builds a table and swaps it in whole.
    pub plans: Seam<[Option<Plan>; 8], Motor>,
    /// **Commutations that preempted COMP's decision** (`com-top` only): the
    /// event the A/B is about, counted where it happens rather than inferred
    /// from an outcome maximum -- the campaign's own rule (instrument
    /// decisions, not outcomes), and the pre-run review's point that a null at
    /// 47.5% would otherwise be indistinguishable from "it never happened"
    /// (E167). Never written without the feature; last field, as for
    /// `Det::in_decide`.
    pub preempts: AtomicU32,
    /// Of those, the ones that landed inside `com_arm`'s own write sequence:
    /// the hazard's own window (E170).
    pub arm_preempts: AtomicU32,
}

/// A scan word: `core`'s `AtomicU32`, whose load and store are a plain
/// `ldr`/`str` on thumbv6m. portable-atomic (with its `critical-section`
/// feature) masks interrupts around every access, which grew the DMA root
/// from 99 to 123 cycles (E116); the words need no read-modify-write.
type Word = core::sync::atomic::AtomicU32;

/// The latest ADC scan, published by the DMA root under a seqlock.
#[repr(C)]
pub struct Scan {
    /// Odd while the DMA root writes, +2 per scan. `SeqCst` at both ends.
    pub seq: AtomicU32,
    phase_a: Word,
    phase_b: Word,
    phase_c: Word,
    bus: Word,
    vref: Word,
}

impl Scan {
    /// The DMA root's publish, straight-line.
    #[inline(always)]
    pub fn publish(&self, s: &RawScan) {
        self.seq.fetch_add(1, Ordering::SeqCst);
        self.phase_a
            .store(u32::from(s.phase_a), core::sync::atomic::Ordering::Relaxed);
        self.phase_b
            .store(u32::from(s.phase_b), core::sync::atomic::Ordering::Relaxed);
        self.phase_c
            .store(u32::from(s.phase_c), core::sync::atomic::Ordering::Relaxed);
        self.bus.store(u32::from(s.bus), core::sync::atomic::Ordering::Relaxed);
        self.vref
            .store(u32::from(s.vref), core::sync::atomic::Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::SeqCst);
    }

    /// A consistent copy of the latest scan and its sequence, or `None` if
    /// every one of eight attempts collided with the DMA root.
    #[must_use]
    pub fn snapshot(&self) -> Option<(u32, RawScan)> {
        let mut tries = 0u8;
        while tries < 8 {
            let s1 = self.seq.load(Ordering::SeqCst);
            if s1 & 1 == 0 {
                let out = RawScan {
                    phase_a: self.phase_a.load(core::sync::atomic::Ordering::Relaxed) as u16,
                    phase_b: self.phase_b.load(core::sync::atomic::Ordering::Relaxed) as u16,
                    phase_c: self.phase_c.load(core::sync::atomic::Ordering::Relaxed) as u16,
                    bus: self.bus.load(core::sync::atomic::Ordering::Relaxed) as u16,
                    vref: self.vref.load(core::sync::atomic::Ordering::Relaxed) as u16,
                };
                portable_atomic::fence(Ordering::Acquire);
                if self.seq.load(Ordering::SeqCst) == s1 {
                    return Some((s1, out));
                }
            }
            tries += 1;
        }
        None
    }

    /// The seqlock's value.
    #[inline(always)]
    #[must_use]
    pub fn sequence(&self, order: Ordering) -> u32 {
        self.seq.load(order)
    }
}

/// UART transmit ring capacity. **A power of two**: indices are masked, not
/// reduced with `%`, which avoids a soft division and lets the compiler elide
/// the bounds check (and the `core::fmt` panic formatter behind it).
pub const TX_CAP: usize = 4096;
const TX_MASK: usize = TX_CAP - 1;

/// The UART transmit ring, drained one byte per loop pass.
///
/// Typed atomics throughout -- `core`'s, whose load and store are a plain
/// `ldrb`/`ldr`/`str` on thumbv6m -- so it needs no lock, no `RefCell` and no
/// `static mut`. Every access is `Relaxed`, and that is exact rather than
/// merely sufficient: the foreground is both the only producer (`say`) and
/// the only consumer (`tx_drain`), so the ring's accesses are ordered by
/// program order. No interrupt handler writes to the UART (E073: no report
/// traffic while the comparator is in charge). An E116 `RefCell` version
/// made the per-pass drain an out-of-line call, slowed the run loop ~7% and
/// moved the level-revisit count and the current proxy out of the cohort.
pub struct TxRing {
    buf: [AtomicU8; TX_CAP],
    head: AtomicUsize,
    tail: AtomicUsize,
    /// Bytes dropped because the ring was full. Reported, never silent.
    dropped: CoreU32,
}

impl TxRing {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [const { AtomicU8::new(0) }; TX_CAP],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: CoreU32::new(0),
        }
    }

    /// Enqueue one byte; drops (and counts) when full.
    #[inline]
    pub fn push(&self, b: u8) {
        let head = self.head.load(Relaxed);
        let next = (head + 1) & TX_MASK;
        if next == self.tail.load(Relaxed) {
            self.dropped.store(self.dropped.load(Relaxed).wrapping_add(1), Relaxed);
            return;
        }
        self.buf[head & TX_MASK].store(b, Relaxed);
        self.head.store(next, Relaxed);
    }

    /// Dequeue one byte.
    #[inline]
    pub fn pop(&self) -> Option<u8> {
        let tail = self.tail.load(Relaxed);
        if tail == self.head.load(Relaxed) {
            return None;
        }
        let b = self.buf[tail & TX_MASK].load(Relaxed);
        self.tail.store((tail + 1) & TX_MASK, Relaxed);
        Some(b)
    }

    /// The next byte, without dequeuing it.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<u8> {
        let tail = self.tail.load(Relaxed);
        if tail == self.head.load(Relaxed) {
            None
        } else {
            Some(self.buf[tail & TX_MASK].load(Relaxed))
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.head.load(Relaxed) == self.tail.load(Relaxed)
    }

    #[must_use]
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Relaxed)
    }
}

impl Default for TxRing {
    fn default() -> Self {
        Self::new()
    }
}

const fn u() -> AtomicU32 {
    AtomicU32::new(0)
}

const fn f() -> AtomicBool {
    AtomicBool::new(false)
}

const fn w() -> Word {
    Word::new(0)
}

static EDGE: Edge = Edge {
    raw: u(),
    seq: u(),
    level: u(),
};
static DET: Det = Det {
    active: f(),
    sector_start_raw: u(),
    step: AtomicU32::new(1),
    advance: AtomicU32::new(20),
    accept_avg: u(),
    accept_blank: u(),
    accept_seq: u(),
    accept_raw: u(),
    accept_wait: u(),
    spent_max: u(),
    late_arms: u(),
    rebase: u(),
    cap_armed: f(),
    zc: Seam::new(None),
    rate: Seam::new(Rate::new()),
    in_decide: f(),
    in_arm: f(),
};
static COMP: CompHealth = CompHealth {
    storm: f(),
    storm_step: u(),
    overrun: f(),
    call_max_us: u(),
    overrun_inject_us: u(),
};
static DRV: Drv = Drv {
    active: f(),
    deferred: f(),
    last_raw: u(),
    step: AtomicU32::new(1),
    epoch: u(),
    acc_seq: u(),
    acc_raw: u(),
    acc_epoch: u(),
    acc_step: u(),
    acc_interval: u(),
    sector_raw: u(),
    acc_pos: u(),
    early: u(),
    unstable: u(),
    defers: u(),
    rate: Seam::new(Rate::new()),
};
static GUARD: GuardState = GuardState {
    active: f(),
    tracking: f(),
    reason: u(),
    ext: u(),
    raw: u(),
    last_tick: u(),
    start: u(),
    ticks: u(),
    gap_max: u(),
    adc_seen: u(),
    adc_at: u(),
    watch: Seam::new(EventWatch::new(
        0,
        crate::protection::EVENT_MIN_US,
        crate::tracking::EVENT_MAX_US,
    )),
};
static COM: Com = Com {
    active: f(),
    phase: u(),
    step: AtomicU32::new(1),
    count: u(),
    sched_raw: u(),
    late_max: u(),
    blank_arms: u(),
    blank_latched: u(),
    six: Seam::new(SixSlot::seeded(0)),
    plans: Seam::new([None; 8]),
    preempts: u(),
    arm_preempts: u(),
};
static SCAN: Scan = Scan {
    seq: u(),
    phase_a: w(),
    phase_b: w(),
    phase_c: w(),
    bus: w(),
    vref: w(),
};
static PANIC_LINE: AtomicU32 = u();

/// The seam itself: the one way any context reaches shared state.
///
/// Zero-sized. Each group is its own private static behind an accessor, so a
/// root addresses a field as its group's address plus a small immediate
/// offset (a nested static would need the offset built in a register on the
/// M0+, costing every access two cycles; E116 measured it).
pub struct Shared(());

/// The single instance.
pub static SHARED: Shared = Shared(());

impl Shared {
    /// Open-loop comparator edges.
    #[inline(always)]
    #[must_use]
    pub fn edge(&self) -> &'static Edge {
        &EDGE
    }

    /// The closed-loop detector.
    #[inline(always)]
    #[must_use]
    pub fn det(&self) -> &'static Det {
        &DET
    }

    /// Closed-loop COMP health.
    #[inline(always)]
    #[must_use]
    pub fn comp(&self) -> &'static CompHealth {
        &COMP
    }

    /// The driven-stage observer.
    #[inline(always)]
    #[must_use]
    pub fn drv(&self) -> &'static Drv {
        &DRV
    }

    /// The guard root.
    #[inline(always)]
    #[must_use]
    pub fn guard(&self) -> &'static GuardState {
        &GUARD
    }

    /// The COM root.
    #[inline(always)]
    #[must_use]
    pub fn com(&self) -> &'static Com {
        &COM
    }

    /// The latest ADC scan.
    #[inline(always)]
    #[must_use]
    pub fn scan(&self) -> &'static Scan {
        &SCAN
    }

    /// Line number of the last panic, for SWD read-back.
    #[inline(always)]
    #[must_use]
    pub fn panic_line(&self) -> &'static AtomicU32 {
        &PANIC_LINE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_scan() -> Scan {
        Scan {
            seq: u(),
            phase_a: w(),
            phase_b: w(),
            phase_c: w(),
            bus: w(),
            vref: w(),
        }
    }

    #[test]
    fn tx_ring_drops_and_counts_when_full() {
        let r = TxRing::new();
        for i in 0..TX_CAP {
            r.push(i as u8);
        }
        // One slot is kept empty to tell full from empty.
        assert_eq!(r.dropped(), 1);
        assert_eq!(r.pop(), Some(0));
        r.push(7);
        assert_eq!(r.dropped(), 1);
        let mut n = 0;
        while r.pop().is_some() {
            n += 1;
        }
        assert_eq!(n, TX_CAP - 1);
        assert!(r.is_empty());
    }

    #[test]
    fn scan_round_trips_under_the_seqlock() {
        let scan = new_scan();
        let raw = RawScan {
            phase_a: 1,
            phase_b: 2,
            phase_c: 3,
            bus: 4,
            vref: 5,
        };
        scan.publish(&raw);
        let (seq, got) = scan.snapshot().unwrap();
        assert_eq!(seq, 2);
        assert_eq!(
            (got.phase_a, got.phase_b, got.phase_c, got.bus, got.vref),
            (1, 2, 3, 4, 5)
        );
    }

    #[test]
    fn scan_refuses_a_torn_read() {
        let scan = new_scan();
        scan.seq.store(3, Ordering::SeqCst); // mid-write, forever
        assert!(scan.snapshot().is_none());
    }

    #[test]
    fn foreground_lock_refuses_a_nested_borrow_instead_of_aliasing() {
        // Host: `thread_mode()` is true and `interrupt::free` is not callable
        // off-target, so exercise the flag rule `lock` applies through the
        // same RefCell directly.
        let s: Seam<u32, Motor> = Seam::new(1);
        // SAFETY: a token on the host test thread, where nothing preempts.
        let cs = unsafe { CriticalSection::new() };
        let cell = s.cell.borrow(&cs);
        let outer = cell.try_borrow_mut();
        assert!(outer.is_ok());
        assert!(cell.try_borrow_mut().is_err());
    }
}
