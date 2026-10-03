# Task: rebuild binz as a small typed firmware crate

## Objective

Create a new standalone firmware package under `binz/` that reproduces the
currently qualified G071/DRV8304 behavior through the 50% operating point,
without inheriting the historical experiment crate's feature matrix.

The existing `binz/` package, captures, frozen ELF, and lab notebook remain the
behavioral and forensic reference. Do not rewrite them during this task.

## Package shape

Create an independent package, for example `binz/firmware50/`:

```text
firmware50/
  Cargo.toml
  .cargo/config.toml
  src/
    lib.rs
    bemf.rs
    commutation.rs
    protection.rs
    restart.rs
    telemetry.rs
    policy.rs
  bin/
    shell-pwm.rs
    uart-echo.rs
```

Use `bin/` for genuinely separate firmware personalities or entry points.
Do not create a feature flag for each experiment. `examples/` may be used for
host-side or disabled hardware tests, but production firmware entry points
belong in `bin/`.

The new package must build independently with an explicit `--manifest-path`.
It must not become a member of the parent `rm32` workspace and must not depend
on the historical `binz` package.

## Configuration design

Use one canonical production type composition rather than Cargo feature
combinations:

```rust
type Production = Controller<
    BemfPolicy,
    CurrentProtection,
    BusSagProtection,
    RestartPolicy,
    CompactTelemetry,
>;
```

Use marker types and traits for policy/personality choices. Use const generics
only for small bounded numeric parameters such as PWM frequency or persistence
depth. Group related policy decisions into meaningful types; do not create a
large boolean or const-generic parameter list.

`#[cfg]` should be rare and structural only: target-specific HAL glue,
unavailable hardware peripherals, or an entire separately built diagnostic
binary. Normal control behavior, protection policy, telemetry ownership, and
commutation choices must be represented in ordinary Rust types and values.

## Functional starting point

Port only behavior demonstrated by the frozen qualified image:

- sensorless BEMF comparator path and commutation scheduling;
- current protection/foldback semantics;
- fast bus-sag hard stop;
- nFAULT, tracking, watchdog, and all-off handling;
- ordinary restart/startup and retained-duty restoration;
- compact post-run telemetry sufficient to explain a stop.

Do not port historical probes, abandoned A/Bs, UART capture machinery, or
one-off campaign policies unless a current acceptance test requires them.

## Build requirements

The production profile must be release-optimized for the M0:

- `opt-level = "s"` or `"z"` based on measured image/timing results;
- thin LTO;
- one codegen unit;
- panic/formatting choices appropriate for the target;
- no reachable `__aeabi_uidiv`, `__aeabi_uidivmod`, or other forbidden soft
  arithmetic from the motor ISR roots;
- reproducible ELF hash and size report.

Add a small linker/disassembly audit for the four motor roots: COMP, COM, DMA,
and guard. Keep the audit fail-closed when a reachable helper or unbounded loop
is discovered.

## Verification gates

Before any powered run:

1. Host unit tests for every policy and protection state transition.
2. Cross-build the production binary for `thumbv6m-none-eabi`.
3. Run the ISR arithmetic/linker audit and inspect the release disassembly.
4. Run disabled gate/ENABLE/MOE/CCR/nFAULT preflights.
5. Verify the new binary hash and retain the build manifest.

On the bench, reproduce the reference campaign in this order:

- 10% protected hold, three runs;
- 25% protected hold, three runs;
- 50% protected hold, three runs;
- ordinary-start recovery at 50%, three runs;
- final outputs-off and nFAULT-high evidence after every run.

Only after that baseline is reproduced should the new crate resume envelope
discovery above 50% or add diagnostic instrumentation.

## Non-goals and boundaries

- Do not edit, build, flash, or refactor sibling `../rm32` or `../rm32_stm32`.
- Do not weaken current, bus-sag, nFAULT, tracking, watchdog, or all-off
  protections to make the new crate pass.
- Do not treat the old feature matrix as a compatibility contract.
- Do not claim parity from a diagnostic build whose observer changes ISR timing.
- Do not delete historical captures or the lab notebook.

## Deliverables

- standalone `firmware50` package and documented build command;
- one production `bin/shell-pwm.rs` image;
- optional separate diagnostic binaries, each with a stated purpose and cost;
- release ELF, SHA-256, size report, and ISR arithmetic audit;
- host tests and the reproduced 10/25/50% qualification records;
- a short migration note mapping retained behavior to the old frozen image.
