# WCET estimates: firmware50 ISR roots and the timing budget to 100% duty

Written 2026-09-22, on image E131 `2A7694CD` (`captures/elf/2A7694CD.e131.elf`),
after campaign 5. The pre-power qualification figures (notebook E130–E131)
count cycles at **0 flash wait states**. This document works out what that
leaves out, and how many microseconds are left per commutation step as the
rotor speeds up. It is an estimate: every figure says whether it was
**measured**, **modelled** or **extrapolated**.

The prefetch step that follows from it, E133, is recorded in `LAB_NOTEBOOK.md`
and summarised in the last section.

## 1. The machine

| fact | value | source |
|---|---|---|
| core | Cortex-M0+, 64 MHz (HSI16 → PLL), 15.6 ns / cycle | `bin/board.rs` `init`, `Config::pll()` |
| FLASH_ACR | `0x00040602`: LATENCY = 2 wait states, **PRFTEN = 0 (prefetch off)**, ICEN = 1 (instruction cache on), DBG_SWEN = 1 | read over SWD, bench idle, 2026-09-22 |
| COMP (`ADC_COMP`, IRQ 12) | NVIC 0x40 | notebook E081 gate-5 table |
| COM (`TIM16`, IRQ 21) | 0x40 | same |
| DMA (`DMA1_CHANNEL1`, IRQ 9) | 0x40 | same |
| guard (`TIM6_DAC_LPTIM1`) | **0x00**: it preempts all three | same |

COM and DMA share COMP's priority. They cannot preempt it, but either can make
it wait:
* one of them may already be running when a comparator edge arrives;
* DMA has the lower IRQ number, so it wins a tie in pending order.

Only the guard can preempt COMP.

## 2. The cycle model

`scripts/isr_cycles.py` builds each root's control-flow graph from the linked
ELF and takes the longest path. It charges Cortex-M0+ instruction costs:
* ALU 1;
* LDR/STR 2;
* PUSH/POP 1 + registers (+2 for a POP into PC);
* taken branch 3, not-taken branch 1;
* BL 4 plus the callee's cost;
* MRS/MSR 4.

Loops must be given bounds; a loop without one is refused. COMP has two loops,
both bounded by the persistence-filter read count.

**The filter read count follows speed** (`bemf::mapped_filter_level`, AM32's
`map()`). It runs from 3 reads at an average interval of ≤ 100 half-µs to 12
reads at ≥ 500 half-µs, linearly in between (`+ (x−100)·9/400`):

| rung | eHz | ci µs | filter reads |
|---|---|---|---|
| 10% | 396 | 421 | 12 |
| 15% | 704 | 237 | 11 |
| 20% | 941 | 177 | 8 |
| 25% | 1186 | 141 | **7** |
| 50% (binz) | 2096 | 80 | 4 |
| ≥ ~2.4 keHz | — | ≤ 69 | 3 (floor) |

### 2.1 Adding flash wait states (`--fetch-model`)

The 0-wait-state figure ignores the flash. With 2 wait states and prefetch
off, a fetch that misses the instruction cache stalls the pipeline. The
optional fetch model in `isr_cycles.py` (its defaults keep the 0-wait-state
output unchanged) adds W = 2 cycles:

* per literal-pool load (`ldr rX, [pc, #…]`), a data read from flash;
* per taken branch, to refetch the target;
* per `--per-insn` × every instruction, the sequential miss rate:
  * 0.25 is one miss per 64-bit line of thumb16 code;
  * 1 means every instruction misses;
* `--loop-hit` treats a loop's second and later trips as cache hits.

What the model does not include:
* extra cycles for peripheral loads through the APB bridge (COMP's comparator
  reads are such loads), so every row below is slightly low;
* exception entry and exit, about 15 + 13 cycles plus fetch stalls;
* any foreground critical section (`interrupt::free`, `Seam` locks). That
  delays COMP entry by up to its length, and this tool does not bound it.

### 2.2 Results on E131, cycles (µs at 64 MHz)

| model | COMP 12 reads | COMP 7 reads (25%) | COMP 4 reads | COMP 3 reads | COM | DMA | guard |
|---|---|---|---|---|---|---|---|
| 0 wait states (qualification figure) | 1026 (16.0) | 866 (13.5) | 770 | 738 (11.5) | 287 | 78 | 204 |
| W = 2, literal loads and taken branches only | 1218 | 1038 | 930 | 894 | 341 | 84 | 245 |
| **W = 2, one miss per 4 instructions, loop repeats cached ("plausible")** | 1372 (21.4) | **1212 (18.9)** | 1116 | **1084 (16.9)** | **420 (6.6)** | **101** | **301** |
| W = 2, one miss per 2 instructions | 1881 | 1581 | 1401 | 1341 | 499 | 118 | 358 |
| **W = 2, every instruction misses ("pessimistic")** | 2546 (39.8) | **2126 (33.2)** | 1874 | 1790 (28.0) | 657 | 152 | 472 |

Reproduce, for example:

```
python scripts/isr_cycles.py --elf captures/elf/2A7694CD.e131.elf --root ADC_COMP \
    --loop-bound 7 --loop-bound 7 --fetch-model --per-insn 0.25 --loop-hit
```

For comparison, E128's COMP under the plausible model is 1512 / 1346 / 1214 at
12 / 7 / 3 reads (0-wait-state: 1176 / 1011 / 912). E131's removal of the
masked atomics saves about 130–140 cycles on every path in both models.

## 3. Cross-check against the on-chip measurements (25%, E131)

The firmware stamps TIM17 (1 MHz, 1 µs resolution):

| measured | E131 at 25% | what it covers |
|---|---|---|
| `spent_max_us` (R_COMP) | **13 µs** | COMP entry stamp → the COM one-shot armed |
| `comp_call_max_us` | **18 µs** | entry stamp → end of the decision, **including** any guard preemption |
| `late_arms` | **0** | arms that found the wait already elapsed |
| `com_late_max_us` | 5–8 µs | lateness of the commutation itself |

A guard preemption costs about 301 cycles under the plausible model, plus
about 30 cycles of entry and exit: roughly 5 µs, or up to about 8 µs
pessimistic. Take that out of 18 µs and COMP's own call time is about
10–13 µs, which is ≤ 18.9 µs (plausible, 7 reads). It sits between the
0-wait-state figure and the plausible bound. That is expected: the longest
modelled path assumes both filter loops run the full count in one call.

## 4. Response time and the wait window at 25%

### 4.1 Worst-case edge → end of COMP (plausible model, 7 reads)

| term | cycles |
|---|---|
| blocking: TIM16 already running | 420 |
| blocking: DMA pending, wins the tie | 101 |
| COMP exception entry | ~20 |
| COMP body | 1212 |
| one guard preemption + its entry/exit | ~330 |
| **total** | **≈ 2080 cycles ≈ 32.5 µs** |

This excludes foreground critical sections (§2.1).

### 4.2 The wait

Commutation is armed for

```
wait_time(ci, level) = (ci >> 1) − advance_of(ci, level)
advance_of(ci, level) = (ci >> 6)·level + (((ci & 63)·level) >> 6)   ≈ ci·level/64
```

so wait ≈ ci·(32 − level)/64. Level 20 (firmware50) → wait ≈ 0.1875·ci.
Level 26 (what binz needed at 50%) → wait ≈ 0.094·ci.

At 25% the E131 hold intervals are ci 138–142 µs, which give a wait of 26–27 µs
(26 at ci 138, 139 and 141; 27 at 140 and 142).

| R_COMP at 25% | µs | against a 26 µs wait |
|---|---|---|
| measured | 13 | fits, 13 µs to spare |
| plausible bound (whole body, 7 reads) | 18.9 | fits |
| pessimistic bound | 33.2 | **does not fit** |

**At 25%, the margin is established by measurement and the plausible model.
It is not established by a strict static bound.**

## 5. Speed against duty, and the extrapolation to 100%

### 5.1 Data

| duty | eHz | source |
|---|---|---|
| 10% | 396 | firmware50 / oracle (401) |
| 15% | 704 | firmware50 / oracle |
| 20% | 941 | oracle (firmware50 E131 947–951) |
| 25% | 1186 | oracle (firmware50 E131 1181–1185) |
| 40% | 1727 | binz, `GRAYBEARD_TRANSIT_SURGE.md` (E809) |
| 45% | 1926 | binz, `DUTY_50_CAMPAIGN.md` |
| 49% | 2064 (2044–2083) | binz, `GRAYBEARD_SAG_REFERENCE.md`, advance 24 |
| 50% | 2096 | binz 2026-09-19 final, advance 26, ~1.6 A |

The slope falls from about 62 eHz per % of duty (10→15%, where friction takes
its share) to about 49 (15→25), about 36–40 (25→45) and about 34 (45→50).

The 40/45/49% points come from binz's 50% campaign. Which motor they ran on is
not checked: motor #3 went in on 09-19, and only the 50% point is certain to be
on it. Fit (b) below leaves them out and agrees within 15 eHz.

### 5.2 Model

For steady state with a propeller load (drag torque ∝ ω²):

```
V·d = Ke·ω + R·I,   I ≈ I0 + c·ω²     ⇒   d = a + b·ω + c·ω²
```

The linear term is back-EMF, the square term is prop drag through the winding
resistance, and the constant is friction and offset. Least squares, then solve
d = 1:

| fit | points | max residual (duty) | ω at 60% | 75% | 90% | **100%** |
|---|---|---|---|---|---|---|
| (a) all | 8 | 0.7% | 2397 | 2817 | 3199 | **3437** |
| (b) firmware50 15–25% + binz 50% | 4 | 0.2% | 2400 | 2814 | 3189 | **3423** |
| (c) all but 10% | 7 | 0.6% | 2412 | 2853 | 3258 | **3512** |
| straight line (no taper) | — | — | — | — | — | 4070–4190, upper bound |

Coefficients of (a): a = 0.0404, b = 1.278e-4 per eHz, c = 4.41e-8 per eHz².

**Estimate: about 3.4–3.5 keHz at 100%, extrapolated.** This assumes the bus
holds about 11.5 V. Power grows about as ω³. The 50% point draws about
18 W (1.6 A from the PSU), so 100% would draw about 18 × (3437/2096)³ ≈ 80 W,
about 7 A at 11.6 V. The resulting sag pulls the real top speed below the fit.
The bench's 1 A PSU limit caps this rig at roughly 30–35% of the curve.

## 5a. What `spent_max_us` measures, and what it leaves out

**Correction, 2026-09-23, after a review by the binz author agent.** Earlier
sections of this document treated the measured `spent_max_us` as the
edge-to-arm latency and quoted a margin against the wait from it. It is
narrower than that, and it is a measured maximum, not a bound.

`roots.rs:365` stamps `spent` from the handler's own entry stamp `raw` to the
instruction **before** `com_arm()`. So the figure excludes, at both ends:

* **before the stamp:** the comparator-to-EXTI-to-NVIC dispatch, any blocking
  by the same-priority COM and DMA roots or preemption by the guard, the
  exception entry, and the two register writes of `line_disable()` and
  `clear_pending()` that precede `let raw = …`;
* **after the stamp:** `com_arm()` itself -- a clock read, two shared stores
  and the TIM16 register writes. `com_arm` is inlined into the root, so it has
  no symbol of its own and this cost is not separately measured; the
  instruction count puts it near 1 µs with wait states.

What the firmware does measure end to end:

| measured | at 37.5% | what it covers |
|---|---|---|
| `spent_max_us` | 11 µs | entry stamp → before the arm |
| `late_arms` | **0** | arms that found `wait − spent` already exhausted |
| `com_late_max_us` | 10–11 µs | the one-shot firing later than the instant `com_arm` scheduled, so it includes TIM16 dispatch and any blocking |
| `comp_call_max_us` | 15 µs | entry stamp → end of the decision, including a guard preemption |

So the honest statement of the gate is **empirical, not arithmetic**:
`late_arms` was 0 in every run of every rung, which is the firmware itself
reporting that the wait had not elapsed when it armed. The "4 µs of margin" an
earlier draft quoted at 37.5% overstates what is established, because it
compares the wait against the narrow figure only.

Separately, `com_late_max_us` of 10–11 µs says the commutation lands about
10 µs after the instant it was scheduled for, at every rung. At 37.5% that is
roughly 6 electrical degrees of advance lost. It did not break the loop -- the
speed and rate-identity gates passed with it -- but it is the largest single
term in the chain and it is not part of R_COMP.

**A trap worth naming, because this document walked into it** (notebook E142).
Since the figure's start point excludes the arming path, *moving the start
point later in the handler improves the number without improving anything*.
Campaign 7's first step reordered the acceptance arm so the timer was armed
before four bookkeeping stores; the stamp moved ~1 µs earlier, `spent_max_us`
duly fell from 11 µs to 10, and the rotor at 37.5% got **0.65% slower**,
because the compiler left the stores between the stamp and the write while
`left = wait − spent` still assumed the write happened at the stamp. The
commutation therefore landed 1 µs late and gave away that much advance. The
A/B against the archived image caught it; the improved number alone would have
banked a regression as a win. E144's answer is to compute the remaining wait
from a clock read taken **at** the write (`com_arm_at`), so nothing can sit in
between and the reported figure covers the path it claims to.

**To establish a real bound** the stamp has to move to the first instruction of
the root, before the masking writes, with a second stamp after `com_arm`, or
the edge has to be timestamped in hardware (a TIM2 input capture on the
comparator output, which binz has a path for). Both change ISR code and need
their own qualification; neither was done in campaign 6. That is the first step
for any campaign above 37.5%.

## 5b. Measured R_COMP per rung, campaign 6

Every campaign-6 run reports `spent_max_us` = 11 µs on the E135 line of images
(prefetch on), at every rung tried, and `late_arms` = 0 in all of them. Against
the wait at the measured interval, with the advance schedule's level 20 below
35% duty:

| rung | measured `ci` µs | wait µs (level 20) | R_COMP µs | margin µs |
|---|---|---|---|---|
| 25% | 139 | 26 | 11 | 15 |
| 27.5% | 125–128 | 23–24 | 11 | 12–13 |
| 28.8% | 123 | 23 | 11 | 12 |
| 30% | 120 | 23 | 11 | 12 |
| 32.5% | 113 | 21 | 11 | 10 |
| 33.8% | 112 | 21 | 11 | 10 |
| 35% | 116 | **19** (level 22 from 35% duty) | 11 | 8 |
| **37.5%** | **97** | **15** | **11** | **4 on the narrow figure** |

The margin column compares the wait against `spent_max_us` only, so read it
with §5a: the established result at each rung is `late_arms` = 0, not the
arithmetic difference.

Campaign 6 qualified every rung to 37.5% (notebook E137–E141) with
`late_arms` = 0 throughout, so up to there the arm demonstrably beat the wait.

Extending the arithmetic on the narrow figure alone, 11 µs meets the level-22
wait at `ci` ≈ 70 µs, about **2.4 keHz or roughly 56% duty** (`wait_time(70,
22)` = 11; at `ci` 59 the wait is already 9). Because the narrow figure omits
the terms in §5a, the real crossing is **at or below** that, and the only
honest way to find it is to measure a complete edge-to-arm figure first.
An earlier draft of this document put the crossing at `ci` ≈ 59 µs / 2.8 keHz /
75% duty; that was arithmetically wrong as well as based on the narrow figure,
and it is withdrawn.

So gate 4 held at every rung attempted, including the two that failed on
tracking: **the detector's timing margin is not what limits this campaign.**
What limits it is a swallowed crossing desyncing the drive (notebook E138), a
signal-and-recovery problem rather than a latency one. The crossing point in
§6 stands, with the crossing corrected: on the narrow figure 11 µs meets the
level-22 wait at ci ≈ 70 µs, about 2.4 keHz, still well above where the
tracking failures start -- and per section 5a the true crossing is at or below
that, because the figure omits the dispatch, masking and arming terms.

## 5c. Campaign 7: what the response time permits, and what it forbids

Two results from campaign 7's prerequisites (notebook E142–E147) bear on every
figure in this document.

**The response time is 11 µs and does not reduce by re-stamping.** Arming the
one-shot before the acceptance bookkeeping moved the measured figure to 10 µs
and cost 1% of rotor speed at 37.5%, where the remaining wait is only 5–6 µs;
compensating at the write did not recover it. Both variants were reverted, so
`spent_max_us` = 11 µs stands as the qualified figure, and the honest reading
of the gate is `late_arms` = 0 rather than the arithmetic difference (§5a).

**That forbids the oracle's high-duty advance.** With R_COMP 10–11 µs and the
goal's ≥ 3 µs margin rule, the highest admissible advance level is:

| rung | oracle eHz | `ci` µs | wait at 22 | at 24 | at 26 | highest admissible |
|---|---|---|---|---|---|---|
| 37.5% | 1650 | 101 | 16 | 13 | 9 | 24 |
| 40% | 1736 | 96 | 15 | 12 | 9 | 23 |
| 45% | 1893 | 88 | 14 | 11 | 9 | 23 |
| 50% | 2096 | 80 | 13 | 10 | 8 | 22 |

So level 26 -- what the frozen oracle applies above 35% through its post-COM
wait override -- is never admissible here, and the schedule stays at 20 below
35% duty and 22 above. That is a **named divergence from the oracle**: its
images carry a different ISR budget, and copying the override on this firmware
would arm late every sector, which is the late-commutation braking this
document's own §5a describes at one-sixth the size.

**The steady bus droop is about −1.0% per amp**, measured at 25 / 30 / 35 /
37.5% from the sag guard's filtered reference (notebook E146): −0.71% at
0.399 A metered, −1.21% at 0.890 A metered, so roughly 0.12 Ω of supply and
path. The transient `bus_min` reads −7.7 to −8.6% at *every* rung with no
trend in current, so it is a switching artefact and not sag; the earlier
"about 11% per amp" slope fitted to those minima is withdrawn (§5a's
correction list).

### 5d. Measured per rung through 50% (campaign 7)

`spent_max_us` = **11 µs** at every rung from 15% to 50%, so the figure does
not grow with speed; the wait does shrink.

**Correction (notebook E152): `late_arms` is not zero everywhere.** An earlier
version of this section said it was, and the campaign leaned on that. Across
588 production captures it is zero in 584 and non-zero in four:
`c7-500_01` (1), `c7-500c_01` (1), `c7-500c_03` (2) and -- in a *qualifying*
45% run -- `c7f-450_02` (1). So the arm does occasionally miss the wait at 45%
and 50%, and neither a 3 µs arithmetic floor nor a "late_arms = 0" event gate
is satisfied by this firmware at the top rungs.

| rung | measured `ci` µs | advance level | wait µs | R_COMP µs | margin |
|---|---|---|---|---|---|
| 40% | 95 | 22 | 15 | 11 | 4 |
| 42.5% | 87 | 22 | 14 | 11 | 3 |
| 45% | 84 | 22 | 14 | 11 | 3 |
| 47.5% | 80 | 22 | 13 | 11 | **2** |
| 50% | 77 | 22 | 13 | 11 | 2 |

At 47.5% and 50% the margin on the narrow figure is 2 µs, **below campaign 7's
own 3 µs floor**: 47.5% met every other gate 3/3, and 45% is the highest rung
that also meets the margin clause. That is stated as a tension, not resolved
here (notebook E150). It is also why the oracle's advance of 24–26 was never
admissible (§5c) -- and why 50% could not be helped by the one lever that
addresses slip. Note this is the
**narrow** figure (§5a): the true edge-to-arm delay is larger, so these margins
are upper bounds. The largest *measured* term in the chain is not in this
figure at all -- `com_late_max_us`, the one-shot firing 8–11 µs after the
instant it was scheduled for, which advance does not change and which
`spent_max_us` does not cover (E152).

## 6. The budget per commutation step, 25% → 100%

`ci = 1e6 / (6·eHz)`; wait from §4.2 (integer formula); filter reads from §2;
COMP and COM from §2.2 (plausible model).

**The advance level firmware50 actually uses** is `run::policy::AdvancePolicy`:
20 below 35% duty and **22 at or above** (the qualified image's schedule,
`binz/AGENTS.md:133`). Level 26 is binz's override at 50%, not this firmware's
policy, and is shown only for comparison.

| point | eHz | ci µs | reads | wait, **level 22** (level 20 / 26) | COMP µs | COM µs | COMP + COM share of ci |
|---|---|---|---|---|---|---|---|
| 25% | 1186 | 141 | 7 | (26) / 13 | 18.9 | 6.6 | 18% |
| 40% | 1727 | 97 | 5 | **15** (18 / 9) | ~17.9 | 6.6 | 25% |
| 50% | 2096 | 80 | 4 | **13** (15 / 8) | 17.4 | 6.6 | 30% |
| ~60% | 2400 | 69 | 3 | **11** (13 / 6) | 16.9 | 6.6 | 34% |
| ~75% | 2817 | 59 | 3 | **9** (11 / 6) | 16.9 | 6.6 | 40% |
| **100%** | **3437** | **48** | **3** | **8** (9 / 5) | 16.9 | 6.6 | **49%** |
| 100%, linear bound | 4100 | 41 | 3 | **6** (8 / 4) | 16.9 | 6.6 | 57% |

**Measured R_COMP does not scale with the filter depth.** Across every E131
rung, `spent_max_us` is 13 and `comp_call_max_us` is 18, at 11 reads (15%,
ci 235), 8 reads (20%, ci 174) and 7 reads (25%, ci 139) alike. With prefetch
(E133) both drop together, to 10 and 14–15, again independent of depth. These
are maxima, so they are set by the worst instance — which includes one guard
preemption and 1 µs of quantization — not by the average read count. So the
table's modelled COMP column falling from 18.9 to 16.9 µs as the depth drops
to 3 is **not** something the measurement supports: the right expectation at
100% is the same 13 µs (10 µs with prefetch) unless the arm path is changed.

The share column counts only one accepted COMP call and one COM call per step.
Chatter entries (refused COMP calls) come on top: at 25% the entry peak
reached 64/ms, which would be about 3 per step at 100%.

**Where it breaks.** Take R_COMP as the measured 13 µs, which does not fall
with depth. The wait window drops below it at about:

* **level 22** (this firmware above 35% duty): ci ≈ 83 µs, **≈ 2.0 keHz, about
  48% duty**;
* **level 20:** ci ≈ 70 µs, **≈ 2.4 keHz, about 60% duty**;
* **level 26** (binz's override): ci ≈ 138 µs, **≈ 1.2 keHz, about 25% duty**.

With prefetch's 10 µs, level 22 reaches ci ≈ 64 µs, about 2.6 keHz. So
prefetch buys duty on this inequality but nowhere near 100%: the arm path
itself has to change. All of these are on the narrow figure (section 5a), so
they are upper bounds on the reachable duty, not predictions.

Advance and arm time compete for the same window. A late arm fires the
commutation (R_COMP − wait) late, which is advance lost: at 48 µs per step,
1 µs ≈ 1.24 electrical degrees. At 100% with level 20, R_COMP 12 against a wait
of 9 costs about 3 µs, or 4°. With level 26, 12 against 5 costs about 7 µs,
or 9°: most of the extra advance is spent before it can be applied.

**Levers, in the order this document suggests them:**

1. **Flash prefetch (E133).** Shrinks every wait-state term above without
   touching the code.
2. **Arm first.** Load the COM one-shot at the top of COMP, before the
   bookkeeping, so R_COMP becomes a few µs and no longer grows with the
   filter work.
3. **COM above COMP** at high speed, as binz did above 48%, so a commutation is
   never queued behind a COMP call.
4. **Trim the 3-read path**: the filter floor, rate and storm bookkeeping.

## 7. E133: flash prefetch on (a separate qualification step)

**Change.** `FLASH_ACR.PRFTEN = 1`, set by a named-field modify after the HAL's
clock setup. It is a modify, not a write, so DBG_SWEN keeps its value (that
trap is recorded in binz). No ISR instruction changes; the four roots must
disassemble identically to E131.

**What it should buy.** Prefetch fetches the next flash line while the current
one executes. Sequential code then stops stalling, which is the `--per-insn`
term. Branch targets and literal-pool loads still pay the wait states. The
expected result is that measured times move from the plausible row toward the
"literal loads and branches only" row of §2.2: COMP at 7 reads from about 1212
toward about 1038 cycles, roughly −2.7 µs.

**How the win is recorded.** Same bench, same session, E131 against E133, 25%
hot, per-run medians:
* `spent_max_us`, `comp_call_max_us`, `com_late_max_us`;
* foreground passes `loop_iters_closed`, a direct throughput measure for code
  running from flash;
* the campaign-5 behaviour quantities, for drift.

### Result (notebook E133): the win is real; the step fails qualification and is reverted

The image was `152C7299` (archived `captures/elf/152C7299.e133.elf`). FLASH_ACR
read `0x00040702` over SWD: PRFTEN on, DBG_SWEN kept. The four ISR roots are
instruction-identical to E131, so any change below comes from the fetch path.

| measured | E131 | E133 | |
|---|---|---|---|
| R_COMP `spent_max_us` | 13 | **10** | −3 µs, −23% |
| `comp_call_max_us` | 18 | **14–15** | −3 to −4 µs |
| `com_late_max_us` | 5 | 4 | |
| COMP entry peak, driven stage (same window) | 46–49 / ms | 47–58 / ms | faster handler, more chatter serviced |
| 15% hot | 3/3 PASS | **5/5 CompStorm** at 40–45 ms into the closed loop, peak 76–80 / ms against the 64 cut | ABAB in one session |

E133's maxima cover only about 40 ms of closed loop, so they are lower bounds.
They still agree with the −2.7 µs predicted above: prefetch removes most of the
sequential-miss term. In the tables of §2.2 and §6, the "literal loads and
branches only" row is roughly where firmware50 would sit with prefetch on.
Measured, that is an R_COMP of 10 µs, at every filter depth tested, against a
wait of 8 µs at level 22 at 100% — still short, though it moves the crossing
from about 48% to about 55% duty, on the narrow figure (section 5a).

**Why it is not kept.** Faster handling raises the rate at which COMP services
the HYST-0 comparator's chatter. Early in the closed loop, where the float
windows are long, that rate crosses the unchanged 64/ms storm cut. This is the
same mechanism as E095, E116c and E131. Taking the speed needs a decision on
the storm cut, the handover window, the comparator or COMP's re-enable
policy. Those are protection and detector choices, so they are the operator's
call. The production image stays E131 `2A7694CD`.

**For the 100% budget, this means** the "levers" list in §6 has a coupling.
Every lever that makes COMP faster also raises the chatter entry rate, so
chatter has to be handled first.

### E134 + E135: the chatter handled, then prefetch taken (both qualified)

**E134** holds the comparator line masked from the commutation to the blanking
floor -- the span in which the gate refuses everything anyway -- using the
one-shot the reverse blank already used. Measured at 15%: the entry peak falls
from 65–68 to 56–60 / ms, `too_early` from 520k to 0, and the foreground gains
25% more passes.

The E121 capture shows why: the dispatches in that span are about three per
sector, ~20 µs apart, and with the window masked **none of them latch a pending
flag** (`blank_latched` = 0 over 590k sector arms and three restarts). The early
chatter is not deferred, it stops happening -- an inference that the servicing
excites it, consistent with E095's monotonic peak-versus-handler-speed.

**E135** then turns prefetch on, and the whole campaign bar passes: 15/20/25
each 3/3, restart 3/3, every protection, `storm` 0 throughout, where E133's
prefetch alone failed 15% five times out of five.

Measured at 25%, E131 → E135: R_COMP 13 → **11 µs**, `comp_call_max_us`
18 → **15 µs**, foreground passes 1.198M → **1.982M**, `gt150` 3–4 → **0**, at
unchanged rotor speed. That matches this document's prediction of about −2.7 µs.

**Revised crossing point.** R_COMP 11 µs against the level-22 wait meets at
ci ≈ 70 µs, so the inequality holds to about **2.4 keHz** on the narrow figure
(section 5a), against roughly 2.0 keHz before these steps. Reaching 100% still requires the arm path to
change, as §6 says: arming the one-shot before the bookkeeping is the next
lever, and COM above COMP at speed after it.
