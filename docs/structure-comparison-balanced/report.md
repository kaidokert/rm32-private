# Balanced deep dive: MinZ `am32_clone` vs `rm32` / `rm32_stm32`

## Scope and evidence

This is a current-tree comparison of the simplified MinZ AM32 clone against the
combined rm32 portable core and STM32 firmware. It compares production
architecture, runtime wiring, hardware boundaries, protocols, safety,
observability, and verification. Deleted and out-of-scope historical files are
not used as evidence.

The file-level source of truth is:

| Side | In-scope artifacts | Exhaustive inventory | Cross-check |
| --- | ---: | --- | --- |
| MinZ | 31 | [minz-modules.yaml](minz-modules.yaml) | 31/31 paths accounted for; no missing, extra, or duplicate paths |
| rm32 + rm32_stm32 | 211 | [rm32-modules.yaml](rm32-modules.yaml) | 211/211 paths accounted for; no missing, extra, or duplicate paths |

The inventories are reconciled through [crosscut-map.yaml](crosscut-map.yaml).
The three independent second-wave reviews are preserved in
[audit-minz.yaml](audit-minz.yaml), [audit-rm32.yaml](audit-rm32.yaml), and
[audit-crosscut.yaml](audit-crosscut.yaml); [audit.yaml](audit.yaml) records the
final reconciled verdict and verification results.

## Executive summary

The two sides now share a recognizable architectural spine. Both separate
portable six-step motor-control logic from STM32 hardware adapters, expose
static-dispatch motor HAL traits, divide work among comparator, commutation,
periodic, and foreground contexts, and host-test their portable logic. MinZ's
[`am32_hal.rs`](../../minz/core/src/am32_hal.rs#L26-L205) deliberately uses the
same major vocabulary as rm32's [`hal.rs`](../../rm32/src/hal.rs#L14-L68):
PWM output, phase output, comparator, interval timer, commutation timer, and a
motor bundle.

They are nevertheless different products. MinZ is a deliberately narrow,
single-board AM32 behavior clone and bench-observation image. Its strengths are
behavioral visibility, fixed L431 fidelity, explicit bounded waits, a live
watchdog, and compact control-semantic diagnostics. rm32 is a configurable ESC
product platform: four MCU families, fourteen generated board variants,
DShot/servo inputs, persistent settings, multiple telemetry protocols,
measurement and limiting loops, optional product modes, and a process-level
behavioral test harness.

The right conclusion is therefore not that one is a subset of the other.
Their motor-control centers overlap strongly, while each has meaningful
one-sided capabilities. Some rm32 capabilities are also only library,
manifest, test, or currently boot-disabled features; this report distinguishes
those from firmware-active behavior.

## Architectural topology

| Concern | MinZ `am32_clone` | rm32 / rm32_stm32 | Relationship |
| --- | --- | --- | --- |
| Portable control | Seven compact core modules centered on AM32 constants, control, ISR bodies, loop bands, drive-state views, black box, and ZC trace | A broader portable crate with control state, system orchestration, input protocols, modes, measurement, telemetry, configuration, and typed units | Direct at the six-step core; partial overall |
| Firmware composition | One L431 entry point wires fixed peripherals and borrowed atomic clusters in [`am32_clone.rs`](../../minz/examples/am32_clone.rs#L258-L388) | Target main composes generated board data, owned main/ISR states, settings, input capture, product services, and per-MCU adapters in [`main.rs`](../../rm32_stm32/src/bin/main.rs#L169-L255) | Partial |
| Hardware layer | Nine focused L431/platform modules plus the platform crate root | Shared adapters plus family-specific implementations for F051, G071, L431, and G431 | Direct on L431 boundaries; rm32 is much broader |
| State ownership | Independent atomics grouped into borrowed `Sched`, `Drive`, `Duty`, and `Bench` views; mostly Relaxed ordering | ISR-exclusive state, foreground `MainState`, and directional Acquire/Release mailboxes in [`shared_comm.rs`](../../rm32/src/shared_comm.rs#L89-L223) | Partial |
| Configuration | Fixed board and compile-time constants; `pwm48` is the only behavior feature | Persistent versioned settings, generated board constants, and Cargo-selected MCU/board features | Partial |
| Link boundary | Fixed L431 image at `0x08000000`, 256 KiB flash | Application at `0x08001000` with bootloader/EEPROM reservations and a 58 KiB application region | Same target family, different deployment contract |

MinZ's real-time path is intentionally easy to follow. A comparator edge enters
the portable comparator body, a commutation timer performs the delayed sector
change, TIM6 performs the lower-rate control/safety work, and foreground bands
apply commands and drain diagnostics. The corresponding rm32 path is more
distributed: family vector wrappers call shared ISR handlers, communicate
through typed shared state, and feed `SystemTick`/`MainState` product logic.

## Counterpart map

The reconciled map contains three unqualified direct structural matches and
eleven partial matches. “Partial” normally means that the same engineering
concern exists on both sides but the runtime contract, breadth, or ownership
model differs.

| Concept | Result | Important qualification |
| --- | --- | --- |
| Portable motor HAL | Direct | MinZ copies the central rm32 trait shape, then adds clone-specific seams such as recorder, critical-section, injected-ADC, and loop-timer traits |
| Commutation and BEMF control | Direct domain match | MinZ preserves AM32 comparator pending-bit camping; rm32 L431 clears and masks at ISR entry |
| L431 PWM, phase, and timers | Direct | rm32 additionally implements three other MCU families |
| Interrupt contexts | Partial | COMP, commutation, and TIM6 correspond; MinZ UART RX and rm32 DMA/frame-processing contexts are one-sided |
| Foreground orchestration | Partial | MinZ directly schedules a small AM32 band set; rm32 adds product modes, flash, LEDs, reset, and richer protection |
| State and communication | Partial | Borrowed atomic clusters versus owned state plus directional mailboxes |
| ADC and safety | Partial | MinZ observes a synchronous four-channel burst and applies hard current/vbat kills; rm32 feeds product measurement, telemetry, and modulating limits |
| Signal input | Partial concern, different protocols | Bench UART commands versus DMA pulse capture for DShot and servo PWM |
| Telemetry output | Partial concern, different protocols | ZC trace/text versus KISS, bidirectional DShot, EDT, and ESC-info |
| Observability | Partial | MinZ records semantic motor events and freezes on fault; rm32 records operational snapshots, counters, logs, and reset cause |
| Host verification | Partial | 77 MinZ core tests versus 253 rm32 core tests plus an executable process harness and 72 vector files |
| Target selection | Partial | One fixed L431 board versus a generated multi-MCU/multi-board matrix |

The black-box modules are not omitted or flattened into generic “logging.”
MinZ's [`blackbox.rs`](../../minz/core/src/blackbox.rs#L1-L66) owns the portable
64-event chronological ring and freeze semantics; [`bb.rs`](../../minz/src/bb.rs#L16-L35)
adapts it to on-target storage and text replay. rm32's nearest counterpart,
[`dbg_frame_history.rs`](../../rm32_stm32/src/dbg_frame_history.rs#L1-L50),
stores decoded input-frame snapshots. Both are bounded chronological
diagnostics, but they capture different domains and are therefore a partial,
not direct, counterpart.

## Capabilities present only on the MinZ side

| Capability | Current status | Nearest rm32 behavior |
| --- | --- | --- |
| Freeze-on-fault semantic motor-event black box | Active | Optional frame history and counters do not retain the same pre-fault control-event history |
| Continuous 15-byte per-commutation ZC trace with adaptive batching | Active | Product telemetry and debug logging, not a per-commutation stream |
| Bench UART throttle plus stop/info/trace/black-box commands | Active | Product RC inputs; CRSF is declared but unwired |
| PWM-synchronous phase-A, phase-B, current, and vbat injected ADC burst | Active | Product ADC measurements without the same exposed phase observer |
| Central bounded hardware-wait helper with cumulative timeout telemetry | Active | rm32 has a bounded register wait helper, but not MinZ's continue-after-timeout counter contract |
| Independently enabled watchdog in the compared firmware | Active | rm32 implements watchdog support but current main does not start it |
| Regular ADC/DMA setup and 4 KiB current ring retained for bench behavior | Configured but not consumed | No corresponding retained dormant path |
| 4 KiB DMA diagnostic UART ring with stale-EN/lost-completion repair | Active | UART/DMA facilities without this self-repair contract |
| Comparator pending-bit camping until the half-interval gate opens | Active | L431 clears/masks on entry; non-L431 handling has a separate acknowledgement concern |

These are not merely filenames. For example, the injected ADC path is actively
triggered and harvested, although the current loop consumes current and vbat
while discarding the phase samples. The dormant regular ADC/DMA group is
explicitly classified as configured-but-unwired rather than production signal
processing.

## Capabilities present only on the rm32 side

| Capability | Current status |
| --- | --- |
| Four MCU families and fourteen generated board variants | Active/build-selectable |
| DShot150/300/600 and bidirectional DShot eRPM/GCR telemetry | Active |
| Servo PWM input, calibration, bidirectional and RC-car mappings | Active |
| DShot command handling and EDT scheduling | Active |
| Versioned persistent EEPROM configuration and migration | Active, with a foreground/ISR-copy persistence caveat |
| Sine startup and six-step changeover | Wired, but forcibly disabled by current boot configuration |
| Voltage, current, temperature, and speed measurement/limiting machinery | Active/config-dependent; speed PID is harness-only |
| Stall and stuck-rotor protection | Implemented and tested, but forcibly disabled by current boot configuration |
| Startup sounds and reset-cause reporting | Active |
| WS2812 status output | Active on supporting boards |
| Bootloader-oriented signal-timeout reset flow | Wired, but currently suppressed |
| Executable stdin/stdout mock-HAL harness and 72 behavioral vectors | Active test infrastructure |
| Brushed and fixed-duty/fixed-speed modes | Library-only |
| CRSF parser and shared byte handler | Declared but no target UART RX vector wires it |
| DroneCAN dependency/feature | Manifest-only |
| KISS telemetry and ESC-info packets | Active |
| MultiShot conversion helper | Library-only/unwired |
| Typed voltage, current, temperature, and electrical-time quantities | Active portable abstraction |

DShot and PWM were inventoried on the rm32 side at both the portable and
hardware layers. DShot detection/decoding, command handling, bidirectional
encoding, EDT, pulse capture, and ISR integration are represented separately.
Likewise, “PWM” is not one item: the inventory distinguishes motor PWM/phase
drive, servo PWM input, calibration/mapping, timer capture, board/MCU adapters,
and the library-only MultiShot helper.

## Declared versus actually wired

The largest source of misleading comparisons is counting source presence as
live product behavior.

On MinZ, all 25 Rust source modules in the current scope are active in the
crate/build graph; six additional artifacts are configuration data. The
important internal qualification is the retained regular ADC/DMA path: it is
configured but not started or consumed. Some HAL methods also exist to preserve
the shared interface while the fixed clone uses a narrower subset.

On rm32, the 211 artifacts classify as 101 active, 4 active-optional, 3
library-only, 2 declared-unwired, 5 debug-only, 78 test-only, and 18
configuration-data artifacts. In particular:

- CRSF has a parser and callable handler but no current target UART receive
  vector invokes it.
- DroneCAN exists as a Cargo feature/dependency, not a firmware implementation.
- Brushed and fixed modes are library code, not selected by current firmware.
- MultiShot has conversion code but no active detector/capture route.
- External NTC support is declared, but every current board configuration
  disables it.
- Current/stall and normal product protections are reachable; speed PID is
  enabled only by the host harness.
- Current boot code forcibly clears stuck-rotor, stall, bidirectional,
  sine-start, and brake-on-stop settings, so those implemented branches are not
  live in the compared image.

## Safety and correctness findings

The comparison surfaced two particularly important cross-context issues.

First, MinZ's safety kill is not fully latched. `safety_kill` forces the bridge
off, disables comparator/commutation activity, freezes the black box, and sets
`killed = true` in
[`am32_control.rs`](../../minz/core/src/am32_control.rs#L182-L204). Foreground
request handling later consumes that notification with `swap(false, ...)` in
[`am32_clone.rs`](../../minz/examples/am32_clone.rs#L413-L428). TIM6 gates on
the current flag value in
[`am32_isr.rs`](../../minz/core/src/am32_isr.rs#L136-L166). That turns the
nominally latched fault into a consumable event and can make restart eligible.
Also, MinZ's panic reporter halts for watchdog recovery but does not itself
perform an explicit emergency bridge-off write in
[`panic.rs`](../../minz/src/panic.rs#L1-L32). The widespread Relaxed atomic
protocol deserves a separate ordering/ownership review.

Second, rm32's shared ISR state relies on an equal-priority non-nesting
assumption. `IsrCell` can yield mutable access to the same `TargetIsrState` from
multiple handlers in
[`isr_handlers.rs`](../../rm32_stm32/src/isr_handlers.rs#L12-L69), while the
L431 setup assigns priorities 0, 1, 2, and 3 in
[`init.rs`](../../rm32_stm32/src/mcu_l431/init.rs#L219-L249). Nested access
would violate the aliasing contract, making this the highest-severity finding.

Other high-priority rm32 findings are:

- Non-L431 comparator handlers do not mirror the L431 explicit pending-bit
  acknowledgement/masking behavior.
- Settings mutated in ISR-owned state can diverge from the foreground copy
  that persistence writes.
- Board selection and MCU feature selection can disagree, and the build does
  not enforce exactly one MCU feature.

These findings are comparison results, not proof that a failure has occurred
on hardware.

## Verification and limits

Current-tree verification succeeded for:

- MinZ core: 77 tests passed.
- MinZ firmware: release checks passed for the default and `pwm48` variants.
- rm32 portable core: 253 tests passed.
- rm32 firmware: release builds passed for G071, F051, L431, and G431 targets.

The rm32 process-vector suite discovered 72 vector files and ran 78 cases after
explicitly selecting the Windows `.exe` harness. It produced 77 passes and one
failure: `temperature_limit` retained `duty_cycle_maximum = 2000` at tick 2.
That failure is reported, not normalized away.

No probe, real ESC, motor, oscilloscope, or hardware-in-loop run was performed.
Runtime assertions about timer routing, comparator behavior, ADC timing,
watchdog reset, power-stage shutdown, and protocol waveforms remain
source/build/host-test conclusions until exercised on hardware.

## Bottom line

The simplified MinZ tree is now structurally close to rm32 where it intends to
be: portable six-step control, AM32 ISR decomposition, static HAL boundaries,
and L431 timer/phase hardware. It is not a general rm32 replacement. It trades
the product matrix for a focused clone with unusually strong bench
observability and fixed-target behavioral fidelity.

rm32 remains substantially broader in product input protocols, especially
DShot and servo PWM; board/MCU coverage; persistent configuration; telemetry;
protection loops; operating modes; and system-level verification. MinZ remains
distinct in its control-semantic frozen black box, continuous zero-cross trace,
interactive bench UART, synchronous phase observer, live watchdog, bounded
wait telemetry, and comparator camping behavior.

For future convergence, the most valuable work is not bulk module copying. It
is to resolve the two ownership/safety hazards, decide which one-sided
capabilities are intentional product boundaries, and promote only the desired
ones through explicit HAL and state contracts.
