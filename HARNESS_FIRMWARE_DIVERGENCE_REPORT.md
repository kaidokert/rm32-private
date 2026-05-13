# Harness/firmware divergence — structural report

**Trigger.** Stalled-motor false-sync detection (AM32 cuts throttle within ~1 s; rm32 buzzes indefinitely). Root cause: `rm32_stm32/src/bin/main.rs` is missing the explicit transfer

```rust
if isr.commutation.desync_check() {
    main_state.set_desync_check(true);
    isr.commutation.set_desync_check(false);
}
```

that the harness performs at `rm32/src/bin/harness.rs:432-436`. Without this transfer in firmware, `MainState.desync_check` is permanently `false`, the entire desync-detect block at `rm32/src/main_state.rs:323-358` is dead code, and false-sync (the failure mode where the BEMF comparator triggers on inductive ringing of a stalled motor instead of true rotation) goes unhandled.

This is the **second** of-this-shape bug found in the past two days. The first (stuck-rotor stall-rate, fixed in commit `ed9ac29`) was the firmware missing `SET_INTERVAL_TIMER_COUNT(0)` after stall registration — the harness had no such bug because it advances `interval_timer` explicitly per tick from its own field. Same class: **a non-trivial fragment of the per-tick orchestration lives only in `harness.rs`, not in the shared library code, and the firmware happens to forget to include the equivalent.**

Per the user's question: why aren't these paths unified, what's blocking it, and how do we structurally eliminate the bug class?

---

## What's actually shared vs duplicated today

Looking at `rm32/src/bin/harness.rs::do_tick` (lines 396-443) side-by-side with `rm32_stm32/src/bin/main.rs::main`'s loop body (lines 270-417):

### Already shared (called from both)

- `system.tick_input(shared, &mut main_state)` — input processing
- `system.tick_main(shared, &mut main_state, &mut adc, &mut telem)` — main pipeline
- `isr_logic::ten_khz_tick(&mut ctx)` — 20 kHz ISR body
- `isr_logic::commutation_timer_expired(...)`, `bemf_zero_cross(...)` — other ISR bodies

The intent is documented at `bin/main.rs:348-349`:
```rust
// Shared system tick: input processing + main loop pipeline.
// Same function called by harness — eliminates divergence.
```

That works for the inside-out logic. **It doesn't catch the orchestration between calls.**

### Orchestration logic that is NOT shared (lives in both places, hand-mirrored)

The following per-tick steps appear in `harness.rs::do_tick` AND need to appear in `bin/main.rs`'s loop. They are not wrapped in any shared function:

| Harness step | Firmware equivalent | Status |
|---|---|---|
| `shared.set_send_esc_info_flag(false)` (1-shot clear) | (cleared inside `tick_main` poll?) | ? |
| `shared.set_newinput(self.throttle_value)` | DSHOT/PWM decode in DMA IRQ → `set_newinput` | different mechanisms, OK |
| `self.hal.interval.count += 1` | TIM hardware increments | OK, abstracted away |
| `self.handle_transfer()` (input capture decode) | `handle_exti_frame` ISR | different mechanisms, OK |
| `system.tick_input` | same | ✓ shared |
| `isr_logic::ten_khz_tick(&mut ctx)` | TIM6 ISR calls same | ✓ shared via call site |
| **`if commutation.desync_check() { main.set_desync_check(true); commutation.set_desync_check(false) }`** | **MISSING** | ✗ **bug** |
| `system.tick_main` | same | ✓ shared |

The harness explicitly transfers `commutation.desync_check` → `MainState.desync_check` between `ten_khz_tick` and `tick_main`. The firmware needs to do the same transfer — except the firmware's `commutation` lives inside `ISR_LOCAL.get()` and must be accessed through `isr::with_isr_state(|isr| { ... })` because it's behind an `UnsafeCell` + critical-section guard. The harness can do it as direct field access; the firmware needs the closure form. Different syntax, same intent — and the firmware never got the line.

---

## Why aren't they unified?

Two structural reasons make literal code-sharing of `do_tick`/`main` non-trivial today:

### 1. Different state ownership models

- **Harness**: `Commutation`, `BemfState`, `DutyState`, `Hal` are all direct fields of `Harness`. Same-thread access. No locking.
- **Firmware**: those same types live inside `ISR_LOCAL: IsrCell` (`isr_handlers.rs:69`) which is an `UnsafeCell<Option<TargetIsrState>>` accessed via `with_isr_state(|isr| { ... })` that disables interrupts for the closure body. Main-loop code cannot dereference these fields directly; it has to go through the closure.

The harness's `if self.commutation.desync_check() { ... }` becomes, in firmware, `isr::with_isr_state(|isr| if isr.commutation.desync_check() { ... })`. Same logic, different syntactic shape. Today's shared functions like `tick_main` take `&SharedState` (the atomic-flag-backed bidirectional channel) and `&mut MainState` (the main-loop-exclusive blob) — neither of those captures `Commutation`, which is ISR-exclusive.

### 2. Different concurrency model

- **Harness**: single-threaded, no interrupts, free to mutate everything in any order.
- **Firmware**: cooperatively scheduled main loop + preemptive ISRs. Accessing ISR-owned state from main requires `with_isr_state` (interrupt-free critical section). Accessing main-owned state from an ISR requires going through atomic `SharedState` channels.

These are real constraints, not arbitrary divergence — but they push the *orchestration* (who-talks-to-whom-when) into the two main loops rather than into shared code, where it gets hand-duplicated and drifts.

---

## Pattern of the bug class

Every harness/firmware divergence found so far has the same shape:

1. The harness handles a piece of orchestration inline in `do_tick` (a flag transfer, a state copy, a synthetic ISR step).
2. The shared library functions (`tick_input`, `tick_main`, `ten_khz_tick`) do NOT include this piece — they trust the caller to do it.
3. The firmware author *should* mirror the harness's orchestration in `main.rs`. They forget, or never see the harness code.
4. Blackbox tests run the harness, which contains the orchestration. Tests pass.
5. Bug surfaces only on real hardware, only under conditions that trigger the missing path.

**Why blackbox tests can't see it.** The test harness IS one of the divergent paths. By construction, every blackbox test runs against a path that has the orchestration. The path that doesn't have it (firmware) is never executed by tests.

A perfect harness would be *byte-for-byte identical* to firmware in everything except platform shims (timer count source, ADC source, etc.). Today's harness is more like a parallel re-implementation of the main loop.

---

## How to eliminate the class

Three increasingly-invasive options, in order of effort:

### Option A — Pull every orchestration step into a shared function (small refactor, immediate)

Identify each line in `harness.rs::do_tick` that does cross-component orchestration (the desync_check transfer is one; there are probably others). Move each into a named function in shared library code:

```rust
// New in rm32/src/system.rs or similar
pub fn post_isr_orchestration<S: SharedComm>(
    commutation: &mut Commutation,
    main: &mut MainState<LED>,
) {
    if commutation.desync_check() {
        main.set_desync_check(true);
        commutation.set_desync_check(false);
    }
    // ... future similar transfers ...
}
```

Both harness AND firmware call `post_isr_orchestration` between `ten_khz_tick` and `tick_main`. Harness calls it directly; firmware wraps with `isr::with_isr_state`. The forgetting-to-do-it failure mode becomes a forgetting-to-call-it failure mode, which has TWO call sites instead of one — slightly less likely to slip but doesn't eliminate the class.

**Pros:** small, immediate, no API churn. Catches future similar bugs as long as discipline holds.

**Cons:** still relies on developer discipline at the call site. A future maintainer in firmware land can still forget to call the new function.

### Option B — Single shared `run_loop` over a `Platform` trait (medium refactor)

Lift the main-loop body itself into a shared `pub fn run_loop<P: Platform>(p: &mut P) -> !` (or non-`!` for tests). Define a `Platform` trait that abstracts every platform-specific operation:

```rust
pub trait Platform {
    type Hal: MotorHal;
    type Adc: Adc;
    type Telem: TelemetryUart;

    fn shared(&self) -> &SharedState;
    fn main_state(&mut self) -> &mut MainState;
    fn with_isr_state<R>(&mut self, f: impl FnOnce(&mut IsrState) -> R) -> R;
    fn adc(&mut self) -> &mut Self::Adc;
    fn telem(&mut self) -> &mut Self::Telem;
    fn reset(&mut self) -> !;
    fn reload_watchdog(&mut self);
    fn wait_for_interrupt(&mut self);   // wfi on firmware, no-op or yield on harness
    fn now_tick(&self) -> u32;          // current tick — for log heartbeat
}
```

`run_loop` is the single canonical orchestration:

```rust
pub fn run_loop<P: Platform>(p: &mut P) -> ! {
    loop {
        p.tick_input();
        // ... post-ISR transfers ...
        p.with_isr_state(|isr| {
            if isr.commutation.desync_check() {
                p.main_state().set_desync_check(true);
                isr.commutation.set_desync_check(false);
            }
        });
        p.tick_main();
        // ... poll one-shots ...
        p.reload_watchdog();
        p.wait_for_interrupt();
    }
}
```

Harness and firmware both implement `Platform`. Harness's `with_isr_state` is a direct closure; firmware's is the current `isr::with_isr_state`. Harness's `wait_for_interrupt` is a no-op (or runs the next vector line); firmware's is `cortex_m::asm::wfi`.

**Pros:** eliminates the class. There is one main loop. Adding orchestration to it is a single edit that BOTH paths inherit. Tests automatically exercise the firmware's orchestration shape because there's only one.

**Cons:** non-trivial refactor. Defines a `Platform` trait that needs to capture every quirk (RTT vs stdout for logging, sine-mode handling, LED handling, etc.). The closure-style `with_isr_state` is awkward to express in trait form (HRTB plus lifetime gymnastics) — likely needs unsafe or duplicate-but-tightly-bound traits.

### Option C — Make the harness call into firmware's `main()` via a no-std-compatible entry point (big refactor)

Refactor `rm32_stm32/src/bin/main.rs` so it's a thin shim:

```rust
#[entry]
fn main() -> ! {
    let platform = HardwarePlatform::init();
    rm32::run_loop(platform);
}
```

The harness similarly:

```rust
fn main() {
    let mut platform = HarnessPlatform::new();
    rm32::run_one_loop_iteration(&mut platform);  // call N times from test
}
```

If `rm32::run_loop` and `rm32::run_one_loop_iteration` share the same body (the latter being a single iteration of the former), the harness physically cannot have an orchestration step the firmware lacks — same code.

**Pros:** structurally bulletproof. The class disappears.

**Cons:** significant lift. The `Platform` trait gets more demanding (Iterator-like control flow for the harness vs `loop` for firmware). Some firmware operations (`SCB::sys_reset`) are `-> !` and need explicit no-std-compatible mocking in harness. The `wfi` → "advance to next test vector line" mapping needs to handle the no-input idle case the firmware faces.

---

## Concrete recommendations

**Immediately:** Option A. Fix the current bug (move the desync_check transfer into a shared function) AND audit `harness.rs::do_tick` for any other orchestration steps that aren't shared functions. Each missing step gets pulled into a named function called from both. Estimate: half-day audit + small commits per finding.

**Short term (this week or next):** start Option B. Define `Platform` trait, move at least the main-loop body into shared code. The platform-specific shims stay small. Even an incomplete `Platform` migration (covering the most-divergence-prone steps) prevents the most likely future bugs of this class.

**Don't do Option C** unless `Platform` turns out to be a clean abstraction in practice. The risk is over-engineering; a `Platform` trait that everyone hates and works around is worse than two parallel paths.

**Process change to harden any option:** every commit to `harness.rs::do_tick` MUST be accompanied by an identical-shape commit to `rm32_stm32/src/bin/main.rs` (or the equivalent shared function). Add a pre-commit/CI grep: `if grep -P 'do_tick' shows changed orchestration, fail unless main.rs or run_loop also changed`. Crude but cheap.

---

## Specific to the immediate bug

Fix is one block in `rm32_stm32/src/bin/main.rs`, mirroring harness lines 432-436. Insert between `system.tick_input(...)` and `system.tick_main(...)` at line 350-351:

```rust
// Sync desync_check from commutation (ISR-owned) into MainState. Must run
// after ten_khz_tick has had a chance to update commutation (i.e. after
// any ISR ticks since last iteration), before tick_main consumes the
// flag. Harness does this inline at harness.rs:432-436; firmware needs
// the with_isr_state wrapper because commutation is behind ISR_LOCAL.
isr::with_isr_state(|isr| {
    if isr.commutation.desync_check() {
        main_state.set_desync_check(true);
        isr.commutation.set_desync_check(false);
    }
});
```

Verify on bench: stalled-prop test should now produce 2-3 "jiggle" cycles and throttle cut within ~1 s, matching AM32. Add a blackbox vector `stall_false_sync.txt` that drives the desync conditions (jitter-injected `average_interval`, `zero_crosses > 10`) and asserts `running` drops to 0 plus `bemf_timeout_happened` eventually crosses threshold.
