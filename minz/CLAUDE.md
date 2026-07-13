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

### HYBRID ADC (2026-07-12) — supersedes single-group GECKO/WAXWING

`src/adc_sync.rs` now runs TWO ADC1 groups at once (branch
`bisect_init_changes`, on tag `checkpoint-24k-monsterfix`=715097d):

- **Injected group** — hardware-triggered by TIM1 TRGO2 (OC4REF
  falling, `CNT==CCR4`=SAMPLE_TICKS, mid-ON), **JADSTART-armed**. One
  4-channel burst: ch9(A) ch10(B) ch8(current) ch11(vbat) at
  47.5/47.5/12.5/47.5 cycles. `inj_read()->(A,B,current,vbat)`, read
  in TIM1_UP at wrap. This REPRODUCES the old regular-group mid-ON
  (A,B,current) triplet bit-for-bit and folds vbat in, so adc-confirm
  / vbus-decay / overcurrent / sag-kill are control-behavior-identical
  (the old `last_frame()`+`vbat_pump()` are gone). ⚠ a
  hardware-triggered injected group STILL needs JADSTART to arm — see
  the memory `reference-stm32-injected-jadstart`; omitting it left
  vbat/A/B=0 (caught by a BENCH-NEVER-DRIFTS bisect vs the control:
  control vbat=8.15 V, broken build 0 V, same bench).
- **Regular group** — ch8(current) only, CONTINUOUS free-run (no
  trigger), DMA circular into `CUR_RING` (2048 samples ≈ 0.55 ms,
  ~150 samples/PWM cycle): the intra-cycle current MICROSCOPE for the
  spike-conduction autopsy (smooth winding-limited BEMF-aided ramp
  4-5 A vs sub-µs railed shoot-through at a switching edge). Injected
  preempts it for ~2.2 µs/cycle at mid-ON — a marked, harmless gap.
  **GATED OFF by default** (`oversample_start`/`oversample_stop`,
  commit 8996dc3): qualification found the free-run ch8 was interfering
  with the injected ch8 read (idle current +13 counts/+300 mA vs the
  pre-hybrid control; raw 13 vs 0). So the free-run is STOPPED during
  normal lock — the ADC then does ONLY the injected mid-ON burst =
  identical activity to the pre-hybrid build — and enabled ON DEMAND
  (the `G` key self-fills the ring first; the `J`-armed >4 A trigger
  runs it while hunting, then stops). Verified: with it off, idle
  current = raw 1 (matches control).

Qualification (2026-07-12, paired vs `checkpoint-24k-monsterfix`).
**The non-gated hybrid HAD a real regression** — the free-run oversample
interfering with the injected ADC (proven: idle current +300 mA; and it
degraded lock robustness — the non-gated build broke at amp 14-16 vs the
control's 16-18, and threw more ZC-STARVED kills). That regression is
CODE, not "bench warming" — an earlier draft of this note wrongly
blamed a session-cumulative warming trend; there is NO bench warming
(see [[feedback-bench-never-drifts]]). Run-to-run break-point spread is
the engage/lock LOTTERY (a documented stochastic property), NOT a
trend. **Fix = the gate above** (free-run OFF during lock). Gated build
evidence: engage first-try on every paired run (both builds), steady
qzc 100% both, identical lock speeds (388/451/519 Hz), and 3 interleaved
ABAB pairs with hybrid ≥ control every pair (interleaving is valid
regardless of any run-to-run factor — it's the honest way to A/B).
`checkpoint-24k-hybrid`=902c4d7 tags the gated build.

**The "broke at amp 16-20" observations were SWIFT OFF, not degradation.**
Every ladder ran cl_lock_map WITHOUT `--fast`, so both builds hit the
~amp 18-20 confirm wall (~500 Hz). WITH `--fast` (SWIFT) the same gated
hybrid on the same bench climbs amp 16→44 = 515→1316 Hz, qzc 100% every
rung, currents 66→700 mA, vbat steady 8.16-8.25 V. A missing test flag,
not the environment — the honest version of "why does a fixed binary
give different results" is always "check the config / bisect the code,"
never "the bench drifted."

**Shoot-through autopsy — ANSWERED: it is NOT shoot-through.** The mid-ON
`i_raw` sees one point per PWM cycle and misses intra-cycle spikes (why
the oversample first "saw nothing"). Fix: the WAX auto-trigger now scans
the FREE-RUN RING PEAK (stride 4), and `gecko_load.py --hunt` arms J and
holds in the spike regime. Caught a real >4 A spike at amp 46: **peak
4.59 A, 0 railed, intra-cycle shape a SMOOTH 2→4.6 A ramp over ~150 µs
across multiple PWM cycles** (`captures/spikehunt46_*.png`) — winding-
limited BEMF-aided current on an L/R constant, NOT a sub-µs shoot-through
rail. `onset_probe.py` on `sweep55_t48.bin` proves the cause-order: the
commutation INTERVAL diverges first (100→70→160 µs at ~1400 Hz, seeded
by a premature SWIFT accept, qzc_off=13), THEN current ramps; i_avg≈i_max
(sustained). **Root cause of the amp draw = commutation-timing divergence
at the speed limit, NOT shoot-through, NOT the supply** — the supply sag
is downstream of the amp draw, never the cause.

**WHY the divergence** (black-box on WAX-trigger, `af5bdd2`): every `ACC`
in the bb shows d=**24 µs = the LPTIM2 commutation-delay FLOOR**
(`commutation_delay_us().max(24)`). At ~124 µs windows / advance 16° the
ideal delay ≈ 29−elapsed ≤ 24 → clamped, so the loop runs at zero
scheduling margin; commutations run late; the window after a late (esp.
always-blind phase-C dead-reckon — sectors 0/3 have no ADC) opens late →
its ZC is missed (`NOZ`, d=3-4 raw edges) → `BLD` blind mistimed
commutation → BEMF-aided current spike. Then **FIX #1** (window.rs: at
interval<160 µs a SINGLE miss widens the gate 30%→8%) **+ SWIFT**
(`on_held_edge` returns `AcceptNow`, BYPASSING `confirm_step` under lock)
amplify it — the 8% gate + ~1 µs blank + confirm-free accept take a
premature edge, misses recur (bb: sec4,sec4,sec2), interval grows
124→163 µs → runaway; amp 46 recovers, amp 48 compounds. Levers:
lower/rework the LPTIM2 delay floor (the hard limit); make SWIFT confirm
(not `AcceptNow`) in reacq; raise the speed blank floor; better phase-C
dead-reckoning. Tools: `gecko_load.py --fast --amp N [--hunt S]`,
`onset_probe.py`, bb-on-WAX-trigger.

**DELAY FLOOR REWORKED 24→8 µs** (commit `032faf1`): it was a SOFTWARE
over-clamp in `commutation_delay_us().max(24)`, not a hardware limit —
at ~1400 Hz the real delay is ~10 µs so `.max(24)` added 14 µs of
lateness every commutation. `LPTIM2_MIN_DELAY_US` 24→8 (core/timing.rs) +
`schedule_us` clamp min 16→4. Envelope amp 45→50, dropout 48→52, 1482 Hz
@ amp 50, sweep 15→50 qzc 100% (`floor8_full_map.png`). Confirmed
positively: on-demand bb (`B` key / `gecko_load.py --bb`) at amp 48 shows
ACC delays **9-13 µs varying**, not floored. The /64→/16 LPTIM2 clock
speedup FAILED (motor 2× fast — L431 LPTIM2 kernel doesn't scale as
`/PRESC` predicts; reverted); /64 delivers 8 µs fine. LPTIM2 is still
suboptimal (bounce + ARROK sync + `delay(200)` = ~3 µs/commutation baked
into `elapsed`); a GP-timer one-pulse (TIM7, or TIM16 = production rm32's
COM_TIMER) is the right peripheral if pushing past amp 52.

Dumps: **`G` key** = on-demand oversample dump; the **>4 A auto-trigger**
(`J` arms `WAX_TRIG_ARMED`) freezes+dumps `CUR_RING` (pre-trigger
buffer = the spike ONSET) then appends a `j` context dump.
`scripts/gecko.py` renders current-vs-time + intra-cycle zoom
(`--force` on-demand, `--wait N` for an armed trigger). The `j`
WAXWING dump now sources A/B/current from CPU-written per-cycle CTX
rings (`CTX_A/B/I` + `CTX_MARK` in motor_tester2.rs), aligned with
`PWM_SAMPLE_BUF` status — waxwing.py format unchanged. **Validated at
IDLE only** (vbat matches control, A/B sane, all dumps render);
engage/lock + a real spike autopsy need an operator-supervised spin.

The single-group GECKO description below is the PRIOR design (kept for
the trigger/sampling constraints, which still bind the injected burst):

### GECKO — PWM-synchronous continuous current sampling

`src/adc_sync.rs`: TIM1 OC4REF (falling edge at CNT=CCR4, inside the
high-side ON window — see the "FALCON current status" section for
the trigger/sampling constraints that CCR4 must respect) → TRGO2
(CR2.MMS2, raw-bits — PAC lacks the field) → ADC1 ch8 (PA3, INA180)
hardware trigger, EXTSEL=EXT10, one conversion per PWM cycle. No DMA,
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

## FALCON closed loop — WIP status (honest)

Machinery all works: `y` engages at the next qualified ZC, the COMP→
LPTIM2 chain (one-shot at interval·(30°−adv)/60°, 0.8 µs resolution,
`src/lptim2_oneshot.rs`) commutates, TIM7 freezes its stepper and
runs the desync watchdog (no qZC for 3 intervals → kill+coast; fired
correctly when amp dropped to 3), overcurrent failsafe stands.
Sustained 6+ s / ~4000 commutations at "97 Hz", current LOWER than
open loop, clean kills.

**But the lock was self-referential**: interval frozen at ±1 µs while
amp swept 10→15→5 (a true lock must accelerate with volts). The
first-persistent-edge-past-gate detector reproduces its own schedule:
commutate at qZC+interval/2 where qZC ≈ gate+ε ⇒ next window
identical, rotor ignored. The 0.6 µs COMP-ISR persistence check can't
reject PWM dwell (comparator sits still for tens of µs between
edges), and a static commanded-f gate also caps any tracking.

FALCON v2 (current code): candidate ZC in COMP ISR, confirmation
deferred to TIM1_UP wrap samples (2 consecutive PWM cycles must hold
the post-ZC level), adaptive gate at 40 % of measured interval,
schedule compensated for confirm latency. Open loop: qzc still flows
(87-100 %) but σ degrades to 55-190 µs (wrap-point reads discard good
candidates; later/worse ones win — sector 1 worst). Closed loop:
engages then desyncs (auto-kill works).

**Discriminator probe verdict** (`falcon_probe.py` bursts at 4
setpoints + 3 CL episodes, `falcon_stats.py` replays rules against
the analog linfit ZC of every A/B float window):

| group | rule | accept | premature | latency (frames) |
|---|---|---|---|---|
| CL episodes (206 win) | comp-wrap | 100 % | **25 %** | +2.5±2.3 |
| CL episodes | **adc-sign** | **100 %** | **0 %** | **+1.1±0.7** |
| open f100/200 | comp-wrap | 94-100 % | **41-73 %** | ~0 |
| open f100/200 | adc-sign | 89-95 % | 29-41 % | +1..+8 |
| open f300 | adc-sign | 88 % | 0 % | +3.3±2.5 |

The wrap-sampled COMP bit is premature-prone (dwells at the expected
level before the true ZC — the self-lock mechanism, quantified). The
**sign of the mid-ON ADC sample (phase − neutral)** is the clean
confirmation: in closed-loop conditions 100 % accept, 0 % premature,
46±29 µs latency. (v2 also held CL ACTIVE ≥1 s in all 3 probe
episodes — marginal, not dead.)

**FALCON v3 (LANDED — true lock achieved)**: candidates confirmed in
TIM1_UP by the ADC sign from the WAX ring (float phase vs driven-pair
neutral; vbus = decaying max of driven-high reads) for A/B windows;
sectors 0/3 (phase C, no ADC) dead-reckoned; interval updates
span-divided; free-run-at-1.0×T + ZC-refine scheduling (AM32
semantics — a 1.5×T "fallback" compounded lag instead); 30 %
adaptive gate; window-generation guard + `free` around the accept
(the commutation ISR can preempt TIM1_UP mid-reschedule); LPTIM
enable→start needs 2 counter clocks (asm delay in `schedule_us` —
without it SNGSTRT is silently dropped and the chain dies).

**Result**: engaged at f=100/amp=10, locked at its ~250 Hz
equilibrium, and **throttle-following verified in both directions**
(amp 10→12→10 ⇒ 254→312→249 Hz with lock held, ~37 k commutations /
25 s, zero desyncs, 33-66 mA).

**The "acceleration envelope limit" was a phantom.** A 64-event
black-box ring (dumped automatically on desync) caught the real
killer in one reproduction: the TIM7 desync watchdog computed
`ticks_10us() − LAST_COMM` with `now` read BEFORE the reference — a
commutation ISR preempting between the reads updates LAST_COMM to a
newer tick, the subtraction underflows to ~4×10⁹, and a perfectly
healthy 309 Hz lock gets killed 20 µs after a refined commutation.
Probability ∝ commutation rate, which is why it masqueraded as an
envelope ceiling. Fix: load the cross-ISR reference BEFORE `now` +
top-bit clamp. **General rule: a wrapping timestamp delta whose
reference is written by a higher-priority context must read the
reference first (or clamp) — the false trip looks exactly like the
fault being guarded.**

Black box stays in the firmware (`bb` lines after any desync):
REF/BLD/DRK commutation classes, ACC/NOZ/DIS ZC events, ENG/DSY
(+ STV starvation, RAQ re-acquisition — see below).

## FALCON current status (2026-07-09) — SWIFT era, 970 Hz @ 100 %

Supersedes the older FALCON notes above where they conflict. Full
chronology and evidence in `FALCON_HARDENING.md` §7–15.

### The SWIFT arc (2026-07-09) — chasing AM32's full throttle

Driving question: AM32 runs 100 % throttle on this exact unmodified
board — why couldn't we? Each gap found, cribbed, and A/B-verified:

- **SWIFT edge path** (`M` key toggles; `cl_lock_map.py --fast`, the
  key is STATEFUL across sessions — script presses twice if needed):
  under lock, a comp edge that passes gate + persistence is accepted
  IMMEDIATELY in the COMP ISR (µs timestamp, zero wrap latency) —
  AM32's architecture. The ADC-confirm path (unchanged, still
  default) quantizes accepts to PWM wraps + confirm latency; that
  per-window tax was the ~620-650 Hz ceiling. A/B: +117 Hz same
  session. Engage/low-speed keep ADC-confirm — our advantage regime.
- **Speed-adaptive blanking**: AM32-L431 has NO PWM-edge time blank —
  its filter is persistence depth scaled with speed (main.c:2112).
  Our fixed 8 µs blank left the comparator blind ~75 % of every
  48 kHz window. Now: blank = min(user, interval/75) µs; persistence
  deepens 5 → 12 reads when the blank falls below 5 µs. Low speed
  degenerates to the proven combo exactly. First no-break ladder
  ever (117 k windows) immediately after.
- **Auto-advance ramp**: 0° below ~280 Hz (measured no-op regime) →
  12° cap by ~1.2 kHz; manual `t`/`T` nonzero overrides. A speed
  DROP under more throttle = late-commutation braking, NOT V/f
  saturation (that lesson cost a day: static 8° recovered +108 Hz at
  amp 34; static 16° kills the engage transit — the ramp gives both
  regimes their angle automatically).
- **µs gate** (`SECTOR_START_US`/`SECTOR_GATE_US`, was 10 µs ticks:
  15 % quantization at 170 µs windows) and **telemetry decimation**
  (every 5th window above ~925 Hz; 5 coprime with 6 so sectors
  rotate; host handles seq gaps).
- **AMP_MAX now 50** (was 20/25 — each raise exposed the next layer:
  amp-cap equilibria masquerading as "voltage walls", the PSU
  exonerated TWICE via the per-point vbat logging now in the lock
  maps/meta CSV, prop ω³ current, then advance).

**Current record: amp 35 = 970 Hz electrical (~8,300 RPM), qzc
100 %** (`captures/phase1_map.png`); marginal edge amp 36. Roadmap
(90 %+ throttle ≈ 2.4 kHz elec, 66 µs windows): Phase 2 = prop OFF
+ fresh bench, ladder 36→50→70→90 %; Phase 3 = watchdog-cadence
audit at sub-166 µs windows + re-acq-at-speed tuning; Phase 4 =
runtime 24/48 kHz carrier switch + rm32 portback. HEDGEHOG (caps
A/B) is DELETED — the unmodified board runs 100 % coverage to
970 Hz; software filtering was always sufficient.

### Protection stack (each live-fire tested on the bench)

- **ZC-starvation watchdog**: no ACCEPTED qZC for 12 intervals under
  CL → kill (`!! CL ZC-STARVED`, bb `STV`). Catches the stalled-rotor
  **zombie field** that nothing else can: rotor stalls, free-run keeps
  commutating (chain watchdog content), detached field draws little
  current (no OC trip). Operator eyes caught it first; now firmware
  does.
- **Runaway floor**: interval < 160 µs → kill. A runaway self-feeds
  on junk accepts so the starvation guard alone never fires.
- **Symmetric rate bound** new ∈ [0.6, 1.8]×old per accept — kills
  harmonic locks (observed 2×-rotor lock: "539 Hz" @ qzc 28 % vs true
  257 Hz @ 100 %; qzc ≈ 100 % or it isn't a lock).
- **Estimator reset on arm** (`y`): a poisoned static interval (e.g.
  144 µs left by a runaway) is otherwise UNRECOVERABLE — the floor
  kills every engage while the bound rejects every honest sample.
  Individually-correct guards can deadlock as a system.
- **Re-acquisition** (bb `RAQ`): 2 consecutive ZC-less A/B windows →
  gate 30 %→8 %, full 2-confirm, qZC chain broken, interval re-seeds
  from two fresh ZCs bounded [0.5, 2]×old. Moved the 24 kHz wall
  474 → ~510 Hz (the lockout spiral: blind windows → phase lag → ZCs
  drift past the estimate-referenced gate → starve).
- **Confirm depth**: 2 wraps until CL_ACTIVE, 1 after. Open-loop
  1-confirm is 52-69 % premature (probe replay) and caused engage
  runaways when shipped unconditionally.

### Lock maps — the deliverable renders

`cl_lock_map.py` (steady-state throttle ladder; `--adv`; engagement
is VALIDATED — 1.2 s sample must show qzc ≥ 90 %, ≤4 retries — the
"engagement lottery" is real) → `plot_lock_map.py` (4-panel PNG).
- 24 kHz, 7.5 V, prop: amp 11-17 ⇒ 326-500 Hz, qzc 100 % everywhere.
- **48 kHz: amp 15-20 ⇒ 405-555 Hz, qzc 100 %, 52.8 k windows, zero
  breaks** (`captures/pwm48final_map.png`). The ~480 Hz confirmation
  wall is gone (21 µs confirm quantum).

### 48 kHz recipe (currently flashed; `lib.rs::PWM_FREQUENCY_HZ`)

Won with ZERO loop-logic changes — constraints only:
- **Phase channels keep 47.5-cycle ADC sampling.** Non-negotiable:
  sector 2's confirm rule (2×float-A vs driven-high B, where 2×A ≈
  vbus at the ZC by construction) goes to exactly 0 % qZC at
  24.5/12.5 cycles — at BOTH carriers. Diagnostic: per-sector
  owl_report table / `scripts/sector_polarity_check.py`.
- ch8 (current) drops to 12.5 cycles — INA180 output is low-Z —
  bringing the 3-channel sequence end to ~3.06 µs.
- Trigger stays 0x64 = 1.25 µs (1.0 µs blinds sector 2 — turn-on
  ringing tail; 0.6 µs samples during dead-time and flatlines ch9).
- Comp blank 8 µs (20 µs covers an entire 48 kHz period → the loop
  self-blinds: 170-300 raw edges/window, zero candidates).
- **Engage at amp 15** — the smallest ON window (3.125 µs) that fits
  the sequence. CL floor = amp 15 / ~405 Hz; below that, use the
  24 kHz build (blank 20, engage 10). Runtime carrier switching is
  the future unification.

### Convicted — do NOT retry naively (all A/B'd on a healthy bench)

±2 % "physical" slew clamp (locked 46 Hz crawl — under a synchronous
loop the interval measurement echoes the loop's own field; tight
clamps remove the convergence signal); commanded-rate estimator seed
at arm; wrap-armed ADC-sign candidates; asymmetric fast-α smoothing;
gate at 20 % (early noise edges reach the 1-confirm fast path).
Estimator meddling under lock is treacherous — the failures live on
branch `wip_48khz_campaign` with black-box anatomy.

### Advance (`t` +2° / `T` −2°, clamped 0..28)

At bench loads +2..+20° changes nothing (speed and current flat;
coverage droops above ~14°); negative advance collapses the rotor to
a ~5 Hz crawl that the loop *tracks* (qzc 70-84 %). Not a lever until
real load. Engage always at 0° — the open-loop spin-up consumes
ADVANCE_DEG too.

### Bench drift protocol (bit us twice)

Multi-hour sessions degrade until even bit-identical known-good
builds fail their own ladders (amp 17 → amp 11); recovers after ~1 h
rest; motor was cool — cause unknown (driver/FET thermals? supply?).
**Always re-run the known-good build as a control before trusting a
late-session A/B.**

### Scripts added this arc (all `scripts/`)

`cl_lock_map.py` / `plot_lock_map.py` (lock maps), `cl_adv_sweep.py`
/ `plot_adv_map.py` (advance sweeps), `cl_ramp_sweep.py` (ramp
envelope; classifies sub-150 µs windows as RUNAWAY so a detached
field can never read as SURVIVED), `sector_polarity_check.py`
(empirical per-sector BEMF polarity/offset from waxwing cdumps),
`falcon_stats.py --confirms N`.

Branches: main work on `bisect_init_changes` (48 kHz build flashed,
84514d2); `wip_48khz_campaign` holds the failed-experiment forensics.
Remaining polish: ~~throttle slew limit~~ (DONE — 1 %/50 ms via
minz_core::throttle, snap-on-arm), runtime carrier switch, HEDGEHOG
A/B deleted (unmodified board proven to 970 Hz).

## minz-core — host-testable control logic (`core/`, 2026-07-11)

The FALCON loop's pure logic AND every wire/dump serializer live in
`minz/core/` (`minz-core`, no_std, sole dep portable-atomic):
`timing` (advance/delay/gate/blank/persistence/confirm), `estimator`
(the full OWL accept incl. re-acq re-seed), `guards` (watchdog/OC/
sag + `since_us`, `apply_isr_kill` flag matrix, `sag_step`,
`trip_accum_step`), `mode` (arm/kill/CL state machine behind the
`r`/`q`/`w`/`y`/`m` keys — `step(Cmd)->Actions`), `window`
(`close_float_window` behind the `WindowState<'a>` atomic-ref seam),
`zc` (adc-sign confirm rule + vbus decay — seed of the full ZC
machine), `throttle` (slew+snap), `ticks` (`compose_1us` — firmware
adoption pending), `wire` (MAGPIE v4 WindowRec, used directly),
`blackbox` (ring + `format_dump` + the `EV_*` event-code authority),
`drive` (sector geometry + commutation_class + freerun 1.0×T rule),
`dump` (cdump framer, `l`/`e` renderers, edge_dump_status), `sense`
(the one vbat/isns calibration), `rates`, `ui`, `a85`, and `zc` —
now the FULL candidate/confirm/accept state machine (on_held_edge /
confirm_step + accept_gen_current / accept_publish; the B1 TOCTOU
gen-guard hole is closed via a load-time generation snapshot, with
the interleaving as a named test). 114 tests, 99.2 % line coverage
(`cd core; cargo test` / `cargo llvm-cov`); every past incident is a
named regression test. Firmware (`motor_tester2.rs`, ~2,700 lines:
ISR glue + statics + init) calls into core at every corresponding
site — change loop or wire behavior in core, never inline. Hardware
lib modules: `minz::{uart_tx, usart2_rx, iwdg}`. `core/` is a
detached workspace with its own `.cargo/config.toml` (host target) —
do not fold it into the parent or tests build for ARM. Details:
FALCON_HARDENING.md §17; **`EXTRACTION_ROADMAP.md` is COMPLETE** and
kept for the incident notes — including the E1 verification war
story (single-engage bench verdicts are lottery noise; judge engage
quality only with n≥4 kill/re-arm attempts or the ladder's validated
engage) and the known pre-existing sector-1/2 ADC-confirm blindness
at low amp (invisible under SWIFT).

## Where we are (2026-07-11, end of the extraction arc)

State of the bench and the open question, so the next session starts
oriented:

**THE 2026-07-11/12 REGRESSION SAGA — ROOT CAUSE: PRFTEN.** The
"extraction regression" was flash-layout sensitivity: PRFTEN off
(AM32 parity — an rm32 constraint, never a minz one) + 4WS makes
hot-ISR fetch timing depend on code alignment, so ANY build (even
init-only changes — proven by the E5 piecewise step dropping
engage@15 from 8/8 to 3-4/8) moves the marginal engage regime;
token-identical hybrids flip-flopped accordingly. Fix: PRFTEN
enabled in minz::board_init (8d32e66) — same binary went 3-4/8 →
8/8. The FULL extraction is re-landed and gated (dfac519): every
hot core fn #[inline], objdump-verified inlined, 8-attempt gates at
each step, SWIFT + ADC-confirm both verified, final sweep 15→50 qzc
100 % (`captures/refactor_complete2_*` — amp 50 held for the first
time). Also real and separate: a time-varying bench factor
(drift-controlled ABAB proved both effects at once) — engage-quality
A/Bs remain paired-ABAB-vs-tagged-control only; keep a
`sweep-good-*` tag current.

**Firmware**: the flashed build (branch `bisect_init_changes`) is
the fully-extracted one — the whole extraction roadmap is COMPLETE
(see `EXTRACTION_ROADMAP.md`, kept for incident notes). Envelope
re-verified end-to-end on it: full-range ladder amp 15→45 = 461→
1,222 Hz, qzc 100 % every rung/sector, 80 k windows
(`captures/fullrange2_map.png` + `_dropout.png`). Amp 10 rung
refused sustained CL — that's the documented 48 kHz floor (amp 15
minimum ON window), not a fault.

**The dropout anatomy is now quantified — and it is NOT throttle
steps.** Throttle is already a smooth gradient everywhere (keys set
TARGET; firmware slews 1 %/50 ms; scripts can't produce a real
step). Steady-dwell test, constant throttle, zero commands in
flight, 8 s each: amp 44 → 40 events >2 A (~5/s, worst bus dip
5.26 V — survived); amp 46 → 44; amp 48 → 55 (~7/s). The discrete
2-8 A phase-agnostic current-spike events are a STEADY-STATE
phenomenon from ≲amp 44 up, each ~1-15 ms; the lock rides through
almost all of them. A "dropout" is the unlucky event that is both
deeper than −10 %-of-baseline AND longer than the sag guard's
1.3 ms debounce (kill fires, clean, r/q re-arms). The supply cliff
(PSU-driven collapse) lands wherever one such event snowballs —
observed anywhere amp ~46-52 depending on PSU state.

**Spike events — first root cause LANDED (adfad27): the stale-CCR
brake short.** Old `set_six_step` wrote per-phase CCRs (duty on hi,
0 elsewhere) with the low leg timer-driven; CCR writes are preloaded
(OCxPE=1 → land at the next PWM wrap, ≤20.8 µs later at 48 kHz)
while the MODER role flip is immediate. On every commutation where
the hi role moved (every other one), the new PWM leg compared
against its stale CCR=0 → complementary output solid high → its low
FET on beside the real low leg = BEMF shorted line-line through two
low FETs for 0–20.8 µs. Phase-agnostic, speed-scaled, and it ends
exactly at the wrap — 1.25 µs before GECKO's sample point, so
telemetry only caught the snowballed survivors. Fix = exact AM32
parity: `SET_DUTY_CYCLE_ALL` (same duty in all three CCRs, always)
+ GPIO-forced low leg via `PhaseRole{Pwm,Low,Float}` in
`tim1_motor_pwm.rs`, BSRR-before-MODER so old-low-off / new-low-on
land in one register write. Drift-controlled ABA at amp 44/46/48
(8 s dwells, >2 A events): fix 69 → old 115 → fix 69 (−40 %, ~4σ;
the old build also threw a ramp ZC-STARVED kill). Ladder 15→50 all
qzc 100 %, amp 50 = 1,374 Hz, rungs ~1 % faster — the brake drag is
gone (`captures/phaseroles1_map.png`). Amp 55 transit still died on
a real 4.5 V sag: **residual ~2.5 events/s at amp 44-48 remain
unexplained** — investigation continues below.

**TIM1_UP scheduling health (2026-07-13, `t1u:` line in `i`)**: DWT
gap detector at ISR entry. At amp 44-48 CL, 12 % of 48 kHz cycles
LOST (counter deficit), 27 % late-or-lost, maxgap 91-100 µs — the
confirm/GECKO/failsafe machinery has multi-cycle blackouts. The
TIM7↔TIM1 priority swap (TIM1=2, TIM7=3, commit 63a5697; also fixes
the previously-FALSE "TIM7 can't interrupt us" invariant in
TIM1_UP's accumulator writes) improved losses only −20 % (maxgap
69 µs) and left spike events UNCHANGED (79 vs 82) — TIM7 was a
minor blocker. The dominant blocker is the LEVEL-1 tier itself:
LPTIM2's commutation ISR carries `close_float_window` (window
close + bb + MAGPIE wire serialize) inside `free` at every
commutation (7.8 k/s at 1300 Hz). Blocking budget ≈ 24 % of wall
time vs ~13 % of legit COMP/SysTick/CC load. Priorities are now
emitted over UART at boot (`prio:` line, hardware read-back) so
every session log records them. Boot fw also prints the RTT dumps.

**Carrier A/B — events are electrical, NOT CPU (2026-07-13, commit
6c7c4a3):** dropped 48→24 kHz to reclaim ISR headroom. It worked for
scheduling (cpu 75→57 %, tim1_up=24000 exact/zero true losses, old
~650 Hz ceiling gone → 1,230 Hz @ amp 44) but the spike events got
**4.5× more frequent and 2.7× deeper** at the same amp (6.4/s, 7.3 A,
5.0 V dips, audible chops; envelope dies 44→46 vs 50→55). Events
scale with carrier RIPPLE, not CPU/ISR health — the decisive
decoupling. **Staying at 24 kHz** for the investigation (CPU
eliminated as a confound + amplified events = better microscope);
48 kHz is the perf recipe once the mechanism is fixed.

**Spike census — QUALIFIED (2026-07-13, `count_spikes.py` with
per-100ms/1s binning + Fano + tier split; 20 s dwells on the
light-rearm build).** Two populations: **small events (2–4 A,
single-window)** grow gradually with amp (1.2/s @38 → 1.7/s @40 →
3.3/s @42 → 3.9/s @44) and are **uniform/Poisson (Fano ~1)** —
benign independent mistimed commutations. **MONSTERS (≥4 A) have a
sharp THRESHOLD near amp 41** (0/s at ≤40, 1.85/s @42, 2.65/s @44),
last 26–32 ms, and are **CLUSTERED/bursty (Fano ~1.9–2.9)** — NOT
independent. Rate unchanged by the CPU levers (a44 6.5/s vs 6.4/s
pre-DWT) → the events are electrical, not scheduling.

**Monster autopsy — SOLVED (2026-07-13, monster-only J-trigger,
`WAX_TRIG_RAW=150`≈4 A, 3 captures `monster1..3`, all identical).**
The monster is a **cluster of 12–21 consecutive ZC-DETECTION MISSES**
(the per-PWM-cycle comparator bit stops transitioning in float
windows), spanning 10–18 ms. Universal facts across all 3:
- **The rotor keeps turning** — driven-phase BEMF plateaus stay FULL
  (973/4096) through the cascade. NOT a stall.
- **Commutation sequence stays clean** (zero wrong-sector) and the
  interval holds ~125 µs — the loop FREE-RUNS blind at the frozen
  interval, stepping the field without ZC confirmation.
- **The current spike FOLLOWS the first cascade miss** (cause-before-
  effect confirmed at PWM-cycle resolution in all 3) and ramps
  monotonically 1 A → 4 A+ as the field drifts off the (still
  turning) rotor → line-line current → bus sag (4.4–5.1 V).
- **Isolated single misses (0.2–2.2 % of clean windows) are RIDDEN
  THROUGH**; only the CLUSTER is a monster. This IS the census
  clustering (Fano ~2).
- **The loop RECOVERS** (all 3 survived, CL ACTIVE after) — re-acq
  eventually re-locks, but only after the 10–18 ms current transient.

**Mechanism = window-position runaway** (timing, not mechanical, not
comparator-hardware, not shoot-through): the BEMF is present but its
neutral crossing falls OUTSIDE the detection window. One miss → the
free-run schedule drifts → the next window is mis-positioned → the ZC
lands outside it → miss → compounds → spiral, aggravated by the
rising current distorting the BEMF. The amp-41 threshold is where
windows get short enough (125 µs) that a schedule drift pushes the ZC
out. **Why the loop can't ride through: the re-acquisition (2
consecutive A/B misses → widen gate) is too slow at high amp** — 12–21
misses accumulate before it recovers, letting current ramp to 4 A+.
Fix directions (control-logic, minz-core, testable): (a) break the
cascade FASTER — trigger re-acq after 1 miss / immediate wide-window
search at high amp; (b) CURRENT-CLAMP the duty during blind free-run
to cap the monster amplitude while re-acquiring; (c) advance/window
tuning so ZCs sit centrally with drift margin. (`captures/monster1..3
_zoom.png`.)

**CPU headroom — levers #1 + #1b LANDED. The wall clock is now
DWT.CYCCNT (commit 4fd43f1, tag `checkpoint-24k-dwt`).** History:
#1 dropped SysTick 100 kHz→5/s (systick-timer 200 ms wrap) — real
but it traded ISR rate for a heavy 64-bit snapshot cost, the 24-bit
SysTick tradeoff. #1b escapes it entirely: **DWT.CYCCNT** is a free-
running 32-bit counter at the full 80 MHz core, single-instruction
read, ZERO periodic ISR (SysTick fully retired). No-silent-overflow
design (the hard requirement): CYCCNT wraps every 53.7 s → extended
to 64 bits by `cyc_extend` in the EXISTING 24 kHz TIM1_UP (sole
writer, can't miss a 53.7 s wrap; shares the miss-detector's CYCCNT
read); `now_cyc64` reads HIGH,LAST,CYCCNT,HIGH with double-HIGH
retry + `cyc<last` self-compensation (DWT has no pending bit, reader
compensates itself). `ticks_1us`/`ticks_10us` derive from the u64 so
they keep a CLEAN power-of-two wrap (71 min / 12 h) → every
`wrapping_sub` consumer AND the MAGPIE wire timestamp UNCHANGED;
`now_10us_64` (true u64) backs the epoch pacing so it can't straddle
a wrap and hang; CYCCNT zeroed at enable (a power-up value near 2³²
could wrap during calibration before the extender is live);
**`CLOCK_BACK` tripwire** (main-loop monotonic check, `clkback=` in
`i`) makes any residual glitch LOUD not silent — verified 0 across
all runs incl. under sustained CL load. Regression: ladder 15→40 qzc 100 % IDENTICAL to baseline; amp 44
inconclusive (bench was drifting — the known-good control ALSO broke
at 44 in a tight A/B, so no DWT-specific regression); idle `dur cyc`
IDENTICAL to the systick build (t1u≈332, cc≈81, t7≈143, comp=0). ⚠
**The idle-loop `cpu=%` (busy%) is UNRELIABLE — do not quote it.**
Same binary, same idle ISR load, read 20 % on one boot and 40 % on
another: its calibration baseline is boot-sensitive (the free-loop
`cal` gets perturbed at boot), so busy% is neither boot-stable NOR
cross-build comparable. **The DWT `dur cyc:` line in `i` (per-ISR
last-pass cycles × rate) is the ONLY trustworthy CPU accounting** —
it showed identical load here, proving no regression.

**LPTIM2 (commutation ISR) — MEASURED breakdown @amp40/1200 Hz
(2026-07-13), the honest picture that corrects the earlier "15.7 %
close_float_window elephant" guess.** Whole ISR ≈ 1871 cyc (~17 % CPU
at this speed). Sub-segments (DWT-timed): entry+commutation-class-bb
~335, **set_six_step + COMP2 mux/edges (actuation) 380**,
**close_float_window 579**, **schedule_us 531**, rest ~40. So the
cost is DISTRIBUTED, not concentrated in close.

**Lever #2 (defer close_float_window's build to main) — ATTEMPTED,
BACKFIRED, REVERTED.** Split the core so the commutation ISR enqueued
a raw `RawWindow` snapshot and main did the derived-field arithmetic
(`from_raw`). Result: close went 579 → **653 cyc (+74, WORSE)**,
reverted (confirmed back to 579). Why: on Cortex-M4 the "deferred"
arithmetic is CHEAP (hardware divide, single-cycle min/sub, ~40 cyc),
while `RawWindow` (44 B) is BIGGER than the `WindowRec` (28 B) it
replaced, so the extra struct-plumbing + larger queue-copy cost more
than the arithmetic saved. **Lesson: don't defer cheap M4 arithmetic
behind a bigger snapshot struct; measure the segment (drift-immune)
before and after.** The real reducible in close is the atomic
bookkeeping (~15 resets) + the `Mutex<RefCell>` enqueue borrow — but
the Mutex is justified (BOTH TIM7 open-loop AND LPTIM2 CL enqueue
share the producer), so it needs a lock-free MPSC to remove, not
worth the risk now.

**Lever #2 v2 — `schedule_us` LIGHT re-arm: VALIDATED + LANDED
(commit d431783). The real win.** `schedule_us` (531 cyc) is
dominated by `cortex_m::asm::delay(200)` (a 2.5 µs busy-wait) + the
disable/enable bounce + the ARROK poll. The free-run reschedule runs
it every commutation from the LPTIM2 ISR — which fires AT the ARR
match, where single-counting mode has HALTED the counter with ENABLE
still set and the kernel warm. So the bounce+delay are unnecessary
there: `reschedule_light` skips them (ICR clear → ARR write → ARROK
poll → SNGSTRT). Measured A/B @amp40: LPTIM2 ISR **1890 → 1570 cyc
(−310, ~2.8 % CPU at 1200 Hz, more at speed)**, full sweep 15→44 qzc
100 %, amp 44 LOCKS (the full-path DWT build died at 44 same
session — plausible top-end gain from removing the busy-wait's COMP
blocking, mechanism-supported not paired-proven).

**ARROK conundrum SOLVED (instrumented the poll):** it completes in
**≤26 spins in BOTH paths, guard NEVER hits** (10 000 backstop). The
ARROK poll is the necessary-and-sufficient sync for the ARR write
crossing into the LPTIM kernel domain (PCLK/64). The `delay(200)`
was ONLY the post-disable kernel warm-up, NOT the ARR sync — proof:
`light_max` (26) slightly EXCEEDS `full_max` (15) because without
the delay pre-warming, the poll absorbs the few extra sync clocks
(same total sync, moved from fixed busy-wait into the cheap bounded
poll). `ARROK_GUARD_HITS` kept as a permanent safety tripwire
(`arrok_guard_hits=` in `i`; must stay 0). The **COMP ZC-refine
path keeps the full `schedule_us`** — it overwrites a RUNNING count,
and single mode ignores SNGSTRT while counting, so cancelling needs
the disable/enable.

**Lever #2 v1 (defer close_float_window build) — earlier ATTEMPT,
BACKFIRED, REVERTED** (kept above): don't defer cheap M4 arithmetic
behind a bigger snapshot struct.

Remaining levers: (3) kill TIM1_CC, COMP reads TIM1.CNT directly;
(4) gate ADC-confirm under SWIFT-at-speed; (5) MAGPIE serialize in
main; (6) half-rate vbat/sag; and possibly extend the light-re-arm
insight to shrink the COMP-refine full path. **DWT.CYCCNT is
Cortex-M4 only — absent on F051/G071 M0 rm32 targets; a portback
there needs a chained hardware timer.**

**Monster fixes — ALL 3 LANDED + VALIDATED (2026-07-13, final commit
7abd19f). Monsters ELIMINATED, engage-neutral.** The monsters are a
ZC-miss cascade / window-position runaway (autopsy above). All three
are SPEED-gated to the high-speed regime where monsters exist
(interval < `window::HIGH_SPEED_US`=160 µs, ~amp ≥42); at engage
(~1667 µs) they are OFF.
- **#3 advance margin:** `timing::auto_advance_deg` speed-gated boost
  (below 200 µs, +6° by 130 µs) lands the ZC ~16° later, off the 30 %
  gate (measured ZC at 0.33 vs gate 0.30 = ~0 margin was the trigger).
- **#2 blind-amp clamp (`guards::blind_amp_clamp`):** at a sustained
  cascade (`cl_noz_run ≥3`) cut amp 2/3 then 1/3.
- **#1 faster cascade break:** re-acq widens the gate on the 1st A/B
  miss (was 2nd); chain-break still at the 2nd.
- **Result:** census 20 s dwells, monster ≥4 A: **amp-42 39 → 0,
  amp-44 56 → 0**, both qzc 100 % full-dwell (`fixed_census_*.png`).
  Small 2-4 A benign events remain (unchanged).
- **Engage VALIDATED neutral:** paired same-bench control lightrearm
  3/8 == speed-gated all-3 3/8 (the session's 5/6→3/8 engage swing is
  the stochastic engage LOTTERY hitting both equally). 123 core tests.

**THE ENGAGE-REGRESSION SAGA — a `BENCH NEVER DRIFTS` lesson.** The
first cut gated #1/#2 on a `CL_LOCKED` settle-lock (12 accepts) — it
latched in ~20 ms, far too fast to protect the ~1.5 s engage, so the
fixes fired mid-engage: #1's 8 % re-acq gate let PWM noise corrupt
the lock, #2's clamp starved engage torque. I mis-attributed the
engage failures to "bench drift" and shipped with a "rested-bench
re-validation pending" caveat. **The bench was fine** — proven when
the 48 kHz known-good engaged 4/4 and 24 kHz lightrearm 5/6 while the
fix build went **0/6** on the SAME bench. BISECT (not drift): 5/6 →
#3 3/6 → #2 1/6 → #1 0/6. Fix = SPEED gate (above). Operator
directive: never blame "bench drift" again — bisect against a
known-good TAG. (There was no 48 kHz tag either — now
`checkpoint-48k-known-good`=c31e1e3; the 24 kHz working reference is
`checkpoint-24k-lightrearm`.) 24 kHz single-shot engage is inherently
~50 % / high-variance (the lottery), so validate engage with n≥8
PAIRED against a tagged control, same session. Also on the shelf: the
ticks_1us wrap-hole fix (core `ticks::compose_1us` is ready,
firmware adoption pending), runtime 24/48 kHz carrier switch, rm32
portback, and the newly-flagged sector-1 ADC-confirm blindness at
low amp (WAXWING look at the `2B < A` rule's margins).

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
