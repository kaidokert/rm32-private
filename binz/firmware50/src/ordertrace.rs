//! Compact event-order diagnostic. Not a physical-edge capture or lean timing proof.
//! Each IRQ owns its ring, so COM may preempt COMP without sharing a mutable row.
//! Accepted timestamps are TIM17 service-entry microseconds. Bridge timestamps
//! bracket a TIM17 read with two genuine TIM2 reads; a wide bracket is uncertainty,
//! not precise phase. Stored only after arming / bridge application respectively.

use crate::chain::ChainLog;
use crate::report::Sink;
use crate::sagtrace::{Block, SagLog, SagRing};
use crate::shared::{CompPrio, Motor, Root, Seam};
use portable_atomic::{AtomicBool, Ordering::Relaxed};

pub const LEN: usize = 128;
const _: () = assert!(LEN.is_power_of_two());

#[derive(Clone, Copy, Default)]
pub struct Accepted {
    pub id: u32,
    pub raw: u16,
    pub interval: u16,
    pub average: u16,
    pub wait: u16,
    pub step: u8,
}

#[derive(Clone, Copy, Default)]
pub struct Stamp {
    pub before: u32,
    pub after: u32,
    pub coarse: u16,
}

#[derive(Clone, Copy, Default)]
pub struct Commutated {
    pub id: u32,
    pub stamp: Stamp,
    pub service: u16,
    pub scheduled: u16,
    pub step: u8,
}

struct Ring<T: Copy> {
    total: u32,
    rows: [T; LEN],
}
impl<T: Copy> Ring<T> {
    const fn new(empty: T) -> Self {
        Self {
            total: 0,
            rows: [empty; LEN],
        }
    }
    #[inline(always)]
    fn push(&mut self, row: T) {
        self.rows[(self.total as usize) & (LEN - 1)] = row;
        self.total = self.total.wrapping_add(1);
    }
    fn len(&self) -> usize {
        (self.total as usize).min(LEN)
    }
    fn row(&self, offset: usize) -> Option<T> {
        (offset < self.len())
            .then(|| self.rows[(self.total.wrapping_sub(self.len() as u32) as usize + offset) & (LEN - 1)])
    }
}
const _: () = assert!(core::mem::size_of::<Ring<Accepted>>() + core::mem::size_of::<Ring<Commutated>>() <= 6144);
static ON: AtomicBool = AtomicBool::new(false);
static ACC: Seam<Ring<Accepted>, CompPrio> = Seam::new(Ring::new(Accepted {
    id: 0,
    raw: 0,
    interval: 0,
    average: 0,
    wait: 0,
    step: 0,
}));
static COM: Seam<Ring<Commutated>, Motor> = Seam::new(Ring::new(Commutated {
    id: 0,
    stamp: Stamp {
        before: 0,
        after: 0,
        coarse: 0,
    },
    service: 0,
    scheduled: 0,
    step: 0,
}));

pub struct OrderRing;
impl ChainLog for OrderRing {
    // Do not turn on the larger recorder's per-dispatch clocks/counters.
    const ON: bool = false;
    const ORDER: bool = true;
    #[inline(always)]
    fn accept(_: &mut Root<CompPrio>, _: &crate::chain::Arm) {}
    #[inline(always)]
    fn service(_: &mut Root<Motor>, _: u16, _: u16, _: u16, _: u32, _: u32, _: u8) {}
    #[inline(always)]
    fn order_accept(at: &mut Root<CompPrio>, row: Accepted) {
        if ON.load(Relaxed) {
            ACC.root(at, |r| r.push(row));
        }
    }
    #[inline(always)]
    fn order_bridge(at: &mut Root<Motor>, row: Commutated) {
        if ON.load(Relaxed) {
            COM.root(at, |r| r.push(row));
        }
    }
}

impl OrderRing {
    pub fn arm_next_run() {
        ON.store(false, Relaxed);
        let _ = ACC.lock(|r| r.total = 0);
        let _ = COM.lock(|r| r.total = 0);
        ON.store(true, Relaxed);
    }
    pub fn disarm() {
        ON.store(false, Relaxed);
    }
    /// OFF only. Copy one row under each short lock, never mask across UART.
    pub fn emit(out: &mut impl Sink) {
        let (Some((an, at)), Some((bn, bt))) = (ACC.lock(|r| (r.len(), r.total)), COM.lock(|r| (r.len(), r.total)))
        else {
            return;
        };
        out.say("ORDERSNAP ");
        for (name, value) in [
            ("row_v", 1),
            ("fine_hz", crate::fine::FINE_HZ),
            ("acc_len", an as u32),
            ("acc_total", at),
            ("com_len", bn as u32),
            ("com_total", bt),
            ("frozen", u32::from(!ON.load(Relaxed))),
        ] {
            out.kv(name, value);
        }
        out.say("\r\n");
        out.flush();
        for i in 0..an {
            if let Some(Some(r)) = ACC.lock(|r| r.row(i)) {
                line(
                    out,
                    "ORDERA",
                    &[
                        r.id,
                        r.raw.into(),
                        r.interval.into(),
                        r.average.into(),
                        r.wait.into(),
                        r.step.into(),
                    ],
                );
            }
        }
        for i in 0..bn {
            if let Some(Some(r)) = COM.lock(|r| r.row(i)) {
                line(
                    out,
                    "ORDERB",
                    &[
                        r.id,
                        r.service.into(),
                        r.scheduled.into(),
                        r.stamp.coarse.into(),
                        r.stamp.before,
                        r.stamp.after,
                        r.step.into(),
                    ],
                );
            }
        }
        out.say("ORDEREND\r\n");
        out.flush();
    }
}

fn line(out: &mut impl Sink, prefix: &str, values: &[u32]) {
    out.say(prefix);
    for v in values {
        out.say(" ");
        out.say_u32(*v);
    }
    out.say("\r\n");
    out.flush();
}

/// Reuse unchanged sag inputs and freeze event tails at the same stop decision.
pub struct OrderSag;
impl SagLog for OrderSag {
    const ON: bool = true;
    fn block(b: &Block) {
        SagRing::block(b);
    }
    fn freeze() {
        OrderRing::disarm();
        SagRing::freeze();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_accepted_tails_replay_production_estimate_and_wait() {
        use crate::bemf::{FixedFilter, Outcome, ZeroCross};
        for fixture in [
            include_str!("../scripts/testdata/order_e382.txt"),
            include_str!("../scripts/testdata/order_e383.txt"),
        ] {
            let seed: u32 = fixture
                .lines()
                .find_map(|s| s.strip_prefix("# seed_us="))
                .unwrap()
                .parse()
                .unwrap();
            let rows: std::vec::Vec<std::vec::Vec<u32>> = fixture
                .lines()
                .filter(|s| !s.starts_with('#') && !s.is_empty())
                .map(|s| s.split_whitespace().map(|v| v.parse().unwrap()).collect())
                .collect();
            assert_eq!(rows.len(), 128);
            let first = &rows[0];
            let mut zc = ZeroCross::from_state([
                first[3],
                first[2],
                crate::run::policy::SECTOR_FLOOR_US,
                seed + seed / 2,
                32,
            ]);
            for pair in rows.windows(2) {
                let (before, row) = (&pair[0], &pair[1]);
                assert_eq!(row[0].wrapping_sub(before[0]), 1);
                assert_eq!((row[1] as u16).wrapping_sub(before[1] as u16) as u32, row[2]);
                assert_eq!(row[5], before[5] % 6 + 1);
                // Accepted-only records cannot replay refusals or verify the
                // physical crossing. All reads agree here by construction.
                let accepted = zc.offer(row[2], true, 16, &FixedFilter::<1>, || true);
                match accepted {
                    Outcome::Accepted {
                        wait, average_interval, ..
                    } => {
                        assert_eq!((average_interval, wait), (row[3], row[4]), "ordinal {}", row[0]);
                    }
                    other => panic!("recorded acceptance {} refused: {other:?}", row[0]),
                }
            }
        }
    }
    #[test]
    fn freeze_is_idempotent_and_disables_both_writers_gate() {
        ON.store(true, Relaxed);
        OrderRing::disarm();
        assert!(!ON.load(Relaxed));
        OrderRing::disarm();
        assert!(!ON.load(Relaxed));
    }
    #[test]
    fn circular_tail_is_bounded_and_chronological() {
        let mut r = Ring::new(0u32);
        assert_eq!(r.row(0), None);
        for i in 0..LEN as u32 + 9 {
            r.push(i);
        }
        assert_eq!(r.len(), LEN);
        assert_eq!(r.row(0), Some(9));
        assert_eq!(r.row(LEN - 1), Some(LEN as u32 + 8));
        assert_eq!(r.row(LEN), None);
    }
    #[test]
    fn ordinal_is_not_ring_position_and_stamp_wraps_are_explicit() {
        let mut r = Ring::new(Accepted::default());
        r.push(Accepted {
            id: u32::MAX,
            raw: 65530,
            ..Accepted::default()
        });
        r.push(Accepted {
            id: 0,
            raw: 20,
            ..Accepted::default()
        });
        let (a, b) = (r.row(0).unwrap(), r.row(1).unwrap());
        assert_eq!(b.id.wrapping_sub(a.id), 1);
        assert_eq!(b.raw.wrapping_sub(a.raw), 26);
        let s = Stamp {
            before: u32::MAX - 3,
            after: 4,
            coarse: 0,
        };
        assert_eq!(s.after.wrapping_sub(s.before), 8);
    }
}
