# `with_isr_state` post-IRQ no-op — silent-failure report

## TL;DR

`rm32_stm32::isr::with_isr_state(...)` is a no-op the moment any ISR has fired. Every call to it from the *post-`cortex_m::interrupt::enable()`* portion of `bin/main.rs` silently does nothing. Six call sites in the main loop are affected. The function neither panics nor returns a `Result`; the closure simply doesn't execute, and the compiler can't see anything wrong.

The previous "ISR-vs-main divergence" report (`HARNESS_FIRMWARE_DIVERGENCE_REPORT.md`) and the harness/firmware unification work (`SystemTick::run_tick`) both rest on `with_isr_state` actually executing. It doesn't. The desync-detection fix we committed never ran on the chip — that's why `last_average_interval=0` was stuck in the bench log.

## Mechanism (verbatim from source)

Two separate statics own the ISR-side state:

```rust
// rm32_stm32/src/isr.rs:100
static ISR_STATE: Mutex<RefCell<Option<TargetIsrState>>> =
    Mutex::new(RefCell::new(None));
```

```rust
// rm32_stm32/src/isr_handlers.rs:69
static ISR_LOCAL: IsrCell = IsrCell::new();
```

The hand-off happens on the *first ISR invocation*, inside `IsrCell::get()`:

```rust
// rm32_stm32/src/isr_handlers.rs:50-66 (abridged)
fn get(&self) -> &mut TargetIsrState {
    let opt = unsafe { &mut *self.0.get() };
    let needed_init = opt.is_none();
    let state = opt.get_or_insert_with(||
        isr::take_isr_state().expect("ISR state not initialized")
    );
    if needed_init {
        // First-time init: state was just moved from ISR_STATE into this
        // ISR_LOCAL cell, ...
    }
    state
}
```

`isr::take_isr_state()` removes the `Option`'s contents:

```rust
// rm32_stm32/src/isr.rs:113-115
pub fn take_isr_state() -> Option<TargetIsrState> {
    cortex_m::interrupt::free(|cs| ISR_STATE.borrow(cs).borrow_mut().take())
}
```

After that, `ISR_STATE` holds `None` forever. And `with_isr_state` silently skips when `ISR_STATE` is `None`:

```rust
// rm32_stm32/src/isr.rs:117-124
/// Access ISR state in a critical section (before interrupts take it).
pub fn with_isr_state(f: impl FnOnce(&mut TargetIsrState)) {
    cortex_m::interrupt::free(|cs| {
        if let Some(ref mut state) = *ISR_STATE.borrow(cs).borrow_mut() {
            f(state);
        }
        // else: closure is silently never called
    });
}
```

The doc comment hints at the limitation ("before interrupts take it") but every caller has to guess from a four-word parenthetical that the function silently fails after a specific runtime moment. The function returns `()` so there's no error channel to ignore — calling code looks identical to working code.

## Timeline of failure for `bin/main.rs`

```
boot sequence
├── init_isr_state(isr_state)         // ISR_STATE = Some(...)
├── with_isr_state(... DMA arm ...)    // ✓ works (pre-enable)
├── with_isr_state(... config push ...)
├── with_isr_state(... dead_time ...)
├── unsafe { cortex_m::interrupt::enable() };  // line 250
│       └── first ISR fires (TIM6 @ 20kHz):
│           └── ISR_LOCAL.get() → take_isr_state()
│               └── ISR_STATE = None        ← ghost moment
└── loop {                                  ← every with_isr_state below is a no-op
       with_isr_state(... read commutation.desync_check ...)
       with_isr_state(... sine PWM compare ...)
       with_isr_state(... sine changeover ...)
       with_isr_state(... sync_isr_to_main ...)   ← the desync fix lives here
       with_isr_state(... arming beeps ...)
       with_isr_state(... save settings: copy isr.config back ...)
   }
```

## Confirmed no-op call sites in the main loop

Located via `grep -nE "with_isr_state" rm32_stm32/src/bin/main.rs` and cross-checked against `cortex_m::interrupt::enable()` at line 250:

| Line | Closure body — what it pretends to do | Reality |
|---|---|---|
| 276 | `dchk = isr.commutation.desync_check();` (diagnostic for this report) | dchk reads default `false` |
| 307 | `isr.hal.pwm.set_compare{1,2,3}(...)` (sine PWM application) | PWM duty isn't written |
| 321 | `isr.commutation.set_step(...); isr.hal.phase.com_step(...); ...` (sine→BLDC changeover) | Changeover doesn't execute |
| 345 | `sys.sync_isr_to_main(&mut isr.commutation, main)` (Option-A desync transfer) | **`desync_check` never transfers — explains `last=0` in bench log** |
| 354 | Arming beeps via `sounds.play_input(...)` | Arming beeps never play |
| 397 | `main_state.config = isr.config;` (save-settings flushes ISR-side config back to main) | Wrong config gets flashed (uses pre-boot copy, not ISR-mutated copy) |

## Symptoms observed on the bench, now explained

1. **Stalled-motor false-sync never detected (`bemf_to_hap=0`, `last=0` always).** The desync transfer at line 345 was a no-op. `commutation.desync_check` is set by ISR but never copied to `main_state.desync_check`. The desync detection block at `main_state.rs:329-358` never enters. Real cause of this entire session's investigation.
2. **No arming chirp despite "I think I hear it" from earlier session.** Line 354 closure never runs. What the user heard was probably the WS2812 LED bit-bang or the startup tune tail, not the arming beeps.
3. **Sine-mode startup unverified.** Both line 307 (PWM compare per step) and line 321 (changeover hand-off) are no-ops. Sine mode is currently impossible to exercise correctly from this firmware.
4. **Save settings may persist stale config.** Line 397 copies *from* ISR-state *to* main_state — that's a no-op, so any ISR-side EEPROM-config mutation (from DSHOT-PROGRAMMING commands) gets lost on save. The flash write happens but writes whatever `main_state.config` already had.

The motor *spins correctly* despite all this because the throttle → duty → PWM data path goes through `SharedState` atomics (`needs_reset`, `set_newinput`, `set_adjusted_input`, `set_duty_cycle_setpoint`, etc.). All the *control plane* is correctly atomically-shared. Only the *occasional cross-context reads from main → ISR-local fields* are silently broken.

## Why this slipped through

`with_isr_state` looks like it works. Three reasons it's silent:
1. **Signature returns `()`** — no `Result` to ignore, no panic, no obvious failure surface.
2. **The early-boot calls work fine** (pre-`interrupt::enable()` they're safe). Authors test it once during boot, confirm it works, and assume it works in the loop too.
3. **The doc comment understates the trap.** "Access ISR state in a critical section (before interrupts take it)" — the "before interrupts take it" parenthetical is the entire safety story, written in eight characters, easy to miss.

This is exactly the ghost-state failure mode the previous reports flagged: harness has the right code, firmware *appears* to have the right code, both compile, blackbox tests pass (because harness doesn't go through `with_isr_state` at all), and the chip silently misbehaves.

## Fix paths

Three options, increasing invasiveness.

### Option 1 — Hotfix for the desync transfer (small, immediate)

Move `desync_check` to a `SharedState` atomic, the same pattern that `needs_reset`, `save_settings_flag`, `all_off_requested`, etc. already use:

```rust
// rm32/src/shared_state.rs
desync_check_pending: AtomicBool,
```

Inside the ISR (`isr_logic::commutation_timer_expired` at `rm32/src/control/isr_logic.rs:188-`), after calling `commutation.advance()`:

```rust
if commutation.desync_check {
    shared.set_desync_check_pending(true);
    commutation.desync_check = false;  // local clear
}
```

Main-loop's `sync_isr_to_main` becomes:

```rust
pub fn sync_isr_to_main<LED: OutputPin>(
    &self, shared: &SharedState, main: &mut MainState<LED>,
) {
    if shared.desync_check_pending() {
        shared.set_desync_check_pending(false);
        main.set_desync_check(true);
    }
}
```

No more `with_isr_state` for this path. Fixes the immediate symptom (stalled false-sync not detected) and matches the existing pattern.

### Option 2 — Audit and rewrite all six broken call sites

The other five no-ops in the main loop should each be re-classified:
- **Lines 307, 321 (sine PWM apply + changeover):** move into the TIM1-or-similar ISR via SharedState command flags, or restructure so the entire sine-mode handling lives in the ISR and main just sets/clears `stepper_sine` via `shared`.
- **Line 354 (arming beeps):** beep generation needs HAL access. Either move sound generation into the ISR (it already does I/O), or expose a thin "tone request" SharedState channel and let the next ISR pass play one tone-step.
- **Line 397 (save settings: copy `isr.config` → `main_state.config`):** swap the direction — have the ISR-side command processor publish its mutated config via `shared` atomics or a structured `SharedState::config_snapshot` page that main reads.
- **Line 276 (diagnostic):** delete or read via the new shared atomic.

Estimate: half-day audit + per-site change, no API surface churn for outside callers.

### Option 3 — Make `with_isr_state` actually work, or remove it

Make it impossible to call `with_isr_state` post-enable. Either:

(a) **Panic on `None`** — change `if let Some(...)` to `else { panic!("with_isr_state called after first ISR took state"); }`. Loud failure beats silent. But the existing call sites at 276/307/321/345/354/397 would all panic at first use, so this is a "force the audit immediately" approach.

(b) **Make it `unsafe { with_isr_state_in_init(...) }`** and only call it during boot. Move all post-boot calls to the SharedState pattern. This kills the footgun.

(c) **Replace the dual-statics architecture entirely.** Single `Mutex<RefCell<Option<...>>>` shared by both ISR and main. ISRs eat the cost of `interrupt::free` per access (already happens, with_isr_state already does it). Trade: ISR latency might suffer; main can actually touch ISR state correctly. Big refactor but eliminates the bug class.

## Strong recommendation

**Do Option 1 right now** to fix the user-visible desync bug. **Then do Option 3a (panic)** before any more development — turn this from a silent failure into a build-time/early-test failure. If the panic shows up at boot, that's a 60-second find-and-fix, and we never get bitten by this class again.

Option 2's full audit can happen in parallel; each call site can be migrated to SharedState as its panic shows up, or proactively.

## Lessons / process notes

- **Silent no-ops are worse than panics.** When a function can fail invisibly, every caller has to remember the failure mode. They don't. The compiler can't enforce it. Tests against the harness don't exercise it (harness doesn't use `with_isr_state`).
- **Doc comments understating runtime invariants are bug-attractors.** The "(before interrupts take it)" comment is technically correct but the failure mode it implies (silent no-op forever after enable) is invisible from the call site.
- **Architecturally-equivalent-looking code paths in harness vs firmware are the recurring villain in this codebase.** The harness does direct field access; firmware uses `with_isr_state`. The latter looks like field access but is gated on a runtime invariant the harness doesn't have. Every "harness has it, firmware doesn't" bug we've found has had this shape. Option 3 makes them genuinely identical and stops the cycle.
