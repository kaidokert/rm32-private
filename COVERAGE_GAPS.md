# AM32 C Firmware — Functional Coverage Gap Analysis

Generated from llvm-cov line coverage of `Src/` across unit tests (Catch2)
and blackbox tests (Python harness). Combined coverage: 78% lines, 81%
regions, 74% branches. **373 lines (22%) covered by neither test suite.**

## Method

- Build: `cmake -DCOVERAGE=ON`, clang with `-fprofile-instr-generate -fcoverage-mapping`
- Unit: `am32_tests` (Catch2, 156 test cases, 452 assertions)
- Blackbox: `am32_harness` driven by `pytest` (44 test vectors)
- Combined: `llvm-profdata merge` of both profiles

## Tier 1 — Critical Safety Gaps (zero coverage, safety-critical)

| # | File | Lines | Function/Block | What It Does | Risk If Missing | Coverage | Rust Port Status |
|---|------|-------|---------------|-------------|----------------|----------|-----------------|
| 1 | signal.c | 158 | `transfercomplete()` servo `buffersize=3` | When pin HIGH at DMA completion, sets `buffersize=3` to capture extra edge for realignment | Servo PWM never decodes on misaligned phase — motor unresponsive | Neither | **CONFIRMED BUG** — fixed (CaptureSize enum) |
| 2 | main.c | 2074-2100 | LVC per-cell and absolute voltage cutoff | Compares battery voltage against threshold, increments `low_voltage_count`, kills motor | Battery drains to zero — LiPo fire risk | Neither | Needs audit |
| 3 | main.c | 1919-1946 | Signal timeout failsafe (armed + unarmed) | After 0.5s (armed) or 2s (unarmed) signal loss: `allOff()`, disarm, `NVIC_SystemReset()` | Motor runs indefinitely on lost receiver | Neither | Needs audit |
| 4 | main.c | 2212-2285 | Sine-to-BLDC changeover | Sets ~10 state vars (`stepper_sine=0`, `running=1`, `commutation_interval=9000`, etc.) for mode transition | Immediate desync on every sine-mode motor start | Neither | Needs audit |
| 5 | dshot.c | 144 | EDT disarm on zero-throttle | When `EDT_ARM_ENABLE=1` and throttle=0, sets `EDT_ARMED=0` | Motor cannot be stopped via DShot zero command | Neither | Needs audit |
| 6 | dshot.c | 129-136 | Throttle gate when EDT not armed | Throttle values >47 silently dropped when `EDT_ARMED=0` | Motor spins when flight controller hasn't armed it | Neither | **CONFIRMED BUG** — fixed (EDT_ARMED guard) |
| 7 | main.c | 1723-1862 | `main()/init()` — RC-car overrides | Disables `stuck_rotor_protection`, forces `bi_direction=1`, adjusts duty for RC-car mode | RC-car ESC with stuck_rotor_protection ON fights direction changes | Neither | **MISSING** — all 11 init-time overrides absent |
| 8 | signal.c | 63-85 | Servo calibration pipeline | High threshold averaging (50+ samples), low threshold (75+ samples), `saveEEpromSettings()` | Calibration appears to work but never persists to flash | Neither | Pipeline present (12/13 steps); **EEPROM save missing** |

## Tier 2 — High Risk (ISR path, motor protection, protocol)

| # | File | Lines | Function/Block | What It Does | Risk If Missing | Coverage |
|---|------|-------|---------------|-------------|----------------|----------|
| 9 | dshot.c | 91-94 | Bidirectional DShot auto-detection | Counts `high_pin_count > 100` while unarmed, sets `dshot_telemetry=1` (inverts CRC) | False trigger = all frames fail CRC = total signal loss | Neither | **FIXED** — high_pin_count counter + bidir_detected flag |
| 10 | main.c | 1394-1402 | Falling-edge BEMF zero-cross | `bemfcounter > min_bemf_counts_down` in old_routine when `rising=0` | Half of commutation sensing untested — 50% desync rate | Neither | Present — zero_cross_detected(rising) with min_counts_down |
| 11 | main.c | 900-901 + 2151-2153 | Auto-advance computation + usage | `auto_advance_level = map(duty_cycle, 100, 2000, 13, 23)` used in commutation ISR | `auto_advance_level=0` = late commutation = desync at any RPM | Neither | Present — map() in main_state.rs, applied via SharedComm |
| 12 | main.c | 1421-1442 | Stall/speed/current PID clamping | Upper/lower clamps on all three PID output accumulators | Integral windup → full-throttle runaway or duty underflow | Neither | Present — integral + output limits in pid.rs, tested |
| 13 | main.c | 693-713 | Dead time override for driving brake | `driving_brake_strength < 10`: increases dead time, adjusts all duty thresholds, writes TIM1->BDTR | Insufficient dead time during braking → FET shoot-through → hardware damage | Neither | Present — derive_motor_config + set_dead_time_override |
| 14 | main.c | 1176-1178 | `!old_routine` motor restart | Calls `startMotor()` when `old_routine=0` and input >= 47 | Motor won't restart after interrupt-mode stop | Neither | Present — MotorEvent::StartMotor transition |
| 15 | signal.c | 205-206, 219-220 | High-freq MCU prescaler (`CPU_MHZ > 100`) | Sets `output_timer_prescaler` for DShot timing on G431 (170MHz) | Wrong DShot output timing = garbled telemetry or arming failure | Neither | **FIXED** — CaptureConfig.prescaler applied in all MCU ISRs |
| 16 | main.c | 2078-2080, 2087-2089 | LVC voltage recovery inhibit | Once LVC triggered, voltage recovery does NOT reset count | Motor restarts on dying battery that briefly recovers — crash risk | Neither | Present — latch tested in lvc_mode1_recovery_inhibit |

## Tier 3 — Moderate Risk (operational correctness)

| # | File | Lines | Function/Block | What It Does | Risk If Missing | Coverage |
|---|------|-------|---------------|-------------|----------------|----------|
| 17 | main.c | 1909-1918 | Variable PWM mode 2 | Computes `tim1_arr` from `average_interval` — auto PWM frequency | Duty overflow at high RPM → FET overheating | Neither |
| 18 | main.c | 1687-1716 | `checkDeviceInfo()` | Reads bootloader device info for EEPROM address selection | Wrong EEPROM address → reads garbage settings | Neither |
| 19 | main.c | 1490-1492 | Active brake on stop (mode 2) | `comStep(2)` + active brake power duty when stopped + armed | Motor freewheels instead of holding position | Neither |
| 20 | main.c | 1344-1350 | LVC cell count at arming | `cell_count = voltage / 370`, beep per cell | Wrong cell count → wrong cutoff voltage | Neither |
| 21 | main.c | 1037-1064 | DShot RC-car wrong-direction braking | Sets `adjusted_input=0`, activates prop brake, checks `return_to_center` | Power applied in wrong direction during rapid reversal | Partial |
| 22 | dshot.c | 261-266 | EDT voltage/temperature frames | Encodes `battery_voltage/25` and `degrees_celsius` into EDT packets | FC displays garbage voltage/temperature | Neither |
| 23 | signal.c | 183 | Calibration jitter rejection | Resets `enter_calibration_count` when stick jitters > 50 | Noisy servo signal accidentally triggers calibration | Neither |
| 24 | signal.c | 255-260 | Protocol re-confirmation in detectInput | `checkDshot()`/`checkServo()` re-validation on second frame | False protocol lock from single noisy capture | Neither |
| 25 | main.c | 629-632 | PWM frequency from EEPROM | Custom `pwm_frequency` 8-144 → timer divider | Audible noise or FET heating at wrong frequency | Neither |
| 26 | main.c | 760-763 | Slow ramp (`max_ramp < 10`) | Sets `ramp_divider=9`, uses raw EEPROM ramp values | Missing slow-ramp mode for specialized applications | Neither |
| 27 | main.c | 785-787 | Bidir polling changeover halving | Halves `polling_mode_changeover` for bidirectional mode | Later mode transition → missed zero-crosses during direction change | Neither |
| 28 | dshot.c | 173-183 | Beacon tones 2-5 | `play_tone_flag` for DShot commands 2-5 | Missing beep patterns for lost-drone locator | Neither |

## Coverage Delta: Unit vs Blackbox

| Category | Lines | Description |
|----------|-------|-------------|
| Both suites | 967 | Core decode, arming, commutation, BEMF polling |
| Unit only | 221 | EDT telemetry, DShot commands, sine phase advance, harness blind spots |
| Blackbox only | 137 | BEMF timeout, RC-car reverse, save-settings — fragile, no targeted tests |
| **Neither** | **373** | LVC, signal timeout, sine changeover, PID clamps, calibration, init |

## Root Cause Patterns

### 1. Feedback loops between decoder and HAL
**signal.c #1, dshot.c #9**: Where the decoder modifies HAL capture
behavior for the next cycle (`buffersize`, `dshot_telemetry`). Tests
use synthetic buffers that are always pre-aligned, so the realignment
/ mode-switch logic is invisible.

### 2. Protection latches
**main.c #2, #3, #5, #16**: Where a fault condition triggers a one-way
state change (LVC cutoff, signal timeout reset, EDT disarm, voltage
recovery inhibit). Tests exercise the normal path but never trigger
the fault.

### 3. State machine transitions
**main.c #4, #14**: Where multiple state variables must be set atomically
for a mode change (sine→BLDC, old_routine→running). Tests exercise
steady-state within each mode but not the transitions.

### 4. PID saturation
**main.c #12**: All three PID controllers have untested clamp paths.
The PIDs are exercised in the normal range but never driven to limits.

### 5. Init-time config overrides
**main.c #7**: Where `loadEEpromSettings()` applies config-dependent
overrides (RC-car disables stuck_rotor, `!comp_pwm` disables sine_start).
Tests use default config, so override branches are invisible.

### 6. Compile-time-gated branches
**signal.c #15**: Where `CPU_FREQUENCY_MHZ` changes the prescaler. Test
binary compiles with a fixed value, so the other branch is dead code
in the test binary. Only detectable via multi-config builds.

## Confirmed Bugs Found Via This Analysis

| # | File | Bug | Found By | Fixed |
|---|------|-----|----------|-------|
| 1 | signal.c:158 | `buffersize=3` feedback loop lost in Rust port | L431 hardware bringup | CaptureSize enum in TransferActions |
| 2 | dshot.c:129-136 | EDT_ARMED throttle gate missing in Rust firmware ISR | coderabbitai review + C coverage analysis | EDT_ARMED guard + regression test |

## Recommended Actions

1. **Audit Tier 1 items #2-#8 against Rust code** — each is a potential
   "lost in port" bug like #1 and #6
2. **Add C tests for `buffersize=3`** — mock `getInputPinState()=true`
3. **Add C tests for LVC and signal timeout** — the two most
   safety-critical completely uncovered paths
4. **Add blackbox vectors for EDT arming/disarming** — started with
   `edt_armed_throttle_gate.txt`
5. **Grep Rust port for every Tier 1 item** — verify each has a code
   path and ideally a test
6. **Run coverage on multi-config builds** — `CPU_FREQUENCY_MHZ=170`
   to exercise high-freq prescaler branches
