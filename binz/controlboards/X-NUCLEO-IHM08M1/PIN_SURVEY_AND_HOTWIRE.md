# X-NUCLEO-IHM08M1 + NUCLEO-G071RB — pin survey & dev-rack hotwire recipe

**Goal:** an ESC dev rack that runs **AM32 with config only** (a committed board
definition, close cousin of the other g071 boards), ports 1:1 to **rm32**, and uses
**binz** as the bring-up/validation instrument from baby-steps to full envelope — with
**all** peripherals, ADC channels, and host instrumentation live.

**Verdict:** the X-NUCLEO-IHM08M1 is the right control board. It fixes the
EVLDRIVE102H's fatal flaw (low sides on non-TIM1 PD3/PD4 → no complementary 6-PWM),
and every required signal lands on a valid G071 pin. **All requirements are
satisfiable with lift-and-jumper moves only (no chip desoldering).** There is exactly
one forced move: the AM32-exact comparator owns PA2/PA3, which are the on-board
ST-Link VCOM — so VCOM relocates to PC10/PC11. Everything else stacks on top.

Sources: rendered IHM08M1 schematic (`x-nucleo-ihm08m1_schematic.pdf`, Fig 1–6),
UM1996, `um2324` (NUCLEO-64 morpho table), and rm32 source
(`rm32_stm32/src/mcu_g071/comp_init.rs`, `input_capture.rs`, `telemetry_uart.rs`,
`boards/*_g071.yaml`, `build.rs::g071_comp2_inm`).

---

## 1. The one hard constraint (silicon-forced)

AM32's / rm32's G071 sensorless uses **COMP2**, muxing one comparator across the three
floating phases. The G071 COMP2 input mux offers **only** these external pins
(`rm32_stm32/build.rs::g071_comp2_inm`, and `comp_init.rs`):

```
COMP2_INM (the 3 muxed phases)  ∈  { PB3, PB7, PA2 }   — no other options exist
COMP2_INP (neutral reference)    =  PA3
```

**PA2 + PA3 are the NUCLEO-G071RB on-board ST-Link VCOM (USART2 → COM7).** So
"AM32-exact comparator BEMF" and "on-board VCOM" are literally the same two pins.

Proof that no BEMF phase reaches the comparator on the *default* board routing (the
IHM08M1 sends BEMF1/2/3 to PC3/PB11/PB13, all ADC pins):

```
IHM08M1 BEMF default { PC3, PB11, PB13 }  ∩  COMP2-capable { PB3, PB7, PA2 }  =  ∅
```

→ Unmodified AM32 would drive perfectly but never *sense* BEMF. The hotwire below
moves the three BEMF nets onto PB3/PB7/PA2 so the comparator can see them, and
relocates VCOM off PA2/PA3.

Consequence: **on the M0/G071, "keep on-board COM7" and "AM32-exact comparator" cannot
share pins.** VCOM moves; it is not lost.

---

## 2. Full pin budget — everything lives

| Function | G071 pin(s) | Peripheral | Provided by | Move |
|---|---|---|---|---|
| Gate U high / low | PA8 / PA7 | TIM1_CH1 / CH1N | shield | none ✅ |
| Gate V high / low | PA9 / PB0 | TIM1_CH2 / CH2N | shield | none ✅ |
| Gate W high / low | PA10 / PB1 | TIM1_CH3 / CH3N | shield | none ✅ |
| Brake / overcurrent | PA6 (+ CPOUT PA12) | TIM1_BKIN | shield HW comparators (lmv331) | none ✅ |
| **Comparator BEMF (AM32)** | INM **PB3 / PB7 / PA2**, INP **PA3** | COMP2 | hotwire | 3 wires + 3-R star |
| **ADC-BEMF (binz witness)** | **PC3 / PB11 / PB13** | ADC | shield (keep R59/R60/R62) | none — parallel ✅ |
| Current, 3-shunt | **PA0 / PC1 / PC0** | ADC | shield, J5/J6 = 3Sh | jumper |
| Bus voltage | PA1 | ADC_IN1 (÷19.15, 169K/9.31K) | shield | none ✅ |
| Temperature (NTC) | PC2 | ADC | shield | none ✅ |
| Throttle input (DShot/PWM) | **PB4** | TIM3_CH1 | rm32 g071 default | free PB4 (§3.5) |
| KISS telemetry (AM32) | **PB6** | USART1 half-duplex | rm32 g071 default | none — PB6 free ✅ |
| **Bench VCOM (relocated)** | **PC10 / PC11** | USART3 | new | ext USB-TTL or ST-Link reroute (§3.7) |

**8 ADC channels live:** 3 current + bus + temp + 3 BEMF. The BEMF divider nodes feed
the comparator **and** the ADC pins at once (both are high-Z analog taps off the same
node), so binz gets a real-time ADC witness of the exact waveform the hardware
comparator is deciding on — hardware ZC and software ZC on one signal. Ideal for
validation.

---

## 3. Rework checklist (physical, ordered)

**Only forced trade:** AM32's COMP2 owns PA2+PA3 (silicon) = the Nucleo VCOM pins →
VCOM relocates to PC10/PC11. Everything else stacks cleanly.

**① Shield resistors — REMOVE**
- [ ] **R77** → frees **PB4** for TIM3_CH1 throttle/DShot. R77 is populated (routes
  PB4-PWM → CURRENT REF → U23/**CPOUT (PA12)** adjustable limit + J7 tap). Clean to pull:
  AM32's real OC is the *separate* per-phase path (U24/25/26: Vshunt vs **fixed** Vref
  R179/R180 ≈ 0.30 V → **BKIN/PA6**), untouched. Loose end: CURRENT REF then floats
  (R76 also N.M.) → CPOUT may chatter — harmless if **PA12 left unconfigured**. Timing:
  needed only before first real DShot, not before first spin (binz injects throttle by UART).

**② Shield resistors — LEAVE IN (don't touch)**
- **R59 / R60 / R62** — BEMF1/2/3 stay on ADC pins **PC3 / PB11 / PB13** (ADC-BEMF witness
  + extra ADC channels).
- **R81** — inert with JP3 open (10k pull-up gated OFF by JP3; 4k7 pull-down R34 is N.M.;
  only a 10pF cap + a reverse-biased BAT30 clamp remain). Leave it; the diode is free input
  protection. **Constraint: JP3 stays OPEN** (closing it biases the COMP2 node ~+200 mV).

**③ Add jumper wires — on the breakout header pins (short + twisted, away from FETs)**
- [ ] **PC3 (C7_37) → PB3** — BEMF1 → COMP2 INM IO1 (phase_a)
- [ ] **PB11 (C10_18) → PB7** — BEMF2 → COMP2 INM IO2 (phase_b)
- [ ] **PB13 (C10_11) → PA2** — BEMF3 → COMP2 INM IO3 (phase_c)
- *Tap the header pins, NOT the R59/60/62 SMD pads: R59/60/62 stay populated (0 Ω), so the
  breakout pin and the divider pad are the same net — the header is easier and robust.
  COMP2 runs hysteresis=none → keep leads short/twisted, off the gate area, or noise can
  throw false crossings (the ADC witness on the same pin will reveal it).*

**④ Add virtual-neutral star (3 resistors)**
- [ ] **3 × 47k 1%** — one leg each from **PC3, PB11, PB13** (same breakout pins) → a common node
- [ ] **common node → PA3** — COMP2 INP neutral (the star the ADC-sensing board omits)
- *47k, not 10k: the star loads each ~1.8k divider node; 47k attenuates the off-crossing
  BEMF deviation the comparator sees by ~4% vs ~15% at 10k (the ZC point itself is
  unshifted — symmetric loading). Preserves low-speed BEMF SNR.*

**⑤ Shield jumpers**
- [x] **JP3 — OPEN** (confirmed on-bench 2026-09-06)
- [ ] **JP1 — OPEN** (default; current-sense bias off, correct for single-shunt)
- [ ] **JP2 — set deliberately.** UM default = **CLOSED**; it modifies the current-sense
  op-amp **gain** (NOT a 6-step selector). Whatever you choose, **`millivolt_per_amp` in
  the board def must match that gain** — confirm the two gain values against Fig 5 first.
- [ ] **C5 mounted** (6-step; remove C3/C5/C7 only for FOC — ships 6-step-ready, verify).
- [ ] **J5 / J6 → 1-shunt** (AM32-exact; current on PA0). *Option: 3-shunt → +2 ADC current
  channels (PC1/PC0) for binz, but not AM32's native topology → make it a 2nd board-def variant.*
- [ ] **J9 — OPEN** + power per UM (bus >12 V safe)

**⑥ Nucleo — VCOM relocation (PA2/PA3 now comparator analog inputs)**
- [ ] **Cut the two ST-Link VCP solder bridges** on PA2 and PA3 (exact SB IDs from the
  G071RB UM). **MANDATORY on every VCOM path, external adapter included:** the ST-Link's
  VCP TX actively drives PA3 (= target USART2_RX = your COMP2 neutral). Left connected, its
  idle-high pins the neutral at 3.3 V and the comparator never crosses. Not hygiene — a hard cut.
- [ ] **VCOM on PC10/PC11 (USART3)** — external USB-TTL, *or* wire the ST-Link VCP TX/RX
  pads → PC10/PC11 to keep COM7.

---

## 4. Board-definition deltas ("config only")

### rm32 — `boards/x_nucleo_ihm08m1_g071.yaml` (cousin of `gen_64k_g071.yaml`)
```yaml
name: "X-NUCLEO-IHM08M1 G071"
mcu: stm32g071
dead_time: 60                 # tune to STL220N6F7 + L6398 (start ~AM32 default)
voltage_divider: <set for ÷19.15>   # 169K / 9.31K bus divider
millivolt_per_amp: <10 mΩ shunt × TSV994 gain>
current_offset: 0
current_adc_channel: 0        # PA0 = ADC_IN0 (Curr_fdbk1, single-shunt)
voltage_adc_channel: 1        # PA1 = ADC_IN1 (VBUS)
stall_protect_interval: 6500
min_bemf_counts: 3
has_led: true
led_pin: <free pin>           # NOT PA5 if reused; PB2 is the shield RED LED
bemf_pins:
  phase_a: PB3   # COMP2 INM IO1  — standard g071 set, unchanged
  phase_b: PB7   # COMP2 INM IO2
  phase_c: PA2   # COMP2 INM IO3
# INP neutral = PA3 (hardcoded in comp_init.rs) — provided by the 3-R star (§3.4)
# throttle input = TIM3_CH1 (PB4); KISS telem = USART1 (PB6) — g071 defaults
```
The `bemf_pins` are the **standard g071 COMP2 set** — no comparator code change, which
is the entire reason this board qualifies as "config only."

### AM32 — new `targets.h` entry
Inherit the G071 COMP2 / TIM1 config (PB3/PB7/PA2 + PA3 is exactly what AM32's g071
already does) and set the divider/shunt constants. A config target, not a source edit.

---

## 5. What each instrument gives you

- **AM32-exact switching + comparator BEMF** — config-only, the reference behavior to
  parity-check rm32 against.
- **binz ADC-BEMF, in parallel on the same nets** — software ZC alongside hardware ZC on
  one waveform; the validation oracle from baby-steps to envelope.
- **8 ADC channels** — 3-phase current (3-shunt), bus, temp, 3× BEMF.
- **KISS/EDT telemetry** on PB6 (USART1 HD) — AM32's own ESC telemetry, unmodified.
- **Bench VCOM** on PC10/PC11 (USART3) — binz's high-rate host pipe.
- **RTT** over SWD (PA13/PA14) — always available, untouched by any of the above.
- **Throttle** via TIM3_CH1 (PB4) or bench-UART injection.

---

## 6. Confirm-on-board checklist (byte-clean the last mile)

- [x] **JP3 OPEN** — confirmed open on-bench (2026-09-06). Keeps the Hall/encoder 10k pull-ups off PB3 (a closed JP3 would bias the COMP2 input ~+200 mV).
- [ ] R77 (PB4 current-ref-PWM) is N.M. → PB4 free for TIM3_CH1.
- [ ] PB7, PC10, PC11, PB6 not driven by the shield (absent from Fig 6 nets → expected free; verify with a meter).
- [ ] Exact NUCLEO-G071RB SB IDs for the PA2/PA3 ↔ ST-Link VCP bridges (only if doing the reroute).
- [ ] BEMF divider (÷5.5, 3V3-clamped) crosses COMP2's reference at half-bus at the rack's operating voltage (8–12 V here → in ADC range without heavy clamp); set INP/reference accordingly.
- [ ] Populate 3-R star (3 × 10 kΩ, BEMF1/2/3 → PA3) for the COMP2 neutral.
- [ ] J5/J6 shunt mode matches the board-def variant (1Sh for AM32-exact / 3Sh for max ADC).

---

## 7. Why not the alternatives

- **EVLDRIVE102H:** low sides on PD3/PD4 (non-TIM1) → no complementary 6-PWM; BEMF ADC-only
  with 2-of-3 comparator reach and no neutral. Fatal for AM32-style comparator sensorless.
- **NUCLEO-G431RB (M4):** the G4's flexible comparators *do* reach the BEMF pins, so
  AM32 comparator sensorless works with just a board-def and no BEMF hotwire — but it's
  Cortex-M4, not the M0 target. If the M0 constraint ever relaxes, the IHM08M1 + G431RB is
  the zero-hotwire combo.
