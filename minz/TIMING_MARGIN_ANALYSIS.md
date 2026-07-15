# Timing-margin analysis — the amp-68 / ~1850 Hz ceiling (2026-07-13)

Data gathered to analyse why the closed loop breaks at ~amp 67-68 /
~1850 Hz. Short version: **it is a commutation-timing-margin collapse
on the SCHEDULE side, not the supply.** The VBAT SAG KILL is the guard
that fires last; the amp draw (an 11 A ZC-miss monster) is the cause,
the bus sag is the effect.

Board: L431, 24 kHz PWM, 8 V bench, no caps. Build: TOPEND-gated
reacq-confirm + phase-C re-time, floor 8 µs, `AMP_MAX=75`.

## Cause-order (from the dropout, `night2_5070_t68.bin`)

`onset_probe` at the monster (window ~1130):

```
1129  s5  Y  int 80  qzc 39  i 2015   vbat 7.93   <- steady, clean
1130  s0  Y  int110  qzc 63  i 3035   vbat 6.86   <- phase-C window, LATE ZC, current rising
1131  s5  --         int130  ----    i 8462   vbat 5.41   <- first MISS, current explodes
1132+       cascade: ---- misses, interval thrashes 80<->160, i 7-11 A, vbat 5-6 V
```

Timing goes first (a late phase-C ZC → the next window misses), THEN
current runs away, THEN the bus sags. The bottom panel of
`night2_5070_dropout.png` shows the precursors clearly: small 3-5 A
single-window spikes scattered through the whole dwell (recovering),
growing, until one triggers the full cascade.

## The two margins at amp 66 (steady, 91 µs window, 1825 Hz)

### SCHEDULE side — EXHAUSTED

`gecko_load --bb` ACC commutation delays, and a temp pre-floor probe:

| quantity | value | note |
|---|---|---|
| delay budget = interval·(30−adv)/60 | **15.2 µs** | adv = 20° (auto_advance capped) |
| `elapsed` (ZC detect → schedule call) | **~10 µs** | ISR + persistence + estimator + confirm; a FIXED cost |
| ideal delay (pre-floor) = budget − elapsed | **~5 µs** | measured 4-6 µs, all 6 sectors |
| LPTIM2 floor (`LPTIM2_MIN_DELAY_US`) | **8 µs** | ideal 5 clamped UP to 8 |
| → commutation fires | **~3 µs / 12° LATE, zero slack** | |

ACC delay is **100 % floored at 8 µs from ~amp 58 through 66**
(at amp 48 it was still 9-13 µs; amp 50 ~8-9). So the schedule margin
runs out around amp 55 and is fully gone by 58.

The killer is two FIXED costs — **~10 µs elapsed latency + the 8 µs
floor** — against a delay budget (15 µs at amp 66) that keeps shrinking
with speed. Once the budget minus elapsed drops below the floor, every
commutation is clamped and there is no slack to absorb a perturbation.

### DETECTION side — still healthy (~3σ)

`owl_report` per-sector at amp 66:

- qzc_off **~48 µs** (53 % of the 91 µs window), σ 6-7 µs
- gate = interval·3/10 = **27 µs** → ~21 µs margin ≈ 3σ
- qzc **100 %** steady, all sectors even; pred_err bias −4 µs (the late
  commutations show up here)

So the ZC is NOT crowding the gate in steady state. But with 3σ ≈ the
whole margin, a perturbation that shifts the schedule (which the floored
late commutation does) pushes the next ZC toward the gate.

### Phase C is the shakiest predictor

`owl_report` on the monster transit (`t68`), err/win per sector:

| sector | phase | err/win |
|---|---|---|
| 0 | **C** | **3.4 %** |
| 3 | **C** | **3.6 %** |
| 4 | B | 3.8 % |
| 1 | B | 4.2 % |
| 2 | A | 1.5 % |
| 5 | A | 1.4 % |

Phase C (0/3) is re-timed from the comparator but has no ADC cross-check
(sectors 2/5 = phase A are the tightest). The monster **initiated at
sector 0** (phase C). C is where the zero-slack schedule gets its first
perturbation.

## Synthesis

At ~1850 Hz the commutation-timing margin is eaten by two fixed costs
(~10 µs `elapsed` + 8 µs floor ≈ 18 µs) against a ~15 µs delay budget
that shrinks with speed. The schedule is fully floored (no slack), the
detection margin is ~3σ, and phase C is the weak sector. A late phase-C
ZC → the next window's ZC drifts toward the gate → miss → ZC-miss
cascade → 11 A monster → bus sag. Same monster mechanism as amp-42, now
at 85-91 µs windows where the fixes (tuned at ~130 µs) can't catch it.

## Levers (for the analysis — not yet done)

1. **Cut the `elapsed` latency (~10 µs)** — the biggest fixed cost. Parts:
   the persistence loop, the estimator work before the elapsed read, the
   confirm, and the LPTIM2 setup (bounce + ARROK + delay(200), ~3 µs).
2. **Lower the delay floor (8 µs)** — GP-timer one-pulse (TIM7/TIM16)
   removes the LPTIM overhead; parked per operator.
3. **Advance vs floor tension**: advance 20° shrinks the delay budget to
   15 µs. Lower advance → bigger budget (less floored) but the ZC crowds
   the gate (worse detection). They trade against each other at the top.
4. **Phase-C robustness** — it initiates the cascade; a tighter C
   confirm or dead-reckon accuracy would raise the perturbation floor.

## FREQUENCY-DOMAIN model — the ceiling is an f_e limit, not a motor/throttle limit

The controller's limit is set by FIXED latencies vs the shrinking window,
so it lives in **electrical frequency**, independent of the motor Kv or
the supply voltage. A higher-Kv motor or more volts just reaches the same
f_e ceiling at LOWER throttle; this motor at 8 V happens to hit it at
~amp 66-68. Window `T = 1e6/(6·f_e)` µs.

### Fixed latencies (measured; do NOT scale with T)

| cost | value | what it is |
|---|---|---|
| `elapsed` | **~15 µs** (12-18) | ZC-ISR-entry → schedule-call: persistence loop + estimator + confirm + the reads |
| LPTIM2 commutation ISR | **~22 µs** | set_six_step + mux + close_float_window + reschedule; shares prio 1 with COMP, so it's BLIND time — a ZC landing here waits |
| delay floor | **8 µs** | `LPTIM2_MIN_DELAY_US` (bounce + ARROK + delay(200)) |
| ZC jitter σ | **~7 µs** | roughly constant in µs |

### T-dependent budgets

- window = `T`; gate = `0.30·T`; ZC→commutation delay budget =
  `T·(30−adv)/60`. Advance auto-ramps to a 20° cap, so at speed the
  budget is `T·10/60 = T/6`.

### Where each margin hits zero

| margin | condition | T | **f_e** |
|---|---|---|---|
| schedule delay starts flooring | `T/6 − elapsed < 8` | ~120 µs | ~1400 Hz (obs. amp 55-58) |
| **schedule budget = elapsed (goes negative)** | `T/6 = 15` | **90 µs** | **~1850 Hz** |
| detection margin < 3σ | `0.23·T < 21` | ~91 µs | **~1830 Hz** |

**Both walls land at ~1850 Hz** — and it's the SAME cause: the ~15 µs
`elapsed` is (a) the whole delay budget at T=90, and (b) via the
resulting late commutation, what pushes the next ZC toward the gate.
This is exactly the observed break (amp 68 ≈ 1850 Hz). The +22 µs
commutation-ISR blind time (24 % of a 90 µs window) is the aggravator
that turns a phase-C wobble into a miss.

### Implication for the 95 % target and other motors

- Projected 95 % on THIS motor/voltage ≈ **2000-2200 Hz** (user eyeball).
  That is ABOVE the ~1850 Hz timing ceiling → **the controller can't
  hold 95 % here today, and it would break at ~1850 Hz on ANY motor**
  (higher-Kv / more-volts just arrive there sooner).
- For robustness we want headroom well above 2200 — say a 2500-3000 Hz
  ceiling — so a hotter motor or a fresh battery doesn't walk into the
  wall.

### Levers, sized to a target ceiling

Ceiling ≈ where `T/6 ≈ elapsed` (+ the floor and detection track it):

| target f_e | T | need budget `T/6` > elapsed → elapsed ≤ | how |
|---|---|---|---|
| 1850 (now) | 90 | ~15 µs | current |
| 2200 (95 %) | 76 | ~11 µs | cut elapsed ~4 µs |
| 2500 | 67 | ~9 µs | cut elapsed ~6 µs + floor→~4 |
| 3000 (margin) | 56 | ~7 µs | cut elapsed ~8 µs + floor→~2 (TIM16) |

Reducible pieces of `elapsed` (~15 µs): the persistence loop (12 reads
at speed), the estimator work done before the elapsed read, and the
LPTIM2-ISR blind time bleeding into it (the "LPTIM diet" lever). Plus the
8 µs floor (GP-timer one-pulse, parked).

**One re-tuning lever that needs NO latency cut:** advance vs the two
margins. Advance at the 20° cap sets the budget to `T/6` (small) to buy
detection margin (ZC lands later, away from the gate). But at the ceiling
the SCHEDULE budget is the binding constraint. A lower advance at the
very top gives a bigger budget (`T·(30−adv)/60`) at the cost of detection
margin — there is an optimum advance-vs-f_e profile that balances the two
walls, and it currently over-weights detection. Worth a sweep: hold amp
~62-66 and step advance down, watch qzc% (detection) vs the ACC-delay
floor (schedule) — the crossover is the better cap.

## TIM15 swap — DONE (2026-07-13, floor 8→2 µs)

Swapped the commutation one-shot LPTIM2 → **TIM15** (the L431-clean GP
timer: own `TIM1_BRK_TIM15` vector, stays priority 1. NOT TIM16 — its
vector is the ADC ISR's `TIM1_UP_TIM16`, which would force an ISR merge +
priority collapse; NOT TIM17 — doesn't exist on L431). TIM15 is a 1 µs-
tick APB2 timer with no clock-domain ARR sync and no warm-up, so
`schedule_us` = stop→ARR→UG→start delivers a **2 µs floor** (was 8 on
LPTIM2). `LPTIM2_MIN_DELAY_US` 8→2.

**Two integration fixes killed the naive-swap chop:**
1. **Prescaler reset (UG)**: a bare `CNT=0` left the prescaler mid-period
   → 0-1 µs per-commutation jitter → broke at amp 43. UG resets CNT +
   prescaler → phase-consistent tick → recovered to amp 60.
2. **FET-first ISR reorder**: `set_six_step` now fires before bb_record /
   counters (they were ahead of it, jittering the commutation instant) →
   reaches amp 64.

**Result**: naive swap broke at 43 → fixed build reaches **amp 64 /
1811 Hz, qzc 99-100%** (fresh climb) = ~LPTIM2 parity (66) WITH the floor
win. The floor win shows as the ACC delay firing ~9 µs earlier (owl
pred-bias −13 µs vs LPTIM2 −7 µs = commutations less late).

**Residual (honest)**: prediction jitter **2-3 % of window vs LPTIM2's
1.2 %**. It is bias-linked (firing ~9 µs earlier at the ceiling is a
tighter-coupled operating point), NOT a preamble artifact (the reorder
didn't move it). The loop rides it at 99-100 % qzc. Open question for a
future pass: is the jitter worth trading a bit of the floor win back
(raise floor 2→4-5) to halve it — or is it intrinsic to the earlier
firing? A floor-vs-jitter sweep would settle it.

## Residual-jitter A/B experiments (2026-07-13)

Question: why does TIM15 show ~2-3 % prediction jitter vs LPTIM2's 1.2 %?

**Ruled out — clock resolution.** LPTIM2 = 0.8 µs/tick, TIM15 = 1.0 µs.
Quantization σ = q/√12 → 0.23 vs 0.29 µs, a 0.06 µs difference. Not the
~1.5 µs seen. (Tick verified correct by exact low-speed lock frequency.)

**A1 — floor 2 vs 8 (bias-linked hypothesis): REFUTED.** If firing ~9 µs
earlier (floor=2) caused the jitter, floor=8 (firing later, ≈LPTIM2
timing) should be cleaner. Opposite happened: floor=8 gave MORE jitter
(6.6 % vs 4.7 % @ amp 45) AND a worse envelope (broke amp 45-50 vs 55).
So the earlier firing is NOT the jitter source — it's beneficial. The
floor-win commutation timing is good; the jitter is elsewhere.

**A2 — oversample off vs on (bus-contention hypothesis): CONFOUNDED.**
Arming the free-run oversample (DMA1 + ADC hammering APB2/AHB) reliably
broke the lock at amp 40 (2/2) where it held with oversample off (2/2).
Consistent with a peripheral-domain sensitivity — BUT arming also
re-introduces the known free-run↔injected ADC ch8 interference (the
+300 mA offset that's WHY it's gated off during lock), so this can't
separate pure APB2 bus-contention from ADC channel interference.

**Standing confound in the headline comparison.** The LPTIM2 "1.2 %
baseline" (`night2_5070`) used floor=8 AND the original ISR order; the
TIM15 build uses floor=2 AND the FET-first reorder. So "TIM15 = 2× jitter"
is not purely the timer swap. A clean isolation needs LPTIM2 rebuilt with
floor=2 + the reorder (everything matched but the timer).

**Net:** the residual is NOT the floor win / earlier firing (A1, decisive).
It's most likely a peripheral-domain effect — TIM15 lives on **APB2 with
the ADC/DMA**; LPTIM2 lived on **APB1**, isolated from that traffic (the
operator's "clock source / AHB bus" instinct, right flavor). Not yet
cleanly isolated from the ADC-ch8 interference. Follow-ups to nail it:
(a) LPTIM2 + floor2 + reorder control build; (b) a bus-load test that
doesn't touch the ADC (e.g. a memcpy DMA burst on AHB during lock).

## Follow-up experiments — DECISIVE (2026-07-13)

**(a) One-variable control: LPTIM2 vs TIM15, everything else identical
(floor2 + FET-first reorder, same ADC config).** Measured at amp 55-64:

| build | jitter @55 | jitter @64 | envelope | qzc |
|---|---|---|---|---|
| **LPTIM2** + floor2 + reorder | **0.9-1.0 %** | 1.4 % | amp 64 / 1805 Hz | **100 %** |
| **TIM15** + floor2 + reorder | 3.4 % | 2.3 % | amp 64 / 1811 Hz | 99 % |

**The jitter IS the timer.** Same floor, same reorder, same (oversample-
off) ADC → the ~2.5 % extra is purely the LPTIM2→TIM15 swap. This ALSO
resolves the A2 confound: with identical ADC config TIM15 still jitters
3×, so it is NOT the ADC-ch8 interference — it's the peripheral domain
(TIM15 on APB2 with the injected ADC's traffic; LPTIM2 on APB1, isolated).

**The floor win below ~6.5 µs is worthless here.** LPTIM2+floor2 (effective
~6.5 µs floor: the 4 µs schedule clamp + the delay(200) warm-up) and TIM15
(2 µs floor) reach the SAME envelope (amp 64). So TIM15's 4.5 µs-earlier
firing bought NOTHING for the top end (that limit is the sag/bus/detection,
not the floor) and cost 3× jitter. **LPTIM2+floor2 is strictly better.**

**(b) AHB bus-load test (mem-to-mem DMA, no ADC): BLOCKED.** Adding the
bus-load instrumentation shifted the flash layout into a no-engage regime
(the documented PRFTEN-off alignment sensitivity — even init-only code
moves the marginal engage; HEAD engaged fine on the same bench, the
instrumented build failed 4× every attempt). Not run. Not needed for the
verdict: (a) already isolated the jitter to the timer with the ADC
controlled — (b) would only have confirmed the *mechanism* is specifically
APB2 contention vs another timer-intrinsic property.

**DECISION: revert the TIM15 swap → LPTIM2 + floor2 + reorder** (this
commit). Captures the useful part of the "8→2" (8→~6.5 µs, all the
envelope the floor can buy) at 3× less jitter and qzc 100 %. TIM15 lives
in git history (`checkpoint`-able) if a true sub-6 µs floor is ever needed
AND the APB2 jitter is addressed (e.g. move the ADC off APB2, or arm the
timer from an APB1-side write).

## close_float_window → main: ATTEMPTED, NET-NEGATIVE, REVERTED (2026-07-13)

Goal: move the WindowRec build + serialize off the priority-1 commutation
ISR (at 3 kHz the 19.6 µs ISR exceeds the ~16 µs gate and eats detection).

**Decomposition first (measured, amp 50):** whole ISR `lp2` ≈ 1569 cyc
(19.6 µs); `close` ≈ 574 cyc non-build (4/5 windows) / ~824 build (1/5);
`ticks_both` only 46 cyc. So the deferrable *build* is 1/5 windows and its
*serialize was already in main* — the 574 cyc bulk is per-window CONTROL
(miss/reacq/span + control-critical resets) + non-inlined cross-crate call
overhead, most of which cannot move.

**Implemented the clean architecture anyway** — a double-buffer (bank flip,
no copy): core split into `window_control_step` (ISR: exact control, proven
byte-identical by `split_matches_monolith` over healthy/miss/cascade/
C-window/stream-off/decimated) + `build_window_rec` (main: reads the
completed diagnostic bank). Firmware banked the 7 diagnostic accumulators,
flipped `WIN_BANK` at each close, handed `{bank, CloseScalars}` to main via
a queue.

**Bench verdict: the commutation ISR grew +186 cyc (1569 → 1757), amp 50
broke.** The handoff plumbing — per-call `window_control()` struct build
(16 refs, because `raw` is now a dynamic bank ref so it can't be a const
static) + the cross-fn `control_step` call returning `CloseScalars` by
value + the `PendingClose` enqueue + the flip — costs MORE than the moved-
out work (build 1/5 + 6 diagnostic resets) saves. **This is the SAME M4
lesson as lever-#2 v1 (`RawWindow` +74 cyc), now confirmed a second time:
on Cortex-M4 the deferred work is cheap and the struct/queue plumbing
dominates.** Reverted the firmware (lp2 back to 1570, locks restored). The
core split stays committed (`c9864c5`, proven-equivalent, harmless) in case
the plumbing is ever made cheap (LTO/inline) — but as-is, "move close to
main" is NOT a viable ISR-reclaim lever on this MCU.

**So the ISR-shortening path is NOT relocating close.** The real reclaim
is elsewhere: (1) the `elapsed` cut on the COMP path (not plumbing-bound);
(2) making the cross-crate close cheaper via inline/LTO (needs an engage-
layout re-validation); (3) shrinking the per-window control work itself
(gate pred_err to streaming-only, etc.) — a code-gen / arithmetic problem,
not an ISR-placement one.

### RE-DONE + INTEGRATED, corrected design (commit 60cb24a)

The +186 regression was caused by flipping+enqueuing EVERY window (5× the
baseline's 1/5 enqueue). Corrected: **flip+defer only on the 1/5 STREAMED
windows; clear the bank inline on the 4/5 non-streamed** (`clear_diag_bank`).
Plus `WindowControl` is now a **const static** (no per-window struct build;
the banked `raw` NOZ datum is passed by value). Result:
- **Locks amp 40/50/58 at qzc 100 %**, control proven byte-identical
  (`split_matches_monolith`); `lp2` uniform ~1685 — the build-window spike
  is GONE from the ISR (worst-case, the 3 kHz-relevant metric, improves).
- **Honest trade**: the typical non-streamed path is still +116 cyc vs the
  baseline non-build path — the cross-crate function-boundary tax
  (`control_step` + `build`/`clear` as separate non-inlined calls +
  `CloseScalars` returned by value). So worst-case better, average worse.
- **LTO** would inline the boundary away (net-positive everywhere) but
  shifts the flash layout into a no-engage regime (tried thin LTO +
  codegen-units=1 → engage failed; reverted). It needs a supervised
  engage-layout re-validation to adopt.
- **amp-64 envelope validation INCOMPLETE** — the bench lost power mid-test
  (probe: "target voltage 0.02 V"). Whether the +116 typical cost costs the
  top rung (baseline reached 64) is unresolved; re-check on a powered bench.

Net: the build+serialize ARE off the ISR (goal met) and it locks, but the
function-boundary tax makes it a worst-case-win / average-loss until LTO or
`#[inline]`-able core fns remove the boundary. Keep or revert is a judgment
call pending the amp-64 re-check + an LTO engage-validation.

### RESOLVED — envelope recovered (commit b718cab, post-fuse re-check)

The amp-64 re-check (fresh bench after a blown fuse) first showed a real
64→62 regression. Two wastes on the 4/5 NON-streamed windows were the tax,
both fixed:
1. `window_control_step` computed + returned ALL record scalars
   (pred_err/zc_off/qzc_off/len) even on non-streamed windows where they're
   discarded → now **gated on `record`** (default scalars otherwise).
2. The non-streamed bank clear went via `window_diag()` + a cross-crate
   `clear_diag_bank` call → now **7 direct inline stores** in the firmware.

**Result: reaches amp 64 (qzc 99 %, ≈ baseline's 100 %) — envelope-neutral.**
`lp2`: non-streamed ~1621, streamed ~1768 — the streamed worst case is now
BELOW the baseline's ~1819 build-window, so the **worst-case ISR is shorter**
(the actual 3 kHz lever) at ~envelope-parity today. The build+serialize are
off the commutation ISR, control proven byte-identical. **Verdict: the
double-buffer is the right pattern (worst-case-positive, envelope-neutral);
the earlier "net-negative" was purely un-optimized split overhead.** A clean
average-win too still needs LTO/inline to erase the residual boundary tax
(needs the engage-layout re-validation).

## LTO spike — RESOLVED, LTO ON (2026-07-14, commit 02bcf22, tag `checkpoint-lto`)

**The "LTO breaks engage" mystery was a stack/.bss overlap, and it was a
latent landmine in EVERY build.** Forensics on the failed LTO session logs
found 5 boot banners per run — the board was CRASH-LOOPING (panic/fault →
IWDG 1 s → reboot at every `q`; motor never moved), not failing to engage.
objdump then showed main's prologue reserving **14,656 B of stack** against
~12.8 KB of headroom (48 K RAM − 35.2 K bss): LLVM merges the `j`-handler's
14 KB of match-arm snapshot arrays into main's prologue frame, so the deep
frame overlapped the top of .bss from boot (the linker even defines
`_stack_end` right there; nothing enforces it). **Non-LTO builds survived
by link-order luck** — benign telemetry statics happened to sit at the top
of bss; LTO reshuffled bss and put critical state under the stack.

Fix: **freeze-and-dump-in-place** (`CTX_FREEZE` pauses the TIM1_UP ring
writes for the ~110 ms `j` dump; `wax_cdump` reads the CTX rings + status
directly — no snapshot). Main frame 14.7 KB → 4.6 KB (both builds).

Result with `lto = "thin"` + `codegen-units = 1`: zero reboots across 5
ladder runs, engage first-try, **ladder 50/58/62/64 all qzc 100 %** (the
best single ladder either build has produced; 2/5 runs to the top vs the
non-LTO 2/4 — lottery parity, no regression), `lp2` 1658–1757 vs 1708–1823
(a small real win), text −1.2 %, `j` dump verified. The flash-layout
"engage sensitivity" attributed to LTO (and to the bus-load build, which
crash-looped for the same reason) is EXPLAINED and gone.

## `elapsed` re-measurement + decomposition (2026-07-14, LTO build)

Re-measured the ZC→schedule latency on the `checkpoint-lto` build (bb ACC
temp instrumentation, reverted after):

- **`elapsed` = 16–18 µs (median 16–17)** at amp 50 and amp 64 — LTO did
  NOT move it (pre-LTO: 15–19, median 18). Confirms the ceiling math: at
  amp 64 (T≈92.7 µs) the schedule budget `T/6` = 15.4 µs < elapsed → the
  budget is fully consumed by our own latency; every delay floors. **The
  ~1800 Hz ceiling IS the elapsed wall.**

- **Decomposition (amp 60, n=32, packed bb word):**
  | segment | µs | contents |
  |---|---|---|
  | COMP tail (ISR entry → accept-fn entry) | **10–12** | persistence loop (~4 µs @ 12 reads) + pre-accept ISR body: blanking math, EDGE_BUF store, sector counters, MAGPIE valid/first-zc, atomics (LDREX/STREX RMWs) |
  | accept-path compute (before the ts read) | **6–8** | accept_publish: estimator load-run-store, publishes, advance calc |
  | total | 17–19 | (instrumentation adds ~1) |

Cut targets this sizes (the next lever, NOT yet done):
1. **Accept path (~7 µs): schedule FIRST, estimator after** — compute the
   delay from the PRE-update interval (identical at steady state, one
   window stale during accel) and call `schedule_us` before the estimator
   work, mirroring the FET-first commutation-ISR reorder. Reclaims most
   of the 6–8 µs.
2. **COMP tail (~11 µs): accept-dispatch before diagnostics** — the
   edge-buf/sector-counter/MAGPIE stores run before the persistence loop
   today; move them after the accept path so the hot ZC→schedule path is
   gate-check → persistence → accept only. The persistence loop itself
   (~4 µs at 12 reads) is the noise defense — touch last.

Realistic combined: elapsed 17 → ~7–9 µs → moves the `T/6` wall from
~1800 Hz to ~2.4–2.6 kHz. The commutation-ISR blind time (~21 µs) becomes
the next binding wall at ~2.2–2.5 kHz (the priority-split/pend idea).

## Schedule-first, estimator-after — LANDED (2026-07-14, commit b7afb8e)

The `elapsed` cut, executed: the accept path now arms the LPTIM2 shot
FIRST (from `zc::schedule_precheck`, host-tested to agree with
`accept_publish`'s schedule decision across the full matrix, incl. the
engage `cl_armed` case) using the PRE-update interval, then runs the
estimator/publish/engage off the hot path. Both accept callers funnel
through the one site.

**Measured result:** ACC delays at amp 62 = **9–13 µs, varying** (pre-cut:
`elapsed` 16.5 µs exceeded the whole budget → every delay floored) →
back-computed `elapsed` ≈ **8–11 µs** (the 6–8 µs accept-compute is off
the path; the ~10–12 µs COMP tail remains). **The schedule wall moves
~1800 Hz → ~2.5–2.9 kHz**; the commutation-ISR blind time (~21 µs) now
binds first (~2.2–2.5 kHz) — next lever: accept-dispatch-before-
diagnostics in the COMP ISR (~5–7 µs more), then the priority-split idea.

Regression check: 3/3 ladders to ≥62 qzc 100 % (1/3 to 64; the LTO
baseline was 2/5 with two breaks at 50–58), jitter 1.0 % (= best),
pred bias −13…−15 % → **−9.7 %** (commutations measurably less late).
126 core tests (`schedule_precheck_agrees_with_accept_publish`).

## Accept-dispatch-before-diagnostics — LANDED (2026-07-14, commit 412db65)

Lever 2 of the `elapsed` cut: the COMP ISR's diagnostic stores (EDGE_BUF,
sector counters, VALID/WINDOW_VALID/FIRST_ZC — atomic RMWs) ran before
the persistence + accept dispatch. Reordered to filters → persistence →
accept → diagnostics; every reject path keeps its exact previous counter
behavior (host-visible telemetry unchanged).

**Measured: ACC delays @ amp 62 = 12–14 µs (mean 13.1)** vs lever-1-only
9–13 (mean ~11) → **`elapsed` ≈ 7 µs cumulative** (was 16.5 pre-lever-1).
The schedule wall is now **~3.5+ kHz — past the commutation-ISR
blind-time wall (~2.2–2.5 kHz)**, so `elapsed` cutting stops here per
plan; the persistence loop stays untouched (noise defense).

Regressions: none — ladders {64, 62, 64} (parity), qzc 100 % every locked
rung, jitter 1.4–2.1 %, pred bias improved again (−9.7 % → −7.5…−9.0 %),
zero reboots.

**Remaining path to 3 kHz: ONE wall — the commutation-ISR blind time
(~20 µs at priority 1).** The sized approach: priority-split — keep
actuation (set_six_step + mux, ~5 µs) at prio 1, NVIC-pend the
control/close tail to a lower-priority vector (~30 cyc handoff, no queue
plumbing — the M4-cheap alternative to "move to main").

## Data files (`captures/`)

- `night2_5070_map.png`, `_dropout.png`, `_a{50,56,62,66}.bin`, `_t68.bin`
- `marg{58,62,66}_*_bb.txt` (ACC delays floored at 8), `rawd66_*_bb.txt`
  (pre-floor ideal ~5 µs)
- `owl_report --infile` on the a/t bins for per-sector tables
