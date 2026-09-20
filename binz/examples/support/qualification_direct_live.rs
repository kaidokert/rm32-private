//! COMP owns mutation; foreground reset/dump only after dispatch unwinds.
//! Guard safing does not touch this storage. No hardware or gate writes.
use super::*;
#[cfg(feature = "bench-reject-time")]
#[path = "reject_time.rs"]
mod reject_time;
#[cfg(feature = "bench-reject-time")]
static mut LATE: reject_time::Ring = reject_time::Ring::new();
static mut COUNTER: qualification_counts::Counter = qualification_counts::Counter::new();
static mut HALF_AVERAGE: u32 = 0;
static mut EPOCH: u32 = 0;
static mut TOTAL: u32 = 0;
static mut INVALID: bool = false;
static mut LAST_STEP: u8 = 0;
static mut ROWS: [[u32; 4]; 16] = [[0; 4]; 16];
/// Foreground only, while reset/report owns the stopped observation.
pub fn epoch() -> u32 {
    unsafe { EPOCH }
}

pub fn reset() {
    unsafe {
        COUNTER = qualification_counts::Counter::new();
        EPOCH = EPOCH.saturating_add(1);
        INVALID = EPOCH == u32::MAX;
        TOTAL = 0;
        LAST_STEP = 0;
        #[cfg(feature = "bench-reject-time")]
        (&mut *core::ptr::addr_of_mut!(LATE)).reset();
    }
}
pub struct Dispatch;
// Fuse dispatch admission with the first-read latch in each caller. The RAM
// acceptance helper remains out of line; this removes only setup call cost.
#[inline(always)]
pub fn begin(powered: bool, average: u32) -> Dispatch {
    unsafe {
        if powered && !INVALID {
            HALF_AVERAGE = average >> 1;
            (&mut *core::ptr::addr_of_mut!(COUNTER)).begin();
        } else {
            (&mut *core::ptr::addr_of_mut!(COUNTER)).end();
        }
        Dispatch
    }
}
impl Drop for Dispatch {
    fn drop(&mut self) {
        unsafe {
            (&mut *core::ptr::addr_of_mut!(COUNTER)).end();
        }
    }
}
#[inline]
pub fn first_count(count: u16) {
    unsafe {
        (&mut *core::ptr::addr_of_mut!(COUNTER)).first_count(count, HALF_AVERAGE);
    }
}
#[cfg(feature = "bench-qualification-reject")]
#[cfg_attr(not(feature = "bench-reject-time"), inline(always))]
#[cfg_attr(feature = "bench-reject-time", inline(never))]
pub fn persistence_rejected(read_index: u16) {
    unsafe {
        let counter = &mut *core::ptr::addr_of_mut!(COUNTER);
        if counter.active() {
            counter.persistence_rejected(read_index);
            #[cfg(feature = "bench-reject-time")]
            if read_index != 0 && counter.active() && !counter.invalid() {
                // Callback-time coordinates, NOT mismatch/physical-edge timestamps.
                // TIM2 brackets sequential reads of PWM and guard-tick phase.
                let interval = (*stm32::TIM2::ptr()).cnt().read().bits() as u16;
                let pwm = (*stm32::TIM1::ptr()).cnt().read().bits() as u16;
                let guard = (*stm32::TIM6::ptr()).cnt().read().bits() as u16;
                let end = (*stm32::TIM2::ptr()).cnt().read().bits() as u16;
                let c = counter.snapshot();
                (&mut *core::ptr::addr_of_mut!(LATE)).push([
                    TOTAL,
                    c.dispatched as u32 | ((read_index as u32) << 16),
                    c.last_open as u32 | ((interval as u32) << 16),
                    pwm as u32 | ((guard as u32) << 16),
                    end as u32,
                ]);
            }
        }
    }
}
fn packed(before: u32, step: u8, c: qualification_counts::Counts) -> [u32; 4] {
    [
        before,
        step as u32 | ((c.dispatched as u32) << 16),
        c.closed as u32 | ((c.open as u32) << 16),
        c.first_open as u32 | ((c.last_open as u32) << 16),
    ]
}
/// Called only at the recorder commit, after electrical/ownership guards.
#[inline(never)]
#[cfg_attr(
    feature = "bench-direct-ram",
    unsafe(link_section = ".data.direct_accept")
)]
pub fn accepted(before: u32, step: u8) {
    unsafe {
        if !(&*core::ptr::addr_of!(COUNTER)).active() {
            return;
        }
        if before != TOTAL
            || !(1..=6).contains(&step)
            || (LAST_STEP != 0 && step != if LAST_STEP == 6 { 1 } else { LAST_STEP + 1 })
            || TOTAL == u32::MAX
        {
            INVALID = true;
            (&mut *core::ptr::addr_of_mut!(COUNTER)).end();
            return;
        }
        let Some(words) = (&mut *core::ptr::addr_of_mut!(COUNTER)).accepted_words() else {
            INVALID = true;
            return;
        };
        ROWS[(TOTAL & 15) as usize] = [before, words[0] | step as u32, words[1], words[2]];
        TOTAL += 1;
        LAST_STEP = step;
    }
}
#[inline(never)]
pub fn dump<W: Write>(out: &mut W, capture: bool, final_accepted: u32) {
    unsafe {
        if !capture {
            return;
        }
        let active = (&*core::ptr::addr_of!(COUNTER)).active();
        let c = &mut *core::ptr::addr_of_mut!(COUNTER);
        c.freeze();
        let epoch = EPOCH;
        let total = TOTAL;
        let n = total.min(16);
        let omitted = total - n;
        let version = if cfg!(feature = "bench-qualification-reject") {
            2
        } else {
            1
        };
        let _ = writeln!(
            out,
            "QUALDIRECT epoch={} n={} total={} omitted={} invalid={} active={} final_accepted={} dispatched_only=1 partial_until_unwind=1 wire=qd85-v{}",
            epoch,
            n,
            total,
            omitted,
            (INVALID || c.invalid()) as u8,
            active as u8,
            final_accepted,
            version
        );
        for index in 0..=n {
            let partial = index == n;
            let mut row = if partial {
                packed(TOTAL, 0, c.snapshot())
            } else {
                ROWS[((omitted + index) & 15) as usize]
            };
            if partial && cfg!(feature = "bench-qualification-reject") {
                row[1] |= (c.rejection_mask() as u32) << 4;
            }
            let mut words = [0u16; 11];
            words[0] = EPOCH as u16;
            words[1] = (EPOCH >> 16) as u16;
            for j in 0..4 {
                words[2 + 2 * j] = row[j] as u16;
                words[3 + 2 * j] = (row[j] >> 16) as u16;
            }
            words[10] = partial as u16;
            let _ = snapshot::record(out, "QD85", &words);
        }
        #[cfg(feature = "bench-reject-time")]
        {
            let late = &*core::ptr::addr_of!(LATE);
            let count = late.total.min(8);
            let omitted = late.total - count;
            let _ = writeln!(
                out,
                "REJECTTIME epoch={} n={} total={} omitted={} final_accepted={} invalid={} callback_coordinates=1 wire=rt85-v1",
                epoch, count, late.total, omitted, total, late.invalid as u8
            );
            for seq in omitted..late.total {
                let row = late.rows[(seq & 7) as usize];
                let mut words = [0u16; 14];
                words[0] = epoch as u16;
                words[1] = (epoch >> 16) as u16;
                words[2] = seq as u16;
                words[3] = (seq >> 16) as u16;
                for j in 0..5 {
                    words[4 + j * 2] = row[j] as u16;
                    words[5 + j * 2] = (row[j] >> 16) as u16;
                }
                let _ = snapshot::record(out, "RT85", &words);
            }
        }
    }
}

// Mode dispatch and function-call overhead precede the measurement. Keep
// inputs opaque so specialization cannot substitute fixed counter values.
#[inline(never)]
fn measure<const MODE: u8>(preload: u32, step: u8, average: u32, count: u16) -> (u32, u32, bool) {
    let preload = core::hint::black_box(preload);
    let step = core::hint::black_box(step);
    let average = core::hint::black_box(average);
    let count = core::hint::black_box(count);
    #[cfg(feature = "bench-qualification-reject")]
    let reject_index = core::hint::black_box(11u16);
    let began = t17();
    let cycle_begin = cortex_m::peripheral::SYST::get_current();
    let d = begin(MODE != 4, average);
    if MODE != 3 {
        first_count(count);
        first_count(999);
    }
    #[cfg(feature = "bench-qualification-reject")]
    if MODE == 1 {
        persistence_rejected(reject_index);
    }
    if MODE == 2 {
        accepted(preload, step);
    }
    drop(d);
    let cycle_end = cortex_m::peripheral::SYST::get_current();
    let elapsed = t17().wrapping_sub(began) as u32;
    let cycles = if cycle_begin >= cycle_end {
        cycle_begin - cycle_end
    } else {
        64000 - cycle_end + cycle_begin
    };
    let valid = cycle_begin < 64000
        && cycle_end < 64000
        && elapsed < 1000
        && cycles > 0
        && cycles <= elapsed * 64 + 64;
    (elapsed, cycles, valid)
}

#[inline(never)]
pub fn check<W: Write>(out: &mut W) {
    if core_bench::active()
        || powered_timer::owns()
        || driven_run::owns()
        || !core_bench::bridge_disabled()
    {
        let _ = writeln!(out, "DIRECTCHECK refused=1");
        return;
    }
    // Existing 64 MHz / 1 ms SysTick, read-only. Reading CSR clears its
    // foreground blink COUNTFLAG; no safety clock uses that flag. No reload,
    // counter, interrupt-enable or TIM17 writes are made by this instrument.
    let syst_ctrl = unsafe { (*cortex_m::peripheral::SYST::PTR).csr.read() } & 7;
    let syst_reload = cortex_m::peripheral::SYST::get_reload();
    if syst_ctrl != 5 || syst_reload != 63999 {
        let _ = writeln!(out, "DIRECTCHECK clock_refused=1");
        return;
    }
    for preload in [0u32, 16, 32] {
        for mode in 0..5 {
            let mut passed = 0;
            let mut maximum = 0;
            let mut max_cycles = 0;
            let mut cycle_valid = true;
            for _ in 0..16 {
                reset();
                for n in 0..preload {
                    let d = begin(true, 1000);
                    first_count(600);
                    accepted(n, (n % 6 + 1) as u8);
                    drop(d);
                }
                let step = core::hint::black_box((preload % 6 + 1) as u8);
                let average = core::hint::black_box(1000);
                let count = core::hint::black_box(if mode == 0 { 500 } else { 501 });
                let (elapsed, cycles, valid) = match mode {
                    0 => measure::<0>(preload, step, average, count),
                    1 => measure::<1>(preload, step, average, count),
                    2 => measure::<2>(preload, step, average, count),
                    3 => measure::<3>(preload, step, average, count),
                    _ => measure::<4>(preload, step, average, count),
                };
                cycle_valid &= valid;
                max_cycles = max_cycles.max(cycles);
                maximum = maximum.max(elapsed);
                let valid = unsafe {
                    let c = (&*core::ptr::addr_of!(COUNTER)).snapshot();
                    let mut expected = qualification_counts::Counts::ZERO;
                    if mode != 2 && mode != 4 {
                        expected.dispatched = 1;
                        if mode == 0 {
                            expected.closed = 1;
                        }
                        if mode == 1 {
                            expected.open = 1;
                            expected.first_open = 501;
                            expected.last_open = 501;
                        }
                    }
                    let row_ok = mode != 2
                        || ROWS[(preload & 15) as usize]
                            == packed(
                                preload,
                                step,
                                qualification_counts::Counts {
                                    dispatched: 1,
                                    closed: 0,
                                    open: 1,
                                    first_open: 501,
                                    last_open: 501,
                                },
                            );
                    let mask_ok = (&*core::ptr::addr_of!(COUNTER)).rejection_mask()
                        == if cfg!(feature = "bench-qualification-reject") && mode == 1 {
                            1 << 11
                        } else {
                            0
                        };
                    mask_ok
                        && !(&*core::ptr::addr_of!(COUNTER)).active()
                        && !INVALID
                        && !(&*core::ptr::addr_of!(COUNTER)).invalid()
                        && TOTAL == preload + (mode == 2) as u32
                        && c == expected
                        && row_ok
                };
                if valid {
                    passed += 1;
                }
            }
            let _ = writeln!(
                out,
                "DIRECTCHECK preload={} mode={} passed={} total=16 max_us={} gate_authority=0",
                preload, mode, passed, maximum
            );
            let _ = writeln!(
                out,
                "DIRECTCYCLES preload={} mode={} max_cycles={} valid={} core_hz=64000000 reload=63999 subtraction=0 gate_authority=0",
                preload, mode, max_cycles, cycle_valid as u8
            );
        }
    }
    reset();
    for n in 0..40 {
        #[cfg(feature = "bench-qualification-reject")]
        {
            let d = begin(true, 1000);
            first_count(600);
            persistence_rejected((n % 12) as u16);
            drop(d);
        }
        let d = begin(true, 1000);
        first_count(600);
        accepted(n, (n % 6 + 1) as u8);
        drop(d);
    }
    let d = begin(true, 1000);
    first_count(777);
    #[cfg(feature = "bench-qualification-reject")]
    persistence_rejected(11);
    drop(d);
    dump(out, true, 40);
    reset();
    let _ = writeln!(
        out,
        "DIRECTCHECK END disabled={}",
        core_bench::bridge_disabled() as u8
    );
}
