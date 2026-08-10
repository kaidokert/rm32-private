# Action map: rm32 → AM32 parity on the Vimdrones L431 bench

Objective: bring `rm32`/`rm32_stm32` to the performance parity that `minz/am32_clone`
already demonstrates on this board, carry the minz instrumentation forward as
**optional** parts of the rm32 build, and keep the blackbox vector suite green the
whole way. Vetted on the Vimdrones L431 first; other MCU families later.

Companion evidence: `docs/structure-comparison-balanced/` (module inventories,
crosscut map, audits). This map adds the sequencing and the concrete deltas.

---

## Standing rules (apply to every rung)

1. **One rung per behavioral change**, bench-verified before the next. After every
   bench test: lock map + dropout plot (the minz toolchain). Tag a known-good
   image per rung; a regression is always bisected against the tag, never
   attributed to the bench.
2. **Vector arbitration rule** (how the circles get squared): the vectors emulate
   AM32, and so does minz — so a *true* parity change should keep them green. If a
   parity rung fails a vector, run the same vector against the C harness
   (`build/am32_harness`, AM32-derived). If C agrees with the *new* behavior, the
   vector was pinning an rm32 divergence — regenerate it from C. If C agrees with
   the *old* behavior, the rung is wrong — fix the rung, never the vector.
3. **Compile-out contract**: every new module/feature must compile to nothing when
   off, so the four pre-commit cross-builds (G071/F051/L431/G431) stay green.
   Follow the `debuguart` gating precedent exactly: feature in
   `rm32_stm32/Cargo.toml`, module gate in `lib.rs`, `#[cfg]` at call-sites
   (`rm32_stm32/src/lib.rs:12-36`, `debug_uart.rs:15`).
4. **Firmware-side first**: nothing under `rm32_stm32/` links into `rm32_harness`,
   so instrumentation and wiring changes there cannot move vectors. Portable
   pieces that go into `rm32/src` must be additive (new modules, new defaulted
   traits) and must not change existing `print_state` fields
   (`rm32/src/bin/harness.rs:499-558`).

---

## Phase 0 — Prerequisites (before any new ISR or control change)

### 0.1 Close the open `IsrCell` aliasing finding  *(the report's highest-severity item — still unfixed; `f1face8` only fixed the two minz findings)*
`IsrCell` hands `&mut TargetIsrState` to multiple handlers under an equal-priority
non-nesting assumption (`rm32_stm32/src/isr_handlers.rs:16-69`), but
`mcu_l431/init.rs:245-249` assigns split priorities 0/0/1/2/3 — preemption between
state-touching ISRs is real. Adding a USART2 RX vector without settling this walks
straight into the hazard. Minimum viable fix: audit which handlers actually touch
`ISR_LOCAL`, enforce (statically or by construction) that they share one preemption
level, or split the state so preempting ISRs own disjoint pieces. New bench ISRs
must be ring-push-only (no `ISR_LOCAL` access) regardless.

### 0.2 Baseline + tags
- `cargo build -p rm32 --release`; run vectors on Windows with
  `AM32_HARNESS=target\release\rm32_harness.exe` — confirm 72 vectors green
  (one known failure to triage: `temperature_limit` per the balanced report).
- Build + flash current rm32 L431 image; record its (choppy) behavior with the
  minz map/dropout tooling as the "before" reference. Tag both trees.
- Branch decision (user): continue on `am32_sheet` or cut a fresh branch off
  `main` for the rm32-side work.

---

## Phase 1 — Steal the bench UART control channel from am32_clone

Purpose: drive rm32 with the existing operator toolchain (`uart_cmd.py`,
`fly.py`, sweeps, lock maps) so every parity rung in Phase 2 is measurable the
same way minz was. This is the fastest route to a controlled bench.

New feature: **`benchuart`** (rm32_stm32, L431-only at first).

| Piece | Source in minz | Lands in | Notes |
| --- | --- | --- | --- |
| USART2 RX @2 Mbaud on PA2 (SWAP), byte SPSC ring | `minz/src/usart2_rx.rs`, ring in `minz/core/src/am32.rs` | `rm32_stm32` module + `mcu_l431/interrupts.rs` vector | ISR pushes bytes only — no `ISR_LOCAL` (see 0.1). USART2 is completely unused on L431 today. |
| ASCII duty parser (percent/permille, `s`/`w` stop, `i` info, `Z` trace, `b` blackbox) | `UartDuty` in `minz/core/src/am32.rs:350-442` | portable module (rm32 core, additive) or rm32_stm32 | Host-testable; port minz's parser tests with it. |
| Diagnostic DMA TX ring, 4 KiB, self-healing | `minz/src/uart_tx.rs` | `rm32_stm32`, USART1/PB6 @2 Mbaud | Supersedes 115200 `debuguart` when on (same peripheral/pin). Decide: extend `debuguart` vs sibling feature; either way mutually exclusive with KISS telemetry. |
| Bounded-wait helper w/ cumulative timeout counter | `minz/src/spin.rs` | fold into `rm32_stm32/src/regs.rs::wait_for` | Small, generally useful. |
| Main-loop command band | `am32_clone.rs` foreground bands | new poll band in `bin/main.rs` after the `send_esc_info_flag` band (~line 449) | Drains ring → parser → throttle input. |

**Pin/feature exclusivity (hard constraints, confirmed in code):**
- PA2 is unconditionally TIM15 DShot capture (`mcu_l431/input_capture.rs`). Under
  `benchuart`, skip input-capture init/DMA1_CH5/EXTI15_10 arming and feed
  throttle from the parser instead. PA2 UART and DShot cannot coexist.
- PB6/USART1 serves KISS telem *or* debuguart *or* bench TX — one owner per build.

**Throttle injection point** needs a design decision during implementation: the
parser's output must enter the same shared-state path DShot decode uses (set
`input`, `input_set`, arm) so everything downstream (arming, ramp, LVC) behaves
identically. Signal-timeout/deadman: reuse minz's 3-second UART deadman semantics
so a dead bench script zeros throttle.

Deliverable: rm32 image on the bench, motor controllable from `uart_cmd.py`/
`fly.py`, baseline lock map recorded.

## Phase 2 — Control parity rungs (the core of the job)

Ranked by expected effect on the mid-throttle chop; each is one rung with
bench + vector verification. All cite both sides.

1. **Commutation advance: rm32 runs 0°, minz/AM32 run ~15°.** `temp_advance`
   defaults 0 (`rm32/src/control/state.rs:440`) and firmware never seeds it from
   `advance_level`; minz fixes `TEMP_ADVANCE=16` → commutate at `ci/2 − ci/4`
   (`minz/core/src/am32_loop.rs:34`). rm32 commutates a full 15° late. Fix: map
   `advance_level → temp_advance` exactly as AM32's `loadEEpromSettings` does.
   Small change, likely the biggest single win. Check whether the harness already
   maps it (if so, vectors already encode the correct behavior).
2. **COMP acceptance gate + pending-bit camping.** minz gates on
   `interval.count() > average_interval/2` and *camps* on a post-ZC pending edge
   until the gate opens, never masking on rejection
   (`minz/core/src/am32_isr.rs:43-59`). rm32 clears **and masks** EXTI at ISR
   entry (`mcu_l431/interrupts.rs:44-49`) and has no gate
   (`rm32/src/control/isr_logic.rs:281-298`) — a noise edge either commutates
   early or masks the comparator for the rest of the window so the true ZC is
   lost. This is the clearest AM32-parity mechanism rm32 lacks and matches the
   BEMF re-lock failure signature. Note: the mask-at-entry was itself a fix for
   the COMP-storm freeze — the gate must land together with the camp policy so
   noise edges are still bounded (gate-closed + pre-ZC level → clear; that is
   what keeps the storm away in minz/AM32).
3. **Strict polling/interrupt exclusivity + polling fallback on slowdown.**
   minz: comparator interrupts are only enabled outside `old_routine`
   (`am32_isr.rs:122-124`), startup is polling-only (`am32_control.rs:159-163`),
   and interrupt mode falls back to polling when
   `average_interval > changeover+500` (`am32_control.rs:72-74`). rm32 enables
   the comparator at startup (`isr_logic.rs:87`) and unconditionally each
   commutation (`isr_logic.rs:250`) so both BEMF paths run concurrently, and has
   no slowdown fallback (`motor_mode.rs:80-85`). Also adopt minz's startup seeds
   (`ci=10000`, interval counter 5000, `am32_control.rs:165-180`).
4. **Recovery authority.** Desync: minz kicks duty down to `MIN_STARTUP_DUTY/2`
   (`am32_control.rs:284`); rm32 doesn't (`rm32/src/main_state.rs:334-361`).
   BEMF timeout: minz actively re-commutates via `zcfoundroutine`
   (`am32_control.rs:240-261`); rm32 recovers passively
   (`main_state.rs:306-324`).
5. **Confirmation thresholds / EEPROM-derived constants.** rm32's compiled
   defaults are weaker than the L431 factory values minz hard-codes:
   `min_bemf_counts` 4/2 vs 6/3, `bad_count_threshold` 2 vs 3, `minimum_duty` 5
   vs 45, `min_startup` 120 vs 145, `startup_max` 200 vs 445
   (`state.rs:582-600, 439` vs `am32_loop.rs:22-53`). Verify what the flashed
   EEPROM actually supplies at runtime and that the derivation chain reproduces
   AM32's `loadEEpromSettings` numbers; fix the derivation, not the defaults,
   where possible.

Non-gaps confirmed (don't touch): interval blend arithmetic is identical; ramp
rates/selection identical; filter map identical; desync predicate identical;
variable-PWM mode-1 map identical (verify `variable_pwm` is on in bench EEPROM);
the opposite comparator polarity conventions are internally consistent on both
sides.

Exit criterion for Phase 2: rm32 lock map and dropout tail at-or-better vs the
archived am32_clone maps, back-to-back on the same bench session.

**Inherited open items (not transplant regressions).** The 2026-07-24 bench
study (artifact "am32_clone vs AM32 — side-by-side bench study",
`913684ef-178c-457b-9bda-5228e311d77a`) shows am32_clone itself is at parity at
the top end (2315 vs 2335 Hz @100%) and *better* on steady-state quality (half
the excursion rate, 4.9 vs 9.8 per 1k), but documents three open deltas vs
AM32. A Phase 2 bench comparison of rm32 against **AM32** will show these too —
they come *with* the transplanted behavior and must not be misread as
transplant failures. Judge the transplant against **am32_clone**, and track
these separately:

1. **Low-throttle speed deficit**: 389 vs 528 Hz at 10% (ratio 0.74),
   converging to 1.00 by ~60%. *Round 2 found the mechanism*: the changeover
   code is verbatim-correct, but at 3–4% AM32's comparator lock repeatedly
   fails at low BEMF, hits the 22.5 ms BEMF timeout, and cycles back to
   polling (86–100% polling residency) — polling-mode commutation dynamics
   give it more low-end speed. The clone *holds* interrupt-mode lock to the
   floor (99%+ running at 4%) at a speed cost. Open **decision**, not a bug:
   chase AM32's polling-residency behavior or accept held-lock. Whatever is
   decided applies identically to rm32 post-transplant.
2. **Down-slam vbat kill — the guard was right** (Round 2 reclassification).
   With the guard retuned transient-proof (floor 5.95→5.5 V + 10 ms
   consecutive-tick debounce), the from-100% down-slam *still* kills: the bus
   genuinely sits at ~5.1 V for tens of ms during the clone's deceleration.
   Not a guard-tuning issue — a real decel-event mechanism, linked to item 3.
   Slams from ≥90% stay off-limits on this PSU until the decel mechanism is
   fixed. Port the retuned guard (5.5 V + debounce) as-is in Phase 3.4.
3. **Down-step braking 2× faster than AM32** (60→10%: 177 ms vs 342 ms).
   Round 2 *eliminated the ramp table* as the cause (AM32 uses the same
   global 2/6/16 rates, `max_ramp=160` is a no-op, `ramp_divider=0` both
   sides). Mechanism open; next probe is the per-commutation `duty(t)` column
   in the existing step CSVs (pipeline vs braking-physics split).

Bench context for all of the above: single PSU, single motor, **prop mounted**
(not a no-load bench).

## Phase 3 — Port the instrumentation as optional features

Ordered so each tool is available to debug the next; several can land early if
Phase 2 needs them (blackbox and ZC trace are the debugging eyes — consider
pulling 3.1/3.2 ahead of the harder Phase 2 rungs).

1. **`blackbox`**: portable 64-event freeze-on-fault ring
   (`minz/core/src/blackbox.rs`) → new additive module in rm32 core (keeps its
   host tests); DWT-timestamp adapter (`minz/src/bb.rs`) → rm32_stm32,
   `cfg`-gated to M4 targets; drain band + `b` command. Event set: start with
   minz's REF/ACC/DSY plus rm32-specific events (mode transitions,
   IsrAction, desync source).
2. **`zctrace`**: 15-byte per-commutation stream + 50-on/50-off batching
   (`minz/core/src/zct_trace.rs`, record packing in `am32.rs:268-301`) →
   producers in `handle_tim14`/`handle_comp`, drain band, `Z` toggle. Keep the
   wire format byte-identical so `zctrace_capture.py` and the plot suite work
   unchanged.
3. **Injected ADC burst** (phase A/B + current + vbat, PWM-synchronous,
   `minz/src/adc_sync.rs`): L431 feature; reconcile with rm32's existing `Adc`
   trait rather than adding a parallel path where possible. Carry the
   dormant regular-ADC/DMA config initially (empirically bench-load-bearing;
   see memory) and schedule a later experiment to drop it.
4. **Hard OC/UV kill + latched-kill semantics** (post-`f1face8`): decide whether
   rm32's LVC/`IsrAction::AllOff` path already covers this or whether the raw
   windowed-current kill is worth porting as a bench safety net.
5. **Watchdog**: minz runs IWDG at ~1 s; rm32's start is commented out
   (`bin/main.rs:124`). Re-enable (bench feature first, default later).
6. **RAM budget**: 48 KiB SRAM1; the two 4 KiB rings ≈ 17% on top of existing
   state. Fine on L431, `cfg` them off M0 parts; SRAM2 (16 KiB, currently
   unlinked) is the escape hatch if pressure appears.

Feature naming proposal: `benchuart`, `blackbox`, `zctrace` individually, plus a
convenience `bench = ["benchuart", "blackbox", "zctrace", "debuguart"]`.

## Phase 4 — Switch validation to Betaflight / DShot

Once parity holds under bench UART: build without `benchuart` (PA2 back to
TIM15 capture, PB6 back to KISS telem), re-verify parity under DShot600 from BF,
then burn down the backlogged DShot items: buf[0] alignment (WIP `2b92824`),
DSHOT150 detection, self-validating bidir auto-detect, GCR response validation.
Blackbox + zctrace remain available (they don't use PA2) — that's the payoff of
making them independent features.

## Phase 5 (later) — other MCU families

Propagate: the COMP pending-bit/gate policy per family (F051/G071/G431 COMP ISR
wrappers still have the pre-existing early-return ack bug noted in CLAUDE.md),
the priority fix, and whichever instrumentation each family's RAM allows.
Out of scope for this effort's couple-day horizon.

---

## Open decisions (need user input, none block Phase 0)

1. Branch for the rm32-side work (`am32_sheet` vs fresh off `main`).
2. `debuguart` vs bench DMA TX: extend the existing feature to 2 Mbaud DMA, or
   keep two mutually exclusive features.
3. Where the ASCII parser lives (rm32 core as additive module — recommended, it
   is host-testable — vs rm32_stm32).
4. Whether minz stays frozen as the reference implementation until Phase 2 exits
   (recommended: yes, it is the known-good against which every rung is judged).
