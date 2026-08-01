# Public-release cleanup ledger (am32_sheet → semiclean)

Working ledger for staging the private `am32_sheet` tree onto the public
`semiclean` branch (`E:\m\robot\esc\rm32_clean`). Process per item: fix on
`am32_sheet` → validate (host tests + 4 cross-builds + bench where drive
behavior could move) → cherry-pick onto `semiclean`.

Backout round 1 is DONE (`e13be25` / cherry-pick `13e71f1`): dead
intervention levers ('O','T','Y','C','F','N','U','L'), healer, void
autopsy, first-anomaly latch, HW-timed injected scan. Bench-validated at
parity (2% ladder 10→100→10: dsy=0, exc=2/1.69M comms).

Legend: [ ] open · [x] done · [~] deferred/ticketed

## Tier A — production-breaking crutches (fix before any release)

- [x] **A1. Ungated bench EEPROM force-overwrite** — `bin/main.rs`
  ("BENCH: deterministic factory-baseline config, PERSISTED"). No
  `#[cfg]`: every production boot clobbered the user's saved config with
  the bench baseline and defeated the Configurator. Fix: gate block +
  the `[cfg xx]` hex dump under `benchuart`.
- [x] **A2. Self-reset on signal_timeout neutered everywhere** —
  `bin/main.rs` bottom: `sys.reset()` commented out ("RESET
  SUPPRESSED"). Breaks BF passthrough → AM32 Configurator in ALL
  builds; host test `signal_timeout_armed_requests_reset` asserts a flag
  firmware then ignores. Fix: restore reset for non-`benchuart`; keep
  suppression bench-only (bench has no DShot source — the chip would
  bounce every idle 2 s).
- [~] **A3. Arming beeps regressed** — old code played cell-count beeps
  on arm; now LED-only with TODO ("beeps need HAL access — tone request
  via SharedState"). Needs a tone-request channel main→ISR-owned HAL.
  Real plumbing work — ticketed, not a gate fix.
- [~] **A4. EEPROM save path stale** — DShot save-settings writes
  `main_state.config`, but the ISR command processor mutates its own
  copy; Configurator-written settings may not persist. Needs
  changed-field publication via SharedState. Ticketed.
- [x] **A5. bench_guard enforced in production M4 builds with bench
  thresholds** (found in the two-agent re-review) — battery profile
  kills latched at 14.0 V OV / 15 A OC; a 4S pack trips OVOLT within
  ~60 ms and the ESC stays dead until reset. RESOLVED: guard gated
  under `debuguart` (bench builds keep it; production protection = the
  AM32 mechanism set: LVC, current-limit PID, stuck rotor).
- [x] **A6. Feature build-matrix breaks** (fixed: compile_error guards; examples/ exist in private tree — copy to clean staging) — `debuguart` fails to
  compile on G071/F051/G431 (L431 PAC syntax, no compile_error guard);
  `zctrace` fails on non-L431 (`comp_at_pre_zc_level` cfg mismatch);
  `Cargo.toml` declares `[[example]] bringup`/`bringup_pac` for files
  that don't exist (`cargo check --examples` / `cargo test` fail).
- [x] **A7. Ungated prints in production ISRs** (ISR prints removed; [loop] heartbeat kept — it is !running-gated; comp_init boot prints kept — main context, pre-IRQ) — `[exti] frame#…` +
  `DETECTED` rprintlns in `handle_exti_frame` (prio-2), `[isr] state
  moved` in `IsrCell::get` (prio-0 first entry), comp_init COMP2_CSR
  boot rprintlns, and the 300-byte `[loop]` heartbeat block ungated in
  production main. The repo's own rule: never print in ISRs.

## Tier B — bench/prod split inconsistencies (decide which side wins)

- [x] **B5. AUTO drive mode promoted to ALL builds** (proven necessary by the first BF DSHOT300 full ladder: always-comp churned the whole envelope at <400 Hz e, 16k desyncs, 99.9% excursions; AUTO holds parity) — the kept divergence (comp
  in interrupt mode, diode in polling) lives in `COMP_PWM_LIVE=3`,
  `cfg(benchuart)`. Production silently runs always-comp (AM32
  verbatim) — the churn-attractor behavior measured and rejected on the
  bench. Decide: promote AUTO into `effective_comp_pwm()` proper for all
  builds, or accept verbatim in prod.
- [ ] **B6. Atomic com writer is bench-only** — `l431_atomic_com_step`
  measured 10-20× fewer deaf windows than sequential, default-on under
  `benchuart`, but the dispatch check is `cfg(benchuart)` → production
  L431 uses the sequential writer. Promote to always-on for L431.
- [ ] **B7. memory.x hardcoded to L431** — 48K RAM / 58K flash now links
  into ALL four MCU builds (F051 has 8K RAM). build.rs has zero
  memory.x handling. Fix: per-chip (or per-board-yaml) flash/RAM values,
  build.rs emits memory.x into OUT_DIR + adds link search path.
- [ ] **B8. Bench probe serial in `.cargo/config.toml`** — the
  `thumbv7em` runner hardcodes the personal ST-LINK serial. Move to an
  untracked local config or document as bench-specific.

## Tier C — solved-case instruments (backout round 2 candidates)

- [x] **C9. Canary/shadow machinery** (REMOVED — round-2 backout; bench re-validation pending wiring repair) — `COMP_PWM_SHADOW`,
  `CANARY_HITS`, 8 `canary()` sites, `DIODE_PWM_CALLS`,
  `DIODE_SEEN_VAL`, `[canary]` print (`phase.rs`, `bin/main.rs`,
  `isr_handlers.rs`). The corruption it hunted was the match-arm
  stack-frame overlap — case closed. Remove unless wanted as a standing
  corruption tripwire.
- [x] **C10. Bisect toggles** (REMOVED — kept divergences now unconditional; SWIER racefix always on) — `RACEFIX_OFF` + 'R', divergence-mask
  'K'/'M'/'P'/'Q' (`shared_state.rs` divergence_mask consumers in
  `main_state.rs`/`isr_logic.rs`). Kept divergences are settled; axes
  are historical. Cheap to keep for future re-bisects — operator call.
- [ ] **C11. DSHOT-debug-era dumps** — `dbg_frame_history` `[snap]` dump
  + the six `dbg_*_last_cyc` fields. Keep through the DShot/bidir
  phase, then prune to the core perf set (isr_tick, t6/t14/comp).
- [x] **C12. Storm-hunt leftovers** (REMOVED — N-pin traps, midw, 'G' lever; comp_gate stale is unconditional) — N-pin traps, `midw`, `GateToggle
  'G'` (comp_gate FRESH/STALE lever; verbatim STALE won). Keep the
  `comp_gate` module itself — that's control, not instrumentation. All
  zctrace-gated, cheap either way.

## Cross-MCU asymmetries (follow-up tickets, cite in the parity PRs)

- G431 boots with ALL NVIC priorities at level 0 (stub
  `adjust_irq_priorities`) — the exact configuration whose fix was the
  L431 chop breakthrough; G071/F051 likewise.
- DMA CGIF-clear hygiene fixed only on L431 CH5; F051/G071/G431 still
  clear only inside the TC branch (TE-only event storms).
- COMP ISR gate/camp/racefix architecture is L431-only; other families
  run bare pre-ack-and-run. `comp_gate::latch` is a dead store there.
- memory.x is L431-hardcoded into all four builds (B7).

See docs/public_pr_plan.md for the full 18-PR landing sequence and the
merged trim list.

## Confirmed keepers (operator directive: useful facilities stay)

reset-cause capture · flight recorder ('B') · exc=/wex=/cm= parity
counters · desync branch counters f/dc/ds/do · drops= · bench guard
(always-on) · bench UART band + two-frame confirmation + deadman ·
WAXWING/GECKO ('J'/'g'/'x') · HistDump 'H' + LEAN counters · blackbox ·
zctrace stream + edge probe · DWT watchpoint disarm-at-boot · IWDG
always-on · desync architecture + orbit trip in main_state · verbatim
duty_ceiling + tests · 1 kHz dispatch · wide frametime bounds ·
DebugMonitor watchpoint catcher ('V', zctrace-gated)
