# rm32 integration restart — bringup plan v2

How to restart the rm32 integration on the Vimdrones L431 bench. Companion to
`docs/rm32-parity-action-map.md` (strategy + parity gap details); this file is
the operational rung ladder. Ordering logic: nothing that changes motor
behavior lands until the instruments that can see motor behavior are in place,
and nothing lands at all until the baseline is tagged.

## Rung 0 — baselines, no code changes

1. **Vector suite green** on current tree (`AM32_HARNESS` pointed at
   `target\release\rm32_harness.exe` on Windows). Triage the known
   `temperature_limit` failure. Tag.
2. **Flash current rm32 L431 `debuguart` build**, confirm the motor spins as it
   does today (choppy at 40% *is* the baseline). Capture a `port_41.log`
   reference. Tag the image.
3. **Peripheral mapping audit.** With both firmwares available on the same
   board, dump live register state via the existing probe-rs tooling
   (`scripts/dump_l431_regs.py`, `minz/scripts/clone_regcheck.py`) and diff
   rm32 against the *clone's* known-good state — the clone is now a better
   reference than AM32 because it is the exact configuration we are converging
   to. Fix what diverges (usual suspects: TIM1 ARR, COMP config, NVIC
   priorities). This answers "are the mappings right" mechanically, not by
   eyeball.
4. **Write down the `IsrCell` invariant.** Audit which ISRs touch `ISR_LOCAL`
   (`rm32_stm32/src/isr_handlers.rs:16-69`); either fix the priority-aliasing
   now or commit to the rule that every new bench ISR is ring-push-only and
   never touches `ISR_LOCAL`. Deferring the full fix is acceptable *only* with
   that rule enforced — the bench UART design below doesn't need shared ISR
   state.

## Rung 1 — the bench wire as a cfg-switch (`benchuart`)

The right first port: everything after it becomes measurable with the existing
operator toolchain. One rm32_stm32 feature that:

- **Skips input-capture init entirely** (TIM15/DMA1_CH5/EXTI15_10 never armed)
  — PA2 freed for USART2 RX @2 Mbaud (SWAP). DSHOT/PWM input and bench UART
  are build-time mutually exclusive; that is the cfg-switch.
- **Takes USART1/PB6** for the 4 KiB self-healing DMA TX ring @2 Mbaud
  (displacing KISS telemetry / 115200 debuguart — one owner per build).
- **Ports from minz**: `spin.rs` (bounded waits + timeout counter, fold into
  `rm32_stm32/src/regs.rs::wait_for`), `uart_tx.rs` (DMA ring w/ stale-EN /
  lost-completion repair), `usart2_rx.rs` (ring-push-only RX ISR), and the
  `UartDuty` ASCII parser — parser as a portable module so its host tests come
  along.
- **Throttle injection** into the same shared input path DShot decode feeds
  (`input`, `input_set`, arming) so arming/ramp/LVC behave identically. Plus
  the 3-second UART deadman.
- **Watchdog on** in the bench build (rm32's `start_watchdog` is commented out
  in `bin/main.rs`; minz runs IWDG ~1 s live — part of why its panics are
  recoverable and visible).

Exit check: motor runs under `uart_cmd.py` / `fly.py`; all four cross-builds
green; vectors untouched (all changes firmware-side).

## Rung 2 — last reboot reason + info line

Cheapest observability first:

- rm32 already has `read_and_clear_reset_cause()` per MCU — surface it in the
  boot banner over the bench wire (nearly free).
- Emit an `i` info line in **minz's exact field format** so `bench.py` and the
  plot scripts parse rm32 without modification.

## Rung 3 — blackbox

Port the 64-event freeze-on-fault core (`minz/core/src/blackbox.rs`) into the
rm32 crate as an additive module (host tests included); DWT-timestamp adapter
(`minz/src/bb.rs`) into rm32_stm32 behind a `blackbox` feature (M4 targets
only). Wire freeze to the existing LVC/`IsrAction::AllOff` path. Record
commutation (REF), zero-cross (ACC), desync (DSY), mode-transition, and
IsrAction events. `b` dump command. This tool made every minz fault legible —
it must be in place *before* the control-parity rungs, not after.

## Rung 4 — autopsy stream (`zctrace`)

Port `zct_trace` + record packing (`minz/core/src/zct_trace.rs`, packing in
`am32.rs`), producers in `handle_tim14` / `handle_comp`, `Z` toggle. Wire
format **byte-identical** to the 15-byte protocol so `zctrace_capture.py`,
`plot_lock_map.py`, the dropout plots, and the whole bench-study comparison
pipeline work unchanged against rm32.

## Rung 5 — only now, control parity

Start the Phase 2 ladder from `docs/rm32-parity-action-map.md`:

1. advance mapping (`advance_level → temp_advance`; rm32 currently runs 0°),
2. COMP half-interval gate + pending-bit camping,
3. polling/interrupt exclusivity + slowdown fallback + startup seeds,
4. recovery authority (desync duty kick-down, active re-kick),
5. confirmation thresholds / EEPROM derivation.

One rung per behavioral change, measured with lock map + dropout tail, judged
against the archived am32_clone maps (not AM32 — see the inherited open items
in the action map).

## Deliberately deferred

**Injected-ADC phase observer** (`minz/src/adc_sync.rs`): the most invasive
port (ADC trigger timing, the dormant-but-load-bearing regular-group config),
and none of rungs 0–5 need it. It joins after parity holds, when current/vbat
telemetry on the info line wants upgrading from rm32's existing ADC path.
