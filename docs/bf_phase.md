# Betaflight/DShot phase — state ledger (as of 2026-08-01)

The production-input phase that followed the drive-envelope campaign.
FC: STM32F411 Discovery, Betaflight 2026.6.0-alpha (MSP API 1.48), USB
CLI on **COM42**. ESC log: PB6 debuguart @115200 on **COM41** (TX-only —
the ESC has no host-RX in BF-phase builds; PA2 belongs to the FC).
Build: `stm32l431,debuguart`. Throttle path: BF CLI `motor 0 <value>`
override (`scripts/bf_motor.py`), settings via `scripts/bf_cli.py`.

## VALIDATED (all at rm32 `d0a900d`)

| item | result |
|---|---|
| PWM | detect + self-arm + 25% spin + FC-reboot recovery |
| DSHOT150 | first-ever validation of the PSC-3 bucket; crc_fail=0 |
| DSHOT300 | crc_fail=0 sustained (500 f/s); 25% spin |
| DSHOT600 | crc_fail=0; 25% spin |
| Bidir DSHOT300 (idle) | self-validating commit; BF decoded 4,869 GCR responses, 0 invalid |
| Bidir under load | ladder 15/25/35/50% → 46.6k/70.4k/93.8k/117.9k eRPM, 0.00% invalid every rung, monotonic, cross-checks bench-era speeds |
| EDT | flags RTVCS; VCC 12.25 V exact, temp matches ESC reading, live sag under spin, 0.00% invalid |

## The five first-contact defects (all fixed same-phase)

1. **Servo-cal zeroed** (`06a808e`): ZEROED config → 750–1750 µs window;
   BF's 1000 µs disarm pulse read as ~25% throttle → could never arm.
   AM32 Configurator defaults are byte 128 → 1006/2006/1502 µs.
2. **NDTR-33 sliding capture window** (`0d7b7d1`): 33 edges captured per
   32-edge frame slid the window +1 edge/frame → 97% BadCrc (the ~2 ms
   inter-frame gap wraps u16 and sneaks past the frametime window). AM32
   `buffersize = 32` = frame-locked: TC on the frame's last edge, re-arm
   in the gap. The 33 was a workaround for a stale slot 0 that the
   RCC-reset re-arm had already cured.
3. **EDT voltage units** (`d0a900d`): `/25` instead of spec 0.25 V/LSB —
   every 3S pack saturated the payload byte (live: `edtv=0x4ff`).
4. **EDT current units** (`d0a900d`): `/50` instead of spec 1 A/LSB.
5. **EDT single-shot init** (`d0a900d`): one lost `0xE00` disabled EDT
   forever → periodic re-announce every 512 responses (phase 256).

## Betaflight facts (hard-won, reverify against BF source when in doubt)

- **`dshot_edt = FORCE` is required on this bench.** With `ON`, BF
  decodes typed frames only after ITS cmd-13 handshake seeds
  `telemetryTypes` (BF sends the burst at motor-init and flight-arm —
  a CLI bench does neither while the ESC can decode; measured: ESC
  cmd-frame counter stayed 0). `FORCE` sets `edtAlwaysDecode`.
  Production quads flight-arm, so `ON` works there (rm32's command gate
  is AM32-verbatim: armed && !running, dshot.c:157 parity).
- BF CLI `exit` and `save` REBOOT the FC → ~1–2 s signal gap → the ESC
  does one reset/re-detect/re-arm cycle (~4 s). Expected, not a fault.
  `bf_cli.py --stay` avoids it but leaves the FC in CLI mode (MSP dead
  until exit).
- The web Configurator (Chrome tab) holds COM42 via WebSerial and
  re-grabs on auto-connect after every FC reboot — close/disconnect it
  before scripting.
- BF keeps `dshot_bidir` across protocol switches — set it explicitly
  OFF before unidir tests. Bidir at DSHOT150 outputs nothing.
- BF motor row flags: R=rpm T=temp V=voltage C=current S=status;
  `dshot_telemetry_info` is the ground-truth readback.

## Bench facts

- **No-signal beep-bootloop every ~2–3 s is stock AM32 behavior** (the
  restored signal-timeout reset). Frames stopping the loop is the health
  signal. `proto=1` + climbing `crc_fail` = wire fine, decode broken.
- **First engage after repower can churn** — non-monotonic garbage eRPM
  ladders (measured twice). Rerun on a clean engage before believing a
  bad ladder. Root cause unexplained — top open investigation.
- The last-words diagnostic (`signal_timeout RESET: proto=... crc_...`)
  plus the frame-delta autopsy print before each reset cracked both
  transport bugs — keep them.
- Flight recorder (SR_* rings) is debuguart-gated now; the `[sr]`
  heartbeat field is its required LTO keep-alive consumer (write-only
  statics get dead-store-eliminated). Rings die on reset — probe-rs
  read them BEFORE the CLI-exit FC reboot. probe-rs RAM reads of a
  running ESC are safe.
- Instruments: `serial_tail.py`, `bf_cli.py`, `bf_motor.py`,
  `protocol_matrix.py` (fresh-evidence detection: settle → flush → two
  consecutive clean samples; stale serial backlog manufactures false
  verdicts), `bidir_load.py`, `edt_check.py`. All kill-guarded.

## Burn-down list (everything left to run into the ground)

Flight-readiness:
- [x] **Full-throttle envelope re-qual UNDER BF DSHOT300: PASS**
      (08-01, post power-fix, full pack): 1,712,046 comms, dsy=0,
      exc=22 (0.01/1k), 0 resets, recorder min_ci=107 (3,115 Hz e at
      100%), 6.43 A peak, 10.49 V min. The parity claim now stands
      wire-to-winding through the flight stack.
- [~] Engage churn class, REVISED: the "always-comp attractor" B5
      evidence from the broken-supply runs was CONFOUNDED (B5 stays on
      bench-era evidence). The remaining beast: **bidir engage on a
      FRESH pack churns** (~10x slow garbage ladders, 3 consecutive
      runs) while unidir on the same pack runs the full envelope clean
      and bidir on a worn pack ran clean — firmware exonerated twice by
      flash-A/B (cb10c0c churns today, was clean this morning). This is
      the parity ledger's "90% restart-transient on fresh pack" item in
      bidir form. Next session: recorder rings + WAX on the engage.
- [x] **EDT FORCE crutch RETIRED** — the full production handshake
      proven end-to-end via MSP RC injection (bf_msp.py --arm-probe /
      --arm-fly): all arming gates cleared (mixer CUSTOM kills
      DSHOT_TELEM; acc_hardware=NONE + rpm_filter_harmonics=0 kill
      LOAD/RPMFILTER), BF ARMED (flightModeFlags bit set), sent its
      arm-time cmd-13 burst (ESC counter 1->61), ESC accepted
      (armed&&!running, AM32-verbatim), EDT activated, and BF decodes
      RTVCS at plain `dshot_edt = ON` — reproduced across two
      independent armed sessions. EDT current: the C flag proves
      current frames flow in loaded sessions; a numeric CURR>=1 capture
      needs an in-stream MSP_MOTOR_TELEMETRY read (throttle-down before
      disarm leaves the retained sample at idle) — polish step.
      bidir@600: committed + stable at idle.
- [x] Protocol robustness soak (FC-reboot axis): **20/20** — 5 cycles
      × DSHOT300/600/150/PWM, zero misses, latency 2.7-7.8 s.
      Cold-boot/battery-replug axes remain (need hands).
- [x] Bidir at idle re-validated on FULL current stack (post Tier-C
      backout): BiDShot stable, crc frozen at the 105 transition tail,
      80k+ passes. The earlier "405/613 bidir death" resolved as
      FC-side config half-states from glitched CLI save sequences —
      verify `get` after every `set`+`save`; wires were never at fault.
- [x] **Downthrottle blip: PASS at the clone standard** (bf_slam.py,
      3 cycles 100%<->10% double slams from running): 305,832 locked
      comms, **dsy=0**, 0 resets; exc=802 (2.6/1k) = the slam
      transients themselves. The historical "blip" is not a desync.
      NOTE: three earlier cm=0 "churn" readings were the instrument
      wiping its own evidence (post-read placed after the CLI-exit FC
      reboot). SCAR: read counters BEFORE teardown, always tee.
- [ ] Bidir at DSHOT600; EDT current under real load (needs >1 A rungs).
- [~] DSHOT command verbs: MSP2 side channel VALIDATED (bf_msp.py,
      cmd counter 0->6 proof); A4 save path FIXED (config write-through
      ring, host-tested); A3 tone channel BUILT + WIRED (ToneScheduler
      stepped by the 20 kHz tick; beacons via cmd 1-5, arming tune on
      just_armed; post-tone spin verified clean — confirm audibility by
      ear). Remaining: exact AM32 beacon-tune port (current notes are
      approximations), direction-change + save-persist verification on
      bench, dumps-over-flight-link verbs, Configurator passthrough.
- [~] AM32 Configurator passthrough: transport chain PROVEN HEADLESSLY
      to the bootloader boundary (bf_4way.py: MSP_SET_4WAY_IF -> 1 ESC,
      4-way protocol v108 alive, frames round-trip). DeviceInitFlash
      fails (ack=15, 4 retries) because the BENCH'S PATCHED BOOTLOADER
      auto-boots the app after ~500 ms idle-high — it never waits in
      DFU for the init sequence. Deliberate bench trade-off (the fast
      auto-boot is what makes the reset loop self-recover); a stock-
      bootloader ESC would complete the hop. Full end-to-end (read/
      write/persist, exercising A2+A4) = stock bootloader + browser
      session, user-assisted.
- [ ] KISS telemetry on PB6 (prod telemetry) + the poll-print
      disturbance question (RTT vs PB6-TX crosstalk) — the last open
      item from the parity campaign.

Production hygiene (docs/public_cleanup_tiers.md + public_pr_plan.md):
- [ ] A3 arming beeps, A4 EEPROM save path, A5 bench_guard thresholds.
- [ ] Tier B decisions (AUTO drive + atomic writer promotion, memory.x
      from build.rs, probe serial), Tier C backout round 2.
- [ ] Cherry-pick the BF-phase commit chain onto the clean branch;
      execute the 18-PR public landing plan.
- [ ] Non-L431 parity ports (NVIC priorities, DMA hygiene, COMP gate)
      + non-L431 bench bringup.
