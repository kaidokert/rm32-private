# Feature-validation roadmap — "full AM32-equivalent" declaration

Goal: close every feature gap between "the production input stack +
drive envelope are validated" (done, see bf_phase.md) and "for THIS
Vimdrones L431 board, rm32 is a fully validated working AM32
equivalent, with extra goodies." Each feature lists method + gate.

Test-suite baseline (2026-08-01): 282/282 host, 78/78 blackbox (all
historical xfails gone), 4 MCU cross-builds green on every commit.

## Tier 1 — local now, no rewiring, no hands

- [ ] **Direction change + save-persist round trip** (last item-6
  residue; the only A4 gap without hardware proof). `dshotprog 0 7/8`
  (direction) + `dshotprog 0 12` x6 (save), probe-rs reset, verify:
  (a) EEPROM bytes over SWD (`probe-rs read b32 0x0800F800`,
  dir_reversed offset), (b) reversed spin direction after reboot,
  (c) restore + re-verify. EEPROM SWD read CONFIRMED working (live
  dump shows boot-enable 0x01 + servo-cal 128/128/128/50).
- [ ] **Long-soak retention** — 15-30 min continuous at 70-100% on
  battery under BF DSHOT300, staircase engage; [sr] deltas after
  (dsy=0 standard), thermal drift eyeballed via EDT temp.
- [ ] **LVC live trip via config trick** — set low-voltage cutoff
  ABOVE the pack voltage (per-cell threshold), spin, verify duty
  ramps down / cuts per AM32 semantics; restore. No PSU fiddling.
- [ ] **Current-limit PID** — set a low current limit, load at 50%+,
  verify duty clamps (EDT current + [sr] ma vs unlimited baseline);
  restore.
- [ ] **Timing-advance sweep** — spin the same rung at advance
  settings 0/1/2/3, verify clean lock at each (dsy=0) + note ci
  shift. (Regime-scope: mid-throttle.)
- [ ] **Brake-on-stop** — enable, spin, stop; verify active braking
  (rapid ci collapse / audible) vs coast baseline; restore.
- [ ] **EDT temp sanity** — degC reads 7-9 on a ~20 C bench;
  check use_ntc/offset math against AM32 for this board before
  calling the temperature channel validated.

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

- [ ] **Host->ESC UART input** — USB-TTL TX to the spare pad
  (operator to identify; previously soft-UART in minz/rinz era).
  Gives interactive bench control while BF keeps the signal wire.
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
