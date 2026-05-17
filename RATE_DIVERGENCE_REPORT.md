# Rate-of-call divergence from AM32 — PIDs and ADC at 20× their intended rate

## Summary

rm32's `main_state.tick` (the body of the 20 kHz main loop) runs ADC sampling, voltage/current filtering, the three PID controllers (stall protection, current limit, speed control), and a handful of derived calculations on **every tick**. AM32 puts the equivalent work behind a 1 kHz divider gate.

**Identical computational work, 20× the rate.**

Bench measurement on Vimdrones L431 KCU6 at 80 MHz, 50 µs (4000 cyc) period:

- Main-loop iter cost on rm32: **1317–1392 cyc = 16.5–17.4 µs**, every iter, all motor modes
- Estimated equivalent at AM32 rates: **~582 cyc = ~7.3 µs**

That's roughly 10 µs of every 50 µs period (20% of CPU) being burned on work that should be running at 1 kHz.

## Evidence

### AM32 source — the 1 kHz gate

`AM32/Src/main.c:1397`:

```c
one_khz_loop_counter++;
...
if (one_khz_loop_counter > PID_LOOP_DIVIDER) {   // 1khz PID loop
    PROCESS_ADC_FLAG = 1;  // ADC at 1 kHz, not 20 kHz
    one_khz_loop_counter = 0;
    if (use_current_limit && running) {
        // ... PID work ...
    }
    // ... duty_ceiling, etc ...
}
```

`PID_LOOP_DIVIDER` is 20 (matches `LOOP_FREQUENCY_HZ / 1000 = 20000/1000 = 20`).

### rm32 source — no gate at all

`rm32/src/main_state.rs:425-475`:

```rust
let smoothed_v = AdcCount(self.measurements.voltage_filter.update(adc.raw_voltage()));
let smoothed_c = AdcCount(self.measurements.current_filter.update(adc.raw_current()));
self.measurements.battery_voltage = smoothed_v.to_millivolts(self.voltage_divider);
self.measurements.actual_current = smoothed_c.to_milliamps(...);
self.measurements.degrees_celsius = ntc::ntc_degrees(adc.raw_temperature());
adc.start_conversion();
shared.set_actual_current(self.measurements.actual_current.0);
shared.set_battery_voltage(self.measurements.battery_voltage.0);
shared.set_degrees_celsius(self.measurements.degrees_celsius.0);
// ...
self.pid.tick_stall(shared.commutation_interval() as i32);
self.pid.tick_current_limit(actual_current, target, min_duty, running);
self.pid.tick_speed_control(e_com_time, zc, running);
```

No `if (...)` gate. Every call lives at the top level of `main_state.tick`, which runs every main-loop iter (20 kHz).

`MainState` does not even define a `one_khz_counter` field. There is a `ten_khz_counter` at line 498 used for the `consumed_current` accumulation (1 Hz), but no 1 kHz dispatcher.

## Per-period cycle attribution

| Block | Cycles (estimated) | Currently runs at | Should run at |
|---|---|---|---|
| ADC reads + 3× IIR filter + unit conv + 3 atomic publish | ~400 | 20 kHz | 1 kHz |
| Stall + current + speed PIDs (3 PID ticks) | ~300 | 20 kHz | 1 kHz |
| LVC counter increment + threshold check | ~50 | 20 kHz | 1 kHz |
| `duty_ceiling()` + dynamic `filter_level` map + `auto_advance` map + `min_bemf_counts` | ~150 | 20 kHz | every main iter (correct after wfi removal) |
| `adjust_irq_priorities` (3× NVIC IPR writes) | ~50 | 20 kHz | on state change |
| `process_input` + BEMF housekeeping + signal_timeout + stall detect + desync | ~300 | 20 kHz | 20 kHz (correct) |
| LED blink + watchdog reload | ~100 | 20 kHz | 20 kHz (correct) |
| `log_counter` + gated branches | ~50 | 20 kHz | 20 kHz (correct) |
| Bracket overhead (DWT + atomic store) | ~40 | 20 kHz | 20 kHz |
| **Total measured** | **~1390** | | |
| **Estimated after AM32 1 kHz gate** | | | **~582** |

Note: cycle attribution is estimated by inspection. The aggregate `main_last_cyc = 1390` is DWT-measured; the row-by-row breakdown is not separately bracketed.

**Correction to the original report**: an earlier version of this table incorrectly listed `duty_ceiling`, `filter_level`, `auto_advance`, and `min_bemf_counts` as belonging in the 1 kHz block. On closer reading of `main.c:2010-2123`, only ADC + LVC + the 3 PIDs (`main.c:1397-1434`) are inside `PROCESS_ADC_FLAG`; the rest run every main iter in AM32 (outside the gate). Post-wfi-removal our main runs at ~75 kHz, which matches AM32's "every main iter" pattern. LVC moved into the 1 kHz block as a follow-up; the rest stay where they are.

## Fix

1. Add an atomic `one_khz_counter` on `SharedState` (matches AM32's `uint16_t one_khz_loop_counter` — global, accessed from both ISR and main).
2. Increment in `ten_khz_tick` (TIM6 ISR), matching AM32's `main.c:1317`.
3. In `main_state.tick`, gate the ADC + 3 PID + LVC blocks behind `if shared.one_khz_counter_check_and_reset(PID_LOOP_DIVIDER) { ... }`. `PID_LOOP_DIVIDER = 20` (AM32's `targets.h:5318`).
4. Keep at 20 kHz (every main iter): `process_input`, BEMF timeout housekeeping, stall detection, desync detection, signal_timeout, LED blink, `consumed_current` accumulation, `duty_ceiling`, `filter_level`, `auto_advance`, `min_bemf_counts`, `variable_pwm`. These are all what AM32 also runs every main iter (some inside `if(stepper_sine == 0)` outside the `PROCESS_ADC_FLAG` gate).
5. Remove `wfi()` from main loop to match AM32's spinning `while(1)` (separate concern, but tangled — see commit `5b69dc6`).
6. (Future) Move `adjust_irq_priorities` out of every-iter — call only on commutation_interval state transitions, not every iter. It writes NVIC IPR every iter even when nothing changed.

Verification: `main_last_cyc` drops from ~1390 to ~1000-1100 cyc per iter (savings: ADC + 3 PIDs + LVC ≈ 300 cyc/iter on the 19-of-20 skip iters). The 1-in-20 "work iter" runs the gated block plus everything else (~1400 cyc), so the average across iters is ~1050 cyc.

## How this slipped through testing

### Per-call assertions cannot catch per-frequency divergence

The blackbox harness asserts on HAL call counters: `assert!(alloff_count > 0)`, `assert_eq!(mask_interrupts_count, expected)`. It verifies *what was called*, not *how often per unit time*.

A test that would have caught this: simulate 20,000 ticks (1 second of 20 kHz operation), assert that `adc.start_conversion()` was called between 950 and 1050 times. **No such assertion exists for any HAL method**, anywhere in the test suite.

The same harness runs vectors against both the C build (AM32 itself) and the Rust harness. If C calls ADC 1000 times during a vector and Rust calls it 20000 times, both eventually produce similar filtered outputs — per-sample work is identical, only the rate differs — so output-based assertions don't fire. The harness was designed to catch behavioral divergences (missing `allOff()` calls, wrong state transitions, etc.) and it found those. It cannot see rate divergences by construction.

### A measurement bug hid the symptom

Investigation into bench-observed motor chops initially hypothesized "TIM6 ISR overrunning its 50 µs budget" based on `t6_max = 5500–7600 cyc` reported by DWT instrumentation. That instrumentation used `fetch_max` (LDREX/STREX loop) *and* an inline `rprintln!` heartbeat *inside* the timing bracket. Both inflated the measurement.

After stripping the instrumentation overhead, actual TIM6 cost is 6.8–9.3 µs typical / 19 µs spike. Plenty of headroom. The chops are not TIM6-bound — but a whole investigation (NVIC priority shift, COMP mask companion fix, ~3 commits) went into the "TIM6 budget" angle before that became clear.

The instrumentation strip then forced the question "if ISRs only eat 14 µs out of 50, where do the other 36 µs go?" — bracketed main loop → got 17 µs → looked at `main_state.tick` → found the missing 1 kHz gate.

**A measurement bug hid an architectural bug. Diagnostic infrastructure that includes itself in the bracket is worse than no measurement, because it produces confident-looking wrong answers.**

### COVERAGE_GAPS.md scope was behavioral, not dispatch-rate

The coverage audit focused on whether code paths existed and were exercised. There is no notion of "this work runs at rate X" in the audit. Rate divergence is invisible to that framing.

## Recommended testing improvements

### (a) Rate-of-call assertions in the blackbox harness — HIGH PRIORITY

Each HAL call counter should have an expected calls-per-second range. Run vectors with simulated-time bookkeeping (the harness already counts ticks), assert at end:

```
assert_call_rate("adc.start_conversion", expected_per_sec=1000, tolerance=0.05);
assert_call_rate("send_telemetry",      expected_per_sec=2,    tolerance=0.10);
assert_call_rate("comutate",            expected_per_sec=running ? f(rpm) : 0, ...);
```

Will catch any future port that diverges in rate. Would have caught the current bug on the day it was introduced.

### (b) Cycle-budget runtime assertion on bench-debug builds

The DWT instrumentation in `handle_tim6` and the main loop is already in place. Add a self-policing panic:

```rust
#[cfg(feature = "bench_assert_budget")]
if main_last_cyc > MAIN_BUDGET_THRESHOLD {
    over_budget_count += 1;
    if over_budget_count > 100 {
        panic!("main loop blew budget {} consecutive ticks", over_budget_count);
    }
} else {
    over_budget_count = 0;
}
```

Configurable threshold per-MCU. Consecutive-N-failures gate avoids false positives. Bench runs would self-report regressions; users wouldn't have the panic in release builds.

### (c) One-time AM32 frequency-domain audit

Enumerate every function/macro call in AM32's `tenKhzRoutine`, the main-loop body, and the ISRs. Tag each with its expected dispatch rate (20 kHz, 1 kHz, on-event, on-state-transition). Diff against rm32's equivalents. Each mismatch is either a documented intentional change or a port bug.

**Almost certainly there are more 20× divergences than this one.** ADC + PIDs is just the one we found because it was big enough to show up in cycle counts. Smaller rate mismatches (e.g., something that should run on-state-transition but runs every tick) are invisible to current tests and to cycle measurement until they accumulate.

## Severity

- **Correctness**: not corrupting state. Filtered values are over-sampled, not wrong.
- **Performance**: 20% of CPU continuously wasted on work that should run at 1 kHz. Reduces headroom for legitimate ISR storms (DSHOT frame bursts, high-eRPM commutation rates) by the same 20%.
- **Behavior**: PID loops running at 20× rate change their effective tuning (proportional terms unchanged; integrals accumulate 20× faster; derivative gain is 20× more sensitive). Motor behavior is therefore subtly different from AM32 in any closed-loop mode (`use_current_limit`, `drive_by_rpm`, stall protection). This may be one contributor to residual motor chops at low/mid throttle, though that remains unconfirmed.
- **Discovery cost**: significant. This bug masquerades as a CPU-headroom issue and points the investigation in the wrong direction until measurement infrastructure is corrected.
