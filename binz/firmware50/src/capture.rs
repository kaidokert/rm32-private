//! Decision capture, for the replay test that pins the 25% loop's
//! accept/refuse sequence (goal item 7).
//!
//! The COMP root's decision is generic over an [`EdgeLog`]. Production uses
//! [`NoLog`], whose `ON` is `false`, so every recording branch folds away and
//! the root's code is unchanged (E121 checks the machine code). The
//! diagnostic `edge-capture` image uses [`Ring`]: once the loop has accepted
//! [`CAPTURE_AFTER_ACCEPTS`] crossings it snapshots the estimator's state and
//! records the next [`CAPTURE_LEN`] decisions, each with its inputs -- the
//! count, the edge polarity, the advance, every live comparator read -- and
//! its outcome. `run::replay` feeds them back through the production policy.
//!
//! Every non-generic item here is `#[inline]`, so it is generated only in an
//! image that uses it: merely compiling `Ring`'s methods into the library
//! changed the production COMP root's machine code (E121).
//!
//! A diagnostic observer changes COMP's timing, so a capture is evidence of
//! what the decision function did with those inputs, not of the production
//! image's loop quality (the rebuild task: "do not claim parity from a
//! diagnostic build").

use crate::bemf::Outcome;
use crate::shared::{CompPrio, Root, Seam};

/// Decisions recorded per capture: 1536 x 12 bytes, about 18 KB of RAM.
pub const CAPTURE_LEN: usize = 1536;
/// Accepted crossings before the capture starts: at 25% the 7.5 s ramp is
/// over, so the capture sees the loop at its target duty.
pub const CAPTURE_AFTER_ACCEPTS: u32 = 80_000;
/// Accepted crossings before a **circular** capture starts (E138). Lower,
/// because a run that stops on a fault at 30% may not reach 80 000 accepts:
/// the ring then keeps the last `CAPTURE_LEN` decisions before the stop.
pub const CAPTURE_AFTER_ACCEPTS_CIRCULAR: u32 = 15_000;

/// What the decision made of one edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    #[default]
    Accepted = 0,
    TooEarly = 1,
    Unstable = 2,
    /// `count` beyond the estimate's band: the ISR re-bases, no offer.
    Rebase = 3,
}

/// One COMP decision, inputs and outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decision {
    /// µs since the last accepted crossing (the ISR's own 16-bit count).
    pub count: u16,
    /// The live comparator reads, oldest in bit 0; the number taken in bits
    /// 12..16.
    pub reads: u16,
    /// For an acceptance: the commutation wait and the new estimate, µs.
    pub wait: u16,
    pub average: u16,
    pub kind: Kind,
    pub rising: bool,
    pub advance: u8,
}

impl Decision {
    /// Record an offer's inputs and outcome.
    #[inline]
    #[must_use]
    pub fn of(count: u32, rising: bool, advance: u32, reads: u16, n: u16, outcome: &Outcome) -> Self {
        let (kind, wait, average) = match *outcome {
            Outcome::Accepted {
                wait, average_interval, ..
            } => (Kind::Accepted, wait, average_interval),
            Outcome::TooEarly => (Kind::TooEarly, 0, 0),
            Outcome::Unstable => (Kind::Unstable, 0, 0),
        };
        Self {
            count: count as u16,
            reads: (reads & 0x0FFF) | (n << 12),
            wait: wait as u16,
            average: average as u16,
            kind,
            rising,
            advance: advance as u8,
        }
    }

    /// A re-base (no offer was made).
    #[inline]
    #[must_use]
    pub const fn rebase(count: u32) -> Self {
        Self {
            count: count as u16,
            reads: 0,
            wait: 0,
            average: 0,
            kind: Kind::Rebase,
            rising: false,
            advance: 0,
        }
    }

    /// The number of live reads the decision took.
    #[inline]
    #[must_use]
    pub const fn reads_taken(&self) -> u16 {
        self.reads >> 12
    }
}

/// Where the COMP root's decisions go.
pub trait EdgeLog {
    /// `false` folds every recording branch away.
    const ON: bool;
    /// Is this decision to be recorded? Arms the capture when due.
    fn arm(at: &mut Root<CompPrio>) -> bool;
    fn push(at: &mut Root<CompPrio>, d: Decision);
}

/// Production: nothing is recorded.
pub struct NoLog;

impl EdgeLog for NoLog {
    const ON: bool = false;
    #[inline(always)]
    fn arm(_at: &mut Root<CompPrio>) -> bool {
        false
    }
    #[inline(always)]
    fn push(_at: &mut Root<CompPrio>, _d: Decision) {}
}

/// The next circular index. Written out rather than `% CAPTURE_LEN`, which
/// is a helper division on this M0+ (E154).
#[inline]
const fn bump(i: usize) -> usize {
    if i + 1 == CAPTURE_LEN {
        0
    } else {
        i + 1
    }
}

/// The capture's storage: the estimator's state at the first recorded
/// decision, and the decisions.
pub struct Capture {
    /// Armed by the foreground for one run ([`Ring::arm_next_run`]).
    pub enabled: bool,
    pub started: bool,
    /// Keep the **last** `CAPTURE_LEN` decisions instead of the first (E138).
    /// The first-N window shows the loop at target; a run that dies needs the
    /// decisions that led to the stop, and those are the last ones.
    pub circular: bool,
    /// Decisions offered since the start point, which exceeds `len` once the
    /// circular ring has wrapped.
    pub total: u32,
    /// Where the next circular write lands. Carried rather than derived:
    /// `total % CAPTURE_LEN` is a helper division, and `CAPTURE_LEN` is not a
    /// power of two, so on this M0+ the modulo put `__aeabi_uidivmod` under
    /// the COMP root -- which the link-time math audit refuses (E154).
    pub next: usize,
    pub snapshot: [u32; 5],
    pub len: usize,
    pub entries: [Decision; CAPTURE_LEN],
}

static RING: Seam<Capture, CompPrio> = Seam::new(Capture {
    enabled: false,
    started: false,
    circular: false,
    total: 0,
    next: 0,
    snapshot: [0; 5],
    len: 0,
    entries: [Decision {
        count: 0,
        reads: 0,
        wait: 0,
        average: 0,
        kind: Kind::Accepted,
        rising: false,
        advance: 0,
    }; CAPTURE_LEN],
});

/// The diagnostic image's recorder.
pub struct Ring;

impl EdgeLog for Ring {
    const ON: bool = true;

    #[inline]
    fn arm(at: &mut Root<CompPrio>) -> bool {
        // Disarmed or full: one flag test. Recording: one more.
        let waiting = RING.root(at, |r| {
            match (r.enabled && (r.circular || r.len < CAPTURE_LEN), r.started) {
                (false, _) => None,
                (true, true) => Some(false),
                (true, false) => Some(true),
            }
        });
        match waiting {
            None => false,
            Some(false) => true,
            // Waiting for the start point: the estimator's count decides.
            Some(true) => {
                let state = crate::shared::SHARED
                    .det()
                    .zc
                    .root(at, |z| z.as_ref().map(|z| (z.counts().0, z.state())));
                match state {
                    Some((accepted, snap))
                        if accepted
                            >= if RING.root(at, |r| r.circular) {
                                CAPTURE_AFTER_ACCEPTS_CIRCULAR
                            } else {
                                CAPTURE_AFTER_ACCEPTS
                            } =>
                    {
                        RING.root(at, |r| {
                            r.started = true;
                            r.snapshot = snap;
                            true
                        })
                    }
                    _ => false,
                }
            }
        }
    }

    #[inline]
    fn push(at: &mut Root<CompPrio>, d: Decision) {
        RING.root(at, |r| {
            if r.circular {
                let i = r.next;
                r.entries[i] = d;
                r.next = bump(i);
                r.total = r.total.wrapping_add(1);
                if r.len < CAPTURE_LEN {
                    r.len += 1;
                }
            } else if let Some(e) = r.entries.get_mut(r.len) {
                *e = d;
                r.len += 1;
                r.total = r.total.wrapping_add(1);
            }
        });
    }
}

impl Ring {
    /// Arm the capture for the next run: empty, waiting for its start point.
    #[inline]
    pub fn arm_next_run() {
        Self::arm(false);
    }

    /// Arm keeping the last `CAPTURE_LEN` decisions (E138): what a run that
    /// stops on a fault needs.
    #[inline]
    pub fn arm_next_run_circular() {
        Self::arm(true);
    }

    #[inline]
    fn arm(circular: bool) {
        let _ = RING.lock(|r| {
            r.enabled = true;
            r.started = false;
            r.circular = circular;
            r.total = 0;
            r.next = 0;
            r.len = 0;
        });
    }

    /// After the run: the snapshot and the recorded decisions, if the capture
    /// started, via `f` (foreground, interrupts masked for its duration).
    #[inline]
    pub fn read<R>(f: impl FnOnce(&Capture) -> R) -> Option<R> {
        RING.lock(|r| f(r))
    }
}

/// A capture's text form, one decision per line, parsed back by
/// `run::replay` (host) from the file `scripts/capture_edges.py` saves.
pub fn emit(c: &Capture, out: &mut impl crate::report::Sink) {
    out.say("CAPSNAP ");
    out.kv("avg", c.snapshot[0]);
    out.kv("last", c.snapshot[1]);
    out.kv("min", c.snapshot[2]);
    out.kv("max", c.snapshot[3]);
    out.kv("blank", c.snapshot[4]);
    out.kv("after", CAPTURE_AFTER_ACCEPTS);
    out.kv("len", c.len as u32);
    out.kv("circular", u32::from(c.circular));
    out.kv("total", c.total);
    out.say("\r\n");
    out.flush();
    let start = if c.circular && c.total as usize > c.len {
        c.total as usize % CAPTURE_LEN
    } else {
        0
    };
    for k in 0..c.len {
        let d = &c.entries[(start + k) % CAPTURE_LEN];
        out.say("CAP ");
        out.say_u32(u32::from(d.count));
        out.say(" ");
        out.say_u32(u32::from(d.reads));
        out.say(" ");
        out.say_u32(u32::from(d.wait));
        out.say(" ");
        out.say_u32(u32::from(d.average));
        out.say(" ");
        out.say_u32(d.kind as u32);
        out.say(" ");
        out.say_u32(u32::from(d.rising));
        out.say(" ");
        out.say_u32(u32::from(d.advance));
        out.say("\r\n");
        out.flush();
    }
    out.say("CAPEND\r\n");
    out.flush();
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_circular_index_wraps_at_the_end_and_nowhere_else() {
        assert_eq!(super::bump(0), 1);
        assert_eq!(super::bump(super::CAPTURE_LEN - 2), super::CAPTURE_LEN - 1);
        assert_eq!(super::bump(super::CAPTURE_LEN - 1), 0);
    }
}
