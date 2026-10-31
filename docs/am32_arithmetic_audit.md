# AM32 arithmetic audit

AM32 is rm32's reference. This table ties every helper rm32 shares with
AM32 `Src/functions.c` to a test against a literal transcription of the C
(`rm32/src/functions.rs::am32_c`), or names the deliberate difference.

Why it exists: until 2026-10-31 rm32's `map()` was a straight line
`(x - in_min) * span / in_span + out_min`, while AM32's is a bisection with
floored midpoints. They differ by up to 4 counts on most inputs (1811 of
2001 throttle inputs, 1399 of 1401 ramp voltages). The exactness tests of
the time compared rm32 against its own formula, so nothing caught it.
Tests here compare against the C, never against rm32.

## `Src/functions.c`

| AM32 | rm32 | Result vs AM32 | Test |
|---|---|---|---|
| `map()` (functions.c:22) | `functions::map` | identical; recursion written as a loop | `functions::tests::map_matches_am32_*` (one per call site, below) and `map_matches_am32_on_random_wide_parameters` |
| `getAbsDif()` | `functions::get_abs_dif` | identical where C is defined (no `int` overflow) | `functions::tests::get_abs_dif_matches_am32` |
| `update_crc8()` | `functions::update_crc8` | identical, all 65,536 inputs | `functions::tests::update_crc8_matches_am32_exhaustively` |
| `get_crc8()` | `functions::get_crc8` | identical for every length 0..=255 | `functions::tests::get_crc8_matches_am32` |
| `delayMicros()` / `delayMillis()` | port `System::delay_micros` / `delay_millis` -> `rm32_stm32::delay::delay_us` | waits at least `us`, timed on the core-clock SysTick | hardware only (no host test). Fixed in this change: F051/G071/G431 used `asm::delay(us * MHz)`, which waits 3-4x too long; that stretched sine-start steps and tones |
| `get_timer_us16()`, `gpio_mode_*` | none | MCU-specific (AT32/GD32/WCH) | n/a |

AM32's `map()` has an unreachable branch: the `in_min > in_max` swap can
never run, because passing both clamps implies `in_min < x < in_max`. A
reversed input range therefore returns `out_max` for `x >= in_max` and
`out_min` otherwise. rm32 keeps that behaviour (it is the reference); no AM32
call site uses a reversed input range. rm32's brushed reverse map now
reverses the output range instead (`brushed.rs`).

## `map()` call sites

Every test sweeps every input a few counts past both ends, over the
configuration range of the parameters.

| AM32 | rm32 | Test (`functions::tests::`) |
|---|---|---|
| main.c:818 reverse_speed_threshold | `main_state::reverse_speed_threshold` (new: rm32 used AM32's 1500 initializer, which `loadEEpromSettings` always overwrites) | `map_matches_am32_reverse_speed_threshold`; `tests/platform_timing.rs::reversal_speed_gate_follows_motor_kv_for_dshot_and_servo` |
| main.c:1012/1027/1050/1064 servo dead band | `input_mapping.rs` | `map_matches_am32_servo_dead_band_maps` |
| main.c:1172/1176 sine-start input | `input_mapping::sine_start_map` | `map_matches_am32_sine_start_input` |
| main.c:1227/1229 duty setpoint | `DutyState::compute_setpoint` | `map_matches_am32_duty_setpoint`; `control::state::tests::duty_setpoint_matches_am32_for_all_inputs` |
| main.c:1892 voltage ramp | `DutyState::ramp_limit` | `map_matches_am32_voltage_ramp`; `control::state::tests::voltage_ramp_and_pwm_compare_match_am32` |
| main.c:2088 brushed duty | `brushed.rs` (unidirectional) | `map_matches_am32_brushed_duty` |
| main.c:2362 variable PWM | `main_state::variable_pwm_mode1` | `map_matches_am32_variable_pwm` |
| main.c:2619 low-rpm limit | `main_state::duty_ceiling` | `map_matches_am32_low_rpm_limit`; `main_state::tests::duty_ceiling_matches_am32_rpm_limit_shape` |
| main.c:2628 / 2092 temperature limit | `main_state::duty_ceiling` | `map_matches_am32_temperature_limit` |
| main.c:2634 filter level | `MainState::tick` | `map_matches_am32_filter_and_advance` |
| main.c:2641 auto advance | `MainState::tick` | `map_matches_am32_filter_and_advance` |
| main.c:2686/2688 sine angle | `sine.rs` | `map_matches_am32_sine_angle_and_step_delay` |
| main.c:2717 sine step delay | `sine.rs` | `map_matches_am32_sine_angle_and_step_delay` |
| signal.c:42 one-shot | `signal.rs` | `map_matches_am32_signal_maps` |
| signal.c:90/93/97 servo | `servo.rs`, `signal::compute_servo_unidirectional` | `map_matches_am32_signal_maps` |
| main.c:1182 speed control loop | not ported (`target_e_com_time` is never set) | none |

AM32 states the ranges at main.c:2362 and 2634 in its 2 MHz timer ticks.
The bisection's rounding depends on the endpoints, so rm32 converts the
input to 2 MHz ticks (`platform::ticks_to_am32`, free at 2 MHz) and maps
over AM32's literal ranges instead of scaling the endpoints.

## Constant divisions (`rm32/src/fast_math.rs`)

AM32 writes these as plain `/` on every core. rm32 uses the same helpers on
every chip (no per-core `cfg`), each exact for every value of its argument
type, tested exhaustively on host:

| Helper | Domain tested | Used for |
|---|---|---|
| `div3_i32` / `div3_u32` | every `i32` / `u32` | `average_interval = e_com_time / 3` (2 MHz) |
| `div5_u32`, `div10_u32` | every `u32` | proportional brake (main.c:1932) |
| `div2000_u32` | every `u32` | PWM compare (main.c:1927), active brake (main.c:1936) |

The voltage-ramp and setpoint divisions (`div1400_ramp`, `div1910_sine`)
are gone: those paths call `map()`, as AM32 does.

## Differences kept or found outside `functions.c`

- Proportional brake (main.c:1932): AM32's u16 result wraps when
  `drag_brake_strength > 10`; rm32 saturates to 0. rm32 also used to
  truncate the product to u16 before subtracting (wrong for large timer
  periods); fixed.
- Active brake (main.c:1936): AM32 adds `DEAD_TIME`; rm32 does not, and
  clamps to `tim1_arr`. Not yet resolved; inline `main.c` formulas are the
  next audit.
- Brushed: rm32's brushed module is not used by any firmware build; its
  maximum duty is `max * 95 / 100` where AM32 uses `ARR - ARR / 20`.
