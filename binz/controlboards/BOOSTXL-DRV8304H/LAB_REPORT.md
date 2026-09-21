# LAB REPORT — NUCLEO-G071RB ↔ BOOSTXL-DRV8304H wiring validation

## Conventions (read first)
- **APPEND ONLY.** Never edit or delete a prior entry. A correction is a NEW dated
  entry that names the entry/claim it corrects. The record of being wrong stays.
- Every claim carries **EVIDENCE** (the exact command/measurement) and a status:
  - **PROVEN** — directly observed, reproducible.
  - **DISPROVEN** — directly contradicted by observation.
  - **OPEN** — not yet tested.
  - **ASSUMED** — believed but unverified → treated as OPEN until tested.
- A claim is scoped to the **wiring state** it was measured in. The SWD-short rewire
  (2026-09-11) is a hard boundary: nothing measured before it validates the wiring after it.

---

## Entry 001 — 2026-09-12 — Initial state of knowledge

### Wiring-state boundary
After the SWD short (a CN7 sense wire loaded PA13/PA14, killed the debug link), **all
Nucleo-side wires except UART TX/RX were removed and re-landed.** Every result from
before that rewire is INVALIDATED for the present wiring (see bottom of this entry).

### PROVEN (present wiring)
| # | Claim | Evidence |
|---|---|---|
| P1 | SWD flash path works (probe 066CFF…, chip STM32G071RB) | multiple `probe-rs download`/`reset` OK; `probe-rs info` read the ARM DP + ROM table |
| P2 | UART console works, bidirectional. USART3 PC10=TX, PC11=RX ↔ FTDI COM41 @115200. Adapter RX→PC10, TX→PC11, GND common. | shell echoes typed chars and returns command output |
| P3 | DRV user LED wire **PB5 → J4-15** lands correctly | `led-alt` blinked it antiphase with LD4; operator confirmed "blinks correctly" |
| P4 | Nucleo LD4 on **PA5** works | same test |
| P5 | VBUS sense **PA6 ← VSENVM (J5-3)** lands correctly and reads live | `drv-vm`/shell `a`: tracked bus 10.0 / 11.8 / 15.27 V as PSU changed; ≈×11.94, ~1% low |
| P6 | Motor bus power present at the DRV | shell `a`: VBUS 11.6–15.3 V across the session |
| P7 | nFAULT wire **PB14 ← nFAULT (J5-15)** lands; DRV not faulting | shell `i`: `nflt=1` (high = no fault) at rest and with en=1 |
| P8 | A current path exists: with **en=1**, static vector **ah(PA10)=1 + bl(PB0)=1** drew ~1 A A→B, bus 11.72→11.60 V | shell: `en=1; ah=1; bl=1; a` → IB CSA 1658→1571 mV (≈1.2 A), VBUS sag 120 mV |
| P9 | DRV is enabled when en=1 (gate outputs pass) | P8 current only flows if the DRV is out of sleep |

### DISPROVEN
| # | Claim | Evidence |
|---|---|---|
| D1 | The ≤10% bit-banged sine (1 kHz carrier, 10 Hz elec) moves the motor | shell `sine`: current flowed, VBUS stable, VSENC pulled to 0 — but **operator: "absolutely nothing moved."** Attributed to weak drive (10% smeared over 3 phases → tiny phase-to-phase differential), NOT yet shown to be a wiring fault. |

### OPEN (must validate before trusting the wiring)
| # | Question |
|---|---|
| O1 | Do gates **INHB(PA9), INHC(PA8), INLA(PB1), INLC(PA7)** land correctly? (only PA10/PB0 exercised, in P8) |
| O2 | Does P8's current actually flow through phase **A high + B low**, i.e. do `ah`/`bl` map to the intended FETs? (current proves *a* path, not *which*) |
| O3 | Do BEMF sense wires **VSENA→PB3, VSENB→PB7, VSENC→PA2, neutral→PA3** land correctly? |
| O4 | Do current wires **ISENA→PA0, ISENB→PA1, ISENC→PA4** land correctly? (P8's IB deflection is suggestive, not isolated) |
| O5 | Is ENABLE driven by **PD1→J5-9**, or is the DRV self-enabled by the EVM bias? (never tested en=0 vs en=1 for gate action) |
| O6 | Is the motor connected to all three **J2 (MOTA/MOTB/MOTC)**? (A & B implied by P8; C untested) |
| O7 | Did the rotor produce torque on the static vector? (electrical current proven; physical motion not yet reported) |

### INVALIDATED PRIOR EVIDENCE
- **"Motor spun under `drv-spin6x` six-step" — VOID.** Measured before the SWD-short
  rewire. All Nucleo-side gate/sense wires were re-landed since; it does not validate
  the present wiring.

---

## Entry 002 — 2026-09-12 — Validation protocol (planned, not yet executed)

Uses the live `shell` firmware (interactive over COM41). Each test isolates a wire by
driving a pin and reading an independent channel that must respond. Power on, PSU
current-limited (~0.5–1 A). Between every test: `off`.

- **V1 — Phase-C gate + sense** (C is ADC-readable on PA2=VSENC):
  `off; a` (baseline VSENC) → `en=1; ch=1; a` (expect VSENC → ~VM×0.084 ≈ high) →
  `off; cl=1; a` (expect VSENC → ~0). PROVES INHC(PA8), INLC(PA7), VSENC(PA2).
- **V2 — Phase-A gate + sense** (A on comparator PB3):
  `en=1; ah=1; c` then `off; al=1; c` — the A comparator must flip between the two.
  PROVES INHA(PA10), INLA(PB1), VSENA(PB3), neutral(PA3).
- **V3 — Phase-B gate + sense** (B on comparator PB7): as V2 with `bh`/`bl`, watch B.
  PROVES INHB(PA9), INLB(PB0), VSENB(PB7).
- **V4 — ENABLE isolation:** `en=0; ah=1; bl=1; a` (expect NO current) then `en=1; a`
  (expect current). PROVES O5 (PD1→ENABLE) vs EVM self-bias.
- **V5 — Current-sense wires:** drive each conducting vector, read the low-side CSA of
  the sinking phase (IA/IB/IC on PA0/PA1/PA4). Each must deflect from idle only when
  its phase sinks. PROVES ISENA/B/C.
- **V6 — Motor continuity (power OFF):** DMM each J2 phase pin to the motor lead; and
  gate wire continuity G071 pin ↔ J4 pin, no cross-wiring.
- **V7 — Torque:** operator watches the shaft during V1–V3 highs/lows and the static
  vector for a snap/lock/step.

Results append as Entry 003+.

---

## Entry 003 — 2026-09-12 — V1/V2 executed: HIGH SIDES ARE NOT SOURCING

Ran via live `shell-sine` firmware, PSU on (~11.7 V), current-limited. Phase C read
quantitatively on ADC VSENC (PA2); A/B read on the COMP2 comparator (`c`), where
`hi` = phase BELOW neutral, `lo` = phase ABOVE neutral.

### Raw data
| cmd | VSENC (PA2) | NEU (PA3) | currents (IA/IB/IC mV, idle≈1655) | note |
|---|---|---|---|---|
| all off (baseline) | **820 mV** | 819 mV | 1653/1663/1647 (idle) | phases float ~VM (body-diode pull-up) |
| en=1, ch=1 (C high) | **0 mV** | 2 | 1603/1590/1650 | C went to GND — WRONG for a high-side |
| en=1, cl=1 (C low) | **4 mV** | 0 | 1611/1611/1647 | C at GND — correct for a low-side |
| en=1, ch=1+al=1 (C high, A low) | **0 mV** | 1 | 1587/1581/1639 | C still GND with a real return — C never sources |
| en=1, ah=1+bl=1 (A high, B low) | 0 | — | `c` → **A=hi B=hi C=lo** | A commanded HIGH reads BELOW neutral |

### PROVEN
- **P10 — Phase C high side does not source.** Commanding C high (ch=PA8=INHC), with or
  without a return path, leaves phase C at GND (VSENC 0 mV vs 820 mV floating). Cleanest
  single measurement in the set (VSENC is a direct ADC read).
- **P11 — Low-side pull works on C.** cl=1 → VSENC 0 (expected). A phase CAN be pulled to GND.
- **P12 — VSENC (PA2) and neutral (PA3) sense wires are live and land correctly.** They read
  820/819 at rest and track to 0 under drive — they respond to real phase-node changes.

### DISPROVEN / corrected
- **Corrects P8/P9 interpretation (Entry 001).** The ~1 A in the static vector was read as
  "a high side works." Entry 003 shows it flows through the **high-side body diodes** +
  commanded low sides, NOT through any high-side FET: with B low and A/C floating at ~VM
  (diode pull-up), VM→(A high-side body diode)→A→motor→B(low)→GND carries ~1 A with every
  high FET OFF. So P8 does NOT prove INHA works; it only proves a diode+low-side path exists.
- **Explains D1 (Entry 001).** The sine "moved nothing" because low-sides + body-diodes
  cannot make a rotating field — no high-side modulation, no net torque. Not a weak-drive
  problem after all; a no-high-side problem.

### HYPOTHESIS (unproven — do not treat as fact)
High-side drive is dead across phases. Candidate causes, in order to test:
1. **High-side gate-input wires mis-landed in the post-SWD rewire** — INHA/INHB/INHC =
   PA10/PA9/PA8 → J4-1/5/9. The rewire touched exactly these. (A single-phase test only
   cleanly covered C; A/B inferred.)
2. **Charge pump / high-side gate supply (VCP) not running** — would kill all 3 highs at
   once (more parsimonious than 3 wire errors). On the DRV board, not the rewire — but
   check ENABLE is FULLY asserting (not just the ~1.65 V EVM bias) since the charge pump
   needs the device awake.
3. MCU high-side pins not actually toggling (least likely — `i`/`p` show ODR follows).

### NEXT (planned)
- **V6a (do first, decisive, power OFF): DMM continuity** G071 **PA10→J4-1, PA9→J4-5,
  PA8→J4-9** (the three high-side gate wires). A mis-land here is the top suspect.
- **V2b: clean A & B high-side test** — ah=1 + bl=1 + cl=1 (A high, both others low): if A
  high works, current rises and A sits at VM; compare to C.
- **Charge-pump check:** meter VCP/VM test points on the DRV board if continuity is good.
- Not yet run: V3 (phase B), V4 (ENABLE isolation en=0 vs en=1), V5 (current-sense wires),
  V6 motor continuity, V7 torque.

---

## Entry 004 — 2026-09-12 — V2b: ALL THREE high sides dead (uniform)

Test: `en=1; ah=1; bl=1; cl=1` (A high, B and C low — two return paths for A).
Result: `c` → **A=hi B=hi C=hi** (all phases at/below neutral), VSENC=0, NEU=0,
IA=IB=1528 mV (~1.9 A via body diodes), IC idle.

### PROVEN
- **P13 — All three high sides fail identically.** Phase A commanded high, with two
  low-side returns, still never rises above neutral. Same failure as phase C (P10).
  The three high sides are uniformly non-sourcing.

### HYPOTHESIS — refined (leading candidate is now WIRING)
Uniform failure of all three highs, with all three **low** sides working, has one clean
mechanical explanation: the **three high-side gate-input wires do not land at the DRV** —
INHA=PA10→J4-1, INHB=PA9→J4-5, INHC=PA8→J4-9 — so INHx sits low/floating at the DRV and
the high FETs are never commanded on, while INLA/B/C (J4-3/7/11) are connected and the low
FETs obey. This fits: (a) the post-SWD rewire touched exactly these gate wires; (b) the
low sides demonstrably work; (c) `drv-spin6x` DID drive high sides before the rewire
(Entry 001 INVALIDATED note) — so the silicon/charge-pump was fine then. Charge-pump death
is the alternative but is less likely given the wires were just redone and lows work.
Still a hypothesis until continuity is metered.

### DECISIVE NEXT — V6a (power OFF, DMM continuity)
Meter G071 pin ↔ DRV J4 pin for the three HIGH-side wires:
- **PA10 (D2 / CN10-33) ↔ J4-1 (INHA)**
- **PA9  (D8 / CN10-21) ↔ J4-5 (INHB)**
- **PA8  (D7 / CN10-23) ↔ J4-9 (INHC)**
Any open / wrong-pin here confirms the fault. Also meter the three lows (J4-3/7/11) as the
known-good control — they should ring out. If all six ring out correctly, escalate to the
DRV charge-pump/VCP check (board-level, not wiring).

---

## Entry 005 — 2026-09-12 — V4: PD1 does not disable the DRV

Ran twice through the live `shell-sine` firmware over USART3/COM41. This test used only
the C low-side command; no high-side/source command or source-sink motor vector was
applied. Each sequence ended with `off` and an output-state readback.

### Raw data
| sequence | VBUS | IA / IB / IC | VSENC | NEU |
|---|---:|---:|---:|---:|
| baseline `off` | 11641 mV | 1657 / 1661 / 1648 mV | 823 mV | 822 mV |
| `en=0; cl=1; a` | 11820 mV | 1669 / 1662 / 1666 mV | **0 mV** | 0 mV |
| `en=1; cl=1; a` | 11808 mV | 1658 / 1656 / 1651 mV | **2 mV** | 0 mV |
| repeat `en=0; cl=1; a` | 11593 mV | 1658 / 1658 / 1660 mV | **0 mV** | 0 mV |
| repeat `en=1; cl=1; a` | 11725 mV | 1662 / 1650 / 1651 mV | **0 mV** | 0 mV |

Final `p` readback: `ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0`, `sine=0`.

### PROVEN
- **P14 — The C low-side responds with PD1 commanded either low or high.** PA7/`cl=1`
  pulls VSENC from the 823 mV floating baseline to 0–2 mV in both states. The result was
  reproduced.
- **P15 — The test did not create a sustained bus-current event.** VBUS remained
  11.59–11.82 V and all current-amplifier readings remained near their half-supply
  offsets.

### DISPROVEN
- **D2 — The present PD1 control path does not disable gate action.** If
  PD1→J5-9 controlled ENABLE as intended, `en=0; cl=1` could not pull phase C to ground.
  Therefore `off` clears all six MCU gate commands but cannot presently be credited with
  putting the DRV8304 to sleep.

### OPEN — cause not yet isolated
- PD1→J5-9 may be open or mislanded.
- J5-9 may be tied or biased enabled independently of PD1.
- The voltage at J5-9 has not been measured, so this entry does not distinguish those
  causes.

---

## Entry 006 — 2026-09-12 — Single-low phase signatures are non-identifying with motor connected

Goal: use one low-side command at a time and the three BEMF feedback paths to map each
MCU output to a DRV phase. All high-side commands were held low. Each case began and
ended with `off`.

### Raw data
| command | COMP2 report | VBUS | IA / IB / IC | VSENC | NEU |
|---|---|---:|---:|---:|---:|
| `al=1` | A=hi B=lo C=hi | 11772 mV | 1667 / 1659 / 1662 mV | 12 mV | 0 mV |
| `bl=1` | A=hi B=hi C=hi | 11784 mV | 1664 / 1663 / 1660 mV | 0 mV | 2 mV |
| `cl=1` | A=hi B=lo C=hi | 11665 mV | 1672 / 1668 / 1622 mV | 3 mV | 0 mV |

Final `p` readback: all six gate commands and `en` low, `sine=0`.

### PROVEN
- **P16 — Each of `al`, `bl`, and `cl` produces a downstream phase-network
  response.** In every case VSENC moved from its ~823 mV all-off value to 0–12 mV.
  Therefore none of those three MCU commands is wholly inert at the power-stage/motor
  network level.
- **P17 — The energized motor windings couple the phase observations.** Pulling the A,
  B, or C command path low also pulled the measured C phase and resistor-star neutral
  near ground. VBUS remained 11.67–11.78 V.

### NOT PROVEN
- These observations do **not** prove that `al`, `bl`, and `cl` land on INLA, INLB, and
  INLC respectively. With J2/motor connected, a low phase pulls the other phase nodes
  through the windings, so the BEMF comparator pattern is not a unique phase signature.
- The current-amplifier values in this no-source test are too close to their offsets and
  insufficiently repeatable to identify the active low-side channel.

### DECISIVE NEXT
Temporarily disconnect all three motor leads from J2. With the phase nodes isolated,
repeat each of the six individual gate commands. A low command must pull only its own
phase below neutral; a high command must raise only its own phase above neutral. This
tests the gate-input and phase-feedback wiring without motor current and cleanly
separates a gate-wire fault from charge-pump/high-side failure.

---

## Entry 007 — 2026-09-12 — Corrected PD1 wiring proves ENABLE; wake-state anomaly remains

The user found PD1 miswired at the Nucleo end and corrected it. The motor's single
three-wire plug was disconnected for every test in this entry. Firmware was
`shell-sine` over USART3/COM41; each sequence ended with `off`.

### Direct ENABLE measurement after correction
With a DMM on DRV header J5-9:

| firmware command/state | J5-9 measured |
|---|---:|
| `off` / `en=0` | **0.001 V** |
| `en=1` | **3.3 V** |

This supersedes Entry 005's conclusion about the old, miswired PD1 path. Entry 005 is
retained as the observation that led to finding the wiring error.

### Firmware-observed functional comparison, PSU at about 11.7 V

| state | VBUS | IA / IB / IC | VSENC | NEU | COMP2 | nFAULT |
|---|---:|---:|---:|---:|---|---:|
| `off` (`en=0`, all gates 0) | 11617 mV | 892 / 862 / 1448 mV | **1 mV** | 3 mV | A=hi B=hi C=hi | 1 |
| `en=1`, all gates 0 | 11665 mV | 1666 / 1655 / 1656 mV | **842 mV** | 822 mV | A=hi B=hi C=lo | 1 |
| `en=0; ch=1` (earlier run) | 11808 mV | 781 / 769 / 1331 mV | **4 mV** | 0 mV | A=hi B=hi C=hi | 1 |
| `en=1; ch=1` (earlier run) | 11701 mV | 1667 / 1675 / 1658 mV | **971 mV** | 869 mV | A=hi B=hi C=lo | 1 |

MCU readback confirmed all six gate commands low in both all-zero rows. In the `ch=1`
rows it confirmed only `ch` high. The final readback was all six gate commands low,
`en=0`, and `sine=0`.

### PROVEN
- **P18 — The corrected PD1-to-J5-9 ENABLE wire works end-to-end.** The physical pin
  follows the firmware command from 1 mV to 3.3 V.
- **P19 — ENABLE controls DRV power state.** With ENABLE low, the current-sense outputs
  lose their approximately half-supply bias and the phase/neutral feedback falls to
  ground. With ENABLE high, all three current-sense outputs return to about 1.66 V.
- **P20 — The C high command has an ENABLE-dependent downstream effect.** `ch=1` does
  nothing measurable while asleep, but while awake it raises VSENC from 842 mV to
  971 mV and produces the C-above-neutral comparator signature. VBUS stays stable and
  nFAULT remains deasserted. This disproves a total charge-pump/all-high-sides failure.

### OPEN — all-zero awake state is abnormal
With the motor unplugged and all six MCU gate commands confirmed low, merely asserting
ENABLE reproducibly moves VSENC from about 1–5 mV to about 840 mV and NEU from about
2–3 mV to about 822 mV. The board's 82 kΩ / 7.5 kΩ phase divider should pull an isolated
Hi-Z phase toward ground, so this cannot yet be called a normal floating-phase value.

### DECISIVE NEXT
Measure the physical MOTC/J2 phase-C terminal to power ground in the two firmware-held
states `en=0, all gates=0` and `en=1, all gates=0`. If MOTC itself rises with ENABLE,
the unexpected state is in the driver/power stage. If MOTC stays near 0 V while VSENC
reports about 840 mV, the C feedback wire or ADC landing is wrong.

---

## Entry 008 — 2026-09-12 — Unloaded Hi-Z clarification and first monitored motor burst

### Unloaded measurements
With the motor unplugged, PSU at approximately 11.7 V, and all six MCU gate outputs
confirmed low:

| state | MOTA | MOTB | MOTC | VSENC / NEU |
|---|---:|---:|---:|---:|
| `en=0` | ~0 V | ~0 V | ~0 V | 1–13 mV / 3 mV |
| `en=1` | 9.6 V | 9.7 V | 10.0 V | 840 mV / 823 mV |

At the DRV header in the awake all-zero state, DMM readings were INHA=0.02 V and
INLA=0.02 V. Across the phase-A high-side MOSFET control, black on TP8/MOTA and red on
TP7/GHA, the measured gate-to-source voltage was **0 V**.

### CORRECTION to Entry 007 interpretation
The high unloaded phase voltages do not prove that the high-side MOSFETs are on.
For phase A, both logic inputs are low at the DRV and GHA−MOTA is 0 V, directly proving
the high-side gate is not enhanced. Treat the phase rise as a Hi-Z/leakage/coupling
observation unless a loaded or gate-to-source measurement proves otherwise. The MCU
to J4 A-input wiring remains consistent with the intended off command.

### Motor-connected monitored burst
The motor's three-wire plug was reconnected only after the PSU was switched off.
Operator-selected envelope:

- firmware: `shell-sine`
- carrier: 1 kHz bit-banged GPIO
- electrical frequency: 50 Hz
- maximum on-time: 100 µs / 10%
- commanded duration: 5 seconds (firmware also retained its 8-second timeout)
- PSU current limit: 0.8 A
- powered baseline: VBUS=11904 mV; IA/IB/IC=1666/1664/1661 mV;
  nFAULT=1

The host issued `sine`, polled feedback throughout, then issued `off` at 5.201 s.

| observed during burst | result |
|---|---|
| VBUS range | **11701–11868 mV** |
| sampled IA range | **−142 to +114 mA** |
| sampled IB range | **−42 to +214 mA** |
| sampled IC range | **−57 to +185 mA** |
| VSENC range | **0–8 mV** |
| NEU range | **0–5 mV** |
| nFAULT | **1 in every poll** |
| host abort | none |

Final readback: `ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0`, `sine=0`, `sf=50Hz`;
VBUS=11772 mV and nFAULT=1.

### PROVEN
- **P21 — The requested five-second low-duty command completed without bus collapse or
  a reported DRV fault.** The 11.7–11.9 V bus stayed far below the 0.8 A PSU-limit
  collapse signature, and nFAULT stayed deasserted.
- **P22 — Host and firmware safing completed.** The bridge ended with all six commands
  and ENABLE low.

### NOT PROVEN
- The asynchronous `a`, `c`, and `i` shell requests all sampled the approximately 90%
  low-side/freewheel interval (`ah/bh/ch=0`, `al/bl/cl=1`). They did not directly
  capture any 100 µs high-side pulse. Therefore this run alone does not prove that all
  three high-side wires switched or that a rotating field reached the motor.
- Current samples near zero and a stable bus show that the run was electrically mild;
  they are not proof of rotation. Operator observation of motion, sound, and PSU
  behavior remains to be added.

### DECISIVE NEXT
Add carrier-synchronous firmware capture inside `sine_pulse`: sample the three current
amplifiers and phase feedback during the high interval and during the low/freewheel
interval, accumulate extrema/counts for each phase, and print a summary only after the
bridge is safely off. This will verify pulse delivery using onboard feedback without
randomly timed DMM or serial samples.

### Operator observation added after the run
The motor **audibly and mechanically tried to spin, very noisily, but did not begin
continuous rotation**. This proves the energized bridge produced motor torque. It does
not distinguish an excessively abrupt 0→50 Hz open-loop start from a missing or
unbalanced phase drive.

---

## Entry 009 — 2026-09-12 — Corrected complementary PWM and RAM capture

The `shell-sine` bit-bang path had a concrete waveform defect: after a phase's high-side
pulse ended, its low side stayed off until the longest of all three high pulses ended.
The shorter-duty phases were therefore Hi-Z instead of low during the intended
line-to-line drive interval. `sine_pulse` was corrected to turn off equal-deadline high
gates as a group, wait 1 µs, and restore each corresponding low side immediately.

The firmware now also uses a 500-record / 12 KiB rolling RAM capture. At 500 Hz it
retains the final one second of raw IA, IB, IC, VSENC, neutral, VBUS, VREFINT, electrical
phase/frequency, commanded on-times, and nFAULT. It disables all gates and ENABLE before
emitting an explicitly framed fixed-width hex dump. The host tool
`scripts/drv_capture.py` validates, saves, decodes, plots, and can replay a saved dump.

### Corrected-waveform test at 10% maximum
At 5 Hz electrical and 100 µs maximum on-time, the DRV asserted nFAULT after about
20 ms. Firmware stopped and disabled the bridge. The two pre-fault records showed:

- IA raw at the negative ADC rail (`0`)
- IB raw 3011–3106 and IC raw 2388–2587
- VBUS approximately 11.23 V then 10.39 V

Artifact: `captures/drv_5hz_corrected_20260912.{txt,csv,png}`.

This is a real high-current/fault event, not a serial-decoder artifact. The 0.8 A PSU
limit constrains average bus current but does not prevent the EVM bulk capacitor from
supplying a much larger millisecond-scale recirculating phase current.

### Corrected-waveform test at 1% maximum
Runtime maximum on-time was reduced to 10 µs (1%). Same motor, 5 Hz electrical,
1 kHz carrier, 5 seconds, 0.8 A PSU limit. The run reached its normal firmware timeout
and produced all 500 expected records:

| quantity | observed |
|---|---:|
| nFAULT-low samples | 0 |
| VBUS | 11.506–12.109 V |
| IA | −9.737 to +7.167 A |
| IB | −7.370 to +5.920 A |
| IC | −5.723 to +4.823 A |

The large phase-current values are not comparable to the PSU's 0.8 A bus-current limit;
they include recirculating motor current. None of the three ADC channels railed in this
run.

Five-Hz fundamental fits over the retained one-second window:

| channel | amplitude | phase |
|---|---:|---:|
| IA | 7.512 A | 150.5° |
| IB | 5.956 A | 30.3° |
| IC | 4.770 A | −89.3° |

Adjacent phase separations are 120.2° and 119.6°. Pairwise waveform correlations are
−0.507, −0.492, and −0.499 (ideal balanced three-phase sinusoids: −0.5).

Artifact: `captures/drv_5hz_1pct_ring_20260912.{txt,csv,png}`.

### PROVEN
- **P23 — All three commanded phase waveforms are present in the current-feedback
  network with the correct 120° sequence.** This strongly supports correct participation
  of all three gate pairs and all three CSA wires; it is incompatible with one wholly
  dead phase.
- **P24 — The rolling capture/dump/replay path works end-to-end on the G071.** It filled,
  froze only after drive disable, emitted 500/500 valid records, decoded, and rendered.
- **P25 — One percent is inside the present electrical fault envelope; ten percent is
  not.** The 1% run completed five seconds without nFAULT or bus collapse; corrected
  10% drive faulted in about 20 ms.

### CAUTION — apparent current-amplitude imbalance is not isolated
ADC sampling is software-sequential in the order IA, IB, IC after the PWM pulse. Current
can decay materially across that scan, so the order itself can make IA appear largest
and IC smallest. Do not diagnose winding, shunt, or gate imbalance from these amplitudes
until the acquisition order is rotated or a hardware-triggered scan is used.

---

## Entry 010 - 2026-09-12 - Startup campaign instrumentation and first 6% attempts

The operator supplied a new measured constraint: this motor will not spin below 6%
duty at the present bus voltage. The prior 1% result is therefore retained as a clean
wiring/feedback diagnostic, not treated as a viable startup setting.

`shell-sine` now has a `run<hz>` campaign mode with explicit ALIGN, START, RAMP, HOLD,
and disabled COAST states. The full five-second energized interval is sampled at 100 Hz
into a 500-record ring. IA/IB/IC conversion order rotates ABC/BCA/CAB. Every record also
contains VBUS, VSENC, neutral, VREFINT, commanded phase/frequency/on-times, state, nFAULT,
and comparator A/B/C. After any normal limit or guard, all six gates and ENABLE are
cleared before a separate 500-record, 1 kHz coast capture and serial dump. A local panic
handler replaces the incompatible EVLDRIVE board safing map. Release build static RAM is
19,096 bytes of the G071's 36 KiB.

### Attempt 1 - 250 ms ALIGN, 1 Hz START, target 10 Hz, 6% maximum

- nFAULT stopped drive at approximately 291 ms; 29 complete 100 Hz drive records were
  retained (`reason=2`).
- All 25 ALIGN samples through 250 ms had VBUS in the expected range. Once the 1 Hz
  START began, sampled VBUS fell 11.50 -> 10.34 -> 7.97 -> 6.98 V over 40 ms.
- Coast data did not prove motion: C-minus-neutral span was only 13.8 mV and the ordered
  comparator-transition score was 0.200 after debounce.
- Artifact: `captures/drv_campaign_10hz_6pct_20260912.{txt,csv,png}` plus
  `captures/drv_campaign_10hz_6pct_20260912_coast.csv`.

### Attempt 2 - 250 ms ALIGN, 5 Hz START, target 10 Hz, 6% maximum

- nFAULT stopped drive at the ALIGN-to-START boundary; all 25 retained records were
  ALIGN (`reason=2`), so no complete 10 ms START sample occurred.
- In this attempt VBUS began collapsing during the high end of the rising ALIGN:
  11.45 V at 220 ms, 9.84 V at 230 ms, 8.24 V at 240 ms, and 7.31 V at 250 ms.
- Coast data again disproved meaningful rotation: C-minus-neutral span 11.4 mV and
  comparator phase-order score 0.214.
- Artifact: `captures/drv_campaign_5to10hz_6pct_20260912.{txt,csv,png}` plus
  `captures/drv_campaign_5to10hz_6pct_20260912_coast.csv`.

Both host exits were followed by direct `p` readback showing AH/BH/CH/AL/BL/CL=0,
ENABLE=0, drive mode=0, and coast=0.

### PROVEN

- **P26 - Six percent is not safe as a near-static 250 ms dwell with the PSU limited
  to 0.8 A.** The bus collapses as the alignment amplitude approaches 6%, then nFAULT
  asserts. Changing the following START from 1 Hz to 5 Hz could not help because the
  second attempt reached the collapse before START was sampled.
- **P27 - Comparator chatter near zero is not rotor motion.** Coast speed is now accepted
  only with debounced edges on all phases, a coherent cyclic phase order, and material
  phase-C-minus-neutral amplitude. Both attempts failed those gates.

### DECISIVE NEXT

Reduce ALIGN from 250 ms to 20 ms, immediately enter a 5 Hz START at the operator's 6%
minimum, and ramp 5 -> 10 Hz. This changes only the demonstrated unsafe dwell time while
preserving the requested duty floor and capture/abort envelope.

---

## Entry 011 - 2026-09-12 - High-frequency catch and successful 50 Hz hold

### Why the low-frequency approach was abandoned

A third bit-banged attempt reduced ALIGN to 20 ms, then applied 6% at 5 Hz. It
asserted nFAULT after about 120 ms total (`reason=2`), with VBUS reaching 7.09 V.
The coast record had no valid motion signature. Artifact:
`captures/drv_campaign_shortalign_5to10hz_6pct_20260912.{txt,csv,png}` plus its
`_coast.csv`.

The operator correctly identified the underlying control error: a low electrical
frequency is not required for this motor to follow a sinusoidal field. The local older
bench record also identifies approximately 7% / 100 electrical Hz as a catch point.
At 1-5 Hz, 6% instead behaves like a long near-DC winding excitation and produces the
observed stall current and PSU-limit collapse.

For separation of waveform correctness from motor response, a final 1 kHz bit-banged
test ran at fixed 100 Hz / 6% for five seconds. Its exact repeating ten-point command
sequence fitted at 100 Hz with 119.0 and 119.7 degree phase separation. Current feedback
also participated on all phases with approximately 120 degree order. However, VBUS was
current-limited to 7.66-8.51 V and the coast signature was incoherent, so this was not
accepted as rotation. Artifact:
`captures/drv_fixed_100hz_6pct_1khzcap_20260912.{txt,csv,png}` plus `_coast.csv`.

### Raw TIM1 PWM conversion

The drive was converted to the repository's bench-proven raw TIM1 register pattern:

- 10 kHz carrier: ARR=6399 at 64 MHz
- PWM1 plus preload on CH1/CH2/CH3
- main and complementary outputs enabled, active high
- approximately 0.4 us hardware dead time
- native pin pairs mapped CH3=A (PA10/PB1), CH2=B (PA9/PB0), CH1=C (PA8/PA7)
- MOE=0 plus CCR1/2/3=0 on every stop, followed by DRV ENABLE=0

The runtime `du` value now represents tenths of one percent; `du70` is 7.0% and yields
at most about 7 us ON in each 100 us carrier period. A preflight readback before drive
showed ARR=6399, all CCRs=0, MOE=0, ENABLE=0, and nFAULT=1.

A two-second fixed 100 Hz / 7% TIM1 catch test then completed without fault:

| quantity | observed |
|---|---:|
| VBUS | 11.46-12.08 V |
| IA | -7.22 to +8.09 A |
| IB | -8.86 to +7.25 A |
| IC | -10.68 to +10.85 A |
| nFAULT-low samples | 0 |

The first 80 ms of the 2 kHz disabled coast capture produced a repeating ordered
sequence including `B- C+ A- B+ C- A+`. Debounced phase order score was 0.824. A
quadratic electrical-phase-versus-time fit (to account for rapid coast deceleration)
had R2=0.99721 and extrapolated 45.23 electrical Hz at disable. This is proof of actual
rotation, though the rotor was not following the commanded 100 Hz field synchronously.
Artifact: `captures/drv_tim1_100hz_7pct_2s_20260912.{txt,csv,png}` plus `_coast.csv`.

### Accepted campaign - 100 Hz catch -> 50 Hz hold

Profile: 20 ms ALIGN at 1%, 980 ms CATCH at 100 Hz / 7%, 2.0 s controlled ramp to
50 Hz / 6%, 2.0 s HOLD at 50 Hz / 6%, followed by gates/MOE/ENABLE off and a 250 ms,
2 kHz coast capture. The attempt reached its normal five-second energized limit
(`reason=1`) with all 500 retained drive records from the final hold:

| quantity | observed |
|---|---:|
| commanded hold | 50.0 Hz, 6.0% peak |
| VBUS | 11.10-12.14 V (mean 11.68 V) |
| IA fundamental | 3.24 A |
| IB fundamental | 2.95 A |
| IC fundamental | 3.15 A |
| current phase separation | 120.0 / 120.1 degrees |
| nFAULT-low samples | 0 |

Coast BEMF was unambiguous: 16 consecutive debounced transitions followed A->B->C
with score 1.000 and alternating polarity. Phase-C-minus-neutral spanned 13.7 mV and
changed mean level by 2.3 mV with comparator C. Because this low-inertia motor slows
materially during the observation window, the raw-window median is only 35.71 Hz and
must not be mistaken for speed at switch-off. Fitting accumulated electrical phase
against coast time gave R2=0.99783 and an extrapolated switch-off speed of **47.24
electrical Hz**, inside the 45-55 Hz acceptance band.

Artifact: `captures/drv_tim1_catch100_ramp50_hold_6pct_20260912.{txt,csv,png}` plus
`captures/drv_tim1_catch100_ramp50_hold_6pct_20260912_coast.csv`.

A repeat with the 50 Hz hold raised to 7% also completed cleanly (VBUS 11.34-12.03 V,
no fault) but added current without improving speed. Its coast fit was 45.75 Hz with
R2=0.99906. Therefore 6% is retained as the better 50 Hz hold setting. Artifact:
`captures/drv_tim1_catch100_ramp50_hold_7pct_20260912.{txt,csv,png}` plus `_coast.csv`.

Final direct serial readback after the repeat:

```text
AH/BH/CH/AL/BL/CL = 0
ENABLE = 0
TIM1 MOE = 0
CCR A/B/C = 0/0/0
nFAULT = 1
drive mode = 0; coast = 0
```

### PROVEN

- **P28 - The six gate pairs and TIM1 peripheral mapping produce a sensible balanced
  three-phase waveform.** Both commanded duty and measured current have the expected
  phase order; the final 50 Hz currents are balanced within about 10% in amplitude.
- **P29 - The motor starts and rotates continuously under the 100 Hz catch -> 50 Hz
  hold profile.** Ordered, alternating three-phase coast BEMF distinguishes this from
  torque twitching, motor noise, or command-only evidence.
- **P30 - The requested 50 electrical Hz result is achieved.** The no-fault 50 Hz hold
  ends at a coast-extrapolated 47.24 Hz, within the specified 45-55 Hz band, with no bus
  collapse and a verified safe shutdown.

---

## Entry 012 — 2026-09-12 — Dedicated hardware-PWM clone and repeatability

Same free-wired NUCLEO-G071RB / BOOSTXL-DRV8304H rig and connected motor;
operator-set PSU limit 800 mA, supply left on, PD1 ENABLE used for isolation.
No wiring change in this campaign. Firmware: `examples/shell-pwm.rs`, cloned
from the already-working raw-TIM1 version of `shell-sine.rs`. The reference
file remains byte-identical, SHA256:
`B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E`.

Removed unused GPIO bit-bang functions and manual gate/ENABLE commands.
Duty variables now explicitly use tenths of a percent, with a 10% clamp.
Preserved the 100 eHz / 7% catch, ramp to 50 eHz / 6%, hardware complementary
drive, fault/ADC guards, local board-correct panic safing, and capture/replay.
This clone is a cleanup and verification of the initial TIM1 conversion,
not evidence that the immediately preceding reference still bit-banged.

### Timing and peripheral verification

Added `pwmcheck`: with ENABLE=0, count PA10 rising edges at 6% duty, then
disable MOE and zero all CCRs. Both initial and final checks returned:

```text
PWMCHECK en=0 edges=101 span_us=10000 PSC=0 ARR=6399 CCMR1=6868 CCMR2=68 CCER=555 BDTR=c1a
```

This measures 100 PWM periods in 10 ms against TIM17: 10 kHz, with the expected
raw register configuration and A=CH3, B=CH2, C=CH1 mapping. DTG=26 at 64 MHz
configures 406.25 ns dead-time; physical gate dead-time was not scope-measured.
Added 500 actual coast timestamps (about 2 KiB RAM). Initial coast intervals
were 500/501 us, supporting 2 kHz capture. Replay now uses measured coast time
when present; legacy dumps retain nominal timing.

The first clone attempt reported 5,000,096 us energized. A foreground wall-time
cutoff at 4,999,000 us now leaves one control interval of margin. Final-build
runs 02/03 reported 4,999,099 / 4,999,063 us, each 4987 control ticks. Maximum
control interval was 7641 / 7642 us: startup console output stretches the
initial update. Hardware PWM continues independently. Drive record time is
still nominal tick time; coast elapsed time is measured. This is a polling
timeout, not independent hardware protection against a hung CPU.

### Acceptance results

Pre-run gates: normal timeout, no sampled fault, no sustained bus collapse,
ordered alternating coast transitions, fitted disable speed 45–55 eHz,
then gates/MOE/ENABLE off. Each full attempt retained 500 drive and 500 coast
records. All three full attempts met the feedback gates:

| Capture suffix | VBUS range | Disable speed fit | Fit R2 | Phase order score | Stop |
|---|---|---|---|---|---|
| validation_01 (before margin fix) | 11.00–12.17 V | 52.07 eHz | 0.99837 | 1.000 | reason 1 |
| validation_02 (final build) | 10.80–12.22 V | 53.07 eHz | 0.99920 | 1.000 | reason 1 |
| validation_03 (final build) | 11.45–12.26 V | 53.15 eHz | 0.99905 | 1.000 | reason 1 |

No nFAULT-low samples in these captures. Early-coast cycle estimates were
47.59 and 49.97 eHz for runs 02/03. Disable speeds above are extrapolated
from the first 80 ms of coast, not direct tachometer readings or proof of
continuous synchronous tracking throughout startup. Late coast becomes noisy
as the rotor stops; it is excluded from the speed fit.

Least-squares current fundamental versus captured commanded electrical angle:

| Dataset | IA / IB / IC fundamental amplitude |
|---|---|
| Entry 011 accepted reference | 3.238 / 2.953 / 3.146 A |
| Clone validation_02 | 3.061 / 3.032 / 3.040 A |
| Clone validation_03 | 2.980 / 2.971 / 3.096 A |

Clone phase separations are approximately 119–121 degrees. This supports
balanced three-phase excitation comparable to the reference, not smooth
instantaneous winding current: asynchronous ADC samples include roughly
10 A pulse/freewheel excursions. These must not be interpreted as PSU average
current. Run 03 plot inspected: comparable command modulation, stable bus,
and orderly initial coast followed by low-speed comparator noise.

### Early stop and final safe state

A separate fixed 100 Hz / 7% test requested terminal `off` after 0.5 host
seconds. It stopped with reason 3 at reported 552,759 us (host serial polling
adds latency), then retained both buffers and completed the dump. No sampled
fault. Its coast did NOT pass motion validation (C had no accepted early
edges, phase order score 0.286); this is only a stop-path test, not an accepted
startup or speed result. No destructive fault injection was performed.

After this test and the disabled-driver PWM diagnostic, direct serial `p`/`i`
reported all six gate outputs AND input readbacks zero, ENABLE=0, MOE=0,
CCR A/B/C=0/0/0, mode=0, coast=0, nFAULT=1. COM41 was closed/released.

Artifacts: `captures/shell_pwm_validation_01`, `_02`, `_03`, and
`captures/shell_pwm_early_off`, each with `.txt`, `.csv`, `.png`, `_coast.csv`.
Offline replay of run 03 (`shell_pwm_replay_check`) reproduced all summary
metrics. Release build passes with inherited unsafe-operation warnings;
image size is 24,752 bytes text and 21,104 bytes BSS (stack additional).
Terminal/build instructions: `SHELL_PWM.md` in this directory.

Conclusion: dedicated hardware-PWM clone flashed and repeated successfully,
approximately 50 eHz supported by coast feedback, with normal and early stop
paths exercised and final physical MCU output levels safe. Absolute clock
calibration, oscilloscope dead-time validation, and closed-loop speed control
remain outside this campaign.

---

## Entry 013 — 2026-09-12 — Quiet/Ascii85 instrumentation qualified

New user-authorized campaign: confirm phase identity and spinning BEMF, then
explore toward 25–30% only with tracking/current evidence. No duty-ceiling
increase yet; this entry qualifies the instrumentation prerequisite only.
Same connected motor, DRV/G071 wiring and operator-set 800 mA PSU limit.

`shell-pwm` now defaults to a short TIMING/DONE summary. `cap1` arms one
completed-attempt dump and resets to quiet afterward; `cap0` cancels.
The Python fixture requires the a85-v1 acknowledgement before starting and
requests off/cap0 in cleanup. Buffered capture/guards remain active in quiet
mode. Ascii85 encoding and CRC computation occur only after disable.

The format uses explicit little-endian fields, per-record CRC32/ISO-HDLC,
length/count/order checks, and measured coast timestamps; no struct-padding
serialization. Ascii85 was selected from minz's GECKO/WAXWING host decoders.
Existing raw hex and CTIME captures remain replayable. Headers are not CRC'd.
Five Python tests pass (handshake, missing handshake/no motion, real-capture
roundtrip, corruption/truncation/version errors, Rust-produced encoding
vector); two standalone Rust encoder tests pass. Release build passes with
the inherited unsafe-operation warnings. Text=27,640 B, BSS=21,104 B.

After `off` and a safe-state readback, downloaded/reset the new image.
`pwmcheck` returned 101 edges/10000 us, PSC=0 ARR=6399 CCMR1=6868
CCMR2=68 CCER=555 BDTR=c1a, ENABLE=0. All six MCU input readbacks low.

Full fixture baseline: `captures/bemf_a85_baseline_01.{txt,csv,png}` and
`_coast.csv`, 500 drive + 500 coast records with valid CRCs, reason=1,
energized_us=4,999,070. VBUS=11.36–11.93 V; no nFAULT-low sample. Initial
coast estimate=47.51 eHz; disable-extrapolated fit=51.65 eHz, R2=0.99939,
phase order score=1.000, C-minus-neutral span=13.7 mV. This reproduces the
accepted rotation baseline, but does not prove labeled phase identity.

Raw transaction size=40,680 B versus 62,795 B for the prior 500+500 hex
run (about 35% smaller, including CRC overhead). A separate unarmed run
emitted only 333 B including command echoes and summaries, no D85/C85/CAP
rows, reason=1, energized_us=4,999,124. Afterwards p/i confirmed all gates,
MOE, CCRs and ENABLE zero; mode/coast=0, nFAULT=1. Port released.

Pre-flash ENABLE-low current ADC values were far below midscale. These are
sleep-state observations, not credible motor current; enabled zero-drive
offsets must be established for phase-current tests. No new wiring fault
is inferred. Next: unplug the motor connector for uncoupled phase-voltage
identity tests; then resolve INH/INL pairing and ISEN association before
interpreting six-step BEMF. No six-step or elevated-duty qualification yet.

---

## Entry 014 — 2026-09-12 — Enabled zero-drive current baselines

While awaiting motor-unplug confirmation, added and flashed `sensezero`.
It calls gates_off (MOE and CCRs zero), wakes ENABLE for 2 ms, acquires up to
32 seven-channel ADC scans with a 10 ms foreground bound, disables ENABLE,
then prints min/mean/max. No gate activation and no motor-running command.
ADC conversion polling remains inherited/blocking; the time bound is not an
independent watchdog. Wake-period nFAULT and scan-period nFAULT are separate.

Two final-image repeats: awake_us=4860, n=32, wake_fault=1, scan_fault=0.
Current-channel means (raw 12-bit ADC) were:

| Repeat | PA0 / nominal IA | PA1 / nominal IB | PA4 / nominal IC |
|---|---|---|---|
| 1 | 2053 | 2055 | 2054 |
| 2 | 2052 | 2048 | 2054 |

All are close to midscale=2048, unlike sleep-state readings. This supports
enabled zero-drive baselines, not phase identities or absolute gain accuracy.
The largest within-scan current-channel spread was 37 raw counts; a single
sample must not be treated as precise zero or as a small-current measurement.
VSENC/neutral means were 1014/1016 in both repeats; these are common-mode
voltage with no drive, not spinning BEMF. nFAULT was observed low during the
2 ms wake interval but never during the subsequent scans. This localizes the
earlier fault_seen=1 diagnostic; it does not establish a datasheet diagnosis.

Artifact: `captures/bemf_enabled_zero_01.txt`, including final p/i readback:
all six MCU gate levels=0, MOE=0, CCRs=0, ENABLE=0, mode/coast=0, nFAULT=1.
COM41 closed. Firmware compiles with existing warnings. No motor phase
identity has yet been proven and the duty ceiling remains unchanged.

---

## Entry 015 — 2026-09-12 — Unloaded gate-to-voltage phase identity

Operator explicitly confirmed motor connector unplugged. PSU left on at the
previous 800 mA limit. Added `map0`..`map5` to shell-pwm: single-input unloaded
diagnostics for AH/BH/CH/AL/BL/CL, respectively. Wake driver with gates off,
sample baseline, assert only the selected MCU gate input, sample, clear all
gates, sample recovery, disable ENABLE, restore normal TIM1 configuration,
then print. Three records include all seven ADC channels, all comparator
bits, physical six-pin readback, and elapsed time. This is an unloaded static
pulse, NOT a higher-duty motor waveform. Do not run with motor connected.

First low-input tests failed the MCU readback prerequisite (all pins stayed
low). Corrected diagnostic CCER: complementary force-inactive requires the
main channel enabled too on this setup; its main pin remains forced LOW.
No low-wire inference is drawn from the invalid initial tests. Artifact:
`captures/bemf_phase_map_initial.txt`. This repeats the known raw-TIM1
complementary-channel setup lesson; not evidence of damaged low sides.

Final image, two repeats per input, all pulse_us=810 and fault=0:

| Input / MCU pin | Only active pin mask | Comparator A/B/C during pulse | Identified feedback |
|---|---|---|---|
| AH / PA10 | 1 | 0/1/1 | A / PB3 |
| BH / PA9 | 2 | 1/0/1 | B / PB7 |
| CH / PA8 | 4 | 1/1/0 | C / PA2 |
| AL / PB1 | 8 | 1/0/0 | A / PB3 |
| BL / PB0 | 16 | 0/1/0 | B / PB7 |
| CL / PA7 | 32 | 0/0/1 | C / PA2 |

Baseline unloaded phases were biased high; low assertions measurably pulled
the selected phase below neutral, unlike passive before/after states. Phase-C
ADC independently cross-checks polarity: CH gives VC-neutral=+121/+113 raw
counts; CL gives -581/-591. For AH/BH the C channel remains below neutral;
for AL/BL it remains above neutral. Thus comparator high means selected phase
below neutral, consistent across these tests. Current channels remain near
midscale as expected without winding load; no ISEN identity verdict yet.

Artifact: `captures/bemf_phase_map_01.txt`. Replay verifier:
`python scripts/drv_phase_map.py --infile captures/bemf_phase_map_01.txt`.
It requires complete stages, <2 ms pulse, no fault, exact one-pin gate
readback, unique comparator response, C ADC polarity agreement, at least two
consistent repeats/input, and bijective high and low mappings. Result:
paired=True, nominal_order=True. These are relative command-to-feedback
identities, not an independent physical silkscreen anchor: a common global
permutation of gate and feedback labels remains observationally equivalent.

Final p/i: all six gates low, MOE=0, CCRs=0, ENABLE=0, mode/coast=0,
nFAULT=1. COM41 released. Original shell-sine hash unchanged. Next requires
motor reconnection to identify ISEN using actual controlled winding current,
then qualified six-step BEMF observation. No increased-duty test performed.

---

## Entry 016 — 2026-09-12 — ISEN identity: logical A/C reversed

Operator confirmed motor reconnected. Added `pair0`..`pair5`, six directed
phase pairs using the qualified gate/VSEN identities from Entry 015. Source
uses 6% complementary PWM at 10 kHz, sink forced low, third channel disabled.
Sixteen ADC scans begin 25–35 us into each carrier period (OFF-window current
recirculation), with rotating ADC order. Zero offsets measured awake before
each burst. Hardcoded diagnostic bounds: 1.9 ms foreground cutoff, nFAULT,
800-count current-deviation guard, bus 70% baseline guard, initial bus >=800
counts. These are not a proven average-current limiter; PSU stays at 800 mA.

Each of 12 bursts (two per directed pair) finished in 1592 us, n=16, reason=0.
The two active channels had opposite mean deviations of 225–270 raw counts;
inactive channel means stayed within 3 counts of zero. Direction reversal
reversed the signs. Representative first repeats, physical ADC order:

| Drive | ADC0 delta | ADC1 delta | ADC4 delta |
|---|---:|---:|---:|
| A>B | +1 | -245 | +244 |
| B>A | -1 | +247 | -243 |
| A>C | -233 | -3 | +225 |
| C>A | +270 | 0 | -257 |
| B>C | -239 | +231 | -2 |
| C>B | +263 | -253 | 0 |

**Measured relative mapping: phase A current = PA4/ADC4, B = PA1/ADC1,
C = PA0/ADC0.** The old firmware IA/IC labels were reversed relative to
the gate/VSEN labels. No physical wire move performed. This identifies an
electrical association, not which physical cable or common phase-label
permutation caused it. Gain accuracy is not calibrated by this test.

Raw artifact: `captures/bemf_current_identity_01.txt`, including final safe
readback. Verifier `scripts/drv_current_identity.py` enumerates all six ADC
permutations and requires complete repeated probes, no abort, opposite-sign
means >100 counts, inactive mean <10% of the weaker active mean. Exactly one
mapping satisfies all 12 observations: (4,1,0). Those numerical classifier
thresholds were chosen after examining this dataset; independent qualification
is still needed before treating them as universal automated acceptance limits.

Corrected shell-pwm's logical capture and `a` channel reads through
CURRENT_ADC=[4,1,0]. Dumps now include `IMAP ia=4 ib=1 ic=0`. Raw diagnostics
still identify physical ADC channels. Original shell-sine is untouched; prior
captures/analyses retain old IA/IC labels. Historical balanced-waveform results
still show activity but their labeled phase interpretation must be corrected.

Flashed corrected firmware and repeated accepted startup/50 Hz hold:
`captures/bemf_corrected_imap_01.{txt,csv,png}` and `_coast.csv`. 500 drive +
500 coast records; reason=1; energized_us=4,999,495; no sampled fault;
VBUS=10.93–12.44 V. Early coast estimate=49.84 eHz; disable-fit=52.90 eHz,
R2=0.99843, phase-order score=1.000. This verifies the known spinning baseline
after the channel-label correction, not six-step BEMF qualification.

Final artifact `captures/bemf_corrected_imap_safe.txt`: physical MCU gates all
low, MOE=0, CCRs=0, ENABLE=0, mode/coast=0, nFAULT=1. COM41 released. Build
and five host protocol tests pass; original reference hash unchanged. The
relative drive/voltage/current map is now established. Next: six-step BEMF
observation, sampling/blanking validation and feedback-qualified progression;
motor remains connected. No exploration above 10% has been performed.

---

## Entry 017 — 2026-09-12 — Six-step pin-state preflight, driver disabled

Added `examples/support/sixstep.rs`, a host-testable TIM1 register planner
following rm32_stm32/src/phase.rs's 1-based convention:
A>B, C>B, C>A, B>A, B>C, A>C. Comparator selection follows rm32's
comparator.rs/G071Comp input setup: floating phases C,A,B,C,A,B. Source
uses complementary PWM, sink forced low, floating channel both CCER enables
clear. Duty clamps at 10%. `sixstep_apply` disables MOE before changes,
selects COMP2, then reenables MOE; caller owns ENABLE and runtime guards.
No control authority is granted to BEMF yet.

Two standalone Rust tests pass: all six floating phases, distinct source/sink,
floating enables clear, driven enables correct, CCR=384 at 6%, clamp at 10%
including u32::MAX input, invalid step rejection. Release build passes with
inherited warnings.

Flashed and ran `sixcheck` twice with ENABLE low throughout. Each sector
observed six MCU input levels for 1 ms at 6% PWM, 703–704 polls. OR/AND masks
(AH/BH/CH/AL/BL/CL = bits0..5) exactly matched:

| Step | OR: ever high | AND: always high |
|---|---:|---:|
| 1 | 25 | 16 |
| 2 | 52 | 16 |
| 3 | 44 | 8 |
| 4 | 26 | 8 |
| 5 | 50 | 32 |
| 6 | 41 | 32 |

Each source main/complementary pin toggled, sink low stayed high, and both
floating inputs stayed low at all observed instants. This is MCU-pin preflight
only, not scope dead-time proof or energized floating-phase/BEMF qualification.
It catches the complementary-enable mistake seen in Entry 015 before drive.

Artifact: `captures/bemf_sixcheck_01.txt`, including final p/i: all gates=0,
MOE=0, CCRs=0, ENABLE=0, nFAULT=1. Port closed; motor remains connected.
No motor-running commands this entry. Next: bounded spinning six-step
observation with timestamped PWM-window samples and explicit rejection
reasons, before detections control commutation.

---

## Entry 018 — 2026-09-12 — First spinning six-step observation, NOT qualified

Implemented `examples/support/observation.rs`: after 4.7 s of the accepted
sine profile, select the nearest source/sink pair from commanded sine angle
and apply rm32's six-step order at fixed 50 eHz / 6% for at most 96 ms.
Comparator has no authority over commutation. Motor remains connected,
operator PSU limit 800 mA. Total energized time stays below 5 s. Additional
observation aborts: nFAULT, any received UART byte, current deviation >1200
raw counts from midscale, bus below 70% of observation-entry sample. This is
not qualified tracking protection; no increase above 10% is permitted yet.

`obs1` arms one observation; fixture `--observe` requires its acknowledgement.
Firmware and host restrict this mode to run50 / du60. `obs0` cancels.
Encoding happens after disabled coast capture. A 192x16-u16 static buffer
adds 6144 B; measured total BSS=27248 B, leaving about 9.6 KiB for stack, not
a measured stack high-water guarantee. Current assignments remain ADC4/1/0.

Records include timestamp, sector age, step/floating phase, TIM1 count,
ON/OFF comparator states, VC/neutral, currents/bus/VREF and rejection flags.
Two host decoder tests pass (CRC/length/count/framing and known sample).
The existing five capture/replay tests also pass. No claim that switching
levels or one threshold transition alone proves a BEMF zero-cross.

### Attempt 01: timing limitation exposed

`captures/bemf_six_observe_01.{txt,csv,png}`, `_coast.csv`, `_obs.csv`.
190 observation records; energized_us=4,796,010; reason=1. Full seven-channel
scan starting 25–35 us into PWM crossed the 100 us carrier boundary on every
record. Wire a85-v1 therefore rejects all records as fully synchronized ADC
snapshots. Digital comparator samples precede that scan but must be treated
separately. Coast fit=58.34 eHz, R2=.99951, phase order=1.000; motion valid
but outside the baseline 45–55 eHz band.

### Attempt 02: BEMF ADC-pair timing separated from slow telemetry

Wire a85-v2 changes `cnt_end` to the count immediately after VC and neutral
conversions. Reject/status bits: 1 = within 200 us commutation/mux blank;
2 = no captured ON comparator within previous 50 us; 4 = slow telemetry scan
crossed PWM wrap (not itself invalidating earlier BEMF samples); 8 = VC/neutral
pair crossed wrap. Host keeps v1 semantics for historical files.

`captures/bemf_six_observe_02.{txt,csv,png}`, `_coast.csv`, `_obs.csv`, `_obs.png`.
190 records over 95,710 us; total energized_us=4,796,536; reason=1. BEMF ADC
pair ended at TIM1 counts 3200–3606 (50–56.3 us): zero pair-wrap rejections.
10 records within blanking, 16 missing recent ON capture; all 190 slow scans
still cross wrap. Raw current/bus telemetry is consequently NOT all acquired
at the same PWM position. Observation bus raw=1191–1261, max current deviation
239 counts; no current/bus/nFAULT abort occurred. Printed main capture ranges
refer to the preceding sine hold, not this observation buffer.

Each sector has 24–30 records passing combined timing checks and both digital
levels occur across the overlaid repetitions. Plot inspected: OFF comparator
is predominantly low in sectors 1/3/5 and high in 2/4/6, with ON/OFF differences
and inter-revolution variability. This is NOT yet a repeatable crossing at the
expected sector position. Phase-C floating analog signal is small: step1
VC-neutral range -2..16 raw counts, step4 -5..0 in timing-valid samples.
Noise/offset and sector phase alignment remain material concerns.

Coast after attempt 02 gives fit=42.62 eHz, R2=.99497, order=.857; it fails
the existing motion-valid and 45–55 eHz gates. Do not describe these two
attempts as repeatable synchronized six-step operation. Initial handoff picks
a sector but starts a full sector dwell regardless of the sine angle's
fractional position; handoff phase and actual rotor tracking require study.
No further duty increase is justified by these data.

Final p/i saved in `captures/bemf_six_observe_safe.txt`: all gates/CCRs/MOE/
ENABLE zero, nFAULT=1, mode/coast=0. Port closed. Remaining work: establish
phase-aligned repeatable handoff, distinguish PWM artifacts from true BEMF
ramps, validate blanking/mux/window choices, then reuse minz's detector
decisions and tracking guards before expanding the operating range.

---

## Entry 019 — 2026-09-12 — Phase-continuous handoff tested; BEMF still unqualified

Removed the arbitrary full-sector first dwell. Main now remembers the last
actually applied sine angle (not the next tick's angle), extrapolates it by
the measured elapsed time since that update, and passes it into observation.
The observer advances that same 32-bit electrical phase from TIM17 elapsed
time; a compile-time 256-entry lookup selects rm32 source/sink sectors from
the original sine extrema. Sector boundaries thus follow the ongoing command
phase rather than restarting a dwell timer. This aligns command phase, NOT
measured rotor phase. Lookup resolution and foreground ADC blocking still
quantize/delay actual commutation. A third host Rust test verifies extrema
selection against every rm32 sector. Build passes with inherited warnings.

Two unchanged-envelope tests: 4.7 s proven sine, <=96 ms observation,
50 eHz/6%, PSU limit unchanged. Artifacts:
`captures/bemf_phase_aligned_01` and `_02`, each `.txt`, `.csv`, `_coast.csv`,
`.png`, `_obs.csv`, `_obs.png`. Both ended normally, energized_us=4,796,356
and 4,796,454, 189 observation records, no guard abort. BEMF ADC pairs again
stayed within the OFF window; 52/48 records lacked a recent ON read, 7/10
were inside commutation blanking. Combined telemetry scans still cross wrap.

Coast disable fits: 41.78 eHz (R2=.99622, order=.875) and 45.43 eHz
(R2=.99757, order=.750). Motion-valid classifier passes both, but only the
second is in the 45–55 band, and neither shows the earlier clean order score
of 1.000. No repeatable six-step tracking/zero-cross qualification is claimed.
In the first test, timing-valid OFF comparator levels never changed inside
sectors 1/3/4/5/6; ON levels are variable within and across revolutions.
Thus fixing handoff timing did not by itself establish the desired sensing.

Inspected cached schematic rendering `block_04_Half-bridges_and_Sense.png`:
phase dividers shown as 82k/7.5k; the 0.1 uF phase-sense capacitors C2/C3/C4
are marked DNP. Do not blame an assumed fitted 0.1 uF filter without physical
evidence. Current-sense filtering is separately shown as 56 ohm / 2200 pF.
Current ADC sampling is configured with SMPR=7; future ON-window analog
experiments must explicitly measure acquisition timing and settling instead
of assuming this long-sample scan can fit a 6 us pulse.

Next discriminating work: qualify an ON-window C/neutral analog measurement
and compare it with the comparator at measured timestamps, separate offset/
ground-clamp effects from true BEMF, and assess commutation phase against that
evidence. No duty increase or BEMF-controlled commutation justified yet.
Final p/i in `captures/bemf_phase_aligned_safe.txt`: all gates/CCRs/MOE/
ENABLE zero, mode/coast=0, nFAULT=1. Port released; motor remains connected.

---

## Entry 020 — 2026-09-12 — ADC pair timing for ON-window sensing

Added `adc_bemf_pair`: fixed regular sequence ADC2 then ADC3, one channel-mask
selection/start, two EOC reads. This avoids a second channel-selection
handshake between VC and neutral. It has 100 us polling timeouts and overrun
checks invoking board-safe panic. No claim that these are simultaneous samples.
Added `adctiming`, always MOE/ENABLE off: 32 pairs per SMPR selector, measure
TIM1 count delta at 64 MHz, restore SMPR=7. Two live repeats after flash:

| SMPR selector | Min=max ticks, both repeats | Pair duration |
|---|---:|---:|
| 0 | 280 | 4.375 us |
| 1 | 280 | 4.375 us |
| 2 | 309 | 4.828125 us |
| 3 | 361 | 5.640625 us |
| 4 | 415 | 6.484375 us |
| 5 | 573 | 8.953125 us |
| 6 | 897 | 14.015625 us |
| 7 (existing long sample) | 1539 | 24.046875 us |

These are measured end-to-end call durations including channel selection,
conversion and read overhead, not only ADC acquisition durations. The full
setting-2 pair is close to a 6 us ON pulse, leaving insufficient comfortable
margin for arbitrary trigger position/gate transitions. Next: prepare the
channels before the ON window and time conversion/read in-window, then test
short-sample settling accuracy before trusting the analog BEMF comparison.
Do not infer accuracy merely because conversion is fast. No energized run
or duty change in this entry; spinning observer still uses its prior sampling.

Artifact: `captures/bemf_adc_pair_timing_01.txt`, including final p/i showing
all gates, CCRs, MOE and ENABLE zero, nFAULT=1, mode/coast=0. Port released;
motor remains connected. Release build passes with inherited unsafe warnings.

---

## Entry 021 — 2026-09-12 — Prepared ON-window pair: timing passes, signal not qualified

Same G071/DRV8304H measured map, connected motor, approximately 12 V supply,
800 mA PSU limit. Original shell-sine unchanged; shell-pwm observer remains
50 electrical Hz / 6%, no ceiling increase. ADC sequence preparation moved
outside the measured conversion window. New disabled-driver `adcwindow`
diagnostic checks 128 pairs per setting and source-input level before/after.

Initial trigger counts 64..96 failed: SMPR2 ended at 356..385, 12/128 misses;
SMPR3 ended at 377..401, 103/128 misses (ON ends at count 384).
Artifact: captures/bemf_adc_window_initial.txt. Earlier trigger 32..64 passed
twice: SMPR2 start43..72/end324..353; SMPR3 start42..63/end345..366;
zero misses at both settings. ENABLE remained low throughout preflight.
Artifact: captures/bemf_adc_window_01.txt. Chose SMPR2 for more timing margin,
not on an assumption of proven analog acquisition accuracy.

Spinning artifact: captures/bemf_on_pair_01.txt, standard CSV/coast/plots and
_obs.csv/_obs.png. OBS a85-v3 records ON-window ADC2 then ADC3, early ON
comparator, later OFF comparator, then slow current/bus/VREF telemetry.
Timestamps now refer to pair completion. See SHELL_PWM.md for version semantics.
Build and replay checks pass; host accepts historical v1/v2 separately.

Measured energized time 4,796,048 us, normal completion, no fault sample.
162 observations over 95,765 us; all pairs ended at counts 305..337, before
384, with zero ON-window or wrap rejections. Seven rows were inside the
200 us commutation blank. Coast disable extrapolation 46.73 electrical Hz,
fit R2=.99573, phase-order score .929, motion classifier passes. This supports
continued rotation, not proof of sector-by-sector tracking during drive.

Important unresolved discrepancy: floating-C valid rows in step1 have C-N
-28..147 ADC counts, step4 -76..226. Step4's early ON comparator stays high
(C below neutral), while most later ADC pairs show positive C-N. Step1 also
has many such disagreements. The comparator is sampled roughly four us
before pair completion, and VC/neutral are sequential short acquisitions.
Thus timing-fit is proven but settling/analog accuracy and a repeatable
six-sector BEMF crossing sequence are NOT. No wiring fault inferred from this.
Next discriminating measurement is comparator near pair completion alongside
the early comparator, followed by acquisition/settling cross-checks. Do not
feed these unqualified samples into commutation or increase duty.

The standard capture current/bus summaries describe the preceding sine hold,
not the short six-step interval; PWM current peaks are not PSU average current.
Final captures/bemf_on_pair_01_safe.txt verifies all six gates, CCRs, MOE and
ENABLE zero; nFAULT=1, idle, port released. Motor remains connected.

---

## Entry 022 — 2026-09-12 — Late comparator does not resolve analog discrepancy

Added OBS a85-v4 flags bit3: comparator read just after ADC conversion; cnt_end
now brackets this extra read too. Early comparator and OFF comparator remain
unchanged. Host retains v1/v2/v3 decoding and adds an explicit floating-C sign
comparison, excluding reject bits 1/2/8 and absolute C-N <=10 raw counts.
This deadband is an analysis choice, not an accuracy calibration.

Same connected G071/DRV8304H, measured phase/current map, approximately 12 V,
800 mA PSU limit. Two unchanged-envelope runs: 4.7 s sine then <=96 ms of
50 electrical Hz / 6% six-step. No higher duty or BEMF control enabled.
Artifacts: captures/bemf_late_on_01 and _02 (.txt, .csv, _coast.csv, .png,
_obs.csv, _obs.png). Energized durations 4,796,028 and 4,796,023 us.
Both normal completion, no fault samples, 162 observation rows each.

Both runs: zero ON-window/wrap rejections, three commutation-blank rows.
End counts319..345 and320..345 remain below384. Floating-C usable comparisons:

| Run | Usable | Early sign matches | Late sign matches | Early/late changes |
|---|---:|---:|---:|---:|
| 01 | 51 | 2 | 5 | 7 |
| 02 | 52 | 6 | 5 | 9 |

Thus the discrepancy is reproducible and simply moving the comparator read
to after the pair does not reconcile it. This does NOT isolate the fault to
wiring or establish ADC accuracy. Sequential acquisition across a switching
pulse, short-sample settling, neutral input/reference and comparator setup
still need discriminating checks. Next: qualify ADC/neutral comparison under
stable inputs and check acquisition order/timing before interpreting crossings.

Coast disable fits46.27/42.84 electrical Hz, R2=.99278/.99691, order score1.0
both; both motion-valid but only first inside45..55. Therefore rotor tracking
is not repeatably qualified either. No all-sector BEMF result claimed.
Release build passed (existing warnings); nine host replay tests passed.
shell-sine SHA256 remains B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.
Final captures/bemf_late_on_safe.txt verifies all gates/CCRs/MOE/ENABLE zero,
nFAULT1, idle. Serial released; motor connected. Campaign remains incomplete.

---

## Entry 023 — 2026-09-12 — Disabled-driver ADC acquisition dependence confirmed

No energized tests in this entry. Connected motor, PSU on at existing setting,
all gate outputs/MOE/ENABLE disabled. Added adcverify and adcsettle idle
diagnostics; they restore SMPR7. Release builds pass with inherited warnings.
ADC pair preparation now accepts a mask; spinning ADC2/3 behavior is unchanged.

Two adcverify repeats: CFGR1=0, CFGR2=0x80000000, COMP config low bits0x281
(output bit differs). PAC register layout confirms no polarity inversion or
blanking bits in that comparator value; this is not independent validation
of the physical input path. Bus/reference means agree within four counts
between separate and paired reads, about1213..1217/1505..1506 counts.
No evidence of reversed regular sequence on these distinguishable inputs.

Neutral differs markedly by acquisition: short separate mean60, pair57;
long separate3..4, pair3. C is also sample-time-sensitive but less so:
short separate9..11/pair12 versus long2..3/pair3. Short-sample analog accuracy
is therefore demonstrably unqualified, even without power-stage switching.
Artifact: captures/bemf_adc_verify_01.txt, including final safe p/i.

adcsettle repeats32 trials for each conditioning channel. Long ADC2 (near
ground) followed by eight SMPR2 ADC3 reads gives neutral first64..65,
second132..133, eighth164..165; following SMPR7 neutral gives4..5.
Long ADC13 (internal reference) conditioning gives first97..98, second133,
eighth165, then long4..5. Two runs repeat closely. Thus preceding channel
affects the first short result, and repeated short conversions do NOT converge
to the long result. Do not assume discarding one sample fixes this.

This identifies a concrete measurement limitation, not its complete circuit
cause or an absolute calibration. Source impedance, acquisition charge effects,
and ADC configuration need further checks. It does not prove a wiring defect
or fully explain the energized comparator discrepancy. Next discriminate the
sample-duration threshold and validate the input/reference path before trusting
analog ON-window sign. No duty increase or BEMF-controlled commutation justified.

Artifact: captures/bemf_adc_settle_01.txt includes final p/i: all gates/CCRs/
MOE/ENABLE zero, nFAULT1, idle. Serial closed; motor connected. Original
shell-sine untouched. No change to campaign objective or completion status.

---

## Entry 024 — 2026-09-12 — Acquisition sweep and digital-only observation

Added disabled-driver adcsweep: ADC2/3, SMPR0..7, preceding long ADC2 or13,
32 trials each. Two repeats in captures/bemf_adc_sweep_01.txt including safe
p/i. Neutral eighth-read means (both preceding channels/repeats combined):
SMPR0=214..215,1=196..198,2=129..130,3=81..82,4=48..49,5=12..13,
6=5..6,7=4. Interleaved long readings4..6. Setting6 closely matches long
baseline at this idle input, but Entry020 pair duration14.016 us cannot fit
6 us ON pulse. Not an absolute calibration. C is much less affected:
setting3 eighth6..7 versus long3..4. Different code timing shifts short-read
plateaus versus Entry023; exact short counts are not a reusable correction.

New hypothesis to discriminate: acquiring neutral may perturb comparator
reference. OBS v5 omits ADC2/3 altogether in the 96 ms interval; current/bus/
VREF sampling and guards retained. Analog fields65535 and flagbit4 explicitly
mark absence; decoder excludes them from analog analysis. Early comparator
retained; late comparator sampled after count320 instead of after ADC pair.
This changes timing slightly too, so cannot isolate ADC perturbation by itself.

One connected-motor run at existing approximately12 V,800 mA limit: known
4.7 s sine then50 electrical Hz/6% six-step. Artifact bemf_digital_only_01
with raw, CSV/coast, standard and observation plots. Normal completion,
energized4,796,195 us, no fault.162 rows, eight blanking rejects, zero window
rejects; late-read bracket ends341..361 (<384). Coast estimate44.97 electrical
Hz, R2=.99703, order.857, motion-valid but narrowly outside45..55 speed gate.

Inspected observation plot: late comparator has both states in every sector,
but changes are not a clean repeatable all-sector crossing sequence. Step4
shows late values both0/1 across much of the sector while early/OFF remain1;
steps3/6 show sparse late departures. Removing analog sampling has NOT yet
qualified BEMF or tracking, nor established the perturbation hypothesis.
Need per-revolution/timestamp crossing analysis and controlled sampling-window
comparison; avoid mistaking overlaid varying states for valid rotor crossings.

Release build passes with inherited warnings. Host tests cover missing analog
data explicitly. Final captures/bemf_digital_only_safe.txt: all gate outputs,
CCRs, MOE,ENABLE zero; nFAULT1,idle. Serial released, motor connected. Ceiling
unchanged; no unqualified detections control commutation. Goal still active.

---

## Entry 025 — 2026-09-12 — Per-visit transition audit, no hardware execution

Added scripts/drv_crossings.py to separate individual commutation visits, retain
0/1/rejected state strings and bracket observed transitions by actual sector
sample times. First/last visits excluded as possibly truncated. A supported
transition requires two adjacent valid samples before and after; rejections
break adjacency. This is a conservative evidence audit, NOT a new commutation
filter, a claim of true zero crossings, or a replacement for minz logic.
Four tests cover supported bracket, spike, rejected gap and sector boundary;
14 host tests pass in total. Inspected minz am32::bemf_count_step: its level
counter alone cannot establish a crossing when a sector begins already at
the expected level. No control changes made or goal requirements relaxed.

Replayed bemf_late_on_01, _02, and bemf_digital_only_01; respective
_crossings.csv artifacts retain every visit. Each has28 complete visits:

| Capture | Late ON no transition | Late ON multiple | Exactly one supported |
|---|---:|---:|---:|
| late_on_01 | 13 | 2 | 3 (steps3,4 only) |
| late_on_02 | 13 | 2 | 3 (steps3,4,5 only) |
| digital_only_01 | 9 | 3 | 6 (steps2,5,6 only) |

Early ON has27/25/26 no-transition visits; OFF22/20/25. Thus both levels
appearing in each overlaid step plot does not establish a repeated crossing
in each visit. Sparse ~600 us observations also limit detection support;
this audit is evidence of insufficient qualification, not proof BEMF is absent.

Next discriminating capture: comparator states across several measured PWM
positions, retaining actual timing or explicit miss flags, with ADC2/3 omitted.
Keep existing6%/50 electrical Hz,800 mA and<5 s limits. Compare transition
timing across visits before attributing it to rotor motion. Do not increase
duty, feed these detections into control, or request speculative rewiring.
Hardware state unchanged from Entry024's verified disabled state; no live
serial connection opened this entry. Full campaign remains incomplete.

---

## Entry 026 — 2026-09-12 — PWM-position trace exposes window dependence

OBS v6 retains 16-word records, repurposes absent VC/neutral words as digital
levels/miss masks over12 slots (SHELL_PWM.md defines exact counts). Each read
must fit target..target+80 counts, else absent in host analysis. No ADC2/3
sampling during observer. Timestamp is early read, not sweep completion.
Current/bus/VREF guard scan follows the trace, introducing additional foreground
latency; no real-time-performance qualification claimed.

Two same-envelope connected-motor attempts: approximately12 V,800 mA limit,
4.7 s sine then50 electrical Hz/6% six-step. Energized4,796,190/4,796,208 us,
normal completion, no fault samples,162 rows each. Raw artifacts
captures/bemf_pwm_slots_01/02 plus CSV/coast/plots/_obs and _crossings.csv.

First trace: close slots overran. Miss counts128=0,224=143,320=162,448=162,
640=8; all later slots zero misses. Legacy ON timing rejected all162 rows.
Second: deliberately omitted224/448 (mask retained); misses128=27,320=33,
all slots640 and later zero. Slot320 end384..406: STILL not ON-qualified,
legacy bit2 rejects all rows. Valid slot320 brackets may straddle falling
edge; do not label them clean ON data. These failed timing checks are retained.

Per-slot analyzer now uses individual miss flags and commutation blanking;
legacy late-ON invalidity does not invalidate separate correctly timed OFF
slots. Tests cover this and v6 mask/analog absence semantics.
Among28 complete visits, slot640 (10..11.25 us after PWM start) has7/9 visits
with exactly one supported transition; second includes at least one in every
step but is far from repeatable coverage. At960 (15..16.25 us), only2/2;
at1600 (25..26.25 us),0/1. Slots4800/5600 (75/87.5 us) have no transitions
in all28 visits in both runs. This is strong sampling-window dependence,
not proof of true BEMF crossings or a wiring problem.

Coast disable estimates43.93/41.93 electrical Hz, R2=.99346/.99652,
order.867/.923; motion-valid but both outside45..55. Neither sensing nor
tracking gate passes. PWM pulse width/settling and open-loop handoff/load
remain candidate limitations. No duty increase or BEMF control permitted.
Further work must improve valid active-vector observation and assess tracking,
not merely count occasional crossings as success.

Final captures/bemf_pwm_slots_safe.txt verifies all six gate outputs/CCRs/MOE/
ENABLE zero,nFAULT1,idle; serial released,motor connected. Builds passed with
inherited warnings. Original shell-sine unchanged; full goal remains active.

---

## Entry 027 — 2026-09-12 — Active-vector timing finally qualified in full trace

First moved 32-bit clock bookkeeping out of the ON interval: retain raw TIM17
early timestamp, reconstruct after trace. This alone did NOT fix the slot
loop overhead. bemf_pwm_slots_03 has162/162 ON timing rejects (end392..414),
97 slot320 misses. Coast41.99 electrical Hz, order.667, motion classifier
fails. Normal completion4,796,275 us, no fault samples. Retained raw artifacts.

Then inlined two explicit early reads (counts128 and320), deferring masks and
loop work until afterward. Added obstiming: identical observer execution but
ENABLE held low, returns concise timing summary. Two repeats162 rows each,
late bounds355..381, zero ON rejects; zero requested-slot misses (224/448
intentionally absent). Artifact captures/bemf_obstiming_01.txt includes safe p/i.
This avoids using energized runs merely to debug code scheduling.

Energized confirmation bemf_pwm_slots_04: same connected motor, approximately
12 V/800 mA PSU limit,4.7 s sine then50 electrical Hz/6% six-step. Normal
completion4,796,416 us, no fault samples.162 rows, late end351..377 (<384),
zero ON rejects, zero requested-slot misses, eight commutation-blank rows.
Thus the full digital trace now has a genuinely timing-valid late ON read.
Wire v6 still marks omitted224/448 and absent analog data explicitly.

Coast45.80 electrical Hz fitR2=.99650, order1.000, motion classifier and
45..55 speed gate pass this run. However, among28 complete visits, late ON
has9 single supported transitions: step counts1,2,0,2,2,2. Nine visits have
no transition and two multiple transitions. This does NOT qualify all-sector
BEMF tracking. Early count128 has19 multiple-transition visits and no single
supported transition; slot640 has5 supported, slot960 only1, late OFF none.
Next work can focus on higher-density late-ON evidence and per-visit phase
alignment rather than continuing to adjust an unverified timestamp window.
Any density change must preserve or separately requalify current/bus guard
latency and measured sampling bounds. No duty increase or closed-loop use.

Artifacts bemf_pwm_slots_03/04: raw, CSV/coast/plots, _obs, _crossings.csv.
Final captures/bemf_pwm_slots_04_safe.txt verifies gates/CCRs/MOE/ENABLE zero,
nFAULT1,idle; UART released,motor connected. Builds passed with inherited
warnings. Original shell-sine untouched; campaign still incomplete.

---

## Entry 028 — 2026-09-12 — Dense late-ON captures show all steps, not all visits

Raised observer capacity192->384 and reduced minimum sample interval500->100 us.
Actual interval remains trace/guard-limited, around200 us. Each sample retains
all previous current/bus/VREF guard reads; no guard decimation. Reduced preceding
sine snapshot500->256 records to fund RAM, coast500 unchanged. Static RAM
27,248->27,536 bytes (+288), text50,032 bytes. Remaining9,328 bytes of36 KiB
is not a measured stack high-water. Original shell-sine untouched.

Disabled preflight twice:384 rows,78,533 us, late357..379, zero ON rejects and
zero requested-slot misses;224/448 explicitly omitted as before. Safe p/i in
captures/bemf_dense_timing.txt. Build and16 host tests pass.

Two connected-motor attempts, existing approximately12 V/800 mA limit,
known4.7 s sine followed by50 electrical Hz/6% dense observer. Normal endings
4,778,831/4,778,836 us, no fault samples. Artifacts captures/bemf_dense_01/02
with raw/CSV/coast/plots/_obs/_crossings.csv. Each384 rows over78,450/78,451 us;
late351..369 (<384), zero ON timing rejection,25 commutation-blank rows,
zero requested-slot misses. Observer ends on capacity before96 ms.

23 complete sector visits each. Exactly one supported late-ON transition in
11/12 visits; per-step counts1,1,1,3,2,3 and1,1,2,3,2,3. Thus each step appears
in both captures, but seven visits have no transition in each and one has
multiple. The remaining edge visits lack two-sample support on both sides.
This is better resolution, NOT repeatable all-visit/all-sector qualification.
Example first capture: step3 crossing bracket2499..2700 us after commutation,
step4 at500..700 us, step5 at1900..2099 us. Phase timing is nonuniform;
next assess direction/phase alignment using established commutation geometry.

Coast disable estimates55.24/56.36 electrical Hz, R2=.99931/.99976, order1.0
and motion-valid both. Both remain outside45..55 gate; do not round into a
pass or infer robust rotor tracking from these estimates alone. Shorter
observation and denser foreground updates also changed the experiment, so
differences versus96 ms capture are not attributable solely to sample density.

Final captures/bemf_dense_safe.txt confirms all gates/CCRs/MOE/ENABLE zero,
nFAULT1,idle; UART released,motor connected. No duty ceiling change or BEMF
control. Analog cross-check and robust all-sector tracking remain incomplete.

---

## Entry 029 — 2026-09-12 — Direction audit agrees with minz; coverage incomplete

No hardware execution. Inspected minz/core/src/drive.rs::edges_for and the
actual minz/src/comp2.rs caller: am32_change_comp_input uses mode3 with sector
step-1. Therefore expected raw comparator transition is rising on odd rm32
steps, falling on even. minz explicitly handles its comparator polarity;
rm32_stm32/src/comparator.rs instead maps its generic rising flag to falling
EXTI. Do NOT blindly transplant the latter selection into this bench observer.
This is a convention conflict to handle in an eventual HAL integration, not
evidence the tested phase wires should be swapped.

Added scripts/drv_phase_audit.py, retaining individual supported crossing
brackets, comparing directions to the inspected minz convention and deriving
same-step period bounds only for adjacent electrical cycles (visit difference6).
No interpolation across missing visits. No analog or rotor-lock qualification
is inferred by the tool; its report explicitly remains unqualified. Three
tests cover direction, opposite edge and period bounds/missing visit.19 host
tests pass. Outputs bemf_dense_01/02_phase_audit.json.

Every supported single transition in both dense captures agrees with expected
direction:11/11 and12/12. All six steps represented in each, but coverage per
step remains only1..3 crossings out of3..4 complete visits. Neighbor-cycle
same-step period bounds (only available for steps4/5/6) correspond roughly to
47.99..52.53 electrical Hz across both captures. These few intervals are
consistent with near50 eHz motion but cannot establish all-sector tracking.
Coast extrapolations55.24/56.36 remain outside the predefined gate; do not
substitute a favorable sparse-period estimate for the failed independent check.

Crossing placement differs substantially by step: step1 around0.5..0.7 ms,
step2 around2.1..2.5 ms, step3 around2.5..2.9 ms, step4 around0.5..1.7 ms,
step5 around1.9..2.5 ms, step6 around2.1..2.9 ms after commutation. Missing
crossings, phase placement, analog acquisition and neutral-reference validity
remain unresolved. No feedback control, timing advance or duty change made.
Next detector work must use established polarity/filter logic in observe-only
mode and retain before/after evidence; a level counter alone is not proof
of a crossing. Hardware remains at Entry028's verified disabled state.

---

## Entry 030 — 2026-09-12 — Shared minz-based observe-only detector replay

No hardware execution or flash. Added examples/support/detector.rs as a pure
allocation-free state machine. It calls actual minz_core::drive::edges_for
and minz_core::am32::bemf_count_step, not copied control science. A bench
evidence wrapper requires two opposite-level readings before arming, then
two expected readings with bad threshold0. Invalid samples reset baseline/
filter support; sector changes reset state; one candidate latch per sector.
No peripheral, scheduler, duty or commutation interface exists. Thresholds
are provisional observation settings, NOT production control qualification.

Standalone tools/observer-replay crate imports the same detector source and
the sibling minz-core dependency. Four host tests pass: expected-level-only
cannot trigger, all six polarities/latch, invalid-gap/sector reset, and spike
handling. Library compile-check for thumbv6m-none-eabi passes. Host Cargo emits
incremental-cache Access Denied notes but compilation/tests finish successfully;
this is not a hardware or functional test failure. README gives exact commands.
Live shell-pwm has not yet imported this module; running image unchanged.

Replay captures/bemf_dense_01_obs.csv and _02 gives12/13 candidates, per-step
1,1,1,4,2,3 and1,1,2,4,2,3. Reasons (invalid,baseline,opposite,accumulating,
candidate,latched) are [25,34,240,15,12,58] and [25,34,236,15,13,61].
Artifact captures/bemf_detector_replay_01.txt preserves output. Confirmation
ages are about0.9..3.1 ms after commutation, NOT original crossing timestamps.

These counts exceed the full-sector single-transition audit by one per run:
online confirmation can precede a later reversal. The distinction is explicit;
do not weaken the all-sector qualification gate to make candidates into proof.
Next integrate decision/rejection flags into the live capture only, verify exact
host/firmware replay agreement and timing overhead with ENABLElow first. Still
no BEMF control or duty increase, and analog qualification remains outstanding.
Hardware stays at Entry028's verified disabled state, motor connected.

---

## Entry 031 — 2026-09-12 — Live minz observer decisions exactly replay

Imported shared detector into shell-pwm; it runs after timed digital reads
and slow ADC telemetry. No candidate is consumed by commutation or duty code.
OBS v7 encodes expected/armed/event/latched in flags5..8 and reason0..5 in
reject4..6. Timing bits0..3 unchanged. Valid input requires timing/blanking
pass, slot320 present and nFAULT high. Host replay asserts every decision
and reason against the same Rust source, not a Python reimplementation.

Two disabled preflights:384 rows, late351..373, zero requested-slot misses,
zero ON rejects,78,586/78,590 us. captures/bemf_decision_timing.txt includes
safe p/i. Builds pass (inherited warnings),20 Python tests and4 detector host
tests pass. Image text50,652/static RAM27,536 bytes. Proper board-local panic
retained; directly using minz-core does not import binz's old-board panic.

One connected-motor run at existing approximately12 V/800 mA limit:
known4.7 s sine then50 electrical Hz/6% observer. Normal end4,778,803 us,
no fault. captures/bemf_decisions_01 plus CSV/coast/plots/_obs/_crossings.
384 observations over78,518 us,25 blanked, late351..373, zero ON rejects and
zero requested-slot misses (224/448 remain explicitly omitted).

All384 live decisions/reasons exactly match host replay.15 candidates,
per step2,1,2,4,3,3; reason counts[25,20,236,17,15,71]. Saved in
captures/bemf_decisions_01_replay.txt. Full-sector audit accepts only13/23
visits; six no-transition and two multiple-transition visits remain. Agreement
verifies implementation, not physical BEMF truth or control suitability.
Coast57.25 electrical Hz,R2=.99951,order1.0,motion-valid; outside45..55 gate.

Next substantive gate is improving/qualifying physical crossings and rotor
phase tracking, including the missing analog cross-check, not adding more
confidence to candidate count alone. No feedback control/duty increase.
Final captures/bemf_decisions_01_safe.txt verifies all gates/CCRs/MOE/ENABLE
zero,nFAULT1,idle. Serial released,motor connected. Goal remains incomplete.

---

## Entry 032 — 2026-09-12 — Single longer-acquisition ADC can fit ON interval

No energized test. Existing connected motor/PSU state, gates/MOE/ENABLE zero.
Added adc_single_convert for a preselected channel,100 us timeout and overrun
check through board-safe panic. Added adcsingle idle diagnostic:128 single
ADC3 conversions for each SMPR4/5/6; record TIM1 start/end and reject outside
26..383. Timer keeps running but gate drive is off, so this measures timing
against the prospective6% pulse, not transistor state or analog accuracy.
Restores SMPR7. Compiled and flashed shell-pwm; spinning observer unchanged.

Two repeats in captures/bemf_adc_single_timing.txt:

| SMPR | Start counts | End counts | Failures |
|---|---|---|---|
| 4 |44..69 both|244..269 both|0/128 both|
| 5 |51..64 /50..63|349..362 /348..361|0/128 both|
| 6 |42..74 both|486..518 both|128/128 both|

Setting5 single conversion/read takes298 counts=4.65625 us in this diagnostic,
excluding channel preparation. It fits where setting5 full pair (~9 us) cannot.
Setting6 (~7 us single) remains too long. Additional comparator/timestamp
instructions must be included in the eventual live timing qualification; do
not assume this spare margin survives arbitrary instrumentation changes.

Entry024 idle sweep showed setting5 neutral means9..14 versus long4..6,
while setting6 closely matched long readings. Thus setting5 is a candidate
cross-check with residual acquisition dependence, not a calibrated reference.
Next: matched-window C and neutral single acquisitions on separate PWM cycles,
retain per-reading channel/time/end-count and never label them simultaneous.
Require comparable sector and bounded cycle separation before forming C-N;
use comparator agreement and explicit uncertainty instead of declaring success
from sign alone. Existing digital observer and no-control policy unchanged.

Final raw artifact includes p/i confirming all gates/CCRs/MOE/ENABLE zero,
nFAULT1,idle. UART closed,motor connected. Build passed with inherited warnings;
no duty/carrier change or BEMF-controlled commutation. Full goal incomplete.

---

## Entry 033 — 2026-09-12 — Graybeard memo redirects production qualification

Operator forwarded GRAYBEARD_BEMF_MEMO.md after requesting a pause. Read the
memo completely and checked actual minz source. No edits to analog observation
mode had begun before the pause; no new hardware commands or flash this entry.

Source confirms POLLING_MODE_CHANGEOVER2000 half-us ticks, switch at ci<2000
in zcfoundroutine, fallback at average_interval>2500 in commutate. At50 eHz,
ci~6667 belongs to polling;200 eHz~1667 belongs to interrupt mode. Current binz
observer is a polled microscope, not EXTI; nevertheless the campaign's emphasis
on perfect per-visit raw crossings at50 eHz was not the reference's operating
qualification. Further sequential/alternating ADC-sign work is deferred.

Use interval-lock, dropout, timeout/recovery and reference performance at
matched speed as acceptance evidence. Preserve all recorded measurement facts
without promoting the strict offline single-transition audit to a production
23/23 requirement. No claim that the ADC disagreement's physical cause is
fully proven; it is no longer the critical production-path blocker.

Important source qualifications to the memo: zcfoundroutine is polling and
uses polling_blend; interrupt blend is in the COM ISR. interrupt_routine arms
COM timer after accepted input and COM ISR disables its interrupt on service.
Thus replay must model actual events/timers, not presume free-running predicted
commutation through arbitrary missing edges. desync_due's50% threshold is gated
at average_interval<2000 and is not by itself a lock or safety certificate.

Updated BEMF_PARITY_PLAN with next work: full reference sequence replay,
reference snapshot including modified-source hashes, matched-speed acceptance
metrics, then staged V/f progression toward200–250 eHz under retained envelope
gates. HEAD2efda4911c4be8e238e16d192d821151bd775887 has modified minz-core
files and is not yet a frozen reference. No speed/duty ceiling changed here.
Last verified hardware state remains gates/MOE/ENABLE off, serial released.

---

## Entry 034 — 2026-09-12 — Freeze modified reference; verify source baseline

Read replacement active goal: staged progression toward 200 eHz, optionally
250 eHz and at most 30% duty only after current/bus/tracking evidence; 800 mA,
five-second limit and loss-of-tracking protection precede envelope expansion.
No flash, serial command or motor run this entry. No assertion of a fresh
hardware readback; last measured safe state remains Entry 032.

Created captures/reference/minz_20260912.zip with 128 payload files, individual
SHA256 manifest, scoped Git status and tracked diff. Archive SHA256:
2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
Includes modified control sources, MCU adapters/examples and analysis scripts;
excludes raw captures/external dependencies. See reference/README for scope.
Script verifies archive bytes and can reject live sibling changes before replay.
This does not establish provenance of historical firmware captures.

Reference core host tests: 83 passed. Capture/replay Python suite: 24 passed,
including four snapshot checks (valid, corrupt archive, live edit/addition).
Live source matches snapshot. Incremental-cache access warning is nonfatal.

Benchmark search found older SWIFT carrier results at 1112–1661 eHz, AM32
curve beginning at 528 eHz, and high-speed AM32 data at 1642–2325 eHz, on
different supply conditions. None establishes current clone parity at 200 eHz.
Continue provenance/search and integrated timer-driven HAL simulation; do not
replace the requested lock/dropout/recovery benchmarks with component tests.
Sparse polled observer captures cannot reconstruct every EXTI persistence read.
Goal remains incomplete; original spin firmware and drive ceiling unchanged.

---

## Entry 035 — 2026-09-12 — Connected logical-time AM32 sequence regressions

Added tools/observer-replay/tests/sequence.rs and a host-only HAL/storage fixture
adapted from the frozen reference. All controller calculations/calls still come
from the actual minz-core dependency; no sibling edits or firmware modifications.
Live reference hashes rechecked against the archive and match.

Eight sequence tests pass, plus four existing detector tests. Includes 120
synthetic crossing/commutation cycles at both 1667 and 1333 half-us ticks
(approximately 200/250 eHz), average/history and sector order, polling wait and
changeover, full TIM6 duty/polling call, persistence rejection, blank/pending
semantics, fallback/desync distinction, timeout/re-kick and safe cancellation.

Decisive safety evidence from connected sequence: after the last accepted event's
COM fires, missing edges produce no further commutations. Average and desync can
remain unchanged. At >45000 interval ticks the separate reference timeout path
re-kicks and returns to polling. Therefore no-desync is not lock and cannot be
our loss-of-tracking abort. Preserve independent accepted-event age and progress.

Limits explicitly recorded in harness README: synthetic stimulus is not a rotor
model or captured bench replay. Logical COM deadline assumes immediate active
ARR, whereas minz's hardware wrapper uses ARPE with no explicit UG on arm;
preload behavior must be modeled/measured. Persistence cost, interrupt preemption,
pending-bit interrupt storms and M0 timing remain unqualified. Reference ADC
fixture values do not establish DRV protection. Host all_off does not prove pins.

No flash or motor command this entry. Drive ceiling and shell-sine unchanged;
no fresh physical safe-state claim. Next work remains register-faithful timer/
event capture, stale-event tracking protection and observe-only bench validation
before giving BEMF control authority or expanding the envelope.

---

## Entry 036 — 2026-09-12 — Disabled TIM16 hardware exposes preload delay

Preflight COM41 at115200: off/p/i confirms all gate outputs and inputs0,
MOE0,ENABLE0,CCRs0,nFAULT1,idle. No motor-driving command. Added comtiming
diagnostic in support/com_timing.rs; built and serialized download/reset of
shell-pwm. No duty/carrier or working spin-profile change; shell-sine untouched.

TIM16 PSC31 gives half-us ticks. No timer IRQ or AF output enabled. Seed active
ARR1999, then request399. Written pre-run gates: immediate/update modes must
measure190–210 us; untransferred preload990–1010 us. Two repeats of16 trials
per mode, saved raw in captures/bemf_com_timing.txt:

| ARR mode | Repeat1 us | Repeat2 us | Timeouts |
|---|---|---|---|
| ARPE off, immediate |200–201|200–201|0|
| ARPE on, no UG |1000–1001|1000–1001|0|
| ARPE on, explicit UG |200–201|200–201|0|

This validates the timer clock and confirms that the requested ARR alone does
not define the next deadline with preload active. Do not blindly copy the
reference wrapper's ARPE/no-UG register sequence into the G071 adapter while
assuming immediate activation. This does not prove a fault in the running minz
rig: its free-running timer may have transferred preload before a given arm.

Added selectable active/shadow ARR behavior and seeded-delay regression to the
host sequence fixture:9 sequence tests pass. Full masked/free-running overflow
history and ISR latency remain unmodeled/unmeasured. Plan G071 adapter with
explicit activation, then qualify interrupt scheduling under real workload.

Image text52724,data0,bss27536 bytes. Final p/i in raw artifact verifies all
gates/CCRs/MOE/ENABLE0,nFAULT1,idle. TIM16 stopped, serial closed. Motor stayed
disabled throughout. Goal incomplete; no expanded envelope or BEMF control.

---

## Entry 037 — 2026-09-12 — Immediate-ARR COM adapter delivers hardware IRQs

Added support/com_timer.rs implementing minz-core ComTimer/ComTimerExt.
G071 TIM16 PSC31,ARPEoff; arm stops timer, clears pending/UIF, writes CNT/ARR,
then enables update and restarts in a short critical section. Diagnostic-only
TIM16 vector timestamps via TIM17, disables update, clears UIF and counts.
No phase/MOE/ENABLE control exists in the ISR. No reused old-board safing.

Preflight off/p/i confirmed safe; build and serialized flash/reset succeeded.
Pre-run acceptance:32 requested/32 delivered each batch,0 timeout,190–215 us
from arm bracket to handler timestamp for ARR399 (200 us). Two comirq calls:

| Foreground | Repeat1 | Repeat2 |
|---|---|---|
| idle |32 IRQ,201–202 us,0 timeout|32 IRQ,201–202 us,0 timeout|
| ADC6 repeated reads |32 IRQ,201–202 us,0 timeout|32 IRQ,201–202 us,0 timeout|

Raw captures/bemf_com_irq.txt retains output and final safe p/i. All128
requests delivered. This includes software arm overhead and interrupt entry
timestamp cost, not a pure exception-latency measurement. It does not qualify
full core COM handler WCET, comparator interrupt storm/preemption or rotor lock.
No motor energized and no speed/duty envelope change.

Image text54044,data0,bss27544. shell-sine hash unchanged. Timer stopped,
NVIC masked/unpended; final all gates/CCRs/MOE/ENABLE0,nFAULT1,idle. Serial
closed. Next: connect comparator/interval and observer-only core service to
the qualified timer primitive, retaining independent tracking/protection.
Full BEMF-controlled performance goal remains incomplete.

---

## Entry 038 — 2026-09-12 — Actual reference COMP/COM executes on G071

Added reusable no_std owned core-state clusters adapted from frozen reference
mock. Added core_bench: synthetic comparator/EXTI seam, real TIM2 interval
timer at2 MHz and qualified TIM16 adapter. COM interrupt dispatch invokes actual
minz-core tim1_up_tim16_isr; foreground synthetic edges invoke actual comp_isr.
Phase/PWM HAL cannot write gates: phase requests only increment a counter.
Reference sources verified unchanged against frozen manifest before build.

Preflight off/p/i safe; serialized flash/reset. Pre-run gates:32 sent/32 COM,
zero masked requests, each service<100 us. Two repeats, raw retained at
captures/bemf_core_bench.txt:

| Input ci (half-us ticks) | Sent/COM both | COMP max us | COM max us | Average both |
|---|---|---|---|---|
|1667 (~200 eHz)|32/32|25 both|44 /43|1665|
|1333 (~250 eHz)|32/32|25 both|44 /44|1331|

All batches masked0. Time includes software adapter/reference path, with12
persistence reads and ZCT writes. COM timestamp bracket excludes vector entry
and return. Comparator stimulus is software, NOT real COMP2/EXTI electrical
edges. Phase/mux register writes and interference from other production ISRs
are not included. No claim of full ISR WCET, polling changeover or rotor lock.

The8-entry ZCT ring retains7 and reports25 overwritten records for32 events.
This is explicit loss accounting, NOT lossless capture/replay. Need a drained
or larger compact event fixture for the real observation campaign. Analog
current/bus/tracking guards are not implemented by this diagnostic.

Image text57332,data0,bss27880. Final p/i confirms gates/CCRs/MOE/ENABLE0,
nFAULT1,idle. TIM2 stopped,TIM16 stopped/masked,serial closed. Motor remained
disabled and10% drive ceiling unchanged. Next: real comparator adapter/event
capture and independent tracking protection; full goal remains incomplete.

---

## Entry 039 — 2026-09-12 — Real COMP2 EXTI adapter and stable routing control

Added comp_input implementing minz Comparator/CompExti on real COMP2/EXTI18.
Local cached HAL confirms line18, PA3 positive code2, PB3/PB7/PA2 negative6/7/8.
Phase sequence C,A,B,C,A,B; raw rising on odd steps matches minz polarity,
not generic rm32's inverted edge API. ADC_COMP vector currently captures one
edge then masks; it does NOT yet call core COMP service or commutate.

Preflight off/p/i safe. Built/flashed diagnostic compirq, which toggles output
polarity while gates/MOE/ENABLE remain off. Pre-run gate8/8 correct pending
edge+output level on each step. Two repeats had no timeouts but only31/48 and
27/48 full agreements; expected edge may already have reversed before read.
Raw captures/bemf_comp_irq.txt. Do not infer bad wiring from idle-level behavior.

Added stable-input control compirqref: INM3 full internal VREFINT, INP remains
external neutral. Same polarity stimulus and selected-edge routing; no motor.
Rebuilt/flashed, two repeats passed8/8 on all six steps:96/96,zero timeouts.
Odd steps flag5(rising pending,output high); even flag2(falling pending,low).
Raw captures/bemf_comp_irq_ref.txt. This isolates working comparator-output /
EXTI delivery from external idle phase-vs-neutral conditions. Consistent with
idle chatter, not proof of its exact analog cause, and not physical BEMF proof.

Both diagnostics restore original COMP2 CSR and mask/clear EXTI18/NVIC; final
raw p/i confirms gates/CCRs/MOE/ENABLE0,nFAULT1,idle. Serial closed. No drive
envelope change. Next integrate the real input seam with core observe-only
service, bounded IRQ work, loss-accounted capture and tracking protection.
Full goal remains incomplete.

---

## Entry 040 — 2026-09-12 — Variable eHz controls; return to real motor capture

Operator asked why no recent spins and requested variable eHz shell controls.
Implemented ehz1..250 (set/query, no start), bare run uses stored target,
runN sets+starts; observation now follows target/duty. Fixed unsigned ramp
underflow for targets above100 eHz; shared parser/up-down-ramp tests pass.
Observer ON-validity boundary follows requested duty instead of fixed384counts.
Python capture supports variable campaign target.10% ceiling and five-second
limit retained; no automatic duty scaling. Higher configured range is not
measured qualification and does not authorize unguarded envelope expansion.

Built/flashed current image, including dormant coreirq integration from the
interrupted prior turn (not executed or qualified yet). Shell verified ehz200
set/query, ehz251 rejection, reset50. Preflight supply11713mV; enabled CSA
means2050/2049/2053 for ADC0/1/4. Sleep-state current readings are invalid.
sensezero wake_fault1 then scan_fault0 matches Entry016's recorded wake behavior.
Raw captures/variable_ehz_preflight.txt retains disabled-state checks.

Executed real existing profile:100eHz7%catch ->50eHz6%hold ->six-step observer.
captures/bemf_return_spin_01 raw/CSV/coast/plots/obs retained. Normal reason1,
energized4778714us,256 retained drive rows,Vbus11347–11870mV,zero fault samples.
Current instantaneous reported excursions reach approximately11A; these are not
PSU-average current and do not qualify an increased duty. Supply limit retained
at operator-established800mA; no fresh external current-meter reading this run.

Coast rotation valid, A->B->C order1.0; extrapolated disable56.64eHz,R2=.99928.
Historical45–55 endpoint check fails; not proof of precise50Hz tracking.
Observer384rows/78446us,24commutation-blank rejects,zero ON-timing rejects or
requested-slot misses (224/448 intentionally omitted). All six phases show late
comparator states0/1. Host replay verifies384/384 decisions,13 candidates over
all six steps; not full AM32 lock proof. Reported max control interval7643us
also remains a timing concern before declaring production timing qualified.

Final bemf_return_spin_01_safe.txt verifies gates/CCRs/MOE/ENABLE0,nFAULT1,idle;
serial closed. Real spinning resumed, but full BEMF control, independent tracking
abort and higher-speed qualification remain unfinished. No claim of200eHz spin.

---

## Entry 041 — 2026-09-12 — Independent stale-event watch, live baseline

Added portable event_watch with latched stale/counter-backward outcomes.
Age is checked before accepting a new count, so late events cannot hide a blind
interval; duplicate counts cannot refresh; unsigned time/count wraps covered.
Five new tests pass (11 library tests total). Observation integrates the watch
at each loop pass with22.5ms limit from minz BEMF_TIMEOUT_TICKS/2; new reason6
flows through existing gates/MOE/ENABLE-off exit. It watches bench-detector
candidates, NOT independently proven rotor events. This is a necessary stale-
signal backstop, not completed loss-of-tracking protection. Sine startup/hold
is not covered by this watch and higher-speed envelope remains unqualified.

Built/flashed and ran existing50eHz6% profile with observation, raw artifacts
captures/bemf_watch_spin_01*. Normal reason1,4778786us energized, no faults in
256retained drive samples,Vbus11519–11923mV. Instantaneous phase-current values
are not a PSU-average-current qualification. Coast motion valid,orderA->B->C
score1.0,disable extrapolation53.49eHz,R2=.99916.384observer records/78445us,
24blanked,zero ON timing rejects or requested-slot misses,late bracket345–363.
Max control interval7644us persists; production cadence remains unqualified.

Watch did not false-trip this run. Host tests exercise stale/latching behavior,
but hardware stale-event fault injection has not yet been performed. Do not
report this successful normal run as validation of the fault shutdown path.
Final *_safe.txt confirms gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART released.
No higher-speed/duty run. Goal remains incomplete.

---

## Entry 042 — 2026-09-12 — Remove UART-induced energized control gap

Source inspection found RUN enabled PWM then synchronously printed its long
banner before the next control update. Moved banner+flush before MOE/drive
clock start, for campaign and fixed-sine modes. Added reason7 timing-overrun
stop for control intervals>2000us, before applying the next waveform update.
Stop uses existing gates/MOE/ENABLE-off and coast machinery. No duty/speed
increase;1kHz envelope update rate unchanged (only5updates/cycle at200eHz).

Built/flashed, actual existing50eHz6% campaign+observation captured as
captures/bemf_cadence_spin_01*. Pre-run gates:control gap<=2ms,no fault,
valid coast rotation,verified shutdown. Measured max_control_interval1003us
versus7644us in Entry041, corroborating the source-identified UART delay.
Normal reason1,4779121us energized,4694updates;Vbus11365–11852mV in retained
256samples,zero fault samples. Coast motion valid,order1.0,disable58.17eHz
R2=.99951 (outside historical45–55 gate; not precise target-lock proof).
Observer384rows/78524us,25blanked,zero ON rejects/requested-slot misses.

Final *_safe.txt verifies gates/CCRs/MOE/ENABLE0,nFAULT1,idle;serial closed.
The normal run validates removal of the startup gap, not injected-overrun
shutdown latency.1kHz waveform granularity, physical tracking protection and
full spinning reference BEMF path remain unresolved before higher-speed work.
Full goal remains incomplete.

---

## Entry 043 — 2026-09-12 — Elapsed-time waveform updates decoupled from ADC

Separated nominal100us waveform updates from1kHz envelope/ADC collection.
Q32 phase increment uses actual elapsed time; frequency-to-rate division runs
at envelope cadence, not in fast path. Fixed-sine and campaign initialize phase
clocks after banner; observer handoff extrapolates from last waveform timestamp.
Existing capture schema and shell-sine preserved. Added cadence-independence
arithmetic regression;12host library tests passed and firmware built.

Real50eHz6% campaign+observer in captures/bemf_fastwave_spin_01*. Normal end,
4779103us energized,4692control ticks,23463wave updates. Max control1005us;
max waveform gap604us FAILS pre-run500us check, despite much faster average
waveform service. Foreground ADC/mux workload still blocks it. Do not claim
10kHz deterministic updates or qualified200eHz waveform delivery.
Coast49.57eHz extrapolated,R2=.99891,order1.0,motion valid.256retained drive
samples,Vbus11498–12033mV,no fault samples. Final *_safe.txt confirms all
gates/CCRs/MOE/ENABLE0,nFAULT1,idle. UART released. Higher envelope not run.

---

## Entry 044 — 2026-09-12 — USART3 DMA/CRC baud spike on actual USB-TTL link

Operator requested a DMA speed/CRC spike, explicitly noting MCU baud capability
does not prove the attached adapter's limit. Implemented disabled-driver
uart_dma diagnostic: USART3 RX/TX request54/55 via DMA1CH1/DMAMUX,1024byte buffer,
32fixed frames per trial. Host frames include magic,sequence,deterministic data
and CRC32; MCU verifies RX CRC, XOR-transforms payload and generates reply CRC.
Host checks every reply byte/sequence/CRC, retains raw replies and shell logs.
Deadline2s per transfer; completion/error restores original UART registers and
115200. Oversampling8 configuration included for>4M but not bench-tested.

Built/flashed; no drive commands. scripts/drv_uart_dma.py retains JSON and raw
artifacts uart_dma_01* and uart_dma_high_01*. Measured roundtrip aggregate byte
throughput (both directions, including headers/CRC; not payload-only):

| Requested baud | Correct frames | Aggregate bytes/s | Firmware errors |
|---|---|---|---|
|115200|32/32|10684|0|
|460800|32/32|32569|0|
|1000000|32/32|52470|0|
|2000000|32/32|70745|0|
|3000000|32/32|78962|0|
|4000000|0/32|not measured|host received0bytes for first reply|

Sweep stops at first failure;6M/8M not run.4M failure localizes only to whole
link/configuration, not conclusively the FTDI silicon. Timeout recovery to115200
worked and final p/i safe in every artifact. No CRC/payload error among160
successful frames (160KiB each direction). These are SHORT stop-and-wait trials;
USB latency and firmware software CRC cost are included. Not simultaneous
full-duplex saturation, DMA-ring overrun stress or long-duration qualification.
Default shell115200 unchanged. This demonstrates useful speed headroom without
claiming8M support. Motor remained disabled; full BEMF goal remains incomplete.

---

## Entry 045 — 2026-09-12 — Timer-driven waveform: real baseline spin

Moved sine CCR updates into TIM6 interrupt at 100 us; envelope/ADC remain in
foreground. Elapsed TIM17 time advances phase. ISR contains no ADC or UART;
nFAULT, >500 us service gap and 4.999 s deadline safe gates and ENABLE.
gates_off cancels TIM6 before clearing outputs. Startup arms timer/MOE inside
one critical section so a fault ISR cannot be followed by foreground MOE-on.
Observer handoff stops waveform timer and extrapolates phase continuously.

Predeclared check: baseline 100 eHz/7% catch to 50 eHz/6% hold, <5 s,
waveform gap <=500 us, bus/fault/coast checks. Existing connected DRV8304H/G071
mapping, nominal 12 V and operator-set 800 mA PSU limit retained; no rewiring.
Built/flashed shell-pwm and ran drv_capture --observe --target 50
--duty-tenths 60 --tag bemf_irqwave_spin_01. Raw/CSV/plots retained in captures.

Measured: normal reason1 end, 4778714 us energized, 4686 control updates,
max control interval1041 us. 47001 waveform updates, max gap103 us (PASS,
previous foreground604 us), max ISR cost36 us. This is substantial M0 CPU
usage (~36% worst measured cost at10kHz), not free sensing capacity.
256 retained drive samples: Vbus11111–12989 mV, zero nFAULT samples.
Phase-current sampled peaks reached about11.7 A; not average PSU current.
Coast motion valid, phase order score1.0, extrapolated55.64 eHz R2=.99970;
old45–55 gate FAIL. Do not claim rotor lock or qualify higher duty from this.

384 observer records over78466 us, 24 blank rejects, zero ON timing-invalid
records, 16 candidates across all six steps. Host replay matched384/384 live
decisions; this remains the small candidate detector, not full AM32 lock proof.
Final bemf_irqwave_spin_01_safe.txt verifies all six gates/CCRs/MOE/ENABLE0,
nFAULT1 and idle. Serial closed. 12 host unit +9 sequence +24 Python tests pass.
shell-sine SHA256 unchanged (B6BBA660...54206E).
Full reference-controlled BEMF, matched-speed parity and expanded-envelope
current/tracking qualification remain incomplete; next is actual COMP IRQ
through full reference logic with outputs disabled, then spinning observation.

---

## Entry 046 — 2026-09-12 — Actual COMP IRQ through reference COMP/COM

Ran coreirq with gates/MOE/ENABLE disabled. Internal VREF comparator separation
plus polarity toggles forces actual COMP2/EXTI18 events into minz comp_isr;
real TIM2 interval and TIM16 COM IRQ execute the reference routines. Phase HAL
records requests only and cannot drive gates. This is synthetic-event timing,
NOT physical BEMF or full polling/changeover qualification.

captures/core_irq_01.txt: ci1667 and1333 half-us ticks (~200/250eHz), each
32sent/32IRQ/32COM, no masked attempts. COMP max33us/COM max50us. Existing
8-slot ring dropped25 per block, inadequate for consecutive replay evidence.
Expanded board-local ZctStore to64 slots (63 usable), +1680bytes. Firmware
data+bss30684bytes leaves6180bytes of36KiB for stack; stack high-water unmeasured.
cap1 now opts corebench/coreirq into one-shot post-disabled Z85/CRC dumps of
canonical minz ZCT bytes; ordinary commands retain summary-only output.

Rebuilt/flashed, captures/core_irq_02.txt: each timing32sent/32IRQ/32COM,
zero drops. COMP max32us, COM49us at200/48us at250; avg1665/1331 ticks.
scripts/drv_core_trace.py verifies64/64 CRC-framed records and consecutive
six-step order. thiszc1664..1729 and1330..1395 ticks (includes startup).
This parser validates transport/order, not complete AM32 decision replay.
Frozen reference source check still passes128files and archive hash unchanged.
Both attempts final p/i confirm all gates/CCRs/MOE/ENABLE0,nFAULT1; UART closed.
Next unresolved physical work: spinning floating-phase comparator observations
through reference logic with correct mux/physical-step identity, then guarded
control authority. Synthetic success does not justify expanded duty yet.

---

## Entry 047 — 2026-09-12 — Reference polling replay reaches causal boundary

Added host --full-polling <seed_ci_half_us> <obs.csv>, using actual minz
polling_bemf_check -> zcfoundroutine (blocking wait, commutation/changeover),
interval averaging, desync band, filter/duty band and timeout/re-kick band.
No local replacement of reference control. Initial interval is explicit;
last crossing is ASSUMED half a sector before recorded sector start. Uses only
valid recorded late-ON comparator levels at their recorded ~200us cadence;
does not interpolate them to reference20kHz or invent missing edges.
Thus this is conditional sample-cadence replay, NOT production timing parity.

Ran on bemf_irqwave_spin_01_obs.csv with ci6667 (~50eHz). Reference requests
step2 at2973.5us after10polls; first subsequent usable-time row3112us still
describes physical step1/floatingC. Stops before feeding that wrong-phase row.
Summary avg6703, one COM, zero desync/recovery, explicitly lock_proven=0.
Raw output in bemf_irqwave_spin_01_full_polling.txt; integration regression
pins the divergence and prevents silently continuing/reseeding on wrong mux.
No motor commands or flash this entry; last verified disabled state Entry046.

This exposes why a single sequential floating-phase trace cannot prove an
independent closed-loop trajectory after its first differing commutation.
Next live observation must keep recorded physical drive/mux identity explicit
and report reference-request timing relative to actual forced commutations;
do not present reseeded sector trials as uninterrupted rotor lock. Full
production polling cadence and interrupt-mode physical BEMF remain unqualified.

---

## Entry 048 — 2026-09-12 — Live motor levels reach reference polling request

Added record-only live polling prefix in existing six-step observation window.
Uses core_bench state + actual minz polling_bemf_check/zcfoundroutine, averaging,
desync and timeout bands. Comparator HAL returns latest retained valid late-ON
sample; cannot alter physical mux. Phase HAL records request only; cannot alter
gates. Stops observing on first COM request or physical-step mismatch; never
reseeds sectors and calls that uninterrupted lock. Real TIM2 advances during
reference blocking wait. Initial prior-ZC assumption is ci/2, explicitly not a
measured rotor timestamp. Enabled only50..166eHz to bound initial polling wait;
this is still ~200us sample cadence, NOT production20kHz qualification.

Built/flashed and ran two baseline100eHz7%catch ->50eHz6%hold campaigns with
motor connected, unchanged DRV8304/G071 map, nominal12V/800mA PSU setting.
Pre-run gates: <5s, max waveform gap<=500us, current/bus/nFAULT observer guards,
final all-off. No increased duty or target speed.

bemf_corelive_spin_01: normal4779888us, maxwave102us, bus11502..11858mV,
no fault samples, coast54.28eHz extrapolated/R2=.99903/order1/motion valid.
Reference summary initially absent from fixture output due to placement only
in obstiming command; do not infer its result. Fixed summary routing and reran.

bemf_corelive_spin_02: normal4780689us, maxwave103us/maxISR36us, bus11191..
11853mV, no fault samples. COREOBS status2, polls7, maxcall950us, seedci6666,
physicalstep1, requestedstep2 at3164us, avg6714, com1, gate_authority0.
This is a real reference request based on live motor comparator samples, not
proof they are exclusively rotor BEMF. Blocking reference wait materially
pauses microscope/foreground forced-drive service; must account for this before
production use. Observer384rows/80137us,25blank rejects,zero ON timing rejects;
small detector replay384/384 matches (separate from full-reference prefix).
Coast56.18eHz/R2=.99956/order1/motion valid; old45..55 gate FAIL, not lock.

Both captures retain raw/CSV/plots and *_safe.txt with gates/CCRs/MOE/ENABLE0,
nFAULT1,idle. Serial released. Firmware data+bss30712bytes; stack6152bytes
available in36KiB, high-water not measured. Remaining work includes production
polling cadence and scheduling around blocking wait, physical interrupt-mode
BEMF, sustained tracking/current guards and matched-speed reference parity.

---

## Entry 049 — 2026-09-12 — Six-step IRQ scheduler attempt NOT qualified

Preserved reference blocking wait. Extended TIM6 waveform scheduler to forced
six-step, so reference foreground polling cannot hold up physical commutation.
Separated bridge_clear from scheduler-cancelling gates_off; IRQ-owned writes
use bridge_clear, safing still cancels scheduler first. Added96ms six-step IRQ
deadline, nFAULT/service-gap guards, and mixed-phase scan rejection when the
physical step changes during acquisition. Existing wire format retained.

Built/flashed, baseline50eHz6% attempted via tag bemf_sixirq_spin_01. Fixture
timed out without COAST END, then serial off/p/i all returned no bytes. This
run FAILS completion/evidence gates. No raw capture was retained because the
fixture only saved successful parses; fixed it to preserve received bytes in
finally on both success and failure, after issuing safing requests.

Reset with ST-Link; captures/bemf_sixirq_spin_01_recovery.txt records actual
post-reset p/i: all six gates/CCRs/MOE/ENABLE0,nFAULT1,idle. Serial released.
Pre-reset safety/energized duration was NOT independently verified; do not
retroactively claim a normal5s completion. Panic handler is wired to safe the
correct board, but panic cause itself is not proven. Hypothesis: new ISR
preemption interferes with the narrow CNT32..64 capture wait and causes a
timeout panic. Must reproduce with driver disabled before further motor tests.
New six-step scheduler is unqualified; do not use it to increase envelope.

---

## Entry 050 — 2026-09-12 — Disabled reproduction localizes silent stop

Operator confirms minz bench fully disconnected. No reference-hardware run or
rewiring is available; continue G071 work and keep matched-speed hardware parity
unverified. Frozen-source and existing-capture comparisons remain available.

Added debugger-readable PANIC_LINE, written after correct-board safing. Built/
flashed; driver-disabled obstiming reproduced unresponsive shell. Read RAM
0x20000008 via probe-rs:00000059 = line89, observation.rs late PWM-slot timeout.
This proves the disabled reproduction's panic site, not every detail of the
previous energized failure. New timer interrupts can disrupt instrument waits.

Changed observation window/slot timeouts to bounded counted retries, not panic.
read_slot returns explicit invalid sentinel on timeout. Outer96ms deadline,
nFAULT, service-gap guard and candidate-age watchdog remain; discarded traces
increment TRACE_TIMEOUTS, emitted as OBSLOSS in fixture dumps. No controller
logic changed. Rebuilt/flashed and repeated disabled test successfully returned
to shell:157records, reason6 (candidate age),34480us. COREOBS stops on physical
step mismatch with no request. Early/late ON brackets358..380, zero ON rejects;
late OFF slot5600 misses151/157, so instrumentation timing is NOT fully clean.
No motor spin this entry. All gates/CCRs/MOE/ENABLE0,nFAULT1 verified by p/i;
raw diagnostic transcript in captures/sixirq_disabled_recovery_01.txt.
Next qualify relevant sampling/timer timing without demanding every unused OFF
slot; do not equate graceful timeout recovery with qualified BEMF control.

---

## Entry 051 — 2026-09-12 — Real spin: forced IRQ waveform survives reference wait

Retried same baseline with the Entry050 built/flashed firmware after disabled
reproduction returned safely:100eHz7%catch ->50eHz6%hold, no envelope increase,
unchanged connected DRV8304/G071 wiring, nominal12V/800mA operator PSU setting.
Pre-run gates: <=5s energized, max waveform service gap<=500us, existing
current/bus/nFAULT guards, coast motion and final safe readback.

captures/bemf_sixirq_spin_02*: normalreason1,4783380us energized,47831waveform
updates across sine AND six-step, maxgap103us/maxISR38us. COREOBS status2,
polls5, requeststep2 at2443us, seededci6666/avg6658, one record-only request.
maxcall944us with wait_wave_updates10: direct evidence TIM6 kept servicing
forced waveform during the reference's intentional blocking wait. This fixes
the foreground scheduling interference without changing minz control science.

256retained drive samples: bus11489..12109mV, zero nFAULT samples. Coast motion
valid, extrapolated54.16eHz/R2=.99886/phase-order1.0;45..55 gate passes this run.
Observer384records/82800us, ONbracket360..382counts, zero ONtiming rejects;
11discarded trace attempts,13mixed-phase/wrap rejects,3blank rejects. OFFslot
5600 misses379/384 (diagnostic limitation, not silently valid data). Candidate
replay384/384 decisions matches,15candidates across all steps. Full reference
still observes only first causal prefix; it does NOT control gates or prove lock.

Final *_safe.txt actual p/i verifies all gates/CCRs/MOE/ENABLE0,nFAULT1,idle.
Fixture persisted raw bytes before parsing; serial released. Next advance is
production-cadence physical comparator sampling/control qualification, not more
requirements for unused OFF-slot perfection. Higher-speed/current/tracking and
matched-speed hardware parity remain unverified (minz bench disconnected).

---

## Entry 052 — 2026-09-12 — Direct comparator20kHz polling prefix on motor

Added opt-in corepoll1 (one-shot next observation, corepoll0 cancels). TIM7
50us IRQ priority0xc0 calls actual reference polling logic using direct COMP2
reads, not retained ON samples. TIM6 priority0 forced waveform preempts the
reference blocking wait. Observer phase/mux remain record-only; stop at first
request or physical-step mismatch. gates_off masks both schedulers. ISR time
uses16-bit TIM17 prefix elapsed, not foreground clock_us mutable extension.
This remains a bounded-prefix observer, not a complete motor controller.

Disabled corepoll_disabled_01 showed69us maxpoll gap: reference main bands had
mistakenly been placed in every poll. Moved averaging/filter/desync/recovery
bands to foreground, leaving polling ISR separate. corepoll_disabled_02:
50us maxgap over4polls,803us request call,8wave updates during wait; all safe.
Disabled inputs produced a request: direct proof request counts alone cannot
distinguish actual rotor BEMF from idle comparator levels. No lock claim.

Rebuilt/flashed; armed corepoll1, then standard50eHz6%baseline capture fixture
tag bemf_core20_spin_01. Normal4786263us, waveformmaxgap103us/maxISR37us.
COREPOLL direct=1 maxgap55us over58polls, COREOBS maxcall31us, NO request before
physical step changed (status3). This differs from slower ON-sample polling;
must characterize physical comparator polarity/PWM timing over sectors, not
silently substitute the candidate detector. No higher speed/duty attempted.
256drive samples bus11397..11903mV/no fault samples. Coast47.36eHz extrapolated,
R2=.99845,order.8,motion valid.32discarded trace attempts explicitly counted.
Final *_safe.txt p/i all gates/CCRs/MOE/ENABLE0,nFAULT1,idle. Serial released.
Cadence result is a short prefix, not sustained20kHz qualification or lock.

---

## Entry 053 — 2026-09-12 — Live shell sweep reaches measured~97eHz

Operator explicitly requests serial live steps50,60,70,... toward200. Corrected
earlier claim: shell previously rejected everything except off while driving.
Added hold-only ehzN, within10eHz of current target, no duty changes/no timer
restart; short F<n> acknowledgment. Fixture --step-targets supports five steps
at3.2,3.5,3.8,4.1,4.4s, keeps raw commands/acks. Firmware feedback guard now
also applies observer's1200-count phase-peak backstop during sine; bus floor
8400mV for nominal12V bench (not generic voltage config). Not average current.

Earlier rung bemf_rung100_01:100eHz7% with short observer, reason6 candidate-age
abort at4759680us; bus11456..12043mV, no fault samples. Coast A had0edges and
motion validation failed. Safe readback retained; not validated100eHz lock.

live_sweep_50_100_01: first burst ehz60 arrived truncated as e, long busy reply
caused4963us control gap and reason7 safe abort at3217456us. No speed step
accepted. Fixed live command transport to3ms inter-byte pacing, shortened busy
reply. Preserved failure and safe artifacts; do not count it as completed sweep.

live_sweep_50_100_02: startup100eHz7%catch ->50eHz6.5%, then live60/70/80/90/100
all five acknowledged. Normal4998584us energized, maxcontrol1400us, maxwave103us.
Retained final256samples bus11386..11882mV/no faults, phase peaks~5.1A sampled.
Coast extrapolated97.23eHz/R2=.99991/order1.0/motion valid. This is first clean
measured~100eHz endpoint of stepped sweep, NOT full BEMF-controlled lock. Old
45..55Hz parser flag is irrelevant to100Hz target; measured error is-2.77%.

live_sweep_100_150_01: attempted start/hold100 then110..150 atsame6.5%. Peak
guard stopped at2144373us BEFORE any live step. Sample peak~13.95A, bus11472..
11876mV/no fault samples. Coast A0edges/order.286/motioninvalid. This identifies
a nonrepeatable startup/100Hz-hold path, not a150Hz speed ceiling. No guard
relaxation or duty increase. Next progression should preserve successful50Hz
entry and staircase instead of assuming prolonged100Hz startup is equivalent.

Every artifact has *_safe.txt p/i all gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART
released. Board/wiring unchanged, nominal12V/800mA PSU. No minz hardware used.
Python28tests passed before sweep. Higher-speed/current/tracking and final
reference parity goal remain incomplete.

---

## Entry 054 — 2026-09-12 — Staged live sweep reaches measured198.99eHz

Extended host-only --step-targets to15steps within3.2..4.4s (even spacing),
leaving firmware unchanged. Same proven50Hzentry,6.5%duty,100Hz7%catch,
10eHz command increments; no duty escalation or raised guard. Successful
prior endpoint/current/bus/coast measurements informed each next attempt.
Unchanged DRV8304/G071 wiring, nominal12V/800mA PSU. Every completed sweep
ended before5s with waveformgap103us; all raw commands/acks/captures retained.

| Capture prefix | Endpoint result | Outcome |
|---|---|---|
|live_sweep_50_110_01|108.51eHz coast, R2 .99990, order1|normal4998892us; bus11425..11966mV|
|live_sweep_50_130_01|no steps sent|phasepeak abort1613685us during startup; sampled14.68A|
|live_sweep_50_130_02|128.65eHz, R2 .99994, order1|normal4998778us; bus11487..11867mV|
|live_sweep_50_150_01|148.59eHz, R2 .99993, order1|normal4998486us; bus11433..11902mV|
|live_sweep_50_180_01|179.54eHz, R2 .99992, order1|normal4998781us; bus11500..11924mV|
|live_sweep_50_200_01|no steps sent|phasepeak abort1613781us during startup; sampled15.18A|
|live_sweep_50_200_02|198.99eHz, R2 .99993, order1|normal4998704us; bus11421..11995mV|

200 successful trial acknowledgedF60 throughF200. Retained final256drive
samples phase peaks~5.1A, zero nFAULT samples; not calibrated average current.
Wave49989updates/maxISR37us/maxgap103us, controlmax1516us. Coast motionvalid,
27/28/28phase edges, initial frequency199.20eHz. Extrapolated endpoint error
-0.505%; old45..55Hz parser flag irrelevant. Snapshot phaseC-neutral span
40.4mV during coast supports increased BEMF, not driven-comparator qualification.

Every attempt has *_safe.txt final p/i all gates/CCRs/MOE/ENABLE0,nFAULT1,idle;
serial released between runs. Two startup aborts retained and not called speed
limits; no retry increased duty or relaxed current guard. Startup is demonstrably
intermittent even with identical profile. No indefinite retry campaign.

Milestone: measured open-loop endpoint~200eHz at6.5% using live staircase.
NOT closed-loop control, sustained lock/current parity, or repeatable200Hz
qualification. Next use this entry profile for bounded comparator observation
in the reference interrupt-mode regime. Minz hardware remains disconnected.

## Entry 055 — 2026-09-12 — First200eHz six-step comparator capture

Fixture now permits --observe with live --step-targets. Same6.5%duty,50Hzentry,
15steps60..200; firmware unchanged and no guard relaxation. Last acknowledged
target200 drives the4700ms handoff. Raw captures preserved.

bemf_sweep200_obs_01: phasepeak abort743376us in100Hz catch before sweep or
observation. Peak~14.86A, no nFAULT samples; startup failure retained.
bemf_sweep200_obs_02 repeat: all15steps acknowledged, reason6 age-watch abort
at4722723us. Final256sine samples bus11504..12003mV/no faults, phase peaks~3.3A.
Waveformmaxgap103us/maxISR37us. Microscope87rows/22372us:17invalid (2blank,
15mixedphase), zero ON timing rejects, bracket358..380counts.12discarded traces,
OFF5600miss85/87 explicit. Small-detector replay87/87 matches: zero candidates,
4accumulating states. No watchdog extension to force a pass.

COREOBS status0: reference POLLING prefix intentionally not run at200Hz; range
is50..166. Thus this does not qualify reference interrupt mode. Zero sparse
low-rate candidates is not evidence of absent BEMF; raw levels vary and the
opposite/confirmation requirement can miss short sectors. Coast motionvalid,
extrapolated183.98eHz/R2=.99992/order1, initial199.20eHz. Short six-step handoff
has not yet demonstrated preserved speed lock.

Both *_safe.txt confirm gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART released.
Next is actual COMP2/EXTI -> reference COMP/COM observe-only prefix at200Hz,
with physical mux identity and divergence stop, not slow-detector tuning.

## Entry 056 — 2026-09-12 — Real COMP interrupts at200eHz reach reference

For observe windows167..250eHz, core_bench now selects actual COMP2/EXTI18
input into reference comp_isr and TIM16 COM ISR. Unlike synthetic coreirq,
never substitutes VREF or polarity toggles. Output HAL still records only;
reference mux changes after a request are suppressed. Stop on first requested
COM, physical phase change, or256IRQ diagnostic storm cap. Forced TIM6priority0
preempts COMPpriority0x40 and COMpriority0x80; final gates_off stops all paths.
Timestamp ISR prefix via TIM17, not foreground mutable clock extension.

Built/flashed and disabled obstiming200 preflight:7IRQs, COMPmax46us,
COMmax53us,1request at428us, gates/ENABLE disabled. This once again shows that
idle electrical inputs can produce accepted requests; acceptance alone is not
rotor evidence. Transcript coreexti200_disabled_01.txt includes safing.

bemf_exti200_01: startup peak abort117364us before staircase, retained/safed.
bemf_exti200_02 repeat: all15commandsF60..F200 accepted, unchanged6.5%duty.
COREEXTI live1,3IRQs, COMPmax35us, events0/COM0. OBSstatus3 physicalstep4:
no accepted crossing before forced physical phase changed, then observe-only
prefix stopped. Full microscopic window later stopped on candidate-age guard
(reason6), total4723521us energized. Wavegap103us/maxISR38us. Retained sine
bus11479..11872mV/no fault samples; sampled phase peaks~8.3A. Coastinitial200Hz,
extrapolated186.61Hz/R2=.99993/order1/motionvalid. No claim of preserved lock.

COREOBS sample_cadence_only remains a legacy polling-field label (1 here);
COREEXTI live1 is authoritative mode identification for these captures.
Both motor attempts *_safe.txt p/i gates/CCRs/MOE/ENABLE0,nFAULT1,idle, UART
released. Frozen128source check passes. Next expose blank/persistence rejection
at each real IRQ so diagnostics distinguish arrivals from accepted crossings.
Control authority, repeated lock, startup reliability and parity remain open.

## Entry 057 — 2026-09-12 — Reference read trace identifies persistence rejection

Added bounded32x14u16 IRQ trace to board HAL, no edits to minz source. Captures
the actual first IntervalTimer::count and Comparator::output_level reads used
by comp_isr, plus accepted EV_ACC delta, pending/masked state and elapsed cost.
No extra comparator read for diagnostics. I85/CRC dump only with fixture cap1
after gates disabled; explicit drop count. SRAMdata+bss31676bytes (~5KiB left
for stack; high-water unmeasured). Instrumentation overhead is not free.

Built/flashed, bemf_extitrace200_01: full15step staircase to200 acknowledged,
unchanged6.5%duty. Run4723233us then candidate-watch reason6, wavegap103us,
maxISR38us. Bus10911..11921mV, no fault samples, sampled phase peaks~7.8A.
COREEXTI2IRQs, max54us including tracing, no accepted event/COM; stop on
physical-step mismatch. Trace n2/drop0, CRC validated by drv_irq_trace.py:

-199us, step2, PWMcount562, gate_count1200 > avg1666/2=833: gate OPEN.
 Reference read comparator once:1 vs expected rising0; persistence rejects.
-299us, PWMcount560: zero gate/comparator reads, no accepted event, pending0.
 Reference found no pending event by service (not a persistence/blank reject).

PWMcounts nearly identical one100us carrier apart suggest switching-related
pulses; this is an inference, not proof of all crossing mechanisms. Coast
187.57eHz/R2=.99994/order1/motionvalid. Raw+CSV+plots retained; final safe p/i
in *_safe.txt confirms gates/CCRs/MOE/ENABLE0,nFAULT1,idle. UART released.
Need broader rotor-referenced event evidence, not reduced persistence merely
to accept these pulses. Full control/parity remains incomplete.

## Entry 058 — 2026-09-12 — Full-sector prefix confirms PWM-related rejection

Reference EXTI observer now waits for exactly one fresh forced-sector boundary
(status6), then seeds prior interval and starts. No repeated sector resets or
lock claim. TIM6 applies physical mux first, records step, then arms observer.
Fixed foreground stale-row step check to use actual physical step instead.
Same200eHz/6.5%staircase; no filter/duty/guard change. Built/flashed.

bemf_boundary200_01: all15steps accepted; boundary wait731us, next full sector2
observed.10IRQs/10CRC rows/drop0, all persistence rejects on FIRST comparator
read1 vs expected0. Gate open in every case (actual interval908..2450ticks,
threshold833). Later events304..804us occur100us apart atPWMcount560..563,
about8.8us into10kHz PWM. This rules out too-short initial partial sector and
blanking as explanations for this trial. Corresponds to6.5us source ON pulse
already ended by IRQ entry; latency/window alignment is now a concrete issue
to separate from rotor crossing phase and electrical signal validity.

Runtime4722709us then existing candidate-age abort; wavegap104us/maxISR49us,
COMPmax48us including trace. Bus11434..12004mV/no faults, sampled peaks~4.8A.
Coast187.98eHz/R2=.99994/order1/motionvalid. Raw/csv/plots plus *_safe.txt
retain all gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART closed. No BEMF authority.
Next targeted test can vary observation duty/window within existing envelope
to test short-ON/ISR timing, retaining current guards; do not weaken persistence.

## Entry 059 — 2026-09-12 — Observation duty control; startup prevents8% test

Added idle-only obsdu<N>0..100tenths,0inherits sine duty. Consumed once at
observation entry and capped at existing10% firmware ceiling. Fixture option
--observe-duty-tenths requires --observe, verifies arm acknowledgment and clears
override in finally; OBSDUTY records actual applied observation duty on dump.
Startup and staircase remain6.5%; planned short observation8%tests pulse-width
effect without changing reference filter or raising current guard.

Built/flashed and made two attempts only. bemf_obs8_200_01 phasepeak abort
719200us during catch; bemf_obs8_200_02 phasepeak abort2177993us during ramp.
Neither sent a live step or reached observation. OBSDUTY armed80 is NOT proof
8% was applied: there is no observation/applied-duty record. No8% result exists.
Peak sampled currents~14.55/~14.01A respectively; retained bus11421..12098 and
11421..12470mV, no nFAULT samples. Wavegap103us throughout. Raw/CSV/plots kept.

Both *_safe.txt p/i verify gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART closed.
Python30tests pass. Stop repeated attempts here; startup reliability now directly
prevents the planned comparison. Investigate startup transient/path using the
existing captures; do not weaken current guard or claim this is an8%duty limit.

## Entry 060 — 2026-09-12 — Startup trips span ADC scan ordinals

Read-only review of the existing DRV8304H shell-pwm captures, same established
logical current mapping and 800mA PSU limit. No firmware flash, energization,
wiring change or guard change in this entry. Added reproducible analysis in
scripts/drv_startup_review.py; original CAP header supplies physical ring index
and reason. Four unit tests cover wrap/order and missing/mismatched metadata.

Five failed startup captures have final excursions1212..1312counts from2048,
above the unchanged1200count peak backstop. Four have no excursion>1000 in
the preceding50records; live_sweep_50_130_01 has two. All five contain no nFAULT
assertion; VREF ranges1499..1513 across these captures. Final bus11.55..11.76V.
This is not a sustained rise in the retained 1kHz samples, but sub-millisecond
current and rotor motion during startup remain unresolved. Do not call the
trip an artifact, or use healthy bus voltage as proof of safe winding current.

| Capture | Tick / eHz | Peak phase | ADC order / peak ordinal | theta / on A,B,C |
| --- | --- | --- | --- | --- |
| bemf_obs8_200_01 | 717 / 100 | B | ABC / 2 | 209 / 0,6,4 |
| bemf_obs8_200_02 | 2171 / 70.75 | C | CAB / 1 | 140 / 2,1,7 |
| bemf_exti200_01 | 117 / 100 | C | CAB / 1 | 161 / 1,3,7 |
| live_sweep_50_130_01 | 1609 / 84.8 | C | ABC / 3 | 164 / 1,3,7 |
| live_sweep_50_200_01 | 1609 / 84.8 | C | ABC / 3 | 164 / 1,3,7 |

Source check: current_read_rotated rotates by physical ring index, not global
tick; slot255 and slot0 both start A. Peaks span first, second and third scan
positions, so a fault exclusive to the first conversion after VREF does not
explain all five. Two independent failures coincide at tick1609/theta164 and
all five peak on the phase with the largest commanded ON value. These are
consistent with waveform-dependent pulse current, not proof of its mechanism.
ADC reads are sequential; their sum is not a simultaneous Kirchhoff check.
Successful live_sweep_50_200_02 retains only the END of its run, so it cannot
serve as a matched-time successful-startup control.

Next discriminating measurement: capture PWM counter and elapsed conversion
time around each phase-current conversion on a failed sample, with the same
peak abort and no confirmation delay. Current captures have only commanded
phase/ON metadata, not actual PWM sampling timestamps. Use that evidence to
separate pulse-position correlation from channel-switch acquisition effects
before changing startup or protection. The8% observation test remains unrun;
200eHz open-loop remains demonstrated, BEMF control remains unqualified.

## Entry 061 — 2026-09-12 — Current timing and direct-upward startup comparison

Added30bytes foreground-owned last-current-scan timing to shell-pwm: each
phase's TIM1 counter before/after adc_read, TIM17 elapsed microseconds,
CCR register before conversion, and raw result. CURRENTTIMING dumps only
with cap1 after safing. These bracket software/channel selection/conversion
and ISR preemption, NOT the precise sample aperture; CCR may be preloaded.
No waveform, protection threshold, reference filter or authority change.
Build/flash succeeded. Same DRV wire map, nominal12V/800mA PSU,5s limit.

bemf_currenttiming_01 used existing50->200 staircase/6.5%, planned8% observation.
Peak abort at1497759us, tick1493, command87.7eHz, theta164, BEFORE live steps.
C raw3379: PWM6208->647 (wrap at6400),13us bracket, CCR429 (6.70us).
Thus bracket includes source ON pulse; exact aperture remains unknown.
A bracket40us (includes possible preemption), B13us. Finalbus11.691V,
retained10.456..13.155V, no nFAULT. Coast fit49.15eHz/R2=.99666/order.846,
well below command87.7: suggests startup tracking loss, not a proven mechanism.
Wavegap103us,maxISR37us,maxcontrol1042us. No8% observation applied.

Tested existing run100 target to remove100->50 downward ramp, then live
110..200 in10eHz increments; startup catch7%, remaining sine6.5% unchanged.
bemf_direct100_200_01 reached all commands and actually applied obsdu80.
Runtime4723215us, observation87records/22341us, candidate-age reason6.
Real reference prefix: fresh-boundary wait824us, sixIRQs, one accepted event
and one record-only COM request at634us (step1->2), status2/gate_authority0.
First three IRQs persistence-reject after9/12/12reads; fourth accepts after12.
COMPmax105us and COMmax63us, with trace enabled. Unlike previous single-read
rejects, persistence can finish here, but different physical sector/startup
means this is NOT a controlled proof that8% pulse width fixed sensing.
Bus11.481..12.033V/no fault, sampled peaks~7.1A. Coast motion_valid FALSE,
order.308/no defensible disable-frequency fit: accepted input is NOT lock.
Host small-detector replay matches87decisions; this is not full-core replay.

Matched sine_direct100_200_01 without observation also reached all commands,
ended normally4998197us. Bus11.452..12.138V/no fault, sampled peaks~10.0A,
wavegap103us/maxISR37us/maxcontrol1481us. Coast again motion_valid FALSE,
only six edges/order.400/no frequency fit. Therefore removing the downward
ramp has NOT qualified a replacement startup; bad coast cannot be attributed
solely to the six-step handoff. Do not promote this route or the apparent
199.76Hz sparse-edge median as demonstrated200Hz rotor tracking.

All three raw/CSV/coast/plot artifacts retained, each *_safe.txt confirms
gate outputs/inputs, CCRs, MOE and ENABLE0, nFAULT1, idle. UART closed.
34Python tests pass. Next needs positive startup rotor-tracking evidence
before another sensing-duty comparison; the historical50->200 successful
route remains the only qualified200Hz open-loop route, not repeatable yet.

## Entry 062 — 2026-09-12 — Rejected mode-leak hypothesis; catch fails before ramp

Corrected a premature conversational diagnosis: run/sine do not themselves
restore TIM1 channel modes, but observation::run DOES restore0x6868/0x68/0x555
after safing. Read-only probe-rs read b32 at0x40012C18 returned exactly
00006868 00000068 00000555 after Entry061. Thus retained six-step modes do
NOT explain these failed starts; no speculative mode patch was made.

Also corrected scope of CURRENTTIMING: current_read_rotated is shared by
sine capture AND observation. When an observation executes, the final timing
belongs to its last current scan, not the final CAP sine row. In Entry061
bemf_direct100_200_01 that explains ccr=0,0,512. The other two runs had no
observation and their timing does correspond to the final sine sample.

Host fixture --duration now also supports campaign runs (previously fixed-only),
using the existing off->coast path with unchanged0.1..5s bounds. This permits
startup checkpoints without changing waveform or reflashing. Host duration
includes banner/UART latency and is not an exact firmware stage boundary;
TIMING and final capture tick remain authoritative.

startup_catch_coast_01 requested run50/6.5%, host stop0.9s, no observation or
live speed changes, same DRV mapping/12V nominal/800mA limit. It stopped EARLIER
on current guard at156460us/tick156, in100eHz/7% catch, reason4. C raw3299,
PWM6230->663 around wrap,13us bracket, CCR439. This independently repeats
the pulse-straddling trip timing seen in Entry061. Bus11.321..11.892V,
final11.672V/no fault; wavegap102us/maxISR37us/maxcontrol1041us. Coast has
only11edges/order.600, motion_validFALSE, no defensible speed fit.

The downward ramp cannot explain this particular failure: the catch itself
did not establish measured rotor tracking before its current trip. Do not
spend more identical5s trials to investigate a failure already occurring
within157ms. Need a targeted startup capture/strategy with tracking evidence,
retaining peak protection; no threshold or duty expansion justified here.
Raw/CSV/coast/plot retained; *_safe.txt verifies all gates/CCRs/MOE/ENABLE0,
nFAULT1 and idle. Serial closed.34Python tests pass. Goal still incomplete.

## Entry 063 — 2026-09-12 — 6.5% catch recovers measured200eHz rotation

Added idle catchdu<N>,1..100tenths, default70 unchanged. Ramp begins at the
selected catch duty. Fixture --catch-duty-tenths verifies acknowledgment and
restores70 on exit. Built/flashed; same100eHz catch frequency,20ms1% alignment,
50eHz target/6.5% sine, unchanged peak/bus/timing/fault guards,800mA PSU/5s.

startup_catch65_coast_01 requested0.9s host stop but transmitted only 'o':
foreground ADC/single-byte UART overran the burst. Independent firmware cutoff
ended at4999002us, reason1. This is NOT a startup checkpoint. It is a full
profile success: coast51.03eHz/R2=.99882/order1, motionvalid, bus11.472..11.852V,
sampled phase peaks~4.3A at end. Fixed host live off to byte-pace3ms like ehz;
finally also terminates partial input before paced off. No weaker safety limit.

startup_catch65_coast_02 now accepted host stop at928783us/reason3, tick926,
during100eHz/6.5% catch. No current trip, but coast motionvalidFALSE/order.278,
no credible disable-speed fit. Bus10.566..12.360V/no fault. Catch synchronization
is not proven; later downward ramp may be what captures the rotor.

sine_catch65_200_01: full50->200 staircase at6.5%, catch also6.5%, all15steps
accepted, normal5s end. Coast198.88eHz/R2=.99992/order1/motionvalid,81edges,
C-neutral39.6mV span. Bus11.111..11.887V/no fault; retained phase peaks~8.0A.
This proves the reduced-catch route CAN reach200, not statistical reliability.

bemf_catch65_200_01: same staircase plus actual8% observation, total4723419us,
84records/22341us, candidate-age reason6. Coast193.88eHz/R2=.99994/order1,
81edges/motionvalid,C-neutral47.7mV span. Bus11.500..11.873V/no fault, retained
sine peaks~10.7A. Observer fresh-boundary wait324us, fullstep1,11IRQs/no trace
drops, all first-read0 versus expected1 persistence rejects. Gate counts
901..2464 >833. Later IRQs PWM1041..1110 (~16..17us) after8us source pulse.
COMPmax45us, COM0/no accepted event; wavegap103us/maxISR46us. Unlike Entry061's
accepted input with invalid coast, this trial has strong rotor-motion evidence
but no accepted reference crossing. No BEMF gate authority granted.

Small-detector replay matches84live decisions, not full-core proof. Host tests
updated to assemble byte-paced command lines rather than assume one write per
command; pacing/termination tests added. All four runs have raw/CSV/coast/plots
and *_safe.txt confirming all gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART closed.
Next production-path work can use this6.5% catch as an explicitly selected
trial setting and investigate IRQ latency/persistence at measured rotor speed.

## Entry 064 — 2026-09-12 — Reference build optimization halves ISR service

Applied minz's release lto="thin",codegen-units=1 to binz Cargo.toml, preserving
all algorithm/filter/guard settings. This affects release builds throughout
the crate; shell-sine source remains unchanged but a rebuild uses new settings.
Flash text82344->102748bytes; data+bss31720->31716. Stack high-water not measured.

Matched driver-disabled coreirq benchmark before/after,32events each at seed
ci1667 and1333: both64/64COM, no drops. COMPmax56->29us, COMmax53->23us;
averages1663/1330 ->1666/1332. Captures coreirq_pre_lto_01.txt and
coreirq_lto_01.txt include gates/MOE/ENABLE0 safing. Synthetic interrupts,
not physical rotor/reference hardware parity. Built/flashed successfully.

bemf_lto200_01: same6.5% catch and50->200 staircase, actual8% observation.
No startup trip;45observation records/11989us then phasepeak guard, reason4,
total4712366us. Final OBS C raw3257 exceeds1200count deviation; do not
infer no peak trip from sine-only CAP current extrema. Guard unchanged.
Coast198.50eHz/R2=.99993/order1/motionvalid,82edges,C-neutral38.8mV span.
Sine bus11.431..12.007V/no fault samples. All15speed commands accepted.

Real fullstep1prefix: wait275us,12IRQs/no acceptedCOM. COMPmax20us versus
45us in prior measured-motion8%trial; waveISRmax28us versus46, gap101us.
All events still persistence-reject, but first/third/ninth now initially read
expected1 and reject after4/3/2reads as level changes0. Others reject first
read. Later typical PWMcounter~810 (12.7us), earlier than prior~1100 (17.2us).
This proves useful scheduling improvement and exposes short-lived expected
levels; it does not prove those levels are rotor crossings or justify reducing
reference persistence. Trace overhead remains part of these timings.

Raw/CSV/coast/observation/plots retained; *_safe.txt confirms gates/CCRs/MOE/
ENABLE0,nFAULT1,idle,UART closed.36Python+22Rust tests pass, frozen128reference
source hashes still match. No BEMF output authority or raised drive ceiling.
Next distinguish trace/read overhead from physical pulse persistence without
discarding reference filter or widening duty after this observation peak trip.

## Entry 065 — 2026-09-12 — Trace-free reference path still has no acceptance

Added idle coretrace0/1. Default1 preserves detailed trace.0 skips IRQ metadata
collection and per-read counter writes but keeps actual minz comp_isr,
persistence, physical-prefix checks,256IRQcap and all drive guards. Comparator
reads still check the trace-enabled flag; this is reduced overhead, not a
claim of the absolute minimum possible production path. Summary reports mode.

After build/flash initial UART was silent: blank coretrace0_preflight_01 and
bemf_notrace200_01 retain failed attempt. Fixture failed cap1 handshake before
issuing run. Readback PANIC_LINE at current symbol0x20007AD0 was0; TIM1BDTR
0x0c1a (MOE0), GPIOD ODR0 (ENABLE0). Another reset restored boot banner and
UART; cause of initial silence unproven. Disabled coretrace0_preflight_02:
64/64syntheticCOM,COMPmax27us/COM23us, outputs verified off.

bemf_notrace200_02: usual6.5%catch,50->200 staircase/6.5%sine,8%observation.
Actual CORETRACE0,19realIRQs/fullstep2prefix, no accepted event/COM, status3
physical divergence. COMPmax20us. IRQTRACE n0/drop0 correctly reflects trace
off, not absence of interrupts. Observation7records/2290us then phasepeak
abort: lastAraw628, deviation1420>1200. Total4702874us, wavegap101us/ISR28us,
maxcontrol1511us. No threshold increase or retry to force this to pass.

Coast has88edges/order1/R2=.99993/motionvalid; initial edge speed199.60eHz,
disable extrapolation212.44eHz. Their disagreement cautions against treating
the extrapolated value as exact lock; rotor motion is clear. Sine retained
bus11.255..12.109V/no fault. Trace disabling alone did not produce acceptance
in this tested sector; different sector/short abort prevents a clean timing
A/B conclusion. Current trip prevents using a longer8%window as next step.

Raw/CSV/coast/observation/plots retained; *_safe.txt verifies all gates/CCRs/
MOE/ENABLE0,nFAULT1,idle and restores coretrace1. UART closed. Remaining work
needs rotor-relative phase/sector evidence and a controlled handoff, not merely
faster bookkeeping or weaker filtering. No BEMF gate authority granted.

## Entry 066 — 2026-09-12 — Quantified handoff voltage step;6.5% observation

scripts/drv_handoff_voltage.py reads actual SINE_LUT and audits all256phase
bins with firmware integer CCR arithmetic. At6.5%sine, source-minus-sink
CCR309..362.8%sixstep drives pairCCR512:1.414..1.657times the commanded pair
difference.6.5%sixstepCCR416 still1.149..1.346times. This is NOT a space-vector,
torque, current, or rotor-angle measurement: floating-third-phase behavior,
dead-time/drop/BEMF/preload excluded. Thus8%handoff was a41..66%pair-command
increase, not a small voltage step. Two audit tests added;38Python tests pass.

bemf_handoff65_200_01 used unchanged optimized firmware, trace1,6.5%catch,
50->200staircase/6.5%sine, observation reduced8->6.5% only. All15steps accepted.
Observation86records/22394us, ends candidate-age reason6 (NO current trip),
total4722652us. Sinebus11.300..12.104V/no fault, sampled peaks~8.4A. Coast
75edges/order1/R2=.99992/motionvalid; disablefit184.07eHz, initial199.60eHz:
motion proven, maintained200Hz lock through handoff NOT proven.

Freshstep6prefix wait676us,12realIRQs/no accepted event/COM, physical-divergence
status3. First event expected0 lasts2reads then1; all others firstread1reject.
Later arrivalsPWM327..359 (~5.1..5.6us), inside nominal6.5us source pulse,
yet actual reference firstread has already returned pre-crossing level. IRQ
timestamp precedes that read; neither implies aperture timing. COMPmax16us,
wavegap101us/ISR28us, trace retained/no drops. No persistence reduction.

Reduced handoff duty avoided the recent8%peak trips in this trial but did not
establish valid reference crossings. Next needs measured rotor-relative phase
at handoff (coast/command alignment), not interpreting PWM-synchronous IRQs as
rotor events. All raw/CSV/coast/observation/plots retained, *_safe.txt verifies
all gates/CCRs/MOE/ENABLE0,nFAULT1,idle; UART closed. No firmware flash this turn.

## Entry 067 — 2026-09-12 — Command-to-coast phase anchor prepared, not flashed

Existing CAP theta is the last1kHz snapshot, not exact gate-disable phase.
Observation further separates it from disable by up to22.5ms. Old captures
therefore cannot supply a defensible rotor-relative handoff correction merely
by extrapolating the last CAP theta. No phase correction has been applied.

wave_timer::stop now latches extrapolated commanded Q32phase and TIM17time
once on active->stopped, plus sine/six-step mode; repeated idle safing preserves
the latch. start invalidates it. First coast scan records elapsed16bit timer
delay from this marker. Fixture-only PHASEANCHOR is printed after safing when
both a valid stop and coast samples exist. Existing record encodings unchanged.
This marker precedes bridge_clear/MOEclear by software latency and is NOT
physical rotor phase or exact gate-edge time.16bit delay is modulo65536us;
measurement requires first-coast latency below that. Coast comparator reads
follow sequential ADC work;2kHz sampling alone gives up to36electrical degrees
per interval at200Hz. Future phase fits must retain those timing uncertainties.

Release shell-pwm builds;38existing host tests pass (not hardware validation
of new anchor or modified stop timing). No flash/motor run this entry. Hardware
still has Entry065 build, last safed in Entry066; new binary is local only.
Next verify marker/reset behavior while disabled, then obtain a sine-only
200Hz coast reference before using any angle result to alter handoff.

## Entry 068 — 2026-09-12 — Phase anchor validated; measured200Hz sine reference

Before flashing, corrected marker bookkeeping on TIM6 guard exit: LAST was
updated before guard evaluation while PHASE still held previous tick. stop()
would miss one100us phase increment. LAST now updates after successful guard
check; guard exit extrapolates from the old LAST. Waveform arithmetic and
guard thresholds unchanged. Added driver-disabled phasecheck: duty0/MOE0/
ENABLE0, run timer~400us then stop twice, report reset/valid/stability.

Built/flashed. phasecheck twice returned reset_valid0, stopped_valid1, stable1,
phase_q32=358200081, six0,en0,moe0 identically. Transcript phasecheck_01.txt
also verifies all gate inputs/outputs0. This validates reset/latch/repeated-stop
behavior, not exact physical disable latency or ISR-deadline angle accuracy.

sine_phaseanchor200_01:6.5%catch,50->200staircase/6.5%sine,NO observation.
Normal stop4998495us; all15commands accepted. Bus11.220..11.995V/no fault,
sampled phase peaks~8.8A. Coast82edges/order1/R2=.99995/motionvalid,
disablefit198.75eHz, initial199.52eHz, C-neutral46.1mV span.
PHASEANCHOR commanded_q32=3373404263 (~282.8deg),stop_t17=46508,sixstep0,
first_coast_delay676us. First CTIME elapsed502us, so coast time-origin lies
174us after marker; marker-to-ADC-start coordinates can now be aligned.
Comparator readings occur later in each scan; this additional latency and
sample bracketing remain mandatory uncertainty terms. Do not fit a precise
rotor angle from nominal first-edge times alone. No handoff-angle change yet.

Raw/CSV/coast/plot and *_safe.txt retained; all gates/CCRs/MOE/ENABLE0,nFAULT1,
idle confirmed,UART closed. This supplies the missing command/coast anchor
for a host-side interval/phase analysis; it does not yet demonstrate BEMF lock.

## Entry 069 — 2026-09-12 — Measured sequential coast timing exposes phase offset

Coast timestamp precedes4ADCreads and three comp_read mux-settling delays
(asm::delay3000 each). Added192bytes for actual post-read offsets of A/B/C
in first32coast rows, printed cap1-only as COASTCOMP. No legacy row encoding
or sample-value changes. Release text104756,data16,bss31908 (~4.9KiB stack
space; high-water unmeasured). Built/flashed; disabled phasecheck passed again.

sine_coasttimes200_01: same6.5%catch,50->200/6.5%sine, no handoff. Normal5s
end, all15commands accepted, bus11.498..11.878V/no fault; sampled peaks~8.1A.
Coast82edges/order1/R2=.99993,198.59eHz disablefit,199.04initial/motionvalid.
Measured A/B/Cread offsets190/331..332/473us, so treating rows as simultaneous
would bias phases differently by up to34degrees at200Hz.

PHASEANCHOR q32=2095753167, firstcoast577us; firstCTIME501us -> origin+76us.
scripts/drv_coast_phase.py brackets deglitched early edges using measured read
offsets, then expresses them in a counterfactual constant200Hz command phase.
Ideal crossings use raw-comparator polarity and LUT phase offsets0/85/170.
First-cycle command-minus-ideal brackets (degrees):
C1[-49.67,-13.46], A0[-56.68,-20.54], B1[-70.78,-34.71],
C0[-48.74,-12.59], A1[-55.89,-19.67], B0[-69.92,-33.77].
These share a negative offset under this model. They are NOT a confidence
interval for rotor load angle or a calibrated correction: command extrapolation
ignores coast deceleration; analog delay, pre-MOE marker latency, LUT quantization
and read-timestamp overhead remain. Nevertheless they justify a targeted
handoff-phase hypothesis, unlike treating PWM-related IRQ arrivals as ZCs.

Parser refuses older captures without measured offsets. Regression tests cover
six edge identities, exact timestamp alignment and missing metadata rejection.
No angle correction applied yet. Raw/CSV/coast/plot plus *_safe.txt retained;
gates/CCRs/MOE/ENABLE0,nFAULT1,idle verified,UART closed.

## Entry 070 — 2026-09-12 — +30deg handoff passes persistence, not rotor lock

Added idle-only signed obsphase-60..60, consumed once at observation entry;
default0. Positive adds Q32command angle before initial step selection and
forced timer start; no phase/frequency change to sine startup. Actual OBSPHASE
printed in fixture dump. campaign::phase_shift host test checks signed/zero/
wrap behavior. Release build and13Rust library tests pass. Built/flashed;
disabled preflight rejects obsphase61, accepts30, gates/MOE/ENABLE0.

bemf_advance30_200_01: same6.5%catch,50->200staircase/6.5%sine, observation
6.5%, only handoff offset+30. All15commands accepted; actual OBSPHASE30.
86obsrecords/22509us, candidate-age reason6,total4722708us, no current trip.
Sinebus11.257..11.853V/no fault, retained phase peaks~5.7A. Coast71edges/order1/
R2=.99992/motionvalid, disablefit174.99eHz,initial166.28eHz. Rotor motion
continues, but maintained200Hz tracking through the handoff is NOT demonstrated.

Freshstep3prefix wait277us: firstIRQ at12us, gate864>833, all12actualreference
reads1(expected1), EV_ACC1. One record-only COM request step3->4 at264us,
then prefix stops on requested divergence(status2). Two subsequent already-
pending IRQ services blank-reject, no extra acceptance. COMPmax44us/COM21us,
wavegap101us/ISR28us. Gate authority remains0.

This contrasts with zero-offset all-reject trials but is not a matched-sector
repeat or valid crossing certificate. An acceptance12us after boundary is
consistent with an already-post-crossing level or transition artifact; seeded
interval initialization opens the reference gate immediately. Do not mistake
12persistent reads for a measured rotor ZC or grant control from this result.
Full event timing/phase correspondence and repeatability remain necessary.

Raw/CSV/coast/observation/plots retained, obsphase30_preflight_01 transcript,
and *_safe.txt verifying all gates/CCRs/MOE/ENABLE0,nFAULT1,idle. Explicit
obsphase0 clears any residual setting; UART closed. No filter/guard changes.

## Entry 071 — 2026-09-12 — Reference-consistent seed restores initial blanking

Source audit found an inconsistent modeled initial trajectory: observer set
elapsed-since-ZC=ci/2 at fresh commutation, while its programmed COM wait was
ci/4. Actual reference resets interval at acceptance and commutates after
wait_time+1. With TEMP_ADVANCE16,ci1666:advance416,wait417,elapsed418ticks.
Old833tick seed opened the >833 blank gate almost immediately, contributing
to early boundary acceptance. Both are assumptions, not measured prior ZCs.

Initialize wait using actual minz advance_of/wait_time and live-IRQ interval
using wait+1, including fresh physical boundary. Polling interval seed remains
ci/2. CORESEED exposes assumed wait/elapsed. No edits to minz filter, algorithms
or drive guards. New host full-sequence test: postlevel at12us after modeled
COM stays blank/pending; atelapsed834 it may accept, retaining reference
camp-on-pending behavior. Ten sequence tests and release build pass.

Built/flashed, seedwait_preflight_01 outputs off, obsphase30 armed.
bemf_seedwait30_200_01: same6.5%catch/staircase/observation,+30deg. All15speed
commands accepted, total4723317us,84obsrecords/22210us then candidate-age
reason6. No current trip. Bus11.436..11.945V/no fault, retained sine peaks
~10.2A. Coast71edges/order1/R2=.99995/motionvalid,174.15eHz disablefit,
166.28initial: again no maintained200Hz handoff lock.

Fullstep1prefix,27IRQs/no acceptance/COM,stopphysicaldivergence. First13us
event gate448<833 is now correctly blanked. Repeated pending postlevel services
remain blank rather than accepting it immediately. Later input fails reference
persistence. COMPmax32us, wavegap101us/ISR28us. Different observed sector from
Entry070 means not a controlled causal proof that seed alone removed acceptance;
the initialization inconsistency and corrected blank timing are independently
source/host/hardware evidenced. More acceptance is not the optimization target.

Raw/CSV/coast/observation/plots and *_safe.txt retained; gates/CCRs/MOE/ENABLE0,
nFAULT1,idle verified, obsphase0 cleared,UART closed. This removes a permissive
observer initialization, but rotor-derived startup state and BEMF lock remain
unproven. No BEMF gate authority or duty-ceiling expansion.

## Entry 072 — 2026-09-12 — Drive-direction/edge-parity mismatch found in HAL

Source comparison: binz sixstep::plan follows rm32 A>B,C>B,C>A,B>A,B>C,A>C.
Actual minz tim1_motor_pwm::set_roles_for_step HIGH=[0,0,1,1,2,2],
LOW=[1,2,2,0,0,1] gives A>B,A>C,B>C,B>A,C>A,C>B: opposite rotation order.
Its com_step comment claiming direct rm32 equivalence is not authoritative
over that executable table. Both use raw neutral-INP/phase-INM comparator,
but minz-core expects logical postlevel odd1/even0. Reusing raw minz polarity
unchanged with binz's opposite drive sequence is inconsistent.

scripts/drv_polarity_geometry.py audits actual sine LUT slope at each floating
phase neutral crossing for binz's six drive pairs. All six give physical raw
postlevel[0,1,0,1,0,1], opposite reference[1,0,1,0,1,0]. This is command-geometry
evidence, not an assertion of measured rotor angle or validation of every wire.
41Python tests pass, including six-phase slope regression.

Prepared live-IRQ adapter correction: Comp.output_level inverts raw only for
LIVE_IRQ; physical EXTI edge selection likewise uses !reference_rising. Keep
gate map, physical phase order, raw observation/coast bits and minz-core source
unchanged. COREPOL explicitly labels IRQ trace levels as reference-HAL values.
Synthetic disabled benchmarks retain their original polarity path. Polling
and small raw diagnostic detector are not adapted by this live-only change;
they must not be claimed as parity-qualified. Existing candidate-age backstop
remains unchanged and may still terminate a successful live prefix.

Release build succeeds. NOT flashed or bench-tested this entry: hardware
remains Entry071 firmware and last verified off there. Next validate physical
edge/level inversion on the bench, then repeat zero-offset200Hz observation;
do not combine this polarity correction with the speculative+30deg offset.

## Entry 073 — 2026-09-12 — Corrected live polarity accepts after reference blank

Flashed Entry072 live-only HAL level/edge correction. First reset left UART
silent (blank polarity_preflight_01 retained); no drive command sent. Second
reset with port open restored banner. polarity_preflight_02 ran obstiming200
with ENABLE0; COREPOL inverted1, real interrupts serviced, then all gates/
MOE/ENABLE0 verified. Idle acceptance is not motor evidence or a full edge
polarity qualification; command geometry and subsequent physical trace supply
the relevant evidence. Initial reset-silence cause remains unproven.

bemf_polarity200_01:6.5%catch,50->200staircase/6.5%sine,6.5%observation,
actual phase offset0. All15steps accepted, no current/fault abort.85obsrecords/
22500us, total4723096us, candidate-age reason6. Sinebus11.373..11.937V/no fault,
sampled peaks~9.6A. Coast76edges/order1/R2=.99992/motionvalid,184.25eHz
disablefit,199.52initial. Motion continues; handoff maintaining200Hz not proven.

Fullstep6reference prefix,12IRQs: first11 gate counts448..820 below833, blank
camping on pending postlevel (one prelevel read at105us). At215us gate854>833,
all12reference-HAL reads0(expected0), acceptedEV_ACC. Raw level is inverted
only at HAL; these trace zeros mean physical raw1. COM request step6->1 at470us,
then record-only prefix stops(status2). COMPmax46us/COM27us,wavegap102us/ISR28us.
The earlier all-reject behavior is no longer present in this sector. However
acceptance as the gate opens can represent an already-post-crossing level;
not proof of a rotor-timed ZC or sustained reference tracking. No gate authority.

Raw/CSV/coast/observation/plots retained plus *_safe.txt: all gates/CCRs/MOE/
ENABLE0,nFAULT1,idle,phaseoverride0,UART closed. Next integration must propagate
the direction/polarity convention consistently through polling/diagnostic
paths and validate a measured-state transition, not bypass reference blanking
or award lock from a single accepted prefix.

## Entry 074 — 2026-09-12 — Consistent physical-observer polarity; versioned replay

Prepared explicit PHYSICAL_OBSERVATION context for Comp.output_level: real
IRQ, timer polling and sampled polling all invert physical raw into reference
polarity. Synthetic corebench/coreirq clears this context and retains synthetic
semantics. Sampled polling still stores raw LEVEL, so inversion happens once
at the HAL boundary. Gate mapping and reference source untouched.

Small diagnostic detector also consumes !late_raw. This changes diagnostic
decisions and thus candidate-age progress, so observation wire bumpedv7->v8.
Raw comparator/PWM masks and all16wire words unchanged; decision flags now
refer to corrected input. Python retains raw late_on and adds separate
reference_late_on. Both small-detector and full-polling Rust replay invert
onlyv8, preserving historicalv7 behavior. Future hardware may run longer if
new valid diagnostic candidates refresh the unchanged22.5ms watch;96ms
observation and5s total drive bounds remain. Candidates still are not lock.

Release builds;42Python tests and24Rust tests pass. Decoder tests coverv1-v8,
rejectv9, and confirm identical rawbits/different reference level. Historical
bemf_polarity200_01_obs.csv replay still matches all85old live decisions.
No flash or hardware run this entry; board remains Entry073 build, last verified
off there. v8 hardware decisions and corrected polling require bench validation.

## Entry 075 — 2026-09-12 — v8 bench/replay match; repeated step6 acceptance

Flashed Entry074. Opened UART before reset; boot and off preflight succeeded
without second reset (not proof of prior reset-silence cause). Capture
polarity_v8_preflight_01 verifies phaseoffset0 and all outputs disabled.

bemf_polarity_v8_200_01: unchanged6.5%catch,50->200staircase/6.5%sine and
6.5%observation, offset0. All15commands accepted.85obsrecords/22498us, total
4723234us, candidate-age reason6/no current trip. Bus11.434..11.862V/no fault,
retained sine phase peaks~7.9A. Coast76edges/order1/R2=.99993/motionvalid,
186.42eHz disablefit,199.52initial; not maintained200Hz handoff lock.

Corrected physical-HAL polarity reported1. Freshstep6prefix repeats previous
behavior:12IRQs, one accepted input and record-only COM step6->1 at473us
(prior470us), status2. COMPmax47us/COM27us,wavegap101us/ISR28us. Repeating
this one sector does not establish all-phase acceptance or controller lock.
Small corrected detector reports zero candidates and times out; host v8 replay
matches ALL85live decisions exactly. No interpretation of rawbits was changed.

Raw/CSV/coast/observation/plots retained; *_safe.txt verifies gates/CCRs/MOE/
ENABLE0,nFAULT1,idle;UART closed. v8 capture/replay now bench-validated, while
corrected polling still needs its own test. Next observe reference prefixes
at selected fresh sectors rather than repeatedly sampling whichever sector
happens to follow handoff. Preserve causal-divergence stop and no gate authority.

## Entry 076 — 2026-09-12 — Sector selector built; UART failure prevents trial

Added idle obssector0..6, one-shot consumed at observation begin.0 selects
next fresh sector (old behavior);1..6 waits for that chosen physical boundary
before starting exactly one reference prefix. No changes to forced waveform,
timeouts, authority or divergence stop. COREBOUNDARY records selected_sector.
Release builds/flashes. Sector1 trial was intended, but NOT run.

Two reset/preflight attempts produced only truncated boot bytes, no command
acknowledgments (sector1_preflight_01/_02 retained). No run command issued.
Read-only SWD: PANIC_LINE0 at current symbol0x20007B94; ICSR0 (thread mode),
DHCSR0x01000001; USART3CR1=0x0d,CR2/3=0,BRR=0x22b,ISR=0x00600010.
TIM1BDTR0x0c1a hasMOE0; GPIOD ODR0 hasENABLE0. Cause of boot/UART stall remains
unproven; recurrent initial UART silence now did not recover on second reset.
Attempted probe-rs debug command rejected attach serialization before providing
CPU registers; no live debugger handle remains. UART connections closed.

Do not infer hardware BEMF results or selector validation from this turn.
Next restore a responsive shell using disabled-driver diagnostics before any
motor attempt. Existing selector code is flashed but unqualified on hardware.

## Entry 077 — 2026-09-12 — UART stall localized to boot TX polling

No motor command or flash this entry. Serial off probe returned no bytes.
Read-only CPU inspection via probe-rs GDB stub located PC=0x080072a0,
SP=0x20008a08, LR=0x08017ee7, r3=0x4000481c, r2=0x00600010.
Current ELF addr2line/disassembly maps PC to HAL USART3 TXE polling inside
the boot-banner write at shell-pwm.rs:818. r6=4 is the banner byte index.
This is before the shell loop; not an observation or motor-control stall.
SP is inside available RAM, not evidence that total stack headroom is qualified.

GDB tooling needed two workarounds: explicit IPv4 listener (default advertised
both addresses but only IPv6 listened), and direct RSP register decoding.
GNU GDB rejected the stub target XML and, with XML disabled, register layout.
scripts/debug_rsp_registers.py reads/checksums/expands RSP run-length packets;
use only with driver already disabled. It leaves CPU halted. Stub exits when
the diagnostic disconnects; all server sessions confirmed terminated.

After inspection, SWD readback CCR1/2/3/4=0, BDTR=0x0c1a (MOE=0),
GPIOD IDR/ODR=0 (ENABLE=0); RCC CCIPR/CCIPR2=0. No motor attempt.
UART initialization/kernel clock/peripheral state remains to diagnose; this
entry proves the blocking instruction, not its underlying cause. No reset or
speculative firmware workaround applied.

## Entry 078 — 2026-09-12 — UART recovered by restoring peripheral clock

RCC APBENR1 at0x4002103c read0x08000000: DBG clock on, USART3EN(bit18)
off. Local HAL enable.rs maps USART3 to APB1 bit18 and BasicConfig startup
calls enable. USART3 CR1=0x0d, BRR=0x22b, ISR=0x00600010 while boot stalled.
With bridge disabled, wrote only missing USART3EN into observed register:
probe-rs write b32 0x4002103c 0x08040000 (same G071/probe selector).
Immediately received remaining banner, then successful off/p/i acknowledgments.
No reset, flash, firmware change or drive command needed for recovery.

Missing USART3 clock is experimentally confirmed proximate cause. What cleared
it is NOT yet proven. A subsequent independent probe-rs read returned08040000
and shell remained responsive, so ordinary attachment did not reproduce loss.
Installed probe-rs is0.28.0 git v0.27.0-159-g3c10cd38; cached0.32.0 source
uses read-modify-write on this register for debugger clock. A reset/startup
race is a hypothesis, not an established tool bug. Do not blindly restore this
constant during motor operation: other APB1 timers must be preserved.

Safe serial readback: all six gate outputs/inputs0, CCRs0, MOE0, ENABLE0,
nFAULT1. obssector7 correctly rejected, obssector1 acknowledged, phaseoffset0.

## Entry 079 — 2026-09-12 — Selected sector1 accepts reference prefix at200eHz

bemf_sector1_200_01: current Entry076 firmware, unchanged board/wiring,
800mA PSU limit,6.5%catch and sine, all15live50->200 step acknowledgments,
6.5%observation,phaseoffset0,selectedsector1. No BEMF gate authority.
Sine retained bus11.365..12.139V,no fault samples,phase pulse extrema up to
11.07A (not average current). No phase-current guard trip; reason6 tracking
timeout.85observationrecords over22399us; waveform47227updates,maxgap103us,
maxISR28us. Under5s energized bound retained.

Selector waited4578us for freshsector1. First11reference IRQ calls blanked
(gate449..820 vs833). At216us gate855,12/12 reference-level1 reads passed;
record-only COM requested1->2 at471us,then prefix status2 stopped. COMPmax46us,
COM27us. This extends prior step6 result to opposite-parity step1. Acceptance
right after blanking still does NOT establish rotor-timed ZC or sustained lock.

Coast initial estimate200.00eHz,disable-fit184.97eHz,R2=.99993,phaseorder1,
motionvalid. Handoff sustaining200eHz not proven. v8 host replay matches all85
diagnostic decisions (zero candidates). Raw,CSV,coast,observation,plots and
*_safe.txt retained. Explicit off/obssector0/p/i verified all gate outputs and
inputs0,CCRs0,MOE0,ENABLE0,nFAULT1,idle. UART closed. Next qualify remaining
selected prefixes and address actual tracking rather than treating acceptance
or the absence of hardware faults as closed-loop success.

## Entry 080 — 2026-09-12 — Remaining sector prefixes pass; coverage is not lock

Four actual motor runs, bemf_sector{2,3,4,5}_200_01, current Entry076 firmware,
same G071/DRV8304 wiring, 800mA supply limit. Each used6.5%catch/sine and
6.5%observation, phaseoffset0, all15 acknowledged50->200 live steps. No flash,
new duty ceiling or BEMF gate authority. Each remained below5s energized and
ended on existing diagnostic-candidate timeout reason6, not nFAULT/current.

| Selected sector | COM request us | Next step | Sine bus V min..max | Peak phase A (pulse) | Coast disable-fit eHz |
|---|---:|---:|---|---:|---:|
| 2 | 471 | 3 | 10.662..13.155 | 9.74 | 185.89 |
| 3 | 473 | 4 | 11.361..11.846 | 7.22 | 185.30 |
| 4 | 471 | 5 | 11.467..11.908 | 7.55 | 186.79 |
| 5 | 472 | 6 | 11.366..12.030 | 9.87 | 186.29 |

All selected prefixes:12IRQs,one accepted reference input,one record-only COM,
status2,COMPmax47us/COM27us. Wave maxgap101..104us,ISR28us. Coast initial
estimates199.92..200.00eHz,order1,R2>=.99992,motionvalid. These estimates do
not prove maintained200Hz through handoff. Sector2 bus spread is wider than
other runs but above existing8.4V abort; no expanded envelope inferred.

Together with Entries075/079, physical prefix acceptance has now been observed
in all six sectors, on all three comparator selections and both polarities.
Near-identical request times471..473us expose the important remaining limit:
the expected comparator level is already present before the assumed blank gate
opens. This validates the HAL/filter/timer execution path, not a measured prior
ZC seed or rotor-following trajectory. Do not repeat these prefixes as a lock
test. Next integration needs actual accepted-event timing across steps and
causal physical/request divergence handling, first through simulated HAL;
independent loss-of-tracking protection must follow production accepted events,
not simply relax the small diagnostic detector to make its timeout disappear.

Each retained85observationrecords; all340 live v8 diagnostic decisions match
host replay, zero diagnostic candidates. Raw/CSV/coast/observation/plots and
per-run *_safe.txt retained. Post-run serial off/p/i verified all six gate
outputs AND inputs0,CCRs0,MOE0,ENABLE0,nFAULT1 after every attempt. Final
obssector0 acknowledged,UART closed. No live process or debugger remains.

## Entry 081 — 2026-09-12 — Synthetic post-level feedback defeats freshness alone

No hardware access, flash or motor run. Extended existing full-reference
logical-time HAL test with the missing adversarial sequence: expected level
already present after EVERY synthetic COM, no independent rotor crossings.
Same1666tick initial interval and417tick wait/fresh-COM seed as bench prefixes;
real minz COMP persistence, COM blend/scheduling and main recovery bands run.
Zero-cost simulated comparator reads remain a limitation vs measured M0 time.

Twelve accepts produce average_interval1666,1632,1545,1431,1302,1161,1008,
879,790,716,648,583 ticks. First inter-COM gap782ticks(391us),last312(156us).
An independent2500us accepted-event freshness watchdog never trips, because
events keep arriving. Reference desync FIRST trips at COM12,sets runningfalse
and pollingtrue; regression asserts this, not a false claim of indefinite
desync immunity. This is a conditional failure-mode demonstration, NOT proof
the actual bench comparator is dominated by switching artifacts.

Consequence: do not replace diagnostic-candidate timeout with accepted-event
count alone and call that tracking. Handoff qualification needs independent
rotor-related timing/progress evidence; repeated first-sector reseeding would
hide the interval collapse this simulation now exposes. Existing observation
stops at first counterfactual COM and all hardware guards remain unchanged.

All25host Rust tests pass (13library,11sequence,1recorded polling). Frozen
reference archive/source verification passes; shell-sine source unchanged.
Updated observer-replay README with exact model limits and measured synthetic
values. Hardware remains last Entry080 verified off; no handles opened.

## Entry 082 — 2026-09-12 — Corrected-polarity +30deg handoff does not fix early acceptance

Source review: observation STEP_LUT chooses max/min of actual SINE_LUT and
uses the established rm32 pair sequence. No obvious sector-table mismatch
found; this is not independent physical rotor-angle calibration. Prior coast
phase brackets motivated a controlled +30deg experiment, now with corrected
HAL polarity (earlier phase trials predated that correction).

bemf_sector1_shift30_200_01: same current firmware/wiring,800mA PSU limit,
6.5%catch/sine/observation,50->200 live staircase,selectedsector1,one-shot
phase+30. Boundaries unchanged:5s drive,8.4V sine bus floor,current/nFAULT,
96ms observer and22.5ms diagnostic-event timeout. BEMF gate authority0.
All15frequency steps acknowledged;85obsrecords/22214us,reason6,no fault or
phase-current trip. Sine bus11.491..11.999V,retained phase pulse peak5.60A.

First prefix raw-inverted level0 at13us then expected1 from31us onward;
accepted at217us(gate856,12reads),COM1->2 at472us,status2. Zero-offset
Entry079 accepted216us/COM471us: +30 did not move acceptance meaningfully
away from the assumed blank gate. COMPmax46us/COM27us,wavegap105us/ISR28us.
Coast disable-fit174.87eHz vs zero-offset184.97;initial166.67,order1,
R2=.99995,motionvalid. This is evidence against adopting+30 as a handoff fix,
not proof of comparator artifact dominance or calibrated rotor deceleration.

Host v8 replay matches85/85 diagnostic decisions,zero candidates. Raw/CSV/
coast/observation/plots/preflight/safe retained. Explicit off,obsphase0,
obssector0,p,i confirmed all gates/CCRs/MOE/ENABLE0,nFAULT1,idle;UART closed.
Do not promote the phase-shift hypothesis into a firmware default. Subsequent
tracking work must distinguish the switching-correlated early level from
rotor-related timing without counting this repeated prefix as new lock proof.

## Entry 083 — 2026-09-12 — Diagnostic timeout cannot establish tracking loss at200eHz

Read-only analysis of sector1..5 zero-offset captures and sector1_shift30.
Detector requires at least FOUR consecutive valid samples per sector: two
opposite to arm and two expected to accept. Invalid resets its baseline;
sector change resets its entire state. Actual slow observation capture at
200eHz supplies at most THREE consecutive valid samples per complete visit.
scripts/drv_detector_capacity.py uses recorded decision_reason==Invalid to
audit exact live validity, not a guessed PWM/blanking validity mask.

Zero-offset captures:26complete visits each;shift30:25. ALL155complete visits
have max-valid-streak<=3 and zero visits with capacity for a candidate,
irrespective of underlying analog quality. Unit tests cover insufficient
samples, invalid gaps, and capacity not implying an actual crossing.
All45Python tests pass. --details emits visit records;default summaries quiet.

Correction to repeated prior 'tracking timeout' wording: reason6 is the slow
DIAGNOSTIC detector's age timeout. For these traces its inability to produce
events is structural; it cannot diagnose rotor loss or refute the actual COMP
filter's accepted events. Independent coast/handoff timing concerns remain.
Conversely reference-event freshness alone is insufficient (Entry081).

Only explanatory comments changed in observation.rs; no executable behavior,
guard, waveform, threshold or firmware flash changed. Keep the present bounded
abort until a qualified production-event timing monitor replaces this
diagnostic dependency. Do not increase sparse ADC/sample requirements or tune
the small counter to manufacture passes. Next implementation should separate
production comparator timing from slow diagnostic capture and qualify that
monitor against genuine rotor-related timing plus missing/spurious events.
No motor run or serial/debug handle this entry; last measured safe Entry082.

## Entry 084 — 2026-09-12 — Accepted-event envelope monitor passes reference simulations

Added allocation-free accepted_timing.rs, linked into host/M0 replay library,
NOT yet into shell-pwm. Consumes individual timestamped accepted events with
logical1..6sector order; external min/max interval limits, independent of
controller average_interval. Missing/late,too-fast,and wrong-sector faults
latch. First event establishes phase only; it cannot establish interval/lock.
Caller must poll without events and feed acquisition timestamps,not batched
main-loop arrival times. No peripheral or gate-control access.

Host mock now timestamps actual reference Recorder events. Using test-only
1333..2000half-us interval bounds (~167..250eHz), monitor passes all120actual
EV_ACC events from clean synthetic200eHz reference train and flags stale at
last+2001ticks. Entry081 post-level sequence flags TooFast at SECOND accepted
event,versus reference desync at COM12. This does not change the hardware
envelope or claim plausible in-band false events are detected.

Four unit tests cover clock/sector wrap,absent first event,late-event latch,
inclusive bounds,fast-event latch,duplicates,skips,and invalid sectors.
All29Rust tests pass; replay library compile-check for thumbv6m passes.
No flash/motor/serial/debug work; hardware remains last measured off Entry082.
Next integrate event acquisition/monitor reporting observe-only before using
it as an abort source. A stopped causal-prefix observer must not masquerade
as a continuously observed stream or silently reset monitor history per sector.

## Entry 085 — 2026-09-12 — Observe-only accepted acquisition integrated, not flashed

shell-pwm now buffers four timestamped actual EV_ACC records independently of
optional per-read IRQ tracing.32byte payload storage plus counters. Only live
IRQ prefix writes it; timestamps taken in Recorder callback relative to fresh
sector start, not foreground poll time. This is acceptance-record time, NOT
the physical zero-cross instant. Current bounded prefix is below one16bit
TIM17 wrap; do not reuse that assumption for a longer continuous observer.

First live_stop freezes observed-end time; later gates_off does not extend it.
After bridge disable, summary feeds shared accepted_timing monitor with
explicit report-only1333..2000tick test bounds. Reports measured interval
count, fault, observed end,continuous=0,lock_proven=0. One accepted event has
ZERO measured intervals and cannot qualify a train. No monitor output affects
gates or replaces the unchanged diagnostic timeout. Prefix causal-stop remains.

Fixture cap1 alone emits ACCEPTLOG/A85 acquisition records (4u16+CRC32,
Ascii85). Default/timing-only commands emit no binary event records. Host
drv_accepted_events.py validates CRC/length/count/sector and rejects overflow.
All48Python tests pass, release shell-pwm builds with existing warnings.
ISR acquisition cost/stop timestamp and actual wire output still need hardware
qualification. No flash or motor run this entry; board remains Entry076 build,
last outputs verified off Entry082. New ELF is built but not deployed.

## Entry 086 — 2026-09-12 — Accepted-event log bench validated at200eHz

Flashed Entry085 after serial off/p/i verified safe. First reset produced full
banner and normal shell; no UART clock recovery needed. Same G071/DRV8304,
unchanged wiring/800mA supply limit. bemf_acceptlog_200_01:6.5%catch/sine/obs,
all15acknowledged50->200 steps,sector1,phase0; unchanged5s/current/bus/nFAULT
and diagnostic timeout guards. All85obsrecords retained;reason6,no fault or
phase-current trip. Sine bus11.444..11.850V,phase pulse peak10.80A (not average).

ACCEPTLOG n1/drop0; CRC-valid A85 acquired at253us,step1,reference interval
913half-us ticks. IRQ entered211us,gate848>833,12reads,tracecost47us. Callback
timestamp is later than entry as expected, not physical crossing time.
COM request467us,first stop frozen493us. Report-only monitor fault0,intervals0,
continuous0,lock_proven0. Does not invent a stale event during remaining22ms
of forced drive after causal prefix stops. Retained-capture decoder regression
checks these acquisition/scope fields.

COMPmax50us vs prior46..47,COM27us; wave maxgap100us/maxISR28us. Difference
is whole-build observed cost,not an isolated cycle benchmark of the logger.
Coast initial198.97eHz,disable-fit186.93,R2=.99993,order1,motionvalid; no lock
inferred. All85v8 diagnostic decisions replay exactly;49Python tests pass.

Raw/CSV/coast/observation/plots/preflight/safe files retained. Explicit off,
obssector0,p,i verified six gate inputs/outputs0,CCRs0,MOE0,ENABLE0,nFAULT1.
UART closed. Current hardware now Entry085 build. Acquisition primitive is
bench validated, but continuous production-event monitor/rotor qualification
and BEMF gate authority remain unfinished; no further single-prefix repeats
are needed to prove this log format.

## Entry 087 — 2026-09-12 — Measured coast supports multi-step virtual sequence study

Added reference-HAL coast sensitivity test using Entry086's measured three
comparator levels with per-read COASTCOMP offsets,not simultaneous scan times.
Only32rows/96samples have measured offsets; no later timestamps fabricated.
Unlike energized floating-phase replay,bridge-disabled all-phase input permits
virtual step changes without claiming the motor followed counterfactual gates.
Actual reference COMP/COM/blend/main bands execute; fixed1666tick interval and
418tick prior-event seed are explicit assumptions. All six initial sectors run.

Sample-and-hold outcomes (seed:accepts/COM,final average ticks,stop):
1:14/14,1850,end;2:5/5,2641,polling;3:4/4,2770,polling;
4:4/4,2996,polling;5:16/16,1842,end;6:15/15,1844,end. Desync0 throughout.
No favorable seed promoted to physical lock. Sparse held levels,zero-cost
persistence reads and sample-cadence IRQ service do NOT reproduce real edge
timing or mux-change transients. Test asserts source timestamp/order and
causal event/commutation invariants,not a hardware lock certificate.

All30Rust tests pass. No firmware edits/flash or hardware handles this entry;
last measured safe state Entry086 remains. This identifies a useful next
experiment: after proven spin,disable bridge and run actual comparator/reference
sequence with virtual COM selecting only the sense mux,never gate drive. That
can qualify continuous production interrupt timing on real coasting BEMF before
attempting motor-control authority. Scope must remain bounded,log accepted
events and stop/mode transitions,and keep ENABLE/MOE/gates off throughout.

## Entry 088 — 2026-09-12 — Continuous coast-only reference path built, not flashed

Added idle coastref0..6:0cancels,1..6arms a one-shot initial virtual sector.
On next NORMAL completed drive only(reason1,target167..250eHz),after all
outputs disabled and before normal coast sampling,actual COMP/COM reference
sequence runs for<=20ms. No synthetic comparator reference,slow ADC sampler
or repeated per-sector seed. Virtual COM updates reference state and actual
comparator mux only; Output HAL still cannot drive any gates. Actual polarity
adapter retained. Existing powered-drive envelope/guards unchanged.

Coast loop requires ENABLE/MOE/all six input readbacks low before entry and
checks them continuously. Stops:1duration,2polling changeover,3reference
runningfalse,4bridge-state mismatch,5nFAULT,6reference status/IRQ backstop.
Does NOT implement polling in this first coast test. Existing256IRQ backstop
remains. No waveform timer runs. Exit stops IRQ/timers,restores comparator CSR,
reasserts gates/ENABLE off. Full scope timestamps kept belowTIM17wrap.

Added idle coastcheck: same20ms/sector1/200eHzseed path with bridge disabled,
for no-spin preflight. Does not enable driver. Next hardware action should
qualify coastcheck before using coastref during a spin. cap1 alone enables
binary event records. Acceptance capacity increased4->32(224extra payload
bytes); overflow counted/host rejected. continuous=1 denotes uninterrupted
measurement scope,NOT lock; report-only timing envelope does not abort drive.

Normal coast samples are delayed by this test and carry their real elapsed
timestamps; COASTREF labels that delay. Do not compare its first coast edge
to an undelayed baseline as if both were sampled immediately after gate-off.
No high-fidelity control-authority claim from compile/host tests.

Release builds: text108516,data16,bss32200 (~4.6KiB remainingRAM before stack
qualification).30Rust and49Python tests pass; these retain existing model/
decoder coverage,not hardware qualification of the new coast branch. No flash
or motor run this entry. Hardware remains Entry085 build,last safe Entry086.

## Entry 089 — 2026-09-12 — Coast-only hardware sequence produces21accepted events

Flashed Entry088 after verified off/p/i;first reset normal. coastcheck_01
disabled-driver preflight:zeroaccepts,257IRQs,stop6/status5 IRQ backstop,
observed4114us,monitor stale. Gate-state check remained1;post p/i confirmed
gates/CCRs/MOE/ENABLE0,nFAULT1. Idle comparator IRQ activity is not rotor BEMF.

coastref1_200_01: actual spin with6.5%catch/sine,all15live50->200steps,800mA
PSU limit,normal5s exit(reason1);obs0,coastref1. No powered-drive envelope
changes. Bus11.337..11.836V,phase pulse peak9.22A,no nFAULT/current trip.
Wave49990updates,maxgap101us/ISR28us. Then full actual comparator/COMP/COM/
main reference bands on coasting motor,bridge disabled,no virtual gate output.

21CRC-valid accepted records and21virtual COMs,logical1..6order repeated;
107IRQs,COMPmax50us,COM24us. Acceptance times start238us,1573us,2367us and
continue to17887us. Initial gap1335us;remaining19gaps775..959us. Final
average1734half-us ticks,desync0,polling0,running1. Stop1time bound,
observed end20008us,gate-disabled check1. Monitor fault1 is valid: initial
1335usgap already exceeds report-only1000usmax;last accepted-event age2121us
also exceeds it. No claim of fully tracked20ms interval or closed-loop drive.

This is continuous measured rotor-generated input through actual reference
sequence,not a single-prefix acceptance or sample-held synthetic replay.
Long first gap and late loss of accepted progress remain to explain.21events
and unchanged desync alone are not a lock certificate. Do not relax monitor
to turn this run green. Next compare repeatability/seed sensitivity and inspect
late pending/mux/reference state to distinguish missed edges from rotor decay.

Delayed ordinary coast starts~20ms after disable,initial fit199.12eHz and
disable extrapolation198.40,R2=.99989/order1/motionvalid. These fits have a
larger blind early interval than prior ordinary coast captures; not direct
measurements of rotor phase during the comparator test.

Raw/CSV/coast/plots and *_safe.txt retained;50Python tests pass,including
retained21eventorder/gaps/terminal-age regression. Explicit off/coastref0/p/i
verified allgateinputs/outputs0,CCRs0,MOE0,ENABLE0,nFAULT1. UART closed.
Current hardware now Entry088 build. No powered BEMF control authority.

## Entry 090 — 2026-09-12 — Repeat exposes duplicate accepts before virtual COM

Added fixture-only COASTSTATE snapshot at first active live_stop, before
normal IRQ shutdown: logical step/expected,raw CSR,pending/IMR,interval count,
COM DIER,last IRQ time/read outcome,software masked.48byte state plus last-IRQ
timestamp. Timestamp store is coast-only; no powered waveform/guard changes.
Build/flashed after verified off;normal boot/preflight. On IRQ-count exit the
existing backstop masks/clears EXTI BEFORE live_stop; thus that stop snapshot
cannot reconstruct pre-backstop pending/mask state. Do not infer it can.

coastref1_state_200_01:same6.5%catch/sine,all15steps50->200,800mA limit,
normal5s exit,sector1coast. Powered bus11.502..12.085V,phase pulse peak8.29A,
no current/nFAULTtrip. Coast bridge state remained disabled throughout.

17CRC-valid accepts but only9virtual COM,not previous21/21. Accepted logical
sectors1,1,2,3,3,4,5,5,6,6,1,1,2,2,3,3,4. First1157us,then1624us same
sector1. Subsequent same-sector pairs recur at~320..467us. This violates the
intended one accepted input per commutation window; cannot call it rotor lock.
At8914us stop6/status5,257IRQs hit bounded IRQ backstop. COMPmax51us/COM23us,
average1036ticks,desync0,polling0,running1. Monitor fault1 latched initial
late event; it does not separately report subsequent wrong-sector faults.

Terminal snapshot:step4 expected0,CSR1073742465,pending0,IMR4294443008,
interval418,COM DIER1,lastIRQ8896us,lastgate391/one level0read,masked1.
Masked/pending fields reflect IRQ-count backstop as noted above. COM remained
armed when the test stopped. Precise duplicate-accept mechanism unproven;
next inspect shared-IRQ dispatch vs EXTI/software masking and pending-COM
state. Reference comp_isr checks pending/level but relies on its adapter to
dispatch only legitimate source interrupts. Do not paper over duplicates by
filtering the log,raising IRQ cap or relaxing event timing.

Delayed coast indicates continued rotor motion(fit198.97eHz/order1),not proof
of reference tracking. Raw/CSV/coast/plots/preflight/safe retained. Explicit
off/coastref0/p/i verified six gateinputs/outputs0,CCRs0,MOE0,ENABLE0,nFAULT1.
UART closed. Hardware now this snapshot build; no gate authority granted.

## Entry 091 — 2026-09-12 — Masked-dispatch regression and adapter guard prepared

Source confirms reference comp_isr tests EXTI pending and comparator level,
not interrupt-enable state (a hardware dispatch precondition). G071 shared
ADC_COMP adapter previously called it without checking software MASKED or
EXTI18 IMR. Entry090's duplicate accepts are compatible with bad masked/queued
dispatch, but existing snapshot does not prove that is the physical cause.

Host regression explicitly delays COM service past its deadline and injects
a queued invocation with pending set while comparator masked. Unguarded
reference call produces second EV_ACC in same sector and overwrites scheduled
COM deadline. New shared irq_dispatch qualifier rejects it,retaining one
accepted event and original deadline. This is a modeled causal mechanism,
not claimed reproduction of exact MCU interrupt source/latency.

Added qualifier to real-IRQ G071 adapter:dispatch requires software unmasked,
hardware EXTI18 enabled,and pending. Skips are counted separately in
COREDISPATCH (software_masked,hardware_masked,not_pending). Does not clear
pending inputs or change reference filter/blanking/COM decisions. Original
256IRQ backstop remains before qualifier so unexpected repeated shared IRQs
still terminate boundedly. Pure predicate tests cover all8input combinations.

All32Rust tests pass; release shell-pwm builds. No flash/motor/hardware handle
this entry. Board remains Entry090 snapshot build,last safe there. Next flash,
disabled preflight and coast repeat should test whether skip counters expose
the real failure and whether duplicate accepts disappear. Do not declare
root cause or tracking fixed from this host regression alone.

## Entry 092 — 2026-09-12 — Requested live-shell 50 to 200 eHz sweep

User requested live speed control in 10 eHz steps. Existing Entry090 firmware,
G071/DRV8304H current wire map; no flash or firmware edits. Explicitly cancelled
obs and coastref first. Preflight p/i showed gates/ENABLE/MOE/CCRs zero,nFAULT1.
Numeric gate: 6.5% catch and sine duty, existing10% ceiling, operator800mA PSU
limit, <=5s powered, all15speed acknowledgements and no guard trip. Existing
current/bus/nFAULT/timing protections unchanged; no BEMF gate authority.

live_shell_sweep_200_01: fixture ran existing100eHz catch/ramp to50, then live
60,70,...,200 commands over approximately1.2s of hold. All15 F acknowledgements
present. Normal reason1 stop at4998304us. Wave49990updates,maxgap101us,
maxISR28us; foregroundmax1473us. Final256drive samples: bus11.513..11.867V,
no fault samples, phase pulse absolute peak10.212A (not PSU average current).
Capture is the final256ms, not per-rung current qualification.

500 CRC-decoded coast records: initial fitted199.28eHz, disable-extrapolated
200.67eHz,R2=.99993,phaseorderA->B->C score1,motionvalid. Full-coast166.44eHz
includes deceleration; historical45..55diagnostic false is expected at200.
Evidence supports rotor reaching the final commanded speed, not closed-loop
lock or proof of each intermediate rung. Raw/CSV/coast/plot retained.

Post off/p/i confirmed all six gates,ENABLE,MOE,CCRs zero,nFAULT1; safe transcript
retained and COM41 closed. Entry091 adapter guard remains unflashed.

## Entry 093 — 2026-09-12 — Source guard exposes masked-vector storm

Revalidated32Rust tests and release build; flashed Entry091 after off/p/i.
Normal boot. Disabled coastcheck: zero accepts,257IRQ cap,4626us,all skip
counters zero,outputs still disabled. New guard confirmed in COREDISPATCH.

coastref_guard_200_01: same current G071/DRV map,6.5% catch/sine,800mA PSU
limit,all15 live50->200 acknowledgements,normal reason1 at4998551us.
Existing current/bus/nFAULT/timing guards and10% ceiling unchanged.
Final256drive samples bus10.445..11.923V,pulse peak7.672A,no fault samples.
Wave49990updates,maxgap101us,ISR28us. This does not qualify average current.

Bridge-disabled coastreference accepted exactly1event:1649us,sector1,
referenceinterval3752ticks. Then255software-masked dispatch skips,zero other
skip reasons. Total257IRQ,zeroCOM,COMPmax51us; IRQcap stop6/status5 at2697us.
COM DIER remained1; reference avg1666,polling0,running1,desync0. Duplicate
accepts prevented in this run, but reference progress still fails. Strong
evidence for repeated shared-vector dispatch while softwaremasked; NOT yet
proof which hardware source asserts it. Guard reason priority means software
mask count alone does not prove EXTI hardware mask state at those invocations.
Existing terminal snapshot follows backstop masking/clearing,so insufficient.
Delayed coastfit199.95eHz atdisable,R2=.99992,order1 confirms rotor continued.

Added first-skipped-dispatch register snapshot: timestamp,EXTI IMR/RPR/FPR,
ADC ISR/IER,COMP1/2CSR. Only first coast skip,32bytes+validflag; fixture-only
output after stop. No flag clearing/filter/timer/drive changes. Release builds
with existing warnings. Snapshot source/ELF NOT flashed; board now Entry091
source-guard build. Next use snapshot to discriminate remaining IRQ source
before changing masking or clearing semantics. Do not raise IRQ cap.

Preflight/raw/CSV/coast/plot/safe artifacts retained. Explicit off/coastref0/p/i
verified all gate inputs/outputs,MOE,CCRs,ENABLE0,nFAULT1. COM41 closed.

## Entry 094 — 2026-09-12 — Pending EXTI survives IMR mask; NVIC masking restores progress

Flashed first-skip snapshot build after safe checks. coastref_source_200_01:
6.5% catch/sine,800mA PSU limit,all15 live50->200 steps,normal reason1 stop
4998691us. Final256samples bus11.455..12.048V,pulse peak8.396A,no faults.
One accept2231us/sector1/4916ticks,251softwaremasked skips,257IRQcap at3431us,
zeroCOM. First skip2243us: IMR=4294443008(0xfff80000,bit18clear),RPR0,
FPR262144(bit18set),ADC ISR10251,ADC IER0,COMP1CSR0,COMP2CSR641.
This captures a pending comparator source AFTER hardware masking, before
backstop clearing. It excludes enabled ADC interrupt and active COMP1 there.

RM0444 section13.4/Table66 explains IMR gates latching/wakeup; clearing the
pending bit clears CPU request, and CPU interrupt masking requires NVIC.
Source: https://www.st.com/resource/en/reference_manual/rm0444-stm32g0x1-advanced-armbased-32bit-mcus-stmicroelectronics.pdf
Updated Input.mask_interrupts to mask ADC_COMP NVIC before clearing IMR18.
Enable sets IMR18,clears stale NVIC pending (NOT EXTI pending),then unmasks
NVIC. Existing change_input/core retain EXTI clearing ownership. This vector
ownership is shell-specific: polled ADC and disabled COMP1; unsafe to reuse
unchanged with independent ADC/COMP1 IRQ clients. No filter/COM/IRQcap changes.

Built/flashed adapter fix. Boot UART stalled after bannerprefix even on second
reset. Read APBENR1=08000000,PD1ODR0,TIM1CCR1=0,BDTR0c1a(MOE0); restoring
USART3 clock only to08040000 resumed banner and off acknowledgement (same
Entry077/078 startup issue). Not attributed to comparator fix. No drive while
UART unavailable. compirqref then passed48/48 forced edges,zero timeouts;
p/i alloutputs0,nFAULT1 before drive.32Rust tests pass;release builds.

coastref_nvic_200_01: unchanged6.5%/800mA/200eHz/<5s envelope,normalreason1,
bus10.682..11.896V,pulse peak8.354A,no faults. Same15acknowledgedsteps.
Coast completes20ms(stop1 at20029us),122IRQs,zero dispatchskips,COMP51us,
COM24us.21CRC-valid accepts,21virtualCOM,sector1..6ordered withoutduplicates.
Firstaccept2480us,then20gaps791..954us,finalaccept19762us/sector3; final
avg1741ticks,COMDIER0,softwaremask0,desync0,polling0,running1. Monitorfault1
is STILL correct: firstevent misses1000usacquisitiondeadline. Subsequent
intervals and terminalage267us fit report-only timing bounds. This is one
successful post-acquisition coast progression,not full tracking/lock or
BEMF-driven operation. Delayedcoastfit199.83eHz/order1/R2=.99987.

Bothattempts raw/CSV/coast/plot/preflight/safe retained. Both ended explicit
off/coastref0/p/i with six gates/ENABLE/MOE/CCRs0,nFAULT1;UARTclosed.
Currentboard NVIC-maskfix build. Next repeat for stability/seed sensitivity,
then address measured acquisition timing; do not relax watchdog to call pass.

## Entry 095 — 2026-09-12 — NVIC fix repeats; opposite seed also progresses

Current Entry094 firmware, no flash or waveform edits. Both new attempts use
current G071/DRV map,6.5%catch/sine,operator800mA PSU limit,existing10%ceiling,
five-second powered bound,all15live50->200steps. obs0; reference receives
sense-mux authority only after bridge disabled. Numeric gates unchanged:
no powered fault/current/bus/timing trip,ordered accepted events,no duplicate
sectors or masked dispatch storm,20ms coast bound. Acquisition monitor remains
1333..2000ticks report-only; its first-event deadline is not bypassed.

coastref_nvic_200_02(seed1): normaldrive reason1,final256sample bus11.541..
11.869V,pulsepeak8.972A,no fault samples. Coast stop1 at20007us,127IRQs,
COMPmax51us/COM24us,zero dispatchskips.22accepts/21COM,last accepted19920us
has COM pending at20ms cutoff (not a missing COM claim). First1836us;21gaps
791..942us,ordered1..6,finalavg1742ticks. Initial acquisition remains late,
monitorfault1. Delayedcoastfit199.12eHz/order1/R2=.99986.

coastref_nvic_seed4_200_01: only virtualseed changes to4 (same physical phaseC
sensing with opposite expectededge). Normaldrive reason1,bus11.373..11.900V,
pulsepeak6.868A,no faults. Coaststop1 at20014us,119IRQs,COMP51us/COM24us,
zero dispatchskips.21accepts/21COM,ordered4,5,6,1,2,3...,first2439us;20gaps
793..955us,final19751us,avg1745ticks. Monitorfault1 initial acquisition.
Delayedcoastfit198.32eHz/order1/R2=.99989. No desync/polling fallback in either.

Together with Entry094: three20ms runs without storm/duplicate acceptance,
post-acquisition gaps all791..955us. Evidence for repeatable bridge-disabled
reference progression,not powered lock. Opposite seed does not remove initial
delay. Current path seeds an assumed commutation/prior ZC,not measured rotor
phase; therefore this is not a qualified startup acquisition procedure.
Next implement/qualify reference polling acquisition and transition in the
bridge-disabled path rather than grind more arbitrary-seed IRQ repeats or
retune timing bounds. Current coast_run still stops at polling changeover;
full polling/recovery is host-tested but not yet continuously bench integrated.

Added retained-capture regression for allthree runs: CRC/count/order,gap
extrema,zero skips,and STILL fault1/noauthority.51Python tests pass. Both
attempts have raw/CSV/coast/plots/safe logs. Seed4 arm acknowledged after
seed1 off/p/i; no separate preflight file forseed4. Explicit final off/coastref0/
p/i verified gates/ENABLE/MOE/CCRs0,nFAULT1 after each;COM41 closed.

## Entry 096 — 2026-09-12 — Reference polling startup integration prepared

Added explicit one-shot coastpoll1/0 setting for next coastref/coastcheck.
Default IRQ-only test unchanged. Polling mode calls actual start_motor with
runningfalse/zero_crosses0: reference advances seed and sets interval10000,
counter5000. State initialization is interrupt-atomic; no partially initialized
state dispatch. Uses existing polled-ADC shell and record-only gate HAL.

TIM7 nominal20kHz calls actual polling_bemf_check while old_routine. Reference
owns blocking zcfoundroutine,commutation,mux,blend and IRQchangeover. Timer
remains available after changeover for fallback. Existing main bands retain
average/filter/desync/timeout/recovery calls. This is NOT the full reference
duty/ADC safety ISR (its ADC conversion/thresholds are wrong for this board).
All powered waveform and DRV safing guards unchanged. Coast loop no longer
stops merely for old_routine when explicit polling mode selected.

Summary separates pollingcoms/IRQtransitions/firsttransition/waitguardhits/
recovery/desync and pollingcost/gap. IRQ acceptedlog format unchanged and
explicitly incomplete for mixed-mode acceptance. No synthetic EV_ACC inserted.
New polling invocations stop at20ms; an already-running bounded reference
spinwait may finish past deadline. Needs disabled preflight timing measurement.
20ms test cannot by itself qualify22.5ms no-edge timeout recovery.

Host regression starts actualreference from10000/5000 startup seed,injects
synthetic1666tick sector intervals,proves pollingconvergence toIRQ after>5
zero_crosses,no waitguardhit,no erroneouslyarmedCOM. Not a rotor acquisition
simulation.33Rust tests and releasebuild pass (existingwarnings); specific
newtest rerun separately exit0. Source/ELF NOTflashed. No serial/probe/motor
use thisentry;board still Entry094NVICfix,lastsafeEntry095. Next disabled
coastpoll1/coastcheck before spinning. Shell-sine and siblingcore unchanged.

## Entry 097 — 2026-09-12 — Measured polling-to-IRQ changeover, then reference desync

Built/flashed Entry096 after explicit off/p/i;normal firstboot. Disabled
coastpoll1/coastcheck captured in coastpoll_disabled_01.txt:399poll calls,
max37us,gap51us,no waitguardhit,one pollingcom plus startupcom,zeroIRQ,
stop1 at20018us. Idlelevel progress is NOT rotor evidence. p/i alloutputs0.

Armed obs0/coastpoll1/coastref1;coastpoll_200_01 uses unchanged G071/DRV map,
6.5%catch/sine,800mA PSU limit,10%ceiling,<5s,and existing powered fault/
current/bus/timing guards. All15live50->200 steps acknowledged;reason1 normal
drive stop4998349us. Wave49990updates,maxgap101us/ISR28us. Final256samples
bus11.328..11.941V,pulsepeak9.025A,no faults (not averagecurrent qualification).

Bridge-disabled reference startup:12pollingcoms,one transition toIRQ at8708us,
136pollingcalls,max425us/gap427us,waitguard0. FiveIRQaccepts9148s2,10038s3,
10891s4,11805s5,12607s6 (us/sector); four gaps890,853,914,802us.30IRQcalls,
COMP52us/COM24us,zero maskeddispatch skips.18totalvirtualCOM includes startup
and12polling and5IRQCOM. Reference desync1 set runningfalse/old_routinetrue,
stop3 at12887us,avg1806ticks. No recovery observed;test stops on runningfalse.
This proves physical polling->IRQ execution,not successful acquisition/lock.
IRQ-only timingmonitorfault1 is not a mixed-mode event-gap classifier.
Delayedcoastfit198.57eHz/order1/R2=.99993 confirms continuedrotor motion.

Reference desync_check_band compares last_average to average on eligible
sectorwrap,then overwrites previous average. Need exact decision operands
before attributing stop to rotor or startup interval-history transient. Added
firstEV_DSY snapshot(us,previousavg,newavg,ci),fixture-only COASTDSY reporting.
16bytes+flag,no filter/threshold/controlchange. Releasebuildpasses; snapshot
source/ELF NOTflashed. Board remainsEntry096pollingbuild. Next capture desync
operands and replay against startup history; do not simply loosen guard.

Raw/CSV/coast/plot/safe retained. off/coastref0/coastpoll0/p/i confirms allsix
gateinputs/outputs,MOE,CCRs,ENABLE0,nFAULT1. UARTclosed. No ongoing handles.

## Entry 098 — 2026-09-12 — Two polling-startup repeats survive; desync remains intermittent

Flashed Entry097 EV_DSY snapshot after explicit off/p/i;normalboot. No filter,
threshold,drive or state-seed changes. Current G071/DRV map,6.5%catch/sine,
operator800mA limit,10%ceiling,<5s,existing fault/current/bus/timing guards.
Each run acknowledged15live50->200steps,obs0/coastpoll1/coastref1. Sensing
bridge-disabled,zero gateauthority. These repeats target intermittentdesync,
not expanded envelope qualification.

coastpoll_dsy_200_01:normalreason1,energized4999002us,bus10.874..11.879V,
pulsepeak8.556A,no fault samples.11pollingcoms,IRQchangeover6704us,103calls,
max373us/gap375us,no waitguard.15CRC IRQaccepts7204..19390us,ordered1..6;
14gaps817..922us.27totalvirtualCOM (1startup+11polling+15IRQ).80IRQcalls,
COMP52us/COM24us,zero dispatchskips. Coaststop1 at20014us,avg1741,desync0.
Delayedcoastdisablefit199.45eHz,R2=.99990,order1. Initial simple edge estimate
166.67eHz differs from extrapolated fit; neither is direct earlycoast phase.

coastpoll_dsy_200_02:normalreason1,energized4998679us,bus10.833..11.948V,
pulsepeak9.747A,no fault samples.13pollingcoms,IRQchangeover12058us,195calls,
max476us/gap478us,no waitguard.9CRC IRQaccepts12527..19551us,ordered3..6..;
8gaps824..928us.23totalvirtualCOM(1startup+13polling+9IRQ).48IRQcalls,
COMP52us/COM24us,zero dispatchskips. Coaststop1 at20011us,avg1738,desync0.
Delayedcoastdisablefit198.17eHz,R2=.99986,order1.

Neither emits COASTDSY because neither desynced. Diagnostic operands remain
unmeasured; do NOT claim previousfailure fixed by diagnostic-only change.
Combined polling campaign: one desync stop and two20ms completions,variable
acquisition6.704..12.058ms. IRQ-only timingmonitor stillfault1; cannot use it
as a full mixed-mode acquisition classifier. Poweredlock/recoveryunqualified.
No threshold relaxation. Next needs complete mixed-mode event/history replay
or capturedDSY operands,not declaration of success from two non-failing runs.

Bothraw/CSV/coast/plots/safe retained,firstpreflight retained. Secondarmedafter
firstverifiedoff. Everyexit off/coastref0/coastpoll0/p/i: allsixgates,ENABLE,
MOE,CCRs0,nFAULT1. COM41closed. Board nowEV_DSYsnapshotbuild;no unflashededits.

## Entry 099 — 2026-09-12 — Mixed-mode commutation history prepared

Existing IRQ-only accepts cannot reconstruct startup interval history at the
first eligible desync comparison. Added32x8u16 history records for actual
EV_REF/EV_DSY callbacks during explicit coastpoll mode: timestamp,kind,step,
eventinterval,average,previousaverage,polling,zero_crosses. Captures startup,
polling commutations and IRQ commutations through the same reference callback.
Short interrupt-atomic snapshot protects foregroundDSY versus ISRREF; no
formatting/gate writes. Fixture-only H85/CRC dump,explicit overflow count.
Normal shell stays quiet; existing raw formats and IRQ accept logs unchanged.

EV_REF occurs after history-slot push but before foreground average update
and before polling->IRQchangeover. That ordering is retained,not normalized
away. EV_DSY previous-average read precedes reference overwrite.16bit times/
values restricted to bounded currenttest; do not extend blindly past65ms.
Decoder validates framing/version/count/CRC/fields/timeorder, rejectsoverflow.
54Python tests pass,releasebuildpasses. Source and ELF NOTflashed; nohardware
handle or motor action thisentry. Board remainsEntry097DSYsnapshotbuild.

ELF text113216,data16,bss32848:4000bytes of36KiB remain before stack/runtime
qualification. Addedhistory512bytes plus counters; further instrumentation
must respect this shrinking margin. shell-sine SHA256 unchanged:
B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.
Nextflash/disabledpreflight then one mixedstartup capture and replay actual
history/desync operands. Not a fix or a trackingqualification result.

## Entry 100 — 2026-09-12 — Mixed history captured and reference averages replay exactly

Flashed Entry099 after off/p/i. UARTclock startupissue recurred:APBENR1
08000000,PD1ODR0,BDTR0c1a verified;write08040000 restored shell. Initial
bootread had invalidUTF8,subsequent offack normal. Disabled coastpollcheck
20013us,400calls,max42us/gap51us,no waitguard;outputsverifiedoff.

coast_history_200_01: unchangedG071/DRV map,6.5%catch/sine,800mA PSUlimit,
10%ceiling,five-second drivebound,all15live50->200acks. Normalreason1,
final256samplesbus11.379..12.094V,pulsepeak8.290A,no faults. Existingpowered
guards unchanged. Onlycoast getsrecord-onlyreference control/sensemux.

26CRC-validH85records,nooverflow,allEV_REF.11pollingcoms,IRQchangeover7523us,
116pollcalls,max420us/gap422us,no waitguard.26totalvirtualCOM,20msstop1,
avg1746ticks,desync0,coastfit199.18eHz/order1/R2=.99990. No reproduction of
earlierdesync and noCOASTDSY; no claim it is fixed.

Historyshows reference startup pushes8908,6855,5316,...into sectorintervals,
raising rollingavg from1666 to5313 beforeconvergence. Firsteligiblewrap updates
previousavgto2432;nextcomparison2432->1773 has difference659<=886 and passes.
Added hosttest replaying first18recordedcommutations through actualreference
slotpush/store_average_interval/desync_check_band. Everycapturedpre-bandavg
andpreviousavg matches; postprevious1773,zero desync.34Rusttests pass.
This reproduces averaging/desync bookkeeping,NOT unrecorded comparatorlevel
decisions or previousfailedrun. Startuphistory transient is visible; causal
attribution of Entry097desync stillrequires its comparison operands/history.

Raw/CSV/coast/plot/preflight/safe retained. off/coastref0/coastpoll0/p/i
verified sixgates,MOE,CCRs,ENABLE0,nFAULT1;COM41closed. Board nowhistorybuild.
No firmwarechanges afterflash;newhosttestonly. Nextcapture failinghistory or
exercise seed-sensitive startup in hostmodel; no thresholdrelaxation.

## Entry 101 — 2026-09-12 — Sample-held startup sensitivity through actual reference

Extended hostMockHal with optional timestamped allphase reference-polarity
samples. Timeadvance updates selectedphase and latches selectededge only
while enabled; muxchange masks/clears and selects latest measuredphaselevel.
Existing mockbehavior unchanged withouttimeline. During referenceblocking
waits inputtime continues advancing; COM deadlines serviced by existingRig.
Found/fixed hostadvanceunderflow when modeled ISR timerreads consume time
past requestedadvanceendpoint: no clockrewind,only advance remainingpositive
time. This is hostfixture repair,not MCU firmware timingchange.

New test reuses measured96phase samples (32rows with actualCOASTCOMPoffsets)
from bemf_acceptlog_200_01 bridge-disabled coast. Actual start_motor then
50us polling/checks and COMP/COM plus mainbands for allsix seedsectors.
Explicit simulationcost0.5us perintervalread and1us externaldispatchcadence;
sample-and-hold levels,not edge-resolutionhardware replay or motor model.

Seeds1..5 firstIRQ at9671,11465,11473,12164,15222us;seed6 noIRQtransition
before measuredtraceends. VirtualCOM20,19,18,17,16,15 respectively;finalavg
1797,1853,1904,2003,2155,2285ticks. No desync,allrunningtrue,waitguard0.
This supports startupseed sensitivity but DOES NOT reproduce Entry097failure.
Only structural sequence/wait invariants asserted; no seed declaredlocked.
35Rusttests pass. No firmwareedit/build/flash/serial/motor action thisentry;
board remainsEntry099historybuild,lastverifiedoffEntry100. Hardwarefailure
still needs exactfailedhistory; synthetic/passingsamples cannot certify it.

## Entry 102 — 2026-09-12 — Captured desync operands identify startup-history convergence

Twoactualtests,currentEntry099historyfirmware,noflash. G071/DRVcurrentmap,
6.5%catch/sine,800mA PSUlimit,10%ceiling,five-secondguard,15acknowledged
50->200steps each. Existingpoweredcurrent/bus/nFAULT/timingchecksunchanged.
obs0/coastpoll1;gateauthority0 throughout bridge-disabledreference sequence.

coast_history_seed4_200_01:normalreason1,bus11.490..11.845V,pulsepeak5.824A,
no faults.25historyrecords/25virtualCOM,12pollcoms,IRQtransition9620us,
151pollcalls,max451us/gap453us,waitguard0.20msstop1,avg1752,desync0,
coastfit197.88eHz/order1/R2=.99988. Prioraveragecapture shows foreground
can retain an average computed before an ISR updated a sector; don't impose
strict immediate post-COM main-band scheduling on every capture.

coast_history_seed2_200_01:normalreason1,bus10.693..12.052V,pulsepeak9.919A,
no poweredfault.13pollcoms,IRQtransition11916us,193pollcalls,max472us/gap474us,
waitguard0.17virtualCOM +1DSY historyrecord,CRCvalid/nooverflow. At14385us
EV_DSYrecord and14389us diagnostic:previousavg3010,newavg1917,ci1717.
Reference predicate abs(3010-1917)=1093 > (1917>>1)=958,avg<2000,so stop is
expected from those operands. runningfalse/old_routinetrue,stop3. Motor still
coasts,disablefit198.97eHz/order1/R2=.99992; no sudden rotorcollapse implied.

Historyprovenance: cold startseed10000 produces firstpollci9533,then7574,
6105,5003,... rollingavgpeaks5975,decays3010,then1917. The3010comparison
baseline is startup-historyconvergence,not a measured sudden change in rotor
speed. Startup here is applied to an already200eHz coastingrotor. This
explains the capturedreference stop mechanism without changing wiring or
looseningdesync. Exactforeground/ISRpreemption retaining3010 not reconstructed.
Do not equate this with blanketproof every historicaldesync had samecause.

Newhostregression replays measured17slotpushes through realreferenceaverage,
asserts3010/1917 historyvalues,then supplies capturedoperands to actualdesync
band and reproduces stopped/reroutedstate. Specifictestpasses. This is not
full edge/CPUtimingreplay. Next address acquisition initialization for a
spinningrotor using measured timing (or qualify actualreference recovery),
not keep cold-starting at an assumed slowinterval and relaxing its guard.

Bothraw/CSV/coast/plot/safe retained;seed4preflight retained,seed2armedafter
seed4verifiedoff. Everyexit off/coastref0/coastpoll0/p/i: sixgates,MOE,CCRs,
ENABLE0,nFAULT1. COM41closed. Onlyhosttest/labedits;board unchangedhistorybuild.

## Entry 103 — 2026-09-12 — Measured flying-acquisition seed validator prepared

Added no_std flying_acquire module (host-linked only,not shell firmware yet).
Consumes independentlyqualifiededges in referencepolarity/logicalA/B/C.
SequenceC+,A-,B+,C-,A+,B- follows establishedphysicalrotation. Candidate
requires12measuredsectorintervals (twoelectricalcycles),each1333..2000half-us
ticks;returns finalobservedsector,edge timestamp,measuredmeaninterval.20ms
overall acquisitionbound;too-fast/slow/wrongorder/invalidphase/expired each
latchesexplicitfault. One-shotseed timestampcannotrefreshfromlatertraffic.
No gate/timer access,no referencealgorithmchange,no synthesizedEV_ACC.

This is an experimental fail-closed seed path,NOT a new productionlock
criterion. It does not yet consume raw GPIO/comparator samples:caller must
qualify muxsettling,samplecontinuity,persistence,and reject initialstaticlevel
as an edge. Coherentfalseedges can pass timing/order; not proof of BEMF alone.
Hardware acquisition and reference handoff remain unimplemented/unqualified.
Do not treat this narrower module as completion or impose perfectper-sector
crossings on the ongoing reference tracking path.

Fiveunit tests cover allsix initialsectors,two-cycleboundary,individualtiming
limits,missing/duplicate/reversed/spikeedge rejection,invalidphase,timeout,
u32timewrap,and stale-seednonrefresh.41Rusttests pass;M0libcheck passes.
No serial/probe/motor action,no shellbinarybuild/flash. Board remainsEntry099
historybuild,lastverifiedoffEntry102. Next implement bridge-disabled rawedge
qualification and measure its timing before plugging seed into referenceCOM.

## Entry 104 — 2026-09-12 — Live UART 50-to-200 eHz sweep

User requested live shell speed stepping. Ran existing Entry099 shell-pwm
firmware, no flash; new flying-acquisition source is not on the board.
G071/DRV8304H current wiring, unchanged 800 mA PSU setting, 6.5% catch and
hold duty, 10% ceiling. Preflight off/p/i: six gates, ENABLE, MOE and CCRs
zero, nFAULT=1. Cleared obs/coastref/coastpoll for plain sine plus coast.
Pass gates: energized <=5 s, bus >=8.4 V, no current/fault/timing abort.

Fixture: python scripts/drv_capture.py --target 50 --duty-tenths 65
--catch-duty-tenths 65 --step-targets
60,70,80,90,100,110,120,130,140,150,160,170,180,190,200
--tag live_shell_sweep_200_01

All fifteen live commands acknowledged F60 through F200. Proven startup
still uses 100 eHz catch then ramps to 50 before the live hold sweep.
Normal reason=1; energized 4,998,864 us. Wave max gap 101 us, ISR 28 us;
foreground max interval 1388 us. Retained 256 terminal drive samples show
bus 11.504..11.862 V, no fault samples, maximum phase pulse 7.891 A
(not average PSU current). Not a full per-rung current characterization.
500 coast samples: initial 199.28 eHz; disable extrapolation 199.18 eHz,
R2=0.99993; phase order A->B->C score=1.000. Full-coast median 165.95 eHz
includes deceleration. Historical 45..55 target flag is irrelevant here.
This demonstrates actual rotor speed near 200 eHz, not BEMF closed-loop lock.

Raw CRC-checked capture, CSV, coast CSV and plot retained under
captures/live_shell_sweep_200_01*. Postflight off/p/i retained in _safe.txt:
six gate inputs/outputs, ENABLE, MOE, CCRs all zero, nFAULT=1. COM41 closed.

## Entry 105 — 2026-09-12 — Bridge-disabled flying acquisition measured

Implemented shell flying_bench with flycheck and one-shot coastfly1/0.
COMP2 scans logical A/B/C, reference polarity, experimental 10 us mux settle.
Per-phase EdgeFilter requires a second changed-level sample >=20 us later;
first static samples are baselines, not edges. Gap >100 us latches refusal.
Acquire checks ordered edges and twelve 1333..2000 half-us intervals, bounded
20 ms. No controller handoff or gate authority. Every refusal is reported;
simultaneous coastref/coastfly rejects both rather than running sequentially.
Host sampled-mux test covers all six initial sectors. 44 Rust tests pass.
Release build passes with existing unsafe-op/unused warnings. Flash text
118360, data16, bss32892 bytes; RAM remaining before stack=3956 bytes.
Flashed after verified off. UART startup clock issue recurred: APBENR1
08000000, PD1 ODR0, BDTR0c1a/MOE0, all three CCR0. Restored only USART3 clock
bit to 08040000; initial malformed boot bytes then normal off/p/i ack.
Cause remains unproven; this register intervention was with bridge disabled.

Stationary flycheck_01: result3 WrongOrder after669 us,44 samples,maximum
per-phase gap44 us,3 cancelled candidates,zero qualified intervals,no seed.
It rejected near-neutral transitions rather than timing out without edges.
No claim of analog silence or qualified noise immunity. Outputs stayed off.

Powered coastfly_200_01: G071/DRV current map,800mA PSU setting,6.5% catch/
sine,10% ceiling,normal five-second sweep50->200 in acknowledged10eHz steps.
Existing gates: bus>=8.4V,rawcurrent deviation<=1200 counts/nonrail,nFAULT,
wave gap<=500us,foreground<=2ms,energized<=5s. No expanded envelope.
Normal reason1,energized4,998,699us,wave gap101us/ISR28us,main1296us.
Retained256drive samples:bus11.444..11.937V,pulsepeak8.458A,no fault samples.
Pulse current is not PSU average and does not qualify duty expansion.

FLY result1: measured step2,edge_ticks20424,interval_ticks1687 (843.5us,
197.589eHz),elapsed10264us,706samples,maximum per-phase gap94ticks=47us,
cancelled0,intervals12,disabled1,gate_authority0. Confirmed seed timestamp
is52us before report completion; it must not be treated as a current edge.
This is measured rotor acquisition, not closed-loop lock. Later coast data
starts after acquisition: extrapolated disable199.59eHz,R2=.99989,phaseorder
score1.000,initial199.60eHz. Rawdrive/coast CRC-checked artifacts and plot
retained, but individual acquisition edges are not yet retained in this build.
Next consume measured phase/interval/age in reference observe-only COM handoff,
with explicit late-seed refusal; do not call cold start_motor or relax desync.

Postflight off/coastfly0/coastref0/coastpoll0/p/i retained in _safe.txt:
all six gate inputs/outputs,ENABLE,MOE,CCRs0,nFAULT1. COM41 closed. Board now
runs this flying-probe build. No changes to shell-sine or sibling minz-core.

## Entry 106 — 2026-09-12 — Measured-seed reference handoff implemented, unflashed

Previous turn was progress: Entry105 acquired actual ordered rotor timing.
Added coasttrack1/0, reusing the flying scan and coast reference loop with
record-only output HAL. Exact measured interval initializes all six history
slots, average/prior average,last/this ZC and reference advance/wait. Initial
step/polarity comes from the measured edge; twelve acquisition intervals are
explicitly distinguished from controller accepted events. No start_motor,
no synthetic EV_ACC, no changes to reference desync/filter/blanking science.

Before timer arm, measure edge age again after setup. Shared Seed::handoff
computes remaining ARR=reference_wait-age, with >=64 half-us ticks (32us)
remaining required; late/invalid input refuses FLY result11. Interval timer
starts at measured age. Comparator stays masked until actual first COM ISR
changes the sense mux and reenables it. Further COMs require real accepted
reference input. The timer-arm instruction cost still needs MCU measurement:
the host model proves arithmetic/event ordering, not register-write latency.
Current scan and tracking remain bridge-disabled; no energized handoff exists.

Host tests cover captured104tick acquisition age, late/boundary rejection,
all six initial sectors through actual COM ISR, unchanged initialized average,
no early/duplicate COM, zero fabricated EV_ACC, and next input arming nextCOM.
46 total Rust tests pass; release build passes existing warnings. Parameter
validation precedes frequency division. No serial/probe/motor operations this
turn. Board remains Entry105 acquisition-only firmware, lastverifiedoff there.
Next review timer-arm cost and execute coasttrack observe-only on hardware;
do not infer hardware qualification from these source/host results.

## Entry 107 — 2026-09-12 — Measured handoff sustains reference coast progression twice

Entry106 source was concrete progress; this turn built/flashed it after fresh
off/p/i verification. Added TIM17 measurement around final handoff arithmetic,
counter setup and timer enable; >16us refuses/halts timer with FLY result11.
Minimum remaining ARR margin stays32us. Build text118836,data16,bss32904:
3944 bytes RAM remain before stack. UART boot normal, no clock repair.

Two G071/DRV8304H current-map runs,unchanged800mA PSU setting,6.5%catch/sine,
10%ceiling,all15 live50->200steps acknowledged. obs0/coastref0/coastpoll0,
coasttrack1. Numeric powered gates:<=5s,VBUS>=8.4V,rawcurrent deviation1200
counts/nonrail,nFAULT,wavegap<=500us,foreground<=2ms. Probe20msbridge-disabled,
no outputHAL gate authority. Both normal reason1; source does not cold-start
or relax reference guards. Subsequent coast capture delayed by acquisition
plus tracking; full-coast median165.95Hz includes deceleration.

coasttrack_200_01: energized4,998,304us,mainmax1480us. Retained terminaldrive
bus11.512..11.823V,pulsepeak8.093A,no fault samples. Flying step1,ci1696ticks,
edge22214ticks,elapsed11161us,738samples,maxphasegap48us,cancelled0,12intervals.
Handoff age268ticks=134us,ARR156,arm3us.22CRC-valid accepted events,first726us,
last19479us,all sectors2,3,4,5,6,1...ordered;inter-event gaps786..971us.
23COM=one explicit measuredbootstrap plus22accepted-inputCOM;avg1806ticks,
20,025usobserved end,stop1,desync0,polling0,running1,ACCEPTTIMINGfault0.
COMP131calls,max53us;COMmax26us;sourceguard skipcounters0. Latercoastfit
disable199.52eHz,R2=.99976,order1.000.

coasttrack_200_02: energized4,998,557us,mainmax1504us,bus11.414..11.854V,
pulsepeak6.615A,no fault samples. Flying step1,ci1704ticks,edge20960ticks,
elapsed10534us,696samples,maxphasegap48us,cancelled0,12intervals. Handoff
age278ticks=139us,ARR148,arm2us.22accepted/23COM,avg1810ticks,20,014usend,
normalstop1,no desync/polling/stale-event monitor fault. COMP141calls,max53us,
COMmax25us. Latercoastfitdisable199.56eHz,R2=.99979,order1.000. Both acquired
sector1; repeatability is not all-sector hardware validation. Othersectors
are host-tested only. No claim that 2..3us arm cost measures analog crossing
timestamp error; muxsampling/confirmation uncertainty remains separate.

New retained-capture regression CRC-decodes both acceptance logs, verifies
ordered22events,first/interevent/end freshness<=1ms,23COM,measuredseed metadata,
arm<=16us andgateauthority0.55Python tests pass;46Rusttests lastEntry106.
Raw/CSV/coast/plots preserved under coasttrack_200_01/02; off/coasttrack0/
coastref0/coastpoll0/p/i _safe files show allsixgateinputs/outputs,ENABLE,
MOE,CCRs0,nFAULT1. COM41closed. Board runs this timed measured-handoff build.

This removes the observed cold-start history mismatch in these two runs and
qualifies repeatable short bridge-disabled referenceprogress,not sustained
powered rotorlock or fullparity. Next validate drivenphase/gate alignment and
switching-artifact behavior with a bounded observe-only handoff, then give
referenceCOM guarded short gateauthority only after that evidence. Average
current qualification,dutyexpansion,fullrecovery/lock benchmarks remain open.

## Entry 108 — 2026-09-12 — Phase contract tests and engage-rate consult

Before consult, linked sixstep.rs into host tests (its three local tests were
not previously part of observer-replay). New integration test reads the actual
shell SINE_LUT and proves increasing phase advances binz steps5,6,1,2,3,4,5;
source/sink match extrema, acquisition edge labels match the floating phase,
and actual reference COM increments select the next binz voltage vector.
50 Rust tests pass. Geometry/polarity evidence only, not rotor lag or powered
handoff qualification. minz hardware HIGH/LOW table traverses the opposite
order from binz/rm32; preserve binz mapping rather than copying that table.
No firmware edit/build/flash or hardware run during this phase-contract work.

Read GRAYBEARD_ENGAGE_CONSULT.md in full after it arrived. Adopt fixed-condition
rate characterization rather than further forensics on individual startup
desyncs. E106 is already flashed with E107 arm-time guard; E107 yielded2/2
successful measured-seed coast tracking attempts. Complete an eight-attempt
cohort on the SAME build,6.5%catch/drive,50->200 live sweep,800mA setting,
unchanged current/bus/timing/five-second guards. Count acquisition failures,
late-seed refusals and reference dropouts in the denominator; never replace
failed attempts with retries or cherry-pick successful seeds.

Predeclared coast success: normal powered exit within5s,no hardware guard;
FLYresult1 with measured CORESEED,arm<=16us;full20ms reference observation,
ordered CRC-valid non-overflow acceptance log,first/interevent/end gap<=1ms,
interevent>=666us,onebootstrapCOM+acceptedCOMs accounted for including a
possible pending finalCOM;no desync or polling fallback,verified outputs off.
Report actual counts/timing spread as well as binary rate. Safe-state failure
stops cohort. Other guard failures stop for assessment, not automatic retries.
This is a single-treatment COAST acquisition/tracking rate, not a paired
comparison, powered engage success, or parity certificate. A paired baseline
requires matched reference evidence; minz bench remains disconnected. The
consult's promised reference is pending, not assumed available or measured.
After this bounded cohort, move to powered-handoff qualification, not more
coast cohorts solely to improve the percentage. Board remains E107,lastoff02.

## Entry 109 — 2026-09-12 — Eight-attempt measured coast cohort: 8/8

Completed attempts03..08 on unchanged E107 firmware under E108 protocol;
all attempts retained,none replaced. G071/DRV current wiring,6.5%catch/sine,
50->200eHz live10eHz steps,800mA PSU setting,10%firmwareceiling,five-second
limit and existing bus/current/nFAULT/timing guards unchanged. All six new
runs normal reason1,no poweredguard/fault;every postflight verifiedsixgates,
ENABLE,MOE,CCRs0,nFAULT1. COM41closed,no flash or firmwareedit this turn.

Whole cohort01..08:8/8 measuredacquisition+20msreferencecoasttracking success,
22accepted events and23COM each (onebootstrap+22input-driven). No acquisition
refusals,latehandoff,desync,pollingfallback,staleeventfault or logoverflow.
CRCdecode+hostregression validates order,first/endfreshness<=1ms,interevent
666..1000us,arm<=16us,energized<=5s,offstates;55Python tests pass.

| Attempt | Seed step | First accept us | Min/max interevent us | End age us |
|---|---:|---:|---:|---:|
|01|1|726|786/971|546|
|02|1|694|812/995|531|
|03|1|717|814/991|472|
|04|5|690|829/984|530|
|05|2|844|808/986|568|
|06|6|839|814/961|509|
|07|2|849|803/982|538|
|08|1|740|817/969|582|

Across cohort:seedinterval1689..1704ticks,arm2..3us,terminalavg1800..1810ticks.
Retained terminaldrive bus minimum11.255V,maximum12.107V;largest phasepulse
9.100A (NOT PSUaverage). Powered duration4,998,304..4,999,004us. Latercoast
disable extrapolation195.69..200.67eHz includes a~30msbackwardfit afterprobe;
not an independent instantaneous tacho reading. Raw/CSV/coast/plots and _safe
files retained for each;03..08 preflight transcripts also retained.

Rate is descriptive8/8 at this singlecondition,not proof of100%population
reliability,not pairedminzparity,andnotpoweredclosed-looplock. Seeds3/4 not
represented. Cohort complete: no further coast-only repetitions needed now.
Next qualify poweredhandoff using existingminz mechanism and explicit
loss-of-tracking/current guards. User reiterated that duty may rise through
20% to hard30% exploratoryceiling with safetyguards;6.5%was fixed here only
for comparablecohort,not an authorizationlimit. Firmwarestillcaps10%; before
raising it,verify protection during poweredreferenceoperation. PSU800mAlimit
does not independently bound recirculating/phasepulse current.

## Entry 110 — 2026-09-12 — Powered-segment guard policy prepared

Previous cohort completed8/8; no more coast-rate attempts. Inspected TIM6
wave scheduler,DRV safing,referenceCOM writer and current observation hook.
Existing coasttrack starts after the five-second drive: it MUST NOT simply
reenable outputs there. Future powered handoff must instead interrupt drive
early (existing microscope uses4.7s) and inherit original elapsed budget.

Added no_std powered_guard.rs,host-linked only. Constructor requires valid
phasecurrent/bus/VREF snapshot and logicalseed. Independent periodicpoll
enforces remaining original5s budget,20mssegment limit,<=200us pollgap,
<=1msfeedbackage,nFAULT and hostabort. Actual EV_ACC feeds firstexpected
sector=seed+1,then ordered666..1000us intervals and<=1msfreshness. Bootstrap
COM is not an accepted event. All fault reasons latch; late feedback cannot
erase an already missed feedback deadline. Currentrail/deviation1200counts
and bus8.4V thresholds preserve existing envelope,not dutyexpansion approval.

Five tests cover inheritedbudget and u32wrap,valideventstream at segmentlimit,
missing/wrong/fast events,stale feedback/polling,pulse/bus/driver/host faults
and latching. Initial latch test mistakenly constructed a freshguard between
assertions; corrected test,not policy.55Rusttests pass,M0libcheckpasses.
No hardware/serial/probe action. Module not yet linked into shell firmware,
no runtimeprotection or gateauthority added. Next implement independenttimer
adapter with immediateDRVoff on policyfault,check bridge-disabled before
connecting bounded poweredhandoff. Board remainsE107,lastverifiedoff08.

## Entry 111 — 2026-09-12 — Independent guard timer/safing path bench-tested disabled

Added powered_timer.rs: TIM6 at100us,priority0,exclusive ownership versus
wave_timer,short critical-section access to Guard,explicit reason latching,
immediate timer stop + gates_off + PD1low on fault. NoADC/UART inISR, no
gateenable writes. gates_off cancels adapter; vector dispatch selects guard
when owned. Feedback/EV_ACC entrypoints prepared,not connected to powered
control. guardcheck idlecommand rejects non-disabled bridge and exercises
syntheticfeedback,no motor motion,not ADC/current calibration qualification.

Releasebuild passes existing warnings; flashed after verifiedoff. Binary
text124692,data16,bss32944;RAM3904bytesbeforestack,flash~6.3KiBremaining.
PostflashUSART3clockissue again:readAPBENR1=08000000,PD1ODR0,BDTR0c1a,
threeCCR0. Restored USART3bit to08040000 only whiledisabled;bootgarbage
then normaloff/p/i. No claim rootcause repaired.

captures/powered_guardcheck_01.txt: mode0 nofeedback ->reason4 at1006us,
mode1 freshsyntheticfeedback/noedges ->reason8 at1006us,mode2 originalbudget
only500usleft ->reason1 at505us. ISRmax10/10/8us,hostbackstop0,disabled1
allcases. This proves hardwaretimerdispatch into policy and DRVsafing code,
not power removal from an energizedbridge. Postoff/p/i allsixgates,ENABLE,
MOE,CCRs0,nFAULT1. COM41closed. Board nowruns guardtimerbuild.

Measureddeadline trip latency5us means runtime integration must reserve
shutdownmargin inside total5s;do notstart deadline at exactremainingbudget
and call a lateISR strictfive-second compliance. Beforepoweredtest,also
prevent a preemptedCOM from re-enabling outputs afterguardtrip (latched
authoritycheck at finalwriter),connectrealADCfeedback andactualEV_ACC,
and preserve safing through all exits. No poweredgateauthority added yet.

## Entry 112 — 2026-09-12 — Shutdown margin and guarded writer prepared, unflashed

Reserved200us inside original5s budget in powered_timer.start; reservations
at/after4,999,800us or overflowing elapsed refuse. Sharedhelper unit tests
cover margin boundaries and overflow. Guard policy remains explicit about
its20ms poweredsegment; no restarting userbudget at handoff.

Prepared powered_timer.commit: in one critical section checks activeguard,
reason latch,policyhealthy,nFAULT,step1..6,duty1..currentceiling; then writes
binz sixstep and enablesPD1. Policyhealthy closes the interval between a
feedback/eventfault and trip's hardwarecleanup. No caller yet: not reachable
from shell/COM. Veto and maximumcommitcost counters added. Criticalsection
cost must be measured before relying on margin underpoweredoperation; test
post-trip commit refusal before allowing successfulcommit on hardware.

Actualreference EV_ACC nowfeeds poweredguard when adapterownsTIM6. Bootstrap
COM is still excluded. Initially used wrong local variable step; compile
caught it,corrected to Recordersector+1. Buildnowpasses. Preparedforeground
sample_feedback with logicalADC4/1/0,bus6,VREF13,100usboundedchannel/config/
conversion waits and explicit AdcTimeout fault11. No caller yet. RealADC
integration must own theADC and treat capture timestamps consistently; no
claim of phasecurrent averaging or qualified higherduty from this helper.

56Rusttests pass;releasebuild passes existing warnings. No flash/serial/motor
action this turn. Board remainsE111,lastverifiedoffguardcheck01. Next wire
earlycampaignhook,realfeedback,latched writer and measuredseed into bounded
observe-only/dry-run first; reserveoriginaltime and stop on hostinput. Guard
policy is not yet a demonstrated complete poweredhandoff safety system.

## Entry 113 — 2026-09-12 — Integrated early dry handoff; first acquisition refused

Implemented engagedry1/0 one-shot: stop provenopenloop at4.7s,disabled flying
acquisition,then measuredreferenceCOM+TIM6guard+realADC4/1/0/6/13 feedback.
Any receivedbyte aborts referenceportion;acquisitionitself staysdisabled and
bounded20ms. No COM outputwriter call,no new gateauthority. Guardbudget
inherits originalrun_start and reserves200us. Pathresult distinguishes
acquisitionrefusal3,setuprefusal2,pathran1;not a successverdict. Realfeedback
and EV_ACC flow through guard only if acquisition and setup succeed.

Releasebuilt/flashed afteroffverification:text129208,data20,bss32952,
~1844flashbytes and3892RAMbytesbeforestack remain. Optionaldiagnostics will
need separating before largerpoweredcontrol additions. PostflashUARTclock
issue08000000 recurred,PD1ODR0/BDTR0c1a/CCR0 verified;restored08040000,
then normaloff/p/i. Rootcause not repaired.

Re-ran disabled guardcheck onreservedbudget build:stale/noedges1006us,
ISR9/10us;500usoriginaltimeleft deadline305us,ISR8us. Allhostbackstop0,
disabled1. This demonstrates195usremainingmargin in that specifictest,
not a bound on arbitraryinterruptblocking or laterpoweredwriter duration.

engagedry_200_01:6.5%catch/drive,800mAsetting,all15steps50->200acknowledged,
unchangedbus8.4V/current1200counts/nFAULT/timing/five-secondguards. Stop
transitionreported4,700,732us (includes302usdisabledacquisition,not precise
MOE-highduration). Retaineddrivebus11.297..11.873V,pulsepeak8.602A,nofault.
Acquisition result3WrongOrder after302us,18samples,maxphasegap47us,zero
intervals/cancelled. Referenceguard path DID NOT START. Latercoastfit199.81
eHz,R2=.99992,order1.000. This is one earlyhandoff refusal,not rotorstall,
not part of the completedfixed-conditioncohort,not an engagedry rate result.

Capture POWERPATH reason1/305us was stale metadata from precedingguardcheck,
NOT a deadlinefailure duringmotorattempt. Corrected unflashedsource to emit
POWERPATH not_started=1 acquisition_refused=1 for this branch;raw unchanged.
Next compare switch-off-to-acquisition timing against successfulcohort and
make required acquisitionentryblanking explicit before poweredoperation.
Do not relax order/timing thresholds or claim integratedguardpass from this
attempt. off/engagedry0/p/i _safe confirms sixgates/ENABLE/MOE/CCRs0,nFAULT1,
COM41closed. Board is pre-reporting-fix engagedrybuild. Raw/CSV/coast/plot/pre/
safe retained. No poweredBEMFhandoff attempted.

## Entry 114 — 2026-09-12 — Acquisition succeeds unchanged; dry setup window needs shortening

Source inspection corrects tentative UART/blanking explanation: normal
cohort timer/wallclocklimit exit does notprint before acquisition; only the
legacy sine_tickslimit branch prints. Thus no demonstrated UART-created
blanking difference. Did not add arbitraryblanking or relax acquisition.

Unchanged E113board engagedry_200_02:6.5%catch/drive,800mAsetting,existing
numericguards,15acknowledged50->200steps. Offat~4.7s;reportedtransition
4,711,659us includesdisabledacquisition/setup. Bus11.467..11.869V,pulse
peak9.381A,no fault samples. Acquisition succeeds:step1,interval1700ticks,
edge22104ticks,elapsed11107us,726samples,maxphasegap48us,cancelled0,12intervals.
ENGAGEDRYpathresult2 setuprefusal;POWERPATHreason0,ISRmax3us,active0,disabled1.
Guard started,so setup failure is in subsequent seed/arm timing checks;
oldfirmware didnot capture rejectedage,so lateage vs >16usarm not directly
measured. No referenceprogress claimed. Latercoastfit192.95eHz,R2=.99993,
order1.000; fit origin resets afterdisabledacquisition,not originaldisable.

New unflashedsource moves timer/counter observe_begin initialization before
acquisition,withoutputs/COMPIRQsdisabled; measuredstep/polarity reset after
acquisition. ADCbaseline+guardstart still run aftermeasurement. This removes
setup work from freshedge->COM deadline rather than permitting lateseeds.
Added FLY_ATTEMPT and rejectedage/ARRsentinel diagnostics;setuprefusal reports
COASTSTOP8,CORESEEDarmed0,not silence. Buildpasses existing warnings. Need
benchmeasure before claiming fix; no poweredwriter caller introduced.

Earlydry attempts01/02 bothretained (acquisitionrefusal/setuprefusal);do not
mix with E109eight-runcohort or label thempoweredengages. Raw/CSV/coast/plot/
pre/safe retained. Postoff/engagedry0/p/i allsixgates,ENABLE,MOE,CCRs0,nFAULT1,
COM41closed. Board remainsE113pre-reportingfixbuild,lastverifiedoff02.

## Entry 115 — 2026-09-12 — Measured remaining setup shortfall: 9.5us

Flashed E114 preinitializedreference build after freshsafechecks. UARTclock
again08000000 withPD1ODR0/BDTR0c1a/CCR0;restored08040000 whiledisabled,
then normaloff/p/i. Binarytext129592,data20,bss32952. Same6.5%/800mAsetting/
50->200sweep,current1200counts,bus8.4V,nFAULT/timing/five-secondguards.

engagedry_preinit_200_01:normalpoweredexit,transition4,709,491usincludes
disabledwork;bus11.281..11.945V,pulse9.331A,no fault. AcquisitionTooSlow
after8840us,nineintervals,578samples,maxphasegap48us. Guard/reference not
started. Latercoastfit192.97eHz,R2=.99994 (post-acquisitiontimeorigin).

engagedry_preinit_200_02:normalpoweredexit,transition4,711,637us,bus10.923..
11.856V,pulse7.834A,no fault. Acquisitionstep3,ci1692ticks,edge21242ticks,
elapsed10675us,698samples,maxgap48us,cancelled0,twelveintervals. Setuprefused
COASTSTOP8,armed0,noCOM. Measurededgeage378ticks=189us;referencewait423ticks
leaves45ticks=22.5us,below64ticks=32us requiredmargin by9.5us. No guardfault,
ISRmax4us. This specificallymeasures age-marginrefusal,not rotorloss.

Moved TIM6guard PSC/ARR/RCC/priority setup into prepare_early beforeacquisition
as well (newunflashedsource). start_prepared still validatesdisabledstate,
freshADCbaseline,originalbudget and starts guardtimer only afterseed;fresh
deadline checkunchanged. Standardguardcheck start retainsfullsetup path.
Buildpasses. Must benchverify timing gain; do notclaim dryintegrationpass yet.
Bothattemptsretained,not cherry-picked replacementspasses. Postoff/engagedry0/
p/i _safe filesverify allsixgates,ENABLE,MOE,CCRs0,nFAULT1. COM41closed.
Board remainsE114build,lastsafe preinit02. No poweredreferencegateauthority.

## Entry 116 — 2026-09-12 — Timer preconfiguration saves5us; pipeline ADC before seed

FlashedE115afteroffchecks,UARTbootnormal. engagedry_fastarm_200_01 uses
unchanged6.5%catch/drive,800mAsetting,50->200steps,existingguards. Powered
phase normal,transition4,711,626usincludesdisabledwork,bus11.375..11.891V,
pulsepeak10.045A,no fault samples. Acquisitionstep3,ci1692ticks,edge21424,
elapsed10766us,704samples,maxphasegap48us,12intervals,cancelled0. Setup
refusedarmed0,COASTSTOP8,edgeage368ticks=184us:5usfasterthanE115capture,
but remaining55ticks=27.5usstill4.5usshortof32usmargin. Guardreason0,
ISRmax0,noCOM. Keepfailurerecord;not an engage-ratepass.

Next sourcechange (unflashed) pipelines one bounded ADC conversion after
each fullthreephase acquisition scan,roundrobin4/1/0/6/13,only forengagedry.
Ordinarycoastfly/coasttrack unchanged. Baseline stores each conversion's
starttimestamp,validmask;requiresallfive andoldestage+200ussetupallowance
<=1ms. Guard inherits thisage instead of pretending itwas sampled atstart.
Removes the blockingfive-channelbaseline scan from freshedge->COM setup.
Existing100usper-phasegap andorderedinterval checks remain; ADC-loaded
acquisitioncadence must nowbe hardwareverified. ADCtimeout reportsFLY12.
FullADCfeedbackcontinues duringreferencewindow once started. New age test
proves stale baseline cannot refreshatguardstart.57Rusttests pass,release
buildpasses. No newpoweredwriter call.

Postoff/engagedry0/p/i saved _safe: allsixgates,ENABLE,MOE,CCRs0,nFAULT1,
COM41closed. Raw/CSV/coast/plot/pre/safe retained. Board remainsE115fastarm
build,lastverifiedoff01;ADCpipeline source not flashed yet.

## Entry 117 — 2026-09-12 — Integrated dry handoff reaches tracking; ADC timeout stops12ms

FlashedE116ADCpipeline aftersafechecks,UARTbootnormal. Binarytext130060,
data20,bss32976(~992flashbytesleft). engagedry_pipeline_200_01 same6.5%/
800mAsetting/50->200steps/numericguards. Normalpoweredphase,reportedend
4,723,096usincludesdisabledwork;bus11.425..12.080V,pulse11.335A,no fault.
Acquisitionstep5,ci1701ticks,edge21232ticks,elapsed10687us,523samples,
maxphasegap126ticks=63us (<100uslimit),12intervals,cancelled0.

Handoff SUCCESSFULLY ARMED: age274ticks,remainingARR151=75.5us,arm4us.
This clears original32usmargin without relaxing it. RealADCbaseline age
preserved,referenceEV_ACC connected to independentlyrunningTIM6guard.
Reference progressed14COM with14acceptedinputrecords (one initialbootstrap,
onefinalacceptedCOM not yet serviced),no desync/polling,avg1761ticks,
acceptancemonitorfault0,observedprefix12293us. Independentguard stopped
at12260us reason11ADCtimeout;ISRmax4us. Not full20ms success orpoweredlock.

Timeoutstage not captured in this build. Boundedreader currently has three
refusal sites: ADSTARTalreadybusy,channelreadywait,conversionwait. Added
unflashedADCFAULT snapshotstage/channel/elapsed/CR/ISR to distinguish them;
buildpasses. Do not assume analogfault or simply increase timeouts without
checking actualfailure. A configuration delay can consume currentshared
100usbudget before conversion launches; this is a source-level possibility,
not yet measuredcause. Latercoast C-neutralspan1264mV is anomalous afterADC
timeout; do not use it as BEMFamplitude or trust channelidentity without
ADCstate cleanup. Comparator phaseorder remains1;coastfit usesneworigin.

Raw/CSV/coast/plot/pre/safe retained. off/engagedry0/p/i verify sixgates,
ENABLE,MOE,CCRs0,nFAULT1,COM41closed. Board remainsE116pipelinebuild,lastoff01.
Poweredcommitstillhasnocaller; no poweredreferenceattempt. Next resolve ADC
transactiontimeout/cleanup and finish guarded drywindow before gateauthority.

## Entry 118 — 2026-09-12 — ADC deadline bug measured and fixed; guarded dry window passes

Three new physical attempts retained, all shell-pwm / current DRV wiring,
800 mA PSU setting, 6.5% catch and drive, acknowledged 50->200 eHz steps,
existing 1200-count phase peak / 8.4 V bus / nFAULT / timing / five-second guards.
Reference handoff remains bridge-disabled, not powered closed-loop authority.

1. `engagedry_adcstage_200_01`: acquisition step4, ci1691 ticks, max phase
   gap63us; handoff armed, then ADC timeout at4307us. ADCFAULT stage3,
   channel13 (VREFINT), elapsed134us, CR=0x10000005, ISR=0x280b: conversion
   still active. Bus11.491..11.869V, phase pulse peak5.820A, no fault samples.
2. `engagedry_adclaunch_200_01`: added launch timestamp and bounded cleanup.
   Acquisition step6 ci1701, handoff armed. ADCFAULT stage3 channel13:
   launch164us, failure165us. This proves conversion started after the shared
   100us deadline had expired and was rejected only1us later. Guard stopped
   at2518us. Bus11.477..11.917V, pulse7.985A, no fault samples. Cleanup waits
   at most100us for the single conversion to finish after safing/IRQ shutdown,
   drains DR and clears EOC/EOS/OVR; a wedged ADC panics safe instead of
   returning to the unbounded legacy reader. Later C-neutral span28.3mV,
   versus1219.7mV without cleanup in attempt1. Consistent with the prior
   anomalous coast data being transaction contamination, not physical BEMF.
3. `engagedry_adcfix_200_01`: conversion now gets its own100us wait beginning
   after launch, separate from the100us channel-selection wait. The independent
   guard's 1ms full-feedback freshness limit is unchanged. Full dry window PASS:
   acquisition step6 ci1701, 12 intervals, gap63us, seed age274ticks,
   ARR151ticks, arm4us. POWERPATH reason2 (planned segment deadline),
   stop20005us, ISR max15us, disabled1. COASTREF stop1, no desync/polling,
   running1; accepted-event CRC decode gives22 ordered accepts,23 COM events
   including bootstrap, average1800ticks. First accept823us, inter-event
   gaps815..975us, last19471us, observed end20037us. ACCEPTTIMING fault0.
   Bus11.162..11.999V, phase pulse peak9.734A, no fault samples. Later
   coast C-neutral span24.3mV. Its time origin follows the dry interval;
   the old 45..55Hz fixture verdict is not this campaign's acceptance gate.

All attempts ended with explicit off/engagedry0/p/i: six gate inputs,
ENABLE, MOE, CCRs all zero; nFAULT1. Safe transcripts retained; pre transcripts
for attempts2/3 retained. COM41 closed. Before the final run, post-reset UART
was silent: read APBENR1=08000000, PD1ODR0, BDTR0c1a, CCR1..3=0, then restored
only USART3 clock bit (08040000). Normal shell acknowledgements followed.
First two resets had normal UART. No powered run used an unacknowledged shell.

Current board and source are the ADC-fix build: text130356/data20/bss33004.
Release build and57 Rust tests pass; these host tests do not exercise the
hardware ADC waits. The physical attempt above is the ADC integration evidence.
No duty expansion or powered reference commit yet. Next: post-trip writer
refusal qualification, then a short guarded powered reference handoff.

## Entry 119 — 2026-09-12 — Post-stop writer refuses all sectors; flash headroom recovered

Flashed the post-stop guard diagnostic after off/p/i verification. UART boot
normal. `guardcheck` now invokes the actual `powered_timer::commit(step,65)`
for steps1..6 after each independent guard stop. All18 calls refused; each
POSTSTOP reports6 refused, disabled1. Mode0 feedback-stale reason4 stopped
1005us, ISR max10us; mode1 missing-event reason8 stopped1006us, max10us;
mode2 campaign-deadline reason1 stopped305us, max9us. Host backstop0 for all.
Synthetic feedback only; no successful gate write or motor spin in this test.
This qualifies refusal after completed shutdown, not arbitrary interrupt-race
interleavings or the critical-section cost of a successful powered commit.

`captures/guard_poststop_01.txt` retains diagnostic and post off/p/i. All six
gate inputs, PD1 ENABLE, TIM1 MOE and CCRs zero, nFAULT1. COM41 closed.
Board remains this full-probe post-stop build (text131024/data16/bss33012),
only32flash bytes free. No powered reference caller yet.

New unflashed build organization: six old ADC characterization shell commands
are optional behind Cargo feature `bench-adc-probes`; source and feature build
preserved. Normal ADC feedback/current guards, capture, replay and observer
remain in default firmware. Default text123080/data16/bss33012 saves7944bytes.
Both default and feature-enabled release builds pass. SHELL_PWM.md and AGENTS
document the feature and guarded commands. Default binary is currently built,
but not flashed. Next implement and audit short powered reference handoff
using the qualified guarded writer; do not confuse this lockout check with
powered closed-loop qualification.

## Entry 120 — 2026-09-12 — First powered reference commit; ENABLE wake-up stop

Implemented explicit idle-only `engage1`/`engage0` alongside unchanged
`engagedry1`. After the existing4.7s open-loop prefix and measured disabled
acquisition, real minz reference COM calls `powered_timer::commit` with the
run's duty. No bypass writer. Commit rechecks guard deadlines/freshness and
nFAULT in its atomic write section; late COM after shutdown cannot revive
gates or re-enable comparator IRQ dispatch. Count successful writes, vetoes,
write cost and powered feedback extrema separately from open-loop captures.
First experiment remains20ms maximum and<=10% firmware ceiling.

Flashed normal release after off/p/i. UART normal. Repeated `guardcheck` on
this exact build: all18 post-stop sector writes refused; modes4/8/1 at
1006/1006/304us, max ISR9/10/9us, disabled1 and host backstop0. Pre transcript
saved with `captures/engage_power_200_01_pre.txt`.

One powered attempt: `engage_power_200_01`, current DRV map,800mA PSU setting,
6.5% catch/drive, all15 acknowledged50->200eHz steps. Open-loop bus11.316..
12.149V, retained phase pulse peak9.741A, no fault samples. Acquisition step4,
ci1701ticks,12intervals,max phase gap63us. Seed age278ticks, ARR147ticks,
arm4us. Actual reference bootstrap commanded step5; POWERCOMMITS applied1,
commit max20us, COM ISR48us. POWERPATH reason7 (nFAULT) stopped205us, max
guard ISR15us, veto0. One complete feedback scan: peak_abs_raw189 counts,
bus_min11665mV. No BEMF accepts before stop; COASTREF stop5, COREOBS com1,
gate_authority1. ACCEPTTIMING fault0 over238us/zero intervals is NOT a pass.
Later coast confirms motion, but neither coast nor one gate commit proves lock.

Read cached `drv8304.pdf` pages6 and35 (TI SLVSE39B): tWAKE maximum1ms;
device must finish wake-up before accepting inputs; nFAULT is held low while
internal regulators enable/disable. Source currently raises ENABLE at the
first PWM commit after~11ms disabled acquisition. Therefore it violates the
documented wake-before-input requirement. The205us nFAULT stop is consistent
with this documented transition, not evidence of a rotor-tracking problem.
No fault identity register is available on this H interface, so do not claim
all other fault causes were electrically excluded. Do not relax nFAULT.
Next: wake with all gate inputs/MOE off BEFORE fresh-seed acquisition, then
preserve awake state through guard arming; every rejection still EN/MOE off.

Final off/engage0/p/i saved `_safe.txt`: all six inputs, ENABLE, MOE and CCRs0,
nFAULT1. COM41 closed. Board remains powered-entry release, no further run.
57 Rust tests and55 Python tests pass. Tests cover pure reference/policy and
replay, not all hardware callback interleavings. Raw/CSV/coast/plot retained.

Full optional ADC-probe build overflowed release flash by4320bytes after the
new path. Added dedicated `bench-probes` profile (inherits release, opt-level=s)
and documented its command/path. Full-feature size build passes at text78924,
data20,bss33096; NOT flashed, and its timings are unqualified for motor work.
Normal motor release remains the measured profile. No duty expansion or
closed-loop success claimed.

## Entry 121 — 2026-09-12 — Awake acquisition fixes nFAULT stop; seven powered COMs

Implemented bounded wake before fresh-seed acquisition: all six gate inputs
and MOE remain low for1.1ms, any UART byte aborts, and nFAULT must be high at
completion. Only that no-PWM wake window tolerates documented regulator
transitions. Explicit awake acquisition checks ENABLE/nFAULT and all gates/MOE
every phase visit; successful acquisition preserves ENABLE into guard arming.
Every refusal/timeout and final exit lowers ENABLE. Ordinary coast/dry probes
keep their prior disabled-driver behavior. Commit-time nFAULT guard unchanged.

Flashed normal release after off/p/i. Silent post-reset UART: APBENR1 read
08000000, PD1ODR0, BDTR0c1a, CCRs0; restored USART3 clock08040000, drained
boot garbage and received normal shell acknowledgements before any run.

`engage_awake_200_01`: same current DRV map,800mA PSU setting,6.5% catch/drive,
all15 acknowledged50->200eHz steps. Open-loop retained bus11.460..11.922V,
phase pulse peak10.844A, no fault samples. DRVWAKE completed1101us. Acquisition
step5 ci1702ticks,12intervals,max phase gap64us; FLY disabled0 because ENABLE
is intentionally high, but all gate inputs/MOE stayed low. Seed age278ticks,
ARR148ticks,arm4us. No nFAULT stop after this correction.

Actual powered reference ran7 COMs, writer max20us, full COM ISR48us. Guard
stopped5516us reason8 (tracking), ISRmax4us. Powered ADC26scans,
peak_abs_raw456 (<1200), bus_min11462mV (>8400). Seven accepted records:
752/1677/2402/3315/4029/4943/5559us, steps6/1/2/3/4/5/6. Last rejected
acceptance is logged after safing overhead, hence after guard-stop timestamp.
No desync/polling; avg1686ticks. Last inter-record gap616us violates our fixed
666us minimum; the independent guard prevented its scheduled COM from firing.
Seven commits include bootstrap plus six accepted-event COMs, NOT seven
uninterrupted complete electrical cycles. Full20ms test FAIL, not lock.

Intervals alternate longer/shorter:925/725/913/714/914/616us. Same-sector
span752->5559 is4807us (~208eHz), within proposed167..250eHz range despite
the short final individual interval. This is evidence that per-sector timing
alone cannot establish rotor overspeed. It does not yet exclude switching
artifacts or loss of tracking. Reference blanking accepts after avg/2 timer
ticks, not a fixed666us; before changing the extra guard, compare a rolling
same-sector/full-cycle bound plus ordering/staleness against reference replay.
Do not simply delete the guard or describe absence of desync as lock.

Raw/CSV/coast/plot/pre/safe retained. off/engage0/p/i verify six gates, ENABLE,
MOE, CCRs0 and nFAULT1; COM41 closed. Later coast span34.8mV and ordered phase
events confirm continued motion, but delayed coast fit is not powered lock.
Current board is awake-handoff release. No duty expansion. Next resolve the
independent tracking envelope using cycle-level evidence, then resume bounded
powered tests rather than repeating the wake-up diagnosis.

## Entry 122 — 2026-09-12 — Cycle guard reaches22 powered COMs; motor accelerates to250eHz

Added full-reference logical-time replay of E121 interval register values
1672/1846/1447/1825/1425/1815/1216ticks. Actual reference COMP/COM/blend/average/
desync sequence accepts all7 with IRQ mode maintained and no desync. Synthetic
post-level input proves timing legality only, NOT physical BEMF provenance.

Revised independent guard: ordered333..1000us inter-event gaps, unchanged1ms
missing-event deadline, plus every same-sector return4000..6000us (rolling
six timestamp slots).333us corresponds to avg/2 blanking at250eHz; electrical
speed is bounded by full cycles instead of assuming six equal event intervals.
New CycleTiming fault is POWERPATH12. Host tests cover E121 alternating times,
fast trains with individually legal gaps, cycle boundaries, wrapping timestamp
and short-spike rejection.61 Rust tests pass. All current/bus/fault/time guards
remain unchanged. Old ACCEPTTIMING output remains explicitly report-only with
the historical narrower band; it is not the new guard's pass/fail verdict.

Flashed cycle build (text129872/data80/bss33000) after safechecks. Post-reset
UART clock recovery only after reading APBENR1=08000000, PD1ODR0, BDTR0c1a,
CCRs0; restored08040000, then acknowledged shell commands. `guardcheck` on
this build passed all18 post-stop refusals: reasons4/8/1 at1005/1006/305us,
ISRmax10/10/9us, disabled1 and host backstop0.

`engage_cycle_200_01`: current DRV map,800mA setting,6.5% catch/drive, all15
acknowledged50->200steps. Open-loop bus11.270..12.046V, pulse9.845A,no fault.
Wake1100us; acquisitionstep5 ci1701,12intervals,gap63us,age294ticks,ARR131,
arm4us. Powered22COMs over15948us, then CycleTiming12; write20us, COM48us,
guardISR4us, ADC80scans,peak_abs_raw499 (<1200),bus_min11522mV. No nFAULT,
desync or polling; avg1364ticks at stop. Last same-sector span in logged
records step3:7898->12085->15999us, final3914us (~255eHz); final record has
post-trip logging overhead. Later coast fit~258eHz supports actual acceleration,
not merely a lone premature edge. Still NOT full20ms pass or repeatable lock.

`engage_cycle_d60_200_01`: same firmware/speed steps and6.5% catch, but6.0%
run/hold (which also selects handoff duty). No valid spin evidence at end:
acquisition WrongOrder366us/zero intervals, no powered guard start/COM; later
coast no comparator edges. Open-loop bus11.491..12.046V,pulse4.590A,no fault.
This changed the open-loop waveform too, so cannot isolate handoff duty.
Do not keep repeating an insufficient drive profile as a lock test.

Both raw/CSV/coast/plot/pre/safe captures retained. Post off/engage0/p/i verify
all six gate inputs, ENABLE,MOE,CCRs0 and nFAULT1. COM41 closed. Board remains
cycle-guard firmware, last verified off d60_01. No duty expansion.

New unflashed source `engageduN` independently selects handoff duty (tenths%,
max100); engagedu0 follows run duty. Normal6.5% spin-up can now be retained
while testing a6.0% reference segment. Requested duty explicitly reported;
successful commits remain separate. Release build passes. Next test that
isolated handoff change, then qualify a fixed cohort if the full window passes.

## Entry 123 — 2026-09-12 — First complete powered20ms window; IRQ lifetime cap removed

Flashed independent-handoff-duty build after safechecks. `engagedu60` with
6.5% catch/run and all15 acknowledged50->200eHz steps isolates6.0% reference
drive from startup. Both flashes this entry needed the known UART clock repair:
APBENR1 read08000000, PD1ODR0, BDTR0c1a, CCRs0; then write08040000 and wait
for normal acknowledgements before running. Current DRV map,800mA setting.

`engage_trim60_200_01`: acquisition step5 ci1702,12intervals,gap64us; wake
1100us,seed age296ticks,ARR130,arm4us. Powered24COMs and23accepts over17486us,
ADC91scans,peak_abs_raw509,bus_min11354mV,write20us. No guardfault/desync/
polling; stopped COASTREF6/COREOBS5 at COMP_CALLS257. Source proves this is
the old256 lifetime interrupt quota, not a measured starvation rate. Open-loop
bus10.503..11.975V,pulse8.897A,no fault samples. Failed full-window attempt.

Replaced lifetime quota ONLY for powered sessions with64 calls per fixed1ms
bucket. Diagnostic modes retain prior256 quota. Exceeding the rate masks and
clears COMP, stops reference and immediately aborts guarded output. Independent
1ms feedback/accepted-event age and200us guard-tick rules remain. Bucket
boundary can admit two bursts; the independent timing guards still apply.
New pure tests verify2000 sustained calls at70us cadence without lifetime
cutoff, timestamp wrap, burst65 rejection and permanent fault latch.63 Rust
tests pass (42policy+1recorded+20sequence). Full old build overflowed192bytes;
`obstiming`/`obstiming200` moved into optional characterization feature,
preserving normal observer/guards. Normal release nowtext124208/data80/
bss33008. Size change is compiler/layout sensitive; timings remeasured below.

`engage_rate60_200_01`: SAME6.5% catch/run, engagedu60,800mA,50->200 steps.
First full powered20ms PASS: wake1100us, acquisitionstep2 ci1702,gap64us,
12intervals; seedage296ticks,ARR130,arm4us. POWERPATH2 planned segment
deadline stopped20005us,guardISRmax16us,write20us,veto0,disabled1.28applied
COMs (bootstrap+27accepted-input COMs),27ordered accepted records,no drops.
COASTREF1,COREOBS1,no desync/polling.99powered feedback scans,
peak_abs_raw403 (<1200),bus_min11522mV. IRQRATEpeak19/ms (<64). Open-loop
retained bus11.490..11.972V,pulse9.187A,no fault samples.

Accepted timestamps855..19779us; same-sector periods trend5010 toward4005us,
so this passing20ms segment approaches the250eHz bound rather than proving
steady200eHz speed. Referenceavg1346ticks atstop. Later coast indicates speed
near250eHz but is delayed and not a lock certificate. Historical report-only
ACCEPTTIMING may still mark narrow-band failure; actual cycle guard did not.
This is one successful short powered handoff, NOT a completed repeatability
cohort, long-duration lock, recovery qualification or minz performance parity.

Bothattempts raw/CSV/coast/plot/pre/safe retained. Post off/engage0/engagedu0/
p/i verify allsix gateinputs,ENABLE,MOE,CCRs0 andnFAULT1; COM41closed. Board
is rate-limited powered build,lastsafe rate60_01,override reset tofollowrun.
55Python replay tests pass. Next measure repeatability/margin with startup
unchanged, then extend powered duration only after stable tracking in range.

## Entry 124 — 2026-09-13 — Restart repair, powered cohort 7/8, longer-window authorization

Found a repeat-run configuration bug: six-step drive leaves TIM1 CCMR/CCER
roles installed, while the next sine startup previously changed only CCRs.
After failed `engage_rate60_200_02`, register readback CCMR1/CCMR2/CCER was
00004040/00000068/00000505, not the three-phase sine configuration.
Added `prepare_sine()` to both run and direct sine entry: gates and ENABLE off,
restore 6868/68/555, issue update, verify readback before enabling.
This supersedes Entry 122's inference that the failed d60 startup demonstrates
insufficient duty: that repeat was contaminated by retained six-step modes.

Flashed repaired release: text124200/data80/bss33008. `engage_restore60_200_01`
and `_02` both acquired and powered without a reset between runs; cycle guard12
stopped them at18366/19707us (25/27 COMs). Retained6.5% catch/run and reduced
only powered handoff duty to5.5% for a fixed eight-attempt cohort.

Artifacts: `captures/engage_restore55_200_01` through `_08` (raw, CSV, coast,
plots). Current DRV wiring, PSU800mA,50->200eHz in10eHz live steps, same firmware,
no resets/flashes between attempts. All eight acquired valid seeds. Seven
completed the20ms powered window: 01/02/03/04/06/07/08. Each has27 applied COMs
and26 CRC-valid, ordered accepted events, no log drops. Same-sector periods
across these seven range4143..5004us (~200..241eHz); still accelerating, not
proof of steady-speed lock. Successful feedback99..102 scans, peak_abs_raw
389..471 (<1200), bus_min11438..11557mV; IRQ peak20..24/ms (<64).
02/06 ended via foreground COASTREF1 with POWERPATH0 and observed_end>=20000us;
the other successes hit planned segment deadline2. Both are normal completion.

Attempt05 failed tracking guard8 at1706us: two COMs, two ordered CRC-valid
accepted records, no full-cycle measurement. Feedback7 scans, peak_abs_raw350,
bus_min11510mV. Late second record is after guard trip/logging overhead; do not
count it as timely progress. No desync is not a pass. Cohort result is7/8, not
repeatable long-duration lock or parity. All eight post-run OUT/IN records
pass host verification: six gates, ENABLE, MOE and CCRs zero; nFAULT high.
Last verified state is08 off, handoff override cleared, UART closed.

Host fixture now supports explicit `--engage-duty-tenths`, acknowledged arming,
pre/post verified off readbacks retained in raw captures, and cleanup of the
override. Added readback rejection tests;57 Python tests pass.

Operator now authorizes10s as the default campaign duration, superseding the
earlier5s instruction, while retaining short diagnostic tests. NOT implemented
or flashed yet: current campaign ceiling5s, actual powered-reference window20ms.
Extension must spend additional time under BEMF control, not merely open loop.
Audit found powered_timer uses a16-bit1MHz elapsed clock (wrap65.536ms), core
has short elapsed-time checks and32 accepted-event slots, and host duration is
capped at5s. Update timebase, bounded capture/aggregates, both deadline paths and
tests together. Retain missing-event/current/bus/fault/ISR guards and the measured
cycle-speed bound;5.5% already accelerates toward it, so longer runs need a
stable operating point rather than relaxed guard thresholds.

## Entry 125 — 2026-09-13 — Powered elapsed-clock extension (unflashed)

Implemented `sampled_clock::Clock` and integrated it into powered_timer.
It accumulates actual modulo16-bit TIM17 differences into32-bit elapsed time;
hardware reads and clock updates are in the same critical section, preventing
an interrupted foreground read from following a newer ISR sample. Reset occurs
only on session start. The100us guard samples it continuously; dispatch delays
are preserved as measured elapsed time, not replaced by nominal tick counts.

Four pure tests cover10s across153 hardware wraps, irregular/repeated reads
including a201us dispatch gap, fresh-session reset, and32-bit software wrap.
67 Rust tests pass (46policy/unit,1recorded,20reference sequence); no_std M0
library check and normal release shell-pwm build pass. Existing compiler warnings
remain. shell-sine SHA256 unchanged:
B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.

This is a sampled extension, NOT a watchdog for CPU/interrupt blackouts longer
than65.536ms: it requires a sample within each hardware wrap. Independent200us
dispatch checks remain, but cannot reconstruct an entire missed hardware wrap.
That limitation also existed in the original16-bit guard clock.

No flash or motor run this entry; board remains Entry124 firmware and last
verified off state. New critical-section cost must be remeasured on hardware.
No deadlines raised:5s campaign and20ms reference still enforced in source.
Remaining extension work includes core observation timestamps (currently16-bit),
bounded accepted-event evidence beyond32 records, duration plumbing/guards and
host fixture. Do not interpret the10s synthetic clock test as motor qualification.

## Entry 126 — 2026-09-13 — Duration is an engineering choice, not permission gate

Operator explicitly clarified: choose test duration as needed, including
multi-minute frequency/duty sweeps and longer steady-setting runs. There is
nothing special about5s or10s. This supersedes the original goal's5s ceiling
and Entry124's framing of10s as an authorization ceiling. Do not request
permission for each duration. Choose finite experiment timeouts and useful
observation windows from the measurement, with continuous current/bus/fault/
tracking protection and host abort; retain verified-off completion.

Firmware's existing5s/20ms limits are implementation constraints to remove
coherently, not operator restrictions. This clarification does not erase the
goal's measured-current/tracking gates for duty expansion or authorize disabling
safety guards. No additional hardware action or new motor result in this entry.

## Entry 127 — 2026-09-13 — Extended observer clock; short-run hardware regression passes

Extended core observation elapsed time with the same serialized sampled clock
as powered_timer. Session reset, accepted-event32-bit timestamps, stop age,
desync/dispatch snapshots, mode-change time and foreground/polling deadlines
now consume extended time. Legacy16-bit prefix IRQ/history encodings remain;
their format and limited storage still need explicit treatment before long
captures. Reference commutation/interval/filter algorithms are unchanged.

Normal release builds. Flashed after UART OUT/IN off verification. The known
post-reset UART clock issue recurred: APBENR1=08000000, PD1ODR0, BDTR0c1a,
CCRs0 read before restoring APBENR1=08040000. Normal shell acknowledgements
and off readbacks then returned. No motor command issued while UART silent.

Disabled `guardcheck` passed all18 post-stop writer refusals. Synthetic stale
feedback, absent accepted event and campaign deadline stopped at1005/1006/304us
with reasons4/8/1, ISRmax10/10/9us, host_backstop0 and disabled1 for all three.
These test dispatch/safing, not analog calibration or multi-wrap uptime.

`captures/engage_clock55_200_01` retains raw/CSV/coast/plot and pre/post off
readbacks. Same current DRV map,800mA,6.5% catch/run,5.5% powered duty,
50->200eHz via15 acknowledged10eHz increments. Acquisition successful with
12intervals,ci1702ticks,max phase gap64us. Powered window completed20ms:
POWERPATH2 at20005us,27COMs,26 CRC-valid ordered accepted records/no drops,
COASTREF1,COREOBS1,no desync/polling. Feedback101 scans,peak_abs_raw444,
bus_min11080mV; guardISRmax16us,commit19us,veto0,IRQpeak21/ms. All safety
thresholds unchanged. Open-loop bus11.448..11.893V,pulse8.735A,no fault samples.

Post-run gates/ENABLE/MOE/CCRs zero and nFAULT high verified; override cleared,
COM41 closed. Board now runs extended-clock firmware. This is a hardware
timing regression pass, not a longer-duration test: campaign still5s and powered
reference still20ms. Remaining work is bounded long-capture evidence and
configurable durations, then sustained operating-point/recovery qualification.

## Entry 128 — 2026-09-13 — Bounded suffix capture; sustained-lock consult reviewed

Unflashed source adds a32-record suffix ring after the existing32-record startup
prefix. Fixed memory, constant-time push; chronological suffix dump after stop.
Existing ACCEPTLOG/A85 remains and reports prefix overflow as before, so legacy
complete-stream replay still refuses truncated data. New ACCEPTTAIL/T85 reports
suffix count, total accepted records and exact omitted-middle count, with the
same per-record CRC and full32-bit microsecond timestamp. No ISR formatting.

Host `drv_accepted_events.py --windows` explicitly decodes the two windows,
checks CRC/count/accounting/sector order and timestamp order, and retains the
middle gap rather than joining it into a fictitious continuous trace. Tests
cover empty/partial/full/overwritten buffers,144000 events (~2minutes200eHz),
long timestamps, corrupt/missing records and legacy overflow rejection.
69 Rust tests and60 Python tests pass; normal release builds. No new flash/run:
board remains Entry127 extended-clock firmware, last verified off clock55_01.

Read GRAYBEARD_SUSTAINED_LOCK.md in full. Adopt diagnosis before attributing
failures to variance; require whole-run timing variation/dropout evidence and
independent rotor-rate support, not elapsed runtime/no-desync alone. Start/end
windows are forensic evidence only, not whole-run quality or harmonic exclusion.
Whole-run aggregates/time bins and configurable duration remain to implement.

Applicability corrections checked against current source:
- Actual minz-core am32_isr COM disables its own interrupt; the separate
  bemf_timeout_rekick waits BEMF_TIMEOUT_TICKS. Do not import FALCON's presumed
  free-running missed-window cascade into this one-shot scheduler (see E035
  and prior memo corrections). Current independent1ms tracking stop preempts
  the reference22.5ms recovery; recovery policy/qualification remains unresolved.
- Accepted/COM ratio alone is not independent qzc here: accepted input normally
  schedules the next COM. Do not label this ratio a rotor-lock certificate or
  claim harmonic exclusion from it. Establish a meaningful denominator and
  cross-check rotor-related rate independently.
- TIM6 priority0 guards feed powered_clock every100us; COMP priority0x40 cannot
  preempt TIM6. The IRQ-rate guard is useful but not the sole clock-liveness
  mechanism. Long global interrupt masking still defeats sampled-wrap recovery.
- Powered guard mutations and now() hardware sampling are serialized in critical
  sections. Preserve this ordering rather than adding a top-bit clamp that could
  hide a genuine stale timestamp. Review each other watchdog caller separately.

Fixed5.5% remains an accelerating operating point, not a governor. The current
speed bound stays active when longer durations are introduced; do not loosen it
merely to obtain a longer-looking pass.

## Entry 129 — 2026-09-13 — First sustained powered operation: two10-second runs

Implemented selectable powered window `engagems20..600000` (ms, idle only,
default1000) and fixture `--engage-ms`. Startup stays bounded5s; prepared guard
uses selected segment deadline and total backstop5s+segment, carrying prior
elapsed time plus200us dispatch reserve. Existing diagnostic constructor keeps
its5s/20ms defaults. Foreground campaign clock is serviced during long reference
execution. All current/bus/fault/1ms tracking/cycle-speed/200us dispatch rules
remain unchanged. Host timeout grows with requested duration, handshake is
mandatory, cleanup restores1000ms and verifies off.600000ms is a finite interface
engineering range, not a newly invented operator permission boundary.

Added whole-stream event/gap/same-sector-cycle integer moments, order-error count,
first/last timestamps and excluded-gap count. No division/float/formatting on
event path;64-bit sums avoid32-bit overflow. Gaps>65535us are explicitly excluded
from moments (counted), not silently multiplied with overflow. Prefix/suffix
CRC records remain separate. Host tests reconstruct exact moments from complete
short hardware streams and verify10s suffix timestamps/accounting.73 Rust tests
and62 Python tests pass; normal release builds. Two-minute synthetic policy
test verifies continuous guards and exact deadline, not physical qualification.

Flashed after off checks. Known post-reset UART recovery: read APBENR1=08000000,
PD1ODR0,BDTR0c1a,CCRs0 before restoring08040000. Guardcheck all18 writer refusals
passed: reasons4/8/1 at1006/1006/305us,ISR9/10/9us,host_backstop0,disabled1.

All following attempts: current G071/DRV map,800mA PSU setting,6.5% catch/run,
50->200eHz via15 acknowledged10eHz steps, same firmware, no resets between runs.
Only handoff duty and requested powered duration changed. Raw/CSV/coast/plot
and verified pre/post off readbacks retained under each capture tag.

| Capture | Handoff | Requested | Actual powered result |
|---|---:|---:|---|
| sustain55_200_01 |5.5%|1s|CycleTiming12 at26394us,36COMs; coast~251eHz|
| sustain50_200_01 |5.0%|1s|CycleTiming12 at45182us,62COMs; coast~250eHz|
| sustain45_200_01 |4.5%|1s|Deadline2 at1000005us,1410COMs; coast~236eHz|
| sustain45_200_10s_01 |4.5%|10s|Deadline2 at10000005us,14192COMs|
| sustain45_200_10s_02 |4.5%|10s|Deadline2 at10000005us,14215COMs|
| sustain45_200_10s_03 |4.5%|10s|Tracking8 at1800us,2COMs; early handoff failure|

5.5/5.0% accelerate to the speed bound; coast independently supports actual
rotor acceleration rather than accepting a doubled controller-rate claim.
Guard event timestamp precedes post-trip capture/format bookkeeping:55's final
logged cycle4001us does not contradict guard's sub4000us event-time decision.
Do not use a post-trip record to erase a missed deadline.

The two10s completions retain14191/14214 accepted events, zero sector-order
errors, no desync/polling, no tracking/driver/current/bus stop, no excluded gaps.
Whole-run same-sector cycle mean4227.600/4220.726us (~236.54/236.93eHz),
sigma41.921/42.965us including startup transient; gap sigma49.275/49.184us.
Same-sector ranges4099..5101 /4077..5039us remain within guard envelope.
Coast disable-extrapolated frequency236.13/237.63eHz agrees; raw2kHz coast's
simple quantized199/248Hz edge estimates are not the fitted operating frequency.
ADC feedback50047/49810 scans,peak_abs_raw352/351 (<1200),bus_min10853/10913mV
(>8400). COMPmax65us,COMmax52us,write20us,guardISR16us,IRQpeak22/21 per ms.
Timed100us guard survived~152 hardware wraps each without clock regression.
First/last32 records retained, omitted middle14127/14150 explicitly reported;
whole-stream moments cover that middle but cannot reconstruct a dropout plot.

Third10s attempt acquired valid step3 ci1701 seed, age288ticks,remainingARR137,
arm4us. Two accepted records819/1850us (1031us gap) and two COMs; guard stopped
at1800us, before late record bookkeeping. Feedback8 scans,peak288,bus11617mV,
no driver/desync flag. This is an early engagement failure, NOT a late sustained
dropout. Batch stopped for inspection, not retried away. Compare with E124's
early missing-event failure; cause remains to diagnose, not declare variance.

All six attempts' window CRC/count/order and final off readbacks verified.
Last state03 gates/ENABLE/MOE/CCRs0,nFAULT1,override cleared,COM41 closed.
Board/source now sustained-window firmware. Major result: powered operation
has progressed from20ms to two complete10s runs at a measured steady point.
NOT completed repeatability across range, independent qzc/harmonic exclusion,
time-resolved dropout map, average-current qualification or recovery parity.
The1ms safety stop still preempts22.5ms reference recovery; do not claim recovery
was exercised merely because the run lasted longer than22.5ms.

## Entry 130 — 2026-09-13 — Handoff timing evidence; tracing cost isolated

E129 failed10s_03 second accepted reference interval=2031half-us ticks
(1015.5us), versus recorded event gap1031us including bookkeeping. E124's
restore55_05 second interval=2075ticks. Both acquired step3 and failed on the
second accepted sector5, but step3 also succeeded in E129's1s run; correlation
is not a deterministic root cause. Independent reference interval exceeding1ms
argues against a sampled-clock underflow-only false trip. Keep guards unchanged.

Found an instrumentation plumbing gap: per-read IRQ tracing was enabled and
consuming time, but powered-run dumps never called trace_dump. Connected it
only when fixture capture is armed, after outputs are disabled. Firmware default
TRACE_ENABLED remains true for this comparison; fixture `--core-trace 0|1`
now explicitly handshakes either mode and disables tracing on cleanup.
Source/board differ only by this post-stop dump connection from E129; no motor
algorithm, duty, guard or acquisition change. Normal release builds.

Flashed after off checks; same known UART clock recovery after reading
APBENR1=08000000,PD1ODR0,BDTR0c1a,CCRs0, then write08040000. Guardcheck passed
18/18 writer refusals: reasons4/8/1 at1005/1006/305us,ISR10/10/8us,backstop0.

Current map,800mA,6.5% catch/run,4.5% handoff,50->200eHz stepped startup:
- `handoff_trace1_01`:1s completes;32 CRC-valid prefix IRQ records now retained.
  First two accepted handlers read12 levels each,56/58us captured cost. Several
  earlier handlers reject after1 read; two reject after5/2 reads following a
  within-handler level change. Full COMPmax72us,COM51us. Thus per-read tracing
  sits directly in the persistence loop and changes its time aperture, not
  merely post-event logging cost. This successful trace is NOT the missing
  trace of E129's failure; do not retroactively reconstruct that failure.
- `handoff_trace0_01`:same firmware,1s completes without per-read tracing.
  COMPmax52us,COM51us,1420appliedCOMs,5411feedback scans,peak342,bus11140mV,
  IRQpeak26/ms. Acquired step1 versus prior trace1step5: not sector-matched.
- `sustain_notrace45_10s_01`:10s completes in the lighter mode,14279COMs,
  14278accepted events,53819feedback scans,peak346,bus10996mV;COMP52us,COM52us,
  guardISR16us,write20us,IRQpeak25/ms. Coast fitted disable rate237.83eHz.
  Zero order errors, no tracking/driver/desync/polling stop;planned deadline2.

Measured instrumentation cost is20us at maximum COMP service in these runs.
IRQ counts also increase with shorter handlers; do not infer total CPU saving
from max handler duration alone. Disabling tracing passes sustained operation,
but this small, sector-unmatched comparison does NOT prove the intermittent
handoff failure fixed. Next targeted comparison should control measured seed
sector (without inventing a seed or relaxing acquisition) and distinguish
filter aperture from acquisition/phase timing.

Raw/CSV/coast/plots retained for all three; accepted-window CRC/accounting and
post-run off verified. IRQ decoder now checks declared count when present;
hardware trace regression confirms32 sequential CRC records and persistence
outcomes.63 Python tests pass. Board is trace-dump build; runtime trace0,
handoff override cleared,engagems1000,COM41 closed,alloutputs verified off.

## Entry 131 — 2026-09-13 — Selected-sector comparison separates acquisition failures

Added diagnostic `flyseed0..6` and fixture `--seed-sector`: wait for selected
FIRST real edge, then unchanged12 measured intervals;0 retains original any
sector behavior. Acquisition deadline remains20ms from initial scan, not reset
when selected edge arrives. Completed seeds remain one-shot/unrefreshable.
FLYSELECT records desired sector and skipped initial edges; refusal clears stale
selection report. Unit test covers all6 sectors,12 fresh intervals,original
expiry and invalid selection.74 Rust tests,63 Python tests and release build pass.

Flashed after off checks. Known UART recovery reads APBENR1=08000000,PD1ODR0,
BDTR0c1a,CCRs0 before write08040000. Guardcheck18/18 writer refusals passed:
reasons4/8/1 at1005/1006/305us,ISR10/10/9us,backstop0,disabled1.

Fixed four-pair alternating cohort `seed3_trace1_01..04` and
`seed3_trace0_01..04`: current DRV map,800mA,6.5% catch/run,4.5% handoff,
50->200 stepped startup,1s requested powered,selected real seed3. Same firmware,
no resets. Each attempt retained; batch paused after first acquisition refusal,
inspected and then resumed remaining planned cases without redoing it.

| Pair | Trace1 | Trace0 |
|---|---|---|
|01|1s complete; seed1733ticks,14612us acquisition|1s complete;1703ticks,11789us|
|02|TooSlow acquisition at5526us,1interval; no power|TooSlow at12845us,9intervals; no power|
|03|1s complete;1723ticks,12510us|TooSlow at5045us,1interval; no power|
|04|1s complete;1703ticks,11128us|1s complete;1713ticks,14013us|

All FIVE valid acquisitions produced full powered1s completion, across both
trace modes; THREE failures precede reference/trace execution. All successful
seeds are sector3 with12intervals. Trace1 COMPmax65..66us,trace0 52us; COM51us.
Thus this set does NOT reproduce the intermittent powered failure or establish
tracing as its cause. Acquisition durations/speeds differ because sector
selection adds coast time; not an exactly matched operating-state experiment.
Both E129 and E124 powered failures remain unexplained, not declared variance.
All8 raw/CSV/coast/plot artifacts retained; windows/IRQ CRC and post-off readbacks
verified. Selector restored0; do not adopt selected-sector exclusion as a fix.

Two further attempts sought a lower steady operating point, but neither reached
powered4.0% handoff and neither qualifies that operating point:
- `sustain40_200_10s_01`: usual stepped profile,trace0,seed0,requested10s.
  Open-loop current guard CAPreason4 at2005953us,command75.03eHz during100->50
  down-ramp. Last sample logicalB raw3267 exceeds3248 threshold (rawabs1219),
  scaled pulse~14.113A; bus~11.641V,nFAULT high. No acquisition/powered segment.
  This pulse is NOT PSU average. Off verified; did not retry this fault away.
- Inspected existing ramp source and tried the already-supported gradual
  `run200`100->200 ramp, same6.5% duty/guards, removing the down-ramp as a
  variable: `sustain40_direct200_10s_01`. No electrical guard trip, but flying
  acquisition WrongOrder at1381us/zero intervals, no powered segment. Last
  open-loop phase-C ADC span only~10.5mV and later coast lacks valid3phase
  rotation (only2B edges). Therefore no qualified spin/handoff, not a better
  startup and not a4.0% powered failure. Startup mechanism remains to diagnose.

No startup-profile change adopted. Existing stepped profile remains the measured
baseline, with its observed current/acquisition/early-handoff failures recorded.
This distinguishes unreliable entry from successful sustained operation; does
not diminish or generalize the prior complete10s runs. Duty/speed/current/age
guards unchanged. Last board state direct200_01 off verified: six gates,
ENABLE,MOE,CCRs0,nFAULT1,trace0,flyseed0,engagems1000,override0,COM41 closed.

## Entry 132 — 2026-09-13 — Full-startup retention; initial capture remains unqualified

Prior direct200 failure retained only final256ms, so it could not locate loss
within startup. Added `capstride1..100` and host `--capture-stride`,default1 and
restored1 on fixture exit. ADC/guard cadence remains1kHz: sample first, check
rail/phase peak/bus/flags each tick, then independently retain everyNth sample
or ANY ADC-triggered fault. Current-channel rotation still follows original
control index, not retained-record index. No intermediate writes corrupt the
ring head. Asynchronous pre-scan faults may have no new ADC sample.

Stride19 fits4.7s in256existing records without added buffer RAM. CAPCADENCE
reports exact adc_hz1000/divisor19; legacy sample_hz52 is only integer-rounded
metadata. Original tick timestamps remain exact and drive host plots/scaling.
Pure retention test covers all4700ticks/default1 and faults between stride ticks;
hardware replay tests verify exact19ms spacing and original time_ms.75 Rust
tests and64 Python tests pass. Default build initially overflowed384bytes;
old `comtiming` moved to existing bench-adc-probes feature, restoring release
fit. No controller or safety guard removed. ADC scan timing rechecked below.

Flashed after off readbacks. Same known UART clock repair after reading
APBENR1=08000000,PD1ODR0,BDTR0c1a,CCRs0,then write08040000. Guardcheck18/18
post-stop refusals passed, reasons4/8/1 at1005/1006/305us,ISR9/10/8us,backstop0.

Three attempts on current DRV map,800mA,6.5% catch/run,trace0,stride19:
- `startup_direct200_sparse_01`:100->200 ramp,4.5% handoff armed for1s but
  never reached.246CRC-valid samples from19..4674ms span catch/ramp/hold.
  Phase-C max~10.53mV over ALL stages, not just final hold; neutralmax6.5mV,
  phase pulses<=5.153A,bus11.459..11.935V,no fault samples. Wavegap102us,
  ISR28us,controlgap1034us. Acquisition WrongOrder1021us/zero intervals;
  later coast only4B edges,no valid3phase rotation. This does NOT show a
  healthy100eHz catch followed by loss solely during the upward ramp. Sparse
  ADC readings remain vulnerable to sampling effects; not alone a rotor test.
- `startup_catch100_coast_01`:original run50 profile,host abort at0.9s, before
  down-ramp.48samples19..912ms,host-stop reason3. Running phaseC sometimes
  reaches916mV,unlike prior attempt; that alone is not qualified rotation.
  Disabled coast A/B/C edges8/8/3,poor phase-order agreement(.333),no usable
  common-rate fit; coast_motion_valid false.100eHz catch NOT independently
  qualified in this attempt. No current/bus/driver stop; sparse peak10.880A.
- `startup_downramp_coast_01`:same profile,host abort2.9s near52.35eHz, before
  live speed ladder.153samples19..2907ms;host-stop reason3,elapsed2925103us.
  Disabled coast clean A->B->C order(score1),fit52.37eHz,R2.99901,
  coast_motion_valid true. No electrical guard stop; sparse peak11.003A,
  bus11.448..11.862V. Successful rotor capture by end of down-ramp.

These are separate starts, NOT a counterfactual timeline of one rotor. Evidence
supports initial rotor capture as a separate reliability issue and explains
why deleting the down-ramp cannot simply be assumed harmless. It does NOT
prove motor can never start at100eHz, a wiring fault, or a unique root cause.
Static alignment is currently1%/20ms followed by100eHz6.5% catch; any change
to that entry trajectory needs a measured comparison, not relaxed guards.

All raw/CSV/coast/plots retained and off verified. Source/board now sparse-log
build;runtime trace0,seed0,stride1,engagems1000,override0;last downramp_coast_01
gates/ENABLE/MOE/CCRs0,nFAULT1,COM41closed. Known ten-second powered operation
is unchanged evidence; overall startup/acquisition/recovery/parity still open.

## Entry 133 — 2026-09-13 — Repeated10-second operation near206eHz;5/8 entries

No firmware changes/flash this entry. Used sparse-log build/current DRV map,
PSU800mA,6.5% catch/run and original50->200 live-step profile,trace0,seed0,
stride19. Selected ONLY powered handoff duty4.0%,requested10seconds.
Fixed eight-attempt cohort `sustain40_stepped_sparse_01..08`, no resets between
attempts. All attempts retained, including acquisition refusals.

| Attempt | Acquisition | Powered result |
|---|---|---|
|01|seed1,1703ticks|10s complete,12337COMs,205.610eHz mean|
|02|TooSlow after11intervals at10762us|not started|
|03|seed6,1693ticks|10s complete,12355COMs,205.918eHz|
|04|TooSlow after9intervals at9378us|not started|
|05|TooSlow after11intervals at11181us|not started|
|06|seed1,1703ticks|10s complete,12355COMs,205.907eHz|
|07|seed3,1703ticks|10s complete,12375COMs,206.245eHz|
|08|seed1,1703ticks|10s complete,12397COMs,206.618eHz|

Result:5/8 overall complete entries;5/5 powered starts completed ten seconds.
No powered dropout,desync,polling,current/bus/driver stop in those five. This is
a repeated sustained operating point near target200eHz, NOT robust8/8 entry or
completed parity. Acquired speeds/seed sectors reported, not inferred nominal.
Three refusals occur before powered drive; do not label them sustained dropouts.

Whole-stream same-sector mean4839.859..4863.583us, sigma33.333..34.823us including
startup. Coast fits support rotor rate:01~206.35,06~205.46,07~205.56,08~205.88eHz;
all successful coasts have correct3phase order. Event-derived rate remains a
controller measurement, not alone harmonic exclusion. Powered phase peak_raw
316..327 (<1200),bus_min10817..10960mV (>8400),COMwrite18us,COMPmax52us,
COMmax50us,guardISR<=16us. No average-current qualification inferred from peaks.

01/06 end via foreground with POWERPATH0 and elapsed>=10s.03/07/08 end via
independent segment deadline2 at10000005us.03/08 report COASTREF7 because the
foreground first sees guard ownership released; this is normal planned stop,
not a failure merely because COASTREF isn't1. Specific guard reason/time and
recorded observation end determine completion.08 has one tail EV_ACC recorded
after observed stop; retained explicitly and not credited as an additional
powered commutation. No accepted/COM ratio promoted to qzc.

Added offline `drv_sustained_report.py`; output
`captures/sustain40_stepped_sparse_cohort.csv` includes all8attempts. It verifies
raw ADC-frame CRC,accepted-window CRC/count/sequence,whole-stream count agreement,
and final off after COAST END. New regression caught and fixed a report bug:
using the last OUT anywhere could mistake preflight safing for post-run safing
when a log was truncated. Tests also distinguish planned timer/foreground stops,
early tracking fault,acquisition refusal,startup stop and premature deadline.
68 Python tests pass. This reporter does not certify physical lock/parity.

All8 raw/CSV/coast/plots retained,final off verified. Last08 six gates,ENABLE,
MOE,CCRs0,nFAULT1;trace0,seed0,stride1,engagems1000,override0,COM41closed.
Next reliability work must address passive qualification's TooSlow refusals
with actual edge timing evidence; widening a threshold without that evidence
would not explain them. Range/recovery/average-current and reference parity
remain incomplete despite the now-repeated sustained200eHz-class operation.

## Entry 134 — 2026-09-13 — Acquisition timeout precedes candidate confirmation

Added fixture-only fault snapshot F85 (13 little-endian u16 plus CRC32,
Ascii85): last sector/edge, decision tick/phase, each filter's packed stable
and candidate level, candidate onset and last sample. All timestamps half-us.
No regular per-sample snapshot writes. Zero last_step means no usable prior
edge snapshot (including the outer acquisition deadline); do not decode zeros
as measured candidates. Initial capture build leaves acceptance unchanged.

Six-attempt diagnostic batch acq_snapshot_01..06 uses current DRV map,800mA,
6.5% original run50 catch/down-ramp then50->200 ladder,4% powered duty,1s
powered window,trace0,seed0,stride19. All outcomes retained in
captures/acq_snapshot_cohort.csv, all final gates/ENABLE/MOE/CCRs off,nFAULT1.
01: startup phase-current stop at1.505s, Araw3324 (>3248), pulse14.75A;
not acquisition or powered-handoff failure.02..05: four complete1s powered
windows near205eHz.06: passive TooSlow after11intervals,11250us, no power.

06 F85 decoded with CRC verified:
`(6,20468,22488,0,9,0,22488,8,0,22398,4,22428,22428)`.
Last sector6 at20468ticks; expected C rising next. C stable0,candidate1,
onset22428: gap1960ticks=980us, INSIDE physical maximum1000us.
A visit polls at22488: wall gap2020ticks=1010us, while C is still awaiting
confirmation (candidate age30us). Previous code immediately latches TooSlow.
This proves premature rejection of an in-range pending candidate, NOT that
the candidate would necessarily persist on its next sample. No subsequent
C sample exists in this failed capture. Earlier three E133 refusals lacked
this snapshot and cannot all be retrospectively assigned the same cause.

Implemented Acquire::poll_filtered: only the expected phase/level's existing
candidate, with physical onset gap1333..2000ticks, can defer wall timeout.
Candidate age<=240ticks (40dwell+200maximum sample gap), latest sample age<=200,
filter healthy, no existing fault/ready seed, original20ms overall deadline.
Actual edge acceptance still uses strict original onset bounds. No blind
timeout grace, no powered-watchdog change, no controller/duty/guard relaxation.
FLYWAIT counts actual deferred polls; candidate cancellation/late onset/missing
visit/wrong phase cannot prolong acquisition. Tests include captured timings,
no-input/late/cancelled/stale/wrong-phase cases, absolute timeout and u32wrap.
79 Rust tests,68 Python tests pass; M0 check and default release build pass.

Initial fix (before FLYWAIT counter/normal-age fast path) hardware:
acq_pending_fix_01,10s complete,seed4/1699ticks,max same-phase gap67us,
12352COMs,205.873eHz whole-stream rate,cycle sigma33.492us,coast205.18eHz.
Phase peak_raw328,bus_min10913mV;COMP/COM guard path normal, segment timeout
at10000004us, final off verified. Captures/acq_pending_fix_initial.csv.
One success demonstrates operation, not a statistically qualified fix.
Counter build subsequently flashed and guardcheck18/18 post-stop refusals
passed; its powered outcome is recorded below. Both fix flashes booted with
USART3 clock already enabled; only initial snapshot build needed known
APBENR1 repair after exact disabled register checks.

Final counter build: acq_pending_count_01 completes10s,seed4/1701ticks,
FLYWAIT polls0 (this run did NOT exercise the grace branch), phase gap63us.
12372COMs/12371accepts,206.201eHz,cycle sigma34.608us,coast205.09eHz.
52927 feedback scans,peak_raw321,bus_min10937mV,guardISR16us,commit18us.
POWERPATH2 at10000005us/COASTREF7 is the planned independent segment stop.
Report captures/acq_pending_count_initial.csv verifies post-coast off.
Source/board now match counter build; trace0,seed0,stride1,engagems1000,
override0,all six gates/ENABLE/MOE/CCRs0,nFAULT1,COM41closed.
shell-sine SHA256 remains B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.
Next: repeat cohort on this exact build, retain FLYWAIT counts and any F85
refusals. Need actual grace-then-confirm hardware evidence and overall entry
reliability; no claim that every historical TooSlow was this race or that
range/recovery/average-current/reference parity is complete.

## Entry 135 — 2026-09-13 — Four repeat entries pass; pending edge confirms

No firmware edits/flash. Current E134 counter build, DRV map,800mA PSU,
6.5% original catch/down-ramp and50->200 ladder,4% powered duty,trace0,
seed0,stride19. Fixed four-attempt batch acq_repeat_01..04 requests10s each.
All four acquired twelve intervals and completed the powered window.
Seed sectors3/6/5/1,intervals1701/1701/1691/1701ticks.
FLYWAIT polls0/0/2/0. Attempt03 therefore demonstrates the new candidate-only
wait followed by successful qualification and sustained powered operation on
hardware. Two polls do not necessarily mean two different candidate edges.
No cancellation in03; maximum same-phase gap66us remains below100us.

Whole-stream rates205.830..206.286eHz,cycle sigma33.443..34.182us including
entry transient. COMs12350/12354/12364/12377; no powered dropout/desync,
electrical stop or acquisition refusal in this batch. Coast fits support
rotor rates205.86/205.19/206.81/207.19eHz with correct three-phase order.
Powered peak_abs_raw312..325,bus_min10829..10865mV,52899..53261ADC scans,
guardISR15..16us,commit18us,IRQpeak27..29/ms against64limit.
Every run ended at independent segment deadline10000005us and final off
was verified. All raw/ADC/coast/plot artifacts retained; CRC/count/order and
post-coast safe readbacks checked by captures/acq_repeat_cohort.csv.

Host reporter now includes acquisition_deferred_polls; missing old metric
remains blank, not zero. Regression includes recorded03 with two waits and
completed powered outcome.69 Python tests pass. Four successes are not a
population reliability guarantee and do not prove all prior refusals fixed.
A separate60s hold at identical settings follows to extend retention evidence.

## Entry 136 — 2026-09-13 — First complete60-second BEMF-controlled hold

Same unchanged counter firmware/settings as E135, only engagems60000.
sustain40_60s_01 acquired seed3/1701ticks, no deferred polls, max phase gap63us.
Completed full60s:74368 appliedCOMs,74367accepted events,order_bad0,
no desync/polling/electrical/tracking stop. Independent segment deadline2
at60000005us; observation ends60000044us. Whole-stream rate206.577eHz,
cycle mean4840.818us,sigma32.141us,min4721,max5183; gap655..969us.
No excluded moments. Coast extrapolation207.00eHz,R2.99994,correct3phase
order supports actual rotor speed rather than merely controller rate.

319175 powered ADC scans,peak_abs_raw337 (<1200),bus_min10817mV (>8400).
GuardISR16us,commit18us,IRQpeak29/ms (<64). All final gates,ENABLE,MOE,
CCRs0,nFAULT1; fixture restored trace0,seed0,stride1,engagems1000,override0,
COM41closed. captures/sustain40_60s_initial.csv verifies raw CRCs, counts,
retained windows and post-coast off. Prefix/tail omit74303middle events;
whole-stream moments cover them but there is still no full dropout timeline.
Regression checks counts above65535 without truncation. This extends measured
retention to60s, not a qzc/harmonic/recovery/average-current parity certificate.
Next work should address those outstanding metrics/policies, rather than
requiring another acquisition forensics loop absent a new failure.

## Entry 137 — 2026-09-13 — Recovery parity boundary made executable

No flash or motor commands. Reverified frozen minz archive/source:128files,
SHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
Added sequence.rs reference_rekick_occurs_after_shadow_power_guard_has_latched_tracking.
Actual minz-core receives120synthetic200eHz events; its timed accepted events
feed the actual pure powered Guard under100us polling with fresh ADC feedback.
After missing input, Tracking latches1001..1100us after last acceptance.
Unguarded reference later proposes one timeout re-kick and returns to polling;
fresh feedback, accepted events and polls cannot clear the already-failed guard.
This is policy replay alongside timed reference, NOT hardware dropout injection
or a claim of integrated peripheral safing/recovery. Existing bench guardcheck
is separate evidence of post-stop commit refusal.

Source check: powered foreground exits when owns() is false; commit() checks
ownership/reason and polls the latch within its write critical section. Thus
minute-long successful drive does not exercise reference timeout recovery.
Updated BEMF_PARITY_PLAN with current requirement/evidence gaps and explicit
stop-only -> disabled acquisition -> new guarded authority sequence. Preserve
absolute campaign time on re-entry; existing startup-only elapsed<5s helper
cannot be reused silently after long holds. No relaxed tracking timeout,
automatic restart, synthetic live fault or changed firmware this entry.
Full host Rust suite80tests passes (58policy +1recorded +21sequence);
known incremental-cache AccessDenied warning does not fail test execution.

## Entry 138 — 2026-09-13 — Scheduled BEMF suppression trips live tracking guard

Added explicit idle dropout1/dropout0 and host --dropout. Fixed2s powered
injection, one-shot consumed by powered/coast entry; fixture requires powered
window>2s, checks acknowledgement, clears arm on exit. Default unarmed.
At deadline foreground enters critical section, suppresses comparator delivery
and clears pending. A later COM cannot re-enable comparator delivery; a queued
COMP ISR is also masked/cleared. Pending one-shot COM is NOT cancelled by the
injection itself. No fake events, direct guard trip, altered TIM6 priority,
ADC/guard bypass or automatic restart. Observer initialization clears applied
state; report gives actual injection time and stop-only semantics.

Default build initially overflowed480bytes. Moved old comirq diagnostic behind
existing bench-adc-probes feature, like comtiming; default motor/guards remain.
Build fits. Flashed after off checks, known USART3 clock recovery only after
APBENR1=08000000,PD1ODR0,BDTR0c1a,CCRs0. Guardcheck18/18 post-stop vetoes pass.

dropout_stop_01: current DRV map,800mA,6.5% original startup/50->200 ladder,
4% powered duty,10s planned window,trace0,seed0,stride19,--dropout.
Applied observer time2000066us; last accepted1999963us,2466events,order_bad0.
POWERPATH Tracking8 at guard-clock2001006us, disabled observer end2001045us.
Same-clock conservative injection-to-disabled observation979us and last-event
to-disabled1082us. Guard clock and observer origins differ slightly; don't
subtract them to claim exact electrical gate latency. No event accepted after
suppression, no restart. GuardISR17us,commit18us,10631powered ADC scans,
peak_raw306,bus_min11104mV. Pre-injection whole-event rate205.524eHz;
post-disable coast205.22eHz supports still-spinning rotor during sense loss.
Final gates/ENABLE/MOE/CCRs0,nFAULT1 verified, raw/ADC/coast/plot retained.

Reporter retains powered_stopped outcome (NOT a completed10s hold), adds
dropout_stop_verified and same-clock latency bound. Negative tests reject
wrong guard reason, automatic-restart claim, and excessive shutdown latency.
71 Python tests pass. captures/dropout_stop_initial.csv validates CRCs,
event accounting, no retained post-stop events and final off. This is injected
sense-delivery loss, not a motor-load disturbance or recovered operation.
Next stage is disabled re-acquisition with separate evidence before new power.

Follow-up dropout_clear_01 omits injection, uses same settings and3s window:
completed past the2s injection point, with no DROPOUT applied line. This checks
the injection latch/arm do not contaminate a subsequent normal start. Final
off verified by captures/dropout_clear_initial.csv. Firmware/source now match
dropout build, trace0,seed0,stride1,engagems1000,override0,dropout disarmed,
COM41closed. This fresh commanded start is NOT automatic flying recovery.

## Entry 139 — 2026-09-13 — Two disabled re-acquisitions after injected loss

After injected Tracking8 only, observe_end stops COM/COMP execution, gates off
and ENABLE0, then existing acquire_feedback runs two complete measured cycles.
No wake, new guard or gate authority. Discard returned seed deliberately at
this stage. Save/restore initial REPORT/FAILURE/SELECTION/BASELINE and pending
flag so initial acquisition and powered capture remain intact. Recovery has
separate RECOVERYACQ summary and optional CRC RF85 failure snapshot (same13u16
schema as F85). Reset recovery report on next observation initialization.
No acquisition filter/timeout changes. Default release fits;80Rust tests pass.
Flashed after verified off, known exact UART clock repair,guardcheck18/18 pass.

Two attempts dropout_reacq_01/02: DRV map,800mA,6.5% startup with original
50->200 ladder,4% powered,10s timeout,trace0,seed0,stride19,--dropout.
Both Tracking8 stopped with all outputs off after sense suppression at2s.
Observer last-accept -> disabled end1088/1072us, injection -> end978/690us.
Initial seeds2/5 (1701/1690ticks) preserved separately from recovery seeds2/1.
Recovery result1 in both:12fresh intervals,1645/1634ticks (~202.63/204.00eHz),
10402/10366us acquisition time,max same-phase gap63us,disabled1,gate_authority0.
Powered peak_raw306/315,bus_min11044/11128mV. Final gates/ENABLE/MOE/CCRs0,
nFAULT1, no automatic restart. All raw/ADC/coast/plots retained and CRC/count/
safe checks in captures/dropout_reacq_initial.csv. Host distinguishes successful
passive seed from powered recovery; both runs remain powered_stopped, not full
ten-second completions.72Python tests pass, including malformed qualification
and non-disabled-state rejection. Source/board match;COM41closed,dropout0,
trace0,seed0,stride1,engagems1000,override0.

Coast sampling now starts AFTER the additional~10.4ms re-acquisition. Existing
coast_disable_extrapolated_frequency198.67/199.89 labels are NOT true stop-time
speed in these captures; they extrapolate to delayed coast origin. Host now
prints an explicit warning for RECOVERYACQ captures. Do not compare those
numbers to pre-recovery stop speed as though there were no disabled interval.

Next: new guarded re-entry with absolute budget retained. ENABLE wake takes
~1.1ms, longer than seed handoff margin: do NOT reuse a pre-wake seed after
waking. Wake with all gates off BEFORE final qualification, then use fresh
ADC baseline and current seed; retain original fault and first-segment metrics.
Current disabled probe is evidence for feasibility, not permission to bypass
freshness or proof of automatic recovered operation.

## Entry 140 — 2026-09-13 — Absolute re-entry budget policy tested, not live

Added pure powered_guard::SessionBudget with original campaign ceiling and
powered-window end retained from first handoff. initial() preserves startup
elapsed<5s and20ms..600s window rules. reentry() consumes its single attempt,
accepts only Tracking as prior fault, counts shutdown/wake/acquisition/setup
against original deadlines, reserves200us, and requires>=20ms remaining.
Refuses backwards elapsed time, addition overflow, expired/near-expired window,
all other fault classes, and repeated attempts (including after refusal).
Budget is NOT drive authority: caller still needs verified disabled state,
wake, fresh qualification, current baseline, pending-event cleanup and new
guard. Must retain the same object; reconstructing it on retry is invalid.

Test scenario: initial startup4.7s +60s requested. After45s powered plus12ms
recovery overhead,49.712s total elapsed, remaining segment14.9878s; original
campaign ceiling65s and powered end64.7s are preserved (200us reserve).
Actual pure Guard runs that remainder with100us polling/fresh feedback and
800us ordered accepted events, including u32clock wrap, then stops exactly
at segment deadline. Other tests cover electrical/host faults, old elapsed,
exhausted time and repeated re-entry.82Rust tests and M0 library check pass.

No flash, motor command or live restart integration this entry. Board remains
E139 disabled re-acquisition build; new SessionBudget currently has no live
caller. Next integration must retain first-segment fault/accepted/current data
before preparing the second segment, initialize budget once at original
handoff, and use wake-before-final-acquisition rather than refreshing an old
seed. Do not claim resumed motor control from these policy tests.

## Entry 141 — 2026-09-13 — First-segment archive; stack hazard interrupts qualification

Added owned StoppedSegment/Archive: power fault/time/commits/current metrics,
whole event statistics,32prefix and32suffix plus omitted count. Refuses unsafe
capture, invalid accounting and overwrite. Freeze after observe_end and off,
before passive re-acquisition; prepare_early clears it only for a new campaign.
Future recovery must not call that archive-clearing entry helper. Fixture-only
FIRSTSEG/P185/T185 frames retain separate first-segment identity. Host decoder
validates CRC/accounting/order; tests use translated known60s capture frames,
not a fabricated claim of successful hardware archive.84Rust/74Python tests
pass; default release fits after old compirq/compirqref diagnostics move to
bench-adc-probes (initial overflow608bytes). No drive protections removed.

Two attempts on initial archive build, current DRV800mA/proven6.5% startup,
4% powered with10s timeout and injected2s dropout:
recovery_archive_01 stopped at850us Tracking, BEFORE injection. It contains
impossible FLY values (step1073825828,interval536905288), COASTSTATE step34104,
and CRC-valid accepted record(897us,step176,interval1941). These are firmware
memory corruption evidence, NOT ordinary engagement variance. Initial terse
interpretation as handoff failure was incomplete. Reporter correctly rejects
invalid sector; no recovery_archive_initial.csv was produced. Raw artifacts
must remain, not be repaired into passing measurements.
02 stopped on startup phase current (~14.04A pulse), also before injection.
Both final off verified. No archive exercised; no recovery qualified.

Stopped powered testing when corrupt data was decoded. ELF RAM: .data204,
.bss34076 with alignment, __ebss/_stack_end0x200085ec; stack top0x20009000,
only2580bytes stack region. Disassembly showed coast_run_inner reserved796
bytes (+20register push) because the late archive temporary lived in the
powered function frame. This is a concrete stack-pressure regression and
leading corruption cause; no painted-stack measurement yet proves collision.
Moved bulk capture to #[inline(never)] freeze_first_segment, called only after
COM/COMP/guard writers stop. Powered frame now140bytes (+20push),656bytes less.
Static RAM unchanged. Final release flashed after off; exact known USART3
clock recovery,guardcheck18/18 passed, final all gates/ENABLE/MOE/CCRs0,nFAULT1.
COM41closed, defaults after reset; no powered test of corrected build yet.

NEXT BEFORE MOTOR: measure/protect stack margin (including nested IRQs and
disabled archive copy), then requalify this build. Do not grant recovery power
or explain corrupt captures as noise. SessionBudget still not connected to
live re-entry. Board/source match non-inlined archive build; first-segment
hardware preservation remains unverified despite host tests.

## Entry 142 — 2026-09-13 — Stack margin measured; first archive verified on hardware

Recovered static RAM without shrinking motor/ADC/event/coast buffers:
uartdma stress command/buffer is now opt-in bench-transport-probes; RTT console
buffer1024->256bytes (UART fixture transport unchanged). Default ELF stack
end0x20007ef4,top0x20009000,span4364bytes vs2580 previously. Non-inlined
post-stop archive helper retained; powered coast frame140bytes.

Added stack_probe: at boot, with interrupts masked, paints only unused memory
from linker stack bottom to currentMSP-64, word aligned. It never paints live
frames or repaints during runs. Idle shell `stack` and post-run STACK report
span/painted/untouched. Diagnostic watermark, NOT MPU/overflow protection or
guarantee against every untested nesting. Use stack command only while idle.
Flashed after off; USART3 clock enabled normally. Boot/guardcheck reports:
span4364,painted2700,untouched2584; guardcheck18/18 post-stop vetoes pass.

One guarded requalification stack_archive_01, current DRV800mA,6.5% original
startup/50->200 ladder,4% powered,10s timeout,trace0,seed0,stride19,--dropout.
Pass gate: valid data, tracking stop, archive equality, final off,>=512bytes
untouched. Observed1888bytes untouched after run AND after all dump formatting;
conservative observed stack-use span4364-1888=2476bytes. No corrupt fields.
FIRSTSEG fault8,stop guard-time2000506us,2463COMs,10606ADC scans,peak_raw306,
bus_min11151mV,observer end2000544us.2462accepted events;32prefix+32tail with
2398omitted. CRC/order/accounting-valid P185/T185 windows exactly equal original
A85/T85 windows. New regression exercises this REAL hardware archive; synthetic
format tests remain labelled separately.75Python tests pass.

Injected at2000123us,last accepted1999478us; disabled observer end2000544us,
421us after injection/1066us after last event. Recovery passive seed2,
1653ticks (~201.65eHz),12intervals in10329us,max phase gap62us,disabled1,
gate_authority0. No automatic powered re-entry. Raw/ADC/coast/plot retained,
captures/stack_archive_initial.csv validates final off and stop/acquisition.
Final six gates/ENABLE/MOE/CCRs0,nFAULT1,COM41closed;trace0,seed0,stride1,
engagems1000,override0,dropout0. Board/source match stack-watermark build.

This supports stack pressure as the prior corruption mechanism, not a unique
proof or exhaustive stack qualification. Future second-segment wake/prepare/
acquire/ISR nesting still needs measured margin. Retain watermark for recovery
integration. SessionBudget remains unconnected to live restart; next work can
now preserve verified first-segment data while adding that second segment.

## Entry 143 — 2026-09-13 — First guarded powered re-entry completes remaining window

Explicit idle reentry1/0 and host --reentry (requires --dropout), default off,
one-shot. First-segment SessionBudget initialized before initial handoff;
after injected Tracking8 only, archive must exist and bridge be disabled.
Re-entry consumes one attempt. No recursion/repeated retries: stop/mask old
controller, retain FIRSTSEG, prepare fresh observer WITHOUT clearing archive,
wake with all gates off, acquire_awake_feedback for12new intervals, validate
remaining original budget, then start_reentry using fresh baseline/seed.
start_reentry requires actual prior reason8 and awakened ready driver; all
normal phase-current/bus/fault/timing/accepted-age guards remain. First report
and recovery report separated; new awake baseline retained for guard validation.
Driver ENABLE is high during final acquisition, hence RECOVERYACQ disabled0,
but no gate authority during acquisition. This is not the earlier ENABLE0 probe.

REENTRY result codes:1ineligible/no archive,2host abort,3missing initial budget,
4wake refused,5acquisition refused,6budget refused,7second-segment path entered.
7 alone is NOT successful recovered operation; guard/events/deadline determine
that. Header includes original powered end, resume elapsed, remaining window
and final elapsed in original run clock. First injection time retained even
though observer resets for second segment. No new deadline minted at retry.
84Rust tests/76Python tests pass. Default build fits,stack span4340bytes,
powered-frame156bytes,resume wrapper92bytes (excluding20byte register pushes).
Flashed after safe readbacks; boot USART3 clock normal;guardcheck18/18 pass.

reentry_power_01: current DRV800mA,6.5% original50->200 startup ladder,4%
powered,10s original window,trace0,seed0,stride19,--dropout --reentry.
First segment: injection2000016us,Tracking8 stop2000406 guard-clock,
disabled observer end2000449us;2466COMs/2465accepts,10707ADC scans,
peak_raw318,bus_min11008mV. Archive32prefix+32tail,2401omitted,CRC-valid.
New awake acquisition:seed4,1660ticks,12intervals in10837us,max phase gap63us.
Second segment:9868COMs/9867accepts over7987141us remaining window,
205.906eHz,cycle sigma32.295us,peak_raw312,bus_min11032mV,ordered events,
no desync/polling/early guard. Normal foreground completion,reason0,
observer end7987217us. Final coast205.09eHz with correct3phase order.
This coast follows the SECOND powered stop, so the extra disabled-acquisition
origin warning is not applicable; host now distinguishes reentry result7.

Original powered end14712860us since run start; resume at6725519us,
remaining7987141us plus200us reserve exactly reaches original end.
Final elapsed14712766us:94us BEFORE original deadline, not a new10s run.
STACK untouched1832bytes (pass>=512) with valid controller/capture data.
Final gates/ENABLE/MOE/CCRs0,nFAULT1,COM41closed,trace0,seed0,stride1,
engagems1000,override0,dropout0,reentry0. Source/board match reentry build.

captures/reentry_power_initial.csv validates separate FIRSTSEG CRC/order,
normal second-window completion, new qualification, original deadline
accounting, exactly-one-attempt declaration and final off. Negative tests
reject changed deadline, overrun, non-tracking first fault and retry-count
claim. Primary accepted stream is second segment; never join its timestamps
to first-segment relative timestamps. This is ONE successful injected-sense-loss
recovery, not repeated recovery qualification, arbitrary load-disturbance
recovery, average-current qualification or completed minz parity.

## Entry 144 — 2026-09-13 — Fixed3/3 repeat powered recoveries near206eHz

No firmware edits/flash/reset. Current E143 build, DRV800mA,6.5% original
startup/50->200 ladder,4% powered duty,10s original window,trace0,seed0,
stride19,--dropout --reentry. Fixed batch reentry_repeat_01..03, all retained.
All three reach injected loss, stop Tracking8, archive first segment, wake,
qualify fresh12interval seed, and complete remaining~7.987s powered window.
No startup/acquisition refusal or early second-segment fault in this batch.

Recovery sectors2/1/1,interval1660/1650/1660ticks,acquisition10806/10838/10837us,
max same-phase gap63us. Initial acquisition02 also deferred two candidate polls
then qualified. Second-segment rates206.174/206.076/206.155eHz,cycle sigma
31.982/32.384/32.616us. COMs9880/9876/9879;accepted9880/9875/9878.
End via independent segment timeout on01/02 and normal foreground on03.
01 has one late writer veto at the planned deadline, not a recovered dropout.
All complete64/83/88us BEFORE their respective ORIGINAL powered deadlines.
No resetting the10s allowance at re-entry. Coast fits206.08/206.56/206.60eHz,
correct3phase order support actual post-recovery rotor speed.

First segment peak_raw323/308/337,bus_min11068/11116/11223mV;
second peak_raw318/356/314,bus_min11092/10937/10865mV,42402/42660/42474ADCscans.
No phase/bus/driver limit exceeded. No average-current inference from peaks.
All stack reports span4340,painted2660,untouched1832. Reporter now requires
valid stack metadata/span>=4096/untouched>=512 as an additional recovery gate;
tests reject absent/low margin.76Python tests pass. CRC/order/accounting of
FIRSTSEG and second segment, original deadline, qualification and final off
verified by captures/reentry_repeat_cohort.csv (reentry_verified1 for all3).
All raw/CSV/coast/plots retained;final six gates/ENABLE/MOE/CCRs0,nFAULT1.
COM41closed,trace0,seed0,stride1,engagems1000,override0,dropout0,reentry0.

Together with E143: four observed successes, including a predeclared3attempt
repeat batch. This qualifies the tested injection/wake/reacquisition/re-entry
path at this point, NOT an arbitrary load disturbance, repeated retries within
one run, recovery across the whole range, mean-current performance or minz
matched-speed parity. Next range work should use the same two-segment metric
at the already-characterized~237eHz point; mean-current/reference benchmarks
and time-resolved quality remain explicit gaps.

## Entry 145 — 2026-09-13 — ~237eHz recovery limited by individual acquisition interval

At previously characterized4.5% powered duty, unchanged6.5% original startup,
50->200 ladder,800mA,10s original window,trace0,seed0,stride19,--dropout --reentry:
reentry45_01 sustains first segment until injected2s loss (2848COMs), then safe
Tracking8. Recovery acquisition refuses TooFast after2valid intervals,2493us;
no new power. First archive valid,stack margin1832,final off verified.
RF85=(1,3564,4972,0,8,0,4972,8,0,4880,9,0,4910). Expected next A falling,
but qualified candidate onset is absent: confirmation already cleared it.
Do NOT mistake decision-wall gap1408ticks for rejected onset gap.

Added failure-only FLYEDGE/RECOVERYEDGE qualified_tick (bit-marked internal
optional onset), preserving existing13word F85/RF85 wire schema. No acquisition
limits, drive path or guards changed. First acquisition onset retained separately
from recovery onset with original report save/restore. Release fits,flashed after
off,exact known USART3 clock repair,guardcheck18/18 pass. Boot stack4332bytes.

reentry45_edge_01 on diagnostic build: first segment2844COMs/2843accepts in
2000852us observer duration (~236.90eHz COM count; bootstrap included),peak_raw362,
bus_min11128mV. Injected stop Tracking8; awake recovery refuses TooFast after
7valid intervals,6101us,max sample gap63us. RF85 CRC verified:
(1,10782,12188,0,8,0,12188,8,0,12096,9,0,12126).
RECOVERYEDGE onset12066: C-rise last edge10782 -> A-fall12066 gives1284ticks
=642us, below1333ticks/666.5us individual acquisition minimum. Decision arrives
61us after candidate onset. This is a measured interval rejection, not another
confirmation-timeout race. No recovery power attempted. Stack untouched1912,
all final gates/ENABLE/MOE/CCRs0,nFAULT1,COM41closed,fixture settings cleared.
Raw/ADC/coast/plots and reports reentry45_initial.csv/reentry45_edge_initial.csv
retain both refusals.76Python tests pass. Firmware/source match onset-diagnostic
build; old4% recovery cohort remains evidence from E143 build, not remeasured here.

Both refusals localize to C-rise -> A-fall; seven preceding valid intervals in
second attempt already span a full cycle. This supports unequal per-sector
timing as a leading explanation but does not itself prove the failed edge is
rotor truth or rule out an artifact. Powered policy already allows shorter
individual gaps with an independent full-cycle speed bound; acquisition still
requires EVERY interval1333..2000ticks. Next: apply/test full-cycle qualification
to fresh acquisition while retaining ordered12intervals, bounded individual
spacing, dwell/sampling continuity and physical cycle range. Do not just lower
a threshold or claim237eHz recovery complete. Robust recovery remains~206eHz.

## Entry 146 — 2026-09-13 — full-cycle acquisition qualification, host verified only

Fresh acquisition now retains twelve ordered intervals but checks every repeated
sector against8000..12000 half-us ticks (4..6ms electrical cycle). Seven rolling
same-sector checks precede a seed. Individual gaps remain bounded666..2000ticks,
matching the powered333..1000us policy. Persistence40ticks, sample gap200ticks,
absolute20ms acquisition cap, bounded candidate-only confirmation allowance,
one-shot seed and handoff freshness are unchanged. Powered guards are unchanged.

New tests accept a synthetic unequal-sector8436tick cycle containing the measured
1284tick short sector, including timestamp wrap, and reject a uniform1320tick
sector train at its first7920tick full cycle. Existing order, spike, missing-edge,
sampling continuity, deadline and stale-seed tests pass. This synthetic train is
NOT a replay of a fully measured recovery cycle or proof of rotor truth.
Cycle faults are distinct result14/15 (fast/slow); fixture-only FLYCYCLE and
RECOVERYCYCLE expose checked count, min/max and rejected cycle. F85/RF85 schema
is unchanged. Report storage expanded from11 to15words; remeasure stack margin
before qualifying the powered recovery path.

86Rust tests pass, release shell-pwm build fits. No flash or motor run performed
for this entry: board remains on E145 firmware, new source is NOT hardware
qualified. Next bench gate: disabled checks/guardcheck and stack span>=4096,
then guarded~237eHz injected recovery with seven valid cycles, original deadline,
independent first/second event archives, stack untouched>=512 and final off.

## Entry 147 — 2026-09-13 — cycle acquisition passes twice; recovery seed expires during setup

E146 release flashed after UART off verification. Post-reset register reads
showed APBENR1=08000000,PD1ODR0,BDTR0c1a,CCRs0; exact known USART3 clock repair
to08040000 restored UART. guardcheck18/18 post-stop writes refused; boot stack
span4296,painted2704,untouched2516. No electrical/drive guards changed.

Two real attempts reentry45_cycle_01/02,4.5% powered duty,6.5% original startup,
50->200 ladder,800mA,10s original window,trace0,seed0,stride19,dropout+reentry.
First segments2838/2839COMs (2837/2838accepted) reached injected2s Tracking stop,
peak_raw354/340,bus_min11163/11175mV; separate first archives retained.
Fresh recovery acquisition now PASSES on both,12intervals,seven full-cycle
checks each, no rejected cycle. Full-cycle min/max8444/8708 and8448/8720ticks;
seed sectors1/3,mean intervals1439/1430ticks,elapsed9109/9040us,max sample gap136ticks.

Neither second segment commutated: CORESEED edge_age_ticks=352 on BOTH runs,
armed0/refusal_stop8. Nominal reference wait is about one-quarter interval
(359/357ticks), leaving only7/5ticks versus the retained64tick arm margin.
This localizes the next limit to post-edge acquisition/report/guard/setup latency,
not individual-sector rejection. REENTRY result7 only means path entered;
POWERCOMMITS0 and ACCEPTQUALITY events0 explicitly disprove recovery success.
Do not shorten the seed safety margin or fabricate a later edge. Next investigate
moving nonessential work before acquisition or after timer arming with bounded
cost and correct archive/guard ordering.

Both runs ended verified six gates/MOE/ENABLE/CCRs0,nFAULT1; fixture settings
cleared,COM41closed. Stack untouched1876 on both. Raw/CSV/coast/plots retained,
reentry45_cycle_pair.csv includes both failures. Coast extrapolated-stop timing
after failed second entry is not a validated true shutdown speed. Sustained
~237eHz first segments and qualified passive cycles are progress, NOT recovered
operation or completed current/reference parity.

## Entry 148 — 2026-09-13 — measured recovery setup latency, no margin relaxation

Added foreground-only five-stage SEEDLAT ages, printed only with fixture capture
after shutdown. These retain the actual seed onset and add no output authority.
Release built/flashed after off verification; post-reset APBENR1 already08040000
(no repair),PD1ODR0,MOEoff/CCRs0. guardcheck18/18,boot stack4276/2684/2496.

reentry45_latency_01 repeats E147 profile and reaches2847 first-segment COMs,
2846accepted,Tracking8 after injected2s loss. peak_raw342,bus_min11128mV.
Fresh acquisition passes12intervals/seven cycles8406..8566ticks,seed5ci1423,
9111us acquisition,max phase gap134ticks.
SEEDLAT ages in half-us ticks: entry224, reset252, feedback280, guard330,
reference338; actual handoff check360. Thus the edge is already112us old at
coast_run_inner entry; subsequent setup costs68us: reset14us, feedback14us,
guard25us, reference4us, IRQ setup/check11us (instrumentation included).
Reference wait355ticks is already exceeded at the check; armed0, no second COM.
This is not a guard false trip: it is the unchanged seed freshness refusal.

Next optimization needs savings across the acquisition-return/setup path;
feedback conversion alone (14us) cannot recover the required32us arm margin.
Prefer preparing work before the final fresh edge; preserve actual edge origin,
ordered acquisition, current/bus validation and original deadline. No margin,
dwell, mux-settle or powered guard changes made in this diagnostic build.
Stack untouched1840,final gates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed/settings
cleared. Raw/CSV/coast/plot and reentry45_latency_initial.csv retain refusal.

## Entry 149 — 2026-09-13 — earlier final-edge confirmation enables ~238eHz recovery twice

Instead of relaxing32us handoff margin or moving guard validation after power,
the acquisition scheduler now keeps the settled mux on the final expected
candidate for ONE extra bounded20us visit. Previously its confirmation waited
the entire~60-67us three-phase scan. Only after11valid intervals, an expected
in-range candidate triggers this visit. The same EdgeFilter sample still checks
dwell40ticks, cancellation and sampling gaps; Acquire still checks order,
full-cycle timing and absolute deadline. No fabricated edge timestamp, reduced
mux settling, shortened dwell or powered-guard change. Counter is fixture-only
FLYCYCLECONFIRM/RECOVERYCYCLECONFIRM final_visits. Report grows to16words.
New host test verifies the hint cannot bypass dwell/cancellation/order, and a
later real qualifying edge retains its own onset.87Rust tests,76Python tests
pass; release builds; shell-sine SHA unchanged.

Flashed after verified off. APBENR1 already08040000,PD1ODR0,MOEoff/CCRs0;
guardcheck18/18; boot stack4272,painted2680,untouched2492. Two real attempts
reentry45_confirm_01/02 with unchanged4.5% powered/6.5% original startup,
50->200 ladder,800mA,10s original window,trace0,seed0,stride19,dropout+reentry.
BOTH recover and complete remaining~7.989s. New seeds1/4,ci1427ticks,
seven cycle checks8468..8664 /8468..8660ticks,one final confirmation visit each.
SEEDLAT entry144ticks versus224 previously; handoff age280 versus360:40us saved.
Remaining ARR77ticks=38.5us (above unchanged64tick margin),actual arm4us.

01 second segment11396COMs/11396accepts,237.752eHz,cycle sigma22.471us,
42933ADCscans,peak_raw346,bus_min11008mV. Accepted event count equaling COMs
can include a last acceptance whose pending COM is cancelled at deadline;
it is not an independent qzc denominator.02 also completes with original
deadline and fresh acquisition; exact metrics retained in pair CSV.
Both end on independent segment timeout,final campaign timestamps48/84us
BEFORE ORIGINAL powered end. Recovery verification1 for both in
captures/reentry45_confirm_pair.csv; separate CRC first/second event archives,
raw/ADC/coast/plots retained. Coast extrapolated speeds238.06/237.13eHz support
rotor speed here, not merely a fast accepted-event counter.

Stack untouched1836 for both,all final gates/MOE/ENABLE/CCRs0,nFAULT1;
COM41closed,fixture settings cleared. This is two successful tested injected
Tracking recoveries near238eHz, NOT arbitrary-disturbance robustness or completed
mean-current/reference parity. Need broader repeat/range qualification and
time-resolved quality/current benchmarks;32us handoff margin remains tight.

## Entry 150 — 2026-09-13 — same-build range check reveals post-stop recorder race

Unchanged E149 flashed firmware. Two attempts,4.0 then4.5% powered duty,usual
6.5% startup50->200,800mA,10s original window,trace0,seed0,stride19,dropout+reentry.
reentry_range_40_01 passes:206.215eHz after recovery,9882COMs/9881accepts,
cycle sigma32.480us,peak_raw309,bus_min10972mV,final125us before original end.
Fresh12interval/seven-cycle acquisition passes; seed1659ticks,arm135ticks.

reentry_range_45_01 reaches237.977eHz and original deadline,11407COMs/accepts,
cycle sigma22.928us,peak_raw372,bus_min11044mV,final22us before original end.
Fresh acquisition passes seven8468..8650tick cycles,seed1426,arm77ticks.
BUT last accepted record7988883us is35us AFTER PREFIX_END/quality end7988848us.
Tail CRC validates that record; it is not parser noise. Initial reporter wrongly
promoted this to reentry_verified1. reentry_range_pair.csv is SUPERSEDED by
reentry_range_pair_audited.csv: 206point passes,238point withheld/powered_stopped.
This outcome is an evidence-consistency failure, not proof of post-stop gate
reactivation or rotor desync. Final gates/MOE/ENABLE/CCRs0,nFAULT1 verified both;
stack1836,COM41closed/settings cleared,all raw/CSV/coast/plots retained.

Host verification now requires new-format recovery's seven in-range cycle checks,
zero rejected cycle,valid seed sector,elapsed<20ms,max sample gap<=200ticks.
Legacy no-cycle captures remain replayable. Completed-window certification also
requires no retained events beyond end and last_us<=end_us.78Python tests pass,
including mutation tests for missing/bad cycle evidence and this actual post-stop
capture. Initial E149 captures contain no such post-stop records.

Leading source hypothesis: EV_ACC recorder can resume after priority0 TIM6 safing;
its predicate checks LIVE_IRQ and OBS_STATUS but not ACTIVE or stopped guard.
live_stop freezes PREFIX_END before an interrupted record may resume. Next audit
and fix record/stop serialization without extending frozen end or weakening the
independent gate guard; then requalify this operating point. Do not hide the late
record or call it random variance. Frozen minz archive/source verified unchanged:
128files,SHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.

## Entry 151 — 2026-09-13 — serialize recorder with stop; one powered requalification

live_stop now freezes first end and closes ACTIVE within one critical section.
EV_ACC recording checks ACTIVE (and powered ownership when powered) INSIDE the
same critical section as timestamp/statistics/prefix-or-tail writes. A callback
resumed after guard safing cannot append to the closed record stream. Such
attempts increment explicit RECORDGUARD late_accepts and do not change accepted
record totals. max_us measures the record section plus timing overhead; fixture
output only. Counters reset per observer segment (not cumulative over first and
recovered segments). Original stop timestamp is not extended to hide late data.
Reference algorithm and independent gate commit guards unchanged.

Release build and87Rust regression tests pass (host tests do not execute NVIC
preemption). Flashed after UART off; known APBENR1=08000000 clock repair applied
only afterPD1ODR0,BDTR0c1a,CCRs0 reads. guardcheck18/18,boot stack4264/2680/2492.
reentry_recordguard_01 stops during original startup at3.278560s,near60eHz,
on current ADC rail/peak guard reason4. Recorder path not reached; preserve as
startup failure,not a recorder regression or powered qualification.

One unchanged repeat reentry_recordguard_02 passes recovery near238.042eHz,
11409second COMs/accepts,cycle sigma22.187us,peak_raw348,bus_min10937mV,
original remaining7.988201s completed,final110us before original deadline.
Fresh12interval/seven-cycle acquisition,one final confirmation visit. All
retained timestamps<=end; RECORDGUARD late_accepts0,max_us15. This measures
bounded recording cost but does NOT demonstrate the late-refusal branch fired.
Further deterministic stopped-record testing/repeats remain useful.

Both final gates/MOE/ENABLE/CCRs0,nFAULT1;stack untouched2416/1836 respectively,
COM41closed/settings cleared. Raw/CSV/coast/plots retained and
reentry_recordguard_pair.csv includes BOTH attempts: startup failure then valid
recovery. E150 invalid post-stop capture remains unchanged. Mean-current,
time-resolved quality and matched-reference parity still open.

## Entry 152 — 2026-09-13 — direct stopped-record check and30s recovery campaign

Idle-only recordcheck calls the REAL recorder with LIVE_IRQ/OBS_STATUS left in
the states a stopped callback can see,ACTIVEfalse,POWER_DUTY0 then45,all six
sectors each. It restores configuration/counter diagnostics afterward,never
unmasks IRQs or grants gate authority. Checks12refusals and unchanged accepted
counts/statistics event count/prefix-tail counts/EVENTS/frozen end. This tests
the branch deterministically,not every NVIC preemption instant.

Adding this exceeded flash by256bytes. Older standalone corebench/coreirq now
join bench-adc-probes; production reference IRQ path remains enabled. Default
release builds. Flashed after off; exact known USART3 clock repair after safe
register reads. guardcheck18/18,recordcheck twice each reports:
RECORDCHECK pass=1 rejected=12 expected=12 unchanged_log_and_end=1 disabled=1 gate_authority=0
Thus24real-recorder stopped callbacks refused while bridge disabled. Stack after
these probes span4264,painted2680,untouched2404.78Python regressions pass.

reentry_recordcheck_30s_01: unchanged4.5% powered/6.5% startup50->200,800mA,
30s original powered allowance,trace0,seed0,stride19,dropout+reentry. Injected
loss at2s followed by fresh12interval/seven-cycle seed1424ticks in9039us and
successful remaining27.988168s run.40007second COMs/40006accepts,238.236eHz,
cycle sigma22.225us,147965ADCscans,peak_raw362,bus_min10793mV. No accepted
timestamps after end. RECORDGUARD late_accepts0,max_us15 in powered segment;
synthetic diagnostic above proves refusal branch independently. Final49us
before ORIGINAL30s-powered deadline,stack untouched1836. Reporter verifies
recovery/cycles/final off; captures/reentry_recordcheck_30s_report.csv and all
raw/ADC/coast/plots retained. Gates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed/settings
cleared. One30s campaign is not a repeatability cohort; mean-current and
matched-reference quality benchmarks remain incomplete.

## Entry 153 — 2026-09-13 — historical reference evidence recovered and frozen

Read FALCON_HARDENING and actual MAGPIE parser/lock-map scripts. Local historical
raw captures exist despite minz bench disconnection. New reproducible
scripts/minz_historical_baseline.py uses ONLY magpie.py from verified source ZIP
SHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
It snapshots three raw bins,two session logs,parser and historical notes with
per-file SHA256 manifest into captures/reference/minz_falcon_historical_20260913.zip.
All7payload hashes read back/verified; archive SHA256:
94df42404cd3e1735039fcba647bea93d795e6e11cd8823d446c921219355138.

lockmap_a9:5543windows,4.01632s,230.020eHz,99.98196%qZC,one missing window,
no sequence/sector gaps; nominal window-mean44.841mA,time-weighted44.850mA.
lockmap_a10:6220windows,257.476eHz,100% observed qZC,but one sequence/sector
discontinuity retained; nominal51.892mA. Do not claim uninterrupted coverage.
lockmap20_a9:4930windows,202.944eHz,20.5477%qZC,3917missing windows,longest
observed consecutive miss run5,nominal134.30mA. This is a DEGRADED historical
point,not an acceptable200eHz reference. The session/notes explain why low
post-engage amplitude can be a degraded regime; raw metrics stay authoritative.

These captures use historical FALCON independent windows,not proven frozen
AM32 firmware. MAGPIE has sanity checks but noCRC. Current conversion is nominal
3.3V/30mV-per-A DC-link sense,not DRV's bipolar phase sense; load/supply/controller
differences preclude current parity claims. Their window sigma is not binz's
whole-electrical-cycle sigma. Manifest explicitly marks parity/provenance false.
Three metric tests cover sequence wrap/gaps,independent miss denominator and
time-weighted current;81Python tests pass. BEMF_PARITY_PLAN current section
updated through this evidence. No hardware commands or firmware changes in
this entry; board remains on E152 with its last verified off state.

## Entry 154 — 2026-09-13 — bounded time-binned tracking statistics, not yet flashed

Added event_timeline.rs: six equal-time bins of accepted-event count and min/max
inter-event gap.64bytes per timeline; u32 counts cover900000events/600s without
u16 truncation. Gap crossing a bin boundary belongs to arriving event's bin;
empty bins remain empty. Gap>=65535us explicitly saturates max sentinel. Events
at/beyond planned end remain counted with overrun_events; reversed timestamps
are refused. No inference of physical missed crossings,qZC coverage or rotor
lock from this accepted-only stream.

Integrated push inside E151's stop-serialized record transaction. Fresh timeline
reset per observer and again with actual powered remaining duration at handoff.
StoppedSegment owns separate first timeline; archive validation requires its
count==whole-stream event count. Fixture-only ET85/FT85 six rows carry existing
Ascii85/CRC framing:bin,event_count_lo,event_count_hi,min_gap_us,max_gap_us.
TIMELINE header identifies planned window/bin width,total and overrun count;
TIMELINEREFUSED exposes invalid-timestamp attempts. No terminal bulk output.

91Rust tests pass,including four new timeline tests and archive preservation;
M0 library check passes. Final release rebuild passes after observer-reset update.
Board remains E152,not this instrumentation. Next:
strict host decoder/time plot,validate first/second totals and measured stop
extent,then build/flash and remeasure recorder latency,seed margin and stack
headroom before treating timeline captures as qualified. Added128static bytes
plus archive temporary may matter; do not infer stack safety from old margin.

## Entry 155 — 2026-09-13 — timeline decode validated; handoff setup regression localized

drv_event_timeline.py strictly checks CRC/row order/counts,header bounds,per-segment
whole-stream totals,stop extent,overrun accounting and refused timestamps. Four
tests cover corruption/missing/duplicate records,large counts,partial observed
bins and independent first-segment origin.85Python tests pass. Sustained reporter
now validates timelines when present; legacy captures remain replayable.

E154 release flashed after off; known UART clock repair after safe registers.
guardcheck18/18 andrecordcheck12/12 pass. Stack span4136,painted2552,untouched2276
after diagnostics. reentry_timeline_01 uses usual4.5% powered/6.5% startup,
50->200,800mA,10s,dropout+reentry. First segment2838COMs/2837accepts completes
to injected stop; FT85 CRC/totals valid. Fresh12interval/seven-cycle recovery
qualifies, but second handoff refuses freshness: edge_age298ticks,ci1436,
remaining~61ticks below64minimum. SEEDLAT reset188 vs172 previously: newly
inserted timeline initialization consumed~8-9us and erased tight arm margin.
No second power. ET85 correctly contains zero events; timeline_verified1 means
data integrity ONLY,not recovered operation. Reporter classifies powered_stopped.
Final off verified,nFAULT1,stack untouched1540,COM41closed/settings cleared.

Raw/CSV/coast and initial time plot retained. Visually inspected plot; subsequent
plotter correction uses observed partial-bin widths and marks stop explicitly,
labels empty second segment rather than suggesting a full-width observed bin.
Re-render with a new output filename; existing image remains initial artifact.

Source fix now moves timeline initialization AFTER ARR programming inside the
existing IRQ-disabled arm section. Its cost is included in unchanged16us arm
budget;32us freshness margin stays unchanged. No gate/recorder can run while
the initialization is in progress. Release rebuild passes. This fix is NOT yet
flashed: board still runs E154 initializer-before-arm build. Next flash/check
then test actual arm cost and first/second timeline counts on recovered operation.

## Entry 156 — 2026-09-13 — recovery with validated time bins, arm12us

E155 after-ARR initialization fix flashed following off verification. Known
USART3 clock repair only after APBENR1=08000000,PD1ODR0,BDTR0c1a,CCRs0 reads.
guardcheck18/18,recordcheck12/12,boot stack4136/2552/2276. No duty/guard changes.
reentry_timeline_arm_01: usual4.5% powered/6.5% startup50->200,800mA,10s,
trace0,seed0,stride19,dropout+reentry. First2839accepted events to injected stop;
new seed1435ticks,age280ticks,remaining ARR79ticks,actual arm12us INCLUDING
timeline initialization (limit16us). Recovery completes original remaining
7.988668s near237.403eHz,11380COMs/11379accepts,cycle sigma21.647us,
peak_raw355,bus_min10960mV,final89us before ORIGINALdeadline.

Both ET85/FT85 timelines CRC/accounting/stop extents validate. First bins contain
2364/475events then unobserved bins; recovered six bins1895/1895/1897/1897/1898/1897
events over~1.331445s each. Recovered min/max gap across bins558..839us; last two
bins' maxima837/839 versus792..795 earlier,visible despite steady count rate.
This is time-resolved accepted-event evidence,not qZC coverage or rotor proof.
TIMELINEREFUSED0,overrun0,no records after stop; RECORDGUARD late0,max_us20.
Stack untouched1548 (above512 gate),span4136. Final all gates/MOE/ENABLE/CCRs0,
nFAULT1,COM41closed/fixture cleared. shell-sine SHA unchanged.

captures/reentry_timeline_arm_initial.csv verifies recovery ANDtimeline. Raw,
ADC/coast and reentry_timeline_arm_01_timing.png retained; plot visually checked
for separate clocks,partial observed first bin extent and explicit stop markers.
Real capture added to decoder regression suite:86Python tests pass. One successful
timelined recovery is not repeatability/current/reference parity completion.

## Entry 157 — 2026-09-13 — current topology verified; offset bias measured

Visually inspected cached half-bridge/DRV schematic images and read drv8304.pdf
pp28-31. Three7mOhm LOW-SIDE shunts,not inline phase current. Signed long-term
sum can estimate bridge return current if phase sampling is unbiased; sequential
instantaneous sum or absolute phase peaks cannot represent meanPSUcurrent.

Ran four idle sensezero bursts using existing firmware,32samples each,
awake4939us,MOE/gatesoff throughout. Means(ch0,ch1,ch4):2052/2049/2051,
2054/2049/2053,2054/2050/2044,2054/2053/2050. Relative fixed2048 sum8/12/4/13counts
implies about92/138/46/150mA at nominal3.3V,70mV/A despite no drive. Each reports
wake_fault1 during wake,scan_fault0 afterward; this is not a running-driver
fault. Typical mean VREF1503..1504,vcal1662. Shortburst/noisy offsets do NOT
constitute final calibration. Every command endedEN/MOE0;finalp/i verifies
all gates/CCRs0,nFAULT1. COM41closed. No motor spin or firmware changes.

CURRENT_MEASUREMENT_PLAN.md records the decisive next gates: same-reader
per-wake zero statistics,proven PWM/sector sampling occupancy,signed accumulation,
uncertainty and independent mean-current anchor. Auto calibration across ENABLE
means recovery cannot silently reuse a previous-wake zero. Existing fast guards
remain unchanged. Budget RAM explicitly before histograms; current span4136
leaves40bytes above the existing span gate. This evidence prevents falsely
declaring current parity from hardcoded2048 or the800mA supply setting.

## Entry 158 — 2026-09-13 — same-reader bounded offset probe implemented

Added zero_stats.rs for512sample raw count,sum,u64squares,min/max; tests verify
wide square sum,word encoding,count bound and invalid ADC input refusal.
93Rust tests pass. Optional idle zerocheck uses actual powered_timer::read_channel
on4/1/0/6/13 after existing1100us driver wake,with all gates/MOEoff,ready/output
checks every conversion and200ms sampled-clock deadline. Partial/failure results
remain explicit; printing occurs afterENoff. It never applies calibration or
changes a guard. cap1 enables five compact CRC Z85 records and is consumed.

Normal release with probe exceeded flash by4288bytes. New bench-current-probes
feature isolates it; both normal release and size-optimized bench-probes feature
build now succeed. The latter is for disabled metrology only,not qualified for
motor IRQ timing. No flash or hardware calls this entry: board remains E156
normal release,previously verified off. Next:strict host Z85 validation,run
longer per-wake zero repetitions on diagnostic image,then restore qualified
release before any powered run. Document raw variance/repeat drift; do not
declare32sample noisy offsets a calibration or equate nominal current toPSU.

## Entry 159 — 2026-09-13 —512sample offsets measured; wake-to-wake shift persists

drv_zero_check.py captures idle off/p/i,cap1,zerocheck,then unconditional
off/cap0/stack/p/i cleanup,retaining raw even when decode fails. Decoder requires
one success header,512samples on exact4/1/0/6/13 channel order,CRC/moment bounds,
and POST-record off/nFAULT readback. --input replays without UART.

Optional bench-probes/current image flashed after safe checks. zero_guarded_01
collected complete records but immediate ZEROCHK disabled0 failed validation;
subsequent p/i alloff. Do not promote it to validated zero. Added bounded100us
poll afterPD1clear before declaring shutdown; failure remains reason7. GPIO
readback latency is the working explanation,not proof of which individual pin
was late. Rebuilt/reflashed diagnostic,then three unchanged repeated wakes:

|capture|duration_us|ch4 mean|ch1 mean|ch0 mean|
|---|---:|---:|---:|---:|
|zero_guarded_02|63673|2053.47070|2049.59766|2054.51563|
|zero_guarded_03|63672|2049.16797|2049.53320|2054.70898|
|zero_guarded_04|63673|2054.58984|2050.82031|2055.80469|

All512samples/channel,CRC/moments/finaloff validate. Current-channel individual
sample SD8.48..9.59raw counts; channel4 wake means span5.421875counts despite
larger averages. Fixed2048 subtraction still implies a sizable false no-drive
current. These establish repeatability limits,not an applied calibration. No
IID confidence claim from potentially correlated samples. Bus/VREF raw moments
also retained. No motor commands were issued on the size-optimized image.

Normal RELEASE restored afterward (optional probe excluded). Post-reset exact
known clock repair after safe reads,guardcheck18/18,recordcheck12/12,stack
span4136/painted2552/untouched2276,final gates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed.
89Python tests pass including real validated/rejected captures. Source shutdown
poll change only lives in optional diagnostic; normal motor path is unchanged.
Next current work needs same-wake baseline/measurement coupling and demonstrated
PWM-phase sampling coverage,not importing these offsets as permanent constants.

## Entry 160 — 2026-09-13 — same-wake paired offsets are substantially steadier

Optional zerocheck now takes two consecutive512sample sets without another
ENABLE transition. Total200ms bound unchanged,allgates/MOEoff; prints only after
verifiedENoff. Z85 first/ZB85 second,header windows2/same_wake1. Host decoder
requires both complete channel sets and finaloff; legacy single-window replay
retained.90Python tests pass,diagnostic build fits. Normal production path unchanged.

Diagnostic flashed after off verification; post-reset UART clock alreadyon,
safePD1/MOE/CCRs reads. zero_samewake_01/02/03 all succeed in125121us each,
1024samples per channel total. Delta(second-first) raw logicalA/B/C(ch4/1/0):
01:+0.490234,+0.068359,-0.658203
02:-0.328125,-0.027344,-0.914063
03:+1.406250,-0.423828,-2.025391
Nominal signed-sum current differences -1.146/-14.612/-12.004mA. Individual
channel changes remain visible; do not assume independent errors or claim this
is a statistical confidence bound. These are short stationary,gates-off pairs,
not PWM-current measurements or long-run drift qualification. They support
per-wake baseline coupling over permanently subtracting2048. Bus/VREF moments
retained for voltage normalization; all raw CRC/finaloff checks pass.

Normal release restored after measurements; known UART clock repair applied
after safe reads,guardcheck18/18,recordcheck12/12. Final span4136,untouched2276,
allgates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed. No motor commands during diagnostic
image use. Next current qualification must add proven PWM-phase occupancy and
same-wake baseline/measurement on the actual powered path,with RAM/timing budget
and an independent current anchor. Current/safety thresholds were not changed.

## Entry 161 — 2026-09-13 — voltage-normalized zero replay; coverage still unproven

Host-only drv_zero_check now accepts explicit --vcal for per-window voltage
normalization. Uses factory calibration at3000mV (E157 recorded1662), measured
mean VREFINT and CSA mean counts. Preserves legacy default decode, raw captures,
and all CRC/off validation; applies no firmware calibration. Two added tests
cover common ADC scale cancellation, a real differential offset, missing paired
window, invalid calibration and zero reference. All92Python tests pass.

Replayed zero_samewake_01/02/03 with1662: signed summed CSA voltage drift divided
by nominal70mV/A gives +1.893/-13.552/+10.204mA-equivalent. Third-pair VDDA changes
3312.381→3313.417mV, reversing the nominal-count -12.004mA-equivalent result.
This is output/bias drift, not actual current or a confidence interval. Ratio
of window means cannot recover within-window covariance. Evidence reinforces
the need for reference normalization and independent current anchoring.

Source inspection also confirms powered feedback lacks PWM-phase/aperture
records: scan cadence alone cannot establish unbiased current sampling.
No firmware changes, flash, UART access or motor run this entry. Last hardware
safing remains E160, not a newly observed live state. Next implementation needs
budgeted conversion-launch/sector occupancy alongside same-wake baseline,
without altering the fast guards or consuming fresh-seed arm margin.

## Entry 162 — 2026-09-13 — bounded ADC launch coverage wired, not yet flashed

Added adc_occupancy.rs and powered read_channel integration: first192 current
channel launch attempts per primary powered segment, eight TIM1 bins/channel.
Thirty-byte foreground-owned structure; no ISR histogram or UART work.
COMMITS generation plus TIM17 elapsed time brackets TIM1 reads around ADSTART:
reject generation change, backwards/out-of-range counter, >2us bracket,
>128tick width or bin straddle. This accounts for sixstep_write issuing EGR
on each commutation. Rejections consume the attempt budget; never replace bad
data until bins look filled. Launches may later fail ADC completion and are
NOT successful sample counts. Launch bracket is NOT the sampling aperture.

Fixture-only ADCLAUNCH/AL85 rows use existing CRC codec, reporting primary
segment only (re-entry resets the collector; first segment not archived).
Host drv_adc_occupancy checks header, CRC, ordered three channels,192 cap and
count conservation. It explicitly does not certify current accuracy. No quiet
terminal dump added. No changes to duty or guard limits or shell-sine.

Release builds; ELF stack_start20009000 minus stack_end20007ff4 =4108bytes,
above4096 gate but only12bytes slack.96Rust tests pass including three coverage
tests; M0 library check and94Python tests pass. shell-sine hash remains
B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.
No flash/UART/motor commands this entry. Device remains prior qualified image;
new target release is UNQUALIFIED for live timing. Before powered use: safe
flash/readback, guardcheck/recordcheck/stack, then bounded known-point capture
with measured guard/ADC/stack timing. Do not infer long-run uniformity, joint
sector coverage, physical aperture or mean current from early launch bins.

## Entry 163 — 2026-09-13 — two powered coverage runs; early-bin deficit repeats

Verified UART all gates/MOE/ENABLE/CCRs0,nFAULT1 before flash. Downloaded E162
normal release; reset readbacks RCC08000000,PD1ODR0,BDTR0c1a,CCRs0 permitted
known USART clock repair08040000. guardcheck18/18 stopped writes refused;
recordcheck12/12; idle stack4108/2524/2248; outputs verifiedoff before motor.

Ran unchanged fixture profile twice:50target,6.5% catch/start,10eHz ladder
60..200,4% powered handoff,10000ms window,stride19,trace0,seed0,no injected
dropout/reentry. Existing~11.8V rig and800mA PSU setting; no supply adjustment
or independent mean-current reading. launch40_01 and02 retain raw/csv/png/coast.
Both powered windows complete at205.418/205.653eHz,12325/12339COMs,
12324/12338acceptances,cycle sigma33.000/33.041us. Feedback scans50195/50561,
peak_abs_raw333 each,bus floors10925/10471mV. Guard ISR max15/16us,commit18us,
recorder20us,late_accepts0; untouched stack1860 each. All final gate/MOE/EN
readbacks0,nFAULT1 and COM41closed. No ADC fault or premature tracking stop.

CRC-validated first192launch attempts/channel:

|run|phase|rejected|eight PWM launch bins|
|---|---|---:|---|
|01|A|7|12,14,31,26,25,26,33,18|
|01|B|6|16,13,24,25,25,27,29,27|
|01|C|8|6,22,26,27,24,28,27,24|
|02|A|6|10,19,28,23,28,28,25,25|
|02|B|5|13,21,27,21,25,32,28,20|
|02|C|10|9,24,22,34,20,26,27,20|

All bins visited but earliest-bin undercoverage repeats. This disproves an
assumption that broad visitation alone establishes uniform sampling; it does
not identify cause or quantify current bias. TIM1 reset at commutation and
foreground ADC scheduling are candidate mechanisms,not yet isolated. Brackets
cover launches,not apertures; no joint-sector or long-run occupancy evidence.

Sustained reporter now validates optional ADCLAUNCH/AL85 before classifying a
run; missing/corrupt channel data refuses analysis.95Python tests pass including
real hardware capture and tampered-record regression. Pair summary:
captures/launch40_pair_verified.csv (2/2complete,not lock/current certificate).
Current flashed E162 now has two known-point powered passes; broader range and
re-entry timing on this build remain untested. Next: source-grounded aperture
timing and decorrelation design using this observed bias,without duty expansion.

## Entry 164 — 2026-09-13 — live ADC settings establish tracking-time limitation

Read-only probe confirms ADC_CFGR1/CFGR2/SMPR=00000000/80000000/00000007 and
RCC_CFGR00000012 on E162. Cross-checked current source and installed G071 PAC:
PCLK/4=16MHz,160.5cycle tracking,12-bit SAR12.5cycles. Derived tracking10.03125us
and conversion10.8125us excluding launch/selection overhead. Nominal PWM-on
slice at4%/10kHz is4us; launch histograms cannot be interpreted as sample/hold
endpoint histograms or current correction weights.

ST RM0444 indexed sections15.3.9/15.4.3 retrieved: sampling charges the hold
capacitor; EOSMP marks sampling end, EOC conversion completion. Full PDF fetch
timed out; no claim to have read/rendered the complete manual. Current plan
links the source. Tracking is not an ideal boxcar average. A shorter sample
setting would require source-settling/VREFINT validation and renewed zero tests;
do not alter the existing guard reader merely to fit inside4us.

No firmware edits,flash or motor run. After probe reads,UART off/p/i verified
all gates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed. This changes the next design:
characterize sample/hold timing and explicit ADC ownership for timed sampling,
not a histogram-based correction of the existing mean. E163's two powered
passes remain valid retention evidence,not current parity.

## Entry 165 — 2026-09-13 — fixed3/3 sustained recovery cohort near238eHz

Same flashed E162 build as E163; initial UART off/stack/p/i confirms disabled
and span4108. Existing rig/800mA PSU setting,unchanged6.5% startup/catch and
50→200eHz ladder in10eHz steps. Powered4.5%,30000ms window,stride19,trace0,seed0,
explicit --dropout --reentry. Three planned attempts,none omitted or replaced:

|capture launch45_reentry30_|eHz|recovered COMs|cycle sigma_us|bus floor_mV|peak_abs_raw|
|---|---:|---:|---:|---:|---:|
|01|237.509|39886|21.749|10769|349|
|02|237.826|39939|21.039|10853|344|
|03|238.031|39973|21.146|10841|343|

All inject COMP suppression at~2s,stop on tracking,qualify fresh12interval seed
with seven rolling cycle checks,then complete remaining~27.988s inside original
30s powered campaign budget. Re-acquisition9019/9035/8988us; one retry each.
First-segment archives remain distinct from recovered clocks. Final elapsed
precedes original end by137/20/83us respectively. Every raw capture passes
post-run allgates/MOE/ENABLE/CCRs0,nFAULT1;fixture cleanup closesCOM41.
Stack untouched1520 each,recorder max20us,late_accepts0. Run01 foreground end
wins shutdown race (POWERPATH reason0);02/03 segment guard ends (reason2).
Reporter validates original deadline and stopped state,not reason alone.

captures/launch45_reentry30_cohort.csv reports3/3 powered_window_complete and
reentry_verified1 with valid timelines/launch CRC rows. Timing plot for01
generated and visually inspected: approximately stable accepted-event rate in
six recovered bins; no assertion of independent qZC or rotor lock. All raw
attempts,startup/coast CSVs and plots retained.95Python tests pass.

Frozen128file minz archive and live source reverified SHA256
2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44;
shell-sine hash unchanged. Added PORTABLE_WINS.md separating reusable policy
from G071 register fixes and timing/stack caveats; no sibling writes. Updated
parity plan to current evidence. Same-build sustained recovery at~238 is now
repeated,not just a single success; current/independent quality/matched-reference
provenance remain unfinished. No duty ceiling expansion or ADC changes.

## Entry 166 — 2026-09-13 — independent early-coast full-period bounds

Added host-only drv_coast_periods.py: first verifies existing capture CRCs,
powered completion and post-run safing via sustained reporter, then uses the
first32 measured coast timestamps and per-phase comparator completion offsets.
Read windows run from preceding comparator completion (scan entry for A) to
the current completion. Explicit --uncertainty-us expands each endpoint to
cover unmeasured caller/scan skew and quantization; it is an assumption,not a
measured hardware guarantee. Crossings are bracketed between adjacent opposite
level reads; same-polarity crossings two transitions apart give full-period
bounds. No controller timing is used to select edges, no polynomial fit or
extrapolation to gate-disable time. Overlapping read bounds, missing offsets,
insufficient cycles and invalid raw captures refuse analysis.

Replayed launch45_reentry30_01/02/03 at uncertainties0 and100us. Each phase has
5–6 full periods in the early32row window. At100us the smallest lower-frequency
bounds are184.4–184.5eHz for A and186.1eHz for B/C; largest upper bounds are
319.4–319.5 and314.5–314.6eHz respectively. Broad bounds include the controller's
237.5–238.0eHz and exclude half that rate (~119eHz) for every retained period.
This is evidence against a contemporaneous half-speed rotor conditional on
real/no-missed comparator transitions and the specified timing uncertainty.
Coast is after drive removal; no assertion of whole-powered-run rotor lock,
independent-window qZC, or bounded rotor acceleration/deceleration is made.

Three tests cover full-vs-half-cycle units, missing motion, read ordering and
real capture/missing-offset refusal;98Python tests pass. Reproduction:
python scripts/drv_coast_periods.py captures/launch45_reentry30_01.txt --uncertainty-us 100
No firmware,UART or motor
actions this entry; E165 finaloff remains the last observed hardware state.

## Entry 167 — 2026-09-13 — IRQ-prefix information limits verified in replay

PSU-anchor operator reading remains pending; no operator-observed measurement
was started. Investigated existing comparator evidence instead. core_bench sets
TRACE_READS only around comp_isr and retains first/last levels plus read count
for32handlers. No pre-IRQ history,intermediate sequence or independent sensing
window denominator exists in this format; trace0 records none of these reads.

Added drv_irq_trace.py --evidence-summary without changing legacy row decode.
Reports omitted handlers,unequal endpoints and accepted handlers whose
endpoints cannot establish a transition. Equal endpoints do NOT mean no
transition occurred: one can precede handler entry or occur between endpoints.
Unequal endpoints still cannot identify rotor vs switching origin. No qZC or
fresh-edge acceptance rate is manufactured.

Replayed handoff_trace1_01 and seed3_trace1_04:32retained,13119/13185omitted,
two handlers with unequal endpoints,zero accepted with unequal endpoints and
two accepted with inconclusive equal endpoints in each prefix. These are old
captures,not current E162 observations. This rules out using this prefix as
whole-run quality evidence; it does not diagnose bad lock.99Python tests pass.
No firmware/flash/UART/motor actions this entry. Earlier off confirmation from
supply-anchor preparation remains the last live state observation.

## Entry 168 — 2026-09-13 — separate ADC hardware-trigger route probe built

Inspected src/harvest.rs's existing EVL TIM6→ADC→DMA spine. It uses101us
trigger cadence to sweep100us PWM phase, but cannot be imported directly:
TIM6 now owns the100us safety guard,its channel map/safing are EVL-specific,
and the current shell ADC has exclusive foreground ownership.

Caught an actual source-description conflict: installed G071 PAC EXTSEL enum
labels011 as TIM2CH4 and101 as TIM2CH3, whereas ST RM0444 Rev6 Table73 lists
011=TIM3_TRGO and101=TIM6_TRGO. Official indexed table retrieved from
https://www.st.com/resource/en/reference_manual/rm0444-stm32g0x1-advanced-armbased-32bit-mcus-stmicroelectronics.pdf
Use manual-grounded raw bits and verify on hardware; don't rewrite old harvest
to match stale enum names. TIM2 is already the reference interval timer.

Added optional bench-current-probes command adctriggercheck. All gates/ENABLE
off,no wake: TIM3 TRGO101us,ADC13 VREFINT,same sample time,DMAMUX request5,
DMA1CH1 finite32halfwords. Refuses active resources,10ms wait bound,checks
transfer error/remaining count/ADC overrun/range and expected3–4ms duration.
Stops trigger and ADC before releasing64byte local buffer; bounded ADC-stop
failure panics through board safing. Restores prior ADC config/channel mask,
leaves borrowed TIM3/DMA disabled. It is not a powered ADC-ownership solution.

Diagnostic size-optimized and normal release builds pass. New command is
excluded by constant feature gating in normal release; no flash/UART/motor
commands this entry. Next: disabled hardware repetitions and normal-image
restore. A valid trigger route is necessary but does not prove unbiased mean
current,source settling or whole-run quality. Operator PSU reading still pending.

## Entry 169 — 2026-09-13 — TIM3/ADC/DMA route verified3/3 with driver disabled

Initial UART off/p/i verified allgates/MOE/ENABLE/CCRs0,nFAULT1. Flashed optional
bench-probes/current diagnostic,reset; known USART clock repair applied only
after RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 readbacks. New fixture
scripts/drv_adc_trigger_check.py sends no motor commands,retains raw including
failure,executes three adctriggercheck calls and unconditional off/stack/p/i.

captures/adc_trigger_01.txt: all result0,n32,elapsed3247/3246/3246us,VREFINT raw
min1497/1499/1499 and max1508/1509/1508. Confirms rawEXTSEL3 TIM3_TRGO at101us,
ADC13 and DMAMUX request5/DMA1CH1 finite transfer on this G071. Successful
firmware result also checks remaining DMA count0,no ADC overrun/error and
complete-buffer raw range. Text summaries,not CRC raw-sample dumps; don't
promote these aggregate diagnostics to precision voltage/current captures.
Disabled diagnostic stack span4024,untouched2508: not motor-qualified.

Normal release restored immediately afterward. Safe reset readbacks then exact
USART clock repair;guardcheck18/18,recordcheck12/12;span4108,untouched2248.
Final UART allgates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed. No motor run or driver
wake in this entry.100Python tests pass including actual trigger replay and
failed/short/missing-finaloff refusal.

Next DMA work must prove five-channel scan ordering and coherent buffer
ownership/overrun refusal,then arbitrate against the existing foreground guard
reader. Current probe cannot run concurrently with that reader. The successful
route test is not permission to replace guard feedback or claim PWM occupancy.

## Entry 170 — 2026-09-13 — finite five-channel DMA scans pass2/2

Extended optional disabled probe with adcscancheck: TIM3 triggers32 scans of
ADC0/1/4/6/13 into160halfwords. Noncircular DMA is stopped with ADC/timer before
buffer reading; captures expose remaining0/stopped1. cap1 alone enables32CRC
AS85 rows,index plus five raw values; normal terminal gets one summary.
Bitmask ascending order is C/B/A/bus/VREF,not logicalA/B/C; host explicitly
reorders. New drv_adc_scan_check.py verifies metadata,count/order,CRC,VREF/raw
range and finaloff; retains failure captures and always cleans up.

After preflightoff,diagnostic flashed; reset UART clock already08040000 with
safe GPIO/TIM1 reads. adc_scan_01/02 each pass32scans,160words,3288/3287us,
remaining0,stopped1. VREF1503–1510/1501–1509,bus1196–1232/1195–1227. Current
channels change between captures (A1883–2021 then1766–1883,B1809–1930 then
1687–1777,C1797–1933 then1647–1763) with ENABLEoff. These voltages are not
awake-CSA zero calibrations. Current mapping is a code/source contract retained
from prior wiring tests; these disabled values don't independently prove each
physical phase identity. Finite transfer and distinct bus/VREF placement are
observed; no live cyclic-buffer ownership claim.

Normal existing release binary restored after diagnostics; known UART repair
only after RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 reads. guardcheck18/18,
recordcheck12/12,stack4108/2524/2248,final allgates/MOE/ENABLE/CCRs0,nFAULT1,
COM41closed. No driver wake/motor runs this entry. Diagnostic stack span4024,
untouched2184,not a motor-qualified image.103Python tests pass including real
scan replay,logical reorder and partial/unstopped/missing-finaloff refusal.

Next integration requires producer/consumer ownership for ongoing scans and
explicit arbitration with the guard reader. Do not read a live single DMA
buffer as though it were an immutable five-channel snapshot. Existing normal
motor control and guard reader remain unchanged; new scan code is optional.

## Entry 171 — 2026-09-13 — two-half DMA ownership policy tested on host/M0

Added pure examples/support/dma_snapshot.rs through observer-replay. Proposed
geometry is two five-word scan halves,one hardware scan per101us. Lease::begin
requires exactly one HT/TC completion flag and NDTR showing DMA working on the
OTHER half. Both flags are refused,not interpreted as two safe snapshots.
Caller must clear only the leased flag,copy volatile words,then verify flags,
NDTR and elapsed time before publishing. finish refuses a boundary flag,error,
changed acquisition epoch,position reversal or elapsed>100us. This bound relies
on the stated hardware trigger/scan geometry; it is not a generic DMA proof.

Tests cover both halves,every NDTR0..11,midcopy wrap,both pending flags,long
pause that can hide a DMA revolution,transfer error,epoch restart and logical
ADC0/1/4→C/B/A reorder.100Rust tests pass (78unit,1recorded,21sequence),M0
library check passes. No driver/hardware integration,no flash/UART/motor run,
no production RAM change. Latest live off observation remains E170.

Next is disabled cyclic-DMA hardware exercise using these checks,including an
intentional delayed consumer that MUST refuse data. Only after that may this
publisher replace foreground feedback under unchanged current/bus/age guards.
Current/quality parity and the independent PSU anchor remain unresolved.

## Entry 172 — 2026-09-13 — cyclic DMA ownership and delayed-consumer refusal on hardware

Connected dma_snapshot::Lease to optional disabled adccycliccheck/stall.
TIM3/ADC five-channel scans write a10halfword circular ring. Foreground leases
the completed half,acknowledges only its flag,copies volatile words,then checks
NDTR/flags/time before storing an owned diagnostic scan.32accepted copies stop
the experiment;10ms backstop unchanged. Fixed epoch0 is valid only for this
single-owner probe with no restart. No active-guard feedback caller added.
adccyclicstall waits350us before consumption to accumulate ambiguous flags.

After preflightoff,flashed diagnostic and applied known UART-clock repair after
safe register reads. Raw fixtures: adc_cyclic_01,adc_cyclic_stall_01,adc_cyclic_02.
Normal runs each copy32coherent scans in3286us,copy_max4us,remaining10 at stop,
lease_fault0. Delayed run refuses at350us,result9/lease_fault2(AmbiguousFlags),
copies0,remaining5; no invalid snapshot is published. A subsequent normal run
passes,demonstrating reuse after refusal. All report stopped1 with finaloff.
CRC CS85 rows carry only accepted copies; missing/error/stall-publication
regressions are checked by drv_adc_cyclic_check.py.105Python tests pass.

Diagnostic stack span4024,untouched2056; no driver wake/motor commands. Restored
existing normal release afterward,reset/readback then known clock repair.
guardcheck18/18,recordcheck12/12,normal stack4108/2524/2248,final allgates/MOE/
ENABLE/CCRs0,nFAULT1,COM41closed. This does not qualify ISR coexistence or
powered-current accuracy. Next integration requires a publisher with explicit
freshness/error handling and unchanged guard thresholds,plus production RAM
budget; do not replace foreground ADC reads with unchecked circular memory.

## Entry 173 — 2026-09-13 — acquisition-aged guard feedback contract

Added Guard::feedback_aged(now,age_us,sample) for a future DMA publisher.
Checks delivery-time gap and sample age against unchanged1000us limit BEFORE
calling existing electrical checks with the original acquisition timestamp.
Repeated consumption of the same cached frame cannot refresh last_feedback;
late new data cannot repair an already missed deadline. Reordered acquisition
timestamps fail closed through the existing stale-feedback check. Existing
foreground feedback method/callers and electrical thresholds are unchanged.

Three tests cover repeated cached-frame delivery until expiry,new-but-late and
reordered delivery,latched refusal,modular32bit clock wrap and current fault
preservation.103Rust tests pass (81unit,1recorded,21sequence);M0 library check
passes. No extra guard state/staticRAM. Publisher still must supply truthful
age in the same serialized guard clock domain; this method cannot infer
acquisition time from DMA flags. No firmware flash/UART/motor action this entry;
latest live off observation E172. Integration and independent current anchor
remain open,not redefined as completion of these helper tests.

## Entry 174 — 2026-09-13 — experimental DMA feedback connected, not flashed

New opt-in bench-dma-feedback feature connects adc_stream.rs to the actual
powered foreground loop. Startup/flying acquisition retain original ADC reads;
stream starts on first powered service after seed timer arming,not inside the
16us arm critical section. TIM3/101us hardware scans drive a10word circular
ring,with exclusive foreground consumer using tested Lease checks and expected
half alternation. Only NEW coherent scans feed Guard::feedback_aged; no cached
frame replay. Lower-bound trigger epoch sampled before CEN plus scan_count*101
preserves acquisition age. Missing triggers make age older,not refreshed.
This timing assumption still needs validation with actual control IRQ load.

Stream has no gate authority. Errors report ADCFAULT stage21..28,trip existing
AdcTimeout fault and stop the stream. Normal exit disables bridge first,then
stops ADC/DMA and restores prior ADC config before coast/re-acquisition. State
is foreground-owned,so fixed lease epoch0 has no concurrent restart within a
lease. Safety ISR remains independent and can disable outputs during DMA work.
Current/bus/fault/age thresholds unchanged. No live DMA interrupt added.

Initial experimental link exceeded flash by384bytes. Non-inlined stream helpers
alone did not fix it. Standalone pwmcheck now requires bench-adc-probes only
when bench-dma-feedback is enabled; normal default command remains available.
Guardcheck/recordcheck and motor controls remain. Experimental build replaces
old foreground-launch histogram storage with stream state and reports
DMAFEEDBACK instead of incompatible ADCLAUNCH rows. Old replay unchanged.

Experimental release now builds; ELF stack_start20009000/end20008000 =>4096bytes
EXACTLY existing minimum. Experimental ELF SHA256 at this build:
1FEA07F7F78215ADC0709E94A553FA4DC62C4A1A6E0CA736CCD86769FD1F4312.
103Rust/105Python tests pass. Normal no-feature release also rebuilt successfully
afterward,so target/.../shell-pwm now contains DEFAULT,not experimental binary.
Device was not flashed or accessed: remains previous normal image,latest
observedoff E172. No hardware qualification is claimed for the new stream.

Next: rebuild explicit feature,verify disabled safing/stack,then bounded known
point with all raw failures retained. Powered IRQ coexistence,acquisition-age
assumption,ADC cleanup and re-entry need measured evidence before promotion.

## Entry 175 — 2026-09-13 — powered DMA consumer falls behind; guard refuses data

Built/flashed explicit bench-dma-feedback after preflightoff. Safe reset reads
then known USART repair;guardcheck18/18,recordcheck12/12,stack4096/2512/2236.
Attempted same50→200ladder,6.5% startup/catch,4% powered,10000ms planned window,
trace0/stride19. All attempts retained in dma_feedback40_attempts.csv:

- dma_feedback40_01: startup completes;AdcTimeout guard reason11 at831us after
  five published DMA scans,ADCFAULT stage25(begin ownership refused). Stack
  untouched1904,peak122,busfloor11605. No completed powered window.
- Added packed DMA flags/remaining-count details to stream errors and explicit
  DMAFAULT line; rebuilt/reflashed experimental. No refusal thresholds changed.
- dma_feedback40_02: startup current guard reason4 before DMA starts. Retained
  as startup_stopped,not evidence of DMA behavior.
- dma_feedback40_03: startup completes;reason11 at800us after four scans.
  DMAFAULT code5 flags7 remaining5: GIF+TCIF+HTIF set,not TEIF. Consumer has
  accumulated both completion flags and correctly refuses ambiguous ownership.
  Stack untouched1904,peak140,busfloor11689. No completed powered window.

Both powered attempts disable alloutputs and restore ADC for valid coast dumps;
allthree finaloff/nFAULT readbacks pass. DMAFLAG diagnostic is authoritative
for the third attempt; do not assume it proves the exact cause of the first.
Legacy ADCFAULT launch_us carries packed details for stage>=20; use decoded
DMAFAULT flags/count,not that legacy field as a duration in these cases.

Restored no-feature default release after tests;reset RCC already08040000,
safePD1/TIM1 readbacks. UART guardcheck18/18,recordcheck12/12,stack4108/2524/2248,
final allgates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed. NewDMA is NOT qualified or
promoted. Next must localize consumer scheduling delay under real COMP/COM
load; accepting both flags or refreshing cached scans would conceal data loss.

## Entry 176 — 2026-09-13 — measured301us DMA consumer service gap

Added last-poll TIM17 timestamp in stream State padding (experimental stack
span remains4096). Every poll, including empty polls, updates it. Ownership
errors encode modular service gap in upper16bits; DMAFAULT decodes this
separately from flags/NDTR. This is diagnostic elapsed time, not a >65ms
blackout watchdog. No timing/ownership/electrical thresholds changed.

Rebuilt/flashed explicit bench-dma-feedback after verifiedoff; disabled
guardcheck18/18,recordcheck12/12 passed. Known50→200eHz ladder,6.5% startup,
4% powered,10000ms planned,trace0/stride19: dma_gap40_01 raw/CSV/PNG/coast
retained. Startup and fresh acquisition completed. Powered guard AdcTimeout11
at818us after4scans; DMAFAULT code5 flags7 remaining5 service_gap_us301.
This directly proves foreground went301us between polls, enough for both
completion latches at101us scan period. It does NOT attribute that gap solely
to IRQs:16COMP calls,max47us,COMmax40us are counts/maxima,not summed CPU time.
Peak_abs_raw135,busfloor11593mV,stack untouched1888. No completed powered
window. dma_gap40_attempts.csv validates capture/finaloff;0/1complete.

105Python tests pass; experimental and default release builds pass. Restored
default firmware, safe register readback then known UART-clock repair;
guardcheck18/18,recordcheck12/12,stack4108/2524/2248. Allgates/MOE/ENABLE/CCRs0,
nFAULT1,COM41closed. shell-sine hash unchanged. Next distinguish time in
foreground reference bands from interrupt preemption, then provide bounded
DMA publication without weakening ambiguous-ownership or acquisition-age
checks. Current/quality/reference parity remains incomplete.

## Entry 177 — 2026-09-13 — COMP spans exceed DMA foreground service budget

Experimental-only COMP wrapper now measures complete handler spans, including
early returns. COMP_MAX packs low16 maximum and high16 modular accumulated
span; printed maximum remains decoded. No extra static RAM. ADC_FAULT[1]
holds the previous sum while polling, then the inter-poll delta on refusal.
Snapshot is immediately before poll (approximately matching its TIM17 gap).
Spans include higher-priority preemption and omit exception entry/exit and
wrapper bookkeeping; they are NOT exclusive COMP CPU time. Normal default
build does not accumulate spans or alter reference filtering.

After preflightoff and explicit experimental flash,guardcheck18/18 and
recordcheck12/12 passed,stack4096/2512/2236. dma_irqgap40_01 uses same
50→200ladder,6.5% startup,4% powered,10000ms bound,trace0/stride19. Startup and
acquisition complete; powered AdcTimeout11 at856us after5scans. DMAFAULT code5
flags7 remaining5 service_gap_us246 comp_span_us173.14COMP calls,max57us,
COMmax41us. Peak_abs_raw183,busfloor11450mV,untouched stack1912. Raw capture,
CSV,PNG,coast retained; dma_irqgap40_attempts.csv validates0/1complete and
finaloff. No sustained DMA pass.

COMP-handler spans alone exceed101us scan cadence during this gap. Thus
foreground-only optimization cannot reliably satisfy the current ring's
deadline. Source am32_isr::comp_isr deliberately retains a pending post-ZC
level while its timing gate is closed; this can repeatedly service COMP.
This source mechanism is consistent with the delay, not independently counted
as the exact cause of each handler in this capture.

Next implementation direction: bounded interrupt-owned DMA publisher able to
preempt COMP, preserving guard scheduling, coherent-copy/age refusal and
explicit start/stop ownership. Do not invoke current foreground-owned stream
from an ISR concurrently. Publication storage and interrupt nesting must fit
the4096byte stack-span gate; prioritize this actual scheduling fix over more
foreground timing probes. Retain full minz reference semantics.

Both release builds and105Python tests pass. Default rebuilt/restored; safe
reset reads followed by known UART clock repair. Final guardcheck18/18,
recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFAULT1,COM41
closed. Full current/quality/reference parity remains incomplete.

## Entry 178 — 2026-09-13 — DMA IRQ delivery works; sustained tracking1/3

Opt-in bench-dma-feedback now uses DMA1_CHANNEL1 HT/TC/TE interrupts. Only
the DMA ISR consumes scans while enabled. Foreground starts/restores with
the vector masked; stop masks/unpends before borrowing state and stopping ADC.
ISR error masks delivery, immediately trips existing guard, leaves ADCSTOP/
restore to foreground. It never enables gates. Foreground no longer polls
circular data. No mailbox or skipped-frame publication: each coherent scan
goes directly through existing conversion and acquisition-aged guard feedback.
Calibration is read from the same factory VREFINT_CAL_ADDR as foreground.

DMA priority0 equals guard TIM6 priority0; neither nests the other, both
preempt COMP0x40 and COM0x80. Existing200us guard tick-gap,1000us sample age,
electrical and tracking limits unchanged. DMA ISR maximum uses one padding
byte in40byte State,saturates255us; fixture DMAFEEDBACK max_us_u8 exposes it.
COMP-span diagnostic from E177 removed after answering its question; historical
raw records retain it. Experimental link initially exceededflash32bytes;
non-inlined startup helper and shorter new diagnostic label restore fit.
ELF stack span4096 exactly. Experimental SHA256:
E393E94CA87DA025B5341755D177513F8F790C8556A83AD7EDB4C176C37F7714.

103Rust tests and105Python tests pass,M0 library check passes. After preflight
off/flash,guardcheck18/18,recordcheck12/12,stack4096/2512/2236. Test pass gates
stated before running: full10s,no guardfault,DMA ISR<100us,untouched stack>=512,
final outputs off. Same50→200ladder,6.5% startup,4% powered,trace0,stride19.
All three attempts retained in dma_irq40_cohort.csv:

-01: Tracking8 at12875us,126DMA scans,15COMs,peak301,busfloor11510mV.
  Accepted14events,last11914us,end12919us,one late callback refused. Notpass.
-02: full10s completed,no guardfault,99008DMA scans,12271accepted events,
  mean accepted-cycle rate204.522eHz,peak405,busfloor10387mV. First pass.
-03: Tracking8 at6227235us,61654DMA scans,7636accepted events,mean accepted
  cycle rate204.406eHz,peak403,busfloor10781mV. Notpass.

Allthree DMA ISR maxima27us,guardISRmax4us,untouched stack1800. No DMAFAULT
ownership errors. Later COMP/COM maxima88/78us include preemption; commitmax46.
All capture CRC/coast/finaloff checks pass. After02 hardware readback confirmed
DMA ch1 EN/interrupt bits clear,ADC ADSTARTclear/CFGR1zero,NVIC ISERzero.
This demonstrates delivery past the old sub-ms failure,not reliable tracking
or current metrology.1/3 qualifies neither promotion nor parity. Tracking
loss remains real; added high-priority service is a timing-interference
candidate,not proven sole cause. Next reduce ISR interference while retaining
every scan's electrical checks,coherent ownership and original acquisition
age; do not label tracking failures variance or relax the guard to pass.

Restored default release after cohort; safe register reads,RCCalready08040000.
Final guardcheck18/18,recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed. shell-sine SHAunchanged. Default still uses original
foreground ADC path. Whole-run independent quality,mean current and matched
frozen-reference parity remain open.

## Entry 179 — 2026-09-13 — exact reciprocal optimization measured slower; reverted

Inspected compiled DMA handler: two __aeabi_uidiv calls per scan,one variable
VREF divisor and one constant100. Trial replaced only /100 with exact
((n as u64*1374389535)>>37). Equivalence proof for allu32 and tests over5million
consecutive inputs,full-width boundaries and100000pseudorandom values passed.
Experimental build initially exceededflash96bytes; sharing conversion via
inline(never) restored fit,stack4096. Disassembly confirmed __aeabi_lmul helper
instead of the fixed-divisor call. Thus trial combines arithmetic replacement
and shared-function layout,not an isolated multiply instruction benchmark.

After preflightoff/flash and guardcheck18/18,recordcheck12/12,ran same known
50→200ladder,6.5% startup,4% powered,10s planned,trace0/stride19.
dma_fastdiv40_01 raw/CSV/PNG/coast retained. Startup and acquisition completed;
Tracking8 at8597us,84coherent DMA scans,noDMAFAULT. DMA max33us versus prior
27us; stack untouched1728 versus1800.9accepted events,last7628us,end8638us.
Peak_abs_raw294,busfloor11534mV. dma_fastdiv40_initial.csv validates capture
and finaloff,0/1complete. Arithmetic correctness did not yield timing gain.

Rejected and reverted this trial entirely,including new adc_scale helper/tests
and conversion sharing. Historical formula/proof retained here: M*100=2^37+28;
for n=100q+r the residual28q+r*M is nonnegative and at most137267154781<2^37,
so the shift equals q. No user files removed; only this turn's helper deleted.
105Rust/105Python tests passed during trial; retained source returns to E178
with103Rust tests. Normal release rebuilt/restored; next work should address
priority workload partitioning,not assume this reciprocal change is faster.
Current/quality/reference parity remains incomplete.
Post-restore RCCalready08040000,safePD1/BDTR/CCR readback. Finalguardcheck18/18,
recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/CCRs0,nFAULT1,COM41closed.

## Entry 180 — 2026-09-13 — FIFO splits IRQ workload;3/3 complete10s near205eHz

Experimental DMA ISR now performs coherent-copy checks and the SAME raw phase
current limits,then publishes an owned scan/timestamp into an8-entry FIFO.
Foreground drains at most8entries per visit and performs original conversion,
bus/VREF and acquisition-aged feedback checks. No latest-only overwrite,
timestamp refresh or sample skipping. Queue-full latches refusal and trips
AdcTimeout with DMAFAULT code9; queued old data still expires at1000us.
Phase overcurrent remains immediate in DMA ISR. Bus feedback can now be
delayed by queued foreground service,within unchanged stale-feedback bound.
DMA and TIM6 remain equalpriority0,COMP0x40. Start clears queue metadata while
DMA IRQ masked; foreground pop uses a short critical section; producer never
runs concurrently with a queue mutable borrow. Guard never accesses queue.

Queue uses132bytes. Experimental optional IRQ trace prefix now27rows instead
of32,reclaiming140bytes; wire row format and normal32row capacity unchanged.
Experimental sensezero legacy metrology command now requires bench-current-
probes (normal availability unchanged); this removed704byte flash overflow.
The newer zerocheck was already gated. Motor feedback/guards/dumps retained.
Final experimental stackspan4096. ELF SHA256:
EAD28778694FB67E3E9ED52209CF1D577C875C4EBD58884B0CABBF6E4B1BD9AF.

107Rust tests pass: FIFO ordering/wrap,full-latch/restart,stale queued timestamp,
and allu16 phase values in each phase against unchanged848..3248inclusive
limits.105Python tests,M0 library check and both release builds pass.
After safe flash/known UART repair,guardcheck18/18,recordcheck12/12;
stack4096/2584/2308 idle. Same50→200ladder,6.5% startup,4% powered,10s planned,
trace0/stride19. All three attempts retained in dma_fifo40_cohort.csv:

-01: fullwindow,normal segment deadline2 at10000015us;205.323eHz,
  cycle sigma36.609us,12320COMs,12319accepted,peak408,busfloor10901mV.
-02: fullwindow,reason0;205.323eHz,sigma35.908us,12319COMs/accepted,
  peak402,busfloor10841mV.
-03: fullwindow,reason0;205.415eHz,sigma35.863us,12325COMs,12324accepted,
  peak415,busfloor10638mV.

Each processed99008DMA scans,maxIRQ10us (prior direct-conversion27us),
queuepeak4/8,untouchedstack1872. No DMAFAULT or tracking failure. Reporter
validates capture CRC,timelines and finaloff for3/3. These are repeat10s
retention passes,not full independentqZC/current/reference parity. Queue
overflow and restart are host-tested,not yet injected on powered hardware;
longer holds and dropout/reentry with this exact FIFO remain next gates.

Restored normal no-feature image after cohort. Safe register reads with
RCCalready08040000; normal remains foreground ADC. Keep FIFO opt-in until
long-run/reentry evidence and sampling/current qualification are complete.
Finalguardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed.

## Entry 181 — 2026-09-13 — FIFO30s dropout/reentry3/3 near205–206eHz

Rebuilt current bench-dma-feedback,stackspan4096,preflightoff/flash/safe
readback/known UART-clock repair. Guardcheck18/18,recordcheck12/12 pass.
Same50→200ladder,6.5% startup/catch,4% powered,30000ms original budget,
trace0/stride19,explicit --dropout --reentry. No control/guard changes this
entry. Rebuilt experimental ELF SHA256:
8BAA245AB16EE439FB2829F7E9C610C89CB1D25046DE183907D55A3F0C7DB2DE.
All three attempts retained in dma_fifo40_reentry30_cohort.csv:

-01:205.423eHz,cycle sigma34.700us,34495resumedCOMs,277094resumedDMA scans,
  peak404,busfloor10351mV. Fresh12interval acquisition10780us. Final deadline
  margin149us,original34712744/final34712595us.
-02:205.774eHz,sigma34.930us,34553resumedCOMs,peak412,busfloor10435mV.
  Fresh acquisition10801us,deadline margin88us.
-03:206.044eHz,sigma34.245us,34600resumedCOMs,277098resumedDMA scans,
  peak406,busfloor10590mV. Fresh acquisition10864us,deadline margin150us.

All injected at~2s; first segment archived with Tracking8,then one fresh
re-entry completed remaining~27.987s. Reporter verifies reentry_verified1,
timelines,CRC windows,zero post-stop tail records and finaloff in3/3. Each
resumed DMA max10us,queuepeak4/8,stackspan4096/untouched1580. NoDMAFAULT.
Queue reset and ADC restoration now exercised in real powered re-entry;
injected FIFO overflow remains only host-tested. Viewed first campaign's
timeline PNG: stable resumed accepted-event rate across six bins; not
independent qZC or harmonic-lock proof. No mean supply-current claim.

Both builds pass;shell-sine hash unchanged. Restored default firmware after
cohort,safe register readback then known UART repair. Next qualify FIFO at
the already-tested~238eHz/4.5% operating point; keep current/independent-quality
and frozen-reference requirements open. This is not completion of parity.
Finalguardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed.

## Entry 182 — 2026-09-13 — FIFO30s recovery3/3 at237.5–237.6eHz

Same E181 experimental source/build,bench-dma-feedback. Safe preflight/flash,
RCCalready08040000,guardcheck18/18,recordcheck12/12,stack4096/2584/2308.
50→200startup ladder,6.5% startup/catch,4.5% powered duty,30000ms original
budget,trace0/stride19,explicit dropout/reentry. No limits/control changes.
All attempts retained in dma_fifo45_reentry30_cohort.csv:

-01:237.564eHz,cycle sigma27.023us,39894resumedCOMs,277108DMA scans,
  peak452,busfloor10638mV,acquisition9062us,deadline margin29us.
-02:237.478eHz,sigma26.905us,39881COMs,277114scans,peak454,busfloor10387mV,
  acquisition8997us,deadline margin66us.
-03:237.616eHz,sigma26.752us,39904COMs,277114scans,peak448,busfloor10471mV,
  acquisition9048us,deadline margin104us.

Each injected at~2s,archived first Tracking8 stop,reacquired12fresh intervals,
and completed remaining~27.989s. Reporter verifies3/3 reentry_verified1,
originaldeadline,CRC windows/timelines,zero post-stop tail records and finaloff.
All DMAmax10us,queuepeak4/8,stackuntouched1580. NoDMAFAULT. First two ended on
normal segment deadline2;third foreground completion0. This is successful
range/recovery qualification of this transport path,not full-goal completion.

Compared with E165 foreground-ADC cohort at same4.5%: cycle sigma~27us versus
~21us,so FIFO has a measured timing cost despite retention. Raw peaks448–454
versus343–349 are not mean-current evidence; different sampling coverage can
change peaks. No independent supply-current measurement was made.
Viewed first campaign timeline PNG: stable resumed accepted-event rate across
sixbins. Its independent disabled-coast period analysis with100us supplied
timestamp uncertainty has every early frequency lower bound>half controller
rate (lowest184.775Hz vs118.782Hz). Conditional on valid/no-missed edges and
timing assumption; not whole-runqZC or exact shutdown speed.

Both builds pass. Restored normal firmware after cohort,safe register reads
then known UART repair. Next use coherent scans for meaningful current
aggregation and hardware-test FIFO failure refusal. Same-wake calibration,
coverage,independent current anchor and matched reference quality remain open.
Finalguardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed.

## Entry 183 — 2026-09-13 — raw current sums captured; delivery race found/fixed

Opt-in DMA firmware adds foreground-owned segment-local five-channel64bit
sums,count,first/last acquisition stamps. A/B/C signed relative2048 for storage
ONLY,not zero calibration;bus/VREF remain unsigned-valued raw sums. Up to
6million scans (>600s at101us),strict101us timestamp continuity and ADC bounds.
No accumulation or output added to DMA ISR. Preparation clears stats before
fresh acquisition,including eventual refusal. Re-entry clears rather than
mixes epochs: output is final-segment-only,not a whole recovery-campaign sum.

Fixture-only ADCSTATS plus five S85 CRC records. Each11u16payload holds
channel,n32,first32,last32,sum64(two's complement);26bytes includingCRC.
drv_current_sums.py validates all rows,range,timestamps and feedback-count
agreement; sustained reporter validates optional records without breaking old
captures. No mA output or ratios-of-means presented as calibrated current.
New56byte state reduced experimental optionalIRQ prefix27→24rows (normal32).
25rows left4092stack after linker alignment,so it was rejected before flash;
24rows yields4124.110Rust/108Python tests pass,M0 library check,both builds pass.

Two attempts,same50→200ladder,6.5% startup,4% powered,10s bound,trace0/stride19:
- dma_sums40_01: Tracking8 at2733286us. Raw sums27058scans vsfeedback27057:
  stop between pre-conversion aggregation and feedback delivery. Decoder and
  sustained reporter correctly reject; raw/CSV/PNG/coast retained,not a pass.
  Diagnostic-only decode without feedback cross-check established the one-scan
  discrepancy; it does not qualify those sums.
- Fixed feedback_inner to return its exact recording decision. Foreground now
  aggregates only delivered feedback,after conversion; no enlarged critical
  section and no relaxed host count check. dma_sums40_02 completed10s near
  205.127eHz,normal segment deadline2 at10000017us. All5S85records validate,
  n99008,first137,last9999844us,signed sums343203/298086/850249,bus120159428,
  VREF149199130. Peak406,busfloor10853mV,DMAmax10us,queuepeak4,stackuntouched1892.
  dma_sums40_fixed_initial.csv describes ONLY corrected second attempt; a
  combined-prefix report still rejects the known invalid first capture.

Corrected experimental ELF SHA256:
01F81843290BA29B5669F83C64581DA53F00900D3F61BA95A0744C25E3B992FB.
Current sums are uncalibrated supporting data: same-wake offset,coverage,
within-scan VDDA/covariance treatment and independent supply anchor unresolved.
Boundary-race correction has one hardware pass,not forced exhaustive timing
coverage. FIFO overflow injection and corrected-build recovery still pending.
Restored normal release afterward,safe readback then known UART-clock repair.
Finalguardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed.

## Entry 184 — 2026-09-13 — sums recovery, real FIFO-full shutdown and same-boot reuse

First tested corrected E183 aggregation with same50→200ladder,6.5% startup,
4.5% powered,30s budget,--dropout --reentry,trace0/stride19. Safe preflight,
guardcheck18/18,recordcheck12/12,stack4124/2612/2336.
dma_sums45_reentry30_01 completed re-entry at236.680eHz,cycle sigma26.367us,
39746resumedCOMs. S85 verifies277111resumed scans,first139,last27988249us,
signed sums461801/647639/1936900,bus335683851,VREF416967220. First archive
has19809scans: final sums do NOT mix that earlier epoch. Reentry_verified1,
original deadline margin153us,peak452,busfloor10328mV,DMAmax10us,queuepeak4,
stackuntouched1600. No calibrated-current claim.

Added explicit bench-dma-stall feature depending on bench-dma-feedback. This
fault-injection build withholds foreground FIFO consumption after exactly20000
delivered scans,once per boot. Main/reference/host handling,DMA immediate
phase-current checks and all guardIRQs remain live. No blocking delay or
freshness extension. StallOnce states0armed/1holding/2spent; next preparation
releases hold and prevents reinjection. Fixture FIFOFAULT exposes state.
Normal/bench-dma-feedback builds do not enable injection. Host test verifies
threshold,latch and preparation/reuse policy.111Rust/108Python tests pass.

Built/flashed fault feature after prior verifiedoff; safe readback/known UART
repair,guardcheck18/18,recordcheck12/12,stack4116/2604/2328. Fault ELF SHA256:
EDEAC010ACC90F16B9D181E330469F33B9BBD556BA5745661FBBFDD74B22CD40.
Both fault/reuse runs use same ladder,4% powered,10s bound,no dropout/reentry:
- dma_fifo_stall_01: holds at20000 delivered scans; queue reaches8/8,
  DMAFAULT code9,AdcTimeout11 at2021016us. S85 count20000 agrees with feedback;
  peak405,busfloor10984mV,IRQmax22us includes fault shutdown,stackuntouched1892.
  This is the EXPECTED fault-test pass,not a completed powered window.
- Without reset/reflash, dma_fifo_reuse_01 completes10s at205.131eHz,
  FIFOFAULT state2,n99008validated sum/feedback scans,DMAmax10us,queuepeak4,
  peak397,busfloor10841mV,stackuntouched1892. No accidental reinjection or
  stale-queue reuse. This is explicit new operator-fixture start,not automatic
  restart from the ADC fault.

drv_fifo_fault_check.py verifies both retained captures (CRC,faultcode,queue
depth,counts,completed reuse,finaloff); same-boot provenance is the above bench
sequence,not inferred from UART text alone. Raw/CSV/PNG/coast files retained.
Default release restored after tests,safe readback then known UART-clock repair.
The coherent-delivery/failure/restart substrate is now measured; meaningful
current still needs same-wake zero,coverage and voltage-normalization evidence
plus an independent supply anchor. Full reference parity remains incomplete.
Finalguardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/
CCRs0,nFAULT1,COM41closed.

## Entry 185 — 2026-09-13 — calibration epoch audit, not another transport test

Read current source handoff and resume paths: both explicitly cycle ENABLE
before acquiring a flying seed. A stationary pre-start CSA zero therefore
cannot automatically calibrate either powered segment. Experimental DMA start
requires powered ownership; existing idle zero_check is a different acquisition
mode. A zero during moving coast requires proof against diode current, not
merely all commanded gates low. Preserve shutdown and fresh-seed timing.

Revalidated dma_sums40_02,dma_fifo_reuse_01,dma_sums45_reentry30_01 via current
sustained reporter and raw-sums decoder. All three completed their powered
windows with valid counts/CRC/finaloff. Centered summed raw means respectively
15.064823,8.317075,10.993212 counts. The two~205eHz captures differ6.747748counts;
not calibrated current, nor isolated evidence of a particular cause.
Full per-phase values and source constraints added to CURRENT_MEASUREMENT_PLAN.

Requested actual PSU idle readings/display resolution and operator readiness
for the existing finite known-point supply-anchor hold. No reply at this entry;
no new motor run,flash or hardware-state change. Need that independent current
fact rather than treating more uncalibrated sums as current parity. Other full
goal gaps (independent whole-run quality and matched frozen reference) remain.

## Entry 186 — 2026-09-13 — portable delivery contract and reference revalidation

Updated PORTABLE_WINS with the previously missing E180–184 DMA lease/FIFO,
acquisition-age guard and exact-delivery aggregation invariants. Separates
portable policy from G071 trigger/priority/register sequencing,records10us
normal/22us fault IRQ observations,stack constraints and cycle-sigma cost,
and excludes bench-dma-stall from production behavior. BEMF_PARITY_PLAN now
opens with current measured status rather than E165. No sibling port claimed.

Re-ran freeze_minz_reference --check-source: all128selected files still match,
archive SHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
Observer replay111Rust tests pass (89unit+1recorded+21sequence),108Python
tests pass,and thumbv6m library check passes. Existing incremental-cache
AccessDenied notes do not prevent test execution. shell-sine SHA remains
B6BBA6604F9534C45B2581253FC5EBB49AE1F24DA3E48891AE3D1C161D54206E.

This completes documentation of these measured portable candidates,not full
reference parity. Frozen-source identity does not establish historical capture
firmware identity. Actual supply anchor and independent whole-run quality
remain missing. No motor command,flash or hardware change this entry.

## Entry 187 — 2026-09-13 — PSU baseline and startup-current stop

Operator confirms idle PSU11.7V,0.0007A (0.7mA),board powered,motor stopped.
Post-sleep COM41 recovered without flash; guardcheck18/18,recordcheck12/12,
stack4108/2524/2248 and all outputs off verified.

Announced and executed intended60s observed hold:6.5% startup/catch,
50->200eHz stepped ladder,4.5% intended BEMF duty,trace0,stride19,
no dropout/reentry. psu_anchor45_60s_01 stopped in startup at3264473us,
target60eHz,current reason4. No BEMF handoff or sustained hold reached.
Fault-sample logicalA/B/C raw1801/2018/3251;C exceeds unchanged3248 limit
by3counts. C timing bracket33us crosses PWM wrap; this is not aperture
measurement or proof of a false trip. No automatic electrical-fault retry.
Bus raw-capture minimum11198.1mV,no nFAULT samples,waveform maxgap100us,
ISRmax28us. Host validator confirms startup_stopped and post-capture finaloff.
Raw/CSV/PNG/coast retained;allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed.

Operator observed approximately0.211A on PSU during the brief run. This is
an actual supply-display observation,not the800mA limit setting. Display
integration time,resolution,CV/CC and simultaneous running voltage unreported.
Do not call211mA a settled238eHz mean or compare it directly with a fast
low-side phase-current peak. The intended supply-anchor hold remains unfinished.

## Entry 188 — 2026-09-13 — 60s BEMF hold at237.64eHz; operator observes70mA

Reviewed E187 fault at the first50->60eHz transition,6.5% hold duty. One
controlled change: reduce startup target/ramp-end duty to6.2%,retain6.5%
initial100eHz catch,the same50->200ladder and4.5% powered BEMF duty. No
firmware edit/flash,threshold increase or automatic retry. Existing guards,
finite60s window,trace0,stride19,no dropout/reentry. Announced run to operator.

psu_anchor45_62start_01 completes60000013us observation,237.642508eHz from
whole-stream cycle moments,cycle sigma26.252182us,85551COMs/85550accepted
events,order_bad0,excluded0. Host sustained validator passes CRC,event windows,
timeline and finaloff; one completed attempt,not independent whole-runqZC.
302973powered feedback scans,peak_abs_raw360,busfloor10829mV,TIM6max4us,
commitmax20us,stack4108/2524/1860. Normal completion reason0 (foreground end),
allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed. Raw/CSV/PNG/coast and
psu_anchor45_62start_initial.csv retained; E187 failure remains separately
retained,not omitted from history. This is1/1 at revised startup,not proven
causal correction or a repeatability cohort.

During this live hold operator reports 'runs about0.070amp': approximately
70mA actual PSU display current,versus0.7mA reported idle. This establishes
an external operating-current observation at the measured speed,not a sampled
60s time average or shunt calibration. Simultaneous running voltage,CV/CC,
display resolution and variation remain unreported. Do not silently use idle
11.7V as measured running voltage. Arithmetic difference69.3mA is only an
approximate idle-subtracted supply reading,not pure motor current/loss.
Full matched-reference/current calibration/independent-quality parity remains
open; this result resolves the absence of any sustained external current fact.

## Entry 189 — 2026-09-13 — small duty step: two30s holds near243eHz

E188 sustained~70mA supply observation plus unchanged raw-current/bus/tracking
guards supported one incremental powered-duty test4.5->4.6%,not a ceiling
increase. Same normal firmware,6.5% catch,6.2% startup target,50->200ladder,
trace0,stride19,no dropout/reentry. Thirty-second finite windows. After first
pass,repeated the same point once instead of further raising duty.

| Capture | eHz | cycle sigma us | accepted events | ADC scans | peak_abs_raw | bus floor mV |
|---|---:|---:|---:|---:|---:|---:|
| range46_62start_01 |243.050508|32.698676|43748|152079|356|10817|
| range46_62start_02 |243.253341|33.093834|43784|152007|386|10865|

Both complete30s with normal segment-deadline reason2,guardISRmax16us,
commitmax20us,stack4108/2524/1860. Raw validators pass CRC,event windows,
timeline and finaloff. range46_62start_pair.csv retains2/2attempts. Earlier
E187 startup failure remains separate evidence,not erased by these passes.
The revised6.2% startup now has three successful attempts across E188–189,
not a randomized demonstration that duty alone caused the improvement.

Both independent early-coast period analyses (--uncertainty-us100) exclude
half the controller rate under valid/no-missed-edge and timing assumptions;
minimum frequency lower bound184.467810Hz. Not exact shutdown speed or
whole-runqZC. Cycle sigma~33us exceeds E188's26.25us at237.64eHz; do not
equate higher speed with higher quality. No new operator supply reading at
4.6%; E188's70mA is NOT assigned to this point. Recovery at243eHz untested.
All attempts end with allgates/MOE/ENABLE/CCRs0,nFLT1;COM41closed. No flash,
guard relaxation or firmware edits. Highest repeated hold point now~243eHz,
not a demonstrated motor speed limit or full matched-reference parity.

## Entry 190 — 2026-09-13 — first243eHz recovery; startup/entry failures retained

Three intended30s4.6% recovery attempts,normal image,no flash/guard changes,
same50->200ladder,trace0,stride19,--dropout --reentry. Explicitly bounded one
injectedTracking recovery; electrical faults never auto-retry.

- recovery46_62start_01:6.5% catch/6.2% target; startup current abort at1637741us,
  before BEMF acquisition. C raw3293>3248; A/B1965/1923. C bracket35us across
  PWM wrap is not proof of an artifact. Thus E189's successful starts did not
  establish that lowering only ramp endpoint eliminated startup trips.
- Controlled change to6.2% catch AND target,retaining timings and all guards.
  recovery46_all62_01 passes electrical startup but refuses passive seed:
  FLYTooSlow5,4intervals,4494us. CRC-valid F85 words:
  (4,6950,8972,1,8,0,8940,9,0,8972,8,0,8876).
  Last step4 edge6950,decision8972 =>1011us wall gap; expected phaseA has no
  pending candidate. Existing candidate-only grace does not apply. No BEMF
  power or recovery occurred; no seed criterion relaxed.
- Same6.2/6.2settings repeated: recovery46_all62_02 completes. Injected loss
  at2000005us,first Tracking8 stop2000905us; first archive retains2911events.
  Fresh12interval seed in8731us,7full-cycle checks8218..8508half-us ticks,
  interval1393. Re-entry then completes27988717us near243.314678eHz,
  cycle sigma26.807023us,40861COMs/40860events,141615feedback scans,
  peak_abs_raw355,busfloor10793mV. Original campaign deadline34711578us,
  actual end34711446us (132us margin). reentry_verified1,timeline valid,
  stack4108/2524/1520. This is one successful recovery,not a repeat cohort.

recovery46_campaign_initial.csv includes all three attempts:1/3completed,
not a homogeneous startup-profile cohort. Raw/CSV/PNG/coast retained for each.
Every finaloff validates allgates/MOE/ENABLE/CCRs0,nFLT1;COM41closed.
No new operator supply observation at4.6%. Highest repeated hold remains~243eHz;
recovery there now demonstrated once. Startup peak and passive entry reliability
remain distinct unresolved issues; no claim of calibrated-current/qZC parity.

## Entry 191 — 2026-09-13 — two more243eHz recoveries, unchanged settings

Declared two additional30s attempts at E190's6.2% catch AND startup target,
4.6% powered duty,same50->200ladder,trace0,stride19,injected loss at2s plus
one bounded re-entry. No firmware/guard change or additional duty step.

| Capture | resumed eHz | cycle sigma us | reacquire us | resumed ADC scans | peak_abs_raw | bus floor mV | deadline margin us |
|---|---:|---:|---:|---:|---:|---:|---:|
| recovery46_all62_03 |243.098734|27.219692|8794|142120|360|10877|163|
| recovery46_all62_04 |243.272083|27.177793|8732|142258|347|10865|90|

Both complete original30s budgets with~27.989s resumed operation,qualified
12interval/7cycle recovery seeds,valid first archives,timelines and finaloff.
reentry_verified1 for each,stack4108/2524/1520. Allgates/MOE/ENABLE/CCRs0,
nFLT1,COM41closed. No new PSU-display observation collected.

recovery46_all62_cohort.csv includes ALL four attempts at these settings:
one initial passive-acquisition refusal and three successful recovery windows
(E190's02 plus these03/04). Thus3/4entry-to-completion and3/3powered recoveries,
not4/4nor a population reliability estimate. E190's earlier6.5% catch electrical
abort remains separately retained. No new electrical trip in the two repeats.
Viewed04timeline plot: steady accepted-event rate across six resumed bins,
no visible late-run growth in gap extrema; initial resumed maximum gap higher
than later bins. Accepted timing is not independentqZC or whole-run rotor proof.

This completes the planned two-repeat extension. Highest repeated recovery
point~243eHz now has three passes; startup/passive-entry reliability and full
independent-current/quality/frozen-reference parity remain open.

## Entry 192 — 2026-09-13 — recorded entry refusal regression; no blind grace

Inspected current acquire_inner round-robin sampling and poll_filtered against
E190 recovery46_all62_01's saved F85. Last step4 edge6950half-us ticks;
expected phaseA sampled low at8940,decision8972 on phaseB. Thus A was low
at995us after the preceding edge; refusal at1011us,physical interval cap1000us.
No expected candidate exists. A crossing in the final5us before the limit is
possible but unobserved; the snapshot cannot distinguish it from a truly late
or absent crossing. This is not evidence authorizing longer blind grace or a
claimed acquisition scheduler fix. Other-phase continuity also constrains any
future targeted mux revisits; do not simply stop surveying the other inputs.

Added test recorded_e190_missing_candidate_must_not_get_blind_grace in
flying_acquire.rs. Synthetic valid history reconstructs the KNOWN final
step/interval count; saved filter states produce TooSlow with zero grace waits.
A subsequently supplied hypothetical backdated edge cannot revive the latched
refusal. Test explicitly does not claim replay of unrecorded earlier waveform.
Runtime policy unchanged; test-only code,no firmware flash or motor run.
Reference selected128files still match archive;112Rust tests pass
(90unit+1recorded+21sequence),existing incremental-cache notes only.

Current result remains three243eHz recovered windows out of four attempts at
6.2% catch/target. This regression protects evidence validity; it does not
improve startup reliability or supply the independent whole-run quality and
matched-reference benchmarks still needed for the full goal.

## Entry 193 — 2026-09-13 — historical benchmark replay without live minz

Added --verify-archive to scripts/minz_historical_baseline.py: validates
archive membership/payload hashes,exact three-capture set,parser identity
against pinned frozen-source archive,and recomputes saved metrics. No live
minz read or archive modification. Historical archive SHA remains
94df42404cd3e1735039fcba647bea93d795e6e11cd8823d446c921219355138.

All three historical summaries reproduce: healthy230.019852eHz FALCON sample,
5543windows,99.981959%qZC,one missing window,nominal time-weighted44.849724mA;
257.476335eHz sample has one sequence discontinuity despite100%qZC among
retained windows;202.943797eHz sample only20.547667%qZC,not a healthy baseline.
These measurements are reference context,not matched frozen-AM32 provenance.
FALCON single-window sigma cannot be compared directly with binz cycle sigma;
nominal DC-link conversion cannot calibrate DRV low-side shunts or identify
power efficiency against operator's70mA reading without matched conditions.

110Python tests pass,including real retained-archive replay and rejection of
corrupt payload,altered metrics and missing capture metrics. README documents
the new repeatable command. No motor run,firmware flash or sibling edits.
External matched-reference identity/measurement remains required for full
parity; this tool verifies the available evidence without overstating it.

## Entry 194 — revised goal; separate compile-time running envelope

Operator supersedes prior objective: duty-led experiments up to hard30% PWM,
no fixed250eHz goal ceiling,staged safety qualification before expansion,
archived minz data only/no reruns. Updated AGENTS,acceptance audit and parity
plan to cancel the obsolete reference-hardware blocker. Current attachment:
90c16e2b-6f02-47e6-b689-ddc0f9a0895d/pasted-text-1.txt.

Source audit: powered full-cycle lower bound4000us enforces250eHz separately
from startup seed qualification and shell frequency commands. Refactored
powered_guard into RunGuard<const MIN_CYCLE_US,const MIN_EVENT_US>,with
existing Guard alias fixed at4000/333. No runtime selector or added fields.
Slow-cycle6ms and missing-event1ms bounds remain independent and unchanged.
Invalid profile combinations refuse construction. Startup policy untouched.

Tests demonstrate synthetic3333/277profile accepts556us-sector timing while
default guard still trips full-cycle overspeed; profile still trips overcurrent,
missing acceptance and3330us-cycle overspeed. Equal profile instance sizes.
This is a qualification mechanism,NOT300eHz hardware evidence or approval
to select that profile blindly.114Rust tests pass (92unit+1recorded+21sequence),
M0 library check and default release build pass. No firmware flash/motor run;
live firmware and safety envelope remain unchanged. Next steps must qualify
driven acquisition/handoff and a deliberately selected faster profile with
measured timing/current,not merely raise the old constants or command cap.

## Entry 195 — driven acquisition guard policy; TIM6 ownership conflict identified

Audited observation.rs,wave_timer.rs and reference adapter: old microscope
uses TIM6 for100us waveform steps while powered_timer requires TIM6 as the
independent guard. Cannot combine those owners. Old observation also uses
foreground current checks and a slow diagnostic detector; it is not the
new driven-acquisition/handoff path. Preserve TIM6 safety ownership; choose
and qualify a separate waveform scheduler before any new hardware experiment.

Added driven_guard.rs pure policy,host/M0 library only,no hardware caller.
Bounded<=20ms diagnostic window,independent tick gap200us,feedback age1ms,
shared existing phase/bus/VREF validator,driver/host stops and latched faults.
Command order is checked separately from real BEMF acceptance. authorize
does not feed tick timestamp or fabricate accepted events; no handoff or
closed-loop authority is granted by this policy. Caller must serialize gate
writes with stops and retain original campaign deadline outside local window.

Tests cover missing independent tick despite continued commands,absolute
deadline/clock wrap,post-stop refusal,stale/reordered feedback,current/bus/
driver/host faults and invalid/skipped sectors.118Rust tests pass
(96unit+1recorded+21sequence),M0 library check and default release build pass.
Existing powered guard now calls the same electrical validator; limits unchanged.
No firmware flash or motor run. Integration,disabled authority checks,actual
guard/scheduler timing and driven sensing qualification remain to be done.

## Entry 196 — disabled TIM3 scheduler/TIM6 guard hardware test

Optional bench-driven-entry adds idle drivencheck with NO gate-enable path.
TIM3 emits833us command steps (~200eHz),TIM6 checks every100us. Synthetic
feedback explicitly labeled; physical outputs/ENABLE must stay off. Both IRQs
and foreground state access serialized; guard priority0,scheduler0x80.
Window20ms,foreground22ms backstop. Commands check latched policy but never
call sixstep_write. Post-stop authorization must refuse.

Initial wrong IRQ name TIM3_TIM4 corrected from installed G071 PAC (TIM3=16)
before flash. Flash overflow2656bytes then resolved by excluding legacy
sensezero/pwmcheck from optional build unless respective diagnostic features
requested. Optional IRQ trace24rows,normal32. Build stack span4284. ELF SHA:
DD6964D8424951BFEACDDE0B27EEA759872172100FDAD033F696D44A06F3636E.

Verified old outputs off,flashed optional image,safe register readback.
drv_driven_check.py captures three trials in captures/drivencheck_01.txt:
each reason2 at20005us,200guard ticks,24commands,guardISRmax4us,scheduler
ISRmax9us,poststop_refused1,disabled1. Stack4284/2772/2656.111Python tests
pass including raw trial validation and malformed-result refusal.
Disabled timer coexistence only; no powered waveform,actual ADC freshness,
COMP coexistence,tick failure injection or handoff qualification yet.

Restored normal release. Reset initially had RCC08000000 with PD1/MOE/CCRs
safe; known UART clock correction to08040000 did not restore responses on
two checks. Another reset restored UART without reflash. Finalguardcheck18/18,
recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFLT1,
COM41closed. No motor energized during this work.

## Entry 197 — real ADC with disabled scheduler; startup update interrupt fixed (2026-09-13)

Optional bench-driven-entry now has drivenadc: wake CSA with ENABLE, keep
all gates/MOE off, sample existing logical A/B/C current,bus,VREF while TIM3
schedules833us commands and TIM6 guards100us. ADC conversion outside critical
sections; feedback conservatively aged from scan START. Initial scan also
aged across setup; new policy test rejects stale initialization without
resetting a latched fault. No physical sector writes or BEMF claims.

Prior UART-silent preflights drivenadc_01/02 never issued the test. Console
recovered after safe register checks and known RCC USART3 clock correction;
the recurring console startup root cause is not established here.
First actual real-ADC capture drivenadc_03 failed the predeclared timing
contract: all3 trials had25commands/201ticks in20003us (expected23..24commands),
despite199scans,maxage106us and safeoff. Retained as failure,not relaxed away.
Initial ELF SHA FDBBD7325A67D110FC71781A15E5044E1270BDD18D4C6A00FE0F803B9D38A711.

Local ref/stm32g0xx-hal/src/timer/mod.rs documents URS suppressing software-UG
interrupts. Set URS before UG and retain through timer start; explicitly clear
TIM6 DIER at setup too. Corrected capture drivenadc_04:3/3 reason2 stops at
20005us,200ticks,24commands,199scans each,maxage102us,guardISRmax4..5us,
schedulermax9us. Peak raw deviations63/72/57,minimum bus11569/11486/11140mV.
These are gates-disabled CSA readings,not motor-current calibration.
Synthetic regression drivencheck_02 also3/3passes,stop20009..20010us,
200ticks/24commands,ISRmax5/10us. Poststop authorization refused each trial.
Optional stack4268/2748/2528. Corrected ELF SHA
8A2201C2F59908E6DAF0A8FA6A0F79A36A515A02903BDFC7D45004CB96ECD562.

112Python tests and119Rust tests pass,including capture rejection for extra
startup command and stale/missing real feedback. Default release restored.
After safe readback and USART3 clock correction,guardcheck18/18,
recordcheck12/12,stack4108/2524/2248; all gates/MOE/ENABLE/CCRs0,nFLT1.
COM41closed. No motor spin this entry. Next integration must add qualified
driven COMP observation and eventual guarded handoff; disabled timing results
do not establish those or justify a higher running-speed guard yet.

## Entry 198 — driven comparator sample provenance wrapper (2026-09-13)

Inspected existing observation,detector,sixstep,comp_input and flying_acquire.
Reuse detector's minz polarity and bemf_count_step,not a new control law.
New driven_observer.rs is host/M0-only,not connected to gates or firmware.
Requires a physical command epoch increment on every commutation; checking
step alone can accept a stale sample after a full six-step cycle (ABA).
Before/after epochs must match the current sector; read bracket<=2us,
post-command blank200us and sample gap<=100us,matching existing diagnostic
contracts. Reject counters expose missing sector,epoch,blank,bracket and gap.
Invalid data clears qualification; an already emitted epoch remains latched.

Existing two-opposite/two-expected detector remains unchanged. Candidates
carry first observed expected-level timestamp plus confirmation timestamp;
command timestamps never substitute for measured crossings. Counts separate
commands,candidates and completed sectors without a candidate. No automatic
Acquire seed or handoff is granted; these stricter diagnostic requirements
must not be confused with production AM32's level-counter arming semantics.

Five tests cover every phase,command-only no-event sequence,stale epoch after
six sectors,mixed read epochs,blank/read bracket,gaps,post-rejection duplicate
prevention,reordered commands and timestamp/epoch wrap.124Rust tests pass
(102unit+1recorded+21sequence); M0 no_std library check passes.
No flash or motor run; installed normal firmware untouched since E197 finaloff.
Next caller must serialize physical command/mux and sample snapshots,measure
actual cadence,and feed only qualified observations to seed policy. Important:
E197's102us ADC scan cannot by itself satisfy this100us COMP sample-gap limit;
do not bolt a foreground sample after each scan and claim continuity. A bounded
interrupt sampling schedule or interleaved ADC acquisition needs qualification.

## Entry 199 — real comparator/ADC coexistence on 50us timer (2026-09-13)

Optional drivencomp runs the E198 observer inside serialized TIM6 at50us,
alongside real foreground ADC and synthetic833us TIM3 sectors. Real COMP2
mux changes match floating phases; raw level inverted into HAL polarity.
The critical section brackets both comparator read and command epoch; blank
starts after mux change. No sixstep_write or PWM authority. Restore saved
COMP CSR with EXTI masked after stopping. Existing drivenadc/drivencheck
retain100us guard cadence. These are diagnostic modes,not entry drivers.

First optional link overflow1440bytes. Excluded phasecheck/sixcheck only on
bench-driven-entry unless bench-adc-probes explicitly selected; default
firmware keeps them. No safety or motor feedback removed. Optional stack4172.
ELF SHA24A1B9DF1B5332A980864172B21403F13ACB183E9BA6D982E5DC3E9DD31B898A.

Predeclared gates:3trials,20ms deadline,398..401guard ticks,23..24commands,
COMP397..400reads,maxgap100us,readbracket2us,all6steps,zero epoch/bracket/gap
rejections,ADCage<=1000us,electrical checks intact,poststoprefused/finaloff.
captures/drivencomp_01.txt passes scheduling contract3/3:stop20010..20011us,
400ticks/24commands,399reads,maxgap62..63us,bracket1us,stepsmask63,
98..99blank rejections and zero other rejections. Guard/sampleISRmax11us,
sectorISRmax13..14us. ADC172..173scans,maxage133us,rawpeaks48/37/41,
minimum bus11522/11510/11283mV. Stack4172/2764/2496.

IMPORTANT negative control: gates-OFF comparator produces5/2/4candidates,
19/22/20completed sectors without candidate. Therefore candidate presence
is demonstrably NOT proof of motor rotation or qualified BEMF. This capture
does not isolate analog noise from other idle comparator activity. Do not
grant handoff from candidate counts; require measured sequence/cycle/age
qualification under drive and retained raw evidence. No motor run this entry.

113Python tests pass,including rejecting gap/bracket/epoch/missing-step and
incorrect synthetic-sector provenance;124Rust tests pass. Default release
restored. Safe readback then known USART3 RCC correction;guardcheck18/18,
recordcheck12/12,stack4108/2524/2248. All gates/MOE/ENABLE/CCRs0,nFLT1,
COM41closed. Physical gate integration,driven edge-sequence qualification and
handoff are still required; this closes the timer/ADC sampling-cadence gap only.

## Entry 200 — duty-led running beyond250eHz (2026-09-13)

Separate from pending driven-acquisition work,exercise user-authorized staged
duty envelope. Optional bench-range300 selects previously synthetic-tested
RunGuard<3333,277> in powered_timer ONLY. Approximately300eHz upper running
guard,unchanged6ms slow cycle,1ms accepted/feedback age,200us guard tick gap,
raw phase848..3248,bus8400mV floor,nFAULT,host abort and finite deadlines.
No change to startup acquisition,AM32 logic or hard10% firmware PWM ceiling.
No30% target implied. RUNLIMIT dump explicitly advertises experimental profile;
normal build stays4000/333. Host report preserves/rejects unknown profiles.

Protocol: same6.2% catch/startup,50Hz then10Hz rungs to200Hz,existing passive
12interval seed; first4.6% for10s,then4.8%,then5.0% only after prior run's
current/bus/timing/tracking/finaloff checks. PSU setting remains operator800mA;
no new operator current/voltage reading obtained this entry. No inference of
calibrated amps from raw pulses. Optional ELF SHA
B43970571495CA7131AF50F29782ECF39E0B499569292858748130027D207334.
Preflightoff,guardcheck18/18,recordcheck12/12; stack4108/2524/2248.

All three10s holds complete,reason2 deadline,order_bad0,desync0,no veto.
captures/range300_46_01:242.887eHz,cycle sigma42.529us,14572accepted,
50771ADC scans,peak386,busminimum10781mV.
range300_48_01:255.104eHz,sigma43.806us,15305accepted,51819scans,
peak378,bus10948mV.
range300_50_01:267.201eHz,sigma47.030us,16030accepted,51530scans,
peak389,bus10937mV. EachguardISRmax16us,commitmax20us,COMPmax60us,
COMmax52us,untouchedstack1860. Statistics include entry acceleration,not
duration-matched steady-state sigma versus older30s runs.

Viewed range300_48_01_timeline.png: accepted-event rate flat after entry,
retained later-bin gaps roughly530..760us. This is controller event timing,
NOT independent qZC. Early coast bounds at5.0% with100us supplied uncertainty
all exceed half controller rate; conditional no-missed-edge evidence only.
Legacy fixture's naive coast frequencies/extrapolations are not shutdown-speed
or lock proof. captures/range300_initial_campaign.csv retains all3attempts.
114Python/124Rust tests pass. No averaging/current-limit weakening.

## Entry 201 — higher-speed recovery safely refuses acquisition ceiling

Same range300 ELF,5.0% duty,original30s budget,one injected tracking loss at
2s with one reentry attempt. captures/range300_50_recovery_01 retains failure.
Tracking8 stops at2001005us;first segment3196COMs/3195accepted,10341scans,
peak406,busminimum11116mV,guardISRmax17us. First-segment archive retained.
Fresh acquisition refuses result14/CycleTooFast after4547us and5intervals:
first full cycle7578half-us ticks (=3789us,~263.922eHz),below unchanged8000
minimum. REENTRYresult5,no powered restart. This is an acquisition policy
limit,not observed failure to sustain the higher running setting. Do not
relax sequence/age checks or call recovery successful.

captures/range300_campaign_01.csv accounts for all4attempts:3complete holds,
1injected-loss campaign ending in safe recovery refusal. Next coordinate
acquisition,Seed::handoff,core_bench seed checks and fixture verification with
an explicit experimentally bounded profile. Faster running alone cannot
qualify faster seed admission. Driven acquisition and independent current/
quality requirements remain open. Normal release rebuilt for restoration.
Restoration complete: safe register readback and known USART3 clock correction,
guardcheck18/18,recordcheck12/12,stack4108/2524/2248;allgates/MOE/ENABLE/CCRs0,
nFLT1,COM41closed. shell-sine SHA unchanged from successful reference.

## Entry 202 — faster seeds accepted; handoff setup budget now limiting (2026-09-13)

Acquisition is now const-profiled: default8000/666half-us cycle/individual
minima unchanged; bench-range300 uses6666/554 and seed-average minimum1111
instead of1333. Powered handoff consumes the same selected minimum. Retain
12ordered intervals,seven full-cycle checks,12000cycle maximum,2000individual
maximum,40tick dwell,200tick phase-visit continuity,20ms acquisition window,
candidate-only grace,32us remaining arm margin and16us actual arm budget.
Standalone no-output coast_flying still has its separate250Hz entry guard;
only powered recovery paths are exercised above250 here.

Host report recognizes faster recovery thresholds only with explicit matching
experimental RUNLIMIT and cycle metadata. Older run-only experimental captures
with8000/666 remain interpreted under their original acquisition limits.
New Rust tests use synthetic equal sectors of1263ticks (7578cycle inspired by
E201,NOT reconstruction of its unrecorded edge stream),require full sequence,
check wrap,fresh/late handoff,overspeed,too-short sector,wrong order and missing
edge refusal.125Rust tests pass in default and experimental profiles;M0check
passes.115Python tests include refusing to certify the new late handoff.

Initial coordinated ELF SHA
C9384BE44B191C466EE90917A6851B63AE91ADFDF66A406521F9E4147807499E.
Preflightoff/guardcheck18/18/recordcheck12/12,stack4108/2524/2248.
Same6.2% startup,5.0% powered,30s originalbudget,2s injected loss,one retry:
range300_seed50_01: initial powered segment3195accepted,guard Tracking8;
fresh recovery12intervals,average1267ticks,8388us acquisition,seven cycles
7456..7628ticks,phasegap138ticks. Handoff refuses COASTREFstop8,zero resumed
events/commits/feedback. SEEDLAT entry144/reset172/feedback200/guard254/
reference262half-us ticks. Sequence passed; remaining arm margin did not.

Tested moving diagnostic ADC occupancy clearing to prepare,not post-seed.
ELF723B103DF937F80B2A79CC495E12B654B4BFA1CF5926AF94D1616769F62FAA79.
range300_seed50_02 again acquired12intervals,average1267,8322us,seven cycles
7578..7718ticks,phasegap140. EXACT same SEEDLAT144/172/200/254/262;handoff
again refused with zero resumed events. Optimization has no measured benefit
and was reverted,not retained as a claimed fix. Both attempts retained in
range300_seed50_first_pair.csv,0/2 recovered powered windows. REENTRYresult7
means path entered only. No current/age/arm-margin safeguards weakened.

Next is staged guard/timer preparation before final seed qualification,with
explicit ownership and latched-stop checks; not refreshing the edge timestamp
or reducing required arm time. Driven-under-power seed acquisition also remains
unfinished. Normal firmware rebuilt for restoration after this experiment.
Normal firmware restored;safe register checks and USART3 RCC correction,
guardcheck18/18,recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,
nFLT1,COM41closed. No pending motor command or fixture process remains.

## Entry 203 — measured latency reductions yield267eHz recovery (2026-09-13)

E202 indicated a larger staged-setup refactor, but inspected smaller concrete
late work first. No change to dwell,edge timestamp,32us remaining arm minimum,
16us actual arm maximum,sequence/cycle/current/voltage/age guards.

1. Final confirming visit now waits until20us after the comparator sample,
not20us after filter/hint bookkeeping. EdgeFilter independently enforces40ticks.
range300_dwell50_01 still refuses late:SEEDLAT138/166/194/248/256,CORESEEDage276,
average1270ticks. Saves3us vsE202,not enough. ELF SHA
6A458FF7D9E16B204E6DF78A50B19A42E216A1FBFA2FB3A0ED9FAD537D788640.

2. Experimental bench-range300 precomputes baseline bus voltage during ADC
acquisition after every raw update once all5channels exist. Cached value reset
on acquisition entry; original per-channel timestamps and200us conservative
setup allowance retained;calibration checked against factory value. No fresh
timestamp is assigned to old data. Disabled-only reacquisition restores cache
with its old baseline; awake reacquisition retains the fresh pair. Normal
build keeps on-demand conversion. Four-byte cache plus alignment costs8RAM.
range300_cached50_01 still refuses:SEEDLAT138/166/176/228/236,CORESEEDage256,
average1263ticks. Another10us saved;maximumphasegap158ticks (<200). ELF SHA
5EF7A96DB38FEE814B382C41609805DF2B2E65D0B1C60CAD982DC5E615C56B1A.

3. Successful awake acquire return keeps direct bridge_clear but avoids a
second full scheduler shutdown: acquire_inner unconditionally gates_off at
entry and never activates a scheduler/output while sensing. Full gates_off
and ENABLEoff remain on ALL refusal/fault paths. COMP interrupts still masked
and saved mux restored; no new authority granted. Final ELF SHA
DD0F31F9EC4C217DB053DE0D32BF4A8BD2E146358753DDB46BE81BCACFC0E8C2.

Same6.2%catch/startup,50-to200Hz10Hzrungs,5.0%powered,30soriginalbudget,
2sinjectedloss,onefresh-seed retry. Final-build cohort:
range300_fastreturn50_01:recovery12intervals/seven cycles7504..7672ticks,
8241us acquisition,maxphasegap160ticks;SEEDLAT128/158/166/218/226,
CORESEEDage248,remainingARR69ticks,actualarm11us. Resumed~27.989s at267.637eHz,
cyclesigma25.197us,144429ADCscans,peak392,busminimum10913mV.
range300_fastreturn50_02:cycles7522..7678,7725us,maxgap160;identicalSEEDLAT,
age248,ARR68ticks,arm12us. Resumed~27.9895s at267.535eHz,sigma24.985us,
144054scans,peak379,bus10913mV. Both no desync/orderfault/veto,valid archived
first segments and timelines,originaldeadline margins142/145us,stack1512
untouched/span4100. Reporter reentry_verified1 on BOTH,not merely result7.
range300_fastreturn50_pair.csv retains2/2;earlier two intermediate refusals
remain separately retained and are not hidden in this same-build cohort.

125Rust tests and116Python tests pass;new host regression requires matching
faster profile,seven cycles and originaldeadline for recovery certification.
These results establish injected-loss recovery at5%,not arbitrary faults,
independentqZC or calibrated averagecurrent. Arm remainder34..34.5us versus
32us minimum is still tight. Do not blindly increase recovery speed; further
headroom and actual under-drive qualification/handoff remain goal work.
Normal release restored,guardcheck18/18,recordcheck12/12,stack4108/2524/2248,
allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed. Default speed profile unchanged.

## Entry 204 — shared shutdown revokes new scheduler ownership (2026-09-13)

Before adding physical writes to the driven scheduler,inspection found shared
gates_off stopped only old owners; TIM3/new ACTIVE was not revoked. This was
not a live motor failure (new scheduler had no gate authority),but would be
unsafe to carry into powered integration. Added optional driven_probe::cancel
to shared gates_off. Serialized with both IRQs; latches HostAbort,stops/masks
TIM3/TIM6,clears ACTIVE and ENABLE,never recursively calls gates_off.

New idle-only drivenstop invokes actual shared gates_off at>=5ms during real
ADC/COMP disabled test,then invokes both timer callbacks after stop. Require
unchanged command count,timers/owner/outputs off,poststop authorization refusal.
captures/drivenstop_01.txt:3/3 stop at5074us,101ticks,6commands,43scans,
ADCmaxage127..128us,COMPmaxgap63us/readbracket1us;both late callbacks refused.
Same-boot fresh20ms runs drivencomp_afterstop_01:3/3pass,400ticks/24commands,
399reads,168scans,ADCage131us,COMPgap<=63us,ISRmax11/14us. Candidate counts
remain idle activity,not BEMF. Stack4172/2772/2440. Optional ELF SHA
7F222FA10381C5B82AEB02BF5970FC935FC9FC4BA7077EA8EE80FC85E8B95604.

117Python tests pass,including rejecting non-revoked owners/timers,changed
commands and absent callback exercise. Optional and default release builds
pass. No motor spin or physical-output authority introduced. Powered waveform
integration and under-drive seed/handoff remain required goal work.
Normal firmware restored;safe readback and USART3 clock correction,guardcheck
18/18,recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFLT1,
COM41closed. No pending fixture or motor command remains.

## Entry 205 — commanded-phase continuity for upcoming driven transition (2026-09-13)

Inspected startup transition: old observation uses halt_phase then phase-aware
wave_timer; new disabled TIM3 test starts step1 with full833us periods. It
cannot be connected unchanged to arbitrary startup phase without extending a
partial sector or resetting phase. Added pure phase_schedule::next(theta,rate)
using actual sine extrema sector geometry. Returns current/next sector and
remaining time to exact next LUT boundary,ceil-rounded to1us. Lookup of next
boundary precomputed at compile time;runtime uses32-bit arithmetic/division.
Caller must extrapolate phase to scheduling instant and account for setup
latency; this helper grants no output/rotor/handoff authority.

Moved exact256startup samples unchanged into support/sine_table.rs,verified
entry-for-entry against pre-edit table. Existing observation sector LUT now
shares phase_schedule::STEPS; source-based geometry/voltage audit tools and
tests follow new table location. shell-sine reference untouched. Initial
new test caught its own non-wrapping phase multiplication; corrected to
wrapping_mul. Existing source-extraction test initially followed old path,
then updated without changing expected sector/polarity assertions.

Tests cover every256phase-bin position at four fractional offsets for
50/100/167/200/250/300eHz,exact previous-vs-next microsecond sector,forward
order,phase wrap,and24successive boundaries without rounded-period drift.
127Rust tests pass (105unit+1recorded+21sequence),117Python tests pass,
M0librarycheck and optional/default release builds pass. No firmware flash or
motor command: installed firmware remains E204 normal/finaloff. Physical
under-drive integration was not completed this entry and remains next work.

## Entry 206 — phase-aware TIM3 scheduling measured with ADC/COMP (2026-09-13)

New disabled-only drivenphase uses fixed synthetic startup phase0x12345678 and
the actual200eHz Q0.32 rate858993. Initial guard/mux sector matches the shared
waveform table,first timer period is remaining partial sector. Subsequent
boundaries calculated from original phase/deadline,not ISR entry time. ARR
updates without UG/counter reset preserve hardware elapsed time. Reject
early/>50us-late dispatch or mismatched planned sector; no physical gate write.

Predeclared fixture independently brute-forces waveform sector changes each
microsecond and checks first/next deadline,alongside existing ADC/COMP/timer/
shutdown contracts. captures/drivenphase_01.txt passes3/3:stop20004us,
400ticks/24commands,first75us,nextdeadline20075us,dispatchlateness11..12us,
guardISR11us,sectorISR21us. ADC167..168scans,age139..141us;COMP399reads,
gap55..59us,bracket1us,all6sectors,zero nonblank rejection. Idle candidates
2..5 remain NOT rotation evidence. Stack4156/2748/2400.
Optional ELF SHA67D980AC2CBB6E623CE0EB9C0440CB9388AA023693F493FADD070961D2CC9B3B.

Shared stop regression drivenstop_phasebuild_01 passes3/3 at5086..5087us,
late callbacks cannot advance commands;fresh sessions remain possible.
118Python tests pass,including rejection of full833us first sector,shifted
nextdeadline,excesslateness or false physical-phase provenance. Optional and
default release builds pass. Still NO powered integration or measured rotor
phase; actual halt-to-launch extrapolation and guarded writes remain next.
Normal firmware restored,registeroff verified,guardcheck18/18,recordcheck12/12,
stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed.

## Entry 207 — first actual guarded under-drive observer (2026-09-13)

Added separate bench-driven-power build and one-shot driveobs1 command.
Normal firmware and successful shell-sine reference remain unchanged in behavior.
New driven_run owns TIM3 commutation and TIM6 guard/COMP50us; shared gates_off
revokes it,serialized authorize/write/stop prevents late physical commits.
Build refuses ADC-DMA co-ownership of TIM3. Initial powered experiment restricted
to200eHz/4..6.2% duty; this is not a new objective ceiling. No BEMF candidate
can grant handoff or change commanded commutation. Existing electrical limits,
feedback age1ms,tickgap200us,host abort,20ms local deadline and original5s startup
deadline retained. End clears bridge/MOE/CCRs/ENABLE and both timers; late
callbacks invoked after shared shutdown have no owner and cannot write gates.

Transition extrapolates actual startup stop phase/time to scheduling instant,
accounts for setup and remaining partial sector,refuses stale/near-boundary
launch,then changes ARR without UG so ISR time does not accumulate into phase.
Commanded phase is not measured rotor phase. Suppress old coast PHASEANCHOR
for this mode: it belongs to earlier startup stop,not final driven shutdown.
Old microscope storage is unreferenced in this build,freeing RAM for bounded
DQ85 comparator/PWM/read-bracket/epoch records,DA85 all five guard-feedback
channels and acquisition ages,DC85 physical command times. Every row has CRC.
No UART output while driving/coasting; full records require cap1.

New scripts/drv_driven_run.py uses established6.2% catch/target,50->200eHz
10Hz rungs,then20ms driven observation. Numeric gates declared before running:
window20..20.2ms,23..25commutations,COMPgaps<=100us,ISRmax50/100us,
dispatchlateness<=50us,ADCage<=1ms,unchanged electrical thresholds,coherent
command/sample epochs,valid CRC/coast/finaloff and stack>=4096/untouched>=512.
Independent host replay verifies opposite-level arming and candidates; a
completed capture explicitly does NOT prove BEMF lock or a handoff seed.

captures/driven_power_01.txt retained REJECTED: reached20ms/24commutations,
399COMP/188ADC,zero candidates,peak1135raw,busmin11020mV,finaloff verified.
Host found last ADC time20026+age93=20119us AFTER reported stop20118us.
Cause: end recorded guard decision before feedback bookkeeping and physical
shutdown. Fixed by taking stop timestamp AFTER bridge/ENABLE writes; host
tolerance not widened. Added regression refusing ADC beyond physical stop.
ADC capacity increased192->256 because actual188scans left little capacity
margin; overflow still stops instead of overwriting. Raw offending feedback
is retained before guard refusal,not discarded. No electrical limits changed.

captures/driven_power_02.txt PASSES instrumentation/guard gates:
start119/stop20149us =>20.030ms,24commutations,399COMP/188ADC,
ISR maxima10/25us,dispatchlate8us,ADCage128us,peak657raw,busmin11271mV.
Stackspan9252/painted7860/untouched7228. Alloff,nFLT1. Corrected ELF SHA
AFCBCF81B379DBFE38F8951BC7E6075FF24A9E5FDC20DB2BAECD83C4B75E2A20.
PSU setting remains operator800mA; last reported idle11.7V. No new operator
current measurement: raw phase peaks are not calibrated mean PSU current.

BOTH powered intervals have ZERO qualified candidates. First trace PWM CNT
538..6347 gives0/399 ON reads (CCR396); second261..6388 gives35/399 ON reads.
Most nonblank samples already expected; opposite samples occur sparsely.
Strong next hypothesis: two consecutive opposite samples at50us spacing
cannot arm when opposite evidence exists only in a6.2us PWM ON window every
100us. This is a sampling/qualification constraint,not proof of wiring trouble
or motor lock. Need phase-aware comparable samples/production-path qualification
under drive before connecting a seed to the preserved AM32 handoff sequence.
Do not solve it by weakening electrical guards or counting commanded sectors
as BEMF. No further duty/speed expansion justified by this observation alone.

127Rust tests (105unit+1recorded+21sequence),127Python tests,optional/default
release builds pass. CPU utilization percentage still unmeasured; ISR maxima
here describe this new20ms observer,not sustained closed-loop headroom.
Normal firmware restored; known USART3 RCC clock bit corrected only after
safe register readback. guardcheck18/18,recordcheck12/12,stack4108/2524/2248,
allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed. No live motor/fixture process.

## Entry 208 — PWM-triggered DMA aperture and actual comparator capture (2026-09-13)

Previous goal turn was progress (E207 code,two powered traces,new timing bug
and sampling evidence). Revalidated goal/source/captures; no live motor handle.
E20750us polling can miss all6.2us ON windows and cannot collect two consecutive
ON-only opposite samples. Added PWM sample DMA instead of CPU busy-waiting in
a highest-priority guard ISR. No existing guard or phase polarity relaxed.

Local G071 HAL dmamux.rs maps TIM1_CH4 request23. New pwm_sample_dma uses
DMA1 CH2 (separate from UART/ADC channel1),32-bit peripheral/memory widths,
finite noncircular buffers,no DMA sampling interrupt. TIM1 CH4 output frozen/
externally disabled,CCDS0 selects compare requests. Shared gates_off revokes
CC4DE and channel EN. Resource-busy checks refuse stealing existing channels.

First idle-only pwmdmatiming: read TIM1 CNT atCCR4=192 and320,64words each,
with foreground ADC conversions running. Predeclared gate:all samples within
target..target+32counts (0.5us),finite64completion6250..6700us,>=20ADCscans,
flags7,noerrors,request/channel stopped,unchanged NDTR over250us after stop,
verified alloff/nFAULT. captures/pwm_dma_timing_01.txt passes both:
target192 ->196..198;target320 ->324..326;72ADCscans each,6358/6361us.
This measures request-to-CNT-read latency4..6cycles (~0.063..0.094us),not full
motor-ISR contention or analog comparator settling. ENABLE remained0.
Timing-only ELF SHA3B5118ABFF792B0F263037D431546442409A3711C82FFAE33000C825CAE630DF.

Connected the same request atCCR4=192 to COMP2 CSR during the existing20ms
guarded six-step observer. Buffer256words; transfer error or exhaustion aborts
with reason20. No candidate controls outputs. PC85 retains indexed full CSR
words; PD85 records cumulative DMA counts bracketing each physical gate write.
Host excludes ambiguous boundary indices,checks every stable sample's mux,
enabled/noninverted CSR configuration and CRC. First two stable samples per
epoch excluded conservatively before offline arming; no precise edge seed
or live handoff inferred. Old DQ85 comparator/ADC/command trace remains intact.

captures/driven_pwm_dma_01.txt passes existing guard/instrumentation gates:
200eHzcommand/6.2%duty after50->200ramped startup;start119/stop20151us,
20.032ms,24commutations,399oldCOMP,188ADC. ISRmax9/26us,lateness12us,
ADCage140us,peak942raw,busmin11438mV.216DMAwords,212stable-epoch words;
4boundary samples excluded,not deleted. Stack7800/6384/5752;alloff/nFLT1.
Powered-capture ELF SHAF555F984F94DDA792D8DE2B7C27E63B7947FAFD8D4C7B186E2318901027B0866.
PSU limit remains operator800mA,last idle11.7V. No new average-current reading.

Result changes next action:72opposite samples after blanking are now visible,
but still ZERO candidates with the E207 inverted-level observer. DMA patterns
often run expected->opposite within a sector. Offline raw-polarity replay
finds6candidates at physical epochs0/2/4/6/15/17,steps4/6/2/4/1/3:NOT a full
ordered sequence or lock. Therefore aperture access alone was not the full
entry problem. CORRECTED E209: E072's inverted polarity is also active in the
sustained path: prepare_early calls observe_begin,setting PHYSICAL_OBSERVATION;
recorded COREPOL physical_raw_inverted=1 confirms it. The original E208 claim
that sustained coast-reference used raw was wrong. Do not blindly
transfer either convention or flip wiring. Reconcile actual driven phase,
direction and production qualification with retained waveform evidence before
granting a measured seed authority. No electrical/duty/speed expansion here.

133Python tests,127Rust tests,optional/default release builds pass. New tests
reject bad DMA flags/counts/CRC,missing ownership/safing/provenance,late aperture
samples and invalid record framing. Synthetic polarity replay is labeled as
such and grants no physical pass. PORTABLE_WINS documents method and limits.
Normal firmware restored;RCCalready08040000,registeroff safe. guardcheck18/18,
recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed.
shell-sine SHA unchanged. Goal remains incomplete; no active fixture process.

## Entry 209 — correct polarity inference; later ON sample still lacks qualification (2026-09-13)

Previous turn was progress (DMA timing qualification plus actual powered CSR
capture). Revalidated goal/current files before proceeding. Traced full state
initialization,not only Comp.output_level's conditional: prepare_early calls
observe_begin,which sets PHYSICAL_OBSERVATION=true. Sustained successful
range300_fastreturn50_02.txt reports COREPOL live_raw_inverted=1 and
physical_raw_inverted=1. Thus E208's statement that the working sustained
path uses raw was WRONG; corrected that paragraph and added recorded-evidence
regression. flying_bench also inverts physical CSR samples before filtering.
The driven observer's inversion matches this established path. No polarity
flip,rewiring or handoff-authority change follows from opposite-polarity replay.

Added idle-only drivepwm192/320 setting in bench-driven-power. Both DMA targets
were qualified in E208 disabled timing tests. prepare_comp receives duty and
refuses unless target+32counts lies inside commanded ON time; recorded target
is APPLIED,not a later shell setting. Host validates same margin and explicit
setting acknowledgement. Fixture --pwm-target320 resets setting to192 on exit.
Only sample timing changes; keep established6.2% startup/under-drive duty,
50->200eHzramp,20ms guard window,all electrical/tracking/finite/off requirements.

captures/driven_pwm320_01.txt passes instrumentation/electrical gate:
start124/stop20144us =>20.020ms;24commutations,399oldCOMP,188ADC,
216PWM-DMA samples,all216outside ambiguous command brackets. MaxISR10/26us,
lateness6us,ADCage128us,peak1143raw,busmin11486mV,stack7792/6376/5744.
45opposite-level samples after blanking,0established-polarity candidates.
Offline alternate raw polarity yields12candidate sectors,not a qualified
timestamped seed or lock. Samples often change expected->opposite later in
the interval; the first10full sectors are all expected. This does not prove
a true BEMF crossing rather than driven-current/switching/phase-alignment
effects.3->5us within ON alone did not solve qualification; do not grind more
equivalent sample offsets or flip polarity blindly. Driven alignment/current
effects versus the working entry path remain next unresolved mechanism.
ELF SHA5099904E346D4B8F3AB825EF260B3BB1559A791DCA48E569FB1EEF81569F49D8.

135Python tests pass,including captured target provenance,bad target/outside
ON refusal,and actual sustained inversion evidence. Optional/default release
builds pass. No minz/rm32 source changes,mean-current claim or range expansion.
Normal firmware restored;safe register check preceded known USART3 RCC bit
correction. guardcheck18/18,recordcheck12/12,stack4108/2524/2248,allgates/MOE/
ENABLE/CCRs0,nFLT1,COM41closed. No active motor or fixture process; goal open.

## Entry 210 — separate observer duty; recover independent coast evidence (2026-09-13)

Previous goal turn was progress:applied-target experiment and correction of
the false sustained-polarity inference. Re-read goal/current sources. Selected
lower under-drive duty to test driven-current contribution without changing
startup speed/duty,polarity,sampling target or electrical safety thresholds.
Sine and six-step apply different waveforms; equal duty is not equal voltage
vector/current. Do not infer torque equivalence from the commanded pair alone.

Added idle-only drivedu40..62,0=inherit startup duty. Stored setting consumed
once at driven_run entry,never changes sine startup. Fixture --drive-duty
validates requested/applied target and clears any pending setting on every
exit. PWM sample+32counts must fit selected ON pulse; invalid CLI combinations
refuse before hardware. No duty above existing6.2% experiment cap introduced.
All three attempts retain6.2% catch/startup,50->200eHzramp,20ms driven interval,
CCR4=192,800mAoperatorPSU setting,electrical/timing/host guards and finaloff.

captures/driven_du54_01.txt:5.4%,20.021ms,24commutations,399oldCOMP,188ADC,
216DMA(215stable),ISRmax10/27us,late9us,ADCage128us,peak869raw,busmin11151mV.
captures/driven_du46_01.txt:4.6%,20.031ms,24commutations,399oldCOMP,188ADC,
215DMA(allstable),ISRmax10/26us,late7us,ADCage127us,peak547raw,busmin11199mV.
Both have0established/inverted-polarity candidates;offline raw replay6each.
Peaks are measured per-attempt,not a calibrated mean or controlled same-rotor-
phase comparison. Lower duty alone did not solve crossing qualification.
Bothstack7792/6368/5736;alloff/nFLT1.ELF SHA
970ABE9BE69DA9030FB2E5183DD8D8A3E6A91FBF5B7C31E5D93801CF0B05FA5D.

Needed independent rotation evidence,not more equivalent duty repetitions.
Existing sustained coast analyzer correctly refused these observer captures
(no ACCEPTQUALITY). Inspection found E207 suppressed valid COASTCOMP timing
brackets together with the invalid startup PHASEANCHOR. Separated them: coast
read brackets now dump whenever coast rows exist,while stale phase anchor
stays suppressed for this mode. Extracted reusable coast_bounds helper; old
sustained analyze still verifies its original powered path before calling it.
For driven data caller verifies drv_driven_run first. No fabricated comparator
timestamps or relabeling of old captures; older missing-bracket records remain
insufficient for this measurement.

captures/driven_du46_coast_01.txt:4.6%,20.021ms,24commutations,399oldCOMP,
188ADC,215stableDMA,ISRmax9/26us,late10us,ADCage129us,peak667raw,busmin11522mV.
Again0established candidates (rawalternate4),alloff. Corrected-capture ELF SHA
2F2DE61F2FB82D88432DD8E1A3F32D0A87ED3B0F5C1EE4A97CCF5D8EE425066A.
First32coast rows now have valid separate comparator timing. With explicit
100us read uncertainty,first full-period frequency bounds A169.75..243.25,
B171.14..240.44,C171.14..240.33eHz;fourfull-period bounds perphase recovered.
They are consistent with roughly200eHz rotation,conditional on valid edges
and no missed transitions. They are NOT exact shutdown speed or whole-run
lock/qZC evidence. This separates missing driven qualification from absence
of rotation; phase alignment and driven-current effects remain unresolved.

136Python tests pass,including actual requested-duty provenance and coast
brackets without a stale phase anchor;optional/default release builds pass.
No reference-hardware reruns,minz/rm32 edits,current calibration claim,or
goal-envelope expansion. Normal firmware restored;safe register reads before
known USART3 clock correction. guardcheck18/18,recordcheck12/12,stack4108/2524/
2248,allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed,shell-sine hash unchanged.
No active fixture or motor process; full goal remains incomplete.

## Entry 211 — offline phase fit identifies missing alignment measurement (2026-09-13)

Previous turn was progress (three powered attempts,duty separation and restored
independent coast brackets). Re-read goal/currentstate. Used retained E210
coast evidence to test phase/direction before another waveform change.
Added scripts/drv_coast_phase.py,read-only with no hardware access. Validates
actual driven capture/CRC/electrical/finaloff first,then uses fixed first12
coast rows and their separate comparator read intervals. Fits a balanced
sinusoidal phase-voltage model at constant frequency,raw neutral-vs-phase
comparator sign,1degree/1Hz grid,150..260eHz,both rotation directions.
Read interval must be shorter than a half-cycle; sign may occur anywhere in
that interval. No invented edge-midpoint precision or controller-fed edge
selection. Assumptions include valid transitions,no aliasing,and explicit
caller timing uncertainty. This is feasibility,not statistical confidence.

For captures/driven_du46_coast_01.txt with100us additional uncertainty:
823forward grid fits,frequency projection178..219eHz,coast-origin phase
projection167..220degrees. Reverse direction has0fits within this search.
Sensitivity0/50/200us gives325/543/1547forward fits and phase projections
177..210 /172..215 /155..230deg respectively;reverse remains0. Balanced-phase
model uses exact120degree spacing,not the startup LUT's85/256approximation.

Commanded phase extrapolated to actual driven stop is80.824389deg. HOWEVER,
its origin differs from the fitted coast-origin phase: exact elapsed gap
between driven shutdown and coast_start_us is not captured. Existing source
performs safing/cleanup and separate clock reads there. Therefore the result
suggests a substantial alignment discrepancy but is NOT a measured phase
offset and must not be applied as a commutation correction. Need timestamp
brackets linking those origins before changing alignment; no polarity flip
or wiring diagnosis follows. This gives a discriminating next measurement
instead of more equivalent duty/PWM-aperture sweeps.

138Python tests pass in current tree,including synthetic known phase/direction,
interval-censored crossing,wide-interval refusal,and actual capture provenance
with phase_correction_authorized=false. No firmware edits,flash,motor run or
reference-board access this entry. Installed firmware remains E210 normal and
its last verified outputs-off state; no fixture process started. Goal remains
open: driven qualification/handoff and full envelope/parity requirements are
not replaced by this offline model.

## Entry 212 — measured clock relation; phase advance reveals candidate sequence (2026-09-13)

Previous goal turn was progress (reproducible phase fit identified missing
clock relation). Added TIM17 reads bracketing coast_start_us acquisition after
actual driven shutdown; CB85 records valid/min/max delay with CRC. Reset at
each run; reject invalid/reversed/>1ms gap or >20us bracket. No timestamp
refresh of an edge or live handoff. Host preserves old captures as missing
origin evidence; conditional phase projection is emitted only with valid CB85.

captures/driven_coast_anchor_01.txt,unchanged4.6%/200eHz20ms observer:
24commands/399oldCOMP/188ADC/215DMA,20.006ms,peak760raw,busmin11534mV,
ISRmax10/26us,late14us,ADCage128us,0candidates,alloff. Gap10..11us measured.
With100us coast-read uncertainty,1003forward grid fits project model phase
relative to commanded stop at+76..152degrees;reverse fits0 within150..260eHz.
Clock gap is too small to explain that projection. Constant-frequency sine
model remains conditional,not a calibrated correction or proof of lock.

Selected bounded+30degree experiment,NOT the full modeled offset. Added
idle-only drivephase one-shot; uses existing campaign::phase_shift and exact
phase_schedule boundaries. Startup6.2% sine/ramp remains unchanged. Shift only
the subsequent4.6%/200eHz20ms driven interval. Applied value recorded separately
from target; fixture validates acknowledgement/applied value,clears pending
setting on all exits. Polarity,electrical limits,deadlines and handoff authority
unchanged. After+30timing/electrical pass,allowed the next+30increment to+60.
No larger shift permitted; no blanket phase calibration from the model.

captures/driven_phase30_01.txt:
20.019ms,24commands,399oldCOMP,188ADC,215PWM-DMA,ISRmax10/26us,late8us,
ADCage129us,peak595raw,busmin11522mV,clockgap11..12us. Established-polarity
old sampler now has7consecutive candidate edges (previous zero-offset runs0).
PWM-synchronous offline replay has2candidate sectors. Stack7776/6360/5728.
ELF SHA3E6EA1861E4BCB6746D02020EC226318B38CF22A311384D3E478ED2CA2253EBE.

captures/driven_phase60_01.txt:
20.006ms,24commands,399oldCOMP,188ADC,215PWM-DMA,ISRmax10/26us,late8us,
ADCage141us,peak556raw,busmin11366mV,clockgap10..11us.8old-sampler candidates,
longest5consecutive;10PWM-synchronous candidate sectors. Alloff/nFLT1,
stack7776/6360/5728.ELF SHA
F4BD8937AB9D16D4C643FD59D6F92203BF3546E1A5D05798146AD8E3FA19F977.

Phase advance materially changes driven qualification without a polarity flip.
But10candidate sectors are NOT12qualified intervals/a fresh seed or lock.
At+60,early epochs often have only one expected sample at sector end (cannot
confirm);later epochs lose opposite baseline during first-two-sample blank.
At+30,early expected crossings qualify then disappear as their position moves
earlier. Preserve these raw patterns rather than picking favorable counts.
The next work is fresh under-drive event qualification using production
timing.100us PWM samples are useful evidence but cannot be converted blindly
to a precise final seed timestamp; sampled confirmation/phase uncertainty
would consume the measured handoff budget. No further blind phase sweep.

141Python tests pass,including CRC/bracket rejection and recorded phase-sweep
provenance;optional/default release builds pass. No average-current calibration,
reference hardware rerun or minz/rm32 edit. Normal firmware restored,RCCalready
08040000,safe register reads,guardcheck18/18,recordcheck12/12,stack4108/2524/2248,
allgates/MOE/ENABLE/CCRs0,nFLT1,COM41closed. No live fixture; full goal open.

## Entry 213 — production COMP ISR record-only seam (2026-09-13)

Previous reply answered CPU-headroom status but made no campaign changes:
no-progress for the goal audit. Re-read current objective; continued the E212
driven-entry qualification problem rather than expanding duty/speed blindly.

Source inspection confirms diagnostic driven_observer/Detector admission is
not production IRQ admission. The diagnostic requires two opposite and two
expected levels across coarse visits, plus200us post-command blank. Actual
minz comp_isr gates at interval.count()>average_interval/2, then calls
interrupt_routine's immediate filter_level comparator reads. It does not
require two previous coarse opposite samples. Closed-gate expected level
leaves pending set: real IRQ-rate protection is still required.

Added examples/support/driven_irq_core.rs: generic no_std adapter invokes that
actual comp_isr, with supplied comparator, interval timer and critical section.
PWM/phase methods and COM timer are private record-only sinks; physical gate
or COM timer implementations cannot enter this bundle. EV_ACC plus requested
ARR returns Acceptance; other sink activity flags UnexpectedCall. Invalid
step, zero/>12 filter or uninitialized average refuse before hardware access.
12 is the existing core_bench configured filter. The supplied interval timer
IS sampled/reset and comparator pending/mask IS operated by minz as usual;
this is not a side-effect-free observer and must own those sensing resources.
No COM ISR is dispatched here and no output/handoff authority is returned.

Six new synthetic tests call the actual shared adapter: all six sectors with
exactly12 immediate expected reads and no historical opposite baseline;
each possible single bad-read position; strict833/834 gate boundary for
average1666; pending camp versus opposite-level clear; invalid state no-access;
actual sampled interval1587 rather than assumed1666, last/this shift, no
duplicate acceptance without pending and no physical COM deadline afterward.
These are contracts, NOT replayed BEMF, measured timing, or lock evidence.

133Rust tests pass in BOTH default and bench-range300 profiles
(105unit+6new+1recorded+21sequence); thumbv6m-none-eabi library check passes.
Incremental-cache AccessDenied notes do not fail these commands (all exit0).
No sibling minz/rm32 edits, flash, UART, or motor commands; installed firmware
untouched, no newly claimed bench off-state measurement. CPU occupancy remains
unmeasured. Adapter is not linked into shell-pwm yet. Next: live sensing HAL,
serialized physical epochs, real interval origins, IRQ-rate and stop ownership,
timestamped record-only accepts during the same guarded20ms trial. Only then
can real ordered intervals support fresh-seed/handoff work. Do not repeatedly
replay one100us DMA sample as if it supplied immediate production reads.

## Entry 214 — connect real COMP IRQ to optional driven observer (2026-09-13)

Previous goal turn progressed the production ISR adapter and tests. This turn
adds bench-driven-irq, implying bench-driven-power, without altering default
firmware behavior. driven_irq_live supplies actual inverted COMP2 input,
existing EXTI18 mask/clear semantics, TIM2 at2MHz/16bit and critical sections
to driven_irq_core::visit. Root ADC_COMP dispatches here only while driven_run
owns the experiment. TIM3 still owns every forced physical commutation; no
physical COM timer is supplied to the adapter and no handoff is attempted.

Per-command setup follows actual sixstep_write, changes selected input/edge,
clears pending and enables for that physical epoch. Existing minz polarity
inversion retained. Average1666ticks and wait416/filter12 are explicitly fixed
diagnostic configuration, NOT an acquired/blended estimator. TIM2 starts at
prepare and resets only on actual minz acceptance: first interval is startup-
relative and flagged previous_accept_exists=0, never a fabricated ZC interval.
Later intervals may span missing sectors; the flag alone does NOT qualify a
sequence. Records require subsequent ordered-interval qualification for seed.

Rate64calls/1ms bucket retained alongside existing50us guard/200us tick-gap/
1ms feedback freshness/current/bus/nFAULT/host/finite deadlines. Unexpected
core seam, invalid epoch or rate failure immediately returns to driven end
with reason21. Shared end revokes ownership, masks and clears COMP, stops TIM2,
then existing DMA/TIM3/TIM6/output safing. New command and enable paths refuse
without driven ownership. Reset occurs at each attempted run before admission
so an early refusal cannot dump a previous session's IRQ records.

Fixture-only DRIVENIRQ summary and CRC DI85 rows retain epoch,step,before/after
ISR acquisition bracket,actual interval,requested ARR and previous-accept flag.
Host validates physical epoch windows,one acceptance per epoch,strict production
gate,record counts,mask state,rate<=64,ISR measured maximum<=50us and subsequent
intervals against successive time brackets (two half-us ticks endpoint allowance).
Malformed/absent provenance is not silently accepted when IRQ records exist.
Legacy captures without the extension still replay; zero accepts is reported
honestly and cannot establish functioning IRQ BEMF or a handoff.

142Python tests pass (new synthetic IRQ/bracket/epoch/rate rejection coverage).
Optional and default release builds pass. Optional ELF SHA256
DD343D1BBBD670A8C89B6B5C0E318FDC6F744AFFD8E8AD233600F8C8A6EAF224.
Optional linker stack bounds20007334..20009000 give7372bytes available, NOT
measured high-water headroom. No flash/UART/motor commands this turn; installed
firmware unchanged. Root ELF subsequently rebuilt normal release.

Next actual bench gate: safe idle checks and stack, then same E2124.6%/+60deg/
200eHz20ms driven trial with retained ADC/DMA/old-sampler evidence. Require
existing electrical/deadline/finaloff checks plus new IRQ record validation,
nonzero real delivery and measured handler/stack margin; retain any fault.
No increase of speed/duty, seed authority or production filter tuning here.

## Entry 215 — real driven IRQ sequence; fix observer/scheduler priority (2026-09-13)

Previous turn progressed integration. Verified no live bench fixture/probe
process,normal UARToff,then installed E214 optional ELF. Idle guardcheck18/18,
recordcheck12/12,stack7372/5948/5672,alloff/nFLT1. All four attempts below use
same6.2%startup50->200 ramp then requested4.6%/+60degree/200eHz20ms observer,
operator800mA PSU setting retained. No physical wiring or threshold changes.

captures/driven_irq60_01.txt: FAIL reason15 forced-sector lateness,stop1559us
vsstart149us.47COMP calls,2real accepts,IRQmax14us,ratepeak24;accept epochs0/1
at619..629/1532..1542us,interval961/1825ticks. Second physical command at696us;
next planned deadline1503us. IRQ acceptance completes1542us and guard/dispatch
bookkeeping follows; forced sector misses50us lateness allowance. Finaloff,
stackuntouched5316. Pending-camp comparator work at priority0x40 can starve
forced TIM3 at0x80 despite short individual handlers and no rate-limit fault.

Fix scoped ONLY to bench-driven-irq: guard stays0,TIM3 now0x40,COMP0x80.
Forced timer owns physical deadlines here,unlike production comparator-led
commutation. No filter/blank/gate/rate/lateness/electrical guard relaxed.
Optional ELF SHA13ABB08382A3FDFCAB25ACACAC9260C811F6F67EBE0D719CA1F99AE35398B0D5.

captures/driven_irq60_02.txt: startup FAIL reason4 (ADC rail/phase peak path),
before driven observer. Raw capture retained,finaloff verified. Do not erase
this entry failure from the same-build campaign denominator.

captures/driven_irq60_03.txt: PASS20.040ms,24forced commands,399oldCOMP,
148ADC,216DMA,383actualCOMP IRQcalls,23accepted events consecutiveepochs2..24.
IRQmax14us,ratepeak46/64,guardmax11us,sector36us,late8us,ADCagemax313us.
Peak483raw,busmin11283mV,oldcandidates10/PWMcandidates10. First12measured
post-initial intervals1462..1691ticks;seven rolling cycles9337..9664ticks.
Stop-to-coastgap38us,stackuntouched5316,finaloff/nFLT1.

captures/driven_irq60_04.txt: PASS20.040ms,24commands,399oldCOMP,146ADC,
215DMA,393IRQcalls,24accepted consecutiveepochs1..24. IRQmax14us,ratepeak39,
guard11us,sector36us,late8us,ADCagemax321us. Peak441raw,busmin11498mV,
oldcandidates11/PWMcandidates10. First12real intervals1397..1739ticks;
seven rolling cycles9402..9758ticks. Gap65..66us,stackuntouched5316,finaloff.

Both successful trials satisfy existing individual666..2000 and cycle8000..
12000 acquisition bounds on first12REAL post-initial intervals,offline only.
The initial interval is prepare-relative and excluded. Full timestamp/epoch/
CRC/interval-bracket checks pass. This establishes usable driven IRQ event
sequences,NOT a live fresh Seed,closed-loop handoff,independent qZC or current
calibration. Same fixed-priority build cohort2/3entry-to-observer completion,
2/2that reached observer; initial-priority lateness failure also retained.

Next use this sequence to qualify under drive and hand over at a fresh actual
acceptance,with ownership transfer/estimator initialization and measured arm
margin. Do not keep collecting equivalent20ms proofs or infer lock from counts.
Normal build restored. Reset briefly left UARTsilent; read RCC08000000,
PD1ODR0,BDTR00000c1a,CCRs0 then applied known USART3 clock enable08040000.
UART guardcheck18/18,recordcheck12/12,stack4108/2524/2248,allgates/MOE/ENABLE/
CCRs0,nFLT1 confirmed. COM41closed,no live fixture. Full goal remains open.

## Entry 216 — live one-shot driven seed qualification (2026-09-13)

Previous turn made hardware progress: two consecutive-epoch IRQ sequences.
Added driven_seed::Qualification using existing flying_acquire::RuntimeAcquire.
Acceptances supply physical epoch/step,ISR before/after brackets and actual
TIM2 interval. Invalid brackets,nonconsecutive/duplicate epochs,interval vs
timestamp disagreement and sequence/cycle faults latch refusal. First accepted
timestamp initializes acquisition; prepare-relative interval is never counted.
After13acceptances,existing12interval/seven-cycle checks produce Seed. Its
edge_tick is bracket START in half-us units: conservative age,not refreshed
at ISR return or at later accepts. Seed::handoff unchanged32us margin remains.

Connected qualifier to actual IRQ acceptance path; CRC DS85 fixture-only dump
reports ready/step/timestamp/average/interval and cycle counts/fault. Host checks
ready seed exactly against first13DI85 records,including missing-epoch rejection,
seven cycle bounds,mean and immutable timestamp. Synthetic mutation test uses
real E215DI85 input but fabricated DS85; explicitly NOT new hardware evidence.
Four Rust tests cover12-not13intervals,wrap,stale handoff refusal,duplicate/
missing epochs,brackets,interval mismatch and too-fast cycle. Default/range300
each137Rust tests pass;144Python tests pass;M0 library check passes.

Optional full release initially overflows flash1856bytes. Omitted obsolete
disabled-only drivencheck/drivenadc/drivencomp/drivenstop/drivenphase shell
entrypoints ONLY for bench-driven-irq. All remain in older driven-entry/power
profiles; normal guardcheck/recordcheck,ADC feedback and safety unchanged.
Optional and default full release now build. New ISR qualification cost and
stack high-water are UNMEASURED; installed firmware untouched,normal ELF last
built. No flash,UART or motor commands this turn.

Handoff integration constraint found in actual source: core_bench::prepare_early
calls powered_timer::prepare (gates/ENABLEoff,TIM6 reconfigure),and observe_begin
also controls shared COMP/COM timers. It cannot be called while driven owner
is active. Existing powered entry also fetches flying_bench's baseline,not the
fresh driven ADC sample. Next must stage core storage before fresh qualification,
then explicitly transfer stopped forced scheduler to prepared powered guard
with original feedback timestamps and already-awake ENABLE. Preserve actual
edge clock origin,measured remaining arm budget,original finite timeout and
output-off refusal. No fresh handoff or closed-loop startup claim yet.

## Entry 217 — live seed qualified and ISR cost measured (2026-09-13)

Previous turn implemented/tested qualifier. Verified idle/no live bench process,
installed optional E216 build;guardcheck18/18,recordcheck12/12,stack7428/6020/
5744,outputs off/nFLT1. ELF SHA
F01F6FB3405E82F8CB0DF4AB71F8C008AA0F930C40BC1186B7A533F10A20222C.
Same6.2%startup50->200,4.6%/+60deg/200eHz observer,800mA PSU limit unchanged.

captures/driven_seed60_01.txt: refusal reason19,stop161us,start0,no commands,
reads or ADC;remaining initial partial sector failed setup admission. Raw
failure retained,not successful entry or IRQ qualification. Finaloff.

captures/driven_seed60_02.txt:20.019ms completion,24commands,398coarseCOMP,
146ADC,216PWM-DMA.350actual IRQcalls,24accepts,ratepeak39. Live DS85 reports
ready1,step1,edge_tick21014half-us,mean1595ticks,12intervals,7rollingcycles
9408..9758ticks,fault0. Host verifies seed exactly against DI85,including
conservative bracket-start timestamp and exclusion of initial interval.
IRQmax33us including live qualification (E215 without it14us),guardmax11us,
sector37us,late24us,ADCage354us. Peak623raw,busminimum11498mV. Coarse sampler
12candidates,PWM replay11,neither used to fabricate seed. Stack untouched5388,
span7428. Finaloff/nFLT1;stop/coastgap50..51us. No threshold relaxed.

This is actual under-drive seed qualification,not handoff. Timestamp stays at
the first qualified event even though observation continues; it is stale by
the eventual dump and cannot be used to start power after20ms. New code must
consume at fresh qualification. Existing guarded_power_run expects ready()
AWAKE token and flying_bench baseline; driven_run has physical ENABLE awake
and fresh feedback but does not own those APIs/tokens. Transfer must explicitly
carry provenance/original ADC timestamps,not reuse unrelated cached baseline.
prepare_early mutates shared timers and shuts outputs; stage initialization
before forced drive,then bounded ownership adoption after qualification.

Normal release restored,RCCalready08040000;guardcheck18/18,recordcheck12/12,
stack4108/2524/2248,allgates/MOE/ENABLE/CCRs0,nFLT1. COM41closed. Added raw-
capture regression distinguishing actual seed pass from partial-sector refusal.

## Entry 218 — explicit release size optimization, s/z comparison (2026-09-13)

Operator requests --release,LTO,size opts (s or z depending). Existing builds
already used release,thin LTO,one codegen unit,but default opt-level3. Set
release opt-level="s" explicitly;retain lto="thin",codegen-units=1,debug=2.
Debug information remains useful for stack/autopsy and is not flashed as ELF
debug sections. bench-probes inherited profile comment corrected accordingly.

Built same current optional bench-driven-irq source in both settings:
s:text89716,data956,bss28556 ->text+data90672bytes;SHA
14465FB6B5140E9BE919F365F64FDBEF3244CF1D002908854AA7B87523E52E98.
z:text82968,data1124,bss28588 ->text+data84092bytes;SHA
6FA1BFC6FD89583884C5DE00B8811B61AFFD6F1457220A3BBD0AB3A0F784B021.
z saves6580bytes of text+data here;runtime timing not measured. z used a
temporary CARGO_PROFILE_RELEASE_OPT_LEVEL override restored afterward.
Keep s default until runtime evidence supports selecting z. Normal release
also builds with new s setting;it is the last ELF in root target directory.

No flash,UARTrun or motor command during this optimization change. Installed
normal firmware remains prior optimization3 version. Earlier handler maxima,
stack high-water and timing margins are historical evidence for prior build,
NOT timing qualification of s or z. Next hardware test must requalify selected
size profile before relying on those budgets for driven-to-powered handoff.
Full objective remains active;handoff implementation not completed this turn.

## Entry 219 — size-s timing runs; initial partial-sector seed gap (2026-09-13)

Previous turn changed operator-requested build profile and measured size.
Installed release/s/thin-LTO optional IRQ observer after verified idle. First
postreset UARTsilent; safe registersPD1ODR0,BDTR00000c1a,CCRs0,RCC08000000.
Known USART3 clockenable did not initially recover configuration: USART3 CR1/
BRR0. captures/driven_seed_s60_01.txt retains fixture preflight refusal only,
no motor start. Direct debug CLI errored; live GDB listener PID65312 confirmed
but IPv4/IPv6local connections timed out. Added optional --host ::1 to register
script,then stopped that exact GDB process and verified terminal handle.
With outputs previously verifiedoff,halted through DHCSR,selected PC through
DCRSR and read DCRDR0800b030. addr2line/disassembly place it in main clock_us,
NOT an initialization wait. No flash written during diagnosis; subsequent
download/reset resumed execution.

Added explicit USART3 RCCenable/readback/DSB BEFORE existing HAL configuration.
First boot shell worked,but repeat reset was silent,RCC08000000 with UART
CR1=0d/BRR22b retained. Thus pre-init ordering is NOT proven to fix reset wart.
Known external clockenable08040000 restored UART. Debug/reset interaction with
clock state remains a hypothesis;do not call it an established compiler bug.
Optional ELF SHA16C971F4A4245A249D91B0828EDDD5226A5D45812ABB1CCFA5FC3020668F3A48.
Idle guardcheck18/18,recordcheck12/12,stack7348/6116/5856 and alloff/nFLT1 pass.

Same6.2%startup,4.6%/+60deg/200eHz observer,800mA PSU limit retained:
driven_seed_s60_02:20.042ms,24commands,399coarseCOMP,132ADC,216DMA,
367IRQcalls/23accepts,IRQmax23us,ratepeak40,guard11us,sector37us,late9us,
ADCage321us,peak455raw,bus11569mV. No seed,fault2.
driven_seed_s60_03:20.013ms,24commands,398coarseCOMP,133ADC,216DMA,
364IRQcalls/23accepts,IRQmax23us,ratepeak41,guard11us,sector38us,late7us,
ADCage311us,peak481raw,bus11534mV. No seed,fault2.
Both pass observer electrical/timing/finaloff checks,NOT seed/handoff. Both
accepted epoch sequence exactly0,2..23. Initial partialsector0 has an acceptance,
epoch1does not,and latch correctly refuses the gap despite later22consecutive
acceptances. No unknown polarity change or relaxed guards.23us maximum is
NOT full successful12interval qualifier cost: it refused at second acceptance.

Next address initial partial-sector admission explicitly (exclude it as
acquisition anchor,or bounded entirely-fresh acquisition after refusal),then
fresh handoff. Never join0->2 as consecutive or infer seed from event count.
No further equivalent repeats warranted. Optional size-s image remains
installed at turn end,not normal; UARToff/guardcheck18/18/recordcheck12/12,
stack and allgate/MOE/ENABLE/CCRs0,nFLT1 verified. COM41closed,no live fixture.

## Entry 220 — full-sector seed admission and size-s qualification (2026-09-13)

Previous turn measured size-s runs and exposed partial0->missing1 sequence.
Qualification now ignores epoch0 ONLY before the first admitted edge: epoch0
is known to begin partway through startup's sector. Raw DI85 retained. Every
gap after first full-sector edge remains a latched refusal; no missing edge
fabricated. New unit test supplies partial0,missing1,then13full accepts from2
and requires all12intervals. Timestamp immutability/sequence/age checks remain.
DS85 header explicitly marks partial_epoch_excluded=1;host understands both
old and new admission policies,never retroactively reinterprets old captures.

138Rust tests pass,prior146Python suite passes,optional release/s/thin-LTO
build passes. SHA3677727D5D40938C7C25370B1432FDF098025B6F3719291BD2633C4E203EE3C6.
Verified idle/no live bench process before flash. UARTreset wart recurred;
RCC08000000 and PD1ODR0/BDTR00000c1a/CCRs0 verified before known08040000clock
repair. UARTguardcheck18/18,recordcheck12/12,stack7348/6116/5856,alloff/nFLT1.

Same6.2%startup50->200,4.6%/+60deg/200eHz observer,800mA setting:
driven_seed_full60_01:20.032ms,24commands,399oldCOMP,131ADC,216DMA,
388IRQcalls/24accepts,IRQmax33us,ratepeak45,guard11us,sector37us,late15us,
ADCage339us,peak555raw,busmin11510mV. Seedstep4,edge21728half-us,mean1605,
12intervals/7cycles9466..9804,fault0. Raw trace starts epoch1 (no epoch0accept).
driven_seed_full60_02:20.008ms,24commands,399oldCOMP,131ADC,215DMA,
371IRQcalls/23accepts,IRQmax33us,ratepeak46,guard11us,sector38us,late16us,
ADCage370us,peak522raw,busmin11402mV. Seededge23222half-us,mean1592,
12intervals/7cycles,fault0. Raw trace starts epoch2 (no epoch0accept).

Both independently pass DS85-vs-DI85/electrical/timing/finaloff verification.
These qualify complete live seed calculation cost on size-s,NOT hardware
execution of the ignore0 branch (tested synthetically),NOT fresh handoff or
closed-loop startup. No more equivalent observer repeats needed; implement
staged core initialization plus current-owner feedback/awake adoption and
fresh-edge arm with unchanged margin. Optional image remains installed,
fixture finaloff confirmed allgates/MOE/ENABLE/CCRs0,nFLT1;COM41closed.

## Entry 221 — experimental fresh ownership-transfer implementation (2026-09-13)

Previous turn qualified live seed on size-s. Added separate feature
bench-driven-handoff,implies IRQ,with explicit idle-only drivex1/0 one-shot.
Normal IRQ observer and installed E220image do not gain automatic transfer.

Before forced drive,prepare_driven resets/stages actual core recorder/estimator/
COM configuration without disabling awake ENABLE. Driven setup then owns its
TIM2/COMP/TIM6 as usual. At first qualified seed,IRQ checks live owner,nFAULT,
ENABLE,matching physical step,existing guard freshness/deadlines,original ADC
acquisition stamp and unchanged Seed::handoff margin. Private-field Transfer
constructed only there;reason22 stops forced TIM3/TIM6,COMP and DMA,clears
bridge but keeps ENABLEawake. All refusal/fault/cancel paths still disable.
Transferred awake duration is measured from guarded driven start (>=1100us),
not a guessed startup time. Token is removed from static storage exactly once.

Foreground calls new core driven_power_run. Abort,duty or stale token refuses
with outputs/ENABLEoff. powered_timer::adopt_driven requires old owner stopped,
bridgeoff,nFAULT/ENABLE and fresh original edge/feedback;configures stopped
TIM6 and adopts awake state without a wake pulse. Existing coast_run_inner now
accepts OPTIONAL initial(sample,acquired TIM17) parameter: original callers
passNone,use unchanged flying_bench baseline. Driven entry passesSome and
recomputes age from original timestamp,never cached unrelated acquisition.
Existing run guard,AM32 seeded state,16us actual arm bound,32us remaining arm
margin,COM ISR/filter/accepted-age logic and finite powered window remain.
No accepted event or interval is fabricated by transfer. No retries in this
first experimental path;prepare_driven explicitly clears dropout/reentry arms.

DX85 fixture-only stamps:step,seededge,mean,release time,foreground time,
original feedback age,foreground-entered marker. DRIVEX result1 means core
routine entered/returned,NOT proof of lock/completion. Actual powered summary/
accepted timeline/guard must establish outcome. Caller retains pre-transfer
DRIVEOBS/DI85/DS85 raw records and adds reference-controller powered summaries.

Default and new-feature release/s/thin-LTO builds pass. Experimental text91996,
data956,bss28636,stackbounds2000739c..20009000 (7268bytes linker space,not
measured margin).138existingRust and147Python tests pass;these do NOT exercise
new hardware ownership transfer. No flash,UARTrun or motor command this turn.
Installed E220observer remains unchanged,root ELF last rebuilt normal.

Before live transfer: add fixture support and verifier for shortened reason22
acquisition plus powered segment;existing observer verifier must keep refusing
this as a20ms observation pass. CB85 currently refers to acquisition release,
not final powered shutdown,and will be invalid after long hold;exclude from
handoff coast-origin claims or add correct final origin. Exercise disabled
ownership/late token refusal. Then run finite first powered transfer with raw
failures retained and measured arm/IRQ/stack margins;do not claim done here.

## Entry 222 — transfer-specific fixture and evidence checks (2026-09-13)

Re-read active objective. Previous immediate build-profile turn confirmed the
requested settings/build but did not advance handoff qualification. Continued
the E221 prerequisites without issuing flash or motor commands.

Fixed two reporting defects: driven dump duplicated the shell's core summary;
it now emits only the additional powered summary. Acquisition release cannot
timestamp final coast after powered operation. Reason22 explicitly reports
DRIVENCOAST unavailable=1 acquisition_release_not_final_stop=1, with no CB85.
The firmware also clears that bracket rather than allowing a long powered hold
to modulo-wrap into a plausible sub-millisecond delay.

live_capture now has explicit drive_handoff opt-in, requires driven mode and
20..600000ms finite duration, checks window and drivex1 acknowledgements before
run, extends host timeout and resets drivex0/window in finally before verified
finaloff. Missing handshake refuses before run; partial raw evidence retained.

New scripts/drv_driven_handoff.py separates acquisition transfer from powered
completion. Explicit transfer acquisition accepts reason22 with a qualified
12-interval live seed, complete physical/ADC/DMA/IRQ records and finaloff. The
old default verifier still requires the full20ms reason2 observation. DS85 and
DX85 must match; original ADC stamp must match final completed driven scan;
release/foreground/edge order, fresh core seed,32us remaining arm and16us arm
cost must hold. Singleton powered summaries, CRC accepted timeline, electrical
limits and actual requested-window completion are required: DRIVEX result1 is
not sufficient. Independent BEMF lock remains explicitly unproven.

156Python tests pass (synthetic provenance mutations and fake-port handshake
tests added),138Rust tests pass. These do NOT yet prove a complete successful
transfer capture passes the new verifier, nor actual disabled ownership/late
token refusal. Those checks remain before first powered handoff; also inspect
disarming when an earlier manual drivex arm survives into a new fixture.

Experimental release/s/thin-LTO build passes: text92092,data956,bss28636.
ELF SHA256 5A6CAA68BF7B76000C6974D0D1E9E68D80F2212709C4208B891E7B2F45A2C835.
No hardware access; installed image remains E220 observer. Root ELF is now
experimental handoff, so rebuild the intended feature explicitly before flash.

## Entry 223 — first powered handoff attempts: fresh-arm timing refusals (2026-09-13)

Previous turn made implementation/test progress. This turn exercised the new
boundary on hardware after closing revocation and decoder prerequisites.

Shared driven cancel previously left an already-released foreground token
intact. Added a generation stamp to Transfer; every shared cancel increments
generation,clears RELEASED and active transfer arm. Freshness checks generation
as well as original seed/feedback times. This is cancellation revocation,NOT a
blackout watchdog for the16-bit clock. NEXT_TRANSFER remains across internal
startup gates_off by design; explicit host off now disarms it even if powered.
Idle-only transfercheck uses synthetic tokens and real cancel/adopt/late sector,
guard and COMP callbacks without EVER raising ENABLE or driving gates. Checks
valid timing,expired remaining margin,stale feedback,insufficient awake time,
ENABLE-off adoption refusal,generation invalidation,stored-token removal and
late-callback outputs-off. First image3/3runs passed8/8; poststop writers18/18
and recordcheck12/12 passed. Second image transfercheck again8/8.

Host tests now include an explicitly SYNTHETIC in-memory composite of two
different archived experiments to exercise complete acquisition+powered
decoding. It is not saved as a hardware capture or counted as a motor result.
Mutations reject wrong duration,missing/duplicated summaries and failed arm.

Two real attempts: same6.2% startup50->200eHz,4.6% observer/transfer duty,
+60degree commanded phase,20ms requested powered window,unchanged800mA PSU
setting and electrical/tracking/finite-duration guards. No guard was relaxed.

driven_handoff46_01: acquisition10.728ms,13commands,212coarseCOMP,57ADC,
115DMA,279IRQcalls/13accepts,maxIRQ35us,late26us.12interval seed mean1596,
edge21826half-us,7cycles9422..9738. Acquisition peak309raw,busmin11498mV.
DX release10951us,foreground10994us,original feedbackage443us. Core latency
entry202,reset246,feedback252,guard328,reference336ticks; actualarm age360.
wait399 minusage360 leaves39ticks=19.5us,below64ticks=32us required.
CORESEEDarmed0,COASTREFstop8,DRIVEXresult2,POWERCOMMITS0,accepted0.

The shortened DMA stopped before128 of256words,so no HTIF/GIF flags yet.
Old verifier demandedflags5 for every length. Corrected exact count-dependent
contract: below128 flags0,otherwiseflags5 for allowed n<256; error/complete
flags still rejected. Regression retains this actual refusal. This is an
instrument correction,not changing motor safety or declaring handoff success.

Source showed coast_run_inner repeated full gates_off after adopt_driven had
already verified old timers/COMP/DMA stopped and bridgeclear. For the explicit
driven initial-feedback path ONLY,it now uses bridge_clear; other callers keep
full shutdown. Second build flashed and disabled check passed before rerun.

driven_handoff46_02: acquisition10.658ms,13commands,211coarseCOMP,55ADC,
114DMA,284IRQcalls/13accepts,maxIRQ35us,late18us. Seedmean1599,
edge21686half-us,cycles9434..9756. Peak326raw,busmin11545mV.
DX release10881us,foreground10938us,feedbackage402us. Core latencyentry228,
reset260,feedback264,guard340,reference348ticks; actualarm age372.
wait400 minus372 leaves28ticks=14us: again refuses,zero poweredCOMs/events.
Reset-stage measured cost improved22->16us,while foregroundarrival was later;
do not call the net handoff timing improved or either run a closed-loop pass.

Both raw attempts retained and independently verify acquisition/electrical/
finaloff; full handoff verifier rejects. Both stackspan7260,untouched5320,
allgates/MOE/CCRs/ENABLE0,nFLT1. UARTclosed,no live fixture. Both flash resets
had RCC08000000/UARTsilent; readback first verified PD1ODR0,BDTR00000c1a,
CCRs0,then known RCC08040000 workaround restored UART. Not a new fix claim.

158Python/138Rust tests pass. Installed second experimental release/s/thin-LTO
image text93560,data956,bss28644,SHA256
2DEF99CDB95F1DA33670363877B9BAD72267FD9077524A081812A2186C604791.
Root ELF matches installed feature. Next reduce measured foreground/guard
setup latency by staging noncritical state before final edge; start_inner still
does full gates_off/coverage reset plus guard construction after adoption.
Keep original feedback timestamp,actual accepted edge and32us arm floor.
Do not repeat equivalent runs without addressing that measured timing budget.

## Entry 224 — first completed under-drive BEMF handoff (2026-09-13)

Previous goal turn was progress: measured late-arm refusal localized setup
budget. This turn staged non-edge-dependent work and tested actual transfer.

powered_timer::stage_driven clears run statistics/ADC coverage before forced
acquisition,without timers/gate writes. adopt_driven requires and consumes this
stage after fresh token validation,then sets one-shot DRIVEN_ADOPTED. Shared
cancel revokes staged/adopted flags. start_inner consumes adoption even on
refusal,requires awake/prepared/non-reentry,skips redundant full gatesoff/reset
ONLY for that token. Still validates current/bus/seed/deadlines and constructs
new guard with original feedback age. Normal/recovery entry keeps old path.

First staged build disabled transfercheck8/8,poststop18/18,recordcheck12/12.
driven_handoff46_03: ci1594,edge21750,release10913us,foreground10931us,
feedbackage518us. Core actualage286ticks,remaining113ticks=56.5us,arm11us.
One physical poweredCOM but zeroaccepts; Tracking8 stops1006us,recordend1043us,
peak91raw,bus11629mV,stackuntouched5316. CORESTATE interval_cnt286 equalled
the exact seeded age despite24real COMP calls. Source confirmed driven release
stops TIM2 after prepare_driven had configured it; core arm set CNT but never
restarted CEN. Thus the avg/2 acceptance timing gate stayed permanently closed.

Fixed explicit driven seed arm: after Interval.set_count(age),start TIM2 CR1=1
inside same masked critical section. Both owners use PSC31/ARR65535. No fake
elapsed count,accepted event or relaxed time margin. Disabled transfercheck
passed again after flash. First reset this turn had workingUART; second needed
known RCC08000000->08040000 workaround after verifiedPD1ODR0/BDTR0c1a/CCRs0.

Same final build,three attempts retained,4.6%duty/+60degree/200eHz acquisition,
6.2%50->200startup and operator800mA setting. Guard limits unchanged:

- driven_handoff46_04 requested20ms: seed1592,edge24234,release12155us,
  foreground12254us(delay99us),feedbackage476us. Actualage446ticks exceeds
  wait398: armed0,zero poweredCOM/events,finaloff. Timer fix not exercised.
- driven_handoff46_05 requested20ms: seed1607,edge22332,release11204us,
  foreground11219us(delay15us),feedbackage403us. Remaining124ticks=62us,
  arm11us. Completes20.042ms,27physicalCOMs/26accepted events,zero late tail,
  no orderfault/desync,powereddeadline2. Cyclemean4520.75us (~221.202eHz),
  sigma66.776us,uncalibrated poweredpeak323raw,busmin11510mV. Acquisition
  peak351raw,bus11557mV,IRQmax35us,forcedlate44us. Stackuntouched5272/span7256.
- driven_handoff46_06 requested1000ms: seed1602,edge21668,release10872us,
  foreground10944us(delay72us),feedbackage399us. Actualage392ticks leaves
  9ticks versus64required: armed0,zero poweredCOM/events,finaloff.

This is ONE completed fresh under-drive acquisition->actual AM32 powered
window,not sustained/repeatable entry or independentqZC parity. Final-build
cohort1/3entered-and-completed,2/3latearmrefusals,3/3validlive seeds. Do not
count requested1s as a1s motor result. Every fixture verified final gates,
MOE,CCRs,ENABLE0 and nFLT1;COM41closed,no livefixture. CRC raw logs retained.

160Python/138Rust tests pass. New regressions retain frozen-clock failure,
actual fullwindow pass and same-build refusals; wrong duration stays rejected.
Default and optional release/s/thin-LTO builds pass. Installed/rootoptional
text93800,data956,bss28648,SHA256
DD83FABEC571F9E8EC81CE917EAD1E99843ECFC522F464B0C7296A673B6EB073.

Next: reduce variable release-to-foreground latency,not equivalent retry loops.
Synchronous sample_feedback is in progress when IRQ releases ownership; its
remaining scan is a source-grounded likely contributor to99/72us versus15us.
Consider bounded cancellation/yield of that scan after ownership loss,with
ADC cleanup and original last-completed feedback preserved. No timestamp
refresh or reduced32us armfloor. Then repeat entry and qualify longer holds.

## Entry 225 — acquisition scan yield and ten-second under-drive entry (2026-09-13)

Previous turn made hardware progress: first20ms fresh handoff,remaining
foreground-return variation. Implemented sample_driven_feedback ONLY in the
optional handoff acquisition loop. Before each channel,check driven ownership;
finish a single read_channel transaction already in progress,then abandon the
rest if released. No outstanding conversion returned,no ADC abort sequence,
no partial sample published. Check again before conversion/scaling. Existing
normal powered sample_feedback unchanged. LAST_FEEDBACK remains last completed
driven sample with original acquisition timestamp. Diagnostic DRIVENYIELD
records completed channelcount or no scan in progress. Guard thresholds intact.

Release/s/thin-LTO build flashed after UART outputs-off; reset console worked
without clock workaround this time. Disabled transfercheck8/8 and outputs-off
passed before driving. All runs use6.2%50->200startup,+60degree acquisition
phase,operator800mA PSU setting and unchanged electrical/tracking safeguards.

driven_yield46_01,4.6%,requested1s: seed1597,armremaining117ticks=58.5us,
armcost13us,release->foreground17us. Yield flag1,completed5channels (last scan
discarded before scaling/publication). Actual powered run598976us then
CycleTiming12,860COM/859accepted,zero poststop tail. Mean239.36eHz,cyclemean
4177.83us,sigma72.87us. Peak360raw,busmin11175mV. Retained rejection,not1s
pass. The guard trips an individual same-phase cycle,not average speed; no
evidence that average239eHz alone violates250eHz profile. Exact offending
guard cycle needs richer event/guard evidence if investigated further.

Reduced duty to4.5% for longer-window qualification rather than changeguard:

- driven_yield45_01,requested1s: complete1000042us,1409COM/1408accepted,
  mean234.875eHz,cyclemean4257.58us,sigma57.58us. Remaining118ticks=59us,
  arm12us,release->foreground17us. DRIVENYIELD abandoned1/completed1: actual
  partialscan early-exit branch exercised. Original-feedback provenance passes.
  Powered4187scans,peak360raw,busmin11068mV;stackuntouched5256.
- driven_yield45_02,requested10s: initial partialsector setup refusesreason19
  before observer drive (start0,stop258us,no reads/scans/commands). Startup
  already ran; don't call this a powered10s attempt completion. Raw retained.
- driven_yield45_03,requested10s: complete10000042us,14157COM/14156accepted,
  mean235.943eHz,cyclemean4238.31us,sigma38.33us,zero poststop events/orderfault.
  Remaining108ticks=54us,arm13us,release->foreground22us,feedbackage456us.
  Yield abandoned0: release happened between scans,so this trace alone does
  not demonstrate early-exit branch. Powered41926scans,peak361raw,bus10972mV,
  stackuntouched5256/span7248. COMP128265calls,maxwall82us;COMmaxwall69us,
  guardmax20us,commitmax21us. These are wall maxima,not exclusiveCPU utilization.

At4.5%,2/3entry-to-completion and2/2powered completions at differing1s/10s
durations; not a fixed-duration reliability cohort. At4.6%,0/1requested1s
completion despite successful arm. All four attempts retained,outputs/MOE/
CCRs/ENABLEoff and nFLT1 verified,COM41closed,no livefixture. Current figures
are raw phase peaks,NOT calibrated phase average or PSU mA. Ten-second result
is meaningful under-drive sustained operation,not completed range/recovery/
independentqZC/reference parity objective.

161Python/138Rust tests pass. New regression validates actual1channel yield,
full1s/10s captures,original feedback identities and retained guard/startup
refusals; malformed partial-feedback provenance rejected. Default/optional
release/s/thin-LTO builds pass. Installed optional image text94112,data956,
bss28656,SHA25680E6021108C8FE02103A114E55DD5CE316360265FE8DC3954E69215CEDEF6B99.
Root ELF last rebuilt normal,not flashed. Rebuild feature explicitly nexttime.

Next: bounded admission when initial partial sector is too short,then fixed
repeatability and recovery cohorts through this new entry path. Address that
known deterministic refusal rather than retrying indefinitely or loosening
timing/current limits. Duty-range expansion remains staged after this evidence.

## Entry 226 — bounded initial alignment, fixed four-attempt cohort (2026-09-13)

Previous turn advanced under-drive sustained operation but exposed deterministic
initialpartialreason19 refusal. Added Boundary::initial_wait: only when initial
remaining sector<100us,wait to that boundary with gates disabled,then recompute
from original anchor plus actual elapsed TIM17 time. No extension of partial
sector,no synthetic fullsector start. Both before/afterwait bridgeoff checks,
ENABLE/nFAULT checks,unchanged500us overalllaunch limit,100us minimum admitted
sector,original baselineage and32us final timerarm margin remain. IRQs masked
only during existing disabled setup plus bounded<100us wait;no powered timers
running there. DRIVEALIGN reports initial remainder,requested/actualwait.

New host test covers all256phase indices/four fractional positions at50,100,
167,200,250,300eHz and0/1/10/20us boundaryovershoot,checking the nextphase and
remainingwindow. Host capture verifier checks provenance/counts/launchbound.
This is commanded-waveform continuity,not independent rotorangle measurement.

Buildrelease/s/thin-LTO flashed,disabled transfercheck8/8 andoffverified before
motor. ResetUARTsilent; exactRCC08000000/PD1ODR0/BDTR0c1a/CCRs0checked before
knownRCC08040000clock recovery. Fixed4attempt cohort declared beforefirstdrive:
same6.2%50->200startup,4.5%driven/+60degrees,10s poweredwindow,operator800mA
PSUsetting. All four raw attempts retained,none substituted or omitted.

| Capture suffix | Result | COM / accepted | Mean eHz | Cycle sigma us | Arm remaining us / cost us | Peak raw | Bus min mV |
| --- | --- | --- | --- | --- | --- | --- | --- |
| driven_align45_01 | full10s | 14114 / 14113 | 235.2264 | 36.6631 | 47 / 13 | 368 | 11092 |
| driven_align45_02 | full10s | 14134 / 14133 | 235.5605 | 37.7465 | 60 / 13 | 360 | 10937 |
| driven_align45_03 | full10s | 14131 / 14131 | 235.5250 | 37.6347 | 61.5 / 14 | 370 | 11080 |
| driven_align45_04 | full10s | 14134 / 14133 | 235.5578 | 38.3994 | 48 / 14 | 394 | 11104 |

_01initialremaining18us: requested18us,actualbusywait7us because computation/
checks consumed part of remainder. Actualstart264us,nextdeadline1104us,
admitted840usinitialsector. It exercises the previously-refusing branch on
hardware through full acquisition/handoff/sustained run. Otherinitialremainders
117/550/485us neednowait. Original20msforced guard and500uslaunch bound retained.
Release->foreground29/16/16/30us; completed12intervalseed averages1598/1608/
1604/1609ticks. No late acceptedrecords,no desync/orderfault,powereddeadline2.

_03equalCOM/accepted is not an inconsistency: bootstrapCOM plus a finalaccepted
event whose futureCOM can be cancelled at deadline allows equality; no rule
that everyfinitecapture MUST endCOM=accepted+1. These streams remain accepted
controller event evidence,not independentqZC or optical speed proof.

4/4 entry-to-completion at this operating point/build,not arbitraryspeed or
disturbance qualification. Stackuntouchedmin5248;finalallgates/MOE/CCRs/ENABLE0,
nFLT1 verifiedeveryattempt,COM41closed,no livefixture. Rawcurrentpeaks are
uncalibrated. _04poweredADC42167scans/10s,COMP126963calls,maxwall82us,COM55us,
guard20us,commit21us;do not reinterpret maxima as exclusiveCPUutilization.
captures/driven_align45_cohort.csv records fixedcohort,rawtxt remainsauthority.

162Python/139Rusttests pass,optionalreleasebuildpass. Installed/rootoptional
text94584,data956,bss28664,SHA256
4FBE97BC43466A0FBFE59A7047B9DEF6F8364ADDB5939D78B6D0172E4E591161.
Next controlledtrackingloss/recovery with this entry. Currentprepare_driven
clears injection/reentryarms and driven_power_run clearsreentrysession;fixture
also refusesmixingthem. Review/integrate explicitrecovery/archive/original
deadline ownership rather than sending ignoredcommands or repeatingthiscohort.

## Entry 227 — driven tracking-loss shutdown, archive and one-shot control (2026-09-13)

Previous goal turn made fixed-cohort progress. Reviewed existing guarded_power_run
wrapper and resume_once: SessionBudget preserves originaldeadline,freeze helper
keeps firstsegment offactive stack,wake precedes freshpassiveacquisition,and
reentry is oneattempt only. New driven path had intentionally disabled injection/
retry. This turn verifies stop/archive through newentry before restartauthority.

Added separate idle-only drivedrop1/0 one-shot (handofffeature). Its own pending
arm is consumed into existingDROP_ARM only by prepare_driven. Explicitoffclears
pendingarm. No automaticreentry enabled; prepare still clearsREENTRY_ARM.
Fixture accepts driven+dropout ONLY with explicit drive_handoff,newack,and
window>2000ms. Cleanup disarms andverifiesoff; older genericdropoutack cannot
accidentally certify support. Failedackhost test proves no runcommand.

Host --dropout verifier requires actualtrackingstop timing,firstarchive fault8,
matching primary/frozen prefix/tail/count/skipped andphysicalcommitcount,CRC
timelines,noREENTRYrecord,validinitialDS/DX/CORESEED andfinaloff. Resultmarks
tracking_loss_stop_verified,NOT poweredwindowcomplete or recoveredoperation.

Flashedrelease/s/thin-LTO,UARTresetworked,disabledtransfercheck8/8/offpassed.
Allattempts6.2%50->200startup,4.5%driven/+60,operator800mAPSUsetting:

driven_drop45_01 requested10swith2sinjection. Validseed1602ticks,remaining
119ticks=59.5us,arm13us,release->foreground17us. Suppressionat2000166us;
disabledobservation2000943us,777usafterinjection,withinunchanged acceptedage
trackingguard.2818COM/2817accepted,mean234.789eHz,sigma47.042us,zero poststop
tail. Poweredpeak359raw,busmin11128mV. FIRSTSEGexactlymatchesprimary retained
eventwindows. Disabledpassivereacquisitionresult1,12intervals/sevencycles,
8869us,meanticks1435. No newgateauthority,no poweredrestart. Stackuntouched4956.

Sameboot unarmedcontrol attempts:
- driven_drop45_control_01 requested3s: drivenobserverendsdeadline20.042ms,
  noqualifiedseed,fault2. Acceptedepochs1,3,4.. skip2;timed gap3346ticks at
  epoch3. Missingintervalcorrectlyrefused; no fabricatedbridge acrossgap.
  Retained as acquisitionfailure,not completion or proof of3sinjectionabsence.
- driven_drop45_control_02 completes3000043us,4233COM/4232accepted,235.162eHz,
  sigma44.067us,remaining127ticks=63.5us,arm13us. Poweredpeak363raw,bus11104mV.
  NoDROPOUTappliedrecord,noFIRSTSEG/reacquisition; freshunarmedrun did not
  reinject. Stackuntouched4956(sameboot highwaterincludespreviousarchive).

Allthree attempts finalgates/MOE/CCRs/ENABLEoff,nFLT1,COM41closed,no livefixture.
164Python/139Rusttestspass,includingrealstop/archive/control/missingepoch
regressions and explicitnewhandshake. Installed/rootoptionaltext94596,data956,
bss28664,SHA256A66834FEE76C393DCE630CDE78C005D59B56D789C66FB0073A92456F06103992.

Next: connect onebounded resume_once through driven entry with original
SessionBudget. Preserve separate initialarm snapshot before recovery resets
CORESEED; DS/DX referinitialseed whereas subsequentCORESEEDwillrefernewseed.
Likewise preserveoriginalrequestedwindow versusresumedremainingwindow. Reuse
realawakepassiveacquisition/guardedresume,don'tclaimit is underdriverecovery.
Thisturn establishedstop/archive/one-shotbehavior,not recoveredpoweredrun.

## Entry 228 — powered recovery reached; strict deadline audit fails by6us (2026-09-13)

Previous turn establishedstop/archive; this turn connected existing bounded
recovery through new entry. Separateidle drivereentry1/0 flag consumed in
prepare_driven intoREENTRY_ARM; off clearsfuturearm. Fixture permits onlyexplicit
handoff+dropout+reentry,requires newhandshake,disarmsinfinally. Existing normal
reentry commands remainunchanged. driven_power_run uses SessionBudget from
firstentry,setsREENTRY_SESSION to suppresspassive-only diagnosticreacq,then
calls existingresume_once once. Wake-before-freshpassiveseed,oneattempt limit,
Tracking-only check,frozenfirstarchive and originaldeadline preserved.

New CRC DFA85 stores initial actualarmmetadata BEFORE recoveryobserve_begin
overwrites it: sevenu32fields as little-endianu16pairs,ci/edgeage/remainingARR/
armcost/armed/firstelapsed/requestedwindow. DS/DX matchthisinitialarm; later
CORESEED matchesRECOVERYACQ. Host verifiesseparate identities,firststopage,
electricallimits,archives,originalbudget andresumedwindow. No syntheticpass.

Firstbuildflashed,UARTclockworkaroundafterexactsaferegisterchecks,disabled
transfer8/8,poststop18/18,recordcheck12/12passed. Allpoweredattempts4.5%/+60,
10sbudget/2sinjection,6.2%50->200startup,operator800mAlimit:

- driven_reentry45_01: startupcurrentabort at1.697s,phaseCraw3277 exceeds3248
  guard. CURRENTTIMING bracketcrossesPWMwrap; do not declare falsepositive
  from that alone. No drivenacquisition/recovery. Retained.
- driven_reentry45_02: initialseed1609,actualage316/remaining86/arm13ticks-us
  metadata preserved. FirstsegmentTracking8,2822COM/2821accepted,thenfresh
  recoveryseed1432,12intervals/7cycles8568..8746,8964us acquisition. Actual
  resumeage308 leaves50ticks=25us<32usfloor: armed0,zero resumedCOM. Initial
  andsecondseed recordsremainseparate. REENTRYresult7 isNOTsuccess.

Source shows resume_once alreadystopsallowners in prepare beforeawakeacq;
coast_run_inner was repeatingfullgatesoff. For explicitresumeSome ONLY,now
bridge_clear likealreadyvalidateddriveninitialpath. No marginreduction.
Secondbuildflashed,UARTworked,disabledtransfer8/8passed.

- driven_reentry45_03: acceptedsequencegap,DSfault2,no seed;20msobserverend.
  Retained startup/acquisitionrefusal,not recoveredrun.
- driven_reentry45_04: initialseed1610 andactualinitialarm archived; first
  segment2822COM/2821accepted,Trackingstop2001110us,observedend2001146us,
  peak356raw,bus11163mV. Freshawakepassiveseed1446,12intervals/7cycles8564..8788,
  9144usacquisition. Resume resetstage24->16us versus_02; actualage292 leaves
  70ticks=35us,arm13us,so resumedpowerreallyruns. Resumedwindow7988271us,
  11285COM/11284accepted,235.449eHz,cyclemean4247.208us,sigma34.542us,
  peak384raw,bus10948mV,zero poststoprecord,stackuntouched4908.

STRICTRESULT remainsFAIL: REENTRYresume_elapsed6723741,remaining7988145,
originalend14712086. resume+remaining+200==original exactly. Actualfinalelapsed
14712092 exceedsoriginalby6us. Existinghostreentry_verified=0 correctlyrejects
even though resumedpoweredwindow completes. Do not relabelasqualifiedrecovery
or widen timestamp tolerance. Need account setup/guarddispatch/return overhead
inside originaldeadline,perhaps explicitlargerreserve for thispath; currently
SessionBudget andlegacyverifier both assume200us. Any change needs versioned
provenance,policytests,andpreservedhistoricalcapturesemantics. Initial andfinal
timingclockdomains differ; do not subtractunrelatedstamps to claim earlieroff.

167Python/139Rusttestspass; regressions retainallfourfailures,initialDFAidentity
and6usdeadlineviolation. Default/optionalrelease/s/thin-LTObuildspass. Installed
secondoptionaltext95676,data956,bss28704,SHA256
F302555E928C2BAEE4E40269A0DB4B0FB4B74C665C44990C2AD071EF94DC0E9E.
RootELF lastrebuiltNORMAL(notflashed). Everyattemptverifiedfinalgates/MOE/CCRs/
ENABLE0,nFLT1,COM41closed,no livefixture. TwootherPythonymodemprocesses used
COM8 forunrelatedrock5board,inspectednotkilled. Next fixdeadlineaccounting then
qualify repeatrecovery; do not substitute moreunarmedholds for thiscondition.

## Entry 229 — strict-deadline recovery passes three times (2026-09-13)

Previous turn produced actualrecovery butfailedoriginaldeadlineby6us. Added
SessionBudget::reentry_reserved<const RESERVE>,restricted200/300us. Existing
reentry() andnormalrecovery use200. Drivenresume uses300,subtractingadditional
100us fromavailablepoweredsegment; originalend andcampaignlimit unchanged.
Reservebudgetsonlytermination/setup margin,NOT seedarmmargin (still32us).
Everyretryreport includes REENTRYRESERVE us=... included_in_original_deadline=1.
Hostusesmissingmarker ashistorical200,acceptsonlyexplicit200/300,singleton;
unknown/malformed/duplicatemarkersreject. Strictfinal<=originalcheckunchanged.
Policytestsverifyextra100us reduction,absoluteend,oneattempt,invalidreserve,
andinsufficientremainingtime. RelabelingE228lateness stillfails regression.

Flashrelease/s/thin-LTO; knownUARTclockrecoveryafter exactsaferegisterchecks,
disabledtransfercheck8/8/offpassed. Same4.5%/+60/10sbudget/2sinjection,
6.2%50->200startup andoperator800mAsetting. Initialpassfollowed bytwoannounced
same-settings confirmations; all3attemptsretained,all3entry+recoverypasses:

| Capture | Resumed COM / accepted | Mean eHz | Cycle sigma us | Finish before original us | Peak raw | Bus min mV |
| --- | --- | --- | --- | --- | --- | --- |
| driven_reserve45_01 | 11265 / 11265 | 235.0252 | 34.2645 | 145 | 357 | 10984 |
| driven_reserve45_02 | 11293 / 11292 | 235.6102 | 33.6899 | 198 | 356 | 10948 |
| driven_reserve45_03 | 11292 / 11291 | 235.5817 | 35.2490 | 232 | 367 | 11116 |

EachinitialDFAseedwas separatelyverified againstDS/DX. Eachnewrecoveryseed1448
ticks,12intervals/sevencycles. Actualsecondarmremaining70/68/70ticks=35/34/35us,
cost14/13/14us. This remains only2..3usabove32usfloor: donot extrapolatefaster
recovery. Resumedwindows~7.988s,original10sbudget retainedincludingstop/wake/
reacq. Firstarchivesfault8 andacceptedage-stopboundverified; no poststoprecords,
electrical/driver/stacklimits pass. Stackuntouched4908each,sameboothighwater.

_02/_03POWERPATHreason0 withCOASTREFstop1 is normalforeground deadline stop;
_01reason2 istimerguarddeadline. Bothvalidalready-definedcompletionpaths,
notmissingfaultreports. COM=accepted can occurwhenlastacceptedCOMiscancelled
atdeadline; notindependentqZCproof. Current valuesremain rawphasepeaks,notmA.

Allfixturesverifiedfinalgates/MOE/CCRs/ENABLE0,nFLT1;COM41closed,no livefixture.
168Python/140Rusttestspassincludingall3realrecoveries andreserve-marker
mutations. Bothnormal/optionalrelease/s/thin-LTObuildspass. Installedoptional
text96252,data956,bss28704,SHA256
6A2B4D5C7326643F258B7F5308C1132CC80CC2B0EA5E74DABCCDFDF0BE426100.
RootELFlastrebuildNORMAL,notflashed. captures/driven_reserve45_cohort.csv
recordsall3. Olderstartupcurrent/missingepoch/latearmfailuresremainvaliddata.

Goalstillopen: moreequivalent4.5%repeatsaren'tthenextneed. Continuecurrent/
timingandarchiveparity measurements,thenstagedduty-ledexpansionwithguarded
recoveryandidentifiedlimits. Do notforce30%orclaimarbitrarydisturbancerecovery
fromthisoneinjectionclassatoneoperatingpoint.

---

## Entry 230 — 2026-09-13 — Replayable timing audit, not CPU occupancy

Added scripts/drv_timing_report.py and TIMING_HEADROOM.md. Replays all three
E229 captures through the complete driven/recovery verifier before reporting
timing, input SHA256 and separate initial/recovery arm margins. No new motor
run, flash or serial access. Previous release-profile verification completed
the user's build request but did not expand the campaign envelope; this turn
adds tested timing evidence extraction for the next instrumented experiment.

Recovered COMP call rate12828.7..12845.3/s, ADC scans4200.9..4208.3/s. Wall
maxima COMP82us/COM55us, guard20/4/4us and commit21us. Source review confirms
COMP/COM timers start after dispatch/status checks and omit early returns and
root/exception overhead; nested preemption can inflate wall brackets. Thus
maxima multiplied by rates neither measure saturation nor CPU headroom.
Report explicitly returns utilization=null and does not qualify faster drive.

Recovery margin above32us is3/2/3us; initial margin24/25/25.5us. Actual arm
cost leaves2/3/2us under16us maximum. Distinguish those from original-end
termination margins145/198/232us. These point to recovery and instrumentation
latency as separate constraints, not an arbitrary250eHz goal ceiling.

Four new tests cover all three real recoveries, initial segment, duplicate/
missing/synthetic timing summaries and rejection of E228's6us-late recovery.
Full Python suite172/172 passes. Firmware unchanged. Rebuilt root optional
handoff ELF matches installed E229 SHA256
6A2B4D5C7326643F258B7F5308C1132CC80CC2B0EA5E74DABCCDFDF0BE426100.
Release opt-level=s, thinLTO, one codegen unit; no z runtime qualification.

Next: whole-vector/nesting-aware runtime accounting with measured probe cost,
then staged expansion with current/tracking evidence. Foreground ADC work must
not disappear from a CPU-utilization claim. Goal remains active/incomplete;
this audit neither supplies calibrated current nor independent rotor parity.

---

## Entry 231 — 2026-09-13 — Optional nested IRQ meter, not yet bench-qualified

Implemented bench-cpu-timing: all six motor vector software bodies have RAII
entry/exit accounting, including early returns. Pure irq_accounting module
partitions TIM17 elapsed time among foreground and nested IRQ contexts under
serialized reads; no double-counting. It rejects visible>1ms gaps, bad nesting,
600s/counter overflow. These invalidate measurement only, not motor authority.
Whole65ms blackout aliasing remains explicitly unsupported.

Reset occurs with powered statistics. Start is first powered guard tick,
excluding acquisition/initialguardperiod and avoiding reset work at seedarm.
Stop freezes on poweredowner termination even inside nested guard. LaterDrop
cannot mutate frozen evidence. Recovery resetsmeter, so report is lastsegment,
not both segments or completecampaign. Optional inactive wrappers stillcost
cycles during acquisition, and meter finish adds stop-pathcost: benchmeasure
before interpreting or wideninganything.

CPUMETER plus seven CRC CPU85 rows are fixture-only, no live UARTspam.
drv_cpu_meter validates header, uniquecontexts, CRC, stoppedstate and sumtime.
Sustainedreporter validates these optionalrecords wheneverpresent. Context0
is foreground/unattributed NOT idle; no CPUutilizationpercentage. Hardware
exception and probeoverhead are not separately measured. Shared meter snapshot
is copied underinterruptserialization before UART, avoiding referencealiasing.

Added idle cpucheck, refuses unless gates and ENABLE are alreadyoff. Measures
256 inactive/active/nested softwareentry-exit pairs, then clears syntheticstats.
Not runonhardwareyet. Next measure actualcost and inspect addedstoplatency;
only then a finite known4.5%powered qualification, not a faster-duty experiment.

FiveRust tests cover nestedpartition, stopsinsideIRQ, latenonmutatingreturns,
clockwraps10s, invalidnesting/gaps/overflow. ThreePython tests coverwireidentity,
sumconsistency, duplicate/malformed/CRC records andinvalidgap evidence.
145Rust/175Pythonpass, M0no_stdlibcheckpasses, normal andoptional release/s/
thinLTO builds pass. Instrumented text98040,data956,bss28784,SHA256
5CE5A70A46D1EE867338B91427CC92F90B319B095AE3E780CC54781DB8E9E311.
RootELF isthisexperimentalbuild. InstalledfirmwareremainsE2296A2B4D5C...;
no flash,serialor motoroperation thisturn. No current/duty/speed/arm/deadline
limitschanged. Goalremainsopen; this is instrumentationimplementation,
not measuredCPUheadroom or expandedmotorqualification.

---

## Entry 232 — 2026-09-13 — Hardware idle CPU probe overhead fails

Previous instrumentationimplementation was progress; this turn qualified its
cost before motoruse. PreflightCOM41 off/p/i verified allgates/MOE/CCRs/ENABLE0,
nFLT1 and no competingprobe/fixture. Flashed5CE5A70A... release/s/thinLTO.
Declared gate beforetest: all3modefaults0 and measuredmaximum<=10us.

cpu_overhead_01: inactive sum592us/256,max3;active2128/256,max9;
nested4288/256,max17. Allfault0,finaloffverified. FAIL overheadgate.
Inlining hotaccountingfunctions and bitmask insteadofstackmembershipscan,
sameguards/algorithm invariants, then145Rusttestspass andnewreleasebuild.

Flashed C43B672BC2181BC72304B822B836D41DF3EDC85A7B2F594DB667703CC2026C11,
text98248,data956,bss28784. cpu_overhead_02 UARTsilent failedpreflight,
retained. Debugreads RCC08000000,PD1ODR0,BDTR00000c1a,CCRs0 confirmedexact
knownsafestate beforeRCC08040000UARTclockworkaround. Notpermanentresetfix.

cpu_overhead_03: inactive552/256,max3;active2104/256,max9;
nested4240/256,max17. Allfault0/finaloffverified. StillFAIL unchangedgate.
Nestedcase containstwo softwarepairs,but do not change gateaftermeasurement
to call17us a pass. Negligibleimprovement means moreinlinehints aren'tthe
nextaction. ScopeDropdisassembly retains significant bookkeeping/spills;
next lighter accountingdesign with same nested/clock/overflow correctness.

Added saved-idlefixture drv_cpu_check.py and3tests preserving bothrealfailed
measurements asfailures, malformed/syntheticboundarytests andUARTsilentrefusal.
Finalsafing validatesbefore timingclassification. 178Python/145Rustpass.
No motorstart thisturn. Installed/rootremainexperimentalC43B672B...,NOT
motor-qualified. COM41closed,noprobe/fixturealive,lastoutputsdisabled,nFLT1.
Goalremainsactive: actualruntimeCPUpartition andcurrent/envelope/parity work
remain, and the failed instrument must not be used to widen operatinglimits.

---

## Entry 233 — 2026-09-13 — Specialized probe reduces overhead, still fails

Const-generic vector Scope<ID> replaces runtimeID, permitsconstant exitindex
and removesdynamicboundsbranches. reprC puts hotstate atshortoffsets.
cpu_overhead_05 onSHA34AF6FF5...: inactive520/256us,max3;active1520/256,max6;
nested3024/256,max12. Clear improvement from17us, stillFAIL <=10us gate.

Then packed sixIDs into18bits and changedleave tochargevalidatedknownID,
retainingallnesting/clock/overflowguards and7contextpartition. Newfull-depth
unwind/reentrytest supplements existingtests. cpu_overhead_07: inactive
512/256,max2;active1528/256,max6;nested3064/256,max12. No materialgain.
Allmeasurementfaults0, finalgates/MOE/CCRs/ENABLEoff,nFLT1. No motorcommands.
_04/_06UARTsilentpreflightfailures retained; eachreadRCC08000000,PD1ODR0,
BDTR00000c1a,CCRs0 beforeknownRCC08040000workaround. Notpermanentfix.

146Rust/178Pythonpass,includingpreservedfailedcapturesandall6nestedcontexts.
Installed/root instrumentedSHA256
7B09BD5259A2C07AAA09A8B103D9336F9B6CA5F6E47A040295441FCA76223585,
text99580,data956,bss28784. Release/s/thinLTO. Notmotorqualified.
COM41closed,lastfinaloffverified. Goalstillactive. Next considerseparate
IRQ-union accountatouterboundaries toreducecoststructurally: aggregateIRQtime
isusefulwithout7wayattribution. Musthaveexplicitnewprotocol/tests,notreuse
CPU85withdifferentmeaning. Foregroundnotidle; noCPUutilizationmeasuredyet.

---

## Entry 234 — 2026-09-13 — IRQ-union meter qualified in real powered holds

New optional bench-cpu-union uses irq_union::Meter, charges outerboundaries
only, validatesnestedidentities andcounts nestedworkonce. Distincttwo-context
CPUUNION/CU85 CRCprotocol; legacy7contextCPU85notreinterpreted. Hostrejects
mixedheaders/rows. Testscompare10sunionagainstoriginalpervectorpartition,
all6nestedIRQs, stopsinsideguard, invalidnesting, visiblegaps, overflow, and
intentionalzero timestampsatnestedentry/exit. Normalmotoralgorithmunchanged.

Firstunionbuild4BF1B763... idle_overhead_02max2/7/11us fails unchanged10us
allmodegate. _01UARTsilentpreflight retained; exactRCC08000000/PD1ODR0/
BDTR00000c1a/CCRs0 beforeUARTclockworkaround. OmitnestedTIM17reads andavoid
unneededpoweredownerreadonstartedmeter. NewbuildCAD4FB0C... resetUARTworked;
cpu_union_overhead_03max2/7/10,means1.5625/6.375/9.875us,fault0allmodes,
CPUCHECKTYPEunion1,finaloff verified: PASS predeclaredgate. Priorfailuresstay.

Disabledtransfercheck8/8,poststop18/18,recordcheck12/12pass. Guarddiagnostics
stop1009/1009/309us withsyntheticfeedback, alloutputsdisabled. Thenannounced
first1s poweredqualification atknown4.5%/+60, followedby10s afterfirstpass.
Same6.2%startup/50->200eHzforcedramp, user800mAPSUsetting, no guardrelaxation.

cpu_union45_01 full1s233.545eHz,1401COM/1400accepted,cycle sigma57.93us,
CPUelapsed1000008,IRQ522718,foreground477290,22375calls:52.271%softwareIRQ.
cpu_union45_02 full10s234.449eHz,14067/14066,cycle sigma41.10us,
CPUelapsed10000010,IRQ5278743,foreground4721267,224901calls:52.787%softwareIRQ.
Bothmeterfault0,maxgap95/97us,maxdepth2,CRC/sumvalid. Rawpeaks347/359,
busfloors11151/10937mV,stackuntouched5160. Seedarm49/36.5us,cost12/13us.
10sCOMPmax85us,COM58us,guard22us,commit24us,36303ADCscans. No dropout/recovery
requestedinthesetwoholds. Bothcompletefullhandoffverificationandfinaloff/nFLT1.

MeasuredIRQfraction includesprobeeffects, notcalibratedtotalCPUutilization.
Foregroundisnotidle. No subtractionofidlecheckscalar toinventcorrectedCPU%;
exceptioncostnotseparate. Current remainsrawpeak,no newPSUread/calibratedmean.
Not independentqZCparity orexpandedrange. Thisinstrumentnowyieldsevidence;
nextmove recovery/current/envelopework ratherthanmoreidleprobetuning.

181Python/151Rusttests pass,M0no_stdlibcheckpass,release/s/thinLTObuildpass.
Installed/rootSHA256CAD4FB0C7AE48939A377AA94F8EC44E5AB0B10387C83061B01B874A840880AC1,
text99728,data956,bss28740. COM41closed,noliverunhandle,lastoutputsverifiedoff.
Goalactive/incomplete. Preserveallfailedpreflights/overheadattempts; newpasses
do not retrofitqualification ontoearlierinstrumentbuilds orprovewideenvelope.

---

## Entry 235 — 2026-09-13 — Instrumented recovery, duty-led extension to266eHz

Revalidated currentgoal/installednotes/no competingfixture. First10s4.5%
injectedtrackingloss+onerecovery onCAD4FB0C... build. cpu_union_reentry45_01
passes: first2807COM/2806events,Tracking8stop;fresh12intervalseed1446ticks,
secondarmremaining64ticks=32us EXACTfloor,cost13us. Resumed~7.988s234.298eHz,
11230COM/11229events,sigma38.396us,raw357,bus10674mV,stack4864. Original
deadline14711235 vsfinal14711076:159usspare. CPU4195593/7987912us=52.524%,
fault0. First/recoveryarchives andactualseedidentitiesverifiedseparately.

Next4.6%10s withnormal250profile: cpu_union46_01 stopsCycleTiming12 at
1.244s,1788COM/1787accepted,mean239.499eHz,sigma62.797us,raw363,bus10948,
CPUfault0. Loweraverage doesnotcontradictindividualcycleboundfailure. Retained
asfailure,notfullwindow andnotproofactualrotorspeedexceeded250eHz.

Announceddeliberateexistingbench-range300 profile forstagedsustainedtests:
cycleminimum4000->3333us,event333->277us,coordinatedpassiveacq8000/666->
6666/554ticks. Electrical/rawcurrent/bus/nFAULT,1mstracking,200ustickgap,
32usarmfloor,originaldeadline andhard30%dutypermission unchanged. Preceding
4.5%holds/recovery and4.6%electrical/IRQfeedback justifiedboundednexttest,
notunrestrictedexpansion. No higher-speedrecoveryclaim.

151Rust rangeprofiletests/buildpass. FlashedD9E8C1C...;resetUARTworked.
cpu_union_range_overhead_01max2/7/10us,fault0 andfinaloffpass.
Sequential10s tests, eachnextstepannouncedafterprecedingvalidhold:

| Capture | Duty | Mean eHz | COM / accepted | IRQ % | Cycle sigma us | Raw peak | Bus min mV |
| --- | --- | --- | --- | --- | --- | --- | --- |
| cpu_union_range46_01 | 4.6% | 240.1958 | 14412 / 14411 | 52.5026 | 43.2197 | 368 | 10972 |
| cpu_union_range48_01 | 4.8% | 252.7915 | 15167 / 15166 | 52.3406 | 45.6416 | 389 | 10769 |
| cpu_union_range50_01 | 5.0% | 266.0952 | 15965 / 15964 | 52.9098 | 46.8264 | 383 | 10913 |

All3fullwindow/CRC/timeline/CPUfault0/electrical/finaloffverified. Actualinitial
arm49/40/45.5us,cost13us;stack5176/5164/5140. At5%COMPmax89us,COM58us,
guard23us,commit24us,36886ADCscans. No CPUcorrectedutilization:IRQfraction
includesprobeandforegroundnotidle. Currentstillrawpeak,notcalibratedmean.
TheseareONE sustainedpassperpoint,notrepeatability orhigher-pointrecovery.

182Python/151Rustrangeprofiletests pass. All5motorattemptsthisturnretained
(recoverypass,normalprofileguardstop,3rangeholds),allfinaloutputsdisabled/
nFLT1. No livefixture;COM41closed. Installed/rootSHA256
D9E8C1C4CFF7F15AA86187CD381806818498A83DB1AC065A2D2E1032B558F4CE,
release/s/thinLTO,featuresbench-driven-handoff,bench-cpu-union,bench-range300,
text99752,data956,bss28740. Goalactive: nexthigher-pointrecovery/current
characterization andstagedrepeatability/expansion;do notforce30%orclaimparity.

---

## Entry 236 — 2026-09-13 — Recovery240eHz, fresh-window acquisition option

cpu_union_range_reentry46_01 onD9E8C1C... passes original10sbudget+2sinjection:
resumed240.525eHz,11529COM/11529events,sigma41.11us,seed1413ticks,remaining
77ticks38.5us,cost13us;deadline137usspare. Raw368,bus10698mV,stack4872.
CPUelapsed7988706,IRQ4223049,foreground3765657,fault0. Firstarchive2879COM/
2878events,Tracking8,raw367,bus11032. Fullrecovery/archives/finaloffpass.

Next4.8% cpu_union_range_reentry48_01 failedbeforehandoff: epoch1accepted,
epoch2missing,epoch3..24orderedafterward. ExistingQualificationlatchedfault2
atfirstgap andneveradmittednewhistory. No poweredrecoveryattempt inthisfile;
20msforcedobserverexpiredsafely. Rawfailure retained.

Optional bench-driven-reanchor now allows ONEfreshqualificationwindow after
forwardepochgap2..6consistentwithphysicalstep. Requiresvalidbracket/time,
no completedseed; duplicates/backwardepochs/secondgap/fataltimingrefuse.
RecreatesRuntimeAcquirewithORIGINALstart,discardsoldintervals, requiresfresh
13edges/12intervals/sevencycles. No timerextension,edgerefresh,gapjoining,
minzfilterchange ornewpowerpermission. DRIVENSEEDRESTART exposescount/anchor;
hostrequirescorrespondingfirstr awgap,pre-gapvalidity andfreshcontiguouswindow.
Legacyunmarkedrecordskeepoldsemantics. OfflineRust/Pythonreplay ofactual
failedtrace qualifiesepochs3..15,seed1566ticks,edge24930half-us; oldhardware
capturestillfailsfullmotorverification. This isnotretroactivehardwareproof.

Release/s/thinLTOnewbuildSHA256
28D440C07A444B6219D01862E16B66FCCE543A70CE0C54DCAAB206F87713709C,
text100100,data964,bss28740. Flashed;UARTresetworked. Disabledcpucheck
reanchor_overhead_01max2/7/10us passesunchangedgate/finaloff.

reanchor_reentry48_01: initialhandoff succeeds(seed1585,remaining46us,cost13),
drives2s3026COM/3025events theninjectedTrackingstop. Freshpassive12interval
recoveryseed1338ticks,actualedgeage278ticks leaves57ticks28.5us,below32us.
CORESEEDarmed0,remaining_arrsentinel,zerosecondsegment; strictverifierFAIL.
No recoverypowergranted. Reanchorcount0,so newrestartbranchremainsbench-unproven.
Firstraw355,bus10913;finaloutputs/MOE/CCRs/ENABLEoff,nFLT1verified.

SEEDLAT114/146/160/246/254half-us showsguardsetup86ticks43us afterfeedback.
Source start_inner stilldoesgates_off/reset_run_statistics onpassiverecovery;
resume_once preparesperipherals BEFOREacquisition butnotthese statistics.
Nextinvestigateearlystatisticsstaging whilepreservingTrackingreason8,
firstarchive,stop/cancelownershipandoneattemptbudget. Do notlower32usfloor
orrefreshseed. Runtimeprobeinit/resetmaycontributetothisstage;measurenotassume.

186Python/154Rustrangeprofiletestspass. Installed/rootnewoptionalbuildabove,
COM41closed,no runningfixture,lastfinaloffverified. Threeattemptsthisturn
retained(46recoverypass,48pre-handoffrefusal,48late-recoveryrefusal).
Goalactive/incomplete: reliableentry/recoveryathigherpoints,current/parity/
repeatability remain. No forced30%target orclaimofuniversalrecovery.

---

## Entry 237 — 2026-09-13 — Early statistics preparation restores recovery margin

Optional bench-reentry-staging moves recorder/coverage/CPUstatisticsreset out
of the freshseed path. Important sourceordering: acquire_inner calls shared
gates_off itself, so staging inresume_once beforethatwouldberevoked. Instead
stage immediatelyAFTERacquisition'sfinalshutdown,BEFOREitsfirstmeasurement,
onlyawakeTracking8withoutputsdisabled. Saves/restoresreason8andSTOP_US;
clearingothersegmentstats is afterFIRST_SEGMENTarchive exists. Newguardstill
builtfromactualfreshsample/age/seed/limits afteracquisition.

REENTRY_STATS_STAGED isone-shotoptimization,notoutputpermission. stopclears
it evenwhenalreadyinactive; start_inner consumesitbeforeanyrefusal. Onlyawake
resumeSome/nonadoptedpath skipsduplicategatesoff/reset; clearsreason/STOPat
admission. Allold guard/sample/seed/absolute-budget requirementsremain.
REENTRYSTATS used=1 preparation_before_acquisition=1 gate_authority=0 exposes
use. Fullhostverifierchecksnewmarkerifpresent; legacycapturerulesunchanged.

Flashed63801C85... release/s/thinLTO. reentry_stage_overhead_01UARTsilent
preflightretained; exactRCC08000000/PD1ODR0/BDTR00000c1a/CCRs0 confirmedbefore
knownRCC08040000UARTworkaround. _02overheadmax2/7/10 passes. Idle
reentrystatscheck3/3 verifieswrongreasonrefusal, preservedTrackingprovenance,
inactive-stoprevocation withgatesdisabled. Transfer8/8,poststop18/18,record12/12
pass. No syntheticdiagnostic grantsoutputauthority.

reentry_stage48_01 completes4.8%recovery252.823eHz,12119COM/12118accepted,
raw369,bus10996,stack4856,originaldeadline118usspare. Actualseed1345,
edgeage246,remaining90ticks45us,cost13us. SEEDLATguardstage158->214=56ticks
28us, versusprevious160->246=86ticks43us:15ussaved. Thispasseswithout
changing32usfloor orrefreshingseed. Reanchorcount0 onthisrun.

Thenfirst5.0%recoveryplusTWOannouncedsame-settingconfirmations:

| Capture | Resumed eHz | COM / accepted | Raw peak | Bus min mV | Recovery arm us | Original deadline spare us |
| --- | --- | --- | --- | --- | --- | --- |
| reentry_stage50_01 | 266.0281 | 12752 / 12751 | 391 | 10937 | 36.5 | 159 |
| reentry_stage50_02 | 266.0756 | 12754 / 12754 | 388 | 10972 | 34.5 | 221 |
| reentry_stage50_03 | 266.0721 | 12754 / 12753 | 383 | 10841 | 36.5 | 97 |

All3startup->injectedloss->recoverycompletewithinoriginal10sbudget,~7.989s
resumed. Allguardstage28us,armcost13us,untouchedstack4856. CPUunionvalid,
~52.4..52.8%includesprobe; foregroundnotidle. NotcalibratedCPUorcurrentparity.
_01alsoexercisesREALreanchorcount1anchor_epoch3: oldearlygapdiscarded,
fresh13orderededges qualify1548tickinitialseed at24506half-us; actualhandoff
andlaterrecoverybothpass. _02/_03reanchorcount0. Thisisfirstbenchproof of
freshwindowbranch,notjustoffline replay. RawDIgapandDS/DX/DFAidentityhostchecked.

Fourmotorattemptsthisturnallpass,allfinalgates/MOE/CCRs/ENABLEoff,nFLT1.
COM41closed,nolivefixture. 188Python/154Rustrangeprofiletests pass;normaland
experimentalreleasebuildspass. Installed/rootSHA256
63801C8567644EC6327576F3AEECB75B7C51DE87C98F6E46213EB62248D94C36,
featuresbench-driven-reanchor,bench-cpu-union,bench-range300,bench-reentry-staging,
text101068,data964,bss28740. Goalactive/incomplete: moreequivalent5%runsnot
nextpriority; advancecurrentcharacterization/furtherstagedrange andportable/
archiveparityconsolidation. Higherrecoveryisnotinferredfrom2.5..4.5usspare.

---

## Entry 238 — 2026-09-13 — 278eHz sustained boundary, current-anchor opportunity

Onunchanged63801C85... build, nextduty5.2% staged_range52_01 completes10s at
278.1656eHz,16690COM/16689accepted,sigma39.582us,rawpeak391,bus10913mV,
stack4856. Initialseed1551,arm44us/cost13us;reanchorcount0. CPUunion
5378512/10000008us=53.785%,validfault0. Electrical/tracking/profileguards
remainunchanged; dutyexpansionwasannouncedafterprevious5%repeatqualification.

staged_reentry52_01 thenattemptsoneinjectedTrackingrecoveryinside10sbudget.
First3330COM/3329accepted,raw392,bus10925;freshpassive12intervalseed1212
half-us ticks. Actualedgeage246 giveswait303-age246=57ticks28.5us<32floor.
CORESEEDarmed0,zerosecondsegment. Refusalretained,finaloffverified. Thus
5.2%/278eHzsustainedpassedbutrecoveryNOTqualified;5.0%3/3resultstillseparate.
Guardstagealready28us; do notlowerthearmfloor orrefreshseedtoeraseboundary.

Switchedto currentcharacterization. Re-read E185/CURRENT_MEASUREMENT_PLAN:
standalonezerocheck can'tcalibrateanewENABLEepoch. No newzero oroffsetwas
applied. AskedoperatorasynchronouslyforPSUcurrent/voltageduringannounced
30s5.0%hold. current_anchor50_01 completes30s266.3187eHz,47937COM/47936events,
sigma42.164us,raw403,bus10901mV,stack4856,initialarm35us/cost14.
CPUunion15561338/30000010usvalid. No operatorreadingreceivedasyet,so this
fileisNOTanindependentcurrentanchororcalibratedmeanmeasurement. Historical
~70mAat238eHzand800mAsettingarenotreplacements. Keeprequestunresolvedwithout
blockingunrelatedgoalworkorrepeatingequivalentholdsblindly.

PORTABLE_WINS consolidatesopaqueTransfer/completedscan-yield,freshreanchor,
earlystatsstaging,IRQ-unionandseparateinitial/recoveryseedarchivecontracts.
No siblingrm32/minzeditclaimed. Frozenreference--check-source passesall128
files,archiveSHA2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44,
HEAD2efda4911c4be8e238e16d192d821151bd775887. No referencehardwarerun.

189Python tests pass,includingnewholdandlaterecoverynegativecase. Firmware
unchanged,installed/root63801C8567644EC6327576F3AEECB75B7C51DE87C98F6E46213EB62248D94C36.
All3motorattemptsretained,eachfinalgates/MOE/CCRs/ENABLEoff,nFLT1;COM41closed.
Goalactive/incomplete: mean-currentmethod/independentanchor,archiveassessment
andqualifiedenvelope workremain. Identifiedrecoverylatencyboundaryis evidence,
notfullobjectivecompletion orreason toinventamaximumdutygoal.

## Entry 239 - 2026-09-13 - Remove post-seed diagnostic copies (offline)

Compiled E237 path confirms180bytes of report/failure copy-out and restoration
after acquisition. flying_bench now selects initial/recovery diagnostic storage
at compile time and writes recovery evidence directly. Initial report/failure/
edge remain untouched; selection, sector, pending and disabled baseline are
still restored. Awake baseline retains original ADC timestamps. Qualification,
20us confirmation, control,32us arm floor and all shutdowns remain unchanged.

E238's5.2% seed1212/age246 leaves28.5us: needs at least3.5us reduction just to
reach the unchanged floor at that same seed. No predicted/claimed timing win:
entry57us includes required dwell and other work, not merely diagnostic copies.
See TIMING_HEADROOM.md for compiled addresses and validation requirements.

Normal and experimental release/s/thinLTO builds pass;189Python/154Rust tests
pass but do not directly execute this firmware diagnostic-routing change.
Next disabled archive/refusal isolation check, then timed same-setting recovery.
Specialization increases text1708bytes; reacquire inlines acquisition into
492-byte frame, requiring renewed stack measurement before extrapolation.
Root experimental SHA DA6B5F4B45EA0A4CEF98BC8F9BD86438D9C97FB74423B7538193732C4E3EEF79,
text102776/data964/bss28740. NOT FLASHED. Installed remains E23763801C85.
No UART/probe/motor commands; no new hardware safety-state observation claimed.
Goal remains incomplete;5.2% recovery remains unqualified.

## Entry 240 - 2026-09-13 - Archive routing works; acquisition regression remains

Added disabled-only archivecheck under bench-reentry-staging. It uses actual
publication helpers to check isolation both directions, then actual awake
reacquire while ENABLE is off to exercise refusal8 and initial-state retention.
It restores diagnostic snapshots. This does not prove successful live routing.
scripts/drv_archive_check.py reserves raw output, verifies preflight and finally
off, requires3/3 checks on each of3 trials. Both compiled variants pass.
CPU overhead checks on both builds pass unchanged maxima2/7/10us.

Initial build FE64F844A341069D75CC4619E26B8AF03DB19B9344D3C937F9455E5D3E230C43:
archive_reentry50_01 and _02 each start at5.0%, drive until2s injection,
stop on Tracking and refuse passive recovery with result7. Comparator phase
maxgap202half-us=101us exceeds unchanged100us EdgeFilter limit. They qualified
only3 and2 intervals respectively: NO fresh seed, NO powered recovery.
First _01 archive3181COM/3180events,peak381raw,bus10984mV.

Narrow test: mark acquire_inner inline(never), keeping diagnostic routing and
all guards unchanged. archive_noinline_reentry50_01 ALSO refuses gap202 after
6intervals/4658us. Initial3183COM/3182events,peak378raw,bus11080mV. This rejects
the hypothesis that preventing wrapper inlining alone fixes the regression.
No arm-latency saving is measured. Do not proceed to5.2% or relax gap guards.
Next isolate the changed acquisition-loop timing against the pre-refactor path.
The full verifier's immediate error is staging provenance (used0), but raw
RECOVERYACQ establishes that acquisition refused before start_reentry.

All3 powered attempts finalgates/MOE/CCRs/ENABLE0,nFLT1;untouchedstack4784.
Two initial flash preflights were UARTsilent, retained as archive_routing_01
and archive_noinline_check_01. Each exact safe register set RCC08000000,
PD1ODR0,BDTRc1a,threeCCR0 was checked before known RCC08040000 UART workaround.
Successful disabled captures archive_routing_02/archive_noinline_check_02,
overhead archive_cpu_01/archive_noinline_cpu_01. UART closed afterward.

Installed/root C4F26BF28A73525DE526FBCC25B82C6001C18278687DF0D63C6D888EB98EF116,
text104960,data964,bss28740;release/s/thinLTO and reanchor,cpu-union,range300,
reentry-staging features. Experimental build/189Python tests pass. Installed
image is NOT recovery-qualified; previous E237 results remain historical.

## Entry 241 - 2026-09-13 - Shared acquisition comparison; console issue

Runtime recovery destination now passed into one non-inlined acquire_inner,
instead of two const specializations. nm confirms one compiled function.
Direct archive publication retained. archive_shared_check_02 passes3x3;
archive_shared_cpu_01 passes maxima2/7/10us. _check_01 UARTsilent retained;
known clock workaround after exact safe registers restored that build.

archive_shared_reentry50_01:5.0%,2s initial drive3176COM/3175events,
peak410raw,bus10996mV; tracking injection stopped drive. Recovery reached
11intervals/7904us then refused gap202half-us (101us>100us). All outputs off,
nFLT1 at final readback. Single shared function does not eliminate failure.
No measured seed-arm improvement, no higher-duty attempt.

Next source-grounded cost reduction: outputs_disabled now tests identical
GPIOA gates7..10 mask0x780 and GPIOB0..1 mask3 with one read per port,
plus unchanged MOE test. ENABLE/nFAULT checks stay separate. Exhaustive
65536values per port comparison against original boolean pin tests passes.
This is snapshot-value equivalence, not a timing/hardware qualification.

New mask build will not answer UART. archive_mask_check_01/_02,
archive_mask_cpu_01 and archive_mask_reentry50_01 are ALL preflight failures,
not motor runs; retained. Subsequent fixture invocations should have been
gated on each prerequisite success; their own preflight prevented commands.
Clock initially08000000,PD1ODR0,BDTRc1a,CCRs0. Known clockwrite yields08040000
but still silent,USART3 CR1/BRR read0. Safe register readbacks unchanged.
Brief halt/read/resume PC0800c7cc maps shell main,not a boot wait. Debug CLI
failed its DAP argument serialization before useful attach. No permanent
UART diagnosis yet; investigate initialization before more fixtures.

Installed/root460C0BD89A64ECF29E3D9A06F2CD3BFCFD87271AA38F559A9681C7799D9A561B,
text104208/data964/bss28740,release/s/thinLTO and same four experimental
features. Registers show bridge/ENABLE off, but no fresh UARTfinaloff on this
build. No live fixture. This image is NOT hardware-qualified. Goal incomplete.

## Entry 242 - 2026-09-13 - Console recovery; 5.2% repeated recovery passes

probe-rs verify matches E241 ELF. Read-only register checks show no held reset,
USART3 pins PC10/11 AF0 configured,clock enabled but BRR/CR1 zero. Local HAL
configuration confirms64MHz/115200 gives555=0x22b,8N1 UE/TE/RE givesCR1=0xd.
After confirming PD1ODR0,BDTRc1a,threeCCRs0, wrote ONLY BRR andCR1 to restore
console. archive_mask_restored_01 passes3x3 actual archive/refusal checks and
finaloff. archive_mask_cpu_02 passes2/7/10us maxima. This is a demonstrated
recovery workaround, NOT an explanation or permanent fix for lost init state.

archive_mask_reentry50_02 (first actual mask-build motor attempt) passes5.0%
10soriginalbudget,2s loss injection,~7.99s recovery at265.7919eHz. Freshseed1267,
age210half-us,remaining107ticks53.5us,cost13us;gap170ticks85us. Raw395,
bus10937mV,stack4768. Deadline253usspare. Contrast failed shared-unmasked trial
at101us gap: masked gate checks remove that observed sampling failure.

Then fixed initial+two-confirmation cohort at5.2%,same startup6.2% and+60phase:

| Capture suffix | Mean eHz | COM / accepted | Cycle sigma us | Raw peak | Bus floor mV | Recovery arm us | Deadline spare us |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| archive_mask_reentry52_01 | 278.2563 | 13339 / 13338 | 30.4283 | 398 | 10948 | 46.5 | 95 |
| archive_mask_reentry52_02 | 278.7390 | 13362 / 13362 | 29.6977 | 386 | 11032 | 47.0 | 114 |
| archive_mask_reentry52_03 | 278.4644 | 13349 / 13348 | 30.5672 | 404 | 11104 | 45.5 | 174 |

ALL3 full fixture verifications pass: initial under-drive handoff, injected
tracking stop, fresh12interval/sevencycle recovery, separate archives,original
10sdeadline and outputs-off/nFLT1. Recoveryseeds1212/1215/1211 and actualages
210/210/212ticks,cost13/14/13us. Acquisition gaps83/85/84us. _01 has SAME1212
seed interval as E238 late refusal, but age210 instead of246:18us younger,
46.5us rather than28.5us remaining. Arm floor32us unchanged. This improvement
includes report routing/shared-loop/masked checks; not one isolated attribution.

SEEDLAT _01/_02=98/118/134/178/186half-us;_03=98/120/134/180/188.
CPUunionfault0,maxgap96/96/97us,nesting2,partitionvalid; no CPUutilization claim.
COMPwallmax85,COM58,guard23,commit24us; all4motorattemptsstackuntouched4768.
Raw peaks are not calibrated mean-current. No new independent PSU reading.
scripts/drv_timing_report.py --reentry revalidates all3 captures and margins.

No flash or firmware edits this entry; installed/root still
460C0BD89A64ECF29E3D9A06F2CD3BFCFD87271AA38F559A9681C7799D9A561B,
release/s/thinLTO,text104208/data964/bss28740. Console closed,all4finaloff.
The E241 preflight failures remain distinct from these actual motor attempts.
Next calibrated-current/parity/staged envelope work, not more equivalent5.2%
repeats. Goal incomplete; no demand to force30%,no guard expansion this entry.

## Entry 243 - 2026-09-13 - Duty boundary: 5.3% passes, 5.4% cycle guard

E244 correction:3329us below is the recorder's whole-stream minimum,not the
actual rejected guard interval. Those timestamps are sampled separately and
the guard-rejected event is omitted. CycleTiming stop is real; its exact
decision delta was not captured. Do not equate these two pieces of evidence.

Same installed E241/E242 release/s/thinLTO460C0BD8 build,11.7V operator supply
setting/800mA limit unchanged; no new PSU current reading. No firmware/flash
changes. After E242's three5.2% recoveries, test5.4% under unchanged range300.

mask_range54_01: requested10s,stops at636869us on CycleTiming12.1099COM,
1098accepted,rawpeak405,busfloor11056mV. Full-cycle moments show minimum3329us
below3333guard,mean3475.17us (~287.76eHz),sigma116.76us including acceleration.
One short accepted-event cycle is not proof of actual rotor speed>300eHz.
Retained FAIL; no guard change or5.4% recovery trial.

Bisected duty to5.3%. mask_range53_01 full10s hold passes284.9606eHz,
17097COM/17096accepted,cycle sigma44.873us,raw403,bus10925mV.
Then announced fixed first+two confirmation recovery cohort:

| Capture | Recovered eHz | Cycle sigma us | COM | Raw peak | Bus floor mV | Arm remaining us | Deadline spare us |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| mask_reentry53_01 | 285.4628 | 35.0568 | 13684 | 404 | 10877 | 43.5 | 219 |
| mask_reentry53_02 | 286.2717 | 34.7328 | 13724 | 399 | 10948 | 43.5 | 164 |
| mask_reentry53_03 | 285.2342 | 34.6075 | 13674 | 394 | 10984 | 43.0 | 120 |

ALL3 full verifications pass: guarded under-drive entry,2s tracking injection,
shutdown,fresh12interval/sevencycle recovery and remainder of original10s.
Seeds1189/1187/1185,actualages210ticks,armcost14us against16us; acquisition
gaps85/85/84us. Cycle minima3343/3351/3346us are only10/18/13us above guard.
CPUunionfault0,maxgap96us,stack4768. All5actualattempts finaloff/nFLT1,
COM41closed. Timing reporter revalidates all3 recoveries.

Immediate expansion boundary is short-cycle guard, not insufficient arm margin.
No permission inferred to widen it without current/sensing/timing analysis.
Next current qualification and short-cycle distribution/phase examination;
don't substitute more equivalent5.3% repeats. Raw peaks still not calibrated
mean current; no independent physical qZC proof or full parity completion.

## Entry 244 - 2026-09-13 - Exact guard-fault snapshot, not recorder inference

Source audit: core_bench::Obs.record calls powered_timer::accepted BEFORE
sampling observation_elapsed for accepted-event statistics/log. A trip clears
ownership; recorder then refuses the event (mask_range54_01 LATE_ACCEPTS1).
Its retained32-event tail spans618546..636379us with26same-step differences
3346..3541us.1034middle events missing. The3329minimum comes from whole-stream
recorder statistics elsewhere,not evidence of exact guard decision atstop.
This corrects E243's overly specific causal reading; no limit changed.

Added RunGuard::refused_cycle(now,step) fault-only getter, using original
cycle_at retained after refusal. Caller passes SAME now as accepted(), inside
existing critical section, before trip. Static16-byte CYCLE_FAULT survives
safing and resets with run statistics. reason12 summary prints step,previous,
decision,wrappingdelta,guard_timestamp1; allzero identifies unavailable event
snapshot. No extra per-guard state. Successful path adds fault comparison,
not snapshot writes; actual compiled timing remains to be measured.

Host drv_cycle_fault decoder checks fields,wrapping arithmetic,profile/reason
and violated bound; wired into sustained reporter. Legacy absence is unknown,
never inferred from recorder minimum. Tests include guard32bitwrap and host
wrongdelta/duplicates/healthyinterval rejection.192Python/155Rust pass.
Experimentalrelease/s/thinLTO build passes,text104464,data964,bss28756,
rootSHA6406EA73851577B13F795AFDABA1A92B32A43CEE6A874EAF0F13672FCC07867E.
NOT FLASHED. Installed460C0BD8 remains. No UART/probe/motor thisentry.
Next qualify disableddiagnostics/overhead thenone5.4%fault-diagnostic trial;
currentcharacterization also stillopen. Goal notcomplete.

## Entry 245 - 2026-09-13 - Actual guard delta captured on hardware

Preflash UARToff verified. Installed E2446406EA73 image; resetUART works on
this attempt, no clock/BRR workaround needed (not a permanent-fix claim).
cyclefault_archive_01 passes3x3 routing/refusal checks;cyclefault_cpu_01
maxima2/7/10us pass. Then one known5.3% recovery requalification:
cyclefault_reentry53_01 completes original10s/2s injection,remaining~7.99s
at284.9163eHz,sigma37.3973us,freshseed1179,age212ticks,remaining41.5us,
armcost13us. Raw400,bus10960,stack4752,deadline93usspare. CPUunionvalid/fault0,
COMPwall86,COM59,guard23,commit24us. Full reentry/timing verifiers pass.

Planned diagnostic cyclefault_range54_01 requests10s5.4%,retains FAILreason12
after161956us,271COM/270accepted. Fault snapshot:
step1,previous158616us,decision161947us,delta3331us against3333usfloor.
Stop timestamp follows by9us. Recorder minimum3348us (264cycles) is distinct;
rejected event omitted as expected (late_accepts1). Raw387,bus11462mV.
Exact guard delta now independently arithmetic-checked by host; test preserves
the real3331vs3348 distinction and rejects substituted recorder value.

This establishes the software guard boundary. It does not prove a motor or
MCU physical maximum, nor infer true rotor300eHz from one sampled interval.
No bound changed; future coordinated expansion needs current/sensing/timing
evidence. Don't repeat this same failing probe simply to get more fault draws.
Current qualification/parity remain open.

Both actual attempts finalgates/MOE/CCRs/ENABLEoff,nFLT1;COM41closed.
Installed/root6406EA73851577B13F795AFDABA1A92B32A43CEE6A874EAF0F13672FCC07867E,
release/s/thinLTO,text104464/data964/bss28756.193Python tests pass,155Rust
already passed E244. Goal remains incomplete.

## Entry 246 - 2026-09-13 - Expose current launch coverage in driven captures

Found runtime ADC_COVERAGE collection already active but driven fixture dump
omitted coverage_dump (legacy engage path emitted it). Added existing dump
after powered summary in driven_run::dump,which is fixture-only/postshutdown.
No change to sampling,ADCsettings,guards or epoch behavior.

Flashed release/s/thinLTO362CE448...;first UARTpreflightcurrent_coverage_archive_01
silent/retained. ExactRCC08000000/PD1ODR0/BDTRc1a/CCRs0 checked before known
clock08040000 workaround restoredUART. current_coverage_archive_02 passes3x3,
current_coverage_cpu_01 maxima2/7/10us. One1s5.3%measurementrun follows:
current_coverage53_01 PASS283.4784eHz1700COM/1700accepted,raw392,bus11140mV,
stack5052,allfinaloff/nFLT1. Allthree192launchcountarrays CRC/conservation pass.
Rejected9/9/5;A bins8,10,16,33,24,20,23,49;B16,11,29,34,26,23,25,19;
C8,21,26,26,29,24,28,25. Early-window launch coverage is visibly uneven.
No aperture/jointsector/unbiasedmean claim; a longerhold cannot extend192cap.

Source audit updates E185: CURRENT successful drivenhandoff retainsENABLE
through stage/release/adopt/awake start,unlike oldcoast+wake. Prestartzero
could share FIRSTsegmentepoch now,with explicit provenance/stationarity/drift
and samplecoverage requirements. Recovery stillrewakes; don'treuseinitialzero.
Next same-wakeinitialbaseline andtimedcurrentmeasurement design; no more
standalonezero/longhold repetitions or arbitrarycurrent estimates.

Installed/root362CE448EA84407BDB7D6813CE1AFACD6EA1C6F943291F9E467D05311C193847,
text104376/data964/bss28756,experimentalfeaturesunchanged.194Python tests pass
including full newdrivencapture coverage regression. No new recoverycohort,
COM41closed. Goal remains incomplete.

## Entry 247 - 2026-09-13 - Baseline epoch policy prerequisite

Added pure no_std calibration_epoch module to host/M0 replay library only.
One tracker perboard/boot issues non-Copytokens while commanded+physicalENABLE
and nFAULT healthy,gatesoff. Repeated high commands preserve epoch; every low
revokes; failed physical readback in tokencheck revokes even if later healthy.
Overflow is permanent refusal,not identitywrap. No GPIO/offset/control authority.
Six tests cover initial refusal,repeatedhigh,recoverywake,midscan shutdown,
physicalfault andoverflow;161Rusttotalpass,M0libcheckpass. Installedfirmware
unchanged362CE448...;no UART/flash/motor action.

Next livesingletonadapter atPD1write seam mustserialize outputwrite+metadata
with tokenoperations,physicalshutdownfirst; audit raw/panic bypasses and measure
hotCOMoverhead. Only then bounded stationaryprestartbaseline integration.
Purepolicytests are not liveepoch/standstill/calibration proof. Goal incomplete.

## Entry 248 - 2026-09-13 - Live ENABLE epoch adapter and timing check

Optional bench-current-epoch routes only set_pin(3,1) through calibration_live.
Singleton state/tokenops and PD1write are serialized in interrupt::free;
physicalwrite precedes provenanceupdate. Low invalidates, repeatedhigh keeps
identity. No gatecommand/offset authority. Panic uses thisset_pin path;
supportsource rawGPIOD audit finds adapteronly. External unwitnessed hardware
faults/resets remain provenance limitations; readbackfailure revokes tokens.

epochcheck gatedidle/outputsdisabled,uses twoCSA wakes withno gates. Five
checks cover prewake refusal,wake validity,repeatedhigh retention,oldtoken
revokedafterlow/newwake,newtoken validity; finaldisablealways. Doesnotrestore
oldtracker. Fixture drv_epoch_check reservesraw/finaloff,initialmaxwrite5usgate.
epoch_check_01: all5checks onall3trials;256repeatedhigh total184us,max1us each.
epoch_archive_01 passes3x3;epoch_cpu_01 maxima2/6/10pass.

Actualepoch_reentry53_01 full10soriginalbudget/2sloss injection recoveryPASS
284.9513eHz,sigma36.7917us,arm43.5us/cost14us,deadline132usspare,stack4752.
COMPwall86/COM59/guard23/commit25us. Full fixture and timingreport pass.
No baseline captured/applied;not evidence that a real baseline survivedhandoff.

Installed/root8E119A1D18EA06D0B91AB5E058336EED7301AD2A427A7D182E9509E7A14505AD,
release/s/thinLTO,text107932/data964/bss28764,priorfourfeatures+current-epoch.
UARTresetworkedthisflash,notpermanentfix. Allfinaloff/nFLT1,COM41closed.
161Rustpass. Nextboundedprestartbaseline/hostprovenance;goal incomplete.

## Entry 249 - 2026-09-13 - Bounded prestart raw baseline implemented, not flashed

New optionalbench-current-baseline impliesepoch. RUN branch afternormalwake,
beforeanypwm_sine phasewrite calls prestart_baseline::acquire:128samples on
channels4/1/0/6/13 using unchangedpowered read_channel.50ms total deadline,
perconversionexistingbounds,hostbyteabort,gatesoff andepoch/nFAULTchecks.
Returnsfalse->gates+ENABLEoff,cap1dumps partialraw evidence. Success retains
the alreadyawake epoch and rawstats/token; no guardoffset orvalueconversion.

Before driven pre-transferstaging (not onfreshseed path),entry_check records
whether same token is stillvalid. It cannot grant handoff. Fixturepostshutdown
dump addsPREBASE and5CRC BZ85 records ofcount/sum/squares/min/max,channels
explicit. entry_same_epoch0=notchecked,1=match,2=mismatch. stationary_verified0
and offsets_applied0: baseline is not yet provenzero current orcalibration.
Rawrecord remains aftershutdown while tokenidentity is revoked byepochtracker.
No recovery application; initialbaseline mustnotbe reused afterrecoverywake.

Host drv_prestart_baseline validates CRC/fields/moments/counts and successful
50ms bound; integrated sustainedreport.196Pythontests pass includingnegative
count/deadline/missing/duplicate/effectiveoffset metadata. Normal andoptional
release/s/thinLTObuilds pass. This is source/host evidence only; timedphysical
baseline,abort/refusal andsamewakehandoff NOT yet bench-qualified.

Rootoptional9BE065B5762F01F0C5A4AAA7F6A96A334F87DF1332F9DE2A5DD8D6BC6EC8D3CF,
text109356,data1088,bss28784. NOT FLASHED; installedE2488E119A1D unchanged.
No UART/probe/motor thisentry. Next boundedbaseline hardwarechecks thenfirst
samewakehandoffcapture; stationarity/drift/coverage remain separate gates.

## Entry 250 - 2026-09-13 - Baseline hardware tests and first same-epoch capture

Added idle basecheck+drv_baseline_check fixture. Actualscan paths:ENABLEoff
refusal;syntheticabortafter10conversions;actualENABLEdropafter10conversions;
complete128/channel scan thenlow/rewake invalidates token. All4casespass on
bothprebase_check_01/_02. Partialcases2samples/channel214..215us;complete
13537/13549us,below50ms. No gates commanded; eachcase finaldisabled.

Firstprebase_handoff53_01 refuses beforedrive,elapsed2us/0samples: parser
dispatchesonCR and pendingLF was read as abort. CorrectedRUNbaselinecallback
ignoresONLYCR/LF; any otherbyte includingoff's firstbyte/control-C stillaborts.
Failure retained. Hostfixture initially timedout waitingCOASTEND and sentidle
rampcommands; now detects exactrefusal,drains0.3s partialBZrecords and raises
withfinallyoff. This hostimprovement not itself a new livefailuretrial.

Correctedprebase_handoff53_02 completes1s5.3% handoff,1697COM,raw400,
bus11128mV,stack4904,initialarmremaining56us/cost13us. PREBASEstatus2,
elapsed13690us,128samples each4/1/0/6/13,entry_same_epoch1 atpre-transfer
staging. Raw sums262662/263374/263187/155251/192441; CRC/momentschecked.
stationary_verified0/offsets_applied0: no zero-current orcalibratedmean claim.
This demonstrates rawscan+tokenmatch+successfulhandoff,not laterrecovery
calibration orphysicalstandstill. Poweredlaunchcoverage remainsuneven.

BothflashresetsUARTworked. prebase_cpu_01/_02max2/6..7/10pass. Alltests and
bothattemptsfinaloff/nFLT1,COM41closed.197Pythonpass,including real successful
baseline andCRLFrefusal. Installed/root
6729CBE34266057713DDF7B11E9A988CDC7FEA61A63BFE171690B77438E582E4,
release/s/thinLTO,text110072/data1088/bss28784,priorfourfeatures+baseline.
Next explicit recoveryepochprovenance andtimedcurrentmeasurement;goalopen.

## Entry 251 - 2026-09-13 - Initial baseline revoked on actual powered recovery

Added fixture-only BASEEPOCH metadata. Recovery checks the retained initial
token immediately after driver wake, before fresh-edge acquisition. It never
replaces the original baseline statistics or entry result. Host decoding
requires checked=1/matches=0 for result7 recovery when this marker is present;
legacy missing markers remain unknown, not newly certified epoch evidence.
No current offsets, motor guards, duty bounds or minz control changes.

Release/s/thinLTO build with bench-driven-reanchor,bench-cpu-union,
bench-range300,bench-reentry-staging,bench-current-baseline installed:
SHA256 F7C7CA0DECB2330EC95E1C0B56422DB742CDD73C89E2CB176299D57E80D479F9.
text110276/data1088/bss28788. Before flash UART verified alloff/nFLT1.
Reset reproduced silent UART: prebase_epoch_check_01 retained preflight
failure, no motor command. Exact probe checks RCC08000000,PD1ODR0,BDTRc1a,
threeCCR0 preceded known RCC08040000 restoration. No permanent fix claim.
prebase_epoch_check_02 passes all4cases: zero samples when disabled,
2/channel on abort/drop (213/216us),128/channel complete13509us then token
refused after rewake. prebase_epoch_cpu_01 maxima2/7/10us pass.

One motor attempt prebase_epoch_reentry53_01, current G071/DRV wiring,
operator's unchanged11.7V/800mA bench setting, startup6.2% and existing
50-to200eHz ramp,5.3% driven/closed-loop duty,+60degree observer shift,
10-second original budget with2-second loss injection. Required full
handoff/recovery/archive/deadline checks, rawpeak<=1200,bus>=8400mV,
arm>=32us,cost<=16us, revoked baseline and finaloff. All pass:
- Resumed mean284.393eHz,cycle sigma36.419us,13633COM/13633accepted.
- Recovery seed1192half-us ticks,actualage210,remaining44us,cost14us.
- Originaldeadline14711128us,final14711035us:93us spare.
- Rawpeak400,busminimum10972mV,stackuntouched4608bytes,CPUunionfault0.
- Initial PREBASE128/channel13690us,entry_same_epoch1. BASEEPOCH checked1,
  matches0,initial_records_only1. Stationary_verified0,offsets_applied0.
- Finally ENABLE low,all six gate readbacks low,MOE0,CCRs0,nFLT1;port closed.

This is one new-build recovery pass, not a new repeatability cohort or
calibrated mean-current result. Initial baseline cannot calibrate resumed
measurements. Next timed/unbiased powered sampling and stationarity/drift
evidence; current, parity and envelope objective remains incomplete.

## Entry 252 - 2026-09-13 - Sequential driven-to-ADC TIM3 reuse, not flashed

Source audit: driven_run::end disables TIM3/TIM6, masks their vectors, stops
PWM-capture DMA and only then releases qualified handoff. Powered foreground
service_feedback starts adc_stream after actual core arm. coast_run_inner
stops/restores adc_stream before recovery acquisition. Thus simultaneous
ownership is unnecessary, but stale scheduler state must be refused.

New explicit bench-driven-dma implies bench-driven-handoff and DMA feedback.
Other driven-power+DMA builds still compile-refuse. Stream start additionally
refuses code11 when either forced/probe owner is active, TIM3 CEN/DIER is set,
TIM1 CC4DE is set, or DMAch2 is enabled. It masks/unpends TIM3 before configuring
ADC TRGO. Existing idle ADC/ch1 checks and all FIFO/current/age/stop behavior
remain. No timer writes steal a live forced owner. Fixture-only DMAOWNER
declares the policy; host rejects wrong/duplicate markers or missing stream
metadata. Declaration is not independent proof of hardware ownership.

Experimental release/s/thinLTO build succeeds with E251 features plus
bench-driven-dma: SHA954493745B8728D0B1A141A72060F88AE5155656D85904FC9FC1AD1EECF848EC,
text112944/data1088/bss28992. 200Python and161Rust tests pass; these cover host
protocol and existing pure logic, NOT the new register admission on hardware.
No flash/UART/motor actions. Installed remains E251 F7C7CA0D... foreground ADC.

Next disabled checks and short lower-point powered qualification, measuring
DMA start/feedback latency, FIFO delivery, ISR overhead, stack and final restore.
Do not extrapolate older standalone DMA timing to this combined size-s image.
Timed scans alone do not establish unbiased PWM/sector coverage or calibrated
current; these and baseline drift remain current-measurement work.

## Entry 253 - 2026-09-13 - Combined stream starts, but feedback-age guard stops run

Installed E252 image954493745B8728D0B1A141A72060F88AE5155656D85904FC9FC1AD1EECF848EC,
release/s/thinLTO, same six explicit features. Preflash UART alloff verified;
reset UART worked without workaround. driven_dma_base_01 passes4/4 cases,
complete baseline13085us. driven_dma_cpu_01 maxima2/7/10 pass; archive_01
passes3/3 checks on3 trials. These do not qualify motor timing.

One actual attempt driven_dma45_01, unchanged G071/DRV wiring and operator
11.7V/800mA PSU setting,6.2% startup/50-to200 ramp,+60degree observer,
4.5% BEMF duty,requested1second. Required complete handoff/window, consistent
ADC count/timestamps, original electrical/age/timing guards and finaloff.
FAIL: reason4 FeedbackStale after10809us guard time/10844us observation,
14COM and14accepted. Initialarm remaining54us/cost12 passes. Streaming did
start, so this is not sequential-owner refusal code11.

97 delivered scans at101us: first153,last9849us; all5CRC sums/counts agree.
DMA IRQ max11us,queuepeak8/8,rawpeak319,busminimum11366mV,stack4700bytes.
CPUunion valid/fault0:6897us IRQ,3912us foreground in10809us. Foreground is
not idle and this is not total CPU utilization. Queue pressure plus stale
feedback suggests consumption lag; precise cost attribution is not proven.
No FIFO-overflow fault was reported. Do not relabel this as a completed run
because the partial current statistics are internally consistent.

Initialbaseline128/channel13266us,entry_same_epoch1,no calibrated offsets.
FinalENABLE/allgates0,MOE0,CCRs0,nFLT1 verified; COM41closed. Failure retained
and regression test requires full motor verifier to reject it. No identical
retry or guard relaxation. Next investigate processing cost or a separately
specified slower phase-decohered ADC cadence, preserving every queued sample's
original acquisition time and the1ms age limit. Image remains installed but
is NOT combined-motor-qualified; E251 foreground successes remain historical.

## Entry 254 - 2026-09-13 - Explicit 201us ADC profile clears first short-run gate

E253 queue pressure motivated reducing measurement load, not relaxing any
guard. Added optional bench-dma-201 (impliesDMA), default remains101us.
Shared PERIOD_US sets TIM3 ARR, original acquisition timestamps and strict
current-sum spacing. Both201 and101 are relatively prime to nominal100usPWM;
this arithmetic alone does NOT prove PWM/sector coverage with commutation
resets. Host accepts only explicit101/201 metadata, validates sums against
that spacing and rejects duplicate/unknown cadence. Legacy absent marker101.
Raw currents,1msfeedbackage,FIFO capacity/overflow,COMP sensing unchanged.

Installed release/s/thinLTO E252 feature set plusbench-dma-201:
A662E01E79CACE09CC301E608C20A42972ECB5C507312C550B91BAC23E7248AE,
text112964/data1088/bss28992.202Python/162Rust pass before bench. UARTsilent
resetpreflight driven_dma201_base_01 retained; exactsafe RCC08000000,
PD1ODR0,BDTRc1a,CCRs0 beforeknownRCC08040000 restoresconsole.
base_02 all4casespass,complete13084us;CPU_01max2/6/10pass.

One actual same-setting trial driven_dma201_45_01: G071/DRV unchanged wiring,
operator11.7V/800mA setting,6.2%startup ramp50..200eHz,+60observer,4.5%BEMF,
1second. Full handoff/window/current-count/timestamp/finaloff checks PASS.
233.335eHz,cycle sigma58.873us,1400COM/1399accepted,4974ADCscans first253
last999826us,queuepeak2/8,DMAmax11us. Rawpeak456,busmin11104mV,
stackuntouched4660,armremaining66.5us/cost13. CPUunionIRQ567387 of1000009us,
fault0; foreground432622us is not idle. Commit bracket max39us includes
preemption, not exclusive execution cost.

Initialbaseline128/channel13266us,sameepoch1,stationarityunverified,nooffsets.
Allfinaloff/nFLT1,COM41closed. Added real201us capture regression plus wrong
cadence rejection. The reduction clears the observed short-run failure;
precise processing-cost attribution and long-run/recovery qualification remain.
Next longerhold/recovery then powered sampling coverage/baseline drift work.

## Entry 255 - 2026-09-13 - Same-boot sums reset fixed; hold passes, recovery fails age

First10s request driven_dma201_hold45_01 on existingE254 boot fails at393us,
DMAfault10 sum admission: POWERFEEDBACK1 but S85still4974rows identicalto
preceding1srun. Sourcecause: stage_driven bypassesprepare,which alone cleared
CURRENT_SUMS. Addedclear_sums to reset_run_statistics BEFORE acquisition;
prepare earlyclear retained for laterrefusal evidence. No guardchanges.

Installed323A7CF866B845AF6F51F84E8135C2D97FF46D0FC2484259CBA8840DC111AA1C,
text112980/data1088/bss28992,release/s/thinLTO,sameE254features. Silentreset
cpu_01preflightretained. ExactRCC08000000/PD1ODR0/BDTRc1a/CCRs0 before
knownRCC08040000workaround;cpu_02max2/7/10pass.

SameG071/DRV wiring,operator11.7V/800mA,6.2%startup50..200,+60observer,
4.5%BEMF,10soriginalbudget. Corrected reset_hold45_01 fullPASS234.256eHz,
cycle sigma45.207us,14055COM/14054accepted,49749scans255..9999603us,
queue2/8,DMAmax11us,raw491,bus10686,stack4660,initialarm57.5us/cost13.
Baseline13265us128/channel sameepoch1. Allfullcapture checks pass.

WITHOUT reboot next reset_reentry45_01 succeedsinitialstartup and2ssegment,
theninjectedTrackingloss leadsnew12intervalseed1449ticks,age210,
remaining76us/cost14. ActualrecoveryFAILreason4 FeedbackStale at409us,
observation450us,1COM/0accepted. ADConeframefirst242us,raw120,bus11653,
queue1/DMAmax10us,stack4404. This is NOT stalesums: rawcount1matchesfeedback.
BASEEPOCHchecked1/matches0. Recoveryfeedback transition requires investigation:
initial acquisition feedback is deliberatelyaged; exact initialage and delivery
decision time not exposed. Do not claim precise rootcause from stop timealone.

Allthreeattemptsretained,finaloff/MOE0/CCRs0/nFLT1verified,COM41closed.
Next instrument or resolve initialfeedback-to-stream delivery budget without
refreshingolddata/loweringageguard.201us sustainedhold now demonstrated;
poweredrecovery oncombinedprofile remainsunqualified. No calibratedcurrent.

## Entry 256 - 2026-09-13 - First DMA delivery age evidence added, not flashed

Source trace confirms feedback_aged checks BOTH new frame age and elapsed
time since previous feedback before applying the acquisition timestamp.
Recovery baseline uses oldest channel time plus200us conservative allowance.
E255 lacks the actual initial age/first delivery decision time, so fresh
frame242us plusstop409us does not by itself establish which check failed.

Added fixture-only FEEDBACKFIRST fields seen,initial_age_us,decision_us,
previous_us,acquired_us,fault. Initial age recorded after guard admission;
first DMA delivery recorded inside existing guard critical section using
the exact decision timestamp and previous timestamp, no extra clock read.
It is diagnostic only, does not refresh feedback or alter guard thresholds.
seen0 explicitly means no captured delivery, including stop-before-delivery.
Record resets with segment statistics; recovery overwrites initial-segment
record just like other segment-local powered statistics.

Pure regression: syntheticinitialage650,delivery409,newframeage167 correctly
fails since oldfeedback1059us; delivery340/age98 succeeds with timestamp242.
These are example values, NOT measured E255initialage.163Rust tests pass.
Release/s/thinLTO combined201us build passes, root
69F29BD9CFD833528BFDB65C1B152809645103125A0C29F904ABF3DEA76EA9C6,
text113220/data1088/bss29020. NOT FLASHED; installed323A7CF8 unchanged.
No UART/probe/motor action. Next disabled timingcheck then recovery capture
to distinguish expiredinitialfeedback from delayed newframe; preserve1msguard.

## Entry 257 - 2026-09-13 - Recovery initial-feedback budget observed; earlier first trigger pending

Installed E256 diagnostic69F29BD9CFD833528BFDB65C1B152809645103125A0C29F904ABF3DEA76EA9C6.
UARTsilentfirst_delivery_cpu_01 retained; exactRCC08000000/PD1ODR0/BDTRc1a/
CCRs0 beforeknownclockwrite08040000. cpu_02 max2/6/10pass. Oneactual
first_delivery_reentry45_01,unchanged4.5%/10s/2sinjection campaign:
recoveryseed1452,age212,remaining75.5us,cost13us;FAILFeedbackStale4at409us,
1COM0accepted,stack4376. FEEDBACKFIRSTseen0 initial_age_us674; this leaves
326us to firstdelivery before original1ms expires. ADCscan1/queuepeak1 exists,
but guard revoked before delivery criticalsection; seen0 is NOT noADCframe.
No falselyrefreshedtimestamp. Finaloff/nFLT1verified,COM41closed.

Pending fix retains201us steadymeasurement but firsttrigger101us. Preload
stoppedTIM3CNT=PERIOD-FIRST_TRIGGER (100for201,0forlegacy101),thenCEN with
existingconservativeorigin. Timestamp=origin+101+(count-1)*PERIOD; no UG or
syntheticADCtrigger, subsequentspacingunchanged. FixtureDMASTART declares
first/steadycadence; host rejects mismatched/duplicate policy. This removes
100us initialwait withoutchangingfeedbackguards/COMP sensing orolddataage.

Buildpassesrelease/s/thinLTO;root314A45EAE39646396E3B955A3F225E566B7561A503C24D8B17E287609C6DD118,
text113328/data1088/bss29020. NOT FLASHED;installeddiagnostic69F29 unchanged.
Next qualifyactual firsttrigger/deliveryandrecovery; no successclaimyet.

## Entry 258 - 2026-09-13 - Earlier first trigger passes 10s and30s recovery

Installed314A45EAE39646396E3B955A3F225E566B7561A503C24D8B17E287609C6DD118,
release/s/thinLTO,sameE257features,text113328/data1088/bss29020.
UARTsilentearly_trigger_cpu_01retained; exactRCC08000000/PD1ODR0/BDTRc1a/
CCRs0 beforeknown08040000clockwrite. cpu_02max2/7/10pass.

Twoactual SAMEBOOT G071/DRV attempts,unchangedoperator11.7V/800mAsetting,
6.2%startup50..200eHz,+60observer,4.5%BEMF,lossinjected2s:
- early_trigger_reentry45_01 full10sbudgetPASS:233.873eHz,cycle sigma43.656us,
  11209COM/11208accepted,39739resumedscans142..7987480us,queue2/8.
  Firstdelivery initialage642,decision301,previous4294966654,acquired142,
  fault0: previousage943us,newframe159us. Recoveryarm77.5us/cost14,
  originaldeadline172usspare. Raw471,bus10996,stack4376.
- early_trigger_reentry45_30s_01 full30sbudgetPASS:234.370eHz,sigma43.770us,
  39358COM/39357accepted,139244resumedscans142..27987985us,queue3/8.
  Firstdelivery initialage636,decision300,previous4294966660,acquired142,
  fault0: previousage936us,newframe158us. Recoveryarm76.5us/cost14,
  deadline164usspare. Raw510,bus10471,stack4376.

BothDMAmax11us,BASEEPOCHchecked1/matches0,allfullcapture/archive/deadline
checks pass and finalENABLE/gates/MOE/CCRs0,nFLT1 verified. COM41closed.
Host now validates optionalFEEDBACKFIRST fields,modularpreviousidentity,
firstS85timestamp and successfulfirstdeliveryages;206Python tests pass.
Not scope to infer physicalPWMcoverage,calibratedcurrent or independentqzc
from these passes. Oldfailures retained. Two differentdurations are not a
fixedrepeatabilitycohort atallpoints. Next coverage/calibration/stagedenvelope,
notmoreequivalentlow-point recoveryruns.

## Entry 259 - 2026-09-13 - Trigger-phase DMA histogram implemented, not flashed

Added optionalbench-adc-phase(impliesDMAfeedback). CachedHAL dmamux.rs
mapsTIM3_UP37. ADCstream nowoptionallyconfigures spareDMAch3/mux2 tocopy
TIM1CNT into2halfwordcircularslots onTIM3update, alongside ADCtrigger.
No addedinterrupt. ExistingADCIRQ aftercoherentADCcopy verifiesphaseDMA
index,expectedHT/TCflag,NDTRbefore/aftervolatilecopy andnofreshflags.
Counter>=6400 orstreammismatch refuses withADCfault13; initbusyrefuses12.
StopdisablesUDE/ch3 beforeADCrestoration. ConfigoccursafterTIM3UG while
timerstopped,avoidinga syntheticfirstsample. No gatewriteauthority.

Whole-stream32bin histogram (200TIM1ticks/bin) counts consumedphasewords
BEFOREFIFOdelivery. It is not identicaltofeedbackcount on faults/queuedtail.
FixtureADCPHASE/AP85 exports CRCrows and explicitlydeclares triggeronly,
latency_qualified0,aperture_known0,sector_known0. Hostvalidatesmetadata/order/
CRC/sum; balancedhistogram doesNOTproveunbiasedcurrent.208Python tests pass.

Release/s/thinLTO buildpasses,E258features+bench-adc-phase:
48B4DEB225CABBB94B8ECAD0E53279DFF10C62A7694E35F05B9C44064111AC6B,
text113924/data1088/bss29156. NOT FLASHED; installed314A45 unchanged.
No UART/probe/motor. Next disabledroute/triggerlatencyqualification, then
measure extraADCIRQ/setupcost andphase-to-scan alignment beforecoverageclaims.
The new preparationcost also consumes first-feedback margin; do notassume
the previousE258 recoverytiming applies unchanged. Currentcalibration open.

## Entry 260 - 2026-09-13 - DMA route and first powered phase histogram measured

Added idle adcphasecheck: sameDMAch3 route/slotconsumer readsTIM3ownCNT on
update instead ofTIM1CNT. Outputs/ENABLEmustbeoff, bounded32samples/10ms,
noADCtraffic,thenstops timer/DMA. Gate<=2us countervalue,32validslots.
adc_phase_route_01 passes3checks withmaxCNT0us each (quantized<1us),not a
motorloadedlatencybound. cpu_01max2/7/10pass. ResetUARTworkedwithoutworkaround.

Installed142AEAC31B7022BBB83F9241423590B58CC44426A0118819641D4CD5BC196095,
text114804/data1088/bss29156,release/s/thinLTO,sameE259features. Oneactual
adc_phase45_01 G071/DRV unchangedwiring,operator11.7V/800mAsetting,
6.2%startup50..200,+60observer,4.5%BEMF,1s:fullcapturePASS233.710eHz,
1402COM/1402accepted,sigma66.486us,raw432,bus11092,stack4420,
arm59us/cost13. Firstfeedback initialage582+decision293=875us,acquired159.
DMAmax17us versusprior11 (wallbrackets),queuepeak3/8. Recovery/longrun onthis
meteredbuild NOTyetqualified. Finaloff/MOE0/CCRs0/nFLT1,COM41closed.

4975phasewords consumed,4973ADCframesdelivered;phasecounts precedeFIFO and
include2undeliveredtailframes. All32triggerbinsnonzero but NONUNIFORM:
[174,401,240,127,123,110,101,104,138,163,123,119,92,92,135,118,
128,174,150,124,159,145,155,183,156,159,175,171,135,224,223,154].
Mean155.47,min92,max401. Nominal201usdecoherence doesnotmakeactualtrigger
distributionuniform withcommutationresets. This is trigger-densityevidence,
not measuredADCtrackingaperture,sectorconditionalcoverage orcurrentbias size.
Keep latency_qualified0 because unloadedrouteprobe doesnotboundmotortraffic.
Next currentmethod mustaccountforobservednonuniformity; mereadditionalholds
orwhole-runrawsums cannotestablishunbiasedcurrent. Newrealcapturetestadded.

## Entry 261 - 2026-09-13 - Correct the histogram target before weighting current

E260 measured nonuniform trigger-counter density, not demonstrated sampling
bias. Earlier wording implying nonflatcounts necessarilypreventunbiasedtime
sampling was too strong. sixstep_write executesTIM1EGRUG everycommutation;
the counter resets and its time-occupancy distribution neednotbeuniform.
Inversecount weighting towardflatbins can therefore introducebias.

Added hostpwm_time_occupancy.dwell for explicitresetintervals in timer ticks.
Tested exactagainsttickenumeration and synthetic150-tickreset/100-tickPWM:
uniformtime sampling correctly produces2:1lower-halfoccupancy. For synthetic
signal10inlowerhalf/0upperhalf,true timeaverage20/3,flatphaseaverage5.
These are mathematicalexamples,NOTfitsorexplanationofE260's401/92extrema.
Actualreset-to-resettime and pausesnotfullyretained; no claimknownoccupancy.
ADCphasedecoder adds explicit sampling_bias_provenFalse/time_occupancy_knownFalse.

Read-only E260currentcheck: signedrawsums13465+9690+57039=80194,n4973;
initialbaseline offsets505/128+700/128+1141/128=18.328125counts.
Naivesubtraction~16.126-18.328=-2.202counts. Not a calibratednegative
current measurement or proof ofsamplingbias/offsetdrift; baseline physical
stationarity,analog drift/sequence effects and representativeness unresolved.
No ampsreported, offsetsnotapplied. Prioritizebaselinevalidity andindependent
anchor,notan invented flat-histogram requirement.

212Python tests pass. No firmwareedit/flash/UART/motoraction;installed/root
142AEAC31B7022BBB83F9241423590B58CC44426A0118819641D4CD5BC196095 unchanged.
Goalstillrequires current/timing/parity/envelope evidence; correctiondoesnot
establishcalibration or broaden motorqualification.

## Entry 262 - 2026-09-13 - Phase-capture build sustained30s; raw residual comparison

Unchangedinstalled142AEAC3...E260 release/s/thinLTO. Oneactual
adc_phase_hold45_30s_01 G071/DRV unchangedwiring,operator11.7V/800mAsetting,
6.2%startup50..200,+60observer,4.5%BEMF,30s. FullwindowPASS234.442eHz,
cycle sigma48.174us,42200COM/42199accepted,149253ADCandphasewords,
queuepeak3,DMAmax17us,raw479,busminimum10471mV,stack4420. Initialarm
67.5us/cost13,firstdeliveryinitialage286+decision296=582us,acquired160.
FinalENABLE/gates/MOE/CCRs0,nFLT1verified,COM41closed. No flashthisentry.

AskedoperatorasynchronouslyforPSUvoltage/current/CV/CC duringthisactualrun;
no replyreceived. This is a measurementopportunity,NOT independentanchor.
Do notrepeatidenticalholdsmerelywaitingforananswer.

Addeddrv_baseline_residual hostreport:requiresfullinitialhandoffverification,
completebaselineentryepochmatch andrawsums; refusesREENTRYcaptures because
initialbaselinewasrevoked. Reports rawresidualsonly,ampsNone,stationarity/
drift/calibrationfalse.214Python tests pass,includingrecoveryrefusal.
New poweredcenteredmeans11.115026/7.162817/10.748869counts minusbaseline
6.578125/7.648438/5.976563 =>aggregate8.823588counts. E2601sresidualwas
-2.202245. Differentdurations/startupcontribution/offsets confounded;not
proof ofdrift,calibratedcurrentdifference orsamplingbias. Currentmethod
needsbaselinevalidity/independentcomparison; do notconvertthese toamps.

## Entry 263 - 2026-09-13 - Archive assessment and portable contracts updated

Read-only revalidation: freeze_minz_reference --check-source passes all128files,
archive2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44.
minz_historical_baseline --verify-archive reproduces3capturemetrics andarchive
94df42404cd3e1735039fcba647bea93d795e6e11cd8823d446c921219355138 membership/hashes.
FALCONa9:230.020eHz,5543windows,1missing,qZC99.98196%,nominal44.8497mA;
a10:257.476eHz,100%retainedqZC but1sequencegap;202.944eHzcaptureqZC20.5477%.
These are historicalwindowmetrics,notindependentbinzquality,matchingflashed
AM32identity orcurrentefficiencyparity. No referencebench reruns.

UpdatedBEMF_PARITY_PLAN with explicitcomparisonlimits andcurrentimplementation
status,PORTABLE_WINS withE239..262measuredcontracts,TIMING_HEADROOM withactual
currentcapturechecks. E262phaseDMA30shold at234.442eHz hasCOMP/COM/commit
wallmax115/79/45us;E258nophase30srecovery234.370eHz100/73/39us. Different
paths/builds,notcontrolledoverheadA/B;no CPUutilization/headroompercentclaim.

Documentation now separates older5.3%~285eHzforegroundcohorts from current
4.5%instrumentedqualification, exact3331ussoftwareguardboundary fromphysical
limits, precisioncalibration fromrequiredcurrentevidence, andmissinghistorical
counterparts fromblockers. Portablefixes are candidates withtargetacceptance
contracts,notappliedrm32changes. No firmware/UART/probe/motor thisentry;
installed/root142AEAC3 unchanged. Nextcurrentbuildrecovery/stagedenvelope and
currentevidence;goalnotcomplete,do notsubstitutefurtherequivalentdocreviews.
## Entry 264 - 2026-09-13 - Phase-DMA recovery and duty-led holds through4.8%

No firmwarechange/flash:installed142AEAC31B7022BBB83F9241423590B58CC44426A0118819641D4CD5BC196095,
release/s/thinLTO with201usDMA,early101usfirsttrigger andphasecapture.
ThreeactualsamebootG071/DRVattempts,operator11.7V/800mAsettingunchanged,
6.2%startup50..200eHz,+60observer. No safetythresholdorcontrolchanges.

1. adc_phase_reentry45_01:4.5%,10soriginalbudget,2slossinjection,fullrecovery
   PASS234.451eHz,11237COM/11237accepted,sigma48.331us. Newseed1448age212,
   arm75us/cost13,originaldeadline216usspare. Firstdeliveryoldage660+321=981us,
   newframe174us:19uspreviousageheadroom. Raw471,bus10507,queue3,DMAmax17us,
   stack4232. Foregroundwindowcompletion reportsreason0,notIRQdeadline2;
   fulltimeline/archive/outputsverification passes.39743phasewords consumed.
2. adc_phase_hold46_01:4.6%,10sfullPASS239.818eHz,14389COM/14388accepted,
   sigma50.827us,raw463,bus10948,arm66us/cost13.49750ADCframes,49751phase,
   queue2,DMA17us,stack4232. COMP/COM/commitwallmax115/79/45us.
3. adc_phase_hold48_01:4.8%,10sfullPASS252.080eHz,15125COM/15124accepted,
   sigma53.052us,raw467,bus10948,arm50us/cost14.49750ADCframes,49751phase,
   queue2,DMA17us,stack4232. COMP/COM/commitwallmax115/93/45us.

Eachstepfollowedprecedingpassedguardedrun; retainedcurrent,bus,tracking,
age/arm/deadline/CRC/countchecks. AllfinalENABLE/gates/MOE/CCRs0,nFLT1;
COM41closed. No newoperatorPSUreading; rawcurrentnotcalibratedmean.
Theseare3distinctsingleattempts,notfixedreliabilitycohort orhigherpoint
recoveryqualification. Next4.8%recovery,thenfurtherstagedduty withsameguards;
do nottreatpriorforeground285eHzcohort ascurrentphaseDMAevidence.
## Entry 265 - 2026-09-13 - 4.8% recovery fails previous-feedback age by12us

Actualadc_phase_reentry48_01 oninstalled142AEAC3...,unchangedG071/DRV wiring,
operator11.7V/800mA,6.2%startup50..200,+60observer,4.8%BEMF,10sbudget and
2sinjection. Initialsegmentruns2s; recoveryacquiresfreshseed1344ticks,
actualage210,remaining63us/cost14. RecoveryFAILFeedbackStale4at355us,
observation396us,1COM0accepted,oneADCframe,queue1,DMAmax16us,raw29,
bus11665,stack4232. Allfinaloff/MOE0/CCRs0/nFLT1;COM41closed.

FEEDBACKFIRST seen1initialage660decision352previous4294966636acquired147
fault4. Modularpreviousage1012us exceedsunchanged1000by12;newframeage205us
is withinbound. This is an exactpreviousfeedbackgap failure,not stale new
frame,seedexpiry orFIFOload. No5%hold attempted;don't advancepastthisrefusal.

Addedoptionalbench-dma-fast-start: firstexternaltrigger1us instead of101,
steady201usunchanged. ExistingstoppedCNTpreload becomes200; ADC/DMAsetup
completebeforeCEN; derivedfirsttimestamporigin+1 andsubsequent201spacing
remainconservative, noUG/synthetictrigger oroldfeedbackrefresh. Existing
default101usfirstdelay remains whenfeatureoff. HostDMASTART acceptsONLY
first1/101 withmatchingsteadycadence.215Python tests pass,includingretained
failure/rejectionof falselymarkedfault0.

Release/s/thinLTObuildpasses,E264features+bench-dma-fast-start:
6775326FD8420D4C35E2D4E360F36AF1377B8133E0838B83082D56FDADE23D47,
text114804/data1088/bss29156. NOTFLASHED;installed142AEAC3 unchanged.
Next disabledchecks andhardwarefirst-trigger/recoveryqualification before
claimingfix. No electrical/feedback-age/seed/COMguardchanges.

## Entry 266 - 2026-09-13 - Fast first ADC trigger: recovery and 5% hold pass

Installed E265 image
6775326FD8420D4C35E2D4E360F36AF1377B8133E0838B83082D56FDADE23D47:
release, opt-level=s, thin LTO, codegen-units=1; text114804/data1088/bss29156.
Features: bench-driven-reanchor,bench-cpu-union,bench-range300,
bench-reentry-staging,bench-current-baseline,bench-driven-dma,bench-dma-201,
bench-adc-phase,bench-dma-fast-start. G071/DRV wiring and operator-reported
11.7V/800mA setting unchanged; no new PSU display reading received.
First ADC trigger1us, steady201us; no safety/control threshold changes.

Silent fast_start_cpu_01 preflight retained, no motor activity. Exact safe
register checks and known RCC UART-clock workaround restored console;
fast_start_cpu_02 idle overhead max2/7/10us passes.

- fast_start_reentry48_01: 4.8%,10s original budget,2s injected tracking loss,
  recovery PASS252.329028eHz,12095COM/12094accepted,cycle sigma50.506594us.
  Recovered observation7988948us; original deadline189us spare. Fresh seed1344,
  age212half-us, actualarm62us/cost13. First delivery initialage650+decision246
  =896us previous-feedback age (104us spare), new frame198us old, fault0.
  Phasecount39746,queue2,DMAmax18us,rawpeak480,busmin10865mV,stack4232.
  Baseline recovery epoch revoked as expected; full recovery verifier passes.
- fast_start_hold50_01: 5.0%, full10s PASS265.011853eHz,15901COM/15900accepted,
  cycle sigma52.916220us.49751 ADC and phase records,queue2,DMAmax18us,
  rawpeak489,busmin11020mV,stack4232. Initialarm59us/cost14; first delivery
  initialage354+decision259=613us previous age, acquired59us, fault0.
  COMP/COM/guard/commit wall brackets115/79/23/45us; not CPU utilization.

Continuation found session57386 missing and no live COM41 fixture process.
Recovered completed capture rather than rerunning it. Both captures pass full
offline timing/chronology/CRC/electrical/final-off verification;216 Python
tests pass including fast-start recovery and false-freshness rejection.
Root ELF hash reconfirmed. Live off/p/i again confirms all gates, ENABLE,
MOE and CCRs zero, nFLT1; port closed. No additional motor run on continuation.

These are distinct single-attempt passes, not a fixed reliability cohort,
independent qZC proof or calibrated mean-current result. Faster first launch
clears the observed recovery-age failure on this attempt without widening1ms.
Next qualify5% recovery before further current-build duty exploration;
retain older failed attempts and remaining current/parity/envelope work.

## Entry 267 - 2026-09-13 - Duty-led recovery through5.2% on fast-start build

Previous goal turn made progress: recovered outstanding completed capture,
verified5%hold and recordedE266. This turn re-read objective and confirmed
no live fixture/probe before running three sequential attempts, inspecting
each full verifier result before advancing. Same installed6775326F... image,
E266 feature set/release/s/thinLTO, G071/DRV wiring, operator11.7V/800mA.
No flash, code, safety threshold or PSU changes; no fresh PSU display reading.
Startup remains6.2%50..200eHz with+60degree observer. All windows10s total.

1. captures/fast_start_reentry50_01.txt:5% recovery PASS,2s initial segment
   then fresh seed1282half-us/age212,arm54.5us/cost13. Resumed7989048us,
   264.881824eHz,12697COM/12696accepted,cycle sigma48.465563us.
   Original deadline108us spare. Firstfeedback666+178=844us previous age,
   new frame131us old. Rawpeak489,busmin10877mV,phasecount39746.
2. captures/fast_start_hold52_01.txt:5.2% fullhold PASS,10000050us observed,
   276.478534eHz,16588COM/16588accepted,cycle sigma49.410618us.
   Initialarm59us/cost14,previousfeedback360+259=619us. Rawpeak497,
   busmin10853mV,49750deliveredADC/49751phase records (phasebeforeFIFO).
   Complete initial-baseline residual report passes: centered powered means
   [12.707719,11.330874,7.822030], baseline[8.4375,11.484375,11.742188],
   sumresidual0.196561 rawcounts. Not amps, stationarity or drift proof;
   don't infer zero motor current from this near-zero signed residual.
3. captures/fast_start_reentry52_01.txt:5.2% recovery PASS, freshseed1233,
   age212half-us,actualarm48us/cost13 (16us above minimum). Resumed7989349us,
   276.591530eHz,13259COM/13258accepted,cycle sigma42.986426us.
   Original deadline161us spare. Firstfeedback674+178=852us previous age,
   newframe130us old. Rawpeak505,busmin10829mV,phasecount39748.

Allthree queuepeak2/8,DMAmax18us,untouchedstack4232,COMP/COM/commit wall
brackets115/79/45us. Wall brackets are not exclusive CPU utilization.
All full capture verifiers and independent timing-report invocations pass;
both recovery archives/epoch revocations/original deadlines verified. Each
fixture ends with gates/ENABLE/MOE/CCRs0,nFLT1,closed port; no live handle.
No failures in this three-attempt sequence, but different settings/types
are NOT a fixed repeatability cohort and don't erase historical refusals.

Next current-build5.3% hold/recovery, then deliberate sustained/repeatability
qualification. Prior5.4% exact cycle-guard failure is a software-limit datum,
not a demonstrated physical maximum or permission to widen it blindly.
Current/independent-quality/parity scope remains incomplete; retain raw evidence.

## Entry 268 - 2026-09-13 - 5.3% hold and thirty-second recovery pass

Previous turn made measured progress through5.2%; objective reread and live
process ownership checked before two sequential runs. Same installed6775326F...
release/s/thinLTO,E266 features,G071/DRV wiring,operator11.7V/800mA setting.
No flash/code/guard/supply changes or fresh operator supply-current reading.
6.2% startup50..200eHz,+60observer; powered duty5.3% in both attempts.

- fast_start_hold53_01.txt: full10s PASS283.874531eHz,17032COM/17031accepted,
  cycle sigma52.595300us,rawpeak517,busmin11008mV,49751ADC/phase records.
  Initialarm59us/cost13;firstfeedback286+258=544us previous age,newframe198us.
  IRQ-union6053838 of10000008us is valid software-boundary accounting,
  about60.5%; remaining3946170us is foreground work, NOT idle headroom.
  Baseline residual report sum0.780803rawcounts, not calibrated motor current.
- fast_start_reentry53_30s_01.txt: full30s original budget,2s injected loss,
  recovered27990051us PASS284.217146eHz,47732COM/47731accepted,
  cycle sigma44.946302us,rawpeak579,busmin10901mV,139254phase records.
  Freshseed1192half-us,age212,actualarm43us/cost13 (11us above32us floor).
  Originaldeadline123us spare. Firstfeedback650+179=829us previous age,
  newframe131us. Initialalignment wait13us is exercised and verified.

Both queuepeak2/8,DMAmax18us,stackuntouched4232; COMP/COM/guard/commit
wall maxima115/80/24/45us. Full capture and timing-report checks pass,
including recovery archive/epoch/deadline. All final gates/ENABLE/MOE/CCRs0,
nFLT1; fixtures exited successfully and ports closed. No guard relaxation.

This establishes one current-build longer recovery near284eHz, not a fixed
repeatability cohort. Next two additional30s recoveries at exactly5.3% can
complete a bounded three-attempt cohort with every refusal retained; then
review range/current evidence and limits, not endless equivalent repeats.
Historical5.4% cycle-guard stop remains a known limit datum, not motor maximum.

## Entry 269 - 2026-09-13 - Fixed5.3% thirty-second recovery cohort3/3

Previous turn produced hold/long-recovery evidence. Objective reread and live
process ownership checked; exactly two planned additional attempts ran, with
each complete result inspected before the next. No retry replacement.
Same6775326F... release/s/thinLTO/E266 features, G071/DRV wiring and operator
11.7V/800mA setting. No flash/code/guard/supply change or new supply reading.

fast_start_reentry53_30s_02: PASS284.378942eHz,47758COM/47758accepted,
cycle sigma44.813665us,raw554,bus10865mV,phase139252,queue2,DMA18us.
Freshseed1193half-us,arm43us/cost13,originaldeadline199us spare;
firstpreviousfeedback670+178=848us,newframe130us. Foreground completion
reason0 is valid by full chronology/deadline verification, not a missing fault.

fast_start_reentry53_30s_03: PASS284.541771eHz,47786COM/47785accepted,
cycle sigma44.744321us,raw519,bus10793mV,phase139254,queue3,DMA18us.
Freshseed1193half-us,arm43us/cost13,originaldeadline117us spare;
firstpreviousfeedback631+178=809us,newframe130us. Initial alignment wait51us
is exercised and verified. Both stackuntouched4232 and finaloff/nFLT1.

Together with E268_01, fixed cohort3/3 startup+sustain+injected-loss recovery,
30s original window each. CSV fast_start_reentry53_30s_cohort.csv retains
allthree outcomes; no replacements or additional equivalent trials. Full
timing/capture/archive/epoch/deadline checks pass and both fixtures exited,
portsclosed. COMP/COM/commit wall maxima115/80/45us; not CPU utilization.

Recorded cycle minima across01/02/03 are3351/3346/3337us. Last is only4us
above3333us guard floor, but recorder samples a different timestamp from
guard decisions: do NOT call this an exact4us guard margin. E245 retains
the actual refused guard sample3331us on an older5.4% build.

Next one bounded current-build5.4% probe with allguards unchanged can locate
this build's boundary before considering coordinated speed-limit changes.
Don't continue endless5.3% repeats or claim a physical motor limit from the
software floor. Mean-current validity and broader envelope/parity remain open.

## Entry 270 - 2026-09-13 - Current5.4% trial locates exact cycle-floor stop

Previous turn made progress by completing fixed3/3 recovery cohort. Objective
reread and process ownership checked. One planned10s5.4%hold on unchanged
6775326F... release/s/thinLTO,E266features,G071/DRV wiring,operator11.7V/800mA.
Startup6.2%50..200,+60observer; no flash/guard/supply changes or newPSUreading.

captures/fast_start_hold54_01.txt is a FAILED completion, not a passing spin:
reason12CycleTiming,stop689514us,observed689549us,1188COM/1187accepted.
ExactCYCLEFAULT step3 previous686170 decision689501 delta3331us<3333floor;
shutdown stamp follows13us later. Recorder min3336us excludes rejected event
and uses a separate timestamp; do not substitute it for guard evidence.
Mean of recorded cycles287.308575eHz,sigma114.975792us includes startup,
not equilibrium performance or physical maximum. Rawpeak494,busmin11092mV,
3430ADCframes59..689288us,queue2,DMAmax18us,stack4232. Initialarm57.5us/cost13,
firstpreviousfeedback320+263=583us,newframe204us. Finalgates/ENABLE/MOE/CCRs0,
nFLT1 verified by capture; fixture exits1 on deliberate completion rejection,
portclosed. Retained sums decode consistently but aren't calibratedcurrent.

New realcapture regression verifies exactguard3331 versus recorder3336,
outputs-off and rejection by full completion verifier;217Python tests pass.
No immediate repeat or speed-guard weakening. TIMING_HEADROOM updated with
current cohort and independently derived recovery scheduling constraint:
observed finalseedage212half-us plus64minimumarm needs wait>=276half-us,
roughly302eHz under idealregularintervals. This is a prospective timing
constraint, not a measured physical limit; widening3333 alone cannot fix it.
Next examine bounded acquisition/cleanup latency and coordinated range policy
while preserving real seed timestamps, electrical guards and32usarm floor.

## Entry 271 - 2026-09-13 - Recovery scheduling audit; closure rewrite rejected

Previous turn measured the actual5.4% cycle-floor stop. Objective reread;
this turn performs source/disassembly/build work only, no UART/probe/flash
or motor commands. Installed firmware remains6775326F... lastverifiedoff.

E269_03 SEEDLAT entry100/reset120/feedback134/guard180/reference188 and
actualage212half-us yields approximately50us preentry,10us local setup,
7us baseline/abort,23us guard setup,4us reference seed setup,12us IRQ setup
before actual age measurement. These are broad wall-clock brackets, not
isolated removable instruction costs. Guard setup includes physical ownership
checks, one-shot staging/adoption, fresh sample validation, original limits,
feedback age, publication and timer start. Earlier staging already removed
bulk statistics reset on recovery; do not repeat that proposed optimization.

Compiled current start_inner address0800b4a4,size0x32c,stacklocals188bytes.
Disassembly shows guard data copied into a by-value critical-section closure.
Tested pending Option<Guard> plus borrowed take() to transfer once while
preserving publication order. Release/s/thinLTO build49E278F... compiles,
but stacklocals remain188 and copy remains; text114844 vs114804 (+40).
No measured timing improvement; no hardware qualification warranted on this
evidence. Reverted ONLY this turn's change and rebuilt exact original SHA
6775326FD8420D4C35E2D4E360F36AF1377B8133E0838B83082D56FDADE23D47.

Next investigate guard construction/publication itself: in-place or inert
preacquisition storage, retaining fresh sample/limits/seed-step/age validation
and atomic singleton publication before timer authority. Do not equate
compiler-copy inspection with measured23us overhead or claim a latency win.
No guard widening, reference-control changes or new operating qualification.

## Entry 272 - 2026-09-13 - Construct-under-mask experiment passes motor, loses timing

Previous turn produced source/disassembly evidence and rejected a no-gain
rewrite. Objective reread. Moved guard construction/validation inside existing
publication critical section, retaining samechecks/age/limits and allvalidation
before GUARD/ACTIVE/timer publication. Build A60D1B723B4806B74E3A4C611F66BCD7A2401E7EEFD3EDA32329EC37537F4ACC,
release/s/thinLTO,E266features,text114884/data1088/bss29156. Callerlocals60
plushelperlocals60 (oldcaller188); saves no proven runtime stack or cycles.
163 Rust host tests pass. Pure tests do not execute peripheral publication.

Flashed after liveoff verification. guard_construct_cpu_01 silent preflight
retained; safe RCC08000000,PD1ODR0,BDTRc1a,CCRs0 checked then knownRCC08040000
clockrestore. cpu_02 max2/7/10 and archive_01 three3/3 disabledchecks pass.

One actual guard_construct_reentry53_01,5.3%,10s originalbudget,2sinjection,
unchangedG071/DRV wiring/operator11.7V/800mA/startup6.2%50..200,+60observer:
fullrecoveryPASS283.894020eHz,13610COM/13609accepted,sigma45.097930us,
raw511,bus10805mV,queue2,DMA18us,stack4232,deadline208us spare.
SEEDLAT98/120/134/188/196,actualage220half-us,seed1193,arm39us/cost14.
Guard bracket27us versus old23; same1193seed oldage212/arm43/cost13.
Firstfeedback637+173=810usold,newframe126us. Allfullverifiers/finaloff/nFLT1.
Motor pass is NOT optimization pass: measured margin went backwards4us.

Reverted only this experiment, rebuilt and reflashed EXACT original
6775326FD8420D4C35E2D4E360F36AF1377B8133E0838B83082D56FDADE23D47.
Restoredimage UARTsilent restore_cpu_01 retained; sameexactsaferegs checked,
knownclockwrite, restore_cpu_02 max2/7/10/finaloff pass. Root=installed,
COM41closed. No further motorrun and no claimed new rangequalification.

Do not repeat these two closure-shape microrewrites. Further scheduling
improvement needs inert preacquisition storage with freshvalidation and
publication, not simply larger masking. Alternatively evaluate a modest
coordinated next speed profile with actual-age/arm refusal retained: a
prospective recovery limit is not permission to disable it or proof that
the motor's physical speed ceiling is300eHz. Mean-current/parity stillopen.

## Entry 273 - 2026-09-13 - Explicit310 profile, not yet hardware-qualified

Previous turn made progress by measuring/reverting slower guard construction.
Objective reread. Select a modest coordinated profile extension rather than
repeat failed microoptimizations or treat a software300floor as motormaximum.
Basis: currentbuild3/3 thirty-second5.3% recovery near284eHz,rawpeak<=579,
bus>=10793mV,queue<=3/8,DMA18us,stack4232,actualarm43us/cost13;5.4% stopped
on exact3331usguardinterval,not current/bus/tracking failure. These support
a bounded next probe, not calibratedmean-current or unlimitedrange approval.

Optional bench-range310 implies bench-range300's previously measured fast
baseline/acquisition plumbing. Explicit limits: running cyclemin3226us,
eventmin268us; passive cyclemin6452half-us,individualmin536,seedmin1075.
Slowcycle6000us,event/feedback-age1000us,current/bus/nFLT,tickgap,32usactual
armfloor,16usarmcost,originaldeadline,one recovery and duty cap unchanged.
Default250 and explicit300 profiles retain their existing constants.
Firmware RUNLIMIT declares newpair; host accepts only exact known profile,
requires matching6452/536recovery metadata, and reanchor-prefix replay uses
the matching individual/cycle limits. No refreshedseed or syntheticaccept.

Tests: new pure acquisition checks12interval requirement and6450cycle refusal;
1076tickseed wait269 acceptsage205/remaining64,refuses206 AND observed212.
Thus a newly in-range seed still cannot bypass actualarmfloor. New runtime
guard test covers3222cycle refusal versus3228acceptance,unchangedcurrent and
missing-event/feedback-age refusal. Synthetic relabeled historicalcapture
tests hostprofile matching only, explicitly NOT hardware310 evidence.
165Rust and218Python tests pass.

Release/s/thinLTO build7B1FB220176C5261023F4842E491C3251212DB6EF9A60254968B5284D518ACAE,
text114804/data1088/bss29156. E266features with bench-range310 replacing
explicitbench-range300 (inherited). NOTFLASHED; installed6775326F unchanged,
lastverifiedoff. No UART/probe/motor action thisentry. Next disabledchecks
then finite5.3% regression and5.4% hold/recovery, stopping on genuinefaults;
newprofile does not itself prove any faster operation or recovery.

## Entry 274 - 2026-09-13 -310 profile clears5.4% hold and recovery

Previous turn implemented/tested explicit310profile. Objective reread, root
hash and processownership checked, liveoff verified beforeflash. Installed
7B1FB220176C5261023F4842E491C3251212DB6EF9A60254968B5284D518ACAE,
release/s/thinLTO,E273features/size. SameG071/DRV wiring/operator11.7V/800mA,
6.2%startup50..200,+60observer. No new independentPSUreading/calibration.

range310_cpu_01 silentpreflight retained; exactRCC08000000,PD1ODR0,BDTRc1a,
CCRs0 checked beforeknownRCC08040000write. cpu_02 max2/7/10us and archive_01
three3/3disabledchecks pass. Then three sequential10soriginal-budget runs,
eachfullverifier checked beforeadvancing:

1. range310_reentry53_01:5.3% recoveryPASS284.006478eHz,13615COM/13614accepted,
   sigma45.126606us,raw520,bus11068mV. Freshseed1206,actualarm45us/cost13,
   deadline217us spare; firstpreviousfeedback634+178=812us. Phase39749.
2. range310_hold54_01:5.4% fullholdPASS289.124796eHz,17347COM/17346accepted,
   sigma52.196348us,raw534,bus11032mV,49751ADC/phase records. Initialarm55.5us/
   cost13,firstprevious367+191=558us. Recordercyclemin3318us demonstrates
   records belowold3333floor; notexactguardtimestamps or aphysicalspeedclaim.
3. range310_reentry54_01:5.4% recoveryPASS289.525343eHz,13880COM/13879accepted,
   sigma44.101849us,raw535,bus10554mV. Freshseed1166age212half-us,
   arm40us/cost13 (8us spare),deadline198us spare; firstprevious631+175=806us.
   Phase39751; resumed7989949us after2sinjection withinoriginal10sbudget.

Allqueue2/8,DMAmax18us,stackuntouched4232,COMP/COM/commitwall115/80/45us.
Allfulltiming/capture/archive/epoch/deadline checks pass; finalgates/ENABLE/
MOE/CCRs0,nFLT1,portsclosed. No guardchanges beyondexplicitE273profile,
no current-calibration or independentqZCclaim. Newprofile clears old5.4%
softwarefloor but these distinctsingleattempts are not repeatabilitycohort.
Next5.5%hold then recovery if hold evidencepasses,retaining actual32usarmfloor
and all electrical/loss-of-tracking protections. Goal remains incomplete.

## Entry 275 - 2026-09-13 -5.5% cycle refusal and memo scope clarification

Objective reread on continuation. Prior memo discussion established no new
hardware ceiling; operator explicitly keeps AM32 as a card in reserve if we
run out of ideas/headroom, not an immediate campaign or goal dependency.
Archive-only reference assessment remains current workflow. Memo's52->60.5%
CPU extrapolation conflates instrumentation builds;~302eHz derived current
recovery scheduling limit is not silicon maximum. Archivedminz includes
230/257eHz, not only528+. No goal reduction inferred from the memo.

One actual range310_hold55_01 (run before latest clarification),unchanged
7B1FB220... release/s/thinLTO,E273features,G071/DRV wiring,operator11.7V/
800mA,startup6.2%50..200,+60observer,requested10s5.5%: FAILCycleTiming12.
Stop1125259us,observed1125294,1988COM/1987accepted,rawpeak501,busmin11068mV,
queue2,DMA18us,stack4232. Exactguardstep3 previous1122029 decision1125251,
delta3222us<3226floor;stopstamp8uslater. Recordercyclemin3228different.
Meanrecorded294.531760eHz,sigma96.324795us includesstartup,not equilibrium.
Initialseed1546age272,arm57.5us/cost13;firstprevious276+261=537us.
Allfinalgates/ENABLE/MOE/CCRs0,nFLT1;fixtureterminalexit1correctlyrejects
completion,portclosed. No recoveryattemptafterfailedhold/no guardchanges.

Offline tailcontext inspection: phase3 lastsame-sector recordcycles
[3429,3368,3352,3498]us precede rejectedguard3222us. Otherphase tailcycles
range3292..3476us,not uniformlyapproaching3222. Latestphase3 recorder1122062
is33us afterguardprevious1122029. Do not add/subtract recorderandguardtimes
as ifidentical boundaries. Pattern suggests timing variation; doesNOTidentify
electricalvsISRvsestimator cause or provephysicaloverspeed.

Added drv_cycle_fault.py context CLI: fullsummaryCRC/chronology/finaloff first,
exactguard snapshot plus retainedtail per-sectorcycles,explicitseparateclock
offset,tailonly/rejectedeventnotrecorded/causenotidentified. New realcapture
regression verifies3222,phase3tail,+33us offset and rejectsfalseguarddelta.
219Python tests pass. No newmotor/flash thisanalysis continuation,installed
7B1FB220 unchanged. Next boundedfault-time reference/IRQcontext investigation,
not identical5.5retry or automaticthresholdraise. AM32 remains reserve.

## Entry 276 - 2026-09-13 - Controller snapshot after safing on cycle refusal

Previous turn produced tailcontext evidence and hostreport; objective reread.
Source audit: minz-core am32_isr::interrupt_routine applies persistence,
masksCOMP,updateslast_zc/this_zc,resetsTIM2,armsCOM,then invokes EV_ACC.
Our guard refusal occurs in that callback. Thus liveTIM2afterrefusal is NOT
the rejected edge age; last_zc is priorINTEREVENT,not priorfullphasecycle.
Existing COASTSTATE already captures shutdown-time mux/IRQ/timer state.

Added one-shot CYCLECORE12word state: step,rising,average_interval,
last_average_interval,commutation_interval,this_zc,last_zc,wait_time,
filter_level,zero_crosses,old_routine,running. Only CycleTiming inaccepted()
calls it AFTER trip() completes fullgates/ENABLEsafing. It checksdisabled,
retainsfirstsnapshot undercriticalsection,returnsbeforeEV_ACCcallbackends.
Reference IRQs stopped andcorestate notreset bylive_stop; no normalaccepted
event stats/sampling work added. observe_begin clears it beforeacquisition.
Capture-only postrun CYCLECORE explicitlyafter_safing/before_ev_acc_return;
no outputauthority or guardchanges. Diagnostic-onlycodegen cost not yet
hardwarequalified; physicalshutdownprecedessnapshotcollection.

Host validates optional singleton/exactfields/u32bounds/booleanflags,
after-safing marker andstepmatchingexactCYCLEFAULT. Summaryrejectsmalformed
newmetadata; contextCLI includes it. Syntheticprovenance/rejecttests added,
220Pythonpass. Initialcompile rejectedstaticmutreference; fixedwithmatches!
withoutcreatingreference. Release/s/thinLTObuildpasses, E273features:
A948958FD9C43472269B986AB42476BC8EABDA8BADDE8CB90C64E3E6C32B0258,
text115540/data1088/bss29204. NOTFLASHED,installed7B1FB220 unchanged,
lastverifiedoff; noUART/probe/motor thisentry.

Next disabledchecks andhealthy5.4%regression,then5.5%fault-contextcapture.
Newdata must be interpreted alongside exactguard/COASTSTATE/retainedtail;
snapshot alone won't distinguish electricalcrossing from latency causally.

## Entry 277 - 2026-09-13 - Fault snapshot shows short reference cycle, different sector

Previous turn implementedfaultsnapshot; objective/hash/processownership read,
liveoff verified beforeflashing A948958FD9C43472269B986AB42476BC8EABDA8BADDE8CB90C64E3E6C32B0258.
Same release/s/thinLTO,E273features+snapshot,G071/DRV wiring,operator11.7V/
800mA setting,startup6.2%50..200,+60observer. No newPSUreading/guardchanges.
cycle_core_cpu_01 silentpreflight retained. ExactRCC08000000/PD1ODR0/BDTRc1a/
CCRs0 checked beforeknownclockrestore. cpu_02 max2/7/10 andarchive3x3 pass.

cycle_core_hold54_01:5.4%,10s PASS288.957532eHz,17337COM/17336accepted,
sigma51.756957us,raw519,bus10925mV,arm60us/cost13,stack4356. Fullverifier
andtimingreport pass; nofaultsnapshotexpected. Then cycle_core_hold55_01:
5.5% requested10s,FAILCycleTiming12,stop266542us,observation266576us,
462COM/461accepted,raw531,bus11319mV,stack4356. Exactguardstep1previous
263312 decision266533 delta3221us<3226;stopstamp9uslater. Differentsector
fromE275step3 means priorfailures do not localize a fixed phasefault.

Actual CYCLECOREafter-safing/beforecallbackreturn:step1/rising1,average1127,
previousaverage1127,commutationinterval1113,thiszc1112,lastzc1076,wait278,
filter12,zero_crosses474,polling0,running1. Average corresponds roughly296eHz;
not controllerchangeover/obviousaverageblowup. COASTSTATE lastgate1019,
lastreads12,lastlevel1,intervalcnt53afterreset;nevercall53theedgeinterval.

Five acceptedtail referenceintervals899/1074/1132/1153/1076 plusrefusedthiszc
1112 sum6446half-us=3223us. Guardcycle3221us agrees closely despite different
sampling boundaries. Therefore recorder callback timestamp distortion ALONE
does not explain this shortcycle; it also exists in referencecounterstream.
Theseintervals include reset/readlatency and are not independently measured
physicalrotor periods. Previoussamephase recordcycle3464us and priorstep1
interevent1306 followedbystep2=899 showlong-short variation,not proof ofcause.

Bothqueue2,DMAmax18us,allfinalgates/ENABLE/MOE/CCRs0,nFLT1,portsclosed.
Motorcompletionfailscorrectly on5.5; diagnostic decodepasses. Newactual
snapshotregression verifiesdifferentsector/average/refintervalsum/guarddelta;
221Pythonpass. Installed/rootA948958F unchanged. No higherduty or recovery
attemptafterfailedhold. Next investigate crossing/estimator/ISR timing
variation without claimingphysicalmaximum orblindlyraisingcyclefloor.

## Entry 278 - 2026-09-13 - Explicit trace-off comparison, 5.5% still trips

Same installed A948958FD9C43472269B986AB42476BC8EABDA8BADDE8CB90C64E3E6C32B0258,
release/s/thinLTO/E273 features plus fault snapshot; no firmware edit or flash.
Existing coretrace0 disables per-read instrumentation, preserving filter count12
and every guard. It also changes the persistence time aperture: this is NOT a
pure logging-cost experiment. Earlier low-speed trace results already warned of
that coupling. Fixture now exposes --core-trace 0/1 and validates the singleton
CORETRACE marker. Omitted option inherits shell mode; cleanup leaves tracing OFF.

traceoff_hold54_01.txt: requested10s5.4%, PASS289.800698eHz,17388COM/17387accepted,
cycle sigma43.259212us,raw533,bus10972mV,stack4356,queue2,DMA18us,
initial arm58.5us/cost13. COMP/COM/guard/commit wall maxima92/80/18/45us.
Compared with E277 traced5.4%: sigma51.756957us and COMPmax115us. These are
single runs, not a reliability cohort or isolated causal proof.

traceoff_hold55_01.txt: requested10s5.5%, FAILCycleTiming12 at4404543us,
observed4404578us,7827COM/7826accepted,raw584,bus10769mV,stack4356,queue2,
DMA18us,COMPmax91us/COM79us. Recorded mean296.193901eHz/sigma57.274875us
includes startup and the failed window; not a qualified steady operating point.
Exact guard step4 previous4401316 decision4404535 delta3219us<3226us;
stop stamp8us later. Last same-phase recorder4401349 is33us after previous
guard stamp. Retained preceding step4 cycles3346/3382/3345/3534us again show
a long-short pattern, not proof of physical overspeed or a fixed phase fault.

CYCLECORE after safing/before accepted callback return: step4/rising0,
average1127/previous1140,commutation_interval1104,this_zc1060,last_zc1093,
wait276,filter12,zero_crosses7839,polling0/running1. Trace-dependent last-read
metadata must not be treated as fresh evidence when CORETRACE is disabled.

Both captures retained, verified final outputs off, ports closed. Failed motor
completion remains a failure even though diagnostic context decoding succeeds.
No higher-duty or recovery attempt after the failure. Tracing affects timing
but disabling it did not resolve the 5.5% short-cycle refusal; different run
lifetimes at n=1 do not establish an improvement in reliability. No thresholds
relaxed and no physical/CPU ceiling claimed. AM32 stays the operator's reserve
fallback, not the next required campaign. Host mode validation tests cover
missing/mismatched/duplicate markers and actual captures;224 Python tests pass.

## Entry 279 - 2026-09-13 - Separate reference-cycle sums from recorder clocks

Previous goal turn was progress: explicit trace-mode fixture, two retained
hardware attempts and E278 tests changed the next investigation. This turn
read the objective/current source and analyzed retained data only. No UART,
probe, flash, firmware edit or motor command. InstalledA948958F and last
verified outputs-off state unchanged; shell tracing remainsOFF after E278.

Reference interrupt_routine performs persistence, stores this_zc from TIM2,
resets TIM2, arms COM, then calls EV_ACC. CYCLECORE snapshots that rejected
event after safing; last_zc must match the last recorded accepted interval.
New drv_cycle_fault.reference_cycles checks that match, IRQ/running state and
contiguous tail sector order before summing the last11 accepted intervals
plus refusedthis_zc into two adjacent six-event cycles. Legacy/no snapshot or
insufficient tail yields unavailable, not invented data. Recorder timestamps
check continuity only; no mixed-clock cycle or inferred missing middle.

traceoff_hold55_01 reference intervals, steps5/6/1/2/3/4 repeated:
1223/1075/1208/1069/1188/1295 then885/1073/1258/1058/1093/1060.
Sums7058 and6427half-us ticks =3529 and3213.5us; pairedmean3371.25us
(~296.626eHz reference-event rate). These include reset/read latency and
are NOT independent physical rotor periods. Exactguard remains3219us<3226.
Immediately before those cycles, step4 interval930 increased to1295 while
followingstep5 decreased1223->885. Their pair2153->2180ticks changed13.5us
while the shared accepted boundary shifted182.5us. This is consistent with
crossing-time redistribution, not proof of switching/latency/electrical cause.

Traced cycle_core_hold55_01 reference pair6896/6446ticks (3448/3223us),
mean3335.5us, has a different distribution; do not treat traceoff's single
localized event as a universal mechanism. No steady310eHz or MCUceiling claim.

Source audit: sixstep_write resets TIM1 viaEGRUG everyCOM; COMP persistence
uses twelve software reads, not a fixed-duration qualification aperture.
Existing trace keeps only first32 IRQs, so it cannot identify PWM/COMP timing
at a multi-second fault. Next useful instrumentation is bounded near-fault
COMP/PWM context with explicit overhead qualification, rather than another
identical hold or changing a guard based on average cycle length.

Synthetic test shifts one boundary to create opposite adjacent-cycle errors,
checks invariance to unrelated recorder spacing and rejects mismatched last_zc,
wrong sector/polling provenance. Actual traceoff capture regression verifies
7058/6427 sums;226 Python tests pass. No safety or completion criteria changed.

## Entry 280 - 2026-09-13 - Near-fault IRQ/PWM tail captured on hardware

E279 was progress (reference-clock analysis and tested decoder). Implemented
optional bench-irq-tail: existing24-row buffer becomes rolling, chronological
dump begins at next-write slot when full; drop counts overwritten handlers.
Each record appends us_hi, giving full32bit observation-clock timestamps
without inferred16bit wraps. IRQWINDOW explicitly declares tail/time_bits32/
capacity/appended field; I85 CRC covers the additional word. Existing normal
prefix14word format remains available without feature. TRACE_NEXT resets on
observe_begin. Tail writes every tracedhandler including the safety-refused
finalcallback; tracingOFF skips collection as before. No control/guard edits.

Full host summary validates newtail metadata, CRC/width/count/capacity and
chronology. Legacy prefix still decodes. Tests cover full-widthtime/sequence
wrap, malformed/mixed formats, summary rejection and actual rollingcaptures.
229Python/165Rust pass; release/s/thinLTObuild passes with E273features plus
bench-irq-tail. Image915F90AEC7BBB2E2743BD88AB55FD87E296D1243FBC3E86E22CFDBC6FD779FCC,
text115764/data1088/bss29260 (+224text/+56bss). This is addedinstrumentation,
NOT a performance optimization; its cost must be scoped to these results.

Only unrelatedPDFpython process live; serialoff/p/i confirmedalloutputs0
beforeflash. Download/reset succeeded. Unlike recentflashes UART responded
withoutrepair. irqtail_cpu_01 disabledmax2/6/10us; irqtail_archive_01 all3x3
checks and finaloff pass. Same operator11.7V/800mA setting, no newPSUreading.

irqtail_hold54_01: explicitcoretrace1,10s5.4% PASS288.967359eHz,
17338COM/17337accepted,sigma55.787588us,raw554,bus10948mV,stack4300,
queue3/DMA18us,actualarm60.5us/cost14. COMP/COM/guard/commit maxima
120/109/23/45us; higher than preceding build, no exclusiveCPUutilizationclaim.
Fullrunverifier andtimingreport pass. Final24IRQs near10s retain correct
fulltimestamps andseq80962mod65536; CRC/windowaccounting validates.

Then irqtail_hold55_01: explicitcoretrace1,requested10s5.5%, FAILCycleTiming12
stop439044us,observed439080us,767COM/766accepted,raw510,bus11128mV,
stack4300,queue2/DMA18us,COMP118/COM78/commit45us. Exactguardstep2
previous435815 decision439037 delta3222us<3226. No furtherattemptafterfail.
CYCLECORE avg1126/prev1125,ci1138,this951,last1145,wait285,filter12,poll0/run1.
Two referencecycles7068/6448ticks,pairedmean3379us; notphysicalrotor periods.

Near-fault trace retainsseq3525..3548,OBStimes436182..439004us. Phase5
single-read rejects437183/437283us havePWMcounts721/718 (100usapart);
phase1 rejects438285/438385us have746/744. A phase5 ten-read attempt at
437346us startslevel1 thenends0, rejecting persistence; next handler437403us
accepts12readslevel1. These support PWM-linked qualification variation;
they do not independently identify true rotor-crossing time or electricalcause.

Finalhandler3548 atOBS439004us,step2,PWM1765,gate859>avg1126/2,
reads12,first/last0=expected0,masked1,accepts0,cost104us. CYCLECORE confirms
reference persistence passed andthiszc951 was sampled; independentcycleguard
then stopped it before accepted-recorder count. Therefore accepts0 here must
NOT be classified as persistence rejection (decoder correctly saysunresolved).
OBS_CLOCK and poweredguardCLOCK reset separately: absoluteOBS vsCYCLEFAULT
timestamps must never be subtracted as latency. Finalhandler cost includes
faultsafing/snapshot; not a normalacceptedhandlerbudget.

Both rawattempts retained, finalgates/ENABLE/MOE/CCRs0/nFLT1 verified, COM41
closed; shelltraceOFF aftercleanup. Currentinstalled image is915F90AE,
not oldA948958F. Next investigate switching/qualification timing using this
specific evidence; no physical/CPUceilingclaim, guardrelaxation or AM32switch.

## Entry 281 - 2026-09-13 - Measured qualification bracket; cached mode experiment built

Read objective/currentadapter/taildata. E280 wasprogress (newboundedhardware
faulttrace). This turn no UART/probe/flash/motor command. Installed915F90AE
and lastverifiedoff state unchanged; tracingOFF from E280fixturecleanup.

New drv_irq_trace.gate_accept_timing first validates fullcapture, then joins
each recordedaccepting tailhandler to exactlyone same-sector acceptedtail row
inside its OBS-clock handler bracket. Both timestamps shareOBS_CLOCK, unlike
poweredguard CLOCK. Subtracts gate_count from accepted reference_interval
mod65536, boundedbyhandlerwallcost. Excludes finalrefused event (no accepted
row), ambiguous matches and legacy16bit prefix timestamps. This measures
firstinterval-gateread toacceptedcounterread, including bookkeeping and any
preemption; NOT pure first-to-last comparator aperture or independent ZCtime.

E280healthy retainedsuccessful brackets37.5/56.5/37.5/46.5/37/37us;
failed5.5 retainedsuccessful brackets37/37/46.5/37/67us. All use12reads.
These are substantial fractions of100us PWMperiod. This is a concrete
qualification/scheduling budget, not proof of the fault's physicalcause.

Source Comp.output_level reloaded REAL_IRQ/POLL_TIMER, PHYSICAL_OBSERVATION
andTRACE_READS for everyread. Optional bench-cached-comp creates CachedComp
with source/polarity/tracing flags once per motor() invocation. Flagwriters
audited: configurationoutsidecontrollerinvocations, traceflagset beforecall;
safetyinterrupts mask/stop but don't mutate these read-mode flags. Eachread
still performs freshhardwareCSR (or syntheticLEVEL), and allper-read trace
counterwrites remain. All comparator control/EXTI methods delegate to original
Comp. No pending/signal/safety cache, no filtercount/guard/referencecode edits.
COMPMODE cached_per_call=1 signal_cached=0 safety_cached=0 identifies build
in capture; host rejects malformed/duplicate provenance. Disabled feature
retains original reload-on-each-read adapter via shared inline readhelper.

Build passes release/s/thinLTO,E280features+bench-cached-comp:
807372B6F101925F2AA197690B1FA71975315F90B7716B1DDDB1B11640ED10BA,
text115980/data1088/bss29260. +216text/no staticRAM increase against E280;
CachedComp output_level compiled124bytes versus old204bytes, which is NOT
a runtime speedup measurement. Copy/setup overhead and changed persistence
time aperture require hardware A/B. 231Python/165Rust pass (hostRust suite
does not exercise physicalHAL timing). Initialbuild before provenance marker
was superseded by thishash, neitherflashed.

Next flash/disabledchecks then known5.4%10s timingregression with explicit
coretrace1 and same tailinstrumentation. Require fullrun/capture/finaloff,
unchanged32usarmfloor/16usarmcost/100uscommit guard, rawcurrent/bus/ageguard,
stackspan>=4096/untouched>=512 before5.5faultattempt. Revert experiment if
it doesn't earn its complexity; no thresholdrelaxation or speedupclaim now.

## Entry 282 - 2026-09-13 - Cached adapter reduces measured bracket, not the fault

Re-read objective/currentstatus and confirmedroot807372B6F101925F2AA197690B1FA71975315F90B7716B1DDDB1B11640ED10BA.
E281 wasprogress (measuredbracket analysis andbuiltoptionaladapter). Only
unrelatedPDFpython process live. Serialoff/p/i verifiedallgates/ENABLE/MOE/
CCRs0,nFLT1 beforeflashing. Download/reset succeeded; cachedcomp_cpu_01
silentpreflight retained. ExactprobeRCC08000000/PD1ODR0/BDTRc1a/CCRs0
verified beforeknownRCC08040000UARTclockrestore. cachedcomp_cpu_02 passes
disabledmax2/6/10us, cachedcomp_archive_01 all3x3checks/finaloff pass.

Same E281release/s/thinLTOfeatures and11.7V/800mA operator setting; no new
PSUreading orguard changes. Explicitcoretrace1 for matchedtail comparison.
cachedcomp_hold54_01:10s5.4% PASS289.422271eHz,17365COM/17364accepted,
sigma53.402654us,raw543,bus10996mV,actualarm47.5us/cost13,stack4284,
queue2/DMA18us,COMP/COM/guard/commit max115/81/24/45us. Fullfixture and
timingreport validate. COMPMODE confirmsactualcachedadapter. Retainednormal
accepted gate-to-counter brackets29.5/38.5/47.5/39/29.5us versus E280healthy
37.5/56.5/37.5/46.5/37/37us. Shortestbracket reduced7.5us; differing samples
andpreemption prevent claiming exactwhole-run saving or reliabilitygain.

Then cachedcomp_hold55_01:requested10s5.5%, FAILCycleTiming12at1205461us,
observed1205496us,2130COM/2129accepted,raw504,bus11199mV,stack4284,
queue2/DMA18us,COMP112/COM80/commit45us. Exactguardstep5previous1202230
decision1205453 delta3223us<3226,stopstamp8uslater. CYCLECORE average1122/
previous1127,ci1129,this1114,last1055,wait282,filter12,poll0/run1. Reference
cyclepair6927/6434ticks,pairedmean3340.25us,notphysicalrotorperiods.
Retainednormal gate-to-accept29.5..59.5us; finalIRQseq9740OBS1205435us,
step5PWM3492/gate1055/avg1122,12readslevel1=expected1,masked1/accepts0,
cost99us includingfaultsafing. Referencequalificationpassed; independent
cycleguard refused. Decoder correctlydoesnotcallthis persistencefailure.

Both rawattemptsretained,fullcapture/finaloutputs0/nFLT1 verified,COM41closed,
shelltraceOFF aftercleanup. No higherduty/recovery afterfailedhold. Installed
androot807372B6 unchanged. Optimization measurablyshortenedtracedbracket
but didnotresolve5.5%fault; lifetimesn1arenotreliabilitystatistics. Next useful
test is cachedadapter withtracingOFF against E278normalpath, not another
identicaltracedfailure. Keepallguards; no CPUceiling/physicalspeedclaim.
Actualcachedcapture regression checks bracketvalues andrefused12-readoutcome.

## Entry 283 - 2026-09-13 - Cached comparator, tracing off: three 5.5% holds pass

Objective/currentimage reread; precedingturn wasprogress (hardwarecachedmode
A/B andretainedfailure). Same installed/root807372B6,no flash/sourcechange.
Only unrelatedPDFpython live; serialoff/p/i confirmedgates/ENABLE/MOE/CCRs0,
nFLT1. Explicitcoretrace0 for everyattempt;same11.7V/800mA operator setting,
no newPSUreading, no guardchanges. Cachedmode marker validated in captures.

cachedoff_hold54_01:10s5.4% PASS290.336256eHz,17420COM/17419accepted,
sigma41.350457us,raw577,bus10566mV,arm58us/cost13,stack4284,queue2/DMA18,
COMP/COM/guard/commit max86/81/17/45us. Fullfixture/timing pass. Compared
E278uncachedtraceoff:COMP92us andIRQunion56.7372%; cached56.3232%. These
singlecross-buildruns demonstrate a modestmeasured difference, not isolated
CPUutilization/headroom or reliablefaultelimination. Foreground isnotidle.

cachedoff_hold55_01:10s5.5% PASS296.746958eHz,17804COM/17804accepted,
sigma44.433363us,raw598,bus10937mV,arm46.5us/cost14. This initialsuccess
triggered a predeclared TWO morematchingattempts, stoppingonanyfailure:
- cachedoff_hold55_02 PASS296.900030eHz,17814COM/17813accepted,
  sigma44.545876us,raw592,bus10865mV,arm58.5us/cost13.
- cachedoff_hold55_03 PASS297.522470eHz,17851COM/17850accepted,
  sigma45.692794us,raw564,bus10901mV,arm58us/cost13.
Bothcompleted10s; allthreeCOMPmax86/COM81/commit45us,stack4284,queue2,
DMA18us, fullCRC/acquisition/ownership/ADC/timeline/provenance/finaloffpass.
Guardmax18/17/24us. IRQunion5642080/10000012,5629017/10000011,
5639229/10000009us (~56.3..56.4%); excludes unmeasuredexceptioncost and
doesnot make remainingtime idleheadroom. Acceptedrecordermin03=3223us
isnot exactguardinterval (floor3226); distincttimestampboundaries can differ.

Manifest captures/cachedoff_hold55_cohort.csv retains exactrawSHA256,build,
settings andoutcomes. Thisconfiguration3/3short holds passes where preceding
uncachedtraceoff n1 andcachedtraced n1 failed; neitheroldfailure is erased
or pooledinto thisdifferentconfiguration. No statisticalcausalproof orlong-
duration/recoveryqualification. Actualmanifestregression revalidatesrawhashes,
fullrun completion,trace0/cachedprovenance andreportedcounts/metrics.

Allportsclosed,alloutputsverifiedoff,nFLT1,shelltraceOFF. Nohigherduty or
recoveryattemptthisentry. Next bounded5.5recovery on sameimage/trace0;
actualarm32usfloor/cost16us ceiling andoriginaldeadline remain decisive.
The optimization has earned continuedqualification, not blanketadoption or
a claimthatthe fullgoal iscomplete.

## Entry 284 - 2026-09-13 - Planned 5.5% recovery aborted before injection

Re-read objective/status. E283progress: threeactualshort holdpasses andraw
manifest; that is not robusthold/recoveryproof. Same installed807372B6,
explicitcoretrace0,no flash. OnlyPDFpythonlive,serialoff/p/i preflight confirms
gates/ENABLE/MOE/CCRs0,nFLT1. Attemptcachedoff_reentry55_01 requested10s5.5%
withdropoutat2s andoneboundedreentry,unchangedguards/originaldeadline.

FAIL initialCycleTiming12 at1745911us,BEFORE2sinjection; REENTRYresult1,
first_injection_us=0,resume=0,remaining=0,REENTRYSTATSused0. No recovery
staging ornewarm occurred. Exactstep6 previous1742674 decision1745897
delta3223us<3226,stopstamp14uslater.3103COM/3102accepted,raw584,
bus11140mV,8685ADCscans,queue2/DMA18us,stack4284,COMP92/COM81/commit45us.
CYCLECORE average1129/prev1120,ci1085,this1085,last1118,wait271,filter12,
poll0/run1. Referencecyclepair6947/6436ticks,pairedmean3345.75us. No
physicaloverspeed/causeproof. Fullcapture/context validates; finaloutputs0,
nFLT1,portclosed,shelltraceOFF. No secondpoweredattemptafterfailure.

Thisadds0/1completedrecoverycampaigns with0actualinjections/recoveries,
NOT a failedreacquisition orreentryarmlimit. E283holdcohortremains3/3 inits
settings, but combinedevidence contradictsanyblanketrobust5.5qualification.
Sourceaudit: foreground samplesobservation_elapsed unconditionally inboth
normalanddropout modes. Before2s, inject addsconditionalcomparison butdoes
not maskCOMP/changefilter/timers. No causalclaim thatarmingrecovery caused
thefault, nor thatoptimization eliminatedthe underlyingvariation.

Fixturepreviouslyreported 'reentry statistics staging provenance failed'
because it demandedused1 wheneverreentryrequested. Newearly-refusal branch
validatesactualinitialacquisition/transfer/fullsummary thenALWAYSraises an
explicit 'initial segment stopped before dropout; recovery not exercised;
reason=12' failure. It neveracceptsfailedcampaign orrequiresrecovery-only
armarchive before recoveryexists. Actualcaptureregression and234Pythonpass.

Next5.4%recoverycontrol onthiscached/traceoffimage (notyetqualifiedthere)
beforehigherduty ormoreidentical5.5attempts. Needseparate stableinitialhold
fromrecoverymechanics. No guardschanged/objectivereduced/AM32switch; no
claimthatthe fullgoal or5.5recovery hasbeenachieved.

## Entry 285 - 2026-09-13 - 5.4% recovery control passes 10s and30s

Objective/currentstate reread. E284wasprogress:retainedearlyfailure,accurate
stageclassification; no actualrecoverytestedthere. Sameinstalled807372B6,
explicitcoretrace0,no flash/firmwarechange. Processownershipclear exceptPDF;
serialoff/p/i confirmsallgates/ENABLE/MOE/CCRs0,nFLT1 beforecampaign.
Sameoperator11.7V/800mA setting,no newPSUreading orchangedguards.

cachedoff_reentry54_01 requested10s5.4%:PASS. InjectionOBS2000087us,
FIRSTSEGfault8stop2001009/end2001045,3473COM/3472accepted,raw503,bus11104.
Freshacquisition7454us,seed1167/actualage212,remainingARR80ticks=40us,
armcost13us. Resumed7989867us near290.548783eHz,13929COM/13928accepted,
sigma30.099004us,raw548,bus10937mV. Originaldeadline227usspare; noresetbudget.

Afterfullfixture/timingvalidation, extendedsamecontrol to30soriginalbudget:
cachedoff_reentry54_30s_01 PASS. FIRSTSEGfault8stop2000610/end2000644,
3478COM/3477accepted,raw550,bus11116. Freshacquisition7551us,same1167seed/
age212/40usarm/cost13. Resumed27990179us near291.009858eHz,
48873COM/48872accepted,sigma29.633644us,raw580,bus10638mV. Originalend
34711326/final34711110 leaves216us. Mean/sigma describe resumedsegment,
not entirestartup or an independentrotorclock. Initialcurrentbaseline must
not be reused after recoveryENABLElow (existingepochverifier retained).

BothCOMPmax86/COM81/commit45us,queue2/DMA18us,stack4128untouched,
freshfeedbackdeliveryvalid,allrawCRC/ownership/seed/recoverycycle/accepted-
eventtimeline/originaldeadline/finaloffchecks pass. Finaloutputs0,nFLT1,
COM41closed,shelltraceOFF. No furthermotorattemptthisentry.

This isolates functioningrecoveryat5.4% from E284initial5.5%CycleTiming;
it is not proof5.5recoveryworks, nor a substituteceiling/objective. Updated
TIMING_HEADROOM/BEMF_PARITY_PLAN/PORTABLE_WINS frontsections withcurrent
build-scopedevidence andknownlimits, supersedingstaleE263/E270status.
No claimedcurrentcalibration/independentqZC/physicalmaximum/AM32dependency.
Next addressintermittentaccepted-cycleboundary for expansion, preserve the
nowmeasured10s/30s5.4%recoverycontrol andallpriorfailedattempts.

## Entry 286 - 2026-09-13 - Initial-run current evidence, no calibrated-current claim

Re-read objective/currentmeasurementplan. E285progress:10s/30srecovery
controls andupdatedsummaryartifacts. Workedonremainingcurrentrequirement
withoutshrinkingduty-ledexpansionobjective. Same807372B6/trace0/no flash.
AskednonblockingPSUsettledcurrent+voltagereading forupcoming30s5.4%spin;
no answer receivedbyentrytime. OnlyPDFprocesslive;serialoff/p/i preflight
verifiedgates/ENABLE/MOE/CCRs0,nFLT1.

cachedoff_current54_30s_01:initialhold(no recovery),30s5.4% PASS290.862542eHz,
52355COM/52354accepted,sigma34.034498us,raw588,bus10817mV,149253ADCscans,
queue2/DMA18us,COMP86/COM82/guard23/commit46us,initialarm45.5us/cost14,
stack4128. Fullfixture/timing/currentresidualdecoder validates,finaloutputs0,
nFLT1,COM41closed,shelltraceOFF. RawSHA256
0d6390ba031fd986f2c96c5437f2d86180ba374d9d1bc23dd83580440f33dd86.

Baselinecenteredmeans8.6796875/10.6953125/9.6875 counts;poweredsignedmeans
9.916852593/9.126422919/9.930915961. Summedresidual−0.088308526counts.
Sameinitialepochdoesnotproveoffsetstationarity or calibratedamps. Prior
traceoff_hold54_01,cachedcomp_hold54_01,cachedoff_hold54_01,cachedoff_hold55
01/02/03 yield−5.0502/+4.3808/−4.8375/−0.4320/−6.9857/−6.5069counts.
Differentbuilds/tracing/settings andbaselines; this is diagnosticspread,
not isolateddrift, negativebuscurrent or confirmedpolarity inversion.

Sourceconfoundconfirmed: baseline individualsoftwarechannels4/1/0/6/13 via
read_channel; poweredDMAascending0/1/4/6/13 scan/201us. No claimthiscaused
residual, butsame-modegates-offcomparison is actionable withoutmotorstress.
UpdatedCURRENT_MEASUREMENT_PLAN withnewraw evidence/explicitmissinganchor.
Do not reusehistorical70mA, applyinitialbaselineafterrecovery, or keep
repeatingholds awaitingoperator. Addedactual30scapture regression retaining
uncalibrated/unknown-drift flags. No firmware/waveform/guardchanges thisentry.

## Entry 287 - 2026-09-13 - Idle SW/DMA/SW comparison, no stable mode correction

Objective/ADCstream/prestart/statistics source read. Previousentryprogress:
retained30sinitialmetrology andactionablemodeconfound. Implementedoptional
bench-baseline-dma (currentbaseline+DMAfeedback) idlecommandbasemode.
Admission requiresENABLElow,outputsdisabled,no powered/forced owners or
activeTIM3/DMAproducer. WakesCSA once, takesSW/DMA/SW128x5stats with50ms
perphase checks, outputsdisabled/nFLT/token validity checks. NoPWM/gate
commands, nooffsetapplication. PhysicalENABLElow precedesstreamcleanup.

ADCstreamstart delegatesprivate start_inner(idle_probe). Normalpathrequires
poweredowner; idlepath requiresfeature,gatesoff,ENABLEhigh,no poweredowner.
No fakeguardownership andDMAIRQneverunmasked forprobe; foregroundusesexact
coherentpoll/ADCrestore. Diagnosticsorigin0notstaleguardclock. Same hardware
ADCsequencer/201us cadence/1usfirsttrigger/phaseDMA; notsame ISRworkload as
poweredrun. SWloop channelorder matchesprestart, but checking/gap placement
differs. No claimofidenticalcompleteacquisitiontiming orzeroactualcurrent.

Raw15BM85 rows (mode+existingmomentwords) CRCprotected; BASEMODEexplicit
samewake/gatesoff/nooffsets/dma_polled. Hostexclusivecapturefinallyoff validates
completion/counts/channels/moments/cadence/timing andpostterminatorp/i. Reports
mode-minusSWmidpoint andSWend-start only,ampsNone/driftcorrectionunproven.

Buildrelease/s/thinLTO,E281features+bench-baseline-dma passes:
D40A2D883E133A57CD8B69DE3CF203A4EDBFFCAF058E34DCFBC03C16E9C76028,
text117976/data1088/bss29260. Samephysicalpreflashoff/p/i verified,onlyPDF
pythonlive. Download/reset; basemode_cpu_01silentretained. ExactRCC08000000/
PD1ODR0/BDTRc1a/CCRs0 checkedbeforeknownUARTclockrestore. cpu_02max2/7/10
pass. Thisimage isinstalled; poweredpathnotrequalified,oldmotorbaseline807372B6.

Threeactualgates-offprobes basemode_compare_01..03 pass:
- phaseelapsed11578/25623/11579us; currentDMA-minusSWmidpoint
  [2.70703125,0.03515625,-0.6796875],sum+2.0625counts.
- 11578/25623/11579us; [1.4765625,-1.453125,-2.1171875],sum−2.09375.
- 11579/25624/11579us; [0.19140625,0.0390625,0.94921875],sum+1.1796875.
SWend-startsums3.40625/1.921875/1.09375counts. Changing signs andbefore/after
variation do not support a fixedmodecorrection; cause/settling/drift notisolated.
VREFmode-minusmidpoint +0.65625/+0.4296875/+0.5859375counts,notampscaleproof.

Afterprobesarchivecheck3x3pass/finaloff. No motorspins; outputs0/nFLT1verified,
portsclosed. Nextidle settling/repeatability control,not offsetapplication;
independentPSUanchorstillabsent. Updatedcurrentmeasurementplan; actualthree-
capture regression retainsrawdifferences/no-calibration semantics.165Rustpass;
hostsuite includes newprotocol/actualcapture tests. Flashresettracemode: next
motorfixture must explicitlysetcoretrace0 andrequalifynewimage timing.

## Entry 288 - 2026-09-13 - Measured settling comparison and readiness-order fix

Objective/currentprobe reread; E287progress:threecoherentidlecomparisons,
notcalibration. Addedselectableidle1ms/20mssettling (`basemode`/`basemode20`)
measuredbyTIM17; BASESETTLE hostvalidatesrequestedtarget/actualduration.
Motorstartup,rawcurrentguardsandwaveform unchanged. Firstbuild2E4DB0DF...
flashedafterverifiedoff; silentcpu_01/exactsafeRCC08000000/PD1ODR0/BDTRc1a/
CCRs0 thenknownUARTclockrestore,cpu_02max2/7/10pass.

Plannedorder1/20/20/1/1/20ms stoppedonfirstfailure:basesettle_1ms_01
complete0/fault4/samewake0/measured61us. Initialrevision calledbegin() just
afterENABLE, beforewait; beginrequiresnFLT1, so no token existed duringwake.
Failure is diagnosticordering,not a failedDMAmeasurement;raw/finaloffretained.
Correctedtoexistingstartupordering: duringfixedsettledelayrequireoutputsdisabled
andENABLEhigh, recordwhethernFLTlow; afterwardbeginrequiresnFLT1 andvalid
epoch beforeanysamples. No faulttimeout extension or motor-guardrelaxation.

Corrected4DA1EE7E3B8CAEDF8E7851E136DF197D9E93242CC1A05692845E4DE2AFB3FD70,
release/s/thinLTO/E287features,text118872/data1088/bss29260,flashedafter
failedprobefinaloffverification. Again silentfix_cpu_01/exactsameregs/UART
clockrestore;fix_cpu_02max2/7/10pass. Allsixcorrectedtrials pass inplanned
order1/20/20/1/1/20ms, namedbasesettle_fix_{1,20}ms_01..03. Settledtimes
1000..1001us/20000..20001us; fault_low_seen1 onall,butreadybeforeacquisition.

Current-channel sumsDMA-minusSWmidpoint:
1ms4.109375/3.44140625/2.875counts;
20ms−1.08984375/+0.6953125/−1.05859375counts.
SWafter-before sums1ms3.3125/.8046875/−.625;
20ms−.2578125/2.703125/1.6328125counts. Longerwaitchangesmodecomparison
butdoesnotconsistentlyreducewithin-wakevariation. Smalln, fixedSW/DMA/SW
order anddifferentautocalibrationepochs preventstableoffset/driftcorrection
orampsclaim. No motorstartupdelaychanged. No independentPSUreplyreceived.

Allrawdata/CRC/moments/count/timing/provenance/finaloffvalidated;archive3x3
afterprobespasses. Alloutputs0/nFLT1,portsclosed;NOmotorcommands. Installed
4DA1EE7E motorpathnotqualified; knownmotorbaseline807372B6. Testsretain
firstfailureandsixsuccessfulprobeoutcomes. Boundthismetrologybranchhere;
nextreturntowaveform/referenceaudit for5.5timingboundary,notendlesssettling
sweepsorrelaxingguards. Currentcalibrationremainsanexplicitobjectivegap.

## Entry 289 - 2026-09-13 - Frozen-reference PWM audit and pure role plan

Objective reread; E288progress:sixmeasuredsettlingprobes/orderingfix, bounded
metrologybranch. NoUART/probe/flash/motor thisentry. Installed/root4DA1EE7E
unchanged,lastverifiedoff; motorqualifiedbaseline remains807372B6.

Inspected frozenarchive2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44;
tim1_motor_pwm.rs archived/liveSHA both
cb9f06a60308c80374212d985c8b45466f6044a04bbc030277607e9a245a79af.
Reference set_roles_for_step -> set_phase_roles writesGPIOBSRR/MODER,
notCCRorEGR; allthreeCCRs receiveequalduty via separate set_duty. This
explicitlyavoids staleper-phasezero-preload windows. Binz sixstep_write
clearsbridge/CCRs,rewritesCCMR/CCER/source-onlyCCR,issuesEGRUG,muxesCOMP,
enablesMOE eachCOM. CachedG071PACtim1/egr.rs saysUGreinitializescounter.
Thuscarrierrestart is a verifiedimplementationdifference, notproof ofthe
5.5timingfault. DoNOTsimplyremoveUGwhilekeepingper-phasepreloadedzeros.

CreatedPWM_ROLE_EXPERIMENT.md withsourceidentity/comparison andstrictlive
integrationrequirements: equalactiveCCRs beforefirstenable; no per-COMclear/
UG; guard+writeatomic; MOEclearandbothports'gateGPIOlatchesclearbefore
newsink; globalstop/panic mustclearGPIOlowsaswell; restoresineAFassumptions;
explicitcapture/disabledpreflight andguardedqualification. Existingpolarity,
deadtime,electrical/age/armguards retained. Carrierreset+blanking affects
pulsearea/timing, but noquantitativevoltagechange/causeclaimwithoutmeasurement.

Newpureexamples/support/phase_gpio_plan.rs yieldsBSRR/MODERfields only,
NOoutputauthority orshellintegration. Preservesactualbinzlogicalsequence
andA PA10/PB1,B PA9/PB0,C PA8/PA7; doNOTcopyreferencephaselabels blindly.
SourcebothAF,sinkhighGPIOlow/lowGPIOhigh,floatbothGPIOlow. Testsall6sectors,
explicitpin cases,unrelatedpinpreservation,noBSRRset/resetconflict,invalid
sectors.169Rusttests pass (141+6+1+21),M0no_stdcheckpasses. Referencecode
unchanged. Next implementoptionalreference-stylecarrierexperiment against
thiscontract,not moreidentical5.5runs or a newlyinventedphysicalceiling.

---

## Entry 290 — 2026-09-13 — Optional BEMF role carrier implemented, not flashed

Read GRAYBEARD_ENVELOPE_AND_SENSE.md. Its per-COM UG suspicion agrees with
E289 source audit, but the memo incorrectly assumes binz already has equal
CCRs/GPIO roles. Detection qzc and analog-cause claims remain unproven;
COM:accepted is not an independent detection denominator. No hardware changes.

Added bench-pwm-roles and phase_role_sequence/live modules. Only guarded
powered_timer::commit selects this new path; open-loop/forced startup stays
baseline10kHz for the initial comparison. Existing guard+write critical section
is retained. Sequence: MOE clear, both GPIO-port gate latches clear, initial
equal-CCR345 at5.4%/PWM modes/CCER/UG if unprepared, MODER, GPIO sink, same
COMP mux/polarity, MOE enable. Subsequent commutations have no CCR/UG traffic.
Fixed duty per prepared segment avoids asynchronous mixed-preload duty writes;
unexpected duty change invokes board-local safing panic. bridge_clear resets
preparation and restores all six AF modes only after clearing MOE/CCRs/GPIO.
No safety threshold or reference core code changed.

Sequence host tests cover all36 transitions, initial load, invalid values,
and re-preparation. Added idle-only rolecheck: ENABLE remains low, checks
MODER/ODR/GPIO-role IDR/CCRs/modes/mux and TIM17-vs-TIM1 counter continuity;
clears all outputs before printing. Numeric disabled gate is6 rows flags127,
restored1,disabled1 plus finaloff. UNRUN; strict host fixture/provenance still
needed before flashing/testing. PWMROLES provenance is emitted only in the
existing fixture dump. This is not a waveform or motor qualification.

Candidate ELF SHA256A54DCFEAA449C83AF4300DF3600F7A858FC138026BDF9554633DA01FEAB3B365,
release/s/thinLTO/codegen1,text118720/data1088/bss29264. Features recorded in
PWM_ROLE_EXPERIMENT.md: E285motor feature set plusbench-pwm-roles, without
idlebaselinedma.173Rusttests/M0no_stdcheck/244Pythonpass; existingwarnings.
NOflash/serial/motorrun. Installed remains4DA1EE7E,knownmotor807372B6.
Next fixture+disabledpreflight, then guarded comparison; alltiming/current/
entry/recovery/envelope goal requirements remain active.

---

## Entry 291 — 2026-09-13 — Role carrier on hardware: three holds, changed duty/speed

CandidateA54DCFEAA449C83AF4300DF3600F7A858FC138026BDF9554633DA01FEAB3B365
flashed/reset afterUARTverifiedoff. First roles_disabled_01 wasUARTsilent,
retainedfailure. ReadRCC08000000,PD1ODR0,BDTR0c1a,CCRs0; onlythenrestored
documentedUARTclock08040000. roles_disabled_02 passedall6flags127,8us each,
TIM1delta492..494ticks,GPIOmodes/latches/pins/CCRs/mux/continuity/restore/off.
roles_cpu_01 max2/7/10us;roles_archive_01 3checks x3trials,finaloff.
Strictnewdrv_role_check parser checksroworder/completeness/counterarithmetic/
restored/finaloff. CentraloptionalPWMROLESvalidation andfixture--pwm-roles
requireexactbuildpathmarker.247Python tests pass; firmware unchangedfromE290.

Declared eachpoweredgate inPWM_ROLE_EXPERIMENT.md BEFORErun. All10s,
phase60,coretrace0,unchanged6.2startup,existingnumericguards/800mAlimit;
no newoperatorPSUreading. Fullcapture/CRC/ownership/ADC/timeline/arm/off pass:

| capture | duty | mean eHz | cycle sigma us | COM/accepted | raw peak | bus min mV |
| --- | --- | --- | --- | --- | --- | --- |
| roles_hold54_01 | 5.4% | 268.346942 | 40.544506 | 16101/16100 | 458 | 11092 |
| roles_hold55_01 | 5.5% | 273.691291 | 39.741862 | 16422/16421 | 446 | 11068 |
| roles_hold58_01 | 5.8% | 287.794436 | 39.681237 | 17268/17267 | 456 | 11044 |

SHA256 respectivelya395567815c5e0783864d990a245c22756cb208a9573388281eb0cd784bf79cf,
f0d7ed8abcb901b9ec7f372a78031c287f2f686a34fd5c4ade950418423f1a9b,
4bac27a141625f43f1200e0332853641be98595fdec577a779d03917cab59f15.
Initialarms57/59.5/58.5us,cost14each,COMP86each,COM100/100/101us,
commit49/44/44us,guard19/18/25us,DMA18/queue2all,stack4128all.

Concreteeffect: same5.4% now268eHz vsold~290eHz. No causalclaimaboutpulsearea
withoutdirectwaveformmeasurement. Near-speed5.8%287.8sigma39.68vsold5.4%
290.3sigma41.35 ismodest,n1,notproofjittercured; COMmaximumhigherthanold81.
Thesearethreeinitialholdpasses atthreesettings,NOTarepeatedrecoverycohort,
notindependentqzc/currentparity,notexpanded speedenvelope. Nextinspecttiming
tradeoff/testnearold297edgewithunchangedguards,thenrecoveryqualification.
No caps,24kHz,guardloosening,AM32switchornewreferenceboardexperiment.
Alloutputsverifiedoff,portsclosed; A54DCFEA remainsinstalled.

---

## Entry 292 — 2026-09-13 — Near-edge hold passes; role-only carrier does not cure recovery fault

SameinstalledA54DCFEA,10kHz,phase60/coretrace0/unchanged6.2%startup.
Numeric gates declared beforeeachrun inPWM_ROLE_EXPERIMENT.md; no limits
changed. No newoperatorPSUreading;800mAsetting retained,rawcurrentnotamps.

roles_hold60_01 PASS10s at6.0%:296.270732eHz,17777COM/17776accepted,
sigma39.531078us,raw453,bus10984mV,arm59.5us/cost13,COMP86/COM101/
commit44/guard25us,stack4128/finaloff. SHA256
dcfb7c15f7c7bc29bf7c29ab009e86134adc616ec578dca5edc2ce6f69b73c8f.

roles_reentry60_30s_01 FAIL retained SHA256
27bff371130ef414ef08191c343facca293c9ea1e43efbdd46825e886870e089.
Actualdropout2000024us,FIRSTSEGstop2000909/end2000944,3572COM/3571accepted,
raw567/bus11128. Fresh12intervalacquisition7209us,seed1129,age214halfus,
actualarm34us/cost14 legitimatelyarmed. Resumed92909us166COM/165accepted
thenCycleTiming12:step3previous89640/decision92861/delta3221<3226.
CYCLECOREavg1120/previous1103,ci1125,thiszc1038,lastzc1002,wait281,
filter12,zc178,poll0/run1. Reference6intervalsums6790/6434halfus are not
independentrotorperiods. Raw396/bus11175,stack4036; finaloffverified.
ThusperCOMUG removal DOES NOT eliminate this fault class. No physical
overspeed or analogcause proven; no threshold change/retry withinattempt.

roles_reentry58_30s_01 PASS SHA256
dbfcd2f66c1c8e75d9564f49fefb8b11618c66e924c0a64d88129c9e82decd76.
Fresh12intervalacquisition7516us,seed1167,actualarm39us/cost14.
Resumed27.990049s289.907922eHz48687COM/48687accepted,sigma36.655782us,
raw453/bus11056,COMP86/COM101/commit43/guard24us,DMA18queue2,
stack4000,originalend34712185/final34712022=163usspare. Fullrecovery/
CRC/archive/timeline/ADC/arm/deadline/finaloff verifierspass. Singlepass,
notrepeatedreliabilitycohort orindependentqzc/currentparity.

Nextdistinctlever iscarrierfrequency, notmoreidenticaledge repeats. Audit
found fixed6400 inrolecompare/ADCphasemetadata and192/320sampletargets/
ARR6399 assumptions indrivenPWM DMA. A24kHz experiment must handlethese,
DRVinputminimum/deadtime andrestoration beforepoweredtesting; changingARR
aloneinvalidatesduty/capture. ConsiderBEMF-onlyswitch tokeepstartuptiming
unchanged. Alloutputsverifiedoff/portsclosed,A54DCFEAremainsinstalled.

---

## Entry 293 — 2026-09-13 — Carrier geometry foundation and DRV pulse specification audit

No hardware action. Read cached drv8304.pdf SLVSE39B SHA256
df052f7308b0c8e0dd8759292ac070de936788f43f3a54aa16d7c78babb2db61,
pages7/8/10/21-24. HW tDRIVE4us is NOT minimumPWM: page23 states it ends
on a new command and doesnotextendPWM. Propagation180typ/250maxns and
HWdeadtime120ns do notconstitute a guaranteedminimuminputpulse. No such
minimumfound inreviewed specification; actualgatecharge/IDRIVE/commutation
blanking remainhardwareeffects,notvalues to assume away.

Addedtestedcarrier_profile geometry6400/2666ticks,exactHz10000/24006,
boundedcompare,phasebin andidlecounterarithmetic. sequenceapply_carrier
useschosencompare,whileexistinglivewrapper stays10k. Atcandidate24kHz,
4.0/6.0/6.2% idealMCUhigh after26tickdeadtime is1.25/2.078125/2.171875us;
notguaranteedgate/MOSFETconduction.178Rusttests/M0no_stdcheckpass.
Release/s/thinLTObuildpass,text118712/data1088/bss29264;
rootELF593DA14E34B7F2F07959B4E74DFBAB8F72D385187D4B8BCF5BD832D25E4BBC4A
NOTinstalled. No24kliveoptionorperiodwrites introduced;hardwareA54DCFEA.

ADCstreamensure_started runsfromforeground, so changingperiod atfirstCOM
couldmixperiods. Nextintegration mustestablishperiodwhilebridgeoffbefore
ADCstart,retainperiodsnapshotforhistogram aftersafingrestores10k,update
idlecheck/strictmetadata. Keepstartup anddriven192/320PWMtargets at10k.
Measurefreshseedtimingcostwithunchangedguards. No RC/capsdecision frommemo;
couplednetworkrequiresanalysisbeforeits suggestedtauvalues are actionable.

---

## Entry 294 — 2026-09-13 — 24 kHz BEMF-only carrier runs, but costs more and does not improve jitter here

Implementedbench-pwm-24k impliesroles/DMA. core_bench disabledsetup sets
ARR2665 beforeguard/ADCstart; preparationrefusesoutputsactive/guardowner/
DMAch1active/nonzeroCCRs. applychecksARR; no perCOMperiodchanges. bridge_clear
restoresARR6399afterMOE/CCRs/GPIOclear. Startup/driven192/320sampling stays10k.
ADCphase snapshotsperiod6400/2666 atprepare, binsaccordingly, retainsperiod
afterstop. Hostbinds--carrier-hz24006, --period-ticks2666 andrejects24k
provenancewithnonempty6400phasecapture. IdlecheckalsoverifiesARRrestoration.

249Pythonpass,release/s/thinLTOtext119368/data1088/bss29264,
SHA2563DEAC2F08CCB386876C221756F4D3E5CE2EB9FC75DE6BD893E56F6C0D31C336F
flashed/reset afterverifiedoff. carrier24_disabled_01 UARTsilent retained;
RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 verifiedbeforeknownUARTclock08040000
restoration. carrier24_disabled_02 all6flags127,12..13us/779..781ticks,
restored10k/off. carrier24_cpu_01 max2/7/10;archive_01 3x3/finaloffpass.

Predeclaredone10s at5.8then6.2powered duty; unchanged6.2startup/phase60/
coretrace0/electrical/age/cycle/actualarm/commit guards,800mAsetting. No new
operatorPSUmeasurement. Fullfixture/ADC2666/CRC/timeline/finaloffpass:

| capture | duty | mean eHz | cycle sigma us | COM/accepted | raw peak | bus min mV |
| --- | --- | --- | --- | --- | --- | --- |
| carrier24_hold58_01 | 5.8% | 254.266759 | 58.848151 | 15256/15256 | 245 | 10925 |
| carrier24_hold62_01 | 6.2% | 278.837228 | 58.944872 | 16731/16730 | 268 | 11128 |

SHA256ab20021324de6a73a6b4ea6c994fae9659d0bdcde2d72784c3e3a87461d133fc
anda8437a2d4fc4386c47cb2858c47eb43e00a956c6ed17ac85d0001f547307c251.
Arms54/56us,cost13/14,COMP87both,COM147/199,commit48both,guard25both,
DMA18/queue3then2,ADC49751each,stack4128/4120. IRQunion67.881809/68.538825%
vs10kroles~56.4..56.9; COMP15479/14976calls/s vs~10312at10k5.8.
Thesearesoftwarewallpartitions,notCPUutilization/exclusiveISRcycles.

No demonstrated24k advantage inthisregime; cyclevariationgreater despite
slowerspeed,IRQtraffic/costhigher,COMpreemptiblebracket199notatomiccommit48.
No conclusionthat24kcanneverwork,nominaldutynotequalconduction,rawnotamps,
noindependentqzcparity. Do notforcehigherduty onnominalreferencefrequency
alone. Nextauditqualification/IRQtiming/sourceparity forremainingmechanism;
10kA54DCFEAremainsknowncomparison. No24krecoverytested. Alloutputsverified
off/ARR6399restored,portsclosed,3DEAC2F0 remainsinstalled.

---

## Entry 295 — 2026-09-13 — Inlined comparator adapter: first measured timing improvement at matched speed

Inspectedfrozenminz/core/src/am32_isr.rs vs live:bothSHA256
54b9f4957e69ad3606cc4db90397d2071a536f39ecceb320c54e4768a8c92ab2.
Filterreturnsonthefirstdisagreeingread; consecutiveagreementNOTaveraging.
Thusmemo's24kperiod-spanningargumentdoesnotguaranteeimprovement.
Release3DEAC2F0 stillcalledoutlinedCachedComp::output_level0800462c for
eachpersistence read. bench-inline-comp addsinline(always) tothatmethodand
read_comp_level,unchangedcount/polarity/volatileCSRreads/core/guards.
Newloop08004d88..08004e02containsfreshldr08004d94periteration and
agreementcheck/countbound; nooutput_levelsymbol. Fasterexecutionchanges
analog-timeaperture evenwithsamecount; mustmeasure,notclaimequivalence.

945D67886B499176CBD199ABD4D9595E25E0C097B5FADBA9460FF7A9B908A624
release/s/thinLTOtext119748/data1088/bss29264,250Pythonpass. COMPREAD
strictdecoder/--inline-comp provenance added. Flashedafterverifiedoff;
UARTworkedafterresetwithoutclockrepair. inline24_disabled_01 6flags127,
12..13us/restoreoff;cpu_01max2/7/10;archive_01 3x3/finaloffpass.

Predeclared inline24_hold58_01 one10s5.8%/24006Hz/phase60/coretrace0/
unchanged6.2startup/allguards. PASS253.873708eHz15233COM/15232accepted,
sigma43.497474us,raw228/bus10996mV,arm57.5us/cost14,COMP81/COM135/
commit48/guard25us,ADC49751/DMA18queue2,stack4128/finaloff/ARR6399.
SHA256846cbbd7e8352b15135daee129234730addca1718cdeae98866854fdeae54883.
IRQunion66.492650%,15916COMPcalls/s. Previoussamepoint254.266759eHz,
sigma58.848151us,COMP87/COM147,IRQunion67.881809%. Realfirstimprovement
atnearlymatchedspeed,butn1/notcohort/notindependentqzc/currentcalibration.
Nextconfirmhigher6.2pointandlongerrecovery. No claimnear300faultcured.
Alloutputsverifiedoff/portsclosed,945D6788 remainsinstalled.

---

## Entry 296 — 2026-09-13 — Higher-point inline comparison and 3/3 thirty-second recoveries

Same945D6788,24kBEMF/10kstartup,phase60/coretrace0/6.2%startupandpowered.
No guardchange/newoperatorPSUreading;800mAsetting retained. Everyattempt
declaredbeforeexecution inPWM_ROLE_EXPERIMENT.md, rawcapturesretained.

inline24_hold62_01 PASS10s278.803878eHz16729COM/16728acc,sigma43.982691us,
raw288/bus11008,arm41.5us/cost13,COMP81COM134commit48guard25,
IRQunion67.259549%,stack4064/finaloff. SHA256
f62ebbe4adc934b47488b922aeff4f2fa377645033c08273ff54aed5b1c27cc6.
AgainstE294same-speed278.837228eHz sigma58.944872,COMP87COM199 and
IRQunion68.538825%, theinliningimprovementappearsatbothtestedpoints.

Theninitial30srecoverypass followedbytwopredeclaredconfirmations, stopon
failurepolicy (nonefailed). Fullstartup-to-deadlinecohort3/3, actualinjected
Trackingloss/fresh12intervalacquisition/recovery3/3:

| capture suffix | recovered eHz | cycle sigma us | COM/accepted | arm/cost us | deadline spare us | raw peak/bus min mV |
| --- | --- | --- | --- | --- | --- | --- |
| reentry62_30s_01 | 279.001593 | 40.476702 | 46854/46854 | 42/14 | 208 | 265/11032 |
| reentry62_30s_02 | 279.323196 | 40.828743 | 46909/46908 | 41/14 | 189 | 284/10996 |
| reentry62_30s_03 | 279.048555 | 40.246768 | 46862/46862 | 41/14 | 131 | 279/11044 |

Allcapturesprefixinline24_, resumed~27.989s,COMP81COM135commit48,stack3992.
IRQunion67.37006/67.16424/67.29546%,notCPUutilization. Alloriginaldeadline,
CRC/archive/ownership/ADCperiod2666/timeline/actualarm/finaloff verified.
Hashes/build/settings in captures/inline24_reentry62_cohort.csv. No missing
attempts/retries. Notindependentqzc,calibratedcurrentorhigher-speedqualification.

Timingandportablewinfrontsupdated. Nextduty-ledexpansion needs separating
drivenacquisition/BEMFduty: current6.2cap in driven_run,core_benchadoption,
hostcaptureandverifier is acommandcoupling, notameasuredmotorlimit. Keep
qualifiedstartupandallguards; addexplicitBEMFcommand/actualprovenance before
raisingonlyclosedloopduty. No AM32switchrequired. Alloutputsverifiedoff,
portsclosed,945D6788 remainsinstalled.

---

## Entry 297 — 2026-09-13 — Architecture workload budget, before interrupt deferral

Operatorquestionabout66%IRQat6%duty redirectsnextworkfromdutyexpansion
toarchitecturecharacterization. Fulloriginalgoal retained; no newceiling.
CreatedARCHITECTURE_BUDGET.md anddrv_architecture_report.py, which invokes
fulltiming/recoveryverification beforejoiningCPUunion/eventcounts. Tests
checkqualifiedcapturecounts andrefusefailedrecovery/truncatedsafing.

inline24_reentry62_30s_01:15545.216COMPcalls/s,1674.007COM/s,
4975.111ADCscans/s,9.286COMPvisits/COM,388243/435097=89.23%visitsnot
accepting. ThisisNOTremovableworkfraction. Per-vectorcost/closed-gate/
persistence-rejection splitmissing.597.369usmeanCOMperiod gives38231.6
theoretical64MHzcycles,notavailableheadroom;max81COMP/135COM/48commit
bracketsnotadditive.67.370%IRQunionnotCPUutilization; foregroundnotidle.

Nextmeasureactualreferencegate-decisioncountswithoutper-readtrace before
timerdeferral. TIM2gate usesage sinceacceptedinput, strictCNT>avg>>1,
notCOM-relativetime. Pendingpost-crossing level deliberatelyretainedbefore
gate; blindclearatwakewouldloseit. Anydeferredwake needsowner/epoch/
sectorvalidation,cancelonstop/dropout/reentry,unchangedguards,resource/
priorityaudit andENABLElowpending-behavior checks beforepowereduse.
Contractwritten; no timerintegrationorperformanceclaimfromtheplan.
No firmware/flash/serial/motor action,945D6788 unchanged,lastoffE296.

## Entry 298 — 2026-09-13 — Optional comparator-path instrumentation, offline only

Added bench-comp-paths: captures the FIRST actual Interval::count per dispatched
reference comparator call, then counts no-gate / closed / open-no-accept /
accepted / stopped-or-unknown. No extra TIM2 read or per-level trace enabled.
The average snapshot is taken in COMP, above COM priority; guard-stopped calls
are unknown, not noise rejection. Counters reset at observation_reset (including
recovery), not merely observe_begin. Hardware scheduling and guards unchanged.

Host optional decoder rejects duplicate/malformed/active/saturated metadata;
architecture report requires accepted equality and dispatched<=COMP calls.
Synthetic metadata accounting tests are explicitly NOT hardware measurements.
180 Rust tests, M0 no_std check and 255 Python tests pass. Release build uses
opt-level s, thin LTO, codegen-units 1: text120256/data1120/bss29264.
Candidate SHA256 FDD9319B4099129C3DBA94F62EFAF1B30BD3C0FECD8D58FE4DD4841BFD3AE745.

NOT flashed. Installed945D6788 unchanged; no serial/probe/motor action, last
verified outputs-off E296. No measured branch distribution or overhead yet.
Next add fixture-required provenance, disabled checks, then a bounded same-point
comparison before changing interrupt scheduling. Full duty-led objective remains
open, including wider envelope, current calibration and archived parity limits.

## Entry 299 — 2026-09-13 — Actual comparator-path distribution at 279 eHz

Installed E298 FDD9319B after serial-verified outputs-off. Flash/reset succeeded,
UART responded without clock repair. comppaths_disabled_01 six role checks
flags127/12..13us/restoration passed; cpu_01 max2/6/10us; archive_01 3x3 pass.
Fixture --comp-paths now requires presence and full architecture accounting.

Predeclared one10s hold,6.2% acquisition/powered,phase60,trace0,24k; unchanged
electrical/tracking/deadline/arm gates. comppaths_hold62_01 PASS279.448026eHz,
16767COM/16767accepted,sigma51.146699us,raw247,bus10925mV,arm51us/cost14,
COMP85/COM268/commit48us,IRQunion70.125707%,stack4072,DMA18us/queue3.
Raw SHA256 0B9248A3EFBA2DF2AF8152EA55B0FC29B6B6D27F155E9FD2D81EF913CE597FCA.
Final gates/ENABLE/CCRs off, ARR6399 restored; serial closed.

145198 dispatched calls:21526closed,106905open-no-accept,16767accepted,
no-gate0,stopped/unknown0. 14.8% pre-gate vs73.6% post-gate nonacceptance
changes priority: pre-gate deferral alone cannot remove most visits. These are
counts, NOT exclusive CPU cost or independent noise/rotor classification.
Source reference open branch clears pending then checks consecutive levels.
Post-gate qualification/PWM coupling deserves next architectural investigation.
Counters perturb timing: prior noninstrumented same-duty hold IRQ67.260%,
COMP81/COM134us,sigma43.983us. n1 comparison cannot isolate intrinsic overhead.
No higher duty or recovery test on this image; no guard changes. Goal open.

## Entry 300 — 2026-09-13 — System-wide atomic backend overhead identified

Assembly shows PRIMASK save/disable/restore even on plain average/filter/rising
loads. portable-atomic1.15 cached docs explicitly confirm critical-section backend
does this for all operations; single-core backend avoids it for <=pointer-width
loads/stores, retains RMW exclusion. Both root and HAL forced old backend.

Added opt-in bench-single-core-atomics (--no-default-features); default retains
atomic-critical-section. Clean cached HAL15aca632 selected via root patch;
manifest-only removal of forced backend documented for ignored-checkout replay.
Shared minz-core unchanged. ATOMIC_BACKEND_EXPERIMENT.md records safety contract.
Release/s/thinLTO candidate9223AA9687EDBF96F4266AC1318B660383A792FA0DE41F25ED3A425271C80449,
text110520/data1128/bss29260:9736 fewer text bytes, not measured CPU savings.
New reference average/filter/rising loads directly access memory without masks.
No flash/serial/motor action, installedFDD9319B/lastoffE299 unchanged.
Next disabled semantic/PRIMASK diagnostic +provenance and DMA/privilege audit,
then preflight/requalification; no unmeasured duty or safety expansion.

## Entry 301 — 2026-09-13 — Single-core atomic backend disabled preflight

Added local-state atomiccheck256 repetitions, including nested explicit critical
sections, CAS success/failure and PRIMASK preservation; reads CONTROL privilege.
Added ATOMICBACKEND metadata and --single-core-atomics fixture requirement.
258Python tests pass. DMA destination source audit identifies sample rings,
not controller atomic state. No shared-core implementation/guard changes.

Installed E0FE22777F94C78C05F89D664EE7F54F69ADCD2DBA93C1830CFDF93885006C67,
release/s/thinLTO text111316/data1128/bss29260. atomic01/roles01 UARTsilent;
both raw failures retained. ReadRCC08000000/PD1ODR0/BDTRc1a/CCRs0, then
restored knownUARTclock08040000. atomic02 PASS256 privileged/nested-mask;
roles02 sixflags127/12us/restored; cpu01 max2/7/10; archive01 3x3PASS.
All finaloff verified, ports closed. NOmotor run/CPU speedup claim. Next recorder
refusal preflight then guarded same-point comparison; goal remains open.

## Entry 302 — 2026-09-13 — Single-core atomic backend powered comparison

Same E0FE2277 installed. singlecore_record_01 passes12refusals/unchanged records.
Predeclared10s6.2%/phase60/trace0/24k hold with backend/path provenance;
singlecore_hold62_01 PASS279.45655eHz,16768COM/16767accepted,
sigma39.73778us,IRQ62.47127%,COMP75/COM105/commit46us,arm65.5us/cost12,
raw280,bus11032mV,stack4076,DMA18queue2. RawSHA256
078CCC80EAE836F3CBB9B4C8F616AC9E95ACA912C7A1AFF46F1B0588C5DD90AC.
E299sameinstrumentedpoint:279.448eHz,IRQ70.12571%,sigma51.1467us.
Visits increased145198->178131; improvement is not just reduced IRQ traffic.

Then predeclaredONE30sdropout/reentry same settings/guards:
singlecore_reentry62_30s_01 PASS originaldeadline242usspare, freshseed1211
in7833us,arm53.5us. Resumed27.98931s280.03645eHz,47028COM/accepted,
sigma35.18649us,IRQ62.79455%,COMP75/COM105/commit46us,raw280,bus10972,
stack3552. Paths498017=87028closed+363961open-noaccept+47028accepted;
unknown/no-gate0, per-segment reset/full accounting passes.
Both complete verifiers and finaloff pass, ARR6399 restored, portsclosed.
No guard changes/higher duty. n1hold+n1recovery supports gain but not broad
reliability/current parity. Next reduce optional counter overhead and confirm
recovery before expanding duty; full objective remains open.

## Entry 303 — 2026-09-13 — Fixed single-core backend recovery cohort3/3

Predeclared exactlytwo additional30s samebuild/settings attempts, stoponfailure.
Both passed; combined with E302 one fixed3/3 cohort, no omitted retries.
singlecore_reentry62_30s_02 recovered279.919184eHz,47009COM/47008acc,
sigma35.152348us,IRQ62.442613%,seed1208/acq7833us,arm53us/cost11,
raw274,bus10913,deadline176usspare. Trial03 recovered280.084753eHz,
47036COM/acc,sigma35.092153us,IRQ62.515236%,seed1198/acq7774us,
arm52us/cost12,raw276,bus10757,deadline223usspare. BothCOMP75/COM105/
commit46us,DMA18queue2,stack3552. Full recovery and finaloff checks pass;
ARR6399 restored, portsclosed, sameE0FE2277 remains installed.
captures/singlecore_reentry62_cohort.csv retains rawSHA256 and settings.
TIMING_HEADROOM/PORTABLE_WINS updated; not higher-duty/current parity proof.
Read-only audit: driven_run uses acquisitionduty for BEMF transfer and
core_bench rejects>62. Separate those before duty expansion; no code change
to motor behavior this entry. Optional path-counter overhead still installed.

## Entry 304 — 2026-09-13 — Lean single-core build, optional counters removed

Built E301features withoutbench-comp-paths, release/s/thinLTO/codegen1,
text110740/data1104/bss29260. InstalledSHA256
1E13B559CEE7566B3DA36C593D52D075267E90B218B42B80557F0A8C719321E6.
UART responded without repair; singlelean_atomic01 passes256, roles01 six
flags127/12..13us, cpu01 max2/7/10, archive01 three3/3 checks.

Predeclaredone10s same6.2%/phase60/trace0/24k hold, fullunchangedguards.
singlelean_hold62_01 PASS280.562024eHz,16834COM/16833accepted,
sigma39.503170us,IRQunion58.471589%,COMP72/COM98/commit46us,
actualarm68us/cost12,raw272,bus10996,stack4124,DMA18queue2.
RawSHA256 ab23dfbccd4ec85d21376372094502a1a1f2dedb566adc22d28c5809a5e3d72e.
COMPPATH absent as required. Fullfixture/architecture validation and finaloff
pass, ARR6399restored, portsclosed. Same instrumented singlecorehold was
62.47127%IRQ, so removedcounter build is4.00points lower, not exclusivecost
proof because visitratechanged. Lean n1hold only; oldE0FE owns3/3recovery.
Next decouple bounded startup acquisition duty from BEMF duty; no duty or
speedguard expansion in this entry, broader current/parity goal remains open.

## Entry 305 — 2026-09-13 — Independent fixed BEMF duty, offline implementation

Added bemfdu one-shot idle-disabled selector;0inherits acquisition,40..100
tenths allowed by current carrier. Acquisition remains40..62; handoff and
recovery receive separate selected duty. No mid-segment PWM changes or guard
relaxation. Host --bemf-duty requires ack, cleanup resets0; DUTYSPLIT validated
against request/acquisition. Selection metadata is not independent waveform proof.
Pure inheritance/bounds and host strict parsing tests pass181Rust/261Python;
M0check/release-s-thinLTO build pass,text111536/data1104/bss29276,
candidate09E470D5509BCD9C7509BB78D1D91C499EC3BAB6382A2822C05EDADA90E7BF9B.
Not flashed, noUART/probe/motor action; installed1E13B559,lastoffE304 unchanged.
Next disabled setter/one-shot/abortcleanup/CCR checks and equal-duty regression
before expansion. Explicit setting required for live fixtures to avoid pending
manual settings. Full goal remains open; user IRQfloor question does not imply
hardware PWM itself needs an IRQ per carrier cycle.

## Entry 306 — 2026-09-13 — Duty abort cleanup and disabled diagnostics

Shell off clears pending BEMF selection; generic gates_off does not, since
normal startup invokes it before consuming selection. Added dutycheck6cases
through setter/take/inherit/rejection/clear, no output authority. Added roleduN
to reuse ENABLElow six-sector/CCR/counter check at selected40..100tenths.
Host --duty validates exact requested compare and all existing role flags.
261Python tests pass. Release/s/thinLTO candidate8D1AAABFCEF0D20CC9986E8C8721BBECC58C1013894E313CCC353C40CE06CCB5,
text112316/data1104/bss29276. NOTFLASHED, no serial/probe/motor actions.
Installed1E13B559,lastoffE304 unchanged. Next disabled checks then equal-duty
regression; duty expansion and broader goal remain open.

## Entry 307 — 2026-09-13 — Duty split disabled checks and equal-duty regression

Installed8D1AAABF after outputs-off verification. Atomic01UARTsilent retained;
RCC08000000,PD1ODR0,BDTRc1a,CCRs0 before knownUARTclock08040000 repair.
atomic02passes256; role62/70 sixflags127 each,12us,allCCRs165/186 respectively;
dutyselect6/6, CPU2/7/10,archive3x3 passed. ENABLElow during role tests.

Predeclared10s equal62/62/phase60/trace0/24k: dutysplit_equal62_01 PASS
280.438596eHz,16827COM/16826acc,sigma39.225768us,IRQ58.354967%,
COMP72/COM99/commit46us,arm57us/cost13,raw271,bus11092,stack4180,
DMA18queue2. DUTYSPLIT confirms requested62/62, fullfixture and finaloff pass.
RawSHA256 f2ac13e873687e8f3ff2402028404fe6b87db74960825ef7a070c1dd3a08ded3.
ARR6399 restored, portsclosed. No powered7% or recovery on this build yet.
Next bounded independent increment, retaining current310profile/guards and
all failed attempts. Full envelope/current/parity objective remains open.

## Entry 308 — 2026-09-13 — Independent duty reaches old timing wall

Same8D1AAABF,no flash. Predeclared62acquisition/64BEMF/phase60/trace0/24k
10s PASS292.325923eHz,17540COM/17539acc,sigma38.395928us,IRQ58.429909%,
COMP72/COM97/commit46us,arm68.5cost13,raw280,bus11116,stack4180.
Raw7b036e6aa45fa8b3e483d5835e61eeccf53facf76dbdc09ea15c74d1c08ddfe6.
Next predeclared65BEMF sameguards FAILED714760us CycleTiming12,
1274COM/1273acc,mean297.073974eHz,step5guard711501->714722=3221<3226.
raw227,bus11116,stack4180,DMA18queue2. CYCLECOREavg1109/prev1117/
ci1137/this1113/last896/wait284/filter12/poll0. Referencecycles6837->6436
halfus,pairmean3318.25us,NOTphysicalrotorperiod or causeproof.
Raw27314efe0381d3cbe8a3c7e4e959f42631c3974c1c0c7b04db9666562d8a038b.
Full64fixture and fault65validator confirm outputs-off; bothDUTYSPLIT matches,
ARR6399restored,portsclosed. No higher attempt/guardchange. CPUgain didnot
cure~297timingwall; next qualification/timing diagnosis. Full goal remains open.

## Entry 309 — 2026-09-13 — Trace confirms repeated first-read rejections

Predeclaredone10smaximum trace1 diagnostic, same8D1AAABF/acq62/BEMF65/
phase60/24k, allguardsunchanged. Failed393795us CycleTiming12,
695COM/694accepted,mean294.10545eHz (includesstartup),raw227,bus10996,
stack4172. Exactstep2guard390558->393757=3199<3226. Fullfault/traceCRC/
tailaccounting/finaloff validated; no higherduty. CleanuptraceOFF,portsclosed.
Raw03f09c8627f3c540d7d56c2b6bbe773325e15ab6e11915ba784b0b72a3a63fdb.

24retainedhandlers:18persistence rejects,16onfirstread;3blankgate,2accepted,
1guardrefused after12correctreads. Step2PWMcnt610/608/610 at393549/393590/
393674us repeats41/84us apart, consistentwith24kcarrier41.656us. This is
PWM-related traffic evidence, not analogrootcause or whole-runfraction.
Finalseq6533 count1033 avg1115 reads12 level0expected0: referencequalification
passed then cycleguard refused. DifferentOBS/guard origins prohibit latency
subtraction. Referencecycles6909->6389halfus not physicalrotorperiod.
Next audit hardwarefilter/routing or timed sensing to reduce repeatedentry
while preservingqualifiedcrossings; no basis for calling12readloop the cost
of everyreject or looseningcycleguard. Fullgoal remains open.

## Entry 310 — 2026-09-13 — Internal hardware-filter route identified

ST RM0444Rev6section22.4.28p688 confirms TIM2TI2SEL1 routesCOMP2 internally.
LocalPAC TIM2IC2F defines consecutivesample filters. This is a concrete
candidate to reject shortevents beforeCPUentry, not measured improvement.
TIM2 alreadyowns reference2MHz interval; no PSC/ARR/CNT/SMCR changes orUG
permittedinliveexperiment. Filterclock mustbe verified independentlyofPSC.
HARDWARE_SENSE_FILTER.md records source and disabled polarity/pulse/capture/
overcapture/countercontinuity/restore test contract beforeparallelobserveuse.
FullPDFwebopen exceededlimit; curl0bytes44s cancelled. IndexedSTtext retrieved;
no claimedlocalmanualcache. No code/flash/serial/motoraction. Installed8D1AAABF,
lastoffE309,traceOFF. Next disabledroute experiment, allguard/goal scopeintact.

## Entry 311 — 2026-09-13 — Pure capture-filter register/timing model

PAC TIM2CKD encodings0/1/2 selectkernelclockDiv1/2/4. Addedcapture_filter.rs
for all16filtercodes, independentofPSC, TI2SEL1/CC2S01. Maxfilter15/CKD2
idealconsecutive8sampling bracket896..1024kernelcycles=14..16us at64MHz;
excludes synchronization/IRQdelay and isNOTbench rejectionmeasurement.
184Rusttests/M0no_stdcheck pass, no firmwareintegration/flash/serial/motor.
Installed/root8D1AAABF unchanged,lastoffE309. Next disabled1us/40us pulse
diagnostic filter0/15, measuredwidth/outputlevel/capture/overcapture/restore.
Fullgoal remains open; this model doesnot qualify livefiltered commutation.

## Entry 312 — 2026-09-13 — Disabled filter pulse diagnostic implemented

Addedoptionalbench-capture-filter/filtercheck with guardagainstactiveoutputowners,
ENABLElow throughout. TIM2CH2 risingcapture,code0/15,requested2/40us pulses
viaCOMPpolarity/internalreference, softwarebracketwidth/actuallevel/CC2IF/CC2OF
records. DIERnonzero refuses; no IRQ/DMAenable. Restores savedtimerconfig/CSR,
clearsflags and leavesTIM2/COMPmasked atidle. Morecomplete restore/counter
checks and stricthostverifier needed beforehardwarequalification.
Release/s/thinLTO build passes. NOTFLASHED, noserial/probe/motorrun.
Installed8D1AAABF,lastoffE309 unchanged. Goal andlivecontrol remain unchanged.

## Entry 313 — 2026-09-13 — Hardware comparator filtering pulse test passes

Prior explanatory turn was no campaign progress; resumed with authoritative
source/ELF checks, 263 Python tests, verified off, download/reset succeeded.
Installed EF1E586997CA28C49C8E802EEAA7563E4FB03739EEF7F4A864242BA6FD8AB9A9,
release/s/thinLTO text114164/data1104/bss29276. Added strict host verifier and
extended restore readback CR1/CCER/SMCR/COMP CSR excluding output bit.
Predeclared pulse gates in HARDWARE_SENSE_FILTER.md before flashing.

captures/filtercheck_01.txt SHA256
F8381CB30F817B25FA0476835B5872AACE895E0F6AFC62EAFF48D83724BC73F5:
filter0 captures requested2/40us; filter15 rejects2us, captures40us.
Software timing brackets2/40us; pre/high comparator levels0/1, no overcapture,
ENABLElow throughout, configuration restoration pass, final outputs off.
No UART repair. Follow-on disabled filter_atomic_01 (256 checks),
filter_roles_01 (6 sectors,12..13us), filter_cpu_01 (max2/7/10us),
filter_archive_01 (3 checks x3 trials) pass with finaloff, ports closed.

Proves internal comparator capture routing and two-width pulse discrimination,
not a measured rejection threshold, analog BEMF reliability, CPU savings or
live interval-counter preservation. No motor run. Next counter continuity and
capture timing evidence, followed by observe-only integration without changing
commutation authority. No electrical/timing guards changed; full goal open.

## Entry 314 — 2026-09-13 — Filter timestamp and local counter continuity

Prior turn made hardware progress (E313). Added FILTERTIME pulse-local CNT
before/after, elapsed TIM17 and settled post-level; strict optional historical
decoder, mandatory timing in new fixture. Tests cover count mismatch, post
level, capture delay and missing timing. 264 Python tests pass.
Predeclared gates: CNTdelta vs2*elapsed within4 half-us ticks; capture delay
0..6 ticks unfiltered,28..38 filtered. No motor authority or guard change.
Verified off, downloaded/reset release/s/thinLTO candidate
9AA099C1661E7FCB3EAED14C0DAA92981FC46E6E516E77DF14237A2F1892F020.
captures/filtertime_01.txt PASS: nofilter captures2/40us, filter15 rejects2us
and captures40us request (41us software bracket). Filtered CCR delta30ticks
=15us from pre-write sample; unfiltered0ticks is quantization, not zero delay.
All pulse-local continuity checks pass, post levels low, no overcapture,
restored configuration and finaloff verified. No UART repair, port closed.
This is disabled timing evidence, not analog BEMF/CPU savings or full live
counter-ownership validation. Next observe-only integration with reference
counter reset, mux/polarity settling and stop/recovery epoch handling intact.

## Entry 315 — 2026-09-13 — Observe-only lifecycle scaffold

E314 was hardware progress. Read actual reset/mux paths: Interval::set_count
resets reference TIM2 before EV_ACC; comp_input owns raw edge polarity/mux.
Added optional filter_observe.rs, compiled but not wired or flashed. Provides
stopped/ENABLElow preparation, capture disable around mux, latestCCR snapshot
before counter reset, stop revocation and quiet aggregate dump. No IRQ/DMA,
reference pending, comparator or gate authority. Capture lag uses same counter
epoch; overcapture retained. Immediate rearm includes mux artifacts explicitly.
Release build passed; remaining lifecycle tests/hooks and preservation of CKD
across existing CR1=1 restart must precede hardware use. Installed9AA099C1
unchanged, lastoffE314, no serial/probe/motor action. Full goal remains open.

## Entry 316 — 2026-09-13 — Observer cancellation and counter-epoch tests

Previous turn added scaffold (progress). Added pure filter_epoch model, used
inside observer critical sections. One-use mux tickets reject callbacks after
stop, restart, superseding mux or counter reset. Reset samples only an armed
observer; initial seed reset cannot fabricate a missing/captured event.
Preparation now cancels old hardware capture before checking refusal gates.
Three lifecycle tests pass including wrap/reuse, full replay tests and M0
library check pass (incremental-cache access warnings nonfatal), release build
passes. No live hook, flash or hardware action; installed9AA099C1,lastoffE314.
Next actual owner/release integration, CKD-preserving start, host counters
validation and bounded observe-only BEMF run. No goal or guard scope change.

## Entry 317 — 2026-09-13 — First handoff observer connected behind build flag

Added bench-filter-observe separate from disabled bench-capture-filter.
Hooks: preparation after driven transfer adoption but before output start;
raw comparator mux tickets; comp_input stop revocation; snapshot before
Interval reset; fixture-only summary. Preparation permits awake ENABLE but
requires all gate owners inactive, outputs off, TIM2 stopped and DIER0.
Nonzero seed write invalidates capture without counting it. CEN restart now
preserves CKD. Added strict host counts/extrema/authority decoder with tests.
267 Python tests and release build pass. No flash or serial/motor action;
installed9AA099C1,lastoffE314. Next disabled regressions then no-recovery62/62
bounded hold, full campaign safety/arm verification plus capture observations.
No proven live filtering benefit, no guard changes, full goal remains open.

## Entry 318 — 2026-09-13 — First observer hold; failed preflight disclosed

Installed5D7C23555C3249924DE9D03E1957A73546AEF5E6B8FAAAF2F49B3F53951F014B.
filterobs_pulse/atomic/roles/archive_01 pass. filterobs_cpu_01 FAIL mode2
max11us>10; mode0/1max2/7. Sequencing error: batch command exit reflected
archive success and powered hold launched before acting on CPU failure.
Disclosed to operator, no threshold changed. New fail-fast disabled preflight
script stops on first child failure. No further power until diagnosed.

Exploratory filterobs_hold62_01 (not qualified build) full campaign verifier
PASS10s at280.0267eHz, cycle sigma38.3756us,16802COM/16801accepted;
raw287bus11056,arm48.5us/cost14,stack4004. IRQ59.318%,COMP75COM265commit46.
Observer16801samples/15063captures/2overcapture,lag1..263ticks; schema pass,
count matches accepted. Raw SHA256
D24AAE4E404EBA98F58B6B5EE06DE3702CBC0D5417F1A0D0CD4D583D68CFB945.
Finaloff verified, portclosed. No recovery test. Captures are latest-edge
availability, notqzc; noCPUgain. Next disabled overhead and COMmax regression
diagnosis before replacing any sensing authority or increasing duty.

## Entry 319 — 2026-09-13 — Disabled overhead repeatability evidence

Read objective and cpu_meter/irq_union source. cpucheck brackets synthetic
nested Scope enter/drop, not comparator observer callbacks or NVIC exception
entry. Installed5D7C2355 unchanged. Four retained disabled repeats
filterobs_cpu02..05 pass: mean/max mode0=1.625/2,mode1=6.5/7,
mode2=10.000/10us. Original01 mean10.125/max11 failure remains authoritative.
No cause proven by passing repeats; no gate relaxation/requalification.
Added preflight tests: CPU subprocess failure stops before archive; success
contains only five disabled diagnostics, never a powered command.
All repeats finaloff verified, no motor run, port closed. Next controlled
disabled baseline or finer-resolution bracket to separate instrumentation/code
layout/interrupt effects; COMmax265 remains unexplained. Full goal open.

## Entry 320 — 2026-09-13 — Matched observer-off disabled baseline

Read objective. Archived exact5D7C2355 observer ELF, rebuilt same source with
bench-capture-filter but withoutbench-filter-observe; release/s/thinLTO and
all other features unchanged. Installed baseline
A1E963685400C670FCBCEF4BEEF2E9D0A2AB9257BAAE091056E20E406BED38A9.
filterbase_cpu01/02 failedUARTsilence, retained. ExactreadbacksRCC08000000,
PD1ODR0,BDTRc1a,CCR0/0/0 proved safe before knownUARTclockwrite08040000.
filterbase_cpu03/04 pass nestedmean10/max10. mode1mean6.5/6.625,max7;
mode0mean1.625,max2. Allfinaloff, portsclosed; no motor run.
Scope drop symbols identicaladdresses08003200/080032ac; compare disassembly
for data relocation, no added observer invocation in accounting path. Evidence
doesnot establish observer as cause of prior11usoutlier, nor qualify image.
Keep originalfailure and COMmax265 investigation open. Next finer-resolution
timing or accounting-cost reduction without changing measurement gate.

## Entry 321 — 2026-09-13 — Lazy accounting clock; preflight and hold pass

irq_union enter_clock/leave_clock defer TIM17 read until after state validation,
only on valid outer boundaries. cpu_meter uses these under same critical
sections. Nesting/overflow/elapsed/fault guards unchanged. New callback tests
cover inactive/nested/invalid boundaries; full replay tests and release pass.
Installed81572D65E294CEA12B11CDCA47564357820F212424C9426650EB99F1831BD2CB.
Preflight01 correctlystops on UARTsilence; exactRCC08000000/PD1zero/BDTRc1a/
CCRsallzero before clockrestore08040000. Preflight02 fivechecks allPASS,
CPU nestedmean9.46875/max10 (prior10.0/max10), no measurement gate change.

Predeclared10s62/62phase60trace0hold filterclock_hold62_01 PASSfullverifier:
280.1367eHz16809COM/16808acc, sigma39.0466us,raw283bus10865,arm62cost14,
stack4004,COMP74COM86commit45us,IRQ59.551%. Observer16808samples/
14927captures/5overcapture,lag0..684ticks, schema andfinaloff pass.
SHA25677DA8F464C70356FD10A8D0BBCC7A88A45D4D13E720CFE23A73DE29588315A36.
No overallCPUgain (priorobserver59.318%), no explanation of prior265us COM
outlier from this n1. Counterclock boundary shifts slightly; notexclusivecost.
About88.8%captureavailability isnotqzc; missing captures need timing/filter
study beforeauthoritychange. Alloff/portclosed. Recoverynotattempted, goalopen.

## Entry 322 — 2026-09-13 — Shorter filter sharply improves capture availability

Added bench-filter-short observer code12CKD2 (ideal7..8us), no change to
controller acceptance or safety guards. Disabled diagnostic extended0/15/12;
host validates code12shortwidth<6us,latency14..22half-us ticks;271Python pass.
Installed125B3C24C5FF6B25941F55F5E195F2920D2E5C4D46EEFCE6A261D0DBBEBC7D28.
Preflight01 fails UARTsilence (retained); exactRCC08000000/PD1zero/BDTRc1a/
CCRs0 before knownclockrestore08040000. Preflight02 allfivepass. Code12
rejects2us, longcapture delay16ticks=8us; restore/continuity/finaloff pass.

Predeclared10s62/62phase60trace0 hold filtershort_hold62_01 fullverifierPASS:
280.3238eHz16820COM/16819acc,raw241bus10853,arm62.5cost14,stack3892,
IRQ59.059%,COMP74COM86commit45. FILTERCONFIG12 andFILTEROBSstrictpass:
16819samples/16499captures/0overcapture,lag1..275ticks;98.10%availability
vsE321code15 88.81%.320missingremain. RawSHA256
3E908BFF16FCF1373B43B7BF74216DB2528D7D38AF9AAF2E0D1FA3DA2453C557.
Alloff/portclosed. Filter duration affects availability, not proof of qzc,
correctfirstedge, CPUgain or authorityreadiness. Next residualmiss/timing
diagnosis, preservecontrollerguards. Recovery notattempted; fullgoalopen.

## Entry 323 — 2026-09-13 — Misses span sectors; not late capture arming

Added all-run sectorcounts plus boundedfirst8missing snapshots, strict host
total/range/prefix checks;272Python tests/release pass. Installed
0F4629972F4B489B59A448E9898B69AD7CA68218258FCB0AC6C6B72A2DFFBC72.
Preflight01UARTsilent retained, exactRCC08000000/PD1zero/BDTRc1a/CCRs0
thenknownclockrestore08040000;02allfivechecksPASS. Predeclared10s62/62
phase60trace0hold filtermiss_hold62_01 fullcampaign/detailverifierPASS:
280.3043eHz16819COM/16818acc,raw286bus10793,arm46cost13,stack3628,
IRQ59.716%,COMP75COM86commit45. Captures16539/16818,0overcapture;
sector misses71/23/57/11/82/35 (2803samples each). Allsix affected.
First8raw-after==expected, armingage323..415.5us excludes latearming for
those samples, not qualification latency/chatter. Rawlevel after acceptance
doesnot prove uninterrupted persistence. SHA256
DBE259EDCB190145B441FA4ED51018E60DFEA4F66EC37B444C9EDD38A8ED91A2.
Alloff/portclosed. Next pairedraw/filteredcapture route audit and timestamp
evidence; no authorityswap or safetyguardchange. Fullgoal remainsopen.

## Entry 324 — 2026-09-13 — Indirect capture is not an independent raw path

Read goal and primaryPAC/STRM0444 routing/filter descriptions. Extended idle
pulsecheck with CH1CC1S2/IC1F0 paired with existingdirectCH2, savedCCR1 too.
273Python tests/releasepass. Installed
B58B250AE8FAE45CC3ED0BD7F39AB4AAD8EE4F20B4A7CCFFD674890DB88E7A61.
filterpair01UARTsilence retained; exactsafeRCC/PD1/BDTR/CCR readbacks then
knownclockrestore. filterpair02 allprotocol/restore/finaloff pass, but rejects
the independence hypothesis: both channels rejectshortpulses atfilter15/12,
bothcapturelong withidentical CCRs. IC1F0 isnot a TI2filterbypass.
RawSHA25622EEB46ECC72D8D817DCDFF22DA20C3702F6896C3D8B1D4EB3D1EF93BAEFA7F2.
No motor run, portclosed. Alternative TIM3TI2SEL1=COMP2 confirmed fromST
RM0444section22.4.29; audit existingADCcounter/DMA ownership and modulo201us
age limitation, disabledindependencecheck before integration. Goal remainsopen.

## Entry 325 — 2026-09-13 — TIM3 provides independent raw comparator capture

Read objective; audited adc_stream PSC63/ARR200/CR2updateTRGO and phaseDMA
UDE/ch3. CH2unused, initializationclearsCCER; liveattachmustrespectthat.
Extended disabledfiltercheck to TIM3CH2TI2SEL1/filter0 at1MHz201usperiod,
CR2zero/DIERzero, no ADCtrigger/DMA. Saves/restores10regs includingCNT/CCR2.
Host requires rawcaptureall6, noovercapture, modulo201rawdelay0..3us.
274Python tests/releasepass. Installed
6B8696F29E7B2082A4E2BFA7C836A042CF8D780F398ACF3439E5EFCA75150B0B.
filtert3_01 UARTsilent retained/exactsafeclockrepair;02PASS: rawall6 with1us
delay, filteredTIM2short12/15reject, longdelays7.5/14.5us. Restore/finaloff
pass, portclosed; no motor run. RawSHA256
F31B5143F3313EB8CC8FDFE67777D46A6497774834E540C9D2D65E1129C577A8.
Independentroute established, not liveADCcoexistence. Next preserve timer
trigger/DMA settings and test coexistence before missingcapture-age run.
Rawage willbemodulo201us, not unambiguous wholecommutationage. Goalopen.

## Entry 326 — 2026-09-13 — Raw capture coexists with TIM3 update DMA

Read goal/currentstate. Added bench-filter-raw helper onlychangesCH2capture
fields/status; disabled pulse method refuses ENABLEhigh/outputowner. Extended
existingadcphasecheck:32updateDMAwords plus32rawpulses pertrial, preserved
PSC63ARR200CR1CENCR2zeroDIERUDE, originalDMAmaxcounter<=2us check unchanged.
Installed51F1B6E54E5189FD76D85C6AED0568A750D8E809ACB919A8D61D8F54B56D8CC0.
rawcoexist_01PASS3x32rawcaptures andDMAupdates, maxcounter0/0/0us,
configpreserved1 each. No UARTrepair, finaloffverified, portclosed.275Python
tests pass including strictprotocol/mutation checks, releasebuild pass.
No motor run, noADCload in diagnostic. Next actualADCcoexistence andrawage
integration with sessioncancellation/after-init ordering; controlauthority
unchanged and no fullgoalcompletion claim.

## Entry 327 — 2026-09-13 — Raw capture coexists with actual ADC scans

Read objective. Added rawadccheck using real5channelADC/DMA plusphaseDMA
coherentpoll path at201us, ENABLElow throughout. Private idleprobe accepts
rawdisabledpath only withbench-filter-raw; no poweredowner bypass. Three
128scan/128pulse trials, config/timestamp/VREF/fault/finaloff gates predefined.
InstalledF3CEBD1BA7F6B62159B7F7EEEA8B27CD259435B25CC386576E4826CC5C88767A.
rawadc_01 PASS3trials25662/25663/25662us, VREFmin1503 each,128captures
and128scans, noDMAfault, unchangedPSC/ARR/CR1/CR2/DIER, finaloffverified.
No UARTrepair, no motor run, portclosed.276Python tests/release pass.
Next afterADCinit rawcapturehook with observerepoch/mux/reset/stop and
modulo201us age label, then bounded motorcomparison. Disabledcoexistence
doesnot qualify motorISRload or any new commutationauthority. Goalopen.

## Entry 328 — 2026-09-13 — Live raw captures expose noisy acceptance boundary

Rawcapture now attachesafterADCconfig onlyin activeobserverepoch, validmux
ticket arms, reset snapshots and stoprevokes; rawprefix8 agesexplicitmod201.
277Python/releasepass. Installed
58E0080BEEF6758398AF33961A82E915E933A1F79BD7AD50F787CC732AF23417.
rawlive_preflight01allsevenPASS,noUARTrepair. Predeclared10s62/62phase60
trace0hold rawlive_hold62_01 fullcampaign/detail/rawverifiersPASS:
279.641eHz16779COM/16778acc,raw268bus10937,arm59.5cost13,stack3184,
IRQ59.698%,COMP76COM266commit45. Filter16520/16778,258misses,0overcapture.
8/8rawprefixready/captured/overcapture1; agemod201=29/6/4/1/0/1/1/2us.
Prefix3raw_after1 with expected0: signalreversedbydiagnosticread.
Softwareacceptance isn'tcleanedgegroundtruth; recenttransients supported but
moduloage cannotexcludeolderedges. Do notchase100%coverage byshrinkingfilter.
RawSHA256758B0408FC79845D2B527E1875F5B4CC84F1D0BCD794FE884580AB06056FDB02.
Alloff/portclosed. Next timing-awarefilteredIRQ design preservingreference
gate/persistence/one-shotCOM/ageguards, actuallock rather thancoverage.
COM266outlier unresolved, nofilteredauthoritychange. Fullgoalopen.

## Entry 329 — 2026-09-13 — Filtered source contract tested against real core

Read objective and actualminz comp_isr/interrupt_routine. Added pure capture
source lifecycle and three integration tests using existingMockHal with actual
minz-core ISR (not a reimplementation of its gate/persistence). Strictgate
equality retains postcrossing pending; prelevel clears; persistenceflip does
not arm; valid acceptance uses currentCNT and arms once; stalephase/masked/
noedge have noauthority. Tests pass. FILTERED_IRQ_DESIGN.md records adapter
obligations: selectedsource dispatch, no earlyCCRread/flagclear, correctNVIC
mask/priority, observerflagownership, unchangedreference/independentguards.
No hardware adapter implemented/flashed, no UART/motor action. Installed
58E0080B,lastoffE328 remains. Next disabledhardware source-contract validation
after adapter implementation, then bounded knownpoint trial. Fullgoalopen.

## Entry 330 — 2026-09-13 — Staged filtered interrupt hardware backend

Previous explanatory turn did not change campaign evidence. Continued from
E329 by implementing optional filtered_irq_hw PAC backend, not another
controller: disabled-owner prepare, TIM2 CH2/code12/CKD2, EXTI mask, TIM2
priority0x40, phase tickets, explicit pending/mask/clear/stop, four refusal
counters. No CCR reads, timer start/reset/UG, or gate writes. Backend currently
has no callers or TIM2 vector, and is not connected to controller authority.
Source+observer feature combination fails compilation intentionally so observer
reads/clears cannot steal controller pending flags. Full replay163 unit plus
31 integration tests pass, M0 library check and release/s/thinLTO-configured
firmware check pass. Three new tests cover SR clear mask, edge bits and ticket
revocation. Existing unsafe/dead-code/cache warnings remain; no hardware proof.
No flash/UART/motor action. Root release ELF still SHA256
58E0080BEEF6758398AF33961A82E915E933A1F79BD7AD50F787CC732AF23417,
last verified hardware off remains E328. Next outputs-disabled real TIM2 source
diagnostic, then selected-source controller/setup/recovery/safety integration.
No CPU savings or new operating-envelope qualification claimed. Full goal open.

## Entry 331 — 2026-09-13 — Real filtered source interrupt contract passes disabled

Added outputs-disabled filtersourcecheck and dedicated TIM2 diagnostic vector.
Actual backend exercises maskedcapture, pendingretention/re-delivery, explicit
CC2 clear with unrelatedUIF preservation, phase/stop ticket refusal and directly
invoked late-handler stop. No control dispatch or gateauthority; panic stops
source after clearing physical outputs. Nine timer fields/COMP CSR restoration
checked. Host fixture requires three exact10-check/twoIRQ trials and finaloff;
281Python tests/releasebuild pass. Installed SHA256
54E95D70F514C13AD7BBD55102D8F99A2AF72981D5FA29E221443A6B665DA739.
Preflash UART confirmed off. filtersource_01 refused at initial UART readback,
silent capture retained. Probe RCC08000000, PD1ODR0, BDTRc1a, CCR0/0/0;
only then repaired known UART RCC to08040000. filtersource_02 all3 trials
bits1023 visits2 restored1 disabled1, finaloff verified and serial closed.
Capture SHA256499317423BD1AA9C4158FAD96E820278E7631AC9ECF2D5157FE3F3797A2CDDF5.
No motor experiment this entry, no occupancy improvement claimed. Next
selected-source controller integration preserving driven EXTI, setup/recovery,
stop/panic cancellation and independent guards. Full goal remains open.

## Entry 332 — 2026-09-13 — Filtered source integrated; first live trials fail tracking

bench-filter-control connects TIM2 source to Input traits/core hardware dispatch
and actualminz controller; startup remainsEXTI. Keeps raw persistence/currentCNT
acceptance, original one-shotCOM and guards.10us masked muxsettle before capture
arm is explicit experimental latency. Revocable currentticket, stop/panic source
shutdown, no silent EXTI fallback if startup clears selection. Added fixture-only
FILTERCONTROL prepared/active/refusal deltas. Release build/replay194testsPASS.
Installed955AB08BB5EA9B92E900DB51A6B11D6AD47D337E7F63C5CBE57BABA08AF9165D.
Disabled filtercontrol_source01 three trialsPASS and preflight01 allfivePASS,
noUARTrepair. Predeclared10s62/62phase60trace0 hold62_01 FAILED tracking8 at
1339us observed:2COM/1acc/7IRQ,COMP44COM230commit25,raw162bus11545,arm78ticks
cost12us. One boundedtrace1 followup FAILED tracking8 at1040us:1COM/0acc/0IRQ,
COM80commit25,raw236bus11605,arm75ticks cost12us. Both prepared1/refusals0,
stack3644, finaloff verified, portclosed. No CPU/lock qualification.
Capture hashes:
filtercontrol_hold62_01 A042D4DFC4E609CCA32315AFCD45D7A1E1A41ECEEBEF4EBDDFF5FE4D14FF7F21
filtercontrol_trace62_01 83EF10F5D470906B4CC8ADAC2598D379E27B4AE54F6A356EB30E5D3E15CB7D58.
ZeroIRQ trace means no controller persistence rejection in that attempt. Need
stop-before-clear TIM2capture/delivery-state snapshot; currentCOASTSTATE contains
legacyEXTI state. Investigate captureavailability/10us muxarming timing, no
blind filter-shortening or trackingguard relaxation. Fullgoal remainsopen.

## Entry 333 — 2026-09-13 — No capture despite enabled timer/source/NVIC

Added firststop twelve-field TIM2/backend/NVIC snapshot before source masks or
clears; noCCRread. Decoder validates finaloff/format/ranges. Snapshotbuild
C525178A disabled filterstop_source01 failed first restoration; no poweredrun.
DetailbuildC56F1F88 filterrestore_source01 reproduced same: alltimerfields equal,
COMP CSR40000281->281 only liveoutput VALUEbit30 changed. Corrected configuration
comparison excludesONLYstatusbit30, retains rawvalues and all configurationbits;
unitmutations rejectevery otherbit. Both failedcaptures retained, not discarded.
FinalinstalledAB53C5E3F5004E38A5233EA5C899774CCD99A20BF893B78589AD34C654B4C003,
releasebuild/285Python/195Rust testsPASS. fixedsource01 all3PASS, preflight01
all5PASS, noUARTrepairs. One10s62/62phase60trace1 command failedtracking8 after
1039us:1COM/0acc/0IRQ,COM80commit24,raw146bus11605,stack3808,finaloff/portclosed.
filterstop_trace62_01 SHA25676DE5C2A9F0720024B2681DCDE6536FA9FAD75F1E0C83FCD98D75B8D692ADEA2.
SnapshotSR27(noCC2pending/over),DIER4,CCER16(rising),CR1513(CEN1/CKD2),
CCMR149408(code12/direct),TISEL256(COMP2),CNT2278,rawhigh,armed/enabled/NVICenabled1,
NVICpending0. Actualabsenceofcapture with delivery enabled; notrawpersistence
rejection in this attempt. Next compare immediatearming against10us disabled
muxsettle, hardwarefilter unchanged. Highatstop doesnotprove earlierlevelhistory.
No duty/speed/safetyguardchange, no CPUgain or lockqualification. Fullgoalopen.

## Entry 334 — 2026-09-13 — Earlier capture arming alone fails; duration mismatch identified

bench-filter-early-arm removes added10us capture-disabled muxwait only, keeps
hardwarefilter12/CKD2 and core/guards. Release build installed
4AD6E3D7FE5A540F3EA8664727478B3098E17086DD5D1FF43B77751C6836B5E3.
filterearly_source01 threePASS/preflight01 fivePASS/noUARTrepair. One10s-command
62/62phase60trace1 attempt failedtracking8 at1040us,1COM/0acc/0IRQ,COM70commit44,
raw184bus11617,arm101ticks/cost13us,stack3832. Finaloff verified/portclosed.
SnapshotSR27,DIER4,CCER48(falling),CR1513,CCMR149408,TISEL256,CNT2276;
armed/softwareenabled/NVICenabled1, pending0,rawlow. Earlierarming notsufficient.
Capturefilterearly_trace62_01 SHA256
A5B89830FA360E889120703A741C19729C212029624543CF5621E5EAE5A122B5.
Sourcebudget check: CCR165ticks at24k/6.2%=2.578us nominalPWMwindow,139ticks
aftertimerDT26=2.172us idealpinhigh. Filter12 needs448..512ticks(7..8us).
AnyON-confined usefulpulse wouldbeerased; actualcomparatorpulsewidth notproven.
Next justified candidatecode5/CKD2=56..64ticks(.875..1us), disabledpulse/IRQ
qualification before samepointpoweredtrial. Core rawpersistence andguardsstay.
Purebudgettestadded/full196RustPASS. Candidate5 notimplemented. NoCPUgain or
filteredlockqualification; fullgoalopen.

## Entry 335 — 2026-09-13 — One-microsecond filter restores events, tracking still fails

Added bench-filter-one-us code5/CKD2, earlyarm0. Expanded disabledpulsecheck to
eight rows inclcode5: requested2us/measured3 captured, latency3halfusticks1.5us;
same pulse rejectedby12. No subusrejection measurement. Source01 threePASS,
preflight01 fivePASS,286Python/releasePASS,noUARTrepair. Installed
BEC723805FF65BB922F7F3B6B9A39C36325EF255174AECB6785E40DA1A4FB23C.
Both62/62phase60 bounded10s commands FAILtracking8:
filter1us_hold62_01 trace0 observed10140us,15COM/14acc/110IRQ,COMP64COM258
commit44,raw196bus11569,stack3344. CaptureSHA
8C6AB3532383C5E5ECB0449453F842F0904FF65068B2B803CFFD008F1AA2ACD2.
filter1us_trace62_01 trace1 observed2039us,3COM/2acc/15IRQ,COMP74COM260commit45,
raw143bus11581,stack3312. CaptureSHA
0B9D76A9F7DE4599BDC6CF1D3EF4994896CEAA50B6448116672E600355D46A45.
CRCtrace15rows:7closedgate,2accepted,6openreject; fivefirstreadwrong, lastseq15
at1577us expected0 ->1 byread3. Stoppedsourceenabled/noCC2pending/rawhigh inboth.
Sourceprepared1/refusals0; finaloffverified/portsclosed. Shorterfilter restores
eventdelivery butnot sustainedlock or CPUgain. Next sameadaptercode0 control
isolates filteringdelay vs adapterlatency before further durationtuning. No
motor/current/speedguard changes; fullgoalopen.

## Entry 336 — 2026-09-13 — Unfiltered TIM2 control completes sustained run

Added bench-filter-bypass code0 on sameTIM2backend/earlyarm. Incompatible with
one-us feature bycompileerror. Releasebuild installed
C8E4236D1CC929EFC16730C99FE3017FAF28798D2F3F5ED6512C2ECF2AB434F7.
Preflashoff verified, source01 threePASS/preflight01 fivePASS/noUARTrepair.
filterbypass_hold62_01 full10s62/62phase60trace0 PASS allcampaign gates:
279.549eHz16774COM/16773acc,cyclemean3577.189us sigma42.202us,raw253bus10865,
arm48.5us/cost13,stack3264. IRQunion63.732%,17084visits/s,10.185visits/COM,
COMP73COM258commit45. Prepared1/code0/settle0/refusals0,finaloff/portclosed.
CaptureSHA256C48DD685AC19AA36CA0575D1070D5D25E36B8EC14B6EF3D1656E15D69DF49C2B.
Adaptercan sustain, unlikeprecedingfiltered trials. NotCPUgain vsEXTI~59-60%,
notrecoveryqualification or universal filterlatencylimit. Delayedcapture event
plusrawlevel persistence has a timingcontract: filtering changeswhenthechecks
sample, notjustIRQcounts. Next investigate timing-aware sensingarchitecture,
not longerfiltertuning or invented siliconfloor. No guardsrelaxed; fullgoalopen.

## Entry 337 — 2026-09-13 — Capture-mirror read does not consume controller pending

Before instrumenting edge-to-read latency, tested independent CH1 flagack while
CH1indirectTI2 mirrors samefilteredsource as CH2. Expanded disabledpulse fixture:
readCCR1 then observeSR, readCCR2 then observeSR. All8cases PASS flags6->4->0
when captured,0->0->0 when rejected. Mirrorread cannot consumeCC2pending in this
diagnostic. No controllerhook or motorrun.287Python/releasebuildPASS. Installed
B3997A140A8ECCCD4EDA7E29482CD6219A5976DEA518DBD9DB11DD5027A65B61.
filtermirror_ack01 silentUARTinitialreadbackfailure retained; checked
RCC08000000/PD1zero/BDTRc1a/CCR0/0/0 beforeknownUARTclockrepair. ack02PASS
allpulse/latency/ack/restoration/finaloff gates,portclosed. CaptureSHA
4BAA129021373E73581E548B6DEA9EAD5A4D528DEB7904370A6DA9213F71B59C.
Next bounded first-raw-read latencytrace using mirror, phase/reset/stop timestamp
invalidation, latestedge/overcapture ambiguity explicit. No authoritytimestamp
substitution orCPUgainclaim. E336 remainslatestpoweredpass; fullgoalopen.

## Entry 338 — 2026-09-13 — Instrumented edge-to-first-raw-read timing

Optional bench-filter-latency CH1mirror sharesfilteredTI2, armswithvalidatedphase
ticket, invalidatesphase/reset/stop, neverreadsCCR2. Captures24prefix firstactual
rawreads underPRIMASK; returns same single read to core, observercost explicit.
Quietfixture rows withseq/epoch/valid/over/latestCCR1/rawreadCNTbracket/raw/PWM.
Decoder+tests added,290Python/releasePASS. Installed
034C6E94F96624F0447F4A3BFD490FBED4A1AE82F99AB25535B168A15C181584.
source01 threePASS/preflight01 fivePASS/noUARTrepair. Onebounded10s-command
code5earlyarm0/62duties/phase60/coretrace1 failedtracking8 at2040us:
3COM/2acc/15IRQ,COMP79COM283commit25,raw127bus11581,stack2196,finaloff/portclosed.
filterlat_trace62_01 SHA256
F39E9286D9D55535269F90AF362DC3643B8DE71500AA8DCD5308ACB5F30F65C5.
15mirrorrows allmatchCRCtraceactualfirst(invertedreference). Validlatestages
2.5..40us; step6 35/23/19.5/40/29/23.5us, rawbrackets.5..1us. Threeovercapture
rows cannot identify originalIRQedge; no-newcapture rows have no newage.
Instrumentationaddsdelay; notpureIRQlatency. Delayislargefractionof41.7usPWM
period. Next timing-awarecapturequalification design/replay, no fabricated
expectedlevel/acceptance or guardrelaxation. NoCPUgain/filteredlockclaim; goalopen.

## Entry 339 — 2026-09-13 — Timing replay counterexamples and priority divergence

Added3tests callingactualminzcore: frozen capture590 cannot open strictgate600
atlive601; heldexpectedlevel falselypasses actual11good+1bad persistence; using
capture900 ratherthanlive940 changesestimator whileCOMstillarms301atdispatch.
Full199RustPASS. No sharedcoreedit/flash/UART/motor; installed034C6E94,lastoffE338.
Sourceaudit: minz am32_clone.rs1080-1086 setsCOMP/COM equalprio0. Binz has
COMP0x40/COM0x80. ReusedCOMISR enablesCOMP nearend beforezero_crosses update/
return, so pendingCOMP canpreemptCOM inbinz butnotreference. Specific scheduling
divergence, notyetproof of258usCOMoutliercause. Next isolatedpeerprio0x40 test
onunfilteredsource, guard/DMAremain0, disabledpriorityqualification then samepoint
poweredcomparison. Avoidtimestamp/heldlevel shortcuts; preserveactualcontroller
andallguards. NoCPUgainclaim; fullgoalopen.

## Entry 340 — 2026-09-13 — Peer priority reduces COM wall maximum on a full run

bench-com-peer COM64=sensing64, guard/DMA0unchanged. Newdisabledprioritycheck
actuallypendsTIM2insideTIM16: oldpriorityCOM128 yields123(nested), peer64 yields
132(afterCOMreturn).3trialsbothcases/restorationPASS, no peripheral events/gates.
check01silentUARTreadbackfailure retained; probeRCC08000000/PD1zero/BDTRc1a/
CCR0/0/0 beforeclockrepair. check02PASS,source01PASS,preflight01all5PASS.
292Python/releasePASS, installed
EF4BFEC6B91B872A5D9D7FBD003364CB5687E22900C99F5028F3593AA18D7429.
peerprio_hold62_01 full10s62/62phase60trace0 PASS279.645eHz16779COM/16779acc,
COMP75COM75commit45 (E336COM258),IRQ63.562% (E33663.732%),raw258bus11092,
arm50.5us/cost12,stack3196. Runtimepriorityreadback64/64/0/0. Cycle sigma42.039us
vs42.202usbaseline,n1 notmeaningfulnoisegain. Finaloffverified/portclosed.
CaptureSHA FFF72315A1B83810B50CD191E99471FCA329A76433B60B5098C47A2E56382C14.
Supports prioritynesting ascontributor toCOMwalloutlier, notuniversaloutlier
cause orCPUreduction. Next longer/recoveryqualification, leanEXTIpeercomparison.
No guard/acceptance/duty change; nofilteredlockqualification; fullgoalopen.

## Entry 341 - 2026-09-13 - Peer-priority recovery cohort completes 3/3

Unchanged installed EF4BFEC6B91B872A5D9D7FBD003364CB5687E22900C99F5028F3593AA18D7429,
G071/DRV8304H existing wiring, historical supply11.7V/800mA (no new operator
supply reading). Three predeclared original30s campaigns, startup/BEMF62/62,
phase60, carrier24006, trace0; injected tracking loss at2s and one fresh recovery.
All three full fixtures passed, original deadlines and separate first-segment
archives preserved, final outputs off and serial port closed. No failed attempt
in this cohort. Raw artifacts/hashes in captures/peerprio_reentry62_30s_cohort.csv.

Recovered means279.824/279.903/279.962eHz; COM46993/47007/47016 versus accepted
46992/47006/47016. Cycle sigma38.257/38.683/38.296us. IRQunion63.440/63.446/
63.305%, COMmax75us each, COMP73/73/76us, commit45us. Raw peaks268/264/268,
bus minima10960/10686/10865mV, untouched stack2628bytes. Recovery arm spare
above32us floor3.5/2/2us; original deadline spare200/114/101us. These are raw
current codes, not calibrated amps, and COM:event agreement is not independent
rotor qZC proof. Priority readback64/64/0/0, host strict verifier added with
mutation tests;294Python tests pass. No firmware or guard changes this entry.

Priority improvement survives this bounded recovery cohort, but aggregate CPU
load remains high. TIM1 PWM has no per-switch CPU handler; comparator visits
~16.9k/s versus~1.68k accepts/s. Nonaccepting visits are not automatically
removable. Existing irq_accounting already partitions nested per-vector time;
using it changes observer cost versus current union-only mode. Next isolate
lean EXTI with peer priority (correct selected-vector priority readback first),
then measure exclusive costs if needed. No further identical cohort needed.
Full duty envelope, calibrated current and archive parity requirements remain.

## Entry 342 - 2026-09-13 - Lean EXTI with peer priority lowers measured occupancy

Installed BA1F982B00D90E5AA103D90A07172794F3C08CE4E69F424C9E7826BCD235D4AE,
release/s/thinLTO/codegen1. E341 feature set minusbench-filter-bypass; source
diagnostics remain compiled, but BEMF controller uses normal EXTI/ADC_COMP.
Only source edit selects the actual vector for COREPRIORITY readback. Reused
controller, electrical/tracking/arm guards and peer priority unchanged.

peerexti_priority01 failed initial UART/off readback; no motor command. Exact
probe reads RCC08000000/PD1zero/BDTRc1a/CCRs0/0/0 confirmed safe and known
missing USART clock; restored RCC08040000. priority02 three ordering/restoration
trials PASS (NVIC diagnostic still uses TIM2); preflight01 all5 disabled gates
PASS,294Python tests PASS. Powered metadata separately verifies ADC_COMP64,
COM64,guard0,DMA0. Do not call the disabled probe an EXTI peripheral-edge test.

peerexti_hold62_01 full10s PASS at startup/BEMF62/62,phase60,trace0,24006Hz.
280.447427eHz,16827COM/16827accepted,cycle sigma38.962961us. IRQunion58.871093%,
18756.927COMP visits/s,11.147visits/COM,4975.081ADCscans/s. COMPmax71us,
COM69us,commit45us; raw271,bus11092mV,actual arm65.5us/cost12,stack3300bytes.
Final outputs off verified, port closed. Capture SHA256
0DC67472E5D7EDC5FBC9400D12BFD6D7766D020EC9B46C916EFA93D63DEAD1EA.

Versus E340 peer TIM2 hold:63.562466->58.871093% IRQ (4.691373 percentage
points), COM75->69us, while COMP visit rate increased16874->18757/s. Supports
lower path cost rather than simply fewer visits; does not isolate every cause
or establish a silicon floor. n1 hold, recovery on this image untested. No new
PSU reading/current calibration, duty or range change. Next recovery at this
point and/or existing per-vector meter with independently qualified overhead.

## Entry 343 - 2026-09-13 - EXTI peer recovery passes; exclusive reporting prepared

Unchanged BA1F982B build, no flash/guard changes. peerexti_reentry62_30s_01
passes full original30s injected-loss/recovery fixture, startup/BEMF62/62,
phase60,trace0,24006Hz. Fresh recovery acquired7788us,seed1198ticks; resumed
27.989708s near280.472982eHz,47102COM/47102accepted,cycle sigma33.602123us.
IRQ58.897789%,18749.535COMP/s,COMPmax71us,COM69us,commit45us. Raw266,
bus11068mV,stack2696bytes,original deadline spare198us. Actual recovery arm
54us leaves22us above32us floor (TIM2 cohort only2..3.5us); n1 comparison,
not a universal headroom guarantee. All chronology/electrical/archive/finaloff
checks pass and serial port closes. Capture SHA256
D5BD3498472854FEE9F42F5FBBC19494032C61DE89EEEF4A8B5F486AC9C86809.

Extended drv_architecture_report to decode existing per-vector CPUMETER as
well as CPUUNION: sum IRQ contexts exactly once, retain exclusive software
time/calls/means for guard,COMP,COM,sector,polling,ADC DMA. Host synthetic
partition test checks no double counting and invalid meter refusal;295Python
tests pass. Means include observation perturbation, not clean ISR cycle costs.
No per-vector hardware result on this build yet. Next separate characterization
build usesbench-cpu-timing instead ofbench-cpu-union; disabled overhead gate
must pass before power, and compare live perturbation against this baseline.
No new supply reading or calibrated current claim, full objective remains open.

## Entry 344 - 2026-09-13 - Full exclusive meter fails overhead gate; baseline restored

Archived known BA1F982B00D90E5AA103D90A07172794F3C08CE4E69F424C9E7826BCD235D4AE
to captures/reference/peerexti_ba1f/shell-pwm.elf, copy hash verified. Built and
installed973853D224801C990EC7B37850E5244C0C779F7A5C22938F5D6816851292E451,
same features exceptbench-cpu-timing replacesbench-cpu-union. Release/s/thinLTO.
No source edit. exclusive_cpu01 actual disabled readback succeeded, but CPU
check failed unchanged maximum10us gate: inactive sum512/max2, active1536/max6,
nested3008/max12 over256pairs; faults0, all outputs off. NO motor run.

Review shows this reproduces E233's already documented12us per-vector limit.
Do not loop through that optimization path again or relabel12us as a pass.
Restored archived BA1F982B. exclusive_restore_cpu01 UARTsilent retained;
exact RCC08000000/PD1zero/BDTRc1a/CCRs0/0/0 before known RCC08040000 repair.
exclusive_restore_cpu02 union maxima2/7/10 pass, nestedmean9.515625us, finaloff
verified and portclosed. Rebuilt root with union features to match baseline.

No new live exclusive CPU measurement. A lower-cost candidate can label the
existing union's outermost intervals by their root handler, without sampling
each nested boundary. Such intervals INCLUDE preempting work: they provide
attribution bounds, not exact exclusive cost, and need a distinct protocol and
disabled overhead qualification. Avoid manufacturing exclusive times or
subtracting unmatched runs. Full envelope/current/parity work remains open.

## Entry 345 - 2026-09-13 - Root-attributed union implemented; overhead still fails

Optionalbench-cpu-roots labels existing outermost intervals by root vector,
including nested work. No additional clock reads. Separate CPUROOT/CR85 CRC
records preserve CPUUNION semantics and explicitly deny exclusive attribution.
Host checks six unique contexts and exact sum to IRQ union; unknown/mixed/
duplicate/mismatched records refuse. Rust tests cover nested stop across wrap,
reset and multiple root contexts.200Rust/296Python tests pass, release/s/thinLTO.

Initial DA0937BF8EF686CC56C737CBAABE516C72233AA894C0BCEC31895B4F7CDB0A17
installed. rootirq_cpu01 UARTsilent, exact safe RCC08000000/PD1zero/BDTRc1a/
CCRs0 before knownclockrepair; cpu02 faults0 but nested sum2624/max11 over256,
FAIL unchanged10us gate. No motor. Removed redundant per-interval union add:
root mode now sums root bucket only, publishes union=elapsed-foreground at stop.
All200Rust tests still pass. Latest installed
70AB2CCDD1074F3C15F14A351BC5C4257AF686EDCF4A9524B9AA546BA2F55560.
rootirq_fold_cpu01 sameUARTsilent/exactsafeclockrepair; cpu02 inactive368/max2,
active1752/max7,nested2592/max11,allfaults0: still FAIL. Mean nested10.125us,
not a pass or motor authority. Both failures retained, finaloff/portsclosed.

No new motor/current/CPU-attribution evidence. Diagnostic image remains
installed but unqualified; known motor BA1F982B ELF remains archived under
captures/reference/peerexti_ba1f. Do not run this image powered or loosen the
10us gate. Additional meter work must demonstrate a distinct cost reduction;
otherwise return to the proven baseline and broader envelope/current work.

## Entry 346 - 2026-09-13 - Restored baseline; 6.4% hold and recovery pass

Restored exact archived BA1F982B00D90E5AA103D90A07172794F3C08CE4E69F424C9E7826BCD235D4AE.
envelope_restore_cpu01 union2/7/10 PASS withoutUARTrepair, finaloff. Root build
with union features now648191D9E4F268E701BE92B99F4FCE9E37841FA3006D7FD565A586015F181294,
NOTinstalled; do not identify current source rebuild as exact archivedimage.
Root-attribution diagnostics are parked, no motor run on unqualified images.

On installedBA1F, startup62/BEMF64,phase60,trace0,24006Hz; all current/bus/
tracking/age/cycle3226us/event268us/arm32us/deadline guards unchanged.
peerexti_hold64_01 full10s PASS292.121504eHz17528COM/17527accepted,
sigma37.055158us,IRQ58.710913%,raw264,bus10913,stack3300,arm68us/cost12.
SHA7997830C06C4BC93479EA16370C8A9311E69DCFE01FF5DDD37C0F0D0BF1E84B0.
Then peerexti_reentry64_30s_01 fulloriginal30s injected-loss/recovery PASS:
resumed27.989727s292.412549eHz49108COM/49107accepted,sigma28.875041us,
IRQ58.737594%,raw275,bus10948,stack2696. Fresh acquisition7504us/seed1163,
recoveryarm49.5us (17.5abovefloor),originaldeadline251us spare. COMP71/COM69/
commit45us onboth. SHA E30D57DCCDF8A52F7BBDEEA4D7F3307B0E24CE6103807BC4AFB670670C637509.
Bothfullverifiers/finaloff pass, portsclosed. No new PSU reading or amps claim.

Currentbuild6.4% qualifies one hold and one recovery, not repeatability cohort
or fullenvelope. IRQ does not rise with this modest duty/speed increase; no
linear throttle scaling inferred. Next bounded6.5% hold with unchangedguards,
guided by historical6.5cyclefloor failure; retain exact decision evidence if
it recurs, no automaticthresholdincrease. Currentcalibration/parity stillopen.

## Entry 347 - 2026-09-13 - 6.5% still reaches accepted-cycle guard

SameinstalledBA1F, noflash, startup62/BEMF65/phase60/trace0/24006Hz. One10s
command peerexti_hold65_01 stopped after3340816us, CycleTiming12.5998COM/
5997accepted,mean299.233366eHz,cycle sigma57.010995us. Exactstep5 guard
3337566->3340777=3211us violates3226us floor. Raw247,bus11151,COMPmax70us,
COM69us,commit45us,stack2696. Full diagnostic/CRC/finaloff verified, portclosed.
SHA F24E3685BBA457F355765572F681C919A0CDA91F4D906146BA4B549080FC11F1.

Controlleravg1119/prev1118/ci1098/this1074/last1093/wait275/filter12/poll0.
Adjacent reference-counter cycles6865 and6416half-us ticks, pairmean3320.25us.
These are accepted-signal timing, not independent rotor periods. Recorder
same-sector prior timestamp3337596 is30us after guard's previous stamp; keep
clock samples separate. Electrical guards did not cause this stop. Priority
change kept COMwall69us but didnot eliminate cycle-boundary failure. Neither
physicaloverspeed nor exhaustedCPU is proven, and there is no basis to call
longer time-before-fault a reliabilityfix. No higherduty or recovery attempted.

Addedrealcapture regression requiring exact3211 refusal and rejecting full
window completion;297PythontestsPASS. No firmware/guardthreshold changes.
Next establish repeated6.4recovery and review timing-envelope evidence before
any coordinatedrange expansion; don't retry6.5until it happens to pass.
Currentcalibration/archiveparity and broadergoal remainopen.

## Entry 348 - 2026-09-13 - 6.4% recovery cohort completes 3/3

SameinstalledBA1F, noflash/firmware/guard change. Two predeclared additional
30s original-budget injections/recoveries at startup62/BEMF64,phase60,trace0,
24006Hz bothPASS. Combined peerexti_reentry64_30s_01..03 cohort3/3; rawhashes
and all outcomes in captures/peerexti_reentry64_30s_cohort.csv.
02/03 recovered292.484374/292.512751eHz,COM49120/49125 versusacc49119/49124,
sigma29.166256/29.128475us,IRQ58.368165/58.393214%,COMP71COM69commit45.
Raw280/276,bus10841/10877mV,stack2696; recoveryseed1148/1136,actualages190/190
halfus,arms48.5/47us,cost13us. Deadline190/191us spare. Allfullverifiers and
finaloff pass, portsclosed. Startup acquisitionraw1147 in02 (only53counts
below1200),686 in03; keep this separate from recovered current. Noampsclaim.

This establishes bounded repeatability at6.4%, not higher-duty qualification.
No more equivalentcohort needed. Timingfront updated with actual recoveryage
and hypothetical next-profile arm arithmetic; no range guard changed or
CPU-ceiling claim. Next decision must reconcile repeatablepreceding point,
6.5 accepted-cycle variation, coordinated acquisitionlimits and independent
actualarmfloor, without treating a relaxedfloor as a controllerfix. Fullgoalopen.

## Entry 349 - 2026-09-13 - Optional coordinated320 profile staged, not installed

After3/3 guarded292eHz recoveries and69us COMmax, prepare an incremental test
envelope; operatorgoal explicitly permits speedlimit expansion after preceding
stage validation. This does NOT fix6.5crossing variation or reclassify E347.
Newopt-inbench-range320: runtime cycle3125us/event260us; acquisition
cycle6250halfus/individual520halfus/seedmin1041. Profiles250/300/310 remain.
Current/bus/nFAULT/accepted-age/feedback-age/IRQstorm/deadline guards unchanged;
actualseedarm still>=64halfus=32us,armcost<=16us. Startup duty unchanged.

Tests: ci1041/wait260 acceptsage196 exactly64remaining, refuses197; cycle
1041x6 fails6250floor,1042x6 passes; runtime520spacing failscycle3125 while
521passes. Missingevent/current/staleinitialfeedback continue refusing. Host
requires matching newRUNLIMIT+recoverycycleprofile, rejects mixedmetadata.
202Rust/298Python testsPASS, release/s/thinLTO/codegen1 buildPASS with existing
warnings. Candidate28DAC42D32D6B6D321B47106AA1F6F8A4FE468D4771CA8BB20A25B7D5CAB7074
NOTFLASHED. ActualinstalledBA1F,lastverifiedoffE348; no hardware action thisentry.

Next flash and disabledpriority/pulse/atomic/roles/CPU/archive preflight, then
one10s64hold/fullguard verification and recovery before65. Do not infer320eHz
qualification from the profile name or algebraic2us projectedarmspare. Stop
expansion on a failedactualguard and retain evidence. Calibratedcurrent,
independentrotorquality, broader envelope and archiveparity remainopen.

## Entry 350 - 2026-09-13 - Range320 installed; 6.5% full hold reaches300eHz

Installed28DAC42D32D6B6D321B47106AA1F6F8A4FE468D4771CA8BB20A25B7D5CAB7074.
range320_priority01 UARTsilent retained; exactRCC08000000/PD1zero/BDTRc1a/
CCRs0/0/0 before knownclockrepair. priority02threePASS; preflight01 pulse/
atomic/roles/CPU/archive allPASS,CPUmax2/7/10.298PythonPASS. ActualRUNLIMIT
3125/260 confirmed, priority64/64/0/0; current/age/arm/deadline guards unchanged.

Three powered attempts, startup62,phase60,trace0,24006Hz:
- range320_hold64_01 PASS10s292.492721eHz17550COM/17549acc,IRQ58.480265%,
  sigma38.287226us,raw259,bus10841,arm65.5us/cost13,stack3300.
  SHA FA0571FB761A400ED11B646896F0373913035D1BD760EBDD412ED1245C68E31D.
- range320_reentry64_30s_01 PASSoriginal30s injectedloss/freshrecovery,
  resumed292.786992eHz49171COM/49170acc,IRQ58.661755%,sigma29.054582us,
  raw289,bus10948,arm48us/cost13,seed1150/acq7409us,deadline188usspare,
  stack2696. SHA E38DC0C5924D4E9AC0BD429E7D528D48910AB14DED7ED47B3AA9AF25F6458CBB.
- range320_hold65_01 PASS10s299.971347eHz17999COM/17998acc,IRQ58.374829%,
  sigma42.459776us,raw279,bus10865,arm68.5us/cost12,stack2696.
  SHA 19B22D64C24657DBF2769275303E7DF5FB9391A75A7672C970D4768792AD290E.

COMP71/COM69/commit45us allthree; allfullverifiers/finaloff pass,portsclosed.
This is one successful6.5hold underexplicitlywiderprofile, NOT a jitterfix,
6.5recoveryqualification,320eHzmeasurement or physicalCPUceiling. Earlier
310profilefailure remainsvalid. Next6.5recovery beforehigherduty; no further
profilechange implied. No newPSUreading/currentcalibration/fullparityclaim.

## Entry 351 - 2026-09-13 - 6.5% recovery and6.6% hold pass

Same28DAC42D,noflash/guard change,startup62,phase60,trace0,24006Hz.
range320_reentry65_30s_01 fulloriginal30s injectedloss/freshrecovery PASS:
resumed27.990243s299.968707eHz50377COM/50377accepted,sigma32.794254us,
IRQ58.758443%,raw280,bus10829,stack2696. Seed1124/age192halfus/arm44.5us,
cost12us,acq7238us,originaldeadline199usspare. SHA
F1AFA314164F98EF0E17E11C3CA0474DA7BEE220BA74B2C0375184A6C7FE88B7.
Then range320_hold66_01 full10s PASS304.221654eHz18253COM/accepted,
sigma43.054884us,IRQ58.647754%,raw242,bus10877,arm67.5us/cost13,
stack2696. SHA10C834B968B6BFC80E6510B3412E53BBDFB8798E95B9F4663DAA459A2A89580A.
BothCOMP71/COM69/commit45us,allfullverifiers/finaloff pass,portsclosed.
Singlepoints notcohort; next6.6recovery beforehigherduty, profileunchanged.

NonblockingPSUreading request made during65recovery; no reply atrecording.
Do not substitute historicalPSUcurrent. Offline range320_hold65_01 baseline
report verifiesinitialsameepoch: mean centered[9.986030,6.136801,12.065627]
minusbaseline[9.765625,4.859375,11.046875] gives residualsum2.516584counts,
49751scans. Notcalibratedamps/stationarity/driftproof and notapplicableto
recovery afterdriverwake. Currentmeasurement/fullparitygoal remainsopen.

## Entry 352 - 2026-09-13 - 6.6% recovery passes;6.7% hits next cycle boundary

Same28DAC42D,noflash/guard change. range320_reentry66_30s_01 PASSoriginal30s
with injectedloss/freshrecovery: resumed27.990123s304.198236eHz51088COM/51087acc,
sigma32.018024us,IRQ58.827429%,raw307,bus10984,stack2696. Seed1113/age192,
arm43us/cost12 (11us abovefloor),acq7038us,deadline210usspare. COMP71COM69/
commit45. Startupacquisitionraw1059 vs1200guard. SHA
B0152005A95B11E507FC00F5F521E7E2D95789E14DCE521D005B453E820B748C.

Next10s-command range320_hold67_01 FAILED at191465us CycleTiming12,
344COM/343acc,raw215,bus11343,stack2696,COMP68COM67commit45. Exactstep4guard
188315->191426=3111us violates3125floor. Coreavg1061/prev1085/ci1071/this1025/
last1130/wait268/filter12/poll0. Referencecycles6658->6216halfus, pairmean
3218.5us; not independentrotorperiod. Whole-windowmean299.411eHz includes
startup, cannot call equilibrium speed. SHA
923607D0DDBB953E62F0C0025AF26010B2DB79E989FAC8DA14EBDB5052975AAA.
Both raw/chronology/finaloff verify; second fullcampaign rightlyfails. Portsclosed.
No higherduty or recoveryafterfailure. No electricaltrip/COMoutlier indicated;
physicaloverspeed and cause of crossingvariation remainunproven.

Added OPERATING_ENVELOPE.md concise build/profile-scoped map inclfailure,
timingmargin and calibration/independentquality limitations. Next investigate
timingvariation and diminishingrecoverymargin; not anotherautomaticlimitraise.
Actualarmfloor/current/bus/age/deadline guards remainunchanged. Goalopen.

## Entry 353 - 2026-09-13 - Analog hysteresis candidate staged after source audit

Referencecore keeps12live qualification reads atthisspeed; TEMP_ADVANCE16 is
compiled in sharedCOMISR, so a local seed-only override would mismatch ongoing
scheduling. No sharedcore/advance/filtercount edits. SourcebootCOMP2HYST0;
G071PAC bits16:17, cachedHAL None0/Low1. STDS12232typ low10mV provides a
distinct analogthreshold experiment withoutadditionalISRwork; not immunity to
largePWMcoupling, and thresholdshift can change crossingtime/weakBEMFacquisition.
See COMPARATOR_HYSTERESIS_EXPERIMENT.md with primarysource and safetycontract.

Optionalbench-comp-hyst-low bootwritebit16, existing5ussettle, appliesstartup
AND BEMF. Runtime mux preservesfield. FixtureactualCOMPHYSTreadback plusstrict
--low-hysteresis validation;299Python/release-s-thinLTO PASS. Candidate
779C8978BD2FA478D78B91FAFA73F7689E4080C9B11E7014FBF640F827A6CFC4 NOTFLASHED.
Actual28DAC42D archived captures/reference/range320_28da/shell-pwm.elf andhash
verified beforebuild; remainsinstalled,lastoffE352, noUART/probe/motor action.
Next disabledreadback/preflights then64hold/recovery beforehigherpoint; do not
call a lower resulting speed a same-speed jitterwin. Allguards remainunchanged.

## Entry 354 - 2026-09-13 - Low hysteresis acquisition refusal, exact replay

Installed E353 candidate779C8978; disabled COMP2 CSR40010281 (HYST1).
Initial hystlow_priority01 failed UART before motor activity. Exact disabled
register checks preceded USART clock repair. priority02 passes3trials;
hystlow_preflight01 all5PASS, CPU maxima2/7/10us. First motor fixture
hystlow_hold64_01 requested10s, drive62/BEMF64, phase60, trace0, carrier24006.
It never transferred BEMF authority. Under-drive completed20011us/reason2,
90IRQcalls/22accepts, max28us, seedfault3 afterone restart at epoch4.

Offline CRC decode and actual Qualification source identify the FIRST refusal:
epoch4 accepted at4145us, epoch5 at5264us; delta1119us/2238half-us ticks
exceeds unchanged2000tick maximum. Failure latches at epoch5; remaining inputs
cannot revive it. Thus the20ms deadline is the final stop, not the root refusal.
Rawpeak821counts/busmin11366mV; final gates/en/MOE/CCRs0 and nflt1 verified.
Capture SHA211844DDD2ED020CE65956A7FC53AEC5DD44EEABA33CBC52E144D07B65B2BFA9.
No fresh matched PSU current reading; rawcounts are not amps.

Added Rust replay of exact refusal prefix and Python fullcapture regression:
ordinary observation verifies, full10s handoff verifier correctly rejects.
Focused Rust test and5 Python seed tests pass. No new hardware action during
the offline diagnosis. Installed779C remains unqualified; archived28DAC42D is
the known qualified reference. One refusal does not prove analog hysteresis
caused it. No sustained-jitter/occupancy gain is measured. Next baseline A/B,
not guard widening or indefinite restart. Goal remains open.

## Entry 355 - 2026-09-13 - Matched zero-hysteresis baseline passes

Preflash serial off/readback verified; no active hardware job (the unrelated
python process was pdf-mcp). Verified archived ELF SHA
28DAC42D32D6B6D321B47106AA1F6F8A4FE468D4771CA8BB20A25B7D5CAB7074,
downloaded/reset. Initial hystzero_restore_priority01 UARTsilent retained.
Probe RCC08000000,PD1ODR0,BDTR0c1a,CCRs0/0/0,COMP40000281 confirms disabled
outputs and HYST0. Only then restored USART clock RCC08040000. priority02
passes3trials and preflight01 passes all5; CPUmax2/7/10us. No guard changes.

hystzero_hold64_01 full10s PASS, drive62/BEMF64,phase60,trace0,carrier24006.
Acquisition14accepted,seed1540ticks,raw500,bus11557; entryremaining65.5us,
armcost12us. Powered292.575954eHz,17555COM/17554accepted,IRQ58.773629%,
COMP18346.532/s,ADC4975.082/s,cycle sigma38.048057us,COMP71/COM69/commit45us,
raw267counts,bus10901mV,stack3300. Full timeline/finaloff verify, portclosed.
SHA1D69BD998026E8711B904D4834517CE5AA65EEC7EEA76DF4D6B240F2BC9CDE6C.

The known-good build still passes after restoring HYST0; n1paired comparison
does not establish why lowHYST refused or its sustained-run performance.
Keep qualified28DAC42D installed (root779C ELF is NOT flashed). Park startup-wide
hysteresis candidate; pursue sensing scheduling on baseline. No new speed
qualification, calibrated-current claim, CPU floor, or full-goal completion.

## Entry 356 - 2026-09-13 - Existing decision evidence rules out a broad blanking claim

Offline fullverifiers pass comppaths_hold62_01, singlecore_hold62_01 and
singlecore_reentry62_30s_01. Closed/dispatched14.825/17.455/17.475%,
open-no-accept73.627/73.132/73.082%, respectively. Those older instrumented
builds near280eHz do not measure installed28DA path costs. No exclusiveCPU
attribution, no assertion that open-no-accept is proven persistence rejection.
But most measured visits occur after the gate opens: pre-gate deferral alone
does not address them. Report now exposes nullable decision fractions with an
explicit notCPUfractions flag.300Python tests pass, including unknown paths.

Source confirms inline-comp already enables cached-comp. Mode/polarity/trace
cache exists per invocation; physical samples remain live. Shared core closed
postcrossing leaves pending intentionally, unlike open gate which clears then
qualifies live samples. No firmware/sharedcore edits or hardware activity.
Installed28DAC42D,lastoffE355. Focus next on open-gate work/sensing; do not
repackage existing cache or claim90% rejected visits are removable CPU load.

## Entry 357 - 2026-09-13 - Static live-comparator adapter staged

Archived28DA disassembly comp_isr080040e0 shows per-read mode branches at
0800415e/08004174: cached modes still incur runtime selection in persistence.
Added optional bench-static-comp: only real/inverted/traceoff snapshot selects
StaticComp, whose existing live-read helper receives constant modes. Other
methods delegate original Comp; shared core, pending/interval/acceptance logic
and all guards unchanged. Other modes use existing adapter. No hardware action.

Candidate58B570E06486AE16FB868AF83AFDEFCBED8A8BB270214623266E95C21E1CCA0B,
release/s/thinLTO buildPASS; text121756(+492vsarchive),data1104,bss29324.
301PythonPASS incl strict COMPSTATIC/--static-comp provenance. No speed/CPU gain
claimed: same sample count with shorter aperture can change noise rejection.
STATIC_COMP_EXPERIMENT.md sets code-review/disabled/64hold/recovery sequence.
Installed28DAC42D,lastoffE355 remains; new candidate is NOT flashed. Goalopen.

## Entry 358 - 2026-09-13 - Static comparator hold and recovery pass

Installed58B570E06486AE16FB868AF83AFDEFCBED8A8BB270214623266E95C21E1CCA0B
after code review: ADC_COMP08001c88..08001cb0 loop includes liveCSR08001c90,
no per-read modebranches. Preflash offverified; initialpriority01UARTsilent
retained. ProbeRCC08000000,PD1ODR0,BDTR0c1a,CCRs0/0/0 thenclock08040000.
priority02threePASS/preflight01all5PASS CPU2/7/10; originalguards unchanged.

staticcomp_hold64_01 PASS10s,drive62/BEMF64/phase60/trace0/carrier24006:
292.764012eHz17566COM/17565acc,IRQ54.781862%,sigma34.335750us,COMP65COM69
commit45us,raw279bus11151,entryarm68cost13stack3356. Startupraw714bus11462.
SHA A818E333CDFE294546A73D6277047C839AF33D6ADC112F869BB824CC38457A53.
VersusE355 same-setting292.576eHz/58.774%IRQ, about4points lower despite
moreCOMPvisits19566vs18347/s. n1comparison, no exclusiveCPUattribution.

staticcomp_reentry64_30s_01 PASS original30s injected trackingloss/freshseed:
recovered27.989943s292.772192eHz49168COM/49168acc,IRQ55.198629%,
sigma24.799390us,COMP65COM69commit45,raw271bus11116,stack2696.
Startupraw556,bus11414. Recoveryseed1160ticks/acq7474us,arm49us(17spare),
originaldeadline157usspare. SHA
792F5836E9FFD8F8C48F3191A547E48AC4AE0DA9487371FAB2F6B47123FD1F9B.
Fullfixtures/provenance/finaloffPASS,portsclosed. No calibratedamp/independent
rotorquality/higherdutyqualification claimed. Actualcandidate remainsinstalled;
next repeatability then preceding65/66points before67boundary. Goalopen.

## Entry 359 - 2026-09-13 - Static comparator recovery cohort and6.5% hold

Same58B570 installed, noflash/guardchanges. Two additional original30s64duty
recovery attempts PASS, completing3/3fixedcohort. Fullverifier/CRC/provenance
pass; captures/staticcomp_reentry64_30s_cohort.csv retains allhashes.
02:292.968696eHz49202COM/49201acc,IRQ55.163247%,sigma24.803525us,
raw287,bus10984,arm49.5us(17.5spare),seed1163/acq7550us,deadline195spare.
03:292.858377eHz49183COM/49182acc,IRQ54.817940%,sigma25.107948us,
raw283,bus10889,arm50us(18spare),seed1160/acq7472us,deadline236spare.
BothCOMP65COM69commit45,stack2696. Startupraw510/719 not runningcurrent.
Both finaloff/portclosed. No missing attempts withinthiscohort.

Then staticcomp_hold65_01 PASS10s299.737697eHz17985COM/17984acc,
IRQ54.858436%,sigma39.685148us,COMP65COM69commit45,raw232bus10793,
entryarm67.5cost13stack2696. Startupraw501bus11534. SHA
2C44F04772301451AF347B54D9150AA0B86E19A8B845043ED59917699D7EB33D.
All fullfixtures/finaloffPASS,portsclosed. Next65recovery before66; don't spend
more runs oncompleted64cohort. Rawcounts notcalibratedamps, no higherduty
recovery or independentrotorquality claim. Fullgoal remainsopen.

## Entry 360 - 2026-09-13 - Static comparator6.5/6.6% recovery passes

Same58B570 installed, noflash/guardchanges. Fullfixture/provenance/CRC/finaloff
pass for allthree attempts, portsclosed. drive62/phase60/trace0/carrier24006.

staticcomp_reentry65_30s_01 PASSoriginal30s299.907452eHz50367COM/acc,
IRQ54.910540%,sigma29.081127us,raw295bus11032,COMP65COM69commit45,
stack2696,seed1124/acq7121us,arm44.5us(12.5spare),deadline209spare.
Startupraw807bus11510. SHA
888CF7828C28CAA3F0D01D49E1F33FE2B667040F2982C88DE4CBAC2A31F7CA0D.

staticcomp_hold66_01 PASS10s304.289434eHz18258COM/18257acc,IRQ55.386510%,
sigma40.315108us,raw239bus10960,COMP65COM69commit45,stack2696,
entryarm67.5cost12. Startupraw616bus11545. SHA
05F96FC4362D184F88D4708F8151FC745AC56E9BF92811B17352087689617E73.

staticcomp_reentry66_30s_01 PASSoriginal30s304.197063eHz51088COM/acc,
IRQ55.511549%,sigma28.725908us,raw270bus10865,COMP78COM69commit45,
stack2696,seed1112/acq7019us,arm43us(11spare),deadline120spare.
Startupraw624bus11522. SHA
72AC403F32F0455B2D01AB725AE7614EC68293FBED1814EBB162F56BD1DA509E.
TheCOMP78 maximum is retained; lower aggregate load is not a proven65usWCET.

OPERATING_ENVELOPE updated build-scoped results. Next bounded67 test on same
3125/260 profile, noautomatic guardraise. No matched PSUreading/calibratedamps,
independentrotorquality or fullgoalcompletion claimed.

## Entry 361 - 2026-09-13 -6.7% boundary remains despite lower IRQ load

Same58B570, staticcomp_hold67_01 requested10s drive62/BEMF67/phase60/trace0,
carrier24006. FAILCycleTiming12 after304442us,555COM/554acc,raw212bus11140,
COMP69COM68commit45,stack2696. Fulltimeline/CRC/finaloffverify,portclosed.
SHA7FB272D86794FBBD2DA7EE4F1536C1B4FEADBCF76B52249189AAF30152B86729.

Exactguardstep4previous301275->decision304399=3124us violates3125floor.
Controlleravg1076/prev1079/ci1075/this928/last1128/wait269/filter12,poll0.
Referencecycle6486->6243halfus,pairmean3182.25us; not independentrotorperiod.
CPUunion validshortfailedwindow54.769374%, not qualified steadyoperation.
Overall303.828eHz includesstartup and mustnot be called equilibrium speed.

302Python testsPASS incl exactone-usrefusal regression and fullcampaign
rejection. No thresholdchange/retry/higherduty. The occupancy gain is real at
precedingpoints but doesnot remove thisboundary or proveCPU saturation absent
fromeveryinstant. Physicaloverspeed and crossingvariationcause remainunknown.
PORTABLE_WINS records static adapter with aperturetradeoff; OPERATING_ENVELOPE
retainsfailure. Next acceptance-timing/recovery-latency work, notguardrelaxation.

## Entry 362 - 2026-09-13 - Recovery handoff stage budget from existing capture

No hardware/firmware change, installed58B570,lastoffE361. ExistingSEEDLAT
staticcomp_reentry66_30s_01 ages98/118/132/172/176halfus and CORESEEDage192
give stagebrackets49/10/7/20/2/8us. Sum96us: edge-to-entry49, reset10,
feedback7,guardstartup20,reference2,armsetup8. Comparable64/65recovery ages
also entry98/arm192. These are wallbrackets including instrumentation, not
exclusive costs or provenremovabletime. Mandatory20us confirmation stays.

drv_timing_report now validatesoptional SEEDLAT singleton/shape/unit/chronology/
boundedages and returns nullable stages.304Python testsPASS,including actual
fullrecovery and malformed/duplicate/reordered/unset records. No new samples.
Source shows guardstartup builds RuntimeGuard with freshlimits/feedback and
startsTIM6; statistics already staged. Next examine immutable construction
work versus liveadmission checks; no candidate declared before that audit.
TIMING_HEADROOM contains measured budget. Fullgoal remainsopen.

## Entry 363 - 2026-09-13 - Guard constructor selective inlining staged

Sourceaudit: with_limits checks compiletimeprofile, livecampaign/segmentlimits,
seedstep andfeedback before constructing freshmonitor. start_inner checksage,
installs guard, startsTIM6. Statistics already staged. Avoid newprestaging state
without need; optionalbench-inline-guard instead only adds inline(always) to
constructor. No runtimepolicy, ownership or unsafe changes.

Archived/hashverified installed58B570 at reference/staticcomp_58b5 beforebuild.
InitialB92C code removes constructorcall, start_inner stack180->132; copy/clear
stillpresent. FinalwithGUARDCODE marker candidate
7A6E96AA0382BB0AD8BE69D5D1DBAE4AD67604402CCBA61687D29F9C8B9258B9,
text121668(-88vsbaseline),data1104/bss29324. Release/s/thinLTO/codegen1PASS,
203Rust/305PythonPASSinclstrict --inline-guard provenance. No hardware action,
actual58B570,lastoffE361. Codegen improvement is not measuredlatencygain.
GUARD_CODEGEN_EXPERIMENT.md requires finalassembly/disabledgates then64hold/
recovery andSEEDLAT comparison; no67lottery/thresholdrelaxation. Goalopen.

## Entry 364 - 2026-09-13 - Inline guard runs pass, latency worsens; restored

Installed7A6E afterfinalassembly132bytelocalframe confirms. priority01threePASS,
preflight01all5PASS CPU2/7/10; noUARTrepair. drive62/BEMF64/phase60/trace0,
carrier24006/allguards unchanged. inlineguard_hold64_01PASS10s292.630438eHz,
17558COM/acc,IRQ54.726716%,sigma34.180238us,raw261bus10984,COMP65COM69
commit45,arm55.5cost12stack3356. Startupraw997bus11510. SHA
092DF0FA45AFD5D0074BC772646BA5E0A98741196BE173EBAC46BF65ECEB9715.

inlineguard_reentry64_30s_01PASSoriginal30s292.835859eHz49180COM/49179acc,
sigma24.801852us,raw264bus10889,COMP65COM69commit45stack2696. Seed1153/
acq7428us,arm42cost12,deadline250spare;startupraw787bus11545. SHA
69273151CEFE6196710BD701F536A5C3EAB31EE4A99FC53EEC37AF71D2F8CD08.
SEEDLAT48/10/8/26/2/8us sums102us: guardstage26vs20baseline, armage102vs96.
No favorable latency change established by thisn1; no higherduty/cohort.
Bothfullfixtures/provenance/finaloffPASS,portsclosed. Smallerstack/text isnot
fasterguardstartup. Park optionalfeature; no safetythresholdchanges.

Restored archived58B570, inlineguard_restore_priority01threePASS/finaloff,
noUARTrepair. Actualbaselineinstalled,root7A6E isNOTinstalled. Goalopen.

## Entry 365 - 2026-09-13 - Borrowed guard slot rejected offline

No hardware. LocalSome(guard)/take() inside maskedinstallation preserves checks
but releaseassembly emits TWO68bytecopies at08003e74/08003e84 vsbaselineone.
Callerlocalstack180->188,closure76extra,text121836(+80),data/bssunchanged.
CandidateAF4226C57A84C7C8202F6D799AC6C9739476C6B8C213BDE843D7594F26791A75
neverflashed. Removed experimentfeature/source immediately. Rootbinary remains
rejected and mustnotflash. Actual58B570,lastoffE364 unchanged. Next structural
validation/inplaceinstallation audit, no artificialhealthyguard or age/deadline
refresh. No timinggain claimed fromassembly. Goalopen.

## Entry 366 - 2026-09-13 - Fresh admission token/in-place guard staged

Optionalbench-guard-install: fresh profile/campaign/segment/seed/feedback/age
validation returnsprivate nonCopy profile-specific Admission. Immediateexisting
maskedsection consumes it to constructdestination; then unchangedCLOCK/ACTIVE/
TIM6/NVIC publication. No globalstaging,newunsafe,age refresh or ownershipchange.
Default API unchanged.205Rust testsPASS including cross-product refusalorder,
wrappingfeedbacktimestamps/polls/eventhistory equivalence;306PythonPASSincl
strictGUARDINSTALL/--guard-install. No hardware activity.

Pre-markerDEDBCB code callerstack52vs180,installer24byteclear/fieldstores,
no fullguardmemcpy. Maskedinitializer durationNOTmeasured. Finalcandidate
957F6C5A1CD9F78CE818141263571500299B27C05A4492882A23E64556947F3F,
text121628(-128vs58B570),data1104/bss29324;release/s/thinLTO/codegen1PASS.
Actual58B570,lastoffE364 remains. GUARD_INSTALL_EXPERIMENT.md setsfinalcode/
disabledguard/preflight/64hold/recovery gates and SEEDLAT comparison. Goalopen.

## Entry 367 - 2026-09-13 - In-place installation improves measured handoff

Installed957F finalassembly retains24byteclear/fieldstores,no fullguardcopy.
priority01UARTsilent: safeRCC08000000/PD1ODR0/BDTR0c1a/CCRs0 thenclock08040000.
priority02threePASS. Newdrv_guard_check records3disabledtimerfaults4/8/1 and
18poststopwriterrefusals, finaloffPASS. preflight01all5PASS CPU2/7/10.
309PythonPASSincl retainedguardcheck/malformedfault/poststop/finaloff tests.

guardinstall_hold64_01 PASS10s292.859510eHz17572COM/17571acc,IRQ55.054866%,
sigma35.042979us,COMP65COM69commit45,raw276bus10877,arm72.5cost13stack3356.
Startupraw737bus11175. SHA
790E1C3CD1EF4F252233821036DF367DCB2D48D0C2447BC1BB868A126751B68A.

guardinstall_reentry64_30s_01 PASSoriginal30s292.870297eHz49185COM/49184acc,
IRQ55.246208%,sigma25.076083us,COMP65COM69commit45,raw267bus10817stack2696.
Seed1163/acq7416us,arm54.5cost12(22.5spare),deadline187spare.
SEEDLAT49/10/7/15/2/8us sums91: guardstage15vs20baseline,totalage91vs96.
Startupraw675bus11534. SHA
4CCEB9F95E735E5812DE02800E86317D8A66A5D7514069DE1DF2CB462D5EBBDE.
N1 favorable latency, notWCET or isolatedmaskedsectioncost. Fullfixtures/
provenance/finaloffPASS,portsclosed.957Fremainsinstalled. No safetychanges,
calibratedamps or higherdutyqualification. Nextrepeatability then65/66 before
coordinatedspeedprofiledecision. Fullgoalopen.

## Entry 368 - 2026-09-13 - In-place guard recovery cohort 3/3

Same installed957F, G071/DRV8304H map and operator11.7V/800mA setting.
Completed pending guardinstall_reentry64_30s_02 (terminalexit0), then03.
No competing serial/probe process before03. Startup6.2%, BEMF6.4%,24006Hz,
phase60,trace0, staticcomp/peerpriority/singlecoreatomics/guardinstall unchanged.
Both full fixtures PASS injected tracking stop, fresh recovery, original30s
deadline, chronology/CRC/electrical guards and finaloutputs-off. Ports closed.

02: recovered27.990039s292.861933eHz49183COM/49183accepted,IRQ54.769142%,
sigma24.830430us,raw275bus10937,seed1150/acq7333us,arm53cost12,
deadline206usspare. Startupraw449bus11534. SHA
9bf50965f1e67656d152834028f357c4dabcef9a8b7933ac59141b7942bf0ba7.
03: recovered27.990038s292.728466eHz49161COM/49160accepted,IRQ55.282941%,
sigma24.555539us,raw290bus10865,seed1148/acq7323us,arm52.5cost12,
deadline208usspare. Startupraw519bus11569. SHA
ade681071a8658c34f9e087ab0a200edf0040ba5e11f55107432ec3e70476d84.
BothCOMP65COM69commit45us,stack2696. AllthreeSEEDLAT49/10/7/15/2/8us=91.
Manifest captures/guardinstall_reentry64_30s_cohort.csv includes01 fromE367.
3/3 full attempts, no excluded failures. Favorable handoff stage repeatability
(15vs20us baseline), not masked WCET or lower steady IRQ demand. No calibrated
amps/independentrotorquality claim. Next65/66 checks on957F before coordinated
profile decision; fullgoal remains open. No more same64 cohort needed.

## Entry 369 - 2026-09-13 - Guard installation passes preceding65/66 points

Same957F installed, G071/DRV8304H, operator11.7V/800mA setting. Startup62,
BEMF65 then66, phase60,trace0,24006Hz; all previous guards unchanged.
Sequential hold then original30s injected-loss recovery at each point.
Allfour fullfixtures/provenance/CRC/chronology/deadlines/finaloff PASS, portsclosed.

guardinstall_hold65_01:10s299.918194eHz17995COM/accepted,IRQ54.838281%,
sigma39.397457us,raw264bus11116,initialarm70cost12. Startupraw513bus11498.
SHA1f76576c683bd834f06a9f4c55c1187c2d79c5b03fdcb5be11ce19ec360f8f22.
guardinstall_reentry65_30s_01:27.990288s299.765142eHz50343COM/accepted,
IRQ55.283319%,sigma29.210767us,raw278bus10984,seed1126/acq7174us,
arm50cost12(18spare),deadline236spare. SEEDLAT49/9/8/15/2/8us=91.
Startupraw511bus11557.
SHAc9ea67807ceca995102b0ae05ba89970044095519fe5d38fa5a227886a461c68.

guardinstall_hold66_01:10s304.179745eHz18251COM/18250accepted,
IRQ55.056667%,sigma40.003845us,raw250bus10925,initialarm70.5cost13.
Startupraw974bus11569: do not confuse this near-guard peak with running250.
SHA521955f83d27c96fdee70338251ad24e2ddb91b5c7acc1031b9c1f1c1c0979c2.
guardinstall_reentry66_30s_01:27.990263s304.362638eHz51115COM/accepted,
IRQ55.489570%,sigma28.784949us,raw247bus10769,seed1112/acq6987us,
arm48cost12(16spare),deadline249spare. SEEDLAT49/10/7/15/2/8us=91.
Startupraw424bus11510.
SHAea8e69207c00d5d89422d36184e263c5155e9bf5e88193f2011e9324c51aec0e.

All observedCOMP65COM69commit45us,untouchedstack2696. Previous build recovery
arms44.5/43us become50/48us here; measured setting-specific gain, not WCET or
cycle-jitter cure. One hold/recovery perpoint, not repeat cohorts. Next a
coordinated speedprofile/arm-budget audit; no automatic67 retry or threshold
relaxation. Calibratedcurrent and independentquality remain open; goalactive.

## Entry 370 - 2026-09-13 - Prospective speed versus seed-arm budget

Offline only, installed957F and lastoffE369 unchanged. Read source reference
advance_of/wait_time/TEMP_ADVANCE and actual Seed admission. E368-369 five
recoveries measured91us seedage (not WCET). At prospective320/325/330/340
floor seed intervals1041/1025/1010/980ticks, reference wait260/256/253/245ticks
leaves39/37/35.5/31.5us arm time. Thus330only3.5us above32floor;340fails by0.5.
New measured_seed_age_limits_prospective_speed_profiles test uses actual
reference arithmetic and confirms current admission refuses1010/unsupported
profiles.206Rust testsPASS; existing incrementalAccessDenied notes nonfatal.
No firmware profile enabled, no hardware, no guard changes.

SPEED_ARM_BUDGET.md records coordinated acquisition/runtime requirements and
separates this entry limit from sustained cycle jitter/CPU saturation. Next
audit preparation before the finalqualifiededge (or a fresh qualified-edge
rendezvous after preparation), preserving continuous guarded drive, ownership,
fresh feedback, original timestamps/budget and32us floor. Existing statistics
staging is alreadydone; no tokenprestaging/fakeedges. Fullgoal stillopen.

## Entry 371 - 2026-09-13 - Handoff ownership audit rejects naive early setup

Offline only; root ELF SHA957F matches installed record,lastoffE369 unchanged.
Source distinguishes initial driven Transfer/adopt_driven from recovery:
resume_once prepare/wake ->reacquire_awake ->acquire_inner gatesoff scanning.
Recovery is awake gate-disabled sensing, NOT continuous guarded drive.
TIM6/reference and bulk statistics are already initialized before scan.
Moving24k carrier preparation earlier fails to persist: bridge_clear invokes
revoked_restore_af, restores PWM_ARR6399 and revokes PREPARED. Fresh baseline
timestamps/admission/budget/actualseed checks cannot be moved into stale tokens.

Generated957F acquire_inner08008d70 sizea88, reacquire08009a68 sizeac.
Recoverypublication080096e4..9718 emits directfieldstores/16bytecyclecopy,
not an extra64byte report memcpy. Publisher functions alreadyinline; no
borrow/inline patch justified. Awake wrapper skips oldbaseline restore.
SPEED_ARM_BUDGET.md now records exact move constraints and correction.

Next quantify successful acquisition epilogue/return before continuation
design:49us includes20us mandatorydwell, remainder not exclusivelydiagnostics.
Directcallback keeps large acquisitionframe on poweredstack; inspect frame/
stack before implementing. No firmware/runtime/profile changes, no hardware,
no guardrelaxation or fullgoal claim.

## Entry 372 - 2026-09-13 - Optional acquisition epilogue instrumentation

No hardware. Archived installed957F ELF at reference/guardinstall_957f before
build, hash verified. New optionalbench-acquire-timing records original edge
and qualified/cleared/published/returned timestamps on awake recovery success.
Fourclockreads on return path only, no samplingloop/steadyISR additions.
Capture-only ACQUIRELAT output afterrun. Does not refresh authority/feedback/
seed timestamps. Host optionalstrict parser validates monotonic20msbound,
20usminimumdwell and return-before-SEEDLATentry, reports instrumented brackets
without removable/exclusive claims.310PythonPASS including malformed inputs.

Initial build omitted no-default-features and failed incompatible atomics;
corrected --release --no-default-features with priorfeatures+acquiretiming PASS.
Candidate283E5E7B350D1AFB097507252AD6AEE3B8580997DD03B5B6CD3F333AE9645617,
text122016,data1120,bss29324, s/thinLTO/codegen1. NOTFLASHED; actual957F/
lastoffE369 unchanged. Baseline acquire_inner reserves508bytes localstack
plus savedregisters, relevant to any continuation design. Probe cost unknown.
ACQUIRE_TIMING_EXPERIMENT.md defines code/stack/disabledpreflights then one
matched64recovery with fullguards/finaloff and requiredmarker timingreport.
No speedprofile/safetychange, fullgoalopen.

## Entry 373 - 2026-09-13 - Acquisition stage timestamps measured

Flashed283E5E7B after UARTverifiedoff; generated acquisitionframe508 unchanged.
Postresetpriority01silent retained; exactRCC08000000/PD1ODR0/BDTR0c1a/CCRs0
preceded USARTclock08040000 repair. priority02threePASS,guard01threefaults/
18poststoprefusalsPASS,preflight01fivePASS CPU2/7/10. Same11.7V/800mA setting.
acquiretiming_reentry64_30s_01 fullPASS original30s startup62/BEMF64phase60
trace0carrier24006. Recovered292.832646eHz49178COM/accepted,IRQ54.784513%,
sigma24.727751us,raw263bus10853,COMP65COM69commit45,stack2680.
Seed1147/acq7408us,arm49.5cost12(17.5spare),deadline150spare.
Startupraw868bus11569. SHA069bdc27e874ed347f21f456f8abab31015b0c1aa754d344ff3fd9190d96722a.
ACQUIRELAT14744/14816/14826/14832/14836ticks:36/5/3/2us then5usreturn-to-entry.
SEEDLAT51/10/8/15/2/8=94us versus91baseline. Mandatory20usdwell is inside36,
not removable; timestamp/layout effects included, n1 notexclusive/WCET proof.
Fullfixtures/provenance/timingreport/finaloffPASS,portsclosed.311PythonPASS
with retainedstage regression. Next finalcandidate/edge bookkeeping review;
report/callerreturn alone not the hypothesized large gain. Actual283Einstalled,
957Farchived qualifiedreference. No guards/profilechanged; fullgoalopen.

## Entry 374 - 2026-09-13 - Exact bounded division candidate

FollowingE373 measured36us qualification bracket, generated actual283E edge
routine calls __aeabi_uidiv at0800b286 for sum/12. Exclusivecost unknown.
Optionalbench-seed-div12 replaces accepted-domain division with exact
(sum*21846)>>18 forsum<=24000; fullwidth fallback retainsdivision.
No count/interval/cycle/dwell/seedage changes.207RustPASS including exhaustive
24001sum values andfallback extremes; release/s/thinLTO/codegen1PASS.
Candidate1C974384B2E0C0F856BAFBC3B6D7BF0A6261EC6D0B5C7388AFD3DD69FF7BE6E9,
text122040,data1120,bss29324. NOTFLASHED,actual283E,lastoffE373 unchanged.
SEED_DIVISION_EXPERIMENT.md sets next codegen/provenance/disabledpreflight/
singlematched64recovery checks. No speedup or higherprofileclaim yet.

## Entry 375 - 2026-09-13 - Automatic post-link arithmetic inspection

Operator requested automatic objdump scrutiny of division/wide arithmetic.
Added Windows targetlinker wrapper invoking actualtoolchain rust-lld then
objdump -d -S -C. Sidecars preserve sourceinterleavedassembly and SHA-bound
caller/address/helper/category findings. Directcalls/tailbranches scanned;
inline/indirect arithmetic not exhaustively detected. Empty exactallowlist
supports reason annotations without hiding findings. Auditfailure blockslink;
findingsadvisory, not blanketfailure on intentional arithmetic.
Fullrelease/s/thinLTO/codegen1 rebuildPASS viahook;313PythonPASS. Report/root
SHA148386B19BE2ABC77E8D33D3C8C015E1D7D0915BB73C4FC50589E47B6B95A8E1.
102calls flagged84division/18wide64, no128/floathelpermatch (notabsenceproof).
Actual283E,lastoffE373 unchanged; rebuiltrootNOTFLASHED. MATH_AUDIT.md explains
cachehits, Windowsdependency, manualrescan and reportlimits. Continue E374
fastbranch/provenance/hardwarequalification; no speedupclaim or guardchange.

## Entry 376 - 2026-09-13 - Audit catches speculative division; const derivation

E375148386 assembly performeduidiv0800b28a before rangecheck: initial seedmath
candidate still divided for accepted sums. Caught beforehardware. Fallback
now cold/noinline withopaqueinput; arithmetic preserved acrossu32domain.
Finalcode checksbound0800b2ac/branchesb2ae, MULb2b2/LSR18b2b4 on accepted
sums; fallbackcallonlyb2f6. Operator requested leaning into const: reciprocal
ceildivision, bound/error/overflowassertions explicitlyconst and absent from
runtime path. Measuredsum remainsruntime. SEEDMATH/--seed-div12 exactfixture
provenance added,314Python/207RustPASS. Fullrelease/s/thinLTO/codegen1+mathaudit
PASS, candidate6B71A8226E9964FF15287B97A3F5B19064CBCB61C23A79DFB66F15F2495464B3,
text122160,data1120,bss29324. NOTFLASHED; actual283E,lastoffE373 unchanged.
Next disabledpreflights thenmatched64recovery/strictACQUIRELAT compare;
no hardwaregainclaim, profile/guardchanges or goalcompletion.

## Entry 377 - 2026-09-13 - Exact seed mean improves measured handoff

Installed6B71 afterUARTverifiedoff. Postresetpriority01silent retained;
RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 checked thenUSARTclock08040000 repair.
priority02threePASS,guard01threefaults/18refusalsPASS,preflight01fivePASS
CPU2/7/10. Sameoperator11.7V/800mA setting. No guard/profile changes.
seeddiv_reentry64_30s_01 fullPASS original30s startup62/BEMF64phase60trace0
carrier24006: recovered292.546657eHz49131COM/49130acc,IRQ54.960206%,
sigma24.659004us,raw276bus10853,COMP65COM69commit45,guardmax21,stack2680.
Seed1161/acq7486us,arm56cost13(24spare),deadline144spare. Startupraw511bus11545.
SHAc58c1749f6c5153e7c35cabbe468a3fd6428faddfdb4d922c8fbad057920b2c2.
ACQUIRELAT31/6/3/2/5us vs36/5/3/2/5; SEEDLAT47/10/7/15/2/8=89vs94us.
Matched-settingn1 measuredgain, notexclusivecost/WCET. Seed difference means
arm56vs49.5 gain not solelycode. Fullfixture/SEEDMATH/timing/finaloffPASS,
portsclosed. Next retain exactmath in lean no-acquiretiming build, qualify
before profile expansion; no more equivalent instrumented64cohort needed.

## Entry 378 - 2026-09-14 - Lean exact seed math passes recovery

Archived6B71 probeELF reference/seeddiv_probe_6b71. Built/installed4D43E26B76B77B3C44C570ECA13866D67682ED6BFBB1DCD4CE07FF7BD65153A5
withoutacquiretiming; allguards/seedmath retained. Release/s/thinLTO/codegen1
automaticauditPASS,text121768,data1104,bss29324. Emittedrangebranch/MUL/LSR18
and separatefallback verified. PreflashUARToff,postresetpriority01silent;
RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 beforeUARTclock08040000 repair.
priority02threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
Sameoperator11.7V/800mA setting, startup62/BEMF64phase60trace0carrier24006.
seedlean_reentry64_30s_01 PASSoriginal30s, recovered292.845476eHz49181COM/
49180acc,IRQ55.207670%,sigma24.714818us,raw278bus10937,COMP65COM69commit45,
stack2696. Seed1151/acq7470us,arm57cost12(25spare),deadline179spare.
Startupraw530bus11378. SEEDLAT44/10/8/15/2/8=87us,ACQUIRELATabsent.
SHA7fb698ddbf00c91264b3f4d4b52e429bc8d99240f2503feec8e2c02c5a7da8b0.
Fullfixture/provenance/timing/finaloffPASS,portsclosed. N1notWCET/higherduty
qualification; next65/66 precedingchecks beforeprofileaudit. Fullgoalopen.

## Entry 379 - 2026-09-14 - Lean exact math passes65/66 hold and recovery

Same4D43 installed, sameoperator11.7V/800mA setting, startup62,phase60,trace0,
carrier24006, allguards unchanged. No competingserial/probe process atstart.
Sequential65hold/recovery then66hold/recovery, allfour fullfixtures/provenance/
CRC/chronology/electrical/deadline/finaloffPASS, portsclosed.

seedlean_hold65_01:10s299.836482eHz17991COM/17990acc,IRQ54.821901%,
sigma39.647292us,raw259bus10996,arm75cost12. Startupraw571bus11390.
SHA396d0dc05319735eff6d3e0bca5120123c8f032da3d89586cc3ceb098da8ba13.
seedlean_reentry65_30s_01:299.879047eHz50362COM/acc,IRQ54.880259%,
sigma29.375666us,raw276bus11008,seed1127/acq7251us,arm54cost12,
deadline244spare. Startupraw560bus11545.
SHA6e6bf998d70d52041c97a6a43a79765d58decc6bcaae7214b2e8216e9bb2cf08.
seedlean_hold66_01:10s304.093846eHz18246COM/18245acc,IRQ54.883347%,
sigma40.282463us,raw261bus10996,arm75cost13. Startupraw714bus11557.
SHA6da3b9663f8f02c35f00689fcff0a6869ff284ddf5e2e0e07bdf6487125a1ce6.
seedlean_reentry66_30s_01:304.319682eHz51108COM/acc,IRQ55.575930%,
sigma28.793524us,raw250bus10769,seed1114/acq7056us,arm52.5cost12,
deadline174spare. Startupraw771bus11534.
SHAe01467c49a335677e716bdda88cf23b21bbb67a1edec2d2447b3584510ddeb56.

AllCOMP65COM69commit45,stack2696. Both recoverySEEDLAT44/10/8/15/2/8=87us,
armspare22/20.5us. Onehold/recovery perpoint, not repeatabilitycohorts/WCET.
Next coordinatedprofiledecision with87us measuredage, not more65/66repeats
or blind67lottery. Currentcalibration/independentquality/fullgoal remainopen.

## Entry 380 - 2026-09-14 - Stage coordinated330 test profile

No hardware; actual4D43,lastoffE379 unchanged. ArchivedexactELF at
reference/seedlean_4d43 beforebuild. E378-37987us measuredage/preceding65/66
passes support boundednextprofile, notCPUceiling/jitterfix. At1010seedticks,
wait253-age174=79ticks=39.5us,7.5above unchanged32floor (notWCET).
Optionalbench-range330 runtime3031/252us, acquisition6062/504halfus,
seedmin1010. Twelveintervals/sevencycles and all independent electrical/
age/tracking/deadline/armcost protections unchanged. Hostmetadata coordinated.
209Rust/316PythonPASS includesseedage189pass190refuse,3030cycle refusal,
current/age/tracking and mismatchedprofile protocoltests. Release/s/thinLTO/
codegen1+mathauditPASS;text121780,data1104,bss29324. Candidate
F7155BFB3A5E1CBEEDA8D9CADE8A7F91EFC290093970E7717517102DF279124C NOTFLASHED.
RANGE330_EXPERIMENT.md: disabledpreflight thenmatched66hold/recovery exact
profile readback, onlythenbounded67hold; retainfailure, noautoprofileraise.

## Entry 381 - 2026-09-14 -330 profile qualified at66;67hold passes

InstalledF7155BFB afterUARToff. Postresetpriority01silentretained, exact
RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 beforeclock08040000 repair. priority02
threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
Sameoperator11.7V/800mA setting,startup62phase60trace0carrier24006.
range330_hold66_01PASS10s304.082329eHz18245COM/18244acc,IRQ55.057877%,
raw271bus11128,arm76.5cost13,stack3356;startupraw627bus11545.
SHA7fc5e98402ad90c48744c4b5b691534da869fbcd1533c3e7bfdaa01f9095276f.
range330_reentry66_30s_01PASS304.043397eHz51062COM/51061acc,IRQ54.991055%,
sigma28.653611us,raw272bus10925,arm52cost12,deadline194spare,seed1113/
acq7053us,SEEDLAT87us,stack2696;startupraw757bus11522.
SHA61e016f8c5945dbf00fb3148d250714fa827aecec24e6df85389e12cecddfee2.
Thenrange330_hold67_01PASS10s309.987370eHz18599COM/acc,IRQ55.545126%,
sigma39.488256us,raw265bus10913,arm74.5cost12,stack2696;startupraw760bus11438.
SHA8c5d5bb84f9841a52c556470b00913ca9ceec81f9ef7bf3aacdb483cb9b4b245.
AllCOMP65COM69commit45;fullfixtures/provenance/3031/252readback/finaloffPASS,
portsclosed. Next67recovery beforehigherduty. Expandedprofile evidence not
jitterfix or erasureofoldrefusals; no330eHz/cohort/calibratedcurrentclaim.

## Entry 382 - 2026-09-14 -67 recovery and68 hold/recovery pass

SameF715profile3031/252, operator11.7V/800mA setting,startup62phase60trace0
carrier24006. No competingserial/probe process atstart; no firmware/guardchange.
range330_reentry67_30s_01 PASSoriginal30s310.115756eHz52082COM/52081acc,
IRQ55.034410%,sigma25.097318us,raw263bus10746,seed1088/acq6946us,
arm49cost12(17spare),deadline231spare,COMP65COM69commit45,stack2696.
Startupraw815bus11522.
SHA071c43175dd2da1786c4cbd5ffa41bb8776df0b7b29f943bdae85ad90b699053.
range330_hold68_01 PASS10s316.514610eHz18991COM/18990acc,IRQ55.477931%,
sigma42.460652us,raw250bus10889,initialarm77cost12,stack2696.
Startupraw626bus11545.
SHA3264f53564273b93d15ea882d73bff50a239693ded08c9cf2d4bd2a1882c92c5.
range330_reentry68_30s_01 PASSoriginal30s316.583125eHz53169COM/53168acc,
IRQ55.334205%,sigma28.314809us,raw248bus10984,seed1064/acq6724us,
arm46cost12(14spare),deadline135spare,COMP65COM70commit45,stack2696.
Startupraw466bus11522;initialtransfer release-to-foreground32us,arm63us,
retain this variability rather than assuming fixed20us initialrelease.
SHA5d6fea59b83e6baefecd83f02b46ac8b540ac2f21ceec83e5851e4b2c38b4f00.
Both recoverySEEDLAT44/10/8/15/2/8=87us. Fullfixtures/provenance/chronology/
deadline/electrical/finaloffPASS,portsclosed. Oneattempt pernewcondition,
notrepeatcohort/WCET. Next bounded69hold SAMEprofile; retainfailure, no
automaticprofileincrease. Goalcurrentcalibration/independentquality stillopen.

## Entry 383 - 2026-09-14 - Retained6.9% same-profile cycle refusal

Capture range330_hold69_01.txt, sameF715 build/profile3031/252us,
startup62/phase60/trace0/carrier24006, requested10s hold at69tenths duty.
Operator supply setting remains11.7V/800mA, not a new independent reading.
Fixture exited1: powered tracking window did not complete. Observed421292us,
CycleTiming12,797COM/796accepted,raw peak247counts,bus minimum11044mV,
untouchedstack2696. Outputs-off and timeline verified. No restart or higher
duty attempt followed. Raw counts are not calibrated amps.
SHA256 3fd3151bfe1c6714287d5a5b5514c18cbe7dcbdc42f3f2e80d5369e5f8bc547b.

Offline drv_cycle_fault inspection: step1 guard previous418247us,
decision421256us, delta3009us,22us below3031floor. Recorder's prior same-sector
timestamp418274us is27us later than the guard timestamp; do not mix clocks.
Controller average1044ticks, thisZC1025,lastZC1000,wait255,filter12,
polling0/running1 at fault snapshot. Previous/refused reference cycle sums
6424/6012half-us ticks, paired mean3109us. These are controller intervals,
not an independent rotor period or proof of the cause. Short-window mean
315.320eHz and sigma154.674us do not establish steady speed/quality.
No CPU saturation or physical overspeed cause established by this refusal.

Added retained-capture regression asserting exact guard/reference context,
verified off, unknown cause, and rejection by the full10s verifier.
317 Python tests PASS. Firmware unchanged; no hardware activity during this
offline inspection. Envelope and AGENTS updated. Next acceptance-timing and
recovery-arm-budget audit before further profile decisions, not a blind retry
or automatic guard relaxation. Full goal remains open.

## Entry 384 - 2026-09-14 - Lean recovery boundary arithmetic

No hardware activity or firmware change. Current reference advance_of/wait_time
and TEMP_ADVANCE16 checked against source. E378-382 measured87us seed age
leaves prospective330/340/345/350/351 boundary arm spare7.5/3.5/2/0/-0.5us.
These are hypothetical floored seed intervals, not new speed qualification or
WCET; another5us invalidates340. Current admission still rejects below1010.
Added actual-reference arithmetic/admission regression;210 Rust tests PASS
with range330/cpu-roots/guard-install/seed-div12. Existing incremental-cache
AccessDenied notes nonfatal. Updated SPEED_ARM_BUDGET.md and AGENTS.
This separates E383's cycle refusal from recovery latency. Next inspect the
preparation/final-qualified-edge boundary, retaining all live checks and exact
ownership, before widening profiles. ActualF715,lastoffE383 unchanged.

## Entry 385 - 2026-09-14 - Recovery duplicate-clear candidate staged

Source tracing finds successfulawake acquisition clears bridge before return;
resume_once only checks budget/storesmetadata before coast_run_inner clears
again. Optionalbench-reentry-clear-once omits secondclear only on resume.
Initial handoff, firstclear, fault safing, livechecks, freshfeedback/guard,
carrier preparation and32usarmfloor unchanged. No earlyauthority or timestamp
refresh. See REENTRY_CLEAR_EXPERIMENT.md for obligations/nextqualification.
InstalledF715 archivedhashverified reference/range330_f715, lastoffE383.
StagedNOTFLASHEDA727F8D7274EA33B7A1B54253E8F5D7639A2A467C7E985A82D5D5E6B1B5C7E6B.
Release-s/thinLTO/codegen1 PASS,automatic+manual mathaudit ran successfully
with102advisoryhelpercalls.318PythonPASS including strict optional provenance.
No hardware or measured latency result. Next emittedbranch/stackreview before
disabledchecks and matched68recovery; no higherduty/profile experiment yet.

## Entry 386 - 2026-09-14 - Clear-once recovery hardware pass

InstalledA727F8D7274EA33B7A1B54253E8F5D7639A2A467C7E985A82D5D5E6B1B5C7E6B.
Assembly initialclear08005972 retained,resume skip59ac/59b0,carrier59c0;
coast localstack204 unchanged,text121900/data1104/bss29324.
COM41 verifiedoff beforeflash, unrelatedCOM8 ss untouched. priority01UART
silent retained; RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 exactdisabled checks
before08040000UARTclock repair. priority02threePASS,guard01threefaults/
18poststoprefusals,preflight01fivePASSCPU2/7/10. All before motor command.

clearonce_reentry68_30s_01 PASSoriginal30s, startup62phase60trace0carrier24006,
operator11.7V/800mA setting, no fresh matchedPSU reading. Recovery316.398247eHz,
53138COM/53137accepted, IRQ55.208746%,sigma28.268248us,raw247bus10937,
COMP65COM69commit45stack2696. Startupraw433bus11187. Seed1060/acq6635us,
arm47.5cost13(15.5spare),deadline215spare,SEEDLAT44/9/7/15/2/8=85us.
SHA2e1c9cf0e8dd0a11417aa4f23edb93e1dc10b54bad6cf5c991a339b6b097cff4.
Fullfixture/newmarker/timing/architecture/finaloff verification PASS,portclosed.
ComparedE38287us,2us lower includes1us inreset-to-feedback; not exclusive
clearcost/WCET or steadyCPU improvement. Oneattempt,nobroadercohort or new
speedqualification. No higherduty/profile attempt;E383failure remains.

## Entry 387 - 2026-09-14 - Bounded sector arithmetic and guard audit

Guard-start bulkstats/TIM6 setup alreadystaged; fresh ready/output/feedback/
age/budget checks cannot become cachedauthority. No new safe guardprestaging
identified. Found inlined sector%6 successor sequence invisible to helperaudit:
A727 installer2MUL/shift/sub/add. Privateconst validated successor plus sixinput
compiletimeproof now emitsCMP08003eda/branch3ede/add3ee0. Bothconstructor and
installer use it aftersectorvalidation; exhaustive256input admission test,
211RustPASS. Release-s/thinLTO/codegen1+autoauditPASS,text121900/data1104/
bss29324 unchanged,installerlocalstack28 unchanged. No hardware speedup claim.
ActualA727 archivedhashverifiedreference/clearonce_a727,lastoffE386 unchanged.
StagedNOTFLASHED07AB01E6B970E51402B687E1A84F0A27644E88A32073E124BEF603B988C9D1F1.
Read BOUNDED_SECTOR_MATH.md. No hardware thisturn; do not turn smallcleanup
into endlesscohort or speedguard expansion. Fullgoal remainsopen.

## Entry 388 - 2026-09-14 - Second recovery pass and arithmetic memo action

SameactualA727 clearonce_reentry68_30s_02 PASSoriginal30s/finaloff near316.478eHz,
53151COM/53150accepted,IRQ55.509%,sigma28.279us,raw291bus10901,stack2696,
COMP65COM69commit45,seed1071/acq6635us,arm49cost12(17spare),deadline224,
SEEDLAT85us. Startupraw694bus11545. SHA
0bfb52e9e6d7134917a1c324d6fb7c8bfe3f0d1915a974b6d38fbf7e05df775a.
Memo arrivedduringrun; completed/verified existinghandle, no restart. Third
plannedcohortattempt notstarted:2completed/2PASS,NOT3/3. COM8 untouched.

Read fullGRAYBEARD_M0_ARITHMETIC. Currentcadence4975ADC/1899accepted persecond
does not support its4divides/100us estimate. Itsphase*41>>13 fails/200 at1199.
Sustainedwriter selectedrolepath, sixstep_write belongsforcedstartup/nonrolecfg.
Implemented exactcarriercompare onlyat initialequalCCRload using sideeffecting
noinlinehelper. Existingrefusals/outputordering retained, exhaustive200duty/
carrier initial+steady cases;212RustPASS,release+autoauditPASS.
Generatedpreparedbranch0800c0b4 skipshelperc0bc; divisiononlyhelper08017282.
Candidate05B73D4C4A60A85EAE5510D265BEF36CAF50ED144AC03D57DCD22C2619CF22CF
NOTFLASHED,text121944/data1104/bss29324,includesE387. ActualA727lastoffthisentry.
NoCPUgainclaimed. ReadM0_ARITHMETIC_SWEEP.md; nextdisabled andmatched recovery
qualification then continuecadence-attributed sweep. NoPSUreplyreceivedyet.

## Entry 389 -2026-09-14 - Prepare-only carrier math hardware result

InstalledCC71BC841C327AF82EA599C8C528692B3748FD1AE40F51AADD792FF0790203E9.
StrictCARRIERMATH/--carrier-math provenance;release+autoauditPASS,320Python
beforehardware. Text122056/data1104/bss29324. Finalbranchc0ccskipshelperc0d4.
PreflashCOM41offverified,COM8untouched. Priority01threePASSnoUARTrepair,
guard01threefaults/18refusals,preflight01fivePASSCPU2/7/10. Disabledrole8us
allsteps versus12-13us previously; no outputauthority duringdiagnostics.

carriermath_reentry68_30s_01 PASSoriginal30s,operator11.7V/800mA setting,
startup62phase60trace0carrier24006. Recovered315.805177eHz53038COM/accepted,
IRQ54.723812%,sigma27.804355us,raw284bus10937,COMP65COM67commit41,stack2696.
Seed1072/acq6728us,arm49cost13(17spare),deadline191spare,SEEDLAT45/8/7/16/2/7
=85us. Startupraw558bus11522. SHA
d419273233f7b86ed7e6542b98334d1a5778fb77208fd1935a4f79d6dc599892.
Fullfixture/newmarker/timing/architecture/finaloffPASS,portclosed. N1candidate
vsA727two matches: COM69->67/commit45->41,IRQ55.209/55.509->54.724%.
No exclusivecost/WCETclaim;visitcadence differs,seedage unchanged85us.
No new speedprofile or69retry. Next liveADCphase exactmath audit/proof.

## Entry 390 -2026-09-14 - Exact bins staged; scheduling audit initiated

ActualCC71 archivedhashverifiedreference/carriermath_cc71,lastoffE389 unchanged.
Candidate9B140D993A452E2E77FA9960F87399FCD2029DD5282FF7FC74B505E242807B91
NOTFLASHED. Exactconst phasebinreciprocals5243>>20/805520>>26, compiletime
error/overflowproof; all9066validphases comparedagainstoriginaldivision.
212Rust/release-s/thinLTO/codegen1+autoauditPASS,text122000/data1104/bss29324.
Old10k alreadystrength-reduced,24kactualuidiv; no claimtwohelpersperscan.
Read ADC_BIN_MATH.md; strictcapturemarker/finalreview/hardware stillpending.

Operator scheduling/UART concern tookpriority. No hardwareblame/caps planned.
Sourcecheck: powered callback serial.read().is_ok() nonblocking; noTXwriter/
formatting in coast_run_inner loop; HALTX waits don'tmaskinterrupts. UARTbyte
duration not evidenceofblackout. DMA/guardpriority0preemptCOMP64, exactfault
preemption unproven. coretrace1 changesperreadbookkeeping andbypassesstatic
traceoffadapter; must qualifyobserver before interpreting faultreplay.
SCHEDULING_OUTLIER_AUDIT.md captures nextmaskedpath/chronologyaudit. No new
motorrun,profilechange orAM32flash.30%hardceiling unchanged,fullgoalopen.

## Entry 391 -2026-09-14 - Masked recorder division eliminated in candidate

Mapped recurringmaskedregions: FIFO pop,feedbackpublisher,acceptedguard,
acceptedrecorder,commit transaction,TIM6 andCPUwrappers. ADCconversion is
outsidefeedbackmask; UART notinpoweredTXpath. Acceptedrecorder does real
diagnosticwork insideglobalmask. ArchivedCC71actualuidiv08003a6e maps to
sixbinTimeline::push(us/bin_us), everyaccept. E383 recordermax39us is WALL,
not per-faultmaskedcost. TailI85 lacks otherISRentry/exit and hardwareedge
timestamp; trace1changescomparatoradapter. Cannotnamepreemptorfromit alone.

Replaced timelineindex divide with exactthresholdtree1b..5b, bounded by
constructorb<=100M. Same64bytelayout, bins/timestamps/overruns/refusals.
Threshold/neighbor/u32overrun and exhaustive smallwindows tests;213RustPASS.
Generatedrecorder has no uidiv, just boundedcomparisons and32bitMULby3/5.
Release+autoauditPASS,text121996/data1104/bss29324.
Candidate05BA9841616BE6A0EBE73869B0F99D00087101D0DEEF753B7A3BE7DC32207079
NOTFLASHED,includesE390ADCbin. ActualCC71,lastoffE389 unchanged,nohardware.
No measuredlatencygain oroutliercauseclaim. Nextprovenance/disabled/matched68
qualification before faultreplay; seeSCHEDULING_OUTLIER_AUDIT.md.

## Entry 392 -2026-09-14 - Exact bins qualify;6.9% hold passes same guards

Actual9789F836549D2039AAE6F44DC8F2342E2C782D4525EBDB9EB6BF6D4E5AC3AADD.
StrictBINMATH/--bin-math;release+autoauditPASS,322Python beforehardware.
Text122112/data1104/bss29324. Preflashoffverified;COM8untouched.
priority01threePASSnoUARTrepair,guard01threefaults/18refusals,
preflight01fivePASSCPU2/7/10. Same3031/252profile/priorities/guards,
operator11.7V/800mA setting,startup62phase60trace0carrier24006.

binmath_reentry68_30s_01PASSoriginal30s315.823587eHz53041COM/53040acc,
IRQ53.246705%,sigma25.968931us,raw278bus11092,COMP59COM63commit37,
recorder35DMA14vsCC71recorder41DMA18. Stack2696,seed1070/acq6646,
arm49cost12(17spare),deadline209spare,SEEDLAT85us. Startupraw521bus11522,
initialrelease32us retained. SHA
dbf372fa6d7655240d953094e38c5a942db93abe941af69c497c156e6e08efe7.

After precedingpass,one boundedbinmath_hold69_01PASS10s319.911233eHz,
19195COM/19194accepted,IRQ53.232947%,sigma41.995847us,raw264bus10901,
COMP59COM61commit37,recorder35DMA14,stack2696,initialarm79cost12.
Startupraw977bus11450 (notrunningcurrent,below1200rawguard). SHA
ce121a66e2693d4929adf6d44d8818e62ca93984ba6650bd9bf0d5277f56fe53.
Fullfixtures/newprovenance/timing/architecture/timeline/finaloffPASSboth,
portsclosed. No priority/profilechange. E383failedafter0.421s atsame duty
onolderbuild; new10spassmeaningfulbutnotuniquecauseproof orrepeatcohort.
Next69recovery BEFOREhigherduty. Fullcurrent/quality/paritygoal remainsopen.

## Entry 393 - 2026-09-14 - 6.9% recovery exposes remaining cycle outlier

Revalidated goal/current9789F836549D2039AAE6F44DC8F2342E2C782D4525EBDB9EB6BF6D4E5AC3AADD
root ELF, no competing COM41/probe process; COM8 ss untouched. UART off/p/i
confirmed gates/ENABLE/MOE/CCRs off before attempt. No flash or code/config
change. Same330profile3031/252, startup6.2%, BEMF6.9%, phase60, trace0,
24006Hz carrier, operator-established11.7V/800mA supply; no fresh PSU reading.

One original30s dropout/reentry attempt, captures/binmath_reentry69_30s_01.txt.
Fixture terminal exit1, raw evidence retained. Injection at2000036us followed
by fresh12-interval recovery acquisition6601us, seed1059ticks, re-arm47.5us
(15.5us above32usfloor), arm cost12us, seedage85us. Recovery path entered,
NOT recovery completion: resumed5005492us then CycleTiming12.9615COM/9614
accepted, mean320.143742eHz, cycle sigma26.062005us. Rawpeak256counts,
busmin11104mV, stackuntouched2696. IRQunion53.426692% is software-boundary
accounting, not total CPU utilization. Commitmax37us, recordermax35us,
DMAmax14us/queuepeak2. All maxima are wall brackets, not additive WCET.

Exact guard step4 previous5002442 -> decision5005454 =3012us,19us below3031.
Previous same-sector recorder5002471 is29us later than previous guard; don't
mix clock boundaries. Reference cycles6410/6018 half-us ticks have paired
mean3107us, not independent rotor evidence. Existing tail has no other-ISR
entry/exit timestamps, so cannot convict guard/DMA/UART or analog noise.
Arithmetic optimizations reduced measured overhead but did not eliminate
the intermittent fault; E392's single10s hold remains a pass, not repeatability.

Full completion verifier correctly rejects; cycle-fault decoder validates
exact refusal and outputs-off. Final p confirms all gates/ENABLE/MOE/CCRs0,
ARR6399 restored. Process terminal and portclosed; no retry/higherduty or
profile/priority/guard change. New retained-failure regression rejects this
as a30s recovery. Next scheduling instrumentation needs observer-cost
qualification (trace1 currently changes comparator adapter), not blind retry.
SHA B1FEB1B58D8432BD1A804E6A2E7477AE65C030FD9697B19DB0C8C20205D22888.

## Entry 394 - 2026-09-14 - Bounded scheduling recorder, not yet live

E393 yielded real fault evidence; another unchanged trial is not a cause test.
Added pure scheduling_tail.rs and host replay module:64x8byte records,
total<=544bytes. Each entry/exit carries extended elapsed time and packed
post-transition nesting stack/vector/kind. Explicit omitted history, invalid
nesting/time/count faults; nested stop freezes state and late exits cannot
rewrite it. Ring index is power-of-two mask, no runtime division orwide math.
Four newtests exercise truncation, nesting/stop, badinputs/overflow, full depth
and400001 records over10s with TIM17wraps.217RusttestsPASS; M0librarycheckPASS
(incremental-cache AccessDenied notes nonfatal). No firmware integration,
release relink, flash, UART or motor run. Root9789hashconfirmed,lastoffE393.

SCHEDULING_TAIL_DESIGN.md specifies optional integration into existing CPU
boundary serialization, extra nested clockread cost, disabled10us probe gate,
stack/CRC/epoch proof and preceding6.8% qualification. StaticComp/trace0 stay
unchanged; no per-read instrumentation. This is software-handler chronology,
not physical edge latency or foregroundmask evidence.64rows may omit a full
electrical cycle. Full65ms blackout stillaliases16bittime; no watchdogclaim.
Next integrate/verify observer, not retry6.9 blindly or changeguards/hardware.

## Entry 395 - 2026-09-14 - Optional scheduling hook builds, no flash

bench-scheduling-tail integrated into CPU meter serialization. Firstguard
starts both instruments on shared stamp; entry/exit reuse current sample,
including extra nested reads. Existing reset/frozenfinish lifecycle retained.
CPUCHECK synthetic runs exercise tail and report tailfaults, same10us gate.
Poststop metadata plus one-row-at-a-time CRC dump avoids fulltail stackcopy.
Strict decoder checks nesting/time/count/omission/frozenstop;327PythonPASS.
Release-s/thinLTO/autoauditPASS; no scheduling_tail helper calls in audit.
Candidate78846172C07BACC81E6E06F8173123B20F72A4E1486D9989EE36A01A6B7C20D4
NOTFLASHED. Installed9789 preserved/hashverifiedreference/binmath_9789.
No UART/probe/motor action; lastoffE393. Comparator normaladapter, priorities,
guards and duty unchanged. Next fixture-required observer provenance/CPU
crosscheck, emittedstack review and disabledoverhead gate before any power.
Read SCHEDULING_TAIL_DESIGN.md; no measured schedulingcause orcost yet.

## Entry 396 - 2026-09-14 - Full IRQ chronology fails overhead gate; restored

Strict fixture --scheduling-tail/CPU epoch-count-stop checks and required
CPUCHECKTAIL provenance added;328PythonPASS beforehardware. Finalcandidate
97A69B440DC62A13AB1CF956C613908343ED4BE4112F3B1D3222C63F6BB46ED2 release/
s/thinLTO+autoauditPASS,text123356/data1104/bss29860. Emitted dump_tail local
100bytes, enter/leave12, stamp4 plusregisters, not wholecallgraphstackproof.

Preflashoffverified, no COM41/probe conflict;COM8untouched. Installedcandidate
then schedtail_cpu01 UARTsilent retained. Exactprobe RCC08000000 PD1zero
BDTR00000c1a CCRs0/0/0; onlythenknownUARTclock08040000 repair. cpu02 FAILS
unchanged10us gate: modes0/1/2 max2/15/26us, sums348/3664/6656 over256,
fault0. Motor NEVER commanded. Finaloffverified, process terminal/portclosed.

Restored known9789 from hashverifiedreference/binmath_9789/shell-pwm.elf.
schedtail_restorecpu01 PASS2/7/10, noUARTrepair, finaloffverified. Root97A69
is NOT installed. Newretainedtest pins failure and baseline restoration.
Fullchronology adds too much masked work; no thresholdrelaxation. Next
compact nested-handler presence bits without extra clocks, associated with
accept/refusal, subject to sameobserverqualification. No schedulingcause yet.

## Entry 397 - 2026-09-14 - Compact stop-context probe also exceeds gate

bench-comp-overlap adds only a byte in existing CPU meter (aligned bss+4).
COMP entry resets it; guard1/DMA6 nested under COMP set bits2/64, no extra
clock reads/history/per-read changes. Frozenstop exposes bits only if valid
meter still has COMP on its stack. Foregroundstop and invalidmeter are not
evidence; absence of bits does not clear earlier rejected handler/preentry
delay. Host --comp-overlap and strict marker;218Rust/330PythonPASS.
Release-s/thinLTO+autoauditPASS,text122396/data1104/bss29328. Candidate
E283F2878ED93232CDADCBD823FA575572E47C798EEB459A6BD2FAA13B625E30 installed
only for disabledtest after offverified. overlap_cpu01 UARTworked,max2/7/11,
nested sum2632/256=10.28125us; fault0, finaloff. FAIL unchanged10us maximum,
no motorcommand. No near-pass reinterpretation.

Restored actual9789 archive. overlap_restorecpu01 silentUARTretained; exact
RCC08000000/PD1zero/BDTRc1a/CCRs0 then knownclock08040000 repair.02PASS2/7/10,
finaloff/portclosed. RootE283 NOTinstalled. Next inspect generated nested
path for redundant diagnostic work or choose a different causal experiment;
don't revisit fullhistory or relaxgate. Specificcycleoutlier remainsunknown.

## Entry 398 - 2026-09-14 - Packed overlap fails; stop micro-probe iteration

Source/codegen audit identified separate overlapbyte traffic; folded evidence
into unusedmask bits0/7 alongside existing IRQ bits1..6. Existingvalidations,
entryreset,stopfreeze and noextra-clock behavior unchanged.218RustPASS,
release-s/thinLTO/autoauditPASS,text122372/data1104/bss29324.
CandidateFA4762A54749F0468C4B81C7142912F1366E598099C16554205126526ADB75D4
installed only afteroffverified/noCOM41conflict;COM8untouched. cpu01 UARTsilent
retained; exactRCC08000000/PD1zero/BDTRc1a/CCRs0 thenknownclockrepair.
overlap_packed_cpu02 max2/7/11us,fault0, sums336/1744/2616. Nestedmean10.21875
vs old10.28125: negligible gain and still FAIL10usgate. No motorcommand.

Restored known9789archive; overlap_packed_restorecpu01 PASS2/7/10, noUARTrepair,
finaloff/portclosed. RootFA4762 is NOTinstalled. Retainedregression covers
bothoverlapfailures and bothrestorations. Stop micro-iteration on thisprobe.
Next assess controlledpriority A/B with actual pendingpriority ordering,
starvation/deadline risks and disabledqualification beforepower. No priority
change or causal finding yet; all motor guards/duty/profile unchanged.

## Entry 399 - 2026-09-14 - Isolate DMA priority; retain independent guard

Priority audit rejects assuming count-based64/ms IRQrate bounds wall latency
for a promotedCOMP/loweredguard. Narrower optionalbench-dma-peer changesONLY
DMA0->64, peersCOMP/COM64, safetyguard0 unchanged. ActualPAC0.17G071 vectors
DMA9COMP12TIM6=17COM21. DMA cannotpreemptactiveCOMP but earliernumberwins
pendingpeertie; actualdisabledvectorproof stillrequired. GuardmaypreemptDMA
copy; guarddoesnotaccessSTATE/QUEUE, originalLease100us/flags/NDTR/epoch
validation beforecurrent/FIFOpublication retained. Allage/timestamp/current/
bus/tracking/copy/commit guards unchanged. No rawambiguousscan rescue.

Implementedoptionalsetting and strict --dma-peer readbackexpectation; old
peer mode stillrequiresDMA0.332Python/release-s/thinLTO/autoauditPASS.
NOTFLASHED/noUART/motorrun; actual9789,lastoffE398 restoration unchanged.
DMA_PEER_EXPERIMENT.md specifies disabledactualADC_COMP/DMA nesting/order/
restoreprobe before normalpreflight/preceding68qualification, then69recovery
onlyifprecedingchecks pass. No prioritysafety or outliercause claim frombuild.

## Entry 400 - 2026-09-14 - DMA peer passes actual vectors and6.8% recovery

ActualA2DE7D983DF4F248BCC5C524B17FC52E24D6A8913CE5E4904E08BDD17B896846.
Release-s/thinLTO/autoauditPASS,text123648/data1104/bss29336,334PythonPASS.
Actual ADC_COMP/DMA vectors intercepted only during disabledprobeMODE;
liveproducer/pendingrefusal, originalpriority/enable/pendingrestore. Three
trials each higherDMA123/peer132/bothpending213 PASS dmapeer_priority01.
NoUARTrepair;preflashoffverified/COM8untouched. Guard01threefaults/18refusals,
preflight01fivePASS CPU2/7/10. Probeinactivechecks remainvectoroverhead.

dmapeer_hold68_01PASS10s315.365018eHz18922COM/accepted,IRQ53.497802%,
raw291bus10937,COMP55COM45commit21,DMA22queue2,stack3412,initialarm78cost12.
Startupraw1188/1200guard (notrunningamps),bus11557. SHA
d24c267acbe6e07cf0437f0c68200d77930cf609f7c89d7eee093178f6d75ba8.

Then dmapeer_reentry68_30s_01PASSoriginal30s,315.567716eHz52999COM/52998acc,
IRQ53.134900%,sigma24.416767us,raw285bus10972,COMP41COM48commit21record19,
DMA22queue2,stack2676. Seed1071/acq6673,arm49cost13(17spare),SEEDLAT85us,
deadline188spare;startupraw683bus11534. SHA
90c44a77ae717a103463470af182d62c5530d570765287e574618a88192ed5e6.
Fullfixtures/readbacks/timing/architecture/timeline/finaloffPASSboth, ports
closed. All3031/252/electricalguards retained, DMA64not0 is isolatedchange
plusinactiveprobehooks. N1timinggain, notexclusiveCPU/WCET or faultcauseproof.
Next69recovery beforehigherduty/profile; thisbuild notyettestedat69.

## Entry 401 - 2026-09-14 - DMA peer does not eliminate6.9% fault

Revalidated actual/rootA2DE, no competingCOM41/probe,off/p/i verified;COM8
untouched. One original30s dmapeer_reentry69_30s_01, same3031/252profile,
6.2%startup/6.9%BEMF,phase60trace0,24006Hz,unchangedguards/supplysetting.
Freshrecoveryseed1058/acq6574us,arm47.5us/cost12,then13.088722s recovered
drive near320.006827eHz.25131COM/25130acc,sigma24.427780us,IRQ53.125180%,
raw263bus10937,DMA22queue2/record19/commit21,stack2676. Fixtureterminalexit1:
CycleTiming12 step3 previous13085672->13088683=3011us,20below3031floor.
Previousrecorder+29us,referencecycles6473/6018ticks,pairedmean3122.75us;
1233->838 adjacentintervals, not independentrotor proof. Finaloffverified,
portclosed, no retry/higherduty/guardchange. SHA
613753798939262c190582e2e1546f000590b90792550dac79825224b3a56a0a.

DMApreemption cannot be solecause. Sourceam32_isr.rs99..108 livepersistence
outsideCS (CS109coversmask/reset/arm); guard0 canstillpreempt. Next bounded
qualification-exclusion feasibility with unchangedguardpriority and service
betweenhandlers, no broadguarddemotion. No newcriticalsection/referenceedit
implemented yet. Regression preserves failedcompletion and unknowncause.

## Entry 402 - 2026-09-14 - Stage bounded COMP exclusion, not yet qualified

Referenceaudit: no isolatedqualificationhook, and protectingreads alone leaves
preemption gap beforeacceptance. Avoidsharedcoreedit: optionalbench-comp-critical
wraps ONE unchanged referencecomp_isr on staticreal/inverted/traceoff powered
path, INCLUDING EV_ACC recorder. Exactfilter12 admission, otherwiseabort;
measuresbodyTIM17insidecritical, restoresPRIMASK, thenmax/calls/refusalupdate.
Body>60us addsabort withCOMPCRITICAL refusal. This isposthoc, not independent
watchdog; measurementexcludesprologue/epilogue.100usguardperiod/200usgap
leavesnominal40usotherdelay, notWCETproof. Guardstays0, servicebetween calls.
No UART/ADCwaits in wrappedcall; record/earlyreturn/stop paths needqualification.

Countersresetobservationepoch/poststopmarker;release-s/thinLTO/autoauditPASS.
NOTFLASHED/nohardware. ActualA2DE,lastoffE401 unchanged. Read
COMP_CRITICAL_EXPERIMENT.md: strictfixture anddisabledactualwrapper tests
for earlyreject/accept/stop/PRIMASK/duration required; CPUCHECKalone insufficient.
No sharedminz edits (existingdirtyfilespreserved), no priority/guardrelaxation.

## Entry 403 - 2026-09-14 - Critical wrapper restoration tested, no motor

Actual044FF992156A607940B1B49BFAC29D6A22DB76085C768F050F55CDDEB3659D04.
Sharedcritical_service helper used byproduction anddisabledcheck; strict
--comp-critical/marker and wrapperhostverifiers added.337PythonPASS before
hardware,release-s/thinLTO/autoauditPASS,text125128/data1104/bss29344.
Preflashoffverified/noCOM41conflict/COM8untouched; baselineA2DEarchived.

compcritical_wrapper01 UARTsilentretained; exactRCC08000000 PD1zeroBDTRc1a
CCRs0/0/0 thenknownUARTclockrepair.02PASS fourmodes16/16: earlyreturn1us,
12volatile reads7us,nestedmask6us,intentional61usdelay68us. Invalidfilters
0/13refusedwithoutclosure, PRIMASKrestored. WrapperONLY, notactualreference
acceptedrecordworkload/WCET. Lastmode intentionallyexceeds60 toverifydetect.
guard01threefaults/18refusals;preflight01fivePASSCPU2/7/10;allfinaloffverified,
portsclosed. NOmotorcommand. Next fullservice emittedscope/callgraph/stack
review before preceding68poweredqualification. Do not confuse newdiagnostic
installed044F with previouslymotorqualifiedA2DE. No core/reference edits.

## Entry 404 - 2026-09-14 - Protected COMP service does not eliminate fault

Actual044F unchanged. Emitted PRIMASK save/restore and bounded reference
12-read/accept/recorder scope reviewed before power; no UART or ADC waits.
Detailed addresses, scope caveats and capture hashes in COMP_CRITICAL_EXPERIMENT.md.
compcritical_hold68_01 PASS10s315.798eHz,18948COM/18947accepted,raw293bus10877,
bodymax32us/refused0,stack3328. compcritical_reentry68_30s_01 PASS original30s,
27.990539s resumed315.862790eHz,53047COM/53047accepted,sigma28.461us,
IRQ59.034%,raw254bus10996,bodymax33/refused0,stack2676,arm48us/cost12.
Strict recovery/provenance/timeline/off verification passed. Higher IRQ cost
than preceding DMApeer53.135%; no causal or reliability claim from one pass.

Then ONE compcritical_reentry69_30s_01 FAILED before dropout/recovery:
CycleTiming12 stop159790us,step2 previous156754->159783 delta3029<3031us.
293COM/292accepted,raw233bus11343,critical3048calls/max48/refused0,
stack2676. Referencecycles6386/6053 half-us ticks,pairedmean3109.75us;
previous recorder25us after guard. Not independent rotor overspeed proof.
Fixture terminalexit1, outputs off verified, no higher duty/retry/guard change.
Protection does NOT eliminate fault. Sole preemption explanation unsupported;
no hardware fault established. Next offline startup/acceptance timing comparison.

## Entry 405 - 2026-09-14 - Compare reference and guard cycle clocks offline

No hardware/firmware change; actual044F, finaloffE404. Source confirms reference
count follows persistence and precedes timer reset/COMarm/EV_ACC; guard samples
inside EV_ACC before recorder. Four retained6.9 failures have guard minus six
reference intervals3/3/2/2.5us, respectively range330/binmath/dmapeer/critical.
Short cycles are already present before callback; callback-only delay does
not explain them. Neither measurement is independent rotor/edge timing.
Do not histogram only threshold-selected faults as proof of PWM quantization.
Added clock_closure report fields and regression for all four captures;
see CYCLE_CLOCK_COMPARISON.md for scope and next evidence requirements.

## Entry 406 - 2026-09-14 - Audit comparator capture without changing hardware

ExistingCOMPPATH aggregates decisions but lacks per-event history; historical
E299 mostlyopen-no-accept cannot be assumedcurrent. I85trace1 changes read
bookkeeping and bypasses static/protected path, so not a matched044F observer.
Added offline rejection cadence using full-width consecutive same-sector
reject visits only. Both passing/failing irqtail54/55 andcachedcomp54/55
retained traces contain100us gaps; presence alone does not discriminatefault.
Tests coversequencewrap,missingvisits,sectorchanges,acceptedvisits andretained
captures. COMPARATOR_CAPTURE_AUDIT.md documents next boundeddecision-level
option; no instrumentation installed, no motor/firmware change,actual044F/offE404.

## Entry 407 - 2026-09-14 - Implement pure bounded decision-history storage

Added comp_decision_tail32x16byte ring with existingdecisionclassification,
explicitomissions,invalidmetadata freeze,stopped-call retention thenfreeze,
ordinaloverflow refusal and constant-mask indexing. Five newRust tests cover
wrap/stop/metadata/reset/boundaries/memory. Observer-replay suite passed;
WindowsincrementalAccessDenied notes nonfatal. No shell integration, release
build, flash or motor attempt. Actual044F/offE404 unchanged. See
COMP_DECISION_TAIL_DESIGN.md for remaining liveintegration and timing gates.

## Entry 408 - 2026-09-14 - Opt-in decision tail integration builds

bench-comp-decisions uses existing firstcount instrumentation andstaticpath.
Scope captures entrytime/step, writes afterreference; Drop closes earlyreturns
unknown. Observationreset clears ring; stopped call or poststopdump freezes.
CD85 CRC framing emittedcapture-only, strictdecoder/fixtureflag added.
Release s/LTO build succeeded. NOTFLASHED/nohardware, actual044F/offE404.
Next emittedcode/memory audit anddisabledlive-scope timing beforepower.

## Entry 409 - 2026-09-14 - Decision-tail resource audit and quiet framing

Unflashedcandidate1125C2E3F696E370D7817CB71FC4F6A77F94627B1C1BB454903A9E07C1EE9215,
release text126860/data1128/bss29876,flashspare3084/RAMafterstatics5860.
TargetTAIL532bytes. PrecedingCF23 completion code44local stack/no division
helpers, duplicateclassification; notWCET or measuredstackmargin.
Explicitcaptureheader fixes quietreport vs truncatedcapture ambiguity;
requiredfixture refusesquiet, orphanrowsrefused.343PythonPASS/releasePASS.
No flash/motor;actual044F/offE404. Nextdisabled actualScope lifecycle/timing.

## Entry 410 - 2026-09-14 - Implement disabled actual-Scope diagnostic

decisioncheck five modes x16 uses actual begin/count/finish/Drop, validates
firstcount retention/state/row/freeze; disabled-owner-active admission, no gate
authority or referencecore invocation. Host fixture exclusivecapture/finaloff
and strict lifecycle verification; measuredmaxima notautomatic timingpass.
344PythonPASS,releasecandidateF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818
text127732/data1128/bss29876. NOTFLASHED/NOTRUN, actual044F/offE404 unchanged.
Next archive044F then disabled hardwarequalification; no motor yet.

## Entry 411 - 2026-09-14 - Decision recorder passes disabled board checks

InstalledF7302F50D950378593765275CF88514923183C827B4E5C5AD9D8011067020818
after verifiedoff/noCOM41conflict; preserved044F archive/hashverified.
UARTreset worked withoutrepair. decision_scope01 allfive modes16/16PASS,
max9/9/9/8/8us; actualscope only, notfullISR/referenceWCET.
decision_guard01 3faults/18poststop refusals;decision_preflight01 fivePASS,
CPU2/7/10. Fixtures terminalexit0/finaloff verified/portsclosed. COM8untouched.
No motor. Next preceding6.8 bounded diagnostic with strictcaptureprovenance,
occupancy/criticalmax/stack/guards review before higher duty.

## Entry 412 - 2026-09-14 - Startup current refusal before recorder exercise

ActualF730, noCOM41conflict. One decision_hold68_01 requested10s6.2startup/
6.8BEMF phase60trace0 with strictdecision/path/build flags. Fixtureexit1:
Current5 stop11237us beforehandoff.70CRC ADCrows,peak1256rawcounts vs1200guard;
last11063us logicalC3304,bus11605mV. Not calibratedamps orPSUaveragecurrent.
Decisiontotal0/criticalcalls0: no combinedrecorderoverhead evidence. Finaloff
verified,portclosed,stackuntouched2956. No retry/higherduty/guardchange.
RetainedSHA894653C0410E9BBED58FD5550A0BEE7A0D43660C77577D5A16771657C6B89A19.
Next startup sample-timing/entry review; not recorderfailure/hardwarecause proof.

## Entry 413 - 2026-09-14 - Startup61 reaches powered decision capture

E412 five-channel scan162us lacks exact C aperture/PWMphase; cannot call spike
artifact. Controlled startup62->61 only, BEMF68/allguards/F730 unchanged.
decision_start61_hold68_01 PASS10s316.098372eHz,18966COM/18965accepted,
IRQ70.397%,raw240bus10925,stack2748,criticalmax32/refused0,DMA22queue3.
Startupraw546bus11450/arm58cost12; n1 notcausalstartupreliability proof.
Decision165204total:29792closed/116447open-no-accept/18965accepted,
32CRCrows/165172omitted. Hostchecksaggregateepoch. Current postgate visits
dominate; diagnosticcostmaterial vs~59%priorIRQ, notproductiontelemetry.
Finaloffverified/fixtureexit0/portclosed. Nextpreceding6.8recovery with61startup.
SHA1F2A82EE7E62B98C8CDDB229966EABCBBD6D10A8BFCD9E6A2C1E16CCEF4F28ED.

## Entry 414 - 2026-09-14 - Recovery fault without long qualification stall

SameF730/start61/BEMF68,decision_start61_reentry68_30s_01 original30s.
Recoveryseed1058/acq6594us/arm37.5cost12 then2.666888s resumed316.347eHz.
CycleTiming12 step2 delta3015<3031;5062COM/5061accepted,sigma37.287us,
IRQ70.405%,raw319bus11056,stack2120,criticalmax48/refused0,DMA24queue3.
Finaloffverified/fixtureexit1/portclosed. Not completed recovery.
Decisiontail stoppedcount828 vsreference841=6.5us; precedingacceptcount1218
vsreference1231=6.5us. No long gate-to-accept stall at this refused event;
pre-entry/physicaledge unknown. No cross-epoch timestamp subtraction.
Park furtherpowered diagnostic retries; next retire expensive instrumentation
and critical experiment to archivedbaseline with disabled requalification.
SHA A7163735522BB73C383FD2E8E022987BD4FCD9CA9B88D4C953C749F7707BF24F.

## Entry 415 - 2026-09-14 - Retire diagnostic and restore baseline recovery

Preflashoffverified/noCOM41conflict/COM8untouched. F730archivehashverified;
installed archivedA2DE DMApeer, no decision/criticalfeatures. RootELF stillF730.
retire_decision_guard01 UARTsilent retained; exactRCC08000000/PD1zero/
BDTRc1a/CCRs0 then08040000clockrepair. guard02PASS3faults/18refusals;
retire_decision_preflight01 allfivePASSCPU2/7/10/finaloff.
baseline_start61_reentry68_30s_01 PASS original30s,27.990602s resumed315.7165eHz,
53023COM/53022accepted,IRQ53.529949%,sigma24.118757us,raw260bus10901,
stack2676,seed1072/acq6708/arm49cost12,DMA22queue2. Finaloffverified/exit0.
Lower-overhead6.8 operating point restored, notnew6.9qualification.
SHA E4C728095D9DE28B892D6673E8F0C0CDF29692A7B6046B6872EBE9CCD6329476.

## Entry 416 - 2026-09-14 - Static reference bundles yield no COMP code gain

Hypothesis: avoid reconstructing Sched/Drive pointerbundles on everyCOMPcall.
Made local pure constructors constfn, built optional statics referencing live
atomics (no values cached). Candidate517BEB725EA1049091A74C66C7FA25E275D29A4E0ED0A47756F107BAF2D01129
release-s/LTO build succeeds. Size123648/data1104/bss29336 matchesA2DE.
ADC_COMP080016a4 length0x850 exactdemangleddisassembly identical; reference
comp_isr08004438 length0x1d0 unchanged. WholeELF notidentical; no gainclaim.
Removed experimentalstaticbundlefeature; retainedpure constconstructors and
localreferencebindings. No hardware/flash/motor;actualA2DE/offE415 unchanged.
Next actual repeateddispatch/control work, not anotherpointerbundle experiment.

## Entry 417 - 2026-09-14 - Stage report-only per-dispatch timestamp omission

Sourceaudit LAST_IRQ_US soleconsumer poststopCOAST_END_STATE; everypowered
COMPdispatch samples maskedOBS_CLOCK for it even trace0. Optionalquietfeature
skips onlythisreportupdate, explicitIRQSTAMP unavailablemarker/resetzero incl
reentry. Accepted/stopclock andindependentsafetyclock/feed remainunchanged.
Candidate137F0E1018F3E03B89920C79697A87335A369BE944C79FC943A183061B0CE940
release123836/data1104/bss29336. Strictfixtureflag added. NOTFLASHED/nohardware;
actualA2DE/offE415. Next emittedbranch/clockfeed audit anddisabledqualification.

## Entry 418 - 2026-09-14 - Quiet timestamp candidate fails UART qualification

Verifiedcompiledbranch080019d8 bypassesmaskedreportupdate080019de..1a04 on
poweredtrace0. Preflashoff/noCOM41conflict, flashed137F. guard01UARTsilent;
exactsafeRCC08000000/PD1zero/BDTRc1a/CCRs0 then08040000repair. guard02 and
preflight01_pulse stillsilent, subsequentRCC08040000/safeoutputs. No motor.
RestoredA2DE archive. restore_guard01silent; exactsafechecks/clockrepair then
restore_guard02PASS3faults/18poststoprefusals/finaloff. Portsclosed.
ActualA2DE/root137Fnotinstalled; allquietstamp capturesretained. Causeunknown,
candidateunqualified, nottimingregression proof. Diagnoseboot/UART before retry.

## Entry 419 - 2026-09-14 - Capture working UART register baseline

Source/emittedcandidate boot setsUSART3 clock beforeBRR/controls; no culprit
clearingwrite identified. Added read-only SWD snapshot tool excludingRDR/ICR.
uart_baseline_snapshot01.jsonl APBENR1=08040010 CCIPR0 GPIOCmoderf3afffff
AFRH0 USARTCR1=0d CR2=0 CR3=0 BRR22b ISR006000d0. PD1zero BDTRc1a CCRs0.
TIM6bit4 fromprecedingguardtests, notresetexpectation. ActualA2DE unchanged,
no flash/motor. Next capture failedcandidate USART/pin state before repair;
UART_BOOT_DIAGNOSIS.md documents known/missing evidence.

## Entry 420 - 2026-09-14 - Candidate boot succeeds, preceding hold passes

Actual137F afterverifiedoff. quietstamp_boot_guard01PASSwithoutrepair;
candidateUARTsnapshot matchesbaselineafterguard. PriorUARTfailure remains
unexplained/intermittent. quietstamp_preflight02fivePASSCPU2/7/10.
quietstamp_start61_hold68_01 PASS10s315.435eHz18926COM/18926accepted,
IRQ52.826%,sigma39.193us,raw274bus10901,stack3468,COMP40COM44commit21
DMA22queue2. IRQSTAMPomitted1/strictfixturePASS/finaloff/portclosed.
Notconvincing gain vsbaseline~53.5 fromdifferentwindow. Nextsame61/68recovery.
SHA9C3EB731366CA39089BD1AEC087E7F06DA78C43EED4754FB04AEBE4817049510.

## Entry 421 - 2026-09-14 - Quiet stamp recovery:6.8 pass,6.9 late refusal

Actual137F/start61,quietstamp_start61_reentry68_30s_01 PASS27.990639s resumed
315.395eHz52969COM/52968accepted,IRQ52.738%,sigma23.731us,raw277bus10925,
stack2676,seed1073/acq6749/arm49cost12,DMA22queue2. Finaloff/exit0.
Then onequietstamp_start61_reentry69_30s_01 FAILED27.133800s resumed319.662eHz,
52042COM/52041accepted,sigma24.255us,raw286bus10972,stack2676. CycleTiming12
step1delta3017<3031. Reference6408/6029ticks,residual2.5us,notrotorproof.
Finaloff/exit1/portsclosed; nohigherduty. Timestampomission notfaultfix;
nearlycompleted isnotpass. Hashes/limits recorded QUIET_IRQ_STAMP_EXPERIMENT.md.

## Entry 422 - 2026-09-14 - Review test envelope independently of fault cause

3031us ischosen330profileboundary, notprovensiliconlimit. E4213017us corresponds
to331.46eHz accepted-cycle timing, notindependentrotorspeed. Prospective335
actualreferencewait249ticks=124.5us;seedage85/87 leaves7.5/5.5usmargin above
32floor,93usfails. AddedRusttestPASS alsoassertscurrentadmission rejects995.
No runtimeprofile/guardchange/hardwareaction;actual137F/offE421. Review
ENVELOPE_BOUNDARY_REVIEW.md: next conservativepost-edge budget before any
coordinatedprofileexpansion, notthresholdrelaxation labeledasjitterfix.

## Entry 423 - 2026-09-14 - Attribute the recovery post-edge budget

Read current acquisition, cleanup, reference setup and guard admission source.
Three retained SEEDLAT records (baseline68,quietstamp68,quietstamp69) show
edge-to-entry44us, entry/reset9us, reset/feedback7us, feedback/guard15..16us,
guard/reference2us. These are grouped observations, not WCET or isolated costs.
Bulk reset and bounded seed mean are already optimized; do not rediscover them.
RECOVERY_POST_EDGE_AUDIT.md specifies next candidate: prepare carrier earlier
with recovery-only successful cleanup preserving ARR while clearing all output
and role authority. Generic/failure reset must stay unchanged and live period
checks precede handoff. No code/profile/firmware change or powered attempt;
actual137F/offE421 unchanged. Existing6.9 failure remains retained/unqualified.

## Entry 424 - 2026-09-14 - Stage early recovery carrier candidate

Implemented opt-in bench-reentry-carrier: prepare ARR before acquisition scan,
successful recovery cleanup preserves ARR only, clearing all output/role state.
Generic and failure shutdown still restore startup period. Handoff checks live
ARR/CCRs/owner/DMA, retaining all seed-age/deadline/guard rules. Added ENABLE-low
rolecheck 5-case register diagnostic and strict --reentry-carrier provenance.
Candidate1717BA38D78466DF61A56D99F92A5428BA81239275A681EF5335A91F50751865
release124808/1104/29336, matching advisory mathaudit;353Python/223Rust testsPASS.
No flash or motor run; installed137F/offE421 unchanged. Hardware cleanup check
NOTYETRUN, latency benefit unproven. Next disabled preflights before matched68
recovery. Full details and limitations in RECOVERY_POST_EDGE_AUDIT.md.

## Entry 425 - 2026-09-14 - Early carrier passes recovery, no measured gain

Actual1717 flashed after preflashoff. guard01UARTsilent; read-only boot snapshot
captured disabledUSARTclock08000000 but CR10d/BRR22b,outputs off. Documented
08040000clockrepair restored UART. guard02PASS3faults18refusals;preflight01
fivePASS inclnewENABLE-lowcleanup5/5,CPU2/7/10. Failure capture retained.
earlycarrier_start61_reentry68_30s_01 PASS original30s/injectedloss/recovery,
27.991123s resumed315.704eHz53022COM/53021accepted,sigma23.513us,
IRQ52.426%,raw274bus10889,stack2676,DMA22queue2. Finaloff/portclosed.
SEEDLAT88/106/120/152/156 identical priorquietstamp68; no measured gain.
No higherprofile or6.9 attempt. Candidate1717 remains installed/stopped;
next retire no-gain feature from campaign, preserve evidence. Hash and details
RECOVERY_POST_EDGE_AUDIT.md. Rawcurrent notamps, singlepass notcohort.

## Entry 426 - 2026-09-14 - Declare guarded335 envelope experiment

Archived1717 ELF/audit; rebuilt without no-gain earlycarrier feature. Added
opt-in coordinated335profile2986/248us,acquisition5972/496ticks,seed995.
Allcurrent/bus/tracking/deadline/32usarm/16uscostguards remain. Expansion is
not a jitterfix and does not retroactively pass old330failures. Late seeds
must refuse; no invented WCET prerequisite blocks a safely-refusing trial.
RANGE335_EXPERIMENT.md records rationale from preceding315–316eHz recovery
evidence, missing calibration/independentrotorproof, numeric plan and gates.
CandidateEC510EAC16A5DFDD20713A3FAA7AE0726E346DC3A5D78524920D2CE88A3BC460
release123836/1104/29336,matchingadvisoryaudit.354Python/224Rust335PASS;
old330lib189PASS. Fixed1000tick faulttest now derives below-profile interval;
initialtestfailure and importerror corrected, no production refusal weakened
beyond the explicitly new speed profile. No hardware;actual1717/offE425.
Next disabledpreflights,68recovery,onlyafterpass one69undernewcontract.

## Entry 427 - 2026-09-14 -335 profile hardware:6.8 and6.9 recovery pass

InstalledEC51 after verifiedoff. UARTguard01failed,snapshotbeforeclockrepair
confirmed knownclockdisabled/exactoutputs-safe state. guard02PASS3/18 after
08040000 repair;fivepreflightsPASSCPU2/7/10. Bootcause remains unresolved.
range335_start61_reentry68_30s_01 PASS27.991039s315.703eHz,53021COM=acc,
raw272bus11032,armspare17.5us. Then one69_30s_01 PASS27.990912s319.832eHz,
53715COM/53714acc,sigma24.251us,IRQ52.866%,raw288bus10829,armspare15.5,
cost12,stack2676,DMA22queue2. Both original30s deadlines/injectedloss/recovery/
archives/finaloff verified. All sessions closed,actualEC51 stopped.
Newprofile2986/248,acq5972/496confirmed. Notjitterfix;oldfailures unchanged.
First69pass, next fixedtwo same69attempts fail-fast forcohort before moreduty.
RANGE335_EXPERIMENT.md has hashes,startup peaks and measurement limitations.

## Entry 428 - 2026-09-14 -6.9 recovery3/3;7.0 safely refused

EC51 unchanged.69campaign02+03PASS original30s/one recovery/finaloff,
319.822/319.866eHz,53712/53720COM=accepted,sigma24.190/23.905us,
raw269/291,bus10889/10913,armspare14.5/15.5,stack2676. Declaredcohort3/3.
Frozenreference/range335_ec51 ELFhashverified. NextONE70hold10s FAILED
3.702165s at325.193eHz,CycleTimingstep2delta2964<2986,7223COM/7222accepted,
raw274bus10925,stack2676,DMA22queue2. Refcycles6332/5921ticks,residual3.5us.
Finaloff/portclosed;7.0unqualified,noretry.2retained-evidencetestsPASS.
No CPU/PSU/hardwarecause proven. RANGE335_EXPERIMENT.md hashes/details;
next reviewnextboundary/fresharm beforeanyprofile expansion. ActualEC51/off.

## Entry 429 - 2026-09-14 - Stage340 profile after6.9 cohort

Opt-in340profile2942/245us,acq5884/490ticks,980seed. Expansion underoperator
goal after3/3 preceding69recovery; NOTjitterfix orretroactivepass. Actualwait245
ticks,181age leaves32us,182refuses. All electrical/tracking/deadline/arm guards
unchanged. New boundary/fullsequence/wrap and parser-consistency tests pass.
Candidate14FEB2195EAD922D1451B630962FFF95254AE858547BF2DA4F1CA0114642A0FF
release123832/1104/29336,matchingaudit;357Python+225Rust340PASS,335lib190PASS.
RANGE340_EXPERIMENT.md explicit next disabledpreflights,69recovery,conditional
70hold10s/recovery,fail-fast. No hardware;actualEC51/offE428 unchanged.

## Entry 430 - 2026-09-14 -340 hardware: first7.0 recovery succeeds

Installed14FE afteroff. guard01UARTsilent,snapshotexactsafe then documented
clockrepair08040000;guard02PASS3/18,fivepreflightsPASSCPU2/7/10. Bootdefect
unresolved.69recovery30sPASS319.906eHz,then70hold10sPASS325.762eHz,
then70recovery30sPASS27.991037s resumed325.793eHz54716COM/54715accepted,
sigma23.855us,IRQ53.129%,raw289bus10877,seed1034age85,armspare12.5/cost13,
stack2676,DMA22queue2. Alloriginaldeadlines/archives/finaloffverified,portsclosed.
One70recoverypass,NOTrepeatability/jitterfix. RetainedthreecapturetestPASS.
Next fixedtwo70recoveryruns failfast beforemore duty. Actual14FE/off.
RANGE340_EXPERIMENT.md has hashes,allwindows,startuprawpeaks and limitations.

## Entry 431 - 2026-09-14 -7.0 recovery3/3;7.1 acceleration refusal

14FEunchanged.70recovery02+03PASS original30s/onefreshrecovery/finaloff,
325.767/325.650eHz,54710/54692COM=accepted,sigma24.003/23.921us,
IRQ52.965/52.975%,raw266/299bus10853,armspare12.5/12,stack2676.
Declaredcohort3/3,archivedreference/range340_14fe withverifiedhash.
ThenONE71hold10s FAILED170375us,step4CycleTiming2930<2942,322COM/321acc,
raw242bus11319,stack2676. Ref6136/5868ticks,residual-4us. Mean315.355 includes
acceleration;NOTsteadyoperatingpoint. Finaloff/closed,noretry.7.1unqualified.
2retaineddatatestsPASS;RANGE340_EXPERIMENT.md hashes/details. Next review
acceleration/nextboundary/fresharm,not hardwareblame orautomaticguardwidening.

## Entry 432 - 2026-09-14 - Local tail separates startup average from boundary

Added drv_local_cycles.py: validatedprefix/tailstats,no bridging omitted data,
overlappingcycles explicitlynotindependent,refusededgeexcluded.2testsPASS.
Failed71tail154..170ms median3016us=331.565eHz vsaggregate315.355;prefix3867us.
Passing70holdtail3064.5us and70recoverytail3073us. Refusal2930guard/2934reference
is~2.85%belowlocalmedian,notproof ofphysicaloverspeed/ISRdelay/hardwarefault.
STARTUP_BOUNDARY_COMPARISON.md next345review:121uswait at966seed;85/87usages
leave4/2usspare,90refuses. No profile/hardwarechange,actual14FE/off. No fictitious
steady315eHzfailure or interpolation across257missingevents.

## Entry 433 - 2026-09-14 - Stage345 coordinated profile

After preceding7.0cohort/local71tailreview, staged explicit345profile2899/241us,
acq5798/482ticks,966seed. Referencewait121us;89us agepasses32floor,89.5refuses.
All electrical/tracking/deadline/16usarmcostguards unchanged. NOTjitterfix or
retroactivepass. CandidateFAA0FE858FE21A8456A647FEF36C3818A0A12D7A15B14F94FC0516033FF017B4
release123836/1104/29336,auditmatch;362Python/226Rust345PASS,340lib191PASS.
RANGE345_EXPERIMENT.md declares next disabledpreflights,70recovery,conditional
71hold10s/recovery30s,failfast. No hardware;actual14FE/off unchanged.

## Entry 434 - 2026-09-14 -345 hold succeeds;7.1 recovery window refuses

FAA0installed. UARTguard01silent,safesnapshot+documentedclockrepair;guard02
PASS3/18,fivepreflightsPASSCPU2/7/10.70recoveryPASS325.585eHz,71hold10sPASS
331.995eHz. Next71recoveryFAILED8.245294s resumed332.572eHz,16453COM/16452acc,
sigma23.383us,raw300bus11032,stack2676,DMA24queue2. Reacquisition/arm succeeded:
seed1013age85,armspare9.5cost13. CycleTimingstep2delta2881<2899,ref6220/5758ticks,
residual2us. Allfinaloff/closed,noretry. Failure is sustained,not expiredseed.
RANGE345_EXPERIMENT.md hashes/details. Highest3/3remains14FE/340/70.

## Entry 435 - 2026-09-14 - Stage350 with explicit narrow live-arm margin

Staged coordinated3502858/238us,acq5716/476ticks,952seed. Referencewait119us;
87usagepasses32floor,87.5refuses. Observed85leaves2usnominalspare,NOTWCET.
All electrical/tracking/deadline/16uscostguards unchanged. Explicitexpansion,
NOTjitterfix orretroactivepass. Candidate7F4B25F6022C9A15DF004F59FD8CAC6394C9A3E3B208A5925A1ED099D903F9D9
release123836/1104/29336,auditmatch;364Python/227Rust350PASS,345lib192PASS.
RANGE350_EXPERIMENT.md next disabledpreflights,70recovery,conditional71recovery,
failfast. No hardware;actualFAA0/offE434 unchanged. Calibration/parity stillopen.

## Entry 436 - 2026-09-14 -350 first7.1 recovery passes

Installed7F4B afteroff;guard01PASS3/18withoutUARTrepair,fivepreflightsPASSCPU2/7/10.
70recoveryPASS325.722eHz;71recovery01PASS27.991537s resumed332.201eHz,
55793COM/55792acc,sigma22.968us,IRQ52.961%,raw312bus10901,seed1007age85,
remaining41us/spare9/cost13,stack2676,DMA24queue2. Bothoriginal30sdeadline/
archives/finaloffverified,portsclosed. One71pass,NOTcohort/jitterfix.
Next fixedtwo71recoveries failfast beforemoreduty. RANGE350_EXPERIMENT.md
hashes/currentlimitations. Actual7F4B/off;retainedtwo-capturetestPASS.

## Entry437 -2026-09-14 -7.1% fixed recovery cohort3/3

Same7F4B, no limit/firmware changes. Declared02/03 bothoriginal30sPASS with
startup/injectedtrackingloss/recovery and finaloff. Resumed~27.991s each,
332.233/332.197eHz,55796/55792COM vs55796/55791accepted. sigma22.893/22.946us,
IRQunion53.155/52.933%,raw315/308,bus10889/10901,stack2676,armspare11/9us,
armcost13. DMAmax24/37us,COM54/45,COMP40; maxima notadditive/WCET/cause.
Zero excluded attempts. Cohort with01 now3/3; frozenhashverified7F4B ELF.
RANGE350_EXPERIMENT.md contains hashes/details/limitations. No higherduty
attempt yet; no jitterfix/calibratedcurrent/independentrotorclaim. Actualoff.

## Entry438 -2026-09-14 -same-profile7.2% hold and recovery pass

Same7F4B/350, no guardchange.72hold10sPASS336.229eHz wholewindow including
acceleration,20174COM/20173accepted,raw306bus11068. Then72recovery30sPASS:
27.991286s resumed336.451eHz,56506COM=accepted,sigma23.754us,IRQ53.195%,
raw310bus10805,seed996age85/remaining39.5/spare7.5/cost13,stack2676.
Startupraw1023/902 below1200 but notcalibratedamperes. Bothfinaloff/closed,
zeroexcludedattempts. RANGE350_EXPERIMENT.md hashes/details. Nextfixedtwo
72recoveries failfast beforehigherduty; onepass notcohort. Actual7F4B/off.

## Entry439 -2026-09-14 -7.2% fixed recovery cohort3/3

Same7F4B/350, declared02/03 bothoriginal30sPASS, startup/loss/recovery/finaloff
verified. Resumed336.539/336.403eHz,56522/56497COM vs56521/56497accepted,
sigma23.771/23.769us,IRQ53.022/53.039%,raw309/326,bus10925/10889,stack2676,
armspare9/8us,cost13. Cohort01..03 now3/3, minimumspare7.5us, noexcludedtries.
No guardchange or firmware write. RANGE350_EXPERIMENT.md hashes/details;
restorearchiveREADME updated. Higherduty untested; calibration/parity open.

## Entry440 -2026-09-14 -7.3% same-profile cycle refusal

Single7F4B/350 hold73 failed197217us, CycleTimingstep3 delta2844<2858,
387COM/386accepted,raw268bus11199,stack2676,finaloff/closed. No retry/recovery.
Tailmedian2917us/~342.82eHz, aggregate327includesacceleration, notsteadyproof.
Reference5993/5681ticks,closure3.5us, pairedmean2918.5us. No ISR/hardware/
physicaloverspeedcause established. RANGE350_EXPERIMENT.md hashes/details.
Highest3/3 remains72; review nextprofile/latearm before expansion. Actualoff.

## Entry441 -2026-09-14 -prospective recovery boundary audited

Observed85usedgeage on7F4B, reusedreference arithmetic:355floorseed938wait
117.5us leaves32.5us;356exact32;357/360belowfloor. Oneextrausinvalidates355.
Offline regression preserves350 admission,194lib+34otherreplayPASS. No profile
or runtimechange, embeddedbuild/flash/motor. SPEED_ARM_BUDGET.md distinguishes
actual72seed margin from hypotheticalboundary, not a sustainedspeedceiling.
Next audit remainingpostedge path before broaderspeedexpansion, not repeat
nogaincarrier relocation. Actual7F4B/off;73failure/72cohort unchanged.

## Entry442 -2026-09-14 -timeline soft divide located in arm bracket

Hashmatched7F4B disassembly shows runtime/6 at08006160 in timelineconstructor
inside maskedarm after age sample/timer enable. Constfn not compiletime here.
This costs16usarmbudget, not preceding85usage; moving it earlier can merely
trade budgets. RECOVERY_TIMELINE_AUDIT.md gives exactmetadata/serialization
requirements and excludes unrelated scan/tracking helpercalls. No runtime
change/flash/motor. Next bounded exactconstructor optimization and codegen
comparison; no speedguardchange or measuredgain claimed. Actual7F4B/off.

## Entry443 -2026-09-14 -division-free timeline candidate built

Exact32bit radix4096 division constructor, boundedproof/digitexhaustion and
all12bit boundaries tested.195lib+34otherRust/367PythonPASS. Release7C3724E8
text123892/data1104/bss29336,auditmatched. Constructor no dividehelper,
coastcallcount4->2, but outlined52Bframe: no latencygain yet. Fullhash/proof
RECOVERY_TIMELINE_AUDIT.md. No flash/motor;actual7F4B/off. Next disabled
preflights thenmatched72recovery, compareedgeage AND armcost;guardsunchanged.

## Entry444 -2026-09-14 -timeline candidate passes matched recovery

7C37 installed. UARTbootfault retainedsnapshot safelyoff beforeclockrepair;
guard02PASS3/18,fivepreflightPASSCPU2/7/10.72recovery30sPASS336.395eHz,
56496COM=accepted,sigma23.885us,IRQ53.531%,raw303bus10877,stack2644.
Age86us/arm8us vsreference85/13: combinedmeasured98->94us,onecomparison,
notWCET/reliability. Seed1013remaining40.5/spare8.5us,DMA24queue2.
Finaloff/closed;RECOVERY_TIMELINE_AUDIT.md hashes/details. Nexttwo72repeats
failfast, no73retry/guardchange. Actual7C37/off, UARTbootbugnotfixed.

## Entry445 -2026-09-14 -candidate second72 attempt fails; cohort stopped

7C37 repeat02FAILED24.287464s recovered336.414eHz,CycleTimingstep3 2854<2858,
49024COM/49023accepted,sigma23.699us,raw300bus10948,stack2644,DMA24queue2.
Seed1000age86/remaining39/arm8: recoveredarm succeeded; notarmcostfailure.
Reference5993/5702ticks,closure3us. Finaloff/closed,thirdcancelled,1PASS/1FAIL.
RECOVERY_TIMELINE_AUDIT.md hash/details. Next frozen7F4B matchedreference,
notthreshold widening or patch/hardware attribution. Actual7C37/off.

## Entry446 -2026-09-14 -frozen reference passes after candidate refusal

Actual7F4B restored fromfrozenELF afterverifiedoff. GuardPASSwithoutclockrepair,
fivepreflightPASSCPU2/7/10.72reference30sPASS336.307eHz,56481COM=accepted,
sigma23.907us,IRQ53.172%,raw307bus10937,stack2676,seed1012age85/arm13/spare9.5.
Finaloff/closed. RECOVERY_TIMELINE_AUDIT.md hash/details. Candidate86/8 timing
diff supported, rarefailure cause notproved by one referencepass. Candidate
source/rootELF7C37 stillpresent; installed7F4B/off. No further duty/guardchange.

## Entry447 -2026-09-14 -frozen interrupt body comparison

FourIRQ symbols sameaddresses/sizes; objdumpdiffs onlycallrelocations plus
TIM16pointer to relocated matching am32.rs source-locationmetadata. commit
same312bytes, onlyapply-call displacement afteraddressnormalization.
Noaddedhandlerbodywork established; NOTwholecallgraph/state/timingequivalence.
RECOVERY_TIMELINE_AUDIT.md scope/evidence. No runtimeedit/motor. Actual7F4B/off,
candidate7C37 retained; failurecause unresolved, no guardchange warranted.

## Entry448 -2026-09-14 -inline timeline variant built

inline(always) onlyruntimechange vs7C37, sameexactmath/metadata/guards.
ACEEcandidate release124044/1104/29336,auditmatch, no constructor symbol,
timeline /6 stillabsent.195lib+34otherRustPASS. RECOVERY_TIMELINE_AUDIT.md
fullhash/codegencaveats. No motor/flash,actual7F4B/off. Next disabledchecks
thenmatched72recovery, compareage/armcost/stack; notsustainedfixclaim.

## Entry449 -2026-09-14 -inline timeline first72recovery passes

ACEE installed,guardPASSwithoutrepair,fivepreflightPASSCPU2/7/10.
72original30s startup/loss/recoveryPASS336.409eHz,56498COM/56497accepted,
sigma23.718us,IRQ53.015%,raw311bus10853,stack2676,DMA24queue2.
Seed1000age85/remaining40/spare8/arm7: measured6usarmcost reduction vsref,
noedgeagegain. Allfinaloff/closed. RECOVERY_TIMELINE_AUDIT.md hash/details.
Onepass,notcohort/jitterfix. Next twofixed72inline repeats failfast. Actualoff.

## Entry450 -2026-09-14 -inline cohort2PASS/1FAIL

02PASS336.361eHz;03FAILED24.757799s recovered336.506eHz, CycleTimingstep3
2842<2858,49987COM/49986acc,raw290bus10972,stack2676. Age85/arm7 inallthree;
constructorcall removal didnot eliminate sustainedfailureclass. Reference
cycles6133/5678ticks,closure3us, notcauseproof. Bothfinaloff/closed, no retries.
RECOVERY_TIMELINE_AUDIT.md hashes/details. ActualACEE/off; next acceptedcycle
redistribution investigation, notmoreconstructorvariants/thresholdchanges.

## Entry451 -2026-09-14 -selected fault pairs quantified

drv_fault_pair.py+2testsPASS. Reference/outlined/inline pairmean-minus-local
median+1.5/-37.25/-8.75us; notuniversalexactcancellation. Guard/refclosure
3.5/3/3us rulesoutlargeguardtimestampoverhead alone, notpreacceptancelatency.
ACCEPTED_PAIR_COMPARISON.md selectedfaultlimitations and persectordeltas.
Next boundedpreacceptance evidence, notaggregatemax attribution. No motor/
firmwarechange. ActualACEE/off, allpriorfailedcampaignsremainfailed.

## Entry452 -2026-09-14 -small qualification accumulator staged

Pure <=8B diagnostic completedcall/guardoverlap/maxduration/saturation logic,
three tests, no runtimeintegration. QUALIFICATION_WINDOW_DESIGN.md distinguishes
callbracket from edgearrival and requires finalfaultcall completion/history/
disabledtiming proof. No embeddedbuild/motor. ActualACEE/off unchanged.

## Entry453 -2026-09-14 -qualification history/fault semantics tested

Pure16summaryhistory, host<=320B, explicitaccepted/stopped/ordinal/omissions.
Finalfaultcall storedbeforefreeze; idlefreeze retains pendingwithoutfakeedge.
201lib+34RustPASS including3newhistorytests. Notfirmwareintegrated/timed.
Next optionalhooks/framing/disabledchecks; actualACEE/off, no motor/build.

## Entry454 -2026-09-14 -optional qualification hooks built

4EFB opt-inbuild124976/1104/29624, noflash/motor. Poweredtrace0Scope spans
dispatchbody, guardseqentry observed, completedfaultcallback retained.
PoststopQW85/QP85 behindcapture. Firstcompilemissingimports corrected.
Strictdecoder/disabledScopeoverhead/targetaudit remain, NOTmotorqualified.
QUALIFICATION_WINDOW_DESIGN.md fullhash/scope. ActualACEE/off,root4EFB.

## Entry455 -2026-09-14 -qualification decoder rejection tests

Standalone strictQW85/QP85 header/CRC/counts/ordinal/finalstop parser,
4synthetic testsPASS. Doesnotprove motor/off/epoch; fixtureintegration and
disabledScope timing stillrequired. No hardwarecapture/build/flash/motor.
ActualACEE/off,root4EFB unchanged; QUALIFICATION_WINDOW_DESIGN.md details.

## Entry456 -2026-09-14 -fixture flag and disabled Scope command built

B6EF release125952/1104/29624,378PythonPASS. Opt-infixture trace0 plusdecoder;
qualcheck5modesx16 actualScope, disabled-only, notexecuted. Privatehelperaccess
compilefailures corrected with pub(super) bridgecheck, no safetylogicchange.
No flash/motor,actualACEE/off. Hostverifier/timing/codegen/disabledexecution
remain beforepoweredprobe. QUALIFICATION_WINDOW_DESIGN.md fullhash/details.

## Entry457 -2026-09-14 -disabled qualification probe too expensive

B6EF installedafteroff/hashcheck. UARTbootfaultsnapshot retainedbeforeclock
repair,guard02PASS3/18. ActualScope5modes16/16semanticPASS, max6/5/6/6/1us
FAILpredeclared4/2/4/4/4limits. Hostretainsrefusal/finaloff. No motorcommand;
poweredqualification stopped. ActualB6EF/off,notpowerqualified. Optimize
cost/changeinstrument, neverraisegate to observedcost. Designmemo details.

## Entry458 -2026-09-14 -publication split candidate built

ExtractedHistorypublication to noninline function; validations/counts/freeze
unchanged. Scopefinish328->220B,localframe52->36. F686release125960/1104/29624,
auditmatch,201lib+34RustPASS. No timinggain yet,NOTFLASHED. ActualB6EF/off,
notpowerqualified. Nextfixeddisabledscopegate, no motor. Designmemo fullhash.

## Entry459 -2026-09-14 -publication split still exceeds observer budget

Interrupted guard capture contained only FINALOFF, not readback. Retained
qualsplit_boot_snapshot01 before documented UART clock repair: ENABLE0,
MOE0, CCRs0, USART configured but peripheral clock off. guard02 PASS3/18.
probe-rs verify confirms installed F686 matches root ELF. No reflash/motor.
qualsplit_scope01 semantics5x16 PASS, max6/4/6/6/1us rejects unchanged
4/2/4/4/4 limits. Final six gates/ENABLE/MOE zero and nFAULT1 verified;
port closed. Installed diagnostic NOT power-qualified. Frequent-path1us
improvement insufficient; next cheaper/sparse evidence, not threshold changes.

## Entry460 -2026-09-14 -outlined accumulation costs more, rejected

Scalar-overlap/outlined Window candidate55BA built (202+34 Rust testsPASS),
flashed after off. UART clock-off retained in qualscalar_boot_snapshot01;
documented repair then guard02PASS3/18. qualscalar_scope01 semantic5x16PASS
but7/5/7/7/2us exceeds unchanged4/2/4/4/4, worse than F686. Finaloff verified,
portclosed, no motor. Removed this unsuccessful source change; installed/root
55BA remain rejected diagnostic, source restored E459. Next sparse long-call
instrument with explicit coverage/cost contract, not more accumulator tweaks.

## Entry461 -2026-09-14 -sparse retention contract and host tests

MatchedACEE passing captures01/02 report COMPmax40us, failed03 reports48us.
Not causal proof. New pure qualification_sparse selects>40us or final stopped
call;16rows retain accepted identity, sector, duration, raw entry tick and
overlap flag. Epoch/count/overflow faults explicit; normal calls not counted.
Five new testsPASS (206lib+34otherRust total). NOTliveintegrated or timed;
no build/flash/motor. Installed/root55BA rejected/off unchanged. Designmemo
sets2usnormal/4usselected incremental gates and remaining protocol obligations.

## Entry462 -2026-09-14 -sparse normal path qualifies, selected path does not

Opt-in sparse integrated, releaseC17C (designmemo fullhash). Disabled guard01
PASS3/18 without UARTrepair. qualsparse_scope01 semantic5x16PASS, costs
2/5/5/5/2us reject fixed2/4/4/4/4 gates. Normal-path cost improved; selected
publication remains1us too costly. Finaloff verified, UARTclosed, no motor.
Installed C17C NOTpowerqualified. Strict QS85 decoder, epoch integration audit,
fullCPU preflights and selected-path reduction remain before powered use.

## Entry463 -2026-09-14 -strict sparse substream decoder

QS85 now carries CRC-protected epoch plus row,10u16; strict parser rejects
wrong epoch, flags, ordering, selection, length/CRC and duplicate protocol.
Five new synthetic testsPASS; all387PythonPASS. Empty sparse history is not
no-fault proof; fullcampaign/off verification separate. Release7E6F built,
notflashed (designmemo fullhash). ActualC17C/off remains rejected5usselected.
No hardware access. Next selected cost/epoch integration audit, not power yet.

## Entry464 -2026-09-14 -remove sparse ring clearing after fresh seed

Reset audit found full sparse ring clearing on coast_run_inner fresh-seed path.
Metadata-only reset now hides old rows without clearing192bytes;207lib+34Rust
testsPASS and emitted scalarstores confirmed. Epoch exhaustion cannot reuse1.
Sparse history is finalepoch-only, not part of FIRST_SEGMENT archive; external
epoch binding remains required. Release9343 built NOTflashed (designmemo hash).
ActualC17C/off still selected5us rejected. No hardware; no seed-age gain claimed.

## Entry465 -2026-09-14 -ordering-bound cache, timing still refuses

Candidate60C9 stores exact accepted-order bound instead of prior-row lookup;
207lib+34RustPASS. Flashed afteroff, guard01PASS3/18. sparse_bound_scope01
still2/5/5/5/2us FAIL fixed2/4/4/4/4; finaloff verified, portclosed, no motor.
Installed/root60C9 unqualified. No measured speedup. Fixture currently times
empty ring only; populated/wrapped timing coverage must be added, without
discarding existing cold-path failure. Designmemo fullhash/details.

## Entry466 -2026-09-14 -populated and wrapped sparse timing coverage

Eight-mode disabled test nowpreloads1/16/32rows outside measured bracket;
retention/omissions/finalidentity semantics8x16PASS. Actual1ECC installed,
UARTsnapshot/repair then guard02PASS3/18. scope01 timings2/6/6/6/2/6/6/6us
FAIL unchangedlimits; finaloff verified, portclosed, no motor. Added timed
harnessbranches mean5->6us is not a proven live-code regression. Next isolate
harness cost without assumed subtraction or widened gates. Designmemo details.

## Entry467 -2026-09-14 -hoisted test selection does not remove cost

Synthetic inputs precomputed opaque before timing; shared noninline finish.
Baseline separately reported, never subtracted. Installed2D94 (designmemo hash),
UARTsnapshot/repair then guard02PASS3/18. scope01 semantic8x16PASS but remains
2/6/6/6/2/6/6/6us FAILunchangedlimits. Finaloff verified, portclosed, no motor.
No measured gain; no more harness-only tweaks. Selected publication unresolved.

## Entry468 -2026-09-14 -UART clock loss narrowed by boot breadcrumbs

Fourboot-only APBENR1 snapshots installedA3BD (designmemo hash): preHAL and
postHAL00040000, postADC and postbanner0. Silentguard01 retained; snapshot
current08000000, ENABLE0/MOE0/CCRs0. Documented repair then guard02PASS3/18,
finaloff/closed, no motor. No sourceclockclear found in narrowed interval;
reset/debuginteraction vs firmwarecause stillunproven. No blind permanentfix.

## Entry469 -2026-09-14 -debugger clock-register writer identified

EmittedA3BD postHAL->postADC interval has no APBENR1 store. One traced reset
shows session_drop/debug_core_stop reading then writing4002103c aftercore run.
Payloadnotlogged: startupRMWrace remainshypothesis. uart_reset_trace_guard01
PASS3/18 withoutrepair, offverified. No flash/motor; actualA3BD unchanged.
Trace retained; next debuggersequence/controlledordering, not blindHALpatch.

## Entry470 -2026-09-14 -debugger RMW mechanism and reset alternative

Exactprobe-rs3c10cd38 source uses wholeAPBENR1 read/clearbit27/write during
debug_core_stop, matchingtrace externalwriter. ConcurrentMCU enable can be
lost; exact failedpayload notcaptured. UnchangedA3BD OpenOCD reset3/3 cohort
openocd_reset_guard01..03 PASS3/18 withoutrepair, finaloff verified, ports
closed. No flash/motor. UART_RESET_WORKFLOW.md source/exactresetcommand.
Return to unresolved recordercost; no firmwareclockworkaround added.

## Entry471 -2026-09-14 -inline gain, packed-storage regression retained

Inline Tailpush candidate9451 reducesselected6->5us but stillFAIL4us. Packed
three-word storage candidateEAB5 thenworse2/6/6/6/2/6/7/6us, reverted insource.
Both8x16semanticsPASS, guard3/18PASS, OpenOCDreset noUARTrepair, finaloffverified,
no motor. Actual/rootEAB5/off rejected, sourcebacktoinlineform. Designmemo
fullhashes and208+34 tests of packedcandidate. No gatechanged or failureexcluded.

## Entry472 -2026-09-14 -RAM recorder passes disabled timing gates

Derivedringcounters candidateAAF2 still5usselectedFAIL, retained. Optional
bench-sparse-ram thenmovesonly200byteScopefinish into startup-copied.data.
Actual766D installed (designmemo hash/layout), guard3/18PASS, sparse8x16
semanticsPASS2/4/4/4/2/4/4/4us underunchangedlimits. Allfive disabledpreflights
PASSCPU2/7/10. BothOpenOCDresets noUARTrepair. Finaloffverified, no motor.
.data1528/bss29344RAM; executable.data counted astext bysize. Capture/epoch
fixture binding and hardwareQS85 stillrequired before powereddiagnosis.

## Entry473 -2026-09-14 -hardware sparse wire roundtrip, timing regression

F33F installed (designmemo hash); wire_guard01PASS3/18 noUARTrepair. Actual
Scope+dump encoder produces16CRC/epoch/exactfield-verified QS85 rows,5omitted,
shortfinalstop preserved. Wire roundtripPASS independently. BUT8mode timing
3/4/5/4/2/4/5/4us FAIL unchangedlimits; no powerqualification transferred.
Finaloff/closed, no motor. Next check/report call-site isolation, not gatechange.

## Entry474 -2026-09-14 -sparse/wire pass, broader CPU preflight fails

Outlined disabledcheck candidate2CA5 installed (designmemo hash). guard3/18PASS,
sparse2/4/4/4/2/4/4/4PASS and hardwarewirePASS. Broaderfilter/atomic/phasePASS
but CPU2/7/11us FAIL10us gate; routingstage notrun. Finaloff verified, no motor.
Next nested-accounting/call-layout investigation; no borrowed766D pass or retries.

## Entry475 -2026-09-14 -isolated CPU test restores disabled qualification

CPUcheck noninline only, liveaccountinglogicunchanged. ActualBDE2 installed
(designmemo fullhash); guard3/18PASS, sparse2/4/4/4/2/4/4/4+wirePASS, allfive
broaderpreflightsPASSCPU2/6/9 and routingchecks. Finaloffverified, UARTclosed,
no motor. Improvementis disabledharnesslayout, notliveIRQoccupancy measurement.
Next finalepochbinding/fixtureopt-in thenbounded powereddiagnosis.

## Entry476 -2026-09-14 -powered sparse diagnostic captures a real stop

Final observation epoch/count binding and fixture opt-in implemented;
390 host tests PASS. Actual0F9F build (fullhash/designmemo), disabled guard,
sparse8mode/wire and fullfive preflights PASS, CPU2/6/9. One30s72recovery
attempt FAILED after947550us resumed, CycleTiming12 step2 delta2850<2858.
1916COM/1915accept,337.008eHz aggregate,sigma25.845us,raw288,bus11044mV,
untouchedstack2244. Rawcapture qualsparse_epoch476_start61_reentry72_30s_01.
Sparse epoch134/3rows/noomissions validated: two41usaccepted overlaps and
final57us nonaccepted stopped overlap. Finalcall includes safing, not proof
of persistence preemption. Cyclepair+131.5/-123.5us nearly cancels, closure
11.5us; investigate bracket/work attribution next. No guard changed. Finaloff
verified, UARTclosed. Fullcontract/hashes/limits in QUALIFICATION_WINDOW_DESIGN.

## Entry477 -2026-09-14 -final sparse acceptance semantics resolved

Source/capture join proves E476 finalcall passed reference persistence and
reached EV_ACC; cycle guard then safed and vetoed ACCEPTS log increment.
accepted=false is not persistence rejection.57us includes shutdown, but
CYCLEFAULT delta2850 was sampled before trip, so shutdown cannot cause it.
drv_sparse_fault.py+3testsPASS enforce sector/count/callback provenance and
preserve unknown overlap placement/persistence duration. No runtime changes,
flash or motor. Actual0F9F/off unchanged. Designmemo E477 details next bounded
priority/deadline-slack investigation, not further shutdown-only instrumentation.

## Entry478 -2026-09-14 -bounded exclusion does not remove 7.2% fault

F22D installed (COMP_CRITICAL_EXPERIMENT fullhash/features). Prior0F9F archived.
Exactbuild guard/sparsewire/wrapper/fullfivepreflightsPASS CPU2/6/9. One72/30s
reentry diagnostic FAILED1.724832s resumed CycleTimingstep5 delta2856<2858,
336.449eHz,3482COM/3481accepted,raw259bus11080,stack2244. Protectedreference
31060calls max45us/refused0. Final sparse51us call NOguardoverlap, persistence
passed. Midservicepreemption notnecessarycause; no hardware exoneration/blame
or broad timingproof.16rows/1105omissions, capturequalsparse_critical478_*.
Finaloffverified/UARTclosed, all failures retained. Next preentry/qualification
timing, not wider masks. Limitercounterexampletest also proves64/ms cannot
replace independentguard; no globalpriorityswap.208lib+34RusttestsPASS.

## Entry479 -2026-09-14 -earlier boundary shifts, not long final-call proof

Offline strict join links previous same-sector visit and successor. E476
identity1909 shows+103.5/-93.5us versus shorttail same-sector medians; E478
identity3475 +119.75/-118us. Both earlier identities have complete sparse
coverage and NO >40us selected dispatch. E478 older1105omissions do not hide
3475 because retained suffix begins3431. Four report tests PASS. Preentry
latency/multiple short qualification attempts remain hypotheses, not proven
physical edge delay. ActualF22D/off unchanged; no firmware or motor action.
Next target earlier-boundary qualification sequence. Designmemo E479 details.

## Entry480 -2026-09-14 -carrier discriminator geometry staged

Existing detailedtrace altersadapter or costs too much withtoo shorta history
for preceding-six-sector boundary. Chose carrier24k/20k intervention instead
of another fullcall recorder. PureKhz20 geometry3200ticks/32bins added and
exhaustivelytested208lib+34RustPASS. No liveselection/build/flash/motor.
ActualF22D/off unchanged. CARRIER20_EXPERIMENT.md lists ADC/host/feature and
disabledprerequisites, matchedlean68first, deadtime/ADC/workload confounders.
This is preparation, not evidence20k helps or poweredqualification.

## Entry481 -2026-09-14 -20k integration and matched lean artifacts

Explicit20kfeature integrated with BEMFprepare/restore and selectedADCgeometry.
Hoststrictcarrier/period validation and configurablepreflight;396PythonPASS.
20k9ACC and24k6D02release-s/thinLTO builds/frozenhashes inCARRIER20_EXPERIMENT.
Emitted20kADCbin usesconst10486/MULS/LSR20, no divisionhelper there. NOflash/
motor; actualF22D/off unchanged, root6D02. Next exactbuilddisabledprerequisites
and matched68/10s holds, not72jump or borrowedcarrierqualification.

## Entry482 -2026-09-14 -matched lean carrier holds both complete

6D02at24k then9ACCat20k installed with separateguard/fullfive/ADCroutePASS;
CPU2/6/9both. Eachone68/10s/start61 holdPASS,315.428vs330.282eHz,
IRQunion52.724vs50.426%,raw294vs314,bus10984vs11068,stack3460both.
Sigma39.18vs38.95 includesacceleration; no steadyjitter/reliabilityclaim.
20kADC3200metadata/bins validated, discretephasegrid differs. Finaloffboth,
UARTclosed. Actual9ACC/off,root6D02. CARRIER20_EXPERIMENT hashes/table;
next68/30s injectedloss/reentry, not higherduty or relaxedguards.

## Entry483 -2026-09-14 -20k and24k recovery comparisons pass

One68/30s injectedloss/reentry eachPASS:20k330.757eHz/sigma21.996us/
IRQ50.656%,24k315.751/23.632/52.713. COM/acc55549/55549 and53029/53028.
Raw352/265,bus10925both,stack2668,armspare11/17us. Fresh24krestoreguard/
fullfive/ADCroutePASSCPU2/6/9. Bothfinaloffverified/UARTclosed. Actual/root6D02.
CARRIER20_EXPERIMENT E483 fullhashes/table/limits. No rare-faultfix orcurrent
parityclaim. Next fixedtwoadditional20k68recoveryattempts failfast, not69yet.

## Entry484 -2026-09-14 -20k recovery3/3 andfirst69hold

Restored9ACC withguard/fullfive3200CPU2/6/9/ADCroutePASS. Plannedtwo68/30s
recoveriesPASS330.174and328.954eHz, making3/3withE483; no excludedfailures.
Sigma22.41/22.87us,IRQ50.40/50.23%,raw301/310bus11032/11056,stack2668.
One69/10sholdthenPASS335.061eHz,IRQ50.656%,raw283bus10984,COMP40COM59commit21.
ADC3200andfinaloffverifiedall,UARTclosed. Actual9ACC/off,root6D02.
CARRIER20_EXPERIMENT.md E484fullhashes/denominators. Next69recovery,not70yet;
no guardrelaxation,currentcalibrationorparitycompletionclaim.

## Entry485 -2026-09-14 -20k69 recovery3/3

Same9ACC three69/30s injectedloss/reentryattempts allPASS,noexcludedtries.
333.65..334.19eHz,sigma22.30..22.71us,IRQ50.50..50.88%,raw311..314,
busmin10889,stack2668,armspare>=9.5us. Alloriginaldeadlines/freshseeds/
archives/ADC3200/finaloffverified. AskednonblockingPSUreading,noanswerreceived;
currentcalibrationstillopen. Actual9ACC/off,UARTclosed,root6D02.
CARRIER20_EXPERIMENT E485fullhashes/table. Nextone70hold10s,no guardchanges.

## Entry486 -2026-09-14 -20k70 fails atsimilar local speedto24k73

One70hold FAILED203236us CycleTiming12step2 delta2844<2858.400COM/399acc,
raw297bus11354,stack2668,ADC3200/finaloffverified. No retry/recovery/higherduty.
Localtailmedian2926us/~341.764eHz,refcycle2841.5us,closure2.5us. Compareold
E44024k73failure:~342.818eHz,also2844guarddelta/~0.2s. Differentbuilds and
acceleration, notphysicalrotor/speedwallproof. Carrierwinsworkload andchanges
duty-speed, butnotprovenenvelopeextension. Actual9ACC/off,UARTclosed,root6D02.
CARRIER20_EXPERIMENT E486hashes/details. Next timing/admissionbudget audit;
no guardratchet. Current/referenceparitygoalstillincomplete.

## Entry487 -2026-09-14 -masked fresh-seed setup candidate

Sourceaudit found COMPenable thenimmediatemask within sameglobalmask before
freshseedarm. Opt-inbench-masked-seed-arm keepshardwaremasked and explicitly
setssoftwareMASKED=true; pendingclears/guards/age/bootstrapCOM retained.
BEA2releasebuilt NOTflashed; actual9ACC/off unchanged. No measuredsavings or
runningfaultfixclaim. MASKED_SEED_ARM_EXPERIMENT listsrequired disabledmask/
pending/bootstrap andcodegen/provenance checks beforehardwarequalification.

## Entry488 — 2026-09-14 — masked seed setup qualified at preceding point

C631 installed; guard/register/full-five/ADC-route disabled checks pass,
CPU2/6/9 us. 398 Python and242 Rust tests pass. Masked specialization's emitted
setup preserves software latch, pending clears and real timestamp; no redundant
COMP enable/remask. Disabled probe tests enable primitive, not powered COM ISR.
One 6.9% / 30s injected-loss recovery passes at333.791 eHz, sigma21.938 us,
56058COM/56057accepted, IRQ50.467%, raw350, busmin10781, stack2668.
Seed age83 us versus85 baseline, arm body7 us unchanged; one-run gain, not WCET.
Original deadline, archives, ADC3200 and finaloff verified; UART closed.
MASKED_SEED_ARM_EXPERIMENT E488 records exact hashes, limits and evidence.
Running-cycle outlier/current calibration/parity still open; no guard increase.

## Entry489 — 2026-09-14 — compare boundary pairs in passing tails

Offline six-capture comparison with strict CRC/chronology/off validation.
E486 largest pair belongs to identity393/step2, the previous visit to the
later-refused sector, +90.75/-67 us versus other same-sector tail medians.
Passing E485_02 also contains +56.25/-64.25 us: opposite-sign pairing alone
does not identify the fault. Different speeds/builds/acceleration and short
retention prevent population/causal conclusions. Four host tests added.
BOUNDARY_PAIR_COMPARISON.md documents method, captures and next diagnostic
requirements. No firmware or hardware action; actual C631 remains last safed.

## Entry490 — 2026-09-14 — per-accepted qualification accumulator

Implemented pure host-tested accumulator for dispatched path counts between
accepted boundaries, first/last open count, identity/sector and explicit stop.
Overflow and gaps invalidate, external freeze preserves partial history.
Five new tests; full213 library+34 other Rust tests PASS. No firmware integration,
target timing, flash or motor run. QUALIFICATION_EVENT_DESIGN.md records live
epoch/stop/skip coverage and <=2/4us ordinary/publication experiment gates.
Actual C631 last verified-off state unchanged; no operating-envelope claim.

## Entry491 — 2026-09-14 — staged per-event live diagnostic

Bounded16-row history and powered dispatched-Scope adapter staged behind
bench-qualification-event. Epoch, accepted identity, omissions, invalidity and
explicit partial frame preserved; no extra comparator reads. 215 library+34
other Rust tests PASS. Release63698283 built NOT flashed; text127232/data1144/
bss29764. Math audit SHA matches; Scope stack68bytes, added runtime cost unknown.
Strict host decoding and disabled cost/wire/full preflights remain mandatory.
QUALIFICATION_EVENT_DESIGN E491 details. No motor/UART; actual C631 last safed.

## Entry492 — 2026-09-14 — qualification-event decoder

Strict QE85 decoder with epoch/identity/sector/omission/partial/final-count
validation and CRC. Four synthetic tests; all406 Python tests PASS. Unknown
stopped callback remains unknown, even if actual ACCEPTS incremented; no
persistence or physical-edge claim. External expected epoch/count checks
explicitly reported. No hardware wire/fixture proof yet; disabled Scope cost
test next. No firmware build/UART/motor; actual C631 remains last safed.

## Entry493 — 2026-09-14 — per-event diagnostic cost rejection

Installed43182426 release-s/thin-LTO, guard3/18 PASS, OpenOCD reset without
UART repair. Disabled actual-Scope harness18 modes x16 repetitions: semantics
PASS, timing FAIL. Ordinary7–8us exceeds2us; publication10–13us exceeds4us.
Capture qevent_493_scope01 retained and rejected by strict host verifier.
No motor/fullpreflight after failure. Finaloff verified; UART closed.
QUALIFICATION_EVENT_DESIGN E493 records exact hashes and maxima. Next reduce
frequent-path work, not just ring publication; no guard or timing-gate change.

## Entry494 — 2026-09-14 — hot metadata placement improves rejected probe

History repr(C) places hot accumulator before cold rows. Emitted visit468->368
bytes; 216lib+34Rust tests PASS. Actual7BF33999 installed, guard3/18PASS.
Disabled288 semantic trials PASS; costs6–7us ordinary/9–12us publication still
FAIL2/4us gates, roughly1us saved. No motor or broader preflight; finaloff
verified, UART closed. QUALIFICATION_EVENT_DESIGN E494 hashes/details.
Next simplify collection architecture, not more layout-only optimization.

## Entry495 — 2026-09-14 — publication split still fails

Outlined terminal publication: ordinary History::visit160bytes/localframe4bytes,
216lib+34RustPASS. Installed95FBAD72, guard3/18PASS. Disabled288 semanticsPASS,
ordinarymax6us/accepted13us stillFAIL2/4us gates; no powered run. Finaloff
verified, UART closed. QUALIFICATION_EVENT_DESIGN E495 fullhashes/table.
Next direct first-read/acceptance hook collection instead of further tuning
the costly Scope composition. No causal or operating-envelope claim.

## Entry496 — 2026-09-14 — direct first-read counter

Pure <=16byte counter added: dispatched/first-read closed/open and first/last
open count, frozen/invalid partial state. Four tests including65535 visits;
full220lib+34RustPASS. Quantities differ from QE85 persistence outcomes.
QUALIFICATION_DIRECT_DESIGN.md specifies live ownership/threshold/commit and
cost checks still required. No integration/build/UART/motor; actual95FB safed.

## Entry497 — 2026-09-14 — staged direct collector hooks

New opt-in direct collector begins after dispatcher checks, observes first
actual interval read, and publishes only at accepted recorder commit after
guard/ownership vetoes.16-row packed suffix and separate QD85-v1 framing;
partial counts extend until dispatch unwind, not physical shutdown time.
Release4E1CB559 builds, NOT flashed. No UART/motor; actual95FB last safed.
QUALIFICATION_DIRECT_DESIGN E497 documents exact artifact and remaining
decoder, disabled cost/wire, stack and fullpreflight requirements.

## Entry498 — 2026-09-14 — direct-format decoder

Strict QD85 decoder and four tests added; all412 PythonPASS. Validates CRC,
epoch/identity/sector/suffix/counter/partial geometry and final accepted count.
Partial open bucket remains uncommitted; no physical-edge/persistence claim.
Optional external binding is explicit. Hardware cost/wire still unverified.
No build/UART/motor; root4E1C staged, actual95FB last safed. Design E498 details.

## Entry499 — 2026-09-14 — direct collector measured, still over budget

Installed74E824C4 release-s/thin-LTO; guard3/18PASS. Disabled240 semantic
trials and actual QD85 wrapped16-row/partial CRC wire PASS. Costs3/3/7/2/2us
closed/open/accepted/no-gate/disabled at all preload sizes: stillFAIL2/4us
gates, though materially cheaper than Scope. No motor/fullpreflight. Finaloff
verified, UART closed. QUALIFICATION_DIRECT_DESIGN E499 exact hashes/limits.

## Entry500 — 2026-09-14 — RAM collector helpers

Installed06AB830F: begin88B/accepted216B in startup-copied RAM, .data1440,
bss29608. Disabled guard3/18 and240 semantics/wrapped-wirePASS. Accepted
publication improves7->5..6us; ordinary still3us, FAIL2/4 gates. No motor or
broader preflight. Finaloff verified, UART closed. Design E500 exact hashes.
RAM fetch cost is a measured lever, not sufficient qualification or speed gain.

## Entry501 — 2026-09-14 — RAM first-read helper regresses

InstalledEB6562FC test moved first_count76B to RAM/noninline. Guard3/18 and
240semantics/QD85wirePASS; costs4/4/6/2/3us FAIL2/4 gates, ordinary worse.
No motor/fullpreflight, finaloff verified/UART closed. Reverted first_count
source to inline, no subsequent rebuild/flash; actual/rootEB65 remain rejected.
QUALIFICATION_DIRECT_DESIGN E501 hashes and changed-placement confounder.

## Entry502 — 2026-09-14 — word-aligned retained rows

Installed8ED4A603, four-u32 row storage with identical post-stop QD85 data.
Guard3/18 and240semantics/wrappedwirePASS. Publication5us all buffer states;
ordinary worst3us, stillFAIL2/4 gates. No motor/fullpreflight. Finaloff verified,
UART closed. QUALIFICATION_DIRECT_DESIGN E502 exact hashes and quantized table.

## Entry503 — 2026-09-14 — first-read latch simplification, no timing gain

InstalledD31FAB7A release-s/thin-LTO. Dispatch exit/freeze/reject now close
the first-read latch; acceptance still checks ACTIVE. 221lib+34other RustPASS.
Guard3/18 and240semantics/wrapped/partial QD85 wirePASS. Costs3/3/5/2/2us at
all preloads stillFAIL2/4 gates; no measured worst-case gain over E502.
No motor/fullpreflight. Finaloff verified, UART closed. Direct design E503
records exact hashes. Last powered evidence is E488, not this diagnostic test.

## Entry504 — 2026-09-14 — final-observation capture binding

Staged DIRECTBIND snapshot/report and strict host campaign binding to the
collector epoch and ACCEPTQUALITY final count. Fixture opt-in requires trace0;
all414 Python tests PASS. Release0BDAC792 built, emitted audit SHA matches,
NOT flashed. No UART/motor; actualD31F remains last verified OFF/unqualified.
Cost audit finds ordinary harness has an extra read compared with rejected
reference calls, but acceptance still fails; no timing gate changed. Direct
design E504 details provenance limits and outstanding hardware qualification.

## Entry505 — 2026-09-14 — native packed counters measured

Installed9001C2B5 release-s/thin-LTO. Native u32 count words remove live row
repacking; acceptance helper224->200B but begin88->92B. 222lib+34other Rust
tests PASS including mixed65535-bin bounds and reuse. Guard3/18 and240semantic/
wrapped/partial QD85 wire checks PASS. Timing unchanged3/3/5/2/2us all preloads,
stillFAIL2/4 gates. No motor/fullpreflight. Finaloff verified and UART closed.
Direct design E505 has exact hashes; no measured motor or timing improvement.

## Entry506 — 2026-09-14 — higher-resolution disabled timing

Installed7BB98B24 adds read-only SysTick measurement inside directcheck;
production hooks unchanged. Guard3/18 and240semantic/QD85wirePASS. Raw max
closed146/open160/accepted299cycles at64MHz:2.28125/2.5/4.671875us, stillFAIL
2/4us limits without subtraction. Not just1us rounding. Outer TIM17 includes
extra observer reads, so its acceptance6us is not a live-path regression.
No motor/fullpreflight; finaloff verified/UART closed. Design E506 exact hashes.

## Entry507 — 2026-09-14 — inline setup gives a small measured gain

InstalledB2DB8535, begin inline/accepted RAM. Guard3/18,240semantics/QD85wire
PASS. Worst cycles130/143/290/88/61 save16/17/9 on active cases but still
FAIL128/256. No motor/fullpreflight; finaloff verified/UART closed. Design
E507 hashes. First-read already inlined; no redundant attribute experiment.

## Entry508 — 2026-09-14 — remove threshold clamp without changing comparison

Installed2EB49AB9, full-u32 cached half-average. Exhaustive boundary tests
included:223lib+34other RustPASS. Guard3/18,240semantics/QD85wirePASS.
Worst raw cycles121/134/281/79/61 improve9cycles on active paths; closed
passes128cycles, open and acceptance remain6/25cycles over128/256. No motor
or fullpreflight; finaloff verified/UART closed. Design E508 exact hashes.

## Entry509 — 2026-09-14 — unified collector ownership

Installed87E0DC49: inactive/unread/read state replaces separate ACTIVE/seen.
Acceptance after exit now rejected by pure Counter as well as live adapter.
224lib+34other RustPASS; guard3/18,240semantics/QD85wirePASS. Worst raw cycles
118/134/277/80/50 improve acceptance4cycles, open unchanged; stillFAIL128/256.
No motor/fullpreflight; finaloff verified/UART closed. Design E509 exact hashes.

## Entry510 — 2026-09-14 — acceptance checks use proven counter invariants

InstalledBBC24B6D, READ-first validity and open-implies-dispatched eliminate
redundant hot checks while preserving non-invalid frozen partials.224lib+34
other RustPASS; guard3/18 and240semantics/QD85wirePASS. Raw max118/134/264/
80/50cycles: acceptance improves13cycles, open/accepted still6/8 over128/256.
No motor/fullpreflight; finaloff verified/UART closed. Design E510 hashes.

## Entry511 — 2026-09-14 — direct capture qualified and motor campaign passed

Installed/frozenBC876BF0. Const-mode harness puts mode dispatch outside hook
bracket. Raw worst74/88/236/60/11cycles PASS128/256 without subtraction; outer
TIM17 still fails5>4 because it includes extra observation work. Explicit
cycle-gate retains numeric2/4us limits; old failures retained. Guard/fullfive
preflights3200 CPU2/6/9/ADCroute PASS,416 PythonPASS.

One matched6.1%startup/6.9%BEMF,+60degrees,20kHz,30s dropout/reentry PASS:
333.827eHz, sigma24.09us,56066COM/56065accepted,IRQ53.907%,raw347,bus10937,
stack2200. First actual DIRECTBIND/QD85 epoch246/16rows+partial validates,
as do ADC3200/original deadline/recovery/finaloff. UART closed. No CycleTiming
fix or calibrated-current claim. Design E511 exact hashes, cost scope and
next diagnostic70hold at the previously failing point.

## Entry512 — 2026-09-14 — 7.0% fault has an earlier qualification extension

SameBC876. One70/10s hold FAILED439797us CycleTimingstep2 delta2810<2858;
880COM/879accepted,raw304,bus11343,stack2200. QD85epoch248/final879,ADC3200,
fault/timeline/finaloff validate; UART closed. No retry or guard change.

Strict direct/reference join: earlier long boundary873 step2 has11open visits,
first266.5,last569,reference578us. Prior same-sector867:5open,first287,last458,
reference467us. Final last-open-to-reference is9us in both; extension is before
the final gate read, not late first gate or a long final accept path. Following
874 is404.5us with5open. No physical-edge/persistence/noise/preemption proof.
drv_direct_intervals.py four tests;all420PythonPASS. Design E512 fullhash/table.

## Entry513 — 2026-09-14 — edge/filter source audit and precise same-sector join

No hardware/build; BC876 last verified OFF. Source configures one raw edge,
clears pending on each open visit, and matches reference filter12 atavg977.
The9us bracket includes diagnostic/clear/mask/CS work, not just persistence.
Targeted reference tests4+3+1 PASS. Strict six-ID comparison records boundary
873 vs867: +6open,-1closed,first-20.5us,last+111us,reference+111us. New tests
reject missing/incorrect predecessors;all421PythonPASS. Design E513 details.

## Entry514 — 2026-09-14 — exact rejection-index observer seam

Shared minz-core Recorder gains default-noop persistence_rejected(index),
called once at existing mismatch return with no added comparator read.85core
and224+34binz Rust tests PASS; every0..11 failure index and silent success/
closed-gate behavior checked. Binz callback not connected yet. RootF21FBE7E
built NOTflashed; five PT_LOAD segments exactly match frozenBC876, proving
default-noop codegen on this build. ActualBC876 last safed; no UART/motor.
PERSISTENCE_REJECTION_PROBE.md records proof and bounded opt-in mask plan.

## Entry515 — 2026-09-14 — rejection mask and v2 wire integrated

Opt-in reference callback records12-bit presence mask, no extra comparator
reads/timestamps. QD85-v2 reuses sector-word bits, same11word CRC frames;
host rejects unknown versions, impossible masks and absent requested v2.
227lib+34Rust/422PythonPASS. Installed144CEBD0: guard3/18 and240semantics/v2
wirePASS; runtime-index rejection136cycles FAIL128. No motor/fullpreflight;
finaloff verified. PERSISTENCE_REJECTION_PROBE E515 exact hash/capture proof.

## Entry516 — 2026-09-14 — reordered rejection validation does not improve cost

Installed61402B05, READ-first check preserves frozen partial semantics.
227lib+34RustPASS; guard3/18,240semantics/v2wirePASS. Rejection140cycles vs136,
stillFAIL128; acceptance236<=256. No motor/fullpreflight; finaloff verified,
UART closed. Source retains tested variant. Probe memo E516 exact hashes.

## Entry517 — 2026-09-14 — rejection probe cost passes; powered recovery passes

Explicit open/consumed state removes redundant hot count load and stale-callback
authority.229lib+34Rust/85core/422Python PASS. Installed7C7E773B release-s/LTO;
raw reject126<=128,accept236<=256. All disabled preflights and ADCroute PASS.
One69/30s recovery PASS333.903eHz,sigma24.979us,IRQ54.411%,56078COM/56077acc,
raw338,bus10948,stack2200. QD85-v2 actual rejection positions verified.
Final off/UARTclosed. PERSISTENCE_REJECTION_PROBE E517 full provenance/limits.

## Entry518 — 2026-09-14 — 70 diagnostic exposes late-read mismatch

Same7C7E build,one70/10s hold FAILED after222154us CycleTiming12 step4
2844<2858.438COM/437acc,raw312,bus11462,stack2200; alloff/timeline/CRC pass.
No retry. Boundary431 step4 has actual rejection indices0,1,11,9open visits;
reference interval539.5us (+49us vs prior same sector), next interval425us.
Late rejection also occurs in successful517, so not alone causal evidence.
Partial437 indices0 only; final accepted reference path was motor-guard-vetoed.
Probe memo E518 exact hashes, comparisons and inference limits. No guards changed.

## Entry519 — 2026-09-14 — active probe changes persistence loop footprint

Offline disassembly audit finds15 instructions per successful static loop in
BC876 vs16 current7C7E; one index-preserving move added. Both one comparator
read/no calls. This is not elapsed-time or no-preemption proof. Added repeatable
drv_persistence_codegen.py and3tests; probe memo E519 exact addresses/limits.
No flash/UART/motor;7C7E remains last verified OFF. Next isolate the timing of
an actual failed visit, not infer its location from an aggregate presence mask.

## Entry520 — 2026-09-14 — late callback coordinates; opt-z timing refuses

Added opt-in ring8 of late rejection callback interval/PWM/TIM6 coordinates,
strict RT85-v1 identity/CRC/order decoder.231lib+34Rust/429Python PASS.
opt-s exceeds flash960bytes; opt-z/thinLTO fits, installed84DFB2A3/OFF.
Guard3/18 and240semantics/QD85+RT85 sequence PASS, timing FAIL: rejected546
cycles,closed174,accepted387 exceed original128/256. No motor/fullpreflight.
Finaloff/UARTclosed. Probe memo E520 exact hashes; return to opt-s, reduce
capture footprint/cost. No powered qualification transferred to this build.

## Entries524-525 — 2026-09-14 — authorized PSU-only AM32 control runs

Standalone board-only UART/cap/PD1stop adapter, original comparator/commutation
algorithm retained. E524 early-stop fault localized to UART deadman counting
foreground setInput calls. E525 moves advancement to periodic TIM6. Corrected
2s runs input10/15/20 report~268/432/564eHz, PWM~9.90/14.93/19.92%, running1,
finalENABLElow. E524 premature captures retained, not passes. Exact image hashes,
adaptations and evidence limits in AM32_DRV8304H_BUILD.md. No binz guard waiver
inferred; operator explicitly authorized PSU-only AM32 experiment.

## Entry526 — 2026-09-14 — operator-requested25/30 response

Raised UART and hard compare ceilings to30%, no comparator/commutation edits.
Installed AM32 ELF7826B72D. Separate2s runs25/30 input report687.29/831.26eHz,
CCR667/799 over2666 ticks, zero_crosses8151/9615, running1. No early stop line;
final PD1ODR0 for each; UART closed. Raw am32_526_input25_2s_01 and input30
captures/hashes in AM32 build memo. Short response only, not sustained lock,
independent qZC or exact late-boundary cause. Board remains AM32/ENABLElow.

## Entry527 — 2026-09-14 — foreign flash bytes were AM32 settings

Stopped SWD eepromBuffer matches binz84DF instructions formerly at0800f800,
apart from normal loader/version/board edits. Earlier fast runs are real but
not default-config parity. Auto-advanceF0,bi-direction05,compPWMD5,limits.current1
(limiter enabled!) invalidate assumed settings. Full48byte comparison in AM32
build memo. Replaced board flash loading with repository48byte default baseline.
Installed98615765.10/20input two-second attempts each producezc2/~21eHz:
entry failures, not clean response. Old fixture exit0 corrected to rejectzc<=20.
Both finalPD1low/UARTclosed; no30attempt. Need configuration/startup bisection,
not hardware diagnosis or further duty increase. AM32 remains installed/off.

## Entry528 — 2026-09-14 — known-config bidirectional startup A/B

Known defaults plus bi_direction1, image19D90C45:10input2s response334eHz,
zc3931; following20input2s failed(zc4).10/15/20 ramp1.5s each failed(zc0).
All stopped PD1low; all raw attempts retained in captures/am32_528*.txt.
Stopped SWD found uart_input0 but adjusted_input46 and stuck-rotor latch102:
receiver mapping overwrites UART stop. No per-segment ramp success claim.

## Entry529 — 2026-09-14 — UART stop reset fixed, entry still fails

Board-only mirror UART input after receiver mapping, before unchanged stock
stuck-rotor guard. NativeO3 ELF BC0772AD4E83A485374D077FE42297734B37EF0B4CA37C5EF2699CB9D2454B8A
built/math-audited/downloaded, OpenOCD reset. One10input2s FAILED:avg15743,
zc0,running0,CCR0. Capture am32_529_uartmirror_input10_2s_01.txt retained.
UART verified PD1low; post-stop SWD adjusted_input0/uart_input0/latch0/PD1ODR0.
Fix proves stopped reset semantics, not reliable startup. No more25/30 tests
on known settings. E526 fast responses remain contaminated-config observations.
AM32 build memo has hashes and reproduction; current board BC0772 disabled.

## Entry530 — 2026-09-14 — known-settings AM32 reaches741eHz; ramp failure retained

UART-only target now bypasses dummy receiver decoding, which selected reverse
from newinput48 despite UART-controlled throttle. Configured forward1 preserved.
Stock control/protection unchanged. NativeO3 build E2BFFF04 frozen under
captures/reference/am32_e530_e2bf, installed via download/OpenOCD reset.
10input2s response331eHz; separate ramps10->20 and10->25 (5point steps,1.5s each)
respond586/741eHz, actualPWM21.455/26.519%. Separate10->30 ramp FAILED with
zc0/running0 at stop. Stage unknown without segment telemetry; NOT a30%wall.
All four attempts retained am32_530_uartdecode*.txt; full hashes/table in AM32
build memo. No identical retry. All finalPD1low; stopped SWD forward1,UART0,
adjusted0,latch0,PD1ODR0. Known-config short response above binz334 now observed;
not sustained lock/current parity qualification. AM32 E2BFFF04 remains disabled.

## Entry531 — 2026-09-14 — diagnostic ramp failed in its first segment

Installed AM32 97307B38, nativeO3/math-audited, command-boundary RAM8x7
snapshots printed only after PD1low. Host checks count/order/overflow/bounds;
three decoder tests including actual retained capture. No controller changes.
One10/15/20/25/30 ramp1.5s perstep failed: first10 endpoint alreadyzc0,
running0,CCR0,latch102; same later endpoints. Not a high-speed ceiling test.
Final UARTPD1low verified, no retry. AM32 build memo E531 has hash/protocol.
Raw am32_531_segments_ramp10to30_01.txt retained. Prior E530 failure stage
still unknown; next startup investigation. Binz guards remain unchanged.

## Entry532 — 2026-09-14 — lower-input cohort and exact frozen-build check

Fixed six attempts10,7,7,10,10,7 input,2s each on97307B38: all failed final
zc0/running0; all PD1low verified. Lower input did not help this cohort.
Restored exact E530E2BFFF04 and one10input2s also failed, so E531snapshot
addition is not necessary for failure. No code/guard changes; all seven raw
captures am32_532*.txt retained, table/hashes in AM32 build memo.
Stopped SWD confirms UART0/adjusted0/latch0/PD1ODR0; voltage raw1227,
DMA1084/1227/942. Not calibrated current or loaded bus evidence. No hardware
fault inference. Actual board E530 frozen/off; source/obj still E53197307B38.

## Entry533 — 2026-09-14 — coherent bootstrap, same startup failure

Board-only boot comStep(2) contradicted initialized step1/rising1. Changed to
comStep(step)+changeCompInput while driver disabled. No control/guard changes.
NativeO3/math-audited02592720 installed. One10input2s FAILED avg16126,zc0,
running0,CCR0; UART finalPD1low, no retry. Raw am32_533_bootsector_input10_2s_01.txt.
Three decoder testsPASS. Local DRV datasheet1ms wake matches requested delay;
no timing-violation or hardware-fault claim. Bootstrap correction is not an
entry fix. AM32 build memo E533; actual02592720/off, source matches.

## Entry534 — 2026-09-14 — frozen binz restored; advisory memo reviewed

AM32 stop verified; restored frozenBC876 via download/OpenOCD reset. Disabled
guard3/18, fullfive3200 preflights, CPU2/6/9us, ADCroute3 and direct semantic/
wire/raw128/256cycle gatesPASS (acceptmax236). All finaloff verified. Raw
restorebinz_534_*.txt retained. No motor command; user memo arrived before run.
Read all GRAYBEARD_INSTRUMENT_DISCIPLINE and new E478 falsification banner.
INSTRUMENT_DISCIPLINE_RESPONSE.md retains accepted direction plus source/math
corrections; no guard waiver or new probes adopted. Actualboard BINZ BC876/off,
root ELF differs, AM32 source/obj02592720 remain separate. COM41 binzoff/p/i.

## Entry535 — 2026-09-14 — binz baseline still starts and tracks

FrozenBC876, one6.1%drivenstartup/6.9%BEMF20k/+60deg/10s hold completed:
333.299eHz,19998COM/19997accepted,order_bad0,rawpeak310,busmin11140mV,
IRQunion54.736%,ADC49751 samples/3200period validated,stackuntouched2972.
Finaloff verified. Host initially omitted --dma-peer; rejected provenance after
successful motor window. Same capture revalidated with that flag PASS; no rerun.
Raw restorebinz_535_start61_hold69_10s_01.txt, SHA29753B70A577B6B7C17D6BD17CC8D2618BCCF8D472EFFC48D6FFAD0FD3AD420C.
INSTRUMENT_DISCIPLINE_RESPONSE E535 gives exact commands/limits. No recovery
injection, calibrated-current or independent rotor-speed claim. Currentbinz
driven startup works; AM32recentstartupfailures do not establish hardwarefault.
No firmware/guard change; actualBC876/off. Return main binz comparison work.

## Entry536 — 2026-09-14 — comparator call cadence A/B staged, no motor

Binz changeover-only experiment isn't isolated: measuredseed entersIRQ and
wrapper refuses old_routine return. No sharedcore edit. Instead staged optional
noinline StaticComp read (samevolatileload/polarity/readcount) versus inline.
Bothrelease-s/thinLTO/math-audited and frozen:6D82inline,20F5call. Baseline
15instruction loop; candidate realBL/helper singleCOMP2load verified. No new
recorder; two strict host marker testsPASS. Neither flashed/timed/qualified.
COMP_READ_CADENCE_EXPERIMENT.md exactcommands/hashes/gates/boundednextA-B.
ActualBC876 remains off; root6D82 differs. No motor/UART this entry.

## Entry537 — 2026-09-14 — read-call variant changes cadence, still faults

Disabled12read-span test added; no live instrumentation. Inline3..4us,
call8..9us, NOT fullISRtiming. Both rebuiltrelease-s/thinLTO,own disabledguard/
fullfive3200CPU2/6/9/ADCroutePASS. Inline7E55 one6.9%10sholdPASS333.638eHz.
CallDADD sameholdFAILED8.804s CycleTimingstep2 delta2846<2858,335.692eHz.
Rawpeak298/308,busmin10913/11128,IRQ51.525/51.880%,allfinaloff/ADCvalidated.
ThreehosttestsPASS. COMP_READ_CADENCE_EXPERIMENT E537 fulltable/hashes/limits.
No7%attempts orretries, no guardchange. Callvariant retired; restoredinline7E55
withguard3/18/finaloffPASS. Actual/root7E55/off, frozencompinline_537.

## Entry538 — 2026-09-14 — offline current evidence and count correction

Replayed E537 current sums, trigger-phase histogram and inline prestart
residual with existing host decoders. Inline49750 delivered/49751 phase;
call43802 delivered/43803 phase. Earlier comparison table used phase counts
as ADC-delivery counts; corrected, captures unchanged. E53549751/49751 valid.
Inline residual sum7.029252 rawcounts; calibration/stationarity/drift unproven,
no amps claim. COMP_READ_CADENCE_EXPERIMENT.md E538 retains exact quantities.
Source confirms2858us guard is same-sector accepted-cycle floor; inline min
2900us gives42us margin, not independent rotor-speed or CPU-ceiling evidence.
Requested operator PSU voltage/current/CV-CC observation at existing~334eHz
point; older~70mA observation was~238eHz. No powered run or hardware access,
no firmware/guard change, no new baseline-mode instrumentation.

## Entry539 — 2026-09-14 — archive parity and portable inventory refreshed

Verified frozen128-file minz source archive and all3 historical FALCON
summaries without live minz source/hardware. Hashes unchanged;5 archive tests
pass including corruption and sequence-gap cases. E485 three saved6.9%30s
recovery captures replay through current handoff verifier, hashes match,
recovery/timeline/finaloff all pass. No new runs or current-build qualification.
BEMF_PARITY_PLAN now distinguishes old9ACC recovery, current7E55 singlehold,
same-rig AM32 shortresponse, and minz per-window metrics. Negative reference
lockmap20_a9 with20.547667%qZC retained, not silently excluded or pooled.
PORTABLE_WINS adds E388–389 preparedcarrier and E374–378 boundedconstmath
candidates, with measured-scope limitations and noinlinecall retirement.
No flash/UART/motor, no new speedlimit, no goalcompletion. PSU observation
remains pending; archive mismatch is not a reference-reconnection blocker.

## Entry540 — 2026-09-14 — current inline recovery passes

Freshguard3faults/18refusals/off PASS. No flash/codechange; current7E55 one
30s6.9%BEMF campaign at20k,+60deg,6.1%driven entry passes Tracking injection,
freshseed recovery,original deadline and finaloff. Resumed27.991064s at
333.437206eHz,56000COM/55999accepted,sigma22.303722us,raw350,bus10925mV,
IRQunion50.659776%,stack2668. Seed1007ticks/acquisition6206us,remaining43us,
arm7us,deadline236usspare. ADC/phase139259 each decode,baseline revoked on
recovery so no calibrated-current claim. Oneattempt/onepass oncurrentbuild,
not repeatedcohort. No higherduty/profilechange; UARTclosed/finaloffverified.
COMP_READ_CADENCE_EXPERIMENT E540 exactcommand/hash/evidence. Capture
inline_540_start61_reentry69_30s_01.txt SHA256
330a144198aa37ec458db122e84afcafbc46051b28784ea6bd989101ca94e884.

## Entry541 — 2026-09-14 — inline6.9% recovery3/3 completed

Two predeclared additional30s attempts afterE540 bothPASS, no exclusions or
retries. Current7E55 unchanged; freshdisabledguard3/18PASS. Cohort3/3 at
333.437–333.579eHz, fullcycle sigma22.304–23.052us,IRQ50.660–51.119%,
busmin10829–10937mV,raw343–350. Freshremaining43us/arm7us allthree,
originaldeadline131–261usspare,stack2668. Newcaptures inline_541_start61_
reentry69_30s_02.txt and03.txt; exacthashes/table in COMP_READ_CADENCE_EXPERIMENT.
ADC/phase139259 and139261 decode; allrecovery/timeline/finaloff checksPASS,
UARTclosed. No code/flash/guardchange or speedincrease. Stopsamepointrepeats;
currentanchor andhigherboundarymechanism unresolved. Notfullgoalcompletion.

## Entry542 — 2026-09-14 — guard timestamp is not sole short-cycle cause

Read frozenminz ISR acceptance sequence and localAM32/binz timer adapters.
Coremask/read/reset/arm order matches; binz guard sampleslater atEV_ACC.
E537 fault replay guard2846us/reference2842.5us,closure+3.5us: bothbelow2858,
so retimestampingguard alonecannotresolve thisrefusal. Not anISRlatencybound
orphysicaloverspeed proof.15cyclefaulttestsPASS. COMstop/restart/pendingclear
differsfromAM32macro but no measuredcause; no timer/guardchange ornewprobe.
COMP_READ_CADENCE_EXPERIMENT E542 sourceanchors/limits. No hardwarecommands.

## Entry543 — 2026-09-14 — staged COM running-counter experiment

Optionalbench-com-keep-running omitsonlyinitialCENclear inCOMarm; retains
DIER/CNT/ARR/SR/NVICclear/finalCEN,init/stop. PostrunCOMARM/explicitfixtureflag;
2newhosttests+2regressionPASS. Release-s/thinLTO/codegen1 build andmathaudit,
emittedwrite sequenceverified. Frozen65D4 fullhash/artifacts/missingdisabled
timerqualification inCOM_ARM_EXPERIMENT.md. NOTflashed/timed/qualified;
actual7E55 lastoffE541,rootnowcandidate. No guards/sharedcore/filterchanges,
no liveinstrument orhardwarecommands. Not a demonstratedfix.

## Entry544 — 2026-09-14 — COM-arm candidate disabled checks and hold pass

Newidlecomarmcheck128timer casesPASS,maxarm1us,minslack31us; stopped/running/
staleUIF-NVIC/stoprearm withNVICmasked,nooutputauthority. Onehosttestactual+
malformedcasesPASS. Rebuiltrelease-s/thinLTO/mathaudited DB3B8AAD...2F9055,
frozencomkeep_544; safe7E55 then download/OpenOCDreset. Candidateguard3/18,
fullfive3200preflightCPU2/6/9,ADCroute3PASS. One6.9%10s plannedholdPASS:
334.776903eHz,20086COM/accepted,sigma40.395747us,mincycle2895us,raw307,
bus11020mV,IRQ51.295729%,COMP40/COM44/commit21/guard20us,stack3468.
ADC49750/phase49751 validated; finaloff/UARTclosed. Current/rootDB3B/OFF.
COM_ARM_EXPERIMENT E544 fullhashes/commands. No speed/dutyguard changes;
onehold notproof ofhigherboundaryfix or recoveryqualification.

## Entry545 — 2026-09-14 — COM keep-running does not remove boundary

DB3B freshguardPASS; one69/30srecoveryPASS333.873160eHz,56072COM/accepted,
resumed27.990739s,sigma22.184037us,raw344,bus11032,IRQ51.199225%,stack2676,
deadline206usspare. Thenone70/10s boundaryattempt FAILED CycleTiming12 at
2.041397s,step6 delta2853<2858. Referencecycle2857.5us/previous2983.5us,
closure-4.5us,notphysicalrotorspeed. Raw297,bus11140,IRQ51.458750%; allADC/
phase/finaloff validated. COM_ARM_EXPERIMENT E545 exacthashes/captures.
No retries or guardchanges; candidate RETIRED, no envelopegain. Restored
frozeninline7E55/download/OpenOCD;freshguard3/18/finaloffPASS,UARTclosed.
Actualboard7E55/OFF; rootDB3B stillcandidate and differs. Goalunfinished.

## Entry546 — 2026-09-14 — operator-observed supply anchor obtained

Qualified7E55 freshguardPASS,one6.9%30sholdPASS335.121757eHz,
60322COM/60321accepted,raw322,busmin10996mV,IRQ50.715748%,stack3460,
ADC149253/phase149254. Operatorduringrun:11.7V,about70mA,noCC/CVblips.
Thisnewreading resolves currentpointanchor,not reusedE188. Nominalbaseline
residual2.762374counts(~32mA)doesnotquantitativelymatch70mA; measurement
offset/samplinglimits remain, not a newmotorworkblocker. NoADCcalibrationclaim.
Finaloff/UARTclosed. psu_anchor_546_start61_hold69_30s_01.txt SHA256
00d38bde1aa400b3962ced3e5d0fef980f6bc48748318139555b1137344b10b4.

## Entry547 — 2026-09-14 — explicit cycle-envelope expansion passes7.0%

bench-cycle360 changesonlyrunningfullcyclefloor2858->2778; event238..1000,
seed350limits,32usarmfloor,electrical/deadline/tracking unchanged. Goalpermits
stagedspeedprofiles; predecessor3/3recovery+E546currentanchor justify bounded
test.33Rustguardtests+hostsyntheticprofiletestPASS,release-s/thinLTO/mathaudit.
Flashed09C3 aftersafing/OpenOCD,own guard3/18/fullfive3200CPU2/6/9/ADCroutePASS.
One70/10sPASS340.584838eHz,20435COM/20434accepted,minrecordedcycle2844us,
sigma41.961726us,raw371,bus10948mV,IRQ51.041439%,COMP40/COM45/commit21/
guard16us,ADC/phase49751,stack3460. Finaloff/UARTclosed,current/root09C3.
CYCLE_ENVELOPE_EXPERIMENT.md fullhashes/commands/qualificationlimits.
No automatic nextprofile; nextsamepointrecovery. Notallpriorfaultsharmless
proof andnotindependentqZC. Hardware/PSUwall notdemonstrated.

## Entry548 — 2026-09-14 — 7.0% tracking-loss recovery passes

Same09C3 cycle360profile, freshguard3/18PASS. One30s7.0%campaign completes
injectedTracking shutdown/freshseed recovery/originaltimeline/finaloff.
Resumed27.991285s,340.693223eHz,57219COM/57218accepted,sigma23.010795us,
mincycle2842us,raw321,bus10913mV,IRQ51.000767%,COMP40/COM45/commit21/
guard4us,ADC/phase139260,stack2668. Freshseed996ticks/acq6121us,
remaining41.5us/arm7us,originaldeadline200usspare; seed/armguardsunchanged.
No code/flash/limit change; current09C3/OFF/UARTclosed. Oneattempt/onepass,
notrepeatedcohort. CYCLE_ENVELOPE_EXPERIMENT E548 exacthashandcapture
cycle360_548_start61_reentry70_30s_01.txt.

## Entry549 — 2026-09-14 — 7.0% recovery3/3 and7.1% hold

Same09C3/fixed360profile, freshguard3/18PASS. Two predeclaredadditional70/
30srecoveries PASS =>3/3withE548,noexclusions. Resumed340.532..340.714eHz,
sigma23.011..23.313us,IRQ50.955..51.001%,remaining40..41.5us/arm7,deadlines
200..214usspare,raw321..354,bus10913..11032. AllADC/timeline/finaloffvalid.
Thenone71/10sPASS345.338272eHz,20720COM/20719accepted,mincycle2807us,
raw292,bus11008,IRQ51.067989%,ADC/phase49751,stack2668. No code/flash/
guardchange; current09C3/OFF/UARTclosed. CYCLE_ENVELOPE_EXPERIMENT E549
exacttable/captures/hashes. Next71recovery; no more70repeats orceilingshift.

## Entry550 — 2026-09-14 — 7.1% recovery pass,7.2% cycle refusal

Same09C3,guard3/18PASS.71/30s recoveryPASS344.928551eHz,57930COM/57929acc,
resumed27.991139s,sigma22.213760us,raw369,bus10948,IRQ50.919717%,ADC/phase
139259,stack2668. Freshremaining39us/arm7us,deadline208usspare. Onepass.
Thenone72/10sholdFAILED CycleTiming12 at8.627467s:step2 delta2761<2778,
ref2758.5us/prior2926.5us/twocyclemean2842.5us. Raw304,bus11044,IRQ51.366054%,
ADC/phase42922,stack2668,finaloffvalidated. Avg945 warns seed952floorcould
refuse72recovery,NOTtested. Bothcaptures/hashes inCYCLE_ENVELOPE_EXPERIMENT
E550. No retry/furtherduty/code/flash/guardchanges. Current09C3/OFF/UARTclosed.

## Entry551 — 2026-09-14 — offline speed-window counterfactual

Newhoststudy+4tests: E537/E545/E550 localreferenceonecyclebelowfloor,
twocyclemeanabove. Originalfailuresremainfailed; notwholerunreplay,not
independentrotor/lockproof. Synthetic100usboundaryjittertoleratedbytwocycle,
150ustrips;475->400us sustainedaccelerationtrips400uslaterinexample.
Fastgap/stalegap/order/harmoniccasesrefuse;silentlossrequiresindependentpoll.
CYCLE_ENVELOPE_EXPERIMENT E551 limits/nextpolicyproof. No firmwareorhardware
change,09C3lastoffE550. Fourtestsnotdeploymentqualification.

## Entry552 — 2026-09-14 — candidate window policy, quantified detection delay

Host-only Rust cycle_window_policy plus harness8testsPASS: wrapping clock,
single-cycle warmup, actual-window refusal/latching, existing tracking monitor.
121050 constant-spacing acceleration pairs show max extra detection2772us
(474->462us event spacing, old refusal36/new42), not merely E551's400us.
Conditional <=one-cycle extra-delay argument requires subsequent sliding cycles
to stay belowfloor; alternating cycles may pass the average indefinitely.
This changes protection semantics and is NOT a safe-deploy proof. Candidate
is not linked by firmware. Existing cycle360 guard33tests and offline study
4testsPASS. CYCLE_ENVELOPE_EXPERIMENT E552 documents assumptions/next review.
No flash/UART/motor/build change; installed09C3lastoffE550, no fresh off query.

## Entry553 — 2026-09-14 — 7.1% cohort fails initial acquisition once

Same09C3/root-frozenhash, freshguard3/18PASS. Two planned71/30s recovery runs:
02PASS325.442840eHz,54658COM/54657acc,27.991152s resumed,sigma29.161976us,
raw397,bus10984,IRQ50.425929%,ADC/phase139259,stack2668. Seed1047/age166/
remaining96ticks/arm7us;acquisition6528us,deadline220usspare,finaloffPASS.
03FAILED before handoff:seedfault3, epoch1at1453us ->epoch2at2524us gives
1071usgap=2142ticks>2000; retained TIM2 interval2142 corroborates it. Raw1060,
bus11271,DRIVEOBSreason2timeout20231us. CRC/acquisition/finaloff validated.
MissingRUNLIMIT fixtureerror is consequence of no powered segment, not proof
of profile mismatch. WithE550:2/3campaigns,not3/3 or repeatable345eHz point.
No retries/flash/guardchange;outputsOFF/Uartclosed. Exacthashes in experiment.

Host average-policy harness9testsPASS; counterexample admits1428us cycles
with valid order/gaps and5556us two-cycle sum.32us handoff floor is not applied
to ordinary COM arms. Average-only replacement not deployed; investigate
retained startup failure next, not blind floor/duty expansion.

## Entry554 — 2026-09-14 — fresh acquisition-window policy replay

Failed E553 gap decomposes809us command spacing+262us accept-offset change.
Not a1071us scheduler stall. Staged cfg-only with_timing_reanchor constructor
in driven_seed; livecaller unchanged. One shared restart, corroborated long
interval only, original deadline and12freshintervals/7cycles retained,
discardedfault3 retained. Actual E553 prefix replay: oldfault3/no seed,
candidate seed1543ticks at11785us.39RusttestsPASS including no secondrestart,
deadline/no corrupt-counter rescue. Source/commands/limitations in
CYCLE_ENVELOPE_EXPERIMENT E554. No targetbuild/flash/UART/motor;09C3lastoffE553.
Requires metadata/hostvalidator/live selection and timingqualification beforeuse.

## Entry555 — 2026-09-14 — explicit timing-reanchor build staged

Liveconstructor feature selection andSEEDTIMING postrunprovenance implemented;
hostrequires--seed-timing-reanchor, checksdiscardedcause/anchor/corroborating
TIM2/prefix/deadline.13seed+57drivenPython/39RusttestsPASS. Release-s/thinLTO
root/frozenseedtiming_555 SHA0fe48ab540f544aee2d269c62a50be0d1231c2a31c479377a8c7efceaa51cae0.
Emittedauditretained; resetbranchmemclr+backbranchnotyetbench-timed. No flash/
UART/motor. Actual09C3lastoffE553;rootcandidateDIFFERS. ExperimentE555 full
provenance/nextdisabledchecks. Not a powered startup/recovery qualification.

## Entry556 — 2026-09-14 — disabled seed reset timing refuses candidate

Added seedcheck/hostfixture,64isolatedresetsemantics plussecondgap refusal.
Built/auditedrelease-s/thinLTO E752a7c56ee425a2f6b22efb5fed7fd878ec73873e0447e76d8124ab2d4ffe02,
frozenseedtiming_556. Safed09C3guard3/18 then downloaded/OpenOCDreset.
seedtiming_556_disabled.txt:64casesfailed0,reset_max18us FAILpredeclared10us.
Finaloffverified/UARTclosed. No motor/fullpreflight. Actual/rootE752/OFF,
NOTpowered-qualified. Secondcalleroutlinesaccept inlivepath too; emitted
uidivmod0800908a, plusmemoryreset. Nextboundedremainder/codecost review,
not a timing-gate waiver. ExperimentE556 records scope and codegen change.

## Entry557 — 2026-09-14 — bounded sector math removes3us, gate still fails

driven_seed constsuccessor/aftergap exhaustivelycompiletimeverified;39Rusttests
PASS. Release-s/thinLTO audit shows no Qualification::accept arithmetichelper.
SafedE752guard3/18,download/OpenOCD newBF5f9527143c84c8e6a56e6cdb2a28662d4d3a4219ed245e4a6d4fc9efa83587.
Frozenseedtiming_557. Disabled64semanticsPASS,15us vs18,stillFAIL10usgate.
Finaloffverified/UARTclosed; no motor/fullpreflight. CurrentBF5/OFF/notpowered
qualified. Next proveinplacereset invalidation,notthresholdrelaxation.

## Entry558 — 2026-09-14 — reset invalidation costs13us, common anchor11us

Feature-only inplacereset keepsoriginalstart,clearsseen/validity/counters but
notdeadsectorhistory.216poisoned-state sequences againstfresh object;40tests
PASS. Variant750909... built/audited/flashedafterguard3/18:64semanticsPASS,
13usFAIL10. Thenremove recursive re-entry ->commonanchor,40testsPASS;
19b8e8301ea486ac4969d92eb6dd92817625b78fb5ee0e0822757e86561990e8 built/audited/
flashedafterfreshguard3/18:64semanticsPASS,11usSTILLFAIL10. Bothfrozen/raw
retained,finaloffverified/UARTclosed. Current19B8/OFF/notpoweredqualified.
No motor/fullpreflight/gatechange. CYCLE_ENVELOPE_EXPERIMENT E558 details.

## Entry559 — 2026-09-14 — duplicate check no timing gain, inline worse

Successor-check removal40hosttestsPASS. Built/audited4a26,guard3/18/download/
OpenOCD:64semanticsPASS,11usFAIL10. Thenfeatureinlineaccept builtDC0D,
guard3/18/download/OpenOCD:64semanticsPASS,12usFAIL10. Inline regresses;
do not carry as improvement. Current/rootdc0db63c9b63770cac3811d5caaeeed49387a48faed6d2052de78aa085ba410e,
frozenseedtiming_559b/OFF,notpoweredqualified. Bothfailures/finaloffretained,
UARTclosed,no motor. ExperimentE559:inspectactualtimedbracket/fullISRbudget,
not more speculative one-us variants or silently waived allocation.

## Entry560 — 2026-09-14 — live acquisition-duration refusal staged

Review found50us observerduration previously host-only; sectorlate50us already
live. Promotedobserver50us to immediate counter/COMP+intervalstop/false;caller
safesreason21 beforehandoff. DRIVENIRQBUDGET distinguishes overrun andhost
requireszero. No addedtimerread or relaxedguard. ExcludesouterIRQ/release,
notblackout/WCET proof.13seed+57drivenPythontestsPASS;release-s/thinLTOaudit
root4bc0c363d8242fbb8d1046b43027f56974581a0ab154abff7f1a0fa81c3dfcab,
frozenseedtiming_560 NOTflashed. ActualDC0DlastoffE559. Nextdisabledstoptest,
preflights thenseparatelyqualifiedfullpathmeasurement. Old10us tests remain
failed; no motor/UARTthisentry. ExperimentE560 detailedscope.

## Entry561 — 2026-09-14 — duration stop boundary and preflights pass

Disabledsharedfinish_duration tests0/49/50/51/65535 decision/TIM2/mask/counter,
5PASS. Firstbuildoverflow352;orchestrationdownloadedstale4BCroot erroneously,
reset/guard3/18offverified,no motor. Removed rejectedinline;newrelease-s/
thinLTO/auditedc7d0b31adacd6edfed04bafc5b35aefb1b9540d407a55d2fb14bb9397451827e
downloaded/reset,frozenseedtiming_561. Guard3/18/fullfive3200CPU2/6/9/ADCroute3
PASS,newhostboundaryregressionPASS. Finaloffverified/UARTclosed. No motor.
Next bounded69/10s wholehandlermeasurement withlive50usrefusal/strictseed/
alloriginalguards. Isolated10us test failures NOTreclassified. Discardedfault0
doesnotqualifyrestart. ExperimentE561 full rationale/provenance and nextplan.

## Entry562 — 2026-09-14 — actual handler23us;7.1% recovery3/3

SameC7D0,guard3/18PASS.69/10sholdPASS319.549eHz/19173COM/19172acc,
raw345/bus10984/IRQ50.928%,ADCphase49751,offverified. Thenfixedthree71/30s
recoveryattempts allPASS329.416/329.861/331.621eHz,originaldeadline182/183/
207usspare,seed1019/1021/1025,remaining89/89/90ticks,arm7us. Handler23usmax
all/overruns0,IRQ50.922..51.406%,allCRC/ADC/phase/timeline/finaloffPASS.
Thirdneededmissing-epochreanchor at3;discardedfault0all =>newlong-gaprestart
notexercised. Exacthashes/metricsCYCLE_ENVELOPE_EXPERIMENT E562. No retries/
flash/codechange;currentC7D0/OFF/UARTclosed. Nextbounded72/10s unchangedguards,
not more identicalcohorts. Isolated10us failuresremainfailed,notbranchWCETproof.

## Entry563 — 2026-09-14 — 7.2% hold/recovery and7.3% hold pass

SameC7D0,guard3/18PASS;startup61/phase60/20k/trace0/sameguards.
72hold10sPASS337.700eHz20262COM/20261acc,raw310/bus10984/IRQ51.912%.
72recovery30sPASS336.191eHz56462/56461,sigma25.878us,raw336/bus10937,
IRQ51.544%,seed1000/remaining84ticks/arm7,deadline146usspare.
73hold10sPASS338.904eHz20334/20333,raw318/bus10937/IRQ52.074%.
Allacqhandlermax23us/overruns0/discardedfault0,ADCphase/CRC/timeline/finaloff
PASS,Uartclosed. No retries/code/flash/guardchange,currentC7D0/OFF. Long-gap
restartunexercised. ExperimentE563 exacthashes/fullmetrics;next73recovery.

## Entry564 — 2026-09-14 — 7.4% hold works; startup blocks recovery attempt

SameC7D0/guard3/18.73recovery30sPASS340.004eHz57102COM/57101acc,
raw332/bus10948/IRQ51.459%,seed999/remaining42us/arm7/deadline256usspare.
74hold10sPASS346.242eHz20773/20773,raw323/bus10960/IRQ52.066%.
Then74recoveryFAILED INITIALseed:missingepoch2/reanchor3 thenlonginterval,
fault3at11intervals/6cycles,no poweredhandoff. Startupraw1121/bus11486,
handler23us/overrun0/discardedfault0,allCRC/finaloffvalidated. MissingRUNLIMIT
fixturemessage is consequence of no poweredsegment,not changedprofile.
No retries/flash/code/guardchanges;currentC7D0/OFF/Uartclosed. ExperimentE564
exactartifacts/hashes;nextentrywindow/deadlineanalysis,not75step.

## Entry567 — 2026-09-14 — Native MCP restored; compact summary installed

Installed D75568CCD5BF8B36BF3807DA26336E031562E6D1D82E2ED75849542A3FDF3D2E
(release opt-s/thin LTO, emitted math audit). No control/guard changes.
Native MCP disabled preflights: guard3/18, duration boundaries5, filter,
atomic, six PWM roles at3200ticks, CPU meter2/6/9us, archive3 and ADCroute3.
Raw transcript: captures/terminal_567_preflight.txt.

One quiet `run200`, acquisition6.1%, requested BEMF7%,10s window: FAILED
startup FeedbackStale after5947us.30retained scans, raw peak759 relative2048,
bus minimum11498mV. No BEMF transfer; requested7% was not applied. Direct
built-in ramp differs from historical run50+late UART ramp. Not a7% wall.
New summary correctly reports acquisition failure and omits stale previous
powered/core data. Raw transcript: captures/terminal_567_quiet70_attempt.txt.
Explicit off/p/i: all gates/ENABLE/MOE/CCRs0,nFAULT1; native MCP closed.
Successful BEMF summary not yet verified. Live throttle control still absent.

## Entry568 — 2026-09-14 — Disabled live-duty preload adapter

Added opt-in bench-live-duty-check PAC adapter. Const-prepared requests avoid
runtime division; TIM1 UDIS brackets all three CCR writes without EGR.UG or
CNT reset. No powered UART integration or owner metadata changes yet.
First045A image failed8/24: test compared COMP2 live VALUE bit30 as immutable
configuration. Retained captures/live_duty_568_disabled.txt. Corrected only
that status comparison;41c9c31fb111ef2656a416135e16ea7fb045050fabe6b5109742220664516653
built release-s/thinLTO, math audit passed, downloaded/reset successfully.
Disabled test24/24 PASS,max2us, six sectors with4/8/30/7% preloads each.
Timer continuity/configuration checks pass; CCR readback is PRELOAD, not
active shadow/pulse proof or powered WCET. No motor run on either image.
captures/live_duty_568b_disabled.txt retains result and final off/p/i:
all gates/ENABLE/MOE/CCRs0,nFAULT1. Native MCP closed. Guards unchanged.
Next integrate guarded owner metadata and opt-in foreground UART control.

## Entry569 — 2026-09-14 — Live-control image installed, disabled preflights

Added off cancellation of pending live-arm. Current/root029ff46fd77ade89d65b8e46091948fa0a9f711df486d98fb2b1bcd712dd783a
release-s/thinLTO build and emitted math audit PASS; download and OpenOCD
reset exit0. Native live1/live0 acknowledge. Disabled guard3/18, IRQbudget5,
filter, atomic256, sixroles at3200ticks/62duty, CPU2/6/9us, three archive and
three ADCphase checks reportPASS. captures/live_569_preflight.txt retained.
No motor. Final off/p/i verifies gates/en/MOE/CCRs0,nFAULT1; MCP closed.
Enabled duty writer and full callback timing remain unexercised; E568's2us
PAC-only result is not this guarded path's timing. No speed/guard change.

## Entry570 — 2026-09-14 — Motor baseline with live parsing armed

Same029F. python scripts/live_armed_baseline.py --out captures/live_570_armed70_baseline.txt
uses existing run50->200 pacing, acquisition61/+60deg, BEMF70/10s,20k.
One attempt PASS328.432937eHz,19705COM/19705accepted,10000035us observed,
deadline reason2. Cycle sigma45.494us is higher than recent references; no
equivalence or repeatability claim. Peak raw313, busmin10937mV,49751 ADCscans,
commitmax22us (no live writer called), acquisitionhandler24us/overruns0,
stackuntouched3440. No calibration-to-amps claim. Full handoff script replay
with --cycle360 --seed-timing-reanchor --dma-peer --carrier-hz20000 passes.
Capture SHA77F601C16575964DDE614DE9A5B12AD0978CC769CEF52153C330EF06A0F4A338.
ArmACK was checked at setup but wrapper did not retain it; fixed future
wrapper to save separate.arm.txt, not fabricated retroactively or rerun.
No UART bytes during BEMF, no duty change, no firmware/guard changes.
Final off/p/i verified alloff/nFAULT1; fixture closed UART. Live writer timing
and actual powered request remain open. This is one baseline, not recovery.

## Entry571 — 2026-09-14 — First live duty transition works

Same029F, one run via live_armed_baseline.py --change69, run50->200 startup,
initial70BEMF/10s, query at host7s then du69 at8s only after complete query
reply. D=0046 I=03F0 followed D=0045 I=03F9. Continued to original deadline:
10000036us observed,19410COM/19409accepted,reason2. Peakraw316,busmin10746mV,
49750ADCscans. commit_max22us includes the actual live writer's timed bracket;
it is not isolated writer WCET. Acquisition25us/overruns0,stack3440.
Aggregate323.514eHz,sigma52.592us span BOTH duties, not fixed-point quality.
Existing fullhandoff verifier passes CRC/ADC/timeline/finaloff; it does not
model dynamic duty, so no fixed-setting qualification inferred from its pass.
Capture live_571_change70_to69.txt SHA
F2A7BAEC82C085A136FBA36D20E1FB9A1B3E9BFBC514C6CF258F6C60DD109808;
separate.arm.txt retains livearmACK. No firmware/guard change or repeated
attempt. Final alloff/nFAULT1/UARTclosed. One functional write-and-continue
proof; stop command during live mode and upward changes remain untested.

## Entry572 — 2026-09-14 — Live upward change and stop

Same029F, one live_armed_baseline.py --up-stop run. Query70 at host7s, du69
at8s after full70ACK, du73 at9s after69ACK, off at11s after73ACK. Replies
D0046/I03F8,D0045/I03F5,D0049/I040C retained. HostAbort9 at6272113us of10s
powered budget,12435COM/12434accepted. Rawpeak318,busmin11008mV,ADC31204,
commitmax23us/veto0,stack3440. No electrical/tracking fault; stoplatency not
measured. Aggregate330.429eHz,sigma89.318us span duty changes, notsteadyquality.
Capture live_572_up_stop.txt SHA
2A72CCA379E9F93264814058611EF6B1EF23231F4999D6B449FA5EB34E76AE9B;
armACK separate. Acquisition/transfer/CRC/ADC/timeline/finaloff validation
PASS, expectedHostAbort checked rather than mislabeling a fullwindowpass.
Final gates/en/MOE/CCRs0,nFAULT1,Uartclosed. No firmware/guardchange or retry.
One up/down/stop functional test, not a repeatability or recovery cohort.

## Entry573 — 2026-09-14 — Live8% reaches existing cycle floor

Same029F; one live_armed_baseline.py --step80 run, query7% then du80 after
complete ACK. D0046/I03F6 thenD0050/I03EB. CycleTiming12 stops at3335122us,
6583COM/6582accepted. Rawpeak311,busmin10960mV,16592ADCscans,commitmax22us.
Existing2778us floor refused2755us step3; reference cycle2752.5us versus
prior2825us, guard/referenceclosure2.5us. Controlleraverage935half-us ticks
(~356.5eHz estimator), not independent rotor speed. Retained sector1 cycles
2893/2863/2846/2829/2790us trend towardfloor. No electricalfault or proof of
physical overspeed/desync; no sustained8% qualification. Aggregate328.968eHz
mixes duty/time and is not8% equilibrium. Cycle-fault context decoder validates.
Capture live_573_step80.txt SHA
04677CE28DD6B63D66F53AB4E2C6CF2A2AD8D46C301C99C0AE5A8D3B2D18369B plusarmACK.
CRC/ADC/timeline/acquisition/transfer/finaloff pass; allgates/en/MOE/CCRs0,
nFAULT1,Uartclosed. No repeat/flash/guardchange. Next review timing-envelope
evidence before altering the bound; do not treat this as a hardware wall.

## Entry767 — 2026-09-15 — Restart verifier fixed; count cap removed; restart still 1/3

The operator accepted board ADC current and bus voltage as the routine campaign
instrument after the PSU anchors. Current retains a conservative 10–15%
uncertainty (about 11% high at the 15% point); external PSU corroboration is now
reserved for disagreement, saturation/nonlinearity or a new regime.

The normal-restart fixture had a false-pass condition: firmware
`NORMALRESTART result=1` only means the second startup was launched. The host
now also requires a fresh successful transfer and `POWERPATH reason=2` at the
retained remaining deadline. It correctly accepts the prior genuine E765 pass
and rejects both retained failures.

The E765 feature set still hard-stopped driven acquisition on comparator call
65 in a 1 ms bucket, although the campaign goal classifies per-ISR count caps
as reports. E767 makes this peak report-only on the normal lean path. The
measured 50 us handler-overrun stop and independent command/feedback deadlines
remain. Release build/link audit passed; frozen ELF SHA256 is
`0B0C180FC0CFE6D6BF02759C674D96D4BB8BF208BB1211B6BC005A4BCF4CA82F`.
Disabled guard checks passed 3 timer faults and 18 post-stop refusals.

Three actual restart attempts were retained. Later forensic review corrected
the first failure attribution: with peak71/ms and handler max22us/zero
overruns, the second sine startup stopped on average/current reason4 at about
4.056s before its second powered handoff. The printed power reason8 was stale
state from the deliberately stopped first segment; it was not a second powered
tracking loss.
One stopped on signed-average current, reason25, 2116 us into powered control.
The third had peak65/ms, handler max22 us/zero overruns, and completed the full
22,980,005 us remaining powered deadline with reason2. Thus E767 is still only
1/3 reliable. Removing the count cap was causal and necessary for the pass,
but not sufficient; neither the current nor tracking protections were relaxed.
Next work is first-vs-second startup/handoff state and startup-current parity,
not another duty-envelope sweep.

## Entry770 — 2026-09-15 — Corrected 50 eHz startup makes ordinary restart 3/3

E768 first added an unambiguous completion marker and a one-second
outputs-disabled settle. E769 then fixed a real restart-state bug: the second
handoff had reverted the one-shot 6.1% acquisition / 7.0% BEMF / +60 degree
settings to defaults. Both images still failed intermittently in the initial
sine startup on average current.

The remaining mismatch was the autonomous startup schedule. It claimed to
reproduce the proven host staircase but started with a fixed 100 eHz catch,
then ramped backward toward 50 eHz before stepping to 200 eHz. E770 starts at
50 eHz when the staircase feature is selected. No protection threshold was
changed. Release SHA256:
`6DA455B3E090C3A9720245D9D0627DD9BF2594466D180E43587F8F7870AAB272`.
The disabled guard passed 3 timer faults and 18 post-stop refusals.

Three predeclared 30 s campaigns passed. Their deliberate first tracking
stops were at 2,000,905 / 2,000,905 / 2,001,005 us. Each held outputs disabled
for 1,000,000 us, completed ordinary sine startup, made a fresh powered BEMF
handoff with exact settings replayed, and ran the remaining
22,230,005 / 22,230,011 / 22,230,004 us to reason2. All final-off and strict
host checks passed. Run01 bus minimum was 11.748 V; no current, bus, nFAULT,
tracking or watchdog protection fired after restart.

This is an exploratory 3/3 ordinary-restart result, not higher-duty
qualification. The next useful work is integration with practical live-duty
exploration, not repeating this identical cohort.

## Entry775 — 2026-09-15 — 30% live envelope; 25% recovery 3/3

With the PSU ceiling raised to1.5A, an explicit 1.5A nominal average-current
build live-ramped 7→10→15→20→25→30% and held the full30s powered deadline.
The controller estimate was about1.18keHz and bus minimum11.748V. No electrical
or tracking protection fired.

Ordinary recovery now snapshots the last acknowledged live duty, performs the
proven low-duty startup/handoff, then restores through the existing guarded
writer in5% steps. A0.5s step period reached30% but later tripped average
current, so it was replaced by a2s period without changing any threshold.

Comparator suppression at high duty caused average-current reason25 before
the missing-event watchdog could declare tracking. That is correct protection
ordering and did not authorize restart. A separate immediate reason8 injection
therefore tests recovery policy without adding a stalled-sector current surge;
low-duty E770 already proved the real missing-event detector path.

Exact E775 SHA256
`2BD1C70CCEE48C95C5D58A409279ED5347E67F4FFECA8294C93DB329B4651C62`:
25% recovery passed3/3, each completing the remaining19.237–19.238s at
~1.15–1.18keHz with bus minimum11.737–11.904V. 30% recovery passed2/3 at
~1.34–1.37keHz; the retained third run restored30% then stopped on average
current reason25 at14.043567s. All exits verified off. Thus25% recovery is the
reliable representative point;30% is a demonstrated but current-margin-sensitive
exploratory ceiling, not a qualified recovery point.

## Entry 852-855 — 2026-09-16 — persistence histogram and hard bus-fold pause

E852 introduced a diagnostic-only per-sector persistence histogram. Its u16
counters saturated in a90s run and were replaced by native u32 counters in
E853. Emitted comparison against E850: ADC_COMP +52bytes and accepted recorder
+16bytes; four ISR-root arithmetic audits found no reachable soft helpers.

The matched result falsifies persistence rejection as the speed wall. Stable
10% (filter12) needed7.4..9.4 rejected passes per accepted crossing; stable29%
(filter6) needed1.2..2.8; stable31% (filter5) needed1.16..2.46. Rejections and
their sector fingerprint remain observable, but their normalized rate falls
as speed rises.

E854's explicit nominal4.0A estimator still folded once and settled at31% for
the full60s with no bus-low, nFAULT or tracking stop. E855 then made only this
uncalibrated signed-current excess report-only. The physical4A supply ceiling,
bus-sag, raw-validity, nFAULT, tracking, deadline and watchdog protections
remained active. The ramp toward50 stopped at36% after a complete50-scan bus
block below8.4V: reason25 at29.823280s,35 low-bus samples, cause5 bus743/
vref1506/vcal1662. The final36% persistence epoch was healthy at1.01..1.28
rejects/accept. Per operator instruction this is a hard supply/power-path fold
condition: powered testing pauses. Final off readback passed. Installed E855
SHA `3BD142AB79C8AEF2FC05C08458D3DE2B61B246E051BD9FF49771D28EBC1C4B30`.

## Entry 2026-09-20 — uninterrupted 30 s light-spin hold

Built and flashed a dedicated hold-length variant from the known-good reverse /
48 kHz feature closure. `bench-hold-30s` sets the startup hold to 30,000 ms
and extends the shell, driven-run, powered-guard, and waveform campaign limits
together; release-hybrid (opt-s, thin LTO, codegen-units=1) build succeeded.

At PSU limit 1.0 A, after `AVGNOMINAL accepted=1`, commanded `du100` then
`run200`: 10.0% duty, 200 eHz target. The motor ran continuously for the full
hold. Terminal telemetry: `energized_us=29998980`, `control_ticks=29502`,
`WAVETIMING updates=299990 max_gap_us=104 max_isr_us=6`, `DONE reason=1`,
`drive_records=256`, `coast_records=500`, with no current, bus-sag, nFAULT,
tracking, or watchdog stop. Operator observed approximately 550 mA, well below
the 1 A PSU limit. Final `p/i`: all six gates=0, en=0, MOE=0, CCRs=0,
nFAULT=1. UART was then closed and the debug runner stopped.

This is the requested fixed-setting uninterrupted 30-second light-spin result.
The flashed 30-second ELF SHA-256 was
`406B36EC4268E359FD8BEF551793656580E79CEEE26695C1E9C45BB73E477E12`
(2,669,096 bytes), built with `cargo build --profile release-hybrid
--example shell-pwm --no-default-features --features <qualified-closure>,bench-hold-30s`.
The qualified closure is the comma-separated feature list in
`captures/reference/reverse_48k_com_top_high_20260919/README.md`.

Replication limits: this run's lean post-run summary records the startup
parameters, elapsed energized time, waveform timing, drive/coast record counts,
and protection outcome, but it does not emit a full 30-second phase-current or
board-VBUS waveform. The operator's PSU observation was approximately 550 mA;
numeric phase peaks and VBUS minima require a separate `cap1`/instrumented
capture run.

## Entry 2026-09-20 — low-duty fixed-point recovery check

After physical inspection (no burn/short; phase resistances acceptable; nFAULT
high), the known-good reverse/48-kHz image was flashed and verified. PSU limit
was 1.0 A. A direct open-loop trial at 6.2% duty / 50 eHz stopped after about
1 ms on the firmware's `current ADC rail/peak` protection; outputs were then
off and nFAULT remained high. This point is a stall/current-spike point, not a
usable light-spin setting.

The next fixed point was 10.0% duty / 200 eHz (`du100`, `run200`). Six bounded
`run` windows completed normally, each reporting `energized_us` about
4,998,000--4,999,000, `DONE reason=1`, 256 drive records, and no current,
bus-sag, nFAULT, or tracking stop. Total energized time was approximately
30 seconds at the same setting (the firmware's `run` command is a 5-second
bounded window, so this is a six-window cohort rather than one uninterrupted
30-second hold). Final readback: all gates/en/MOE off, nFAULT=1.

This establishes a repeatable light-spin operating point at 10% / 200 eHz;
the earlier 6.2% / 50 eHz attempt was below the motor's usable torque/speed
point and was correctly caught by instantaneous-current protection.

## Entry 2026-09-20 — 750 mA PSU-limit repeat

With the same dedicated 30-second image and the PSU limit reduced to 750 mA,
repeated `AVGNOMINAL accepted=1`, `du100`, `run200`. The operator observed
approximately 550 mA during the run. It completed the uninterrupted hold:
`energized_us=29998976`, `control_ticks=29502`, `WAVETIMING updates=299990`,
`DONE reason=1`, `drive_records=256`, `coast_records=500`. No PSU foldback,
bus-sag, current, nFAULT, tracking, or watchdog stop occurred. Final readback
again showed all gates/en/MOE/CCRs off and nFAULT=1.

Therefore this light-spin point remains good with at least 200 mA of PSU-limit
headroom removed; the observed demand is below 750 mA.

## Capture completeness note — 2026-09-20

The 30-second control image includes a raw `Capture` record (phase IA/IB/IC,
VSENC, neutral, VBUS, VREF, frequency, PWM state and stage), but also enables
`bench-compact-qual`; at post-run it intentionally clears `dump_armed`, so a
`cap1` command does not emit the bulk A85 records. The attempted sibling build
with compact suppression removed overflowed G071 FLASH by 2752 bytes (and
`.data` by 3952 bytes). Therefore the successful low-duty runs have exact
startup/hold parameters and terminal timing/protection evidence, but not a
full raw phase-current/VBUS waveform or per-event BEMF acceptance trace.
Those require a separately size-reduced capture image or a compact streaming
summary (min/max phase ADC, VBUS/VREF extrema, stage counts and accepted-event
counter) before claiming complete handoff/lock characterization.

## Entry 2026-09-20 — descending low-duty sweep at 750 mA PSU limit

At the same 200 eHz target and 30,000 ms hold image, 7.0% duty completed
`energized_us=29998978` with `DONE reason=1`, 256 drive records, no current,
bus-sag, nFAULT, tracking or watchdog stop. The subsequent 6.2% run completed
`energized_us=29998449` with the same clean result. Finally 6.0% completed
`energized_us=29998886`, again with `DONE reason=1` and no protection trip.
Each run ended with gates/en/MOE off and nFAULT=1.

The lowest successful fixed point demonstrated in this sweep is therefore
**6.0% duty at 200 eHz for a continuous 30 seconds**, with the PSU limited to
750 mA. The earlier 6.2% / 50 eHz test remains a separate stalled low-speed
case and does not contradict the successful 200 eHz point.

## Entry 2026-09-20 — replication contract and compact capture build

Added [`LOW_DUTY_REPLICATION.md`](../../LOW_DUTY_REPLICATION.md), which records
the exact target, feature closure, build profile, hardware/UART setup, command
sequence, compiled startup profile, and the evidence required to distinguish
catch plus sustained BEMF lock from mere audible/mechanical spinning.

Added opt-in `bench-cap-summary`. When `cap1` is armed, it emits compact
`CAPSUMMARY`, `CAPMIN`, `CAPMAX`, and `CAPSTAGE` lines after shutdown, avoiding
the previous full-A85 flash overflow. The release-hybrid build succeeded:
text=128116, data=1200, bss=17160; ELF SHA-256
`C61768B55FF23652F80A08FB17BFA4765CEF4900EFACFD279CC188CCAE44CD17`.
This image has not been flashed or used for a powered run. The summary covers
the final 256 ms ring only; accepted-BEMF/commutation counts and full-run event
interval statistics are still required before claiming independent lock proof.

## Entry 2026-09-20 — first real low-duty BEMF handoff attempt

Built/flashed the compact handoff image (qualified closure plus
`bench-hold-30s`, `bench-cap-summary`, `bench-handoff-early`), SHA
`85286771E9430BC918A2DB28220CDB6A1BE20C18553BF4B1AEAEECE79D344457`,
text=128116/data=1200/bss=17160. At the known benign 6% / 200 eHz setting,
the exact sequence was `cap1`, `avgnominal`, `du60`, `driveobs1`, `drivex1`,
`run200`; the full transcript is retained in
`captures/handshake_6pct_200ehz_handoff_20260920.txt`.

This was the first run that actually exercised handoff. It reached
`DRIVETRANSFER result=1`, `fly_seeded=1`, and `POWERCOMMITS applied=29`, proving
the transfer path executed. It then stopped after 23.041 ms with
`TRACKSTOP event_fault=1`, `BEMFSTOP ... estimated_ehz=192 com=29
lock_proven=0`, reason 8. No current, bus, nFAULT, or fast-sag protection
tripped. Conclusion: the 30-second open-loop holds are benign and repeatable;
closed-loop catch executes but is not yet locked. Do not call the low-duty
campaign a BEMF-lock qualification until this tracking loss is resolved and
the accepted-event interval evidence is present.

A rebuilt compact image added `LOCKSUMMARY` counters without changing the
powered path (SHA `ED90A650BEA682A992418DF2B984124333851C686D772CD71E764E06C220125A`,
text=128596/data=1200/bss=17160). A repeat 6% handoff run reported
`accepted=36`, `commutations=37`, `DRIVETRANSFER result=1`, then
`TRACKSTOP event_fault=1` at `stop_us=30055`; `BEMFSTOP` still reports
`lock_proven=0`. The detailed moments recorder reported `stats_events=0` on
this live path, so accepted/commutation counters are currently the authoritative
lock-adjacent evidence and interval moments remain an instrumentation gap.

The next feature-scoped live-gap census rebuild (SHA
`664456EE30CBE27E963A86834FE217AAD6584033432FC59E905443FB173FB96E`,
text=128948/data=1208/bss=17176) reported on the same 6% handoff:
`accepted=32`, `commutations=33`, `LIVEGAPS n=31 min_us=513 max_us=964`.
It then stopped `TRACKSTOP event_fault=1` at 26.684 ms. The gap series is
plausible and below the 1 ms event-watch limit until the final missing edge;
the stop was not caused by current, bus, nFAULT, or fast-sag protection. This
is the first direct quantitative catch/edge-loss record.

A matched 7% / 200 eHz handoff run repeated the same behavior: transfer result
1 with 34 commutations, followed by `TRACKSTOP event_fault=1` at 27.261 ms;
no electrical protection tripped. Transcript:
`captures/handshake_7pct_200ehz_handoff_20260920.txt`. The 6% and 7% repeats
localize the next work to the handoff/closed-loop acceptance path rather than
the open-loop motor spin or bus-current margin.

After the repeat, outputs were verified off and both serial and debug sessions
were closed.

## Entry 2026-09-20 — handoff A/B with running-level revisit disabled

To separate physical comparator acceptance from the optional software level-revisit
rescue, rebuilt the same compact early-handoff image without
`bench-running-level-revisit`. Image SHA
`4D2C1262D753232886DA4984C9B5809D5936FDD385949685738CDA34CBDC844`
(text=128176/data=1208/bss=17160). The exact command sequence remained
`cap1`, `avgnominal`, `du60`, `driveobs1`, `drivex1`, `run200`.

Result: `DRIVETRANSFER result=1`, `LOCKSUMMARY accepted=28 commutations=29`,
`LIVEGAPS n=27 min_us=482 max_us=956`, then
`TRACKSTOP event_fault=1 stop_us=22790`; `BEMFSTOP` reported
`lock_proven=0`. No current, bus, nFAULT, or fast-sag protection fired.
The full-closure comparison had 32 accepted / 33 commutations and the same
stale-event termination. Removing level-revisit therefore did not remove the
handoff failure; it is not established as the initiating cause. Both runs are
replication evidence, not sustained-lock qualification.

## Correction 2026-09-20 — earlier 10–50% runs used a richer handoff setup

Review of the retained 10% and 50% transcripts found that those runs were not
the minimal `driveobs1`/`drivex1` recipe used by the later 6%/7% experiments.
They also issued `drivephase60`, `drivedu61`, `bemfdu100`, `drivepwm192`, and
an explicit `engagems` window (35 s at 10%; 55 s at 50%) before `run200`.
`bemfdu100` is especially important: it requests a 10.0% post-transfer BEMF
duty while the commanded startup point is about 6.2%.

The 50% transcript records `DRIVETRANSFER result=1`,
`TRACKSTOP event_fault=0`, `POWERPATH reason=2`, and 561,869 commutations;
the 10% transcript likewise reaches `TRACKSTOP event_fault=0` and a normal
deadline. Those are genuine BEMF-controlled runs. The recent 6%/7% failures
omitted these overrides and are therefore not equivalent tests. The low-duty
replication recipe now records the known-good override sequence explicitly.

## Entry 2026-09-20 — proper low-duty BEMF lock reproduced

Flashed frozen qualified ELF `0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0` and used the richer handoff sequence:
`off`, `avgnominal`, `engage0`, `drivephase60`, `drivedu61`, `bemfdu100`,
`drivepwm192`, `driveobs1`, `engagems35000`, `drivex1`, `cap1`, `du62`,
`catchdu62`, `live1`, `run200`.

This is a genuine low-duty closed-loop run. Startup commanded 6.2%; the
post-transfer BEMF duty override was 10.0%, below the 20% ceiling. Transfer
accepted with `fly_seeded=1`; the powered deadline completed at
`35000005us`. `TRACKSTOP event_fault=0` remained clean through shutdown,
`POWERCOMMITS applied=81157` matched `BEMFSTOP com=81157`, and estimated speed
was394eHz (`average_half_us=844`). No current foldback, phase rail, bus sag,
fast-bus, nFAULT, or tracking fault occurred. Final outputs were off.

The executable verifier reports:
`BEMF_LOCK PASS hold_ms=35000 accepted=81157 commutations=81157 last_event_us=34999847 evidence_mode=production`.
Full evidence is retained in `LOW_DUTY_LOCK_EVIDENCE_20260920.md`.
