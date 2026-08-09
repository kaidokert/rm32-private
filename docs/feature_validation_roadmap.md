# Feature-validation roadmap — "full AM32-equivalent" declaration

Goal: close every feature gap between "the production input stack +
drive envelope are validated" (done, see bf_phase.md) and "for THIS
Vimdrones L431 board, rm32 is a fully validated working AM32
equivalent, with extra goodies." Each feature lists method + gate.

Test-suite baseline (2026-08-01): 282/282 host, 78/78 blackbox (all
historical xfails gone), 4 MCU cross-builds green on every commit.

## Merge + recurring-lockup watch item (2026-08-06)

- origin/main (6 public PRs, #41-48) merged into am32_sheet at
  f3175f9; tag pre-main-merge-20260806 pushed to private. 32
  conflicts resolved: yamls kept ours (comments) except
  protondrive_g431 (public revised the G431 INMSEL map wholesale —
  took public for yaml+build.rs pair); build.rs = ours (B7 memory.x)
  + public's enabled_mcu()/common-pin validation; signal.rs took
  public (bounds-safe loop + count==0 early return); mcu.rs/lib.rs/
  main.rs/isr_handlers kept ours (supersets); deduped temp_advance()
  (kept public constants version) + 3 signal timer-wrap tests (kept
  public 32-slot versions). 290 host + 80 blackbox green; bench
  regression dsy=0/200,970 comms at the 50% staircase.
- [!] RECURRING: power-cycle -> boot LOCKUP needing MASS ERASE +
  re-image — SECOND instance (first attributed to power-loss-mid-
  save ECC; this one had NO known interrupted write). The known-good
  tag build also locked = environmental both times; mass erase fixed
  both. Watch: if a third instance occurs, instrument properly
  (dump ECC fault address via DBGMCU before erasing). Also this
  session: the ST-LINK dropped off USB mid-flash once (re-enumerated
  fine), and the no-signal bootloop (stock A2 behavior) resurfaced
  as a red herring during re-wiring.

## Post-merge-3 full re-qual (2026-08-08) — bench-guard floor incident

- SYMPTOM: bf_slam CHECK (cm~17k vs 305k reference) on BOTH a sagged
  and a fresh 12.23 V pack; bf_ladder printed PASS but logged only
  725k comms (half reference). Battery swap did not help — code-cause
  hunt per BENCH-NEVER-DRIFTS.
- ROOT CAUSE (not a merge regression): the bench guard's
  battery-profile vbat floor (8,470 mV / 10 ms, bench_guard.rs) was
  tuned from battB_ sessions that never exceeded ~70% throttle. At
  100% these 3S packs sit at 8.4-8.5 V STEADY and dip 7.24-7.39 V at
  slam inrush — the floor sat inside the normal operating band. Both
  re-qual halves were VBAT-killed mid-test (`!! BENCH KILL` in the
  esclog each time); the guard latches Disarm until reset, so the
  rest of each test ran against a dead ESC. The 07-31 slam reference
  (305,832 comms) ran on the Extech PSU profile (5,500 mV floor) —
  today was the FIRST battery-profile slam ever. All three merges
  touched neither guard nor measurement chain (verified by diff);
  identical code trips identically. Both packs behaved within 150 mV
  of each other — the swap was unnecessary.
- FIXES: B_VBAT_FLOOR_MV 8,470 -> 6,800 with battery-specific 150 ms
  debounce (a dying pack goes deep AND STAYS; accel transients ride
  through). bf_ladder/bf_slam/bf_soak EscLog now latch any
  `BENCH KILL` line and the ladder/slam verdicts fail on it — the
  earlier ladder "PASS" with a kill in its own tee was a verdict
  blind spot (instrument-decisions-not-outcomes).
- Also fixed: bf_slam post-read raced BF's ~5 s CLI motor-stream
  timeout (BF stops DSHOT -> ESC signal-timeout reset wiped counters
  mid-window; [sr] prints only every ~5 s). Post-read now keepalives
  `motor 0 1000` every 2 s.
- RE-QUAL RESULTS on merged tree (fresh pack, retuned guard):
  ladder **PASS** — 1,550,969 comms, dsy=0, exc 0.02/1k, 0 resets,
  min vbat 8,238 mV (old floor would have killed it again), max
  5.0 A. Slam **PASS** — 260,254 comms (reference 305,832), dsy=0,
  0 resets, no kill; keepalive post-read verified. RE-QUAL COMPLETE
  on the merged tree.
- BENCH SCAR: the mid-sweep power losses were the BENCH FUSE blowing
  (twice; ESC reset-cause read "brownout"). NOT the recurring lockup
  (watch item stays at 2 instances). Second blow happened at IDLE
  (motor never engaged) -> suspect connect/power-on cap inrush on a
  marginal fast-blow fuse, not slam load. Suggested: time-delay fuse
  of same rating; check holder/leads for heat discoloration.

## Merge 4 (2026-08-08 evening) + OPEN slam-storm anomaly

- origin/main #55 (beacon tone vector) + #56 (reviewed EDT throttle
  gate) merged at 0a12e40; tag pre-main-merge-20260808b on private.
  Resolutions: took public's AM32-verbatim gate
  (`!edt_arm_enable || edt_armed || value == 0`) over our blunt gate
  removal; kept our on-stack isr_state boot block (theirs' closure
  version is a strict subset); harness = union (their play_tone_flag
  + EDT state-line fields alongside our HAL counters); vectors =
  theirs. 305 host + 80 blackbox green (3 initial failures = stale
  RELEASE harness binary — harness.py runs target/release; rebuild
  BOTH profiles after a merge).
- Bench: ladder PASS x2 on the merged build (1.50M then 1.45M comms,
  dsy=0, 0.01 exc/1k, 0 resets) — the merged throttle gate passes
  vanilla DSHOT at every rung.
- [!] OPEN: bf_slam DESYNC STORM — dsy~2500, exc~19-20k, cm~22k
  (vs 260k PASS ~2.5 h earlier on the same rig). REPRODUCED 3x:
  merged build x2 AND the pre-merge tag build x1 (A/B both
  directions) -> NOT code. Texture: ma=0 throughout, rail NEVER sags
  (12.09 V flat), ladder before AND after storms passes clean ->
  motor tracks gentle profiles but loses lock on 10%->100% steps,
  drawing no current. Signature fits a MECHANICAL LOAD CHANGE
  (prop/nut loosening -> near-unloaded rotor accelerates too fast on
  hard steps for BEMF tracking; gentle ladder unaffected). NEEDS
  HANDS: check prop nut / bell grub screw / coupling, then rerun
  bf_slam. If hardware checks out clean, next suspect is FC-side
  output state (dump BF diff all and compare against the slam7-era
  config).

## Tier 1 — local now, no rewiring, no hands

Config channel: `softuart_cmd.py --set FIELD=VAL --save / --get FIELD`
(field->offset table in the script, anchored on poles=14@[27]).
NOTE advance_level=26 is NOT anomalous — new-format (1.90+) encoding:
(26-10)*0.9375 = 15 deg, the factory default.

- [x] **Direction change + save-persist round trip** (2026-08-01):
  dir_reversed=1 -> save -> persisted -> power-cycle -> survives
  reload -> REVERSED SPIN LOCKED CLEAN (dsy=0 / 157,322 comms,
  eRPM 100,700 @40% == forward curve) -> restored + re-verified.
  (Caveat: rotation sense verified by coherent reversed-sequence
  lock, not by eye — half-applied reversal would churn.)
- [ ] **Long-soak retention** — 15-30 min continuous at 70-100% on
  battery under BF DSHOT300, staircase engage; [sr] deltas after
  (dsy=0 standard), thermal drift eyeballed via EDT temp.
- [x] **LVC live trip** (2026-08-01): mode 1 per-cell with
  low_cell_volt_cutoff=160 -> threshold 3 x 4.10 V = 12.30 V > pack.
  Idle trip: Armed -> Disarmed after the 10 s sustained-low window,
  BF still driving. Mid-spin trip: cut from 57k eRPM to Disarmed
  mid-hold. Restored. FINDING: mode 2 (absolute) compares the raw
  byte against centivolts in AM32 TOO (main.c:2405) — effectively
  dead (<=2.55 V) in both; rm32 matches reference behavior verbatim.
- [x] **Current-limit PID** (2026-08-01): limit=2 (target 400 mA) at
  the 50% rung: SR ring reads 400/400/373 mA — PID pinned EXACTLY at
  target; eRPM 120k -> 64.4k; sag 11.6 -> 12.0 V. Baseline 1,733 mA.
  Gate `0<limit<100` == AM32. Restored.
- [x] **Timing-advance sweep** (2026-08-01): new-format 10/18/26/34
  (0/7.5/15/22.5 deg) at 40%: dsy=0 at ALL FOUR (134k-153k comms
  each); eRPM 101.7k/101.7k/105.3k/104.5k — advance raises speed
  ~3.5% 0->15 deg, plateaus by 22.5. Factory 26 restored.
- [!] **Brake-on-stop — SAFETY-CRITICAL BUG FOUND, bench casualty**
  (2026-08-01): setting brake_on_stop=1 killed the board within
  seconds (SWD unreachable = VDD lost). ROOT CAUSE (confirmed vs
  AM32 phaseouts.c): rm32 ported the brake duty math
  (brake_compare == AM32's arr - brake*arr/2000) and even the
  `proportional_brake()` bridge reconfiguration (high-sides forced
  OUTPUT-off, low-sides to PWM) — but NEVER CALLED IT. The near-ARR
  brake compare landed on the mixed bridge state the stop left
  (one low-side FET solid-on via GPIO, another leg's high-side in
  AF) -> DC VBAT->winding->GND path, locked-rotor burn, supply
  killed. FIXED: isr_logic.rs prop-brake arm now calls
  proportional_brake() before the duty write (re-asserted per tick,
  AM32-style); regression test
  `prop_brake_reconfigures_bridge_before_duty` (the HAL-call-counter
  class — the exact bug family that infra was built for).
  RECOVERY + BENCH CLOSE-OUT (2026-08-01): the board came back after
  ~30 min; partial reflashes still locked the core at boot (LOCKUP,
  no output) — even the pre-casualty known-good build. Cause: flash
  ECC corruption from the power loss DURING the in-flight save; only
  a MASS ERASE + full re-image (bootloader + EEPROM + app) cleared
  it. SCAR: after any power-loss-during-flash-write event, mass
  erase — partial rewrites leave ECC-invalid words that lock the
  core on read and masquerade as a firmware regression.
  BRAKE VS COAST (fixed build, drag_brake_strength=10, 3 stops
  each): brake engaged at armed-idle draws 0 mA (the old code
  cooked here). First post-stop SR sample: brake ci 9006/9666
  (~310 mech rpm, nearly stopped) vs coast ci 1421/3473 (~800-2000
  rpm still spinning) — clean separation, 3-6x faster deceleration.
  dsy=0 across the whole 569k-comm session. Config restored to
  baseline + dump-verified. VALIDATED.
- [x] **EDT temp — ROOT-CAUSED + FIXED, bench confirm pending**
  (2026-08-01): the 7-9 C readings were the missing VDDA rescale.
  Factory TS_CAL points are measured at 3.0 V (L4/G0/G4; F0 at
  3.3 V) while the board runs VDDA=3.3 V; AM32 calls
  __LL_ADC_CALC_TEMPERATURE(3300, raw, 12B) which rescales
  raw*3300/3000 before interpolating — rm32's calc_temperature_pure
  used raw directly (~13 C low + slope error). Fixed with per-family
  cal_vref_mv (L431/G071/G431 3000, F051 3300 = no-op) in
  TempCalibration + the boilerplate macro + the G431 direct site;
  regression test temp_l431_vdda_rescale_matches_am32 (unscaled -5 C
  vs rescaled 21 C on realistic cal values). CONFIRMED LIVE
  (2026-08-01): degC=36-37 on the recovered board (die self-heating
  over a ~22 C bench — plausible; was 7-9 pre-fix). VALIDATED.

## 3D / bidirectional drive campaign (2026-08-01, operator-assisted)

- [x] **Direction evidence without eyes**: 'i' probe extended with
  fwd flag + e_com_time; internal eRPM (60e6/ecom) vs BF GCR eRPM
  agree within 1% AT EVERY RUNG, both directions. fwd flips with the
  commanded half-range; commutation.advance() reverses step order on
  the flag; operator confirmed physical reversal visually (coast-down)
  and by airflow.
- [x] **eRPM anomaly resolved as artifact**: the one-off 394,700
  reading never reproduced under dual-source capture; reverse tops at
  133.9k vs forward 142.2k eRPM at 75% (~6%, prop pushed backwards —
  aero, not firmware). Commanded-vs-actual asymmetry per nominal rung
  is BF-3D's value mapping (deadband 1406/1514, halves compressed),
  not the ESC: per-adj response is linear and matches across
  directions within 2-5%.
- [x] **Raggedness root-caused, no reverse defect**: (a) through-
  neutral flips cost ~2 desyncs each (start drives into a counter-
  rotating rotor after the coast; speed-gated reversal per AM32
  semantics, recovers immediately; 8 dsy / 4 flips / 94k comms);
  (b) direct mid-throttle engages churn in EITHER direction in exact
  QUANTA OF 4 desyncs per episode (the known engage-pattern law —
  one churn episode = ~4 dsy before relock). Staircase-engaged
  steady-state rungs 15-75%: clean both directions (4 dsy / 356k
  comms total = one engage episode).
- SCARS: (1) in BF-3D, motor value 1000 = FULL REVERSE — every kill
  guard/teardown must use 1500 (a 1000-teardown left BF streaming
  max reverse; only the refuse-to-arm-at-nonzero-throttle gate saved
  the bench). (2) rm32 stays Disarmed after an in-session disarm even
  at sustained zero throttle — needed a reset to re-arm; AM32 re-arms
  — DIVERGENCE, investigate the arm state machine. (3) [i] replies
  garble at high reverse duty (PB6 TX corruption under load; PA0 RX
  side stayed e=0) — read BF-side telemetry at high rungs.
  Tool: scripts/dir3d_ladder.py.

## Tier 2 — needs operator ears/hands nearby, no rewiring

- [x] **Beacon audibility CONFIRMED BY EAR** (2026-08-01): all five
  beacon tunes + arming tune distinct at beep_volume=10; volume wired
  AM32-style (was fixed duty 15); tone counters numerically exact
  (141 note-starts / 11 sequences / 0 aborts).
- [x] **Sine-mode smooth start CONFIRMED** (2026-08-01): 3% crawl
  "smooth with a hum, no artifacts", "very smooth and quick" handoff
  to BLDC at the changeover; dsy=0 on the wire; config restored.
- [x] **Stuck-rotor protection — DEBUGGED (2026-08-01, dual-harness
  campaign, stall_probe.py)**. Three-part verdict:
  (1) The standstill-hold OSCILLATION at 5% throttle is AM32-VERBATIM:
  the same-pass clear (zc>100 && raw<200) reopens the latch in BOTH
  implementations — the C reqcheck harness (real AM32 main.c) showed
  the identical bemf 102->0 clear before rust did. NOT a divergence;
  the reference oscillates too. At >=10% throttle (raw>=200) the
  latch HOLDS SOLID in both (T3: 500+ ticks latched). Documented.
  (2) REAL DIVERGENCE FOUND + FIXED: rm32 lacked AM32's !running
  housekeeping (main.c:1256-1259 — continuous zero_crosses/bad_count
  scrub at zero throttle; old_routine covered by rm32's mode model).
  zero_crosses carried across runs (bench: 1809 at idle), polluting
  the zc>1000 fault-clear AND compute_setpoint's zc-gated startup
  boost — the post-stall "won't spin up" churn. Fixed in isr_logic
  zero-throttle branch; regression vector stop_housekeeping.txt.
  Bench-confirmed: zc=0 at idle after spins; 4/4 identical-speed
  start/stop cycles WITH protection enabled (previously degraded).
  (3) No-rearm-after-LVC-disarm is AM32-CORRECT (the reference
  latches LVC until reboot). The one 3D-era no-rearm at neutral
  remains unreproduced — watch item.
  Note: the reqcheck C harness hangs (infinite loop) one tick after
  a latch with throttle streaming — fake-env wait, not firmware;
  probe rows marked DEAD. PHYSICAL SIGN-OFF (2026-08-02): operator
  gripped the bell at 15% throttle — fought ~2 s, went limp, STAYED
  limp; mid-hold wire probe: newinput=346 (BF commanding) with adj=0
  duty=0 (latch cutting). Feature VALIDATED.
- [x] **3D mode VALIDATED** — see the 3D campaign section above.
- [ ] **Cold-boot + battery-replug soak axes** (item-3 residue) —
  every protocol x physical power cycle; zero missed detections.
- [x] **Browser Configurator session VALIDATED (2026-08-02)** — the
  full production loop through am32.ca: connect -> MSP passthrough ->
  DeviceInitFlash (stock bootloader swapped in over SWD) -> settings
  page READ -> WRITE (beep_volume 10->2 + a reserved byte, proving a
  wholesale page rewrite) -> PERSIST (dump-verified [30]=02) -> live
  behavior followed (operator: app tones quieter; the "not lower
  every time" beeps were the STOCK BOOTLOADER's own hardcoded-volume
  beeps — two beepers, one knob). Bench bootloader + volume
  restored + dump-verified afterward.
  HONEST PARAMETER INVENTORY (Configurator page vs our validation):
  hardware-validated = LVC, current limit, timing advance, direction,
  3D, sine start, drag brake, stuck rotor, beep volume, telemetry,
  comp/variable PWM, servo cal bytes. Host-tested-only/untested =
  temperature limit (needs heat), stall protection (crawler rig),
  RC-car reverse, active brake (brake_on_stop=2), current PID gain
  sweeps, custom servo endpoints, startup power, custom
  pwm_frequency, auto_advance — documented, not claimed.

## Tier 3 — gated on the PB6/spare-pin rewire

- [x] **Host->ESC UART input** (2026-08-01): USB-TTL TX -> header pin
  4 (HSE_IN / PA0), 9600 8N1. Minz-era decoder resurrected (8d2ff8c^)
  + direct-PAC glue (EXTI0 start qualify + LPTIM1 38.4 kHz sampler,
  both prio 3). TWO DECODER FIXES en route: clean-frame back-to-back
  start acceptance (the archived version only ever decoded the first
  byte of a burst — masked by its 1-byte-per-2s era scripts) and
  per-start LPTIM CMP phase resync + early stop-bit decision (free-
  running sampler phase made burst decodes garble). Dispatch through
  the shared bench_input vocabulary, replies DEFERRED 150 ms (one
  adapter, two bauds — host must switch back before the reply).
  Config verbs added: `<n>o` offset latch, `<n>v` write via the A4
  ring, `c` persisted-EEPROM hex dump, `S` save (same flag as DSHOT
  cmd 12). PERSIST ROUND TRIP PROVEN: [32]=129 -> save -> page reads
  81 -> restore 128 -> page reads 80. Tools: softuart_cmd.py /
  softuart_check.py.
  INCIDENT + GUARD: a mis-decoded offset (pre-fix garble) landed a
  stray write at [0] (boot-enable); the next reset bricked the app
  jump (bootloader's jump() checks byte0==1 even in the bench
  hardcoded build) — repaired over SWD from the dump. Bench write
  verb now REFUSES offsets <3.
- [ ] **Standalone EEPROM tool** (operator-proposed) — tiny isolated
  flash binary: dump/patch all config bytes over UART, independent
  of the firmware under test. Reuses rm32 config structs. SWD covers
  read/verify today; the tool adds firmware-independent WRITE + a
  no-probe workflow.
- [x] **KISS telemetry — VALIDATED HOST-SIDE (2026-08-02), no rewire
  needed**: PB6/USART1@115200 is the same wire+direction+baud as the
  debug UART — the swap is a build flag, not solder. EN ROUTE FIX:
  `telemetry_on_interval` was a ported-but-unwired config byte (same
  family as proportional_brake/beep_volume) — AM32's periodic
  telemetry trigger (main.c:1664-1672, (30-1+interval) ms + per-ESC
  slot offset) had no rm32 consumer; wired via the one_khz_counter
  pattern + host test interval_telemetry_fires_every_30ms.
  Production build on the bench: 33.0 frames/s EXACT, every CRC
  valid, zero stray bytes. Idle: 37-38 C / 12.08 V / 0 A / 0 eRPM.
  At 40%: eRPM 100.8-101.5k (matches the GCR/BF-path curve within
  1%), 1.20 A, sag to 11.1 V, mAh integrator accumulating (10->18->
  21, retained after stop). Tool: scripts/kiss_read.py.
  FULL BF LOOP VALIDATED (2026-08-02): ESC PB6 spliced to FC PC7
  (UART6 RX) + the adapter tap in parallel; BF `serial 5 1024` +
  feature ESC_SENSOR. The ESC answers BF's DSHOT telemetry-request
  bit (observed 81.7 frames/s = 33 periodic + ~50 request-driven —
  the request path's first exercise ever), and BF's ESC-sensor task
  (100 Hz, 0 lates) decodes them: idle temp=36 C via MSP 134;
  IN-FLIGHT at 30% (MSP RC injection): temp=34 C, rpm=82,800 —
  within 2% of the known curve. Wiring scars: the FC's PA10/UART1 RX
  is entangled with USB OTG on the F411 Discovery (dead for UART —
  use PC7/UART6); serialpassthrough refuses ports that own a
  function ("Invalid port1"); a CLI --stay session silently kills
  MSP (the "$M<" echo tell). ESC restored to request-only
  (interval=0, dump-verified) + debuguart build; FC keeps
  ESC_SENSOR + UART6 as a standing bench capability.

## Declaration text (target)

"For the Vimdrones L431 board: rm32 is a fully validated AM32-clone
equivalent — full drive envelope at parity, entire production input
stack, EDT, Configurator transport — with extra goodies (flight
recorder, parity counters, bench guard, AUTO drive, atomic com
writer). Exceptions, if any, listed explicitly."
