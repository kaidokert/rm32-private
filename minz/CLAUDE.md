# minz — bench prototyping crate

## Timer constraints (HARD RULE)

The rm32 firmware reserves several timers for motor control / DSHOT / PWM,
and bench prototypes in `minz/` **must not collide** with them. The full
inventory:

| Timer    | Status      | Used for                                          |
|----------|-------------|---------------------------------------------------|
| TIM1     | **OFF-LIMITS** | Motor PWM (TIM1_CH1/2/3 + complementary N pins)   |
| TIM2     | **OFF-LIMITS** | Interval timer / reserved in rm32                  |
| TIM6     | **OFF-LIMITS** | 20 kHz tick / `tenKhzRoutine`                      |
| TIM15    | **OFF-LIMITS** | DSHOT capture (TIM15_CH1 via PA2)                  |
| TIM16    | **OFF-LIMITS** | Commutation timing                                 |
| **SysTick** | available | Wall-clock tick (already used by `bitbang_uart`)  |
| **TIM7**    | available | Generic 16-bit timer                              |
| **LPTIM1**  | available | Low-power timer (also clockable from LSE/LSI)     |
| **LPTIM2**  | available | Low-power timer (separate clock domain)           |

If a prototype needs a periodic timer, pick from the "available" set above.
Never reach for TIM1/TIM2/TIM6/TIM15/TIM16 even temporarily, even in
examples — those are the firmware's hardware contract and the bench depends
on them being undisturbed.

## Physical connectors (Vimdrones ESC dev board v1.2)

Silkscreen → MCU pin mapping, from `minz/vimdrones_esc_development_board_v1.2.pdf`:

**J3 — 4-pin 2.54 mm header ("DEBUG / SIGNAL / TELEM"), left to right:**

| Silk | MCU pin | Function |
|------|---------|----------|
| S    | PA2 (via 22R) | TIM15_CH1 — DSHOT/servo signal in (bootloader pin) |
| TX1  | PB6 (via 22R) | USART1_TX — KISS telemetry pad |
| G    | GND     | |
| HSE  | **PA0** | schematic net `HSE_IN`; just PA0 broken out |

USB-TTL hookup for the bench tools (9600 8N1 both ways): adapter TX → **HSE**
(PA0 soft-UART RX), adapter RX → **TX1** (PB6), GND → **G**. S stays free.
For rm32 `debuguart` it's TX1 + G only, at 115200.

**J7 — 3-pin side breakout (WS2812):** 5V / D0 / G. D0 is **PB3** through an
SN74LVC1T45 level shifter (3.3 V → 5 V, output-only). `motor_tester`'s PB3
scope trigger therefore appears on D0 at 5 V — clip the scope trigger here,
no soldering needed.

**J4 — 6-pin SH1.0:** SWD (ST_SWDIO / ST_SWCLK + 3.3 V / GND).

## `examples/motor_tester2.rs` — current bench tool (2 Mbaud, hw RX on PA2)

Copy of `motor_tester.rs` with the comms path upgraded (June 2026). Use this
one; `motor_tester.rs` stays as the 9600-baud/soft-UART reference.

- **RX**: hardware **USART2 on PA2 = J3 `S` pin** via `CR2.SWAP` (PA2's AF7
  is USART2_TX; SWAP routes the receiver onto it, TE off, open-drain — we
  can never drive the line). Raw-register init: the HAL has no swap support
  and would demand PA3 (current sense). Replaces the PA0 soft-UART entirely —
  no more LPTIM1/EXTI0 ISRs (was a constant 4×baud sample-IRQ load).
- **TX**: USART1 PB6 as before, but **push-pull** (OTYPER flipped after HAL
  init — the HAL's half-duplex trait insists on open-drain, whose ~40 kΩ/2-3 µs
  rise caps the line at ~115200). TX-only line, so push-pull is safe.
- **Both directions 2,000,000 baud 8N1** — exact divisors on both sides
  (L431 BRR=80M/40, FT232R=3M×2/3). Verified byte-perfect both ways with the
  `u` blast key + `scripts/uart_blast_check.py` (64 KiB counting pattern).
  Sweep results: 115200 PASS, 921600 PASS, 2M PASS. 3M (FTDI ceiling) needs
  OVER8 + off-frequency divisors — tried, then dropped as not worth it.
- **New key `u`**: blast 64 KiB counting pattern (blocking ~0.35 s at 2M) for
  link verification/throughput. Wiring: FTDI TX→S, FTDI RX→TX1, GND→G; HSE
  (PA0) now free again.
- Everything else (keys, COMP2 pipeline, dumps, ADC cal) identical to
  `motor_tester.rs` below.

## MAGPIE window-record telemetry + MUSTANG baseline (July 2026)

motor_tester2 streams one **22-byte** binary record per float window
(`g` key toggles): sync 5A A5, seq, sector|zc-found flag, window start
(10 µs ticks, u32), window len (10 µs, u16), first-valid-ZC offset
(µs, u16, FFFF=none), raw + gate-surviving edge counts, and window
current min/max/mean (raw 12-bit, GECKO — see below). Per-sector
COMP2 mux is automatic (CHAMELEON, `o` toggles, AM32 changeCompInput
style) so all 6 windows per rev are observed. Frame layout lives in
`scripts/magpie.py`, shared by all host scripts.

### GECKO — PWM-synchronous continuous current sampling

`src/adc_sync.rs`: TIM1 OC4REF (falling edge at CNT=CCR4=250 ≈ 3.1 µs
into the cycle, inside the high-side ON window) → TRGO2 (CR2.MMS2,
raw-bits — PAC lacks the field) → ADC1 ch8 (PA3, INA180) hardware
trigger, EXTSEL=EXT10, one conversion per 24 kHz PWM cycle. No DMA,
no new IRQs: TIM1_UP_TIM16 reads DR at the cycle wrap and maintains
per-window sum/min/max. OVRMOD=1 so a slow reader can't stall it.
**vbat moved to the injected group** (`adc_sync::read_vbat_injected`,
JQDIS=1, software JADSTART) — the HAL `OneShot::read` must NOT be
called after `adc_sync::start` (it rewrites SQR/CFGR under the armed
trigger); `SenseAdc` is kept only for power-up/cal + `adc_to_mv`.
Verified: 0 mA at idle, ~480 mA avg / 564 mA peak per window at
f=50 amp=15, visible per-sector ripple, vbat sag 6.52→6.28 V under
load, tim1_up steady at 24.0 k/s.

Host scripts (all `scripts/`): `uart_cmd.py` (key driver),
`uart_stream.py` (capture + per-sector stats), `sweep_windows.py`
(f-sweep with closed-loop filter-state setting — the blank/edge keys
are RELATIVE, always set state by reading echoes), `plot_windows.py`
(PEACOCK 6-panel rinz-style plots). Captures land in `captures/`
(gitignored).

### Baseline (board 1, NO caps, amp=15, blank=20 µs, phys_ZC edges)

- **Raw config is gate-saturated everywhere**: with no filters the
  noise density is ~1 edge/10 µs, so "first edge after half-window
  gate" always fires within ~11 µs of the gate. Meaningless metric.
- **Filtered, EVEN sectors (rising-BEMF windows) decouple**: first-ZC
  sits +30..+90 µs past the gate with σ ≈ 50-60 µs, stable f=50→250.
  σ/window ≈ 2 % (f=50) → 8 % (f=250). These are usable ZCs — 3/rev.
- **Filtered, ODD sectors (falling-BEMF) stay noise-pinned** (+15 µs,
  σ 12 = gate echo). Falling windows are ~2× noisier in raw counts
  too. Needs persistence-run detection and/or the 4.7 nF caps.
- zc-found dips to 85-92 % at f=300-350 (open-loop slip region).
- Window-length sd spikes (87 µs at f=150 etc.) are TIM7 sector
  quantization — commutation is stepped by the 6 kHz TIM7 ISR, so
  sector timing granularity is 166 µs. Fine for observation; NOT
  fine as the commutation timebase of a closed loop (AM32 uses a
  hardware one-shot timer). Design input for the shadow-lock work.

Preliminary lock verdict: 3 clean ZCs/rev at σ ≤ 8 % of window on the
uncapped board — interval-averaging lock looks feasible even before
the caps; odd sectors are the improvement target.

### Overcurrent failsafe (firmware) + sweep guards (host)

After a stalled unattended sweep drew ~2 A until the user cut power:
- **Firmware**: TIM1_UP averages the 24 kHz current samples over 2048
  cycles (85 ms); >1.5 A avg (56 counts nominal — real trip may be
  ~2 A given the ±25 % sense-cal uncertainty) → ISR-level kill
  (`w`-key actions) + `!! OVERCURRENT TRIP` print; `r`/`q` re-arms.
  Validated by temporarily dropping the threshold below running
  current. Note yesterday's 2 A stall did NOT reproduce under
  supervision — suspect the damage window was the script dying
  without sending `w`.
- **Host** (`sweep_windows.py`): `try/finally` kill on every exit
  path + `--max-ma` (default 800) per-capture check that aborts the
  amp row.

### CONDOR map (hz × amp heatmaps, `scripts/plot_zc_map.py`)

`sweep_windows.py --amps a,b,c` runs the 2D grid (re-arms per amp
row); `plot_zc_map.py --tag X` renders the rinz-`zc_map`-style
4-panel PNG (load-angle proxy / per-sector spread / zc-found % /
current). First full map (board 1, no caps, blank=20, phys_ZC,
f=50-400 × amp=10-18): **low amp wins everywhere** — amp 10-12 rows
are pristine across the whole f range at 100-250 mA; the trouble
pocket (zc→84-89 %, spread→6-11°, ZC drifting late) is HIGH amp ×
f≥300. Sweet-spot shape matches the rinz finding. amp=20 row aborted
by the 800 mA script guard (811 mA at f=50, not a stall).

## OWL shadow-lock estimator (observe-only) — FALCON is GO

The COMP ISR now adds AM32's layer-2 **persistence qualification**: an
edge counts as *the* ZC only if VALUE holds the expected post-ZC level
(even sectors 1 / odd 0, textbook polarity) for 5 spaced reads
(~0.6 µs). TIM7 tracks qZC-to-qZC intervals (¾-smoothing), predicts
each commutation as qZC + interval/2, and streams qzc_off + pred_err
in MAGPIE frame v3 (26 B). `scripts/owl_report.py` prints the
per-sector table + FALCON gate (jitter <15 % of window, prediction
rate >60 %).

Results (board 1, NO caps, amp 15, blank=20, phys_ZC):
- **The persistence filter rescued the odd sectors completely**:
  qualified-ZC rate 100 % in ALL six sectors, qzc_off mid-window with
  σ ≈ 10-12 µs per sector, at f=100 through f=300.
- f=100: pred err −17±21 µs = 1.3 % of window → **GO**.
- f=200: 2.6 % → **GO**.
- f=300: per-window σ still ~15 µs (2.7 %), but between-sector bias
  spread (+122 µs on sec 5 etc.) blows the aggregate to 21 % — that's
  the open-loop slip beat itself, i.e. the thing closing the loop
  removes, not a sensing failure.

Verdict: sensorless lock is supported on the uncapped board across
the usable band. FALCON's remaining work is commutation timing (a
hardware one-shot — LPTIM2 — instead of TIM7's 166 µs-quantized
stepping) + handoff/desync fallback.

## WAXWING waveform scope (`j` key + `scripts/waxwing.py`)

rinz-grade latest_zc view on this board: DMA1_CH1 fills a 2048-frame
ring (85 ms) with per-PWM-cycle ADC triplets ch9/PA4=A, ch10/PA5=B,
ch8=current (LAST — the cycle-wrap harvest and failsafe read it from
the ring, never from DR: a CPU DR read races the DMA request and
shifts all later words a channel). `j` freezes (ADSTP), dumps rinz
cdump/Ascii85 + a status channel (COMP bit|sector), resumes aligned.
`waxwing.py` renders 3 panels: A/B analog waveforms + neutral +
sign-change/linfit ZCs + comparator edges + current; C panel is the
COMP bit (PB7 has no ADC route — the comparator IS channel C).

**Hard-won timing rule**: the whole ADC sequence must finish inside
the PWM ON window or late channels read low-side recirculation (~0 V
on every terminal). Trigger at CCR4=100 (1.25 µs, AM32's point) +
47.5-cycle sampling → done by ~3.3 µs, OK down to amp ≈ 8. (First
attempt: trigger 250 + 247.5-cycle sampling put ch10 at ~6.4 µs, past
the 6.25 µs ON window at amp 15 → phase B read ≈0 everywhere.)

Verified at f=100/amp=15: textbook plateaus + float-window BEMF arcs
on both phases, comparator edges landing on the analog crossings.

Bench recovery note: SWD flaked mid-erase once → half-erased image →
core LOCKUP; NRST is NOT wired to the probe, so recovery = board
powercycle + ST-LINK USB replug + `scripts/flash_watch.ps1` (retries
erase/download/reset until the probe answers). Consider wiring NRST.

## `examples/motor_tester.rs` — previous bench tool (9600, soft-UART)

Open-loop motor spinner with live UART control. Pins on the Vimdrones L431:

| Subsystem | Pin(s)    | Notes                                                      |
|-----------|-----------|------------------------------------------------------------|
| Soft-UART RX (host → ESC) | PA0       | LPTIM1 + EXTI0, 9600 8N1                  |
| USART1 TX (ESC → host)    | PB6       | **Half-duplex** (open-drain AF7 + pull-up) so PB7 stays free for COMP |
| Motor PWM      | PA7..PA10, PB0, PB1 | TIM1 CH1/2/3 + CH1N/2N/3N, AM32 pin map     |
| Battery sense  | PA6       | ADC1_IN11, oneshot read on `i` key                         |
| Current sense  | PA3       | ADC1_IN8, oneshot read on `i` key                          |
| BEMF comp INP+ | PB4       | COMP2 IO1 (virtual neutral)                                |
| BEMF comp INM− | PA4 / PA5 / PB7 | COMP2 IO4/IO5/IO2; switched by `p` key. Textbook-convention labels: A=PA4 (floats sec 2,5), B=PA5 (sec 1,4), C=PB7 (sec 0,3). |

Keys: `d/c/f/v` freq, `a/A/z/S/x` amplitude, `m` sine↔six-step, `r` reset, `w` kill MOE, `i` ADC, `b` COMP2 rate (raw / valid / filt), `p` cycle observed phase A→B→C.

Sense calibration (schematic-derived, single-point bench delta ≤25 % — likely a mix of resistor tol / INA gain variant / PSU readout accuracy):
- `VBAT_DIVIDER_X100 = 933` — 3.6 k / 30 k divider, ratio (30+3.6)/3.6 ≈ 9.33×.
- `ISNS_MV_PER_AMP = 30` — INA180**1** (gain 20 V/V) × 1.5 mΩ shunt.

ADC config worth knowing: `set_sample_time(Cycles640_5)` is essential — HAL default `Cycles2_5` is too short for the ~3.2 kΩ source impedance of the vbat divider; vbat reads ~8 % low until you bump it. `set_resolution` is a no-op (default 12-bit). Calibration runs once inside `ADC::new`; no need to re-run.

## COMP2 BEMF detection — working pipeline

The bench tool extracts real BEMF zero-crossings at AM32-equivalent quality. Final hardware + software pipeline below, plus the validation showing it scales linearly with motor frequency.

### Register state (`comp2::init`)

Matches AM32's `MX_COMP2_Init` + `LL_COMP_Enable` byte-for-byte:

| Field    | Value | Notes                                                  |
|----------|-------|--------------------------------------------------------|
| EN       | 1     | enabled                                                |
| PWRMODE  | 00    | high-speed, ~80 ns propagation                         |
| INMSEL   | 111   | extended select (IO2/IO4/IO5 via INMESEL)              |
| INPSEL   | 0     | IO1 = PB4 (virtual neutral from board R-network)       |
| INMESEL  | varies | 10=PA4 (A) / 11=PA5 (B) / 00=PB7 (C); set by `set_observed_phase` (textbook convention) |
| POLARITY | 0     | non-inverted (VALUE=1 when INP > INM)                  |
| **HYST** | **00** | **none** — AM32 uses zero hardware hysteresis; all noise rejection is in software |
| BLANKING | 000   | not used. AM32 doesn't use TIM-coupled hardware blanking either. |
| BRGEN/SCALEN | 0 | no internal VREFINT scaler                             |
| WINMODE  | 0     | (in COMP12_CSR — reset value, no write needed)         |

Net CSR value after init (observing phase A): **0x00000071**.

### Float-phase isolation (`tim1_motor_pwm::set_six_step` + `set_phase_pin_modes`)

The single biggest discovery on this branch: **CCER=0 + OSSR=0 does NOT cleanly float a phase on this hardware**. The TIM1 channel does release its output, but the MCU pin in AF mode without a driver is electrically Hi-Z — the gate driver IC then sees an undriven input, picks up capacitive coupling from neighbouring PWM traces, and ends up partially conducting the FETs. The "floating" terminal isn't truly floating.

AM32's `phaseAFLOAT` (`AM32/Mcu/l431/Src/phaseouts.c:150-158`) does it differently:

1. GPIO `MODER` of the LOW-side pin → 01 (general-purpose OUTPUT)
2. GPIO `BRR` ← LOW-pin mask (drive 0 V)
3. GPIO `MODER` of the HIGH-side pin → 01
4. GPIO `BRR` ← HIGH-pin mask

TIM1's `CCER` stays all-enabled throughout — the channel keeps toggling internally; the MODER override just isolates it from the pad. The gate driver IC now sees a clean 0 V on both H and L inputs and holds both FETs cleanly OFF.

`tim1_motor_pwm::set_six_step` mirrors this exactly: per-call, the floating phase's pins go to OUTPUT-LOW via MODER + BSRR.BR; active-phase pins stay in ALTERNATE. CCER is set once in `init()` to all-enabled and not touched again. The whole sequence is wrapped in `cortex_m::interrupt::free` like AM32's `__disable_irq()` / `__enable_irq()` envelope.

### The 3-layer software filter (`motor_tester.rs::COMP` ISR + main-loop edge tracking)

Once the float is clean, the comparator output still toggles many times per float window from PWM-coupling on the shared virtual neutral PB4. Three software layers in the ISR isolate the real BEMF crossing:

| Layer | Where | Mechanism |
|-------|-------|-----------|
| **1. Time gate** | ISR head | Discard edges where `TICKS_10US - SECTOR_START_TICK < SECTOR_HALF_TICKS`. Suppresses early-sector PWM transients (first ~30 ° of the 60 ° float window). |
| **2. Direction-aware persistence** | ISR body | Tight loop of `FILTER_LEVEL=5` samples; bail if any sample ≠ `EXPECTED_POST_ZC`. Matches AM32's `for(i<filter_level) if (comp == rising) return;` (`main.c:915-923`). |
| **3. Mask-after-accept** | ISR tail | After a valid edge, call `comp2::set_exti_enabled(false)`. Main loop re-enables it only on the *next* float-sector entry. Caps `filt` at one ZC per float window. |

`EXPECTED_POST_ZC` follows AM32's `rising = !step.is_multiple_of(2)` pattern: even sectors → rising BEMF → post-ZC level = 0; odd sectors → falling → post-ZC = 1.

### Validated BEMF rate vs frequency

`filt` measured at amp=15, phase A (PB7) observed:

| f (Hz) | filt (events/s) | expected (2·f) | match  |
|--------|-----------------|----------------|--------|
| 70     | 137             | 140            | 98 %   |
| 130    | 296             | 260            | 114 %  |
| 200    | 393             | 400            | 98 %   |
| 300    | 536             | 600            | 89 %   |
| 410    | 666             | 820            | 81 %   |

Linear with rotor speed at low f. Above f≈300 the rotor starts slipping the commanded field (open-loop V/f limitation, not a sensing issue) — `filt` is genuinely measuring the *actual* rotor's electrical rate, which lags the commanded one once we run out of torque margin.

### Phase mapping (subtle, easy to get wrong)

The bench tester uses the **textbook 6-step BLDC labeling**, which differs from the Vimdrones board silkscreen by an A↔C swap. Standard convention: phase A floats at sectors 2 and 5 (ZC at 150° / 330°), B at 1 and 4 (90° / 270°), C at 0 and 3 (30° / 210°). The board's silkscreen labels PB7 as "Phase A" but that pin physically floats at sectors 0 and 3 — i.e. it's *phase C* in the textbook convention.

The internal HIGH/LOW table in `tim1_motor_pwm::set_six_step` indexes 0/1/2 as TIM1_CH1/CH2/CH3, mapping to motor terminals 3/2/1 on the Vimdrones board. The motor wiring isn't changed — only the *labels in `comp2::ObservedPhase`* are rotated so they match the textbook convention.

The full mapping in `motor_tester.rs`:

```rust
let (s0, s1) = match observed_phase {
    ObservedPhase::A => (2u8, 5u8),   // PA4 = CH1, ZCs at 150°/330°
    ObservedPhase::B => (1, 4),        // PA5 = CH2, ZCs at  90°/270°
    ObservedPhase::C => (0, 3),        // PB7 = CH3, ZCs at  30°/210°
};
```

`EXPECTED_POST_ZC` in `TIM7` follows from this: even sectors (0/2/4) are *falling* ZCs (post-ZC level = 1), odd sectors (1/3/5) are *rising* (post-ZC level = 0). With INP+ = neutral, INM− = BEMF and POLARITY=0, COMP=1 means BEMF is below neutral.

### `comp2::set_observed_phase(ObservedPhase)`

Live-switches COMP2's INM− between PA4 (A) / PA5 (B) / PB7 (C). Uses `.modify()` so HYST / EN / POLARITY / INP are preserved. Caller masks EXTI around the switch and resets the rate-window state to avoid contaminating the next 1-s sample.

## How we got here (diagnostic chronology, mostly for posterity)

The investigation took several dead-end turns. Compressed log so future-us doesn't re-walk them:

- **Initial setup (`HYST=0`, no gating, both edges)**: ~400 k/s flat-vs-f → hardly any signal. Concluded PWM-edge ringing dominates.
- **`HYST=0b11` (max, ~22 mV)**: ~65 k/s flat-vs-f. Cut ringing per edge but didn't reveal speed-dependent signal.
- **Per-sector EXTI gating to float windows only**: ~24 k/s for phase A, ~16 k/s for B/C. The 1/3-of-revtime ratio matched gate duty, suggesting noise density was uniform across the float window — actually wrong; the right read was "the float wasn't clean."
- **Phase-switch experiment**: ruled out a global mapping bug (asymmetry too small to be wrong-phase, ratio was ~1.5× not 3-5×).
- **Sector-boundary guard band (FLOAT_GUARD_STEPS=5)**: cut counts by 17 % uniformly → eliminated transition-glitch as the asymmetry source.
- **Phase A↔C label inversion fix**: discovered our HIGH/LOW indices were inverted relative to rm32's `PhaseDriver` template. With the fix, all three phases gave comparable counts (~15-25 k).
- **GPIO MODER float (the key fix)**: realised TIM1 CCER+OSSR=0 puts the pin in Hi-Z, not the AM32-style OUTPUT-LOW, which lets the gate driver IC behave unpredictably. Implementing AM32's `phaseAFLOAT` halved the noise floor; valid/raw ratio jumped from ~50 % to ~70 % (events finally clustering in the late half of the sector = consistent with real BEMF dwelling there).
- **AM32-style time gate** (this layer): reduced count proportional to gate duty as expected.
- **Persistence filter "all samples agree" version**: filt ≈ valid (~95 %); we were just confirming "PWM-coupled DC-shifted edges are stable transitions" — they all passed because each individual transition is monotonic.
- **Direction-aware persistence + mask-after-accept** (the closing layer): `filt` finally drops to BEMF rate and scales linearly with f. Pipeline complete.

### Now-understood causes (replaces the earlier "verified non-causes" list, several of which were wrong)

- The phase mapping was *partly wrong* between our internal TIM1-channel-indexed names and the board's YAML-labelled BEMF pins. Fixed by the A↔C swap in the observed-phase → float-sectors table.
- The "MCU pin Hi-Z = floating" assumption was wrong on this hardware. The gate driver doesn't tolerate it; needs OUTPUT-LOW driven by the MCU.
- Hardware hysteresis turned out not to be the right knob. AM32 runs with HYST=00 and so do we.
- Hardware blanking via TIM1 OC also turned out not to be it. AM32 doesn't use it. All noise rejection is in software.

## TIM15 hardware blanking — investigated and removed (May 2026)

Briefly: L431 COMP2 has a documented `BLANKING` field (`COMP2_CSR[20:18]`) that lists `0b100` as "TIM15 OC1 selected as blanking source" (RM0394 22.7.3). We wired it up to see if it would suppress PWM-edge ringing for free. **It does — but only on `COMP_CSR.VALUE`, not on the EXTI line**, which makes it useless for reducing the COMP IRQ rate. We then removed the whole thing in favour of software blanking via a `TIM1_CC` ISR. Worth recording the experiment because the manual sounds like the feature works the way you'd expect, and it doesn't.

### What we built (referenced by git ≤ `46aa27c "Blanked windo"`)

- `minz/src/tim15_blank.rs` — TIM15 in slave-reset mode driven by TIM1's TRGO (= update event), so `TIM15.CNT` re-zeros at every PWM cycle wrap. CH1 in PWM mode 1 with `CCR1 = N` → OC1 HIGH while `CNT < N`, LOW after. `TIM15.SMCR` (offset `0x08`) is `_reserved2` in stm32-rs's SVD so we wrote it via a raw `0x4001_4008` volatile pointer.
- `comp2::init` set `COMP2_CSR.BLANKING = 0b100` (TIM15 OC1) at boot. Per RM 19.3.7, "the **complement** of the blanking signal is ANDed with the comparator output to provide the wanted comparator output" — so OC1 HIGH = comparator gated, which matched `CC1P=0` + PWM mode 1.
- Live-tuning of `CCR1` via the `n` key cycled 0/64/256/1024/2048/3000 ticks (= 0/0.8/3.2/12.8/25.6/37.5 µs of blanking width).

### What we observed

- **Comp IRQ rate flat at ~63-67 kHz regardless of `CCR1` value (0 → 3000 ticks).** Sweeping `BLANKING` field 0..7 also had no effect (the RM only documents `0b100`, but we checked the other encodings too).
- **L-dump (samples `comp2::value()` once per PWM cycle from `TIM1_UP_TIM16`) goes solid `.` at `CCR1 ≥ 256`.** Every cell, every sector, every phase. The VALUE bit is fully gated during the blanking window.
- Forum thread on G431 (linked from the work log) confirmed the split: "blanking only masks the comparator output during the blanking window; it does not disable the comparator itself." On L4 the same applies, and EXTI is wired to the **raw** comparator core, before the blanking AND-gate. The VALUE bit gets gated; the EXTI line doesn't.

### Why we removed it instead of using it

1. **Doesn't suppress COMP IRQs.** The expensive thing about PWM-edge ringing is the ISR storm — we wanted blanking to reduce ISR load, and it doesn't.
2. **Duty-dependent.** TIM15's slave-reset is tied to TIM1's update event (= PWM cycle wrap = rising edge of the high-side FET drive). The OC1 pulse therefore covers the *start* of each PWM cycle, regardless of where the falling edge is. As duty grows, the actual PWM falling edge drifts later in the cycle, away from the blanking window. To track the falling edge you'd have to dynamically reprogram `CCR1` per sector — at which point you might as well do the suppression in software.
3. **Not portable to real-ESC code.** The L431 firmware (rm32) reserves TIM15 for DSHOT capture. The blanking feature only existed on this bench because minz can use TIM15 freely; production code can't.

### Replacement (currently shipping on this branch)

`TIM1` CC1IE/CC2IE/CC3IE are enabled. The `TIM1_CC` ISR latches `ticks_10us()` into `LAST_PWM_EDGE` on every PWM-channel compare match (i.e. every PWM transition, across all driven channels — duty-independent by construction). The `COMP` ISR checks `now - LAST_PWM_EDGE < BLANK_TICKS_10US` at entry and skips the EDGE_BUF / SECTOR_EDGE_COUNT writes if inside the window. `BLANK_TICKS_10US` is live-tunable via `n` (+1) / `N` (-1) keys; `.` / `,` are reserved for ±10 µs coarse steps once SysTick output is bumped from 10 µs to 1 µs resolution via the `systick-timer` crate.

The same logic translates cleanly to real-ESC code: any timer that can interrupt on each PWM transition works as the timestamp source, and the COMP ISR check is one subtract + compare.

## Flash layout — bootloader bypass (May 2026)

`memory.x` originally had `FLASH : ORIGIN = 0x08001000` to sit after the AM32
bootloader (`AM32_L431_BOOTLOADER_PA2_V18` at `0x08000000`). This required the
bootloader to perform a second-chance jump on every flash/reset, which was
unreliable: the bootloader gates its first-chance jump on `RCC_CSR.SFTRSTF == 0`,
but `probe-rs run` always soft-resets the chip (setting SFTRSTF), so that path
was permanently blocked. The second-chance (20 ms TIM2 timeout with no signal on
PA2) also failed when BF was driving DSHOT, keeping `invalid_command` below 101.

**Fix**: `memory.x` now reads:

```
FLASH : ORIGIN = 0x08000000, LENGTH = 64K
RAM   : ORIGIN = 0x20000000, LENGTH = 48K
```

The first `probe-rs run` after this change overwrites the bootloader. To restore
the bootloader: re-flash `AM32_L431_BOOTLOADER_PA2_V18.hex` separately via
STM32CubeProgrammer. For bench-only minz work the bootloader is not needed.

## `examples/chip_diag.rs` — bare-metal register dumper

Skips `board_init::init()` entirely; runs on the default MSI 4 MHz reset clock.
Calls only `minz::panic::ensure_rtt()` then dumps RCC, GPIOA, GPIOB, TIM1,
COMP2, NVIC IPR bytes and SCB AIRCR via a raw `r32(addr)` helper, then loops
printing "alive N" every ~400k nops (~0.9 s at 4 MHz).

Useful for verifying chip state without any HAL clock setup or ISRs running.
Note: GPIOA/B registers read garbage if AHB2ENR is 0 (clocks off) — that's
expected and harmless for this diagnostic.

## PB3 scope trigger

`motor_tester.rs` configures **PB3 as a push-pull output** (GPIO output, no
alternate function). The TIM7 ISR toggles it on every `rev_wrapped` event
(sector 5→0 transition of the commanded electrical cycle), producing a square
wave at exactly the commanded electrical frequency F. Wire PB3 to the external
trigger input of the scope.

Implementation: `static PB3_LEVEL: AtomicBool` tracks state; `fetch_not` + BSRR
write in TIM7 on `rev_wrapped`. The toggle fires only when the motor is armed
(motor-disabled path returns before `rev_wrapped` check).

## Edge-buffer freeze (`E` key)

`static EDGE_DUMP_FREEZE: AtomicBool` — when set, TIM7 skips `ACTIVE_HALF`
flips and `SECTOR_BOUNDARIES` updates so the frozen half stays intact. Motor
keeps running in either state.

- **First `E` press**: sets freeze, immediately triggers a dump of the current
  frozen half. Header line includes `, FROZEN`.
- **Subsequent `e` presses** while frozen: re-dump the same snapshot.
- **Second `E` press**: clears freeze, resumes normal half-flipping.

## BEMF hardware noise — schematic analysis and fix

### BEMF divider network (from `vimdrones_esc_development_board_v1.2` schematic)

Per phase (A/B/C), the sense network is:

```
PHASE_X ──── 30k ────┬──── COMP_INM_X  (→ PA4/PA5/PB7)
                     │
                    3.6k
                     │
                    GND

                    10k
                     │
               COMP2_INP  (→ PB4, virtual neutral)
```

Three 10k resistors (one per phase) form a star whose centre is COMP2_INP
(virtual neutral). **There are no filter caps anywhere in this path.** The
bootstrap cap C17 (1 µF, PHASE_A ↔ FD6288_VB1) is in the phase circuit but is
not a BEMF filter.

Thevenin source impedances:
- COMP_INM node: 30k ∥ 3.6k ≈ **3.3 kΩ**
- COMP2_INP node: three 10k in parallel = **3.33 kΩ**

### Observed noise on scope (100 mV/div Math = CH1−CH2)

With Math at 10 V/div the BEMF signal is invisible (≈ ±200–400 mV amplitude vs
a 10 V/div scale = ±0.02–0.04 div). **Drop Math to 100–200 mV/div** to see the
BEMF arc crossing zero in the float window.

The noise problem: PWM switching on the driven phases couples through the
resistor star into the floating phase and virtual neutral. The noise is
bidirectional — in the **first half** of the float window the noise envelope
dips below zero; in the **second half** it peaks above zero. Both halves look
identical to the comparator: zero-crossings occur in both directions throughout
the entire float window. The time gate + persistence filter reduce false
detections but don't fully eliminate them at this noise level.

### Recommended hardware fix

Add **4.7 nF** at each resistor junction — at the board pads where the 30k and
3.6k meet (for each COMP_INM) and at the centre of the 10k star (COMP2_INP).
**Not at the MCU pins** — placing the cap at the MCU end leaves trace inductance
between the junction and the cap, reducing its effectiveness at high frequency.

RC with 3.3 kΩ + 4.7 nF: f_c ≈ 10 kHz. Attenuation at 24 kHz PWM fundamental:
−8 dB. Attenuation at 200–500 kHz gate-driver ringing: 20–35 dB. Phase delay at
600 Hz electrical: arctan(600/10000) ≈ 3.4° — negligible. If both INP and INM
use matched 4.7 nF the delay is common-mode and cancels in the differential, so
ZC timing accuracy is unaffected.

## G431 timer layout (for reference if porting minz)

AM32 on G431 uses the same TIM1/2/6/15/16 set as L431, plus **TIM17** (utility
timer, prescaler 160−1, free-running at 1 MHz). G431 runs at 160 MHz (HSI16 →
PLL ×40 ÷2 ×2); TIM1 autoreload = 6666 for 24 kHz PWM. NVIC priority differs:
on G431, COMP is priority **2** (lower than motor timers); on L431, COMP is
priority **0** (highest).

For bench work on G431: TIM7 and LPTIM1/2 remain free — same available set as
L431.
