# minz-core extraction roadmap (2026-07-11) — **COMPLETE**

All items landed (F1/F2, E1-E5, B-flags fixed or dispositioned;
the one consciously-skipped cosmetic is noted in E5). End state:
114 host tests, ~99 % line coverage, motor_tester2.rs ≈ 2,700 lines
of ISR glue + statics + init; every decision and every wire byte is
host-tested minz-core code. Kept for the incident notes and seam
patterns.

Product of a two-agent review of `examples/motor_tester2.rs` (~2,750
lines after passes 1–3) against the established minz-core seams:
`WindowState<'a>`-style atomic-ref structs for read-modify-write
state webs, plain args/returns for pure arithmetic, data+sink
closures for serializers, timestamps always as arguments, side
effects returned as data (`CloseOutcome` pattern), critical sections
always owned by the firmware caller.

Line numbers are as of commit `cd54ff6`-era layout — re-grep before
acting; the anchors drift with every edit.

## Status of completed passes (for context)

1. **Pass 1** — timing / estimator / guards / throttle / ticks /
   wire / blackbox (commit `0bd00b8`).
2. **Pass 2** — dump serializers, drive geometry, ui clamps, a85,
   `UartTxWriter` → `minz::uart_tx`, WindowRec dedup (`cd54ff6`).
3. **Pass 3** — `close_float_window` behind the `WindowState<'a>`
   seam + wire offset-sanity hardening (FALCON_HARDENING §17).
   80 host tests, 98.8 % line coverage.

## Fix-first (independent of extraction)

- [x] **F1. Make `w` kill unconditional.** DONE 2026-07-11 (easy-wins
  pass): kill actions run regardless of the `output_enabled` mirror;
  only the "off" print stays conditional.
- [x] **F2. Drain ISR trip flags BEFORE key dispatch.** DONE
  2026-07-11: the desync/sag/OC report blocks moved ahead of the RX
  drain, so an arm in the same pass sees an honest mirror.

## Extraction queue (ranked)

### E1. `zc.rs` — ZC candidate/confirm/accept state machine — DONE 2026-07-11

Landed: `on_held_edge` (COMP, no CS by design — the priority
structure is the lock), `confirm_step` + `accept_gen_current`
(TIM1_UP; the firmware keeps its `free` envelope), `accept_publish`
(shared: publish + estimator + engage decision; the scheduling tail
stays at the call site so the elapsed read keeps its position).
**B1 TOCTOU closed** via the generation snapshot taken at
candidate-load time and validated inside the CS (a close + fresh
re-arm refreshes CAND_GEN — the snapshot doesn't care); consume and
discard are compare-exchanges so a replaced candidate always
survives. Candidate lifetime kept EXACTLY as proven (consumed at
depth, before the CS). 20 zc tests incl. the B1 interleaving
regression; module 97.8 % lines. Bench: SWIFT ladder 463/763/1004 Hz
qzc 100 % (`captures/e1_retry_map.png`) + 5/5 ADC-confirm-path
engages to the amp-10 equilibrium.

**Verification war story (read before trusting single engages):**
the first E1 ladders failed 8/8 engages and a manual test showed a
poisoned-231 µs STV kill — hours of bisection later, the same full
tree passed everything. The "regression" was manufactured by a bad
repro: single engage attempts with no kill/re-arm discipline judged
against the DOCUMENTED engagement lottery, plus one capture polluted
by a leftover CL gate (a failed episode leaves gate_us(reacq) ~20 µs
latched; `r` at the same hz doesn't re-store it — the delta-publish
only fires on hz CHANGE). Same-binary contradiction proved it: the
E5-only build was "sick" single-shot and 5/5 healthy minutes later.
Bench verdicts about engage quality need n≥4 with w/re-arm between
attempts — exactly why cl_lock_map's engage() retries. Also
confirmed pre-existing (NOT a regression): sectors 1 AND 2 are
ADC-confirm-blind at amp 10/f=100 open loop (66 % overall qzc);
under SWIFT this is invisible. Candidate lever for the sector-1
blindness: the sec-1 rule `2B < A` has no vbus margin — worth a
WAXWING look someday.

Original plan for reference:
**Highest value, highest risk.** COMP tail (~2844–2872: SWIFT
immediate accept vs candidate arm, publish order EXPECTED/CONFIRMS/
GEN before ZC), TIM1_UP wrap-confirm (~2600–2656), and
`accept_qualified_zc` (~2880–2937).

Seam: `ZcState<'a>` atomic refs; three entry points mirroring the
three interrupt contexts, zero internal critical sections (the
priority structure — COMP prio 1 preempts TIM1_UP prio 3; LPTIM2
shares prio 1 with COMP — IS the locking):
- `on_qualified_edge(zs, expected, now_us) -> EdgeAction`
  (AcceptNow / Armed / Ignored) — COMP context, no CS by design.
- `confirm_step(zs, observed) -> ConfirmAction` +
  `try_accept(zs, zc_us) -> Option<()>` — TIM1_UP; the firmware's
  `free` stays at the call site, `try_accept` re-validates gen
  INSIDE it.
- `on_accept(zs, zc_us, elapsed_us, sector) -> AcceptOutcome
  { engaged, schedule_us, mask_comp, bb }` — shared.

Firmware keeps: the persistence read-loop (polls `comp2::value()`),
the `free` envelope, and performs schedule/mask/bb from the outcome.

**Must close while extracting — B1 (TOCTOU, gen guard defeatable):**
TIM1_UP loads the candidate; a window close (LPTIM2/TIM7) plus a
fresh COMP arm both land before the accept; the fresh arm refreshes
`CAND_GEN` to the NEW generation, so the gen check passes and a
previous window's timestamp is accepted → junk estimator delta,
inflated `elapsed` in the scheduler. Low probability; worst in
re-acq's 8 % gate at speed. Fix inside `try_accept`: re-load
`CAND_ZC_US` and compare to the local candidate inside the CS (or
publish (gen, zc) as one atomic word). Related B2: capture the whole
candidate tuple consistently — `CAND_EXPECTED`/`CAND_CONFIRMS` can
also swap under a stale local.

Host tests enabled: self-referential lock (wrap-dwell premature
accept), engage-runaway from unconditional 1-confirm, gen-guard
stale-accept, mask-after-accept (one accept per window), C-window
(sec 0/3) neither schedules nor engages, SWIFT vs ADC-confirm regime
split, re-acq strict 2-confirm, and a scripted-interleaving test for
B1 (close→arm→resume-confirm in adversarial order — untestable on
hardware).

Bench check: known-good 48 kHz ladder as A/B control
(`cl_lock_map.py --fast`, amp 15–20 → 405–555 Hz, qzc 100 %), plus
`falcon_probe.py`/`falcon_stats.py` premature-rate replay, bb dumps
on any desync.

### E2. `mode.rs` — arm/kill/CL bench-mode state machine — DONE 2026-07-11

Landed as `core/src/mode.rs` (100 % line coverage): `ModeState<'a>`
atomic refs + `Mirror` locals + `step(Cmd) -> Actions` exactly as
sketched below. All five keys (`r`/`q`/`w`/`y`/`m`) now shuttle
through `mode_cmd()` in the firmware. Regression tests:
snap-on-arm, estimator-reset-on-CL-arm, unconditional kill (F1 as a
test now), zombie-flag matrix, EXTI-only-on-dead→driving, the B4
stale-arm gate (`m` blocked while CL_ARMED — behavior CHANGE, gated
message "CL armed/active - 'y' first"), and a mirror-convergence
property test. Bench: ladder 452/757/1003 Hz qzc 100 % + the full
transition matrix exercised live incl. the B4 gate
(`captures/modepass_sanity_map.png`). NOT changed: `Arm` while
CL_ARMED still allowed and does NOT clear the pending arm
(cl_lock_map's engage-retry flow may rely on it — revisit
deliberately, with the script, if ever).

Original plan for reference:
Logic scattered across `r`/`q`/`w`/`y`/`m` keys (~1290–1573), boot
publication (~1035–1065, "same as `w`"), and the three ISR-kill
report blocks. Seam: `ModeState<'a>` atomic refs + command-in/
actions-out:
`step(ms, mirror, Cmd{Boot|Arm{hz}|Kill|ClToggle|ModeToggle|IsrKill(kind)})
 -> Actions { motor_enabled, exti, snap_amp, baseline_vbat,
 reset_estimator, msg }`.

Pins as tests: estimator-reset-on-arm (poisoned-144 deadlock),
snap-on-arm (4/4 failed engages), EXTI-mask-when-not-driving (650 k
events/s storm), no zombie CL flags after ANY kill (OC zombie-status
incident), gating matrix (`Arm`/`ModeToggle` rejected under CL;
`ClToggle` needs armed six-step), and a property test that
`output_enabled == MOTOR_ENABLED` after any command sequence.

Structurally fixes: F2's class, B3 (OC/sag report blocks trust the
ISR to have cleared `CL_ARMED` — unverified), B4 (`m`/`y` gate on
`CL_ACTIVE` but not `CL_ARMED` → stale arm engages minutes later),
B5 (freq keys under CL stomp `SECTOR_GATE_US` with an open-loop
value — gate the publish on `!CL_ACTIVE`).

Bench check: the live-fire drill set (w during CL, lowered-threshold
OC trip, desync via amp drop, sag via bench dial) + full ladder.

### E3. Pure quick wins — DONE 2026-07-11 (easy-wins pass)

All landed as core modules `zc.rs` (adc_sign_observed + the
sector-2 boundary regression, vbus_decay_step + timescale test),
`sense.rs` (one authoritative calibration — fixed B6: the sag report
now goes through adc_to_mv + vbat_mv instead of ×7507/1000),
`rates.rs` (RateWindow + rate_per_s, wrap + zero-dt tests), drive.rs
additions (commutation_class REF/BLD/DRK table, next_sector,
freerun_reschedule_us with the 1.0×T-not-1.5×T named regression),
dump.rs `edge_dump_status`. Also fixed: B5 (freq keys no longer
stomp SECTOR_GATE_US under CL), B7 (IWDG margin comment corrected to
~3× / `u` blast). 93 core tests, 99.0 % line coverage; bench ladder
15/25/35 → 452/756/1001 Hz qzc 100 % + live `i`/`e`/`w` key checks
(`captures/easywins_sanity_map.png`). Original plan for reference:
- **ADC-sign confirm rule** (~2604–2609) →
  `adc_sign_observed(sector, pa_a, pa_b, vbus_est, comp_value)`.
  Table-driven test pins the sector-2 48 kHz mystery (2×float-A ≈
  vbus at the ZC) and the per-sector polarity conventions.
- **LPTIM2 commutation decision** (~2413–2469) →
  `on_commutation(prev_sector, shot_refined, interval_us, reacq) ->
  CommOutcome { next_sector, bb_ev (REF|BLD|DRK), gate_us,
  reschedule_us }`. Named regression: free-run at 1.0×T, NOT 1.5×T
  (the compounded-lag incident).
- **vbus decaying-max** (~2566–2572) →
  `vbus_decay_step(est, pa_a, pa_b) -> u16` (τ ≈ 21 ms; feeds the
  ADC-sign rule's sectors 4/5 — test the two together end-to-end).
- **Sense conversions** → core `sense.rs`; **fixes B6**: the sag
  report uses hardcoded `raw × 7507 / 1000` while the `i` key goes
  through `adc_to_mv` × divider — two calibrations for one quantity
  that silently diverge if ADC cal changes. Anchor tests: raw 1080 ≈
  8.11 V, sag floor 793.
- **Rate bookkeeping** → `rates.rs`: `RateWindow::latch` + `
  rate_per_s(dcount, dticks) -> Option<u32>` (wrap + dtick==0 guard);
  three inline copies today (1 s boundary, `o`/`p` resets, i-key
  irq/s block).
- **Edge-dump validity/header policy** → extend `dump.rs`:
  `edge_dump_status(sec_starts, window_end, hz) -> Invalid | MotorOff
  | Ok{window_us}`.

### E4. Dedup + hardening — DONE 2026-07-11 (same pass as E2)

`guards::apply_isr_kill` + `KillFlags<'a>` now run all three ISR
kill sites (TIM7 watchdog — which HAD been missing the CL_ARMED
clear — OC, sag); the zombie-flag matrix is host-pinned for every
kill kind. `guards::sag_step` adopts SagGuard's tested semantics at
the firmware's atomic-backed call site (the inline duplicate is
gone). `guards::trip_accum_step` owns the OC window.
`blackbox::EV_*` is the single authority for event codes (window.rs
and drive.rs re-export; all firmware bb_record sites use names, no
magic numbers). Original plan for reference:
- **SagGuard adoption**: the tested `guards::SagGuard` is dead code —
  TIM1_UP re-implements the debounce inline (~2530–2561). Wire the
  tested one in (or a pure `sag_step(raw, baseline, run)`).
- **`apply_kill` flag matrix**: three inline kill sequences (TIM7
  watchdog ~2156, TIM1_UP OC ~2684, sag ~2544) each clear a slightly
  different flag set — the zombie incident was one missing line.
  `KillState<'a>` + kill-kind → flag-matrix as data, host-pinned.
- **bb event codes as one core enum** (magic numbers 0–9 at call
  sites; core window.rs already owns EV_NOZ/EV_RAQ) — shared
  authority with `owl_report.py`'s decode table.
- **OC trip accumulator** (~2663–2701) → `TripAccum::step` around
  the already-extracted `guards::overcurrent`.

### E5. Hardware moves — DONE 2026-07-11 (same pass as E1)

`minz::usart2_rx::init_pa2_rx` (SWAP + UE=0 ordering encoded once),
`minz::iwdg::{start_1s, refresh}` (the burn post-mortem + refresh
cadence contract moved with it), `minz::uart_tx::
usart1_tx_push_pull_pb6`. The example-local `bringup()` restructure
is consciously SKIPPED: pure cosmetics, ~300 lines of borrow-heavy
init shuffling for zero testability gain — revisit only if a
motor_tester3 actually materializes. Original plan for reference:
- USART2-on-PA2 raw init (~1002–1020) → `minz/src/usart2_rx.rs`
  (CR2.SWAP + BRR under UE=0, encoded once).
- IWDG start/refresh (~985–991, refresh ~1228) → `minz/src/iwdg.rs`
  — a safety device deserves a named home; move the burnt-motor
  rationale with it. Also fix B7: the "10× margin" comment is wrong —
  the `u` blast blocks refresh ~0.35 s (real margin ~3×).
- PB6 push-pull flip (~927–936) → helper next to `uart_tx` (the
  trick that makes 2 Mbaud work, currently an anonymous unsafe
  block).
- Remaining init → example-local `fn bringup() -> Bench{..}` for
  readability; optionally fold the two extra `set_irq_prio` calls
  into `priority::set_bench_prios()` (their comments encode real
  lockup lessons — keep them).

### Skip (reviewed, not worth it)
- `SysTick` / `USART2` / `TIM1_CC` ISR bodies — pure hardware glue,
  no decisions.
- MAGPIE drain space policy (2 lines around core constants).

## Known non-bugs / accepted-by-design (so nobody re-flags them)
- bb_record slot race (documented at the statics).
- COMP-only EDGE_BUF writer assumption.
- TIM7 reading `ticks_10us()` twice for the two watchdog deltas —
  each pairs correctly with its ref-before-now load.
- `VALID_COMP_COUNT` vs `WINDOW_VALID` divergence is intentional
  (blanked-but-gated edges count in the former only); fix the doc
  comment at the `WINDOW_VALID` static when nearby.

## Standing verification protocol for every pass
Build + clippy clean → full core suite + `cargo llvm-cov` → flash →
sanity ladder chosen to exercise the extracted path (e.g. amp 35 for
decimation) → lock map + dropout tail rendered and eyeballed →
commit with the coverage numbers in the message. Any statistical
anomaly in the map gets A/B'd against the pre-extraction bins before
being accepted or chased (the 205 % qZC σ turned out to be ONE
host-parser mislock frame — new offset≤len wire rule now guards it).
