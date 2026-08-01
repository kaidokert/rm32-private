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
    /// 'D' — toggle complementary (damped) PWM drive live (bench
    /// diagnostic: splits drive-mode physics from spin-up dynamics).
    DriveToggle,
    /// 'E' — toggle the atomic commutation writer live (bench
    /// diagnostic: sequential per-pin vs one-BSRR/one-MODER per port).
    AtomicToggle,
    /// 'A' — pause/resume ADC conversions live (bench diagnostic:
    /// mux-kickback A/B; measurements freeze while paused).
    AdcToggle,
    /// 'V' — arm the DWT write-watchpoint on the drive override
    /// (bench diagnostic; only effective after a clean power-up).
    WatchArm,
    /// 'J' — arm hardware-timed injected current sampling (one-way;
    /// capture sessions only — the 24 kHz injected preemption degrades
    /// the control scan).
    InjToggle,
    /// 'g' — GECKO one-shot current-microscope capture + dump.
    GeckoGrab,
    /// 'x' — WAXWING phase-voltage ring dump (post-mortem or live).
    WaxDump,
    /// 'H' — ISR-duration histogram dump (then reset). Second H after a
    /// settled hold gives the clean in-hold distribution.
    HistDump,
    /// 'B' — dump the onboard flight recorder (0.5s samples of
    /// ci/current/vbat; poll-law-safe chart data, read post-run).
    RecorderDump,
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
            b'D' => Some(UartCmd::DriveToggle),
            b'E' => Some(UartCmd::AtomicToggle),
            b'A' => Some(UartCmd::AdcToggle),
            b'V' => Some(UartCmd::WatchArm),
            b'J' => Some(UartCmd::InjToggle),
            b'g' => Some(UartCmd::GeckoGrab),
            b'x' => Some(UartCmd::WaxDump),
            b'H' => Some(UartCmd::HistDump),
            b'B' => Some(UartCmd::RecorderDump),
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

// ===============================================================
// ZC_TRACE — 15-byte per-commutation records + batch decimation.
// Ported from minz/core/src/{am32.rs,zct_trace.rs}; wire format is
// byte-identical so zctrace_capture.py / the plot suite work unchanged.
// ===============================================================

/// The ZC_TRACE wire record size (5B A9 sync | flags/step | 6×u16 LE).
pub const ZCT_REC: usize = 15;
/// Batch length in commutations (50-on / 50-off above the wire budget).
pub const ZCT_BATCH_LEN: u32 = 50;
/// Batching engages below this commutation interval (0.5 µs ticks).
pub const ZCT_BATCH_CI_TICKS: u32 = 200;

/// Batch-decimation gate — evaluated per commutation with the
/// pre-increment counter `n`. At each 50-commutation boundary the mode
/// is re-chosen (batch below [`ZCT_BATCH_CI_TICKS`]); between
/// boundaries it holds. Returns `(record, batching)`: `record` is
/// false for the skipped (odd) half of a batch cycle.
#[inline]
pub fn zct_batch_gate(n: u32, ci_ticks: u32, prev_batching: bool) -> (bool, bool) {
    let batching = if n.is_multiple_of(ZCT_BATCH_LEN) {
        ci_ticks < ZCT_BATCH_CI_TICKS
    } else {
        prev_batching
    };
    let record = !(batching && (n / ZCT_BATCH_LEN) % 2 == 1);
    (record, batching)
}

/// One canonical 15-byte ZC_TRACE row: `5B A9` sync, a flags/step byte
/// (step in bits0-2, bit7=old_routine, bit6=batched-mode), then
/// little-endian thiszc / ci / wait / duty / tenkhz / avg.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn zct_pack(
    step: u8,
    old: bool,
    batching: bool,
    thiszc: u16,
    ci: u16,
    wait: u16,
    duty: u16,
    tenkhz: u16,
    avg: u16,
) -> [u8; ZCT_REC] {
    [
        0x5B,
        0xA9,
        (step & 0x07) | if old { 0x80 } else { 0 } | if batching { 0x40 } else { 0 },
        thiszc as u8,
        (thiszc >> 8) as u8,
        ci as u8,
        (ci >> 8) as u8,
        wait as u8,
        (wait >> 8) as u8,
        duty as u8,
        (duty >> 8) as u8,
        tenkhz as u8,
        (tenkhz >> 8) as u8,
        avg as u8,
        (avg >> 8) as u8,
    ]
}

/// One 15-byte edge-probe row: `5B A6` sync, the same flags/step byte as
/// [`zct_pack`], then LE first_edge / comp_entries / tim16_lat / avg
/// (u16 each) + gated_clears / persist_rejects (u8 each). Emitted per
/// commutation alongside the `5B A9` row; carries WHAT THE ACCEPTANCE
/// MACHINERY SAW during the window that ended at this commutation:
///   first_edge — interval count at the first COMP ISR entry (0xFFFF =
///     no edge seen), comp_entries — total COMP ISR entries (camp
///     re-fires make this the storm meter), tim16_lat — interval count
///     at TIM16 ISR entry (vs scheduled wait+1 = commutation latency),
///   gated_clears — pre-ZC edges swallowed while the gate was closed,
///   persist_rejects — gate-open entries the persistence filter refused.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn zct_probe_pack(
    step: u8,
    old: bool,
    batching: bool,
    first_edge: u16,
    comp_entries: u16,
    tim16_lat: u16,
    avg: u16,
    gated_clears: u8,
    persist_rejects: u8,
    last_arm: u16,
) -> [u8; ZCT_REC] {
    [
        0x5B,
        0xA6,
        (step & 0x07) | if old { 0x80 } else { 0 } | if batching { 0x40 } else { 0 },
        first_edge as u8,
        (first_edge >> 8) as u8,
        comp_entries as u8,
        (comp_entries >> 8) as u8,
        tim16_lat as u8,
        (tim16_lat >> 8) as u8,
        avg as u8,
        (avg >> 8) as u8,
        gated_clears,
        persist_rejects,
        last_arm as u8,
        (last_arm >> 8) as u8,
    ]
}

/// ZC_TRACE record ring over borrowed atomics. `push_rec` is NOT
/// self-synchronizing — if more than one ISR priority produces, the
/// caller wraps it in a critical section (this crate stays free of
/// cortex-m). A full ring drops the OLDEST (advances tail) and counts
/// it; the single consumer drains via a byte sink.
pub struct ZctRing<'a, const N: usize> {
    pub ring: &'a [[AtomicU16; ZCT_REC]; N],
    pub head: &'a AtomicUsize,
    pub tail: &'a AtomicUsize,
    pub drop: &'a core::sync::atomic::AtomicU32,
}

impl<const N: usize> ZctRing<'_, N> {
    /// Enqueue one packed record; full ring drops the OLDEST.
    #[inline]
    pub fn push_rec(&self, rec: &[u8; ZCT_REC]) {
        let h = self.head.load(Ordering::Relaxed);
        let nx = (h + 1) % N;
        if nx == self.tail.load(Ordering::Relaxed) {
            // load+store, not fetch_add: producers are caller-serialized
            // (critical section), and M0 targets have no atomic RMW.
            self.drop.store(
                self.drop.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
            self.tail.store(
                (self.tail.load(Ordering::Relaxed) + 1) % N,
                Ordering::Relaxed,
            );
        }
        for (i, b) in rec.iter().enumerate() {
            self.ring[h][i].store(*b as u16, Ordering::Relaxed);
        }
        self.head.store(nx, Ordering::Relaxed);
    }

    /// Drain up to `max_rec` records into `sink`, byte by byte.
    #[inline]
    pub fn drain(&self, max_rec: usize, mut sink: impl FnMut(u8)) {
        let mut nrec = 0;
        while self.tail.load(Ordering::Relaxed) != self.head.load(Ordering::Relaxed)
            && nrec < max_rec
        {
            let t = self.tail.load(Ordering::Relaxed);
            for i in 0..ZCT_REC {
                sink(self.ring[t][i].load(Ordering::Relaxed) as u8);
            }
            self.tail.store((t + 1) % N, Ordering::Relaxed);
            nrec += 1;
        }
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
    fn zct_pack_layout_is_wire_exact() {
        let r = zct_pack(
            5, true, false, 0x1234, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B,
        );
        assert_eq!((r[0], r[1]), (0x5B, 0xA9));
        assert_eq!(r[2], 5 | 0x80, "step bits + old flag");
        assert_eq!((r[3], r[4]), (0x34, 0x12), "thiszc LE");
        assert_eq!((r[13], r[14]), (0x0B, 0x0A), "avg LE");
        let b = zct_pack(2, false, true, 0, 0, 0, 0, 0, 0);
        assert_eq!(b[2], 2 | 0x40, "batching flag = bit6");
    }

    #[test]
    fn zct_probe_pack_layout_is_wire_exact() {
        let r = zct_probe_pack(
            3, false, false, 0x1234, 0x0203, 0x0405, 0x0607, 0xAB, 0xCD, 0x0E0F,
        );
        assert_eq!((r[0], r[1]), (0x5B, 0xA6), "probe sync marker");
        assert_eq!(r[2], 3, "flags/step byte matches zct_pack layout");
        assert_eq!((r[3], r[4]), (0x34, 0x12), "first_edge LE");
        assert_eq!((r[5], r[6]), (0x03, 0x02), "comp_entries LE");
        assert_eq!((r[7], r[8]), (0x05, 0x04), "tim16_lat LE");
        assert_eq!((r[9], r[10]), (0x07, 0x06), "avg LE");
        assert_eq!(
            (r[11], r[12]),
            (0xAB, 0xCD),
            "gated_clears / persist_rejects"
        );
        assert_eq!((r[13], r[14]), (0x0F, 0x0E), "last_arm LE");
    }

    #[test]
    fn zct_gate_batches_below_threshold_in_alternating_50s() {
        // Fast motor (ci < threshold): first 50 recorded, next 50 skipped.
        let mut batching = false;
        let mut recorded = 0;
        for n in 0..200u32 {
            let (rec, b) = zct_batch_gate(n, 100, batching);
            batching = b;
            if rec {
                recorded += 1;
            }
        }
        assert_eq!(recorded, 100, "50-on/50-off over 200 commutations");
        // Slow motor: everything recorded, no batching.
        let (rec, b) = zct_batch_gate(0, 5000, true);
        assert!(rec && !b, "mode re-chosen at boundary");
    }

    #[test]
    fn zct_ring_drops_oldest_and_drains_capped() {
        use core::sync::atomic::AtomicU32;
        static RING: [[AtomicU16; ZCT_REC]; 4] =
            [const { [const { AtomicU16::new(0) }; ZCT_REC] }; 4];
        static HEAD: AtomicUsize = AtomicUsize::new(0);
        static TAIL: AtomicUsize = AtomicUsize::new(0);
        static DROP: AtomicU32 = AtomicU32::new(0);
        let ring = ZctRing {
            ring: &RING,
            head: &HEAD,
            tail: &TAIL,
            drop: &DROP,
        };
        let mut rec = [0u8; ZCT_REC];
        for v in 0..5u8 {
            rec[2] = v;
            ring.push_rec(&rec); // capacity N-1=3: v=0,1 dropped (oldest)
        }
        assert_eq!(DROP.load(Ordering::Relaxed), 2);
        let mut seen = [0u8; 8];
        let mut n = 0;
        ring.drain(3, |b| {
            if n % ZCT_REC == 2 {
                seen[n / ZCT_REC] = b;
            }
            n += 1;
        });
        assert_eq!(n, 3 * ZCT_REC);
        assert_eq!(&seen[..3], &[2, 3, 4], "oldest-dropped, order kept");
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
