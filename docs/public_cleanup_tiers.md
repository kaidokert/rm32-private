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

## Tier B — bench/prod split inconsistencies (decide which side wins)

- [ ] **B5. AUTO drive mode is bench-only** — the kept divergence (comp
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

- [ ] **C9. Canary/shadow machinery** — `COMP_PWM_SHADOW`,
  `CANARY_HITS`, 8 `canary()` sites, `DIODE_PWM_CALLS`,
  `DIODE_SEEN_VAL`, `[canary]` print (`phase.rs`, `bin/main.rs`,
  `isr_handlers.rs`). The corruption it hunted was the match-arm
  stack-frame overlap — case closed. Remove unless wanted as a standing
  corruption tripwire.
- [ ] **C10. Bisect toggles** — `RACEFIX_OFF` + 'R', divergence-mask
  'K'/'M'/'P'/'Q' (`shared_state.rs` divergence_mask consumers in
  `main_state.rs`/`isr_logic.rs`). Kept divergences are settled; axes
  are historical. Cheap to keep for future re-bisects — operator call.
- [ ] **C11. DSHOT-debug-era dumps** — `dbg_frame_history` `[snap]` dump
  + the six `dbg_*_last_cyc` fields. Keep through the DShot/bidir
  phase, then prune to the core perf set (isr_tick, t6/t14/comp).
- [ ] **C12. Storm-hunt leftovers** — N-pin traps, `midw`, `GateToggle
  'G'` (comp_gate FRESH/STALE lever; verbatim STALE won). Keep the
  `comp_gate` module itself — that's control, not instrumentation. All
  zctrace-gated, cheap either way.

## Confirmed keepers (operator directive: useful facilities stay)

reset-cause capture · flight recorder ('B') · exc=/wex=/cm= parity
counters · desync branch counters f/dc/ds/do · drops= · bench guard
(always-on) · bench UART band + two-frame confirmation + deadman ·
WAXWING/GECKO ('J'/'g'/'x') · HistDump 'H' + LEAN counters · blackbox ·
zctrace stream + edge probe · DWT watchpoint disarm-at-boot · IWDG
always-on · desync architecture + orbit trip in main_state · verbatim
duty_ceiling + tests · 1 kHz dispatch · wide frametime bounds ·
DebugMonitor watchpoint catcher ('V', zctrace-gated)
