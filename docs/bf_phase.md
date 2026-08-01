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
- [ ] Full-throttle envelope re-qual UNDER BF DSHOT300 (ladder to 100%
      on battery, dsy=0/exc standard) — the flight-grade sign-off; all
      prior parity numbers were bench-UART throttle.
- [ ] First-engage churn class: root-cause the post-repower garbage
      ladders (recorder rings + WAX are available now).
- [ ] EDT at `dshot_edt = ON` semantics via a real flight-arm (MSP RC
      injection or RX sim) — retire the FORCE crutch on-bench.
- [ ] Protocol robustness soak: each protocol × {cold boot, FC reboot,
      battery replug} × N, zero missed detections/arms.
- [ ] Downthrottle blip vs the clone reference artifact (10% ladder,
      double slams, re-qual protocol).
- [ ] Bidir at DSHOT600; EDT current under real load (needs >1 A rungs).
- [ ] DSHOT command verbs: beacons audible, direction change, save
      settings (needs A4), MSP2_SEND_DSHOT_COMMAND side channel for
      bench dumps ('B'/'H' over the flight link).
- [ ] AM32 Configurator passthrough re-check end-to-end (read + write +
      persist — exercises A2 reset + A4 save path).
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
