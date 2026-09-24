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

/// Events kept. Circular: the dump ends at the stop, which is where a fault
/// puts the interesting rows (E138 learned this the hard way).
pub const CHAIN_LEN: usize = 1024;

/// One event. Twelve bytes, so the ring is 12 KB.
#[derive(Clone, Copy)]
pub struct Beat {
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub d: u16,
    /// 1 = accept, 2 = COM service.
    pub kind: u8,
    pub step: u8,
    /// accept: stage in the low two bits, late arm in bit 7. service: phase.
    pub flag: u8,
    pub pad: u8,
}

impl Beat {
    const EMPTY: Self = Self {
        a: 0,
        b: 0,
        c: 0,
        d: 0,
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
    /// The accepted crossing's entry stamp, and the instant the one-shot was
    /// armed.
    pub crossing: u16,
    pub arm: u16,
    /// The wait asked for, and what the handler had spent when it armed.
    pub wait: u32,
    pub spent: u32,
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
    fn service(at: &mut Root<Motor>, fire: u16, bridge: u16, sched: u16, late: u32, phase: u32, step: u8);
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
                a: a.crossing,
                b: a.arm,
                c: a.wait as u16,
                d: a.spent as u16,
                kind: 1,
                step: a.step,
                flag,
                pad: 0,
            },
        );
    }

    #[inline(always)]
    fn service(at: &mut Root<Motor>, fire: u16, bridge: u16, sched: u16, late: u32, phase: u32, step: u8) {
        push_to(
            &CHAIN_SVC,
            at,
            Beat {
                a: fire,
                b: bridge,
                c: sched,
                d: late as u16,
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
/// One line per event: `CHAIN kind a b c d step flag`. Written after
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
        out.say_u32(u32::from(b.a));
        out.say(" ");
        out.say_u32(u32::from(b.b));
        out.say(" ");
        out.say_u32(u32::from(b.c));
        out.say(" ");
        out.say_u32(u32::from(b.d));
        out.say(" ");
        out.say_u32(u32::from(b.step));
        out.say(" ");
        out.say_u32(u32::from(b.flag));
        out.say("\r\n");
        out.flush();
        k += 1;
    }
}
