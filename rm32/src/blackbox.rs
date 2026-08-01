//! The 64-event black-box ring: fixed storage, wrapping writer,
//! freeze-on-fault, chronological replay.
//!
//! Ported from `minz/core/src/blackbox.rs`, where this ring caught the
//! watchdog underflow race, the estimator walk-ups, and the
//! re-acquisition anatomy. Portable + host-tested here; the firmware
//! adapter (timestamps, critical section, dump-over-UART) lives in
//! `rm32_stm32::bench_bb` behind the `blackbox` feature. The event-code
//! table is the authority shared with the bench scripts
//! (`minz/scripts/bb_postmortem.py`) — codes 0..=12 keep minz's meaning,
//! 13+ are rm32 additions.

pub const BB_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// 10 µs tick truncated to u16 (dt between events is what
    /// matters; the dump renders deltas).
    pub t: u16,
    /// Event type index into [`EV_NAMES`]. 0xFF = empty slot.
    pub ty: u8,
    pub sector: u8,
    pub data: u16,
}

pub const EMPTY: Event = Event {
    t: 0,
    ty: 0xFF,
    sector: 0,
    data: 0,
};

#[derive(Debug, Clone)]
pub struct BlackBox {
    ring: [Event; BB_LEN],
    idx: usize,
    len: usize,
    pub frozen: bool,
}

impl BlackBox {
    pub const fn new() -> Self {
        Self {
            ring: [EMPTY; BB_LEN],
            idx: 0,
            len: 0,
            frozen: false,
        }
    }

    /// Record unless frozen (a fault freezes the ring so the dump
    /// shows the events LEADING TO the kill, not the aftermath).
    pub fn record(&mut self, e: Event) {
        if self.frozen {
            return;
        }
        self.ring[self.idx] = e;
        self.idx = (self.idx + 1) % BB_LEN;
        self.len = (self.len + 1).min(BB_LEN);
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn thaw(&mut self) {
        self.frozen = false;
    }

    /// Chronological iterator, oldest first.
    pub fn replay(&self) -> impl Iterator<Item = &Event> {
        let start = if self.len < BB_LEN { 0 } else { self.idx };
        (0..self.len).map(move |k| &self.ring[(start + k) % BB_LEN])
    }
}

impl Default for BlackBox {
    fn default() -> Self {
        Self::new()
    }
}

/// Event-type display names, indexed by `Event::ty`. 0..=12 are the minz
/// codes (kept so `bb_postmortem.py` decodes both firmwares); 13+ are
/// rm32-specific.
pub const EV_NAMES: [&str; 15] = [
    "REF", "BLD", "DRK", "ACC", "NOZ", "DIS", "DSY", "ENG", "STV", "RAQ", "RSD", "RSC", "KCK",
    "MOD", "KIL",
];

/// Commutation step executed (rm32: every `commutation_timer_expired`).
pub const EV_REF: u8 = 0;
/// COMP ZC ISR fired (rm32: COMP entry; acceptance not yet distinguished).
pub const EV_ACC: u8 = 3;
/// Desync detected.
pub const EV_DSY: u8 = 6;
/// Motor mode changed. data = armed | running<<1 | old_routine<<2 |
/// stepper_sine<<3 packed bits of the NEW mode.
pub const EV_MOD: u8 = 13;
/// Bench-guard safety kill (freezes the ring). data = reason
/// (1=overcurrent, 2=vbat-sag).
pub const EV_KIL: u8 = 14;

/// Render a dump: one `bb +<dt>us NAME s<sec> d=<data>` line per event,
/// dt in µs since the previous event (t is in 10 µs ticks, wrapping
/// u16). `events` arrive oldest-first. Empty slots (ty == 0xFF) skipped.
pub fn format_dump<S: FnMut(&[u8])>(events: impl Iterator<Item = Event>, mut sink: S) {
    use core::fmt::Write;
    struct SinkFmt<'a, S: FnMut(&[u8])> {
        sink: &'a mut S,
    }
    impl<S: FnMut(&[u8])> Write for SinkFmt<'_, S> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            (self.sink)(s.as_bytes());
            Ok(())
        }
    }
    let mut prev_t: Option<u16> = None;
    for e in events {
        if e.ty == 0xFF {
            continue;
        }
        let dt = prev_t.map(|p| e.t.wrapping_sub(p) as u32 * 10).unwrap_or(0);
        prev_t = Some(e.t);
        let _ = write!(
            SinkFmt { sink: &mut sink },
            "bb +{:6}us {} s{} d={}\r\n",
            dt,
            EV_NAMES.get(e.ty as usize).copied().unwrap_or("???"),
            e.sector,
            e.data,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(n: u16) -> Event {
        Event {
            t: n,
            ty: (n % 10) as u8,
            sector: (n % 6) as u8,
            data: n,
        }
    }

    #[test]
    fn partial_fill_replays_in_order() {
        let mut bb = BlackBox::new();
        for n in 0..10 {
            bb.record(ev(n));
        }
        let mut expect = 0u16;
        for e in bb.replay() {
            assert_eq!(e.t, expect);
            expect += 1;
        }
        assert_eq!(expect, 10);
    }

    #[test]
    fn wraparound_keeps_newest_64_in_order() {
        let mut bb = BlackBox::new();
        for n in 0..100 {
            bb.record(ev(n));
        }
        let mut expect = 100 - BB_LEN as u16;
        let mut count = 0;
        for e in bb.replay() {
            assert_eq!(e.t, expect);
            expect += 1;
            count += 1;
        }
        assert_eq!(count, BB_LEN);
    }

    #[test]
    fn dump_renders_deltas_and_names() {
        let events = [
            Event {
                t: 100,
                ty: 7,
                sector: 2,
                data: 650,
            }, // ENG
            Event {
                t: 165,
                ty: 3,
                sector: 3,
                data: 12,
            }, // ACC, +650 µs
            EMPTY, // skipped
            Event {
                t: 5,
                ty: 14,
                sector: 4,
                data: 1,
            }, // KIL, wraps
        ];
        let mut out = [0u8; 256];
        let mut n = 0;
        format_dump(events.into_iter(), |b: &[u8]| {
            out[n..n + b.len()].copy_from_slice(b);
            n += b.len();
        });
        let s = core::str::from_utf8(&out[..n]).unwrap();
        let mut lines = s.split("\r\n").filter(|l| !l.is_empty());
        let l0 = lines.next().unwrap();
        assert!(l0.contains("ENG s2 d=650"), "{l0}");
        assert!(l0.contains("+     0us"), "{l0}");
        let l1 = lines.next().unwrap();
        assert!(l1.contains("+   650us ACC"), "{l1}");
        // 5 - 165 wraps in u16: (5 - 165) mod 65536 = 65376 → ×10 µs.
        let l2 = lines.next().unwrap();
        assert!(l2.contains("+653760us KIL s4 d=1"), "{l2}");
        assert!(lines.next().is_none());
    }

    #[test]
    fn freeze_preserves_the_leadup() {
        let mut bb = BlackBox::new();
        for n in 0..70 {
            bb.record(ev(n));
        }
        bb.freeze();
        for n in 70..200 {
            bb.record(ev(n)); // post-fault noise must not overwrite
        }
        assert_eq!(bb.replay().last().unwrap().t, 69);
        bb.thaw();
        bb.record(ev(200));
        assert_eq!(bb.replay().last().unwrap().t, 200);
    }
}
