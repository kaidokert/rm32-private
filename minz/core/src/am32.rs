//! AM32-verbatim control math and small state machines, extracted
//! from `minz/examples/am32_clone.rs` (a line-cited transliteration of
//! AM32 `Src/main.c` + `functions.c` for VIMDRONES_L431). Every item
//! carries the AM32 citation it was moved with; the point of the
//! extraction is host-testable parity — the exact arithmetic that runs
//! in the ISRs is unit-tested on a PC, not convicted on a burnt bench.
//!
//! Nothing here reads hardware: timestamps, comparator levels and ADC
//! samples arrive as arguments. Atomics-struct helpers ([`Am32Intervals`])
//! follow the `window::WindowState` idiom — a struct of
//! `portable_atomic` refs plus data-in/data-out methods.

// Am32Intervals wraps the firmware's `commutation_intervals[]` static,
// which is `core::sync::atomic::AtomicU32` in the example (all its
// statics are). AtomicU32 exists on both x86 (host tests) and
// thumbv7em, so no portable_atomic shim is needed for this module.
use core::sync::atomic::{AtomicU32, Ordering};

// ===============================================================
// Constants owned here (with their AM32 citations), so the pure
// helpers below don't need long parameter lists. The example
// references these where it used to keep local copies.
// ===============================================================

/// AM32's throttle/duty domain is 0..2000 (main.c throughout).
pub const DUTY_FULL: i32 = 2000;

// --- Ramp rates (main.c:1746-1754; targets.h globals 3113/3117/3121) ---
/// `zero_crosses<150 || last_duty<150` → RAMP_SPEED_STARTUP.
pub const MAX_RAMP_STARTUP: i32 = 2;
/// else `average_interval>500` → RAMP_SPEED_LOW_RPM.
pub const MAX_RAMP_LOW_RPM: i32 = 6;
/// else → RAMP_SPEED_HIGH_RPM.
pub const MAX_RAMP_HIGH_RPM: i32 = 16;

// --- Low-rpm duty ceiling map (main.c:436-439,2450) ---
pub const LOW_RPM_LEVEL: i32 = 20; // main.c:436, thousand-erpm
pub const HIGH_RPM_LEVEL: i32 = 70; // main.c:437
pub const THROTTLE_MAX_AT_LOW_RPM: i32 = 400; // main.c:438
pub const THROTTLE_MAX_AT_HIGH_RPM: i32 = 2000; // main.c:439

// --- ZC_TRACE batch decimation (fork directive 2026-07-20) ---
/// Batch length: 50 commutations on / 50 off above the wire budget.
pub const ZCT_BATCH_LEN: u32 = 50;
/// Below this commutation interval (0.5 µs ticks) the trace batches.
pub const ZCT_BATCH_CI_TICKS: u32 = 200;

// ===============================================================
// map()/getAbsDif() — functions.c.
// ===============================================================

/// AM32 `map` (functions.c:22-40): recursive binary-search
/// interpolation with end clamping. Transliterated verbatim.
#[inline]
pub fn map(x: i32, in_min: i32, in_max: i32, out_min: i32, out_max: i32) -> i32 {
    if x >= in_max {
        return out_max;
    }
    if x <= in_min {
        return out_min;
    }
    if in_min > in_max {
        return map(x, in_max, in_min, out_max, out_min);
    }
    if out_min == out_max {
        return out_min;
    }
    let in_mid = (in_min + in_max) >> 1;
    let out_mid = (out_min + out_max) >> 1;
    if in_min == in_mid {
        return out_mid;
    }
    if x <= in_mid {
        map(x, in_min, in_mid, out_min, out_mid)
    } else {
        map(x, in_mid + 1, in_max, out_mid, out_max)
    }
}

/// getAbsDif (functions.c:42).
#[inline]
pub fn get_abs_dif(a: i32, b: i32) -> u32 {
    (a - b).unsigned_abs()
}

// ===============================================================
// Commutation scheduling math (main.c:900-906, 1872-1874). All
// quantities are INTERVAL_TIMER ticks (0.5 µs). Shift/divide forms
// match the source bit-for-bit (>> where AM32 shifts, / where it
// divides).
// ===============================================================

/// Interrupt-mode interval blend — PeriodElapsedCallback main.c:900:
/// `commutation_interval = (ci + (lastzctime + thiszctime)/2) / 2`.
#[inline]
pub fn blend_interval(ci: u32, lastzc: u32, thiszc: u32) -> u32 {
    (ci + ((lastzc + thiszc) >> 1)) >> 1
}

/// Polling-mode interval blend — zcfoundroutine main.c:1872:
/// `commutation_interval = (thiszctime + 3*ci) / 4`.
#[inline]
pub fn polling_blend(thiszc: u32, ci: u32) -> u32 {
    (thiszc + 3 * ci) / 4
}

/// Advance in ticks — main.c:902/1873: `advance = ci*temp_advance >> 6`.
#[inline]
pub fn advance_of(ci: u32, temp_advance: u32) -> u32 {
    (ci * temp_advance) >> 6
}

/// waitTime — main.c:906/1874: `ci/2 - advance`. AM32's subtraction is
/// unsigned; the transliteration uses `saturating_sub` so a large
/// advance can't underflow (behavior pinned by test).
#[inline]
pub fn wait_time(ci: u32, advance: u32) -> u32 {
    (ci >> 1).saturating_sub(advance)
}

// ===============================================================
// Duty pipeline (main.c:1180-1326 setInput, 1736-1791 tenKhzRoutine).
// ===============================================================

/// duty_cycle_setpoint from throttle input — main.c:1194,1206:
/// `map(input, 47, 2047, minimum_duty_cycle, 2000)` when armed
/// (input>=47), else 0. (map's own endpoint clamps are the "clamps".)
#[inline]
pub fn duty_setpoint(input: u16, min_duty: u16, full: u16) -> u16 {
    if input >= 47 {
        map(input as i32, 47, 2047, min_duty as i32, full as i32) as u16
    } else {
        0
    }
}

/// Ramp step selection — main.c:1746-1754. Startup while young or at
/// low duty, low-rpm above 500-tick intervals, else high-rpm.
#[inline]
pub fn ramp_rate(zero_crosses: u32, last_duty: i32, average_interval: u32) -> i32 {
    if zero_crosses < 150 || last_duty < 150 {
        MAX_RAMP_STARTUP
    } else if average_interval > 500 {
        MAX_RAMP_LOW_RPM
    } else {
        MAX_RAMP_HIGH_RPM
    }
}

/// Ramp `last` toward `target` by at most `rate` per tick, then clamp
/// to the 0..2000 duty domain — main.c:1760-1766,1769. The two
/// one-sided `if`s reproduce AM32's pair of unsigned one-sided clamps
/// without underflow.
#[inline]
pub fn ramp_toward(last: i32, target: i32, rate: i32) -> u16 {
    let mut duty = target;
    if duty - last > rate {
        duty = last + rate;
    }
    if last - duty > rate {
        duty = last - rate;
    }
    duty.clamp(0, DUTY_FULL) as u16
}

/// duty_cycle_maximum low-rpm ceiling — main.c:2441-2450. With
/// low_rpm_throttle_limit on: `e_rpm = 600000/e_com_time`, then
/// `map(e_rpm/10, 20, 70, 400, 2000)`. Off → 2000.
#[inline]
pub fn low_rpm_duty_ceiling(e_com_time_ticks: i32, running: bool, low_rpm_limit: bool) -> u16 {
    if !low_rpm_limit {
        return DUTY_FULL as u16;
    }
    let e_rpm: i32 = if running && e_com_time_ticks > 0 {
        600_000 / e_com_time_ticks
    } else {
        0
    };
    let k_erpm = e_rpm / 10;
    map(
        k_erpm,
        LOW_RPM_LEVEL,
        HIGH_RPM_LEVEL,
        THROTTLE_MAX_AT_LOW_RPM,
        THROTTLE_MAX_AT_HIGH_RPM,
    ) as u16
}

// ===============================================================
// desync predicate — the while(1) desync block main.c:2284-2288.
// ===============================================================

/// True when the average interval jumped more than half its own value
/// AND we are below the 2000-tick speed floor: `getAbsDif(last, avg) >
/// avg/2 && avg < 2000`.
#[inline]
pub fn desync_due(last_average_interval: u32, average_interval: u32) -> bool {
    get_abs_dif(last_average_interval as i32, average_interval as i32) > (average_interval >> 1)
        && average_interval < 2000
}

// ===============================================================
// getBemfState() counter update — main.c:833-851.
// ===============================================================

/// Polling BEMF counter step. When the comparator level matches the
/// commanded direction the counter climbs (saturating); otherwise a
/// bad-read run over `bad_threshold` resets it. Returns the updated
/// `(bemf_counter, bad_count)`. The zero-cross *decision* (counter >
/// min_bemf) lives in the caller because it keys off a different,
/// direction-selected threshold (min_bemf_up/down).
#[inline]
pub fn bemf_count_step(
    bemf_counter: u16,
    bad_count: u16,
    level_matches: bool,
    bad_threshold: u16,
) -> (u16, u16) {
    if level_matches {
        (bemf_counter.saturating_add(1), bad_count)
    } else {
        let bc = bad_count + 1;
        let bemf = if bc > bad_threshold { 0 } else { bemf_counter };
        (bemf, bc)
    }
}

// ===============================================================
// Bench-safety overcurrent accumulator (observer ADC; kill only).
// ===============================================================

/// Windowed current-average accumulator step. Adds `raw`, and when the
/// window fills returns a reset `(0, 0)` plus whether the window
/// average tripped `thresh`. Otherwise carries `(sum, n)` forward.
#[inline]
pub fn oc_step(sum: u32, n: u32, raw: u16, window: u32, thresh: u32) -> (u32, u32, bool) {
    let acc = sum + raw as u32;
    let cnt = n + 1;
    if cnt >= window {
        (0, 0, acc / window > thresh)
    } else {
        (acc, cnt, false)
    }
}

// ===============================================================
// ZC_TRACE record pack + batch-decimation gate (main.c:1542-1567).
// ===============================================================

/// Batch-decimation gate — evaluated per commutation with the
/// pre-increment counter `n`. At each 50-commutation boundary the mode
/// is re-chosen (batch below [`ZCT_BATCH_CI_TICKS`]); between
/// boundaries it holds. Returns `(record, batching)`: `record` is
/// false for the skipped (odd) half of a batch cycle. Batches preserve
/// CONSECUTIVE records (rolling-mean metrics stay valid), and the mode
/// only flips at boundaries (no mid-batch flapping).
#[inline]
pub fn zct_batch_gate(n: u32, ci_ticks: u32, prev_batching: bool) -> (bool, bool) {
    let batching = if n % ZCT_BATCH_LEN == 0 {
        ci_ticks < ZCT_BATCH_CI_TICKS
    } else {
        prev_batching
    };
    let record = !(batching && (n / ZCT_BATCH_LEN) % 2 == 1);
    (record, batching)
}

/// One canonical 15-byte ZC_TRACE row — main.c:1542-1567. `5B A9`
/// sync, a flags/step byte (step in bits0-2, bit7=old_routine,
/// bit6=batched-mode), then little-endian thiszc / ci / wait / duty /
/// tenkhz / avg.
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
) -> [u8; 15] {
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

// ===============================================================
// commutation_intervals[] history — main.c:441,887,2159.
// ===============================================================

/// The 6-slot commutation-interval history (`commutation_intervals[]`),
/// indexed by electrical sector (step-1). Wraps `&'a [AtomicU32; 6]` so
/// the firmware and host tests share one implementation.
///
/// NOTE: AM32 (and the example) compute the average in the main loop
/// from the SUM of the six slots, not at push time; [`push`](Self::push)
/// only stores the slot, and [`e_com_time`](Self::e_com_time) exposes
/// the summed form main uses (`(sum+4)>>1`, main.c:2159). Preserving
/// that split keeps behavior bit-for-bit.
pub struct Am32Intervals<'a> {
    pub slots: &'a [AtomicU32; 6],
}

impl<'a> Am32Intervals<'a> {
    #[inline]
    pub fn new(slots: &'a [AtomicU32; 6]) -> Self {
        Self { slots }
    }

    /// `commutation_intervals[idx] = ticks` (main.c:887).
    #[inline]
    pub fn push(&self, idx: usize, ticks: u32) {
        self.slots[idx].store(ticks, Ordering::Relaxed);
    }

    /// Sum of the six slots (`e_com_time` pre-scale).
    #[inline]
    pub fn sum(&self) -> u32 {
        let mut s = 0u32;
        for slot in self.slots {
            s += slot.load(Ordering::Relaxed);
        }
        s
    }

    /// e_com_time — main.c:2159: `(sum + 4) >> 1`, in 0.5 µs units.
    #[inline]
    pub fn e_com_time(&self) -> i32 {
        (self.sum() as i32 + 4) >> 1
    }
}

// ===============================================================
// UART_DUTY_MODE parser — uart_duty_poll main.c:1367-1416 + fork keys.
// ===============================================================

/// A command decoded from the UART duty-mode byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartCmd {
    /// Commit a throttle value in AM32 input units (48..2047).
    SetThrottle(u16),
    /// 's'/'w', or a committed `0` — stop and zero the throttle.
    Stop,
    /// 'Z' — toggle the ZC_TRACE stream.
    TraceToggle,
    /// 'i' — request an info line.
    Info,
    /// 'b' — request a black-box dump.
    BbDump,
    /// 'G' — request a GECKO free-run current-ring dump.
    GeckoDump,
}

/// The pure UART duty-mode parser state — the digit accumulator that
/// was two `&mut` locals in `parse_rx_byte`. `step` feeds one byte and
/// returns a command when one completes. Semantics mirror
/// main.c:1367-1416 exactly:
/// - digits accumulate up to 4 (further digits ignored, not reset);
/// - 's'/'w' commit [`Stop`](UartCmd::Stop) and clear the accumulator;
/// - 'Z'/'i'/'b'/'G' emit their command WITHOUT touching the accumulator
///   (so `50Z<nl>` still commits 50);
/// - any other byte terminates: with digits pending it commits (a
///   `0` → [`Stop`](UartCmd::Stop); nonzero → percent/permille map to
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
            b's' | b'w' => {
                // main.c:1376-1381 stop; 'w' also latches a bench kill.
                self.acc = 0;
                self.n = 0;
                Some(UartCmd::Stop)
            }
            b'Z' => Some(UartCmd::TraceToggle),
            b'i' => Some(UartCmd::Info),
            b'b' => Some(UartCmd::BbDump),
            b'G' => Some(UartCmd::GeckoDump),
            _ => {
                // terminator → commit (main.c:1382-1415)
                let cmd = if self.n != 0 {
                    let v = self.acc;
                    if v == 0 {
                        Some(UartCmd::Stop) // 0 = STOP (main.c:1397-1398)
                    } else {
                        let mut inn = if v <= 100 {
                            v * 20 + 47 // percent (main.c:1387)
                        } else if v <= 1000 {
                            v * 2 + 47 // permille (main.c:1389)
                        } else {
                            47
                        };
                        if inn < 48 {
                            inn = 48;
                        }
                        if inn > 2047 {
                            inn = 2047;
                        }
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

/// SPSC byte ring over borrowed atomics (the am32_clone USART2 RX
/// ring — sole producer = the RX ISR via [`RxRing::push`], sole
/// consumer = the main loop via [`RxRing::pop`]; a full ring drops
/// the newest byte). Storage lives with the caller; this struct
/// groups the coupled head/tail/ring refs (the cohesion idiom) and
/// owns the index arithmetic. Relaxed ordering is sufficient for
/// SPSC on a single core with data-in-the-atomics.
pub struct RxRing<'a, const N: usize> {
    pub ring: &'a [core::sync::atomic::AtomicU16; N],
    pub head: &'a core::sync::atomic::AtomicUsize,
    pub tail: &'a core::sync::atomic::AtomicUsize,
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

/// ZC_TRACE record ring over borrowed atomics (the am32_clone trace
/// buffer; 15-byte records, [`zct_pack`] layout). Producers push from
/// TWO ISR priorities, so `push_rec` is NOT self-synchronizing — the
/// caller must wrap it in its platform's critical section (core has
/// no cortex-m dependency; that guard stays with the caller). The
/// single consumer drains via a byte sink.
pub struct ZctRing<'a, const N: usize> {
    pub ring: &'a [[core::sync::atomic::AtomicU16; ZCT_REC]; N],
    pub head: &'a core::sync::atomic::AtomicUsize,
    pub tail: &'a core::sync::atomic::AtomicUsize,
    pub drop: &'a AtomicU32,
}

/// The ZC_TRACE wire record size (5B A9 | flags | 6×u16).
pub const ZCT_REC: usize = 15;

impl<const N: usize> ZctRing<'_, N> {
    /// Enqueue one packed record; a full ring drops the OLDEST
    /// (advances tail) and counts it. NOT self-synchronizing — see
    /// the struct docs.
    #[inline]
    pub fn push_rec(&self, rec: &[u8; ZCT_REC]) {
        let h = self.head.load(Ordering::Relaxed);
        let nx = (h + 1) % N;
        if nx == self.tail.load(Ordering::Relaxed) {
            self.drop.fetch_add(1, Ordering::Relaxed);
            self.tail
                .store((self.tail.load(Ordering::Relaxed) + 1) % N, Ordering::Relaxed);
        }
        for (i, b) in rec.iter().enumerate() {
            self.ring[h][i].store(*b as u16, Ordering::Relaxed);
        }
        self.head.store(nx, Ordering::Relaxed);
    }

    /// Drain up to `max_rec` records into `sink`, byte by byte
    /// (main.c:2347-2357 drains 3 per while(1) pass).
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
    use core::sync::atomic::AtomicU32;

    // --- ZctRing -----------------------------------------------

    #[test]
    fn zct_ring_drops_oldest_when_full_and_drains_bounded() {
        use core::sync::atomic::{AtomicU16, AtomicUsize};
        static RING: [[AtomicU16; ZCT_REC]; 4] =
            [const { [const { AtomicU16::new(0) }; ZCT_REC] }; 4];
        static HEAD: AtomicUsize = AtomicUsize::new(0);
        static TAIL: AtomicUsize = AtomicUsize::new(0);
        static DROP: AtomicU32 = AtomicU32::new(0);
        let z = ZctRing { ring: &RING, head: &HEAD, tail: &TAIL, drop: &DROP };
        let mk = |v: u8| {
            let mut r = [0u8; ZCT_REC];
            r[0] = 0x5B;
            r[1] = 0xA9;
            r[2] = v;
            r
        };
        for v in 0..5u8 {
            z.push_rec(&mk(v)); // 5 pushes into capacity 3: 2 oldest dropped
        }
        assert_eq!(DROP.load(Ordering::Relaxed), 2);
        let mut out = [0u8; 64];
        let mut n = 0;
        z.drain(2, |b| {
            out[n] = b;
            n += 1;
        });
        // bounded drain: exactly 2 records, oldest-surviving first
        assert_eq!(n, 2 * ZCT_REC);
        assert_eq!(out[2], 2);
        assert_eq!(out[ZCT_REC + 2], 3);
        let mut n2 = 0;
        z.drain(10, |_| n2 += 1);
        assert_eq!(n2, ZCT_REC); // one record left
    }

    // --- RxRing ------------------------------------------------

    #[test]
    fn rx_ring_push_pop_wraps_and_drops_when_full() {
        use core::sync::atomic::{AtomicU16, AtomicUsize};
        static RING: [AtomicU16; 4] = [const { AtomicU16::new(0) }; 4];
        static HEAD: AtomicUsize = AtomicUsize::new(0);
        static TAIL: AtomicUsize = AtomicUsize::new(0);
        let rx = RxRing { ring: &RING, head: &HEAD, tail: &TAIL };
        assert_eq!(rx.pop(), None);
        rx.push(b'a' as u16);
        rx.push(b'b' as u16);
        rx.push(b'c' as u16);
        // Capacity is N-1: the 4th push must DROP.
        rx.push(b'd' as u16);
        assert_eq!(rx.pop(), Some(b'a'));
        assert_eq!(rx.pop(), Some(b'b'));
        assert_eq!(rx.pop(), Some(b'c'));
        assert_eq!(rx.pop(), None);
        // Wraparound: indices cross N cleanly.
        for k in 0..10u16 {
            rx.push(k);
            assert_eq!(rx.pop(), Some(k as u8));
        }
    }

    // --- map / get_abs_dif -------------------------------------

    #[test]
    fn map_endpoints_mid_and_am32_cases() {
        // Endpoint clamps.
        assert_eq!(map(0, 96, 200, 1666, 3332), 1666);
        assert_eq!(map(300, 96, 200, 1666, 3332), 3332);
        assert_eq!(map(96, 96, 200, 1666, 3332), 1666);
        assert_eq!(map(200, 96, 200, 1666, 3332), 3332);
        // Mid.
        let mid = map(148, 96, 200, 1666, 3332);
        assert!((2470..=2530).contains(&mid), "mid {mid}");
        // variable_pwm carrier case (main.c:2193).
        assert_eq!(map(150, 96, 200, 1666, 3332), map(150, 96, 200, 1666, 3332));
        let c150 = map(150, 96, 200, 1666, 3332);
        assert!((2500..=2600).contains(&c150), "c150 {c150}");
        // duty setpoint endpoints via map.
        assert_eq!(map(47, 47, 2047, 45, 2000), 45);
        assert_eq!(map(2047, 47, 2047, 45, 2000), 2000);
    }

    #[test]
    fn get_abs_dif_symmetric() {
        assert_eq!(get_abs_dif(10, 3), 7);
        assert_eq!(get_abs_dif(3, 10), 7);
        assert_eq!(get_abs_dif(-5, 5), 10);
        assert_eq!(get_abs_dif(100, 100), 0);
    }

    // --- scheduling math ---------------------------------------

    #[test]
    fn advance_and_wait_bench_validated() {
        // ci=746 -> advance 186, wait 187 (bench-validated).
        let a = advance_of(746, 16);
        assert_eq!(a, 186);
        assert_eq!(wait_time(746, a), 187);
        // ci=204 -> wait 51.
        let a2 = advance_of(204, 16);
        assert_eq!(a2, 51);
        assert_eq!(wait_time(204, a2), 51);
        // ci=10000 startup values.
        let a3 = advance_of(10_000, 16);
        assert_eq!(a3, 2500);
        assert_eq!(wait_time(10_000, a3), 2500);
    }

    #[test]
    fn wait_time_saturates_not_underflows() {
        // Large advance can't underflow the unsigned subtraction.
        assert_eq!(wait_time(200, 400), 0);
    }

    #[test]
    fn blend_forms_match_source() {
        // Interrupt blend: (ci + (lz+tz)/2)/2.
        assert_eq!(blend_interval(746, 700, 720), (746 + ((700 + 720) >> 1)) >> 1);
        assert_eq!(blend_interval(1000, 0, 0), 500);
        // Polling blend: (thiszc + 3*ci)/4.
        assert_eq!(polling_blend(700, 746), (700 + 3 * 746) / 4);
        assert_eq!(polling_blend(0, 10_000), 7500);
    }

    // --- duty pipeline -----------------------------------------

    #[test]
    fn duty_setpoint_maps_and_gates() {
        assert_eq!(duty_setpoint(46, 45, 2000), 0);
        assert_eq!(duty_setpoint(47, 45, 2000), 45);
        assert_eq!(duty_setpoint(2047, 45, 2000), 2000);
        // A mid input.
        let m = duty_setpoint(1047, 45, 2000);
        assert!((1000..=1050).contains(&m), "m {m}");
    }

    #[test]
    fn ramp_rate_regimes() {
        // Young: startup rate.
        assert_eq!(ramp_rate(0, 500, 300), MAX_RAMP_STARTUP);
        assert_eq!(ramp_rate(149, 500, 300), MAX_RAMP_STARTUP);
        // Low duty forces startup even when old.
        assert_eq!(ramp_rate(1000, 149, 300), MAX_RAMP_STARTUP);
        // Old + high duty + slow interval: low-rpm.
        assert_eq!(ramp_rate(1000, 500, 600), MAX_RAMP_LOW_RPM);
        // Old + high duty + fast interval: high-rpm.
        assert_eq!(ramp_rate(1000, 500, 400), MAX_RAMP_HIGH_RPM);
    }

    #[test]
    fn ramp_toward_clamps_step_and_domain() {
        // Rising, clamped to rate.
        assert_eq!(ramp_toward(1000, 2000, 16), 1016);
        // Falling, clamped to rate.
        assert_eq!(ramp_toward(2000, 1000, 16), 1984);
        // Within rate: reaches target.
        assert_eq!(ramp_toward(1000, 1010, 16), 1010);
        // Domain clamp at 2000.
        assert_eq!(ramp_toward(1999, 5000, 100), 2000);
        // Domain clamp at 0 (target below zero not reachable via u16;
        // approach from a small last).
        assert_eq!(ramp_toward(10, 0, 100), 0);
    }

    #[test]
    fn low_rpm_ceiling_map_and_off() {
        // Off → 2000.
        assert_eq!(low_rpm_duty_ceiling(1000, true, false), 2000);
        // Not running → e_rpm 0 → below LOW_RPM_LEVEL → 400 floor.
        assert_eq!(low_rpm_duty_ceiling(1000, false, true), 400);
        // Fast (small e_com_time): high k_erpm → 2000 ceiling.
        // e_com_time=100 → e_rpm=6000 → k=600 >= 70 → 2000.
        assert_eq!(low_rpm_duty_ceiling(100, true, true), 2000);
        // A mid speed: e_com_time=15000 → e_rpm=40 → k=4 < 20 → 400.
        assert_eq!(low_rpm_duty_ceiling(15000, true, true), 400);
    }

    // --- desync ------------------------------------------------

    #[test]
    fn desync_boundary_both_sides() {
        // avg=1000: trips when |last-avg| > 500 and avg < 2000.
        assert!(desync_due(1600, 1000)); // dif 600 > 500 → true
        assert!(!desync_due(1400, 1000)); // dif 400 <= 500 → false
        assert!(desync_due(1501, 1000)); // dif 501 > 500 → true
        // exactly avg/2 does NOT trip (> not >=).
        assert!(!desync_due(1500, 1000)); // dif 500, not > 500
        // avg >= 2000 never trips regardless of dif.
        assert!(!desync_due(5000, 2000));
        assert!(!desync_due(5000, 2500));
    }

    // --- bemf counter ------------------------------------------

    #[test]
    fn bemf_count_step_climb_and_reset() {
        // Match: climb, bad_count untouched.
        assert_eq!(bemf_count_step(4, 1, true, 3), (5, 1));
        // Saturating climb at u16::MAX.
        assert_eq!(bemf_count_step(u16::MAX, 0, true, 3), (u16::MAX, 0));
        // Mismatch under threshold: bad climbs, bemf held.
        assert_eq!(bemf_count_step(4, 0, false, 3), (4, 1));
        assert_eq!(bemf_count_step(4, 2, false, 3), (4, 3));
        // Mismatch over threshold: bemf reset to 0.
        assert_eq!(bemf_count_step(4, 3, false, 3), (0, 4));
    }

    // --- overcurrent accumulator -------------------------------

    #[test]
    fn oc_step_accumulates_and_trips() {
        // Mid-window: carry forward.
        assert_eq!(oc_step(100, 5, 20, 1700, 205), (120, 6, false));
        // Window fill, average under threshold: reset, no trip.
        // sum reaching e.g. 1699 samples then last: avg = total/1700.
        let (s, n, trip) = oc_step(1700 * 100, 1699, 100, 1700, 205);
        assert_eq!((s, n), (0, 0));
        assert!(!trip); // avg 100 < 205
        // Window fill, over threshold: trip.
        let (_s, _n, trip2) = oc_step(1700 * 300, 1699, 300, 1700, 205);
        assert!(trip2); // avg 300 > 205
    }

    // --- ZC_TRACE pack + gate ----------------------------------

    #[test]
    fn zct_pack_golden_bytes() {
        // step=3, old_routine, not batched, sample fields.
        let rec = zct_pack(3, true, false, 0x1234, 0x0102, 0x00BB, 0x02C8, 0xABCD, 0x0304);
        assert_eq!(rec[0], 0x5B);
        assert_eq!(rec[1], 0xA9);
        assert_eq!(rec[2], 0x03 | 0x80); // step + old bit, no batch bit
        assert_eq!([rec[3], rec[4]], [0x34, 0x12]); // thiszc LE
        assert_eq!([rec[5], rec[6]], [0x02, 0x01]); // ci LE
        assert_eq!([rec[7], rec[8]], [0xBB, 0x00]); // wait LE
        assert_eq!([rec[9], rec[10]], [0xC8, 0x02]); // duty LE
        assert_eq!([rec[11], rec[12]], [0xCD, 0xAB]); // tenkhz LE
        assert_eq!([rec[13], rec[14]], [0x04, 0x03]); // avg LE
        // Batched flag sets bit6; step masks to 3 bits.
        let rec2 = zct_pack(0x0F, false, true, 0, 0, 0, 0, 0, 0);
        assert_eq!(rec2[2], 0x07 | 0x40); // masked step + batch bit
    }

    #[test]
    fn zct_batch_gate_thresholds_and_boundaries() {
        // Above the CI threshold: never batches, always records.
        let (rec, batch) = zct_batch_gate(0, 300, false);
        assert!(rec && !batch);
        // At a boundary below threshold: batching turns on.
        let (rec, batch) = zct_batch_gate(0, 150, false);
        assert!(rec && batch); // batch cycle 0 (even half) → record
        // Non-boundary holds previous batching.
        let (rec, batch) = zct_batch_gate(25, 300, true);
        assert!(rec && batch); // held true; cycle 0 even → record
        // The skipped (odd) half of a batch: no record.
        // n=50 → boundary, ci below → batch on, cycle 1 (odd) → skip.
        let (rec, batch) = zct_batch_gate(50, 150, false);
        assert!(!rec && batch);
        // n=75, held batching, cycle 1 (odd) → skip.
        let (rec, batch) = zct_batch_gate(75, 300, true);
        assert!(!rec && batch);
        // n=100 → cycle 2 (even) → record again.
        let (rec, _batch) = zct_batch_gate(100, 150, false);
        assert!(rec);
    }

    // --- interval history --------------------------------------

    #[test]
    fn intervals_push_sum_and_e_com_time() {
        let slots: [AtomicU32; 6] = Default::default();
        let iv = Am32Intervals::new(&slots);
        for i in 0..6 {
            iv.push(i, 12500);
        }
        assert_eq!(iv.sum(), 75_000);
        assert_eq!(iv.e_com_time(), (75_000 + 4) >> 1); // 37502
        // Overwrite one slot (wraparound of the index by sector).
        iv.push(0, 6000);
        assert_eq!(iv.sum(), 75_000 - 12_500 + 6_000);
        // Index cycles as the electrical step wraps 0..5.
        for step in 0..12 {
            iv.push(step % 6, step as u32 + 1);
        }
        // Last six writes (step 6..11 → idx 0..5) hold 7..12.
        assert_eq!(iv.sum(), (7 + 8 + 9 + 10 + 11 + 12) as u32);
    }

    // --- UART parser -------------------------------------------

    #[test]
    fn uart_digit_accumulate_and_commit() {
        let mut u = UartDuty::new();
        assert_eq!(u.step(b'5'), None);
        assert_eq!(u.step(b'0'), None);
        // '\n' terminates: 50% → 50*20+47 = 1047.
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(1047)));
        // Accumulator cleared.
        assert_eq!(u.step(b'\n'), None);
    }

    #[test]
    fn uart_zero_commit_is_stop() {
        let mut u = UartDuty::new();
        u.step(b'0');
        assert_eq!(u.step(b'\n'), Some(UartCmd::Stop));
    }

    #[test]
    fn uart_permille_and_clamps() {
        let mut u = UartDuty::new();
        // 500 permille → 500*2+47 = 1047.
        for c in b"500" {
            u.step(*c);
        }
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(1047)));
        // Very large > 1000 → 47 → clamped up to 48.
        for c in b"2000" {
            u.step(*c);
        }
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(48)));
        // 1000 permille → 2047 exactly.
        for c in b"1000" {
            u.step(*c);
        }
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(2047)));
    }

    #[test]
    fn uart_command_bytes() {
        let mut u = UartDuty::new();
        assert_eq!(u.step(b's'), Some(UartCmd::Stop));
        assert_eq!(u.step(b'w'), Some(UartCmd::Stop));
        assert_eq!(u.step(b'Z'), Some(UartCmd::TraceToggle));
        assert_eq!(u.step(b'i'), Some(UartCmd::Info));
        assert_eq!(u.step(b'b'), Some(UartCmd::BbDump));
        assert_eq!(u.step(b'G'), Some(UartCmd::GeckoDump));
    }

    #[test]
    fn uart_control_byte_does_not_disturb_accumulator() {
        // '50Z\n' → Z toggles, THEN newline commits 50.
        let mut u = UartDuty::new();
        u.step(b'5');
        u.step(b'0');
        assert_eq!(u.step(b'Z'), Some(UartCmd::TraceToggle));
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(1047)));
    }

    #[test]
    fn uart_empty_terminator_and_digit_cap() {
        let mut u = UartDuty::new();
        // Bare terminator with no digits → nothing.
        assert_eq!(u.step(b'\n'), None);
        // More than 4 digits: 5th ignored, still commits the 4-digit
        // accumulator (12345 → acc stays 1234 → >1000 → 47 → 48).
        for c in b"12345" {
            u.step(*c);
        }
        assert_eq!(u.step(b'\n'), Some(UartCmd::SetThrottle(48)));
        // 's'/'w' garbage tolerance mid-number: clears then stops.
        u.step(b'9');
        assert_eq!(u.step(b's'), Some(UartCmd::Stop));
        assert_eq!(u.step(b'\n'), None); // accumulator was cleared
    }
}
