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
- Remove the stale `desync_recovery` xfail because the adjusted vector passes.

Validation:

- `cargo build -p rm32 --bin rm32_harness --release`
- `AM32_HARNESS=target/release/rm32_harness pytest tests/blackbox/test_vectors.py -v`
- Result: `73 passed`.

Vectors deliberately left out because they failed on current public code:

- `desync_detection`
- `edt_disarm_on_zero`
- `low_voltage_cutoff`
- `motor_restart`
- `sine_brake_on_stop`
- `sine_changeover`
- `stuck_rotor`
- `stuck_rotor_rate`

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
