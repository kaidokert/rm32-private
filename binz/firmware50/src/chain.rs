//! The commutation timing chain, event by event (campaign 8, E154).
//!
//! Campaign 7 argued about timing on two aggregate counters and got it wrong
//! twice. `spent_max_us` covers only the handler's entry stamp to just before
//! the arm, and `com_late_max_us` is stamped **before** the timer's purpose is
//! dispatched, so it mixes commutations with the blanking and unmask services
//! (E153). Neither can say where a commutation actually landed relative to the
//! crossing that caused it.
//!
//! This records that directly, one row per event:
//!
//! * **accept** -- the accepted crossing's stamp, the instant the one-shot was
//!   armed, the wait asked for, what the handler had spent at the arm, the
//!   sector, whether the arm was already late, and the run stage;
//! * **service** -- when the COM root ran, when the bridge was actually
//!   updated, the instant it was scheduled for, the lateness the firmware
//!   would have reported, the timer's **purpose** (phase 1 commutation, 2
//!   reverse-blank end, 3 blanking-floor arm) and the sector.
//!
//! The quantity the campaign compares across images is derived from these on
//! the host: **effective angle = (commutation instant − crossing instant) /
//! interval**. Nominal advance level is not comparable between images with
//! different latency chains, which is the trap E145 fell into.
//!
//! Production runs [`NoChain`], whose `ON` is `false`, so every call folds
//! away and the four roots keep their machine code (checked per image, as
//! E121 checks the decision recorder). Only the `chain-capture` binary
//! installs [`ChainRing`].

use crate::shared::{CompPrio, Motor, Root, Seam};

/// Events kept per ring. Circular: the dump ends at the stop, which is where a
/// fault puts the interesting rows (E138 learned this the hard way).
///
/// **512, not 1024, and the reason is a real fault** (E185). E180 widened
/// [`Beat`] for the fine stamps, which took the chain image's `.bss` from
/// 28 792 to 32 888 bytes — leaving under 4 KB of the G071's 36 KB for the
/// stack. Every run of that image then died within a millisecond with a
/// comparator storm or an ADC timeout, and it looked for a while like writing
/// TIM2's `ARR` was somehow stopping TIM6: two builds differing only in the
/// *value* written (0 versus `u32::MAX`, with the counter stopped) behaved
/// differently 2/2 each, because the literal changed the code layout and
/// tipped a stack that was already overlapping `.bss`. This bench's own scar
/// warned about exactly that. Two rings of 512 restore ~16 KB of headroom, and
/// [`RING_BUDGET`] plus `scripts/structure_report.py`'s `.bss` ceiling make the
/// next attempt to grow them fail at the desk instead of on the bench.
///
/// 512 events is 43 ms at 47.5%, which the tail-pairing analysis does not care
/// about: it uses the last n rows of each ring.
pub const CHAIN_LEN: usize = 512;

/// The most `.bss` the two rings together may take.
///
/// The G071 has 36 KB of RAM. Production's entire `.bss` is 4184 B -- the
/// "~20 KB" this comment used to claim was wrong by five times (E186 SS5); what
/// actually needs the room is the **stack**, whose largest single frame
/// (`Controller::run`) reserves 5076 B, on a part with no stack guard. 16 KB of
/// rings leaves ~17 KB, and `scripts/structure_report.py` checks the real
/// figure per image including `.data`.
pub const RING_BUDGET: usize = 16 * 1024;
const _: () = assert!(2 * core::mem::size_of::<Chain>() <= RING_BUDGET);

/// One event. **Fourteen** bytes since E180's fine stamps, so each ring is
/// 7 KB at `CHAIN_LEN` 512 (the doc said twelve and 12 KB; E186 SS5).
///
/// **Stamps are deltas in fine-clock ticks (15.6 ns), not µs** (E180). The
/// quantity this ring exists to measure — the crossing-to-bridge delay and the
/// chain inside it — is 4–20 µs, so a 1 µs stamp quantised it in steps the
/// size of the effect, and two A/B comparisons turned on one tick. A `u16` of
/// fine ticks spans 1.02 ms, which every intra-sector delta fits (the longest
/// sector measured is 188 µs); absolute stamps would not fit, which is why
/// they are deltas.
#[derive(Clone, Copy)]
pub struct Beat {
    /// The **coarse** µs stamp, kept for pairing an acceptance with the
    /// commutation it scheduled: an accept row carries the crossing's µs and a
    /// service row the instant it was scheduled for, so `crossing + wait` and
    /// `sched` match to a microsecond or two. Pairing is the one thing the
    /// coarse clock is still better at, because the control path works in µs.
    pub at_us: u16,
    /// The **fine** stamp, low 16 bits of the 64 MHz free-running TIM2: the
    /// crossing (accept) or the handler's entry (service). Wraps every
    /// 1.02 ms, which every delta of interest is far inside.
    pub at_fine: u16,
    /// accept: the arm instant, fine. service: the bridge write, fine.
    pub x_fine: u16,
    /// accept: the requested wait, **µs** — a control quantity, and genuinely
    /// µs-quantised. service: the lateness the firmware would report, µs.
    pub y_us: u16,
    /// accept: what the handler had spent at the arm, fine. service: unused.
    pub z_fine: u16,
    /// 1 = accept, 2 = COM service.
    pub kind: u8,
    pub step: u8,
    /// accept: stage in the low two bits, late arm in bit 7. service: the
    /// timer's purpose in the low seven bits, preempted in bit 7.
    pub flag: u8,
    pub pad: u8,
}

impl Beat {
    const EMPTY: Self = Self {
        at_us: 0,
        at_fine: 0,
        x_fine: 0,
        y_us: 0,
        z_fine: 0,
        kind: 0,
        step: 0,
        flag: 0,
        pad: 0,
    };
}

pub struct Chain {
    pub on: bool,
    pub total: u32,
    pub len: usize,
    pub beats: [Beat; CHAIN_LEN],
}

/// **One ring per root, not one shared ring** (step 6a). The accept rows are
/// written only by COMP and the service rows only by COM, so no `Seam` here is
/// touched by two roots -- which is the same rule that moved the estimator's
/// published pair out of `det.zc`, and it is what lets the COM-above-COMP A/B
/// run the recording image at all. A shared ring would have to mask
/// interrupts in whichever root ends up lower, and that cost would land on one
/// side of the A/B only.
///
/// The host merges the two by their stamps (`scripts/chain.py`), which are on
/// the same 1 MHz clock.
static CHAIN_ACC: Seam<Chain, CompPrio> = Seam::new(Chain {
    on: false,
    total: 0,
    len: 0,
    beats: [Beat::EMPTY; CHAIN_LEN],
});
static CHAIN_SVC: Seam<Chain, Motor> = Seam::new(Chain {
    on: false,
    total: 0,
    len: 0,
    beats: [Beat::EMPTY; CHAIN_LEN],
});

/// One acceptance's arm, as the COMP root saw it. A struct rather than seven
/// arguments, which clippy refuses -- and which reads no better.
pub struct Arm {
    /// The accepted crossing: its coarse µs stamp (for pairing) and its fine
    /// stamp (for every measured delta).
    pub crossing_us: u16,
    pub crossing_fine: u16,
    /// The instant the one-shot was armed, fine.
    pub arm_fine: u16,
    /// The wait asked for, µs — a control quantity. And what the handler had
    /// spent when it armed, in fine ticks.
    pub wait_us: u32,
    pub spent_fine: u16,
    pub step: u8,
    /// The wait was already exhausted at the arm (`late_arms`).
    pub late: bool,
    /// `roots::stage_code`: what the root can see of the run's stage.
    pub stage: u8,
}

/// What a root may record. `ON` false makes every call a no-op that the
/// optimizer removes, leaving production's roots unchanged.
pub trait ChainLog {
    const ON: bool;

    fn accept(at: &mut Root<CompPrio>, a: &Arm);
    /// `fire_fine`/`bridge_fine` are fine stamps; `sched_us` is the coarse
    /// instant the arm asked for (the pairing key) and `late_us` the lateness
    /// the firmware would report.
    fn service(
        at: &mut Root<Motor>,
        fire_fine: u16,
        bridge_fine: u16,
        sched_us: u16,
        late_us: u32,
        phase: u32,
        step: u8,
    );
}

/// Production's recorder: none.
pub struct NoChain;

impl ChainLog for NoChain {
    const ON: bool = false;

    #[inline(always)]
    fn accept(_: &mut Root<CompPrio>, _: &Arm) {}
    #[inline(always)]
    fn service(_: &mut Root<Motor>, _: u16, _: u16, _: u16, _: u32, _: u32, _: u8) {}
}

/// The diagnostic image's recorder.
pub struct ChainRing;

#[inline(always)]
fn push_to<P: crate::shared::Priority>(ring: &'static Seam<Chain, P>, at: &mut Root<P>, b: Beat) {
    ring.root(at, |c| {
        if !c.on {
            return;
        }
        let i = (c.total as usize) % CHAIN_LEN;
        c.beats[i] = b;
        c.total = c.total.wrapping_add(1);
        if c.len < CHAIN_LEN {
            c.len += 1;
        }
    });
}

impl ChainLog for ChainRing {
    const ON: bool = true;

    #[inline(always)]
    fn accept(at: &mut Root<CompPrio>, a: &Arm) {
        let flag = (a.stage & 0x03) | if a.late { 0x80 } else { 0 };
        push_to(
            &CHAIN_ACC,
            at,
            Beat {
                at_us: a.crossing_us,
                at_fine: a.crossing_fine,
                x_fine: a.arm_fine,
                y_us: a.wait_us as u16,
                z_fine: a.spent_fine,
                kind: 1,
                step: a.step,
                flag,
                pad: 0,
            },
        );
    }

    #[inline(always)]
    fn service(
        at: &mut Root<Motor>,
        fire_fine: u16,
        bridge_fine: u16,
        sched_us: u16,
        late_us: u32,
        phase: u32,
        step: u8,
    ) {
        push_to(
            &CHAIN_SVC,
            at,
            Beat {
                at_us: sched_us,
                at_fine: fire_fine,
                x_fine: bridge_fine,
                y_us: late_us as u16,
                z_fine: 0,
                kind: 2,
                step,
                flag: phase as u8,
                pad: 0,
            },
        );
    }
}

impl ChainRing {
    /// Arm the ring for the next run: empty, recording from the first event.
    pub fn arm_next_run() {
        // Written out per ring rather than looped: under `com-top` the two
        // have different ceilings, so they are different types (step 6b).
        let _ = CHAIN_ACC.lock(|c| {
            c.on = true;
            c.total = 0;
            c.len = 0;
        });
        let _ = CHAIN_SVC.lock(|c| {
            c.on = true;
            c.total = 0;
            c.len = 0;
        });
    }

    /// Stop recording, so the dump cannot race a late event.
    pub fn disarm() {
        let _ = CHAIN_ACC.lock(|c| c.on = false);
        let _ = CHAIN_SVC.lock(|c| c.on = false);
    }

    /// Read the ring in the foreground, interrupts masked for the closure.
    pub fn read<R>(f: impl FnOnce(&Chain, &Chain) -> R) -> Option<R> {
        CHAIN_ACC.lock(|acc| CHAIN_SVC.lock(|svc| f(acc, svc)))?
    }
}

/// The ring as text, oldest row first, parsed by `scripts/chain.py`.
///
/// One line per event: `CHAIN kind at_us at_fine x_fine y_us z_fine step flag`,
/// where the fine columns are 64 MHz ticks (15.6 ns) and `y_us` is µs.
/// Written after
/// `safe_off`, like every other byte -- never while the bridge is live, since
/// 115200-baud edges couple into the comparator.
pub fn emit(acc: &Chain, svc: &Chain, out: &mut impl crate::report::Sink) {
    emit_one(acc, out);
    emit_one(svc, out);
    out.say("CHAINEND\r\n");
    out.flush();
}

/// One ring's rows, oldest first.
fn emit_one(c: &Chain, out: &mut impl crate::report::Sink) {
    out.say("CHAINSNAP ");
    out.kv("len", c.len as u32);
    out.kv("total", c.total);
    // Versioned capture units. The fine stamps are TIM2 ticks, and that rate
    // changed from 15.625 ns to 125 ns in campaign 11 -- an 8x reinterpretation
    // of every recorded delta. `scripts/chain.py` reads the rate from here
    // rather than asserting one, and says so when a capture predates this.
    out.kv("fine_hz", crate::fine::FINE_HZ);
    out.kv("span16_us", crate::fine::SPAN16_US);
    out.say("\r\n");
    out.flush();
    let start = if c.total as usize > c.len {
        c.total as usize % CHAIN_LEN
    } else {
        0
    };
    let mut k = 0;
    while k < c.len {
        let b = &c.beats[(start + k) % CHAIN_LEN];
        out.say("CHAIN ");
        out.say_u32(u32::from(b.kind));
        out.say(" ");
        out.say_u32(u32::from(b.at_us));
        out.say(" ");
        out.say_u32(u32::from(b.at_fine));
        out.say(" ");
        out.say_u32(u32::from(b.x_fine));
        out.say(" ");
        out.say_u32(u32::from(b.y_us));
        out.say(" ");
        out.say_u32(u32::from(b.z_fine));
        out.say(" ");
        out.say_u32(u32::from(b.step));
        out.say(" ");
        out.say_u32(u32::from(b.flag));
        out.say("\r\n");
        out.flush();
        k += 1;
    }
}
