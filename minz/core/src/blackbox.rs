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
