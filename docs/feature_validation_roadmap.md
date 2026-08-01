# Feature-validation roadmap — "full AM32-equivalent" declaration

Goal: close every feature gap between "the production input stack +
drive envelope are validated" (done, see bf_phase.md) and "for THIS
Vimdrones L431 board, rm32 is a fully validated working AM32
equivalent, with extra goodies." Each feature lists method + gate.

Test-suite baseline (2026-08-01): 282/282 host, 78/78 blackbox (all
historical xfails gone), 4 MCU cross-builds green on every commit.

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

## Tier 2 — needs operator ears/hands nearby, no rewiring

- [ ] **Beacon audibility** (A3 close-out) — cmds 1-5 + arming tune;
  operator confirms pitch ladder by ear.
- [ ] **Sine-mode smooth start** — enable use_sine_start, low-throttle
  engages; operator judges smoothness (host xfails resolved, but the
  stepper is firmware-main-loop, never bench-run).
- [ ] **Stuck-rotor protection live** — enable, hold rotor (finger on
  bell at LOW duty only), verify protective shutdown + recovery.
- [ ] **Stall / low-RPM handling** — operator loads rotor toward
  stall at low duty; compare against clone behavior.
- [ ] **3D mode (bi_direction)** — BF 3D setup, forward/reverse
  through neutral; changeover halving is host-tested, never
  bench-run.
- [ ] **Cold-boot + battery-replug soak axes** (item-3 residue) —
  every protocol x physical power cycle; zero missed detections.
- [ ] **Browser Configurator session** (optional) — stock bootloader
  swap (procedure proven, bins archived), read/write/persist through
  the real UI, restore bench bootloader.

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
- [ ] **KISS telemetry on PB6** — the production telemetry output,
  currently hijacked by debuguart. Needs PB6 returned to telemetry
  duty for the test (debug via RTT + soft-UART during it). The last
  "AM32 equivalent" feature with zero validation.

## Declaration text (target)

"For the Vimdrones L431 board: rm32 is a fully validated AM32-clone
equivalent — full drive envelope at parity, entire production input
stack, EDT, Configurator transport — with extra goodies (flight
recorder, parity counters, bench guard, AUTO drive, atomic com
writer). Exceptions, if any, listed explicitly."
