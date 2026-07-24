# ISR state aliasing invariant (rung 0.4 audit, 2026-07-24)

## Finding

`IsrCell` (`rm32_stm32/src/isr_handlers.rs:12-69`) hands out `&mut
TargetIsrState` from a `static`, with `unsafe impl Sync` justified by the
documented invariant that **all callers share one NVIC priority level** so
they cannot preempt each other (isr_handlers.rs:19-21).

**That invariant is violated on L431 today.** Since the NVIC priority fix
(`mcu_l431/init.rs:237-250`), the six `ISR_LOCAL.get()` call sites run at
four different preemption levels:

| Handler | `ISR_LOCAL.get()` line | L431 vector | Priority |
|---|---|---|---|
| `handle_comp` | isr_handlers.rs:129 | COMP | 0 |
| `handle_tim14` | isr_handlers.rs:107 | TIM1_UP_TIM16 | 0 |
| `handle_dma_tc` | isr_handlers.rs:150 | DMA1_CH5 | 1 |
| `handle_exti_frame` | isr_handlers.rs:180 | EXTI15_10 | 2 |
| `handle_tim6` | isr_handlers.rs:82 | TIM6_DACUNDER | 3 |
| `handle_crsf_byte` | isr_handlers.rs:371 | (no vector wired) | — |

COMP preempting TIM6 mid-tick is the *design intent* of the priority split
(the BEMF ISR must not wait behind the 20 kHz housekeeping tick). Every such
preemption creates two live `&mut TargetIsrState` — undefined behavior by
Rust's aliasing rules. G071/F051 (no preemption sub-priorities configured the
same way) and the harness (no ISRs) are unaffected.

Observed practical exposure: the handlers touch mostly-disjoint parts of the
state (COMP: bemf/com timer; TIM6: duty/config/transfer), which is why the
bench hasn't visibly corrupted state — but "works so far" is not soundness,
and LLVM is entitled to miscompile on aliasing UB. This is the balanced
structure report's highest-severity finding; still open as of `f1face8`.

## Standing rule (binding for all new code)

**No new ISR may touch `ISR_LOCAL`.** A new interrupt handler (e.g. the
rung-1 `benchuart` USART2 RX) must be ring-push-only: write to a dedicated
lock-free SPSC ring or `SharedState` atomics and return. Parsing/consumption
happens in the main loop. This keeps the new vector outside the aliasing
question entirely, at any priority.

## Fix options for the existing violation (own rung, bench-verified)

1. **Split the state by owner** (preferred): partition `TargetIsrState` into
   per-priority-level cells (COMP+TIM16 state / DMA+EXTI state / TIM6 state),
   each with its own single-level invariant; cross-level communication via
   `SharedState` atomics only. Matches minz's borrowed-atomic-cluster shape.
2. **Priority-mask critical sections**: keep one cell but raise BASEPRI to
   the highest state-touching priority for the duration of each handler's
   state access. Costs COMP latency — likely unacceptable for the BEMF path.
3. **Per-field atomics**: migrate hot shared fields to atomics (minz style).
   Most invasive.

Until one lands, any change to what the handlers touch must re-check the
disjointness by hand. Do not add fields accessed from more than one priority
level.
