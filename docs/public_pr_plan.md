# Public PR landing plan (semiclean → origin/main)

Merged from two independent slice reviews (rm32/ core, rm32_stm32/
firmware) of the staged set `9178056..semiclean`. Companion to
`public_cleanup_tiers.md`. Every PR gates on: 275 host tests + 4-target
production cross-build + L431 × each debug-feature combo it touches;
bench validation where drive behavior can move.

## Phase 0 — prerequisites (before splitting anything)

- Commit or reset the clean worktree (currently dirty on `semiclean4`).
- Fix the three build-matrix breaks so every PR's CI can be green:
  - `debuguart` fails to compile on G071/F051/G431 (L431 PAC syntax in
    `debug_uart.rs`) → `compile_error!` gate like benchuart's, or port.
  - `zctrace` fails on non-L431 (`isr_handlers.rs` calls
    `comp_at_pre_zc_level()` under bare `cfg(zctrace)`) → tighten cfg.
  - `Cargo.toml` declares `[[example]] bringup`/`bringup_pac` but the
    files don't exist → `cargo check --examples`/`cargo test` fail.
    Add the examples or drop the manifest entries.
- **A5 (NEW, Tier-A severity): bench_guard enforced in ALL production
  M4 builds with bench thresholds.** Battery profile kills latched at
  14.0 V OV / 15 A OC — a 4S pack trips OVOLT within ~60 ms and the ESC
  never runs until reset. Either derive thresholds from board/config
  (cell count) or gate enforcement under `benchuart`.

## Phase 1 — foundations

**PR-1: Board-YAML BEMF pin configuration** (~450 lines + core
`board.rs` BemfPins). Fixes the original port's wrong comparator mux
(DAC channels selected on L431); pin selection becomes a required
per-board YAML field resolved in build.rs; L431 INMSEL/INMESEL packing
+ SYSCFGEN fix. All 14 yamls. Hold the Vimdrones sense recalibration
(voltage_divider 933 / mv_per_amp 3 / offset 0) as its own commit —
bench-measured, not vendor data. Extend build.rs to emit per-chip
`memory.x` (Tier B7) here or as PR-1b — prerequisite for calling
non-L431 builds supported (F051 has 8K RAM; current memory.x says 48K).

**PR-2: IsrAction main→ISR channel + AllOff safety parity** (~150
lines, core). Priority-ordered atomic action channel (fetch_max, AllOff
highest); mask_interrupts when !running; harness HalCounts via Rc<Cell>
+ alloff/fullbrake/mask counters in state output. Foundation for PRs
6/8/9. Vector-testable.

**PR-3: L431 register-parity init + NVIC priorities + IWDG** (~350
lines, fw). AM32-matched clock tree, TIM1/TIM6/TIM7, GPIO parity, the
NVIC `level << 4` fix + priority ladder, IWDG LSI/KR-order fix,
`write_tim1_ccr`. Cite as follow-ups in the PR: G431 boots with ALL
priorities at level 0 today (the exact config whose fix was the L431
chop breakthrough) — same for G071/F051.

**PR-4: Commutation timer semantics** (~50 lines, fw `timer.rs`).
Free-running COM timer, AM32-verbatim `set_and_enable`, ARPE-off
divergence (UG/UIF clock-domain race), TIM2 ARR 16-bit. Small,
behavior-critical, own PR so the ~30°-advance race story reviews alone.
Fix the three contradictory comment eras first (struct doc still claims
ARPE|CEN).

## Phase 2 — control-law parity

**PR-5: 1 kHz dispatch rate parity** (~250 lines, core; needs PR-2).
ADC/3 PIDs/LVC moved from 20 kHz to AM32's 1 kHz (LVC cut in 0.5 s
instead of 10 s); ISR-side counter at AM32's placement; LVC mode 2.
4 new LVC tests.

**PR-6: AM32-verbatim control-law fixes** (~200 lines, core).
duty_ceiling verbatim (integer 32/poles + floor 400 anti-runaway),
static-advance publication (rm32 ran 0° at factory settings),
min_bemf_counts 3/6, bad_count/wait_time saturation. Tested.

**PR-7: Commutation engine + changeover parity** (~220 lines, core;
after PR-2 textually). Mid-run polling demotion, comp-enable
exclusivity, ci-only polling→interrupt changeover (the engage lottery),
startup seeds (ci=10000/interval=5000), bemf_zero_cross → bool + EXTI
pre-ack contract.

**PR-8: ISR pending-bit hygiene + COMP acceptance gate** (~250 lines,
fw; needs PR-7's bool). Non-L431 COMP pre-ack, L431 CGIF5 hygiene,
half-avg gate + camp + SWIER race fix, **comp_gate.rs lands HERE — it
is control, not instrumentation** (zctrace-free build failed to lock
below 40% without it). Note in PR: DMA CGIF hygiene + gate architecture
still L431-only on the other three families.

**PR-9: Desync detection + stall recovery parity** (~350 lines, core;
needs PR-2/PR-7). Behavior-not-intent desync trip (no avg=5000 reset —
the dead-code story), DutyKickDown/CommutateKick actions, zcfoundroutine
ci-fold, run_tick unification (kills the harness/firmware orchestration
divergence class).

**PR-10: Signal timeout + needs_reset + firmware reset** (~120 lines,
cross-crate). Armed/unarmed split, needs_reset flag + tests; firmware
side: sys.reset() with debuguart flush, benchuart suppression (already
Tier-A-gated).

## Phase 3 — input pipeline

**PR-11: Input detection robustness** (~140 lines, core signal.rs).
u16-wrap fix (TIM15 is 16-bit), stale buf[0] skip, DSHOT150 bucket,
33-slot layout. Regression tests carry real bench capture values.

**PR-12: CaptureConfig + protocol confirmation + bidir self-validation**
(~1100 lines, cross-crate; needs PR-11). Core transfer.rs (2-frame
re-confirmation, dynamic alignment, bidir commit only after N
inverted-CRC successes, servo-cal persistence) + fw ISR-state lifecycle
(DMA armed post-move), handle_exti_frame → CaptureConfig, EDT throttle
gate removal, set_comp_pwm wiring, wide frametime bounds. Validate: BF
DSHOT300/600 + Configurator passthrough on bench.

## Phase 4 — the chosen divergences

**PR-13: Kept divergences** (~400 lines, cross-crate; needs PR-5/PR-9).
Stay-interrupt fast-rotor + DutyKickHalf, rearm holdoff, reclimb clamp,
orbit trip (vbat-scaled), decision counters. Land WITHOUT the
divergence_mask bisect plumbing (unconditional behavior). Resolve B5/B6
here: promote AUTO drive mode into effective_comp_pwm for all builds and
the atomic com writer to always-on L431 ('D'/'E' levers stay bench-only).
Bench-validated claims; cite the parity re-qual artifact.

**PR-14: Sine changeover + RC-car + max_ramp parity** (~350 lines,
core). tick_sine/apply_sine_changeover, changeover_step channel,
rc_car_overrides/comp_pwm_guard, apply_max_ramp.

## Phase 5 — observability & bench (keepers)

**PR-15: reset_cause + debuguart + panic/HardFault + DWT hygiene**
(~700 lines, cross-crate). Portable reset_cause + 4-family readers +
boot banner; debug_uart + dprintln; panic/HardFault dumps; DWT
watchpoint disarm-at-boot (all 4 comparators); [loop] heartbeat
(GATED under debuguart — currently ungated in production); frame
history. Decide RTT policy (unconditional rtt_init burns RAM on F051).

**PR-16: Bench control band** (~1000 lines, cross-crate). bench_input
parser + RxRing (core), bench_uart DMA RX + two-frame confirm +
deadman, bench_guard (with A5 threshold fix), flight recorder + 'B',
bench_hist + 'H', LEAN + EXC counters, i-line. L431-only by
compile_error. Validate with fly/map scripts.

**PR-17: Optional capture instruments** (~900 lines, cross-crate).
blackbox (core + bench_bb), zctrace + edge_probe (after cfg fix),
WAXWING/GECKO/injected-current ADC instruments, 'V' + DebugMonitor
catcher. Cross-build all 4 targets × {zctrace, blackbox, none}.

**PR-18: bringup spin loop** (small). With the examples decision from
Phase 0.

## Merged trim list (do during PR extraction)

Code:
- Canary/shadow machinery + DIODE_* taps + [canary] print (C9, closed).
- RACEFIX_OFF + 'R', DivToggle 'K'/'M'/'P'/'Q' + divergence_mask
  consumers (C10; note: bit0 'K' was already inert — no consumer).
- N-pin traps (tim6 + tim14), midw, [nviol] print, GateToggle 'G' +
  GATE_STALE fresh branch (C12; make stale unconditional in comp_gate).
- Ungated ISR prints: `[exti] frame#…` + 2× `DETECTED` rprintlns in
  handle_exti_frame (prio-2 ISR), `[isr] state moved` (prio-0 first
  entry), comp_init COMP2_CSR rprintlns — the repo's own rule: never
  print in ISRs.
- dead `was_interrupt_mode` + `let _ =` in main_state stall block.
- handle_exti_frame `static mut FRAME_COUNT` (exists only for the
  trimmed prints).
- shared_state.rs dangling doc comment above zero_crosses (describes
  needs_reset).
- transfer.rs `servo_detection_sets_prescaler` test (self-admittedly
  inconclusive) — fix or drop.

Comments/docs:
- minz/private references (bench_input, blackbox, shared_comm,
  isr_logic, main_state, bench_guard, bench_uart, bench_zct, bench_bb,
  phase, main, adc, init, isr_handlers, debug_uart, panic:port_41.log,
  interrupts:spiral1.bin) → condense to the invariant; keep AM32 main.c
  citations.
- Dangling refs to absent docs: RATE_DIVERGENCE_REPORT.md,
  BRINGUP_NOTES_L431.md, LOST_PRESCALER.md, notes/ISR_STATE_INVARIANT.md
  (referenced 3×; genuinely load-bearing — ship it or inline into
  isr.rs).
- Stale porting-contract text in bemf_zero_cross doc ("F051/G071/G431 do
  NOT pre-ack" — they do now).
- Campaign dates/anecdotes ("07-26", "0/8 climbs", ABAB stories) →
  condense to surviving invariant. KEEP: avg=5000 dead-code story,
  polling exclusivity, fetch_max/AllOff invariant, buf[0] staleness,
  holdoff justification (dsy=289 vs 10, one line).
- config.rs rustdoc mis-attachment: temp_advance() inserted between
  apply_version_defaults and its doc; apply_rc_car_overrides under
  derive_motor_config's param docs. Re-home both docs.
- timer.rs three-era comment contradiction (ARPE on vs off).
- .cargo/config.toml personal probe serial (B8) → local config.

## Follow-up tickets (not in this series)

- A3 arming beeps (tone-request channel main→ISR HAL).
- A4 EEPROM save path (ISR config write-back via SharedState).
- G431/G071/F051 NVIC priority ladder (all level 0 today).
- Non-L431 DMA CGIF hygiene + COMP gate hoist portable.
- one_khz_counter load/store race + u8 wrap (comment or subtract-reset).
- Demotion leaves COMP armed one window in commutation_timer_expired —
  check AM32 masks at main.c:878-881 before calling it parity.
- sounds.rs play_startup IRQ contract is doc-only.
