//! The 64-event black-box ring: fixed storage, wrapping writer,
//! freeze-on-fault, chronological replay. This ring caught the
//! watchdog underflow race, the estimator walk-ups, and the
//! re-acquisition anatomy — it earns unit tests of its own.

pub const BB_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Event {
    /// 10 µs tick truncated to u16 (dt between events is what
    /// matters; the dump renders deltas).
    pub t: u16,
    /// Event type index (REF/BLD/DRK/ACC/NOZ/DIS/DSY/ENG/STV/RAQ).
    pub ty: u8,
    pub sector: u8,
    pub data: u16,
}

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
            ring: [Event {
                t: 0,
                ty: 0xFF,
                sector: 0,
                data: 0,
            }; BB_LEN],
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

/// Event-type display names, indexed by `Event::ty`.
pub const EV_NAMES: [&str; 12] = [
    "REF", "BLD", "DRK", "ACC", "NOZ", "DIS", "DSY", "ENG", "STV", "RAQ", "RSD", "RSC",
];

// The authoritative event codes (indices into EV_NAMES; the decode
// table in scripts/owl_report.py must match). Call sites use these,
// never magic numbers.
/// Commutation re-timed by an accepted ZC.
pub const EV_REF: u8 = 0;
/// A/B window commutated by the blind free-run.
pub const EV_BLD: u8 = 1;
/// Dead-reckoned phase-C window commutation.
pub const EV_DRK: u8 = 2;
/// Qualified ZC accepted.
pub const EV_ACC: u8 = 3;
/// Window closed with no qualified ZC (CL).
pub const EV_NOZ: u8 = 4;
/// Candidate discarded by the confirm rule.
pub const EV_DIS: u8 = 5;
/// Desync watchdog kill.
pub const EV_DSY: u8 = 6;
/// Closed loop engaged.
pub const EV_ENG: u8 = 7;
/// ZC-starvation kill.
pub const EV_STV: u8 = 8;
/// Re-acquisition entered.
pub const EV_RAQ: u8 = 9;
/// R3: sync-loss handled as a duty-clamped RESEED (not a kill).
pub const EV_RSD: u8 = 10;
/// Level-rescue accept: a pre-crossed window (crossing predated the
/// mux switch - no edge ever existed) had its qZC synthesized from
/// the first-wrap level sample. Data = offset us from window start.
pub const EV_RSC: u8 = 11;

/// Render a desync dump: one `bb +<dt>us NAME s<sec> d=<data>` line
/// per event, dt in µs since the previous event (t is in 10 µs
/// ticks, wrapping u16). `events` arrive oldest-first (e.g. from
/// [`BlackBox::replay`], or the firmware's atomic-array ring).
/// Events with `ty == 0xFF` (empty slots) are skipped.
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
        let ts: Vec<u16> = bb.replay().map(|e| e.t).collect();
        assert_eq!(ts, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn wraparound_keeps_newest_64_in_order() {
        let mut bb = BlackBox::new();
        for n in 0..100 {
            bb.record(ev(n));
        }
        let ts: Vec<u16> = bb.replay().map(|e| e.t).collect();
        assert_eq!(ts.len(), BB_LEN);
        assert_eq!(ts[0], 100 - BB_LEN as u16);
        assert_eq!(*ts.last().unwrap(), 99);
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
            Event {
                t: 100,
                ty: 0xFF,
                sector: 0,
                data: 0,
            }, // empty: skipped
            Event {
                t: 5,
                ty: 6,
                sector: 4,
                data: 1,
            }, // DSY, wraps
        ];
        let mut out = Vec::new();
        format_dump(events.into_iter(), |b: &[u8]| out.extend_from_slice(b));
        let s = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = s.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("ENG s2 d=650"));
        assert!(lines[0].contains("+     0us"));
        assert!(lines[1].contains("+   650us ACC"));
        // 5 - 165 wraps in u16: (5 - 165) mod 65536 = 65376 → ×10 µs.
        assert!(lines[2].contains("+653760us DSY s4 d=1"), "{}", lines[2]);
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
