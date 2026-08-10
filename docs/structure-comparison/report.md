# MinZ versus rm32 structural comparison

## Executive read

Both codebases put host-testable motor logic on one side of an ESC-specific
hardware boundary and Cortex-M register/interrupt code on the other. Their
fast paths use static dispatch, and their AM32-derived control loops share the
same broad temporal skeleton: comparator zero-cross detection, delayed
commutation, a roughly 20 kHz control tick, and a spinning foreground loop
([MinZ HAL](../../minz/core/src/am32_hal.rs#L1-L20),
[rm32 HAL](../../rm32/src/hal.rs#L68-L84),
[MinZ contexts](../../minz/examples/am32_clone.rs#L19-L33),
[rm32 handlers](../../rm32_stm32/src/isr_handlers.rs#L71-L146)).

That overlap should not obscure three different implementation profiles:

- **MinZ `am32_clone`** is a compact, fixed-board, forward-only AM32
  transliteration. Program-owned atomics are grouped into four borrowed state
  views, and complete parameterized control/ISR bodies run behind a
  seven-capability HAL
  ([scope](../../minz/examples/am32_clone.rs#L1-L17),
  [state](../../minz/examples/am32_clone.rs#L103-L152),
  [clusters](../../minz/core/src/am32_loop.rs#L74-L137)).
- **MinZ `motor_tester2`** is a distinct interactive bench and closed-loop
  experiment. It reuses many extracted algorithms and selected atomic views,
  but retains large ISR bodies, hardware sequencing, queues, diagnostics, and
  hundreds of statics in the example
  ([purpose](../../minz/examples/motor_tester2.rs#L20-L49),
  [core views](../../minz/examples/motor_tester2.rs#L4686-L4811),
  [COMP path](../../minz/examples/motor_tester2.rs#L6315-L6377)).
- **`rm32` + `rm32_stm32`** is a portable product-shaped core plus a
  multi-MCU platform crate. Mutable main and ISR state are predominantly owned
  separately and communicate through directional atomic interfaces; board data
  is generated from YAML
  ([crate split](../../Cargo.toml#L1-L7),
  [state aggregates](../../rm32_stm32/src/isr.rs#L70-L101),
  [communication boundary](../../rm32/src/shared_comm.rs#L30-L223),
  [board generator](../../rm32_stm32/build.rs#L210-L309)).

The strongest structural difference is therefore **shared atomic control
storage versus owned execution contexts**. The most important current risk is
not stylistic: rm32's L431 interrupt priorities already violate `IsrCell`'s
documented equal-priority exclusivity premise
([premise](../../rm32_stm32/src/isr_handlers.rs#L12-L39),
[actual priorities](../../rm32_stm32/src/mcu_l431/init.rs#L219-L249)).

## Side-by-side inventory

| Dimension | MinZ `am32_clone` | MinZ `motor_tester2` | `rm32` / `rm32_stm32` |
|---|---|---|---|
| Packaging | Fixed-target firmware crate depending on detached `minz-core`; operational program is a Cargo example ([manifests](../../minz/Cargo.toml#L13-L25), [entry](../../minz/examples/am32_clone.rs#L432-L433)). | Same firmware/core crates, but a separate example entry and architecture ([entry](../../minz/examples/motor_tester2.rs#L2028-L2060)). | Root-workspace portable core plus excluded target crate and one normal firmware binary ([workspace](../../Cargo.toml#L1-L7), [firmware entry](../../rm32_stm32/src/bin/main.rs#L1-L7)). |
| Target/board | STM32L431, memory, probe, pins, and peripheral allocation are checked-in constants ([target](../../minz/.cargo/config.toml#L1-L6), [pins](../../minz/src/board_init.rs#L59-L103)). | The same L431 platform, with experiment choices embedded as source flags and constants ([switches](../../minz/examples/motor_tester2.rs#L1185-L1268)). | Four MCU features feed an active-family gateway; fourteen board YAML files are parsed into `BoardConfig` ([features](../../rm32_stm32/Cargo.toml#L38-L43), [gateway](../../rm32_stm32/src/mcu.rs#L22-L41), [schema](../../rm32_stm32/build.rs#L47-L85)). |
| State | Shared atomic globals grouped as `Sched`, `Drive`, `Duty`, and `Bench`; clusters borrow storage rather than own it ([storage](../../minz/examples/am32_clone.rs#L103-L152), [views](../../minz/core/src/am32_loop.rs#L74-L137)). | Large example-owned atomic banks plus heapless queues, critical-section producer handles, ISR-local startup state, and selected core views ([queues](../../minz/examples/motor_tester2.rs#L1628-L1692), [views](../../minz/examples/motor_tester2.rs#L4686-L4811)). | `MainState`, `IsrState`, and atomic `SharedState`; `SharedComm` names direction-specific traffic. Configuration is nevertheless duplicated across main and ISR contexts ([main](../../rm32/src/main_state.rs#L90-L127), [ISR](../../rm32_stm32/src/isr.rs#L70-L88), [mailbox](../../rm32/src/shared_state.rs#L35-L74)). |
| HAL boundary | Seven narrow traits bundled into generic `Hal`; zero-sized target adapters and one call-recording mock ([traits](../../minz/core/src/am32_hal.rs#L22-L137), [mock](../../minz/core/src/am32_hal.rs#L139-L191)). | No equivalent bundle: the example calls concrete PWM/comparator/timer functions around extracted pure decisions ([explicit scope note](../../minz/src/tim1_motor_pwm.rs#L423-L426), [direct effects](../../minz/examples/motor_tester2.rs#L4539-L4553)). | Broad domain traits, `MotorHal` for the fast subset, then lower register-operation traits and reusable drivers in the platform crate ([domain HAL](../../rm32/src/hal.rs#L14-L152), [lower comparator seam](../../rm32_stm32/src/comparator.rs#L1-L28), [timer seam](../../rm32_stm32/src/timer.rs#L14-L61)). |
| Dispatch/ownership signal | HAL methods mostly take `&self`, fitting shared zero-sized/register-backed facades ([contracts](../../minz/core/src/am32_hal.rs#L22-L121)). | Direct module calls make exclusivity and effect ordering an example-level concern ([TIM7 body](../../minz/examples/motor_tester2.rs#L4116-L4137)). | Motor traits use mutable accessors and live in the owned ISR context; slower ADC/UART paths use trait objects ([bundle](../../rm32/src/hal.rs#L68-L84), [dynamic main ports](../../rm32/src/main_state.rs#L263-L269)). |
| Orchestration | `main_entry` orders foreground bands; vector trampolines delegate COMP/TIM16/TIM6 bodies into `minz-core` ([main](../../minz/examples/am32_clone.rs#L322-L360), [vectors](../../minz/examples/am32_clone.rs#L554-L584)). | Foreground and seven ISR bodies jointly orchestrate the system; extracted helpers do not form one runtime object ([TIM7/LPTIM2 roles](../../minz/examples/motor_tester2.rs#L4116-L4137), [commutation ISR](../../minz/examples/motor_tester2.rs#L5027-L5034)). | `SystemTick` shares input/callback/synchronization/`MainState` ordering with the harness, while sine MMIO, LEDs, priority changes, persistence, reset, watchdog, and debug remain in firmware main ([shared slice](../../rm32/src/system.rs#L153-L172), [outer loop](../../rm32_stm32/src/bin/main.rs#L379-L481)). |
| Configuration | Factory-default constants plus a centralized `pwm48` carrier feature; no board manifest or persistent configuration engine ([feature](../../minz/Cargo.toml#L6-L11), [timing constants](../../minz/src/lib.rs#L92-L103)). | Adds source-level experimental switches rather than a unified configuration layer ([switches](../../minz/examples/motor_tester2.rs#L1185-L1268)). | Build-time `BoardConfig`, 192-byte flash-backed `EepromConfig`, migration/default logic, and runtime motor configuration ([board](../../rm32/src/board.rs#L18-L71), [persistent layout](../../rm32/src/config.rs#L17-L85), [load](../../rm32_stm32/src/bin/main.rs#L196-L207)). |
| Output invariant | Duty writes put the same value in all CCRs; commutation only rotates GPIO phase roles ([rationale and table](../../minz/src/tim1_motor_pwm.rs#L205-L246), [operations](../../minz/src/tim1_motor_pwm.rs#L248-L275)). | Reuses those platform functions directly ([adapter scope](../../minz/src/tim1_motor_pwm.rs#L423-L426)). | `PwmOutput` and `PhaseOutput` are separate traits and are reunited in `MotorHal` ([traits](../../rm32/src/hal.rs#L14-L84)). |
| Safety/observability | Latched kill turns output off, masks comparator interrupts, and freezes a fixed black box; ZC trace is bounded ([kill](../../minz/core/src/am32_control.rs#L219-L249), [trace](../../minz/core/src/zct_trace.rs#L30-L75)). | Broader guard, window, ZC, and diagnostic stack, but much of its integration remains in the example ([guard surface](../../minz/core/src/guards.rs#L412-L443), [wiring](../../minz/examples/motor_tester2.rs#L4686-L4811)). | Protection state/actions plus platform `EmergencyOff` and panic-safe-off; checked firmware currently disables stuck/stall protection and suppresses timeout reset ([protection](../../rm32/src/control/state.rs#L296-L325), [panic](../../rm32_stm32/src/panic.rs#L1-L20), [bench overrides](../../rm32_stm32/src/bin/main.rs#L209-L218)). |

## Abstraction overlap and different names

| Shared concept | MinZ name/boundary | rm32 name/boundary | Relationship |
|---|---|---|---|
| Motor hardware bundle | `am32_hal::Hal`, with `MotorPwm`, `CompCtl`, combined `ComTimers` ([source](../../minz/core/src/am32_hal.rs#L22-L137)) | `MotorHal`, with separate `PwmOutput`, `PhaseOutput`, `Comparator`, `IntervalTimer`, `ComTimer` ([source](../../rm32/src/hal.rs#L14-L84)) | Same static-dispatch intent; rm32 splits actuator and timer responsibilities more finely. |
| Commutation/BEMF truth | `Drive` plus timing fields in `Sched` ([source](../../minz/core/src/am32_loop.rs#L82-L111)) | `Commutation`, `BemfState`, `MotorMode`, and selected shared timing fields ([commutation](../../rm32/src/commutation.rs#L3-L12), [BEMF](../../rm32/src/control/state.rs#L8-L22), [mode](../../rm32/src/motor_mode.rs#L1-L87)) | Conceptual overlap, different ownership and transition enforcement. |
| Duty/ramp | `Duty` borrowed atomic cluster ([source](../../minz/core/src/am32_loop.rs#L113-L125)) | Owned `DutyState`, with selected values published through `SharedState` ([source](../../rm32/src/control/state.rs#L24-L40)) | Closest direct state correspondence. |
| Main/ISR bridge | The same global atomics are visible through borrowed clusters ([source](../../minz/examples/am32_clone.rs#L237-L303)) | `SharedComm` directional traits implemented by atomic `SharedState` ([interface](../../rm32/src/shared_comm.rs#L30-L223), [implementation](../../rm32/src/shared_state.rs#L592-L768)) | Fundamental structural mismatch. |
| Foreground pipeline | `main_entry` bands and program vectors ([source](../../minz/examples/am32_clone.rs#L322-L360)) | `SystemTick` shared slice plus platform work around it ([core](../../rm32/src/system.rs#L153-L172), [platform](../../rm32_stm32/src/bin/main.rs#L379-L481)) | Similar responsibilities; only rm32 exposes a reusable process-harness slice. |

`Bench` is not a clean counterpart to one rm32 type: it mixes ADC samples,
kill latches, accumulators, requests, and trace controls that rm32 distributes
among `Measurements`, `ProtectionState`, `SharedState`, and platform logging
([MinZ fields](../../minz/core/src/am32_loop.rs#L127-L137),
[rm32 state](../../rm32/src/control/state.rs#L296-L376),
[shared fields](../../rm32/src/shared_state.rs#L35-L74)).

## Major runtime flows

### MinZ `am32_clone`

1. Boot configures the fixed UART/ADC/COMP/TIM1/TIM2/TIM16/TIM6/watchdog
   assignment, installs priorities, and enters `main_entry`
   ([setup](../../minz/examples/am32_clone.rs#L432-L551)).
2. USART2 pushes bytes into a fixed ring; foreground parses UART duty and
   publishes setpoint state; TIM6 performs ramp/rescale and writes all CCRs
   ([RX service](../../minz/src/usart2_rx.rs#L38-L51),
   [foreground](../../minz/examples/am32_clone.rs#L338-L369),
   [duty path](../../minz/core/src/am32_isr.rs#L151-L210)).
3. Startup polls BEMF in TIM6. An accepted crossing blends interval timing,
   commutates, and switches to comparator interrupts below the changeover
   threshold ([polling](../../minz/core/src/am32_isr.rs#L214-L250),
   [handoff](../../minz/core/src/am32_control.rs#L153-L216)).
4. In interrupt mode, COMP qualifies the crossing and arms TIM16; TIM16 rotates
   phase roles/input, updates timing, records trace, and re-enables COMP
   ([COMP](../../minz/core/src/am32_isr.rs#L33-L99),
   [TIM16](../../minz/core/src/am32_isr.rs#L107-L143)).

### MinZ `motor_tester2`

Foreground owns queues, UI, capture, and bench orchestration; TIM7 supplies the
6 kHz start/drive heartbeat; COMP performs detailed window/guard/qualification;
LPTIM2 executes delayed commutation; TIM6 hosts migrated periodic guard and
sampling work
([entry](../../minz/examples/motor_tester2.rs#L2028-L2060),
[TIM7](../../minz/examples/motor_tester2.rs#L4116-L4137),
[LPTIM2](../../minz/examples/motor_tester2.rs#L5027-L5034),
[COMP](../../minz/examples/motor_tester2.rs#L6315-L6377)).
Core helpers decide many transitions, but the example still orders their
hardware effects, so this is partial extraction rather than a second use of
the `am32_clone` HAL architecture
([core composition](../../minz/examples/motor_tester2.rs#L4887-L4922),
[HAL scope note](../../minz/src/tim1_motor_pwm.rs#L423-L426)).

### rm32 / rm32_stm32

1. Boot loads and transforms `EepromConfig`, constructs `MainState` and
   `IsrState`, stages ISR state, arms capture DMA, and enables interrupts
   ([configuration](../../rm32_stm32/src/bin/main.rs#L196-L255),
   [handoff](../../rm32_stm32/src/bin/main.rs#L257-L270)).
2. TIM6 builds `MotorContext` and calls portable `ten_khz_tick`; comparator and
   commutation handlers call portable zero-cross and timer-expiry algorithms
   ([context](../../rm32_stm32/src/isr_handlers.rs#L71-L119),
   [COMP](../../rm32_stm32/src/isr_handlers.rs#L127-L140),
   [core algorithms](../../rm32/src/control/isr_logic.rs#L216-L298)).
3. Input DMA/EXTI detects DShot or servo, publishes throttle/actions, and may
   send bidirectional DShot/EDT responses
   ([transfer path](../../rm32_stm32/src/isr_handlers.rs#L148-L335)).
4. Foreground runs sine work if active, calls the shared `SystemTick` slice,
   then performs LEDs, IRQ-priority adjustment, save/info side effects, reset
   policy, watchdog, and debug work
   ([firmware loop](../../rm32_stm32/src/bin/main.rs#L379-L481)).

## Important behavioral differences inside the overlap

**Comparator pending contract.** Both paths persistence-filter a comparator
level, capture/reset an interval timer, and arm one-shot commutation. MinZ can
leave a post-cross event pending while its interval gate is closed and unmasks
without clearing it; rm32 L431 clears and masks on ISR entry. The rm32 core
explicitly warns that F051/G071/G431 do not implement the L431 entry mitigation
([MinZ gate](../../minz/core/src/am32_isr.rs#L47-L61),
[MinZ unmask](../../minz/src/comp2.rs#L244-L311),
[rm32 warning](../../rm32/src/control/isr_logic.rs#L268-L280),
[L431 wrapper](../../rm32_stm32/src/mcu_l431/interrupts.rs#L27-L49)).

**Step numbering and physical rotation.** MinZ keeps logical steps 1..6 but
passes `step - 1` into 0..5 phase/comparator adapters; rm32 adapters accept
1..6. On the concrete L431 pin and sense tables there is also a one-step
physical rotation: rm32 step 1 matches MinZ logical step 6, rm32 step 2 matches
MinZ logical step 1, and so on
([MinZ conversion](../../minz/core/src/am32_control.rs#L45-L63),
[MinZ phase table](../../minz/src/tim1_motor_pwm.rs#L231-L240),
[rm32 phase table/alias](../../rm32_stm32/src/phase.rs#L111-L148),
[rm32 sense table](../../rm32_stm32/src/comparator.rs#L28-L44)).

**Dead time is selection-dependent.** MinZ hard-codes TIM1 DTG 45. rm32's
default `stm32l431` selection is `neutron_l431.yaml`, also 45; only an explicit
`BOARD=...vimdrones_l431.yaml` selects 60. This proves separate configuration
sources and possible board-selection drift, not an unconditional default-build
mismatch
([MinZ constant](../../minz/src/lib.rs#L100-L103),
[default selection](../../rm32_stm32/build.rs#L214-L231),
[Neutron](../../rm32_stm32/boards/neutron_l431.yaml#L1-L4),
[VimDrones](../../rm32_stm32/boards/vimdrones_l431.yaml#L1-L4)).

## Declared breadth versus end-to-end wiring

rm32 declares a broader product surface than `am32_clone`: multiple input
modes, DShot commands, EDT, CRSF, bidirectionality, sine start, braking,
protections, and three PID owners are present in core types/modules
([input/config](../../rm32/src/config.rs#L5-L78),
[PID owners](../../rm32/src/control/state.rs#L198-L213),
[mode machine](../../rm32/src/motor_mode.rs#L1-L87)).
The examined firmware wiring is narrower:

- DShot, servo, commands, EDT, and bidirectional transfer have active target
  paths ([input actions](../../rm32_stm32/src/isr_handlers.rs#L242-L335)).
- CRSF has a parser and shared byte handler, but no checked MCU vector caller;
  it is structurally present, not proven end-to-end enabled
  ([handler](../../rm32_stm32/src/isr_handlers.rs#L369-L381),
  [audited caller search](audit-crosscut.yaml#L225-L232)).
- Boot currently clears stuck-rotor, stall, bidirectional, sine, and brake
  settings, and timeout reset is suppressed for bench debugging
  ([overrides](../../rm32_stm32/src/bin/main.rs#L209-L218),
  [reset policy](../../rm32_stm32/src/bin/main.rs#L451-L465)).

Conversely, `motor_tester2` has broader experiments than `am32_clone`, but they
belong to a bench personality rather than a unified MinZ product configuration
([controls](../../minz/examples/motor_tester2.rs#L20-L49),
[experiment switches](../../minz/examples/motor_tester2.rs#L1185-L1268)).

## Verification and current risks

MinZ's detached core reported 258 passing test executions, but a duplicated
`#[test]` causes one double execution and a test-shaped function is not
annotated. Release target checks passed for both primary examples; these are
type checks, not flashed or bench runs
([test source](../../minz/core/src/zc.rs#L1490-L1516),
[audit results](audit-minz.yaml#L363-L380)).

rm32 reported 253 passing unit tests and all four firmware cross-build shapes
passed. Its native harness shares the `SystemTick` core slice with firmware but
runs the control ISR synchronously and omits surrounding platform work
([harness call](../../rm32/src/bin/harness.rs#L463-L494),
[firmware call](../../rm32_stm32/src/bin/main.rs#L379-L407),
[audit results](audit-rm32.yaml#L444-L478)).
The current local black-box run is **77 passed, 1 failed**:
`temperature_limit` still had `duty_cycle_maximum=2000` at tick 2
([recorded run](audit-rm32.yaml#L469-L478)).

Current structural/integration risks, in priority order:

1. **Critical — L431 `IsrCell` soundness.** `get(&self) -> &mut
   TargetIsrState` assumes equal-priority non-preempting callers, yet COMP/TIM16,
   DMA, EXTI, and TIM6 use priorities 0/1/2/3 and all relevant handlers obtain
   the same mutable aggregate
   ([cell](../../rm32_stm32/src/isr_handlers.rs#L12-L69),
   [callers](../../rm32_stm32/src/isr_handlers.rs#L71-L180),
   [priorities](../../rm32_stm32/src/mcu_l431/init.rs#L225-L249)).
2. **High — comparator acknowledgement differs by rm32 family.** Only L431
   applies the documented entry clear/mask required to make filter early-return
   safe ([warning](../../rm32/src/control/isr_logic.rs#L268-L280),
   [other wrappers](../../rm32_stm32/src/mcu_g071/interrupts.rs#L18-L20)).
3. **High — split configuration persistence.** DShot/servo commands mutate the
   ISR config copy, while foreground flash save writes `main_state.config`; the
   missing publication is an explicit TODO
   ([mutation](../../rm32_stm32/src/isr_handlers.rs#L276-L323),
   [save](../../rm32_stm32/src/bin/main.rs#L433-L440)).
4. **High — build selection integrity.** `BOARD` can disagree with the enabled
   MCU feature, and exactly-one MCU feature is intended but not enforced
   ([board selection](../../rm32_stm32/build.rs#L214-L245),
   [feature fan-out](../../rm32_stm32/src/mcu.rs#L22-L32)).
5. **Medium — timing and coverage.** rm32's “1 kHz” dispatch uses a strict
   `> 20` test, yielding one dispatch per 21 TIM6 increments (about 952.4 Hz at
   exactly 20 kHz), and both platform layers remain compile/bench validated
   rather than automatically runtime-tested
   ([divider](../../rm32/src/constants.rs#L28-L31),
   [counter](../../rm32/src/shared_state.rs#L230-L243),
   [CI boundary](../../.github/workflows/ci.yml#L39-L90)).
6. **Medium — MinZ integration concentration.** `motor_tester2`'s broad core
   extraction does not cover its full ISR transactions, while `am32_clone`
   remains fixed to one MCU/board and a factory subset
   ([tester ISR](../../minz/examples/motor_tester2.rs#L6315-L6377),
   [clone scope](../../minz/examples/am32_clone.rs#L5-L17)).

Finally, the comparator-polarity warning in `minz/AGENTS.md` is useful history,
not a present `motor_tester2` defect: the current tester locks `POLARITY=0`, so
its parity-derived expected level is coherent. The warning becomes active again
if dynamic polarity switching returns
([historical caveat](../../minz/AGENTS.md#L99-L111),
[current lock](../../minz/examples/motor_tester2.rs#L4539-L4544),
[expected level](../../minz/examples/motor_tester2.rs#L6561-L6567)).

## Limits of this comparison

This is a checked-in structural review. It does not validate external AM32
behavior, generated binaries, interrupt latency, PAC side effects, DMA
relocation, electrical phase identity, or motor safety on hardware. MinZ's
probe/serial scripts require the physical bench, while rm32 CI executes core
tests/harness vectors and cross-builds platform targets rather than running the
STM32 layer
([MinZ register tool](../../minz/scripts/clone_regcheck.py#L1-L18),
[rm32 CI](../../.github/workflows/ci.yml#L28-L90)).
