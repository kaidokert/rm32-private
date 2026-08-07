# RM32 Public Burn-Down Worklog

Last updated: 2026-08-06

## Branches And Worktrees

- Public main: `/opt/m/rust/esc/rm/rm32`, `main`, now at `f607c76 Add reset cause reporting (#43)`.
- Private bench tip: `/opt/m/rust/esc/rm/rm32-private-bench`, `private-bench-tip`.
- Public clean target: `/opt/m/rust/esc/rm/rm32-public-clean`, `public-ultimate-clean`.
- Public squash projection: `/opt/m/rust/esc/rm/rm32-public-squash`, `public-ultimate-squash`.
- Internal worklog: `/opt/m/rust/esc/rm/rm32-internal-worklog`, `internal/worklog`.

Current public-clean note: a rebaseline was attempted after PR #41 squash-merged.
The conflicted squash-merge attempt was discarded and `rm32-public-clean` was
reset to `origin/main`. The pre-rebaseline clean tree is preserved as
`public-ultimate-clean-before-bemf-rebaseline`.

## Landed Public PRs

### PR #41: Board-configured BEMF pin mappings

Squash merged as:

- `4c920d3 Add board-configured BEMF pin mappings (#41)`

What landed:

- `BemfPins` added to `BoardConfig`.
- Board YAMLs now declare symbolic `bemf_pins`.
- `build.rs` resolves symbolic pins to MCU-specific packed comparator values.
- Comparator constructors take board-provided mappings instead of static maps.
- L431 COMP2 initialization receives the initial phase from board config.

Known compromise:

- This is deliberately not the final design. The representation is still a
  shared `u32` register encoding, not a type-safe Rust model.
- Public follow-up issue: <https://github.com/kaidokert/rm32/issues/42>

### PR #43: Reset cause reporting

Squash merged as:

- `f607c76 Add reset cause reporting (#43)`

What landed:

- `rm32::reset_cause::ResetCause` portable bitflags.
- Per-MCU `read_and_clear_reset_cause()` implementations for F051, G071, G431,
  and L431.
- Boot-time reset-cause log before MCU init.
- `rm32_stm32::dprintln!` RTT macro and `rtt-target` dependency.

Known review follow-ups:

- Macro hygiene and power-reset naming feedback was captured below. It was not
  part of the squash-merged PR #43 state.

### PR #44: Passing blackbox vectors

Public PR:

- <https://github.com/kaidokert/rm32/pull/44>

What is proposed:

- Add the subset of `tests/blackbox` vectors from the cleanup branch that pass
  on current public `main`.
- Keep `desync_recovery` as an xfail because the old-routine parity difference
  is still real.
- Fix `eeprom_init_overrides` to use `eeprom_version=3`, matching the current
  EEPROM version.

Validation:

- `cargo build -p rm32 --bin rm32_harness --release`
- `AM32_HARNESS=target/release/rm32_harness pytest tests/blackbox/test_vectors.py -v`
- Initial result: `73 passed`.
- After review cleanup: `68 passed, 1 xfailed`.

Review cleanup applied:

- Dropped `signal_timeout_armed`; it documents FET-off/DMA-reset/system-reset
  safety behavior but only asserted `armed=0`, and CI failed on it.
- Dropped `beacon_tones`; current harness does not expose `PlayTone`, and the
  vector also needed `armed=1` plus command 4 coverage.
- Dropped `bidir_autodetect`; current harness ignores `input_pin_state`, so the
  vector did not exercise bidirectional autodetect.
- Dropped `calibration_jitter`; current harness ignores `last_input` and
  `enter_calibration_count`, so the vector did not initialize or observe the
  transfer state it described.
- Restored `dead_time_override` to the existing meaningful `load_eeprom` duty
  threshold check.
- Restored `desync_recovery` assertion and xfail.

Vectors deliberately left out because they failed on current public code:

- `desync_detection`
- `edt_disarm_on_zero`
- `low_voltage_cutoff`
- `motor_restart`
- `sine_brake_on_stop`
- `sine_changeover`
- `stuck_rotor`
- `stuck_rotor_rate`

Deferred PR #44 follow-ups:

- Add harness observability for `PlayTone` / `play_tone_flag`, then reintroduce
  beacon commands 2, 3, 4, and 5 with real tone assertions.
- Add harness support for driving input-pin-high state through DShot dispatch,
  then reintroduce bidirectional autodetect with `dshot_telemetry=1`.
- Add supported setup/observation for calibration jitter state, or move that
  coverage to a lower-level transfer unit test.
- Implement or expose signal-timeout safety effects: motor-off/FET-off, DMA
  reset, and system reset. Only then reintroduce `signal_timeout_armed`.
- Strengthen smoke-like vectors when harness-visible signals exist:
  `active_brake_mode2`, `rc_car_braking`, `sine_brake_mode2`,
  `sine_brake_off`, `sine_stepping`, `variable_pwm_mode2`,
  `bemf_falling_edge`, `desync_recovery_bidir`, `pid_clamping`,
  `speed_control`, `lvc_absolute_cutoff`, `lvc_recovery_inhibit`, and
  `edt_voltage_temp`.

## Source Trails

### STM32G431 Comparator INMSEL Encoding

Review comment source trail:

- CodeRabbit comment said G431 PA4/PA5 should use `INMSEL = 0b110`, not `0b000`.
- Verified against ST reference manual RM0440, Table 196, "COMPx inverting input assignment".
- Table result:
  - `INMSEL[2:0] = 000`: `1/4 VREFINT`
  - `INMSEL[2:0] = 110`: `COMP1_INM = PA4`, `COMP2_INM = PA5`
  - `INMSEL[2:0] = 111`: `COMP1_INM = PA0`, `COMP2_INM = PA2`
- Related correction: `PA7` belongs to the COMP2 non-inverting input table, not
  the COMP2 inverting input table.

Implication:

- The private/squash branch had G431 comments and setup that treated IO1 as
  `0b000`; that was wrong for STM32G4.
- If future G431 mappings use COMP2 second inverting input, the YAML should use
  `PA2`, not `PA7`.

## Follow-Up Design Issues

### PR #43 Review Follow-Ups: Reset Cause Reporting

Public PR:

- <https://github.com/kaidokert/rm32/pull/43>

Apply on PR #43:

- Fix exported `dprintln!` macro hygiene by re-exporting `rtt_target` from
  `rm32_stm32` and invoking it as `$crate::rtt_target::rprintln!`.
- Correct `rm32/src/reset_cause.rs` docs: only L4 currently has every portable
  reset flag in the set; G4 does not expose the firewall reset flag.
- Add a distinct power reset flag instead of mapping F0 `PORRSTF` and G0
  `PWRRSTF` to `BROWNOUT`. Cold boot should not log as brownout unless the MCU
  specifically reports a brownout reset.

Deferred / do not apply on PR #43:

- Do not abstract the four `read_and_clear_reset_cause` implementations yet.
  The PAC register accessors differ enough that inline per-MCU decode is easier
  to review in this small PR.
- Do not introduce a debug transport abstraction for `dprintln!` here. The
  immediate issue is macro hygiene for the RTT dependency; transport policy is a
  broader diagnostics decision.

Related future cleanup:

- If reset-cause support grows beyond these four MCUs, consider a shared helper
  for constructing `ResetCause` from booleans, while keeping PAC reads inside
  MCU modules.
- If RTT becomes undesirable for default public firmware, add an explicit
  diagnostics/logging feature rather than hiding the transport behind this reset
  PR.

### BEMF Mapping Should Become Typed

Problem:

- `BoardConfig::bemf_pins` stores packed MCU register values in shared config.
- Build script owns MCU-specific bit packing.
- Invalid states are representable as arbitrary `u32`.
- The generic comparator path has a raw integer boundary.

Desired direction:

- YAML stays symbolic.
- Build step emits typed comparator routes, or MCU modules convert symbolic
  routes to typed route values.
- Raw register packing stays inside MCU-specific modules.
- Invalid routes should fail at build time or be unrepresentable.

Public issue:

- <https://github.com/kaidokert/rm32/issues/42>

### Board MCU Must Match Cargo Feature

Reason:

- `BOARD=...` can select YAML for one MCU while Cargo compiles another MCU
  feature.
- The BEMF mapper would otherwise generate packed values for the wrong
  comparator implementation.

Current PR #41 fix:

- `build.rs` derives the single enabled MCU feature and asserts it matches
  `board.mcu`.

### `common` BEMF Pin Is Not Routed

Reason:

- Simple BEMF YAML accepts `common`, but comparator init still hardcodes the
  positive/common comparator input per MCU.

Current PR #41 fix:

- Validate `common` against the expected input pin for each simple-comparator
  MCU.

Longer-term:

- Typed route work should either make common input explicit or remove it from
  YAML if it is not intended to vary.

## Suspect Public-Cleanup Areas

### PID Loop Divider

Observed code:

```rust
pub const PID_LOOP_DIVIDER: u8 = 20;
```

Risk:

- Global constant assumes TIM6 is 20 kHz on every MCU.
- L431 private branch used `80 MHz / 80 / 51 ~= 19.608 kHz`, so the comment and
  rate assumption were already false there.
- `one_khz_counter_check_and_reset` uses `> divider`; with divider 20, that can
  effectively fire after 21 ticks, not exactly 20.

Follow-up:

- Move divider/rate into chip config or derive from TIM6 frequency.
- Decide whether AM32 parity requires `>` or whether rm32 wants `>=`.

### ADC Channel Selection Is Board-Specific But Often Hardcoded

Risk:

- Board YAML has `current_adc_channel` and `voltage_adc_channel`.
- MCU ADC implementations still appear to hardcode channels/pins.
- This can work on the L431 bench board and fail on another board with the same
  MCU but different ADC wiring.

Follow-up:

- Audit all MCU ADC init paths.
- Thread board ADC channel selection into hardware setup or make per-board
  support explicit.

### Main Loop Debug And Control Baggage

Suspect files:

- `rm32_stm32/src/bin/main.rs`
- `rm32/src/shared_state.rs`
- `rm32/src/transfer.rs`

Examples:

- DWT cycle counter/watchpoint cleanup and heartbeat telemetry.
- Boot progress `dprintln!` blocks.
- `nop()` loop replacing `wfi()` with long rationale.
- Debug counters named as bench/debug artifacts.

Follow-up:

- Remove debug-only telemetry from public path, or isolate under an explicit
  diagnostics feature.
- Make sleep/spin-loop behavior its own reviewed change.

### EEPROM Save Correctness Gap

Observed in private/squash main path:

- EEPROM save path had a TODO around ISR-mutated config not being copied back
  before save.

Risk:

- Public branch should not silently ship a save path known to persist stale
  config state.

Follow-up:

- Make config ownership and mutation flow explicit before exposing this change.

### Arming Feedback Regression

Observed in private/squash main path:

- Arming feedback moved away from motor beeps with TODO that beeps need HAL
  access.

Risk:

- User-visible behavior change.

Follow-up:

- Split into a small behavior PR or restore previous feedback before landing
  unrelated work.

### L431-Specific Parity Blocks

Suspect files:

- `rm32_stm32/src/mcu_l431/init.rs`
- `rm32_stm32/src/mcu_l431/comp_init.rs`
- `rm32_stm32/src/mcu_l431/comparator.rs`

Concerns:

- Large manual clock tree / NVIC / TIM setup changes.
- Long narrative comments from bench/debug history.
- Duplicate raw bit manipulation in comparator init and runtime switching.

Follow-up:

- Split each actual parity need into a tiny PR.
- Keep bench narrative out of public comments.
- Centralize L431 COMP2 packed value decode if this representation survives
  until typed route work.

### L431 Atomic Commutation Path

File:

- `rm32_stm32/src/phase.rs`

Risk:

- L431-specific direct GPIO base-address/pin assumptions inside generic phase
  driver code.
- Cfg-gated, but still board-specific for a nominal MCU target.

Follow-up:

- Make board pin assumptions explicit or keep out of the general public series
  until the board model can represent it.

### Timer ARPE Commentary/Behavior

File:

- `rm32_stm32/src/timer.rs`

Concern:

- Comments conflict about ARPE being deliberately off versus behavior assuming
  ARPE on.

Follow-up:

- Resolve with a small timer PR: one behavior, one rationale, one source trail.

### Storytelling Comments

Common terms to scrub or rewrite before public PRs:

- "bench rail"
- "clone"
- "rung"
- "scar"
- "jitter-floor hunt"
- "churn"
- "sag-killed"
- "wrong-phase orbit"

Files seen with this style:

- `rm32/src/main_state.rs`
- `rm32/src/constants.rs`
- `rm32/src/signal.rs`
- `rm32_stm32/src/mcu_l431/adc.rs`

Guideline:

- Replace with short technical comments describing invariant, source, and
  failure mode. Put long investigation narrative in this internal worklog
  instead.

## PR Sequence Candidates

Likely early public PRs:

- Reset cause support.
- `.cargo/config.toml` target additions.
- Small board/build hygiene that does not change runtime behavior.

Later/larger PRs:

- Transfer/protocol changes with tests.
- ADC board-channel plumbing.
- Control loop and timing behavior.
- L431 parity work.
- Diagnostics/telemetry feature split, if still desired.

## Rebaseline Notes

After a public PR lands:

- Fetch `origin/main`.
- Fast-forward `/opt/m/rust/esc/rm/rm32`.
- For clean branch, prefer rebuilding the final clean tree on top of new main
  rather than replaying hundreds of private commits.
- For squash projection, regenerate from the clean tree after it is rebased.
- Original private bench branch can merge public main instead of rebasing.

Current post-PR #43 state:

- `public-ultimate-clean-before-bemf-rebaseline` preserves the pre-rebaseline
  clean branch.
- `rm32-public-clean` is reset to `origin/main` at `f607c76`.
- `rm32-public-squash` was rebuilt again after PR #43 as one remaining-change
  commit on top of `origin/main`: `a38d337 Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-reset-cause-rebaseline`.
- `rm32-private-bench` fast-forwarded to `private/am32_sheet` at `630a5d6`.
  Merging public `origin/main` into it was attempted and aborted because it
  produced broad BEMF/reset-cause conflicts; the worktree is clean and does not
  contain that merge.
- Most conflicts in future rebuilds are expected BEMF overlap and should keep
  the public landed version for BEMF-only files.

Current post-PR #44 state:

- PR #44, "Add passing blackbox vectors", landed on public `main` as
  `6c0555e`.
- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `6c0555e`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `6c0555e`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `6d6d0a8 Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-blackbox-rebaseline`.
- The remaining blackbox delta in the squash projection is now the deferred
  review set: `beacon_tones`, `bidir_autodetect`, `calibration_jitter`,
  `edt_disarm_on_zero`, `motor_restart`, `signal_timeout_armed`,
  `sine_brake_on_stop`, `sine_changeover`, `stuck_rotor_rate`, plus
  strengthening edits to existing vectors.

Open PR #45:

- PR: https://github.com/kaidokert/rm32/pull/45
- Branch/worktree: `public-pr-static-advance`,
  `/opt/m/rust/esc/rm/rm32-public-pr-static-advance`
- Commit: `48ca447 Apply static commutation advance mapping`
- Scope: shared EEPROM `advance_level` mapping for static commutation
  advance, main-loop publication when dynamic auto-advance is disabled,
  harness parity, and a blackbox vector for old/new/fallback mappings.
- Validation: `cargo fmt --check -p rm32`, `cargo test -p rm32`,
  release harness build, and full blackbox suite (`69 passed, 1 xfailed`).

PR #45 review follow-up:

- Commit: `edbd03f Address static advance review feedback`
- Fixed the ISR sync path so a static zero advance value clears a previous
  nonzero `temp_advance`.
- Strengthened the blackbox vector with distinct old/new/fallback expected
  values and added a focused unit test for zero advance sync.
- Replaced unexplained mapping literals with named constants and a short source
  comment.
- Validation after the follow-up: `cargo fmt --check -p rm32`,
  targeted `temp_advance` and zero-sync tests, release harness build,
  `cargo test -p rm32` (`233 passed`), and full blackbox suite
  (`69 passed, 1 xfailed`).

Current post-PR #45 state:

- PR #45, "Apply static commutation advance mapping", landed on public
  `main` as `515fbe0`.
- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `515fbe0`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `515fbe0`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `d4ce2dc Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-static-advance-rebaseline`.
- Rebaseline conflicts were limited to `rm32/src/bin/harness.rs` and
  `rm32/src/main_state.rs`; resolution kept the public-landed static advance
  behavior while preserving remaining squash-only harness counters.

Closed PR #46:

- PR: https://github.com/kaidokert/rm32/pull/46
- Branch/worktree: `public-pr-cortex-m4-linker-rustflags`,
  `/opt/m/rust/esc/rm/rm32-public-pr-cortex-m4-linker-rustflags`
- Commit: `d812ad4 Add linker rustflags for Cortex-M4 targets`
- Scope: add `-Tlink.x` cargo rustflags for `thumbv7em-none-eabi` and
  `thumbv7em-none-eabihf`; no runtime code changes.
- Validation: firmware builds for `stm32g071`, `stm32f051`, `stm32l431`, and
  `stm32g431` all pass. `cargo fmt --manifest-path rm32_stm32/Cargo.toml
  --check` still reports pre-existing unrelated formatting diffs, so no
  formatting changes were made in this PR.
- Closed without merge on 2026-08-06 after review showed this is not the clean
  standalone slice we wanted:
  - Root-invoked CI/pre-commit commands use `cargo build --manifest-path
    rm32_stm32/Cargo.toml` from repo root, so Cargo may not read
    `rm32_stm32/.cargo/config.toml`; target rustflags there are not guaranteed
    to affect the actual validation path.
  - Adding `-Tlink.x` for M4 targets would still select the single tracked
    `rm32_stm32/memory.x`, currently documented as STM32G071 layout.
    L431/G431 need a deliberate per-MCU memory-script selection plan before
    enabling target-level linker flags.
- Future PR shape: solve Cargo config discovery and per-MCU linker memory
  selection together. Do not re-land the PR #46 one-file form.

Merged PR #47:

- PR: https://github.com/kaidokert/rm32/pull/47
- Branch/worktree: `public-pr-bemf-zc-result`,
  `/opt/m/rust/esc/rm/rm32-public-pr-bemf-zc-result`
- Commit: `6b9e981 Return BEMF zero-cross acceptance result`
- Scope: make `bemf_zero_cross` return whether the comparator transition was
  accepted; update host tests to assert accepted and filtered-out paths; keep
  the STM32 ISR caller behavior unchanged by discarding the result.
- Validation: `cargo fmt --check`, focused `isr_bemf_zero_cross` tests,
  `cargo test -p rm32`, `cargo check --no-default-features --features
  stm32g071 --lib` from `rm32_stm32/`, and `cargo build --release
  --no-default-features --features stm32g071` from `rm32_stm32/`.

Current post-PR #47 state:

- PR #47 landed on public `main` as `fdb3ac7`.
- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `fdb3ac7`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `fdb3ac7`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `1f929fb Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-bemf-zc-result-rebaseline`.
- Rebaseline conflict was limited to `rm32_stm32/src/isr_handlers.rs`; the
  resolution kept the remaining squash-side timing instrumentation and named
  local `accepted` while relying on the now-public `bemf_zero_cross` return
  value.

Merged PR #48:

- PR: https://github.com/kaidokert/rm32/pull/48
- Branch/worktree: `public-pr-dshot-detect-timer-wrap`,
  `/opt/m/rust/esc/rm/rm32-public-pr-dshot-detect-timer-wrap`
- Commits:
  - `c9f4331 Handle timer wrap in DShot input detection`
  - `7639523 Exercise DShot300 timer wrap test`
- Scope: make DShot input detection skip the potentially stale first capture
  delta, compute deltas at 16-bit timer width, average actual valid deltas,
  and add host tests for timer wrap and stale first-slot data. This
  intentionally did not include DShot150, prescaler feedback, or 33-edge
  capture plumbing.
- Review follow-up: CodeRabbit correctly noted the DShot300 wrap test did not
  cross the 16-bit boundary; fixed with the second commit.
- Validation: `cargo fmt --check`, focused `signal::tests`, and
  `cargo test -p rm32`.
- Landed on public `main` as `5387162`.

Merged PR #49:

- PR: https://github.com/kaidokert/rm32/pull/49
- Branch/worktree: `public-pr-ws2812-set-status`,
  `/opt/m/rust/esc/rm/rm32-public-pr-ws2812-set-status`
- Commit: `7f2c1c9 Add WS2812 status helper`
- Scope: add `Ws2812Gpio::set_status()` so WS2812 LED status updates own
  the `interrupt::free` bit-bang wrapper; update firmware call sites to use
  `led.set_status(...)`. No motor-control behavior change.
- Validation: `cargo fmt --check`, `cargo test -p rm32`, and
  `cargo build --release --no-default-features --features stm32g071` from
  `rm32_stm32/`.
- Landed on public `main` as `e7079a3`.

Current post-PR #49 state:

- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `e7079a3`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `e7079a3`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `3a13fea Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-ws2812-rebaseline`.
- Rebaseline conflicts were limited to `rm32/src/signal.rs` and
  `rm32_stm32/src/bin/main.rs`:
  - `signal.rs` kept the public PR #48 wrap/stale-buffer implementation as
    canonical and retained only the remaining DShot150 classifier/test from
    the squash.
  - `main.rs` kept the now-public `led.set_status(...)` call style and the
    remaining squash-side LED-only arming branch.
- Follow-up hygiene: removed the duplicate already-landed `set_status()`
  method from the squash projection after the PR #49 rebase.
- Follow-up hygiene: removed unused `heapless = "0.8"` from
  `rm32_stm32/Cargo.toml`; remaining `heapless` use is in the core `rm32`
  crate and already public.

Merged PR #50:

- PR: https://github.com/kaidokert/rm32/pull/50
- Branch/worktree: `public-pr-dshot150-detection`,
  `/opt/m/rust/esc/rm/rm32-public-pr-dshot150-detection`
- Commit: `6976f04 Add DShot150 input detection`
- Scope: add `SignalType::Dshot150`, classify DShot150 capture spacing in
  `signal::detect_input`, and route DShot150 auto-detection as DShot in
  transfer setup. This intentionally did not include capture prescaler, output
  timing, or broader DShot plumbing changes.
- Validation: `cargo fmt --check`, `cargo test -p rm32 signal::tests --lib`,
  and `cargo test -p rm32`.
- Landed on public `main` as `b9bcf11`.

Current post-PR #50 state:

- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `b9bcf11`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `b9bcf11`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `acda52d Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-dshot150-rebaseline`.
- Rebaseline conflict was limited to `rm32/src/transfer.rs`; the resolution
  kept the remaining squash-side two-step input confirmation and per-protocol
  capture config while using the now-public DShot150 variant.
- Private `am32_sheet` was fetched and checked out at
  `/opt/m/rust/esc/rm/rm32-am32-sheet` on local branch `am32_sheet`, tracking
  `private/am32_sheet` at `7e15d95`.

Merged PR #51:

- PR: https://github.com/kaidokert/rm32/pull/51
- Branch/worktree: `public-pr-sine-changeover`,
  `/opt/m/rust/esc/rm/rm32-public-pr-sine-changeover`
- Commits:
  - `993daf8 Force sine startup changeover at high throttle`
  - `42b2363 Preserve sine phase offsets on forced changeover`
- Scope: force high-throttle sine startup to reach the BLDC changeover point
  immediately once input is above `SINE_CHANGEOVER_THROTTLE`; preserve B/C
  phase offsets when rebasing phase A to zero; document the existing strict
  threshold boundary with a unit test.
- Review follow-up: CodeRabbit and Codex correctly flagged that resetting only
  phase A corrupted the 120/240 degree offsets; fixed with the second commit.
  Sourcery questioned `>` vs `>=`; left strict `>` because it was pre-existing
  behavior and is now covered by `sine_step_threshold_is_strict`.
- Validation: `cargo fmt --check`, `cargo test -p rm32 sine::tests --lib`,
  and `cargo test -p rm32`.
- Landed on public `main` as `7690ae7`.

Current post-PR #51 state:

- `/opt/m/rust/esc/rm/rm32` is fast-forwarded to `origin/main` at `7690ae7`.
- `/opt/m/rust/esc/rm/rm32-public-clean` is reset to `origin/main` at
  `7690ae7`.
- `/opt/m/rust/esc/rm/rm32-public-squash` was rebuilt as one remaining-change
  commit on top of `origin/main`:
  `9b00af1 Squash remaining public clean state onto public main`.
- Previous squash projection is preserved as
  `public-ultimate-squash-before-sine-changeover-rebaseline`.
- Rebaseline conflict was limited to `rm32/src/sine.rs`; the resolution kept
  the reviewed public #51 implementation and dropped the older pre-review
  squash-side sine changeover hunk. `rm32/src/sine.rs` now has no remaining
  diff in the squash projection.
