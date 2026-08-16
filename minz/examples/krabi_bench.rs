//! krabilorean M4 on-silicon timing bench (feature = "krabimon").
//!
//! Runs each arithmetic-personality workload over a real baked commutation
//! window on the L431 @ 80 MHz, DWT.CYCCNT-timed (min/median/max over K reps),
//! and prints the distribution over RTT. Also runs the end-to-end
//! BlockRetainedCaptureProfile (variance + periodicity + Gaussian-MI over one
//! shared centered artifact) and reports both its cost and its result fields.
//! No motor required — timing is value-independent; the baked windows are real.
//!
//! Flash+read:  cargo run --release --example krabi_bench --features krabimon
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use minz::board_init::{BoardInit, init};
use rtt_target::rprintln;
use stm32l4xx_hal::stm32;

// WIN_HI (var 5.4e6) + WIN_LO (var 1.7) — real ci windows from zct_today.
include!("krabi_vectors.rs");

#[cfg(feature = "krabimon")]
mod bench {
    use super::*;
    use core::hint::black_box;
    use krabilorean::personality::{
        BlockPrecisionPolicy, BlockScaled32, BoundedExact64, NearestTiesAway,
    };
    use krabilorean::windowed::dynamics::{
        BlockGaussianMiWorkspace, BlockRetainedCaptureProfile, GaussianMiProfile,
        GaussianMiWorkspace,
    };

    const K: usize = 64;
    const MAXMAG: u32 = 32_768;
    // dynamics window params (MAX_LEN <= 128)
    const DLEN: usize = 128;
    const LAGS: usize = 32;
    const MINP: usize = 2;
    const MAXP: usize = 64;
    const SIG: u32 = 12;

    #[inline(always)]
    fn cyc() -> u32 {
        cortex_m::peripheral::DWT::cycle_count()
    }

    fn dist<F: FnMut()>(mut f: F) -> (u32, u32, u32) {
        let mut s = [0u32; K];
        for x in s.iter_mut() {
            let t0 = cyc();
            f();
            let t1 = cyc();
            *x = t1.wrapping_sub(t0);
        }
        s.sort_unstable();
        (s[0], s[K / 2], s[K - 1])
    }

    fn row(label: &str, d: (u32, u32, u32)) {
        // 80 MHz -> µs = cyc/80
        rprintln!(
            "  {:<26} min {:>6} cyc ({:>6.2} us)  med {:>6}  max {:>6}",
            label,
            d.0,
            d.0 as f32 / 80.0,
            d.1,
            d.2
        );
    }

    fn variance_workloads(win: &[i16; 256]) {
        // Every closure black_boxes the numeric RESULT (not just .is_ok()) so
        // the full accumulation can't be dead-code-eliminated.
        row(
            "exact64 variance",
            dist(|| {
                let r = BoundedExact64::<256>::variance(black_box(win), MAXMAG)
                    .map(|v| v.centered_energy)
                    .unwrap_or(0);
                black_box(r);
            }),
        );
        row(
            "block8 variance",
            dist(|| {
                let r = BlockScaled32::<8, NearestTiesAway>::variance(black_box(win), MAXMAG)
                    .map(|v| {
                        (
                            v.centered_energy.significand,
                            v.population_variance.numerator,
                        )
                    })
                    .unwrap_or((0, 0));
                black_box(r);
            }),
        );
        row(
            "block12 variance",
            dist(|| {
                let r = BlockScaled32::<12, NearestTiesAway>::variance(black_box(win), MAXMAG)
                    .map(|v| {
                        (
                            v.centered_energy.significand,
                            v.population_variance.numerator,
                        )
                    })
                    .unwrap_or((0, 0));
                black_box(r);
            }),
        );
        row(
            "block16 variance",
            dist(|| {
                let r = BlockScaled32::<16, NearestTiesAway>::variance(black_box(win), MAXMAG)
                    .map(|v| {
                        (
                            v.centered_energy.significand,
                            v.population_variance.numerator,
                        )
                    })
                    .unwrap_or((0, 0));
                black_box(r);
            }),
        );
        row(
            "block12 moments (online)",
            dist(|| {
                if let Ok(mut m) = BlockScaled32::<12, NearestTiesAway>::moments(MAXMAG, 256) {
                    for &x in black_box(win).iter() {
                        let _ = m.update(x);
                    }
                    let m = black_box(m); // force the full update loop to materialize
                    let s = m
                        .snapshot()
                        .map(|s| (s.sum, s.sum_of_squares.significand))
                        .unwrap_or((0, 0));
                    black_box(s);
                }
            }),
        );
    }

    fn retained_end_to_end(win: &[i16; 256]) {
        let w: &[i16] = &win[..DLEN];
        // Exact Gaussian-MI (WideExact) timing, for the MI-alone comparison.
        if let Ok(exmi) = GaussianMiProfile::<DLEN, LAGS>::new() {
            let mut ws = GaussianMiWorkspace::<DLEN, LAGS>::new();
            row(
                "exact GaussianMI (WideExact)",
                dist(|| {
                    black_box(exmi.evaluate(black_box(w), &mut ws).is_ok());
                }),
            );
        }
        // End-to-end block retained capture profile.
        match BlockRetainedCaptureProfile::<DLEN, LAGS, MINP, MAXP, SIG, NearestTiesAway>::new(
            MAXMAG,
            BlockPrecisionPolicy::unrestricted(),
        ) {
            Ok(profile) => {
                let mut ws = BlockGaussianMiWorkspace::<LAGS>::new();
                row(
                    "block retained (var+per+MI)",
                    dist(|| {
                        black_box(profile.evaluate(black_box(w), &mut ws).is_ok());
                    }),
                );
                // one real evaluation → report the decision fields
                if let Ok(ev) = profile.evaluate(w, &mut ws) {
                    let pv = ev.variance.population_variance;
                    let var = pv.numerator as f32 * libm_exp2(pv.numerator_binary_exponent)
                        / pv.denominator as f32;
                    let peak = match ev.periodicity.result {
                        krabilorean::feature_bank::periodicity::PeriodicityResult::Defined(f) => {
                            f.strongest_positive.map(|p| p.lag as i32).unwrap_or(-1)
                        }
                        _ => -2,
                    };
                    let mi = ev
                        .gaussian_mi
                        .curve
                        .first_local_minimum
                        .map(|v| v as i32)
                        .unwrap_or(-1);
                    rprintln!(
                        "    -> result: var~{:.1}  period_peak=lag{}  MI_first_min=lag{}",
                        var,
                        peak,
                        mi
                    );
                }
            }
            Err(_) => rprintln!("    block retained: config rejected"),
        }
    }

    // 2^n as f32 without pulling libm (n small, nonneg)
    fn libm_exp2(n: u32) -> f32 {
        let mut v = 1.0f32;
        for _ in 0..n {
            v *= 2.0;
        }
        v
    }

    pub fn run() {
        rprintln!(
            "== krabilorean M4 timing (L431 @ 80 MHz, DWT.CYCCNT, K={}) ==",
            K
        );
        rprintln!("-- WIN_HI (real transition window, var 5.4e6) --");
        variance_workloads(&WIN_HI);
        retained_end_to_end(&WIN_HI);
        rprintln!("-- WIN_LO (steady-lock window, var 1.7 — block collapse regime) --");
        variance_workloads(&WIN_LO);
        retained_end_to_end(&WIN_LO);
        rprintln!("== done ==");
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit { mut cp, clocks, .. } = init(cp, dp.FLASH, dp.RCC, dp.PWR);
    rprintln!("krabi_bench: sysclk={}", clocks.sysclk().raw());
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    unsafe { core::ptr::write_volatile(0xE000_1004 as *mut u32, 0) };

    #[cfg(feature = "krabimon")]
    bench::run();
    #[cfg(not(feature = "krabimon"))]
    rprintln!("krabi_bench: build with --features krabimon");

    loop {
        cortex_m::asm::nop();
    }
}
