//! Disabled-output policy exercise, not a full COMP ISR timing certificate.
use super::*;
use core::hint::black_box;
pub fn run<W: Write>(out: &mut W) {
    if core_bench::active() || powered_timer::owns() || !core_bench::bridge_disabled() {
        let _ = writeln!(out, "SEEDCHECK refused=1");
        return;
    }
    let mut maximum = 0u16;
    let mut failed = 0u32;
    for _ in 0..64 {
        let mut q = driven_seed::Qualification::with_timing_reanchor(0);
        let first = driven_seed::Edge {
            epoch: 1,
            step: 4,
            before: 2906,
            after: 2930,
            interval: 1114,
        };
        let long = driven_seed::Edge {
            epoch: 2,
            step: 5,
            before: 5048,
            after: 5074,
            interval: 2142,
        };
        q.accept(black_box(first));
        let before = t17();
        let result = black_box(&mut q).accept(black_box(long));
        black_box(result);
        let elapsed = t17().wrapping_sub(before);
        maximum = maximum.max(elapsed);
        if result.is_some()
            || q.fault() != 0
            || q.discarded_fault() != 3
            || q.reanchors() != (1, 2)
            || q.intervals() != 0
        {
            failed += 1;
        }
        // The next long interval must latch, not earn another restart.
        q.accept(driven_seed::Edge {
            epoch: 3,
            step: 6,
            before: 7190,
            after: 7216,
            interval: 2142,
        });
        if q.fault() != 3 || q.ready().is_some() {
            failed += 1;
        }
    }
    let _ = writeln!(
        out,
        "SEEDCHECK trials=64 failed={} reset_max_us={} disabled={} full_isr=0 gate_authority=0",
        failed,
        maximum,
        core_bench::bridge_disabled() as u8
    );
}
