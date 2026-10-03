# AM32 G071 — reference disassembly of the three critical ISRs

Source-interleaved disassembly (`objdump -d -S -l`) of the AM32 G071 build for this bench,
with each handler's full static call tree. Regenerate with
`python scripts/am32_isr_dump.py` (from `binz/`).

## Provenance

| | |
|---|---|
| ELF | `E:/m/robot/esc/AM32/obj/AM32_DRV8304H_G071_2.20.elf` |
| sha256 | `02592720fb8a1dca2bea0f2915bed553fe3399eca2dbe8e541d2d5804d4ab2fc` |
| AM32 commit | `b4d28af` (2026-08-22) **plus uncommitted local edits** (below) |
| Build | `-O3 -g3 -ffunction-sections -fomit-frame-pointer -ffast-math`, arm-none-eabi GCC |
| Freshness | ELF 2026-09-14 08:37:09 is newer than every modified source (latest `main.c` 08:37:03) |

**Local edits (`git diff` in the AM32 tree), all bench scaffolding for the NUCLEO-G071RB + DRV8304H target:**
* **Untouched:** `interruptRoutine`, `PeriodElapsedCallback`, `commutate`, `zcfoundroutine`,
  `ADC1_COMP_IRQHandler`, `TIM14_IRQHandler`.
* `tenKhzRoutine`: a UART-throttle deadman (2 lines at the top: counter increment + `bench_stop()`
  on timeout). Everything else in it is stock.
* `SET_DUTY_CYCLE_ALL` → `bench_pwm_all()`, which **caps duty at 30 % of ARR** ("Operator's hard
  30% ceiling" — the invented target recorded as a scar in `binz/CLAUDE.md`). This inlines into
  the duty-setting paths. **This oracle image cannot drive above 30 % as built.**
* EEPROM defaults are hard-coded (no flash read); `setInput` takes UART duty; HardFault drops
  ENABLE and MOE.

## The three ISRs

| file | root | role | reached functions |
|---|---|---|---|
| `comp_ADC1_COMP_IRQHandler.lst` | `ADC1_COMP_IRQHandler` | BEMF comparator edge (EXTI 18) | `getCompOutputLevel`, `interruptRoutine`, `maskPhaseInterrupts` |
| `com_TIM14_IRQHandler.lst` | `TIM14_IRQHandler` | COM_TIMER update | `PeriodElapsedCallback`, `commutate`, `changeCompInput`, `comStep`, `enableCompInterrupts` |
| `tenkhz_TIM6_DAC_LPTIM1_IRQHandler.lst` | `TIM6_DAC_LPTIM1_IRQHandler` | 20 kHz control loop | `tenKhzRoutine` + divides, tune, polling-mode `zcfoundroutine`, … |

`call_trees.txt` lists every reached function with instruction counts and sizes.
Instruction counts are **static** (all paths), not executed-path counts.

## What the comparator path actually does (the parity facts)

1. **`ADC1_COMP_IRQHandler`** (`Mcu/g071/Src/stm32g0xx_it.c:241`), for whichever EXTI 18 edge flag is set:
   * **half-interval gate:** `INTERVAL_TIMER->CNT > average_interval >> 1`. If it passes, clear the
     flag and call `interruptRoutine()`.
   * **early edge:** otherwise, clear the flag only if `getCompOutputLevel() == rising`, then return.
2. **`interruptRoutine`** (`Src/main.c:945`):
   * **filter:** `filter_level` reads of the comparator output. Any mismatch returns without
     acking, re-entering on the next edge. `filter_level` is 2 at high speed, `map(…, 3, 12)` in the
     mid band and 12 at low speed (`main.c:2632-2637`).
   * **accept and arm:** `__disable_irq`; `maskPhaseInterrupts()`; `lastzctime = thiszctime`;
     `thiszctime = INTERVAL_TIMER->CNT`; `INTERVAL_TIMER->CNT = 0`;
     `SET_AND_ENABLE_COM_INT(waitTime + 1)` (G071: `CNT = 0, ARR = time, SR = 0, DIER.UIE = 1`);
     `__enable_irq`.
   * **No arithmetic.** There is no divide, no advance computation and no interval blend in the
     comparator ISR.
3. **`waitTime` is precomputed in the commutation ISR.** `PeriodElapsedCallback` (`main.c:923`)
   runs after `commutate()`:
   * `commutation_interval = (ci + (last + this) / 2) / 2`;
   * `advance = ci * temp_advance >> 6`;
   * `waitTime = ci / 2 - advance`;
   * then `enableCompInterrupts()`.

   So the delay armed at the next crossing is computed **one step ahead**, from intervals already
   seen. The arm is **relative** (counter zeroed at the write). The comparator ISR's latency lands
   in the commutation delay as a near-constant offset, which advance absorbs.

For firmware50 parity, the comparator root's work between the crossing and the arm should be:
the gate compare, the filter reads, one timer read, one timer write and the arm. Everything else
(interval blend, advance, `waitTime`) belongs in the commutation root, where AM32 does it.
**That is AM32's placement, not a relocation of firmware50 work.** Any work AM32 does not do at
all gets deleted.
