# Stuck-rotor protection port-bug report

**Symptom.** With `eeprom.stuck_rotor_protection = 1` on an L431 + Vimdrones board running rm32 firmware, the motor will not spin up at any throttle level. The same setting on AM32 C firmware spins the same motor on the same hardware (stiff bench PSU, identical BF DSHOT300 source) without issue. Disabling `stuck_rotor_protection` in EEPROM makes rm32 spin fine.

**Earlier debug evidence (port_41.log).** When rm32 was running successfully (with `stuck_rotor_protection` hard-coded to 0 in `main.rs`), the loop telemetry recorded:

```
mode=OldRoutine newinput=462 adj=462 bemf_to_hap=102 bemf_to=10 zc=0
mode=OldRoutine newinput=462 adj=462 bemf_to_hap=102 bemf_to=10 zc=0
mode=Running    newinput=462 adj=462 bemf_to_hap=0   bemf_to=10 zc=10000
```

`bemf_timeout_happened` reaches 102 (`BEMF_FAULT_LATCHED` ceiling) *during normal startup*, before BEMF synchronisation completes. With the protection enabled, the latch trips at threshold 10 long before BEMF locks → throttle is zeroed → motor can never accelerate enough for BEMF to lock → permanent latched fault.

---

## Earlier wrong diagnosis (correction)

In an earlier reply I claimed the root cause was that rm32's `main_state.rs` uses `raw_input` (= `newinput`) at four sites where AM32 uses `adjusted_input` (lines 286, 289, 293, 316). That is a real divergence and *would* matter for bidir-DSHOT and sine-start configurations, but it is **not** the explanation for the user's unidirectional-DSHOT case: AM32 sets `adjusted_input = newinput` for the unidirectional path (`AM32/Src/main.c:1099`), so for throttle 462 both AM32's `adjusted_input` and rm32's `raw_input` evaluate to 462 → both pick the strict (10) bucket. Same bucket, same gate, but only one spins. I should have checked that before pointing at the variable mismatch as the bug.

---

## The actual root cause

The stall-detection path differs between AM32 and rm32 in a way that controls how *often* `bemf_timeout_happened` increments during startup.

**AM32 (`Src/main.c:2145-2156`):**

```c
if (INTERVAL_TIMER_COUNT > 45000 && running == 1) {
    bemf_timeout_happened++;
    maskPhaseInterrupts();
    old_routine = 1;
    if (input < 48) {
        running = 0;
        commutation_interval = 5000;
    }
    zero_crosses = 0;
    zcfoundroutine();      // ← (*)
}
```

`zcfoundroutine()` (defined at `Src/main.c:1565`) does:

```c
void zcfoundroutine() {
    thiszctime = INTERVAL_TIMER_COUNT;
    SET_INTERVAL_TIMER_COUNT(0);   // ← interval counter reset to 0
    commutation_interval = (thiszctime + (3 * commutation_interval)) / 4;
    advance = (temp_advance * commutation_interval) >> 6;
    waitTime = commutation_interval / 2 - advance;
    ...                              // ← also synthesises a commutation step
}
```

The critical effect for our bug: **`SET_INTERVAL_TIMER_COUNT(0)` resets the interval timer to zero every time a stall is registered.** The next stall therefore cannot be registered until *another* 22.5 ms (≈45000 ticks at 2 MHz) have elapsed without a real BEMF zero-cross.

**rm32 (`rm32/src/main_state.rs:301-313`):**

```rust
if shared.interval_timer_count() > BEMF_STALL_TIMER_THRESHOLD && shared.running() {
    if self.protection.bemf_timeout_happened != BEMF_FAULT_LATCHED {
        self.protection.bemf_timeout_happened =
            self.protection.bemf_timeout_happened.saturating_add(1);
    }
    shared.set_old_routine(true);
    if shared.adjusted_input() < THROTTLE_MIN_SIGNAL {
        shared.transition(crate::motor_mode::MotorEvent::StopMotor);
        shared.set_commutation_interval(DESYNC_RESET_INTERVAL);
    }
    shared.set_zero_crosses(0);
    // ← NO equivalent to zcfoundroutine()'s SET_INTERVAL_TIMER_COUNT(0)
}
```

There is no reset of `interval_timer_count` here. If the ISR layer also fails to reset it on this synthetic stall (we did not yet verify it does), then on the next main-loop tick the condition `interval_timer_count > 45000` is still true and `bemf_timeout_happened` increments *again*. The counter races ahead at main-loop rate (≈8 kHz on this build, since `EXTI` fires per DSHOT frame), rather than at the 1/22.5ms ≈ 44 Hz rate AM32 throttles it to. In well under a second the counter saturates past 10 (strict threshold) → latch fires.

Even with `stuck_rotor_protection = 0` we *saw* this — the counter reached 102 (the `BEMF_FAULT_LATCHED` ceiling baked into the increment guard) within the first few hundred ms of startup. That's a ≈10× faster rate than AM32 produces, which is exactly the magnitude needed to defeat the 10-stall strict gate.

---

## Why the existing tests didn't catch this

Three test categories cover this code, none of them caught the divergence. Each missed it for a different reason.

### 1. `rm32/src/control/input.rs:261` — `bemf_timeout_latches_input_zero`

```rust
#[test]
fn bemf_timeout_latches_input_zero() {
    let (shared, mut config, mut prot, mut input) = setup();
    config.stuck_rotor_protection = 1;
    prot.bemf_timeout_happened = 20;   // ← pre-set state
    prot.bemf_timeout = 10;            // ← pre-set state
    shared.newinput.set(500);
    process_input(&shared, &config, &mut prot, &mut input);
    assert_eq!(input.input, 0);
    assert_eq!(shared.adjusted_input.get(), 0);
    assert_eq!(prot.bemf_timeout_happened, BEMF_FAULT_LATCHED);
}
```

**Why it missed.** This test only verifies the *latch behaviour given pre-set state*. It manually sets `bemf_timeout_happened = 20` and `bemf_timeout = 10` and then asserts that one tick latches `input` to 0. It does not test *how the counter got to 20* — which is the part that differs from AM32.

### 2. `tests/blackbox/vectors/bemf_timeout.txt`

```
@config
eeprom.stuck_rotor_protection=1
@sequence
tick 1 | throttle=500 | input=500
config bemf_timeout_happened=20    # ← pre-set
tick 1 | throttle=500 | input=0
```

**Why it missed.** Same problem — the vector pre-configures `bemf_timeout_happened = 20` directly via the `config` directive and then asserts a single-tick outcome. Both C and Rust harnesses honour the pre-set state and produce identical output; the test passes for both. The divergence in *the increment rate* (the actual bug) is invisible because the test bypasses the increment path entirely.

### 3. `tests/blackbox/vectors/stuck_rotor.txt`

```
@sequence
tick 1  | throttle=500 interval_timer=1000 | running=1 input=500
tick 1  | throttle=500 interval_timer=50000 |   # ×20 ticks
...
tick 1  | throttle=500 interval_timer=50000 | input=0 running=0 alloff_count>0
```

**Why it missed.** This one *does* exercise the increment path — `interval_timer=50000` is over the 45000 threshold and ticked 20 times. But the test sets `interval_timer` *as an input parameter to each tick*, which both harnesses then read. The difference between AM32 (which resets `INTERVAL_TIMER_COUNT` inside `zcfoundroutine()` after each stall registration) and rm32 (which does not) is hidden because the vector script *overrides* `interval_timer` to 50000 before each tick anyway. The C harness's reset inside the path gets clobbered by the next vector line, masking the divergence.

A vector that runs the stall path *without* manually re-asserting `interval_timer` every tick — i.e., setting it to 50000 once and letting the firmware tick freely — would have caught it. The two harnesses would disagree: C would re-arm the timer to 0 after the first stall and need another 22.5 ms before the second; Rust would increment the counter every loop iteration. After N ticks the `bemf_timeout_happened` values would differ wildly.

### 4. `bemf_timeout_dynamic.txt`

Tests the lenient/strict-bucket selection, but `bemf_timeout_happened` is pre-set in the `config` block. Same flavour of pre-set bypass — doesn't exercise the increment rate.

---

## Test-coverage gap summary

The pattern is consistent: every existing test for this protection block **pre-configures the counter state** and asserts a *single-tick* downstream outcome. None of them simulate a multi-tick startup ramp where `bemf_timeout_happened` grows organically from 0 and races against the threshold. That is exactly the regime where the C and Rust implementations diverge.

This is a structural property of vector-driven blackbox tests when the vectors over-specify intermediate state. The blackbox model trusts that *single-step* matching is enough to demonstrate equivalence, but for any state machine whose bug is a *rate-of-change* divergence, single-step parity tells you nothing.

What would have caught it:
- A blackbox vector that pre-sets `interval_timer=50000` *once*, then runs `tick 200` (let firmware tick freely without re-asserting interval_timer per tick), and asserts on the final `bemf_timeout_happened`. C ≈ 4-8 increments per 100ms (≈22.5ms re-arm cycle). Rust would saturate at 102. The two values diverge by orders of magnitude.
- A unit test in `rm32::control::isr_logic` (or wherever the stall handler in rm32 lives) that asserts `interval_timer_count` is reset to 0 after the stall handler runs — explicitly checking the side-effect that AM32 produces.
- A property-based test that runs the stall-detect handler N times in succession and asserts the counter increments at most ⌈N / (45000/loop_rate)⌉ times — i.e., bounded by the timer re-arm period, not by the loop rate.

---

## Subsidiary issue: `adjusted_input` zeroing on latch

`rm32/src/control/input.rs:163` does `shared.set_adjusted_input(0)` inside the latch path. AM32 (`Src/main.c:1102-1106`) does *not* touch `adjusted_input` on latch — it only sets `input = 0`. This is a behavioural divergence but it does **not** explain the user's symptom. It would matter for downstream consumers of `adjusted_input` (telemetry, ramp recovery) but is not why startup fails. We should still fix it for parity, but separately from the rate-of-increment bug.

---

## Subsidiary issue: `raw_input` vs `adjusted_input` at the four selection sites

The four sites in `main_state.rs` (lines 286, 289, 293, 316) that AM32 indexes with `adjusted_input` are indexed in rm32 with `raw_input`. For the user's unidirectional DSHOT300 case the two values are equal, so this isn't the immediate bug — but for bidir DSHOT, sine-start, and servo-PWM modes the values diverge and rm32 would pick the wrong bucket. Worth fixing as part of "make rm32 actually match AM32," not as a fix for the user's spin failure.

---

## Recommended fix order

1. **Verify the diagnosis.** Instrument rm32 to log `interval_timer_count` and `bemf_timeout_happened` inside the stall-detect block, observe whether `interval_timer_count` is *not* being reset between consecutive triggers as predicted. If the diagnosis holds, proceed to (2).
2. **Add a `set_interval_timer_count(0)` (or whatever rm32 calls it) inside `main_state.rs:301-313`**, matching AM32's `SET_INTERVAL_TIMER_COUNT(0)` inside `zcfoundroutine()`. This is the actual port-parity fix for the user's failure mode.
3. **Add a regression test** of the "tick freely without re-asserting interval_timer" shape described above, ensuring it fails on the pre-fix code and passes on the post-fix code.
4. **(Cleanup, separate commits)** Fix the `set_adjusted_input(0)` on latch and the `raw_input` → `adjusted_input` selection sites for parity in the other input modes.
