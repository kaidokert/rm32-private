//! Idle-time profiler — busy-loop counter ratio'd against a calibrated
//! reference window. Lifted from `ref/usb-servo-rs/servo/src/idle_loop.rs`.
//!
//! Idea: replace the main-loop `nop` slack with a counter increment.
//! At boot, run the busy loop for a known time window (with interrupts
//! enabled) to learn how many increments fit. Then during normal
//! operation, run the same counter between scheduled checkpoints and
//! compare against the calibration: a *lower* count means ISRs were
//! eating cycles → more busy. The busy fraction is `1 - latched/cal`.
//!
//! Call [`calibrate`](IdleLoop::calibrate) **before app ISRs are
//! unmasked**, with the same time source the main loop uses. The
//! reference examples calibrate while SysTick (their wall-clock
//! source) is already running but before USB / servo / encoder
//! interrupts are unmasked — that's the right line: app ISRs would
//! depress the baseline, but the wall-clock tick is part of the
//! system's idle cost and is expected to be present in both windows.
//!
//! `calibration_window == latch_window` is a hard requirement of
//! [`busy_percentage`]'s math. The reference pattern calibrates for
//! 1 s and latches the counter once per second after iterating many
//! microloops.
//!
//! Time source is caller-provided (`get_time: &impl Fn() -> u64`) so
//! the same code drops onto any wall-clock — DWT cycle counter,
//! SysTick-derived 10 µs ticks, an LPTIM2-backed `Instance::now()`,
//! whatever. The reference uses milliseconds; minz typically has
//! 10 µs ticks — both are fine as long as `duration` in [`calibrate`]
//! is in the *same* units the closure returns.
//!
//! ## Usage (mirrors `ref/usb-servo-rs/.../examples/loop_timing.rs`)
//! ```ignore
//! let mut idle = IdleLoop::new();
//! idle.calibrate(/* 1 s in your time units */, &|| now());
//!
//! let mut start = now();
//! loop {
//!     let next_second = start + ONE_SECOND;
//!     let mut next_microloop = start + MICROLOOP_PERIOD;
//!     while now() < next_second {
//!         idle.run_until(next_microloop, &|| now());  // slack
//!         // active work: dequeue keys, dispatch, etc.
//!         next_microloop += MICROLOOP_PERIOD;
//!     }
//!     let busy = idle.busy_percentage(idle.latch()); // once per second
//!     start = next_second;
//! }
//! ```

use core::sync::atomic::{AtomicU32, Ordering};

pub struct IdleLoop {
    counter: AtomicU32,
    calibration: u32,
    calibration_time: u64,
}

impl Default for IdleLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleLoop {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
            calibration: 0,
            calibration_time: 0,
        }
    }

    /// Spin the counter for `duration` time-units, then latch the result
    /// as the reference for `busy_percentage`. Call once after the rest
    /// of the system is running so the captured rate reflects realistic
    /// ISR load.
    pub fn calibrate(&mut self, duration: u64, get_time: &impl Fn() -> u64) -> u32 {
        let start = get_time();
        self.run_until(start + duration, get_time);
        self.calibration_time = get_time() - start;
        self.calibration = self.latch();
        self.calibration
    }

    /// Spin the counter until `get_time() >= until`. This replaces a
    /// `nop` / `wfi` slack at the bottom of the main loop. The
    /// surrounding loop must be paced to a microloop period: each
    /// outer iteration does active work (key dispatch, IO, whatever)
    /// once, then calls `run_until` to spin out the rest of the
    /// microloop. Time spent in the active phase + time stolen by
    /// ISRs both reduce the counter relative to calibration.
    pub fn run_until(&self, until: u64, get_time: &impl Fn() -> u64) {
        while get_time() < until {
            self.counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Atomically read-and-zero the counter. Call once per reporting
    /// window; pass the returned value to [`busy_percentage`].
    pub fn latch(&self) -> u32 {
        self.counter.swap(0, Ordering::Relaxed)
    }

    /// `0.0` = fully busy, `1.0` = fully idle. Returns `0.0` if
    /// calibration hasn't run yet.
    pub fn idle_fraction(&self, latched_value: u32) -> f32 {
        if self.calibration == 0 {
            return 0.0;
        }
        latched_value as f32 / self.calibration as f32
    }

    /// Inverse of [`idle_fraction`], rounded to an integer 0..=100.
    pub fn busy_percentage(&self, latched_value: u32) -> u8 {
        let busy_fraction = 1.0 - self.idle_fraction(latched_value);
        (busy_fraction * 100.0).clamp(0.0, 100.0) as u8
    }
}
