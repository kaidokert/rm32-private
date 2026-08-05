# RM32 Public Burn-Down Worklog

Last updated: 2026-08-05

## Branches And Worktrees

- Public main: `/opt/m/rust/esc/rm/rm32`, `main`, now at `4c920d3 Add board-configured BEMF pin mappings (#41)`.
- Private bench tip: `/opt/m/rust/esc/rm/rm32-private-bench`, `private-bench-tip`.
- Public clean target: `/opt/m/rust/esc/rm/rm32-public-clean`, `public-ultimate-clean`.
- Public squash projection: `/opt/m/rust/esc/rm/rm32-public-squash`, `public-ultimate-squash`.
- Internal worklog: `/opt/m/rust/esc/rm/rm32-internal-worklog`, `internal/worklog`.

Current public-clean note: a rebaseline was attempted after PR #41 squash-merged.
The worktree is mid-conflict and should either be resolved carefully or rebuilt
again from the backup ref `public-ultimate-clean-before-bemf-rebaseline`.

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

Current interrupted attempt:

- `public-ultimate-clean-before-bemf-rebaseline` preserves the pre-rebaseline
  clean branch.
- `rm32-public-clean` has a conflicted squash-merge state after resetting to
  `origin/main` and squash-merging that backup branch.
- Most conflicts are expected BEMF overlap and should keep the public landed
  version for BEMF-only files.

