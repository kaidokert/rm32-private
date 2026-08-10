# `am32_clone` versus rm32 structural comparison

## Executive summary

Both implementations extract AM32-derived motor-control logic from STM32
register code and use statically dispatched ESC-specific traits in the
real-time path. Both follow the same broad runtime shape: comparator
zero-cross detection, delayed six-step commutation, an approximately 20 kHz
control tick, and a continuously running foreground loop
([MinZ HAL](../../minz/core/src/am32_hal.rs#L1-L20),
[rm32 HAL](../../rm32/src/hal.rs#L68-L84),
[MinZ contexts](../../minz/examples/am32_clone.rs#L19-L33),
[rm32 handlers](../../rm32_stm32/src/isr_handlers.rs#L71-L146)).

The central structural difference is state ownership:

- `am32_clone` keeps control truth in program-owned atomic statics grouped into
  borrowed `Sched`, `Drive`, `Duty`, and `Bench` views
  ([storage](../../minz/examples/am32_clone.rs#L103-L152),
  [views](../../minz/core/src/am32_loop.rs#L74-L137)).
- rm32 separates mutable ISR-owned state from main-loop-owned state and uses
  atomic `SharedState` as a directional communication bridge
  ([ISR state](../../rm32_stm32/src/isr.rs#L70-L88),
  [main state](../../rm32/src/main_state.rs#L90-L127),
  [shared interface](../../rm32/src/shared_comm.rs#L30-L223)).

This makes `am32_clone` the smaller and more literal AM32 transliteration,
while rm32 is the broader, multi-target product architecture.

## Side-by-side

| Dimension | MinZ `am32_clone` | `rm32` + `rm32_stm32` |
|---|---|---|
| Primary goal | Factory-default, forward-only AM32 control transliteration for one L431 board ([scope](../../minz/examples/am32_clone.rs#L1-L17)). | Portable ESC core with STM32 adapters, persistent configuration, multiple protocols, and four MCU families ([workspace](../../Cargo.toml#L1-L7), [features](../../rm32_stm32/Cargo.toml#L38-L43)). |
| Packaging | Fixed-target firmware crate consuming a detached, host-testable `minz-core`; the firmware is a Cargo example ([manifests](../../minz/Cargo.toml#L13-L25), [entry](../../minz/examples/am32_clone.rs#L432-L433)). | `rm32` is the root-workspace portable core; `rm32_stm32` is a separately built target crate with one normal firmware binary ([workspace](../../Cargo.toml#L1-L7), [entry](../../rm32_stm32/src/bin/main.rs#L1-L7)). |
| Hardware boundary | Seven narrow traits—`MotorPwm`, `CompCtl`, `ComTimers`, `Recorder`, `Cs`, `InjAdc`, and `LoopTimer`—bundled into generic `Hal` ([source](../../minz/core/src/am32_hal.rs#L22-L137)). | Broad domain HAL split into PWM, phase output, comparator, interval timer, commutation timer, input capture, ADC, flash, telemetry, and system services; `MotorHal` rebundles the fast subset ([source](../../rm32/src/hal.rs#L14-L152)). |
| State ownership | Shared atomic storage borrowed through four cohesion clusters; main and ISR code operate on the same cells ([clusters](../../minz/examples/am32_clone.rs#L237-L303)). | Owned `IsrState` and `MainState`, with selected values, flags, and actions crossing through atomic `SharedState` ([ISR](../../rm32_stm32/src/isr.rs#L70-L88), [mailbox](../../rm32/src/shared_state.rs#L35-L74)). |
| Motor lifecycle | AM32-compatible booleans and counters live in `Drive` and related atomic fields ([source](../../minz/core/src/am32_loop.rs#L93-L111)). | Central `MotorMode`/`MotorEvent` state machine plus `Commutation`, `BemfState`, and `DutyState` ([mode](../../rm32/src/motor_mode.rs#L1-L87), [control state](../../rm32/src/control/state.rs#L8-L40)). |
| Orchestration | `main_entry` orders foreground work; thin vector functions delegate COMP, TIM16, and TIM6 bodies into `minz-core` ([main](../../minz/examples/am32_clone.rs#L322-L360), [vectors](../../minz/examples/am32_clone.rs#L554-L584)). | Portable ISR algorithms are called through shared platform handlers and family wrappers. `SystemTick` shares the core foreground pipeline with the host harness, while platform side effects remain in firmware main ([handlers](../../rm32_stm32/src/isr_handlers.rs#L71-L179), [tick](../../rm32/src/system.rs#L153-L172), [outer loop](../../rm32_stm32/src/bin/main.rs#L379-L481)). |
| Target/configuration | Hard-coded STM32L431 pin map and timing constants, factory-default control parameters, and one `pwm48` carrier feature ([board](../../minz/src/board_init.rs#L59-L103), [feature](../../minz/Cargo.toml#L6-L11)). | Cargo feature selects an MCU family, YAML generates `BoardConfig`, and flash-backed `EepromConfig` supplies runtime policy ([gateway](../../rm32_stm32/src/mcu.rs#L22-L41), [generator](../../rm32_stm32/build.rs#L210-L309), [EEPROM](../../rm32/src/config.rs#L17-L85)). |
| Test seam | Parameterized control and ISR bodies run on the host through a call-recording HAL mock ([mock](../../minz/core/src/am32_hal.rs#L139-L191)). | Core unit tests plus a process-driven native harness using the same `SystemTick` slice and portable ISR logic as firmware ([harness](../../rm32/src/bin/harness.rs#L463-L494), [firmware call](../../rm32_stm32/src/bin/main.rs#L379-L407)). |

## Direct abstraction mapping

| Shared concept | `am32_clone` | rm32 |
|---|---|---|
| Fast hardware facade | `am32_hal::Hal` | `hal::MotorHal` / platform `IsrHal` |
| Commutation state | `Drive` | `Commutation` + `MotorMode` |
| BEMF state | Fields split across `Drive` and `Sched` | `BemfState` plus shared timing fields |
| Duty/ramp state | `Duty` | `DutyState` |
| Main/ISR communication | Shared atomic cells exposed through borrowed clusters | Directional `SharedComm` traits implemented by `SharedState` |
| Foreground pipeline | `main_entry` | `SystemTick` plus platform work around it |

The mapping is conceptual rather than field-for-field. In particular, MinZ
`Bench` combines measurements, kill state, requests, and trace controls that
rm32 distributes among `Measurements`, `ProtectionState`, `SharedState`, and
platform diagnostics
([MinZ fields](../../minz/core/src/am32_loop.rs#L127-L137),
[rm32 state](../../rm32/src/control/state.rs#L296-L376),
[shared fields](../../rm32/src/shared_state.rs#L35-L74)).

## Runtime-flow overlap

Both systems implement the same essential control sequence:

1. A periodic control interrupt applies input/ramp/duty work and performs
   polled BEMF startup.
2. A qualified crossing updates interval timing and eventually hands control
   to comparator-interrupt operation.
3. The comparator path persistence-filters the signal, captures the interval,
   and arms a one-shot commutation timer.
4. The commutation timer advances the six-step state, changes phase roles and
   comparator input, updates timing, and opens the next BEMF window.

MinZ expresses those bodies through `am32_isr` and `am32_control`
([polling and duty](../../minz/core/src/am32_isr.rs#L151-L250),
[handoff](../../minz/core/src/am32_control.rs#L153-L216),
[COMP/TIM16](../../minz/core/src/am32_isr.rs#L33-L143)).
rm32 expresses them through portable `isr_logic` called by target handlers
([tick handler](../../rm32_stm32/src/isr_handlers.rs#L71-L119),
[portable commutation/BEMF](../../rm32/src/control/isr_logic.rs#L216-L298)).

## Important differences inside that overlap

**HAL granularity.** MinZ combines phase-role control with PWM and combines
both timing roles in `ComTimers`. rm32 separates PWM, phase output, interval
timing, and commutation timing before rebundling them in `MotorHal`
([MinZ traits](../../minz/core/src/am32_hal.rs#L22-L87),
[rm32 traits](../../rm32/src/hal.rs#L14-L84)).

**Comparator pending semantics.** MinZ can preserve a post-cross pending event
while its interval gate is closed. rm32 L431 clears and masks the comparator
interrupt at wrapper entry; the portable core warns that the other three MCU
wrappers lack this mitigation
([MinZ gate](../../minz/core/src/am32_isr.rs#L47-L61),
[MinZ unmask](../../minz/src/comp2.rs#L244-L311),
[rm32 warning](../../rm32/src/control/isr_logic.rs#L268-L280),
[L431 wrapper](../../rm32_stm32/src/mcu_l431/interrupts.rs#L27-L49)).

**Step identity.** MinZ converts logical steps 1..6 to 0..5 adapters, whereas
rm32 adapters consume 1..6. The concrete L431 tables also have a one-step
physical rotation: rm32 step 1 corresponds to MinZ logical step 6, rm32 step 2
to MinZ logical step 1, and so on
([MinZ conversion](../../minz/core/src/am32_control.rs#L45-L63),
[MinZ table](../../minz/src/tim1_motor_pwm.rs#L231-L240),
[rm32 table](../../rm32_stm32/src/phase.rs#L111-L148)).

**Scope.** rm32 structurally includes DShot, servo PWM, commands, EDT,
bidirectional transfer, sine start, braking, protections, and PID controllers.
`am32_clone` intentionally implements a smaller UART-duty, factory-default
subset
([clone scope](../../minz/examples/am32_clone.rs#L5-L17),
[rm32 configuration](../../rm32/src/config.rs#L5-L78),
[rm32 control state](../../rm32/src/control/state.rs#L198-L213)).
Declared breadth is not identical to active firmware behavior: the current
rm32 main clears several complex EEPROM settings at boot
([overrides](../../rm32_stm32/src/bin/main.rs#L209-L218)).

## Verification and cautions

- `minz-core` reported 258 passing test executions, with one duplicated test
  execution and one unannotated test-shaped function; the `am32_clone` release
  target check passed
  ([test source](../../minz/core/src/zc.rs#L1490-L1516),
  [audit](audit-minz.yaml#L363-L380)).
- rm32 reported 253 passing unit tests and successful cross-builds for all
  four MCU targets. Its current black-box run was 77 passed and one
  `temperature_limit` failure
  ([audit](audit-rm32.yaml#L444-L478)).
- rm32's L431 `IsrCell` is a current critical soundness risk: it returns a
  mutable reference under a documented equal-priority assumption, but the
  handlers sharing it currently run at priorities 0, 1, 2, and 3
  ([cell](../../rm32_stm32/src/isr_handlers.rs#L12-L69),
  [priorities](../../rm32_stm32/src/mcu_l431/init.rs#L219-L249)).
- Host tests and cross-builds do not validate PAC side effects, interrupt
  latency, DMA behavior, electrical phase identity, or motor safety on
  hardware.

## Bottom line

`am32_clone` is the tighter reference transliteration: fewer layers, shared
atomic truth, narrow hardware capabilities, and close correspondence between
AM32 execution contexts and extracted Rust functions. rm32 is the more
general architecture: explicit ownership domains, a named atomic mailbox,
finer HAL responsibilities, reusable platform adapters, declarative boards,
persistent configuration, and a broader host harness.

Their algorithmic overlap is substantial, but their state/concurrency models
are not interchangeable. Any transfer between them should preserve the
zero-cross/commutation timing semantics while deliberately choosing either
MinZ's shared atomic-cluster model or rm32's owned-context/message-bridge
model.
