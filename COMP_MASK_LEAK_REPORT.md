# COMP IRQ left unmasked outside commutation — latent since the port, exposed by NVIC priority fix

## Summary

rm32 leaves `EXTI.IMR1[22]` (the COMP2 interrupt-enable bit on L431; corresponding lines on other MCU families) unmasked anytime the motor is in `Armed` or `Disarmed`, and across most stop-path transitions (stuck rotor, desync, signal timeout). AM32 explicitly masks COMP at every stop/timeout site — `maskPhaseInterrupts()` is called from **~15 places** in `main.c`.

While NVIC priorities were also wrong (a separate bug where cortex-m's `set_priority` writes the raw byte but STM32L4 expects priority in the upper nibble, so all rm32 priorities collapsed to level 0), this COMP bug was **latent**: same-priority tail-chaining on Cortex-M serialized COMP IRQs with TIM6, so comparator noise on undriven BEMF pins couldn't starve TIM6.

When NVIC priorities were fixed to honor AM32's structure (COMP=0, TIM6=3 — the AM32 priority assignment), the latent bug **immediately freezes the firmware in seconds**: comparator output bouncing on noisy undriven BEMF inputs storms COMP_IRQ, preempts TIM6 indefinitely, and `dbg_isr_tick` stops advancing. Chip alive but main-loop-dead.

The NVIC priority fix and the COMP mask fix are **a single unit** — one without the other is broken. **Same companion fix is needed when porting NVIC priorities to G071/F051/G431.**

## Evidence

### AM32 — explicitly masks at every stop site

`AM32/Mcu/l431/Src/peripherals.c:272-273` — boot init masks COMP IRQ:

```c
LL_EXTI_DisableEvent_0_31(LL_EXTI_LINE_22);
LL_EXTI_DisableIT_0_31(LL_EXTI_LINE_22);
```

`AM32/Src/main.c:941` — `startMotor()` unmasks:

```c
void startMotor() {
    if (running == 0) {
        commutate();
        commutation_interval = 10000;
        SET_INTERVAL_TIMER_COUNT(5000);
        running = 1;
    }
    enableCompInterrupts();   // <-- only here, when actually commutating
}
```

`AM32/Src/main.c` — `maskPhaseInterrupts()` call sites:

```
Line  925: between commutation steps inside zcfoundroutine
Line  994: bidir direction-change StopMotor path
Line 1008: bidir direction-change StopMotor path (mirror)
Line 1070: throttle reverse direction change
Line 1084: throttle reverse direction change (mirror)
Line 1104: BEMF stuck-rotor timeout (allOff branch)
Line 1380: old_routine + running BEMF polling start (synchronization)
Line 1788: brushed-mode startup tune
Line 2067: sine-mode entry
Line 2128: sine-mode active
Line 2148: sine-mode start condition
Line 2161: sine-mode reset
Line 2189: sine-mode direction change
```

The rule encoded in AM32 across these sites: **COMP IRQ is only unmasked during active commutation. Every motor stop, every mode change, every timeout re-masks.**

### rm32 — only one stop path masks

`rm32/src/control/isr_logic.rs:24` (inside `ten_khz_tick`):

```rust
match ctx.shared.isr_action() {
    crate::shared_comm::IsrAction::AllOff => {
        ctx.hal.phase().all_off();
        ctx.hal.comp().mask_interrupts();   // <-- only mask path
    }
    ...
}
```

`IsrAction::AllOff` is only requested from the **LVC** (low-voltage cutoff) path in `rm32/src/main_state.rs:420`. That is the only stop site that masks COMP.

Other stop sites in `main_state.rs`:
- Line 313: stuck rotor detected → `shared.transition(MotorEvent::StopMotor)` — **no comp mask**
- Line 353: desync detected → `shared.transition(MotorEvent::StopMotor)` — **no comp mask**
- Line 376: signal_timeout while armed → `shared.transition(MotorEvent::Disarm)` — **no comp mask**

State machine transitions are decoupled from HAL state. Transitioning to `Armed` or `Disarmed` does not require masking COMP because the trait doesn't enforce it.

`rm32_stm32/src/mcu_l431/comp_init.rs:65` correctly masks COMP at boot:

```rust
exti.imr1.modify(|r, w| w.bits(r.bits() & !(1 << 22)));
```

So COMP starts out masked. But any of the unmask paths (`comp.enable_interrupts()` called in `isr_logic::ten_khz_tick` line 69 on motor start, and in `commutation_timer_expired` line 232) leaves it unmasked indefinitely if the motor subsequently stops via any path other than LVC.

### Hardware observation — firmware freeze with NVIC priorities fixed, COMP mask bug present

After flashing the NVIC priority fix without the companion COMP mask, on the Vimdrones L431 KCU6 bench:

- Chip boots normally.
- Runs for ~13 seconds in `Armed`-idle (after BF starts sending PWM frames and arming completes).
- `dbg_isr_tick` freezes at `0x3f2c7` = 258,759 = ~12.95 s uptime. No more progress.
- Main-loop log (`port_41.log`) stops emitting `[loop n=]` lines.
- Chip is still electrically alive — `probe-rs read` succeeds, SWD responsive.

Diagnostic state at freeze:

```
ISPR1 bit 22 set         → TIM6 IRQ pending, never serviced
ISPR2 bit 0  set         → COMP IRQ pending
EXTI.PR1[22]   = 1       → COMP line pending (cleared in ISR wrapper, but re-pending immediately on next edge)
EXTI.IMR1[22]  = 1       → COMP line UNMASKED (the bug)
COMP2_CSR      = 0x46000071  → COMP2 ON, VALUE bit set (output high; will flip on any noise)
EXTI.RTSR1[22] = 1       → rising-edge sensitive
NVIC priorities          → COMP=0x00, TIM1_UP_TIM16=0x00, DMA1_CH5=0x10, EXTI15_10=0x20, TIM6=0x30 (all correct)
```

GDB backtrace at halt:

```
#0  ... (deep in handle_comp / bemf_zero_cross)
#4  __cortex_m_rt_COMP at mcu_l431/interrupts.rs:39
#13 __cortex_m_rt_EXTI15_10 at mcu_l431/interrupts.rs:71
#24 __cortex_m_rt_TIM6_DACUNDER at mcu_l431/interrupts.rs:14
#28 wfi() at main.rs:456
```

Priority chain: COMP (level 0) preempted EXTI15_10 (level 2) which had preempted TIM6 (level 3). This preemption order is *correct* given the priorities — but COMP fires on comparator noise faster than TIM6 can complete a single tick, so TIM6 never makes forward progress, dbg_isr_tick is frozen, and main loop never gets CPU.

## Fix applied

`rm32_stm32/src/mcu_l431/interrupts.rs::COMP()` now masks `EXTI.IMR1[22]` on every ISR entry alongside the existing PR1 clear:

```rust
let exti = unsafe { &*pac::EXTI::PTR };
unsafe {
    exti.pr1.write(|w| w.bits(1 << 22));
    exti.imr1.modify(|r, w| w.bits(r.bits() & !(1 << 22)));
}
isr_handlers::handle_comp();
```

`commutation_timer_expired` already calls `comp.enable_interrupts()` at line 232 of `isr_logic.rs`, which re-unmasks `IMR1[22]` for the next BEMF detection window. So normal commutation flow is preserved: COMP fires once per phase, the wrapper masks itself, the next `commutation_timer_expired` re-enables COMP.

Defense-in-depth: `rm32/src/control/isr_logic.rs::ten_khz_tick` calls `ctx.hal.comp().mask_interrupts()` at the top of every tick when `!running`. This catches any state where the per-fire ISR mask isn't enough (e.g., transient between unmask and next stop transition).

After the fix: bench survives full throttle sweeps. `dbg_isr_tick` advances at steady 21 kHz indefinitely.

## How this slipped through testing

### The hardware-level interaction has no representation in the harness

The blackbox test harness models a `MockComp` that records calls and returns deterministic comparator output. Real comparator behavior — output bouncing on a floating input pin in the presence of electrical noise — does not exist in the harness. A scenario like "what if the comparator is firing 100k times per second on noise" is not constructible with the current mock.

The harness was designed to validate **behavioral parity** (state transitions, HAL call sequences). Hardware-level transient behavior (noise, edge glitching, register-storm) is **out of scope by design**. We do not have a fault-injection layer.

### The type system doesn't enforce the invariant

`Comparator::enable_interrupts(&mut self)` and `Comparator::mask_interrupts(&mut self)` are independent trait methods. Nothing in the type signature says they must be paired or that `enable_interrupts` is only callable from a "commutating" state.

The HAL trait directly mirrors C's `enableCompInterrupts()` / `maskPhaseInterrupts()` — same shape, same gap.

A typestate version would prevent the bug class entirely:

```rust
pub struct Comparator<S: CompState> { ... }
pub struct Masked;
pub struct Unmasked;

impl Comparator<Masked> {
    pub fn enable_interrupts(self) -> Comparator<Unmasked> { ... }
}
impl Comparator<Unmasked> {
    pub fn mask_interrupts(self) -> Comparator<Masked> { ... }
}

// Motor state machine would be parameterized over comparator state:
fn stop_motor(comp: Comparator<Unmasked>) -> (MotorState<Stopped>, Comparator<Masked>) {
    let masked = comp.mask_interrupts();
    (MotorState::stopped(), masked)
}
```

Then "stop the motor without masking COMP" wouldn't compile — the `Comparator<Unmasked>` would have to be converted to `Masked` before the state transition could return. Rust gave us no extra safety here because we copied the C trait shape verbatim instead of using the type system to encode the protocol.

### The NVIC priority bug hid this

Before the priority shift was fixed, all IRQs ran at level 0. Cortex-M same-priority IRQs don't preempt — they tail-chain after one completes. So COMP firing fast on noise would queue up but TIM6 would still get its 50 µs slot. The COMP storm was invisible.

The only signal would have been "BEMF detection slightly worse than AM32" — and that was buried in the larger noise of an in-progress port with many other ongoing issues (DSHOT alignment, bidir detect, register parity).

### COVERAGE_GAPS.md doesn't list it

The coverage audit focused on behavioral coverage (does the code path exist and get exercised). There is no entry for "COMP is masked on every motor stop". HAL-state invariants were not part of the audit's scope.

## Latent in other MCU families — G071, F051, G431

G071, F051, G431 use the same `BemfComparator` / `isr_logic` / `main_state` code paths. The COMP ISR wrappers in:

- `rm32_stm32/src/mcu_g071/interrupts.rs`
- `rm32_stm32/src/mcu_f051/interrupts.rs`
- `rm32_stm32/src/mcu_g431/interrupts.rs`

do **not** mask after entry. The bug exists on all of them.

The observable symptom (firmware freeze) only manifests when:

1. NVIC priorities are correctly set (so COMP can preempt TIM6/the control timer)
2. Comparator inputs are noisy enough to bounce while not commutating

On L431 both conditions are met as of the priority fix commit. On other families, the NVIC priority code may currently also be broken (have not audited). **Fixing the priority bug on those targets WITHOUT also masking COMP in the wrapper will reproduce the L431 freeze.**

**When porting the NVIC priority fix to G071/F051/G431: port the COMP ISR wrapper mask at the same time. They are a single unit.**

## Recommended testing improvements

### (a) HAL-invariant test: mask-on-stop

For each state transition that ends commutation (`StopMotor`, `Disarm`, stuck rotor latch, LVC), assert in the harness that `mask_interrupts_count` strictly exceeded the previous value, and that the comparator is in the masked state afterward (would require exposing mask state on `MockComp`).

Today only the `IsrAction::AllOff` (LVC) path calls mask. The other stop paths should too — and a test asserting this would have caught the bug.

### (b) Typestate refactor for Comparator

Convert the `Comparator` HAL trait to typestate (sketch above). `enable_interrupts` consumes `Comparator<Masked>` and returns `Comparator<Unmasked>`. Motor state transitions that leave commutating-state must produce a `Comparator<Masked>`.

This is a real refactor — touches every MCU comparator impl, every site that holds a comparator, the motor state machine. But it makes the entire bug class **non-representable** at compile time.

Other HAL traits where the same pattern likely applies and similar latent bugs may exist: phase output (must be all-off in stopped state), commutation timer (must be in known state on stop). Worth auditing them with the same lens.

### (c) Hardware fault-injection layer in the harness

Add a `MockComp` mode that fires comparator out-of-band noise: random EXTI line 22 events at high rate when the harness simulates "motor coasting on undriven phases". Test that the firmware survives this.

Would also catch other ISR-storm bug classes — same harness extension would help with input pin glitching, telemetry collisions, DMA TC under back-pressure, etc.

Of the three: (a) is cheapest, (b) is most rigorous, (c) is most general. Recommend doing (a) immediately and queuing (b) and (c).

## Severity

- **Correctness when latent**: nothing observable. The bug was silently present from the day priorities were misconfigured.
- **Correctness when active**: firmware freeze in seconds. Chip alive, main-loop dead. Requires power cycle (or NRST, which is also marginal on this bench — see CLAUDE.md notes on the bootloader reset semantics).
- **Discovery story**: not found by testing. Found by attempting to flash the NVIC priority fix and observing immediate post-flash freeze, then GDB-attaching to inspect the preemption chain.
- **Other MCU families**: ticking time bomb pending their NVIC priority fixes.
