//! Bench UART throttle input — ASCII duty parser + SPSC byte ring.
//!
//! Ported from the minz am32_clone bench channel (`minz/core/src/am32.rs`,
//! itself line-cited against the AM32 fork's `uart_duty_poll`,
//! main.c:1367-1416) so the existing operator toolchain (`uart_cmd.py`,
//! `fly.py`, the sweep/lock-map scripts) drives rm32 unmodified.
//!
//! Portable + host-tested here; the firmware side (USART2 RX vector, ring
//! storage, throttle injection into `SharedState`) lives in `rm32_stm32`
//! behind the `benchuart` feature. Nothing in this module is reachable
//! from the vector harness.

use core::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

/// A command decoded from the UART duty-mode byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartCmd {
    /// Commit a throttle value in AM32 input units (48..2047).
    SetThrottle(u16),
    /// 's' — stop and zero the throttle.
    Stop,
    /// 'w' — kill: stop + immediate AllOff request (bench-script kill
    /// guards send this on every exit path).
    Kill,
    /// 'Z' — toggle the ZC_TRACE stream (stub until the zctrace rung).
    TraceToggle,
    /// 'i' — request an info line.
    Info,
    /// 'b' — request a black-box dump (stub until the blackbox rung).
    BbDump,
}

/// The pure UART duty-mode parser — a digit accumulator. `step` feeds one
/// byte and returns a command when one completes. Semantics mirror the
/// AM32 fork exactly:
/// - digits accumulate up to 4 (further digits ignored, not reset);
/// - 's'/'w' commit Stop/Kill and clear the accumulator;
/// - 'Z'/'i'/'b' emit their command WITHOUT touching the accumulator
///   (so `50Z<nl>` still commits 50);
/// - any other byte terminates: with digits pending it commits (a `0` →
///   Stop; 1..=100 → percent; 101..=1000 → permille; both map to
///   48..2047), and always clears the accumulator.
#[derive(Debug, Clone, Copy, Default)]
pub struct UartDuty {
    acc: u32,
    n: u8,
}

impl UartDuty {
    #[inline]
    pub fn new() -> Self {
        Self { acc: 0, n: 0 }
    }

    #[inline]
    pub fn step(&mut self, byte: u8) -> Option<UartCmd> {
        match byte {
            b'0'..=b'9' => {
                if self.n < 4 {
                    self.acc = self.acc * 10 + (byte - b'0') as u32;
                    self.n += 1;
                }
                None
            }
            b's' => {
                self.acc = 0;
                self.n = 0;
                Some(UartCmd::Stop)
            }
            b'w' => {
                self.acc = 0;
                self.n = 0;
                Some(UartCmd::Kill)
            }
            b'Z' => Some(UartCmd::TraceToggle),
            b'i' => Some(UartCmd::Info),
            b'b' => Some(UartCmd::BbDump),
            _ => {
                let cmd = if self.n != 0 {
                    let v = self.acc;
                    if v == 0 {
                        Some(UartCmd::Stop)
                    } else {
                        let inn = if v <= 100 {
                            v * 20 + 47 // percent
                        } else if v <= 1000 {
                            v * 2 + 47 // permille
                        } else {
                            47
                        }
                        .clamp(48, 2047);
                        Some(UartCmd::SetThrottle(inn as u16))
                    }
                } else {
                    None
                };
                self.acc = 0;
                self.n = 0;
                cmd
            }
        }
    }
}

/// SPSC byte ring over borrowed atomics: sole producer = the RX ISR via
/// `push`, sole consumer = the main loop via `pop`; a full ring drops the
/// newest byte. Storage lives with the caller. Relaxed ordering is
/// sufficient for SPSC on a single core with data-in-the-atomics.
pub struct RxRing<'a, const N: usize> {
    pub ring: &'a [AtomicU16; N],
    pub head: &'a AtomicUsize,
    pub tail: &'a AtomicUsize,
}

impl<const N: usize> RxRing<'_, N> {
    /// Producer side (ISR): enqueue one byte; full ring drops.
    #[inline]
    pub fn push(&self, c: u16) {
        let h = self.head.load(Ordering::Relaxed);
        let nx = (h + 1) % N;
        if nx != self.tail.load(Ordering::Relaxed) {
            self.ring[h].store(c, Ordering::Relaxed);
            self.head.store(nx, Ordering::Relaxed);
        }
    }

    /// Consumer side (main): dequeue one byte if available.
    #[inline]
    pub fn pop(&self) -> Option<u8> {
        let t = self.tail.load(Ordering::Relaxed);
        if t == self.head.load(Ordering::Relaxed) {
            return None;
        }
        let c = self.ring[t].load(Ordering::Relaxed) as u8;
        self.tail.store((t + 1) % N, Ordering::Relaxed);
        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed all bytes; return the command emitted by the LAST byte.
    fn feed(p: &mut UartDuty, s: &str) -> Option<UartCmd> {
        let mut last = None;
        for b in s.bytes() {
            last = p.step(b);
        }
        last
    }

    #[test]
    fn percent_maps_to_am32_units() {
        let mut p = UartDuty::new();
        assert_eq!(p.step(b'5'), None);
        assert_eq!(p.step(b'0'), None);
        // terminator commits: 50% -> 50*20+47 = 1047
        assert_eq!(p.step(b'\n'), Some(UartCmd::SetThrottle(1047)));
        // 100% -> 2047 (max)
        assert_eq!(feed(&mut p, "100\n"), Some(UartCmd::SetThrottle(2047)));
    }

    #[test]
    fn permille_maps_above_100() {
        let mut p = UartDuty::new();
        // 500 permille -> 500*2+47 = 1047
        assert_eq!(feed(&mut p, "500\n"), Some(UartCmd::SetThrottle(1047)));
    }

    #[test]
    fn zero_commit_is_stop() {
        let mut p = UartDuty::new();
        assert_eq!(feed(&mut p, "0\n"), Some(UartCmd::Stop));
    }

    #[test]
    fn stop_and_kill_keys() {
        let mut p = UartDuty::new();
        assert_eq!(p.step(b's'), Some(UartCmd::Stop));
        assert_eq!(p.step(b'w'), Some(UartCmd::Kill));
        // keys clear a pending accumulator
        assert_eq!(feed(&mut p, "42s"), Some(UartCmd::Stop));
        assert_eq!(p.step(b'\n'), None, "accumulator cleared by 's'");
    }

    #[test]
    fn info_keys_do_not_touch_accumulator() {
        let mut p = UartDuty::new();
        assert_eq!(p.step(b'5'), None);
        assert_eq!(p.step(b'0'), None);
        assert_eq!(p.step(b'i'), Some(UartCmd::Info));
        assert_eq!(p.step(b'Z'), Some(UartCmd::TraceToggle));
        assert_eq!(p.step(b'b'), Some(UartCmd::BbDump));
        // still commits the pending 50
        assert_eq!(p.step(b'\n'), Some(UartCmd::SetThrottle(1047)));
    }

    #[test]
    fn digit_limit_ignores_extras() {
        let mut p = UartDuty::new();
        // 5 digits: the 5th is ignored, not a reset -> 1000 permille
        assert_eq!(feed(&mut p, "10009\n"), Some(UartCmd::SetThrottle(2047)));
    }

    #[test]
    fn out_of_range_clamps_to_floor() {
        let mut p = UartDuty::new();
        // >1000 -> 47 -> clamped up to 48
        assert_eq!(feed(&mut p, "1001\n"), Some(UartCmd::SetThrottle(48)));
    }

    #[test]
    fn ring_spsc_order_and_drop_newest() {
        use core::sync::atomic::{AtomicU16, AtomicUsize};
        static RING: [AtomicU16; 4] = [
            AtomicU16::new(0),
            AtomicU16::new(0),
            AtomicU16::new(0),
            AtomicU16::new(0),
        ];
        static HEAD: AtomicUsize = AtomicUsize::new(0);
        static TAIL: AtomicUsize = AtomicUsize::new(0);
        let r = RxRing {
            ring: &RING,
            head: &HEAD,
            tail: &TAIL,
        };
        r.push(1);
        r.push(2);
        r.push(3);
        r.push(4); // N-1 capacity: dropped
        assert_eq!(r.pop(), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.pop(), Some(3));
        assert_eq!(r.pop(), None);
    }
}
