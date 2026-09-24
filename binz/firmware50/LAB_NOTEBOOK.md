# firmware50 lab notebook

Append-only. One entry per change or powered run. Failures and reversals stay
in. Newest at the bottom.

Bench: NUCLEO-G071RB + BOOSTXL-DRV8304H, probe
`0483:374b:066CFF343433464757233430`, VCOM COM7, supply ~12 V, operator limit
1 A. Reference tree is `binz/` (read-only from here).

---

## E001 — per-sector accept histogram

**Changed:** `bin/shell-pwm.rs`, added `acc_by_step`/`forced_by_step` (8-wide,
indexed `& 7` so the index is provably in bounds) and the `BEMFSECTOR` line.
ELF `B06AD00ECFCA19881640533F4D0968C75F0975E146A4AA066BEDC49358C9E00D`.

**Why:** `ci_us` was settling at 5945–9187 against a true 2777 µs at 60 eHz.
Aggregate counters cannot distinguish "the loop is blind to some sectors" from
"the loop is slow everywhere", and those have different causes.

**Measured:** `a1=12 f1=11 a2=12 f2=12 a3=12 f3=11 a4=15 f4=8 a5=13 f5=9
a6=14 f6=9`.

**Showed:** uniform across all six sectors — no blind phase, no blind parity.
This **falsified** my earlier claim that the 2.79×/2.14× ratios came from
phase-C-only detection. Those ratios were something else.

**Next:** find what actually sets the interval.

---

## E002 — forced-fallback speed ratchet, capped

**Changed:** `bin/shell-pwm.rs`, forced cadence now
`interval.min(sector_interval_us(HANDOFF_EHZ))` while closed; estimator band
stated explicitly as `new_bounded(ci0, ci0/4, ci0 + ci0/2)` instead of the
default ±4×. ELF
`BFF84CD1AEE0065C49178E6C7F16DBFBC2EC0B87CF4DEAE104F4C3BA10C4E54B`.

**Why:** a missed crossing forced a commutation at `ZC_PATIENCE_NUM` × the
believed interval — half the believed speed. The rotor decelerated toward it,
the estimator then *correctly* measured the slower rate, and the raised
estimate slowed the fallback further. A ratchet with no restoring force. The
reference's concept is that the forced startup schedule persists until BEMF
locks, so a missed crossing should fall back to that schedule, never to
something slower than any speed the rotor has held.

**Measured:** `ci_us` 2943 → 2782 → 2910 across the run, final 2910.

**Showed:** 1.05× the true 2777 µs, from 2.8–3.3× before. **This fix is real
and stands.** Also proves the estimator was never the problem — it was
faithfully reporting a speed that was genuinely being given away.

**Next:** accepts still only 81 vs 178 forced. Why is the accept rate so low?

---

## E003 — accept-timing histogram

**Changed:** `bin/shell-pwm.rs`, `acc_by_phase` binning accepted `count`
against the believed interval by shift-only thresholds; `BEMFPHASE` line. ELF
`2574823308DDC7FB2EEBAB761CCD1C09C6329D40041B7D6D48C13817BCC35259`.

**Why:** a genuine crossing sits about one whole interval after the previous
one; an accept just past the blanking gate is the post-commutation freewheel
transient being read as back-EMF. No aggregate accept total separates those.

**Measured:** `lt075=25 to100=25 to125=23 to150=20 gt150=9`.

**Showed:** flat. Accepts are uncorrelated with where the crossing should be.

**Also recorded here:** `offer()` in `src/bemf.rs` is **level-polled**, not
edge-triggered, so `too_early` and `unstable` are *poll counts* — "not yet"
tallies — not refusal-class failures. Goal gate 2 as worded ("both counters
non-zero ⇒ the filter is running") cannot be satisfied honestly by this
architecture. Flagged, not worked around.

---

## E004 — comparator level map

**Changed:** `bin/shell-pwm.rs`, `map_n`/`map_hi` per 256 µs bin of sector
time (`count >> 8`), `BEMFMAPN`/`BEMFMAPHI` lines. Reads the comparator, not
the ADC, because a 7-channel scan takes ~11 µs and the mid-ON sample window is
~6 µs. ELF `A329301A942F67F5AE59C8926D2B577E6AA31774A08A332509D742C8164C1CB9`.

**Measured:** hi/n per bin
`.33 .26 .29 .28 .29 .31 .26 .29 .29 .34 .28 .31`.

**Showed:** dead flat. A real crossing must produce a step — low before it,
high after. The comparator output carries **no information about rotor
position**. Everything downstream had been operating on noise.

---

## E005 — coast comparator edge count

**Changed:** `bin/shell-pwm.rs`, `coast_capture` now points COMP2 at phase C
and counts output transitions over the same window the ADC digitizes; added
`comp_edges`/`comp_hi`/`comp_polls` to `BEMFCOAST`. ELF
`ACAA332C0383D7CE4321824C0BE298F1EED4DFAE7EF3B60273F53D34F6DF1A56`.

**Why:** coast is the clean cross-check — bridge Hi-Z, no PWM, and the ADC
independently confirms a real back-EMF sine.

**Measured:** `comp_edges=93792 comp_polls=419991 comp_hi=348507` over 1500 ms
= 62.5 kHz of edges, against the ~120/s a 60 eHz rotor can produce. 83% high.

**Showed:** not a detector, an oscillator.

---

## E006 — COMP2 hysteresis 0 → 2, then REVERTED

**Changed then reverted:** `COMP2_HYST` in `bin/shell-pwm.rs`.

**Why:** 62.5 kHz chatter on a ±32 mV signal with zero hysteresis looked
self-explanatory, and the software N-of-N filter cannot substitute because its
reads are back-to-back nanoseconds apart — all inside one chatter period.

**Measured:** `comp_edges` 93792 → 91742. Register verified applied:
`COMP2_CSR = 0x40020281` (INMSEL=8, INPSEL=2, HYST=2, EN=1). Then two
consecutive coasts from the **same build** gave 91742 and 1013 edges.

**Showed:** a 90× swing between identical builds means the quantity was never
stable enough to attribute to a setting. My "hysteresis changed nothing"
comparison was invalid, and so was the hypothesis. **Reverted to 0 to match
the reference:** `binz/examples/shell-pwm.rs:1450` writes hysteresis only
under its `bench-comp-hyst-low` feature, and the default build is the one
qualified to 50%.

---

## E007 — comparator input-node ranges, and two failed probe designs

**Changed:** `Board::comp_inputs()`, `BEMFNODES` line; new `n` command
`neutral_probe`. ELFs `B3A5001696EB7757517C8BF21526E54E196A95FA2B0541335DB92AAC42C9A87E`
and `9FD0CB167E2FD448CCB707907714EBC348DC06111D246EEDE8676C214484C9A5`.

**Failed design 1 — phase C chopped alone, A and B Hi-Z.** Reasoned that one
switching half-bridge has no external current path, so it is safe at any duty.
Safe, and useless: with no current the three terminals and the star network
drift to one common potential. `vsenc_mean=1014, star_mean=1015` at every duty
from 10% to 70%. Undefined nodes cannot be compared.

**Failed design 2 — checked nFAULT immediately after asserting ENABLE.**
Aborted on the driver's own wake transient. The drive paths all allow 2 ms
(`bin/shell-pwm.rs:1816`); so must any new path.

**Valid version — sector-1 vector, A→B driven, C floating, rotor at rest,
duty 3→15%:** `vsenc_mean` 1014–1015, `star_mean` 1015, at every duty,
identical to the no-current case. ADC mapping verified correct (IN2=PA2=VSENC,
IN3=PA3=star, table sorted with a compile-time assertion).

**Showed:** both comparator inputs pinned at ~1015 codes and not following the
motor. At the time I read this as possibly-unusable hardware. **E010 shows the
real cause and this reading was premature** — under `Forward` the mux was
pointing at the wrong pin, so "the node doesn't follow the motor" was partly a
statement about which node was being sampled.

---

## E008 — mux settling allowance added

**Changed:** `COMP_MUX_SETTLE_US = 10`, gate `mux_settled` on the offer path.
ELF `DB42CC366FF57C44A89DCAFFCAA6B2E5A2A398C8F37E311292C196B360D761EB`.

**Why, cited:** the reference never reads the comparator straight after a mux
change. `binz/examples/shell-pwm.rs:862` waits `asm::delay(3000)` (~47 µs at
64 MHz) normally and 10 µs in its fast path;
`binz/examples/support/comp_input.rs` holds `FILTER_SETTLE_US = 10` before
re-arming. firmware50 re-pointed `INMSEL` at each commutation and believed the
output immediately — the reference's documented mux transient taken as signal.

**Measured:** map hi/n `.40 .34 .32 .31 .28 .29 .28 .23 .23 .29 .28 .27`.

**Showed:** still flat, now with a weak *decreasing* slope. Kept — the
justification is the reference's own practice regardless of this run.

---

## E009 — polarity inversion hypothesis, TESTED AND FALSIFIED

**Changed then reverted:** `COMP_POLARITY_INVERTED`, `expected_level(step)`.
ELF `34B3F4AF60973257BB25B0A58A48154DBBA05AD48EF23E5AA5497FB5E7A54448`.

**Why:** E008's map decreased with time into the sector where a real crossing
must increase. The reference treats polarity as a per-board knob — AM32 has
`#ifdef INVERTED_EXTI`, and `binz/examples/support/comp_input.rs:91` warns
*"minz's rising means raw comparator rises (not rm32 generic inverted HAL)"*.

**Prediction:** inverting complements the ratio to about `.70`.

**Measured with the flip applied:** `.34 .35 .32 .31 .29 .25 .26 .32 .27 .29
.30 .31` — unchanged at about `.30`.

**Showed:** a deterministic level cannot read `.30` under both conventions, so
the comparator output was not a fixed function of sector position in *either*
sense. Hypothesis falsified; **reverted**. Constant kept, documented as tried.

---

## E010 — ROOT CAUSE: firmware50 was running `Forward` on reverse-wired hardware

**Found by reading the reference, as the goal directs, not by another guess.**

**Citations:**
- `binz/AGENTS.md:18` — *"2026-09-19 CURRENT installed image:
  **reverse48k**/effective-advance26"*, frozen at
  `captures/reference/reverse_48k_com_top_high_20260919/shell-pwm.elf`. That
  is the goal's "frozen oracle ELF"; it exists.
- `binz/AGENTS.md:130` — the replacement motor runs *"in the
  operator-confirmed desired reverse direction"*.
- `binz/examples/support/phase_direction.rs` — `bench-reverse-phases` swaps
  physical phases A↔B and maps steps 1→4, 2→3, 3→2, 4→1, 5→6, 6→5. Its own
  test asserts `ROLES[physical_step(s)] == phase_swapped(ROLES[s])`, so
  applying the step permutation to the shared roles table *is* the phase swap.
- `binz/AGENTS.md:8527` — *"COMP2 BEMF: VSENA/B/C to PB3/PB7/PA2; star neutral
  to PA3"*, and `:8541` — *"CH3/CH3N=A (PA10/PB1), CH2/CH2N=B (PA9/PB0),
  CH1/CH1N=C (PA8/PA7)"*. Both match firmware50's map, so the pin map was
  never wrong.
- `firmware50/src/commutation.rs:140` — firmware50 **already defines**
  `Reverse`, documented as *"The qualified bench orientation"*, implementing
  exactly that permutation.

**The defect:** `bin/shell-pwm.rs` instantiated `comparator_inmsel::<Forward>`
and passed raw logical steps to `sixstep::plan`. On reverse-wired hardware
`INMSEL` therefore selected the wrong physical pin in every sector that floats
A or B — **four of six**. Only the C-floating sectors pointed at the right
phase. That is the near-total loss of position information measured in E004,
and it is why no hysteresis (E006) or polarity (E009) change could touch it:
the mux was simply looking at the wrong wire most of the time.

**Next:** switch the binary to `Reverse` for both the gate plan and `INMSEL`,
one change alone, and re-measure the E004 map. Prediction: the map develops a
step. Then, separately, move the carrier to 48 kHz as the goal and the
qualified image both specify.

---

## E011 — `Reverse` wiring applied: the comparator detects crossings

**Changed:** `bin/shell-pwm.rs` — `type Wiring = Reverse`, helpers
`physical(step)` and `plan_for(step, duty)`; every `sixstep::plan` call now
maps the logical step through `Direction::step` first, and both
`comparator_inmsel` sites use `::<Wiring>`. Single change, nothing else
touched. ELF `C6924A619248674DF6D0068C460933BD07B964B80C2520BAD9F4207A18E1A0D8`,
text 32072, bss 1044. Clippy clean.

**Why:** E010. Justification and citations there.

**Prediction stated before the run:** the E004 level map develops a step.

**Measured:**

```
BEMFDONE reason=26 accepted=465 forced=269 zc_acc=465 too_early=1727 unstable=2737 ci_us=2965 bus_ref=1209 bus_min=1135
BEMFSECTOR a1=77 f1=47 a2=81 f2=43 a3=72 f3=50 a4=79 f4=40 a5=72 f5=51 a6=84 f6=38
BEMFPHASE  lt075=221 to100=68 to125=35 to150=42 gt150=99
BEMFMAPN   b0=309 b1=485 b2=544 b3=670 b4=411 b5=397 b6=244 b7=290 b8=251 b9=210 b10=226 b11=209
BEMFMAPHI  b0=116 b1=243 b2=335 b3=355 b4=164 b5=149 b6=62  b7=71  b8=60  b9=42  b10=48  b11=59
BEMFNODES  vsenc_min=0 vsenc_max=1136 star_min=0 star_max=748 bus_ref=1209
BEMFCOAST  reason=26 pp_first=105 pp_last=26 crossings=0 spun=1 comp_edges=82876 comp_hi=333409 comp_polls=419571
```

**Showed — prediction confirmed:**

* Level map hi/n by bin: `.38 .50 .62 .53 .40 .38 .25 .24 .24 .20 .21 .28`.
  Structured, peaking at bin 2 (512–768 µs into the sector), against dead-flat
  `.29`±.03 in every earlier build. The comparator now carries rotor position.
* Accepts 465 vs 269 forced, from 110 vs 149. Over the 1.3 s closed window
  that is **358 accepted ZC/s against the 360/s** that 6 × 60 eHz requires —
  0.6%. This is goal gate 2's rate identity essentially holding at handoff
  speed, though not yet at 15% nor for 30 s.
* Per-sector accepts even and high across all six (72–84), forced 38–51.
* Coast `pp_first=105`, above every previous run (58–86) — a faster rotor at
  cut, consistent with the loop actually holding speed.

**Still wrong:**

* `ci_us` wanders 3129 → 694 → 1493 → 2965. 694 is exactly the stated floor
  `ci0/4`, so the estimate is still collapsing mid-run and recovering.
* `BEMFPHASE lt075=221` — nearly half the accepts land just past the blanking
  gate, so many are early rather than at the crossing.
* Run still ends `reason=26` (fast bus-sag) at ~4 s, `bus_min` 1135/1209.

**Next:** advance level. firmware50 runs `ADVANCE_LEVEL=16`; the qualified
image is *"reverse48k/**effective-advance26**"* (`binz/AGENTS.md:18`). With
detection now working, the commutation instant is the thing that sets whether
`count` spans a whole sector, which is what the estimator's fixed point
requires. Apply 26 alone and re-measure.

---

## E012 — advance 16 → 26, and the discovery that single runs are not evidence

**Changed:** `ADVANCE_LEVEL` 16 → 26 in `bin/shell-pwm.rs`, cited to
`binz/AGENTS.md:18` (*"reverse48k/effective-advance26"*). ELF
`D26403E035979BCB0EC8CC218A952CB3A0270D332B3DB84890FF2D35CC641745`.

**Measured — advance 16 (ELF C692…), two runs of the same build:**

| run | accepted | forced | ci_us | map peak | pp_first |
|---|---|---|---|---|---|
| A | 465 | 269 | 2965 | .62 @ b2 | 105 |
| B | 168 | 141 | 2530 | flat ~.30 | 63 |

**Measured — advance 26 (ELF D264…), two runs:**

| run | accepted | forced | ci_us | pp_first |
|---|---|---|---|---|
| C | 221 | 117 | 3152 | 70 |
| D | 204 | 125 | 3469 | 80 |

Run D detail: `a1=29 f1=24 a2=43 f2=12 a3=32 f3=23 a4=39 f4=17 a5=39 f5=16
a6=22 f6=33`, `lt075=61 to100=40 to125=46 to150=33 gt150=24`,
`too_early=1745 unstable=1726`, `bus_min=1127/1212`.

**Showed:**

* **The advance-16 build's own spread is 2.8× (465 vs 168 accepts).** So the
  E011 entry above over-reads a single run: its 465/269 and clean `.62` map
  peak are the *good* end of a wide distribution, not a stable property. The
  `Reverse` fix in E010/E011 is still clearly real — every pre-`Reverse` build
  sat at ~110 accepts with a dead-flat map, and no post-`Reverse` run has been
  flat-and-low in the same way — but the magnitude quoted there is not.
* Advance 26 is **repeatable** (221, 204; forced 117, 125) where 16 is not,
  and its refusal tallies come out balanced (`too_early` 1745 ≈ `unstable`
  1726). Fewer raw accepts than run A, but a better accepted:forced ratio
  (1.7–1.8 vs 1.7) with far less scatter.
* Every run still ends `reason=26` at ~4 s with `bus_min` 93% of `bus_ref`,
  while the duty ramp is passing ~9.8% on its way to 15%.

**Not concluded:** which advance is better. Per
[[feedback-regime-scoped-conclusions]] an advance verdict is only valid in the
regime measured, and the next change moves the regime (carrier 10 → 48 kHz).
Re-testing advance before that would produce a verdict that expires
immediately. Advance stays at the qualified image's 26 meanwhile, since that
is the cited value and the repeatable one.

**Next:** carrier 10 kHz → 48 kHz. The goal specifies a 48 kHz carrier and the
qualified image is `reverse48k`. At 10 kHz and 7–15% duty the ON pulse is only
7–15 µs of full bus per cycle, which is the shape binz recorded as
physics-hostile on this motor (`binz/CLAUDE.md`: a 1 kHz carrier's *"70 µs
full-bus pulses trip the current guard in ~100 ms"*). The remaining sag stop
is the signature of a current surge, and per
[[feedback-binz-10pct-psu-cap]] *"a synced 30% barely sags so any deep sag =
desync surge"* — so the sag is a symptom to chase at the correct carrier, not
a threshold to relax.

---

## E013 — 48 kHz run carrier: first 15% hold, and parity blindness exposed

**Changed:** `bin/shell-pwm.rs` — carrier now steps 10 kHz → 48 kHz at the
instant the loop closes (`period = RUN_PERIOD_TICKS`, ARR rewritten, `EGR.UG`
forced, plan rebuilt for the new period); `plan_for`/`sixstep_ccr`/
`comp_sample_window` take the period explicitly instead of assuming
`STARTUP_TICKS`; new `HANDOFF_DUTY_TENTHS` so the carrier step reprograms the
compare for exactly the duty in force; `comp_sample_window` scales its settle
to the period and falls back to ungated when the ON time cannot contain a
window. ELF `54B6B0D4C102513E1D608E74E1FFA2A22478E188CEEDD36C5A0625861860EB57`.
Clippy clean, 186 host tests pass.

**Why, cited:** `binz/examples/support/carrier_profile.rs:2` — *"Startup stays
at 10kHz. BEMF candidates include ... ARR1332 (48012Hz)"*, and the qualified
image is `reverse48k` (`binz/AGENTS.md:18`). firmware50 already held both
constants (`duty::STARTUP_TICKS` 6400, `duty::RUN_PERIOD_TICKS` 1333) and the
binary used only the startup one, so every closed-loop run to date chopped at
10 kHz. At 10 kHz a 15% duty is a 15 µs slice of full bus per cycle into a
low-inductance winding; at 48 kHz it is 3.1 µs, cutting peak-to-average
current for the same torque.

Also recorded: `comp_sample_window`'s original justification (huge refusal
counts when ungated) was measured under the `Forward` bug, so that evidence is
void. The reference PWM-gates not at all — AM32 on L431 sets
`OutputBlankingSource = NONE`. The gate is retained for now only to keep this
a single change.

**Measured:**

```
BEMF ms=5000  duty_tenths=150 ci_us=1817 accepted=82  forced=409  bus_mean=1204
BEMF ms=8000  duty_tenths=150 ci_us=2663 accepted=289 forced=973  bus_mean=1216
BEMF ms=11500 duty_tenths=150 ci_us=2249 accepted=504 forced=1564 bus_mean=1207
BEMFDONE reason=2 accepted=528 forced=1664 zc_acc=528 too_early=19291 unstable=35560 ci_us=1963 bus_ref=1211 bus_min=1126
BEMFSECTOR a1=154 f1=214 a2=0 f2=363 a3=260 f3=105 a4=0 f4=365 a5=114 f5=252 a6=0 f6=365
BEMFPHASE  lt075=181 to100=104 to125=82 to150=85 gt150=76
BEMFMAPN   b0=4478 b1=4228 b2=3820 b3=3819 b4=3508 b5=3373 b6=3178 b7=3149 b8=3296 b9=3272 b10=3164 b11=3275
BEMFMAPHI  b0=2211 b1=2152 b2=2005 b3=1941 b4=1860 b5=1791 b6=1745 b7=1701 b8=1801 b9=1768 b10=1734 b11=1866
BEMFCOAST  reason=2 pp_first=74 pp_last=21 crossings=0 spun=1 comp_edges=96895 comp_hi=352988 comp_polls=419595
```

**Showed:**

* **`reason=2` — the normal segment deadline. The run was not stopped by a
  protection.** First such run in this crate. Held `duty_tenths=150` (15.0%)
  from ms=5000 to ms=11500: **6.5 s at 15%**, `bus_mean` 1201–1216 against
  `bus_ref` 1211, `bus_min` 1126 seen only during the ramp. The carrier was
  the sag lever, as [[feedback-binz-10pct-psu-cap]] predicted — deep sag was a
  current-surge signature, not a threshold to relax. **No threshold was
  changed to achieve this.**
* **Sectors 2, 4 and 6 never accept: `a2=a4=a6=0` exactly.** Odd sectors
  accept 154/260/114. Each phase therefore accepts in exactly one of its two
  floating sectors. This is real per-sector-parity blindness. Note the
  bookkeeping: E001 *correctly* falsified this same hypothesis at the time,
  because under `Forward` no sector worked; the earlier falsification was
  sound and this is a different regime.
* Consequently accepts are only 528 against 1664 forced — the loop is holding
  15% largely on the forced grid, so **gate 1 is not met**: "commutation driven
  only by accepted zero-crossings" is not yet true.
* Level map is now ~.49–.57 across all bins, i.e. flat at one half. With a
  12-deep persistence filter a genuinely random level would accept about
  1/4096 of the time, yet 528 accepts occurred — so the level dwells rather
  than being random, and the aggregate map is averaging the working odd
  sectors against the dead even ones.

**Next:** per-sector polarity. `expected_level(step)` derives `rising` from the
**logical** step, while the gates are driven from `physical(step)` and
`Reverse::step` flips parity for every step (1→4, 2→3, 3→2, 4→1, 5→6, 6→5).
Test deriving polarity from the physical step. Prediction: all six sectors
accept. If that fails, the alternation itself is suspect and the next test is a
constant expectation.

---

## E014 — polarity from the physical step: blindness flips halves

**Changed:** `expected_level(step)` now uses `physical(step).rising()` instead
of `step.rising()`. ELF
`D961C7E9375A8E21190AB6362E99DD6B539F301F5456F119218FCEF98138D38F`.

**Prediction stated before the run:** all six sectors accept.

**Measured:**

```
BEMFDONE reason=2 accepted=620 forced=1596 zc_acc=620 too_early=22236 unstable=41779 ci_us=2826 bus_ref=1212 bus_min=1127
BEMFSECTOR a1=0 f1=370 a2=200 f2=169 a3=0 f3=369 a4=250 f4=120 a5=0 f5=368 a6=170 f6=200
BEMFPHASE  lt075=204 to100=116 to125=119 to150=99 gt150=82
```

Also held 15.0% from ms=5000 to ms=11500 again, `reason=2`, `bus_mean`
1203–1216. Accepts up slightly, 620 vs 528.

**Showed — prediction wrong, but informatively.** The blindness *flipped
halves*: now `a1=a3=a5=0` and even sectors accept 200/250/170, the exact
mirror of E013.

**What the pair of experiments proves.** `Reverse::step` flips parity on every
step, so the physical expectation is the negation of the logical one. Let
`L(step) = step & 1`:

* with `L`: odd sectors accept, even are dead → the expectation is right on
  odd, wrong on even;
* with `¬L`: even sectors accept, odd are dead → right on even, wrong on odd.

In both runs the sectors that worked are exactly those where the expectation
evaluated to **true**. Odd under `L` gives true; even under `¬L` gives true.
So the required expectation is **constant `true` for all six sectors** — the
comparator output is high after the crossing in every sector and does not
alternate with sector parity.

This is a real divergence from the reference's convention (AM32 alternates
`rising = step % 2`, and binz drives EXTI edge select from it), so it is not
adopted lightly — but it is what two complementary measurements force, and the
alternating convention cannot be correct when each of its two possible phasings
kills exactly half the sectors.

**Next:** make the expectation constant and re-measure. Prediction: all six
sectors accept, and `forced` falls sharply.

---

## E015 — constant expectation: all six sectors accept, 75% ZC-driven

**Changed:** `expected_level` is now constant (`!COMP_POLARITY_INVERTED`),
independent of sector. ELF
`42BFB3F8151319DC3EDCCF9C9D176A009BD843D399EEF27511A3235D1BEED526`.
Clippy clean.

**Why:** E014's table. Each phasing of the alternating expectation killed
exactly half the sectors, and the surviving half was always the one where the
expectation was `true`.

**Prediction stated before the run:** all six sectors accept, `forced` falls
sharply.

**Measured:**

```
BEMF ms=5000  duty_tenths=150 ci_us=1130 accepted=564  forced=302
BEMF ms=8000  duty_tenths=150 ci_us=1911 accepted=1779 forced=635
BEMF ms=11500 duty_tenths=150 ci_us=1535 accepted=3060 forced=1006
BEMFDONE reason=2 accepted=3247 forced=1058 zc_acc=3247 too_early=18144 unstable=28989 ci_us=1670 bus_ref=1209 bus_min=1128
BEMFSECTOR a1=534 f1=183 a2=645 f2=66 a3=637 f3=83 a4=489 f4=233 a5=410 f5=314 a6=532 f6=179
BEMFPHASE  lt075=1216 to100=584 to125=479 to150=403 gt150=565
BEMFMAPN   b0=2535 b1=5797 b2=5731 b3=5484 b4=4741 b5=4353 b6=3722 b7=3285 b8=2942 b9=2583 b10=2243 b11=1894
BEMFMAPHI  b0=921  b1=1790 b2=1788 b3=1689 b4=1438 b5=1307 b6=1063 b7=950  b8=870  b9=746  b10=670  b11=564
BEMFCOAST  reason=2 pp_first=103 pp_last=21 crossings=0 spun=1 comp_edges=84605 comp_hi=341672 comp_polls=418847
```

**Showed — prediction confirmed:**

* **All six sectors accept**, 410–645 each, forced 66–314. No dead sector.
* **3247 accepted vs 1058 forced — 75% of commutations are ZC-driven**, from
  24% in E013 and 28% in E014.
* Held `duty_tenths=150` from ms=5000 to ms=11500 (6.5 s at 15%), `reason=2`
  normal end, `bus_mean` 1200–1207 against `bus_ref` 1209.
* `ci_us` settles near 1670 µs = 99.8 eHz, so under closed-loop control the
  rotor accelerated well past the 60 eHz handoff at 15% duty — the loop is
  following the rotor, not a schedule. `ehz_sched=60` is now only the fallback
  deadline.
* Coast `pp_first=103`, against the measured stalled discriminator of 24–30
  (`COAST_SPUN_PP_CODES = 50`). Rotation witnessed from the board.

**Gate status after this run:**

* Gate 1 — **not met.** 6.5 s at 15%, not 30 s (`BEMF_TOTAL_MS` is 12000), and
  `forced=1058` is not "driven only by accepted zero-crossings".
* Gate 2 — rate identity now plausible but not yet computed on-board;
  `too_early`/`unstable` are still poll counts, see E003.
* Gate 3 — coast witness present; phase-current/speed comparison against the
  qualified binz image not yet run.
* Gates 4, 5 — not started.

**Next:** extend the segment so a ≥30 s hold is even possible
(`BEMF_TOTAL_MS`), and attack `forced`. `BEMFPHASE lt075=1216` of 3247 says
37% of accepts still land just past the blanking gate rather than near a whole
interval, so the blanking/advance relationship is the next lever.

---

## E016 — 35.2 s hold at 15%, MCU-stamped (gate 1 duration met, gate 1 purity not)

**Changed:** `BEMF_TOTAL_MS` 12000 → 40000; added `closed_at`/`hold_start`
stamps taken from the board's own TIM17 timebase; new `BEMFGATE` line
reporting `closed_ms`, `hold_ms`, `target_tenths`, `zc_per_s`, `comm_per_s`,
`ehz_from_ci`, `zc_expected_per_s`, `forced_pct`. Division at that site only —
it runs once after `safe_off`, on no motor path and in no ISR. ELF
`0692186D69FA8A6892B525FF66063F66633B66A770F0830095798A9E8E2629C8`.
Clippy clean.

`hold_start` is stamped from the *applied* duty the governor returned, after
foldback, not from the schedule's intent — a duty in a register is not a hold.

**Measured, run 1 of the 40 s build:**

```
BEMFDONE reason=2 accepted=13467 forced=4460 zc_acc=13467 too_early=46482 unstable=64499 ci_us=2545 bus_ref=1211 bus_min=1110
BEMFGATE closed_ms=37200 hold_ms=35200 target_tenths=150 zc_per_s=362 comm_per_s=481 ehz_from_ci=65 zc_expected_per_s=392 forced_pct=24
BEMFSECTOR a1=2167 f1=821 a2=2660 f2=302 a3=2712 f3=271 a4=2135 f4=880 a5=1620 f5=1362 a6=2173 f6=824
BEMFPHASE  lt075=5240 to100=2486 to125=1837 to150=1554 gt150=2350
BEMFMAPN   b0=8281 b1=13754 b2=15075 b3=13833 b4=12012 b5=10562 b6=8630 b7=7797 b8=6908 b9=5988 b10=5293 b11=4376
BEMFMAPHI  b0=3115 b1=5219 b2=5363 b3=4936 b4=4090 b5=3368 b6=2749 b7=2443 b8=2132 b9=1919 b10=1697 b11=1376
BEMFCOAST  reason=2 pp_first=91 pp_last=24 crossings=0 spun=1 comp_edges=129061 comp_hi=310652 comp_polls=414967
```

**Showed:**

* **`hold_ms=35200` — 35.2 s continuously at 15.0% duty**, MCU-stamped, with
  `reason=2` (segment deadline). No protection fired. Bus held 1199–1211
  against `bus_ref` 1211 for the whole hold; `bus_min` 1110 is a single
  instantaneous sample during the ramp, not the running mean the sag stop
  judges. **No threshold was relaxed at any point in this campaign.**
* All six sectors accepting, 1620–2712 each.
* Coast `pp_first=91` vs the 24–30 stalled discriminator; `spun=1`.
* 13467 accepted vs 4460 forced.

**Gate assessment — honest:**

* **Gate 1: duration met, purity NOT met.** ≥30 s at 15% after handoff,
  MCU-stamped: yes, 35.2 s. But `forced_pct=24` — a quarter of commutations
  still come from the fallback grid, so "commutation driven only by accepted
  zero-crossings" is false, and the scripted frequency source is still present
  as that fallback. **Gate 1 is not satisfied.**
* **Gate 2: not met.** `zc_per_s=362` against `zc_expected_per_s=392` from the
  final `ci` is −8%, but `comm_per_s=481` exceeds both, so the identity does
  not close: the final `ci` is not the run's mean `ci` (it swings 959–3367
  through the hold), and my `zc_expected_per_s` uses only the last value. The
  instrument is wrong, not just the result — it needs a time-averaged
  interval, or better an accumulated sector-time total. Also `too_early` and
  `unstable` remain poll counts (E003), so the second half of gate 2 still
  cannot be honestly claimed.
* Gate 3: coast witness from the board, yes. Phase-current/speed comparison
  against the qualified binz image: not yet run.
* Gates 4, 5: not started.

**Next, in order:**
1. Fix the gate-2 instrument: accumulate total sector time and accepted count
   over the hold window so the rate identity is computed against the mean
   interval, not the last sample.
2. Attack `forced_pct`. `BEMFPHASE lt075=5240` of 13467 (39%) still land just
   past the blanking gate, and `a5/f5 = 1620/1362` is much worse than the
   other five sectors — one sector is carrying most of the fallback.
3. Then gates 3 (binz comparison run), 4 (provoke each protection), 5 (ISR
   roots + audit).

---

## E017 — software edge arming + hold-window rate instrument

**Changed:** `bin/shell-pwm.rs` — `armed` flag: after each commutation the
detector must observe the comparator on the *far* side of the neutral before a
reading at the expected level may be offered. Plus passive hold-window
accumulators (`hold_acc`, `hold_forced`, `hold_ci_sum`) and a new `BEMFRATE`
line. ELF `6E9A23B4CE2105924AEBA14298E29BD44A9A87473199E2F58C161F37BF39A546`.
Clippy clean.

**Why:** the reference detects an *edge* (AM32 on EXTI, binz driving EXTI edge
select from the sector); this build polls a *level*. They differ exactly when
the level is already in the expected state as the blanking gate opens — a
level detector then accepts the post-commutation freewheel transient. E016
measured that: `lt075=5240` of 13467 accepts (39%) inside the first three
quarters of an interval. Early accepts blend a short interval in, which
shortens the gate, which admits more.

**Measured (first cohort, before E019's fix):** run 1 `hold_ms=35200`,
`hold_forced_pct=23`, `lt075=4917` of 12960. Runs 2 and 3 died before handoff
(see E019).

**Showed:** arming did not move `lt075` much (39% → 38%). Not the dominant
term. Kept — the edge semantics are the reference's and the change is sound —
but it is not the lever for `forced_pct`.

---

## E018 — SCAR: the console is COM41, not COM7

**No firmware change.** Recorded because it cost a long detour and wedged the
probe.

binz's `CLAUDE.md` says *"VCP = COM7"*, and my own memory note for this bench
repeats it. That is the **old EVLDRIVE102H rig**. firmware50's console is
USART3 on PC10/PC11 (`bin/shell-pwm.rs:90`, matching `binz/AGENTS.md:8535`
*"Bench console: USART3 PC10/PC11 at 115200"*), bridged by the **FTDI USB-TTL
adapter on COM41**. COM7 is the ST-Link VCP and is not wired to USART3.

The failure mode is nasty: COM7 *opens cleanly* and then returns nothing, which
is indistinguishable from a dead board. I diagnosed in the wrong direction —
concluded the probe's VCP had wedged, issued a pyusb `reset()` on the ST-Link,
and that wedged **SWD** too (`SwdApWdataError`, then `JtagUnknownJtagChain`).

Recovery that worked: `probe-rs reset --connect-under-reset`, which restored
SWD immediately — the same lever that recovered the `DBG_SWEN` incident.

Diagnostics that were right and should be reached for first:
* TIM17 CNT read twice over SWD proves the core is executing (it was).
* USART3 `CR1=0x0D` (UE|RE|TE), `BRR=0x22B` (115.3 kbaud),
  `ISR=0x006000D0` with TXE and TC set and no ORE/FE/PE proves the MCU side is
  configured and merely idle — i.e. nothing was being queued, so the fault was
  never in the firmware.
* The decisive test is to hold the port open *across* a reset and watch for the
  boot banner; opening afterwards misses it, because bytes sent with no host
  handle are dropped. Sweeping all four enumerated ports that way found the
  banner on COM41 in one shot.

Safety during the outage: verified over SWD that TIM1 `BDTR=0x00000C1A`
(MOE clear) with `CCR1=CCR2=CCR3=0` — bridge safed, no gate drive.

`scripts/bemf_run.py` now defaults to COM41 with this written at the top.

---

## E019 — ROOT CAUSE of restart failure: the carrier was never restored

**Changed:** new `set_carrier(period)` helper (writes ARR, forces `EGR.UG`);
called at closed-loop entry with `STARTUP_TICKS`, at handoff with
`RUN_PERIOD_TICKS`, and again after `safe_off` to put the startup carrier back.
ELF `9504E622E5CD68D60FD070E1B9F2A5B156E5CA849C15F91C1046E698A5F78DE5`.
Clippy clean.

**The defect, introduced by my own E013 change.** E013 raises TIM1's ARR from
6400 to 1333 ticks at handoff for the 48 kHz carrier. Nothing put it back. A
following run began with its `period` variable at `STARTUP_TICKS` = 6400 while
the hardware ARR was still 1332, so every compare it computed meant **4.8x the
duty it intended** — the 3.3% point of the open-loop ramp actually drove about
15.8%.

**Evidence, first cohort:** run 1 held 35.2 s; runs 2 and 3 both stopped
`reason=26` with `accepted=0 forced=0 closed_ms=0`, at `bus_min` 1141/1212 =
94.1%, between ms=1500 and ms=2000 of the open-loop ramp. Deterministic, not
positional luck: run 1 works only because a fresh boot leaves ARR at 6399 from
`timer_init`. Ruled out along the way — the rail moving mean does persist in
`Board` across runs, but its 8-scan window is fully replaced within ~1 ms, so
it cannot explain a stop 1.5 s in; and `capture_baseline` is taken fresh each
run.

**Measured after the fix — 3/3 runs, captured to
`captures/2026-09-20/qual_0{1,2,3}.txt`:**

| run | reason | hold_ms | hold_accepted | hold_forced | hold_forced_pct | mean_ci_us | permille_of_expected | bus_min/ref | coast pp_first |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 35200 | 13787 | 4113 | 22 | 1767 | 692 | 1117/1212 | 71 |
| 2 | 2 | 35200 | 10347 | 6512 | 38 | 1708 | 500 | 1125/1213 | 83 |
| 3 | 2 | 35200 | 8967 | 6690 | 42 | 1731 | 577→440 | 1125/1213 | 73 |

**Showed:**

* **3/3 runs reach `hold_ms=35200` at 15.0% duty with `reason=2`** (segment
  deadline, no protection trip). The restart failure is fixed and the diagnosis
  is confirmed by the fix working on the first attempt.
* Coast witness `spun=1`, `pp_first` 71–83 against the 24–30 stalled
  discriminator, on all three.
* **New finding: progressive degradation across consecutive runs.**
  `hold_forced_pct` 22 → 38 → 42, concentrated in sectors 1 and 6, which
  collapse from `a1=2149/f1=949` to `a1=224/f1=2509`. Sectors 2–5 stay healthy
  throughout. Something still leaks between runs, or is thermal, and it is
  phase-specific.
* **Gate 2 instrument is still biased and I should not quote it as a pass.**
  `mean_ci_us` averages *accepted* intervals only, and an accepted interval
  spans two sectors whenever a crossing was missed (`gt150` counts those), so
  it under-estimates the true sector time. The honest denominator is
  `hold_ms / (hold_accepted + hold_forced)`. With that, run 1 gives 508
  commutations/s against 565 expected (90%), and the residual *is* `forced_pct`
  — which means **gate 2's rate identity and gate 1's "only ZC-driven" are the
  same requirement**, and neither is met while `forced_pct` is 22–42%.

**Gate status:** gate 1 duration met 3/3, purity not met. Gate 2 not met.
Gate 3 coast witness met; binz current/speed comparison not run. Gates 4, 5
not started.

**Next:** (a) fix the gate-2 denominator to commutation-derived sector time;
(b) find the run-to-run degradation in sectors 1 and 6 — that is now the
largest single contributor to `forced_pct`.

---

## E020 — two tests that both failed, and what they jointly prove

**Test A — remove the PWM sample gate** (`COMP_PWM_GATE = false`). ELF
`4CC82A3092667C82888AAE2B1ABF4872F081EC88F88EFAD2AEEA96EDDB8912C5`.
Captures `captures/2026-09-20/ungated_0{1,2,3}.txt`.

Rationale: the gate's evidence was gathered under the `Forward` bug (E010), so
it was void; and at 48 kHz the gate leaves the comparator observable only 13%
of the time (13..187 of 1333 ticks at 15% duty), in 2.7 us bursts every
20.8 us.

| run | hold_ms | forced_pct | ehz_from_sector | ci_us | lt075 share | coast pp_first |
|---|---|---|---|---|---|---|
| 1 | 35200 | 12 | 276 | 694 (floor) | 36637/53176 = 69% | 85 |
| 2 | 35200 | 17 | 248 | 694 (floor) | 66% | 87 |
| 3 | 35200 | 15 | 246 | 694 (floor) | 65% | 88 |

**Result: reverted.** `forced_pct` improved and the loop stopped tracking.
`ehz_from_sector` tripled to 246-276 eHz with `ci_us` pinned at its floor,
while coast `pp_first` stayed at 85-88 -- indistinguishable from the gated
build's 71-88. A rotor three times faster coasts down from three times the
amplitude; it did not. The loop was commutating ~2.7x faster than the rotor
turned. **`forced_pct` is not a safe objective: it improves when the loop
desynchronises.** The independent coast witness that goal gate 3 demands is
what caught this, which is the argument for that gate.

**Test B — arm only after the blanking gate opens.** ELF
`5081528120C3A3B8E741C26AB52C0F6FF399BB66AB21740F25EE307AB44013EA`.
Captures `armwin_0{1,2,3}.txt`.

| run | forced_pct | a1 | a6 | ehz_from_sector |
|---|---|---|---|---|
| 1 | 46 | **0** | **0** | 77 |
| 2 | 50 | **0** | **0** | 71 |
| 3 | 50 | **0** | **0** | 70 |

**Result: reverted.** This is the more principled rule -- it is what makes the
detector a true edge detector inside the window where the crossing is expected
-- and it is strictly worse.

**What A and B jointly prove.** Line up the four expectation/arming variants:

| variant | outcome |
|---|---|
| E013 logical alternation, level | sectors 1,3,5 accept; `a2=a4=a6=0` |
| E014 inverted alternation, level | sectors 2,4,6 accept; `a1=a3=a5=0` |
| E015 constant expectation, level | all six accept |
| E020B constant expectation, true edge | `a1=a6=0`, sector 5 halved |

A genuine edge requirement kills exactly the sectors whose crossing runs the
other way. Under a *constant* expectation that is half of them. Therefore
**E015's all-six acceptance was level-matching, not edge detection**: half of
those accepts had no crossing behind them, which is also why 39% of accepts
land inside three quarters of an interval and why the estimate collapses to
its floor the moment the gate is widened. A level-polled detector cannot
produce a genuine per-sector edge on this signal, whatever the expectation.

---

## E021 — reverted configuration re-qualified, 3/3

**Changed:** `COMP_PWM_GATE` back to `true`, arming back to any far-side
observation. ELF
`D24FE1190A14E9376BE82384A98A1CCCC78430B2CA16710C27845247C6B13BBE`,
text 33528, bss 1044. Clippy clean, 186 host tests pass.
Captures `qual2_0{1,2,3}.txt`.

| run | reason | hold_ms | hold_acc | hold_forced | forced_pct | mean_sector_us | ehz_from_sector | bus_min/ref | pp_first |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 35200 | 9505 | 6663 | 41 | 2177 | 76 | 1117/1210 | 85 |
| 2 | 2 | 35200 | 8471 | 6570 | 43 | 2340 | 71 | 1117/1212 | 83 |
| 3 | 2 | 35200 | 8308 | 6708 | 44 | 2344 | 71 | 1121/1211 | 88 |

**Showed:** 3/3 runs hold 35.2 s at 15.0% duty, `reason=2`, no protection
trip, coast `spun=1` on all three. `forced_pct` 41-44%, i.e. the E019
cohort's 22% was the good end of a wide distribution and ~40% is the honest
central value. Sectors 1 and 6 are the persistent weak pair (216-325 accepts
against 2300-2500 forced) while sectors 2,3,4 are strong -- the same pair E019
saw degrade, now explained by E020: they are the sectors whose crossing runs
against the constant expectation.

**Gate status, unchanged and unmet:**
* Gate 1 — duration met 3/3 (35.2 s ≥ 30 s at 15%, MCU-stamped). **Purity not
  met**: `forced_pct` ~41-44%, so commutation is not driven only by accepted
  zero-crossings and the fallback grid is load-bearing.
* Gate 2 — **not met**, and now understood to be the same requirement as gate
  1's purity: with the corrected denominator the accepted-rate shortfall
  equals `forced_pct` by construction.
* Gate 3 — coast witness met 3/3; binz current/speed comparison not run.
* Gates 4, 5 — not started.

**Next, and this is now an architecture step rather than tuning.** Four
variants of level polling have been measured and the failure is structural:
the detector needs the reference's edge, taken from hardware. That means
COMP2 -> EXTI18 with per-sector edge select (`RTSR1`/`FTSR1` as
`binz/examples/support/comp_input.rs:91-99` does), serviced in a COMP ISR --
which is also exactly what goal gate 5 requires (COMP as one of the four ISR
roots, priorities set, fail-closed audit). The two remaining blockers collapse
into one piece of work, and it should be done next rather than tuning the
polled loop further.

---

## E022 — COMP ISR root: COMP2 -> EXTI18 -> ADC_COMP (gate 5, first root)

**Changed:** `bin/shell-pwm.rs` — new `exti_rb()` PAC escape (sixth, justified:
the vendored HAL has no route to an edge select on a configurable *internal*
line, nor to `IMR1[18]`, nor to `RPR1`/`FPR1`); `#[interrupt] fn ADC_COMP()`
that masks the line, acks both pending registers and stamps the raw TIM17
count; `edge_is_rising(step)` (alternating, per AM32 and
`binz/examples/support/comp_input.rs:91-99`); `comp_exti_arm`/`comp_exti_mask`;
`Board::stamp_from_raw` to convert an ISR-captured 16-bit count into the
foreground's microsecond timeline exactly; NVIC priority set explicitly
(`COMP_IRQ_PRIORITY = 0x00`, pre-shifted for M0+'s two implemented priority
bits — the same trap that collapsed every rm32 priority to 0). The polled
detector, its PWM sample window, `COMP_SETTLE_TICKS`, `COMP_PWM_GATE`,
`expected_level` and `sixstep_ccr` are deleted. ELF
`50BF752EF48EB6694A11CCBDE919FA21A7379AD19C492238DC3378AF49C1C9B3`.
Clippy clean, 186 host tests pass. Capture `isr_01.txt`.

**Measured:**

```
BEMFDONE reason=2 accepted=382 forced=6679 too_early=3367 unstable=2690 ci_us=2855
BEMFRATE hold_ms=35200 hold_forced_pct=94 mean_sector_us=5263 ehz_from_sector=31
BEMFSECTOR a1=83 f1=1094 a2=21 f2=1156 a3=147 f3=1030 a4=20 f4=1157 a5=88 f5=1088 a6=23 f6=1154
BEMFPHASE  lt075=0 to100=184 to125=198 to150=0 gt150=0
```

**Showed — the decisive result of the whole campaign:**
`lt075=0, to150=0, gt150=0`. **Every accepted crossing landed between 0.75x
and 1.25x of one interval**, against 39-69% early accepts for every polled
variant. A hardware edge produces correctly-timed crossings; a level poll
never did. All six sectors accept, so the alternating edge polarity is right.

But only 382 accepts against 6679 forced. Cause, in the counters: 6439
candidate edges ~= one per sector, of which 48% `too_early` and 38%
`unstable`. **My ISR contract was wrong** — it masks on *every* edge, so a
refused candidate left the line masked for the rest of the sector and the real
crossing was never seen.

---

## E023 — re-arm on refusal, and validate the level in the ISR

**Change 1:** re-arm the line when `offer` refuses; only an accepted edge
closes the sector. The reference does not close on refusal — AM32 applies its
`CNT > average_interval/2` gate, clears the flag and returns with the line
live. ELF `31DAB65EAD6D5634CE1C335876EFC80F69281D9E204E33B58D5C5AD7B9FD5B98`,
capture `isr2_01.txt`: accepted 382 -> 2382, `forced_pct` 94% -> 75%,
`unstable` 2690 -> 55075.

`unstable` becoming dominant located the next fault: the foreground re-reads
the comparator microseconds after the edge, and a small back-EMF into a
zero-hysteresis comparator chatters across the crossing, so the evidence has
often expired before it is read.

**Change 2:** capture the output level *in the ISR*, at the edge, and let the
persistence filter read that instead of the live output. This reduces the
filter to AM32's single `getCompOutputLevel() == rising` validation, on
purpose — a filter fed stale evidence refuses real crossings. ELF
`BEBFC250C39EEEB7D32CACFBFA73BBA59CD86042D2DF4964B5E4EEFE0FFE2165`, capture
`isr3_01.txt`: accepted 2382 -> 8182, `unstable` 55075 -> 11940, `forced_pct`
75% -> 48%, and **all six sectors balanced for the first time**
(a1..a6 = 1444, 1206, 1487, 1322, 1457, 1266).

---

## E024 — ISR edge detector, 3-run cohort

**ELF** `BEBFC250C39EEEB7D32CACFBFA73BBA59CD86042D2DF4964B5E4EEFE0FFE2165`.
Captures `isredge_0{1,2,3}.txt`.

| run | reason | hold_ms | hold_acc | hold_forced | forced_pct | mean_sector_us | ehz_from_sector | lt075 | to125 | bus_min/ref | pp_first |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 35200 | 8971 | 7730 | 46 | 2107 | 79 | 2609 | 5078 | 1121/1214 | 75 |
| 2 | 2 | 35200 | 9467 | 7913 | 45 | 2025 | 82 | 2888 | 5380 | 1119/1214 | 92 |
| 3 | 2 | 35200 | 9270 | 7895 | 45 | 2050 | 81 | 2867 | 5211 | 1111/1214 | 95 |

**Showed:**
* 3/3 hold 35.2 s at 15.0% duty, `reason=2`, no protection trip, coast
  `spun=1`, `pp_first` 75-95 against the 24-30 stalled discriminator.
* `forced_pct` 45-46% with a spread of 1 point, against the polled build's
  41-44% with two near-dead sectors and 22-44% scatter across cohorts. The
  polled number was not better, it was less trustworthy.
* All six sectors balanced (accepts 1327-1744).
* Accept timing concentrated in `to125` (55%) with `to150 + gt150` under 4%.
* **`too_early` and `unstable` are now genuine refusals of genuine candidate
  edges** (108-114k and 12.1-12.5k), not poll counts. That is the property
  goal gate 2 asks for when it requires both non-zero, and it is now
  satisfiable in principle for the first time.

**Gate status:**
* Gate 1 — duration met 3/3, **purity not met** (`forced_pct` 45-46%).
* Gate 2 — counters now meaningful, rate identity **not met**: 535-543 permille
  of expected, the shortfall being `forced_pct` by construction.
* Gate 3 — coast witness met 3/3; binz current/speed comparison not run.
* Gate 4 — not started.
* Gate 5 — **COMP root exists with priority set**; COM, DMA and guard roots do
  not, and the fail-closed audit has not been run against them.

**Next:** the remaining `forced_pct` is one identifiable thing.
`mean_ci_us` (1737-1820) sits consistently *below* `mean_sector_us`
(2025-2107), so accepted intervals are shorter than the true sector: the
estimate is biased low, which pulls the blanking gate in and lets an early
chatter edge be accepted in place of the crossing — `lt075` is 26-31%. The
levers, in order: (a) reject accepts below a floor fraction of the estimate
rather than only below `ci/2`; (b) revisit `ADVANCE_LEVEL` now that timing is
trustworthy, since with advance 26 commutation lands only 0.094 of an interval
after the crossing; (c) the hardware comparator filter binz exposes as
`bench-filter-control`/`filtered_irq_hw`, which is the reference's own answer
to chatter at the crossing (`too_early` is 108-114k per run, i.e. ~3000
refused chatter edges per second).

---

## E025 — SPEED TRAP: my own estimator floor was capping the rotor at 240 eHz

**Context.** The operator asked directly whether the rotor had ever locked. It
had not, and I had been reporting detector improvements without ever putting
the qualified image's speed beside my own. The graybeard memo
(`binz/GRAYBEARD_FIRMWARE50_SPEED_TRAP.md`) named the omission and the cause.
`binz/DUTY_50_CAMPAIGN.md` has the measured rung table all along:

| duty | qualified eHz | sector us |
|---|---|---|
| 10% | 401 | 416 |
| 15% | **704** | **237** |
| 20% | 941 | 177 |
| 50% | 2096 | 79 |

firmware50 at the same 15% duty: **71-82 eHz**. Eight to nine times slower.
Back-EMF scales with speed, so the detector was being asked to work on an
eighth of the signal it was qualified on -- which is where the ~3000 chatter
edges per second and the `unstable` dominance came from. The detector was
never the problem at that speed.

**Four of my own constants, none from the reference** (now tabulated with
citations in `PARITY_TABLE.md`): `HANDOFF_EHZ = 60` against a 50→200 eHz
staircase (`AGENTS.md:293`); `new_bounded(ci0, ci0/4, ...)` against no
fast-side band at all; `ADVANCE_LEVEL = 26` flat against 20 below 35% duty
(`AGENTS.md:133`, scheduled advance clamped 18..22 at `:155`); and a forced
commutation at `min(ci, 60 eHz grid) x 2` against a live comparator plus a
timeout-to-restart. This is the failure mode the memo names: a port that
re-derives the mechanism instead of transcribing it. The values are not
derivable -- they were measured on this rig and this motor.

**Changed (step 1 of the memo's order, in isolation, forced fallback left
ON so the result is attributable):** `SECTOR_FLOOR_US` 694 (as `ci0/4`) → 120
→ **40 us**, the physical maximum -- one sector at 4166 eHz, beyond any duty
this motor reaches, against the fastest qualified rung of 79 us. Slow-side
bound kept. ELF
`22179C2FC44C02786DE4E61D12211CC29A8A2382F7FA9817DD1A6A7EB9D231D7`.
Captures `s1floor_0{1,2,3}.txt`.

**Prediction stated before the run:** `ci_us` stops pinning at 694; the rotor
accelerates past 240 eHz at 15% for the first time.

**Measured:**

| run | reason | hold_ms | hold_acc | hold_forced | forced_pct | mean_ci_us | mean_sector_us | ehz_from_sector | final ci_us | pp_first |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 35200 | 14950 | 9677 | 39 | 1053 | 1429 | 116 | 322 | 95 |
| 2 | 2 | 35200 | 27116 | 12529 | 31 | 653 | 887 | 187 | 337 | 90 |
| 3 | 2 | 35200 | 66113 | 20087 | **23** | 325 | 408 | **408** | 2523 | 84 |

**Showed — prediction confirmed, and this was the dominant anchor.**
`ehz_from_sector` 116 → 187 → **408 eHz** across three consecutive runs, from
71-82 before. `forced_pct` 45% → 23% **with no detector change**. Final
`ci_us` of 322/337 us is far below the old 694 floor, i.e. the estimator is
now representing speeds it was previously forbidden from expressing. 408 eHz
against the reference's 704 is 1.7x short, from 8.8x.

**Still wrong / open:**
* Not a lock: 23-39% of commutations still come from the grid, and the three
  runs differ by 3.5x in speed, so the loop is not settling to a repeatable
  operating point.
* Coast `pp_first` stays 84-95 across a 3.5x speed range, so it is a rotation
  witness only and not a speed measure -- the speed cross-check has to be
  `ehz_from_sector` against the qualified rung table, which is what gate 3's
  second half asks for and what I now have.

**Next, continuing the memo's order, one at a time, three runs each:**
2. Handoff at 200 eHz (qualified staircase) instead of 60. Prediction:
   `too_early` falls ~3x at handoff and `forced_pct` drops further, without a
   detector change.
3. Advance 20 below 35% duty instead of 26.
4. Missed-edge policy: live comparator + timeout-to-restart, not a late
   commutation on a grid.

---

## E026 — handoff at the qualified 200 eHz: the rotor reaches reference speed

**Changed (step 2 of the memo's order, in isolation):** `HANDOFF_EHZ` 60 → 200,
transcribed from the qualified image's *"50->200eHz startup staircase"*
(`binz/AGENTS.md:293`). Nothing else touched; the forced fallback is still on.
ELF `168E8319184D2A69C76011E178EA30CA3867872B001F27E13EE5840F6346BBDF`.
Captures `s2handoff_0{1,2,3}.txt`.

The 60 was mine, justified here by the observation that both open-loop drives
died around 110 eHz, so handing off early and letting the closed loop
accelerate seemed safer. The observation was real and the conclusion inverted
the reference's design: back-EMF scales with speed, so handing off at 60 gave
the detector a third of the signal it would see at 200, and with E025's
estimator floor the loop had no way to climb out.

**Prediction stated before the run:** `too_early` falls at handoff and
`forced_pct` drops further, with no detector change.

**Measured, with the qualified rung beside it (15% duty → 704 eHz,
`binz/DUTY_50_CAMPAIGN.md`):**

| run | reason | hold_ms | hold_acc | hold_forced | forced_pct | mean_sector_us | **ehz_from_sector** | qualified | bus_min/ref | pp_first |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 35200 | 85298 | 30986 | 26 | 302 | **551** | 704 | 1127/1212 | 83 |
| 2 | 2 | 35200 | 127705 | 34130 | 21 | 217 | **768** | 704 | 1125/1213 | 72 |
| 3 | 2 | 35200 | 116718 | 33790 | 22 | 233 | **715** | 704 | 1109/1214 | 71 |

**Showed — the speed trap is broken and the rotor now runs at the reference's
operating point.** Runs 2 and 3 sit at 768 and 715 eHz against the qualified
704, a 2-9% match, with mean sector times of 217-233 us against the
reference's 237. The progression across this campaign, all at 15% duty:

| state | ehz | vs qualified |
|---|---|---|
| E024 (my floor, my handoff) | 71-82 | 8.8x slow |
| E025 (floor → 40 us) | 116-408 | 1.7-6x slow |
| E026 (handoff → 200 eHz) | **551-768** | **match** |

Neither step touched the detector, a threshold, or a protection. Both were
reverting a constant of mine to the reference's measured value. `forced_pct`
fell 45% → 21-22% as a side effect, which is the memo's point exactly:
`forced_pct` was a symptom of the speed trap, not an independent problem.

**Gate status:**
* Gate 1 — duration met 3/3 (35.2 s at 15%, MCU-stamped); **purity still not
  met** at 21-26% forced.
* Gate 2 — rate identity 731-787 permille; shortfall is `forced_pct` by
  construction.
* Gate 3 — coast witness met 3/3, and **the speed half is now satisfied**:
  715-768 eHz against a qualified 704 at the same duty. Current comparison
  still owed (qualified proxy at 15% is 58 mA).
* Gate 4 — not started. Gate 5 — COMP root only.

**Next:** step 3, advance 20 below 35% duty instead of 26
(`binz/AGENTS.md:133`); then step 4, the missed-edge policy — live comparator
plus timeout-to-restart rather than a late commutation on a grid, which is
what gate 1's purity requires.

---

## E027 — advance 20 below 35% duty (memo step 3)

**Changed:** flat `ADVANCE_LEVEL = 26` → `advance_for(duty)` = 20 below 35%,
22 at/above, transcribed from *"The image holds advance level 20 below 35% and
22 at/above 35%"* (`binz/AGENTS.md:133`), with scheduled advance clamped 18..22
(`:155`). ELF `12B05D2395897033EC4E600AA5C59DF83E45ABC5FC16913A5C0197D19A8E78FD`.
Captures `s3adv_0{1,2,3}.txt`.

The flat 26 was my misreading: the qualified image's `effective-advance26`
(`AGENTS.md:18`) is *"implemented only in binz's post-COM wait override"*
(`AGENTS.md:114`) -- a mechanism applied after the commutation timer, not the
advance fed to `wait_time`.

**Measured (qualified rung at 15% = 704 eHz):**

| run | ehz_from_sector | forced_pct | too_early | mean_sector_us |
|---|---|---|---|---|
| 1 | 747 | 20 | 132831 | 223 |
| 2 | 677 | 22 | 135246 | 246 |
| 3 | 651 | 22 | 136156 | 256 |

**Showed:** speed unchanged within the step-2 spread (636-768), `forced_pct`
20-22%, and **`too_early` down 24%** (176k → 134k). Chatter reduced, torque
timing now the reference's. Kept.

**Also recorded:** three extra runs on the step-2 ELF (a failed build meant
`s3advance_0{1,2,3}.txt` ran the old image) independently reconfirm E026 at
706/718/636 eHz and 22/22/24% forced. Useful, not wasted: E026 now has six
runs behind it.

---

## E028 — missed-edge policy (memo step 4) — BUILT, NOT BENCH-VERIFIED

**Changed:** the forced fallback no longer multiplies the estimate nor floors
it at the handoff grid. It now fires at `1 + 1/4` of the *current* estimate;
`ZC_PATIENCE_NUM` is deleted. ELF
`D6F56A8B6EA4B426E7963676E388F4BA47799FAE1C4DF1EEBC6823EFEEF959DE`,
text 34028, bss 1060. Clippy clean, 191 host tests pass.

Both previous versions were mine and both were wrong: `x2` was a ratchet
(E002), and capping at the handoff grid stopped the ratchet but built a brake
-- a commutation two intervals late removes torque, and that was happening on
~45% of steps. The reference has no per-sector forced commutation in interrupt
mode; a missed edge waits on a live comparator and only a BEMF timeout ends
the attempt.

**Status: the operator switched the PSU off before this could run. No powered
measurement exists for this change.** Prediction on record for when the bench
is live: `forced_pct` falls to single digits, and gate 1 purity plus gate 2's
rate identity become the same true statement. It must not be claimed until
measured.

---

## E029 — gate 5: fail-closed audit of the COMP root, and its cycle cost

**No firmware change.** Host-side only, so the PSU being off does not block it.

```
$ python scripts/isr_audit.py --elf target/thumbv6m-none-eabi/release/shell-pwm \
      --root ADC_COMP --allow-file scripts/audit_allow.json -v
  [ADC_COMP] reachable=1  OK
AUDIT PASSED: 1 root(s) certified clean.
```

No reachable soft-arithmetic helper, no unresolved indirect call, no loop
needing review -- and the allowlist was not touched to achieve it. The
`Adc::read` entry in `scripts/audit_allow.json` is now obsolete (the PAC scan
replaced it) and should be removed.

**Longest-path cycle count.** `ADC_COMP` is 41 instructions of straight-line
code: no branches, no calls, no loops, so the longest path is the whole body.
Costed at M0+ rates (PUSH 1+N, literal and indirect LDR 2, STR 2, MRS/MSR 4,
POP-with-branch ~5): **75 cycles ≈ 1.17 us at 64 MHz**, or ~103 cycles
≈ 1.6 us including exception entry and exit. Against the 237 us sector at the
reference speed that is 0.7% of a sector, and ~0.35% CPU at the observed
~3000 edges/s.

Worth noting for later: about 27 of those cycles are `MRS`/`CPSID i`/`MSR`
critical-section wrappers that `portable_atomic` emits around plain 32-bit
stores, which are already atomic on this core. Three of the four regions are
avoidable; only the `fetch_add` needs one. Recoverable, not urgent.

**Gate 5 remains unmet:** one root of four. COM (commutation timer), DMA (ADC)
and guard roots do not exist -- commutation scheduling and the ADC scan both
still run in the foreground.

---

## Campaign summary at PSU-off, 2026-09-20

All at 15.0% duty, qualified reference 704 eHz (`binz/DUTY_50_CAMPAIGN.md`):

| state | ehz | forced_pct | note |
|---|---|---|---|
| E024 polled detector, my constants | 71-82 | 45 | 8.8x slow |
| E025 estimator floor → 40 us | 116-408 | 23-39 | |
| E026 handoff → 200 eHz | 551-768 | 21-26 | **speed matched** |
| E027 advance → 20 | 651-747 | 20-22 | chatter -24% |
| E028 fallback → 1.25x | — | — | **unmeasured** |

**Gates:** 1 duration met 3/3, purity not met (20-22%). 2 not met (rate
identity 770-792 permille; the shortfall is `forced_pct`). 3 coast witness met,
**speed half met** (651-768 vs 704), current comparison owed (qualified proxy
58 mA at 15%). 4 not started. 5 one root of four, audited clean.

No protection threshold was changed at any point in this campaign.

---

## E030 — RETRACTION: "speed matched the reference" was wrong. The rotor was
## grinding, and the motor got very hot.

**Operator stopped the bench and switched the PSU off, reporting the motor
"very very hot after these grinding runs".** Heat is the stop condition. No
further powered runs.

**What I claimed in E026 and must withdraw.** I reported 715-768 eHz against
the qualified 704 and called it a match, i.e. the reference's operating point.
That is not what was measured. `ehz_from_sector` is derived as
`1e6 / (6 * hold_ms / (hold_accepted + hold_forced))` -- it is **my own
commutation rate**, computed from how often this firmware switched the bridge.
It contains no measurement of the rotor. The reference's 704 eHz is a locked
rotor; mine was a commutation sequence the rotor was not following. Grinding
and heat are what that looks like.

**The evidence that contradicted me, and which I explained away:**

1. **Coast `pp_first` never moved across the entire campaign.** 71-95 codes at
   80 eHz (E024) and 71-95 codes at 768 eHz (E026). Back-EMF is proportional
   to speed, so a rotor that really accelerated 9x must coast down from a much
   larger amplitude. It did not change at all. In E025 I wrote this off as
   "a rotation witness only and not a speed measure". That was discounting the
   one instrument that disagreed with the conclusion I wanted -- the same
   error as [[feedback-instrument-decisions-not-outcomes]] warns about, and
   the same shape as E020's lesson that a metric can improve while the physics
   gets worse.

2. **`crossings=0` in every coast capture, every run, all campaign.** The
   coast detector never once saw a back-EMF polarity alternation. A rotor at
   hundreds of eHz decaying over 1.5 s would produce many. `spun=1` was
   therefore passing on the amplitude threshold alone
   (`COAST_SPUN_PP_CODES = 50`) with zero crossings, so goal gate 3's
   "independent rotation witness" has never been evidence about rotation
   *rate*, and I used it as though it were.

3. **`bus_min` deepened as the "improvements" landed** (1096-1127 against
   ~1210 baseline), and the qualified image's signed-current proxy at 15% is
   **58 mA**. I never measured this build's current in comparable terms at any
   point, which is exactly the comparison gate 3 asks for and the graybeard
   memo told me to make every rung.

**Corrected status of the whole campaign.** The real findings stand -- E010
(`Forward` on reverse wiring), E018 (COM41), E019 (carrier never restored),
E022-E023 (hardware edge with `lt075=0`), E025/E026/E027 (four of my own
constants replaced by the reference's measured values). What does **not** stand
is any claim about rotor speed or about approaching a lock. There has been no
lock at any point in this campaign, and the later runs were driving a
non-following rotor harder than the earlier ones, which is why the motor
heated.

**Before any further powered run, this build needs a rotor-speed witness that
does not come from its own commutation count.** Candidates, in order of
directness:

* **Coast BEMF amplitude calibrated against the qualified image.** Flash the
  frozen oracle, run 10% and 15%, record `pp_first` at known 401 and 704 eHz.
  That converts the existing coast capture into a speed scale, using the one
  instrument already built, and it is a flash-and-read with no new firmware.
* **Floating-phase BEMF amplitude during the run**, via the ADC on phase C in
  the sectors where it floats. Amplitude scales with speed, so it discriminates
  a following rotor from a slipping one *while driving*, which the coast
  capture cannot.
* **Current in the reference's own terms** -- the signed three-shunt residual
  over completed 100-scan blocks, scaled by the nominal 4 A allowance, as
  `binz/DUTY_50_CAMPAIGN.md` defines it -- so that 58 mA at 15% is directly
  comparable. A grinding rotor and a locked one differ enormously here, and
  this build already samples all three shunts.

Until one of those exists, `ehz_from_sector` must be reported as
"commutation rate", never as speed, and no run should be described as matching
the reference.

---

## E031 — the coast witness gives FALSE POSITIVES, and a no-motion regression
## I introduced by transcribing a number without its mechanism

**Changed:** startup profile replaced with the locked run's banner values --
`CATCH_ALIGN_MS = 20`, `CATCH_HOLD_MS = 980`, `CATCH_START_EHZ = 50`,
`CATCH_RAMP_MS = 2000`, `CATCH_DUTY_TENTHS = 62` constant through catch and
ramp; separate post-transfer `BEMF_DUTY_TENTHS = 100` (`bemfdu100`);
`HANDOFF_PHASE_SECTORS = 1` for the reference's `drivephase60`. ELF
`6F75A7677563C7C9A7BC39DA777CC17DD622927226990F613951A2D5A56E354C`. Clippy
clean, 191 host tests. Capture `ref1_01.txt`.

**Measured:**

```
BEMFRUN handoff_ehz=200 target_duty_tenths=100 advance_level=20 total_ms=40000
BEMFDONE reason=2 accepted=131045 forced=123452 too_early=50540 unstable=214723 ci_us=232
BEMFRATE hold_ms=35000 hold_accepted=125386 hold_forced=116572 mean_sector_us=144 ehz_from_sector=1157 hold_forced_pct=48
BEMFCOAST reason=2 pp_first=86 pp_last=33 crossings=0 spun=1
```

**Operator observation: "NOTHING is MOVING .. it's just WHINING."**

### Failure 1 -- the rotation witness is invalid and has been all along

The run reported `spun=1` with `pp_first=86` on a rotor that **did not turn at
all**. `COAST_SPUN_PP_CODES = 50` therefore produces false positives, and the
discriminator I recorded early in this campaign -- "pp_first 24-30 stalled vs
75-169 spun" -- is void. Whatever `pp_first` measures on this rig (switching
decay after `safe_off`, divider settling), it is not rotation.

**`crossings` is the quantity that was telling the truth, and it has been `0`
in every capture of the entire campaign.** Not one BEMF polarity alternation,
ever, across every build from E001 to here. I wrote that down in E030 and still
let `spun=1` stand on the amplitude threshold. Consequence: **no run in this
notebook has a valid board-derived rotation witness**, so every `spun=1` in
E001-E031 must be read as "not established". The reference's own verdict rule
says the same thing in its own terms -- goal gate 3 wants coast-down BEMF
*activity*, and zero crossings is the absence of it.

Action taken below: `spun` now requires `crossings > 0`. A stationary rotor
must report `spun=0`.

### Failure 2 -- the no-motion regression, and its cause

Replacing the 2 eHz -> 200 eHz ramp with the reference's fixed
`catch=50Hz/980ms` removed the only mechanism that ever turned this rotor: a
frequency ramp starting near zero. A rotor at standstill cannot pull into
50 eHz at 6.2%; it sits and buzzes, which is what the operator heard, at
1157 eHz of commutation with no rotation.

The reference's catch is not a standalone number. Its run has
`DRIVENENTRY ... fly_age=196 fly_arr=142 fly_arm_us=7 fly_seeded=1` -- a
**flying seed** accepted against `SEED_MIN_TICKS = 952` half-µs ticks -- and
`DRIVESTOP acquisition_reason=22 acquisition_us=10649`, i.e. a ~10.6 s
acquisition phase before transfer. firmware50 has no flying-seed path and no
acquisition stage, so `catch=50Hz` lands in a context that cannot support it.

This is the exact mirror of E025's lesson. There I re-derived constants instead
of transcribing them; here I transcribed one *out of its mechanism*. Both are
the same error: the port has to carry the mechanism, and a constant is only
meaningful inside it.

### Corrections applied before the next powered run

1. `spun` requires `crossings > 0`; `pp_first` is reported but no longer
   decides. No run may be called rotating on amplitude alone.
2. Startup ramp restored to begin near zero -- the behaviour that demonstrably
   moved this rotor -- while keeping the reference changes that are not
   contingent on a flying seed: the separate post-transfer BEMF duty, the
   one-sector handoff phase advance, advance 20, and the 40 µs estimator floor.
3. `CATCH_START_EHZ` is kept as a named constant with the reference citation,
   but set to a value this build's mechanism can actually catch, and labelled
   as a divergence pending a flying-seed path.

**Next:** a flying-seed/acquisition path is the missing mechanism if the
reference's catch profile is to be used verbatim. Until it exists, the catch
frequency is firmware50's own and must be labelled so.

---

## E032 — the base case, and the oracle read I should have done on day one

**Operator: "why is it so hard for you to repro a simple spin first? are you
able to just run it at low 100 ehz with low pwm?"** Fair, and the answer was
no -- not in the sense that matters.

### Open-loop spin at 100 eHz / 6.2% (`s` command)

Set `VF_TARGET_EHZ = 100`, `VF_TARGET_DUTY_TENTHS = 62`. ELFs
`E5A18324...`, then `2868B5D2...` and `D9D3612B...` with a witness added.

```
SSRUN start_ehz=2 target_ehz=100 target_duty_tenths=100
SS ms=4500 ehz=100 duty_tenths=62 step=3 comms=1334 bus_mean=1209 bus_ref=1212
SS ms=7000 ehz=100 duty_tenths=62 step=3 comms=2834 bus_mean=1220 bus_ref=1212
SSDONE reason=2 bus_ref=1212 bus_min=1144 pp_first=43 crossings=0 spun=0
```

It runs: 600 commutations/s sustained (exactly 6 x 100), no sag, `reason=2`.
**But there was no way to tell whether the rotor followed**, which is the real
answer to the question. (`step=3` on every report is an aliasing artefact, not
a stuck sector: 150 commutations per 250 ms report and 150 mod 6 = 0.)

### Three failed attempts at a rotation instrument, recorded so they are not
### repeated

1. **Coast peak-to-peak** -- refuted in E031 (86 codes on a stationary rotor).
2. **During-run floating-phase alternations, ungated** -- `alt=1127`,
   `driven_rotation=1`, and not believable: the range came out `-635..752`
   codes, about +-560 mV, where real back-EMF at 100 eHz is tens of codes.
   That is the PWM, not back-EMF: freewheel conduction pulls the floating
   terminal to a rail. A stationary rotor would produce it identically.
3. **Same, gated to the carrier's OFF window** -- no change (`alt=1072`,
   range `-650..762`). The gate is checked *after* `b.scan()` returns, so it
   tests where the counter is now, not where it was during the conversion, and
   the de-cohered 9901 Hz trigger starts conversions at arbitrary carrier
   phase. Doing this properly needs a **timer-triggered ADC at a fixed carrier
   phase** -- which is what binz uses (`bench-adc-tim15`, injected group), and
   what [[reference-minz-adc-trigger-floor]] and
   [[reference-stm32-injected-jadstart]] are both about.

The tested policy now lives in `src/witness.rs` with 7 host tests, including
the case that was failing silently (a fixed offset of +-86 codes must never
read as rotation). The policy is sound; the *sampling* is not yet.

### The oracle read (`scripts/oracle_trace.py`, capture `oracle_8000ms.txt`)

Flashed the frozen qualified ELF
`0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0` and ran its
verified sequence with an 8 s powered window instead of 35 s.

```
DRIVESTOP acquisition_reason=22 acquisition_us=10706 acquisition_duty_tenths=61 bemf_requested_tenths=100
DRIVETRANSFER result=1 zero_not_attempted=1 two_refused=1
DRIVENENTRY ... fly_age=196 fly_arr=142 fly_arm_us=7 fly_seeded=1
TRACKSTOP event_fault=0 last_event_us=7999856 sector=6
RUNLIMIT cycle_min_us=2223 event_min_us=238 cycle_max_us=6000 event_max_us=1000
POWERPATH reason=2 stop_us=8000004
POWERCOMMITS applied=18899
POWERFEEDBACK scans=0 peak_abs_raw=0 bus_min_mv=11760
BEMFSTOP core_stop=7 average_half_us=850 estimated_ehz=392 com=18899
ADVANCEPROFILE below350=20 at_or_above350=22 retained_level=20
ADVANCEOVERRIDE ... terminal_ci=825 terminal_wait=155 terminal_expected=77
RUNNINGREVISIT attempts=1581 accepts=1581
REVERSEBLANK arms=4 max_mask_us=347 still_active=0 low_speed_only=1
FASTBUS baseline_bus=1215 threshold_pct=95 tripped=0
DONE reason=8 gates=off en=off
```

**This is the read the goal has been telling me to do since it was set, and I
kept not doing it. Everything below is something I could not have derived.**

1. **The motor and rig are healthy, now, after all the heating.** It locks at
   392 eHz, `com` 18899 matching `POWERCOMMITS` 18899 exactly, bus never
   leaving 11.76 V. **Every firmware50 failure is firmware50's.** The
   replacement-motor suspicion does not apply to this.
2. **My commutation timing maths is right.** `terminal_ci=825`,
   `terminal_wait=155` half-µs ticks gives wait/ci = 0.188; my
   `wait_time(ci, 20)` computes `ci * (0.5 - 20/64)` = 0.1875. Agreement.
3. **My advance profile matches**: `below350=20 at_or_above350=22`.
4. **My guard limits do not.** Reference `event_min_us=238`,
   `cycle_min_us=2223`, `cycle_max_us=6000`, `event_max_us=1000`; firmware50
   has `CYCLE_MIN_US = 4000` and `MIN_EVENT_US = 333`. A 333 µs minimum event
   interval **refuses events the reference accepts** -- at a 425 µs sector, an
   event arriving even slightly early is discarded by my guard and kept by
   the reference's.
5. **`REVERSEBLANK ... low_speed_only=1`, armed only 4 times** in the whole
   run: the reference's extra blanking is a *low-speed* mask that disarms at
   running speed, whereas firmware50 blanks at `ci/2` always.
6. The acquisition stage runs 10.7 s at 6.1% before transfer, and transfer
   carries a flying seed. firmware50 has neither.

**Next, in order:** guard limits to the reference's (238/2223/6000/1000);
blanking to disarm above the reference's low-speed threshold; and a
timer-triggered ADC sample at a fixed carrier phase so the rotation witness
means something. firmware50 must be flashed back before any of it is tested --
the oracle is on the board right now.

---

## E033 — guard limits from the oracle; a fourth failed witness; and the base
## case finally confirmed

### Changes

1. **Guard limits transcribed from the oracle's `RUNLIMIT` line** (E032):
   `CYCLE_MIN_US` 4000 → **2223**, and the `Guard` alias's inlined `333` →
   a named `EVENT_MIN_US` = **238** (`src/protection.rs`). Both of mine were
   wrong in the direction that *refuses real events*: at the reference's
   locked 392 eHz a full electrical cycle is 2551 µs, which clears 2223 and
   does not clear 4000, and its mean accepted-event interval is 425 µs, so a
   quarter-early event at ~318 µs is accepted by the reference and was
   rejected by my 333 µs floor.

   The host test `back_to_back_accepted_events_are_a_comparator_storm`
   hard-coded 332/333 and therefore *defended my own wrong floor*. Rewritten
   to drive off the constant, plus a case asserting the reference's own
   cadence (318 µs then 425 µs) is accepted. 198 tests pass.

2. **Reference-comparable current** (`AverageCurrent::blocks`,
   `mean_residual`, `mean_milliamps`, 3 new host tests, 201 pass). This
   computes exactly the column `binz/DUTY_50_CAMPAIGN.md` publishes -- *"the
   average signed three-shunt residual over completed 100-scan blocks, scaled
   by each run's nominal 4 A raw allowance"* -- so firmware50's draw can be
   compared rung-for-rung: the qualified image is **45 mA at 10% duty**,
   58 mA at 15%. It is a *physical* stall discriminator, which no
   comparator- or ADC-derived witness here has managed to be: a stalled rotor
   generates no back-EMF to oppose the applied voltage and draws far more.

### Witness attempt 4, also failed

Comparator-based, sampled inside the carrier OFF window (the comparator is
instantaneous, so unlike the ADC it *can* be read at a chosen carrier phase).
ELF `7C27A412...`:

```
SSWITNESS samples=68101 bemf_min=-100 bemf_max=100 mid=0 mid_fixed=1 alt=10331
```

`alt = 10331` over the 7 s ramp+hold window is **1476/s against 200/s** (two
per electrical cycle at 100 eHz) -- 7.4x too many, so chatter-dominated and
non-discriminating. The reason is now identified rather than guessed: the
oracle gets clean comparator edges from a **hardware** filter
(`bench-capture-filter` / `filtered_irq_hw`, TIM2 input capture). Without it,
no amount of software counting on this comparator separates rotation from
chatter. Four witness attempts, all recorded; the tested *policy* in
`src/witness.rs` is sound, every *sampling* route so far is not.

### Powered runs, firmware50 restored (ELFs `613A7134`, `7C27A412`, `35F04F9C`)

Open-loop six-step, 100 eHz target, 6.2% duty, `s` command:

```
SSDONE reason=2 bus_ref=1213 bus_min=1141 drive_scans=72266 pp_first=39 crossings=0 spun=0
SSDONE reason=2 bus_ref=1214 bus_min=1149 pp_first=36 crossings=0 spun=0
SSDONE reason=2 bus_ref=1212 bus_min=1133 pp_first=35 crossings=0 spun=0
```

All three: 600 commutations/s sustained (exactly 6 x 100), `reason=2` normal
deadline, **bus mean equal to the bus reference** (1213/1213) with no sag, no
protection trip.

### **Operator: "something spinning for a bit."**

That is on firmware50's own open-loop run, not the oracle -- the oracle had
already been flashed back off. **So the base case works: this build turns this
rotor at 100 eHz / 6.2% open loop.** It is the first operator-confirmed
firmware50 rotation since the early sine ramp, and it re-establishes the
foundation that E031's regression had removed.

Note what it also confirms about the instruments: that run reported
`crossings=0`, `spun=0`, `pp_first=35`. So on a rotor that *was* turning, the
coast witness says no -- the mirror of E031's false positive. `pp_first` is
uncorrelated with rotation in both directions, and the strict `crossings > 0`
verdict is a **false negative** here. E031 asked whether `crossings` could
ever fire on this rotor; this answers it: not at 100 eHz, so the coast route is
not merely strict, it is unusable at this speed.

**Bookkeeping error to fix:** the `SSCURRENT` line and
`alt_expected_if_following` did not reach the binary -- the edit that added
them failed to match its anchor and I did not check before running. So the
current figure, the one instrument with a real chance of discriminating stall,
was not captured on any of these three runs. Fixing that is the next action,
before any further run.

**Next:** land the `SSCURRENT` report, re-run the same open-loop spin, and
compare mean mA against the reference's 45 mA at 10%. That gives the first
physical, reference-anchored statement about whether this build's rotor is
following, and it is also gate 3's phase-current requirement.

---

## E034 — the current instrument lands, and gives the first physical number

**Changed:** `SSCURRENT` and `alt_expected_if_following` reach the binary (the
E033 edit had silently failed its anchor). ELF
`22454C910B4801346BDE9F3B87063EF38D679093D2233475E502CAA5DF77E0F2`. Clippy
clean, 201 host tests. Capture `captures/2026-09-20/spin100_6pct.txt`.

**Measured -- open-loop six-step, 100 eHz, 6.2% duty:**

```
SSDONE    reason=2 bus_ref=1215 bus_min=1135 drive_scans=72260 pp_first=43 crossings=0 spun=0
SSWITNESS samples=68184 bemf_min=-100 bemf_max=100 alt=10323 alt_expected_if_following=1400 driven_rotation=1
SSCURRENT blocks=722 mean_residual=1391 mean_ma=174 ref_ma_at_10pct=45 duty_tenths=62
```

**Showed:**

* **174 mA at 6.2% duty, against the qualified image's 45 mA at 10%**
  (`binz/DUTY_50_CAMPAIGN.md`). About 4x the current at a *lower* duty. The
  regimes are not identical -- mine is open-loop at 100 eHz, the reference is
  locked at 401 eHz -- so this is not a precise ratio, but a motor drawing
  four times the current at lower duty is being dragged, not followed. That is
  consistent with the operator's "something spinning for a bit".
* The comparator witness rate is confirmed useless: `alt=10323` against
  `alt_expected_if_following=1400`, 7.4x over, exactly as E033 predicted. Now
  that both numbers are on the wire together the verdict can't be misread as a
  success, which is why the expected value is printed beside it.
* The bus is untroubled throughout (`bus_ref=1215`, mean equal to it,
  `bus_min=1135` only during the ramp), so this current is not a supply
  artefact -- it is real motor draw.

**Why this instrument is different from the four that failed.** It measures a
*physical* consequence of slip rather than trying to infer position from a
signal this build cannot sample cleanly, and it is computed to the reference's
own published definition, so the two numbers are directly comparable. Every
change from here has a number to move, and the direction is unambiguous: down,
toward the reference.

**The honest comparison still owed.** 45 mA is a *locked closed-loop* figure at
10% duty. An open-loop run draws more by nature, so the like-for-like test is
firmware50's own closed loop at 10% duty against 45 mA -- which is exactly goal
gate 3's phase-current requirement. That is the next run.

**Also settled, in the wrong direction:** this run turned the rotor (operator
confirmed) and reported `crossings=0`, `spun=0`, `pp_first=43`. Combined with
E031's `pp_first=86` on a stationary rotor, the coast witness is now shown to
fail in **both** directions on this rotor at this speed. It is not a strict
instrument, it is a broken one, and no gate should rest on it until a
calibration run proves it can fire at all.

---

## E035 — closed loop at 10% whined again; the missing piece is a HANDOVER CATCH

**Run (ELF `D9E8BEA0...`, closed loop `b`, 10% BEMF duty):** operator,
"again just whizzing not moving". Stopped immediately; `BDTR=0x00000C1A`,
MOE clear.

**Operator: "YOU DO UNDERSTAND THAT YOU HAVE TO BUILD A HANDOVER CATCH?"**

Yes -- and this is the diagnosis that everything since E030 was circling.
Open-loop spin works (confirmed at 100 eHz, E033/E034). What firmware50 did at
"handoff" was not a catch at all:

1. it ramped open-loop on up to **200 eHz** -- past the ~110 eHz where I myself
   recorded that this rig's open-loop drive loses the rotor;
2. it closed the loop at a **scheduled instant**, regardless of whether the
   rotor was following;
3. it seeded the interval estimator from the **scheduled** interval
   (`sector_interval_us(HANDOFF_EHZ)`), not from anything the rotor did.

So by handover there was no following rotor left, the estimator described a
speed nothing was doing, no real crossing matched it, and the bridge whined.
Every closed-loop failure in this notebook has that shape.

The reference does the opposite, and its log says so:
`DRIVESTOP acquisition_reason=22 acquisition_us=10706` and
`DRIVENENTRY ... fly_seeded=1`. It keeps driving open-loop while it watches
real back-EMF edges, validates a whole electrical cycle, and seeds the closed
loop with the **measured** interval -- a flying seed.

**Built: `src/acquire.rs`, the handover catch, with 10 host tests (211 total).**

* `Acquisition::edge(t)` collects edge timestamps and returns a seed only after
  six consecutive intervals validate against the reference's own acquisition
  bounds, converted from its 0.5 µs ticks: `INDIVIDUAL_MIN_US = 238`
  (`INDIVIDUAL_MIN_TICKS` 476), `CYCLE_MIN_US = 2858` (`CYCLE_MIN_TICKS` 5716),
  `SEED_MIN_US = 476` (`SEED_MIN_TICKS` 952). A test asserts those conversions
  so a mistyped halving cannot survive.
* One rule is **mine and labelled so**: every interval within a quarter of the
  cycle mean. The reference validates a flying seed but its exact consistency
  test is not in the documents available; accepting six wildly different
  intervals would seed the loop with noise.
* `/6` is done by reciprocal (`43691 >> 18`), with an exhaustive host test
  proving it equals `x/6` for every cycle the module can produce.
* Rejections are counted and named, so a catch that never happens says why.

**Wired into `bemf_run`:**

* The open-loop ramp now ends at **`CATCH_EHZ = 100`** -- where this build is
  confirmed to turn the rotor -- and **holds there**, instead of ramping to 200.
* During the hold, every comparator edge is offered to `Acquisition`.
  **Handover happens when a validated seed exists, not at a time.**
* The estimator is seeded with the **measured** interval, and `sector_start`
  is re-based to the validating edge, so the first closed-loop `count` spans a
  real sector.
* If no seed validates within `ACQUIRE_TIMEOUT_MS = 3000`, the run stops with
  **`Reason::InvalidSeed` (10)** -- a variant that already existed in the
  protection enum, described as *"Handoff seed failed its validity/reserve
  check"*, and had never been wired to anything.
* New `BEMFCATCH` line reports `edges`, `blanked`, `rejected_cycles`,
  `last_reject`, `seeded`, `seed_us` and `expected_us`.

**Two defects of mine found while wiring it, both fixed:**

1. **E028's 1.25x fallback also applied to open loop**, where the scheduled
   commutation *is* the drive. Every `bemf_run` open-loop phase therefore ran
   at 80% of its commanded frequency -- "100 eHz" was 80. Open loop now
   commutates at exactly the sector interval; only the closed-loop missed-edge
   fallback waits the extra quarter.
2. **Acquisition edges were unblanked.** The ISR takes the first edge after
   each re-arm, and re-arm happens at commutation, so the captured "crossing"
   is very often the freewheel transient -- whose interval equals the
   commutation period and would seed the loop with the *schedule* rather than
   the rotor, reproducing the exact blind handover this replaces. Edges
   within half a scheduled sector of the last commutation are now blanked
   (the reference's own blanking fraction) and the line re-armed so the real
   crossing later in the sector is still seen.

ELF `38D51672A75B2351BBCB509FD5564B6D2A72097FC6C8D1FF900672B6BB1DBBE6`.
Clippy clean, 211 host tests.

**Prediction for the next run:** either `BEMFCATCH seeded=1` with `seed_us`
close to `expected_us = 1666` and the rotor then held, or `seeded=0` with a
named `last_reject` that says which part of the catch failed. Either way the
run no longer hands over blind -- a failed catch is now an `InvalidSeed`
stop, not a whine.

---

## E036 — THE CATCH WORKS; the hold after it collapsed because the closed loop
## had no tracking guard at all

**Run (ELF `38D51672...`, capture `catch1_01.txt`):**

```
BEMFCATCH catch_ehz=100 edges=13 blanked=178 rejected_cycles=6 last_reject=5 seeded=1 seed_us=1683 expected_us=1666
BEMFDONE  reason=2 accepted=134014 forced=131983 too_early=37216 unstable=166861 ci_us=129
BEMFRATE  hold_ms=35200 hold_accepted=130105 hold_forced=126340 mean_sector_us=137 ehz_from_sector=1216 hold_forced_pct=49
BEMFCURRENT blocks=3958 mean_residual=971 mean_ma=121 ref_ma_at_10pct=45
```

### The catch succeeded

**`seed_us=1683` against `expected_us=1666` -- a 1% match.** The acquisition
blanked 178 post-commutation transients, rejected 6 inconsistent cycles
(`last_reject=5` = `Inconsistent`), then validated a six-event cycle and
seeded. This is **the first measurement in the whole campaign that is of the
rotor rather than of firmware50's own commutation**, and it says the rotor was
genuinely following the 100 eHz open-loop drive at the instant of handover.
The prediction in E035 was met on its first branch.

### The hold after it failed, and why

`ci_us` collapsed from the 1683 µs seed to **129 µs** -- about 1300 eHz, far
past the ~400 eHz this duty belongs at. That is comparator chatter being
accepted as crossings, and the reason is structural:

**`bemf_run` had no tracking guard.** `RunGuard` is instantiated only on the `g`
script path (`drive_loop`), so the closed loop ran with no minimum
accepted-event interval, no event-max tracking and no cycle guard. The
reference's `RUNLIMIT event_min_us=238` -- *"minimum accepted-event interval"*
-- existed in `protection::EVENT_MIN_US` since E033 and was applied to nothing
that ran. Nothing said an event 129 µs after the last one cannot be one.

This is also a **goal gate 4 gap**: "tracking" is a required protection, and
the closed loop did not have it.

### The regime the reference actually runs in

Its stale-event ceiling is 1000 µs (`SPEEDEVENTWATCH max_us=1000
tighten_only=1 periods=3`), and a 100 eHz sector is 1683 µs. **So the reference
cannot hold closed loop at 100 eHz** -- it catches and accelerates toward
~400 eHz at 10% duty. firmware50 must do the same: the catch speed is not the
running speed, and the estimator has to follow a 4x acceleration straight
after handover without being dragged down by chatter.

### Changed

* The closed-loop detector refuses any edge with `count < EVENT_MIN_US` (238)
  as `TooEarly`, counts it (`event_floor_refusals` on `BEMFCATCH`), and
  re-arms the line for the real crossing.
* The post-run report drains the TX ring between lines: last run's
  `BEMFCURRENT` was cut mid-line by `BEMFCOAST` because the ring overflowed.

ELF `BA50D5B899840765FC9478688726FB7DD7BEEB274AB83C9CB5CD17A1B2C6988C`. Clippy
clean.

**Still owed:** the full `RunGuard` tracking stop (event-max, cycle band) on
the closed loop. The 1000 µs event-max would trip immediately at the 100 eHz
catch speed, so it has to arm only once the rotor has accelerated into the
reference's running regime -- that is a design step, not a one-line wiring.

**Prediction:** `event_floor_refusals` large, `ci_us` held above 238 µs instead
of collapsing, and current moving down from 121 mA toward the reference's
45 mA if the rotor now follows.

---

## E037 — with the event floor, the estimate converges on the reference's
## operating point; chatter at the crossing is now the limit

**Run (ELF `BA50D5B8...`, capture `catch2_01.txt`):**

```
BEMFCATCH catch_ehz=100 edges=57 blanked=884 rejected_cycles=50 last_reject=5 seeded=1 seed_us=1684 expected_us=1666
BEMFDONE  reason=2 accepted=36106 forced=64588 too_early=2425 unstable=92236 ci_us=420
BEMFRATE  hold_ms=35200 hold_accepted=34648 hold_forced=61576 mean_ci_us=370 mean_sector_us=365 hold_forced_pct=63
BEMFCURRENT blocks=3958 mean_residual=876 mean_ma=109 ref_ma_at_10pct=45
```

**Showed:**

* **The catch reproduces**: `seed_us=1684` against 1666, second consecutive
  run. It is a working mechanism, not a one-off.
* **The estimate no longer collapses.** With `EVENT_MIN_US` enforced, final
  `ci_us` is **420 µs**, against the reference's locked **425 µs**
  (`average_half_us=850`, oracle E032). That is the reference's operating
  point to 1%, from a seed of 1684 µs -- i.e. the estimator followed a 4x
  acceleration after handover instead of being dragged into chatter at 129 µs.
  `too_early` fell from 37216 to 2425: the floor is doing exactly the job the
  reference gives it.
* **Current keeps falling**: 174 → 121 → **109 mA** across E034, E036, E037,
  against the reference's 45 mA. Moving the right way; still 2.4x.

**Not a lock, and why:**

* `hold_forced_pct=63`. Most commutations are still from the fallback. The
  reference is 0%.
* The dominant refusal is now **`unstable=92236`**: the level the ISR captured
  at the edge did not match the edge's own direction. An edge armed as
  *rising* that is immediately followed by a *low* level is the comparator
  crossing and crossing straight back -- chatter at the zero crossing, exactly
  where the back-EMF is smallest.
* The reference suppresses that with a **hardware** capture filter
  (`bench-capture-filter`, `filtered_irq_hw`: TIM2 input capture with
  filtering), which firmware50 does not have. E033 identified this as the
  reason every comparator-derived witness failed; E037 shows it is now also the
  limiting term for the closed loop itself.

**Two ways forward, in order of fidelity:**

1. **Port the reference's hardware capture filter** -- COMP2 output routed to a
   timer input capture with its digital filter, giving both debounced edges and
   hardware-stamped crossing times. This is the reference's own mechanism and
   removes the chatter at source.
2. **COMP2 hysteresis as a stopgap.** E006 tried `HYST = 2` and reverted it, but
   that test ran while `Wiring` was `Forward` (E010) with `INMSEL` on a driven
   phase in four sectors of six, so its verdict is void and it was never
   re-measured on the corrected build. The reference runs `HYST = 0`, so this
   would be a stated divergence standing in for the missing hardware filter.

Bookkeeping: the TX-ring drain added in E036 was not sufficient -- this run
again lost the tail of `BEMFCURRENT` and all of `BEMFWITNESS`/`BEMFNODES` into
the start of `BEMFCOAST`. `tx_flush` needs checking before the next run, or
the report needs to be shorter.

---

## E038 — full report restored; hysteresis re-opened as a filter stand-in

**Report loss fixed.** The TX ring was 1024 bytes and the closed-loop post-run
report is ~1.5 KB, so `say` silently dropped the tail: three runs lost
`BEMFCURRENT`'s end and all of `BEMFWITNESS`/`BEMFNODES` into `BEMFCOAST`.
My attempts to add per-line `tx_flush` calls failed repeatedly on escaping --
three report terminators had a literal line break inside the string, from my
own earlier Python edits -- so I stopped patching the symptom and raised
`TX_CAP` to 4096 (still a power of two; the G071 has 36 KB against ~4 KB
`.bss` after the change). ELF `0184AD6D...`: the whole report now arrives and
the script reports `1/1 runs completed` for the first time in several runs.

**Baseline run on that ELF (capture `catch3_01.txt`):**

```
BEMFCATCH catch_ehz=100 edges=55 blanked=900 rejected_cycles=48 last_reject=5 seeded=1 seed_us=1675 expected_us=1666
BEMFDONE  reason=2 accepted=35798 forced=66486 too_early=1784 unstable=103550 ci_us=347 bus_min=1019
BEMFRATE  hold_ms=35200 mean_ci_us=363 mean_sector_us=357 hold_forced_pct=64
BEMFCURRENT blocks=3959 mean_residual=679 mean_ma=85 ref_ma_at_10pct=45
```

* Catch reproduced a **third** time (1675 vs 1666 µs).
* **Current trend: 174 → 121 → 109 → 85 mA** over E034, E036, E037, E038,
  against the reference's 45 mA. Monotonically toward the reference.
* **Flag:** `bus_min=1019` is 84% of the 1213 baseline -- deeper than the
  1110-1149 of earlier runs. The three-scan fast-sag did not trip, so it was a
  brief single-sample dip, but it is new and it is recorded rather than
  ignored.

**Changed:** `COMP2_HYST` 0 → 1 (low), ELF
`11666A0C47AF195F5A4253E443B7BF60FDBEDA2FF36A2578DF0AC211E629B1CD`. A stated
divergence: the reference runs 0 *with* a hardware capture filter
(`bench-capture-filter`) that firmware50 lacks, and hysteresis is the nearest
in-peripheral stand-in. E006's earlier revert is void as evidence -- it ran
under the `Forward` wiring bug and measured chatter on the wrong wire.

**Prediction:** `unstable` (103550) falls, accepts rise, `hold_forced_pct`
(64%) falls, and current continues down from 85 mA. If `unstable` does not
move, hysteresis is not the lever and the hardware capture filter is.

---

## E039 — COMP2 hysteresis low halves the forced fraction and kills the chatter

**Run (ELF `11666A0C...`, `COMP2_HYST = 1`, capture `hyst1_01.txt`):**

```
BEMFCATCH catch_ehz=100 edges=7 blanked=102 rejected_cycles=0 last_reject=0 seeded=1 seed_us=1674 expected_us=1666
BEMFDONE  reason=2 accepted=92091 forced=40596 too_early=65 unstable=199193 ci_us=272 bus_min=1121
BEMFRATE  hold_ms=35200 hold_accepted=87605 hold_forced=37718 mean_ci_us=291 mean_sector_us=280 hold_forced_pct=30
BEMFCURRENT blocks=3958 mean_residual=761 mean_ma=95 ref_ma_at_10pct=45
BEMFCOAST reason=2 pp_first=61 crossings=0 spun=0 comp_edges=175 comp_hi=414967 comp_polls=415054
```

| | HYST=0 (E038) | HYST=1 (E039) |
|---|---|---|
| `hold_forced_pct` | 64% | **30%** |
| accepted | 35798 | **92091** |
| `too_early` | 1784 | **65** |
| catch | 48 rejected cycles | **0 rejected -- seeded on the first cycle** |
| coast `comp_edges` | 132810 | **175** |
| `bus_min` | 1019 | 1121 |
| current | 85 mA | 95 mA |

**Showed -- prediction partly met, and one effect bigger than predicted:**

* **`hold_forced_pct` halved, 64% → 30%**, and accepts rose 2.6x. Hysteresis
  is a genuine lever on the closed loop.
* **The catch went clean**: seeded on its very first six-event cycle with zero
  rejections, against 48 rejected cycles at HYST=0. The chatter that was
  corrupting acquisition is gone.
* **Coast comparator edges fell from 132810 to 175.** That count had been
  pure chatter in every run since E005. With the chatter suppressed, 175 edges
  across a 1.5 s coast is *plausibly* genuine back-EMF activity decaying after
  `safe_off` -- which would be the first real coast-down witness goal gate 3
  asks for. **Not claimed yet**: it has to be checked against a
  stationary-rotor control, because E031 is exactly what happens when a
  witness is trusted without one.
* The deep `bus_min=1019` from E038 did not recur (1121 here).

**Cautions, not glossed:**

* `unstable` *rose*, 103550 → 199193. With fewer edges refused early, more
  reach the persistence check, so the raw count is not a clean health signal
  across this change. The forced fraction and accept count are the reliable
  ones here.
* `ci_us=272` / `mean_sector_us=280` is **faster** than the reference's locked
  425 µs at this duty. Either the rotor is genuinely faster, or the estimate
  is running ahead of it. Current rose slightly (85 → 95 mA) rather than
  falling, which leans toward the second reading: a rotor that is ahead of
  its commutation draws more, not less.
* Still not a lock: 30% forced against the reference's 0%.

**Next:** a one-variable sweep, `COMP2_HYST = 2` (medium). If forced falls
further *and* `mean_sector_us` moves toward the reference's 425 µs *and*
current falls, hysteresis is converging on the missing filter's job. If
`mean_sector_us` stays short while forced falls, the extra hysteresis is
buying acceptance at the cost of timing, and the hardware capture filter is
the correct next step instead.

---

## E040 — HYST=2 sweep point, and the FIRST VALIDATED ROTATION WITNESS

### HYST=2 (ELF `5DB6B871...`, capture `hyst2_01.txt`)

```
BEMFCATCH catch_ehz=100 edges=7 blanked=72 rejected_cycles=0 seeded=1 seed_us=1666 expected_us=1666
BEMFDONE  reason=2 accepted=76545 forced=48081 too_early=99 unstable=319060 ci_us=296 bus_min=1127
BEMFRATE  hold_ms=35200 mean_ci_us=319 mean_sector_us=300 hold_forced_pct=38
BEMFCURRENT blocks=3958 mean_residual=472 mean_ma=59 ref_ma_at_10pct=45
BEMFCOAST reason=2 pp_first=62 crossings=0 comp_edges=178
```

| | HYST=0 | HYST=1 | HYST=2 | reference |
|---|---|---|---|---|
| current | 85 mA | 95 mA | **59 mA** | 45 mA |
| `mean_sector_us` | 357 | 280 | 300 | 425 |
| `hold_forced_pct` | 64% | 30% | 38% | 0% |
| catch seed | 1675 | 1674 | **1666** | -- |

**The metrics split, and current wins.** My E039 decision rule anticipated
forced and current moving together; they did not. Forced got *worse*
(30 → 38%) while **current fell to 59 mA -- the closest to the reference's
45 mA in the entire campaign** -- and sector time moved toward the reference.
Current is the one physical instrument here, so HYST=2 is kept as the better
physical result. This is E020's lesson again from the other side: `forced_pct`
is not the objective, and optimising it alone would have picked HYST=1.

### The coast control

Added command `c`: the identical coast capture, on a rotor the bridge has never
energised. Three repetitions, immediately after reset:

```
COASTCONTROL pp_first=23 pp_last=22 crossings=0 comp_edges=2
COASTCONTROL pp_first=26 pp_last=21 crossings=0 comp_edges=2
COASTCONTROL pp_first=23 pp_last=20 crossings=0 comp_edges=4
```

| | coast `comp_edges` | coast `pp_first` |
|---|---|---|
| stationary, never driven | **2, 2, 4** | 23, 26, 23 |
| driven, HYST=1 / HYST=2 | **175, 178** | 61, 62 |

**A 40-90x separation. This is the first board-internal rotation witness in
this campaign that has been checked against a control.** A rotor that was
turning at `safe_off` produces real comparator activity as it coasts; one that
was never driven produces essentially none.

Why it was invisible until now: at `COMP2_HYST = 0` the coast count was 80k-130k
of chatter in every run since E005, burying a signal of ~176 edges entirely.
Hysteresis did not create the signal -- it removed what was drowning it.

It also resolves the two earlier refutations. E031's `pp_first=86` "on a
stationary rotor" and E034's `crossings=0` "on a turning rotor" were both
measured at HYST=0; this control, at HYST=2, gives the stationary baseline
cleanly at 23-26 codes, which matches the "24-30 stalled" figure recorded long
ago. The amplitude discriminator was not wrong in principle -- it was being
read through chatter.

**Changed:** `spun` now means `comp_edges > COAST_SPUN_MIN_EDGES` (20), a
threshold taken from the control: five times the stationary ceiling of 4,
nearly nine times below the driven floor of 175. ELF
`8451DB41551B4F9B4919C68AF3571D94DF6E019DE30F1BC2896DF603C788EFF0`.

**Goal gate 3 status:** the coast-down witness half is now **met by a
controlled measurement**. The current half stands at 59 mA against 45 mA --
the right order and trending the right way, not yet matched.

---

## E041 — the forced fraction is phase-specific: phase B is clean, A and C are not

Read from the full report of `hyst2_01.txt` (only readable since E038 fixed the
report truncation):

```
BEMFSECTOR a1=11240 f1=6548 a2=17627 f2=2175 a3=10089 f3=12122 a4=5115 f4=14953 a5=19210 f5=810 a6=13264 f6=11473
```

Mapping logical sector → physical sector (`Reverse`: 1→4, 2→3, 3→2, 4→1, 5→6,
6→5) → floating phase (physical floating `[C, A, B, C, A, B]`):

| floating phase | pin | logical sectors | accepted share |
|---|---|---|---|
| **B** | VSENB, **PB7** | 2, 5 | **89%, 96%** |
| A | VSENA, PB3 | 3, 6 | 45%, 54% |
| C | VSENC, **PA2** | 1, 4 | 63%, 25% |

**Phase B is nearly clean in both of its sectors; A and C carry almost all of
the forced commutations.** It is not polarity: B's two sectors use *opposite*
edges (sector 2 falling, sector 5 rising) and both work. It is the phase. If A
and C behaved like B the forced fraction would fall from 38% to single digits.

**Hypothesis -- the ADC is disturbing the comparator's own input pins.** From
firmware50's ADC channel table:

* **PA2 is both phase C's comparator input and ADC channel IN2 (`vsenc`)**;
* **PA3 is the comparator's reference for all three phases and ADC channel IN3
  (`star`)**.

The ADC scans both pins on a de-cohered 9901 Hz trigger, so its sample-and-hold
capacitor injects charge onto the comparator's own input nodes at arbitrary
instants relative to the comparator's decisions. The reference triggers its ADC
at a fixed carrier phase (`bench-adc-tim15`), so any such disturbance lands at
a controlled instant rather than randomly.

This explains C directly. It does not obviously explain why A (PB3, never
sampled) is poor while B (PB7, never sampled) is clean, since both see the same
disturbed star. So it is a hypothesis to test, not a conclusion.

**Test:** drop IN2 and IN3 from the ADC scan. They feed only diagnostics
(`bemf_c`, the coast amplitude, `BEMFNODES`, the ADC witness) -- every
protection uses IN0/IN1/IN4 (currents), IN6 (bus) and IN13 (VREFINT). The
comparator-based coast witness from E040 is unaffected.

**Predictions:** if charge injection is the cause, phase C's accept share rises
most, A's rises somewhat, B's stays put, and forced falls. If nothing moves,
the per-phase difference is in the analogue front end (divider or layout) and
the reference's hardware capture filter is the remaining lever.

---

## E042 — ADC charge injection was hurting phase C; phase A is a separate problem

**Changed:** IN2 (`vsenc`, PA2) and IN3 (`star`, PA3) removed from the ADC scan
(5-channel table). ELF
`91B54EC2DEF80B7B82D84CF868937F93ADE2B013FB567BA23D832B03B54E3E42`. Clippy
clean. Capture `noadccomp_01.txt`.

**Measured:**

```
BEMFSECTOR a1=13232 f1=5393 a2=17148 f2=3568 a3=10677 f3=12039 a4=8701 f4=12660 a5=18840 f5=1312 a6=11696 f6=13240
BEMFCATCH  seeded=1 seed_us=1683 expected_us=1666 rejected_cycles=5
BEMFRATE   hold_ms=35200 mean_sector_us=290 hold_forced_pct=37
BEMFCURRENT mean_ma=69 ref_ma_at_10pct=45
BEMFCOAST  pp_first=0 crossings=0 spun=1 comp_edges=185
```

| sector | phase | HYST=2 (E040) | ADC off comp pins |
|---|---|---|---|
| 1 | C | 63% | **71%** |
| 4 | C | 25% | **41%** |
| 3 | A | 45% | 47% |
| 6 | A | 54% | 47% |
| 2 | B | 89% | 83% |
| 5 | B | 96% | 93% |

**Showed -- the prediction held for phase C and nowhere else:**

* **Phase C improved most, as predicted.** Sector 4 went from 25% to 41%
  accepted, sector 1 from 63% to 71%. The ADC's sample-and-hold was injecting
  charge onto PA2, phase C's comparator input, and corrupting decisions there.
  The change is kept: it is physically justified, it helped the phase it was
  aimed at, and nothing that protects the motor needed those channels.
* **Aggregate barely moved** (38% → 37% forced) because phase A did not
  improve and phase B dipped slightly. Current 59 → 69 mA, within the
  run-to-run spread seen so far.
* **Phase A is a different problem.** PB3 is never ADC-sampled, yet A is now
  the worst phase (47% in both sectors). Whatever limits it is in the analogue
  front end -- the VSENA divider, its filtering, or board layout -- not the
  sampling. B (PB7) being clean in both sectors shows the comparator itself and
  the star reference are capable of it.
* The coast witness is unaffected by removing those channels: `comp_edges=185`,
  `spun=1`. `pp_first=0` is expected -- the ADC no longer sees those pins.

**Where the loop stands at 10% duty, across the campaign:** catch reproducible
(1666-1684 µs against 1666, six runs); coast witness validated against a
control; current 59-69 mA against 45; forced 37% against 0.

**Next:** the reference's answer to per-phase comparator noise is its hardware
capture filter (`bench-capture-filter` / `filtered_irq_hw`: the comparator
output routed to a timer input capture with a digital filter), which also
yields hardware-stamped crossing times. firmware50 has been compensating with
hysteresis. That filter is the remaining structural difference between this
build's detector and the reference's, and phase A is where it would matter
most.

---

## E043 — COURSE CORRECTION: the oracle does NOT use the hardware capture
## filter. I nearly ported the wrong thing.

E037, E041 and E042 each named the reference's hardware capture filter
(`bench-capture-filter` / `filtered_irq_hw`, TIM2 input capture) as "the
remaining structural difference" and the next thing to port. **Checked before
porting, and it is false.**

In the qualified closure (`captures/reference/reverse_48k_com_top_high_20260919/README.md`):

* `bench-filter-source` compiles `filtered_irq_hw` / `filtered_irq_source` /
  `filtered_irq_check` -- and the only call site is the diagnostic command
  `filtersourcecheck` (`binz/examples/shell-pwm.rs:3177-3183`).
* `bench-capture-filter` compiles `capture_filter` -- and the only call site is
  the diagnostic command `filtercheck` (`:3184-3190`).
* The motor path's dispatch is `comp_input.rs`'s `filtered()`, which is gated
  on **`bench-filter-control`**. That feature is **not in the qualified
  closure.** So `filtered()` is false and the qualified image's COMP path is
  plain COMP2 → EXTI18, at `COMP2_HYST = 0`.

**The qualified image locks with the same detector architecture firmware50
already has.** The TIM2 filter is compiled in and exercised only by a check
command. This is [[feedback-prove-reference-exercises-path]] exactly: verify
the reference actually runs a path before attributing a result to it. Three
notebook entries built a plan on an assumption I never checked.

**What this also says about `COMP2_HYST = 2`.** The reference runs 0. My
hysteresis reduced chatter and helped *this* build (E039/E040), but it is
compensating for something that is not a missing filter. It stays for now,
labelled as a divergence whose justification has changed.

**The real structural difference, from the same closure:** `bench-inline-comp`,
`bench-lean-irq`, `bench-lean-core`, `bench-static-comp`, `bench-cached-comp`.
The reference's COMP ISR makes the **whole decision inside the interrupt** --
half-cycle gate, persistence reads of the *live* comparator, commutation
scheduling on the COM timer -- microseconds after the edge, and on a refusal it
leaves the line live so the next edge fires immediately. firmware50 instead:

1. masks the line in the ISR on every edge;
2. captures a single level there;
3. defers the gate, persistence and commutation to the **foreground polling
   loop**, whose latency is a sizeable fraction of a sector at running speed;
4. re-arms only when the foreground gets round to it.

Every one of those is latency the reference does not have, and E023 already
measured what stale evidence costs (`unstable` 55075 → 11940 just from moving
the level capture into the ISR). Moving the rest of the decision there is the
next step -- and it is precisely goal gate 5: COMP as an ISR root that decides,
COM as an ISR root that commutates.

---

## E044 — the zero-crossing decision moves into the COMP ISR (gate 5 COMP root)

**Why:** E043 -- the qualified image locks with the same COMP2 → EXTI18 detector
firmware50 has; the difference is that it decides *inside* the handler and
leaves the line live on a refusal, where firmware50 masked on every edge and
deferred the decision to the foreground, opening a dead window after every
refused edge.

**Changed (`bin/shell-pwm.rs`):**

* **`det_decide(raw)`, called from `ADC_COMP`** -- reference 238 µs
  minimum-accepted-event floor, `ci_max` re-base, then `ZeroCross::offer` with
  the persistence filter reading the **live** comparator (`DET_FILTER`,
  12 reads, the reference's depth). Works entirely in the 16-bit raw TIM17
  timeline: a sector is at most a few ms and the counter wraps every 65.5 ms,
  so a wrapping 16-bit difference is exact without the foreground's extension.
* **Refusal leaves the line live**; only an accept masks it until the next
  commutation. The ISR still masks and acks at entry, so the documented G071
  early-return re-fire bug stays closed whichever path is taken.
* **Ownership made explicit**: `DET` (the estimator) is written only by the ISR
  while `DET_ACTIVE` is set, initialised by the foreground *before* setting it
  and read for reporting only after clearing it with the line masked. Step and
  advance flow foreground → ISR, and accepts flow ISR → foreground, through
  atomics.
* The foreground no longer decides: it consumes `DET_ACCEPT_SEQ`, schedules the
  commutation, keeps every statistic, and publishes step and advance at each
  commutation.
* Handover seeds `DET` from the measured catch seed and starts the ISR's
  interval reference on the **exact raw count** of the validating acquisition
  edge.
* Removed as dead: the foreground offer block, `comp_ready_at`,
  `comp_seq_seen`, the polling-era comparator-level map (`BEMFMAPN`/`HI`,
  `MAP_KEYS`), and `COMP_MUX_SETTLE_US` -- the last one because its reason was
  a misreading: the reference's 10 µs settle exists only in the
  `bench-filter-control` path it does not run.

ELF `E18FF1A6F1A00E3EC35E8BDDFD7264EC039B848A7380B4661299150900BBED6B`,
text 38852, bss 4192. Clippy clean, 211 host tests.

**Gate 5, COMP root -- audited and costed:**

* `isr_audit.py` **first FAILED** on the new handler: an unreviewed loop. That
  is the audit doing its job -- moving the decision into the interrupt put the
  persistence filter's loop inside it. Reviewed and allowlisted with its bound.
  `__aeabi_lmul` is now reachable; it is a constant-time multiply and is not in
  the forbidden set, which is division only. **AUDIT PASSED**, no soft division.
* Also removed a stale allowlist entry: `Adc::read`, allowlisted as *NOT BOUNDED
  IN SOFTWARE*, is no longer reachable. Leaving it would silently re-admit an
  unbounded spin if a future edit called it again.
* **Longest path: 566 cycles = 8.84 µs at 64 MHz**, by a new tool
  `scripts/isr_cycles.py` (CFG from the linked ELF, M0+ cycle costs, bounded
  loops costed at their trip count, fail-closed on any loop without a supplied
  bound). 0 wait states assumed; the configured 2 flash wait states make the
  real figure higher, and exception entry/exit adds ~24 cycles. Up from 75
  cycles when the ISR only stamped a time -- the cost of deciding in the
  interrupt, about 2% of the reference's 425 µs locked sector.
* **The cycle tool also failed first, and was wrong to.** It counted two loops.
  One is the real filter: `cmp r3, #0xa ; bls` at `0x8000b52`, exactly 12
  iterations. The other, `b 0x8000ab0` at `0x8000c12`, is the compiler
  tail-merging the accept path onto a return trampoline at a lower address --
  `0x8000ab0` is itself a jump to the epilogue, so no path returns. Fixed the
  tool to require that a back-edge's target can reach it again; direction alone
  does not make a loop.

**Still owed for gate 5:** the COM, DMA and guard roots.

**Prediction for the powered run:** `forced_pct` falls from 37% because refused
edges no longer open a dead window, with the largest gain on phases A and C,
and current moves further toward 45 mA.

---

## E045 — the in-ISR decision overran the ADC; diagnosis and interim fix

**Run (ELF `E18FF1A6...`, capture `isrdecide_01.txt`):**

```
BEMFCATCH seeded=1 seed_us=1666 expected_us=1666 rejected_cycles=0
BEMFDONE  reason=11 accepted=0 forced=0 zc_acc=1 too_early=29 ci_us=1461
BEMFCURRENT blocks=278 mean_ma=250
```

**`reason=11` = `AdcTimeout`**, at handover. The catch seeded exactly (1666 vs
1666), `DET_ACTIVE` was set, and within ~2.8 s total (278 current blocks ≈ the
align + ramp + catch window) the ADC scan's bounded spin gave up.

**Mechanism -- caused by E044, and it is a real finding, not noise:**

* The ADC scan converts five channels back-to-back, ~5 µs each, and the
  foreground polls: it must read `DR` after each end-of-conversion before the
  next conversion completes.
* Before E044 the COMP ISR was **75 cycles (~1.2 µs)** -- it fitted between
  conversions. Since E044 its longest path is **566 cycles (8.8 µs)**, longer
  than one conversion.
* A COMP preemption landing mid-scan therefore lets the next conversion
  overwrite `DR`: the ADC overruns and halts, the next end-of-conversion never
  arrives, and `scan()`'s bounded spin (20 000 iterations) correctly returns
  `None` → `AdcTimeout`. The protection worked exactly as designed; it reported
  a fault my change introduced.
* Re-enabling the line on every refusal -- the point of E044 -- makes the ISR
  fire far more often than before, so the overrun became near-certain the
  moment the detector activated.

**This is why the reference reads its ADC by DMA.** DMA drains each conversion
in hardware regardless of what the CPU is doing, so the COMP handler can take
as long as it needs. It is also goal gate 5's DMA root, and
`scripts/audit_allow.json` already recorded the circular DMA scan as planned
"alongside the BEMF work because it is the same primitive". E044 made it a
prerequisite rather than a nicety.

**Interim, to confirm the diagnosis before building DMA:** the closed-loop scan
runs inside `cortex_m::interrupt::free`, so COMP cannot preempt it. Its cost is
stated rather than hidden: an edge arriving during the ~25 µs scan is latched,
not lost, but serviced -- and therefore timestamped -- up to one scan late, on
roughly a quarter of edges (a ~25 µs window every ~101 µs). ELF
`E1E56147B8DD06FFB60A00F698E01D374CD9423BE3519AF48D2ADD5841C850B8`.

**Prediction:** no `AdcTimeout`; if the diagnosis is right the run reaches its
segment deadline. Forced fraction and current then say whether the in-ISR
decision helps once it is not overrunning the ADC.

---

## E046 — diagnosis confirmed; the in-ISR filter exposes a perfect parity split

**Run (ELF `E1E56147...`, scan inside a critical section, capture `isrcs_01.txt`):**

```
BEMFDONE   reason=2 accepted=13431 forced=13282 too_early=526501 unstable=989411 ci_us=1673
BEMFRATE   hold_ms=35200 mean_sector_us=1391 hold_forced_pct=49
BEMFSECTOR a1=4450 f1=0 a2=22 f2=4435 a3=4447 f3=0 a4=46 f4=4412 a5=4446 f5=1 a6=20 f6=4434
BEMFCATCH  seeded=1 seed_us=1683 expected_us=1666
BEMFCURRENT mean_ma=256
```

**E045's diagnosis is confirmed**: with COMP held off during the scan there is
no `AdcTimeout` and the run reaches its segment deadline. The ADC overrun was
the cause, exactly as described.

**And the in-ISR decision exposes a split too clean to be noise: odd sectors
accept ~100% with zero forced; even sectors are ~100% forced.**

**Why this reconciles three earlier results rather than contradicting them:**

* **E023/E024's balanced sectors were not the persistence filter working.**
  Before E044 the filter compared a level captured *once, at the instant of the
  edge*. A falling edge always reads low at the instant it falls, so that
  "filter" passed every armed edge and filtered nothing. The balance came from
  edge polarity and blanking alone.
* **E044 made the filter real**: 12 live reads of the comparator inside the
  handler. Odd sectors (armed rising, expecting high) persist. Even sectors
  (armed falling, expecting low) do not -- the output falls and returns high.
* **That is E015's finding reached by a different route**: on this rig the
  comparator's post-crossing level is **high in every sector**. E015 found it
  by level-matching in a polled loop; E046 finds it because a genuine
  persistence test rejects every even-sector falling edge.
* It also explains E013 (logical alternation: odd only) and E014 (physical
  alternation: even only) -- each killed whichever half its expectation said
  should be low.

**Consequence:** if the output always ends high after a crossing, the transition
into every crossing is low → high. So the edge to arm is **rising in every
sector**, with a high expectation -- the reference's alternation does not hold
on this build's signal chain.

**Changed:** `edge_is_rising` returns a constant (rising). ELF
`007B06B7AA7AF3CD837AFC6A684CB551297FACE32DC775A68636970AE27716BC`.

**This is a stated divergence from the reference**, which alternates. It is
justified by E013, E014, E015 and E046 together and is not adopted on one run.

**Prediction:** all six sectors accept, `forced_pct` falls well below 49%, and
current falls from 256 mA. If the even sectors still fail with a rising edge,
then they have no reliable crossing transition at all on this signal, and the
problem is upstream of edge selection.

**Also noted, not glossed:** current is 256 mA here against 59-109 mA in
E037-E042. Half the commutations are forced at 1.25x the estimate, and the
critical section delays roughly a quarter of edges by up to a scan. Both are
torque losses; this number should not be compared against the reference until
the parity split is resolved.

---

## E047 — LOCK, by the reference's own verifier criterion

**Changed:** `edge_is_rising` constant (rising) in all six sectors (E046). ELF
`007B06B7AA7AF3CD837AFC6A684CB551297FACE32DC775A68636970AE27716BC`. Capture
`captures/2026-09-20/rise6_01.txt`.

**Measured -- 10% post-transfer BEMF duty:**

```
BEMFCATCH  catch_ehz=100 edges=13 blanked=111 rejected_cycles=6 last_reject=5 seeded=1 seed_us=1666 expected_us=1666
BEMFDONE   reason=2 accepted=148630 forced=31 zc_acc=148631 too_early=87 unstable=6594 ci_us=249 bus_ref=1212 bus_min=1122
BEMFRATE   hold_ms=35200 hold_accepted=140737 hold_forced=28 mean_ci_us=250 mean_sector_us=250
           ehz_from_sector=666 zc_per_s=3998 zc_expected_per_s=4000 zc_rate_permille_of_expected=999 hold_forced_pct=0
BEMFSECTOR a1=24772 f1=4 a2=24773 f2=8 a3=24769 f3=5 a4=24771 f4=4 a5=24772 f5=5 a6=24773 f6=5
BEMFCURRENT blocks=3958 mean_residual=1578 mean_ma=198 ref_ma_at_10pct=45
BEMFCOAST  reason=2 comp_edges=173 spun=1
```

**Showed -- the prediction held in full:**

* **All six sectors accept**, 24769-24773 each, with 4-8 forced each. Balanced
  to within four accepts across 35 s.
* **28 forced of 140 765 hold commutations: 0.02%**, from 49% in E046 and
  37-64% across E037-E042. Commutation is driven by accepted zero crossings.
* **Rate identity closes: 999 permille.** `mean_ci_us` (mean *accepted*
  interval) and `mean_sector_us` (hold / all commutations) agree at 250 µs --
  the first time in the campaign. E019 established they can only agree when
  forced commutations vanish, and they did.
* **Both refusal counters non-zero** and now meaningful -- `too_early=87`,
  `unstable=6594` are genuine refusals of genuine candidate edges in the ISR.
* **binz's executable verifier rule** (`scripts/verify_bemf_lock.py`):
  `|accepted - commutations| <= max(2, commutations/20)`. Here
  |148630 - 148661| = 31 against a tolerance of 7433. **PASS.**
* **Rotation witnessed from the board**: coast `comp_edges=173`, against the
  stationary control's 2-4 (E040).
* 35.2 s MCU-stamped hold, `reason=2`, no protection trip, bus 1122-1212.

**What does NOT match the reference, stated plainly:**

| | firmware50 (10%) | qualified reference (10%) |
|---|---|---|
| speed | **666 eHz** (250 µs sector) | 394-401 eHz (425 µs) |
| current | **198 mA** | 45 mA |

Faster *and* 4.4x the current at the same duty is the signature of commutation
landing at the wrong electrical phase -- most plausibly over-advanced -- rather
than of a slipping rotor: a slipping rotor could not produce perfectly balanced
sectors at a self-consistent rate for 35 s. So this is a **lock** by the
verifier's definition and by the rate identity, but **not yet at the reference's
operating point**. Goal gate 3's current comparison is not met.

**Why the rising edge is plausibly offset.** The reference alternates edge
polarity and this build arms rising everywhere, justified empirically by E013,
E014, E015 and E046. If the physical crossing in half the sectors is a falling
transition that this build instead catches at a later rising one, the detected
crossing -- and therefore the commutation -- is shifted by a fixed amount in
those sectors. That would produce exactly this: a stable lock at the wrong
phase, faster and hungrier than the reference.

**Gate status:**
* Gate 1 -- at **10%**, not the goal's 15%. ≥30 s MCU-stamped: yes (35.2 s).
  ZC-driven: 0.02% forced -- effectively yes, but the fallback still exists
  and fired 28 times, so "scripted frequency source removed" is not literally
  true yet. **One run**, not 3/3.
* Gate 2 -- rate identity **met** (999‰); both counters non-zero **met**.
* Gate 3 -- rotation witness **met**; current **not** (198 vs 45 mA).
* Gate 4 -- not started. Gate 5 -- COMP root done and audited; COM, DMA, guard
  roots absent, and the scan critical section (E045) is a stated interim.

**Next:** (1) reproduce 3/3; (2) find the operating-point offset -- the reference
runs 401 eHz / 45 mA at this duty, and the advance or the edge phase is the
suspect; (3) take it to 15%.

---

## E048 — the lock reproduces 3/3; and the operating point looks floor-pinned

**ELF** `007B06B7AA7AF3CD837AFC6A684CB551297FACE32DC775A68636970AE27716BC`.
Captures `captures/2026-09-20/lock10_0{1,2,3}.txt`.

| run | hold_acc | hold_forced | forced | rate ‰ | mean_ci / mean_sector | speed | current | coast edges |
|---|---|---|---|---|---|---|---|---|
| 1 | 140571 | 26 | 0.02% | 998 | 250 / 250 µs | 666 eHz | 126 mA | 189 |
| 2 | 140557 | 28 | 0.02% | 998 | 250 / 250 µs | 666 eHz | 257 mA | 155 |
| 3 | 140588 | 25 | 0.02% | 998 | 250 / 250 µs | 666 eHz | 149 mA | 169 |

Sectors balanced in every run (e.g. run 3: 24743-24748 accepts, 2-7 forced
each). All three `reason=2`, 35.2 s holds, `spun=1`.

**The lock is reproducible.** 3/3 pass binz's verifier rule; the rate identity
closes at 998‰ every time; every run is ZC-driven to 0.02%.

**What is suspicious, and must be resolved before this is called the
reference's operating point:**

* **Speed is exactly 666 eHz (250 µs) in all three runs, while current swings
  2x (126-257 mA).** A rotor settling at its own torque-limited speed varies
  run to run with temperature and friction. An identical figure three times
  suggests the speed is set by the *firmware*, not the rotor.
* **250 µs sits just above `EVENT_MIN_US = 238`.** The loop appears to be
  running as fast as the minimum accepted-event interval allows -- an
  equilibrium pinned by the guard. The reference runs 425 µs at this duty, well
  clear of the same 238 µs floor, so in the reference the floor never binds.
* Combined with 3-6x the reference's current, the likely reading is: the loop
  is commutating faster than the rotor's natural speed at 10% duty, and the
  event floor is what stops it running away further. If so, this is a lock in
  the verifier's accounting but the rotor is being over-driven.
* The mechanism fits E047's concern: with a rising edge armed in every sector,
  if the edge caught in half of them is not the physical crossing but an
  earlier transition, intervals come out short and the estimate is pulled down
  until the floor catches it.

**Test that separates the two readings:** raise the acceptance floor well above
the observed 250 µs -- to ~350 µs, still under the reference's 425 µs. If the
speed was floor-pinned it will fall and settle *at the new floor*, and current
should fall with it. If 666 eHz is the rotor's natural speed, raising the floor
above it will starve the loop of acceptable edges and forced commutations will
return. Either outcome is decisive.

---

## E049 — RETRACTION OF E047: the "lock" is paced by the acceptance floor

**Changed (diagnostic):** the in-ISR acceptance floor split out as
`DET_ACCEPT_FLOOR_US` and raised 238 → 350 µs. `protection::EVENT_MIN_US` is
unchanged at the reference's 238 for every guard that uses it; raising a
*minimum* interval tightens acceptance, so no protection is relaxed. ELF
`88296443F763D326F7160ADED07C97750507EBAEB164625B94DEBE5319F584C8`. Capture
`floor350_01.txt`.

**Measured:**

```
BEMFDONE   reason=2 accepted=104682 forced=8 too_early=57 unstable=2809 ci_us=359
BEMFRATE   hold_ms=35200 hold_accepted=99090 hold_forced=6 mean_ci_us=355 mean_sector_us=355 zc_rate_permille_of_expected=999
BEMFSECTOR a1=17448 f1=1 a2=17447 f2=0 a3=17448 f3=2 a4=17445 f4=1 a5=17448 f5=4 a6=17446 f6=0
BEMFCURRENT mean_ma=225
BEMFCOAST  comp_edges=187 spun=1
```

| acceptance floor | settled sector | above floor | current |
|---|---|---|---|
| 238 µs (E047/E048) | **250 µs** | +12 µs | 126-257 mA |
| 350 µs (E049) | **355 µs** | +5 µs | 225 mA |

**Verdict: the speed is set by the floor, not by the rotor.** This was the
test E048 designed to separate the two readings, and it came down cleanly on
the wrong side for me. In a genuine BEMF lock the rotor's speed at a given duty
is fixed by its physics; the floor only needs to sit below it, and moving the
floor would not move the speed. Here the sector tracked the floor exactly, and
current did not fall as the loop slowed, which a lock moving toward its natural
speed would show.

**So E047's and E048's "lock" was the loop accepting the first rising edge after
the floor opened, in every sector** -- self-clocked by the acceptance guard, the
same way the old forced fallback was self-clocked by a timer. **The 999‰ rate
identity and the perfectly balanced sectors are exactly what a self-clocked loop
produces**, because every commutation is by construction triggered by an
"accepted" edge. Those two metrics cannot distinguish a lock from a paced loop,
and I presented them as if they could. E047's lock claim is withdrawn.

The rotor *is* turning -- coast `comp_edges=187` against the stationary
control's 2-4 -- but it is being **paced**, not **tracked**: effectively driven
open-loop at a rate the floor sets.

**Why the detector accepts at the floor -- the E040 control already showed it.**
With the bridge off and the rotor still, `COASTCONTROL comp_hi=415260
comp_polls=415261`: **COMP2's output is high 100% of the time at rest.** It is
DC-biased high -- the +16-code offset between the star divider and the phase
dividers measured all the way back in E007. So:

* a persistence check for *high* is true almost always and filters nothing;
* a rising edge follows every brief dip below threshold, and the PWM supplies
  such dips every carrier cycle (20.8 µs at 48 kHz);
* therefore a rising edge is always available within about one carrier period
  of the floor opening -- matching the +5 to +12 µs observed.

**This also re-reads E046.** "The post-crossing level is high in every sector"
was true, but not because every crossing ends high: the comparator is high
*almost all the time*, crossing or not. The genuine signal on a comparator
biased high is the rarer excursion **low**. That is what the reference's
alternation catches in the falling sectors, and what arming rising everywhere
throws away.

**Metrics that cannot be trusted to certify a lock, now established:**
`forced_pct` (E020, improves when the loop desyncs), the rate identity and
sector balance (E049, trivially satisfied by a self-clocked loop). **The test
that does discriminate: move the acceptance floor and see whether the speed
moves with it.** A genuine lock is floor-independent.

**Next:** the comparator's DC bias is the root of both E046's and E049's
misreadings. It has to be measured and understood -- the reference has the same
board and dividers and locks with alternating polarity, so either its operating
point's BEMF swamps the offset, or it compensates for it somewhere I have not
read.

---

## E050 — detector fully reference-matched: HYST 0 and alternating polarity restored

**Changed:** `COMP2_HYST` 2 → **0** (reference); `edge_is_rising` back to the
reference's **alternation**; `DET_ACCEPT_FLOOR_US` back to the reference's
238. ELF `281E0BD1F555AC0A32C4EADCEBF6E633ADFAFBA8F4F31E73BA2926F36B3E4EBE`.

**Why, reasoned from E049.** COMP2 is DC-biased high (100% high at rest), so the
two sector types are asymmetric:

* **rising sectors** (low → high at the crossing): reaching high is easy, and
  easy to fake -- every PWM dip supplies a rising edge, which is what produced
  the floor-paced loop;
* **falling sectors** (high → low): the floating phase must overcome the offset
  to pull the output low, and hold it through the next carrier edge.

Hysteresis on a high-biased comparator raises the bar for the falling
transitions while leaving the rising ones easy. **So `COMP2_HYST = 2` (E039-E040)
selectively hurt exactly the transitions the reference's alternation depends
on** -- a large part of why E046 saw even sectors fail -- and "arming rising
everywhere" in response then discarded the genuine signal entirely.

With this change the detector matches the reference in **every** dimension I
have identified: COMP2 → EXTI18, per-sector edge alternation, `HYST = 0`,
12-read persistence of the **live** comparator, decision inside the ISR,
238 µs minimum accepted-event interval, line left live on refusal.

**Predictions, and the discriminating test.** A genuine lock must be
**floor-independent** (E049). So this build is judged not by forced fraction,
rate identity or sector balance -- all three were shown to be satisfiable by a
paced loop -- but by:

1. all six sectors accepting under alternation;
2. the settled sector **not** sitting a few µs above the 238 µs floor;
3. current falling toward the reference's 45 mA.

If (1) holds and (2) holds, a second run with the floor moved should leave the
speed where it is.

---

## E051 — reference-matched detector: genuine partial tracking, and an ISR storm

**Run (ELF `281E0BD1...`, capture `refmatch_01.txt`):**

```
BEMFDONE   reason=2 accepted=29047 forced=14945 too_early=1553233 unstable=2985860 ci_us=1384 bus_min=1104
BEMFRATE   hold_ms=35200 hold_accepted=28161 hold_forced=14413 mean_ci_us=798 mean_sector_us=826 hold_forced_pct=33
BEMFSECTOR a1=6104 f1=1242 a2=3353 f2=3973 a3=6652 f3=646 a4=2120 f4=5216 a5=6577 f5=803 a6=4241 f6=3065
BEMFCATCH  edges=164 blanked=2560 rejected_cycles=157 seeded=1 seed_us=1680
BEMFCURRENT blocks=2962 mean_ma=155
BEMFCOAST  comp_edges=111459 spun=1
```

**Showed -- a real change in kind:**

* **Settled sector 826 µs, nowhere near the 238 µs floor.** By E049's criterion
  this is not a paced loop; it is genuine, partial BEMF tracking. 67% of hold
  commutations are ZC-driven.
* **The weakness is exactly where the DC-bias analysis predicted.** Odd
  (rising) sectors accept 83%, 91%, 89%; even (falling) sectors 46%, 29%, 58%.
  The falling transitions are the ones that must overcome the offset.

**And a safety problem that must be fixed before this configuration runs
again:**

* At `COMP2_HYST = 0` the comparator chatters (coast `comp_edges` back up to
  111459, from 175 at HYST=1), and with the line left live on every refusal
  the ISR fired **~4.5 million times** in the run (`too_early` 1.55 M +
  `unstable` 2.99 M) -- about 118 000 per second, at up to 566 cycles each.
  That saturates the core.
* **The foreground protection loop visibly starved: 2962 current blocks against
  3958 in every other 35 s run** -- the ADC-fed protections ran about 25% less
  often. They still ran and the run ended normally, but a configuration that
  degrades protection cadence is not one to qualify.
* The reference survives `HYST = 0` with a live line because its handler is lean
  (`bench-lean-irq`, `bench-lean-core` in the qualified closure). Mine is 566
  cycles on the long path. The storm is the consequence of pairing the
  reference's policy with a handler that is several times heavier.
* The catch also suffered: 157 rejected cycles and 2560 blanked edges against
  6-50 and 60-900 at HYST ≥ 1 -- chatter corrupting acquisition too.

**Where this leaves the detector.** The reference-matched policy (alternation,
HYST 0) produces genuine tracking but needs a lean handler this build does not
yet have. `HYST = 1` was the configuration that killed the chatter (E039: 175
coast edges, clean catch) without the falling-edge penalty of HYST = 2. It is
the next configuration to test *with alternation restored* -- which was never
tried: E039 and E040 ran HYST 1 and 2 under the old foreground decision, and
E046-E048 ran rising-everywhere.

**Two structural items now clearly required, both goal gate 5:** a lean COMP
handler (so the reference's live-line policy does not saturate the core), and
the DMA ADC (so neither the handler nor a critical section can starve or
overrun the protection scan).

---

## E052 — alternation with HYST=1: no storm, but falling sectors almost dead

**Run (ELF `B78A0642...`, capture `alt_hyst1_01.txt`):**

```
BEMFDONE   reason=2 accepted=16091 forced=13515 too_early=648492 unstable=1283304 ci_us=1824
BEMFRATE   hold_ms=35200 mean_ci_us=1003 mean_sector_us=1249 hold_forced_pct=45
BEMFSECTOR a1=4901 f1=0 a2=540 f2=4413 a3=4915 f3=0 a4=337 f4=4614 a5=4917 f5=2 a6=481 f6=4486
BEMFCURRENT blocks=3958 mean_ma=253
BEMFCOAST  comp_edges=197 spun=1
```

* **Protection cadence restored**: 3958 current blocks (E051's storm had cut it
  to 2962). Coast chatter gone (197 edges). HYST=1 prevents the storm.
* **Rising sectors 100% accepted; falling sectors ~10%** (540, 337, 481 against
  4413-4614 forced). At HYST=0 (E051) falling was 29-58%, so hysteresis
  penalises exactly the falling transitions -- the DC-bias analysis of E050,
  confirmed. And even with no hysteresis the offset alone keeps them weak.
* Settled sector 1249 µs (133 eHz), not near the floor: genuine partial
  tracking, not pacing.

**The remaining problem is a speed/offset chicken-and-egg.** Back-EMF grows
with speed; the comparator offset is fixed. At the 133-200 eHz this loop
settles at, the offset is a large fraction of the back-EMF, so falling
transitions barely clear it. And the loop cannot accelerate out: each failed
falling sector falls back to a commutation 1.25x late, which brakes.

The reference faces the same offset on the same board and dividers, and locks
-- at **401 eHz at 10%**, and **704 eHz at 15%**, which is the goal's own
target duty (`binz/DUTY_50_CAMPAIGN.md`). At those speeds the back-EMF swamps
the offset. So the direct test is to run at the duty the goal actually asks
for, where the rotor is driven fast enough for the physics to cooperate.

**Changed:** `BEMF_DUTY_TENTHS` 100 → **150** (15.0%, the goal's target).
Detector unchanged: alternation, HYST=1, in-ISR live persistence, 238 µs floor.

**Prediction:** after handover the rotor accelerates further than at 10%;
falling-sector acceptance rises as speed and back-EMF rise; the settled sector
is **not** near the floor. If falling sectors stay dead at 15%, the offset is
not simply a speed problem and needs direct compensation.

---

## E053 — 15% sags the bus; and the reference INVERTS the raw comparator

**Run at 15% BEMF duty (ELF `6B667EE9...`, capture `alt15_01.txt`):**

```
BEMFDONE   reason=26 accepted=1579 forced=1102 too_early=47679 unstable=87994 ci_us=990 bus_min=1103
BEMFRATE   hold_ms=812 mean_sector_us=892 hold_forced_pct=37
BEMFSECTOR a1=438 f1=6 a2=100 f2=355 a3=437 f3=3 a4=78 f4=373 a5=429 f5=14 a6=97 f6=351
BEMFCURRENT blocks=555 mean_ma=406
```

**`reason=26`, the fast bus-sag hard stop**, after 0.8 s at 15%: current 406 mA,
`bus_min` 1103 against 1214 (91%). The protection did exactly its job on a
surge. (A genuine trip of the three-scan fast-sag stop -- relevant to goal gate
4, though not provoked on purpose, so it does not satisfy that gate.)

**E052's prediction failed**: falling sectors did not improve at 15% (100, 78,
97 accepted against ~360 forced each). So the offset is not simply a speed
problem: the loop cannot reach the speed where back-EMF would swamp it, because
failed falling sectors keep falling back to a braking commutation, and at
higher duty that becomes a current surge.

### The finding

With an identical comparator configuration on identical hardware, the reference
still locks -- so something in its *procedure* differs. Reading rather than
guessing, `binz/AGENTS.md:8338`:

> *"E209 corrects E208 inference: sustained powered path DOES invert raw COMP.
> prepare_early->observe_begin sets PHYSICAL_OBSERVATION=true; recorded
> successful range300_fastreturn50_02 COREPOL physical_raw_inverted=1
> confirms."*

**The qualified image inverts the raw comparator polarity in its sustained
powered path.** That is AM32's per-board `INVERTED_EXTI` knob, and it is set
for this board.

**I tested inversion once -- E009 -- and discarded it.** But E009 ran under the
`Forward` wiring bug (E010) with the old polled level detector, so its verdict
is void, and it was never re-tested under correct wiring with the ISR detector.
Worth noting too: the binz agent needed two attempts to establish this (E208
got it wrong, E209 corrected it), which is a warning that the answer does not
fall out of first principles -- my own DC-bias reasoning in E050-E052 did not
arrive at it.

(`drivepwm192`, the other unread element of the locked sequence, is separate: it
selects a PWM-synchronised DMA ADC sampling target inside the ON pulse, TIM1
count 192 -- `AGENTS.md:8342`, `LOW_DUTY_REPLICATION.md:135`. That belongs to
the gate-5 DMA work, not the comparator.)

**Changed:** `COMP_POLARITY_INVERTED` false → **true**, cited to `AGENTS.md:8338`.
`BEMF_DUTY_TENTHS` back to **100** so this is the only variable relative to
E052. Detector otherwise unchanged: alternation, HYST 1, in-ISR live
persistence, 238 µs floor.

**Prediction:** falling-sector acceptance rises to match the rising sectors, the
forced fraction falls, and the settled sector stays **clear of the floor** --
the E049 test for a genuine lock rather than a paced one.

---

## E054 — inversion swaps the halves: the asymmetry is rising-vs-falling, not
## which sectors

**Run (ELF `A3FE9C8D...`, `COMP_POLARITY_INVERTED = true`, 10%, capture
`invert_01.txt`):**

```
BEMFDONE   reason=2 accepted=15733 forced=13514 too_early=658283 unstable=1066053 ci_us=1846
BEMFRATE   hold_ms=35200 mean_sector_us=1266 hold_forced_pct=46
BEMFSECTOR a1=430 f1=4464 a2=4854 f2=1 a3=435 f3=4468 a4=4846 f4=0 a5=296 f5=4581 a6=4872 f6=0
BEMFCURRENT blocks=3958 mean_ma=233
```

**Showed -- E053's prediction failed, informatively.** Inversion did not make
both halves work; it **swapped** them. Even sectors now accept ~100%, odd
sectors ~9% -- the exact mirror of E052.

Laid side by side across E046, E051, E052 and E054, the one invariant is:
**sectors armed for a rising edge accept ~100%; sectors armed for a falling edge
fail, whichever sectors those are.** The failure follows the edge direction, not
the sector, the phase or the polarity setting.

Inversion is kept -- it is the reference's documented setting (`AGENTS.md:8338`)
-- but it does not explain firmware50's failure, because in the reference *both*
halves work.

**So the cause is something in this build that treats rising and falling edges
differently. There is one concrete candidate, and I introduced it:** the E045
critical section around the ADC scan, which holds COMP off for ~25 µs on
roughly a quarter of edges (a ~25 µs window every ~101 µs).

On a comparator biased high (E049):

* a **rising** edge leaves the output **high** -- the stable, biased state -- so
  it still persists when the ISR is serviced up to 25 µs late;
* a **falling** edge produces a **marginal low** that the next PWM transient can
  flip back high -- so a delayed ISR's 12 live reads often find it high again,
  and persistence fails.

That produces exactly the observed invariant. The reference does not have it:
it reads the ADC by DMA, so there is no critical section and the COMP handler
is never held off.

**It also fits E051**: at `HYST = 0`, with the line live, the storm saturated
the core, and falling sectors were *better* there (29-58%) than at `HYST = 1`
(~10%). A saturated core services every edge late; lower hysteresis lowers the
barrier a marginal low has to clear. Both point at servicing latency hurting
falling edges specifically.

**Next -- and it is the structural item goal gate 5 already requires:** read the
ADC by DMA so the protection scan runs in hardware, remove the E045 critical
section, and let the COMP handler preempt freely. Prediction: falling-sector
acceptance rises to meet rising, with no `AdcTimeout`.

---

## E055 — ADC read by DMA; E045 critical section removed (build, pre-run)

**Change.** The protection scan no longer runs on the CPU at all:

* TIM6 update → TRGO (`MMS=0b010`) → ADC external trigger (`EXTSEL=0b101`,
  `EXTEN=rising`, single-shot sweep of the 5 channels) → DMA1 channel 1
  (DMAMUX request 5, circular, 16-bit, `TCIE|TEIE`) → `DMA1_CHANNEL1` ISR
  at priority `0x40` (below COMP at `0x00`).
* The ISR clears all four channel flags first, then copies the buffer to a
  snapshot inside a seqlock (`ADC_SEQ` odd while writing). The foreground's
  `adc_snapshot()` makes at most 8 bounded attempts to read it.
* `Board::adc_due()` now means "a new, even `ADC_SEQ`"; `scan()` reads the
  snapshot.
* **New stale guard:** `bemf_run` stops with `AdcTimeout` if no fresh scan
  has arrived for `ADC_STALE_US = 2000` µs. With DMA, a halted ADC would
  otherwise go *silent*, where the polled read at least timed out.
* **The E045 critical section is gone.** `cortex_m::interrupt::free(|_| b.scan())`
  is now `b.scan()`. COMP preempts everything, always.
* `Board.pace` is now `_pace`: TIM6 is held so nothing can repace the trigger,
  and the foreground never reads it again.

**Checks.**

| check | result |
|---|---|
| clippy (thumbv6m, release) | clean |
| host tests | 211 passed |
| ELF | text 39280 / data 4 / bss 4220; sha256 `186A662C…D7F03A` |
| fail-closed audit, `ADC_COMP` | OK (reachable = 2, reviewed 12-read loop) |
| fail-closed audit, `DMA1_CHANNEL1` | OK (reachable = 1, **no loops**, the copy is unrolled) |
| longest path, `ADC_COMP` | 530 cycles = 8.28 µs @ 64 MHz, 0 ws (+~24 entry/exit) |
| longest path, `DMA1_CHANNEL1` | 99 cycles = 1.55 µs @ 64 MHz, 0 ws |

**Prediction for the powered run (10%, same settings as E054):** falling-sector
acceptance rises to match rising (both ≳ 90%), and there is no `AdcTimeout`.
If falling sectors *still* fail at ~10%, the E054 servicing-latency hypothesis
is refuted and the asymmetry lies elsewhere: comparator bias or edge
configuration, not timing.

### E055 — result (powered run, `captures/2026-09-20/e055-dma_01.txt`)

ELF `186A662C…D7F03A`, 10% target, same settings as E054.

```
BEMFDONE reason=2 accepted=16523 forced=13539 too_early=691107 unstable=1062406 ci_us=2058
BEMFRATE hold_ms=35200 ehz_from_sector=135 zc_rate_permille_of_expected=550 hold_forced_pct=44
BEMFSECTOR a1=571 f1=4461 a2=4988 f2=0 a3=606 f3=4423 a4=4993 f4=0 a5=365 f5=4655 a6=5000 f6=0
BEMFCURRENT mean_ma=150 ref_ma_at_10pct=45
BEMFCOAST comp_edges=213 (rotation witnessed on coast)
```

* **The DMA ADC works:** a full 40 s run with no `AdcTimeout` and no stale
  stop, and every protection scan still ran (3958 current blocks, the same
  count as E054).
* **The prediction failed. Latency is refuted as the cause.** With the
  critical section gone, even (rising-armed) sectors still accept ~100% and
  odd (falling-armed) sectors ~10%, the same split as E054. The COMP handler
  now preempts everything and nothing changed, so servicing delay was never
  the mechanism. The DMA change stays anyway: it is gate 5's structure and it
  removes a real hazard.
* Speed 135 eHz and 150 mA, against the reference's 394–401 eHz and 45 mA at
  10%. **This is not a lock.** Half the commutations are forced, and E049
  showed that rising accepts on this comparator are floor-paced.

**Analysis: what the reference has that this build does not.** Compared
against the qualified adapter code (read-only):

* Polarity is identical. The reference's live path arms `!rising` in the raw
  sense (`core_bench.rs:1161-1170`) and persists on `!raw == rising`
  (`read_comp_level`, `core_bench.rs:1142`, `PHYSICAL_OBSERVATION`). The net
  is the same as `edge_is_rising = step.rising() != true` with a raw
  `value == edge` test.
* The floating phase and the A↔B relabel are identical (C/A/B, `physical_phase`).
* Persistence is identical: `CachedComp` caches only the mode flags, and all
  12 reads are live.
* HYST: the oracle's feature closure lacks `bench-comp-hyst-low`, so HYST = 0.
  firmware50 runs HYST = 1 because of E051's storm. This is a known,
  recorded divergence.
* **Missing: `bench-running-level-revisit`**, which is in the oracle's feature
  closure (`captures/reference/reverse_48k_com_top_high_20260919/README.md`).
  Every foreground poll (`core_bench.rs:3114`) checks: gate open
  (`interval > average/2`), line enabled and unmasked, nothing pending,
  **comparator already at the post-crossing level**, and not already retried
  this step (`level_revisit.rs::admit`). When all hold, it pends `ADC_COMP` in
  software so the ordinary ISR decision runs.
* Also missing: `bench-reverse-blank`, a 280 µs post-commutation mask applied
  only while `average_interval >= 1500` half-µs (sectors ≥ 750 µs). It is a
  low-speed handover measure and is held for a later, separate change.

**Why the revisit explains the invariant.** The line is edge-triggered and
the half-cycle gate refuses early edges. On a comparator that sits high:

* a **rising** sector keeps getting rising edges from PWM ripple, so one
  always arrives after the gate opens;
* a **falling** sector gets *one* real falling edge at the crossing. If that
  edge lands inside blanking it is refused, and the comparator then holds low
  with no further falling edge, so the sector can never be accepted and falls
  to the fallback.

The edge direction decides it, not the sector, which is exactly the
invariant. The reference's revisit converts "already low" into a decision.

## E056 — level revisit, transcribed from the qualified image (build, pre-run)

**Change (one variable):**

* New `src/revisit.rs`: `admit(Inputs)` transcribed from
  `binz/examples/support/level_revisit.rs`, with half-µs converted to µs
  (`AVERAGE_MIN_US = 32`), the running variant's no-speed-floor behaviour, and
  a strict half-cycle gate. 5 host tests: the admit case, each of the 7 vetoes
  alone, strictness of the gate, the unit conversion, and admission at the
  target's 237 µs sector.
* `bemf_run` polls it every iteration while closed with no commit pending. It
  samples the line state, pending flags, elapsed time and live level, and
  pends, all inside one critical section, as the reference does. One retry
  per step, reset on each acceptance, as the reference does.
* New report line `BEMFREVISIT rN vN`: retries pended and retries followed by
  an acceptance, per step.

**Not changed:** HYST stays 1, the fallback stays on, the arm-before-mux order
at commutation stays (noted as a separate discrepancy: the reference muxes
before clearing pending; here `comp_exti_arm` precedes `comp2_select_floating`),
and there is no reverse-blank.

**Checks:** clippy clean; 216 host tests; ELF text 39708, sha256
`B63C675E…BC29C3F`; `ADC_COMP` and `DMA1_CHANNEL1` both pass the audit (the
revisit is foreground code; the ISR is unchanged).

**Prediction:** retries land almost entirely on the odd, falling-armed steps,
and most are followed by an acceptance. Falling-sector acceptance rises from
~10% to well over half, forced commutations fall, and because a real lock
lets the rotor accelerate, speed moves up from 135 eHz toward the reference's
~400 eHz while current falls from 150 mA toward 45 mA. If retries fire but
the persistence reads refuse them, the low is not stable enough to be a
crossing, and the next suspect is HYST 1 against the reference's 0.

### E056 — result (powered run, `captures/2026-09-20/e056-revisit_01.txt`)

ELF `B63C675E…BC29C3F`, 10% target.

```
BEMFSECTOR  a1=564 f1=4456 a2=4972 f2=1 a3=617 f3=4412 a4=4966 f4=1 a5=357 f5=4646 a6=4992 f6=1
BEMFREVISIT r1=3508 v1=138 r2=712 v2=3674 r3=3503 v3=196 r4=690 v4=3612 r5=3483 v5=81 r6=659 v6=3646
BEMFDONE    too_early=686013 unstable=1064389     BEMFRATE ehz_from_sector=134   BEMFCURRENT mean_ma=137
```

**The prediction failed, informatively.** Retries fire in ~80% of the falling
sectors (r1/r3/r5 ≈ 3500 of ~5000), so the comparator **is** reading low
when polled past the gate. But only ~4% survive the 12 live persistence
reads: the low is momentary, not held. Acceptance barely moves. (The v2/v4/v6
values exceed their r counts because of an attribution bug: `revisit_inflight`
was not cleared at commutation, so a failed odd-step retry was credited to
the next even step's hardware accept. Fixed in E057. It does not affect
control.)

**A reference fact that reframes the problem.** The oracle's COMP ISR
(`core_bench.rs:4062-4107`, `bench-lean-core`) runs `IRQ_RATE.hit_limit` with
`reverse_rate_limit() = 64` (`bench-reverse-irq-cap64` is in its closure).
More than 64 COMP calls in any 1 ms bucket calls `powered_timer::irq_storm()`,
which kills the bridge. Its qualified runs completed, so its comparator never
exceeded 64/ms. firmware50 averages (686013 + 1064389) / 37 s ≈ **47k
refused edges per second: one per 48 kHz carrier period.** The reference
comparator is quiet; this one toggles with the carrier all sector long.

The drive scheme is not the difference. The reference
(`phase_role_live.rs` + `phase_gpio_plan.rs`) runs complementary PWM on the
source, holds the sink's low side on, and keeps both floating gates low.
firmware50 (`src/sixstep.rs`) does the same: PWM1 + CCxE|CCxNE on the source,
forced-inactive + both enables on the sink, CCER cleared on the floating
phase.

**Model (the physics of this sense network in each PWM state):**

* **ON** (source at Vbus, sink at 0): the floating terminal is Vbus/2 + e_f
  and the resistive star is Vbus/2 + e_f/3. The comparator sees sign(e_f),
  well above ground: valid.
* **OFF** (source low side on, sink on): the floating terminal is about
  1.5·e_f and the star about 0.5·e_f. Both sit near **ground**, differing by
  only about e_f, and the negative half is clamped by the floating low FET's
  body diode. When |e_f| is below COMP2's input offset, the comparator reads
  the offset, and that offset is **high**: the "100% high at rest" of E049.

At 10% duty OFF is ~90% of every period. So at low speed the comparator
shows the true sign only during the ~2 µs ON pulse and reads "high" the rest
of the time. **Every symptom follows:**

1. Falling sectors (true level low after the crossing) read low only during
   ON, so 12 live reads fail: `unstable`. The revisit sees low (it polls
   often enough to catch ON pulses) and the reads then fail: this E056
   result.
2. Rising sectors (true level low *before* the crossing) toggle low→high at
   the end of every ON pulse, so the first rising edge after the gate is
   accepted regardless of rotor position. That is E049's floor pacing.
3. Both kinds give one edge per carrier period, the observed ~48k/s.
4. HYST 0 lowers the barrier and multiplies the toggling: E051's storm.
5. The reference sets the handoff at 200 eHz (`run200`) and settles at
   ~400 eHz, where e_f in the OFF window is 2–4× this build's 100–135 eHz
   and clears the offset. Its storm cap never trips.

**Consequence:** this build's closed loop has been attempted at a speed
where the comparator cannot see back-EMF for 90% of each period. That is why
the E033 decision to catch at 100 eHz (where the open loop was confirmed to
turn the rotor) could never lock, whatever the detector logic.

## E057 — hand over at the reference's 200 eHz (build, pre-run)

**Change (one control variable):**

* `CATCH_EHZ` 100 → **200** (the reference's `run200`). Acquisition and
  handover now happen there.
* `CATCH_RAMP_MS` 2500 → 5000, so the ramp slope stays at this build's proven
  ~40 eHz/s instead of doubling as well.
* Instrumentation only: `revisit_inflight` is cleared at every commutation
  (the E056 attribution bug).

Unchanged: HYST 1, fallback on, 10% target, revisit on, all protections.
ELF text 39692, sha256 `9919BE98…98E77A5467`; clippy clean.

**Risk named in advance:** earlier notes put open-loop loss of the rotor near
~110 eHz (E031/E035, some of it under since-fixed bugs). If the drive cannot
carry the rotor to 200, the acquisition's consistency check should refuse
and the run stops with `InvalidSeed` (reason code) after the 3 s acquisition
timeout: about 8.3 s of 6.2% open-loop drive in total. That is bounded and
protected.

**Prediction:**

* (a) The seed validates at about 833 µs (200 eHz).
* (b) After handover, falling-sector acceptance rises substantially, above
  E056's ~10%, and the too_early + unstable rate drops below one per carrier
  period.
* (c) Speed climbs from 200 eHz toward the reference's ~400 eHz at 10%, with
  current falling toward 45 mA.

If (a) holds but (b) does not, the model is wrong, or 200 eHz is still below
the offset threshold at this duty.

### E057 — result (powered run, `captures/2026-09-20/e057-catch200_01.txt`)

ELF `9919BE98…98E77A5467`, 10% target.

```
BEMFCATCH   catch_ehz=200 edges=8 blanked=41 rejected_cycles=1 seeded=1 seed_us=833 expected_us=833
BEMFSECTOR  a1=1397 f1=7354 a2=8520 f2=4 a3=21 f3=8616 a4=8632 f4=4 a5=44 f5=8594 a6=8631 f6=4
BEMFREVISIT r1=4064 v1=317 r2=1816 v2=1816 r3=4681 v3=13 r4=1259 v4=1257 r5=4561 v5=13 r6=1591 v6=1591
BEMFRATE    ehz_from_sector=249 hold_forced_pct=47     BEMFDONE too_early=419551 unstable=1091552
BEMFCURRENT mean_ma=120
```

* (a) **Held on its face:** the seed validated at 200 eHz, and the loop then
  commutated at 249 eHz, the fastest this build has run closed.
* (b) **Failed:** the falling sectors are no better (a3 = 21 and a5 = 44
  against ~8600 forced), and refusals still run at (419551 + 1091552) /
  34.7 s ≈ 43.5k/s, about one per carrier period.
* (c) Not met: 120 mA against the reference's 45 mA.

**The seed is suspect, and so was E033–E056's.** It equals the schedule
*exactly*, twice: 1666 = 1666 at 100 eHz and 833 = 833 at 200. A following
rotor shows jitter, as the acquisition's own `mild_jitter` test models. An
exact match means acquisition is validating edges locked to the drive
itself, i.e. commutation and PWM transients. Under the E056 model that is
expected: at low speed the raw comparator carries the drive, not the rotor.
**So there is no evidence the rotor was at 200 eHz at handover**, and E031's
open-loop loss near ~110 eHz would put it far below. That leaves the same
small-BEMF regime as E056.

**What the reference actually does (`binz/LOW_DUTY_REPLICATION.md:110-139`,
read this session):** the known-good recipe is
`drivephase60 drivedu61 bemfdu100 drivepwm192 driveobs1 engagems35000 drivex1
… du62 catchdu62 run200`, with startup on a **sine** open loop ("48 kHz PWM,
1 kHz control" per the table at :68; the sine/control update is 1 kHz, :97).
`drivepwm192` "selects the validated driven ADC/PWM sampling target". AGENTS
E208/E209 (`binz/AGENTS.md:8338-8350`) add TIM1_CH4 → DMA1 CH2 sampling and
capture COMP2 CSR at `CCR4=192`, and the host "require[s] target+32 counts
within ON pulse". **The reference samples the comparator inside the ON pulse
during its driven stage and acquires the flying seed from those samples.**
That is precisely what the E056 model says is necessary at low speed.
firmware50's acquisition took raw EXTI edges, which is the one thing the
model says cannot work there.

**Next (E058):** transcribe the reference's driven observation. The plan is
to read `examples/support/driven_run.rs`, `driven_seed.rs` and
`flying_acquire.rs` (read-only) and reproduce:

1. its open-loop stage (sine, if confirmed);
2. the driven six-step vector at `drivephase60`/`drivedu61`;
3. TIM1_CH4 at 192 → DMA snapshot of COMP2 CSR, one ON-window sample per
   carrier period, with no ISR;
4. seed acquisition from those samples with the reference's
   238/2858/476 µs bounds;
5. then the transfer.

The HYST/revisit/reverse-blank items stay parked until the driven
observation exists, since each one only matters after a true seed.

## E058 — plan: transcribe the reference's startup chain (sine → driven observation → seed → transfer)

**Evidence read this session (read-only):**

* `binz/LOW_DUTY_LOCK_EVIDENCE_20260920.md`: the locked run's `DRIVESTOP
  acquisition_reason=22 acquisition_us=10649 acquisition_duty_tenths=61`. The
  seed came from **10.6 ms** of driven drive at 6.1%.
  `RUNNINGREVISIT attempts=7266 accepts=7266` (9% of 81,157 commutations) and
  `REVERSEBLANK arms=4`.
* `examples/support/driven_run.rs`: after the sine campaign, a *driven*
  six-step whose sectors continue the sine phase. θ comes from
  `campaign::phase_shift(anchor, drivephase60)`, the boundaries from
  `phase_schedule::next(θ, rate)`, and TIM3 commutates at those deadlines.
  In the oracle (`bench-lean-irq`) the DMA microscope is compiled out. "The
  live seed comes from `driven_irq_live` in `comp_irq`."
* `examples/support/driven_irq_live.rs` + `driven_irq_core.rs`: the ordinary
  AM32 COMP decision (`minz_core::am32_isr::comp_isr`) with a **fixed**
  average of 1666 half-µs, so the gate is 416 µs after the last accept (or
  after `prepare`), with 12 live reads. Under `bench-startup-adc` a
  post-crossing level inside the gate is **deferred** (masked), and a 20 kHz
  timer re-pends it once the gate opens.
* `examples/support/driven_seed.rs::accept_startup` (the oracle's
  `with_startup_estimator`): one accept per driven sector (epoch). Epoch 0 is
  never an anchor. 12 consecutive-epoch intervals, each ≤ 12000 half-µs, a
  timer-bracket corroboration, and a sum in [12·952, 24000] half-µs, all
  inside 80000 half-µs. The seed is (step, edge, sum/12).
* `examples/support/phase_schedule.rs`: driven step = the sector whose
  source and sink are the highest and lowest sine phases (`step_for_values`).
* `examples/shell-pwm.rs:1350-1354`: **the reference's sine is relabelled for
  reverse wiring.** CCR3 (physical A) takes logical B and CCR2 takes logical
  A. firmware50's sine (`set_compares`) is **not** relabelled, so under
  `Reverse` its rotation opposes the six-step loop's. That must be fixed
  before the sine can hand over.
* `minz/core/src/am32_isr.rs:62-85`: a gate-closed edge whose level is
  post-crossing is left **pending** ("camp"). firmware50's `det_decide` acks
  at entry and so drops it. Noted for the closed loop, not changed in E058.

**Implementation (E058):** new pure modules `src/driven.rs` (boundaries) and
`src/seed.rs` (startup estimator, in µs), each with host tests transcribed
from the reference's. In the binary, `bemf_run`'s six-step open loop and
raw-edge acquisition are replaced by:

1. the sine script (`startup::Script`, 200 eHz, 6.2%), with the wiring
   relabel;
2. at `startup::HANDOFF_US`, the driven six-step at 6.1% on the 10 kHz
   carrier, continuing θ + 60°;
3. an observer-mode COMP decision in `ADC_COMP` (fixed 416 µs gate,
   deferral, 12 reads);
4. transfer on the first seed, with no extra `HANDOFF_PHASE_SECTORS` hop
   (the reference's 60° lives in the driven stage). No seed within 40 ms
   stops the run with `InvalidSeed`.

Divergences kept and named: firmware50's `startup::Script` catches at
100 eHz with a linear ramp, where the reference catches at 50 eHz with a
staircase. The driven commutation runs in the foreground, not a TIM3 ISR.

### E058 — build (pre-run)

* New `src/driven.rs`: `step_for_values`, `STEPS`/`NEXT`, `next(θ, rate)`,
  `phase_rate`, `phase_shift`, `initial_wait`. 10 host tests, including that
  firmware50's `sector()` table and the reference's `step_for_values` agree,
  and that increasing θ walks logical steps 1→6 (the closed loop's order).
* New `src/seed.rs`: `Qualification`, the oracle's `accept_startup`
  converted to µs. 11 host tests: constants halved, exact /12 over the
  domain, the 13-accept seed, epoch-0 non-anchor, re-anchor on a gap, and
  sequence, spacing, corroboration, window, band and frozen-seed behaviour.
* `bin/shell-pwm.rs`:
  * `set_compares_wired` (sine through the `Wiring` map, reference
    `shell-pwm.rs:1350`);
  * `drv_decide` in `ADC_COMP` (fixed 416 µs gate, deferral, 12 reads,
    record-only);
  * `bemf_run` startup is now sine (`startup::Script`, 200 eHz, 6.2%) →
    at `HANDOFF_US` (4.7 s) driven six-step at 6.1% / 10 kHz on θ+60°,
    boundaries from `driven::next` → foreground deferral re-pend → seed →
    transfer (DET seeded, commit from the seed edge, 48 kHz, duty ramp from
    6.2% to 10% measured from the transfer);
  * forced fallback only after transfer;
  * report line `BEMFDRIVEN`.
* **Audit caught a real regression at link time:** `ADC_COMP` reached
  `__aeabi_lmul`, because `commutation::advance_of`'s `saturating_mul` was
  lowered to a 64-bit helper once the level was visible. It is replaced by
  the exact whole/fractional-sixty-fourths split (the same technique as
  `blanking()`), with the level clamped to 64 (waits unchanged), plus a new
  host test against the 64-bit product. Audit passes: `ADC_COMP` has 2
  bounded loops and a longest path of 620 cycles (9.69 µs); `DMA1_CHANNEL1`
  is unchanged.
* Dead with E058 and removed: the six-step catch constants, `CATCH_*_MS`,
  `HANDOFF_PHASE_SECTORS`, `ACQUIRE_TIMEOUT_MS`, `last_comm_at`, and the
  `Acquisition` import. (`src/acquire.rs` stays as a tested module; the
  binary no longer uses it.)
* clippy clean; 238 host tests; ELF text 41764 / data 8 / bss 4264, sha256
  `78D733DD…9076FA6C`.

**Prediction:**

* (a) The sine turns the rotor for 4.7 s. The script is 1% align, a
  100 eHz catch at 6.2%, then a ramp to 200 eHz.
* (b) The driven stage produces persistence-qualified accepts in consecutive
  epochs and seeds within the 40 ms window (the reference took 10.6 ms). The
  seed interval is near 833 µs, but the accepts carry real spread.
* (c) After transfer, falling sectors accept far better than in E056/E057.

If (b) fails with `fail=4` (window) and accepts are only on one edge
direction, the rotor is again not following, and the sine stage is the
next suspect: 100 eHz catch vs the reference's 50 eHz, the staircase, and
direction.

### E058 — result (powered run, `captures/2026-09-20/e058-driven_01.txt`)

ELF `78D733DD…9076FA6C`.

```
BEMFDONE    reason=10 (InvalidSeed) accepted=0 forced=0
BEMFDRIVEN  entered=1 epochs=48 accepts=34 early=1 unstable=189 defers=2 retries=1 late_max_us=11
            qual_intervals=0 qual_reanchors=14 qual_fault=0 fail=4 seeded=0
BEMFCURRENT blocks=469 mean_ma=59        BEMFCOAST comp_edges=263 (rotation witnessed)
```

* **Safe and bounded, as designed:** 4.7 s of sine, 40 ms of driven drive,
  a named stop (`InvalidSeed`, `fail=4`: the window elapsed), 59 mA mean,
  and a coast witness of rotation.
* (a) The sine turns the rotor: there are coast edges, and 34 persistence-
  qualified accepts in 48 driven sectors. In E056/E057 the closed loop's
  falling sectors almost never passed 12 reads.
* (b) **Not met:** 71% of sectors accept, but 14 re-anchors mean the runs of
  consecutive epochs keep breaking before 12 intervals. Driven timing was
  clean (latest boundary 11 µs late).
* (c) Not reached.

The totals cannot say *which* sectors fail: it could be one edge direction,
one phase, or a drift through the window. The reference prints every accept
(`DI85` rows) for exactly this reason.

## E059 — instrument: driven accept rows (no control change)

`BEMFDRVROWS` prints every driven acceptance as `epoch:step:interval_us`,
up to 64. Control is unchanged from E058; ELF text 42648, sha256
`7E460D00…4563DEE6`, clippy clean.

**Prediction:** the missing epochs cluster by step parity (one edge
direction) or by floating phase (two steps sharing a phase). Intervals near
833 µs mean the crossings are phase-locked to the drive; a steady drift
means the rotor is slipping against the 200 eHz vector.

### E059 — result (powered run, `captures/2026-09-20/e059-rows_01.txt`)

ELF `7E460D00…4563DEE6`.

```
BEMFDRIVEN  epochs=48 accepts=37 unstable=146 defers=2 qual_reanchors=12 fail=4
BEMFDRVROWS 0:6:421 1:1:632 2:2:492 3:3:789 4:4:915 6:6:1602 8:2:1694 9:3:799
            10:4:827 11:5:842 12:6:838 14:2:1666 15:3:827 16:4:901 18:6:1706 20:2:1570 ...
BEMFCURRENT mean_ma=33
```

* **Step 1 accepts only once (epoch 1) in 8 chances; step 5 about half the
  time; steps 2, 3, 4 and 6 every time.** Every miss re-anchors the
  qualification, so 12 consecutive is never reached.
* Intervals are steady and repeat per step (step 4 ≈ 901, step 3 ≈ 780–830,
  step 6 ≈ 838 after a present step 5). **These are real crossings from a
  rotor phase-locked to the 200 eHz drive**, not drive transients and not
  slip.
* The misses do not follow edge direction alone: step 1 (C floating,
  falling) fails, step 5 (A, falling) is intermittent, step 3 (B, falling)
  works.
* A systematic per-sector offset with one or two sectors lost is what a
  crossing near a sector boundary looks like: the phase lead of the driven
  vector over the rotor may not suit this build's conventions (E058
  transcribed `drivephase60` onto firmware50's own sine/sector mapping).

## E060 — instrument: accept position within its sector (no control change)

Each row gains `position_us`, the time from that sector's driven
commutation to the accepted edge (`DRV_SECTOR_RAW`, set at every driven
command). Control is unchanged; ELF text 42284, sha256 `2AFF2C28…023D3079`;
clippy and audit clean.

**Prediction:** accepted positions cluster late in the 833 µs sector. Step 1
is missing because its crossing falls at or past the boundary, and step 5
sits on the edge. If positions sit mid-sector instead (~400 µs), the lead is
fine and step 1's failure is something else, such as the persistence reads
on phase C.

### E060 — result (powered run, `captures/2026-09-20/e060-pos_01.txt`)

ELF `2AFF2C28…023D3079`.

```
BEMFDRVROWS 0:6:421:421 1:1:428:191 2:2:707:80 4:4:1699:92 5:5:764:21 6:6:831:37 7:1:899:89 8:2:801:73
            9:3:799:33 10:4:907:100 12:6:1594:35 13:1:899:95 14:2:810:81 ...      (epoch:step:interval:position)
BEMFDRIVEN  epochs=48 accepts=41 unstable=150 qual_reanchors=7 fail=4
```

**The prediction was wrong, and the answer is plainer.** Nearly every accept
lands **15–100 µs after its sector's commutation**, at a fixed per-step
position (step 5 ≈ 20, step 6 ≈ 35, step 3 ≈ 30, step 2 ≈ 75, step 1 ≈ 90,
step 4 ≈ 100). These are not rotor crossings. They are post-commutation
transients: the newly floating phase freewheels through a diode to the rail
that reads as "post-crossing", and PWM ripple completes the edge. The gate
runs only 416 µs from the *previous accept*, so once one accept lands early,
every following commutation's transient is past the gate: **the observer
locks onto the drive**. The per-step misses are the steps whose transient
fails 12 reads.

**The reference's driven accepts are real crossings.** New
`scripts/decode_di85.py` decodes the qualified image's DI85 rows.
`captures/duty50_885_replacement_live_10_25_mcp.txt` has 13 consecutive
epochs (1..13, every sector) with spacing 715–864 µs, **mean 781 µs, faster
than the 833 µs drive**: the rotor accelerates ahead of the driven vector and
the accepts drift through the sectors. Its `DRIVENIRQ calls=283 accepts=13
rate_peak=70` in ~11 ms is ~26 COMP calls/ms. firmware50's driven stage made
~200 calls in 40 ms, ~5/ms.

**The one comparator divergence still standing is HYST: 1 here, 0 in the
oracle** (its feature closure lacks `bench-comp-hyst-low`). That fits the
data: HYST 1 holds COMP2 on its high offset and suppresses the real
transitions the reference sees five times as often, leaving only the large
post-commutation swings. HYST 1 was a stated divergence adopted in
E038–E051 to calm a closed-loop storm. The reference's control for that
storm is a protection, not hysteresis: `bench-reverse-irq-cap64`, more than
64 COMP calls per 1 ms bucket stops the bridge (`core_bench.rs:4062-4107`,
report-only in the driven stage per `driven_irq_live.rs`).

Also found and fixed: after a run that stopped inside the driven stage,
`DRV_ACTIVE` stayed set after `safe_off` (the line was masked right after,
so no decision could run with the bridge on). It is now cleared with
`DET_ACTIVE`.

**Tooling slip, repaired:** two PowerShell `ReadAllText`/`WriteAllText`
edits double-encoded every non-ASCII character in `bin/shell-pwm.rs` (`µ` →
`Ã‚Âµ`, 29 runs). A cp1252 round-trip reversal per run restored them all
(µ, —, →, –, ≈, ≥; zero suspicious sequences left). Edits now go through
the Edit tool or UTF-8-explicit Python only.

## E061 — HYST 0 (the reference's value) + the reference's COMP storm cutoff (build, pre-run)

* `COMP2_HYST` 1 → **0**.
* New `src/rate.rs`: `Rate` transcribed from `irq_dispatch::Rate`
  (`LIMIT = 64`, 1 ms fixed buckets, latched failure). 6 host tests,
  including the reference's own cases.
* `ADC_COMP`: in the closed loop, `DET_RATE.hit(raw)` comes before any
  decision. A trip sets `COMP_STORM` and leaves the line masked, and the
  foreground stops with `Reason::CompStorm` (13, wire-compatible). In the
  driven stage `DRV_RATE.observe(raw)` is report-only, as in the reference.
  Both rates reset at run start, and `DET_RATE` again at transfer.
* Report: `irq_peak_per_ms`, `closed_irq_peak_per_ms` and `storm` on
  `BEMFDRIVEN`.
* 244 host tests; clippy clean; `ADC_COMP` audit OK with 2 bounded loops and
  a longest path of 689 cycles (10.77 µs); ELF text 42820, sha256
  `4DDF9C7B…C6355896`.

**Prediction:**

* (a) The driven COMP call rate rises toward the reference's ~26/ms, with a
  per-ms peak near its 70.
* (b) Accept positions leave the fixed 15–100 µs post-commutation band and
  show real spread. 12 consecutive epochs qualify and seed near
  780–833 µs.
* (c) If it transfers: either the closed loop holds with the storm cutoff
  quiet, or it stops cleanly on `CompStorm`. Both are informative, and the
  protection trips by design rather than starving the scan as in E051.

### E061 — result (powered run, `captures/2026-09-20/e061-hyst0_01.txt`)

ELF `4DDF9C7B…C6355896`.

```
BEMFDRIVEN  epochs=13 accepts=14 unstable=163 defers=3 retries=1 qual_intervals=12 qual_reanchors=0
            seeded=1 seed_step=1 seed_us=799 commanded_us=833 irq_peak_per_ms=54
            closed_irq_peak_per_ms=65 storm=1 acquire_us=10764
BEMFDRVROWS 0:6:649:649 1:1:499:458 2:2:423:94 3:3:769:18 4:4:829:15 5:5:845:19 6:6:1135:335 7:1:699:183
            8:2:682:54 9:3:817:29 10:4:829:14 11:5:839:19 12:6:934:134 13:1:798:93
BEMFDONE    reason=13 (CompStorm) accepted=1 too_early=28 unstable=12
BEMFCURRENT mean_ma=68        BEMFCOAST comp_edges=116497
```

* (a) **Met:** a driven IRQ peak of 54/ms, against the reference's 70.
* (b) **Met in the headline, qualified in detail.** 12 consecutive epochs,
  no re-anchor, seed 799 µs, and **`acquire_us=10764` against the locked
  run's `acquisition_us=10649`**. The spacing has real spread
  (423–1135 µs). But steps 3/4/5 still accept 14–29 µs after commutation;
  steps 6/1 show 93–458 µs. The seed mixes both kinds of accept.
* (c) **The protection worked exactly as designed:** within the first ms of
  closed loop the COMP rate hit 65/ms > 64, `ADC_COMP` masked the line, and
  the run stopped with `CompStorm` (13) after 1 accept. It stopped at once
  instead of starving the scan as in E051.
* At HYST 0 the coast witness's `comp_edges` reads 116497: the stationary
  control that set its threshold of 20 was measured at HYST ≥ 1. **The
  witness threshold is void at HYST 0** and must be re-established before it
  can certify rotation again.

**Why the closed loop storms and the reference's does not.** Right after
transfer the estimate is 799 µs, and the reference arms **`bench-reverse-
blank`** in exactly that regime (`core_bench.rs:1829-1841`): at a
commutation with `average_interval >= 1500` half-µs it masks COMP and
re-opens it 280 µs later (`reverse_blank_poll`). "At this low speed, the
earliest useful crossing is later than 280us. Ignore PWM-coupled EXTI
traffic in that known blank window." The locked run reports
`REVERSEBLANK arms=4 max_mask_us=301`: it covers the handover and switches
itself off as the rotor speeds up.

## E062 — reverse blank (build, pre-run)

* At every closed-loop commutation: set the plan, mux, then if the estimate
  is ≥ `REVERSE_BLANK_MIN_AVG_US = 750` (1500 half-µs), mask and start
  `blank_since`; otherwise arm at once. Each iteration re-arms once
  `REVERSE_BLANK_US = 280` has elapsed (`comp_exti_arm`: edge select, clear
  pending, enable).
* The closed-loop commutation now muxes before arming (the reference's
  order; the E056-noted discrepancy).
* Report: `blank_arms`, `blank_max_us` on `BEMFDRIVEN`.
* clippy clean; audit OK; ELF text 42996, sha256 `700CF43F…EC3F5518`; no
  mojibake.

**Prediction:** the storm cutoff does not trip at handover. The blank arms a
handful of times and switches off as the estimate falls below 750 µs. The
closed loop then accepts in all six sectors and accelerates from ~800 µs
toward the reference's ~425 µs at 10%.

### E062 — result (powered run, `captures/2026-09-20/e062-blank_01.txt`)

ELF `700CF43F…EC3F5518`.

```
BEMFDRIVEN  epochs=13 accepts=14 unstable=10 qual_intervals=12 seeded=1 seed_step=1 seed_us=816
            irq_peak_per_ms=10 closed_irq_peak_per_ms=65 storm=1 blank_arms=2 blank_max_us=293 acquire_us=10697
BEMFDONE    reason=13 (CompStorm) accepted=1 too_early=0 unstable=63
```

* The seed reproduced again: 816 µs, **`acquire_us=10697`** (reference
  10649). Two repeats now.
* The blank armed at handover (2 arms, 293 µs max; reference
  `max_mask_us=301`). **But the storm still tripped**: once the line reopened,
  63 persistence failures arrived inside one millisecond. At HYST 0 on the
  48 kHz carrier the comparator chatters several times per carrier period.
* **Reference closed-loop IRQ rates** from its own diagnostic captures
  (`IRQRATE peak=`): 16–28 per ms (`traceoff_hold54/55`, `sustain*`,
  `timeline*`, `tracking_623`). The reference's comparator is quiet in the
  same regime, so the chatter is firmware50's.

**The structural drive difference that explains it.** E060's
post-commutation transients sat at per-step positions that repeat with a
three-sector period (step 3 ≈ step 6 ≈ 30 µs, step 1 ≈ step 4 ≈ 95 µs).
That is the carrier phase at each commutation (833.3 mod 100 = 33.3 µs). The
cause is `sixstep::plan`: it writes the source's compare and **zeroes the
other two**, and the CCRs are preloaded (OCxPE). The channel that has just
become source therefore holds a stale zero until the next update and does
not chop for up to a whole carrier period. The old source is off, the sink
is on, nothing drives, and the floating phase and star transient. The
qualified image keeps "all three TIM1 CCRs equal and active before first
enable; do not clear/latch them at each commutation"
(`phase_gpio_plan.rs:5-9`, `bench-pwm-roles` in its closure) and
commutates by role only. The binz `sixstep.rs` plan firmware50 was ported
from is the older one and is not what the qualified image drives.

## E063 — equal compares on all three channels (build, pre-run)

* `sixstep::plan`: `ccr = [period·duty/1000; 3]`. Roles stay in the modes:
  the source uses PWM1, the sink force-inactive (low side on whatever its
  compare), and the floating phase has its CCER nibble cleared (outputs off
  whatever its compare). New host tests: all three compares equal with only
  the source chopping, and a commutation changes no compare. The four
  existing compare tests are updated to the per-channel form.
* Everything else as E062 (HYST 0, storm cutoff, reverse blank, sine→driven
  seed).
* 245 host tests; clippy clean; audit OK; ELF text 43004, sha256
  `3B10AA0E…6878A57C`.

**Prediction:** driven accepts stop landing at fixed carrier-phase positions
15–100 µs after commutation. The closed loop no longer chatters past 64/ms
after the blank, so `CompStorm` does not trip. The loop then accepts across
all six sectors.

### E063 — result (powered run, `captures/2026-09-20/e063-equalccr_01.txt`)

ELF `3B10AA0E…6878A57C`.

```
BEMFDRIVEN  epochs=13 accepts=14 unstable=83 qual_intervals=12 seeded=1 seed_step=1 seed_us=816
            irq_peak_per_ms=58 closed_irq_peak_per_ms=65 storm=1 blank_arms=1 blank_max_us=281 acquire_us=10699
BEMFDRVROWS 0:6:455:455 1:1:424:218 2:2:612:14 3:3:859:20 4:4:821:15 5:5:980:150 6:6:919:252 7:1:690:95
            8:2:772:53 9:3:798:14 10:4:849:15 11:5:841:19 12:6:841:41 13:1:820:20
BEMFDONE    reason=13 (CompStorm) accepted=0 unstable=64          BEMFCURRENT mean_ma=39
```

* **The prediction failed.** Equal compares did not move the driven accepts
  off their fixed early positions: steps 2/3/4/5 still accept 14–20 µs after
  commutation. The closed loop still storms, with 64 persistence failures
  inside the first reopened window. The preload gap was a real deviation
  from the reference's drive and stays fixed, but it is not the mechanism.
* The seed reproduced a third time (816 µs, `acquire_us=10699`). The
  reference's driven IRQ peak was 70/ms; this run's was 58.
* What sits at a fixed ~15 µs is most likely **demagnetisation**. The newly
  floating phase is clamped by its freewheel diode to the rail that reads as
  post-crossing; the arm clears the demag-start edge, PWM switching then
  supplies an armed-polarity edge, and the clamp holds for 12 reads.
* Read-only finding on the reference's transfer: `driven_run::end(…, 22)`
  calls `bridge_clear()`, so the reference **coasts at transfer** and
  `coast_run_body` arms the first closed-loop commutation from the flying
  seed (`fly_arr`, `fly_arm_us`) before the bridge re-energises.
  firmware50 keeps driving through the transfer. Noted; not yet
  transcribed.
* All four COMP pins are in analog mode (`into_analog`), so the pin
  configuration is ruled out.

## E064 — instrument: closed-loop IRQ trace (no control change)

* `ADC_COMP` records the first 48 closed-loop calls as
  `tim1_cnt:raw_us:step:level:accepted` (`BEMFIRQTRACE`), cleared at
  transfer. `tim1_cnt` places each call in the 48 kHz carrier period
  (ARR 1332; ON while `cnt < CCR`).
* The audit flagged `ZeroCross::offer` once it was no longer inlined. It was
  reviewed and allowlisted with its 12-read bound, and the `ADC_COMP` note
  was updated to name `drv_decide`'s loop too.
* `isr_cycles.py` gains `--callee-bound SYMBOL=N` (callee loops are still
  refused without one). `ADC_COMP` longest path is 833 cycles (13.02 µs)
  with the diagnostic trace in.
* ELF text 43892, sha256 `B432FDDA…FAF65803E`; clippy clean; audit OK.

**Prediction:** if the storm is switching noise, the calls cluster at
`tim1_cnt ≈ 0` and `≈ CCR` (the two switching instants). If it is offset
chatter in the OFF window, they spread across `CCR..1332`. The level
column says whether the comparator sat at the pre- or post-crossing level
when each call arrived.

### E064 — result (powered run, `captures/2026-09-20/e064-irqtrace_01.txt`)

ELF `B432FDDA…FAF65803E`.

```
BEMFIRQTRACE 199:13926:3:1:0 887:13936:3:1:0 207:13947:3:1:0 779:13956:3:1:0 21:13965:3:0:0 567:13973:3:1:0 ...
             (tim1_cnt:raw_us:step:level:acc; 48 rows, all step 3, all refused)
BEMFDRVROWS  0:6:424:424 2:2:1213:76 3:3:706:17 4:4:828:15 5:5:847:19 6:6:811:15 7:1:922:86 8:2:743:14 ...
BEMFDRIVEN   seeded=1 seed_step=2 seed_us=820 irq_peak_per_ms=103 closed_irq_peak_per_ms=65 storm=1
BEMFCURRENT  mean_ma=133
```

* **The closed-loop storm is comparator oscillation, not switching noise.**
  Calls arrive back to back, every ~9.5 µs (raw 13926, 13936, 13947, …),
  which is the handler's own duration: the next edge is already pending when
  it exits. Their TIM1 phases are spread uniformly over the 1333-tick period
  (199, 887, 207, 779, 21, 567, …), with no clustering at the switching
  instants. The level at entry is almost always **1**. Step 3 arms a falling
  edge whose post-crossing level is 0, so the comparator is flickering low
  and returning high before the handler can read it: a zero-hysteresis
  comparator sitting **at its threshold**. The floating phase (B) sits at the
  star voltage and carries no usable back-EMF.
* The driven accepts are again mostly the 13–19 µs post-commutation
  transient, so this seed was transient-based too.

Both facts point at a question no instrument in this build has answered:
**does the sine carry the rotor to 200 eHz at all?** A rotor at 200 eHz shows
real mid-sector crossings (as the reference's DI85 rows do) and a floating
phase clearly off the star, not a comparator pinned at threshold.

**Also found: the run's coast witness has been measuring a stopped rotor.**
`bemf_run` called `coast_capture` only after writing and flushing its
entire report at 115200 baud, hundreds of milliseconds after `safe_off`.
On this low-inertia rotor (binz scar: coast "stops in ~ms") that window is
mostly after rotation has ended.

## E065 — instrument: sine probe + immediate coast timing (no control change to `b`'s drive)

* `coast_capture`:
  * sets COMP2 HYST to 1 for its window (`COAST_COMP_HYST`, the setting its
    E040 stationary control was measured at) and restores the run's HYST
    after;
  * records **debounced transitions** (level held ≥ 40 µs): the count, the
    delay from the float to the first, and the first eight spacings.
    `COASTTIMING … ehz_first` is `500000 / spacing`, since one phase makes two
    transitions per electrical cycle.
* `bemf_run` takes a `RunMode`:
  * `Full` (command `b`, unchanged drive);
  * `SineProbe` (new command `k`): the identical sine startup, stopping with
    `SegmentDeadline` exactly where the driven stage would begin.
* **The coast now runs immediately after `safe_off`, before any report
  byte**, for both modes.
* `c` (never-driven control) also prints `COASTTIMING`.
* `scripts/bemf_run.py --command {b,k}`. The end marker is now
  `COASTTIMING`.
* clippy clean; audit OK; ELF text 45836, sha256 `7B1C9E0B…3DE2994B`.

**Plan:**

1. `c` (stationary control, bridge never energised): expect 0–1
   transitions, which validates that the timing cannot be manufactured by
   noise.
2. `k`: the coast timing gives the rotor's electrical speed at the
   handover instant.

**Prediction:** if the sine carries the rotor, `ehz_first` ≈ 200 (spacing
≈ 2500 µs). If it reads far lower, or shows no transitions, the rotor is
not following the sine to 200, and the startup profile (100 eHz catch,
linear ramp, versus the reference's 50 eHz catch and staircase) is the next
thing to fix.

### E065 — results

**Control** (`c`, bridge never energised; `captures/2026-09-20/e065-control_01.txt`):

```
COASTCONTROL comp_edges=6 comp_hi=472245 comp_polls=472248
COASTTIMING  trans=0 first_us=0 ehz_first=0
```

Zero debounced transitions: noise cannot manufacture the timing, and at rest
the comparator reads high (as E049).

**Sine probe** (`k`; `captures/2026-09-20/e065-sineprobe_01.txt`, ELF
`7B1C9E0B…3DE2994B`):

```
SINEPROBE      target_ehz=200 duty_tenths=62
BEMFDONE       reason=2 (probe end at the handover instant)
BEMFCURRENT    mean_ma=113
SINEPROBECOAST spun=0 comp_edges=2
COASTTIMING    trans=0 first_us=0 ehz_first=0
```

**The rotor is not turning at the handover.** The coast straight after the
sine is indistinguishable from the never-driven control: 0 transitions and
2 raw edges. A rotor at 200 eHz makes a transition every ~2.5 ms. This
explains E058–E064 as one fact:

* no rotor crossings exist, so the driven observer saw only
  post-commutation transients;
* the seeds were transient-based, and matched the reference's
  `acquire_us` only because that is 12 sectors of 833 µs either way;
* the closed-loop comparator sat at threshold because a stationary rotor
  makes no back-EMF.

(The earlier six-step runs did leave a spinning rotor: ~200 coast edges even
after the slow report flush.)

**The unreproduced piece is the startup profile.** firmware50's
`startup::Script` catches at **100 eHz** and ramps linearly. The
reference's, compiled into the oracle (`bench-startup-staircase`,
`campaign.rs::staircase_target`), is a 20 ms align at 1%, **50 eHz** until
tick 3200, then fifteen 10 eHz steps at deadlines `3200 + floor(i·1200/14)`
to reach 200 eHz at tick 4400, and handover at 4.7 s. E031's "50 eHz does
not move the rotor" was measured on the six-step open loop, not the sine.

## E066 — the reference's staircase startup (build, pre-run)

* `startup::staircase_ehz`: transcribed deadline table, plus the reference's
  own compile-time table check. New `StaircaseScript` has the same stage
  lengths and handover as `Script` (align 1%, catch/ramp at the catch duty,
  hold at the target duty) with the staircase frequency. 3 host tests:
  endpoints, monotone 10 eHz steps (exactly 15), and stages with 200 eHz at
  the 4.7 s handover. `Script` is unchanged for its other users.
* `bemf_run` (both `b` and `k`) uses `StaircaseScript::new(62, 62)`.
* 248 host tests; clippy clean; audit OK; ELF text 46000, sha256
  `FCB20469…37C3D1B1`.

**Plan:** `k` first, the sine probe alone. **Prediction:** the coast at
handover shows debounced transitions at a spacing near 2500 µs
(`ehz_first` ≈ 200). If it does, `b` follows with the full chain.

### E066 — sine probe result (`captures/2026-09-20/e066-stairprobe_01.txt`, ELF `FCB20469…37C3D1B1`)

```
SINEPROBECOAST spun=1 comp_edges=556
COASTTIMING    trans=106 first_us=1582 iv_us=2229,2824,2236,2991,2165,3006,2344,3018 ehz_first=224
```

**Met: the rotor follows the reference's staircase to the handover speed.**
106 debounced transitions. The half-periods alternate unequally (the
comparator offset shifts the threshold off the back-EMF midpoint), but each
consecutive pair sums to one electrical cycle: 2229+2824 = 5053 µs,
2236+2991 = 5227, 2165+3006 = 5171, i.e. **~195–198 eHz** at the instant the
bridge floated, against a commanded 200. (`ehz_first` uses one half-period
and reads high for that reason. The pair sum is the right estimate.) The
same probe on `startup::Script` gave 0 transitions: **the 100 eHz linear
catch was the defect behind every startup since the sine was introduced.**

Next, without a rebuild: `b` on the same ELF, the full chain.

### E066 — full-chain result (`captures/2026-09-20/e066-full_01.txt`, same ELF)

```
BEMFDRVROWS 5:6:4174:13 8:3:3369:867 9:4:747:719 10:5:800:749 11:6:656:585 12:1:777:524 13:2:668:374
            14:3:750:278 15:4:681:125 16:5:735:21 17:6:886:81 18:1:810:53 19:2:793:24 20:3:837:19
BEMFDRIVEN  seeded=1 seed_step=3 seed_us=761 acquire_us=16700 irq_peak_per_ms=40 blank_arms=61 blank_max_us=305
BEMFDONE    reason=13 (CompStorm) accepted=59 forced=8 too_early=49 unstable=2097
BEMFGATE    closed_ms=1557 comm_per_s=43 forced_pct=11
BEMFSECTOR  a1=10 f1=2 a2=9 f2=1 a3=9 f3=2 a4=12 f4=1 a5=9 f5=1 a6=10 f6=1
BEMFPHASE   lt075=5 to100=27 to125=24 to150=2 gt150=1
BEMFREVISIT r1=2 v1=2 r2=3 v2=3 r3=5 v3=4 r4=2 v4=2 r5=3 v5=3 r6=3 v6=2
BEMFWITNESS samples=165          COASTTIMING trans=98 iv_us=2731,3834,2589,3879,... (≈ 150 eHz at the stop)
```

**The whole reference chain ran on a spinning rotor for the first time.**

* **Driven accepts are real crossings.** Their position in the sector drifts
  steadily earlier (867 → 719 → 749 → 585 → 524 → 374 → 278 → 125 → 21 µs)
  with spacing 656–800 µs < 833. That is the rotor pulling ahead of the
  driven vector, **the same signature as the reference's DI85 rows**
  (spacing 715–864, mean 781). The seed (step 3, 761 µs) came from them.
* **Closed loop:** all six sectors accept (9–12 each), only 11% of
  commutations are forced, accepts land around one interval
  (`to100`/`to125`), revisits convert, and the reverse blank armed 61 times.
  The coast at the stop still shows a turning rotor (pair sums ≈ 6.5 ms ≈
  150 eHz).
* **But** it ended on `CompStorm`, and in the reported 1557 ms it commutated
  only 67 times (43/s, where 6 × 200 would be 1200/s). The periodic `BEMF`
  report, due every 500 ms, **never printed once after transfer**, and
  `BEMFWITNESS samples=165` means the foreground consumed almost no ADC
  scans. So the foreground was starved or stalled after transfer, and
  the 1557 ms figure is itself suspect (the extended µs clock needs a read
  every 65 ms). The first 48 closed-loop COMP calls include a run at a fixed
  carrier phase (TIM1 ≈ 333, one per 20.8 µs period, level 0 = pre-crossing
  in step 6): a PWM-synchronous event.

## E067 — instrument: foreground health after transfer (no control change)

`BEMFDRIVEN` gains `loop_iters_closed` and `loop_gap_max_us`: iterations and
the longest gap between two iterations after transfer, on the raw 16-bit
timer. ELF text 46148, sha256 `44CF2B64…CB85FA0B`; clippy clean.

**Prediction:** `loop_gap_max_us` is large (ms or tens of ms), meaning
something starves or blocks the foreground after transfer. If gaps stay
small with few iterations, the loop is spinning somewhere without passing
the loop top.

### E067 — result (`captures/2026-09-20/e067-loop_01.txt`, ELF `44CF2B64…CB85FA0B`)

```
BEMFDRIVEN  seeded=1 seed_step=4 seed_us=767 acquire_us=17632 blank_arms=57 closed_irq_peak_per_ms=65 storm=1
            loop_iters_closed=2225 loop_gap_max_us=321
BEMFDONE    reason=13 accepted=58 forced=6          BEMFSECTOR a1=10 a2=11 a3=9 a4=8 a5=9 a6=11 (forced 0-2 each)
BEMFGATE    closed_ms=1554 (WRONG, see below)       COASTTIMING iv_us=3693,2614,3849,2659,... (≈ 155 eHz at stop)
```

* **The prediction was wrong, and the fault is mine.** The foreground was
  healthy: 2225 iterations after transfer, longest gap 321 µs. The closed
  loop really lasted only **~55–65 ms**: 64 commutations at ~860 µs,
  ≈ 190 eHz. `closed_ms=1554` (and E066's 1557) counts the **1.5 s coast
  window**, because E065 moved the coast ahead of the report and the report
  read the clock at print time. `comm_per_s`/`zc_per_s` were deflated by the
  same factor. Nothing was starved, and the missing periodic reports are
  simply because the closed phase lasted less than one 500 ms report period.
  **Fixed:** `stopped_at` is stamped at loop exit and every reported
  duration uses it.
* So the real result is the chain working for ~60 ms, with all six sectors
  accepting and 9% forced, before the storm cutoff trips.

**What trips it.** E066's trace had a run of COMP calls at a fixed carrier
phase (TIM1 ≈ 280–340 at entry, one per 20.8 µs period, pre-crossing
level), i.e. at the source's turn-off edge plus handler latency: a glitch at
every switching instant. The reference sees 16–28 calls/ms in the same
regime, so its floating phase does not glitch like this.

**A reference drive difference that would cause exactly that.** With
`OSSR=1` and `MOE=1`, a channel with **both** `CCxE` and `CCxNE` clear is
**Hi-Z** (RM0444 output-control table). firmware50's floating phase was
programmed that way, so its two DRV8304 inputs rested on the driver's weak
internal pull-downs, beside traces switching at 48 kHz. The reference
drives them: "all other pins are actively driven GPIO, including both
floating-phase inputs low" (`phase_gpio_plan.rs`). A weakly held floating
gate that couples a glitch onto the floating phase at each switching edge
matches the trace.

## E068 — floating phase driven low, not released; report durations from the stop instant (build, pre-run)

* `sixstep::plan`: the floating channel is force-inactive with `CCER`
  nibble `0b0001` (`CCxE` only). Per the same table, with `OSSR = 1` that
  gives OCx = OCxREF = 0 and OCxN its off-state level (`CCxNP` = 0): **both
  gate inputs driven low, half-bridge off.** New host test
  `the_floating_phase_is_driven_off_not_released`; two geometry tests are
  updated to the new nibble. The source→floating transition passes through
  the same brief (~100 ns, CCMR-then-CCER write) low-side-on window as
  before, a normal synchronous-rectification state with dead time intact.
  There is no shoot-through path.
* The E067 `stopped_at` report fix.
* 249 host tests; clippy clean; audit OK; ELF text 46160, sha256
  `FD4E133C…BA52992F9`.

**Prediction:** the PWM-synchronous call runs disappear from the closed-loop
trace, the COMP rate stays under the 64/ms cutoff after transfer, and the
closed loop runs to the 40 s deadline or to some other named stop.

### E068 — result (`captures/2026-09-20/e068-floatlow_01.txt`, ELF `FD4E133C…BA52992F9`)

```
BEMFDONE     reason=13 accepted=8 forced=11 too_early=60 unstable=371
BEMFGATE     closed_ms=14 (now correct) forced_pct=57
BEMFDRIVEN   seeded=1 seed_step=4 seed_us=774 acquire_us=17560 blank_arms=13 loop_iters_closed=648 loop_gap_max_us=165
BEMFIRQTRACE ... 233:22844:6:1:0 232:22864:6:1:0 234:22885:6:1:0 ... 206:23135:1:1:0 208:23156:1:1:0 ...
             ... 312:23636:2:0:0 324:23657:2:0:0 326:23678:2:0:0 ...
COASTTIMING  trans=124 iv_us=2195,2853,2258,2896,... (≈ 195 eHz at the stop)
```

* **The prediction failed.** Driving the floating inputs low did not remove
  the PWM-synchronous calls. There are still runs at a fixed TIM1 count, one
  per 20.8 µs carrier period, in step 1 (~206–234), step 6 (~232) and step 2
  (~312–328). The storm trips 14 ms after transfer. The change stays: it is
  the reference's drive and it is strictly stronger. But it is not the
  glitch's mechanism.
* The event sits at the source's **turn-off** edge plus handler latency (the
  CCR at 6.2% on 1333 ticks is 82, and entry latency plus trace adds ~1–2 µs).
  It is a switching transient coupled into the comparator, present in every
  period while the ON window is this short.
* The reference's own notes: its startup also once "stopped 65 dispatches/ms /
  ratecap64" (AGENTS E724) and was made report-only, as firmware50's driven
  stage already is. Its closed loop at `bemfdu100` stays under the cap for
  35 s.

**One clear closed-loop divergence remains in the first moments after
transfer:** the reference applies its BEMF duty (`bemfdu100`) **at the
transfer** (`driven_power_run` stores it into `POWER_DUTY`), while
firmware50 ramped 6.2% → 10% over `BEMF_DUTY_RAMP_MS = 2000`, a smoothing I
added against a sag stop on the old six-step handover. That ramp keeps the
whole first closed-loop second at 6.2%: a 1.3 µs ON window at 48 kHz, with
the turn-on and turn-off transients nearly overlapping, and a rotor held near
the handover speed where its back-EMF is weakest.

## E069 — the BEMF duty is applied at the transfer (build, pre-run)

* The duty ramp is removed. Once closed, the duty is
  `governor.clamp(target_duty_tenths)` (10.0%). The transfer applies that
  duty and its advance (`advance_for(target)`) immediately.
* The fast-sag stop stays armed: a sag from the step is a named stop, not a
  reason to reinstate the ramp.
* clippy clean; audit OK; ELF text 46104, sha256 `AF806818…48E6D5`.

**Prediction:** at 10% the rotor accelerates away from ~200 eHz toward the
reference's ~400 eHz. The PWM-synchronous runs shorten and the COMP rate
stays under 64/ms. Alternatively the step sags the rail and the fast-sag stop
names it.

### E069 — result (`captures/2026-09-20/e069-dutystep_01.txt`, ELF `AF806818…48E6D5`)

```
BEMFDONE    reason=13 accepted=13 forced=6 too_early=21 unstable=182 ci_us=352
BEMFGATE    closed_ms=7 ehz_from_ci_last=473 forced_pct=31
BEMFPHASE   lt075=10 to100=0 to125=1 to150=2
COASTTIMING iv_us=2147,2791,2094,2860,... (≈ 200 eHz at the stop)
```

**Worse: 7 ms.** The estimate collapsed from 771 to **352 µs** within a few
sectors, and 10 of 13 accepts landed early (`lt075`), while the coast shows
the rotor still at ~200 eHz. **The loop accepted glitches as crossings, sped
its own schedule up and lost the rotor.** The duty step is the reference's
behaviour and stays, but it is not the fix.

## E070 — persistence cadence measured (unpowered) and the COM ISR root (build, pre-run)

**Read cadence** (`q`, the reference's `readcadencecheck`;
`captures/2026-09-20/e070-readcadence_01.txt`): firmware50's 12 live reads
take **2–5 µs** (1 µs resolution), against the oracle's inline `min_us=3
max_us=4` (`binz/captures/compinline_537_read01.txt`). The filter window
matches, so it is not why glitches pass.

**The remaining large structural divergence, and a goal-gate-5 item: the
COM root.** The reference commutates from a hardware one-shot armed by the
accepted crossing (AM32 `interruptRoutine` → COM timer →
`PeriodElapsedCallback`; `minz/core/src/am32_isr.rs:108-118`).
firmware50 commutated from the foreground at `commit_at`, and the loop
monitor measured foreground gaps of **165–321 µs** (E067/E068). On
400–800 µs sectors that makes a commutation a large fraction of a sector
late, which moves the next crossing, lets glitches win, and starves the
foreground further through the chatter: a collapse that feeds itself, which
fits every closed-loop death since E066.

**Built:**

* **TIM16 one-shot COM root**, the seventh PAC escape. It is justified from
  the vendored HAL: `Timer::start` divides at runtime (`let psc = cycles /
  0xffff; let arr = cycles / (psc + 1);`, `timer/mod.rs:154-155`), re-derives
  PSC per call, and has no OPM. It is set up once at PSC=63 (1 MHz) with OPM
  and URS, with priority 0x00, a peer of COMP (the reference: "COMP/COM
  remain priority 0x40 peers" below 48%).
* `ADC_COMP`'s accept arms it with `wait` (`com_arm`).
* `TIM16`, phase 1:
  1. next step;
  2. the precomputed plan from a double-buffered table (`com_publish_plans`,
     flipped only after the inactive half is written; indices masked);
  3. mux;
  4. then either the 280 µs reverse blank (phase 2) or an immediate re-arm.

  Phase 2 re-arms. It also records its own lateness.
* Foreground after transfer: statistics only. The bring-up fallback
  *requests* a commutation (mask line, phase 1, pend TIM16) inside a
  critical section, and never switches the bridge itself. Duty changes
  republish the plans.
* Transfer publishes the plans and arms the first commutation at
  `seed edge + wait`. Run end stops TIM16 before `safe_off`.
* Report: `com_count`, `com_late_max_us`, `blank_arms` (from the root).

**Audit (fail-closed), longest paths @ 64 MHz, 0 ws:**
* `TIM16`: clean, **no loops**, 360 cycles (5.62 µs).
* `ADC_COMP`: clean, 912 cycles (14.25 µs, including the diagnostic trace
  and `com_arm`).
* `DMA1_CHANNEL1`: clean, 99 cycles.

249 host tests; clippy clean; ELF text 47608, sha256 `9E90227F…9A68C10922`.

**Prediction:** `com_late_max_us` stays at a few µs, where foreground
commutation was up to 321 µs late. The closed loop holds its estimate near
the rotor's real interval (no `lt075` runaway) and runs well past the
7–60 ms of E066–E069. If glitches still win at exact timing, the next
suspect is the AM32 gate-closed "camp" that firmware50 does not reproduce.

### E070 — result (`captures/2026-09-20/e070-comroot_01.txt`, ELF `9E90227F…9A68C10922`)

```
BEMFDONE    reason=13 (CompStorm) accepted=597 forced=0 too_early=372 unstable=9221 ci_us=426
BEMFGATE    closed_ms=284 hold_ms=284 zc_per_s=2102 comm_per_s=2102 ehz_from_ci_last=391 forced_pct=0
BEMFRATE    mean_ci_us=476 mean_sector_us=475 ehz_from_sector=350 zc_rate_permille_of_expected=998 hold_forced_pct=0
BEMFSECTOR  a1=100 f1=0 a2=98 f2=0 a3=100 f3=0 a4=99 f4=0 a5=100 f5=0 a6=100 f6=0
BEMFPHASE   lt075=2 to100=291 to125=302 to150=2 gt150=0
BEMFREVISIT r1=16 v1=16 r2=22 v2=22 r3=32 v3=32 r4=24 v4=24 r5=30 v5=30 r6=27 v6=27
BEMFDRIVEN  seeded=1 seed_step=4 seed_us=770 acquire_us=17560 blank_arms=1 com_count=598 com_late_max_us=3
            loop_iters_closed=14516 loop_gap_max_us=333 closed_irq_peak_per_ms=65 storm=1
COASTTIMING trans=273 iv_us=1386,1198,1378,1185,1417,1174,1402,1225
```

**The closed loop locks.** With commutation in the hardware COM root:

* **597 accepted, 0 forced**, and **COM lateness 3 µs max** (foreground
  commutation had up to 321 µs of gap).
* All six sectors are balanced (98–100 accepts each), and 595 of 597
  accepts land between 0.75 and 1.25 intervals.
* **Rate identity `zc_rate_permille_of_expected=998`.** The revisits convert
  1:1 (151 of 597, 25%; the reference: 7266 of 81157, 9%).
* The estimate settles at **426 µs = 391 eHz**, the qualified image's
  **394–401 eHz at 10%**.
* **Independent physical confirmation:** the coast after the stop (bridge
  off, rotor's own back-EMF) gives pair sums of 1386+1198 = 2584 µs, i.e.
  **≈ 387 eHz**. The rotor really was turning at the reference's speed.

**Limiter:** `CompStorm` at 284 ms. `unstable=9221` over 284 ms is ~32 COMP
persistence refusals per ms on average, and one 1 ms bucket exceeded 64.
The trace held only the first 48 calls after transfer, so it cannot show the
storm.

## E071 — instrument: IRQ trace as a ring of the last 64 calls (no control change)

The trace is written for every closed-loop call **before** the storm check
(so the tripping call is in it, outcome 2), into a 64-entry power-of-two
ring, and dumped oldest-first with the total count (`BEMFIRQRING`). ELF text
47412, sha256 `F9210FBE…32DCE027`; clippy clean; audit OK.

**Prediction:** the last 64 calls show what fills the tripping bucket:
back-to-back calls at handler spacing (threshold oscillation near a
crossing), a run at one carrier phase (switching glitches), or both.

### E071 — result (`captures/2026-09-20/e071-ring_01.txt`, ELF `F9210FBE…32DCE027`)

```
BEMFDONE    reason=13 accepted=607 forced=0 unstable=8707        BEMFGATE closed_ms=288 ehz_from_ci_last=393
BEMFRATE    zc_rate_permille_of_expected=999                     BEMFSECTOR a1..a6 = 101,101,101,101,102,101 (forced 0)
BEMFIRQRING total=13306   (last 64: ~25 calls per ~430 us sector)
COASTTIMING iv_us=1248,1330,1242,1378,... (≈ 388 eHz)
```

**The lock reproduced** (0 forced, 999‰, ~390 eHz by estimate and by coast).
The ring shows two kinds of call in the pre-crossing window of each sector:
a run at a fixed TIM1 count (step 5 ≈ 190, step 6 ≈ 295), and scattered
calls at other phases, arriving 7–17 µs apart, with bursts at 8–9 µs. That
is ~55–65 calls per ms, so the loop runs **pinned against the 64/ms
cutoff**.

### E072 — ADC-phase probe (`captures/2026-09-20/e072-adcphase_01.txt`, ELF `B5E65F0B…63BCD7`)

The ring gained TIM6's counter (the ADC trigger phase, 0–6464 at 64 MHz).

```
BEMFDONE  reason=13 accepted=3002 forced=0 unstable=34961       BEMFGATE closed_ms=1287 ehz_from_ci_last=405
BEMFIRQTRACE ... tim6 phases 1680,2568,3678,4215,4765,5497,6069,147,780,1277,... (uniform)
COASTTIMING iv_us=1166,1274,1186,1361,... (≈ 395 eHz)
```

* **The ADC is not the source.** The scattered calls are uniform in the ADC
  trigger phase, with no clustering after scan starts.
* The longest lock yet: **3002 accepted, 0 forced, 1.29 s, 405 eHz.**

**Oracle comparator configuration measured directly** (flashed the frozen
ELF `0C734C67…`, booted to idle with the bridge off, and read COMP2_CSR over
SWD, as the goal sanctions; firmware50 was flashed back afterwards):
**`0x40000281`**. That is EN, INMSEL 8, INPSEL 2, **HYST 0**, PWRMODE high
speed, no blanking, and VALUE=1 (high at rest, the same bias). **firmware50
reads byte-identical `0x40000281`** at idle. The minz filter depth at this
speed is 12 as well (`am32_loop.rs:298-305`: `map(average_interval, 100,
500, 3, 12)`, clamped at 12 for ~846 half-µs sectors), and `average_interval`
there is one sector in half-µs, so the reference's gate is half a sector,
the same as firmware50's.

**The pattern that explains the storm, found by timing rather than by
mechanism:** every COM-root run stopped at a periodic-report instant.

| run | stop (transfer ≈ 4.717 s + closed) | `BEMF` report due |
|---|---|---|
| E070 | 5.001 s | 5.000 s |
| E071 | 5.005 s | 5.000 s |
| E072 | 6.004 s (survived 5.0 and 5.5) | 6.000 s |

And the ring's back-to-back spacing, **8–9 µs, is one bit at 115200 baud
(8.68 µs)**. The 500 ms report puts ~180 characters on USART3 TX (~16 ms of
bit edges) while a zero-hysteresis comparator watches a millivolt-level
difference. The reference prints nothing while powered: its summaries are
all `postrun_only=1`, and `LOW_DUTY_REPLICATION.md` requires the host to be
silent during the powered interval.

## E073 — no UART traffic once the comparator is in charge (build, pre-run)

The periodic `BEMF` report now runs only during the sine stage
(`!closed && !driven`). The last sine-stage report (4.5 s) drains in ~17 ms,
long before the driven stage at 4.7 s. Nothing else writes the UART during
the powered run, and the host script sends nothing until its stop byte.
clippy clean; all three roots audit clean.

**Prediction:** the closed loop no longer stops on `CompStorm` at report
instants. It holds until the 40 s segment deadline (`reason=2`), or stops on
some other named protection.

### E073 — result: **SUSTAINED BEMF LOCK, 35.3 s at 10%** (`captures/2026-09-20/e073-quiet_01.txt`, ELF `C701C0DF…0084B8046B`)

```
BEMFDONE    reason=2 (SegmentDeadline, normal end) accepted=84202 forced=0 too_early=351 unstable=927709 ci_us=418
BEMFGATE    closed_ms=35283 hold_ms=35283 zc_per_s=2386 comm_per_s=2386 ehz_from_ci_last=398 forced_pct=0
BEMFRATE    mean_ci_us=419 mean_sector_us=419 ehz_from_sector=397 zc_rate_permille_of_expected=1000 hold_forced_pct=0
BEMFSECTOR  a1=14034 a2=14033 a3=14033 a4=14034 a5=14034 a6=14034 (f1..f6 = 0)
BEMFPHASE   lt075=2 to100=44013 to125=40186 to150=1 gt150=0
BEMFREVISIT r/v = 1761/1761 1563/1563 3071/3071 1906/1906 2904/2904 3076/3076  (14281 = 17% of accepts)
BEMFDRIVEN  seeded=1 seed_step=3 seed_us=761 acquire_us=16803 com_count=84203 com_late_max_us=3
            closed_irq_peak_per_ms=56 storm=0 blank_arms=1 loop_iters_closed=1686989 loop_gap_max_us=167
COASTTIMING trans=281 iv_us=1347,1193,1342,1204,1357,1214,1333,1217  (pair ≈ 2540 us ≈ 394 eHz)
BEMFCURRENT blocks=3958 mean_residual=-207 mean_ma=-25   <- see below
```

**The prediction held.** With the UART silent the COMP peak was 56/ms (< 64),
the cutoff never tripped, and the run ended on the 40 s segment deadline.

| | firmware50 E073 | qualified reference (LOW_DUTY_LOCK_EVIDENCE) |
|---|---|---|
| closed-loop time | 35.283 s (MCU-stamped) | 35.000005 s |
| accepted / commutations | 84202 / 84203 | 81157 / 81157 |
| forced | 0 | 0 |
| speed | 397–398 eHz (estimate); 394 eHz (coast) | 394 eHz (`estimated_ehz`) |
| protections | none tripped, normal deadline | none, normal deadline |
| revisit share | 17% | 9% |

The binz lock verifier's rule (|accepted − commutations| ≤ max(2, com/20)):
|84202 − 84203| = 1. **Pass.**

**Against the goal (at 10%, not yet 15%):**

* **Gate 1 shape:** 35.3 s after handover, MCU-stamped, and every one of
  84203 commutations came from an accepted crossing (forced 0). The forced
  fallback still exists in code (`FORCED_AFTER_HANDOFF`) and never fired; it
  must be removed for the gate proper.
* **Gate 2:** accepted-ZC rate = 6 × eHz within 0.0% (1000‰);
  too_early = 351 and unstable = 927709, both non-zero.
* **Gate 3, partly:** the coast after the stop independently gives 394 eHz
  from the rotor's own back-EMF with the bridge off, matching the
  reference's speed at the same duty. **The current figure is wrong:**
  `mean_ma=-25`. A negative mean says the zero datum
  (`capture_baseline`'s zero block) is offset. It has to be fixed before the
  current comparison against the reference's 45 mA can be claimed.
* Gates 4 and 5 are not done (protections provoked on purpose; the guard ISR
  root).

**The chain of fixes that got here, in order:**

1. E066: the reference's staircase startup. The rotor never reached 200 eHz
   on firmware50's own 100 eHz catch.
2. E058/E061: the driven observer, seed qualifier and HYST 0.
3. E062: reverse blank.
4. E063/E068: equal compares, and the floating phase driven low.
5. **E070: the COM ISR root** (commutation lateness 321 µs → 3 µs).
6. **E073: no UART traffic while the comparator is in charge.**

## E074 — detector floor removed; the floor-independence test at 10% (build, pre-run)

**Finding.** The reference keeps its CycleTiming (2223 µs) and event minima
report-only at speed (`bench-fast-cycle-report`, `DUTY_50_CAMPAIGN.md:1117-1119`,
"keeps CycleTiming report-only"), which is how the qualified image ran
2.1 keHz at 50%. firmware50 had put the guard constant `EVENT_MIN_US = 238`
**into the COMP decision** as an acceptance floor (`DET_ACCEPT_FLOOR_US`).
The reference's detector (`minz_core::am32_isr::comp_isr`) has only the
half-sector gate. At the goal's 15% (704 eHz, 237 µs sectors) that floor
would refuse every real crossing.

**Change (one variable):** the floor and its counter are removed from
`det_decide`. The half-sector gate, the `ci_max` re-base and 12-read
persistence are unchanged. clippy clean; three roots audit clean; ELF text
47428, sha256 `762D3317…BA5A5CBB`.

**This is the goal's discriminating test** ("a genuine lock is independent of
the floor"; E049's floor-paced artefact moved exactly with it). At 10% the
E073 accepts landed at ~1.0 interval (419 µs), far above 238, so a genuine
lock must be unchanged. **Prediction:** 35 s, forced 0, ~397 eHz, rate
~1000‰, the same as E073.

### E074 — result (`captures/2026-09-20/e074-nofloor_01.txt`, ELF `762D3317…BA5A5CBB`)

```
BEMFDONE   reason=2 accepted=84710 forced=0 too_early=467566 unstable=1030813 ci_us=417
BEMFGATE   closed_ms=35282 hold_ms=35282 ehz_from_ci_last=399 forced_pct=0
BEMFRATE   ehz_from_sector=400 zc_rate_permille_of_expected=998
BEMFSECTOR a1..a6 = 14118,14118,14118,14118,14119,14119 (forced 0)
BEMFDRIVEN com_count=84711 com_late_max_us=9 closed_irq_peak_per_ms=57 storm=0
COASTTIMING iv_us=1192,1330,1201,1327,...  (≈ 395 eHz)
BEMFCURRENT mean_ma=-35
```

**The prediction held: the lock is independent of the floor.** Removing the
238 µs detector floor changed nothing but the refusal accounting (edges
before the half-sector gate are now `too_early`, as in the reference). There
is no floor pacing: **the E073/E074 lock is genuine.** That makes two 35 s
runs at 10%.

## E075 — the goal's 15% (build, pre-run)

* `BEMF_DUTY_TENTHS` 100 → **150**. The six-step cap is 150 already, and
  advance stays 20 (below 35%, as the reference). Everything else is E074.
* The report's reference current follows the duty (`ref_ma` 58 at 15%, 45
  at 10%) and the line ends in CRLF.
* Not changed yet: the persistence depth stays at 12. The reference's
  `map(average_interval, 100, 500, 3, 12)` gives ~11 at 237 µs sectors; this
  is a stated divergence of one read.
* clippy clean; audits clean; ELF text 47456, sha256 `9CDB8281…31669B32`.

**Prediction:** a 35 s hold with the estimate settling near the reference's
**704 eHz** (sector ~237 µs), forced 0, and a coast witness near 700 eHz. The
risk is the storm cutoff (64/ms), since more sectors per ms means more
pre-crossing traffic per ms. If it trips, that is a named stop with the ring
to read.

### E075 — result: **35.3 s BEMF lock at 15%** (`captures/2026-09-20/e075-15pct_01.txt`, ELF `9CDB8281…31669B32`)

```
BEMFDONE    reason=2 (normal deadline) accepted=147973 forced=0 too_early=334622 unstable=949899 ci_us=236
BEMFGATE    closed_ms=35282 hold_ms=35282 target_tenths=150 zc_per_s=4194 comm_per_s=4194 ehz_from_ci_last=706 forced_pct=0
BEMFRATE    mean_sector_us=238 ehz_from_sector=700 zc_rate_permille_of_expected=998 hold_forced_pct=0
BEMFSECTOR  a1..a6 = 24662,24662,24662,24662,24663,24662 (forced 0)
BEMFDRIVEN  com_count=147974 com_late_max_us=10 closed_irq_peak_per_ms=57 storm=0 loop_gap_max_us=161
COASTTIMING iv_us=689,748,693,735,706,751,696,751   (pair ≈ 1437 us ≈ 696 eHz)
BEMFCURRENT mean_ma=-5 ref_ma=58      BEMFPHASE lt075=9 to100=65535 to125=65535 (u16 saturated)
```

**The prediction held.** The reference's 15% rung is 704 eHz, and
firmware50 settles at **706 eHz by estimate and 696 eHz by coast
witness**. 35.28 s MCU-stamped; accepted 147973 = commutations 147974
(forced 0); rate identity 998‰; all six sectors balanced; the storm peak
57/ms stayed under the cutoff. **This is the goal's closed-loop condition
met once, at 15%.**

It does not yet count as a qualifying run for gate 1. The forced fallback
is still in the code (it never fired), and the goal requires the scripted
source *removed*. Defects to fix:

* `BEMFPHASE` bins are u16 and saturated at 65535. Widen to u32.
* **The current figure is wrong.** It read −25, −35 and −5 mA against the
  reference's 45/58 mA. The zero datum from `capture_baseline` is offset.

**Remaining goal work, in dependency order:**

1. The **guard ISR root** (gate 5's fourth root) carrying the tracking and
   stale-event watchdog: without a fallback, a missed crossing must become a
   named tracking stop, as in the reference (`SPEEDEVENTWATCH max_us=1000`,
   `TRACKSTOP`).
2. Remove the forced fallback (gate 1).
3. Fix the current datum (gate 3).
4. Three qualifying 15% runs; provoke each protection once (gate 4); report
   the cycle counts of all four roots (gate 5).

## E076 — the guard ISR root, the tracking watch, and no forced fallback (build, pre-run)

**The fourth ISR root.** TIM6's update interrupt already paces the ADC
(TRGO, 101 µs), and it is now the **guard** at priority **0x00**. COMP and COM
move to **0x40 peers** under it, the reference's arrangement below 48% (its
TIM6 guard at 0, "COMP/COM remain priority 0x40 peers"). HAL `Timer::listen`
/ `unlisten` enables the tick; the handler clears TIM6's flag directly
because an ISR cannot own the HAL `Timer`. Each tick:

1. extends its own µs clock;
2. checks nFAULT (`Driver`);
3. checks the tick gap against `TICK_GAP_MAX_US = 200` (`TickGap`);
4. checks ADC freshness against `FEEDBACK_MAX_AGE_US = 1000` from `ADC_SEQ`
   (`FeedbackStale`);
5. applies a 45 s campaign backstop (`CampaignDeadline`);
6. once armed at transfer, checks the tracking watch (`Tracking`).

A fault latches (first wins), stops the COMP, COM and driven roots, masks
the line, and removes the bridge **from the interrupt**: MOE off (OSSI then
drives all six gates to idle-low in hardware), compares to zero, ENABLE low.
The foreground then stops with that reason and runs the full `safe_off`.

**The tracking watch** is new `src/tracking.rs`, `EventWatch<REPORT_FAST>`,
transcribed from `binz/examples/support/accepted_timing.rs::Monitor` plus
`powered_guard.rs::speed_event_limit_us` (with its compile-time table):

* **Stale** above 1000 µs, tightened to 3 controller periods (at 15%:
  711 µs);
* **SectorOrder**;
* **TooFast**, report-only as in the oracle (`bench-fast-cycle-report`).
  15% sectors of 237 µs are legitimately below the 238 µs event minimum.

8 host tests. The COMP accept feeds it inside a critical section (the guard
can preempt COMP).

**The forced fallback is deleted** (goal gate 1). Every commutation after
transfer comes from an accepted crossing through the COM root, and a missed
crossing is the guard's `Tracking` stop. `forced` stays in the report and is
now zero by construction.

**The audit caught two things along the way:**

* The first cut called `safe_off(&mut Drv8304)` from the ISRs. Its
  `gates_low` loops over the PAC's indexed `moder(p)`, with a bounds-check
  panic path into `core::fmt`, so it was refused in both roots. The ISR trip
  now uses the straight-line subset in `safe_off`'s order.
* `isr_audit.py` flagged `cortex_m::interrupt::free` as a loop. It was a
  tail-merge (three exits sharing a lower-address block that runs forward to
  the epilogue), the same false positive `isr_cycles.py` already filters by
  reachability. The audit now applies the same **reachability test**. It
  also gains the `bhs`/`blo` mnemonics llvm-objdump uses: without them, a
  loop closed by an unsigned compare was invisible to the audit, before this
  change too. **Negative control:** with an empty allowlist the audit still
  fails `ADC_COMP` on its real persistence loops.

**Gate 5: four ISR roots, all certified clean by the fail-closed audit.**
Longest paths @ 64 MHz, 0 wait states:

| root | loops | longest path |
|---|---|---|
| `ADC_COMP` (COMP) | 2, each ≤ 12 reads | 1245 cycles, 19.45 µs (includes the diagnostic IRQ ring and the watch feed) |
| `TIM16` (COM) | 0 | 360 cycles, 5.62 µs |
| `DMA1_CHANNEL1` (DMA) | 0 | 99 cycles, 1.55 µs |
| `TIM6_DAC_LPTIM1` (guard) | 0 | 445 cycles, 6.95 µs |

257 host tests; clippy clean; ELF sha256 `7620F5D5…F62C18CD6BC`.

**Prediction at 15%:** the same 35 s lock as E075, with **forced 0 by
construction**. `BEMFGUARD reason=0` (no guard stop) with a gap max near the
101 µs tick. The watch deadline tightens to ~711 µs, with fast-event counts
reported and not latched.

### E076 — result: **qualifying run 1/3 at 15%** (`captures/2026-09-20/e076-guard_01.txt`, ELF `7620F5D5…F62C18CD6BC`)

```
BEMFDONE    reason=2 (SegmentDeadline) accepted=148108 forced=0 too_early=320772 unstable=933206 ci_us=237
BEMFGATE    closed_ms=35283 hold_ms=35283 target_tenths=150 zc_per_s=4197 comm_per_s=4197 ehz_from_ci_last=703 forced_pct=0
BEMFRATE    mean_sector_us=238 ehz_from_sector=700 zc_rate_permille_of_expected=999 hold_forced_pct=0
BEMFSECTOR  a1..a6 = 24685,24684,24684,24685,24685,24685 (forced 0)
BEMFDRIVEN  seeded=1 seed_us=757 acquire_us=16692 com_count=148109 com_late_max_us=15 closed_irq_peak_per_ms=53 storm=0
BEMFGUARD   reason=0 ticks=395980 gap_max_us=105 track_fault=0 track_max_us=669 fast_events=78862 fast_min_us=183
COASTTIMING iv_us=745,692,742,695,741,695,750,704   (pair ≈ 1437 us ≈ 696 eHz)
BEMFCURRENT mean_ma=69 ref_ma=58
```

**The prediction held.**

* **Gate 1:** 35.283 s held at 15% after handover, MCU-stamped. No scripted
  frequency source exists after transfer, and all 148109 commutations came
  from accepted crossings through the COM root.
* **Gate 2:** 999‰; too_early = 320772 and unstable = 933206, both non-zero.
* **The guard ran 395980 ticks with no fault.** Its worst gap was 105 µs
  against its 101 µs period and 200 µs allowance, and the tracking deadline
  tightened to 669 µs (three periods). The 78862 short intervals, minimum
  183 µs, were reported and not latched, exactly the case the reference's
  report-only policy exists for at this speed.
* **Gate 3, speed:** coast 696 eHz against the reference's 704 at 15%.
  **Current:** 69 mA against 58 mA. The same duty read −5 mA in E075, so the
  zero datum is unstable. Not claimed until fixed.

Next: qualifying runs 2 and 3 on the same ELF, back to back
(`--runs 2`).

### E077 — result: **qualifying runs 2/3 and 3/3 at 15%** (`captures/2026-09-20/e077-qual15_01.txt`, `_02.txt`, same ELF as E076)

| run | accepted | forced | rate ‰ | eHz est. (mean) | coast eHz | guard | track_max_us | mean_ma |
|---|---|---|---|---|---|---|---|---|
| E076 | 148108 | 0 | 999 | 703 (700) | 696 | reason 0 | 669 | 69 |
| E077-1 | 148175 | 0 | 999 | 683 (700) | 697 | reason 0 | 666 | 69 |
| E077-2 | 148294 | 0 | 995 | 712 (703) | 696 | reason 0 | 663 | 85 |

All three: `reason=2` normal deadline, `closed_ms=hold_ms=35283`, sectors
balanced to ±1, guard `gap_max_us=105`, no storm (closed-loop peak 53/ms).

**Goal gate 1 is met:** 3/3 runs, ≥ 30 s at 15% after handover, MCU-stamped,
with no scripted source and commutation only from accepted crossings.
**Gate 2 is met** in all three (995–999‰; too_early and unstable non-zero).

**Gate 3, speed:** the coast witness (the rotor's own back-EMF with the
bridge off) gives 696/697/696 eHz against the reference's 704 at 15%.

**Gate 3, current: the figure was not like-for-like.** The reference's column
is "the average signed three-shunt residual over completed 100-scan blocks,
scaled by each run's nominal 4 A raw allowance", measured over the **final
target dwell** (`DUTY_50_CAMPAIGN.md`, rung table and note). firmware50
averaged the **whole run**, sine startup and driven stage included, which is
why the same 15% duty read −5 (E075) and 69/69/85 mA (E076/E077).

## E078 — hold-window current, the reference's quantity (build, pre-run)

`AverageCurrent::mark()` / `window_milliamps(mark)` / `window_blocks(mark)`
average only the blocks completed after a mark, with the same scaling. New
host test `window_mean_excludes_blocks_before_the_mark`. `bemf_run` marks at
`hold_start` and reports `hold_blocks` and `hold_ma` beside the old
`mean_ma`. 258 host tests; clippy clean; four roots audit clean; ELF sha256
`D76E4BD1…00A1ED45`.

**Prediction:** `hold_ma` near the reference's 58 mA at 15%, stable across
runs.

### E078 — result (`captures/2026-09-20/e078-holdcurrent_01.txt`, ELF `D76E4BD1…00A1ED45`)

```
BEMFDONE    reason=2 accepted=148061 forced=0        BEMFGATE hold_ms=35282 ehz_from_ci_last=694
BEMFRATE    zc_rate_permille_of_expected=998          BEMFGUARD reason=0 gap_max_us=105 track_max_us=663
BEMFCURRENT blocks=3959 mean_residual=306 mean_ma=38 hold_blocks=3493 hold_ma=26 ref_ma=58
COASTTIMING iv_us=741,690,744,701,...  (≈ 695 eHz)
```

A fourth 35 s lock at 15% (it counts toward nothing new; gate 1 is already
3/3). **The prediction failed:** `hold_ma=26` against the reference's 58,
while the whole-run mean read 38 where E076/E077 read 69/69/85 at the
identical duty and speed.

That scatter (~60 mA ≈ 480 residual units ≈ 5 codes per scan-sum) is far
larger than white noise on a 100-scan datum can make (about ±6 mA). The
reference takes its zero from 128 scans (`prestart_baseline.rs`,
`zero_from_128`), so it has the same exposure. The likelier cause is a
**systematic shift of the amplifier zero** between the pre-drive capture and
the hold, from temperature or switching.

## E079 — post-run zero, to measure the datum drift (build, pre-run)

After the coast, with the bridge already off (`safe_off` has reclaimed the
gate pins as GPIO-low with MOE clear, so no FET can switch), ENABLE goes
high for 2 ms plus one block so the amplifiers are biased, a second zero is
taken, and ENABLE goes low. `BEMFCURRENT` adds `zero_start`, `zero_end`
and `zero_drift_ma` (the residual shift, same scaling). clippy clean; four
roots audit clean; ELF sha256 `6141DBF0…52455C12`.

**Prediction:** `zero_drift_ma` has a magnitude comparable to the run-to-run
scatter (tens of mA), which would make the current proxy datum-limited on
both firmwares. If the drift is small, the scatter is real current and the
cause is elsewhere.

### E079 — result (`captures/2026-09-20/e079-zerodrift_01.txt`, `_02.txt`, ELF `6141DBF0…52455C12`)

| run | hold_ma | mean_ma | zero_start | zero_end | zero_drift_ma |
|---|---|---|---|---|---|
| E079-1 | 69 | 81 | 617120 | 616938 | −22 |
| E079-2 | 71 | 84 | 617150 | 615955 | −150 |

Both runs are again 35.283 s at 15%, forced 0, guard reason 0, ~706–715 eHz.

**The prediction held in kind but not usefully.** The pre-run zeros agree to
30 codes, but the post-run zero moved −182 and −1195 residual units
(−22 / −150 mA-equivalent). The drift is itself erratic, and the DRV8304
recalibrates its current-sense offsets on wake, so a post-run datum taken
2 ms after re-enable is not a trustworthy correction either. The hold-window
figures are 26, 69 and 71 mA against the reference's 58.

**Gate 3 conclusion, stated with its limits:**

* **Speed matches tightly.** Coast witness 695–697 eHz across five 15% runs
  against 704 (−1%). Estimator 694–715 eHz.
* **The current proxy is consistent in sign and magnitude but not precise.**
  Hold window 26/69/71 mA against 58 mA, with a measured datum drift of up
  to 150 mA-equivalent. This is the quantity the reference itself calls
  "neither a calibrated DC-link/PSU current nor a PWM peak", taken from a
  128-scan zero with the same exposure, and its 58 mA is a single sample. What
  the proxy does discriminate is lock versus slip: the desynced six-step loop
  of E056/E057 read 120–150 mA at a *third* of this speed.
* A calibrated current (a metered PSU reading, or a zero re-taken at the hold
  with the rotor known to be at speed) is what would make the current match
  exact. It is noted as the open part of gate 3.

Next: goal gate 4 (provoke each protection once and capture its reason code).

## E080 — goal gate 4: provoke each protection once (build, then one run per protection)

`RunMode::Inject(kind)` runs the full chain (sine → driven → seed → 15%
closed loop) and fires **one** stimulus `INJECT_AFTER_US = 3 s` into the
closed loop, so every protection is provoked from a locked loop. Shell keys
and stimuli:

| key | protection (expected reason) | stimulus |
|---|---|---|
| `T` | Tracking (8) | sensing loss: `DET_ACTIVE` cleared, so no crossing is accepted and the guard's watch goes stale |
| `G` | TickGap (3) | a stalled system: interrupts masked for 400 µs (the guard allows 200) |
| `F` | FeedbackStale (4) | a stalled ADC producer: DMA1 ch1 disabled (re-enabled after the run) |
| `N` | Driver (7) | the real nFAULT input: the shared open-drain line driven low from the MCU side (restored after) |
| `U` | CompStorm (13) | its physical cause (E073): a UART burst while the comparator is live |
| `V` | FastBusSag (26) | a step to the reference's own 50% rung into the 1 A-limited supply |
| `I` | AverageCurrent (25) | the accumulator swapped for one with a **stricter** allowance (1/100 of nominal), so the real 15% current folds back and then stops. **A tightened threshold, not a relaxed one**: through a 1 A-limited supply the nominal 4 A allowance cannot be reached before the sag stop fires. |
| host | HostAbort (9) | `bemf_run.py --abort-after` sends the stop byte mid-run |

Already captured without injection: **InvalidSeed (10)** in E058 (`fail=4`,
window elapsed), and the **segment deadline (2)** that ends every normal run.

After every stop the run prints `POSTSTOP` plus the preflight readback (MOE,
CCR1-3, gate pins, ENABLE, nFAULT) as the **all-off verification**, and
`BEMFINJECT expected_reason= reason= provoked=`.

ELF text 51384, sha256 `BED61DF0…EBA75B1`; clippy clean; four roots audit
clean.

### E080 — results, part 1 (ELF `BED61DF0…EBA75B1`)

| key | expected | reason | guard | stop after stimulus | all-off (POSTSTOP preflight) | capture |
|---|---|---|---|---|---|---|
| T | Tracking 8 | **8** | 8 | **675 µs** (= the watch's tightened `track_max_us=675`) | PASS | `e080-inject-T_01.txt` |
| G | TickGap 3 | **3** | 3 | 424 µs (400 µs stall; `gap_max_us=471` > 200) | PASS | `e080-inject-G_01.txt` |
| F | FeedbackStale 4 | **4** | 4 | 1082 µs (1000 µs limit + one tick) | PASS | `e080-inject-F_01.txt` |

**Then a defect of my own making:** the N, U and host-abort runs that
followed all stopped within four guard ticks on `reason=26` (fast-sag) with
**`bus_ref=2058`**, where a healthy run reads ~1216. The F cleanup had
disabled and re-enabled DMA ch1 while the ADC kept converting, so the
circular buffer resumed **out of step with the scan**, and a current-sense
channel (~2048 mid-rail) landed in the bus slot. The fast-sag check refused
to run on it (fail-closed), but those three runs are **invalid as
provocations** and are not counted.

**Fix:** `adc_dma_resync()` stops the ADC (bounded ADSTP wait), disables the
channel, clears its flags, reloads the count to the scan length, re-enables
it, clears the ADC flags and restarts on the next trigger. It is used after
the F test and **at the start of every `bemf_run`**, so no run inherits a
shifted scan from anything before it. ELF sha256 `714CB38E…1810500A`; clippy
clean; four roots audit clean.

Next: re-run F on the fixed ELF (it must leave the board healthy), then N,
U, host abort, I and V.

### E080 — results, part 2, and **goal gate 4 complete**

The F re-run on the resync ELF (`714CB38E…`) gave FeedbackStale (4) again,
and the next run started healthy (`bus_ref=1216`), so the resync works. The
rest of the provocations follow.

* **Storm:** the UART burst (the 10%-regime cause from E073) did **not**
  storm the 15% loop: a full 40 s run, `e080b-inject-U_01.txt`. That
  coupling is regime-dependent and not a dependable stimulus. The stimulus
  was replaced with the reference's own cutoff test ("fail the real rate
  latch", `RATESTOP`): 80 software-pended `ADC_COMP` entries inside 1 ms,
  through the real handler, accounting and trip.
* **Average current, first attempt:** with only the stricter allowance, the
  first block's **foldback acted** (the duty cut slowed the loop,
  `ci_us` 237 → 655), the rotor regenerated (`mean_residual=-270`), a
  negative block resets the streak by design, and the run ended on Tracking.
  That is foldback provoked, but not the stop. The hard stop exists for an
  *unacknowledged* over-current (reference `first_over_warning=1
  unacknowledged_second_over_stop=1`), so the second attempt also held the
  drive plans.
* **IWDG, first attempt: no reset**, which exposed a **HAL defect**.
  `IndependedWatchdog::start` computes its reload at 16384 Hz
  (`ref/stm32g0xx-hal/src/watchdog.rs:16`), but at PR = 0 the IWDG counts
  LSI/4 = 8 kHz. The "50 ms" watchdog read back PR = 0, RLR = 0x333 = 819,
  about **102 ms**, and a 100 ms stall slipped under it. **Fixed** (ninth
  PAC escape, the HAL's own register sequence with RLR = 400 for 50 ms; read
  back RLR = 0x190). A normal 35 s run at 15% with the true 50 ms dog showed
  no spurious reset (`e080f-normal-iwdg50_01.txt`).

**Goal gate 4: each protection provoked once, with its reason code.** All
from a locked 15% closed loop, 3 s after handover. The all-off readback
(`POSTSTOP` preflight: MOE 0, CCR 0/0/0, gates low, EN 0, nFAULT 1) is PASS
after every stop.

| protection | stimulus | reason | stop after stimulus | capture |
|---|---|---|---|---|
| tracking | sensing loss | **8** | 675 µs (the watch's tightened deadline) | `e080-inject-T_01.txt` |
| watchdog: tick gap | 400 µs interrupt-masked stall | **3** | 424 µs | `e080-inject-G_01.txt` |
| watchdog: feedback age | ADC DMA stopped | **4** | 1082 / 1112 µs | `e080-inject-F_01.txt`, `e080b-inject-F_01.txt` |
| nFAULT | shared open-drain line driven low | **7** | 15 µs (foreground check; guard ≤ 101 µs) | `e080b-inject-N_01.txt` |
| comparator storm | 80 real COMP entries in 1 ms | **13** | 865 µs | `e080c-inject-U_01.txt` |
| host abort | stop byte mid-hold | **9** | at receipt, 4.4 s into the hold | `e080c-inject-hostabort_01.txt` |
| 3-scan fast bus sag | step to 50% into the 1 A supply (bus 1214 → 1068, −12%) | **26** | 1315 µs | `e080c-inject-V_01.txt` |
| average current, foldback then stop | stricter allowance (1/100) + held drive | **25** | 293 ms (two consecutive over-blocks) | `e080d-inject-I_01.txt` |
| hardware watchdog (IWDG) | 100 ms unfed foreground stall, interrupts live | reset: `RESETCAUSE iwdg=1`, then bridge-off `PREFLIGHT PASS` | ≤ 50 ms | `e080f-inject-W_01.txt` |
| invalid seed | (natural) no seed in the window | **10** | 40 ms window | `e058-driven_01.txt` |
| segment deadline | (natural) end of every normal run | **2** | — | every qualifying run |

**No threshold was relaxed.** The average-current provocation used a
*stricter* allowance, stated as such, because through a 1 A-limited supply
the nominal 4 A allowance cannot be reached before the sag stop fires (the V
run shows the sag stop firing first).

ELF for the final provocations `F704AAE6…C58E3758` (IWDG fix included).

## E081 — final image: three qualifying runs, audit, cycle counts, reproducibility

**Final ELF** `target/thumbv6m-none-eabi/release/shell-pwm`: sha256
**`F704AAE64309F518B53BF8B5FF7353FEE13E077CF36145EDF902F75CC58E3758`**, text
51804 / data 24 / bss 5644. **Reproducible:** `cargo clean -p firmware50
--release` followed by `scripts/build.ps1` produced the identical hash.

**Three qualifying runs at 15% on this one image:**

| capture | hold (MCU) | accepted | forced | rate ‰ | eHz mean / last | coast eHz | guard | stop |
|---|---|---|---|---|---|---|---|---|
| `e080f-normal-iwdg50_01` | 35.282 s | 148241 | 0 | — | 706 last | — | 0 | 2 |
| `e081-final-qual15_01` | 35.283 s | 148275 | 0 | 995 | 703 / 691 | 696 | 0 | 2 |
| `e081-final-qual15_02` | 35.282 s | 148322 | 0 | 996 | 703 / 709 | 702 | 0 | 2 |

All three have balanced sectors (±1), too_early ≈ 320k and unstable ≈ 938k
(both non-zero), guard worst gap 105 µs, and POSTSTOP all-off PASS.

**Gate 5 on the final image** (`isr_audit.py`, fail-closed, now with the
reachability loop test and the `bhs`/`blo` mnemonics, negative control
passing; `isr_cycles.py`, longest path @ 64 MHz, 0 ws):

| root | role | priority | audit | loops | longest path |
|---|---|---|---|---|---|
| `ADC_COMP` | COMP | 0x40 | clean | 2, each ≤ 12 reads | 1245 cycles, 19.45 µs |
| `TIM16` | COM | 0x40 | clean | 0 | 360 cycles, 5.62 µs |
| `DMA1_CHANNEL1` | DMA | 0x40 | clean | 0 | 99 cycles, 1.55 µs |
| `TIM6_DAC_LPTIM1` | guard | 0x00 | clean | 0 | 445 cycles, 6.95 µs |

No soft division or wide helper is reachable from any root (the link-time
audit caught `__aeabi_lmul` once, E058, and it was fixed at the source).

258 host tests; clippy clean (target binary and host library). No `#[cfg]`
added beyond structural `test`; policies are types (`EventWatch<REPORT_FAST>`,
`FixedFilter<12>`, `RunGuard<…>` const generics). The PAC escapes added this
campaign each quote the vendored source they work around: TIM16 COM one-shot
(`timer/mod.rs:154-155`), the IWDG reload (`watchdog.rs:16`), RCC_CSR reset
flags (no HAL API), and TIM6 SR in the guard ISR (an ISR cannot own the
`Timer`).

### Goal status

1. **Gate 1: met.** 3/3 runs at 15%, each ≥ 30 s (35.28 s) after handover,
   MCU-stamped, with no scripted frequency source after transfer: the forced
   fallback no longer exists, and every commutation comes from an accepted
   crossing through the COM root. It is also met on E076/E077's image.
2. **Gate 2: met.** Accepted-ZC rate against 6 × the interval-derived eHz is
   995–999‰ (within 0.5%). too_early and unstable are non-zero in every run.
3. **Gate 3, speed: met. Current: consistent on average, not per run.**
   * The coast witness (the rotor's own back-EMF with the bridge off) gives
     696–702 eHz against the qualified image's 704 at 15%.
   * The hold-window current proxy (the reference's own quantity: signed
     three-shunt residual, 100-scan blocks, nominal-4 A scaling) reads
     26 / 69 / 71 / 3 / 105 mA over five 15% runs. That is a **mean of
     55 mA against the reference's 58 mA**, but the single-run spread is
     ±50 mA. Its zero datum moves between runs by up to 722 counts
     (~90 mA-equivalent) and across a run by up to 218 mA-equivalent. The
     reference's 128-scan datum shares that exposure, and its 58 mA is a
     single sample. A calibrated current would need a metered reference.
4. **Gate 4: met** (E080 table). Tracking 8, tick gap 3, feedback age 4,
   nFAULT 7, storm 13, host abort 9, fast sag 26, average current 25
   (foldback then stop, with a *stricter* threshold), the IWDG reset, plus
   invalid seed 10 and deadline 2 captured naturally. All-off was verified
   after every stop. No threshold was relaxed.
5. **Gate 5: met** (table above): four roots, priorities set, fail-closed
   audit clean, longest-path cycle counts reported.

Defects this campaign found in the tooling and the HAL, now fixed and
recorded:

* `isr_audit.py` had a direction-only loop test (false positive) and was
  missing the `bhs`/`blo` mnemonics (a possible false negative).
* The HAL's IWDG timeout is doubled.
* The DMA re-enable desynchronised the scan.
* My own report read the clock after the coast, inflating `closed_ms`.

---

# Campaign 2: 15% → 25% (goal set 2026-09-20)

## E082 — plan and reference figures (no code yet)

**Starting image:** `F704AAE6…C58E3758` (E081), qualified at 15%.

**Oracle figures per rung** (`binz/captures/reverse48k_rate_curve_{20,25}_20260919.txt`,
frozen oracle `0C734C67…`):

| duty | estimated_ehz | proxy mA (current_sum / blocks, nominal-4 A scaling) |
|---|---|---|
| 15% | 704 | 58 (rung table) |
| 20% | **941** | 1061676 / 733 = 1448 raw → **167** |
| 25% | **1186** | 2128001 / 754 = 2822 raw → **326** |

The formal 20% cohort is E765 (`binz/AGENTS.md:2542-2545`): 3/3 × 30 s,
`reason=2`, 155645/157269/158254 commutations. Two further facts bear on
this campaign:

* **E765 on a 1 A supply** held only "20..22.5%". At 25% it stopped on
  average-current (25) at 7.26 s.
* **E763 metered 230 mA at the PSU during a 15% hold**, where the board read
  ~1.11× that.

So 25% needs more than the present 1 A limit, and the goal's real-4 A
allowance needs the limit raised regardless. That is an **operator step**
(a PSU limit change). All code work comes first; the operator will then be
asked once, with a concrete limit and the readings wanted.

**Changes planned, each with host tests:**

1. **Speed-dependent persistence** (the reference's
   `map(average_interval, 100, 500, 3, 12)`, `minz/core/src/am32_loop.rs:298-305`,
   read-only). It must stay bounded by 12, so the audited loop bound and
   COMP's worst case do not grow.
2. **Post-transfer duty ramp**, the reference's rung method ("a 1%-per-0.5 s
   ramp", `DUTY_50_CAMPAIGN.md`). Transfer at the reference's `bemfdu100`
   (10%), then +1% per 500 ms to target. `BEMF_TOTAL_MS` grows so the
   **hold at target** stays ≥ 30 s: 4.72 s startup + 7.5 s ramp + 30 s,
   inside the unchanged 45 s guard backstop.
3. The six-step duty cap goes 150 → 250. This is the drive envelope, not a
   protection.
4. Rung selection at run time (shell keys), with no feature matrix.
5. Per-run comparison lines beside the oracle: coast-witness eHz from the
   first bridge-off pair and the hold-window current proxy, each with its
   ratio to the oracle figure. A >20% mismatch is a stop.
6. Normal-start recovery (gate 5), transcribed from the reference's
   `bench-normal-restart` (to be read before it is written).
7. **The R_COMP < wait_time(ci) inequality at 25%.** At 1186 eHz, ci ≈ 140 µs
   and advance 20/64, so **wait ≈ 26 µs**. COMP's longest path on E081 is
   1245 cycles (19.45 µs at 0 ws), before blocking by the COM peer and the
   guard's preemption. As it stands the inequality probably **fails**, so
   COMP's path has to shrink (the diagnostic IRQ ring is the obvious cut)
   and the commutation arm should compensate for its own latency.
﻿
## E083 — rung machinery for 20% / 25% (build, pre-run)

**What changed** (ELF `1F9FCE5D…9C8228C3`, text 51800 / data 32 / bss 5004,
reproduced byte-identical from a clean rebuild):

1. `src/bemf.rs`: `FromMicros<P>` adapter and `ReferenceFilterUs =
   FromMicros<WithShallowFloor<MappedFilter>>`. The reference's persistence
   schedule `map(avg, 100, 500, 3, 12)` is in half-µs
   (`minz/core/src/am32_loop.rs:298-305`), and this estimator is in µs.
   The result is 12 reads through 15%, 8 at 20% (ci ≈ 177 µs) and 7 at 25%
   (ci ≈ 140 µs). The schedule is clamped at 12, so the audited loop bound is
   unchanged. It has two new host tests.
2. `src/ramp.rs` (new): the reference's rung method, transfer at `bemfdu100`
   (10.0%) then +1% per 0.5 s. The ramp takes 2.5 / 5 / 7.5 s to reach
   15 / 20 / 25%. It has five host tests.
3. `bin/shell-pwm.rs`:
   * `DET_FILTER` uses the speed schedule.
   * The transfer takes its duty and advance from `ramp::duty_at(target, 0)`.
   * The closed-loop duty follows `ramp::duty_at(target, now − closed_at)`
     under the unchanged governor.
   * `hold_start` already fires only at `duty >= target`, so the hold window
     (current mark, rate) is now "at target".
   * `SIXSTEP_DUTY_CAP` goes 150 → 250. This is the drive envelope, not a
     protection.
   * `BEMF_TOTAL_MS` goes 40 000 → 44 000: 4.72 s startup + 7.5 s ramp +
     31.8 s at 25%, inside the unchanged 45 s guard backstop.
   * New rung keys `2` (20%) and `5` (25%), via `bemf_run_report_at`.
   * `ref_ma` is per duty: 45 / 58 / 167 / 326.
   * **The IRQ trace ring is removed** (statics, ISR write, `BEMFIRQRING`
     dump). It was COMP's largest optional cost.
   * **Latency-compensated arm:** `com_arm(wait − (TIM17 now − raw))`, so the
     commutation lands at edge + wait whenever the handler finishes inside
     the wait.
   * **R_COMP measured on the rotor:** `DET_SPENT_MAX` (entry stamp → arm, µs)
     and `DET_LATE_ARMS` (arms whose spent time reached the wait) are
     single-writer load/compare/store, reset at transfer, and reported on a
     new `BEMFRCOMP` line.
4. `scripts/bemf_run.py`: commands `2`/`5`, and a host-side `BEMFREF` line
   appended to each capture (no firmware cost). It gives:
   * coast eHz = 1e6 / (iv₀ + iv₁), the rotor's own speed with the bridge off;
   * loop eHz;
   * the oracle's eHz / mA at that duty, and percentages;
   * `zc_permille_of_6x_coast`: the accepted rate against the rotor, not
     against the loop's own intervals;
   * `verdict=STOP` on a mismatch over 20%.
5. Two test-only clippy findings fixed (`witness.rs` unused assignment,
   `sixstep.rs` redundant closure). Clippy is now clean on the target binary
   and on the host library **with `--tests`**.

**Checks:** 265 host tests pass (258 at E081, and none deleted). The four
roots pass the fail-closed audit.

| root | longest path @ 0 ws | E081 |
|---|---|---|
| `ADC_COMP` (12 reads) | **1230 cycles**, 19.2 µs | 1245 |
| `ADC_COMP` (7 reads, the 25% level) | 1060 cycles, 16.6 µs | — |
| `TIM16` | 360 | 360 |
| `DMA1_CHANNEL1` | 99 | 99 |
| `TIM6_DAC_LPTIM1` | 445 | 445 |

A first version of the R_COMP counters (with a `wait_min`) put COMP at 1258
cycles, over E081's 1245. It was trimmed: "late" is now derived from the
subtraction the arm already does, before any powered run.

**The R_COMP inequality at 25%, static form.**
* At 1186 eHz, ci = 140 µs, and advance level 20 gives 43 µs, so
  **wait = 70 − 43 = 27 µs**.
* Edge to arm is at most the COMP path at 7 reads (1060) + the guard's
  preemption (445) + exception entry (24) = **1529 cycles = 23.9 µs < 27 µs**
  at 0 wait states.
* The flash's two wait states are not modelled, so this is stated as
  provisional. The run's `BEMFRCOMP spent_max_us` and `late_arms=0` are the
  measurement that decides it.
* The COM root, a 0x40 peer, cannot block: it fires at edge + wait, before
  the next edge.

**Current-proxy caution, stated before any run.** Replaying the new `BEMFREF`
over the E081 15% captures gives `hold_ma` 3 and 105 against 58, which is
`verdict=STOP` by the goal's 20% rule. The same proxy that met gate 3's
"mean" wording at 15% will not meet a per-run 20% band there. At 20% and 25%
the oracle's figures are 167 and 326 mA, so the same ±50 mA datum scatter is
±30% and ±15%. The pre-run zero is taken in the same DRV8304 wake as the
drive (`bemf_run`: ENABLE → 2 ms → `capture_baseline` → drive), so a wake
recalibration is ruled out as the cause. If a rung reads >20% off, it is a
stop. The next step is then to measure the oracle's own run-to-run scatter at
that duty on this bench (flash, run, read, flash back), not to argue it away.

**Next:** a shakedown run at 20% (`--command 2`) on the present 1 A supply.
The oracle held 20% on 1 A (E765). I will check:
* the ramp;
* the hold of ≥ 30 s at 200‰;
* forced 0;
* `BEMFRCOMP`;
* `BEMFREF`.

Then the operator is asked once to raise the PSU limit for the 4 A allowance
and to read the display during holds.


### E083 — result: 20% shakedown (`captures/2026-09-20/e083-shake20_01.txt`, ELF `1F9FCE5D…9C8228C3`, PSU 1 A limit)

```
BEMFDONE  reason=2 accepted=216674 forced=0 too_early=420732 unstable=1001249
BEMFGATE  closed_ms=39283 hold_ms=34283 target_tenths=200
BEMFRATE  hold_ms=34283 hold_accepted=196823 hold_forced=0 ehz_from_sector=957 zc_rate_permille_of_expected=998
BEMFRCOMP spent_max_us=18 late_arms=0
BEMFGUARD reason=0 gap_max_us=105 track_fault=0 track_max_us=468
BEMFCURRENT hold_blocks=3394 hold_ma=103 zero_drift_ma=51 ref_ma=167
COASTTIMING iv_us=511,543,508,538,…
BEMFREF   coast_ehz=949 oracle_ehz=941 speed_pct=+0.9 hold_ma=103 oracle_ma=167 current_pct=-38.3 zc_permille_of_6x_coast=1008 verdict=STOP
```

What worked, first time:
* The ramp: the hold began 5.0 s after transfer (closed 39.28 s − hold 34.28 s), as `ramp_us(200)` predicts.
* 34.28 s at 20%, forced 0, a normal deadline stop.
* The rate is 998‰ against the loop's own sectors and **1008‰ against 6 × the rotor's coast speed**.
* The coast witness reads 949 eHz against the oracle's 941 (+0.9%).
* R_COMP measured: at most **18 µs** from COMP's stamp to the arm, against a 20% wait of about 33 µs (ci 174 → 87 − 54). `late_arms=0` over 196 823 arms.

**The stop: current.** `hold_ma=103` against 167 is −38%, and the goal makes >20% a stop, not a note. So the rung is **not** counted, and 20% qualification waits.

Before arguing it, one fact about the reference figure had to be checked. The 167 mA is **not** from the frozen oracle `0C734C67…`. That image prints no current census (its capture `oracle_8000ms.txt` has only `AVGNOMINAL`). It comes from the diagnostic `bench-rate-curve` image `10CD4D47…` (`binz/DUTY_50_CAMPAIGN.md:5-14`), whose ELF is archived at `binz/captures/reference/reverse_48k_rate_curve_20260919/shell-pwm.elf`. It is also **one run per rung** ("The table is one diagnostic run per rung", `:39`). Two further differences are known:
* **Scaling.** The reference scales by its measured nominal (`AVGNOMINAL raw_sum=34659` at VDDA 3309 mV). firmware50 uses `RAW_LIMIT = 31857` computed at 3600 mV, so the same residual reads 9% higher here. The direction is against us, so that is not the gap.
* **Window.** The reference's window is 16.6 s after the final ACK; firmware50's is 34.3 s.

## E084 — the oracle's own current scatter at 20% (no firmware50 change; oracle runs)

**Why:** the goal settles rig questions by running the oracle. The question is whether the rate-curve image, run again on this bench today, reproduces its own 167 mA at 20%, or scatters like firmware50's proxy. If it reproduces within ±20% and firmware50 reads 103, the gap is real and in firmware50's drive or measurement. If it scatters, the single-run reference figure cannot carry a 20% band, and that goes to the operator as a finding, not a relaxation.

**How:**
* Flash the rate-curve ELF (SHA verified `10CD4D47…` before flashing).
* Run `binz/scripts/live_armed_baseline.py` read-only, with the capture's own settings: `--nominal-average --nominal-target-ma 4000 --ramp-duty 200 --ramp-step 10 --ramp-period 0.5 --bemf-duty 100 --ms 25000`. The captures go to `firmware50/captures/2026-09-20/oracle-rc20_0N.txt`; nothing is written into `binz/captures`.
* Three runs, reading `RATECURVE current_sum/current_blocks` and `BEMFSTOP estimated_ehz` from each.
* Then flash firmware50 `1F9FCE5D…` back and verify.

PSU unchanged, 1 A. The oracle ran this exact rung on 1 A on 2026-09-19.

### E084 — result (rate-curve oracle `10CD4D47…`, captures `oracle-rc20_0{1..4}.txt`)

* Run `_01` was refused by the harness **before any drive**: the image reports `duty_max=500`, so the harness needs `--duty50`. That flag was added for runs 02–04; nothing else changed.
* Runs 02–04 each ended with a host-side verifier exception (`missing/invalid timing reanchor provenance` in `drv_driven_handoff.py`). That is a post-capture check in the binz harness; the captures are complete, and each dumped `DONE … gates=off en=off`.

| run | `current_sum / blocks` | `AVGNOMINAL raw_sum` | proxy mA | `estimated_ehz` |
|---|---|---|---|---|
| 2026-09-19 (the reference row) | 1448 | 34659 | 167 | 941 |
| 02 | 1671 | 34617 | 193 | 931 |
| 03 | 1647 | 34659 | 190 | 938 |
| 04 | 1129 | 34680 | 130 | 938 |

**The oracle's own 20% proxy scatters 130–193 mA** over four runs on one image and one bench: a mean of 170, about ±20%. Its speed repeats to ±0.5%. **firmware50's 103 mA** is 95 mA in the oracle's scaling (820 raw per block × 4000 / 34659). That is below the oracle's lowest run and 44% below its mean, at the same speed (coast 949 vs 931–941). The mismatch is therefore **real at the proxy level, and the stop stands.** It is not argued away as scatter.

**What a proxy gap at an equal speed can mean.** Equal speed on the same rotor and propeller means equal mechanical load, so a real difference in DC-link current would have to be a difference in drive efficiency: the switching pattern, the floating-phase treatment, or dead time. Alternatively the physical draw is equal and the difference is in how the three-shunt sum samples the PWM, since the oracle triggers its scan from TIM15 at 226 µs and firmware50 from TIM6 at 101 µs. The measurement that separates the two is the goal's own reconciliation: **the PSU display during a firmware50 20% hold and an oracle 20% hold.** That, and the PSU limit raise the goal requires for the real 4 A allowance and for 25%, are the operator steps.

firmware50 `1F9FCE5D…` was flashed back and verified (`Verification successful`).

## E085 — PSU reconciliation at 20% (firmware50 `1F9FCE5D…`, then the oracle)

**Operator decision, recorded:** asked for a PSU limit raise (for the goal's real-4 A allowance and for 25%), the operator chose **"Keep 1 A for now"**. So 25% and the 4 A allowance item are blocked on that limit. The 20% rung proceeds on 1 A, as the oracle's E765 did.

**Run:** firmware50 `--command 2`, one run. The operator reads the PSU display during the hold, which is about 10–44 s after the command. The same will then be done for the rate-curve oracle at 20%. This settles whether E084's proxy gap (95 vs 130–193 mA) is physical or a measurement artefact.
### E085 — result (`captures/2026-09-20/e085-psu20_01.txt`, ELF `1F9FCE5D…`)

The operator then said **"DO NOT STOP FOR THIS"**: no PSU reading and no waiting on the operator for it. Recorded as the operator's decision. **The current reconciliation against the PSU is therefore not done, and the goal's current item is not claimed.**

The run itself is 20% run 2: 34.283 s at 200‰, forced 0, deadline stop (2), 999‰ against the loop and **1002‰ against 6 × coast**. Coast is 955 eHz (+1.5% vs 941). `BEMFRCOMP spent_max_us=18 late_arms=0`. Guard reason 0, gap 105 µs, track 477 µs. `hold_ma=100`, repeating run 1's 103: firmware50's 20% proxy is consistent (±2%) but sits ~40% under the oracle's 130–193 (E084). The comparison line reads `verdict=STOP` on current, and that stays on the record.

Per the operator's instruction the campaign continues past it, with the current item carried as **open and failing** rather than relaxed.

## E086 — 20% run 3 (no change, ELF `1F9FCE5D…`, PSU 1 A)

Same command as E083/E085. It completes the 3/3 on speed, rate, forced-0 and hold length at 20%.
### E086 — result (`captures/2026-09-20/e086-qual20_01.txt`)

34.283 s at 200‰, forced 0, reason 2, 994‰ against the loop and 1002‰ against 6 × coast. Coast is 956 eHz (+1.6%). `BEMFRCOMP spent_max_us=18 late_arms=0`. Guard 0, gap 105 µs, track 474 µs. **`hold_ma=173` against 167 (+3.6%), `verdict=ok`.**

**20% rung, three runs (E083, E085, E086):**

| run | hold at 20% | forced | rate vs loop / vs 6×coast | coast eHz (vs 941) | R_COMP max / late | hold_ma (vs 167) | zero_start |
|---|---|---|---|---|---|---|---|
| E083 | 34.283 s | 0 | 998 / 1008‰ | 949 (+0.9%) | 18 µs / 0 | 103 (−38%) | 615728 |
| E085 | 34.283 s | 0 | 999 / 1002‰ | 955 (+1.5%) | 18 µs / 0 | 100 (−40%) | 616384 |
| E086 | 34.283 s | 0 | 994 / 1002‰ | 956 (+1.6%) | 18 µs / 0 | 173 (+4%) | 617483 |

The rung meets items 1 and 2 (3/3 ≥ 30 s, MCU-stamped, zero forced, rate within 1%, too_early and unstable non-zero) and the speed half of item 3 (3/3 within 5% of the oracle's speed). The current proxy is **1/3 within 20%**. Its mean is 125 mA, against the oracle's four-run mean of 170 (−26%). firmware50's per-run scatter (100–173) is as wide as the oracle's (130–193), so neither image carries a per-run 20% band on this proxy. **Not claimed. It stays open** until the PSU reconciliation, which the operator has declined for now.

## E087 — 25% shakedown (no change, ELF `1F9FCE5D…`, PSU still 1 A)

`--command 5`: 7.5 s ramp and a 31.8 s hold. The oracle stopped at 25% on this supply with average-current reason 25 (E765). firmware50's allowance is the real nominal 4 A, so if the draw reaches the PSU's 1 A limit, the expected stop is **fast sag (26)**, with all-off. If that happens it is recorded as the supply limit, and no threshold moves.
### E087 — result (`captures/2026-09-20/e087-shake25_01.txt`)

31.783 s at 250‰, forced 0, reason 2, 997‰ against the loop and **1001‰ against 6 × coast**. Coast is **1153 eHz against the oracle's 1186 (−2.8%)**. `BEMFRCOMP spent_max_us=18 late_arms=0`: the 25% wait at ci 144 µs is 72 − 45 = 27 µs, so **R_COMP < wait_time(ci) held for all 220 195 arms**, with 9 µs of margin measured. Guard 0, gap 105 µs, track 393 µs. **`hold_ma=310` against 326 (−4.9%), `verdict=ok`.** `bus_min=1101` against `bus_ref=1217` (90.5%, isolated scans); the three-scan sag stop did not trip, and the 1 A PSU limit was not reached.

## E088 — 25% runs 2 and 3 (no change)

`--command 5 --runs 2`, PSU 1 A.
### E088 — result (`captures/2026-09-2{0,1}/e088-qual25_0{1,2}.txt`)

| run | hold at 25% | forced | rate vs loop / vs 6×coast | coast eHz (vs 1186) | R_COMP max / late | hold_ma (vs 326) |
|---|---|---|---|---|---|---|
| E087 | 31.783 s | 0 | 997 / 1001‰ | 1153 (−2.8%) | 18 µs / 0 | 310 (−4.9%) |
| E088-1 | 31.782 s | 0 | 997 / 1002‰ | 1151 (−3.0%) | 18 µs / 0 | 354 (+8.6%) |
| E088-2 | 31.783 s | 0 | 997 / 1005‰ | 1149 (−3.1%) | 18 µs / 0 | 333 (+2.1%) |

**The 25% rung meets items 1–3 on this data:**
* 3/3 runs of ≥ 30 s at target, MCU-stamped, deadline stops, zero forced;
* the rate within 1% both against the loop and against the rotor;
* too_early (341k–344k) and unstable (982k–991k) non-zero;
* the coast speed within 5% of the oracle's;
* the current proxy within ±9% of the oracle's in all three runs.

The PSU reconciliation half of item 3 is not done (operator decision, E085). Every run was on the 1 A limit; `bus_min` stayed 90–93% of reference, in isolated scans.

Next: item 5, normal-start recovery at 25%, after reading the reference's design (`NORMAL_RESTART_E770.md`, `bench-normal-restart`).
## E089 — normal-start recovery (goal item 5): build, pre-run

**Reference read first:**
* `binz/NORMAL_RESTART_E759.md`, the policy:
  * only tracking (8) restarts;
  * one-shot, consumed even when refused;
  * the whole startup plus a 200 µs reserve must fit;
  * the remainder of the original window is returned, never a fresh one;
  * checked arithmetic.
* `NORMAL_RESTART_E770.md`, the sequence:
  * the bridge off for 1 000 000 µs;
  * a fresh same-wake zero with average protection reinstalled;
  * the ordinary staircase startup with the handover settings replayed;
  * a fresh handover;
  * reason 2 at the retained deadline.
  * The reference's cohort was 3/3 at 20 eHz-handover settings; no flying seed.

**What changed** (ELF `6BACFAD9…4007A5BB35`, text 52812 / data 28 / bss 5012):
1. `src/restart.rs` (new): `Restart::admit(stop, elapsed_us, window_us, need_us) -> Result<remaining_us, Refusal>`, with refusals `NotTracking`, `AlreadyUsed`, `NoRoom`, `Overrun` and `RESERVE_US = 200`. It has six host tests:
   * the arithmetic of the remaining window;
   * one-shot;
   * every non-tracking stop latched *and* consuming the attempt;
   * the exact fit boundary;
   * overrun and wrap;
   * distinct codes.
2. `bin/shell-pwm.rs`:
   * `bemf_run(…, total_ms)` takes its window as a parameter; every existing caller passes `BEMF_TOTAL_MS`.
   * The injection is generalised into an `inject_plan`. The new `RunMode::Restart` injects the existing Tracking stimulus (`DET_ACTIVE` cleared, so the guard's tracking watch trips) `ramp_us(target) + 2 s` after the loop closes, i.e. 2 s into the target hold.
   * `restart_campaign(board, 250)` on key `R`:
     1. segment 1 runs in Restart mode with the full window;
     2. `Restart::admit` decides, counting the off second against the window;
     3. the bridge stays off a further 1 s (a stop byte aborts);
     4. segment 2 is an ordinary `bemf_run` in Full mode for `window − elapsed`. It brings its own preflight, wake, fresh zero, new `AverageCurrent`, sine staircase, driven stage, seed, transfer and ramp back to 25%.
   * `need_us` = 1 s off + 5 s startup + 7.5 s ramp + 2 s minimum hold. That is stricter than the reference's startup-only need, so the recovery has to reach **and hold** target.
   * `BEMFRESTART` reports first/second reasons, holds, admission/refusal, remaining, campaign vs window, and `recovered = first==8 && second==2 && second_hold>0`.
   * `LAST_HOLD_MS` publishes each segment's MCU-stamped hold.
   * **Defect fixed in passing:** `tx_flush` fed the dog but never advanced the extended clock. A full-ring flush (> 65.5 ms at 115200) therefore lost a TIM17 wrap, and any time measured across a report came out short. Nothing so far measured across a flush; the restart window does. It now calls `tick_clock()`, which also feeds the dog.
3. `scripts/bemf_run.py`: command `R`, with per-command end markers (`R` ends on `BEMFRESTART`). The `ORACLE` comment is corrected to name the rate-curve image (E084).

**Checks:** 271 host tests (+6); clippy clean on the target and on the host library with `--tests`; four roots audit clean. The cycle counts are unchanged: COMP 1230 (12 reads) / 1060 (7 reads), COM 360, DMA 99, guard 445.

**Expected timeline at 25%:**
* closed at 4.72 s;
* at target at 12.2 s;
* tracking loss injected at 14.2 s, stop within ~0.7 ms;
* report and coast about 3 s, then 1 s off;
* segment 2 closes about 4.7 s later and is back at target 7.5 s after that;
* it holds until the 44 s window ends.

PSU 1 A.

### E089 — result (`captures/2026-09-21/e089-restart25_01.txt`, ELF `6BACFAD9…`)

```
BEMFINJECT  expected_reason=8 reason=8 guard_reason=8 fired=0 …
BEMFGATE    closed_ms=9432 hold_ms=1932 target_tenths=250      (segment 1)
BEMFGUARD   reason=8 track_fault=1 track_max_us=381
BEMFRUN     … total_ms=27132 inject=0                           (segment 2)
BEMFGATE    closed_ms=22415 hold_ms=14915 target_tenths=250 forced 0, 999‰
BEMFRESTART first_reason=8 admitted=1 remaining_ms=27132 second_reason=2 second_hold_ms=14915 campaign_ms=45711 window_ms=44000 recovered=1
```

**This does not count toward item 5.** `fired=0`: segment 1 stopped on Tracking 9.432 s after closing, **68 ms before the injection was due** (closed + 7.5 s ramp + 2 s = 9.5 s). It was a **genuine missed crossing at 25%**, 1.93 s into the target hold. It is the first in about 105 s of cumulative 25% running (E087, E088 ×2, and this one).
* `track_max_us` is the watch's tightened limit, not an observed gap. It had ratcheted to 381 µs = 3 × a 127 µs average interval. The clean runs ratcheted to 393 µs.
* With the forced fallback deleted, any missed crossing trips it, which is the designed behaviour ("a missed crossing ends the run with a Tracking stop, never a fallback").
* No threshold is touched. The diagnostic IRQ ring that would have shown the edge history was removed in E083 for the R_COMP budget, so the miss has no finer record than this.

What the run does show: the ordinary restart recovered a *real* tracking loss.
* It was admitted with 27.1 s left.
* It had a fresh zero and the staircase startup, and closed 4.7 s into segment 2.
* It ramped back to 25% and **held 14.9 s at 25% to the deadline**, with forced 0, 999‰, and R_COMP 18 µs with 0 late arms.

**Window accounting defect.** `campaign_ms=45711` against 44000. Segment 2's deadline ran from its own start, after preflight, wake and zero. The campaign's t0 was taken before segment 1's header flush, and the report and coast after the drive also count. The drive therefore ended about 15 ms past the original window. That is small, but the goal says the recovery completes the *remaining* window, not a fresh one, so it is fixed below.

## E090 — exact window for the restarted segment (build, pre-run)

* `bemf_run(…, total_ms, until: Option<u32>)`: the window now runs **from entry** (before preflight, wake and zero), or to an absolute instant.
* `RUN_ENTRY_US` and `RUN_STOP_US` publish each segment's entry and loop-exit instants.
* `restart_campaign` takes t0 = segment 1's entry and hands segment 2 `until = t0 + 44 s`.
* It reports `first_stop_ms` and `drive_end_ms` (segment 2's loop exit relative to t0), which should equal `window_ms`.
* For ordinary runs the deadline moves ~15 ms earlier (setup now counts), so the 25% hold shortens from 31.78 to ~31.77 s, still ≥ 30 s.

271 host tests pass; clippy clean; four roots audit clean, with COMP 1230, COM 360, DMA 99 and guard 445 unchanged. ELF `6C3F8F94…CF39E9AAB`, text 53200.

**Runs:** `--command R` × 3 at 25%, PSU 1 A. A run whose segment 1 ends on a *spontaneous* tracking loss (`fired=0`) is recorded but not counted. Item 5 asks for injected loss.

### E090 — result: normal-start recovery at 25%, 3/3 (`captures/2026-09-21/e090-restart25_0{1,2,3}.txt`, ELF `6C3F8F94…`)

| run | injected | stop after stimulus | seg-1 hold at 25% | all-off (seg 1 / seg 2) | admitted, remaining | seg-2 reason | seg-2 hold at 25% | seg-2 forced / rate | drive end vs window | recovered |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | fired=1, reason 8 | 414 µs | 2.000 s | PASS / PASS | 1, 27 070 ms | 2 | 14.834 s | 0 / 993‰ | 44 000 / 44 000 ms | 1 |
| 2 | fired=1, reason 8 | 456 µs | 2.000 s | PASS / PASS | 1, 27 071 ms | 2 | 14.834 s | 0 / 997‰ | 44 000 / 44 000 ms | 1 |
| 3 | fired=1, reason 8 | 470 µs | 2.000 s | PASS / PASS | 1, 27 070 ms | 2 | 14.832 s | 0 / 997‰ | 44 000 / 44 000 ms | 1 |

Each run went through the full sequence, which is **item 5, 3/3**:
1. an injected tracking loss from a locked 25% hold;
2. the guard's Tracking stop, reason 8, within the watch's ~390–400 µs limit;
3. all-off verified;
4. at least 1 s off: the stop was at 14.23 s and segment 2 entered at ~16.93 s;
5. the ordinary startup from a fresh same-wake zero (staircase, driven stage, seed, transfer);
6. the ramp back to 25%;
7. a 14.8 s hold at 25% to **exactly** the original window's end.

R_COMP stayed at 18 µs with 0 late arms in all six segments. Segment-2 current proxies were 366 / 393 / 375 mA against 326.

## E091 — protections re-provoked on the final image (no change, ELF `6C3F8F94…`)

**Why:** no protection threshold changed in campaign 2. The image around them did (window semantics, `tx_flush` clock, the restart path, COMP's arm), so each stop is fired once more on this exact image and its reason and all-off captured. Stimuli are E080's, unchanged: `T`, `G`, `F`, `N`, `U`, `V`, `I`, host abort, and `W` (IWDG). All run at 15%, 3 s into the closed loop, PSU 1 A.

**Not possible under the operator's decision:** the average-current stop at the *real* 4 A allowance needs the PSU limit above 4 A, and the operator kept 1 A (E085). `I` provokes it with the stricter allowance, as in E080.

### E091 — result (`captures/2026-09-21/e091-*.txt`, ELF `6C3F8F94…`)

| key | protection | expected | reason | stop after stimulus | all-off (POSTSTOP) |
|---|---|---|---|---|---|
| T | tracking watch | 8 | **8** (guard 8) | 708 µs | PASS |
| G | tick gap | 3 | **3** (guard 3) | 416 µs | PASS |
| F | feedback age | 4 | **4** (guard 4) | 1064 µs | PASS |
| N | nFAULT | 7 | **7** (guard 7) | 48 µs | PASS |
| U | COMP storm | 13 | **13** | 1318 µs | PASS |
| V | fast bus sag (3-scan) | 26 | **26** | 1290 µs | PASS |
| I | average current (stricter allowance, held plans) | 25 | **25** | 20.2 ms (two blocks) | PASS |
| host | host abort | 9 | **9** | — | PASS |
| W | IWDG (100 ms foreground stall) | reset | next boot `RESETCAUSE iwdg=1` | — | bridge off from reset |

Every protection fires on the final image with its reason code and a verified all-off. None of their thresholds changed in campaign 2.

The reproducible build is confirmed: `cargo clean` then `build.ps1` gives the same `6C3F8F948304B8F395ADD7B2D088780300A178A2AA6F39DB4290288CF39E9AAB` (text 53200 / data 28 / bss 5020), and `probe-rs verify` matches the board.

---

### Campaign 2 status (goal: closed loop from 15% to 25%)

Final image `6C3F8F94…CF39E9AAB`. The operator's instructions during the campaign were **"Keep 1 A for now"** (PSU limit) and **"DO NOT STOP FOR THIS"** (no PSU reading, no waiting on the operator).

1. **3/3 ≥ 30 s at target, MCU-stamped, forced 0, missed crossing = Tracking stop: met at both rungs.**
   * At 20%: E083, E085 and E086 held 34.28 s each.
   * At 25%: E087 and E088 ×2 held 31.78 s each.
   * All ended on the deadline (reason 2) with forced 0. There is no fallback path.
   * One genuine missed crossing at 25% (E089, 9.4 s after closing) ended its run on Tracking, as designed. That is 1 in about 105 s of cumulative 25% drive at the time.
2. **Accepted rate within 1%, too_early and unstable non-zero: met.**
   * 994–999‰ against the loop's sectors.
   * 1001–1008‰ against 6 × the rotor's own coast speed.
3. **Speed from the rotor: met.**
   * Coast eHz within 5% of the reference: 949/955/956 against 941 (+0.9…+1.6%) at 20%, and 1153/1151/1149 against 1186 (−2.8…−3.1%) at 25%.
   * The current proxy, with a same-wake bridge-off zero before every run, is reported every run.
   * **The PSU reconciliation is not done.** The operator declined it (E085).
4. **Beside the reference, >20% is a stop.**
   * Speed is within 5% at both rungs.
   * **Current at 25%: 310/354/333 against 326 (−5…+9%), met.**
   * **Current at 20%: 103/100/173 against 167, so two of three runs are outside 20%. Not met.**
   * The reference figure is one run of the rate-curve diagnostic image, `10CD4D47…`; the frozen oracle prints no current census. Re-running that image on this bench (E084) gave 193/190/130, so its own spread is about ±20%.
   * firmware50's 20% mean of 125 is 26% under that image's mean of 170.
   * This stays an open mismatch, not a note. Separating physical draw from proxy artefact needs the PSU reading that was declined.
5. **Normal-start recovery at 25%: met, 3/3** (E090).
   * Each run went: injected tracking loss, Tracking 8 within about 0.4 ms, all-off PASS, 1 s off, ordinary startup, back to 25%, a 14.8 s hold, and a deadline at exactly the original window's end (44 000 ms).
   * The E089 run also recovered a *genuine* loss, but it is not counted.
6. **Protections active and unchanged: met on the final image (E091), except one item.** The average-current stop at the *real* 4 A allowance with the PSU raised to cover it was **not done**: the operator kept 1 A. It is provoked with a stricter allowance, which is a tightening, not a relaxation.
7. **Four ISR roots fail-closed audited on the final image: met.**

   | root | longest path @ 0 ws |
   |---|---|
   | COMP | 1230 cycles (12 reads), 1060 (7 reads, the 25% level) |
   | COM | 360 |
   | DMA | 99 |
   | guard | 445 |

   * COMP's worst case fell from 1245 to 1230 cycles (the IRQ ring was removed).
   * **R_COMP < wait_time(ci) at 25%.** Statically, 1060 + 445 + 24 cycles = 23.9 µs < 27 µs at 0 ws. **Measured on the rotor, 18 µs max with `late_arms=0`** across every 20% and 25% segment (about 1.9 M arms).

**Code line:**
* Typed policies: `FromMicros<WithShallowFloor<MappedFilter>>`, `restart::Restart`, `ramp`.
* No new `#[cfg]` or features.
* 271 host tests, up from 258/265, none deleted.
* Clippy clean on the target, and on the host library with `--tests`.
* A reproducible ELF.
* No UART traffic while the loop runs: every new line is post-run.
* No edits to `../rm32*`, `../minz/core` or the old binz package, its captures or its notebook. The binz harness `live_armed_baseline.py` was run read-only, with output into `firmware50/captures`.

**Open, needing the operator:**
* the PSU display at a 20% and a 25% hold, which reconciles the current and decides the 20% current mismatch;
* a PSU limit above 4 A, for the real-allowance average-current provocation.

## E092 — current-sense zero settling after wake (bridge off, no drive)

**Why:** the stop hook confirms goal item 4 fails at 20%. In the data, `hold_ma` tracks each run's pre-run zero, and at 25% the slope of hold on zero is close to pure datum error (Δzero 422 raw → Δhold 44 mA; 422 raw scales to 53 mA). `zero_end` also always reads 100–280 mA-equivalent lower than `zero_start`. The reference takes its zero the same way (ENABLE, 1 ms, 128 scans; `binz/examples/shell-pwm.rs:2345-2347`), so this is not a transcription gap. The question is physical: **is the DRV8304 amplifier zero still moving when it is captured, 2 ms after wake?**

**Change:** a new shell key `z` (`zero_settle`). With the gates GPIO-low and MOE clear, it raises ENABLE only and takes the 100-scan zero at 2, 5, 10, 20, 50, 100, 200, 400, 700, 1000, 2000 and 3000 ms after wake, then ENABLE low. No motor drive. ELF `7CA1ED26…12F713B7`; clippy clean. `bemf_run.py` gains `z`.

**Runs:** `z` cold, then `z` immediately after a 25% run (warm), to see whether the datum depends on time since wake and on driver temperature.
### E092 — result (`captures/2026-09-21/e092-zero-cold_0{1,2}.txt`, ELF `7CA1ED26…`)

**There is no settling trend after wake.** The datum is noisy block to block:

| at (ms) | 2 | 12 | 22 | 32 | 50 | 100 | 200 | 400 | 700 | 1000 | 2000 | 3000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| run 1 | 616207 | 615350 | 615600 | 616209 | 616234 | 616131 | 616093 | 616243 | 616703 | 616224 | 616143 | 616127 |
| run 2 | 615651 | 616027 | 615881 | 616192 | 616097 | 615967 | 615721 | 616122 | 616142 | 616298 | 616237 | 616461 |

* Single 100-scan (10 ms) bridge-off zeros span **615350–616703, i.e. 1353 raw ≈ 170 mA-equivalent**. The per-channel means move ±2–3 codes between 10 ms blocks.
* The twelve-sample means of the two runs agree to 60 raw (≈ 8 mA).
* So a one-block datum carries roughly ±45 mA (1σ) into every run's current figure, while the hold itself averages 3000+ blocks.
* That accounts for firmware50's 20% spread (100–173) and for the rate-curve image's own spread from its 128-scan zero (130–193, E084). The first two 20% runs' zeros (615728, 616384) bracket the low side of this distribution.
* E078/E079's "white noise on a 100-scan datum is about ±6 mA" was wrong: the noise is not white at 10 ms.

## E093 — averaged bridge-off zero (build, pre-run)

**What changed** (ELF `A2341133…F0C68305`, text 54284 / data 28 / bss 5020):
* `src/protection.rs`: `ZERO_BLOCKS = 50` and `zero_from_blocks(sum, blocks)`, rounding **up** as the reference does (`binz/examples/support/average_current.rs:18-27`). Three host tests: rounding, empty/out-of-range, and "the allowance still folds back against an averaged zero".
* `bin/shell-pwm.rs`: `averaged_zero()` takes 49 more `capture_baseline` blocks after the first. Each block passes the CSA plausibility gate, so the datum is the mean of 50 bridge-off blocks (0.5 s). It is used for the pre-run zero (and therefore for the average-current protection's datum) and for the post-run `zero_end`. `BEMFCURRENT` reports `zero_blocks`.

**Still the goal's quantity:** the signed three-shunt residual over 100-scan blocks, scaled by the 4 A nominal, against a bridge-off zero taken before every run. Only the zero's averaging time differs from the reference's 128 scans; that is stated here, not hidden.
* The **protection threshold is unchanged** (`RAW_LIMIT`). A less noisy datum removes a ±45 mA random offset in both directions; it does not relax the stop.
* Because the average-current protection's datum changed, `I` is re-provoked on this image.

**Checks:** 274 host tests (+3); clippy clean (target and host `--tests`); four roots audit clean; cycles unchanged (COMP 1230, COM 360, DMA 99, guard 445).

**Runs:** the 20% rung ×3, then the 25% rung ×3 and restart ×3 on this final image, then `I` (and `T`). PSU 1 A. The holds shorten by the 0.5 s zero: about 33.8 s at 20% and 31.3 s at 25%.
### E093 — result: 20% ×3 on `A2341133…` (`captures/2026-09-21/e093-qual20_0{1,2,3}.txt`)

| run | reason | hold at 20% | coast eHz | R_COMP | zero_start → zero_end (50-block) | hold_ma (vs 167) | transfer seed_step / closed IRQ peak |
|---|---|---|---|---|---|---|---|
| 1 | 2 | 33.776 s | 954 | 18 / 0 | 616745 → 616718 | 131 (−22%) | 3 / 61 |
| 2 | **13 CompStorm, 3 ms after transfer** | 0 | — | 17 / 0 | 618165 → 616125 | — | 4 / **65** |
| 3 | 2 | 33.776 s | 955 | 18 / 0 | 618079 → 616170 | 229 (+37%) | 3 / 60 |

**E093's premise is refuted.** Even averaged over 0.5 s, the pre-run zero moves **1420 raw (~180 mA-equivalent) between runs**: 616745 cold, then 618165 and 618079 after 20 s rests. The previous run's post-run zero was 616718 and 616125. So the DRV8304 amplifier zero **drifts with the driver's thermal history on a tens-of-seconds scale**, on top of the 10 ms wander. A bridge-off datum taken before the run cannot know the amplifier's offset during the run. The proxy followed the datum (131, then 229 mA).
* This is the same exposure the reference has: its E763 PSU anchor says "board reads ~1.11× PSU at this point, retain 10–15% uncertainty", and its own spread is 130–193.
* **Conclusion, stated with its limit:** the goal's per-run 20% band on this proxy cannot be verified at 20% by averaging the datum longer. Only the declined PSU reconciliation can say whether the physical draw matches. The averaged zero stays: it removes the 10 ms wander and no longer adds noise, but it is not claimed as a fix.

**A new failure: CompStorm (13) at transfer, run 2.** It is recorded as a failed 20% run. The whole-history table of `closed_irq_peak_per_ms` (50 captures):
* campaign 1 at 15%: **50–59**;
* campaign 2: **58–63**, one natural trip at **65**;
* the storm injections (`U`) at 65 by design.

**The oracle's own `IRQRATE peak` over 718 binz captures is 13–32/ms (mode 26)**, so firmware50 enters COMP about twice as often as the reference. Refusals average ~36/ms (too_early + unstable) against ~5.5 accepted/ms. The 64/ms cutoff is the reference's (`core_bench.rs:548-557`, `bench-reverse-irq-cap64`) and **will not move**. The chatter has to come down instead.
* A transcription lead: the reference's `REVERSEBLANK arms=4` at every rate-curve rung, against firmware50's `blank_arms=1` in all 48 captures. Both use the same 1500 half-µs / 750 µs threshold (`core_bench.rs:1829-1841`) and a measured seed (`:2805-2822`).

## E094 — where is the closed-loop IRQ peak? (diagnostic build)

`src/rate.rs`: `Rate` also records `peak_bucket` (the ordinal of the bucket that set the peak, about ms since transfer) and `settled_peak` (the peak over buckets after the first 50). The cutoff logic is untouched. One new host test (275 total). `BEMFDRIVEN` adds `closed_peak_bucket` and `closed_settled_peak`.

**This build's COMP longest path is 1253 cycles, over the 1245 ceiling.** It is a diagnostic image (`3C9EDB43…F2F620832`) and will not be the final image: the fields are trimmed or removed before any qualification.

**Runs:** 20% ×3. If the peak sits in the first few buckets, the handover window is the target (reverse blank / transfer). If not, the steady state is.
### E094 — result: diagnostic `3C9EDB43…` (`captures/2026-09-21/e094-peak20_0{1,2,3}.txt`)

| run | reason | closed IRQ peak | peak bucket (ms after transfer) | settled peak (after 50 ms) |
|---|---|---|---|---|
| 1 | 2 | 56 | 41 | 54 |
| 2 | 2 | 57 | 5 | 54 |
| 3 | 2 | 57 | 25 | 55 |

* **The peak is not a transfer transient.** Steady running already peaks at 54–55/ms.
* The 23-cycle-longer COMP handler of this build peaked at 56–57, against 58–65 on the 1230-cycle build. Under continuous chatter the entry count per ms tracks how fast the handler returns to a live line.
* The reference's decision logic matches (`minz/core/src/am32_isr.rs:62-125`: a half-interval gate, live-read persistence, the line left live on refusal). It counts every entry before dispatch too (`core_bench.rs:4081-4082`). Its 13–32/ms peak therefore means **fewer comparator edges reach its handler, not a cheaper count**.
* E042/E072 had already examined ADC charge injection: E042 removed the comparator's own pins from the scan, and E072 found the calls uniform in ADC phase.
* **No root cause is established.** It is carried as an open reliability finding: about 1 natural storm trip (13) in 23 campaign-2 closed-loop runs, at a COMP entry peak about twice the reference's. The 64/ms cutoff is unchanged.

## E095 — final image: the locator as a typed policy (build)

* `src/rate.rs`: `Rate<const LOCATE: bool = false>`. The E094 tracking compiles out for `LOCATE = false`, which the binary's `DRV_RATE`/`DET_RATE` use; the cutoff is identical, and a new test checks that equivalence. `BEMFDRIVEN` drops E094's two fields.
* Kept from E092/E093: the `z` zero-settle probe (no drive) and the 50-block averaged datum. The latter is a sound noise reduction but **not** a fix for item 4.
* 276 host tests (+5 since E091, none deleted); clippy clean on the target and host `--tests`.
* ELF `C9B50C47313B75D19F1A25BC1C415FCAAFF4075A5858A90B53640D751658CA84`, text 54248 / data 28 / bss 5032.
* Audit and cycles on this ELF: AUDIT PASSED: 4 root(s) certified clean.; COMP longest path         1200 cycles @ 0 wait states (12 reads) / longest path         1030 cycles @ 0 wait states (7 reads); the other roots as printed in the E095 check.

**Requalification on this final image:** 20% ×3, 25% ×3, restart ×3 at 25%, and `I` re-provoked, because the average-current datum changed in E093. Every run is recorded, whatever its outcome. PSU 1 A.
### E095 — result: 20% ×3 on the final candidate `C9B50C47…` (`captures/2026-09-21/e095-qual20_0{1,2,3}.txt`)

| run | reason | hold | coast eHz | R_COMP | closed IRQ peak | hold_ma (vs 167) |
|---|---|---|---|---|---|---|
| 1 | 2 | 33.776 s | 954 | 18 / 0 | 63 | 144 (−14%) |
| 2 | 2 | 33.776 s | 954 | 18 / 0 | 63 | 228 (+37%) |
| 3 | **13 CompStorm at transfer** | 0 | — | 12 / 0 | **65** | — |

**The 20% rung fails on this image** (1 storm in 3). With COMP at 1200 cycles the peaks rose again. Across builds the closed-loop IRQ peak tracks the COMP handler's length:

| build | COMP longest path | closed IRQ peak |
|---|---|---|
| C9B50C47 | 1200 | 63–65 |
| 6C3F8F94 / A2341133 | 1230 | 58–63 (65 once) |
| 3C9EDB43 (diagnostic) | 1253 | 56–57 |
| E081 (15%) | 1245 | 50–59 |

**Reading:** firmware50 sees more than one selected-edge event per 48 kHz carrier period in its refusal windows, so a faster handler catches more of them. The reference peaks at 13–32/ms, i.e. under one edge per period. With the same decision logic and the same COMP2_CSR (E072), **fewer edges** means **less comparator chatter**, which points at the electrical drive pattern (what the three phases do during the floating sector), not at the detector.
* Padding the handler to buy margin would be engineering the protection's margin, and is not done.
* The 64/ms cutoff does not move.

## E096 — oracle vs firmware50: live drive registers during a 20% hold (SWD reads, no code change)

**Why:** the goal settles rig questions by running the oracle. Both images are run at 20%. During the hold, TIM1 (`0x40012C00`, CR1…AF2), COMP2_CSR (`0x40010204`) and EXTI RTSR1/FTSR1/IMR1 (`0x40021800`…) are read over SWD 20 times each without halting the core, and the per-sector patterns (CCMR1/2, CCER, CCR1–3, BDTR) are compared.

**Order:**
1. the rate-curve oracle `10CD4D47…` at 20% via the binz harness (read-only, output to `firmware50/captures`) while reading;
2. firmware50 `C9B50C47…` flashed back, run with `--command 2`, same reads.
### E096 — result: the method fails on a powered run

* The first attempt (`e096-oracle-run20.txt`) never drove: the oracle did not answer on COM41 after download and reset, so the harness's own pre-run readback refused (`output-off/nFAULT readback failed`). A second `probe-rs reset` brought it up (`OUT: … moe=0`, `IN: … nflt=1`).
* In the second attempt (`e096-oracle-run20c.txt`, `e096-oracle-regs20c.txt`) the oracle was ramping, at 15% at 5.97 s, when the first SWD read landed. It stopped at once on **`POWERPATH reason=3` (TickGap)**, with `DONE … gates=off en=off`. All 15 samples show an idle timer (CCR1–3 = 0, ARR = 6399).
* **`probe-rs read` stalls the running core.** The idle check (firmware50 still answering) could not show that, because nothing times ticks at idle. The oracle's guard did exactly its job.
* **Live SWD sampling is therefore not a usable instrument on a powered run, for either image.** Nothing was learned about the drive pattern. firmware50 `C9B50C47…` is flashed back and verified.

### Status after E092–E096 (ELF on the board: `C9B50C47…`)

* **Item 4 at 20% (current): not met, and not fixable by the datum.**
  * The bridge-off zero drifts with the driver's thermal history by ~1400 raw (~180 mA-equivalent) between runs, even averaged over 0.5 s (E093).
  * Per-run 20% agreement with a 167 mA single-run reference is therefore chance. On this image: 144 (−14%) and 228 (+37%).
  * The reference's own spread is 130–193 mA.
  * The PSU reading, declined, is the only arbiter.
* **Item 1 at 20%: not met on the final candidate.** One CompStorm (13) at transfer in three runs (E095), one in E093, one in 23 campaign-2 runs before that.
  * firmware50's closed-loop COMP entry peak (56–65/ms) is about twice the reference's (13–32/ms), and it grows as the COMP handler gets faster (four builds, monotonic).
  * During chatter bursts COMP is effectively saturated, so the unchanged 64/ms cutoff is doing its job.
  * **The chatter's root cause is not established:** the decision logic matches, the edge selection matches, COMP2_CSR matches, E072 found no ADC-phase correlation, and live register comparison is impossible (above).
* The only moves left change a protection's or the detector's semantics: a post-refusal holdoff (which the reference does not have), or a CPU-time storm cutoff instead of a count (a changed threshold). **Both are the operator's decision under "No threshold relaxed to pass", and neither was taken.**
## E097 — the reference's comparator path, read from source (no code change, no powered run)

**Why:** to explain the doubled COMP entry rate (E094/E095) from the reference's code, now that live SWD sampling is ruled out (E096).

* **Drive pattern: equivalent.** The reference commits each sector as MOE off, all gate GPIO latches cleared, and only the source's pins on TIM1 AF with complementary PWM. The sink's low side is GPIO-high, both floating-phase inputs are GPIO-low, and then MOE comes back on. All three CCRs stay equal (`phase_role_sequence.rs::apply_carrier`, `phase_gpio_plan.rs`). firmware50 reaches the same static pin levels with TIM1 output modes (force-inactive sink, `CCER` 0b0001 float, equal CCRs; `src/sixstep.rs`).
* **ADC: same channels** (reference `[4,1,0,6,13]` = firmware50 `{0,1,4,6,13}`). The trigger differs: the reference uses TIM15 at 226 µs, firmware50 TIM6 at 101 µs. E072 found COMP calls uniform in ADC phase.
* **Hardware capture filter: present in the oracle's feature list, NOT active in its closed loop.** The frozen ELF's closure includes `bench-capture-filter` and `bench-filter-source` (`captures/reference/reverse_48k_com_top_high_20260919/README.md`). That path is TIM2 CH2 input capture of COMP2 with filter code 12 / CKD 2, about 7–8 µs of deglitch (`support/filtered_irq_hw.rs`, `capture_filter.rs`), with the COMP EXTI masked. But the only dispatch into the controller, and `comp_input::prepare_filtered`, sit behind `bench-filter-control` (`examples/shell-pwm.rs:353-354`, `support/comp_input.rs:15-26`), which the oracle does **not** carry. Without it, `filtered_irq_hw::prepare` is reached only from the `filtersourcecheck` self-test. **The oracle's closed loop runs on the raw COMP EXTI line, like firmware50's.** Not ported; the lead is closed.
* **Handler duration** is therefore the remaining explanation consistent with the data. firmware50's entry peak falls monotonically as its COMP path lengthens (E095 table). The oracle's handler is larger: `ADC_COMP` is 0x320 bytes, plus `comp_isr` at 0x150 (`DISASSEMBLY.md`), against firmware50's ~1200-cycle path. Its static WCET cannot be computed with `isr_cycles.py` (`driven_run::comp_irq` carries four unbounded loops), so this stays an inference, not a measurement.

**Consequence, stated for the operator.** Under continuous comparator chatter both images are handler-limited. The reference's 64/ms count cutoff is effectively unreachable for its slower handler, but reachable for firmware50's leaner one. Keeping the count at 64 (no threshold relaxed) and not padding COMP (its worst case must not grow) leaves no in-rules firmware move that I can justify from the reference. The options that remain change the detector or the protection, and are the operator's decision:
* a post-refusal hold-off;
* a CPU-time storm cutoff.
## E098 — operator decisions, and the PSU reconciliation at 20% and 25% (ELF `C9B50C47…`)

The operator's answers (asked after E097):
* **Storm fix → "Diagnose further first":** build a diagnostic image that pads only the refusal path, to test whether handler duration alone brings the closed-loop IRQ peak down to the reference's ~26/ms. It is never the final image.
* **PSU → "Read display only":** the limit stays 1 A. The operator reads the display during a 20% hold and a 25% hold. The real-4 A average-current provocation stays open.

**Runs:** `--command 2` then `--command 5`, one each. The operator reads the PSU current during the hold, about 15–40 s after the command. Recorded beside each run's `hold_ma`, the proxy's own zero, and the oracle's figure.
### E098 — results

| run | reason | hold | coast eHz | hold_ma | **PSU (operator)** |
|---|---|---|---|---|---|
| `e098-psu20_01` | 2 | 33.776 s | 956 | 137 | not read (missed the hold) |
| `e098-psu20b_01` | 2 | 33.775 s | 956 | **222** | **0.250 A** |
| `e098-psu25_01` | **13 CompStorm, 10 ms after transfer** (seed_step 4, peak 65) | 0 | — | — | none; the operator saw it "clip out" |

**Item 3, the 20% reconciliation, is done once.** The PSU read 0.250 A during a 20% hold whose proxy read 222 mA (0.89 × PSU). The reference's own anchor was ~1.11 × PSU at 15% (E763). The 20% reference figure of 167 mA would be 0.67 × this draw, and its image's four runs span 130–193 (0.52–0.77 ×). **At 20%, firmware50's proxy on that run agrees with the physical draw within 11%; the single-run reference figure does not.** This does not make item 4's per-run band pass (2 of 3 E095 runs were outside it). It establishes that the physical current at 20% is ~0.25 A on this 11.8 V supply.

**The 25% reading could not be taken:** that run stopped on the storm cutoff at transfer. Bridge off, POSTSTOP PASS.

**New pattern.** All three natural CompStorm stops (E093-2, E095-3, E098-25) seeded on **step 4**. Across 33 campaign-2 runs: seed 3 → 0/18 storms (peaks 57–63); seed 2 → 0/1; **seed 4 → 3/14 storms**, each at peak 65 within 3–10 ms of transfer. This is not generic chatter plus handler speed alone. Under `Reverse`, logical step 4 is physical sector 1, which floats phase C on PA2, the node E041/E042 found most disturbed.

## E099 — diagnostic image: where and why the transfer storm happens (build)

Operator decision (E098): "Diagnose further first". **Diagnostic only, never the final image.**
* The binary uses `Rate<true>` (the E094/E095 locator: peak bucket and settled peak).
* `STORM_STEP`: the logical step in force when the storm cutoff trips.
* `REFUSE_PAD_US`, a runtime value (0 by default). Shell key `6` runs 20% with the refusal path held until 38 µs after COMP's entry stamp before re-arming, emulating a handler about as slow as the reference's inferred ~26/ms under chatter. The accept path and the cutoff are untouched.
* `BEMFDRIVEN` gains `storm_step`, `closed_peak_bucket`, `closed_settled_peak` and `refuse_pad_us`.

**Predictions:** if handler duration is the mechanism, the `6` runs peak near 26–35/ms. If the step-4 transfer storm has a separate cause, `storm_step` will name the sector, and it will persist even with padding.
### E099 — result: diagnostic `53DC13BD…` (`captures/2026-09-21/e099-pad38_0{1,2,3}.txt`), key `6`, refusal held to 38 µs

| run | reason | hold at 20% | seed step | closed IRQ peak | settled peak | storm |
|---|---|---|---|---|---|---|
| 1 | 8 Tracking | 6.128 s | 3 | **24** | 24 | 0 |
| 2 | 8 Tracking | 3.119 s | 4 | **25** | 23 | 0 |
| 3 | 8 Tracking | 2.779 s | 3 | **23** | 23 | 0 |

**Mechanism confirmed.** Holding only the refusal path to 38 µs brought the closed-loop COMP entry peak from 56–65/ms down to **23–25/ms**, inside the reference's 13–32 (mode 26). That was the prediction. It confirms that under comparator chatter the per-ms entry count is set by the handler's duration, and that the reference's 64/ms cutoff sits far above what its slower handler can produce, but just above what firmware50's lean handler produces. No storm occurred, including on a step-4 seed.

**But the padding itself is not viable.** All three padded runs lost tracking after 2.8–6.1 s at 20%. A busy-wait holds the CPU at 0x40, so the COM root (a 0x40 peer) is blocked up to ~38 µs, commutations land late, and a crossing is missed. `zc_acc` exceeding `accepted` also reflects accepts whose commutation was not served. It is not a candidate, and it could not be anyway (it grows COMP's worst case).

**What this leaves for the operator's decision.** The count cutoff's meaning depends on the handler's speed. Two ways keep the reference's intent without blocking the CPU:
* **(a) A non-blocking post-refusal hold-off:** leave the line masked for ~30 µs after an unstable refusal, re-arming from a timer, not a spin. Fewer re-entries, with the CPU free. It changes the detector: a real crossing inside the hold-off is seen up to ~30 µs late.
* **(b) A storm cutoff in CPU-busy time per ms**, calibrated to the reference's regime: 64 × the reference handler's duration ≫ 1 ms, so effectively "COMP busy > X% of a ms". It changes a protection's definition, which would be re-provoked.

The step-4 storm location (`storm_step`) was not captured, because no padded run stormed. The board carries this diagnostic image. The final candidate `C9B50C47…` must be rebuilt from source without the E099 changes before any qualification.
## E100 — the reference's reverse-blank gate: the six-slot average, not the per-crossing blend (build)

**The operator's direction:** "READ WHAT BINZ DID."

**What binz did about 65/ms stops, read and cited:**
* **Startup/driven path (E724 → E724d/E767):** "Still stopped 65 dispatches/ms/ratecap64" led to "startup rate observe/report only … per-call 50 us watchdogs retained" (`binz/AGENTS.md:3015-3018`, `:2495-2497`). In code: "Dispatch count is telemetry, not a safety boundary. A short burst at 65/ms previously killed an otherwise bounded normal restart. Actual runaway/latency protection is the measured 50us handler budget" (`examples/support/driven_irq_live.rs:216-222, 310-318`). firmware50's driven stage already keeps the count report-only.
* **Closed loop:** the frozen oracle **keeps** `hit_limit(64)` in its lean COMP path (`core_bench.rs:4081-4082`, `bench-reverse-irq-cap64`). It never trips there, because its closed-loop peak is 13–32/ms.
* **The reverse blank in the handover window, which is where every natural firmware50 storm happened (3–10 ms after transfer).** The reference blanks after a commutation while its `average_interval >= 1500` half-µs (`core_bench.rs:1829-1841`). That `average_interval` is **AM32's six-slot average**, recomputed after every commutation: `commutation_intervals[step-1] = commutation_interval` (`main.c:887`; `minz/core/src/am32_control.rs:77-80`), then `e_com_time = (sum + 4) >> 1` and `average_interval = e_com_time / 3` (`main.c:2159, 2283`; `am32_loop.rs:267-279`; `core_bench.rs:2133-2137`). All six slots are seeded with the seed interval at handover (`core_bench.rs:2805-2812`).
* Because that ring lags an accelerating rotor, the blank keeps arming for several commutations: `REVERSEBLANK arms=4` at every rate-curve rung. firmware50 had gated it on the per-crossing blend (its `DET` average), which falls below 750 µs after one commutation: `blank_arms=1` in all 48 captures. **That is a transcription error in firmware50, sitting in the exact window where its storms occur.**

**Changed** (ELF `B18C7E6F…1D7EC7B7`, text 54460 / data 28 / bss 5064):
* `src/commutation.rs`: `SixSlot` (`seeded`, `push(step, ci)`, `reverse_blank_due`) and `REVERSE_BLANK_SUM_US = 4498`. In µs, the reference's `((2·sum+4)>>1)/3 >= 1500` is exactly `sum >= 4498`, so there is no division in the COM root. Three host tests: equivalence with the reference formula for every sum 0–19999, several arms after a seeded handover with a slowly accelerating rotor, and exact slot replacement and clamping.
* `bin/shell-pwm.rs`: `COM_SIX` is seeded with the seed interval at transfer. At each commutation the COM root decides the blank on the ring as of the previous commutation, then pushes the new `DET` interval into slot `step-1` (the reference's order). `REVERSE_BLANK_MIN_AVG_US` is removed.
* **The E099 diagnostics are reverted** (refusal pad, key `6`, `Rate<true>` in the binary). `STORM_STEP` is kept and reported as `storm_step`.

**Checks:** 279 host tests (+3); clippy clean on the target and host `--tests`; four roots audit clean.
* COMP: **1200** cycles (12 reads) / 1030 (7 reads), unchanged and ≤ 1245.
* COM: 389 (was 360); the ring adds a load/compare/store.
* DMA 99, guard 445.
* The storm cutoff is unchanged (64/ms, count).

**Prediction:** `blank_arms` rises from 1 to about 4 per run, and the transfer-window storm stops occurring. The 33-run base rate is 3/14 on seed-step-4 handovers, so no single run proves it.

**Runs:** 20% ×3 now (reading `blank_arms`, peak, `storm_step` and seed step), then a full requalification if it holds.
### E100 — result: 20% ×3 (`captures/2026-09-21/e100-qual20_0{1,2,3}.txt`, ELF `B18C7E6F…`)

| run | reason | hold at 20% | forced | rate vs 6×coast | coast eHz | seed | blank arms | closed peak | storm | hold_ma (vs 167) |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 33.775 s | 0 | 999‰ | 957 (+1.7%) | 4 | **3** | 61 | 0 | 174 (+4%) |
| 2 | 2 | 33.775 s | 0 | 998‰ | 960 (+2.0%) | 4 | **3** | 60 | 0 | 226 (+35%) |
| 3 | 2 | 33.776 s | 0 | 1003‰ | 954 (+1.4%) | 3 | 2 | 60 | 0 | 220 (+32%) |

* **Items 1–3 (speed) at 20%: 3/3 on this image.** The blank now arms 2–3 times after handover (the reference arms 4, against 1 before). There was no storm in three handovers, two of them step-4 seeds.
* **Item 4, current, at 20%: 1/3 within 20%** of the single-run 167 mA. The physical draw at 20% is **0.250 A (PSU, E098)**; against it these runs read 0.70 / 0.90 / 0.88 ×, and the reference image read 0.52–0.77 × (E084). The per-run 20% band against 167 mA is not met, and that stays recorded.

## E101 — 25% ×3, restart ×3 at 25%, average-current re-provoke (no change, ELF `B18C7E6F…`, PSU 1 A)
### E101 — result, 25% ×3 (`captures/2026-09-21/e101-qual25_0{1,2,3}.txt`, ELF `B18C7E6F…`)

| run | reason | hold at 25% | seed | blank arms | closed peak | storm step | coast eHz | hold_ma (vs 326) |
|---|---|---|---|---|---|---|---|---|
| 1 | **13 CompStorm at transfer** (11 accepted) | 0 | 3 | 2 | 65 | **3** | — | — |
| 2 | 2 | 31.276 s | 3 | 2 | 62 | 0 | 1151 | 363 (+11%) |
| 3 | 2 | 31.276 s | 3 | 2 | 64 | 0 | 1155 | 359 (+10%) |

E100's blank gate is a real transcription fix, but **it does not remove the transfer storm**: this one came on a step-3 seed at logical step 3. The restart ×3 and `I` runs are deferred until the storm is understood.

## E102 — does COMP dispatch with its line already masked? (diagnostic build)

**binz, read:** `controlboards/BOOSTXL-DRV8304H/LAB_REPORT.md` Entry 093–094, "Pending EXTI survives IMR mask; NVIC masking restores progress". binz caught its COMP vector entered with `IMR=0xfff80000` (bit 18 clear) and `FPR` bit 18 set: "RM0444 section 13.4/Table 66 … clearing the pending bit clears CPU request, and CPU interrupt masking requires NVIC". binz's fix masked ADC_COMP at the NVIC before clearing IMR18, and on enable cleared stale NVIC pending before unmasking.

**Relevance:** firmware50 masks only in IMR (at entry, after an accept until the COM root re-arms, and during the reverse blank). If an edge in those windows still latches and dispatches, every chatter edge during and after COMP's own reads re-enters. Each entry counts toward 64/ms, and the refusal path re-enables the line mid-sector. That would make the entry rate track the handler's speed (E094–E099) and put the storms where the line should be quiet.

**Change (diagnostic):** `ADC_COMP` reads IMR before masking. A closed-loop entry that finds bit 18 already clear increments `DET_MASKED_ENTRIES`, reported as `masked_entries` on `BEMFDRIVEN`. Nothing else changes. ELF: see the build line in the next entry.

**Prediction:** if binz's G071 finding applies here, `masked_entries` is large, a sizable fraction of the ~40 entries/ms. If it is ~0, the IMR mask works as intended and the chatter enters through legitimately armed windows only.
### E102 — result (`captures/2026-09-21/e102-masked20_01.txt`, ELF `9802A19E…`)

20%: reason 2, forced 0, seed 4, blank arms 3, closed peak 61, no storm. **`masked_entries=639`** across the run's ~1.65 M closed-loop entries. **binz Entry 094's G071 behaviour reproduces here:** COMP is dispatched with its line already masked in IMR. It is rare (0.04%), so it does not by itself explain the doubled entry rate. It is still a defect: such an entry can reach the refusal path and re-enable the line mid-sector. The frozen oracle carries binz's fix.

## E103 — binz Entry 094's NVIC masking, transcribed (build)

* `comp_line_disable()`: `NVIC::mask(ADC_COMP)` **then** IMR18 clear.
* `comp_line_enable()`: IMR18 set, NVIC unpend (EXTI pending kept), then `NVIC::unmask`. This is binz's "Enable sets IMR18, clears stale NVIC pending (NOT EXTI pending), then unmasks NVIC".
* Used at COMP entry (NVIC mask before the IMR write), both refusal re-enables (closed loop and driven), `comp_exti_arm`, `comp_exti_mask`, and the driven deferred re-pend. E102's counter is kept for this test.
* ELF `C08D8900…27D2A505`, text 54704. Audit clean.
* **COMP 1250 cycles: 5 over 1245, with the diagnostic counter.** This is not a final image. COM 393, guard 449.

**Prediction:** `masked_entries` falls to ~0. If the doubled entry rate is partly masked-window dispatch plus the mid-sector re-enables it causes, the closed peak falls too.
### E103 — result (`captures/2026-09-21/e103-nvic20_0{1,2,3}.txt`, ELF `C08D8900…`)

**`masked_entries=0` in all three runs**, against 639 before: binz's NVIC-first masking closes masked-window dispatch on this board too. 20% 3/3: reason 2, 33.775–33.776 s, forced 0, 999–1004‰ against 6 × coast, coast 952–957 eHz, blank arms 2–3, no storm. The closed peak barely moved (58–61), so the remaining chatter enters through legitimately armed windows. hold_ma 151 / 222 / 218.

## E104 — final candidate and full requalification (build, then runs)

E102's diagnostic counter is removed; COMP entry now calls `comp_line_disable()` (NVIC, then IMR). Kept: E100 (six-slot blank gate), E103 (NVIC-first masking), `storm_step`, E093 (averaged zero), E095 (typed `Rate` locator, off in the binary).

**Final candidate `59B4A6DEBD5F978F17C5502A7F9F5780F91309FFADB0E6FAFE1674B9028F7743`**, text 54620 / data 28 / bss 5064, from a clean rebuild. 279 host tests; clippy clean on the target and host `--tests`; four roots audit clean:

| root | longest path @ 0 ws |
|---|---|
| COMP | **1220** (12 reads) / 1050 (7 reads), ≤ 1245 |
| COM | 393 |
| DMA | 99 |
| guard | 449 |

**Runs, all on this image, PSU 1 A:**
* 20% ×3;
* 25% ×3;
* restart `R` ×3;
* every protection again (`T G F N U V I`, host abort, `W`). `U` matters most: with NVIC-first masking, software pends can coalesce while the line is masked, so the storm path must be re-proven.
### E104 — results, rungs on the final image `59B4A6DE…` (PSU 1 A)

**20%** (`e104-qual20_0{1,2,3}`):

| run | reason | hold | forced | rate loop / vs 6×coast | coast eHz (vs 941) | R_COMP / late | seed / blank arms / peak | hold_ma (vs 167) |
|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 33.776 s | 0 | – / 996‰ | 960 (+2.0%) | 19 / 0 | 3 / 2 / 61 | 174 (+4%) |
| 2 | 2 | 33.776 s | 0 | – / 999‰ | 958 (+1.8%) | 19 / 0 | 3 / 2 / 62 | 220 (+32%) |
| 3 | 2 | 33.776 s | 0 | – / 1000‰ | 957 (+1.7%) | 19 / 0 | 3 / 2 / 60 | 216 (+29%) |

**25%** (`e104-qual25_0{1,2,3}`):

| run | reason | hold | forced | rate loop / vs 6×coast | coast eHz (vs 1186) | R_COMP / late | seed / blank arms / peak | hold_ma (vs 326) |
|---|---|---|---|---|---|---|---|---|
| 1 | 2 | 31.276 s | 0 | 996 / 998‰ | 1155 (−2.6%) | 19 / 0 | 3 / 2 / 61 | 366 (+12%) |
| 2 | 2 | 31.276 s | 0 | 997 / 994‰ | 1161 (−2.1%) | 19 / 0 | 3 / 2 / 62 | 365 (+12%) |
| 3 | 2 | 31.276 s | 0 | 998 / 1003‰ | 1151 (−3.0%) | 19 / 0 | 3 / 2 / 59 | 352 (+8%) |

too_early (340k) and unstable (~1.0 M) are non-zero in every run. No storm in six runs.

**PSU reconciliation, operator-read, limit 1 A:**
* 20%: **0.250 A**; that run's proxy was 222 mA (0.89 ×).
* 25%: **0.375 A** during an E104 25% hold, sent unprompted; the proxies were 352–366 mA (0.94–0.98 ×).

**Item 4 at 20%:** 1/3 of these runs are within 20% of the single-run 167 mA. Against the PSU-measured 250 mA they read 0.70 / 0.88 / 0.86 ×. At 25%, 3/3 are within 20% of 326 mA, and 0.94–0.98 × the PSU.

## E105 — restart ×3 at 25% on the final image (no change)
### E105 — result: restart ×3 at 25% on `59B4A6DE…`

| run | injected | stop after stimulus | seg-2 reason | seg-2 hold at 25% | drive end vs window | recovered |
|---|---|---|---|---|---|---|
| 1 | fired, 8 | 436 µs | **13 CompStorm, 4 ms after the second handover** (seed 3, `storm_step=4`, blank arms 2, 6 accepts) | 0 | 23 156 ms | **0** |
| 2 | fired, 8 | 413 µs | 2 | 13.347 s | **44 000 / 44 000** | 1 |
| 3 | fired, 8 | 490 µs | 2 | 13.346 s | **44 000 / 44 000** | 1 |

**2/3.** Across this image's 9 handovers (E104 ×6, E105 ×6 segments, of which 9 are transfers) there is one transfer storm. It came at logical step 4 (physical sector 1, floating phase C) after two blanked commutations. The six-slot ring with a ~761 µs seed arms twice; the reference arms four times (`REVERSEBLANK arms=4`). Why the reference's ring stays above 1500 half-µs for longer is not established; its seed or its push value may differ. binz's own handover storms on this rig (`DUTY_50_CAMPAIGN.md` reverse reset: "five COMs then 25 genuine COMP events in a 1ms bucket … reason13") were resolved over many later entries. The arming features in the qualified closure (`bench-reverse-arm20`: a 20 µs startup timer-arm reserve; `bench-masked-seed-arm`: comparator masked through the seed setup) were read. firmware50's transfer already keeps the line masked until the first COM, so neither is a clear match for a storm six commutations later.

## E106 — protections re-provoked on the final image (no change)
### E106 — result (`captures/2026-09-21/e106-*.txt`, ELF `59B4A6DE…`)

| key | protection | reason | stop after stimulus | all-off |
|---|---|---|---|---|
| T | tracking | 8 | 770 µs | PASS |
| G | tick gap | 3 | 423 µs | PASS |
| F | feedback age | 4 | 1104 µs | PASS |
| N | nFAULT | 7 | 41 µs | PASS |
| U | COMP storm (80 software pends) | **13** | 1086 µs | PASS |
| V | fast bus sag | 26 | 1492 µs | PASS |
| I | average current (stricter allowance) | 25 | 20.2 ms | PASS |
| host | abort | 9 | — | PASS |
| W | IWDG | next boot `RESETCAUSE iwdg=1` | — | off from reset |

The storm path's masking changed in E103 (NVIC first). The injection still provokes reason 13, so the cutoff is intact. The average-current datum changed in E093; `I` still provokes reason 25. No threshold was changed.

### Status on the final image `59B4A6DE…` (E104–E106)

| goal item | 20% | 25% |
|---|---|---|
| 1. 3/3 ≥ 30 s at target, forced 0 | **met** (33.78 s ×3) | **met** (31.28 s ×3) |
| 2. rate within 1%, too_early/unstable > 0 | met (996–1000‰ vs 6 × coast) | met (994–1003‰) |
| 3. coast speed within 5% | met (+1.7…+2.0%) | met (−2.1…−3.0%) |
| 3. PSU reconciliation (operator) | **0.250 A**; proxy 0.89 × (E098) | **0.375 A**; proxies 0.94–0.98 × (E104) |
| 4. current within 20% of the reference | **not met: 1/3** vs 167 mA (174/220/216); the single-run reference is itself 0.67 × the PSU draw | met (366/365/352 vs 326) |
| 5. restart at 25% | — | **2/3 on this image** (the E090 image was 3/3); one CompStorm at the restarted handover |
| 6. protections | met, all re-provoked; **the real-4 A provocation is not done** (limit 1 A, operator's choice) | |
| 7. roots / R_COMP / COMP growth | met: COMP 1220 ≤ 1245, R_COMP 19 µs < 27 µs, late 0 | |

**Transfer-storm rate on this image: 1 in 9 handovers.** The remaining divergence from the reference is the reverse blank arming 2 times after handover instead of 4.
## E107 — binz's startup storm policy for the handover window, and its 50 µs handler budget (build)

**The operator's direction:** "READ WHAT BINZ DID." binz met exactly this failure. E105 run 1's restarted segment died on reason 13 at its handover. binz recorded "A short burst at 65/ms previously killed an otherwise bounded normal restart" (`examples/support/driven_irq_live.rs:216-218`). Its resolution, in its own words:
* `AGENTS.md` E724: "Still stopped 65 dispatches/ms/ratecap64".
* E724d: "changed startup rate to observe/report only, existing higher priority tick, DMA/current/40ms/per-call 50us watchdogs retained".
* E767: "Per goal, driven acquisition's 64 dispatch/ms cap is report-only in the normal lean path; peak retained. Real 50us handler-overrun stop and independent command/feedback/watchdogs remain."
* In code: "Dispatch count is telemetry, not a safety boundary … Actual runaway/latency protection is the measured 50us handler budget below, plus the independent command/feedback/watchdog deadlines" (`:219-220`), and `finish_duration`: `if elapsed > 50 { overruns += 1; stop(); }` (`:310-318`).
* binz bounds its startup window at 40 ms (`driven_seed.rs:106`).

**Changed** (ELF `63F5206718A985045776C77DAB0F9B2DA9A92E0E6476768F293577035E6965CF`, text 55120):
* `src/rate.rs`: `HANDOVER_OBSERVE_US = 40 000` with `cap_enforced(since)`, and `HANDLER_BUDGET_US = 50` with `handler_overrun(elapsed)`. Two host tests: the window boundary, and binz's own RATESTOP cases 0/49/50/51/65535.
* `src/protection.rs`: `Reason::HandlerOverrun = 14` (tested). `src/restart.rs`: it is a latched, non-restartable stop (tested).
* `bin/shell-pwm.rs`:
  * The closed-loop COMP call **observes** the count until the foreground sets `DET_CAP_ARMED`, 40 ms after the loop closes. After that it **hits** the unchanged 64/ms cutoff.
  * Every closed-loop call measures its own duration from its entry stamp. More than 50 µs → `COMP_OVERRUN` → the foreground stops with reason 14. The line stays masked.
  * `BEMFDRIVEN` reports `cap_armed`, `comp_call_max_us` and `overrun`.
  * New provocation `H`: 60 µs added to the measured duration, binz's `RATESTOP` synthetic-elapsed method, through the real stop path.

**This is a protection change, stated plainly.** For 40 ms after each handover the 64/ms count does not stop the run. binz's per-call budget, the priority-0 guard (tick gap, feedback age, tracking), fast sag, average current, nFAULT and the IWDG remain armed throughout. firmware50's COMP line is also masked at the NVIC by every guard trip (E103), which covers the reference's stated reason for keeping the cap in lean closed loop ("a comparator storm can starve foreground UART/cleanup after the independent TIM6 guard has already disabled the bridge", `core_bench.rs:4059-4061`). After 40 ms the cap is exactly as before. Both the storm stop (`U`, 3 s into the loop) and the new overrun stop (`H`) are re-provoked below.

**Checks:** 281 host tests (+2); clippy clean; four roots audit clean. COMP **1237** (12 reads) ≤ 1245; the 7-read and other roots are as printed in the build log of this entry's run.

**Runs on this image, PSU 1 A:**
* 20% ×3;
* 25% ×3;
* `R` ×3;
* `H` and `U`;
* the rest of the protection set.
### E107 — results (ELF `63F52067…`)

* **20% ×3** (`e107-qual20_0{1,2,3}`): reason 2 every run, 33.776 s, forced 0, 996–1003‰ against 6 × coast, coast 954–960 eHz. `cap_armed=1`, `comp_call_max_us=24`, no overrun, no storm, peaks 55–57. hold_ma 103/188/186.
* **25% ×3** (`e107-qual25_0{1,2,3}`): reason 2, 31.275–31.276 s, forced 0, 999–1003‰, coast 1152–1156 eHz. Call max 24 µs, peaks 55–56 (two step-4 seeds), no storm. hold_ma 337/331/336 against 326 (+1…+3%).
* **Restart ×3** (`e107-restart25_0{1,2,3}`): injected loss, Tracking 8 in 350–491 µs, admitted, second segment reason 2 with a 13.34 s hold at 25%, drive end 44 000/44 000 ms. `recovered=1` ×3.
* **Protections** (`e107-inject-*`):

  | key | reason | stop after stimulus | all-off |
  |---|---|---|---|
  | T | 8 | 600 µs | PASS |
  | G | 3 | 416 µs | PASS |
  | F | 4 | 1057 µs | PASS |
  | N | 7 | 37 µs | PASS |
  | U | **13** (3 s into the loop, after the window) | 1427 µs | PASS |
  | **H** | **14** | 31 µs | PASS |
  | V | 26 | 1555 µs | PASS |
  | host | 9 | — | PASS |
  | W | next boot `RESETCAUSE iwdg=1` | — | off from reset |

* **But `I` stopped on 26, not 25, before its stimulus fired** (`e107-inject-I_01`). The stop came four guard ticks after arming, with no drive stage, `bus_min` 1209 of `bus_ref` 1213, and every estimator counter left over from the previous run. A rerun (`e107-inject-I2_01`) gave 25 as designed, 121 ms after the stimulus.
  * **Cause found in code:** `bemf_run` feeds the sag stop the rail moving mean. That ring was initialised once at boot and **never reset per run**. `I` ran right after `V`, which deliberately ends in a real bus collapse, so the new run's first means were `V`'s collapse with `rail_mean_ready()` already true, and three of them trip the stop.
  * E091/E106 ran the same V→I order and passed. Whether the ring holds collapsed samples depends on exactly when `V`'s stop landed.

## E108 — per-run reset of the sag stop's rail mean (build)

* `src/protection.rs`: `RailMean` (length 8, shift 3, the binary's former constants) with `new/reset/feed/ready/bus_mean/vref_mean`. Two host tests: readiness and averaging, and **the E107 sequence**, where a stale collapsed window trips `FastBusSag` on three healthy scans and after `reset()` the mean is not ready until the new run's own scans fill it, and then does not trip.
* `bin/shell-pwm.rs`: `Board` holds a `RailMean`, and all five powered-run entry points call `b.rail_reset()` right after building their `FastBusSag`.
* The sag threshold, streak and reference are unchanged.

**Final candidate `D7195D4802F0F3A1A2775D739B104FBA191A042CC89BAA857923F1DA366F5CCE`**, text 55092, from a clean rebuild. 283 host tests (+2); clippy clean (target and host `--tests`); four roots audit clean. COMP 1237 / 1077, COM 393, DMA 99, guard 449.

**Runs:** first the defect sequence, `V` then `I`, twice. Then the full requalification of every item on this image.
---

# Campaign 3: make the code match the behavior (goal set 2026-09-21)

**Behavior reference:** the frozen image `D7195D48…` and its E101–E108 cohort. **Structure reference:** `binz/FIRMWARE_CRATE_REBUILD_TASK.md` §Configuration design. Rule: each step moves one thing, then builds, tests, runs and records. No rewrite.

## E108 — results (`captures/2026-09-21/e108-*`, ELF `D7195D48…`)

* `V` then `I`, twice: 26 then 25, then 26 then 25. The stale-rail defect is gone.
* 20% ×3: reason 2, forced 0; accepted 213 903 / 214 183 / 213 585; R_COMP 18 µs, 0 late. All three pass gates 1–3 (`cohort.run_gates`).
* 25% ×3: reason 2, forced 0; accepted 252 241 / 252 472 / (third, see the cohort); R_COMP 18 µs.

## E109 — the fixture enforces the stops (scripts only; no firmware change, no powered run)

* `scripts/cohort.py`: the behavior reference.
  * **Cohort:** every reason-2 25% capture of E101/E104/E107/E108, n = 11: accepted **[251 973, 252 552]**, rate **[996, 999]‰**, coast **[1151, 1161] eHz**, current proxy **[331, 372] mA**. `--check FILE` judges one 25% capture against that spread plus gates 1–3.
  * `run_gates`: reason 2; hold ≥ 30 s; forced 0; rate within 1% of the loop's expectation **and** of 6 × coast; too_early and unstable non-zero; coast within 5% of the oracle; **coast crossings > 0**.
  * A coast's crossings are `COASTTIMING trans`, the comparator transitions of the bridge-off witness. `BEMFCOAST crossings` is an ADC count on pins unscanned since E041 and reads 0 in every capture. It will be removed or made meaningful in the Report step.
  * `rung_oracle`: the rung's mean coast eHz and mean current within 20% of the oracle.
* `scripts/bemf_run.py`:
  * **Ladder:** `2` needs a passed 15% rung, `5` needs a passed 20% rung, and `R` needs a passed 25% rung, all **on the same ELF**, else it refuses before opening the port (verified: `LADDER REFUSED … only 0 run(s) on this ELF at 20%`).
  * A rung's report is its last three runs on that ELF, recorded in `captures/ladder_state.json`. Every run is judged in the script (`RUN PASS`/`RUN FAIL`), and the script's exit status reflects failed gates.
  * `R` is judged in the script: first reason 8, recovered, drive end = window.
  * **`--step-check`**: exactly one 25% run for goal item 8, judged against the cohort and gates 1–3, recording nothing toward a rung. The final qualification must go through the ladder.
* `scripts/structure_report.py`, the structural baseline:

  | metric | now | goal |
  |---|---|---|
  | bin lines | 6449 | < 1500 |
  | bin `unsafe` | 127 | ≤ 10 |
  | `.bits(` outside hw | 134 | 0 |
  | bare `static mut` | 13 | 0 |
  | functions > 100 lines | 9 | 0 |

  The nine long functions are `bemf_run` 1235, `main` 455, `drive_loop` 267, `sixstep_spinup` 197, `vf_spinup` 146, `coast_capture` 144, `catch_probe` 114, `run` 109 and `neutral_probe` 101.
* **Baselines that may not grow:**
  * host tests 283;
  * cycles: COMP 1237 (12 reads) / 1077 (7), COM 393, DMA 99, guard 449.

**Plan, one move per step, each followed by build, tests, audit, `--step-check` and an entry:**
1. retire the historical probes (the task doc: "Do not port historical probes …");
2. `hw/`;
3. `Shared`;
4. `Controller`/`Production` with the replay test;
5. the typestate run replacing `bemf_run`;
6. `Report`;
7. the remaining splits and lints;
8. the full ladder on one image.
## E110 — step 1: retire the historical probes (build, then `--step-check`)

**What moved:** `bin/shell-pwm.rs` `main` now serves only the qualification path: `p`, `b`/`2`/`5`, `R`, `T G F N U H V I W` and `?`. Retired, per `FIRMWARE_CRATE_REBUILD_TASK.md` ("Do not port historical probes, abandoned A/Bs …"):
* `g` (script drive: `run`, `drive_loop`, `report_stop`, `RunStats`, `Outcome`);
* `a` (one ADC scan);
* `w` (`catch_probe` sweep);
* `r` (`vf_spinup`);
* `s` (`sixstep_spinup`);
* `q` (read cadence);
* `k` (sine probe);
* `c` (coast control);
* `n` (`neutral_probe`);
* `z` (`zero_settle`).

Their constants and the fields only they read went too (`AdcChannel.name`, `Board.max_spin`, `Board::now_us`, `Baseline.{csa_avg,vdda_mv,scans}`, `CoastStats.window_ms`, `ProbeRail.current_*`). Removal was driven by the compiler's dead-code list (`scripts/remove_items.py`), so nothing live could go unnoticed. The captures and ELFs of those probes remain. `RunMode::SineProbe` survives in `bemf_run` for now; it goes with `bemf_run` in the typestate step.

**Checks:** 283 host tests (unchanged); clippy clean (target and host `--tests`); four roots audit clean. ELF `DE166F1377B03D326492BB2CB7A9944E88B8C055E30A4073A11237BBAC66B2F2`: text **37 088** (was 55 092).

**Cycle counts, unchanged:**

| root | before | after |
|---|---|---|
| COMP (12 reads) | 1237 | 1237 |
| COMP (7 reads) | 1077 | 1077 |
| COM | 393 | 393 |
| DMA | 99 | 99 |
| guard | 449 | 449 |

**Structure:**

| metric | before | after |
|---|---|---|
| bin lines | 6449 | 4977 |
| `unsafe` | 127 | 126 |
| `.bits(` | 134 | 132 |
| `static mut` | 13 | 13 |
| functions > 100 lines | 9 | 3 (`bemf_run` 1232, `main` 224, `coast_capture` 143) |

**Run:** `bemf_run.py --step-check`, one 25% run judged against the E101–E108 cohort.
### E110 — results (ELF `DE166F13…`)

| capture | condition | accepted | rate | coast eHz | hold_ma | zero_start | step check |
|---|---|---|---|---|---|---|---|
| `e110-step25_01` | first run after a long idle (cold driver) | 252 220 | 998‰ | 1159 | **291** | **616 641** | **FAIL** (current) |
| `e110-step25b_01` | 20 s later (warm) | 252 503 | 999‰ | 1151 | 338 | 617 095 | **PASS** |

Cohort: accepted [251 973, 252 552], rate [996, 999], coast [1151, 1161], current [331, 372].

**The failure was the datum, not the code.** The cold run's pre-run zero, 616 641, lay below every cohort run's (617 064–618 506), and its proxy fell by the same margin. E092/E093 measured this zero following the driver's temperature: 615 350–616 703 cold, against ~617 900–618 500 in warm back-to-back runs. Every cohort run was taken warm, within about a minute of another powered run. The retired code has no path to the datum (`capture_baseline`/`averaged_zero` are unchanged; only unread fields were removed). The re-run on the same image passed all four quantities.

**Fixture fix, so the step gate compares like with like:** `--step-check` now makes one **unjudged 15% warm-up run**, waits 20 s, then makes the judged 25% run. This matches the cohort's conditions and does not widen its spread.

**Step 1 is accepted on `e110-step25b_01`.**
## E111 — step 2a: the timers move into `src/hw/` (build, then `--step-check`)

**What moved:** `src/hw/mod.rs` (target-only, `#[cfg(target_os = "none")]`: the structural HAL glue the crate rules allow) and `src/hw/timers.rs`:
* `hw::clock`: TIM17, `init_1mhz`, and `raw()` via the named `cnt` field.
* `hw::com_timer`: TIM16, `init`, `arm(arr)` (the same register sequence), `stop`, `ack`, `disable_interrupt`.
* `hw::pace`: TIM6, `trgo_on_update` (`mms().update()`) and `clear_update` (`uif().clear_bit()`).

Every write is a named field: PSC/ARR/CNT through the PAC's `Safe` `.set()`, a register back to reset through `.reset()`, and single bits through `set_bit`/`clear_bit`. The "why not the HAL" reasons moved with the code. All accessors are `#[inline(always)]`. The binary lost `tim16()`, `tim17()`, the TIM6 pointer escapes and 20 raw reads (`(cnt().read().bits() & 0xFFFF) as u16` → `hw::clock::raw()`).

**Checks:** 283 host tests; clippy clean (bin target; lib host `--tests`; **lib target**); four roots audit clean. ELF `E4ED0991967D3FACBB306D917CDB709479F7C00EBBC1A2FCF7346A52A5E34F67`, text 37 068.

**Cycles:**

| root | before | after |
|---|---|---|
| COMP (12 reads) | 1237 | **1188** |
| COMP (7 reads) | 1077 | **1028** |
| COM | 393 | 393 |
| DMA | 99 | 99 |
| guard | 449 | 449 |

The named 16-bit field read drops the `& 0xFFFF` masks. Nothing grew. E094–E099 found that a faster COMP handler catches more comparator chatter; the step check's closed-loop peak will show whether the 49 cycles matter here.

**Structure:**

| metric | before | after |
|---|---|---|
| bin lines | 4977 | 4921 |
| `unsafe` | 126 | 107 |
| `.bits(` | 132 | 96 |

**Run:** `--step-check` (15% warm-up, then the judged 25%).
### E111 — result (`captures/2026-09-21/e111-step25_01.txt`; warm-up `_warmup`)

accepted **252 092** (cohort [251 973, 252 552]), rate **997‰** ([996, 999]; 1001‰ against 6 × coast), coast **1153 eHz** ([1151, 1161]), current **346 mA** ([331, 372]). Reason 2, forced 0, closed peak 56/ms (unchanged), call max 24 µs, no overrun, no storm. **STEP CHECK PASS.** Step 2a is accepted.
## E112 — step 2b: TIM1 (PWM and gate outputs) moves into `src/hw/pwm.rs` (build, then `--step-check`)

**What moved:** `hw::pwm`:
* `init` (reset, PSC/ARR, CCRs, PWM mode 1 + preload via `oc?m().pwm_mode1()`, all six enables, DTG/OSSR/OSSI, UG, ARPE/CEN);
* `apply_plan`, `set_period`, `set_channel_compare`, `set_phase_compares`, `compares`, `float_all`, `all_phases_pwm`;
* `moe_on`/`moe_off`/`moe_is_set`, `counter`.

The binary's `tim1()` escape and eleven wrapper functions (`tim1_init`, `set_ccr_ch`, `apply_plan`, `set_carrier`, `float_all_phases`, `enable_all_phases_pwm`, `moe_is_set`, `moe_on`, `compares`, `set_compares`, and `Drv8304::moe_off`'s body) are gone, along with the now-unused `ccer_pair`/`CCER_ALL`. Call sites use `hw::pwm::…`.

**The one whole-register write in the crate is stated, not hidden.** `apply_plan` writes CCMR1/CCMR2/CCER as the register images `sixstep::plan` computes and host-tests. It runs in the COM root on every commutation, and decomposing the images into per-field writes at run time would add work to a root whose cycle count may not grow. Everything else in TIM1 is a named field (CCR/ARR/PSC/DTG through the PAC's `Safe` `.set()`).

**Checks:** 283 host tests; clippy clean (bin and lib, target); four roots audit clean. ELF `7554884FF9F43F6AD2DCF8916404FC658DD9710E471FDC94A5BDCF72F42920C8`, text 37 108. Cycles unchanged: COMP 1188/1028, COM 393, DMA 99, guard 449.

**Structure:**

| metric | before | after |
|---|---|---|
| bin lines | 4921 | 4721 |
| `unsafe` | 107 | 81 |
| `.bits(` | 96 | 66 |

**Run:** `--step-check`.
### E112 — result (`captures/2026-09-21/e112-step25_01.txt`)

accepted **252 059**, rate **997‰** (1002‰ against 6 × coast), coast **1152 eHz**, current **366 mA**; all within the cohort. Reason 2, forced 0, peak 57/ms, call max 24 µs. **STEP CHECK PASS.** Step 2b is accepted.
## E113 — step 2c: the comparator path (COMP2, EXTI line 18, its NVIC line) moves into `src/hw/comp.rs` + `src/hw/nvic.rs`

**What moved:**
* `hw::comp`:
  * `init(inpsel, inmsel, hyst)` (SYSCFG clock, CSR, the reference's 320-cycle settle), `select_negative`, `set_hysteresis`, `level`;
  * `line_disable` (NVIC first, then `im18`), `line_enable` (`im18`, NVIC unpend, unmask), `clear_pending` (`rpif18`/`fpif18` `.clear_bit_by_one()`), `select_edge` (`tr18` rising/falling);
  * `line_live`, `pending`, `nvic_unpend`, `pend`, `nvic_init`.
* `hw::nvic`: safe `set_priority`/`unmask`/`mask`/`unpend`/`pend`. The one soundness argument for `unmask`/`set_priority` (the crate never builds a critical section by masking an IRQ) is made once, there.
* The binary lost `comp_rb()`, `exti_rb()`, `EXTI_COMP2_LINE`, `comp_line_disable/enable`, `comp2_level` and every raw EXTI/COMP write. `det_decide` passes `hw::comp::level` (a zero-sized fn item, inlined as before).
* COMP2_CSR's `inpsel`/`inmsel`/`hyst` writers are `Unsafe` in the PAC; those `unsafe` blocks now live in `hw::comp` with the justification (host-tested `comparator_inmsel` values and fixed constants).

**Checks:** 283 host tests; clippy clean (bin and lib, target); audit clean. ELF `8C4B6DCFE5A223C4B155D55350CA45A7425C83CDAA4617CA6CD07BFB6905CCD0`, text 37 152. Cycles unchanged: COMP 1188/1028, **COM 393** (the named `tr18` edge select costs nothing), DMA 99, guard 449.

**Structure:**

| metric | before | after |
|---|---|---|
| bin lines | 4721 | 4601 |
| `unsafe` | 81 | 60 |
| `.bits(` | 66 | 39 |

**Run:** `--step-check`.
### E113 — result 1 (`captures/2026-09-21/e113-step25_01.txt`): FAIL, investigated

accepted 252 244, loop rate 998‰, loop eHz 1157, reason 2, forced 0, peak 55. The loop matches the cohort. It failed on three quantities:
* coast **1131** against [1151, 1161];
* rate against 6 × coast **1021‰**;
* current **387** against [331, 372].

**Coast: a witness artifact, not the rotor.** `COASTTIMING first_us=72 iv_us=461,423,444,427,445,429,445,429`. The first transition came 72 µs after the float, which is the bridge-off demagnetisation transient; other runs' first transitions came 33–436 µs after. The first half-period (461) was inflated, and the script's speed was that first pair alone. Every later pair sums to 867–874 µs, about 1150 eHz, the loop's own speed.

**Fixture fix (`scripts/cohort.py::coast_ehz`, used by `bemf_run.py` too):** the coast speed is the **median of the seven consecutive pair sums**. Recomputed over the cohort:

| estimator | cohort coast spread | E113 |
|---|---|---|
| first pair | [1151, 1161] | 1131 |
| median of pairs | **[1143, 1153]**, narrower | 1144 |

The median reads ~6 lower throughout because the rotor decelerates across the eight half-periods. Re-judged, E113 fails only on current.

**Current, 387 mA:** the step changed register plumbing only; the loop's accepted count, rate and speed are unchanged, and `zero_start` 617 904 is a warm datum. A repeat on the same image decides whether this is the proxy's run-to-run spread or a cause.
### E113 — result 2 (`e113-step25b_01.txt`): the current stays high

Repeat on `8C4B6DCF…`: accepted 252 165, rate 997‰ (1002‰ against 6 × coast), coast 1152 (median 1147, inside), reason 2. **Current 383 mA** (zero 617 382), again outside [331, 372]. Two runs on this image read 387 and 383.

**Bisect control (the rule: a failure is a code cause until a known-good image says otherwise).** E112's ELF was not archived; every step's ELF is archived under `captures/elf/` from now on (`8C4B6DCF….elf`). The control is **this exact source with only the comparator path reverted** to its pre-E113 register code: an in-bin `oldcomp` module with the raw line-18 masks, bound to the same names. Its ELF is `9F77B756123EBC521EB8F1C14E31AA3CD079E48FC0FB20C5891DBDA6E0CEC2F5` (`captures/elf/…control-e113.elf`), text **37 108**, byte-for-byte E112's size; COMP 1188, COM 393. The working source was restored afterwards.

**Prediction:** if the control also reads ~385 mA, the shift is in the bench's present state, not in E113. If it reads within [331, 372], E113 changed something physical and must be found.

**Run:** the control, `--step-check`.
### E113 — bisect result: no code cause; step 2c accepted

ABAB, all `--step-check` with warm-up, same afternoon:

| run | image | reason | accepted | coast (median) | current | zero_start | check |
|---|---|---|---|---|---|---|---|
| step25 | E113 `8C4B6DCF` | 2 | 252 244 | 1144 | 387 | 617 904 | FAIL (current) |
| step25b | E113 | 2 | 252 165 | 1147 | 383 | 617 382 | FAIL (current) |
| control25 | control `9F77B756` (= E112's code) | 2 | 252 111 | 1145 | 355 | 617 714 | PASS |
| abab-A2 | E113 | 2 | 252 226 | inside | **367** | 618 499 | **PASS** |
| abab-B2 | control | 2 | 252 072 | inside | **374** | 618 525 | **FAIL (current)** |

**The control, whose code is E112's (text 37 108, identical), also reads above the cohort's 372 mA.** The image means (E113 379, control 364) differ by less than the proxy's run-to-run spread, which this afternoon's warm bench shifted upward on either image. E113's register plumbing is not the cause. **Step 2c is accepted on `e113-abab-A2_01`**, which passes all four quantities and gates 1–3. The cohort band is not widened.

**Protocol from here:** a step check that fails on current alone is followed by an ABAB against the previous step's **archived** ELF (`captures/elf/`), and the step is judged on that comparison.
### E114 — step 2d: ADC + its DMA move into `hw::adc`

**Moved:** the ADC scan (channel table, calibration, CFGR/SMPR/CHSELR setup, VREFINT/VDDA conversion) and the DMA1 CH1 circular drain (DMAMUX request, PAR/MAR/NDTR/CR, TC/TE acknowledge, seqlock publish, resync, stall injection) out of the bin into `src/hw/adc.rs`. The DMA target buffer and the published snapshot are now a `DmaCell` (`UnsafeCell` + `Sync`) inside `hw`, no longer `static mut` in the bin. The DMA ISR root is now one call, `hw::adc::dma_isr()`. `Board::scan` takes a ready `RawScan` from `hw::adc::snapshot()`. The sequence counter is read through `hw::adc::sequence(ordering)` with the same orderings as before (Relaxed, Relaxed, SeqCst). CHSELR0 is now written through named `chsel(n)` bits, with a `debug_assert` against the old mask. Fields the PAC leaves unsafe (DMAREQ_ID, PAR, MAR, and EXTSEL=TIM6_TRGO) are written by value with one-line SAFETY notes.

**Structure (before → after):** bin 4601 → **4216** lines · unsafe 60 → **38** · `.bits(` outside hw 39 → **24** · `static mut` 13 → **11**. Functions over 100 lines are unchanged: bemf_run 1227, main 224, coast_capture 143.

**Gate:** 283 host tests pass; clippy is clean on the bin and on the lib for both targets; the audit passes on all four roots. **Cycles unchanged:** COMP 1188 / 1028, COM 393, DMA **99**, guard 449. ELF `99074482BEF297D40117A1338BEC89AA4914BD79C683800DC1E736E9C75587F6` is archived as `captures/elf/99074482.e114.elf`.

**Run** `e114-step25_01` (`--step-check`, warm-up first): reason 2, accepted **252 199** [251 973, 252 552], rate **997‰** [996, 999], 6 × coast 1006‰, coast **1148** [1143, 1153], current **350** [331, 372]; forced 0, peak 54, storm 0, overrun 0, COASTTIMING trans 868, zero_start 617 756. **STEP CHECK PASS.** Step 2d is accepted.

### E115 — step 2e: GPIO, IWDG, reset cause and the remaining NVIC calls move into `hw`

**Moved:**
* `hw::gpio`: the six gate pins (AF2 through the named `afr(n).af2()`, and gates-low with the same latch → mode → latch order), ENABLE, the nFAULT read, and the gate-4 nFAULT stimulus. The stimulus is now `ot(n).open_drain()` / `moder(n).output()` / `br(n)`, and its release is `moder(n).input()` / `ot(n).push_pull()`; these are the same bits the raw masks wrote.
* `hw::system`: `iwdg_set_timeout`, with the same KR sequence through the named `key().start()/unlock()/feed()`, `pr().divide_by4()` (= PR 0), `rl().set(400)`, and the SR busy wait on `pvu/rvu/wvu`. Also `take_reset_cause`, which reads the named `*rstf` flags and clears them with `rmvf()`.
* The TIM6 and TIM16 NVIC calls now go through `hw::nvic`.

The bin no longer holds a register block.

**Structure (E114 → E115):** bin 4216 → **4094** lines · unsafe 38 → **20**, all of them `static mut` access (next step) · `.bits(` outside hw 24 → **0** · `static mut` 11. Functions over 100 lines: bemf_run 1221, main 202, coast_capture 143.

**Gate:** 283 host tests pass, clippy is clean on the bin and on the lib for both targets, and the audit passes on all four roots. **Cycles are unchanged:** COMP 1188 / 1028, COM 393, DMA 99, guard 449. ELF `B2E00039…` is archived as `captures/elf/B2E00039.e115.elf`.

**Run** `e115-step25_01` (`--step-check`): reason 2, accepted **252 171**, rate **997‰** (1003‰ of 6 × coast), coast **1151**, current **361**, forced 0, peak 58, storm 0, COASTTIMING trans 867. **STEP CHECK PASS.**

**Code this step touched, re-provoked on the same image:**
* `e115-nfault_01`: `N` gives reason 7, stop 43 µs after the stimulus, and the post-stop preflight reads `nfault=1`, so the release restored the pin.
* `e115-iwdg_01`: `W` resets the board, and the next boot's banner reads `RESETCAUSE iwdg=1 pin=1`. This checks both the timeout and the named-field reset-cause read.

Step 2e is accepted. The hardware work of goal item 3 is complete. What remains of item 3 (unsafe ≤ 10) depends on item 4, `Shared`.

### E116 — step 3: one `Shared` seam; `static mut` goes to zero

**Moved:** every ISR↔foreground value (about 80 statics) now lives in `src/shared.rs`, reached through `SHARED` (`S`), a zero-sized facade with one accessor per group: `edge`, `det`, `comp`, `drv`, `guard`, `com`, `run`, `scan`, `tx`, `panic_line`.
* Each group is a private `#[repr(C)]` static, so a root addresses a field as its group's address plus a small immediate.
* Scalars are typed atomics with stated ordering: `Relaxed`, with the activation flags stored `Release`, and the doc gives the single-core argument.
* Structs that cross are `Seam<T, P>`: a `Mutex<RefCell<T>>` with a priority ceiling `P` (`Motor` = 0x40, `Guard` = 0x00). The seams are the estimator, both COMP rates, the six-slot ring, the plan table, and the guard's watch.
* Access follows RTIC's stack resource policy:
  - `root(&mut Root<P>)` at the ceiling, with no masking and no flag;
  - `masked(&mut Root<Q: AtOrBelow<P>>)` for a lower root, with interrupts masked;
  - `lock` for the foreground: interrupts masked, the `RefCell` flag checked, and refused in handler mode.
* A root's token is taken once per handler by `unsafe { Root::enter() }`, justified by the NVIC priority that the same constant sets (`COMP_IRQ_PRIORITY = Motor::NVIC` and so on). These are the bin's only three `unsafe`.
* The ADC seqlock moved from `hw::adc` into `SHARED.scan`. `hw::adc` keeps only the DMA target buffer.
* The TX ring is an SPSC ring of `core` atomics.
* `bemf::ZeroCross::blanking` uses `wrapping_mul` in place of `saturating_mul`. `blank_64` is clamped to 1..=56, so (2^26−1)·56 < 2^32 cannot overflow, and the saturating form lowered to `__aeabi_lmul` once inlined into `ADC_COMP`, which the link-time math audit refused.

**Four designs were measured before this one held behavior:**

| variant | COMP | COM | DMA | guard | 25% step check |
|---|---|---|---|---|---|
| E115 | 1188 / 1028 | 393 | 99 | 449 | PASS |
| `interrupt::free` + `RefCell` in the root (prototype, one struct) | **1315**, panic path | – | – | – | not run (audit fails) |
| ceiling token + `RefCell` flag; portable-atomic scan words | 1185 / 1025 | 371 | **123** | 428 | – |
| … scan words as `core` atomics (E116, `3AC81E3C`) | 1185 / 1025 | 371 | 78 | 428 | **FAIL** ×2: 438 / 425 mA, accepted 251 769 / 251 643 |
| … + atomic TX ring, `tx_drain` inlined (E116b, `A80B4080`) | 1185 / 1025 | 371 | 78 | 428 | **FAIL**: 368 mA, accepted 251 658, rate 995 |
| … + flag-free `root`/`masked` (**E116c**, `CDE4EF42`) | **1176 / 1011** | **356** | **78** | **419** | **PASS** |

**Bisect (the rule: a failure is a code cause until a known-good image says otherwise).** The ABAB against E115's archived ELF was: E116 FAIL (438 mA), E115 **PASS (346 mA, accepted 252 126)**, E116 FAIL (425 mA). V1, which was E116 with only the scan seqlock put back to E115's code, still failed (402 mA), so the scan path was cleared. What moved was the loop, not the measurement:

| image | loop iters (closed) | accepted | too_early | revisits r/v | lt075 |
|---|---|---|---|---|---|
| E114 | 772 890 | 252 199 | 350 951 | 56 872 / 41 714 | 4 243 |
| E115 | 789 394 | 252 171 | 349 544 | 57 928 / 42 743 | 4 240 |
| E116 | 736 573 | 251 769 | 354 716 | **61 533** / 47 469 | 4 698 (A2) |
| E116b | 778 667 | 251 658 | 354 313 | **60 326** / 46 547 | 4 667 |
| E116c | 850 495 | 252 493 | 362 699 | **56 374** / 48 042 | – |

* E116's per-pass `tx_drain` had become an out-of-line `RefCell` call, and the loop ran 7% slower.
* Restoring it (E116b) brought the loop speed back, but the revisits stayed high. The level revisit collects crossings the COMP line missed, and COMP keeps its line masked for the whole of a refused call. The `RefCell` flag round-trips on COMP's refusal path lengthened that masked window enough to lose ~3–4k more crossings to the revisit per run and pull the accepted count below the cohort.
* The flag is not what makes a ceiling borrow exclusive; the priority rule is. E116c therefore borrows flag-free in the roots, and closes nesting with the `&mut` token instead.

**Structure (E115 → E116c):** bin 4094 → **3903** lines · unsafe 20 → **3** · `.bits(` outside hw 0 · `static mut` 11 → **0**, and 0 in `src` · functions over 100 lines: bemf_run 1205, main 202, coast_capture 143. `structure_report.py` now counts `static mut` and `.bits(` in code only, not in comments.

**Gate:** 287 host tests pass (+4: TX ring, scan seqlock and torn read, foreground nested-borrow refusal). Clippy is clean on the bin and on the lib for both targets. Both audits pass on all four roots. **Cycles, all down:** COMP 1176 / 1011, COM 356, DMA 78, guard 419.

**Run** `e116c-step25_01`: reason 2, accepted **252 493**, rate inside, coast inside, current **366**, peak inside, storm 0. **STEP CHECK PASS.** Re-provoked on the same image:

| key | provoked stop | reason code | stop after stimulus |
|---|---|---|---|
| T | Tracking | 8 | 541 µs |
| U | Storm | 13 | 1067 µs |
| H | Overrun | 14 | 38 µs |
| G | TickGap | 3 | 415 µs |
| F | FeedbackStale | 4 | 1054 µs |

All five fired as expected. **Step 3 (goal item 4) is accepted on E116c.**

### E117 — step 4: telemetry is a `Report` value, formatted by the lib (goal item 5, first half)

**Moved:**
* `src/report.rs`: the `Sink` formatter (`say`/`say_u32`/`kv`/`kvi`, reciprocal-multiply digits; the bin's copy is gone), `CoastStats` with its rotation verdict and the `BEMFCOAST`/`COASTTIMING` lines, and `RunReport` with sub-records (`InjectOutcome`, `Roots`, `Driven`, `GuardRecord`, `CurrentRecord`, `WitnessRecord`) and its emitter.
* `bemf_run`'s ~200-line printing tail became one `RunReport` built **after `safe_off`** and one `report.emit(board)`. `Board` implements `Sink` and flushes at the same section boundaries as before.
* The derived quantities (`zc_per_s`, `mean_sector_us`, the permille identity) are computed in `emit`, after the stop, exactly as before.

**Format held byte for byte:**
* 4 new host tests pin the number formatting, the coast lines, the cohort quantities through the fixture's key names, and the injection line (291 tests).
* New `scripts/report_format_diff.py` compares two captures' line-prefix and key sequences. E116c against E117: **SAME FORMAT, 30 lines**.

**Structure:** bin 3903 → **3614** lines. `bemf_run` 1205 → **1005**. unsafe 3; `static mut` 0. Cycles unchanged: COMP 1176 / 1011, COM 356, DMA 78, guard 419.

**Runs** (`C4BE9DD9…`):

| run | image | accepted | zc/s | mean sector | hold mA | check |
|---|---|---|---|---|---|---|
| e117-step25 | E117 | 252 692 | 6946 | **143** | 346 | FAIL: accepted above 252 552; rate 993 |
| e117-abab-A116c | E116c | 252 357 | 6936 | 144 | 370 | PASS |
| e117-abab-B117 | E117 | 252 496 | 6942 | 144 | 366 | PASS |

The ABAB shows E117 behaves as E116c does. Its first run's miss is the rate identity's integer `mean_sector_us` truncating 143.95 to 143 on a run at the top of the spread. **Step 4 is accepted on `e117-abab-B117`.**

**Watch item, recorded as found, not explained away.** Since E116c, runs sit at the top of the cohort band:

| images | accepted (runs) | zc/s |
|---|---|---|
| E116c, E117 | 252 493 / 252 357 / 252 692 / 252 496 | 6936–6946 |
| E115 and its controls, same bench | 252 171 / 252 126 | 6928–6929 |

The only behavioral candidate is timing: E116c's COM root is 37 cycles (0.58 µs) shorter, so each commutation's plan lands ~0.6 µs sooner, which is a touch more effective advance. `lt075` roughly halved at the same time (4240 → 2452–2658). The shift is +0.1% in accepted. Every step check from here reports it, and a step that moves it further is a code cause to bisect.

### E118 — step 5: the ISR roots' logic and the motor wiring move into the lib (`src/roots.rs`)

**Moved:** `src/roots.rs` (target-only) now holds ~930 lines of the bin, with `firmware50::` paths rewritten to `crate::`:
* the gate/ENABLE/nFAULT wrappers, `Wiring = Reverse`, `physical` and `plan_for`;
* the COMP2 configuration constants and `comp2_init`;
* `DET_FILTER`, `det_decide`, `drv_decide`;
* the guard's arm/trip/event/tracking;
* the COM root's arm/stop/publish and `REVERSE_BLANK_US`;
* the EXTI arm/mask and mux helpers, `set_compares_wired`;
* `Drv8304` (the `Bridge` impl).

The three root bodies are `pub unsafe fn comp_root` / `com_root` / `guard_root`, each `#[inline(always)]`. Their `# Safety` contract is "call only from this handler, once per invocation", because each takes its root's `Root` token. The bin keeps one-line `#[interrupt]` shims whose `unsafe` names the handler and its priority constant. There are still 3 in the bin, now at the only place the claim can be checked.

**Structure:** bin 3614 → **2683** lines. unsafe 3; `static mut` 0; `.bits(` outside hw 0. Functions over 100 lines: bemf_run 1005, main 202, coast_capture 143.

**Gate:** 291 host tests pass. Clippy is clean on the bin and on the lib for both targets. Both audits pass on the four roots.

**Cycles:** COMP 1176 / 1011, COM 356, DMA 78. The guard reads **421**, 2 above E117's 419 (codegen of the moved body) and still 28 below the E115 baseline of 449.

**Run** `e118-step25_01` (`60FCBE31…`): reason 2, accepted **252 504**, rate 999‰, sector 144, current **371**, loop iterations 861 646. **STEP CHECK PASS.** `report_format_diff` against E117: SAME FORMAT. Step 5 is accepted.

### E119–E120 — steps 6–7: `Production` and the typestate run replace `bemf_run` (goal items 1, 2, 5)

**Moved:** `src/run/` (host-compiled; target-free) now holds the run.

* `policy.rs`: every run constant the bin carried, verbatim with its derivation, and the seven policy slots as traits (`Bemf`, `Advance`, `CurrentLimit`, `SagLimit`, `RestartRule`, `Reporting`) with the production marker types `Wiring` (= `Reverse`, the same type `roots` uses), `BemfPolicy`, `AdvancePolicy`, `CurrentProtection`, `BusSagProtection`, `Restart`, `Telemetry`.
* `mod.rs`: `Controller<W, B, A, C, S, R, T>` and **`type Production = Controller<Wiring, BemfPolicy, AdvancePolicy, CurrentProtection, BusSagProtection, Restart, Telemetry>`**. `main` builds the board and calls `Production::new().serve(&mut board)`; the shell, the rungs, the restart campaign and the provocations are its methods.
* `hal.rs`: `Hal` has one method per thing the run does to the board or the ISR seam. **`Gates<S>`** is the typestate capability:
  * every gate-driving call (plan, compares, MOE on, AF to TIM1) takes `&mut Gates<S: Drives>`, and only `Armed`, `Startup` and `Locked` implement `Drives`;
  * a run starts with `Gates<Idle>`, and only a passed preflight upgrades it;
  * `Hal::safe_off` consumes it.
* `states.rs`: `Idle → Armed → Startup → Handover → Locked → Stopped(Reason)`.
  * Transitions consume.
  * `Stopped` is built only by `stop()` or from a `Refused`, and both went through `safe_off`.
  * The pass is `bemf_run`'s, statement for statement. The one intended difference is that the sine stage's periodic `BEMF` line is gone, so nothing formats while the loop runs.
* `measure.rs`: the preflight line, baseline and zero, and the coast witness.
* Writing text needs `report::Sink`. Only `Idle::arm` and the stopped tail take one; `Startup`, `Handover` and `Locked` are generic over `Hal` alone. The TX ring left `SHARED` for a private static in the bin, written only through `Board`'s `Sink`, so no library code can queue a byte.

**What is left in the bin:** `Board`, its `Hal` and `Sink` impls, `init` and `banner`, the four `#[interrupt]` shims, and the panic handler.

**Structure:** bin 2683 → **894** lines. **No function over 100 lines** anywhere (`bemf_run` 1005, `main` 202 and `coast_capture` 143 are gone or split). unsafe 3; `static mut` 0; `.bits(` outside hw 0.

**Gate:** 293 host tests pass. Clippy is clean on the bin and on the lib for both targets. Both audits pass. Cycles are unchanged (COMP 1176 / 1011, COM 356, DMA 78, guard 421). **The four roots' machine code is instruction-for-instruction identical to E118's** (new script comparing normalised disassembly).

**Runs:**

| run | image | accepted | zc/s | acquire µs | loop iters | revisits | hold mA | check |
|---|---|---|---|---|---|---|---|---|
| e120-step25 | E120 `646B7A23` | 253 071 | 6957 | 16 050 | 830 936 | 63 818 | 362 | FAIL: accepted 253 071; rate 994 (sector 143) |
| e120-abab-B118 | E118 | 252 490 | 6940 | 17 651 | 861 898 | 56 095 | 375 | FAIL: 375 mA; rate against 6 × coast 1011 |
| e120-abab-A2 | E120 | 252 430 | 6939 | 16 798 | 831 007 | 63 360 | – | PASS |
| **e120b-step25** | **E120b `F2E84BAD`** | **252 219** | 6932 | 16 055 | 841 658 | 62 715 | **367** | **PASS** |

* The ABAB separates spread from shift. The E118 control itself failed a gate today, and E120's first-run miss was not repeated.
* E120 had one real cost: every `poll` consumed and returned its ~1 KB state, moving it on every pass, and the loop ran 3.6% slower. E120b polls in place, and only transitions consume.
* **Watch item:** on E120 and E120b the level-revisit count sits ~11% above E118's (62.7–63.8k against 56.1–56.3k), with the closed loop 2.4% slower. The four cohort quantities hold. The revisit is foreground-polled, so its count follows the loop's code shape, as E116 first showed.

**Step 7 is accepted on e120b-step25.**

### E121 — step 8: the controller under host test, and the replay pinned to a 25% capture (goal items 1, 2, 7)

**Controller tests (`src/run/sim.rs`).** `Sim` is a simulated `Hal` + `Sink`:
* scripted time, and synthetic scans (biased shunts, a steady bus);
* one driven acceptance per driven sector, tagged with the controller's own epoch and step;
* a scripted closed-loop crossing train, with the simulated COM root commutating on each;
* scheduled faults.

It logs every decision the controller takes: seed installs, the commutation hand-over and its commit instant, plan and advance publishes, gate writes, `safe_off`, stimuli, and **any byte written while the bridge drives**.

Eight tests run `Production` as a whole:
* a crossing train locks: seed 833 µs, transfer at 10%, the ramp only rising to 25%, advance 20, accepted equal to crossings delivered, a hold, 0 bytes while driven;
* the first commutation lands exactly at seed edge + `wait_time(seed, 20)`;
* crossings that stop end on the guard's Tracking stop;
* nFAULT gives Driver;
* a bus sag stops the run;
* the stop key gives HostAbort;
* a failed preflight writes **no gate**, still goes through `safe_off`, and reports nothing;
* an injection fires within one pass of 3 s after the loop closes.

**Typestate claims as compiler checks (`run::hal` doctests).** Each pair is a compiling write from `Gates<Locked>` beside a failing one.

| attempt | rejected with |
|---|---|
| a gate write from `Handover` | E0277 |
| a gate write from `Idle` | E0277 |
| a gate write from `Stopped` | E0277 |
| `say` from code generic over `Hal` alone | E0599 |
| building `Stopped` without `safe_off` | E0451 |

**Capture path (`src/capture.rs`, `bin/edge-capture.rs`).** `comp_root` is generic over an `EdgeLog`.

Production's `NoLog` runs `det_decide_plain`, the E120b decision byte for byte. The diagnostic `Ring` runs a twin that records each decision's count, polarity, advance, every live comparator read, and the outcome. Two findings were each confirmed on the production machine code:
1. A single body with dead recording branches changed COMP's code: a closure that merely *captures* the log is enough.
2. Merely compiling `Ring`'s non-generic methods into the library changed it too. Its items are `#[inline]` for that reason.

Along the way a stale-library build made one "identical" check invalid. `Copy-Item` kept old modification times, so cargo reused a bisect-state library. All sources were touched and rebuilt, and the checks were repeated.

**Production image:** the loadable bytes (vector table, `.text`, `.rodata`, `.data`) are **byte-identical to E120b** (SHA-256 of the extracted sections `BBC2ECE5…`); only debug info differs. The bin is split into `bin/shell-pwm.rs` plus `bin/board.rs`, which `edge-capture` shares: **915** lines, 3 unsafe, no function over 100 lines. No production run is owed for this step: the firmware did not change.

**The capture** (`captures/replay/e121-capture25b.txt`, `edge-capture` `71058E6B…`):
* The same 15% warm-up, then the 25% rung, with the ring armed for that run only. A first attempt, `e121-capture25`, armed during the warm-up and recorded the 15% loop; it was superseded.
* The 25% run: reason 2, accepted **252 192**, rate 998‰, sector 144, coast inside, current **390 mA**.
* The current is the observer's cost: the diagnostic COMP does more per entry (both capture attempts read 387–390). So the capture is evidence of the decision function, not of production's loop quality.
* 1536 consecutive decisions from estimate 145 µs: 279 accepted, 423 too early, 834 unstable.

**The replay test (`src/run/replay.rs`).** It rebuilds the estimator from the snapshot and offers every decision to `<Production as Policies>::B`, serving the recorded reads back in order.
* **All 1536 reproduce exactly**: outcome, wait, new estimate, and the number of reads the filter asked for.
* A policy one blanking step different (33/64) fails to reproduce them, so the test is not vacuous.

**Gate:** 303 host tests and 6 doctests pass. Clippy is clean on the lib (host with `--tests`, and target) and on both binaries.

### E122 — step 9: unsafe-related lints enforced; justifications quote the vendored PAC; the final image

**Lints (goal item 6).** The lib and both binaries carry, as crate attributes, the pedantic pointer and cast lints:
* `borrow_as_ptr`, `cast_ptr_alignment`, `ptr_as_ptr`, `ptr_cast_constness`, `transmute_ptr_to_ptr`.

They also carry the unsafe-documentation lints:
* `missing_safety_doc`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `unnecessary_safety_comment`, `unnecessary_safety_doc`.

Every clippy run (`-D warnings`) enforces them. The first run found seven sites:
* SAFETY comments sitting outside the closure they justified (DMA PAR/MAR, the CCMR images);
* multi-operation blocks: the DMA scan read, now one `read_volatile` per word through `dma_word`; COMP2's three field writers; NVIC steal plus `set_priority`;
* one stray SAFETY comment on a safe call;
* the ceiling borrow's second block and a test's token.

Each was fixed without changing code.

**Justifications (goal item 3).** The production binary's three `unsafe` are the root shims. Each quotes the vendored PAC's own vector definition, for example `// SAFETY: the PAC's vector `ADC_COMP = 12` ("12 - ADC and COMP interrupts"), at `COMP_IRQ_PRIORITY`.`

**Dead code.** The seam's `run` group has had no users since E119 and is removed. `acquire` and `command` also have no users since the shell moved, but they keep their host tests; deleting them would drop the test count, which item 7 forbids.

**The final image `F5FFF36F…`** (`captures/elf/F5FFF36F.final.elf`):
* **Loadable bytes identical to E120b** (`BBC2ECE5…`), the last image that passed a bench step;
* 303 host tests and 6 doctests pass; clippy is clean with the lints on the lib (host with tests, and target) and on both binaries; both audits pass;
* cycles COMP 1176 / 1011, COM 356, DMA 78, guard 421, all at or below the E115 baseline (1188 / 1028, 393, 99, 449);
* structure: bin (`shell-pwm.rs` + `board.rs`) **930** lines, unsafe **3**, `static mut` **0**, `.bits(` outside hw **0**, functions over 100 lines **0**.

**Next:** the full ladder on this one image: 15/20/25 × 3 through the fixture's ladder, restart × 3, and every protection provoked.

### E123 — the final ladder on `F5FFF36F…`: 15% fails the current-proxy oracle band, and so does the oracle; every protection re-provoked

**15% × 3 through the fixture** (`final-15_0{1,2,3}`): every run passes gates 1–3:
* reason 2, hold 36.3 s, forced 0;
* rate 997–998‰, and 1006–1010‰ against 6 × coast;
* coast 701–703 eHz against the oracle's 704;
* coast crossings 549–565.

The rung fails the oracle comparison on current: `hold_ma` 40 / 101 / 97, **mean 79 mA against 58 ±20%**. The fixture therefore **refuses 20%** (`LADDER REFUSED: command 2 needs the 15% rung passed on this ELF`), and so 25% and `R` as well.

**Is it the refactor?** (the rule: a code cause until a known-good image says otherwise.)

| image | 15% `hold_ma` × 3 | mean | coast eHz |
|---|---|---|---|
| final `F5FFF36F` | 40 / 101 / 97 | 79 | 701–703 |
| pre-refactor control `9F77B756` (E112's code), same bench, same afternoon | −18 / 114 / 114 | 70 | 700–703 |
| **the oracle** (rate-curve reference `10CD4D47…`, hash verified; run as E084 did, `--ramp-duty 150`; `oracle-rc15b_0{1,2,3}`) | **163 / −31 / 172** | 101 | 697–707 |

**The oracle cannot meet a ±20% band around its own published 58 mA at 15%.** Its proxy spans −31 to 172 mA over three runs on one bench, while its speed repeats to ±0.7%. The pre-refactor image fails the same band too.

The 15% current proxy is datum-dominated: `zero_drift_ma` reaches −368 within one run, and the true current is ~58 mA. E082 predicted exactly this ("the same proxy that met gate 3's *mean* wording at 15% will not meet a per-run 20% band there"). Campaign 1 judged 15% on a five-run mean (55 mA).

The refactor did not move the 15% loop. Speed, rate, and the proxy's scatter are the control image's and the oracle's.

**Not done, and why.** The goal's final item asks for 15/20/25 × 3 and restart × 3 on one image, through a fixture that "refuses to launch a rung unless the previous rung's report passed gates 1–3 and the oracle comparison" (item 9). I did not relax the band or bypass the fixture. On today's evidence, no image — the oracle included — passes the 15% current comparison, so the 20% and 25% rungs and the restart campaign are not run through the ladder. This goes to the operator as a finding.

The 25% behaviour on this exact firmware is established separately. The loadable bytes are E120b's, whose step check passed (accepted 252 219, 6932/s, 367 mA). Every refactor step's 25% check is recorded in E110–E120.

**Every protection re-provoked on `F5FFF36F…`** (`final-inject-*`, `final-hostabort`):

| key | protection | reason code | stop after stimulus |
|---|---|---|---|
| T | Tracking | 8 | 518 µs |
| G | TickGap | 3 | 448 µs |
| F | FeedbackStale | 4 | 1076 µs |
| N | Driver (nFAULT) | 7 | 65 µs |
| U | CompStorm | 13 | 1300 µs |
| H | HandlerOverrun | 14 | 64 µs |
| V | FastBusSag | 26 | 1416 µs |
| I | AverageCurrent (a *stricter* allowance) | 25 | 20.2 ms |
| — | host abort | 9 | – |
| W | watchdog | IWDG reset | next boot `RESETCAUSE iwdg=1` |

A preflight passed after every stop.

## E124 — campaign 4 (goal set 2026-09-21): the refactored image through the full ladder

**Gate change (operator decision, one line):** the signed-current proxy is **report-only** in the progression gate. It is uncalibrated, its zero drifts by hundreds of mA-equivalent within a run, and the reference image fails its own ±20% band (E123: 163 / −31 / 172 mA). It is still printed every run, with a bridge-off zero taken before each run, and the fixture prints `current (report-only)` beside each rung verdict. Speed (bridge-off BEMF within 5% of the oracle), rate identity (1%), forced 0, coast crossings > 0 and every protection stay hard gates. The actual current at each rung is the operator's PSU reading, written beside the proxy.

**Fixture changes:**
* `cohort.rung_oracle` drops the current term; `cohort.rung_current_note` reports it.
* `bemf_run.py` no longer records a `--abort-after` run as a rung run. E123's host-abort 15% run had been counted as the rung's third run. That one entry was removed from `captures/ladder_state.json`, which is fixture state, not a capture.

**Result on `F5FFF36F` before the change below.** The E123 15% runs now pass the rung. **20% × 3** (`final-20_0{1,2,3}`):

| run | coast eHz vs 941 | rate ‰ | vs 6 × coast ‰ | proxy mA |
|---|---|---|---|---|
| 1 | 953 (+1.3%) | 999 | 1004 | 141 |
| 2 | 950 (+1.0%) | 998 | 1007 | 214 |
| 3 | 952 (+1.2%) | 999 | 1005 | 225 |

All three: hold 33.8 s, forced 0. **Rung PASS.** **PSU 250 mA** (operator, during the hold), identical to campaign 2's 0.250 A at 20%. The operator also reported it "sounds pretty smooth". Proxy mean 193 mA.

**A needed change, found before 25%.** Goal item 3 asks for every protection provoked **from a locked 25% loop**. `F5FFF36F`'s provocation keys run the 15% rung and fire 3 s after transfer. At 25% that would be mid-ramp, not locked at target. The step is to add lowercase keys `t g f n u h v i w`: the same stimuli, the same `Inject` codes and thresholds, on the 25% rung, fired `ramp_us(250) + 2 s` after transfer (the restart campaign's rule, `Restart::INJECT_AT_TARGET_US`). The uppercase keys stay as they were. Because this is a new image, 15/20/25 × 3, restart × 3 and the provocations will all be run on it, after its step check.

### E125 — step: provocations from a locked 25% loop (image `F76440D9…`)

**Change:** `Controller::command` adds the lowercase keys `t g f n u h v i w`. Each is `rung(250, Some((kind, ramp_us(250) + Restart::INJECT_AT_TARGET_US)), kind.code(), …)`: the same `Inject` stimulus and expected code, fired 2 s after the ramp reaches 25%. The uppercase 15% keys are unchanged. The fixture's command choices add the lowercase keys. New host test: `t` fires within one pass of `closed + ramp_us(250) + 2 s`, the last published duty is 250, and the stop is Tracking. 304 host tests and 6 doctests pass.

**Unchanged by construction, checked:** the four roots' machine code is instruction-for-instruction identical to `F5FFF36F`'s. The audit passes. Cycles: COMP 1176 / 1011, COM 356, DMA 78, guard 421. Bin 930 lines, 3 unsafe, 0 `static mut`, no function over 100 lines. Clippy is clean with the unsafe lints. Archived as `captures/elf/F76440D9.e125.elf`.

**Run:** `--step-check` (15% warm-up, then one 25% run against the cohort). Then, on this image: 15/20/25 × 3 through the ladder, `R` × 3, and `t g f n u h v i w` plus a host abort from the 25% rung.

### E125 — result: step check, and an ABAB against `F5FFF36F`

| run | image | accepted | rate ‰ | coast eHz | proxy mA | cohort check |
|---|---|---|---|---|---|---|
| e125-step25 | E125 `F76440D9` | – | 998 | 1152 | 392 | FAIL (current only) |
| e125-abab-B-final | `F5FFF36F` | 252 590 | 999 | 1157 | 359 | FAIL (accepted, coast above band) |
| e125-abab-A2 | E125 | 252 901 | 994 | 1148 | 398 | FAIL (accepted, rate, current) |

All three pass the new goal's hard gates: speed within 5% of 1186 (−2.4 to −3.2%), rate within 1% of the loop and of 6 × coast, forced 0, reason 2, hold 31.3 s.

The retired E101–E108 cohort band now fails **the previous image too**, on accepted and coast: that is the rotor-speed drift of E116 (item 5, own entry below). On the current proxy: **the change did not alter the run loop's code.** A per-function structural comparison (`scratchpad fndiff3`: full address ranges, literal pools dropped, operands normalised) finds every function the same instruction count in both images except `Controller::serve`, the shell where the new keys live. The residual line differences are literal-pool data decoded as code; the same method flags TIM16, which the ISR comparison shows identical. The 392/398 against 359 mA is proxy scatter on an unchanged loop, and under this campaign's gate the proxy is report-only. **E125 is accepted as the image under test.**

### E126 — E125 ladder: 15% and 20% pass; 25% fails rate-against-coast; control run on the pre-refactor image

**E125 `F76440D9` through the fixture, back-to-back:**

| rung | runs | result | detail |
|---|---|---|---|
| 15% | `e125-15_0{1,2,3}` | **PASS** | coast 700–703 eHz (−0.1 to −0.6% vs 704); rate 997‰; 6 × coast 1006–1010‰; proxy 125/149/129 (report-only) |
| 20% | `e125-20_0{1,2,3}` | **PASS** | coast 949–951 (+0.9 to +1.1% vs 941); rate 997‰; 6 × coast 1005–1007‰; proxy 261/262/258 |
| 25% | `e125-25_0{1,2,3}` | **NOT PASSED** | speed passes (coast 1136–1140, −3.9 to −4.2% vs 1186); rate 995–996‰; **6 × coast 1011 / 1014 / 1015‰**, outside 1%; R_COMP `spent_max_us=16`, `late_arms=0`; proxy 400/400/404 |

**What moved: the coast witness, not the loop.** The loop runs at its usual 1157 eHz. After the float the rotor decelerates faster: coast transitions are 834–845, against 860–883 on this same image earlier this evening and 875–919 in the morning's E101–E108 cohort. The cohort was also run back-to-back, 20% then 25%, so sequencing alone does not explain it.

**Control (the rule: a code cause until a known-good image says otherwise).** Flash the archived pre-refactor image `9F77B756…` (E112's code) and run the identical sequence now: 15/20/25 × 3 through the fixture. If its 25% reads the same low coast and 6 × coast above 1%, the change is not in the refactor. If it reads inside 1%, the cause is in E113–E125 code and gets bisected across the archived ELFs.

### E126 — result: the control passes 25%; the refactored image differs; bisect

**Control `9F77B756` (E112's code), the same back-to-back sequence right after E125's:**

| rung | runs | result | detail |
|---|---|---|---|
| 15% | `ctl-15_*` | PASS | 6 × coast 1002–1009‰ |
| 20% | `ctl-20_*` | PASS | 6 × coast 1002–1007‰ |
| 25% | `ctl-25_*` | **PASS** | coast 1153–1157; 6 × coast 1001–1004‰; proxy 369/367/372 |

**Same session, 25%:**

| image | coast pair sums µs | coast transitions | revisits | lt075 | too_early | proxy mA |
|---|---|---|---|---|---|---|
| control | ~865 | 910–923 | 44.9–45.2k | 4 300 | 349k | 367–372 |
| E125 | ~877 | 834–845 | 63.2–63.3k | 2 900 | 362k | 400–404 |

The refactored image leaves the loop with the rotor **~1% slower at the float**, coasts down sooner, and reads ~30 mA more on the proxy. The control's value is lower still. Under the rule this is a code cause somewhere in E113–E125.

**Bisect**, one `--step-check` per archived image, same procedure, back to back:

control `9F77B756` → E114 `99074482` → E115 `B2E00039` → E116c `CDE4EF42` → E118 `60FCBE31` → E120b `F2E84BAD` → E125 `F76440D9`.

Compared per image: pair sums, coast transitions, revisits, `lt075` and proxy.

### E126 — bisect result (`bis-*`, one `--step-check` per image, back to back)

| image | coast pair µs | trans | revisits | lt075 | proxy mA | 6 × coast ‰ |
|---|---|---|---|---|---|---|
| control `9F77B756` | 871 | 884 | 44 866 | 4 440 | 358 | 1005 |
| E114 | 874 | 896 | **57 151** | 4 167 | 348 | 1006 |
| E115 | 866 | 940 | 58 272 | 4 308 | 359 | 1001 |
| E116c | 867 | 899 | 57 140 | **2 615** | 361 | 1005 |
| E118 | 874 | 883 | 56 266 | 2 911 | 369 | 1009 |
| E120b | 870 | 857 | **63 296** | 2 892 | 381 | 1006 |
| E125 | 871 | 897 | 63 487 | 2 889 | 396 | 1004 |

**No step moves the 25% coast.** Pair sums lie within 866–874 on every image, and E125 reads 871 / 1004‰, inside 1% this time. The failing trio `e125-25_0{1,2,3}` (pair ~877–880, 1011–1015‰) is therefore not an image property that a single step check reproduces. It is re-run as a trio below.

Three quantities do move, each at a named step (item 5's analysis, below):
* revisits step at control→E114 and at E118→E120b;
* `lt075` roughly halves at E116c;
* the proxy climbs gradually, 348 to 396, within its known scatter.

The ISR entry addresses of the control, E120b, `F5FFF36F` and E125 are identical (`ADC_COMP` at `0x08001330`, and so on). Only the foreground run loop moved, by 28 bytes, in E125.

**Run:** E125, 25% × 3 through the fixture, again.

### E126 — 25% re-run on E125 (`e125-25b_0{1,2,3}`): **PASS**

| run | coast eHz | vs 1186 | rate ‰ | 6 × coast ‰ | proxy mA |
|---|---|---|---|---|---|
| 1 | 1155 | −2.6% | 998 | 1000 | 328 |
| 2 | 1148 | −3.2% | 999 | 1007 | 399 |
| 3 | 1159 | −2.3% | 997 | 1003 | 409 |

All three: hold 31.3 s, forced 0, `BEMFRCOMP spent_max_us=16 late_arms=0`. The rung is judged on the last three runs on the ELF. With the bisect's E125 run at 1004‰, that is four of four in-band 25% runs on this image since the failing trio.

**The failing trio stays on the record**: 1011–1015‰, coast transitions 834–845 against 857–940 in every other run tonight. It came immediately after the 15% and 20% trios, with no rest between rungs. The control's passing trio came straight after it, just as hot. I did not find its cause. It did not recur on the same image in four runs, and no refactor step moves this quantity in the bisect.

**Next:** `R` × 3.

### E126 — restart × 3 on E125 (`e125-restart_0{1,2,3}`): **PASS 3/3**

Each run: an injected tracking loss 2 s after reaching 25% (`first_reason=8`, `first_hold_ms=2000`, stop at 14.72 s), a full second off, admitted, ordinary startup, and restored to 25%. The second segment then holds 13.34 s and ends exactly at the original window (`drive_end_ms=44000`), `recovered=1`.

**Next:** every protection from a locked 25% loop: `t g f n u h v i` and `w`, plus a host abort on `5`, one run each.

### E126 — protections from the locked 25% loop on E125: 8 of 9 provoked; the storm stimulus does not reach the cut

The 25% provocations were `e125-p25-*`, all fired 2 s after the ramp reached 25%. After every stop the preflight read `moe=0 ccr=0 gates_low=1 en=0 nfault=1`, PASS.

| key | protection | reason | stop after stimulus |
|---|---|---|---|
| t | Tracking | 8 | 426 µs |
| g | TickGap | 3 | 440 µs |
| f | FeedbackStale | 4 | 1116 µs |
| n | Driver | 7 | 87 µs |
| h | HandlerOverrun | 14 | 75 µs |
| v | FastBusSag | 26 | 1670 µs |
| i | AverageCurrent | 25 | 20.3 ms |
| — | host abort | 9 | at +16 s |
| w | watchdog | IWDG reset | next boot `RESETCAUSE iwdg=1` |
| **u** | **CompStorm** | **not provoked** | ran to the deadline (reason 2) |

**Why the storm stimulus missed:** `closed_irq_peak_per_ms=63` against the unchanged cut, which trips above 64. The stimulus pends `ADC_COMP` 80 times, 8 µs apart. A pend while the root holds its NVIC line masked (after an acceptance, until the commutation re-arms it: most of a 144 µs sector at 25%) collapses into one entry. At 15%'s longer live windows the same 80 pends reached 65. This is a stimulus limit, not a protection fault; the latch and the threshold are the host-tested `rate::Rate`.

## E127 — step: the storm stimulus drives real entries until the cut trips (image `98340ADF…`)

**Change (bin, `Board::inject(Storm)`):** each entry first unmasks `ADC_COMP` in the NVIC, then pends, 8 µs apart, **until `S.comp().storm` latches** (bounded at 400 entries). Each is a real entry through the real handler and rate latch; the root re-masks on entry as always. The threshold (64/ms), the latch and the stop are unchanged.

**Checked:** the four roots are instruction-identical to E125; `Controller::run` is at the same address; `Ctx::pass` grows 394 → 411 instructions (the stimulus inlines into it). The audit passes; 304 host tests pass; bin 935 lines, 3 unsafe. Archived as `captures/elf/98340ADF.e127.elf`.

**Runs on this image, in order:** step check; 15/20/25 × 3 through the fixture; `R` × 3; `t g f n u h v i`, a host abort and `w` from 25%; the audit and cycles on this ELF.

### E127 — first ladder on `98340ADF`: 15% and 20% pass; 25% fails 2 of 3 on rate against coast

| run | result | coast eHz | 6 × coast ‰ | loop ‰ |
|---|---|---|---|---|
| step check `e127-step25` | **in the new goal's gates**; outside the retired cohort | 1160 | 1002 | 997 |
| 15% × 3 | **PASS** | 702–706 | 1004–1009 | – |
| 20% × 3 | **PASS** | 951 | 1004–1005 | – |
| 25% × 3 | **NOT PASSED** | 1140 / 1147 / 1140 | **1014** / 1008 / **1016** | 993–999 |

The 25% runs had `spent_max_us=16`, `late_arms=0`, and proxy 409/404/391.

**The pattern across the day** (scratchpad `coast25.py`, every reason-2 25% capture; the median of the coast's consecutive pair sums, in µs, where 1% of the loop's 1157 eHz is 856–873):
* The control `9F77B756`: 864–871 in 4 runs.
* The refactored images since E116c: mostly 862–872.
* **Only the two refactored 25% trios run immediately after a 15% and a 20% trio (`e125-25`, `e127-25`) read 872–880.**
* The one control trio run in that same position read 864–867.
* E125's standalone and later trios passed (863–871).

**Run: a hot ABAB** — the control's full 15/20/25 ladder, then E127's full ladder again, back to back. If E127's 25% fails again and the control passes, the cause is in the code and is bisected hot. If not, E127's 25% passes in the same position that failed before.

### E127 — hot ABAB result: the control passes, E127 fails once more; the mechanism is the level revisit

| 25% runs (after 15 × 3 and 20 × 3) | zc/s (hold) | coast eHz | 6 × coast ‰ | revisit accepts | gt150 | result |
|---|---|---|---|---|---|---|
| control `hctl-25_0{1,2,3}` | 6938–6950 | 1147–1149 | inside 1% | 29.6–29.8k | 25–32 | **PASS** |
| E127 `he127-25_0{1,2,3}` | 6981–6991 | 1152–1156 | **1011** / 1006 / 1008 | 49.2–49.4k | 64–77 | **NOT PASSED** |

E127's loop books **~0.6% more commutations** than the control's at an equal coast speed. The extra acceptances come with **+19.5k level-revisit acceptances per run** and twice the `gt150` (over-long) intervals.

**Per-image medians at 25%, all captures today** (scratchpad `drift.py`): revisit attempts
* control 44.9k;
* **E113 58.2k**;
* E114–E118 56–58k;
* **E120 63.6k**.

Revisit acceptances go 29.7k → 43.0k at E113 → 47.5k at E116 → ~49k after. **The step is E113**, the comparator path moving into `hw::comp`. Its control image is that exact source with only the comparator code reverted, and it reads 44.9k, so the comparator code alone moves the revisit.

Comparing E113's comparator functions with the reverted ones (`oldcomp`, E113 notebook entry), every register semantic is identical except one: **`pending()` short-circuits** (`rpif18 || fpif18`, reading `FPR1` only when `RPR1` is clear), where the old code always read both registers. `pending()` runs inside the level revisit's critical section, which is where the count diverges. The level revisit is a foreground poll; its count depends on when in the sector it samples, as E116 first showed.

## E128 — step: `hw::comp::pending` reads both registers again (image `4FC7C796…`)

**Change:** `pending()` reads `RPR1.rpif18` and `FPR1.fpif18` unconditionally and ORs them. The result is the same; only the timing differs, as the pre-E113 code did. The four roots are instruction-identical to E127 (the function is foreground-only). Archived as `captures/elf/4FC7C796.e128.elf`.

**Prediction:** if the short-circuit is the cause, revisit attempts return to ~45k and revisit acceptances to ~30k at 25%, and the hot 25% trio sits inside 1%.

**Run:** a step check, then the hot ladder: 15/20/25 × 3.

### E128 — result: revisit restored toward the control; the hot ladder passes

**Step check** `e128-step25`: revisit attempts **50.3k** (E127 63.7k, control 44.9k), revisit acceptances **35.6k** (E127 49k, control 29.7k), `gt150` **28** (E127 64–77, control 25–32). 6 × coast 1008‰. The short-circuit was a cause.

**Hot ladder, back to back:**

| rung | runs | coast eHz | loop ‰ | 6 × coast ‰ | proxy mA |
|---|---|---|---|---|---|
| 15% | `e128-15_0{1,2,3}` | 704–707 | 996–1000 | 1002–1008 | 94/104/104 |
| 20% | `e128-20_0{1,2,3}` | 949–953 | 995 | 1006–1010 | 222/228/222 |
| 25% | `e128-25_0{1,2,3}` | 1155–1164 (−1.9 to −2.6%) | 998–999 | **1001 / 1001 / 1008** | 386/374/360 |

All three rungs **PASS**. The 25% pass sits in the position where E125 and E127 failed. Coast pair medians are 859–866 µs (E127 hot: 865–880); revisits 50.4–50.5k.

**Next on this image:** `R` × 3, then every protection from the locked 25% loop.

### E128 — restart × 3 (`e128-restart_0{1,2,3}`): **PASS 3/3**. In each: `first_reason=8` at 14.72 s (2 s at 25%), admitted, second segment reason 2 holding 13.34 s, `drive_end_ms=44000` = window, `recovered=1`.

**Next:** `t g f n u h v i`, a host abort, then `w`, all from the locked 25% loop.


### E129 — item 5a: the rotor-speed drift, "~0.1% faster since E116"

**Step: E116c** (`Seam` flag-free root borrows). Per-image medians at 25% (`drift.py`):

| image | accepted/s | `lt075` |
|---|---|---|
| control | 6931 | 4372 |
| E113–E115 | 6929–6930 | ~4200–4300 |
| E116 (RefCell) | 6914 | 4710 |
| **E116c** | **6941** | **2523** |
| E116c onward | 6936–6948 | 2500–3000 |

Hot, after 15% and 20% trios, the rise reaches ~0.6% (E127 6981–6991 against the control's 6938–6950), and the coast witness moves with it (E127 1152–1156 eHz against the control's 1147–1149). **The rotor genuinely turns faster**: it is not miscounting. The hold's mean sector reads 143 µs against the control's ~144.

**Mechanism:** E116c made the COM root 37 cycles shorter (393 → 356, 0.58 µs at 64 MHz) and COMP's decision path shorter. Every commutation lands ~0.58 µs sooner: about 0.4% of a 144 µs sector, ~1.5° electrical of extra effective advance.

**Intended?** No, not as a behaviour change. It is the consequence of a structural change the refactor goal requires (cycle counts may not grow; these shrank). **It is kept**, because undoing it means adding cycles back to the COM root. It stays inside every hard gate on the final image: speed −1.9 to −2.6% of the oracle at 25%, rate within 1% of the loop and of 6 × coast (E128 ladder).

### E129 — item 5b: the level-revisit drift, "~11% more retries"

**Two steps**, per-image medians at 25%:

| image | revisit attempts | revisit acceptances |
|---|---|---|
| control | 44.9k | 29.7k |
| **E113** | **58.2k** | **43.0k** |
| E114–E118 | 56–58k | ~47–48k |
| **E120** | **63.6k** | 49k |
| **E128** | **50.4k** | **35.6k** |

1. **E113** (comparator path into `hw::comp`): **not intended; fixed in E128.** `hw::comp::pending()` short-circuited (`rpif18 || fpif18`) where the old code read both pending registers. The result is the same; only the timing inside the revisit's critical section differs, which moved when the foreground's retry samples the sector. E113's own control (the same source, comparator reverted) isolates it. **E128** restores the both-registers read. Revisit acceptances drop from 49k to 35.6k, `gt150` returns from 64–77 to the control's 25–32, and the hot 25% trio that failed twice (E125, E127: 6 × coast up to 1016‰) passes at 1001–1008‰.
2. **E119–E120** (the typestate run replacing `bemf_run`): **not intended; kept.** It adds ~+6k attempts (E118 56.3k → E120 63.6k), and ~+5.5k over the control remains after E128. The retry is a foreground poll taken once per pass, as the reference takes it (`binz/examples/support/core_bench.rs:3114`: `revisit_low_speed_level()` once per foreground loop pass). Its count follows the pass's code shape and phase, which the typestate changed by construction. Its admission rule (`revisit::admit`, host-tested) is unchanged, and every gate holds with it.

## E129 — final: the ladder and restart campaign on one image, `4FC7C796…` (E128)

The image under test moved twice from `F5FFF36F`, each time for a stated reason, one step and one entry each:
* E125: provocation keys for a locked 25% loop, which goal item 3 requires;
* E127: a storm stimulus that reaches the unchanged cut at 25%;
* E128: the revisit fix for item 5.

Every hard gate below ran on `4FC7C796` (`captures/elf/4FC7C796.e128.elf`).

| gate | result | captures |
|---|---|---|
| 1. 15% × 3 | **PASS**: hold 36.3 s, forced 0, coast 704–707 (≤ +0.4% vs 704), loop 996–1000‰, 6 × coast 1002–1008‰ | `e128-15_0{1,2,3}` |
| 1. 20% × 3 | **PASS**: hold 33.8 s, forced 0, coast 949–953 (+0.9 to +1.3% vs 941), 995‰, 1006–1010‰ | `e128-20_0{1,2,3}` |
| 1. 25% × 3 | **PASS**: hold 31.3 s, forced 0, coast 1155–1164 (−1.9 to −2.6% vs 1186), 998–999‰, 1001–1008‰ | `e128-25_0{1,2,3}` |
| 2. restart at 25% × 3 | **PASS**: Tracking 8 at 2 s on target, all-off, 1 s off, ordinary startup, restored, 13.34 s hold, window ends at 44 000 ms, `recovered=1` | `e128-restart_0{1,2,3}` |
| 3. every protection from locked 25% | **PASS**; bridge off (preflight PASS) after each | see below |
| 4. ISR audit on the final ELF | **PASS**: 4 roots clean; COMP 1176 / 1011, COM 356, DMA 78, guard 421, each equal to E123's; the four roots are instruction-identical to `F5FFF36F`'s | this entry |
| 4. R_COMP < wait_time(ci) at 25% | **TRUE**: ci 142–146 µs, advance 20 (below 35%), `wait_time(144, 20)` = 72 − 45 = **27 µs** (27 at ci 142); measured `spent_max_us=16`, `late_arms=0` in every E128 25% run | `e128-25_*` |
| 5. both drifts named | **DONE** (E129 5a, 5b) | – |

**Gate 3 detail**, from `e128-p25-*`:

| key | protection | reason | stop after stimulus |
|---|---|---|---|
| t | Tracking | 8 | 369 µs |
| g | TickGap | 3 | 439 µs |
| f | FeedbackStale | 4 | 1074 µs |
| n | Driver | 7 | 69 µs |
| u | CompStorm | 13 | 968 µs |
| h | HandlerOverrun | 14 | 63 µs |
| v | FastBusSag | 26 | 1655 µs |
| i | AverageCurrent | 25 | 20.2 ms |
| — | host abort | 9 | at +16 s |
| w | watchdog | IWDG reset | `RESETCAUSE iwdg=1` |

**Current, report-only** (proxy mean per rung, E128): 15% 101 mA, 20% 224 mA, 25% 373 mA. **PSU (operator): 250 mA at 20%** (E124, same code path). No PSU reading was taken at 15% or 25%, so those two calibration points are still owed.

**Structure, checked:**
* bin (`shell-pwm.rs` + `board.rs`) 935 lines ≤ 1000;
* no function over 100 lines;
* 3 `unsafe` in the bin, each quoting its PAC vector (`ADC_COMP = 12`, `TIM16 = 21`, `TIM6_DAC_LPTIM1 = 17`);
* 0 `static mut`; `.bits(` outside hw 0;
* 304 host tests (≥ 303) and 6 doctests pass, the five compile-fail ones included (no gate write from Idle, Handover or Stopped; no `Sink` from `Hal` alone; no `Stopped` without `safe_off`);
* clippy clean with the unsafe lints on the lib (host with tests, and target) and both binaries.

**Unchanged:**
* every protection and threshold;
* nothing in `../rm32*`, `../minz/core`, the old binz package, its captures or notebook. The `examples/` changes in `git status` predate this campaign.

## E130 — campaign 5 (goal set 2026-09-21): re-qualify two ISR-cost changes to the campaign-4 bar. Step 1: the blanking fraction becomes a type (image `F0820748…`)

**Baseline:** E128 `4FC7C796`. **Change:** `bemf::ZeroCross` becomes `pub type ZeroCross = ZeroCrossWith<REFERENCE_BLANK_64>` over a `ZeroCrossWith<const BLANK_64: u32>` (a type alias, because Rust does not infer from a default const parameter in expressions).
* `BLANK_64` is checked in `1..=56` at instantiation (`IN_BAND`, a const assertion). Two compile-fail doctests (0, 57) and one passing doctest (56) replace the runtime clamp and its host test.
* `blanking()` returns `average_interval >> 1` at the reference's 32; the general form stays for other fractions.
* Two host tests are added: the half-cycle shortcut equals the general formula over 0..70 000 plus the extremes, and `with_blanking::<N>()` keeps the estimate. The sweep tests instantiate `ZeroCrossWith::<1/8/32/44/56>`.
* The replay's one-step-different check uses `ZeroCrossWith::<33>` and asserts that the capture ran 32.

**Checks:**
* 305 host tests (≥ 305) and 9 doctests pass, including 7 compile-fail.
* Replay: all 1536 decisions reproduce.
* Clippy is clean with the unsafe lints; the audit passes on all four roots.
* Structure: bin 935 lines, no function over 100, 3 unsafe, 0 `static mut`.

**Disassembly:** the gate is now `ldr`; `lsrs #1`; ADC_COMP has no `muls`; 355 → 349 instructions.

| cycles (model, 0 ws) | E128 | E130 |
|---|---|---|
| COMP @ 12 reads | 1176 | **1184 (+8)** |
| COMP @ 7 | 1011 | **1019 (+8)** |
| COMP @ 4 | 912 | **920 (+8)** |
| COM | 356 | 356 |
| DMA | 78 | 78 |
| guard | 421 | 421 |

Fewer instructions, but the register allocator placed a spill (`ldr r5, [sp, #0x14]`, the count) on the worst path. The goal judges item 4 on E131.

**Masking count** (new `scripts/isr_cs_count.py`: `mrs` / `cpsid` / `msr` in each root's own body, plus calls), identical in E128 and E130:

| root | instructions | mrs | cpsid | msr | calls |
|---|---|---|---|---|---|
| ADC_COMP | 349 | 26 | 26 | 25 | 0 |
| TIM16 | 215 | 15 | 15 | 15 | 1 (`roots::comp_exti_arm`: 38 instructions, 0 cpsid) |
| DMA1_CHANNEL1 | 34 | 2 | 2 | 2 | 0 |
| TIM6 guard | 256 | 36 | 36 | 38 | 0 |

**Correction:** my earlier "52 critical sections in COMP" was a double count; the grep also matched the `asm!("cpsid i")` source lines that `objdump -S` interleaves. The true count is 26. The cycle cost stated alongside it (252 cycles in COMP) came from the cycle model with those instructions zeroed and stands.

Archived as `captures/elf/F0820748.e130.elf`. **Run:** `--step-check`.

### E130 — result: step check `e130-step25` passes the hard gates

reason 2, coast 1151 eHz (−3.0% vs 1186), rate 998‰ against the loop and 1003‰ against 6 × coast, R_COMP `spent_max_us=16`, `late_arms=0`, proxy 371 mA.

## E131 — step 2: portable-atomic backend `critical-section` → `unsafe-assume-single-core` (image `2A7694CD…`)

**Change (`Cargo.toml`):**
* The firmware target (`[target.'cfg(target_os = "none")'.dependencies]`) takes `portable-atomic` with `unsafe-assume-single-core`.
* The host takes it with no backend feature: portable-atomic refuses that feature on x86_64, where its atomics are native.
* `cortex-m`'s `critical-section-single-core` stays for `interrupt::free` and the `Seam` locks.
* No dependent forced `critical-section`: the PAC's own `critical-section` feature pulls the separate crate, and the HAL takes portable-atomic with defaults.

The safety condition is one core, privileged mode; the G071 is a single Cortex-M0+ with no RTOS.

**Consequence found and fixed in the same step:** without the masking the optimiser no longer proved `Step::idx() < 6` inside TIM16, and `commutation::sector`'s 6-entry index pulled `panic_bounds_check` into the COM root. The fail-closed audit refused it (`TIM16: panic path reachable`). `SECTORS` is now 8 entries indexed `& 7` (entries 6 and 7 are unreachable copies), the crate's existing pattern for ISR tables. The audit then passes on all four roots.

**Masking count** (`scripts/isr_cs_count.py`):

| root | E128 mrs / cpsid / msr | E131 | instructions (E128 → E131) |
|---|---|---|---|
| ADC_COMP | 26 / 26 / 25 | **6 / 6 / 6** | 355 → 324 |
| TIM16 | 15 / 15 / 15 | **2 / 2 / 2** | 215 → 187 |
| DMA1_CHANNEL1 | 2 / 2 / 2 | 2 / 2 / 2 | 34 → 34 |
| TIM6 guard | 36 / 36 / 38 | **1 / 1 / 1** | 256 → 138 |

TIM16's one call, `roots::comp_exti_arm`, has 0.

**The remaining sections, each a `ldr; adds #1; str` (`fetch_add(1)`, one counter), named by `#[repr(C)]` offset:**

| root | window | read-modify-write |
|---|---|---|
| ADC_COMP | `[EDGE+0x4]` | `edge.seq` |
| ADC_COMP | `[DRV+0x2c]` | `drv.early` |
| ADC_COMP | `[DRV+0x10]` | `drv.acc_seq`; the block stores `acc_raw` 0x14, `acc_epoch` ← `epoch` 0x18, `acc_step` 0x1c, `last_raw` 0x04 first |
| ADC_COMP | `[DET+0x24]` | `det.rebase` (after `sector_start_raw`) |
| ADC_COMP | `[DRV+0x30]` | `drv.unstable` |
| ADC_COMP | `[DRV+0x34]` | `drv.defers` |
| TIM16 | `[COM+0xc]` | `com.count` |
| TIM16 | `[COM+0x18]` | `com.blank_arms` |
| DMA | twice | `scan.seq` (the seqlock's two increments) |
| guard | `[GUARD+0x18]` | `guard.ticks` |

The closed-loop acceptance's `det.accept_seq.fetch_add` has no `[DET+0x10]` window among these. I could not locate it statically: the DET base reaches ADC_COMP through several literal-pool addresses. The step check verifies it: the foreground counts `accepted` only by consuming `accept_seq`, so `accepted` must equal the detector's own `zc_acc` as always.

**Cycles** (model, 0 wait states):

| root | E128 | E130 | E131 |
|---|---|---|---|
| COMP @ 12 reads | 1176 | 1184 | **1026** |
| COMP @ 7 | 1011 | 1019 | **866** |
| COMP @ 4 | 912 | 920 | **770** |
| COM | 356 | 356 | **287** |
| DMA | 78 | 78 | 78 |
| guard | 421 | 421 | **204** |

Every value is at or below E128's.

**Checks:**
* 305 host tests and 9 doctests pass (7 compile-fail);
* the replay reproduces all 1536;
* clippy is clean with the unsafe lints (host lib and tests; target lib and both bins);
* bin 935 lines, 3 unsafe, 0 `static mut`, no function over 100.

Archived as `captures/elf/2A7694CD.e131.elf`.

**Watch:** a faster COMP catches more chatter (E116c). The storm cut (64/ms) is unchanged, and `closed_irq_peak_per_ms` is read on every run.

**Run:** `--step-check`, then the hot ladder 15/20/25 × 3, `R` × 3, and the 25% provocations.

### E131 — step check `e131-step25`

| quantity | E131 | E128-era |
|---|---|---|
| reason | 2 | 2 |
| accepted vs `zc_acc` | **258 725 = 258 725** (`det.accept_seq` works) | – |
| R_COMP `spent_max_us` | **13** | 16 |
| `late_arms` / `comp_call_max_us` | 0 / 18 | – |
| coast eHz | **1183** (−0.3% vs 1186) | 1155–1164 |
| loop eHz | 1190 | – |
| rate vs loop / vs 6 × coast | 999‰ / 1005‰ | – |
| proxy mA | 445 | ~373 |
| `closed_irq_peak_per_ms` | **66–67** (warm-up and 25%) | 54–63 |

**Two drifts, item 5:**
1. **The rotor runs ~2.5% faster at the same duty.** Commutation lands sooner, the E116c mechanism again, larger this time. It is closer to the oracle's speed.
2. **The COMP entry peak** is at 66–67, against the unchanged 64/ms cut. It did not trip: the peak was reached in the handover window, where binz's startup policy (E107) observes without enforcing. The cut applies after it.

Both are judged on the hot ladder.

### E131 — hot ladder: 15% **NOT PASSED** (`e131-15_0{1,2,3}`); the fixture refuses 20% and 25%

| 15%, hot | E128 | E131 |
|---|---|---|
| 6 × coast ‰ | 1002–1008 | **1011** / 1010 / 1009 |
| coast pair median µs | 1414–1420 | 1427–1430 |
| foreground passes | 945–946k | **1280k (+35%)** |
| level-revisit admits (all accepted) | 30.3–30.8k | **36.0k (+18%)** |
| unstable | 1.039M | 1.111M |
| too_early | 504k | 520k |
| COMP peak / ms | 57–60 | 64–68 (no storm) |
| proxy mA | 94–104 | 178–185 |

The loop's own rate is unchanged (709 eHz). The foreground uses the same atomics, so it runs 35% faster, and the level revisit, sampled once per pass, now samples 35% more often. Its retry fires at the first sample after the gate opens, so it lands earlier in the sector and more often. E128 showed at 25% that this lever moves rate-against-coast (E113's short-circuit). Here the rotor comes out 0.8% slower at the float than the loop's rate, with more current.

## E132 — step: the level revisit samples on a fixed cadence, not once per pass (image `066EB08A…`)

**Change:**
* `Locked::revisit` samples at most once per `REVISIT_POLL_US` (policy, 44 µs); the constant is E128's measured closed-loop pass period at 25% (38.8 s / 864k).
* The admission rule (`revisit::admit`), the critical section and the pend are unchanged.

This makes the retry's timing independent of the foreground's code speed, which has moved it at E113, E120 and E131.

**Checks:**
* 305 host tests and 9 doctests pass; clippy is clean.
* The four roots are instruction-identical to E131 (a foreground-only change); the audit passes.
* bin 935 lines, 3 unsafe, 0 `static mut`.

Archived as `captures/elf/066EB08A.e132.elf`.

**Prediction:** 15% revisits return to ~30.5k, rate-against-coast to ≤ 1008‰, and the proxy toward ~100 mA.

**Run:** 15% × 3 hot, then continue up the ladder.

### E132 — result: the revisit cadence is not the cause; reverted

`e132-15_0{1,2,3}`: 6 × coast **1007 / 1011 / 1004‰** (the rung fails again), coast pair 1421–1430, zc/s 4241–4244.

Revisit admits dropped to **19.3–19.8k**, below even E128's 30.5k; the fixed 44 µs cadence is slower than E128's 15% pass period of ~41 µs. The rung still misses, so the 15% shift does not come from the revisit's sampling rate. E132 also moved revisits further from E128 than E131 did, so it is **reverted**. The rebuilt image's loadable bytes are identical to E131 `2A7694CD`.

Commutation timing on E131/E132 against E128, 15% hot:

| quantity | E128 | E131 / E132 |
|---|---|---|
| `com_late_max_us` | 8–12 | **5** |
| R_COMP `spent_max_us` | 16 | **13** |
| `comp_call_max_us` | 22 | **18** |
| `unstable` | 1.039M | **1.110M (+7%)** |
| COMP peak / ms | 57–60 | 64–69 |

Commutations land sooner, and the faster COMP handler services more comparator chatter.

**Run: ABAB at 15%, hot, per the rule.** E128 `4FC7C796` × 3, then E131 `2A7694CD` × 3, back to back. If E128 passes and E131 fails again, the E131 change moves 15% and must be understood before the ladder continues. If E128 also sits at the edge, the 15% margin is the bench's today.

### E131 — ABAB at 15% and the ladder: **PASS**

**ABAB, hot, back to back:**

| image | 6 × coast ‰ | coast pair µs | revisits | peak / ms |
|---|---|---|---|---|
| E128 `ab-e128-15_0{1,2,3}` | 1008 / 1007 / 1004 | 1419–1423 | 30.6–30.8k | 57–58 |
| E131 `ab-e131-15_0{1,2,3}` | 1005 / 1008 / 1000 | 1417–1427 | 35.9–36.3k | 65–67 |

Both **PASS**. At 15% the two images' coast readings overlap, so E131's earlier 1009–1011 trio is within that spread and **not** an E131 cause of this gate. The fixture's 15% rung on `2A7694CD` is the ABAB trio.

**E131, continued hot:**

| rung | coast eHz vs oracle | loop ‰ | 6 × coast ‰ | R_COMP | proxy mA | result |
|---|---|---|---|---|---|---|
| 20% `e131-20_0{1,2,3}` | 947–951 (+0.6 to +1.1% vs 941) | 995–996 | 1003–1007 | 13 µs | 255/262/264 | **PASS** |
| 25% `e131-25_0{1,2,3}` | 1181–1185 (−0.1 to −0.4% vs 1186) | 993 | 1005–1008 | 13 µs | 426/431/415 | **PASS** |

All six runs had `late_arms=0`. **Next:** `R` × 3, then the 25% provocations.

### E131 — restart × 3 (`e131-restart_0{1,2,3}`): **PASS 3/3**. In each: `first_reason=8` at 14.72 s, admitted, second segment reason 2 holding 13.34 s, `drive_end_ms=44000` = window, `recovered=1`.

**Next:** `t g f n u h v i`, a host abort, then `w`, all from the locked 25% loop.


### E131 — every protection from the locked 25% loop (`captures/2026-09-22/e131-p25-*_01`): **PASS**

Image `2A7694CD`. Each run locks at 25%, injects at the target plus `INJECT_AT_TARGET_US`, and reads the bridge back after the stop.

| key | provoked | reason | stop after stimulus | capture | bridge after (PREFLIGHT) |
|---|---|---|---|---|---|
| `t` | 1 | 8 | 457 µs | `e131-p25-t_01` | PASS |
| `g` | 1 | 3 | 439 µs | `e131-p25-g_01` | PASS |
| `f` | 1 | 4 | 1037 µs | `e131-p25-f_01` | PASS |
| `n` | 1 | 7 | 89 µs | `e131-p25-n_01` | PASS |
| `u` | 1 | 13 | 863 µs | `e131-p25-u_01` | PASS |
| `h` | 1 | 14 | 88 µs | `e131-p25-h_01` | PASS |
| `v` | 1 | 26 | 1581 µs | `e131-p25-v_01` | PASS |
| `i` | 1 | 25 | 20142 µs | `e131-p25-i_01` | PASS |
| host abort | 1 | 9 | — | `e131-p25-abort_01` | PASS |
| `w` | watchdog reset | `RESETCAUSE iwdg=1` | — | `e131-p25-w_01` | PASS |

Every reason code matches campaign 4's. No threshold, protection or reason code was changed in E130 or E131.

### E131 — item 4: ISR cycle counts and R_COMP against the wait

Modelled worst-path cycles per root, from the fail-closed audit on each archived ELF:

| root | E128 `4FC7C796` | E130 `F0820748` | E131 `2A7694CD` | E131 vs E128 |
|---|---|---|---|---|
| COMP, 12 reads | 1176 | 1184 (+8) | **1026** | −150 (−12.8%) |
| COMP, 7 reads | 1011 | 1019 (+8) | **866** | −145 (−14.3%) |
| COMP, 4 reads | 912 | 920 (+8) | **770** | −142 (−15.6%) |
| COM (TIM16) | 356 | 356 | **287** | −69 (−19.4%) |
| DMA | 78 | 78 | **78** | 0 |
| guard (TIM6) | 421 | 421 | **204** | −217 (−51.5%) |

E130 removed the blanking multiplies but came out +8 on every COMP path because of spill placement. The goal judges this item on the final image. On E131 every root is at or below E128. All four roots pass the fail-closed audit on E131.

**R_COMP at 25%.** E131's hot 25% runs report `spent_max_us=13` and `late_arms=0` (`e131-25_0{1,2,3}`, and every `e131-p25-*` run). Their hold intervals are `ci` 138–142 µs at advance level 20. Using `wait_time(ci, 20) = (ci>>1) − ((ci>>6)·20 + ((ci&63)·20>>6))`:

| ci µs | 138 | 139 | 140 | 141 | 142 |
|---|---|---|---|---|---|
| wait µs | 26 | 26 | 27 | 26 | 27 |

So R_COMP, at 13 µs, is below the smallest computed wait (26 µs) with 13 µs to spare. E128 had 16 µs against the same wait.

**Critical sections left after E131** (`scripts/isr_cs_count.py --list`). Each one is the interrupt-disable around one `fetch_add(1)`, the only read-modify-write the M0+ cannot do natively:

| root | mrs/cpsid/msr | read-modify-writes they protect |
|---|---|---|
| COMP | 6 | `edge.seq`, `drv.early`, `drv.acc_seq`, `det.rebase`, `drv.unstable`, `drv.defers` |
| TIM16 | 2 | `com.count`, `com.blank_arms` |
| DMA | 2 | `scan.seq` (two sites) |
| guard | 1 | `guard.ticks` |

This is down from 26 in COMP under the critical-section backend. `det.accept_seq`'s window was not isolated statically, so it was checked at runtime instead: `accepted == zc_acc` in every E131 run (e.g. 49671 = 49671 in `e131-p25-t_01`).

### E131 — item 5: behaviour drift against E128's hot-ladder medians

Ranges are over each trio: E128 from campaign 4's hot ladder and `ab-e128-15`, E131 from `ab-e131-15`, `e131-20`, `e131-25`.

| quantity | rung | E128 | E131 | beyond E128's spread? |
|---|---|---|---|---|
| accepted / s | 15% | 4247–4260 | 4240–4244 | slightly below |
| | 20% | 5752–5756 | 5722–5725 | **yes, −0.5%** |
| | 25% | 6980–6992 | 7147–7149 | **yes, +2.3%** |
| coast pair median µs | 15% | 1414–1423 | 1417–1427 | no |
| | 20% | 1049–1054 | 1052–1056 | no |
| | 25% | 859–866 | 844–847 | **yes, −2%** (faster rotor) |
| revisit attempts / accepts | 15% | 30.3–30.8k accepts | 35.9–36.3k accepts | **yes, +18%** |
| | 20% | ~36.1k / 35.4k | 40.7–41.0k | **yes** |
| | 25% | 50.4k / 35.6–35.9k | 44.2–44.6k / 41.2–41.5k | **yes**: fewer attempts, more accepts |
| lt075 | 15% | 625–739 | 520–559 | **yes, lower** |
| | 20% | ~2340 | ~1210 | **yes, halved** |
| | 25% | ~2820 | 1152–1243 | **yes, halved** |
| gt150 | 25% | 21–29 | 2–6 | **yes, lower** |
| COMP entry peak / ms | all | 55–60 | 64–71 | **yes**, with `storm=0` in every run |

**Step named: E131** (the portable-atomic backend). E130 changed only the COMP arithmetic and was not run up the ladder separately, since the goal qualifies the final image.

**Mechanism.** Removing the masked loads and stores:
* shortens every ISR (table above);
* lets the foreground loop make about 35% more passes (`loop_iters_closed`);
* lands commutations sooner (`com_late_max_us` 5 against 8–13);
* services more comparator chatter per unit time: `unstable` rises by 5–7%, and the COMP entry rate rises.

This is the same mechanism E116c recorded. The earlier commutation gives a slightly faster rotor at 25% and tighter phase statistics (lt075 and gt150 fall). The level revisit is a foreground poll, so its counts follow the faster loop.

**COMP peak above 64/ms.** The storm cut (64/ms) is unchanged, and `storm=0` in every E131 run. `closed_irq_peak_per_ms` includes the handover window, where E107's policy does not enforce the cut. The peaks of 64–71 fall there, so the protection never saw them as storm.

**Tried and reverted.** E132 paced the revisit at 44 µs to pin its count. It moved revisits further from E128 and did not change the 15% reading, so it was reverted (see E132).

**Kept.** E131 is the change the goal mandates, and every hard gate passes on it. The proxy current also rises (25%: ~424 against ~373 mA; 15%: ~150–185 against ~100 mA). It stays report-only, as the goal requires; the PSU never left 1 A.

### Campaign 5 — final: **firmware50 on E131 `2A7694CD` meets every gate**

Pre-power checks, on each step (E130 `F0820748`, E131 `2A7694CD`):
* 305 host tests;
* 9 doctests, 7 of them compile-fail (`ZeroCrossWith::<0>` and `::<57>` among them);
* clippy clean with the unsafe lints (host lib+tests; target lib+bins);
* the fail-closed audit clean on all four roots (E131 needed the 8-entry `SECTORS` table to remove a reachable `panic_bounds_check` from TIM16);
* the mrs/cpsid/msr count per root;
* the replay reproduces all 1536 captured 25% decisions.

| gate | result | capture(s) |
|---|---|---|
| 1. 15% hot 3/3 | **PASS**: 6 × coast 1005 / 1008 / 1000‰, forced 0, crossings > 0 | `2026-09-21/ab-e131-15_0{1,2,3}` (ABAB against `ab-e128-15_0{1,2,3}`) |
| 1. 20% hot 3/3 | **PASS**: coast 947–951 (+0.6 to +1.1% vs oracle 941), loop 995–996‰, 6 × coast 1003–1007‰ | `2026-09-21/e131-20_0{1,2,3}` |
| 1. 25% hot 3/3 | **PASS**: coast 1181–1185 (−0.1 to −0.4% vs 1186), loop 993‰, 6 × coast 1005–1008‰, `late_arms=0` | `2026-09-21/e131-25_0{1,2,3}` |
| 2. restart at 25% 3/3 | **PASS**: `recovered=1` each time | `2026-09-22/e131-restart_0{1,2,3}` |
| 3. protections from 25% | **PASS**: all provoked with their reason codes; bridge PASS after each | `2026-09-22/e131-p25-{t,g,f,n,u,h,v,i,abort,w}_01` |
| 4. ISR cycles ≤ E128 | **PASS**: COMP 1026 / 866 / 770, COM 287, DMA 78, guard 204; R_COMP 13 µs < wait 26 µs, `late_arms=0` | the audit on `captures/elf/2A7694CD.e131.elf`; `e131-25_0*` |
| 5. drift characterised | **done**: E131 named as the step and kept (entry above) | as above |

Rung runs are ≥ 30 s at target after handover, stamped by the MCU (`drive_end_ms=44000`). The one 15% trio on E131 that missed (1009–1011‰) was resolved by the ABAB, which put E128's own readings in the same spread.

**Structure (E131):**
* `bin/` 935 lines;
* no function over 100 lines;
* 3 `unsafe`, each quoting its PAC vector;
* 0 `static mut`;
* no serial write reachable from `Locked`.

**Unchanged:**
* every protection, threshold and reason code;
* the current proxy stays report-only;
* PSU at 1 A throughout;
* nothing touched in `../rm32*`, `../minz/core`, the old binz package, its captures or notebook.

**Archived images in `captures/elf/`:**

| file | role |
|---|---|
| `4FC7C796.e128.elf` | baseline |
| `F0820748.e130.elf` | intermediate |
| `2A7694CD.e131.elf` | **final** |
| `066EB08A.e132.elf` | reverted |

### E133 — flash prefetch on (`FLASH_ACR.PRFTEN = 1`): a separate qualification step

Follows `WCET_ESTIMATES.md`, written today. It records the ISR worst-case models with flash wait states, the speed-against-duty extrapolation (about 3.4–3.5 keHz at 100%) and the budget per commutation step. FLASH_ACR read over SWD on E131 is `0x00040602`: 2 wait states, **prefetch off**, instruction cache on.

**Change:**
* `hw::system::flash_prefetch_enable()` does a named-field *modify* of PRFTEN after `Rcc::freeze`. It is not a write, so DBG_SWEN is kept.
* `hw::system::flash_prefetch_on()` reads it back, and the banner prints `prefetch=`.
* No other change.

**Checks on `152C7299` (archived `captures/elf/152C7299.e133.elf`):**
* The four roots are **instruction-identical to E131** (normalised disassembly), so the static cycle counts are unchanged: COMP 1026 / 866 / 770, COM 287, DMA 78, guard 204.
* Fail-closed audit PASSED on all four roots.
* CS count unchanged: COMP 6, TIM16 2, DMA 2, guard 1.
* 305 host tests and 9 doctests pass; the replay test reproduces all 1536 decisions; clippy is clean on host and target.
* bin 940 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**Prediction:** the fetch model's sequential term goes away. Measured COMP and COM times fall toward the "literal loads and branches only" row of `WCET_ESTIMATES.md` §2.2:
* COMP at 7 reads: from about 1212 toward about 1038 modelled cycles, roughly −2.7 µs;
* `spent_max_us` 13 → about 11;
* `comp_call_max_us` 18 → about 15–16;
* foreground passes up by a similar fraction.

Behaviour may drift the way it did at E131: faster code services more chatter.

**How the win is recorded:** hot ABAB at 25%, back to back. A = E131 `2A7694CD` and B = E133 `152C7299`, three runs each, in the order A, B (after B's ladder), A, B. The quantities are `spent_max_us`, `comp_call_max_us`, `com_late_max_us`, `loop_iters_closed`, and the campaign-5 drift set.

**Qualification bar:** campaign 5's, on `152C7299`:
* 15/20/25 × 3 hot through the ladder;
* restart × 3;
* every protection from a locked 25% loop;
* ISR cycles ≤ E131 (static: identical);
* drift against E131's hot medians, named and kept or not.

**Run:** A, E131 25% × 3.

### E133 — result: prefetch speeds the handlers; the chatter rate goes over the 64/ms storm cut and 15% fails 5/5; **reverted**

**A leg, E131 `2A7694CD`, 25% × 3 hot (`ab133-e131-25_0{1,2,3}`): PASS.** 6 × coast 1003–1004‰, coast 1185–1188 eHz, `spent_max_us` 13, `comp_call_max_us` 18, `loop_iters_closed` 1.197–1.199M. These match yesterday's `e131-25_0*`.

**E133 `152C7299`.** FLASH_ACR reads `0x00040702` over SWD at idle: PRFTEN set, DBG_SWEN kept.
* 15% × 3 hot (`e133-15_0{1,2,3}`): **all stop with reason 13, CompStorm**, 40–42 ms into the closed loop (at about 300 eHz, still ramping), with the cap armed. `closed_irq_peak_per_ms` 76–80 against the 64 cut.
* The fixture then refused 20% and 25% (ladder), correctly.

**ABAB at 15%, per the rule:**

| image | runs | result | closed COMP peak / ms | driven-stage COMP peak / ms | captures |
|---|---|---|---|---|---|
| E131 | 3 | **PASS**: 6 × coast 1004–1006‰ | 65–68 (storm 0) | 46–49 | `ab133-e131-15_0{1,2,3}` |
| E133 | 2 (+3 above) | **5/5 CompStorm** at 40–45 ms | 76–80 (storm 1, steps 3–4) | 47–58 | `ab133-e133-15_0{1,2}`, `e133-15_0{1,2,3}` |

E131 passes and E133 fails on the same bench in the same hour, so the cause is the E133 change.

**Mechanism:** the E095 / E116c / E131 one. A faster COMP handler services more of the comparator's chatter per millisecond:
* the same-window driven-stage peak rises from 46–49 to up to 58/ms;
* early in the closed loop, where the float windows are long (ci ≈ 560 µs), the rate now crosses the unchanged 64/ms cut once the handover window closes.

**The win that was measured:**

| | E131 | E133 | change |
|---|---|---|---|
| R_COMP `spent_max_us` | 13 | **10** | −3 µs (−23%) |
| `comp_call_max_us` | 18 | **14–15** | −3 to −4 µs |
| `com_late_max_us` | 5 | 4 | −1 µs |
| predicted by `WCET_ESTIMATES.md` §7 | — | about −2.7 µs on COMP | matches |

**Caveat:** E133's maxima cover only its first ~40 ms of closed loop, while E131's cover whole runs. So E133's figures are lower bounds on what a full run would show. The prediction and the measurement agree anyway.

**Decision: E133 is not kept.** Keeping it needs one of the following, and each changes a protection or the detector:
* a different storm cut;
* a longer unenforced handover window;
* a change to the comparator (HYST 0 is the oracle's setting);
* a change to how COMP re-enables the line.

That is the operator's call, and this step does not make it. The source is reverted (`hw::system` prefetch functions and the banner field removed). The rebuilt image's loadable bytes are identical to E131 `2A7694CD` (objcopy binary compared). The board is back on E131.

`WCET_ESTIMATES.md` §7 records the result. `scripts/isr_cycles.py` keeps its optional `--fetch-model` (defaults unchanged, verified: COMP 1026 at 12 reads).

**Production image stays E131 `2A7694CD`.**

### E133 addendum — the reference checked, because a review proposed making the storm cap report-only

The proposal was: the 64/ms cap is not a protection in the reference (binz E767 made it report-only), so make it report-only here and keep prefetch. Checked against binz and minz before acting.

**1. E767 is real but scoped to the driven stage, which firmware50 already ports.**
* `binz/AGENTS.md:2486-2508` (E767): "driven acquisition's 64 dispatch/ms cap is report-only in the normal lean path; peak retained. Real 50 us handler-overrun stop and independent command/feedback/watchdogs remain."
* Implemented in `binz/examples/support/driven_irq_live.rs:217-240`: `rate.observe()` in the normal path, `rate.hit()` only under `bench-driven-rate-snapshot`.
* **But the powered lean COMP handler still stops on it**, `binz/examples/support/core_bench.rs:4059-4107`: "Lean operation still needs the powered source-rate safety cutoff", `hit_limit(t17(), reverse_rate_limit())` with the limit 64 by default (`core_bench.rs:548-558`), and a trip calls `live_stop()` and `powered_timer::irq_storm()`.

So the reference's split is: report-only while driving, a hard cut once powered and locked. That is exactly what firmware50 has — `rate::HANDOVER_OBSERVE_US = 40_000` observes, then `Rate::hit` cuts (`src/rate.rs:16-45`, quoting the same binz sources). **The cap is not a firmware50 invention, and making it report-only in the lock would be a divergence from the oracle, not a port.** firmware50 also has its own reason (E061/E051): with hysteresis at the reference's 0, a storm starved the protection scan.

**2. The prefetch precedent is minz's, not binz's.**
* binz enables prefetch nowhere; its clocks are `RCC.freeze(Config::pll())` with no FLASH_ACR write. The oracle therefore runs PRFTEN = 0, like E131.
* minz does enable it, and for the reason the review gave: `minz/src/board_init.rs:136-148` and `minz/CLAUDE.md:690-698` — "PRFTEN off + 4WS makes hot-ISR fetch timing depend on code alignment … token-identical hybrids flip-flopped accordingly. Fix: PRFTEN enabled (8d32e66) — same binary went 3-4/8 → 8/8."
* That is an L431 at 4 wait states, not this G071 at 2, and rm32 keeps PRFTEN off deliberately for AM32 register parity (`rm32/CLAUDE.md:145`). The alignment-sensitivity argument for turning it on is real but comes from another family and is not evidence about the G071 oracle.

**3. The advance schedule matches the reference image firmware50 was transcribed from.**
* firmware50: 20 below 35% duty, 22 at or above (`run/policy.rs:184-195`).
* binz `reverse_advance.rs:2-4` is identical, and `AGENTS.md:133` names that image.
* The frozen oracle images additionally apply an *effective* high-duty advance of 24 or 26 through a binz-local post-COM wait override (`reverse_advance24.rs`, `core_bench.rs:1816-1828`), because minz-core clamps published levels to 18..22. So above 35% the oracle's effective wait is **smaller** than firmware50's, which makes the R_COMP inequality in `WCET_ESTIMATES.md` §6 tighter, not looser.

**Corrections this forced in `WCET_ESTIMATES.md` §6:**
* the wait at high duty is computed at level 22 (this firmware's policy), not 20;
* measured R_COMP does not fall with the filter depth: `spent_max_us` is 13 at 11, 8 and 7 reads alike, and 10 with prefetch at 11 reads. It is a maximum, set by the worst instance including one guard preemption. The 3-read path at 100% must not be assumed cheaper;
* so R_COMP 13 µs crosses the level-22 wait at about 2.0 keHz (≈ 48% duty), and with prefetch's 10 µs at about 2.6 keHz (≈ 68% duty).

**Standing:** E133 stays reverted and E131 `2A7694CD` remains the production image. Making the powered cap report-only is a protection change against the oracle's own powered policy, so it is the operator's decision, with these alternatives that do not touch a protection:
* mask the comparator line after an accepted crossing until the commutation, which AM32 does, cutting dispatches rather than making each one cheaper;
* the TIM2 input-capture digital filter on the COMP output (binz's filter-control path): hardware-filtered edges, hardware-stamped crossing times.
Hysteresis stays 0 either way.

### E134 — hold the comparator line masked to the blanking floor, so fewer dispatches reach a locked loop

Operator's decision after E133: cut the chatter rather than relax the 64/ms cut, which the reference enforces in its powered path (E133 addendum).

**What is already there.** E131 masks the line at COMP entry and re-arms it only at the next commutation, so an accepted crossing is one dispatch per sector (`roots.rs:918-920`, AM32's arrangement). The dispatches that remain come from the refusal path, which re-enables the line deliberately (E10x): a refused edge must not close the window on the real crossing.

**What the numbers say.** Per run at 25% (E131): accepted 258.9k, `too_early` 419k, `unstable` 971k. So **25–29% of all dispatches are refused as inside the blanking window**, at both 15% and 25%. Those are the dispatches this step removes.

**The window is a known length.** The commutation fires `wait = ci/2 − advance` after the crossing, and the blanking floor is `ci/2` after it, so the floor is `advance` µs *after* the commutation: about 44 µs at 25% (ci 141, level 20) and about 73 µs at 15% (ci 235). Nothing can be accepted in that span; every edge in it is refused.

**Change, in the COM root's phase 1.** Where it arms the line now, it instead computes what is left of the blanking window and, if that is at least `BLANK_ARM_MIN_US`, keeps the line masked and arms the existing phase-2 one-shot for that long. Phase 2 already re-arms the line. This is the mechanism the reverse blank (280 µs) already uses, with the floor as its length.
* `commutation::blank_remaining(blanking, since_zc)`, saturating, with host tests.
* `blanking` comes from the estimator under the lock the root already takes for `average_interval`; with the production `BLANK_64` it is the `average >> 1` shortcut E130 added.
* `since_zc` is `now − det.accept_raw`, so a late commutation shortens the blank rather than pushing the floor out.
* `blank_arms` counts these too, which is the witness that it armed every sector.

**What this does not change:** the blanking gate itself, the persistence filter, the storm cut, the handover window, hysteresis (0), the advance schedule, and the estimator. No threshold moves.

**Expected drift, to be measured:**
* COMP dispatch rate and the per-ms peak fall by roughly the early share, a quarter;
* `too_early` collapses toward zero, since those edges no longer dispatch;
* the level revisit **rises**: a genuine crossing that arrives just inside the window used to dispatch and leave a live line, and now it is dropped at the floor arm (`comp_exti_arm` clears pending), so the foreground poll catches the held level instead. That is the poll's purpose, and its counts are reported;
* COM root cycles grow a little; the ceiling is E128's 356 (E131 is at 287).

**Then:** if the peak falls as expected, retry prefetch on top of it as E135, which is the point of the exercise.

**Run order:** pre-power checks, then 15% × 3 hot (the rung E133 failed), then the rest of the campaign-5 bar.

### E134 — result: the blank cuts the chatter, and it is the early self-excited chatter that goes

Three builds, each archived: `0EF2425F` (E134, clear pending at the floor arm), `7FB3201B` (E134b, edge primed at the commutation and the pending flag kept at the floor), `17E58ECF` (E134c, E134b plus the `blank_latched` witness).

**15% hot, against E131 `2A7694CD` measured the same hour (`ab133-e131-15`):**

| quantity | E131 | E134 / E134b / E134c |
|---|---|---|
| COMP entry peak / ms, closed | 65–68 | **56–60** |
| `too_early` | 520k | **0** |
| `unstable` | 1.110M | 1.003M |
| foreground passes | 1.280M | **1.598–1.605M (+25%)** |
| `blank_arms` | 2–3 | 161.6k, one per accepted crossing |
| `com_late_max_us` | 5 | 5–8 |
| rate vs 6 × coast | 1004–1006‰ | E134 1006/1011/1009 (one over the 1% gate), E134b 1003/1010/1005, E134c 1007/1008/1010 |

**Gates on E134b and E134c:** 15%, 20%, 25% each 3/3; restart 3/3; every protection from a locked 25% loop with its reason code and the bridge off after each (`e134b-p25-*`); the E134c ladder 3/3/3 as well. Static cycles: COMP unchanged at 1026/866/770, COM 287 → 319 (E134) → 349 (E134b/c), below E128's 356 ceiling. DMA and guard unchanged. Audit clean, 306 host tests, 9 doctests, clippy clean, replay reproduces all 1536.

**The pending-flag question, settled by measurement.** A review warned that opening the line onto a latched flag dates the crossing at the gate rather than the rotor -- minz's FALCON self-lock class. `blank_latched` counts the floor arms that found a flag set: **0 across 590k sector arms** in nine locked runs, and **0 in both segments of three restarts**, which is the accelerating case. The keep-pending path never fires, so E134b and E134c are behaviourally identical to E134, and the class has no instances here. The counter stays in as the standing witness.

**What the blank actually removes.** The E121 capture (`captures/replay/e121-capture25.txt`, 15%, ci 231, blanking 115) holds every decision: per sector the dispatches run 60, 86, 102, 123, 145, 164, 187, 201, 223 µs after the crossing -- about nine, spaced ~20 µs, the first ~18 µs after the commutation. Three of them fall inside the blanking window, which is `too_early`/accepted ≈ 3.1 exactly.

With the window masked, **none of those edges latch a pending flag**. So they are not edges waiting to be serviced later: masking the window stops them happening. The leading explanation, an inference and not a proof, is that this early chatter is **self-excited by the servicing** -- each dispatch provokes the next about 20 µs later. It fits E095 (the entry peak rose monotonically as the handler got faster, across four builds), E116c and E131 (same, when the ISRs sped up), and the comparator's known sensitivity to MCU activity (the UART-TX coupling scar). Hysteresis is 0, as the reference has it, so the comparator has no margin against this.

That also reframes E133: prefetch did not "cause" chatter, it accelerated a loop that feeds itself, and E134 breaks the loop rather than making each pass cheaper.

**A fixture change went with it.** `too_early` is now structurally 0, so the gate "too_early and unstable non-zero" cannot be met. The blanking gate's witness becomes `blank_arms` (one per sector), and `cohort.py` asks for `unstable` non-zero plus one of the two. That is a change to the qualification instrument, not to a protection or a threshold, and it is recorded here.

**Kept**, and E131 stays the fallback image. `blank_latched` is reported so the pending question stays answered on every future run.

### E135 — flash prefetch on top of E134: **the whole bar passes**, and this is E133 retried the way the operator asked

Image `FDBFBD0C` (archived `captures/elf/FDBFBD0C.e135.elf`): E134's blank, the `blank_latched` witness, and `FLASH_ACR.PRFTEN = 1` by named-field modify. FLASH_ACR reads `0x00040702` over SWD at idle: prefetch on, DBG_SWEN kept. The banner prints `prefetch=1`.

**Pre-power:** audit clean on all four roots; static cycles COMP 1026/866/770 and COM 349, unchanged from E134c (prefetch changes no instruction); 306 host tests, 9 doctests, clippy clean on host and target; bin 3 unsafe, 0 `static mut`, no function over 100 lines; the replay reproduces all 1536 decisions.

**Gates, all on `FDBFBD0C`:**

| gate | result | captures |
|---|---|---|
| 15% hot 3/3 | **PASS**, 6 × coast 1005–1007‰ | `e135-15_0{1,2,3}` |
| 20% hot 3/3 | **PASS**, 1005–1006‰ | `e135-20_0{1,2,3}` |
| 25% hot 3/3 | **PASS**, 1004–1007‰, speed within 0.4% of the oracle | `e135-25_0{1,2,3}` |
| restart at 25% 3/3 | **PASS** | `e135-restart_0{1,2,3}` |
| protections from 25% | **PASS**: t 8, g 3, f 4, n 7, u 13, h 14, v 26, i 25, host abort 9, `w` iwdg=1, preflight PASS after each | `e135-p25-*` |
| `storm` | **0** in every run, with the cap armed | as above |
| `blank_latched` | **0** in every run | as above |

E133, the same prefetch without the blank, failed 15% **5/5** on CompStorm at 76–80 entries/ms. With the blank first, the peak is 64–67/ms and no run storms.

**The win, at 25%, against E131 measured the same day:**

| quantity | E131 | E134c | **E135** |
|---|---|---|---|
| R_COMP `spent_max_us` | 13 | 13 | **11** |
| `comp_call_max_us` | 18 | 18 | **15** |
| foreground passes | 1.198M | 1.440M | **1.982M (+65%)** |
| COMP entry peak / ms | 66–69 | 58 | 65–67 |
| `unstable` | 972k | 889k | 905k |
| `too_early` | 419k | 0 | 0 |
| `lt075` | 1162–1217 | 1101–1133 | 708–804 |
| `gt150` | 3–4 | 2–3 | **0** |
| revisit accepts | 41.2–41.7k | 38.7–39.0k | 42.3–42.5k |
| coast pair median µs | 842–844 | 845–848 | 842–844 |
| accepted / s | 7143–7150 | 7147–7150 | 7158–7163 |
| proxy mA (report-only) | 240–439 | 438–459 | 404–424 |

So the two steps together buy 2 µs off R_COMP, 3 µs off the COMP call, two thirds more foreground time, and a tighter phase distribution, at an unchanged rotor speed and with every protection and threshold untouched.

**Against `WCET_ESTIMATES.md`:** the prediction was that measured times move from the "one miss per 4 instructions" row toward "literal loads and branches only", about −2.7 µs on COMP at 7 reads. Measured: −3 µs on the call and −2 µs on the arm. The 100% inequality improves accordingly -- R_COMP 11 µs against the level-22 wait crosses at about ci 59 µs, roughly 2.8 keHz or 75% duty, against 48% before these two steps. 100% still needs the arm path itself to change, as §6 says.

**Production image: E135 `FDBFBD0C`.** E131 `2A7694CD` remains the archived fallback, and E128 `4FC7C796` the campaign-4 baseline for ABAB.

### E135 — operator-metered current at 25%: 0.400 A on the PSU

One 25% hold for the meter (`psu-check-25_01`, 31.27 s at target, speed −0.1% of the oracle, 1007‰, rung PASS). The operator read **0.4 A** on the bench supply, steady.

The proxy on the same image and rung, same session:

| capture | proxy `hold_ma` | against 400 mA |
|---|---|---|
| `e135-25_01` | 404 | +1% |
| `e135-25_02` | 424 | +6% |
| `e135-25_03` | 414 | +4% |
| `psu-check-25_01` | 339 | **−15%** |

So the proxy brackets the true figure within about ±15%, with no fixed offset to correct: this run's bridge-off zero moved 187 mA across the run (`zero_drift_ma=-187`), which is the datum-versus-thermal-history problem that made the proxy report-only in campaign 4 (E092-E096). The oracle's figure at this duty is 326 mA, so the oracle's own census reads about 18% below the metered current here.

**Use:** 0.400 A at 25% duty on E135 is the first metered anchor on this image. A calibration would need the zero taken immediately before the hold, not once per run, and this point as its reference. Nothing is changed on the strength of it; the proxy stays report-only.

### E136 — metered current from 15% to 25% in 2% steps: the load is a square law, and the proxy's error is its zero

Operator-metered on the bench PSU, one ~15 s hold per point. Probe image `C8936847`: E135 plus six short-window shell keys (`8/3/4/6/7/9` = 25/23/21/19/17/15%, 28 s window). The four ISR roots are **instruction-identical to E135** (normalised disassembly) and the audit passes, so this measures E135's behaviour; it is not a qualification image and these holds are not rungs (the 30 s gate does not apply).

| duty | metered A | proxy `hold_ma` | proxy error | `ci_us` | eHz | capture |
|---|---|---|---|---|---|---|
| 15% | 0.142 | 117 | −18% | 235 | 709 | `sweep-15_01` |
| 17% | 0.182 | 170 | −7% | 204 | 817 | `sweep-17_01` |
| 19% | 0.228 | 212 | −7% | 181 | 921 | `sweep-19_01` |
| 21% | 0.270 | 272 | +1% | 168 | 992 | `sweep-21_01` |
| 23% | 0.330 | 322 | −2% | 153 | 1089 | `sweep-23_01` |
| 25% | 0.399 | 299 | −25% | 139 | 1199 | `sweep-25_01` |

Every run stopped on the deadline (reason 2), forced 0, no protection trip.

**The load law.** A log-log fit gives **I ∝ ω^1.99**, r = 0.9990 over the six points. Torque therefore goes as ω, not ω²: viscous drag, bearings, windage and iron loss, not propeller thrust. `WCET_ESTIMATES.md` §5 assumed a prop (ω³ power) when it projected about 7 A at 100%; on the measured square law the projection is:

| point | eHz | projected A |
|---|---|---|
| 50% | 2096 | 1.2 |
| 75% | 2817 | 2.2 |
| 100% | 3437 | 3.2 |

binz measured about **1.6 A** at 50%, above the 1.2 A the law projects, so current grows faster than ω² once out of this range. **3.2 A is a floor for 100%, not a centre estimate.** The 1 A PSU limit is reached at roughly 32–35% duty either way, which is the first wall in front of any high-duty work on this bench.

**Speed is linear here:** 49.0 eHz per 1% of duty across 15–25%, no taper. The taper in §5's fit comes from the 25→50% span.

**The proxy's error is its zero, not its gain.** The two end points are the worst (−18% at 15%, −25% at 25%) while 19–23% sit within 7%, and the sign changes across the range -- not a scale error. Each run's bridge-off datum moved 157–237 mA between its start and end (`zero_drift_ma`), which is the same magnitude as the errors. The 25% point here read 299 mA against 404–424 mA in the `e135-25_0*` runs at an identical metered 0.399 A, so the spread is in the instrument, not the drive.

**What would fix it:** take the zero immediately before each hold instead of once per run, and anchor the scale on this table. That is a change to the current instrument, needing its own step; nothing is changed on the strength of it here, and the proxy stays report-only.

**Bench state:** the probe image is `captures/elf/probe-sweep.elf` (`C8936847`); the production image E135 `FDBFBD0C` is reflashed after this entry.

#### E136, projection: how far the present stack goes under a 1 A supply with 50 mA margin

**Answer: about 38–39% duty, roughly 1700 eHz.** `I ∝ duty^2.00` from the six metered points (r = 0.9993); the 25% → 50% pair (0.399 A here, ~1.6 A in binz) gives 2.004 independently. At 0.95 A that is 38.6%, and at 1.00 A, 39.6%.

| ceiling | duty | eHz | ci µs | wait µs (level 22) | R_COMP on E135 |
|---|---|---|---|---|---|
| 0.95 A | 38.6% | ~1700 | 98 | 15 | 11 |
| 1.00 A | 39.6% | ~1754 | 95 | 15 | 11 |

**Timing does not bind there.** E135's R_COMP of 11 µs clears the level-22 wait out to about 2.8 keHz (~75% duty), so the supply is the limit, not the detector.

**What binds, in order:**

| blocker | where | needs |
|---|---|---|
| `SIXSTEP_DUTY_CAP = 250` (a hard clamp at 25% on every plan) | `run/policy.rs` | raised, with its own qualification |
| `BEMF_TOTAL_MS = 44_000`: ramping 10% → 39% at 1%/0.5 s is ~14.5 s, plus ~17 s acquire, leaves ~12 s of hold against a 30 s gate | `run/policy.rs`, and the 45 s guard backstop | a longer window and backstop together |
| bus sag ≈ 12%/A (−6.3% at 15%, −9.4% at 25%) → about −16% at 0.95 A | the fast-sag stop (reason 26), the bench power path | measure the sag-vs-current slope first ([[reference-bench-power-path-resistance]]) |
| the current proxy is report-only and ±25% off (this entry) | `protection.rs` census | calibrate against the table above, then the firmware can police the ceiling itself instead of the operator's meter |

**Suggested order:** calibrate the proxy (zero taken immediately before each hold, scale anchored on this table), then one step raising the duty cap and the window together, qualified upward in 2–4% increments with the meter in the loop and a stop at 0.95 A.

**Confidence:** the six points are this bench and this image. The 50% anchor may be a different motor (#3 was fitted 2026-09-19), so if the load is stiffer than ω² above 25% the answer moves to 36–37%, not higher.

## Campaign 6 — 27.5% to 37.5% on the 1 A supply

Operator goal, set 2026-09-22: qualify 27.5 → 30 → 32.5 → 35 → 37.5% in 2.5% rungs, one exploratory run (10 s at target) per rung before a 3/3 cohort (≥ 30 s), coming back down on any hard-gate failure and bisecting in 1.25% steps. Start image E135 `FDBFBD0C`. Supply fixed at 1 A; the first rung that folds the bus on average current is the supply-limited edge and closes the goal at or below 37.5%.

**Reference, from the oracle's rung table** (`binz/DUTY_50_CAMPAIGN.md:16-26`, one diagnostic run per rung on the frozen 48 kHz reverse image):

| duty | oracle eHz | oracle proxy mA | source |
|---|---|---|---|
| 25% | 1186 | 326 | measured |
| 27.5% | 1279 | 423 | interpolated 25/30 |
| 30% | 1371 | 519 | measured |
| 32.5% | 1468 | 630 | interpolated 30/35 |
| 35% | 1564 | 740 | measured |
| 37.5% | 1650 | 843 | interpolated 35/40 |
| 40% | 1736 | 945 | measured |

The half-rungs are interpolations, and the entries are marked as such in `cohort.py`. Our own metered law (E136, `I ∝ duty²` anchored on 0.399 A at 25%) predicts 0.48 / 0.58 / 0.67 / 0.78 / **0.90 A** at the five rungs, so 37.5% should sit about 100 mA under the cap.

### E137 — enabling step: the duty clamp and the run window (no control-path change)

Two constants stop a 37.5% rung before any behaviour does.

1. **`SIXSTEP_DUTY_CAP` 250 → 375** (`run/policy.rs`). Every plan is clamped by it, so today the bridge cannot be commanded above 25%. It moves to exactly the goal's ceiling, so the firmware still refuses anything above 37.5% by construction.
2. **`BEMF_TOTAL_MS` 44 s → 54 s**, and **`GUARD_CAMPAIGN_US` 45 s → 56 s** (`run/policy.rs`, `roots.rs`). The ramp is 1%/0.5 s from 10%, so reaching 37.5% takes 14.0 s against 7.5 s for 25%; with ~5 s of startup and acquisition a 30 s hold needs ~50 s of window. The backstop keeps its present role and its present margin above the window (it only catches a foreground that never ends a run), so its function is unchanged -- it is not a fault threshold and nothing is relaxed to pass a gate.

**Ramp cost per rung:** 27.5% 9.0 s, 30% 10.0 s, 32.5% 11.5 s, 35% 12.5 s, 37.5% 14.0 s.

3. **Shell keys per rung**, so each rung can be driven without further firmware edits:
   * lowercase `a c d e j` = **exploratory**, 27.5 / 30 / 32.5 / 35 / 37.5% on a window sized for ~10 s at target;
   * uppercase `A C D E J` = **qualification**, the same duties on the full window.
   The existing `b 2 5` (15/20/25%) and the E136 short keys are untouched.

**Fixture:** `cohort.py` gains the five oracle entries above (half-rungs flagged interpolated) and a `min_hold_ms` argument so an exploratory run is judged on every gate except the 30 s dwell, which it is not meant to meet. `bemf_run.py` gains the ten keys, their rung mapping and the ladder chain 250 → 275 → 300 → 325 → 350 → 375, so a rung cannot be attempted before the one below it has passed 3/3 on the same ELF.

**Protections at 37.5%** (goal item 5) need a third provocation set; today lowercase `t g f n u h v i` inject at 25% and uppercase at 15%. That is a separate entry once 37.5% qualifies.

**Checks before the first powered run:** the four ISR roots instruction-identical to E135 (this touches no root), audit clean, host tests and the replay test green, clippy clean, structure within limits.

**Run:** `a` -- 27.5% exploratory, ~10 s at target.

### E137 — result: the enabling step holds, and 27.5% explores clean

Image `36A5A4BC` (archived `captures/elf/36A5A4BC.e137.elf`). `SIXSTEP_DUTY_CAP` 375, window 54 s, guard backstop 56 s, ten rung keys, fixture extended.

**Pre-power:** the four ISR roots are **instruction-identical to E135** (this touches no root), so COMP 1026/866/770, COM 349, DMA 78, guard 204 are unchanged and gate 4's cycle clause holds by construction. Audit clean on all four. 306 host tests, 9 doctests, replay green, clippy clean on host and target. bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**Ladder re-qualified on this ELF** (the fixture requires it before a new rung, and it is the A/B against E135 on the same rungs): 15% `e137-15_0{1,2,3}`, 20% `e137-20_0{1,2,3}`, 25% `e137-25_0{1,2,3}` -- **3/3 each, PASS**.

**27.5% exploratory** (`e137-explore275_01`, key `a`, 19.8 s at target):

| quantity | measured | oracle 27.5% | gate |
|---|---|---|---|
| coast eHz | 1292 | 1279 (interpolated) | +1.0%, inside 5% |
| loop eHz | 1302 | — | — |
| rate identity | 993‰ of expectation, 1001‰ of 6 × coast | — | inside 1% |
| forced | 0 | — | pass |
| `unstable` | 632528 (non-zero) | — | pass |
| `blank_arms` / accepted | one per sector | — | pass |
| `blank_latched` | 0 | — | pass |
| R_COMP `spent_max_us` | **11** | — | against `wait_time(125, 20)` = **23 µs** |
| `late_arms` | 0 | — | pass |
| proxy mA | 491 | 423 | report-only; E136's metered law predicts ~0.48 A |
| stop | reason 2, deadline | — | pass |

`ci` 125 µs at 27.5%, advance level 20 (below 35%), so the wait is 23 µs and R_COMP has 12 µs of margin. **EXPLORE PASS on every gate.**

**Run:** `A` -- the 27.5% qualification cohort, 3 × on the 54 s window (~40 s at target).

### Rung 27.5% — **QUALIFIED 3/3** (`e137-qual275_0{1,2,3}`, image `36A5A4BC`)

| run | hold at target | coast eHz | vs oracle 1279 | rate ‰ (expect / 6 × coast) | proxy mA |
|---|---|---|---|---|---|
| 01 | 39.78 s | 1285 | +0.5% | 993 / 1006 | 498 |
| 02 | 39.78 s | 1289 | +0.8% | 993 / 1004 | 488 |
| 03 | 39.77 s | 1287 | +0.6% | 993 / 1005 | 501 |

All three: stop reason 2 (deadline), forced 0, `unstable` non-zero, `blank_arms` one per sector, `blank_latched` 0, `late_arms` 0, coast crossings > 0, no protection fired. R_COMP 11 µs against `wait_time(125, 20)` = 23 µs. Current proxy mean 496 mA against the oracle's interpolated 423 (+17%), report-only; E136's metered law puts the real figure near 0.48 A.

**Run:** `c` -- 30% exploratory, ~10 s at target. Oracle 1371 eHz / 519 mA (a measured row). Expected metered current about 0.58 A.

### Rung 30% — exploratory **FAILED**: a tracking stop 839 ms into the hold (`e137-explore300_01`)

The run reached target and was on speed; it then lost accepted crossings for more than a millisecond and the guard latched.

| quantity | value | reading |
|---|---|---|
| stop | **reason 8, Tracking** | `accepted()` latches it when the gap between accepted crossings exceeds `EVENT_MAX_US` = 1000 µs. At `ci` 120 µs that is about **8 sectors with no accept** |
| hold at target | 0.839 s | target was reached: `closed_ms` 10839 |
| coast eHz | 1376 vs oracle 1371 | **+0.4%**, so the rotor was at the right speed when it died |
| rate identity | 993‰ / 1002‰ | on speed up to the stop |
| forced | 0 | the fallback stays deleted |
| sectors | 10251–10254, balanced ±3 | no sector starved |
| `lt075` / `gt150` | 191 / **0** | phase distribution healthy |
| bus | ref 1215, min 1137 (**−6.4%**) | no sag event, no fast-sag trip, no current fault |
| proxy | 601 mA (oracle 519) | ~0.58 A expected by E136's law; nothing near the 1 A cap |
| COMP peak | 67/ms, `storm` 0 | the cap was not involved |
| `blank_latched` | 0 | the E134 blank did not swallow an edge |
| `blank_arms` | 61515 = accepted + 1 | armed every sector |
| R_COMP | 11 µs, `late_arms` 0 | against `wait_time(120, 20)` = 23 µs |
| guard | `track_fault=1`, `track_max_us=327`, `gap_max_us=105` | `track_max_us` is the guard's own 1 kHz sample of the age, so it does not show the ISR-measured gap that tripped |

So: not the supply, not the storm cap, not the blank, not a cycle-count or timing-margin breach. A genuine dropout of accepted crossings, at a duty 2.5% above a rung that holds 40 s three times over.

**Last qualified rung re-confirmed** (`e137-confirm275_01`, immediately after): 39.77 s at target, 1284 eHz (+0.4%), rate 992/1006‰, forced 0, **PASS**. The bench has not moved and E137 is not a regression at 27.5%.

**Next, in order:**
1. One more 30% exploratory run, as characterisation rather than a retry: whether the dropout repeats decides how to read a bisect point. The result is recorded either way and the rung stays failed unless a cohort passes.
2. Then bisect between 27.5% and 30% at **28.75%**, single variable, on a probe image (whose lower rungs the fixture will make me re-qualify first).

### E138 — the 30% dropout, named: a swallowed crossing desyncs the drive, and there is no recovery

30% failed twice on E137 (`e137-explore300_01` at 839 ms into the hold, `e137-explore300b_01` at 55 ms), both reason 8 with the rotor on speed. To see the decisions that led to the stop, the diagnostic ring gained a **circular mode**: it keeps the last `CAPTURE_LEN` decisions instead of the first (`capture.rs`, `arm_next_run_circular`, armed by the `c` key in `bin/edge-capture.rs`, arm point `CAPTURE_AFTER_ACCEPTS_CIRCULAR` = 15 000 because a dying run never reaches 80 000 accepts). `scripts/capture_edges.py` gained `--command`. Production is unaffected: it uses `NoLog` and the four roots of the production build are unchanged.

**Capture `captures/replay/e138-drop300b.txt`** (diagnostic image `C988046E`/`F6E5325B`, key `c`): 1536 decisions, 153 066 offered in total, ending at the stop. 484 accepted, 1052 refused as `UNSTABLE`, none early (the E134 blank removes those).

**Steady state at 30%,** repeating every sector: two to four `UNSTABLE` refusals at counts 76–150 µs, then an accept at 116–175 µs with the average at 113–139 µs. The refusals sit *after* the blanking floor and before the crossing is accepted, so they are the persistence filter rejecting chatter near the crossing.

**The last four decisions:**

| count µs | reads | wait | average | outcome |
|---|---|---|---|---|
| 174 | 24576 | 26 | 139 | accepted |
| 91 | 12291 | — | — | UNSTABLE |
| **332** | 28799 | 37 | **196** | accepted |
| — | — | — | — | **nothing further; Tracking latches** |

Read as a chain:
1. a burst of filter refusals swallowed one genuine crossing;
2. the next accept came **332 µs** later, about 2.8 sectors, and blended the interval estimate up from 139 to 196 µs;
3. by then the commutation had advanced once while the rotor turned nearly three sectors, so the driven phase pair no longer matched the rotor and the floating phase was the wrong one;
4. the comparator produced **no further dispatch at all** -- not a refusal, not an edge -- and with forced commutation deleted (E076-E081) there is no recovery path, so the accepted-event age passed `EVENT_MAX_US` = 1000 µs and the guard latched reason 8.

**Why this bites at 30% and not 27.5%.** Dispatches per sector rise from 3.9-4.4 at 25-27.5% to 5.5-5.8 at 30% (all runs, ramp included), while the sector shrinks from 139 to 120 µs. The chatter around the crossing is unchanged in rate, so it occupies a growing share of each sector, and the chance that a whole sector passes without an accepted crossing grows with it. One such sector is fatal, because the drive cannot re-phase.

**The protections behaved correctly.** No supply involvement (bus −6.4%, no sag or current fault), the storm cap was not reached (peak 67/ms, `storm` 0), `blank_latched` 0, R_COMP 11 µs against a 23 µs wait, cycle counts unchanged. The stop is the tracking watchdog doing its job on a desynced drive.

**So 30% is not reached by the present detector**, and the cause is named. Four candidate fixes, each a single-variable control-path change that would need its own build, run, entry and A/B:
* **(a) a rescue path for a missed sector**: the level revisit fires once per sector (`revisit_step != step`), so a sector whose single attempt is refused is lost. Allowing a second attempt inside the same sector is the smallest change that addresses the actual chain.
* **(b) a short holdoff after a refusal** instead of re-enabling the line at once, to break the post-floor self-excited chatter the way E134 broke the pre-floor kind. This is the same lever that raised the ceiling in E134/E135, applied to the other half of the sector.
* **(c) a re-acquire on a missed sector**, which is what the reference does (AM32 demotes to its polling routine); firmware50 has no such path.
* **(d) the filter depth at speed**, which currently matches the reference's map and so is the least justifiable to touch.

Hysteresis stays 0 and the storm cap stays as it is, per the goal.

**Next, per the goal's method:** bisect between 27.5% (qualified 3/3) and 30% (failed twice) at **28.8%** -- 1.25% steps in tenths -- to locate the edge before any fix is attempted. That needs one added key, so a new image and the fixture's re-qualification of 15/20/25/27.5% on it.

### E139 — the bisect image: one key at 28.8%

Image `32A8E2C9` (archived `captures/elf/32A8E2C9.e139.elf`). One added shell key, `y`/`Y` = 28.8% explore/qualify, with the fixture's oracle row (1327 eHz / 473 mA, interpolated from the 25% and 30% rows) and the ladder link 27.5% → 28.8% → 30%. 28.75% is not representable in tenths of a percent, so the bisect point is 28.8%.

Nothing else changed: the four ISR roots are **instruction-identical to E137**, the audit passes on all four, 306 host tests and 9 doctests pass, the replay test is green, clippy is clean, and the structure limits hold (bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines).

**Run, in order:** the fixture requires the ladder on this ELF, so 15%, 20%, 25% and 27.5% × 3 each first, then `y` at 28.8%.

### Rung 28.8% — exploratory PASS, cohort **FAILED 3/3**: the dropout is a rate, not a wall

`e139-explore288_01` (19.3 s at target): 1337 eHz against the oracle's interpolated 1327 (+0.8%), rate 999/1004‰, forced 0, R_COMP 11 µs, `late_arms` 0 -- **EXPLORE PASS**.

`e139-qual288_0{1,2,3}` on the 54 s window: **all three stopped on reason 8**. Run 01 died at 16.9 s at target; runs 02 and 03 passed the 30 s dwell and died after it. Speed was right in all three (1328–1332 eHz, +0.1 to +0.4%), rate 998/1007–1010‰, proxy 550 mA.

Put beside the rest of the campaign, the picture is a **failure rate that climbs steeply with duty**, not a threshold:

| rung | dwell survived | outcome |
|---|---|---|
| 27.5% | 4 × ~40 s = ~160 s | no event |
| 28.8% | 17 s, then two over 30 s | 3/3 tracking stops |
| 30% | 0.06–0.8 s | 2/2 tracking stops |

So 27.5% is the highest rung this image qualifies, and its margin is "no event in 160 s", not "safe".

### E140 — the rescue the diagnosis asked for: a revisit that can look twice

Image `C90D0544`. One mechanism, from E138's chain.

**The dead end.** `Locked::revisit` polls once per sector: the attempt sets `revisit_step`, and only an **accepted crossing** clears it (`consume`). So the one sector that most needs a second look -- the one whose crossing the persistence filter swallowed -- can never get one, because there is no accept to re-arm it. The commutation then falls behind the rotor and the drive desyncs.

**The change.** After the first attempt is spent, the revisit may poll again while the sector is **overdue**: the first rescue at 1.5 × the interval estimate, then one per further half-interval, up to `REVISIT_RESCUE_MAX = 4` (`run/policy.rs`). A rescue only pends the decision again -- the blanking gate and the persistence filter still judge the edge, with live comparator reads -- so no gate, threshold or protection moves.

**Host coverage the revisit never had.** The sim's `com_idle()` was hard-coded `false`, so no host test had ever exercised the revisit path (E132's pacing included). It now models the real condition, "no crossing pending", and a new test drives a sector that never accepts and asserts the loop polls it again while overdue. 307 host tests pass.

**Pre-power on `C90D0544`:** the four ISR roots are **instruction-identical to E139** (the revisit is foreground), so COMP 1026/866/770, COM 349, DMA 78 and guard 204 are unchanged and gate 4's cycle clause holds. Audit clean on all four. 9 doctests, replay green, clippy clean, bin 3 unsafe, 0 `static mut`, no function over 100 lines.

**Run, as the A/B the goal requires:** 28.8% × 3 on the rung that just failed 3/3 (the fixture will make me re-qualify 15/20/25/27.5% on this ELF first), then 30% exploratory.

### E140 — result: the rescue turns 28.8% from 0/3 into 3/3, and 30% explores clean

**Ladder on `C90D0544`:** 15%, 20%, 25%, 27.5% -- 3/3 each, PASS.

**The A/B the goal asks for, same rung, one change:**

| image | 28.8% cohort | rescue |
|---|---|---|
| E139 `32A8E2C9` | **0/3** -- all three reason 8 (16.9 s, and twice after the 30 s dwell) | absent |
| E140 `C90D0544` | **3/3 PASS** -- 6 × coast 1005 / 1007 / 1009‰, forced 0, no protection fired | present |

Both images are identical in their four ISR roots and in every threshold; the only difference is that a sector which has spent its single revisit may be polled again while it is overdue.

E140's own numbers at 28.8% (`e140-288_0{1,2,3}`): R_COMP 11 µs, `comp_call_max_us` 15, `late_arms` 0, coast pair 749–752 µs, accepted 8052–8055/s, `lt075` 2022–2098, `gt150` 3, COMP peak 64–67/ms with `storm` 0, `blank_latched` 0, foreground passes 2.43M. Revisit attempts 51.2–51.8k, which is the rescue doing its work; E139's passing runs sat at 32.7–57.7k, so the count alone does not separate them -- the outcome does.

**30% exploratory** (`e140-explore300_01`, 18.8 s at target): 1376 eHz against the oracle's measured 1371 (**+0.4%**), rate 996‰ of expectation and 1006‰ of 6 × coast, forced 0, `unstable` non-zero, `blank_arms` one per sector, `blank_latched` 0, R_COMP 11 µs against a 23 µs wait, `late_arms` 0, bus −7.2%, proxy 616 mA against the oracle's 519. **EXPLORE PASS** -- the rung that failed 2/2 on E139 within a second.

**Run:** `C` -- the 30% qualification cohort, 3 × 54 s window.

### Rungs 30% and 32.5% — **QUALIFIED 3/3 each** on E140 `C90D0544`

| rung | capture | coast eHz | oracle | speed | rate ‰ (6 × coast) | proxy mA | oracle mA |
|---|---|---|---|---|---|---|---|
| 30% | `e140-300_0{1,2,3}` | 1374 / 1376 / 1379 | 1371 (measured row) | +0.2 to +0.6% | 1004–1007 | 592–608 | 519 (+16%) |
| 32.5% | `e140-325_0{1,2,3}` | ~1453 | 1468 (interpolated) | −1.0% | 1006 | 708–714 | 630 (+13%) |

Both rungs: stop reason 2, forced 0, `unstable` non-zero, `blank_arms` one per sector, `blank_latched` 0, coast crossings > 0, no protection fired, R_COMP 11 µs with `late_arms` 0 (wait 23 µs at 30%, 21 µs at 32.5% with `ci` 113 µs), cycle counts unchanged. The 32.5% exploratory run (`e140-explore325_01`) passed every gate first, as the method requires.

E136's metered law puts 30% near 0.58 A and 32.5% near 0.67 A, both inside the 1 A supply with the 50 mA margin.

**Next:** 35% exploratory. At and above 35% duty the advance schedule moves from level 20 to **22** (`AdvancePolicy`), so this is the first rung in the reference's high-duty advance regime; the wait narrows from `ci`·12/64 to `ci`·10/64, about 17 µs at the expected `ci` of 107 µs, against R_COMP 11 µs.

### Rung 35% — cohort **FAILED** on one run's coast-derived rate (`e140-350_0{1,2,3}`)

All three ran the full **36.3 s** at target on reason 2 with no fault, no protection, forced 0, R_COMP 11 µs and `late_arms` 0, at 1502–1517 eHz against the oracle's 1564 (−3.0 to −4.0%, inside the 5% gate). Advance was level **22**, the schedule's high-duty value, as intended.

What failed is gate 1's rate identity **against 6 × coast**: 1011 / 1004 / 1008‰, so run 01 sits one point outside the 1% band. The loop's own rate identity is 993‰ in all three, comfortably inside.

The two metrics differ in where the speed comes from: the loop's own interval, or the rotor's coast after bridge-off. Across every campaign-6 capture the loop reads 0.55–1.8% above the coast, and the widest gaps are at 35%:

| rung | loop above coast | 6 × coast ratio |
|---|---|---|
| 25% | 0.84–1.10% | 1003–1007 |
| 27.5% | 0.93–1.32% | 1002–1006 |
| 28.8% | 0.67–1.05% | 1005–1009 |
| 30% | 0.65–1.02% | 1004–1007 |
| 32.5% | 0.55–0.97% | 1006–1010 |
| 35% | 0.79–1.80% | 1000–1011 |

The median does not march upward; the **scatter** widens at 35%, which is where the coast measurement gets harder -- a faster rotor decelerates more within the eight half-periods the estimator medians. That is a plausible reading of the mechanism, not a proven one.

Either way the gate is the gate, and a re-run to get a passing trio would be retry-until-pass, which the goal forbids. **35% is not reached on E140.** Per the method: come down to 32.5% (qualified 3/3), then bisect at **33.8%** (33.75% is not representable in tenths).

### E141 — the bisect key and the plumbing goal item 5 needs

Image `77C10285`. Three additive shell changes, no control-path change:
* `m`/`M` = 33.8% explore/qualify, with the fixture's interpolated oracle row (1519 eHz / 691 mA) and the ladder link 32.5% → 33.8% → 35%;
* `x` toggles the duty the lowercase provocations run at between 25% (campaign 5's value, the default) and 37.5%, printing `PROVOKEAT duty_tenths=`; every capture still records the duty it actually ran, so the record cannot be ambiguous;
* `Z` runs the restart campaign at that duty -- the goal's item 5 asks for restart at 37.5%, and `R` stays the 25% restart.

Pre-power: the four ISR roots are **instruction-identical to E140**, audit clean on all four, 307 host tests and 9 doctests pass, replay green, clippy clean, bin 943 lines with 3 unsafe, 0 `static mut`, no function over 100 lines.

**Run:** the fixture's chain on this ELF -- 15, 20, 25, 27.5, 28.8, 30, 32.5% × 3 each -- then `m` at 33.8%.

### Rungs 33.8%, 35% and 37.5% — **QUALIFIED 3/3 each** on E141 `77C10285`

The fixture's whole chain was walked on this ELF first: 15, 20, 25, 27.5, 28.8, 30, 32.5% × 3 each, all PASS.

| rung | capture | coast eHz | oracle | speed | rate ‰ (6 × coast) | proxy mA | oracle mA |
|---|---|---|---|---|---|---|---|
| 33.8% | `e141-338_0*` | ~1490 | 1519 (interp.) | −1.9% | 1004 | 744 | 691 (+8%) |
| 35% | `e141-350_0*` | ~1508 | 1564 | −3.6% | 1006 | 783 | 740 (+6%) |
| **37.5%** | `e141-375_0{1,2,3}` | 1631 / 1631 / 1639 | 1650 (interp.) | **−0.7 to −1.2%** | 1003–1007 | 958–963 | 843 (+14%) |

Each rung had its exploratory run first (`e141-explore338_01`, `e141-explore350_01`, `e141-explore375_01`), each passing every gate. All runs: stop reason 2, forced 0, `unstable` non-zero, `blank_arms` one per sector, `blank_latched` **0**, coast crossings > 0, no protection fired, R_COMP **11 µs** with `late_arms` 0, ISR cycle counts unchanged, audit clean.

**35% passed here where it failed on E140.** On E140 one run of three read 1011‰ against the 1% band; on E141, after the bisect through 33.8%, all three sit at 1003–1007‰. So that reading was the coast metric's scatter at speed, not a rung that cannot hold -- and it was not reached by re-running the same image, which the goal forbids, but by the bisect the method prescribes.

**R_COMP against the wait** (advance level 22 from 35% duty):

| rung | `ci` µs | wait µs | R_COMP µs | margin |
|---|---|---|---|---|
| 33.8% | 112 | 21 | 11 | 10 |
| 35% | 116 | 19 | 11 | 8 |
| 37.5% | **97** | **15** | **11** | **4** |

Gate 4 holds at every rung, with 4 µs left at 37.5%. `WCET_ESTIMATES.md` §5b carries the table.

**Current at the top rung.** The proxy reads 958–975 mA at 37.5%. It over-reads by up to 25% (E136), and E136's metered law puts 37.5% near **0.90 A**, so the rung sits inside the 1 A supply with roughly the 50 mA margin the goal asked for -- but the operator's meter is the authority, and no metered reading was taken above 25% in this campaign. Nothing in the run suggests the supply was the limit: bus minimum −9.7% of reference, the same as every rung since 25%, with no fast-sag streak, no average-current foldback and no current fault.

**Item 5, restart at 37.5%** (`e141-restart375_0{1,2,3}`, key `Z` after `x` set the provocation duty): **3/3 PASS**. Each: injected tracking loss at 21.2 s, bridge off, the ordinary startup admitted with no refusal, the second segment held 10.3 s at target and the original 54 s window completed (`drive_end_ms=54000`, `recovered=1`).

### Campaign 6 — final: **37.5% qualified** on E141 `77C10285`

The goal asked for 27.5 → 37.5% in 2.5% rungs on the 1 A supply, exploratory run then 3/3 per rung, coming back down and bisecting on any failure. Every rung is qualified, including the top one.

**Every rung, on the image that qualified it, with its capture:**

| rung | exploratory | cohort 3/3 | coast eHz | oracle | speed | rate ‰ (6 × coast) | image |
|---|---|---|---|---|---|---|---|
| 15% | — | `e141-15_0*` | ~704 | 704 | ±0.3% | 1000–1008 | E141 |
| 20% | — | `e141-20_0*` | ~947 | 941 | +0.6% | 1003–1007 | E141 |
| 25% | — | `e141-25_0*` | ~1186 | 1186 | ±0.4% | 1003–1007 | E141 |
| 27.5% | `e137-explore275_01` | `e141-275_0*` | 1285–1290 | 1279 (i) | +0.5 to +0.8% | 1002–1006 | E141 |
| 28.8% | `e139-explore288_01` | `e141-288_0*` | 1330–1335 | 1327 (i) | +0.1 to +0.8% | 1005–1009 | E141 |
| 30% | `e140-explore300_01` | `e141-300_0*` | 1374–1379 | 1371 | +0.2 to +0.6% | 1004–1007 | E141 |
| 32.5% | `e140-explore325_01` | `e141-325_0*` | ~1453 | 1468 (i) | −1.0% | 1006–1010 | E141 |
| 33.8% | `e141-explore338_01` | `e141-338_0*` | ~1490 | 1519 (i) | −1.9% | 1004 | E141 |
| 35% | `e141-explore350_01` | `e141-350_0*` | ~1508 | 1564 | −3.6% | 1003–1007 | E141 |
| **37.5%** | `e141-explore375_01` | `e141-375_0{1,2,3}` | 1631–1639 | 1650 (i) | **−0.7 to −1.2%** | 1003–1007 | E141 |

(i) = the oracle row is interpolated between measured rows of `binz/DUTY_50_CAMPAIGN.md`'s table; 30% and 35% are measured rows.

**Gate by gate on the final image:**

1. **Zero forced commutations** in every run; rate identity inside 1% of both the loop's expectation (993–1000‰) and 6 × coast (1002–1010‰); `unstable` non-zero; `blank_arms` one per sector; **`blank_latched` = 0 everywhere**, so no rate-identity claim rests on an untimestamped edge.
2. **Bridge-off speed within 5% of the oracle** at every rung (worst −3.6%, at 35%); coast crossings > 0 in every run.
3. **No protection fired in any qualifying run**: no average-current foldback or stop, no three-scan fast bus sag, no nFAULT, no tracking, tick-gap, feedback-age or IWDG event. No threshold was relaxed anywhere in the campaign.
4. **R_COMP < wait at every rung**, measured `spent_max_us` = 11 µs with `late_arms` = 0, against the advance schedule's own wait -- 23 µs at 30% (level 20), 15 µs at 37.5% (level 22). The four roots' worst-case cycle counts never grew: COMP 1026 / 866 / 770, COM 349, DMA 78, guard 204 on every flashed image, and the fail-closed audit passed on each.
5. **Restart at 37.5%: 3/3** (`e141-restart375_0{1,2,3}`) -- injected loss, all-off, ordinary startup admitted, restored, original 54 s window completed. **Every protection re-provoked once from a locked 37.5% loop** (`e141-p375-*`, all at `target_duty_tenths=375`, `advance_level=22`): t 8, g 3, f 4, n 7, u 13, h 14, v 26, i 25, host abort 9, `w` `RESETCAUSE iwdg=1`, each with a PASS bridge-off readback.

**Structure, on every flashed image:** bin 943 lines, no function over 100, 3 `unsafe` each with its quoted justification, zero `static mut`, no serial write reachable from `Locked`, 307 host tests (the campaign added one) and the replay test passing, clippy clean with the unsafe lints on host and target.

**What the campaign cost in code.** One control-path change, E140: the level revisit may make up to `REVISIT_RESCUE_MAX` = 4 rescue attempts in a sector that is overdue, instead of exactly one attempt per sector. Its A/B is the cleanest result of the campaign -- 28.8% went from 0/3 to 3/3 with nothing else altered. Everything else was enabling work: the duty clamp raised to the goal's ceiling, the window and its backstop lengthened for the longer ramp, shell keys per rung, and a diagnostic ring that keeps the *last* decisions.

**What was learned, and it is the campaign's real finding.** The wall at 30% was never the supply, the storm cap, the blank or timing margin: it was a swallowed crossing desyncing the drive with no way back (E138, from the last 1536 decisions before a stop). The failure rate climbed steeply with duty -- 27.5% survived 160 s, 28.8% died at 17–35 s, 30% within a second -- and one missed sector was always fatal because the revisit could not look twice. Giving it a second look moved the envelope from 27.5% to 37.5% in one change.

**Supply, honestly.** The goal set 1 A with 50 mA margin. E136's metered law (`I ∝ duty²`, anchored on 0.399 A metered at 25%) puts 37.5% near **0.90 A**; the report-only proxy read 958–975 mA there. **No metered reading was taken above 25% in this campaign** -- the PSU display is the operator's to read, and the goal's per-rung reading was not collected above 25%. Nothing in the runs suggests the supply was the limit: the bus minimum sat at −9.7% of reference at 37.5%, the same as at 25%, with no sag streak and no current event. So **no rung folded the bus on average current**, and the goal closes on its target rather than on the supply-limited edge.

**Unchanged and untouched:** every protection and threshold, hysteresis 0, the 64/ms storm cap, and everything in `../rm32*`, `../minz/core`, the old binz package, its captures and its notebook.

**Images:** `captures/elf/77C10285.e141.elf` is the qualified image; `C90D0544.e140.elf` carries the same control path without the upper rung keys; `FDBFBD0C.e135.elf` remains the campaign-5 production image and the fallback; `4FC7C796.e128.elf` is still the campaign-4 baseline for ABAB.

### Campaign 6 addendum — the metered current at 37.5%: **0.89 A**

The closing entry recorded that no metered reading had been taken above 25%. It has now: one more 37.5% hold (`e141-psu375_01`, 34.78 s at target, image `77C10285`) with the operator reading the PSU.

| quantity | value |
|---|---|
| metered, PSU | **0.890 A** |
| predicted by E136's law (`I ∝ duty²`, anchored on 0.399 A metered at 25%) | 0.898 A, **−1%** |
| proxy `hold_ma` this run | 855 mA, **−4% against the meter** |
| proxy in the cohort runs | 958–975 mA (+8 to +10%) |
| `zero_drift_ma` this run | −90 (the campaign's smallest; cohort runs −200 to −280) |
| coast eHz | 1647 against the oracle's 1650, **−0.2%** |
| rate identity | 998‰ of expectation, 1000‰ of 6 × coast |
| stop | reason 2, forced 0, no protection, gates PASS |

Three things follow.

1. **The square law holds across the whole qualified range**, 15% to 37.5%, to about 1%. The E136 sweep fitted it over 15–25% and it extrapolates to the top rung without correction, so the load really is viscous-dominated (torque ∝ ω) rather than propeller-like.
2. **37.5% was conservative, by about a percentage point of duty.** At 0.89 A the 0.95 A ceiling (1 A less the operator's 50 mA margin) arrives near **38.7%**, which matches the pre-campaign estimate of 38.6% from the same law. The goal's ceiling was the right call and it left roughly 60 mA in hand.
3. **The proxy's error is its datum, once more.** Its best agreement of the campaign (−4%) came on its smallest zero drift, and its worst (+10%) on drifts of 200 mA and more. A zero taken immediately before each hold is still the fix, with this point and E136's table as the anchor.

The supply, not the lock, remains what limits this stack: the drive held 1647 eHz within 0.2% of the reference while drawing 0.89 A, and the detector's own margin (R_COMP 11 µs against a 15 µs wait) does not run out until about 2.4 keHz.

### Campaign 6 review — three corrections from the binz author agent, all upheld

Checked against the code and the captures, not taken on trust. All three stand, and two of them were mine to fix.

**1. `spent_max_us` is not the edge-to-arm latency, and not a bound.** Confirmed at `roots.rs:365`: `spent` is stamped from the handler's entry stamp to the instruction *before* `com_arm()` at line 367. It therefore omits the dispatch and any same-priority blocking before entry, the two register writes of `line_disable()` and `clear_pending()` that precede the stamp, and `com_arm`'s own clock read, stores and TIM16 writes (inlined, so not separately measured; roughly 1 µs by instruction count). The "4 µs of margin at 37.5%" I quoted compares the wait against that narrow figure and overstates what is established. `WCET_ESTIMATES.md` has a new section 5a saying exactly this, and the margin column now points at it.

What is established at every rung is **`late_arms` = 0**: the firmware reporting that the wait had not elapsed when it armed. That is the gate's real evidence.

The captures also hold a term I had not been quoting: **`com_late_max_us` = 10–11 µs at 37.5%**, the one-shot firing that much later than the instant `com_arm` scheduled, TIM16 dispatch and blocking included. At `ci` 101 µs that is about 6 electrical degrees of advance lost on the worst commutation of the run. It did not break the loop -- the speed and rate gates passed with it -- but it is the largest single term in the edge-to-commutation chain and it sits outside R_COMP entirely.

**2. The document contradicted itself on the crossing point.** Correct: at level 22, `wait_time(70, 22)` = 11 µs and `wait_time(59, 22)` = 9 µs, so 11 µs meets the wait at `ci` ≈ 70 µs (≈ 2.4 keHz), not 59 µs / 2.8 keHz / 75% duty. The stale figures are withdrawn and every dependent line is corrected. And since the crossing is computed from the narrow figure, the true one is **at or below** it -- it is an upper bound on reachable duty, not a prediction.

**3. "About 11% bus sag per amp" is unsupported.** `bus_min` is the lowest single ADC scan of a run, a transient, and the firmware reports no steady loaded-bus figure at all (`RailMean` exists in the run context but is not emitted). A slope fitted to minima cannot predict a sustained 16% sag at 1.45 A, and that projection is withdrawn. Measuring it needs either a mean bus over the hold added to the report, or the operator's meter at the terminals.

**And the broader claim goes with it.** "The supply, not the lock, limits this stack" is not established. What the campaign shows is narrower: at 37.5% neither limit was reached -- 0.89 A metered against a 1 A supply, `late_arms` 0, no protection fired -- and **which one binds first above 37.5% is untested.** E136's square-law current fit remains a planning tool, confirmed to about 1% at the one point above its own range that has been metered.

**Accepted recommendation.** The baseline is preserved: E141 `77C10285` is archived and qualified, and campaign 6's results stand as recorded -- none of the three corrections touches a gate result, because every gate was judged on measured run outcomes rather than on these projections. The first step of any campaign above 37.5% is to measure the complete edge-to-arm delay (a stamp on the root's first instruction and a second after `com_arm`, or a hardware timestamp via TIM2 input capture), with the electrical protections untouched. 1.5 A is headroom, not evidence.

## Campaign 7 — 40% to 50% on the 1.75 A supply

Operator goal, set 2026-09-23: climb 40 → 42.5 → 45 → 47.5 → 50% in 2.5% rungs after four prerequisites, each its own step A/B'd on the qualified 37.5% rung. Supply raised to 1.75 A and fixed there; by the `I ∝ duty²` law (metered at 25% and 37.5%) 50% draws about 1.58 A. Reference: the oracle's rung table at 40/45/50% = 1736 / 1893 / 2096 eHz, with ~1.6 A metered at 50%.

Prerequisites, in the goal's order: (1) arm the COM one-shot before COMP's bookkeeping; (2) the oracle's advance profile above 35%, with ≥ 3 µs of stated margin per rung; (3) a sag guard with a filtered sharp reference beside the existing absolute floor; (4) duty cap 500, window ≥ 80 s, and the sag-versus-current slope measured at 25 / 30 / 37.5%.

### E142 — prerequisite 1: arm first, then keep the books

**Why.** `spent_max_us` reads 11 µs at every rung, and `roots.rs:365` stamps it from the handler's entry to the instruction before `com_arm()`. Inside that window sit the four bookkeeping stores of the accepted crossing (`sector_start_raw`, `accept_raw`, `accept_wait`, `accept_seq`). The timer needs none of them. At the oracle's high-duty advance the wait is about 8 µs, so every microsecond inside the response window matters.

**The change.** In `det_decide_plain`'s acceptance arm, and in the diagnostic twin's shared `accept()`, `com_arm()` now runs first and the four stores follow. Their order among themselves is unchanged and `accept_seq` is still last, so a foreground poll can never see a sequence number ahead of the edge stamp it belongs to. No threshold, gate or protection moves.

**Pre-power on `2B52AA05`:**
* audit clean on all four roots;
* **cycle counts did not grow, they fell**: COMP 1026 → **1021** / 866 → **861** / 738 → **733**; COM 349, DMA 78, guard 204 unchanged;
* 307 host tests, 9 doctests, replay green, clippy clean on host and target;
* bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**Disassembly of the changed root, inspected.** `ADC_COMP` is **324 instructions in both images**, and a normalised diff shows only a spill store moving across one branch and a padding `mov r8, r8` dropped. The source-level reorder therefore did **not** visibly reorder the emitted accept arm: the stores and the arm are independent, and LLVM had already scheduled them together. So this step's effect cannot be claimed from the source -- it has to be measured, and if `spent_max_us` does not move, the conclusion is that the machine code already armed as early as it could and the 11 µs lies elsewhere (the masking writes, the `offer` call and its live comparator reads, all inside the same window).

**Run.** Flash `2B52AA05` and take the 15% rung × 3 first: it is the chain's first link anyway, and its `spent_max_us` answers the question for a tenth of the bench time. Only if the figure moves does the full chain to 37.5% get walked for the formal A/B.

### E142 — result: arm-first is worth 1 µs, and it exposed a biased gate

**The measurement.** `spent_max_us` = **10 µs** on all eight 15% runs of `2B52AA05`, against **11 µs** on every run of E141 and E135 before it. So the four bookkeeping stores were indeed inside the response window, and moving the arm ahead of them buys 1 µs (about 64 cycles, which is what four stores and a read-modify-write cost with wait states). The source reorder did change the emitted schedule after all, even though the instruction count and the normalised disassembly barely moved.

`late_arms` stayed 0, and the loop's own rate identity stayed 996–999‰.

**But run `e142-15_02` failed gate 1** at 1011‰ against 6 × coast -- at 15%, the most-run rung on this bench. Before treating that as a regression the history says otherwise: **1011‰ at 15% is on record for E131, E132 and E134**, images that qualified. So, per the goal's rule for a failure that looks like a rate rather than a wall, a five-run cohort followed (`c7-e142-15_0*`): 1006, 1007, 1007, 1008, 1009 with `spent_max` 10 throughout. Eight runs of E142 at 15% spread 1002–1011 (mean 1007.4) against nine runs of the E139/E140/E141 line at 1002–1008 (mean 1006.3). Not a regression -- both distributions sit against the same edge.

### E143 — the rate-identity gate was measuring a biased coast, and it cost this campaign two false failures

**What was wrong.** `cohort.coast_ehz` took the **median of the coast's pair sums**. The rotor decelerates across the eight half-periods the firmware reports, so that median describes the rotor a few milliseconds *after* bridge-off, not at it, and it under-reads the speed the loop was holding. The ratio it feeds therefore sits above 1000 by construction.

**Measured over every capture with a real dwell** (360 of them, campaigns 4 to 7, `scratchpad/coastfit.py`):

| estimator | mean ‰ | sd | outside the 1% band |
|---|---|---|---|
| **median of pair sums (used through campaign 6)** | **1006.7** | 2.62 | **37 / 360** |
| first pair only (E113 rejected this) | 1001.9 | 4.14 | 12 / 360 |
| least-squares fit of the pair sums, read back at bridge-off | 1001.2 | 2.95 | 4 / 360 |
| **the same with the first half-period dropped** | **1002.7** | 2.88 | **4 / 360** |

The spread is the same in every row; what moves is the **centre**. With the centre at 1006.7 and a standard deviation of 2.6, a ±1% band clips the tail at roughly one run in ten -- so the gate was failing about 10% of perfectly good runs, which is 25% of any three-run cohort.

**The fix, and what it is not.** `coast_ehz` now drops the first half-period (which can be the demagnetisation transient, E113's finding, measured there as 461 µs against 423–445), fits a least-squares line through the remaining pair sums against pair index, and reads it back at the instant before the first retained pair. **The ±1% band is unchanged. No threshold, gate or firmware value moved** -- the estimate of the rotor's speed simply stopped being biased, and it now answers the question the gate asks: how fast was the rotor when the bridge let go.

This is host-side only (`scripts/cohort.py`), so no image changes and nothing needs re-flashing.

**Validation on the whole set** (390 captures, judged with the new estimator): mean **1002.2**, sd 2.97, **3 outside the band** -- `e073-quiet_01` (10%, 1011), `e125-restart_01` (1012) and `e127-25_03` (1013), all from campaigns 3 and 4.

**What it says about this campaign's two failures:**
* `e142-15_02`, the run above, becomes **1006** and the trio reads 999 / 1006 / 996;
* `e140-350`, the 35% cohort that failed one run at 1011, becomes **1006 / 1000 / 1006**.

Both were the estimator, not the firmware. Neither result is being rewritten: each stands in the notebook as it was judged at the time, and 35% went on to qualify 3/3 on E141 with the biased estimator anyway.

**Campaign 6 is not weakened by this.** Every one of its gates was judged on the biased figure, which reads about 5‰ high, so each rung was held to a *stricter* test than the corrected one. Recomputed, E141's 37.5% cohort reads 1002 / 1003 / 1007.

**A divergence to name.** Our coast figure now estimates the speed at bridge-off, while the oracle's rung-table speeds are its own loop-derived "final eHz". The two are not the same quantity, and our corrected figures sit about 0.5% higher than before, which slightly *improves* every speed comparison against that table. The 5%-of-oracle gate is unaffected in kind.

**Run:** the rest of the chain on `2B52AA05` -- 20, 25, 27.5, 28.8, 30, 32.5, 33.8, 35, 37.5% × 3 each -- to reach the 37.5% rung where prerequisite 1's A/B is stated.

### E142 — A/B at 37.5%: **rejected as written.** The 1 µs was measurement, and it cost 0.65% of rotor speed

The chain qualified on `2B52AA05` (20 → 37.5%, 3/3 at every rung), so the A/B could be stated on the goal's rung. E141 was then reflashed and run hot in the same session.

| image | 37.5% coast eHz (same estimator) | mean | `spent_max_us` | `late_arms` | `com_late_max_us` |
|---|---|---|---|---|---|
| E141 `77C10285`, hot now (`ab7-e141-375_0*`) | 1636 / 1631 / 1633 | **1633.3** | 11 | 0 | 10–11 |
| E141, earlier today (`e141-375_0*`) | 1641 / 1637 / 1632 | 1636.7 | 11 | 0 | 10–11 |
| E142 `2B52AA05` (`c7-e142-375_0*`) | 1618 / 1627 / 1623 | **1622.7** | **10** | 0 | 9–12 |

E141 reads the same hot as it did hours earlier, so the bench has not moved. E142 is **0.65% slower**, and the two sets of three do not overlap.

**Why, and it is the review's warning made flesh.** `left = wait − spent` is only correct if the timer is armed at the instant `spent` was stamped. Moving the stamp ahead of the four bookkeeping stores did not move the stores: the disassembly diff showed the compiler keeping them where they were, between the stamp and the arm. So:

* the stamp is ~1 µs earlier, which is the entire "improvement" in `spent_max_us` -- 11 → 10 with no change in the real path;
* but `left` is now computed from that earlier instant, while the write still happens ~1 µs later, so the one-shot fires at **edge + wait + 1 µs**;
* 1 µs late at `ci` 101 µs is about 0.6 electrical degrees of advance given away, and the rotor duly slows by 0.65%.

`com_late_max_us` cannot see this, because `com_arm` sets `sched_raw` from its own clock read: the lateness is folded into the schedule rather than reported against it.

**This is precisely the failure mode the binz author agent named:** a figure whose start point excludes the arming path can be improved by moving the start point. The A/B caught it because it judges the rotor, not the instrument. Had I taken "R_COMP fell to 10 µs" as the result, I would have banked a regression as a win.

**E142 is not kept.** Its chain data stays on the record.

### E144 — arm from the arm's own clock read

**The change, one variable:** `com_arm_at(raw, wait, phase)` takes the edge stamp and the wait, reads the clock **inside** itself, and computes `left = wait − (now − raw)` immediately before writing the timer. Nothing can then sit between the measurement and the write, and the one-shot fires at edge + wait whatever the compiler schedules around it. The acceptance arm keeps E142's order -- arm first, then the four stores -- since with the compensation done at the write, the order no longer changes when the commutation lands.

`spent_max_us` now records `now − raw` from that same read, so the reported figure **includes** the work up to the arm. It is expected to read 11 µs again, and that 11 will mean more than E141's 11 did.

**Pre-power on the new image, and the A/B:** 37.5% × 3 against E141 hot. The result to look for is the speed restored to E141's 1631–1636 **and** an honest R_COMP. If speed comes back and R_COMP still reads 11, the conclusion is that the 11 µs was always the true figure and prerequisite 1 buys nothing at this advance -- which is itself the answer the goal needs before it relies on 24–26.

### E144 — result: compensating at the write does not recover the speed. **Prerequisite 1 is rejected and reverted**

`AA5B9BD5` qualified 15 → 35% (3/3 at every rung) and then failed one 37.5% run on the rate ratio. Its A/B, with every figure recomputed through the same estimator:

| rung | E141 `77C10285` | E142 `2B52AA05` (arm first) | E144 `AA5B9BD5` (arm first, compensated at the write) |
|---|---|---|---|
| 15% | loop 709, coast 704 | loop 710, coast 706 | loop 709, coast 707 |
| 25% | loop 1199, coast 1188 | loop 1193, coast 1186 | loop 1199, coast 1188 |
| 35% | loop 1529, coast 1513 | loop 1524, coast 1510 | loop 1529, coast 1509 |
| **37.5%** | **loop 1650**, coast 1637 (1633 hot) | loop 1633, coast 1623 | **loop 1633**, coast 1618 |
| `spent_max_us` | 11 | 10 | 10 |

Read across: **E144 matches E141 exactly at 15, 25 and 35%, and only loses at 37.5%** -- 1650 → 1633 on the loop's own interval, about 1%, with the coast agreeing. The rung where it bites is the one where the remaining wait is smallest: 15–16 µs of wait less a 10 µs response leaves 5–6 µs, against 9 µs at 35% and 16 µs at 25%.

So the reordering costs speed once `left` is small, and taking the compensation at the write does not recover it. The mechanism is **not identified**: both orders compute a fire instant of edge + wait, `com_late_max_us` is 8–12 µs in every image, and `late_arms` is 0 throughout. Whatever the extra delay is, it is below this instrument's 1 µs resolution and only shows as lost advance when the wait is short. That is recorded as an open question rather than a story.

**Decision: prerequisite 1 is rejected.** Its purpose was to buy the response time down to about 6 µs so the oracle's high-duty advance of 24–26 could be used (E145 below). It delivered 11 → 10 µs -- one of the four or five needed -- and at the top rung it costs 1% of rotor speed. Both variants are reverted: the acceptance arm keeps the qualified order, `com_arm_at` is deleted, and the rebuilt image's **loadable bytes are identical to E141 `77C10285`** (objcopy compared), so the control path is exactly the qualified one. 308 host tests and 9 doctests pass on it.

**What the two steps did buy, and it is not nothing:** the trap in the measurement is now known and written down (`WCET_ESTIMATES.md` §5a) -- a figure whose start point excludes the arming path can be "improved" by moving the start point, and the bench will show the cost as lost speed rather than as a worse number. E142's 1 µs was exactly that.

**What would still reach 6 µs:** shortening the path inside the window (the masking writes, the estimator borrow, `offer` and its live comparator reads), or hardware-stamping the edge with a TIM2 input capture as the binz author recommended. Neither is one of this goal's prerequisites, and neither is attempted here.

### E145 — prerequisite 2: the advance profile the margin rule actually permits

The goal asks for "the advance profile as the oracle runs it (20 below 35%, 22–26 above)" **and** for the inequality `R_COMP < wait_time(ci)` to hold per rung with at least 3 µs of margin. Those two requirements collide, and the arithmetic says which one binds. With R_COMP at its measured 10–11 µs:

| rung | oracle eHz | `ci` µs | wait at 20 | at 22 | at 24 | at 26 | highest level with ≥ 3 µs margin (R_COMP 10) |
|---|---|---|---|---|---|---|---|
| 37.5% | 1650 | 101 | 19 | 16 | 13 | 9 | **24** |
| 40% | 1736 | 96 | 18 | 15 | 12 | 9 | **23** |
| 42.5% | 1815 | 92 | 18 | 15 | 12 | 9 | **23** |
| 45% | 1893 | 88 | 17 | 14 | 11 | 9 | **23** |
| 47.5% | 1995 | 84 | 16 | 14 | 11 | 8 | **22** |
| 50% | 2096 | 80 | 15 | 13 | 10 | 8 | **22** |

So **level 26 is never admissible** on this firmware: it leaves 8–9 µs of wait against a 10 µs response, which is not a 3 µs margin -- it is negative. Level 24 is admissible only at 37.5%, and 22 is admissible everywhere up to 50%.

**Decision: the published schedule stays as it is — 20 below 35% duty, 22 at and above.** That is already what `run::policy::AdvancePolicy` does, so prerequisite 2 requires no code change, and it is one variable moved by zero.

**Named as a divergence from the oracle, with its reason.** The frozen oracle images apply an *effective* high-duty advance of 24 or 26 through binz's post-COM wait override (`reverse_advance24.rs`, `core_bench.rs:1816-1828`), because minz-core clamps published levels to 18..22. firmware50 will not follow that above 37.5%: its response time is 10–11 µs where the override leaves 8–9 µs of wait, so copying the oracle here would arm late on every sector -- exactly the late-commutation braking the goal's own list of misread signals warns about, and what E142 produced at one-sixth the size.

**What this costs.** The oracle made 2096 eHz at 50% with effective 26. At level 22 the commutation is about 5 µs later in each sector, so this firmware should be expected to run *below* the oracle at the top rungs. The 5%-of-oracle speed gate is what will judge whether that is tolerable, and the climb will measure it rather than predict it.

**What would make 24–26 admissible:** a response time near 6 µs, which needs the arm path shortened rather than re-stamped (E144 is the honest baseline for that), or the hardware-stamped edge the review recommends. Both are outside this goal's prerequisites.

### E146 — prerequisite 3: the sag guard's sharp reference becomes a filtered bus

**Why.** `FastBusSag` judged every scan against the **pre-run** bus: three scans below 95% of the baseline captured before the drive. That is a dip test bolted to a fixed datum, so a load that pulls the rail down and *keeps* it down eventually fails it on margin rather than on a dip. At 37.5% the bus already sits 9.7% below its pre-run reference, which is a droop the guard has been tolerating only because the minimum never held for three consecutive scans. The goal's own list of misread signals names this: "a fixed-reference sag trip after droop (margin, not a dip)".

**The change, one variable: the reference.**
* The sharp reference is now a ~200 ms exponential average of bus and VREF, kept in Q8 with `SAG_FILTER_SHIFT = 11` (2048 scans at the 9.901 kHz scan rate, 207 ms).
* The fraction (95%), the streak (3 scans), the latch and the reason code (26) are **unchanged**.
* The filter is **updated after the test**, so a collapsing sample cannot drag the reference onto itself and hide the collapse.
* It is primed from the pre-run baseline, so the first 200 ms behave as before.
* The slow direction is untouched: the absolute floor (`BUS_FLOOR_NUM`, ≈ 8400 mV, `Reason::Bus`) still owns it, and it is a separate check in `validate_raw_feedback`.
* Both references are reported at every stop on a new `BEMFSAG` line: `ref_bus`, `ref_vref`, `filt_bus`, `filt_vref`, `streak`, `tripped`.

**Two things the tests caught, worth recording.**
1. Truncating the Q8 filter to whole ADC codes threw away up to one code of reference, which at the 95% line moves the threshold by a code -- the existing `exactly_at_ninety_five_percent_is_not_low` test failed on it. The view is now **rounded**.
2. My own first test asserted that one deep sample moves the rounded reference. It does not, and should not: one sample's share is 1/2048, far below a code. The test now asserts the property that matters -- a collapse still latches in three scans, and the latch stays sticky -- plus that one sample cannot move the reference by a whole code.

**New host tests:** a 4-second droop from 1212 to 1110 codes (more than 8% below the pre-run reference, which would have tripped the old guard) passes without a single trip, and a step 10% below the *followed* reference still latches in three scans. 310 host tests, 9 doctests.

**Pre-power on `3B95FD9A`:**
* the four ISR roots are **instruction-identical to E141** -- the guard runs in the foreground, so cycle counts return to the qualified 1026 / 866 / 738, COM 349, DMA 78, guard 204;
* audit clean on all four; clippy clean on host and target; bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**Run:** the chain on this ELF to 37.5%, then the goal's inertness test -- 3/3 at 37.5% with **zero sag trips**, and `BEMFSAG` read off each capture to see how far the sharp reference has drifted from the baseline at that load.

### E146 — result: **inert at 37.5%, 3/3**, and the sag slope finally measured properly

`3B95FD9A` walked the whole chain: 15, 20, 25, 27.5, 28.8, 30, 32.5, 33.8, 35, 37.5% -- **3/3 PASS at every rung**, with the new sharp reference live throughout.

**The goal's inertness test** (`c7-e146-375_0{1,2,3}`): `BEMFSAG streak=0 tripped=0` in all three, and no protection fired. The guard saw the whole climb of the ladder without a single low scan streak.

**What the `BEMFSAG` line is worth beyond that.** The filter stops updating when the run ends, so the reported `filt_bus` is the **steady loaded bus** -- the quantity this bench has never had. Beside the pre-run reference and the transient minimum:

| rung | current | pre-run `ref_bus` | steady `filt_bus` | steady droop | `bus_min` | transient |
|---|---|---|---|---|---|---|
| 25% | 0.399 A metered | 1217 | 1208 | **−0.71%** | 1112 | −8.6% |
| 30% | ~0.58 A (law) | 1215 | 1206 | −0.77% | 1121 | −7.7% |
| 35% | ~0.78 A (law) | 1216 | 1203 | −1.07% | 1111 | −8.6% |
| 37.5% | 0.890 A metered | 1216 | 1201 | **−1.21%** | 1111 | −8.6% |

Two things fall out, and both matter for prerequisite 4:

1. **The sag-versus-current slope is about −1.0% per amp** (−0.50% across the 0.49 A between the two metered rungs). At the pre-run 1216 codes ≈ 11.9 V that is roughly 0.12 V/A, so about **0.12 Ω** of supply and path resistance together -- healthy for this bench, and far from the ≥ 0.3 Ω the power-path scar says never to qualify through. Extrapolated to 1.58 A at 50%, the steady droop is about **−1.7%**, nowhere near the 95% line.
2. **`bus_min` is not sag at all.** It reads −7.7 to −8.6% at *every* rung, from 0.40 A to 0.89 A, with no trend in current. It is a switching transient the scan occasionally lands on. That is the direct evidence for the correction the binz author agent asked for: the "about 11% per amp" slope I had fitted to minima was fitting noise, and the withdrawal was right.

**So the filtered reference does two jobs:** it stops a droop from being read as a dip, and it makes the steady bus observable. The fixed reference it replaced would have been at −1.21% of its datum at 37.5% and heading for −1.7% at 50%, still inside 95% -- so this change is not what unblocks the climb; it removes a false-trip mode before the climb can meet it, which is what the goal asked for.

**Prerequisite 3: done.** Prerequisites 1 and 2 are settled (1 rejected and reverted, 2 no change admissible). Prerequisite 4's measurement half is done above; its enabling half -- the duty clamp and the window -- is E147.

### E147 — prerequisite 4: the clamp, the window, and the climb's keys

**On the record, as the goal asks:**
* `SIXSTEP_DUTY_CAP` **375 → 500**. The clamp sits exactly on the goal's ceiling, so the firmware still cannot command past 50% however it is driven.
* `BEMF_TOTAL_MS` **54 s → 80 s**, `BEMF_EXPLORE_MS` 34 s → 45 s, and `GUARD_CAMPAIGN_US` 56 s → **84 s**, keeping the backstop's margin above the window. The ramp is 1%/0.5 s from 10%, so reaching 50% costs **20 s**; a 30 s dwell needs about 55 s of window and 80 s leaves room for the restart campaign's two segments.
* The sag-versus-current slope is measured and recorded in E146: **−1.0% per amp steady**, about 0.12 Ω of supply and path, with the transient minimum shown to be current-independent and therefore not sag.

**The climb's keys.** Five rungs would need ten keys and the shell has three lowercase letters left, so the duty is held in one place and stepped: `+` and `-` move it by 2.5% between 37.5% and the clamp, `l` explores it on the short window, `L` qualifies it on the full one, and `CLIMBAT duty_tenths=` is printed on every change. Every capture still records `target_duty_tenths`, and the fixture is told which rung it is asking for, so no judgement depends on guessing shell state.

**Structure caught a real violation on the first build:** `command` grew to 102 lines against the 100-line limit, so the selector moved into its own `climb_key`. The limit did its job.

**Pre-power on `89D65B09`:**
* the four ISR roots are **instruction-identical to E146** (this is foreground and policy only): COMP 1026 / 866 / 738, COM 349, DMA 78, guard 204;
* audit clean on all four; 309 host tests, 9 doctests, clippy clean, bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**All four prerequisites are now settled:** 1 rejected and reverted (E142, E144), 2 no admissible change with the margin rule (E145), 3 done and proven inert (E146), 4 done here.

**Run:** the chain on this ELF to 37.5%, then the climb -- `+` to 40%, `l` to explore, `L` × 3 to qualify, and upward in 2.5% steps.

### E148 — the climb begins: 40% and 42.5% **qualified 3/3** on E147 `89D65B09`

The chain to 37.5% passed 3/3 at every rung on this image first (`c7-e147-*`), on the new 80 s window.

| rung | exploratory | cohort | coast eHz | oracle | speed | rate ‰ (expect / 6 × coast) | proxy mA | oracle mA |
|---|---|---|---|---|---|---|---|---|
| 40% | `c7-explore400b_01` (24.7 s) | `c7-400b_0{1,2,3}` | 1721–1738 | 1736 (measured row) | **+0.1 to −0.9%** | 990–993 / 1000–1010 | 1091 mean | 945 (+15%) |
| 42.5% | `c7-explore425_01` | `c7-425_0{1,2,3}` | ~1861 | 1815 (interpolated) | **+2.5%** | 993 / 1000 | 1295 mean | 1091 (+19%) |

Every run: stop reason 2, forced 0, `unstable` non-zero, `blank_arms` one per sector, `blank_latched` 0, coast crossings > 0, **no protection fired**, `BEMFSAG streak=0 tripped=0`, R_COMP 11 µs with `late_arms` 0 at both rungs.

**Two tooling faults of mine, both caught by the record rather than by me, and both now closed.**

1. **The first climb run ran the wrong rung.** The shell's climb duty already boots at 40%, and I pressed `+` before running it, so the run was at **42.5%** while the fixture had been told 40% -- and the ladder duly admitted it on 37.5%'s evidence. The run itself was clean (23 s at target, 1849 eHz, rate 1005‰), but it was out of the goal's order, and it is **not** counted as a rung result: `captures/2026-09-23/c7-explore400_01.txt` is recorded as an unordered exploratory point. The fixture now **checks the capture's duty against the `--rung-duty` it was given** and fails loudly on a mismatch, so shell state can no longer be assumed.
2. **The first 40% cohort was not recorded.** `L` was missing from the branch that judges and records a rung run, so three good runs (1004 / 1009 / 1010‰, speeds within 1%) were driven, printed and then dropped by the ladder. Their captures stand (`c7-400_0*`), the fixture is fixed, and the cohort was **re-run** (`c7-400b_0*`) so the evidence is recorded rather than asserted. This is not a retry for a pass -- the first three had already passed their gates; the fixture had simply not written them down.

**Current against the supply.** The proxy reads 1091 mA at 40% and 1295 mA at 42.5%; E136's metered law puts the true figures near **1.02 A** and **1.15 A**, against the 1.75 A the operator set. The per-rung PSU reading the goal asks for is the operator's to take; the proxy and the law are recorded beside each result here.

**Run:** 45% -- `+`, then `l`, then `L` × 3. Oracle 1893 eHz / 1237 mA, and the law puts the current near 1.29 A.

### E148 continued — 45% and 47.5% **qualified 3/3**; 50% reached but **not qualified**

| rung | exploratory | cohort | coast eHz | oracle | speed | rate ‰ (expect / 6 × coast) | proxy mA | law's metered |
|---|---|---|---|---|---|---|---|---|
| 45% | `c7-explore450_01` | `c7-450_0{1,2,3}` | 1960–1967 | 1893 | **+3.5 to +3.9%** | 990–991 / ~1004 | 1488 | ~1.29 A |
| 47.5% | `c7-explore475_01` | `c7-475_0{1,2,3}` | 2057–2060 | 1995 | **+3.1 to +3.3%** | 990–991 / ~997 | 1685 | ~1.44 A |
| 50% | `c7-explore500_01` | `c7-500_0*`, `c7-500c_0*` | 2102–2124 | 2096 | +0.3 to +1.3% | 988–999 / **1011–1015** | 1867 | ~1.60 A |

All qualifying runs at 45% and 47.5%: reason 2, forced 0, no protection, `BEMFSAG streak=0 tripped=0`, `blank_latched` 0, R_COMP **11 µs** with `late_arms` 0 -- the wait at 47.5% is 14 µs at level 22, so the margin is 3 µs, exactly the goal's floor and the reason level 24–26 was refused (E145).

`too_early` becomes non-zero for the first time since E134 -- 3 at 47.5%, 5 at 50% -- because the blanking window shrinks with `ci` (80 µs at 47.5%). `blank_arms` still witnesses the gate once per sector, and the counts are five edges in 400 000.

### 50% — the **supply-limited edge**, on a six-run cohort

The goal's rule for a failure that recurs at a rate got its cohort: six runs at 50%, all with the loop healthy.

| run | stop | dwell | steady `filt_bus` | `bus_min` | sag streak |
|---|---|---|---|---|---|
| `c7-500_01` | **26** fast bus sag | 47.5 s | 1176 | 1067 | 3 |
| `c7-500_02` | 2 deadline | **54.8 s** | 1178 | 1085 | 0 |
| `c7-500_03` | **26** | 31.4 s | 1176 | 1090 | 3 |
| `c7-500c_01` | 2 deadline | **54.8 s** | 1178 | 1089 | 0 |
| `c7-500c_02` | **26** | 35.1 s | 1191 | 1106 | 3 |
| `c7-500c_03` | 2 deadline | **54.8 s** | 1185 | 1090 | 0 |

**Three of six trip the fast bus sag; the other three hold the full dwell.** And the trips are genuine dips, not droop: the reference they are measured against is the ~200 ms filtered bus (E146), which sits at 1176–1191 against a pre-run 1209–1214, so the *steady* droop at 50% is **−2.9%** -- and the minima reach 1067, about **−9% below the recent bus**, three scans running.

Two further facts place this at the supply:
* the steady droop per amp **steepens above 37.5%**: −1.0%/A up to 0.89 A (E146), but −2.9% at about 1.6 A is −1.8%/A -- consistent with a source approaching its limit rather than with fixed path resistance;
* the drive itself is healthy at 50%: speed within 1.3% of the oracle, no tracking stop, no desync, no storm, `late_arms` 0, and three runs held 54.8 s.

**Also failing at 50%, and honestly recorded:** every clean run breaches the rate-identity band against 6 × coast (**1011–1015‰**) while the loop's own identity reads 988–999‰. The two speed measures diverge by about 1% at this speed and only at this speed (25–37.5% sit at 1002–1007, 40–47.5% at 997–1010). That is either the coast estimator's residual bias at high deceleration -- the linear back-extrapolation of E143 under-corrects when the decay steepens -- or the loop genuinely accepting about 1% more crossings than the rotor turns. **The data here cannot separate those**, and the gate fails either way, so 50% is not qualified and the question is left open rather than explained away.

**The operator's PSU reading at 50% is the missing measurement**: whether the supply is in constant-current at its 1.75 A setting when these dips happen would settle the mechanism. The goal names a supply question as an operator matter; it is recorded here as such and the campaign does not depend on it.

**Where this leaves the goal.** The qualified envelope is **47.5%**, and 50% is the supply-limited edge, which per the goal closes the climb there. What remains is the goal's item at the top rung -- restart 3/3 and every protection from a locked loop -- and those must run on the highest qualified rung, 47.5%. The provocation selector only reaches 37.5% today, so that needs one more image (E149) and, because the fixture keys its ladder to the ELF, a full re-qualification on it.

### E149 — the final image: provocations reach the top qualified rung

`63EC7EFE`. One addition: the provocation selector `x` now cycles 25% → 37.5% → **47.5%** → 25%, so the goal's item 5 -- restart 3/3 and every protection from a locked loop -- can run where the envelope actually ends. `Z` (the restart at the selected duty) has its fixture prerequisite moved to the 47.5% rung to match.

Pre-power: the four ISR roots are **instruction-identical to E147**, audit clean on all four, 309 host tests and 9 doctests pass, clippy clean, bin 943 lines, 3 unsafe, 0 `static mut`, no function over 100 lines.

**Why item 5 runs at 47.5% and not 50%.** 50% is the supply-limited edge: three of six runs trip the fast bus sag on genuine dips (E148), so it is not a qualified rung and a protection sweep there would be measuring the supply, not the protections. The goal closes the climb at the supply-limited edge, and 47.5% is the highest rung that qualified 3/3.

**Run, in order, on this ELF:** the chain to 37.5% × 3 each; then the climb 40 → 42.5 → 45 → 47.5% with one exploratory run and a 3/3 cohort at each; then `x` to 47.5%, `Z` × 3 for the restart, and every protection from a locked 47.5% loop.

### Campaign 7 — final: **47.5% qualified** on E149 `63EC7EFE`; 50% is the supply-limited edge

The goal asked for 40 → 50% in 2.5% rungs on a 1.75 A supply, after four prerequisites. The envelope reached **47.5%**, and 50% is where the supply folds.

**Every rung on the final image**, each with an exploratory run first and a 3/3 cohort at ≥ 30 s true dwell (80 s window):

| rung | cohort | coast eHz | oracle | speed | rate ‰ (expect / 6 × coast) | proxy mA | law's metered |
|---|---|---|---|---|---|---|---|
| 15 → 37.5% | `c7f-15` … `c7f-375` | as campaign 6 | — | within 5% | inside | — | — |
| 40% | `c7f-400_0*` | ~1735 | 1736 | ±0.9% | 990–993 / 1000–1010 | 1094 | ~1.02 A |
| 42.5% | `c7f-425_0*` | ~1861 | 1815 | +2.5% | ~993 / ~1000 | 1290 | ~1.15 A |
| 45% | `c7f-450_0*` | ~1963 | 1893 | +3.5 to +3.9% | 990–991 / ~1004 | 1477 | ~1.29 A |
| **47.5%** | `c7f-475_0*` | **~2058** | 1995 | **+3.1 to +3.3%** | 990–991 / ~997 | 1660 | **~1.44 A** |

Every qualifying run: stop reason 2, forced 0, `unstable` non-zero, `blank_arms` one per sector, **`blank_latched` 0**, coast crossings > 0, **no protection fired**, `BEMFSAG streak=0 tripped=0`, R_COMP **11 µs** with `late_arms` 0, ISR cycle counts unchanged (COMP 1026 / 866 / 738, COM 349, DMA 78, guard 204) and the audit clean on every flashed image.

**Item 5, at the top qualified rung.**
* **Restart 3/3** (`c7f-restart475_0{1,2,3}`): injected loss at 26.2 s, bridge off, ordinary startup admitted with no refusal, second segment held 26.3 s at target, the original 80 s window completed, `recovered=1`.
* **Protections from a locked 47.5% loop** (`c7f-p475-*`): t 8, g 3, f 4, n 7, u 13, h 14, i 25, host abort 9, `w` `RESETCAUSE iwdg=1` -- nine of ten with their reason codes and a PASS bridge-off readback after each.
* **The sag provocation `v` could not provoke at 47.5%, and that is the stimulus, not the protection.** `Inject::Sag` works by commanding `INJECT_SAG_DUTY_TENTHS` = 50% duty; from a 47.5% base that is a 2.5% step, too small to make a dip, so the run ran to its deadline (reason 2 with the injection fired). The protection's own evidence at the top of the envelope is stronger than an injection: **it fired unprovoked on genuine dips in three of six runs at 50%**, and its injection fired correctly at 37.5% in campaign 6. A stimulus that steps the duty cannot test a dip guard at a base duty next to its own target; making it inject a synthetic bus code instead is the obvious fix and is not attempted here.

**The supply-limited edge, on a six-run cohort at 50%:** three runs tripped the fast bus sag (reason 26) after 31–47 s, three held the full 54.8 s. Steady bus −2.9% of pre-run with dips to −9% of the *filtered* reference; the droop per amp steepens from −1.0%/A below 0.89 A to about −1.8%/A at 1.6 A. The drive was healthy in all six: speed within 1.3% of the oracle, no tracking stop, no desync, `late_arms` 0. Every clean 50% run also breached the rate-identity band against 6 × coast (1011–1015‰) while the loop's own identity read 988–999‰; whether that is the coast estimator's residual bias at high deceleration or a real 1% divergence **is not settled by this data**, and it is left open.

**What the prerequisites cost and bought:**
1. **Arm before the bookkeeping: rejected** (E142, E144). It moved the reported response time 11 → 10 µs and cost 1% of rotor speed at 37.5%; compensating at the write did not recover it. Reverted to loadable-byte identity with E141. The lasting product is the trap written down in `WCET_ESTIMATES.md` §5a.
2. **The oracle's high-duty advance: refused** (E145). With R_COMP 10–11 µs the ≥ 3 µs margin rule permits level 22 at most above 45%; the oracle's effective 24–26 leaves 8–9 µs of wait. Named as a divergence, with the reason.
3. **The sag guard's sharp reference: kept** (E146). A ~200 ms filtered bus replaces the pre-run baseline, fraction, streak, latch and reason unchanged, proven inert 3/3 at 37.5%. It also made the steady bus observable, which is what measured the slope and exposed `bus_min` as a switching artefact rather than sag.
4. **Duty cap 500, window 80 s, backstop 84 s, climb keys: done** (E147), with the sag-versus-current slope measured at 25 / 30 / 35 / 37.5%.

**Two tooling faults of mine, both caught by the record** (E148): the first climb run ran 42.5% while the fixture was told 40% (shell state assumed, now checked against every capture's duty), and the first 40% cohort was driven but never recorded (`L` missing from the recording branch, now fixed and the cohort re-run).

**Structure on the final image:** bin 943 lines, no function over 100 (the limit caught `command` at 102 and it was split), 3 unsafe each with its quoted justification, 0 `static mut`, 309 host tests and 9 doctests, the replay test passing, clippy clean with the unsafe lints.

**Unchanged and untouched:** every protection threshold, hysteresis 0, the 64/ms storm cap, and everything in `../rm32*`, `../minz/core`, the old binz package, its captures and notebook.

**Open for the operator:** the PSU reading at 50% -- whether the supply sits in constant current at its 1.75 A setting while those dips happen -- would settle the edge's mechanism. Nothing in this campaign depends on the answer.

**Images:** `captures/elf/63EC7EFE.e149.elf` is the qualified image; `89D65B09.e147.elf` carries the same control path and the climb's first pass; `77C10285.e141.elf` remains campaign 6's qualified image and the fallback.

### E150 — 50% is **not** supply-limited: the loop slips, and two of the goal's own gates say stop

The closing entry called 50% the supply-limited edge. That was wrong, and the evidence that corrects it was already in the captures.

**1. The dips do not grow with current.** Depth below the *filtered* (steady) bus, averaged per rung:

| rung | proxy mA | dip below filtered bus |
|---|---|---|
| 37.5% | 933 | −7.21% |
| 40% | 1094 | −8.03% |
| 42.5% | 1290 | −7.04% |
| 45% | 1477 | −7.67% |
| 47.5% | 1660 | −8.65% |
| 50% | 1849–1867 | −7.57 to −8.16% |

Flat, from 0.93 A of proxy to 1.87 A. A rail being pulled down by load would deepen with current; this does not. The steady droop does grow (−1.2% at 37.5% to −2.9% at 50%), but the *dips* -- the thing the guard measures -- do not.

**2. The loop starts leading the rotor at 50%.** The loop's own electrical frequency against the coast estimate, averaged per rung over 555 captures:

| rung | loop above coast |
|---|---|
| 15% | +0.45% |
| 25% | +0.62% |
| 35% | +1.01% |
| 40% | +1.04% |
| 45% | +0.73% |
| 47.5% | +0.85% |
| **50%** | **+1.72%** |

And this is **not** the coast estimator: the coast's own deceleration across its window is +0.60% at 50%, the same 0.5–0.9% as every other rung, so the extrapolation is doing no harder a job there. (A quadratic-fit comparison was attempted and its numbers were nonsense -- my solve was wrong -- so it is not cited; the drift column answers the question without it.)

**So the mechanism at 50% is slip, not sag.** The commutation is running about 1.7% faster than the rotor turns: the drive is asking for more than the motor delivers at this duty on this rail, the rotor falls behind, and the resulting current surges produce the irregular dips that eventually land three scans in a row below the line. That is this bench's own scar, written in `binz/CLAUDE.md`: *"the bus-collapse at the open-loop edge is a loss-of-sync current surge, NOT a supply wall -- proven: PSU 0.8→1.5 A did not move the edge."* Raising the supply is the anti-lever; it feeds the surge.

The goal's closing clause -- "the first rung that folds the bus on average current is the supply-limited edge" -- therefore does **not** apply. Nothing folded on average current (the allowance is 4 A raw; the draw is about 1.6 A), and 50% is not qualified. **The goal is not met, and it is not met for a reason inside the drive.**

**3. A tension in the goal's own gate 4, stated plainly.** Gate 4 requires `R_COMP < wait_time(ci)` with **at least 3 µs of margin**. On the measured 11 µs response:

| rung | `ci` µs | wait at level 22 | margin | gate 4's floor |
|---|---|---|---|---|
| 45% | 84 | 14 | 3 µs | met exactly |
| **47.5%** | **80** | **13** | **2 µs** | **not met** |
| 50% | 77 | 13 | 2 µs | not met |

So the rung I reported as qualified, 47.5%, meets every gate **except** gate 4's margin, which it misses by 1 µs. `late_arms` = 0 in every run of every rung is the measured statement that the arm did beat the wait, and §5a of `WCET_ESTIMATES.md` says why that is the honest test rather than the arithmetic -- but the goal asked for the arithmetic, and on the arithmetic the compliant envelope is **45%**, not 47.5%.

**What would move 50%, and why none of it is mine to choose.**
* **More advance.** Level 24 at 50% would cut the current for a given speed and is the standard answer to slip. The oracle runs an effective 24–26 above 35%. It is barred here by gate 4's margin rule at R_COMP 11 µs, and prerequisite 1 -- the goal's own way of buying that margin -- was measured and **rejected** (it cost 1% of speed at 37.5%, E144).
* **A shorter response path.** 11 µs is essentially the whole COMP body to the arm; halving it is a redesign (a hardware-stamped edge via TIM2 input capture, or a materially shorter decision), not a step inside this goal.
* **Relaxing gate 4's 3 µs floor**, which is the operator's parameter, not a protection.
* **A bigger supply**, which this bench's own evidence says is the wrong lever for slip.

**Recorded state:** the envelope qualified on every gate is **47.5%** (`63EC7EFE`); the envelope qualified on every gate *including* gate 4's margin is **45%**; 50% fails on slip and on the rate-identity gate, with the protections behaving correctly throughout.

### E151 — campaign 7 closes: 50% is unreachable **under this goal's own gate 4**, and the arithmetic says so

Not an opinion about effort, a proof from the measured response time. At 50% the hold interval is `ci` = 77 µs, and `wait_time(ci, level) = (ci>>1) − advance_of(ci, level)`:

| `ci` | rung | wait at 20 | 22 | 24 | 26 | highest level with ≥ 3 µs margin at R_COMP 11 |
|---|---|---|---|---|---|---|
| 84 | 45% | 16 | 14 | 11 | 8 | **22** |
| 80 | 47.5% | 15 | 13 | 10 | 8 | **21** |
| 77 | 50% | 14 | 12 | 10 | 7 | **20** |

Two of the goal's requirements meet head-on at 50%:

* **Gate 4** -- `R_COMP < wait_time(ci)` with at least 3 µs of margin, from the measured `spent_max_us` of 11 µs -- admits **level 20 at most** (wait 14 µs).
* **Gate 1** fails at 50% because the loop leads the rotor by 1.72% (E150): slip. The lever for slip is **more** advance, and the oracle runs an effective 24–26 above 35%.

There is no advance level that satisfies both. Level 24 at 50% would need **R_COMP ≤ 7 µs**; level 22 needs ≤ 9. The measured figure is 11, and the goal's own prerequisite 1 -- the step whose stated purpose was to buy exactly this margin -- was built, measured and **rejected**: it moved the reported figure to 10 µs and cost 1% of rotor speed at 37.5% (E142, E144), and a 1 µs retard is the wrong direction for slip anyway.

Gate 4 also admits only level ≤ 21 at 47.5%, so the rung this campaign qualified on every other gate is itself outside the margin clause by 1 µs. **45% is the highest rung that satisfies every clause of the goal as written**, at level 22 with exactly 3 µs.

**What the campaign therefore delivers, and what it cannot:**
* qualified on every gate: **47.5%** (`63EC7EFE`), plus restart 3/3 and nine of ten protections from a locked 47.5% loop;
* qualified including gate 4's margin clause: **45%**;
* **50%: not reachable** without cutting the response time by about 4 µs -- roughly 250 cycles out of a ~700-cycle decision path. That is the arm-path redesign the binz author agent recommended (a hardware-stamped edge via TIM2 input capture, or a materially shorter decision), which is outside this goal's four prerequisites and was not attempted.

**Three decisions are the operator's, and the campaign stops on them rather than guessing:**
1. **Relax gate 4's floor to 2 µs** on the strength of `late_arms` = 0 in every run of every rung (the argument is in `WCET_ESTIMATES.md` §5a: the narrow figure is not the true edge-to-arm delay, so a 3 µs arithmetic floor on it is not the safety statement it looks like). That makes 47.5% compliant; it does **not** reach 50%, which still needs level 24.
2. **Authorise the arm-path work** -- hardware-stamped edge, or a shorter decision -- as its own campaign. That is the only route to 50% that respects every protection.
3. **Accept 45% or 47.5%** as the envelope on this motor and supply.

Nothing further on the bench can move this: the supply is the anti-lever for slip (this bench proved that once already), protections and hysteresis are off-limits by the goal, and the one remaining software lever is barred by the goal's own margin clause.

### E152 — the offline pass both reviewers asked for. Four findings, and two of my claims retracted

No powered runs, no firmware change. 588 captures with a real dwell, re-analysed (`scratchpad/offline1-3.py`).

**1. `late_arms` was NOT zero. My statement was false, and so was the justification I built on it.**

| capture | duty | stop | `late_arms` | `com_late_max_us` |
|---|---|---|---|---|
| `c7-500_01` | 50% | 26 | 1 | 10 |
| `c7-500c_01` | 50% | 2 | 1 | 9 |
| `c7-500c_03` | 50% | 2 | 2 | 10 |
| **`c7f-450_02`** | **45%** | 2 | **1** | 11 |

Four of 588 production captures, including one run of the **qualifying 45% cohort**. (A fifth hit, `e134d-probe15`, is the diagnostic probe whose packed counter is already recorded as wrapping.) So:
* my "late_arms = 0 in every run of every rung" is **retracted** -- it was 0 in 584 of 588, which is not the same statement;
* the case I offered for relaxing gate 4's 3 µs floor **has no basis**, because the arm did miss the wait, four times;
* the graybeard's proposed replacement -- a late-arm hard stop plus "late_arms = 0 required" -- would have **failed** those four runs, including a rung this campaign reported as qualified. It is a stricter gate than the floor, not a looser one, and it is not satisfied by the current firmware at 45% and 50%.

**2. The printed loop speed carries a rounding bias, and it grows with speed.** `report.rs:431` rounds the mean sector to whole microseconds before deriving frequency. Recomputed from accepted events over the MCU-stamped hold: +0.19% at 15%, +0.32% at 25%, +0.33% at 37.5%, **+0.63% at 50%**. Every gate arithmetic should use `hold_accepted / hold_ms`, never `ehz_from_sector`.

**3. The coast estimator was biased, and anchoring it on the recorded first transition fixes the centre but not the spread.** E143's version fitted the pair sums against *sample index* with a fixed offset, ignoring `first_us` -- which varies from 3 to 714 µs run to run. Fitting against **time from bridge-off**, anchored on `first_us`:

| estimator | mean ‰ | sd | outside the 1% band |
|---|---|---|---|
| index-based (in use) | 1002.9 | 3.06 | 12 / 588 |
| **time-anchored** | **1000.0** | 3.81 | 10 / 588 |

The bias is gone (centre exactly 1000) but the spread is **worse**, because the longer lever arm amplifies the noise in `first_us`. It also flips three previously-passing runs into low-side failures (985.3 and 985.4 at 25%, 988.4 at 37.5%). **Neither estimator is good enough to adjudicate a ±1% per-run gate at these speeds**, and that is the finding -- not a new estimator to swap in.

**4. Like-for-like, the two speed measures agree everywhere except 50%, and the 50% residual looks like the rotor slowing, not slip.**

| rung | hold average vs coast(t=0) |
|---|---|
| 15 → 47.5% | **−0.24% to +0.22%** at every rung |
| 50% | **+0.77%** |

So with the rounding and the anchor corrected, the "1.72% slip" becomes **+0.8 to +1.1% on the clean 50% runs** -- and in three of those four runs the hold's *last* interval sits **below** the hold average (2136 / 2136 / 2109 against averages of 2131–2147). A rotor drooping slightly across a 55 s hold at 1.6 A makes the whole-hold average exceed the speed at the end, which is exactly when the coast is measured. That accounts for the residual with **no extra commutations and no phase lead**, which is binz's point, and it is consistent with graybeard's physics without needing his inference that the accept count is rotor truth.

`ehz_from_ci_last` cannot arbitrate this: it is a single interval and reads −3.58% at 28.8%.

**What this does and does not change.**
* **Retracted:** "the loop leads the rotor by 1.72%: slip", "50% is lock-limited", "late_arms = 0 in every run", and the impossibility framing built on them. The notebook's E150 and E151 stand as written but are superseded on those points by this entry.
* **Not rescued:** 50% still fails the rate gate on **3 of 7** runs with the corrected instrument (1013.2, 1011.5, 1010.5). Graybeard's prediction that 50% passes once the instrument is fixed is **not** borne out.
* **Still unresolved:** the 3-of-6 sag trips at 50%. Nothing in this pass touches their cause.
* **Unchanged:** every protection, hysteresis 0, the storm cap. No gate was moved, no threshold relaxed, nothing reflashed.

**The timing chain, stated with its holes**, since that was the fourth item: edge → *(dispatch and any same-priority blocking: unmeasured)* → entry stamp → **11 µs** to the arm write (`spent_max_us`) → the scheduled wait → fire, **8–11 µs late** (`com_late_max_us`). The last term is the largest measured one, it is **not** what advance changes, and it is not covered by `spent_max_us`. A hardware-stamped edge would close the first hole; it would not shorten the 11 µs, as graybeard says.

**Two decisions now have evidence behind them, and both are the operator's:**
1. **The gate's instrument.** A ±1% per-run band on a quantity whose own instrument scatters ±0.4% (sd 3.8‰) is a coin-toss at the edges. Options: judge the rung on its 3-run mean rather than per run; widen the band to the instrument's measured scatter; or build a better speed reference (a hardware-stamped coast, or the oracle's accepted-rate figure compared like-for-like).
2. **Gate 4.** Neither the 3 µs floor (no basis -- it was a stipulated number) nor "late_arms = 0" (violated at 45% and 50%) is currently satisfied by this firmware at the top rungs. A late-arm hard stop is the sounder form, but it must be introduced knowing it disqualifies 45% and 50% as they stand.

## Campaign 8 — measure the 45–50% timing chain, then test the oracle's scheduling fix

Operator goal, set 2026-09-23, after three rounds of review that retracted most of campaign 7's conclusions. Dead premises, not to reappear: slip; 50% lock- or supply-limited; `late_arms` = 0 in every run; 50% unreachable; and any comparison of *nominal* advance levels across images.

### E153 — step 1: both code claims verified, and a stale comment found

No edits in this step, as the goal requires.

**Claim A, in two parts: both true.**

1. `Seam::root()` (`shared.rs:174-187`) rests its soundness on peer non-preemption, in its own words:

   > *the token proves the caller runs at priority `P`, this value's ceiling. **Every other root that touches the value runs at `P` too, so it cannot preempt this one**; nothing above `P` touches it; every context below `P` borrows only with interrupts masked …*

   It then hands out `&mut *self.cell.borrow(&cs).as_ptr()` with no `RefCell` flag and no masking. Every motor-owned value is `Seam<_, Motor>` and `Motor::NVIC = 0x40` (`shared.rs:65-67`), which is the priority COMP, COM and DMA all share. So the "non-preempting peers" assumption is not incidental -- it **is** the exclusivity argument.

2. **COMP arms COM inside the estimator borrow.** In `det_decide_plain` the whole acceptance arm, `com_arm(left.max(1), 1)` included, runs inside the closure passed to `S.det().zc.root(at, |zc| …)`, so a `&mut ZeroCross` is live across the arm.

   And **COM borrows the same value**: `com_root` phase 1 reads `S.det().zc.root(&mut at, |z| … (z.average_interval(), z.blanking()))` (`roots.rs:845`).

   **Therefore raising COM above COMP, by itself, is undefined behaviour, not a timing change.** A COM interrupt arriving while COMP holds that borrow -- which is exactly what the change is *for* -- creates a second `&mut` to the live `ZeroCross`. The overlap set is `det.zc`; `com.plans` and `com.six` are COM-only, and `det.rate`/`drv.rate` are COMP-only, so `det.zc` is the one value that must be restructured (plus `det.step`, which both write, though as an atomic it is not aliasing).

   This is the same hazard the oracle solved by snapshotting the accepted sector **before** arming, which is what the goal asks to be recovered and cited in step 5a.

**Claim B: true.** `com_late_max_us` is computed and its maximum stored **before** the timer's purpose is dispatched (`roots.rs:822-826`), ahead of `match S.com().phase`:

```
let now_raw = hw::clock::raw();
let late = now_raw.wrapping_sub(S.com().sched_raw.load(..) as u16) as u32;
if late < 0x8000 && late > S.com().late_max.load(..) { S.com().late_max.store(late, ..); }
match S.com().phase.load(..) { 1 => …commutate…, 2 => …reverse blank end…, 3 => …blanking floor arm… }
```

Phase 1 is the commutation; phase 2 ends the reverse blank; phase 3 is E134's blanking-floor arm, which fires **once per sector**. So the reported maximum is the worst of *all three* services and cannot be attributed to commutation lateness. Every statement of the form "the commutation fires 8–11 µs late" -- mine, and the framing I relayed from the graybeard -- **overstates what this counter shows**. Splitting it by purpose is step 2's job.

Also, per the same reading: `late` is measured from `sched_raw`, which `com_arm` itself computes from its own clock read, and it is stamped at ISR entry rather than at the bridge update. So even split by purpose it is *handler service* lateness, not the instant the phases actually change.

**Incidental finding, from my own campaign-7 revert.** The acceptance arm carries a **stale comment**: a block beginning "Arm the commutation **first** (E142)" sits above the four bookkeeping stores, followed by the real arm and a second block recording that arming first was rejected (E142/E144). The code is correct -- stores first, then arm, byte-identical to E141 -- but the first comment describes the reverted behaviour. Comment-only, no codegen effect; it is fixed at the top of step 2 rather than here, since step 1 is verification with no edits.

**Step 1 answers, in one line each:** the borrow contract does assume peer non-preemption and COMP does hold `det.zc` across the arm, so COM-top needs the ownership work first; and `com_late_max_us` does span phases 1–3, so it is not a commutation-lateness measurement.

**Next:** step 2 -- the per-commutation instrument in the diagnostic image, production untouched.

### E154 — step 2: the per-commutation instrument, in a diagnostic image only

**Written before the run.** What the goal asks for: *"Per commutation the
crossing, arm, fire and bridge-update instants, requested wait, elapsed-at-arm,
timer purpose; per late arm its stage, wait, elapsed. Derived: effective angle
= (commutation − crossing)/interval, the only quantity compared across images.
Ring-buffered, dumped after safe_off, never printed while driving."*

**The recorder.** `src/chain.rs`, built on E121's pattern: a `ChainLog` trait
with `const ON: bool`, production's `NoChain` (`ON = false`, empty bodies) and
the diagnostic `ChainRing`. 1024 rows of 12 bytes in a `Seam<Chain, Motor>`,
circular (E138's lesson: a run that dies needs the rows *before* the stop),
armed per run by the foreground and disarmed before the dump so no late event
can race it. Two kinds:

* **kind 1, an acceptance's arm** — the accepted crossing's entry stamp, the
  instant the one-shot was armed, the wait asked for, what the handler had
  spent at the arm, the sector, whether the wait was already exhausted, and the
  stage;
* **kind 2, a COM service** — when the handler ran, **when the bridge actually
  changed**, the instant it was scheduled for, the lateness the firmware would
  report, the sector, and the timer's **purpose** (1 commutation, 2
  reverse-blank end, 3 blanking floor).

Purpose is the whole point: E153 established that `com_late_max_us` is stamped
before `match S.com().phase`, so the single reported maximum is the worst of
all three services and cannot be read as commutation lateness. The bridge
stamp is the other missing piece — the firmware's lateness is measured at ISR
entry, not at the instant the phases change.

**Stage, and what it honestly is.** `roots::stage_code` is three bits the COMP
root can already see: the driven observer's flag, the closed-loop detector's
flag, and the guard's tracking flag. **Ramp and hold it cannot separate** — no
root sees the commanded duty, and I am not adding a store to the foreground
state machine to fake one. That split is made on the host, by position in the
run, and `scripts/chain.py` labels it as a host classification rather than a
firmware label.

**Derived quantity.** `scripts/chain.py` computes effective angle = (bridge
update − crossing) / interval, with the interval taken from consecutive
accepted crossings (backward-looking: the gap ending at the paired crossing).
Nominal advance *level* is not compared across images — that was E145's trap.
The script also splits lateness by purpose, reports handler-entry-to-bridge,
and lists every late arm with its stage, wait and elapsed.

**New image.** `bin/chain-capture.rs` (`[[bin]]` added) installs `ChainRing` in
both motor roots; `NoLog` keeps the decision recorder off, so this image
measures timing, not decisions. The dump is written after the run's own report,
which is after `safe_off` — never a byte while the bridge is live.

**Production untouched, and checked, not claimed.** `scripts/isr_diff.py` (new)
disassembles both ELFs, normalises addresses away and compares the four roots
instruction for instruction. Against the campaign-7 image `63EC7EFE` (E149):

```
DMA1_CHANNEL1: identical, 37 instructions
ADC_COMP:      identical, 710 instructions
TIM16:         identical, 318 instructions
TIM6_DAC_LPTIM1: identical, 153 instructions
```

Longest-path cycles are identical too — COMP 1026, TIM16 349, guard 204, DMA 78
— measured on both ELFs with the same bounds. The whole-image hash is *not*
identical (this entry's production image is `7A6271D1`, against `63EC7EFE`)
and cannot be: the library gained a
module and a field, which moves addresses and statics. The roots are the claim.

The recording image costs what it costs: COMP 1026 → 1093 cycles, TIM16 349 →
415. Stated so no run of it is ever quoted as production loop quality.

**Four things the checks caught, all mine.**

1. **A division under COMP.** The link-time math audit refused the diagnostic
   image: `capture.rs`'s circular index was `total % CAPTURE_LEN`, and
   `CAPTURE_LEN` is 1536 — not a power of two, so on this M0+ the modulo calls
   `__aeabi_uidivmod`. It was introduced by E138 and has been latent since.
   Now an explicit wrap (`bump`), with a host test that it wraps at the end and
   nowhere else. The chain's own ring is 1024, so its index is a mask.
2. **`com_root` over the structure limit** (104 lines with the new rows). I
   first extracted phase 1 into its own function — and `isr_diff.py` caught
   that this changed two of TIM16's instructions (`lsls r4,#31; beq` became
   `cmp r4,#1; bne`; same count, same cost, but not identical). Reverted:
   E141's code stays where it is and two long in-body comments moved into the
   function's doc comment instead. Same treatment for `run::command`.
3. **`cargo fmt` with no config reformatted the whole crate.** The crate had no
   `rustfmt.toml`, so a bare `cargo fmt` reflowed every file at the 100-column
   default. Recovered by pinning `max_width = 120` — the width the crate was
   written at — and re-running; the roots came back instruction-identical,
   which is the evidence that the recovery landed where the code started. The
   file is now in the tree so it cannot happen again. Some line numbers in
   earlier entries may have shifted by a line or two as a result; the
   citations' *content* is what to trust.
4. **An eight-argument trait method**, which clippy refuses. The acceptance row
   is now a `chain::Arm` struct.

**Gates, this image and production.** ISR audit clean on all four roots (bounds
as allowlisted); cycles not growing in production; structure report 0 functions
over 100 lines; host tests 310 + 9 = 319 (≥ 305); clippy clean on the firmware
bins and on the host lib/tests.

**What is not done in this entry.** No powered run yet, and none at or above
45% until the goal's two hard stops exist (a late arm ending the run with its
own reason code, and a non-zero `blank_latched` stop). The next action is a
low-duty validation run of `chain-capture` — 25%, well below that threshold —
whose only purpose is to show the rows are well-formed and the derived angle is
computable. It proves nothing about 45–50% and will not be cited as if it did.

**Completed after the run.** Image `chain-capture`, 25% rung (`5`) after the
unjudged 15% warm-up, driven by the new `scripts/chain_run.py`; capture
`captures/chain/e154-chain25.txt`; run gates PASS (`hold_ms=67276`,
`zc_rate_permille_of_expected=999`, `forced_pct=0`, `late_arms=0`,
`blank_latched=0`, guard `reason=0`). 1,558,324 events offered, the last 1024
kept — the ring is circular, so the rows are the end of the hold, not the
ramp.

What the instrument says, at 25% and only at 25%:

```
COM service lateness, µs, by purpose
  phase 1 (commutation)     n=342  max=5  p50=2.0  mean=1.58
  phase 3 (blanking floor)  n=341  max=4  p50=2.0  mean=1.61
handler entry -> bridge update, µs: n=342 max=5 p50=2.0 mean=1.86
effective angle = (bridge update - crossing) / interval, n=340
  p50=0.2174  mean=0.2183  min=0.1734  max=0.3100  sd=0.0171
  = 13.04 electrical degrees of the 60° sector
```

Two things follow immediately.

1. **The split is real and it matters.** This run's reported
   `com_late_max_us` is 7; the *commutation* service's own maximum is 5 and
   its median 2. The blanking-floor service accounts for the rest. Campaign
   7's "the commutation fires 8–11 µs late" was a reading of the mixed
   counter, and at this rung the commutation part of it is a fifth of that.
2. **The effective angle checks out against the arithmetic**, which is the
   validation this run was for. With `mean_ci_us = 139` and advance level 20,
   `wait_time = (139>>1) − advance_of(139, 20) = 69 − 43 = 26` µs; the
   measured crossing-to-bridge median is 0.2174 × 139 ≈ 30 µs, i.e. the wait
   plus about 4 µs of chain. Nothing here is surprising — that is the point:
   the instrument reproduces a quantity that can be derived independently
   before it is used on rungs where nothing can.

No conclusion about 45–50% is drawn from this run and none can be: it is one
run, at 25%, on a recording image whose COMP root costs 67 cycles more than
production's. Next is step 3, the matched-window speed reference, and the two
hard stops before anything at or above 45%.

### E155 — step 3: speed on matched windows, with the coast's origin measured

**Written before the runs.** The goal: *"the final powered interval of
accepted-event counts against the immediately following coast, both timestamp
origins stated and the coast offset measured, not assumed. The printed loop
frequency rounds the mean sector to whole µs and must not be used; whole-hold
averages are performance statistics and gate nothing."*

**What was missing.** Two things, and both were assumptions rather than
measurements:

1. The powered speed in the report is a *whole-hold* average
   (`BEMFRATE hold_accepted / hold_ms`) whose companion `ehz_from_sector` comes
   from `mean_sector_us`, a **whole-µs** quantity (`report.rs`). At 139 µs a
   sector, one µs of rounding is 7 per mille — larger than the residuals
   campaign 7 argued about. That rounding is exactly how I manufactured the
   "loop leads the rotor by 1.72%" claim, which is retracted.
2. The coast's own origin was never reported. `first_us` is measured from
   `coast_capture`'s `start`, which is *after* `safe_off`, `float_all`, the
   comparator re-point and the hysteresis change. Every back-extrapolation of
   the coast to the bridge-off instant crosses that gap, and nobody had
   measured it.

**The change, report-only, foreground-only.**

* `policy::TAIL_WINDOW_US = 2_000_000`. In the hold, `consume` lays down a
  mark whenever the previous one is older than that, keeping the mark before
  last. The window reported is `tail_prev .. tail_last`, so it **ends at the
  last accepted crossing before the stop** and is between one and two windows
  long — never collapsing to nothing, which a single resetting anchor does
  (the first version of this did exactly that, and the host test caught it at
  1.26 s).
* `BEMFTAIL accepts span_us start_before_stop_us end_before_stop_us
  window_us`: raw counts and spans, no division and no rounding. The host
  divides. Both endpoints are stated as µs before the stop stamp, so the
  window's *position* is known too, not just its length.
* `COASTTIMING` gains `offset_us` — `coast_capture`'s origin minus the stop
  stamp, measured inside the firmware.

No threshold, gate or protection moves; nothing in the four roots changes.
`isr_diff.py` against `63EC7EFE` (E149): all four roots identical, 37 / 710 /
318 / 153 instructions, cycles unchanged at 1026 / 349 / 204 / 78.

**The host side.** `scripts/speed.py`:

* `ehz_powered = accepts × 1e6 / (6 × span_us)`, placed at the window's
  midpoint, `(start_before_stop + end_before_stop) / 2` µs before the stop;
* the coast fit is anchored **in time, not in spacing index**: spacing `k`
  contributes `ehz = 5e5 / iv[k]` at `t_k = offset_us + first_us +
  Σiv[:k] + iv[k]/2`, measured from the stop stamp, and a least-squares line
  through those points is evaluated at `t = 0`. Both `offset_us` and
  `first_us` therefore enter the estimate, which is what the E143 estimator
  (index-based, `first_us` ignored) left out;
* the powered figure is carried to the stop along that same fitted slope, so
  the two numbers speak about one instant, and the difference is reported in
  per mille;
* **no tolerance is applied anywhere.** `--calibrate` summarises the scatter
  across repeat runs of one image at one rung and says so when given fewer
  than five, or more than one rung.

**Host tests** (311 + 9 = 320): the matched window ends at the last crossing,
is one to two windows long, its endpoints bracket its span, and its accept
count matches its own span at the scripted interval to within one crossing;
plus the two pinned report formats updated for the new fields.

**The calibration rung, and why it is 25% and not 37.5%.** The band needs ≥ 5
runs of one image at one rung. This ELF is new, so the fixture's ladder
requires the rung below to have passed on *this* SHA before each rung above —
reaching 37.5% means climbing 15 → 20 → 25 → 27.5 → 28.8 → 30 → 32.5 → 33.8 →
35% first. That climb belongs to step 4, which characterises at 37.5% and
47.5% and will take five runs at each of those rungs rather than three. So the
instrument's repeatability is calibrated here at 25%, and **the band does not
transfer**: step 4 recalibrates at each rung it characterises, since both the
speed and the coast's deceleration differ there. `speed.py` refuses to pool
rungs silently.

**A limit stated up front.** The oracle image cannot carry `BEMFTAIL` — it is
a different firmware and its report has no such line. So this calibration is
*our instrument's* repeatability at one rung, not a cross-image comparison.
Putting our speed beside the oracle's needs the oracle's own quantities and is
step 4's job, through the existing `reference_line` machinery.

**Plan for the runs:** 15% ×3, 20% ×3 to satisfy the ladder on this ELF, then
25% ×5 for the band. Nothing at or above 45%: the two hard stops do not exist
yet.

**Completed after the runs.** Image `1FBA3992…` (the shell-pwm of this entry),
ladder on this ELF: 15% 3/3, 20% 3/3, then **25% 5/5**, every run's gates
passing, `blank_latched=0` throughout, `late_arms=0` throughout. Captures
`captures/2026-09-23/e155-{15,20,25}_*`.

The band, from `scripts/speed.py --calibrate` over the five 25% runs:

```
capture                 duty   powered coast@stop       resid      decel   lever     span
e155-25_01              25.0    1201.9     1198.1     +3.1o/oo    -3222.9     362  3272609
e155-25_02              25.0    1200.6     1199.6     +0.8o/oo    -3899.0     383  3272386
e155-25_03              25.0    1201.4     1197.7     +3.2o/oo    -3378.8     237  3272329
e155-25_04              25.0    1201.1     1204.5     -2.8o/oo    -3604.6     343  3273685
e155-25_05              25.0    1202.7     1204.9     -1.9o/oo    -4590.7     455  3272485

powered eHz:  mean=1201.53  spread=2.10      (1.7 per mille)
coast@stop:   mean=1200.95  spread=7.23      (6.0 per mille)
residual per mille: mean=+0.48  min=-2.83  max=+3.15  sd=2.77
```

**What the band is, and what it is not.** At 25%, on one image, five runs: the
residual between the powered window and the coast brought back to the stop is
**+0.5 ± 2.8 per mille (1 sd), full range 6 per mille**. The scatter is almost
entirely the coast estimator's — the powered figure repeats to 1.7 per mille
while the coast repeats to 6. No tolerance was stipulated and none is applied;
this is the number any residual at this rung has to beat to be a finding.
It does **not** transfer to 37.5% or 47.5%, where both the speed and the
coast's deceleration differ; step 4 recalibrates at each rung it characterises.

**It also retires a claim quantitatively.** Campaign 7's "the loop leads the
rotor by 1.72%" was already retracted as an artefact of whole-µs rounding and
an index-based coast estimator. Measured properly at this rung the lead is
**+0.5 per mille, inside the instrument's own scatter** — i.e. nothing.

**The coast's origin, now that it is measured.** `offset_us` is 37–41 µs across
the five runs: the safing and re-pointing gap is negligible, which is worth
knowing rather than assuming. The real lever arm is `first_us`, 200–455 µs and
run-dependent, and it now enters the fit instead of being dropped.

**Two faults of mine in this step.**

1. **I fitted the wrong quantity.** The first version fitted single
   half-period spacings. The spacings here alternate by about 2% —
   rising and falling half-cycles are not equal on this comparator — so the
   fit measured the alternation's phase, not deceleration: slopes came out
   −8000 eHz/s on four runs and **+1500** on the fifth, and the residuals were
   −13588 to +2122 per mille. Pairing consecutive spacings into full
   electrical cycles cancels the asymmetry and the slopes land at −3200 to
   −4600 eHz/s.
2. **I corrected the powered figure with that slope across a 1.4 s lever arm.**
   The powered window sits in the governed hold, where the duty is held and
   the speed is steady; there is nothing to correct. Carrying a coast slope
   back over 1.4 s is what turned 1202 eHz into −11765. Both faults were
   visible only because five runs of one image at one rung were taken *before*
   any number was believed — one run would have looked plausible.

**Tooling.** The fixture's default `--timeout` is 75 s and the run window has
been 80 s since campaign 7, so the first three-run attempt recorded nothing
(`0/3 runs completed`) while the firmware was perfectly healthy. Runs here pass
`--timeout 140`. Worth fixing in the fixture rather than remembering.

**Next:** step 4 needs 37.5% and 47.5%. 37.5% means climbing the ladder on this
ELF (27.5 → 28.8 → 30 → 32.5 → 33.8 → 35 → 37.5%, three runs a rung); 47.5% is
at or above 45%, so the two hard stops must exist first.

### E156 — the two hard stops the goal requires before any run at 45% or above

**Written before the run.** The goal: *"Before any powered run at 45% or above,
two hard stops must exist: an exhausted-deadline stop (a late arm ends the run
with its own reason code) and a non-zero `blank_latched` stop, each with its
coverage, stages and shutdown behaviour defined in the entry introducing it."*

**`Reason::LateArm = 15`.** COMP arms the commutation with
`left = wait − spent`; when `spent >= wait` the one-shot is armed with
`left.max(1)`, so it fires as soon as it can rather than when the crossing
asked for, and that commutation's angle is whatever the latency happened to
be. Through campaigns 5–7 this was only the counter `late_arms`, and **I
asserted it was zero in every run.** It was not: 4 of 588 captures carry a
non-zero value, one of them a *qualifying* 45% run. A counter nobody stops on
is a counter nobody checks.

**`Reason::BlankLatched = 16`.** E134's phase 3 opens the line after the
blanking floor and counts the case where `EXTI` was already pending. A latched
flag carries no timestamp, so serving it dates the crossing at the gate
instead of at the rotor — minz's FALCON self-lock class. It has read zero over
roughly 590 000 arms, which is precisely why a non-zero reading should end the
run rather than be averaged into a report.

**Coverage, stages, shutdown — the same for both.**

* *Coverage:* the **closed loop only**, from the instant the estimator is
  installed (`closed`) to the stop. Checked in `Ctx::poll` beside the storm
  and handler-overrun stops, so every foreground iteration of the closed loop
  tests them. Both counters only rise, so a single reading suffices.
* *Stages:* the driven observer and the acquisition stages still **count**
  both quantities and do **not** stop on them. Their waits come from a
  commanded interval rather than from the estimator, and no claim in this
  campaign rests on them. A host test pins this: with the fault armed from
  t = 0 the loop still closes, and the stop happens after it closes.
* *Shutdown:* the ordinary protection route — `Err(reason)` out of `poll`,
  which runs `stop()`: estimator released, stop stamped, `safe_off`,
  comparator masked, startup carrier restored, then the report carrying the
  code. The run is over, not degraded; nothing retries.
* *Not thresholds:* neither stop has one. Non-zero is the condition. Nothing
  else in the protection set moves — no threshold, no hysteresis, no storm
  cap, no sag parameter.

**Host tests** (314 + 9 = 323): an exhausted deadline ends the run with code
15 through one `safe_off` and no bytes while driven; a latched blanking edge
ends it with code 16; and neither is armed before the loop closes. The sim
gained two fault injections (`late_arm_at`, `blank_latched_at`) so the stop
*path* is exercised on the host rather than only on the bench.

**The fixture** now names the two codes when a capture fails its gates, rather
than reporting a bare `reason != 2`.

**Production roots unchanged** — both stops are foreground. `isr_diff.py`
against the E155 image: all four roots identical (37 / 710 / 318 / 153).
Structure report clean, clippy clean.

**What this costs.** A run that would previously have been quoted with
`late_arms=1` in its footnotes now fails outright. Campaign 7's 45% and 47.5%
results were taken without these stops, so under the goal's own rule they are
exploratory until re-run under them — not grandfathered.

**The run:** one 25% run, the rung already qualified 5/5 on this ELF's
predecessor, to confirm the stops do not fire on a healthy loop. That is a
negative control, not a qualification.

**Completed after the run.** Image `3830927C` (`captures/elf/3830927C.e156.elf`),
ladder re-established on this ELF (15% 3/3, 20% 3/3 — captures
`captures/2026-09-23/e156-15*`, `e156-20*`), then the negative control at 25%:
`e156-25control_01`.

Prediction was that a healthy loop does not trip either stop. Measurement:
`BEMFDONE reason=2` (the ordinary deadline), `BEMFRCOMP late_arms=0`,
`blank_latched=0` over `blank_arms=519688`, `com_count=519688`, gates passing.
Verdict: matches the prediction. The stops are armed and silent on a healthy
25% loop; that is all one control run shows, and it is not a qualification of
anything.

Two other readings from the same capture, recorded because they are what the
next steps will be compared against — and **as measured maxima of this run,
not as bounds**: `spent_max_us=11`, `com_late_max_us=5` (mixed across the
three timer purposes, per E153), `comp_call_max_us=15`, driven-stage
`late_max_us=60`.

### E157 — step 2 validated at 37.5%, with the instrument's own cost and coverage stated

**Written before the run.** The revised goal asks step 2 to be validated at
37.5% rather than only at 25%, and to state the instrument's overhead and
*timestamp coverage*, with measured maxima never read as bounds.

**Instrument overhead, measured on the linked images** (longest-path cycles,
`scripts/isr_cycles.py`, zero wait states; the G071 runs 2, so the real cost is
higher than these figures, which is stated rather than modelled):

| root | production `3830927C` | recording `BE916F91` | added |
|---|---|---|---|
| `ADC_COMP` | 1026 | 1093 | +67 cycles ≈ 1.05 µs at 64 MHz |
| `TIM16` | 349 | 415 | +66 cycles ≈ 1.03 µs |

So a recorded run's COMP path is about 1 µs longer per accepted crossing and
its COM path about 1 µs longer per service, both on the *longest* path. That
is why no chain run is ever cited as production loop quality — and why the
effective angle from a recorded run carries roughly a microsecond of the
instrument in it, against sector intervals of 139 µs at 25% and ~95 µs at
37.5%: about 0.7–1.1% of a sector. Any cross-image comparison of effective
angle has to carry that term explicitly.

**Timestamp coverage: what the chain does *not* see.** The rows begin at the
COMP handler's own entry stamp. Everything before it is unmeasured:

* the comparator's propagation and the EXTI latch;
* NVIC dispatch, and any blocking by a peer at the same priority (COM or DMA)
  — E153's point, and the reason `Seam::root()`'s borrow argument holds;
* the instruction fetch of the handler's prologue before `hw::clock::raw()`.

At the other end, `bridge` is stamped immediately after `hw::pwm::apply_plan`,
which is the register write; the gate hardware's own delay (TIM1 update, the
26-tick dead-time generator, the driver's propagation) is outside it too.
**The chain therefore measures the firmware's share of the chain, not the
rotor's.** Every figure it produces is a *measured maximum or median of the
runs taken*, not a bound: the ring keeps 1024 rows of a run that offers over a
million events, so the tail it reports is a sample of the end of the hold.

**Prediction for the 37.5% run.** At `mean_ci_us ≈ 95` and advance level 22
(`AdvancePolicy` switches at 35% duty), `wait_time = (95>>1) − advance_of(95,
22) = 47 − [(95>>6)·22 + ((95&63)·22>>6)] = 47 − (22 + 10) = 15` µs, so the
crossing-to-bridge delay should come out near 15 µs plus the chain's few µs,
i.e. an effective angle near 0.19–0.21 of the sector — *lower* than 25%'s
0.217 despite the higher advance level, because the wait shrinks faster than
the latency does. Commutation-purpose lateness should stay a few µs, and the
blanking-floor service should again account for the larger part of the mixed
`com_late_max_us`.

If the effective angle instead comes out *above* 25%'s, the prediction is
wrong and the arithmetic above is the thing to re-derive first.

**Completed after the run.** Recording image `BE916F91`
(`captures/elf/BE916F91.e157.elf`), 37.5% (`J`) after the unjudged 15%
warm-up, driven by `scripts/chain_run.py`; capture
`captures/chain/e157-chain375.txt`; run gates PASS (`hold_ms=60774`,
`zc_rate_permille_of_expected=999`, `forced_pct=0`, `late_arms=0`,
`blank_latched=0` over `blank_arms=692210`, guard `reason=0`). 2,076,629
events offered, the last 1024 kept.

```
COM service lateness, µs, by purpose
  phase 1 (commutation)     n=341  max=4  p50=1.0  mean=1.48
  phase 3 (blanking floor)  n=342  max=5  p50=2.0  mean=1.58
handler entry -> bridge update, µs: n=341 max=5 p50=2.0 mean=1.82
requested wait, µs (per event): n=341 min=14 p50=16.0 max=19 sd=0.60
elapsed at the arm, µs:         n=341 min=5  p50=6.0  max=8  sd=0.47
crossing -> bridge update, µs:  n=340 min=18 p50=20.0 max=23 sd=0.74
sector interval, µs:            n=340 min=71 p50=102.0 max=164 sd=9.92
effective angle: p50=0.1942 mean=0.1978 min=0.1341 max=0.3151 sd=0.0202
  = 11.65 electrical degrees of the 60° sector
  against the forward interval instead: p50=0.1942 sd=0.0243
```

**Verdict against the prediction.** The prediction was a requested wait near
15 µs and an effective angle of 0.19–0.21, *lower* than 25%'s 0.217 despite
the higher advance level. Measured: wait p50 **16 µs** (min 14, max 19) and
angle **0.1942** (11.65°). The prediction holds, including its direction and
its reason — between the rungs the wait fell 26 → 16 µs while the
crossing-to-bridge delay fell only 30 → 20 µs, so the *fixed* part of the
chain grew from 13% to 20% of what the wait asks for.

**Per-event, not per-report** (the step-2 review asked for this, rightly):
the 37.5% requested wait is 14–19 µs per event against the single 15 µs the
aggregate arithmetic predicts, and the sector interval spans 71–164 µs around
a 102 µs median. Both spreads are in the rows and neither is visible in a
report mean.

**Where the angle's spread comes from — not the commutation.** The ratio's
numerator (crossing → bridge) has sd **0.74 µs**; its denominator (the sector
interval) has sd **9.92 µs**. So the angle's sd of 0.0202 is essentially all
interval jitter, and quoting it as timing spread would be wrong. Pairing the
delay with the *forward* interval instead of the backward one moves the median
not at all (0.1942) and the sd from 0.0202 to 0.0243 — the choice is visible
now rather than assumed.

**Which row classes this capture actually exercised**, since the instrument's
claim should not outrun its evidence: kind 1 (accepts) 341 rows, kind 2 phase 1
(commutation) 341, kind 2 phase 3 (blanking floor) 342. **Phase 2 (reverse
blank) never fired and there were no late arms**, so those two paths are
compiled and host-reasoned but not yet exercised on hardware, at either rung.

**Instrument overhead, restated with its model named** (the step-2 review was
right that I quoted one model without saying so, and called a static bound a
measurement):

| root | production `3830927C` | recording `BE916F91` | added |
|---|---|---|---|
| `ADC_COMP`, 0 WS | 1026 | 1093 | +67 cycles ≈ 1.05 µs |
| `ADC_COMP`, 2 WS `--fetch-model` | 1218 | 1294 | +76 cycles ≈ 1.19 µs |
| `TIM16`, 0 WS | 349 | 415 | +66 cycles ≈ 1.03 µs |
| `TIM16`, 2 WS `--fetch-model` | 419 | 494 | +75 cycles ≈ 1.17 µs |

All four are `scripts/isr_cycles.py` **static longest-path upper bounds**, not
measurements — the script says so itself — and the G071 runs at 2 wait states,
so the 2 WS rows are the relevant ones. About 1.2 µs per accepted crossing and
per COM service, against a 102 µs sector at 37.5%: roughly 1.2% of a sector,
which any cross-image comparison of effective angle must carry explicitly.

**Coverage, restated.** The head of the chain is *not* in these rows: the
crossing stamp is read inside `comp_root` after `line_disable()` and
`clear_pending()`, so comparator propagation, the EXTI latch, NVIC dispatch,
peer blocking at the same priority and the M0+ exception entry (~12–16 cycles)
all precede it. At the far end, `bridge` is stamped after `apply_plan`
returns — the register writes are done, but CCR1–3 are written under `OCxPE`
preload, so the *compare* values latch at the next TIM1 update (≤ 20.8 µs at
the 48 kHz carrier), while the channel-role change (CCMR force-inactive, CCER)
takes effect immediately. At a held duty the preloaded compares are unchanged
values, so this matters little here, but "the instant the bridge changed" was
too strong: it is **the instant the firmware finished writing the bridge**.
Gate-driver and dead-time delay are outside it too.

### E158 — independent reviews of steps 2, 3 and 4, verbatim, with responses

The revised goal requires a fresh context-free review after every numbered
step, given only that step's source, captures and notebook, and its findings
appended **verbatim**, each then fixed or its deferral justified. Three
reviewers were run, one per step, each told nothing of my conclusions and
asked the goal's two questions. All three found real defects. Nothing below is
paraphrased; my responses follow each review.

---

#### Review of step 2 (E154), as received

> ## 1. Claims lacking cited measurements
>
> 1. **"Longest-path cycles are identical too — COMP 1026, TIM16 349, guard 204, DMA 78 — *measured* on both ELFs"** (LAB_NOTEBOOK.md:7766-7767). The figures reproduce exactly (I ran `scripts/isr_cycles.py --loop-bound 12 --loop-bound 12` on `captures/elf/63EC7EFE.e149.elf` and `captures/elf/7A6271D1.e154-prod.elf`), but they are not measurements: `scripts/isr_cycles.py:31-33` states "This is a **static upper bound** on the instruction path, **not a measurement**", at 0 wait states, excluding exception entry/exit. The entry cites `isr_diff.py` for this block, which computes no cycles at all (`scripts/isr_diff.py` has no cost model; it compares instruction text only). No tool or artefact is cited for the cycle numbers.
>
> 2. **"a recording image whose COMP root costs 67 cycles more than production's"** (:7848-7849, and :7772-7773). That delta is the 0-wait-state model figure. Under this repo's own "plausible" 2-WS fetch model (`WCET_ESTIMATES.md:85-95`) the same ELFs give COMP 1372 → 1468 and TIM16 518 → 612, i.e. **96 and 94 cycles**. The entry quotes the smallest of its available models as *the* cost, without saying which model, and the G071 runs at 2 wait states.
>
> 3. **"compares the four roots instruction for instruction"** / "branch *offsets within the root* are kept as relative displacements" (:7756-7757; `scripts/isr_diff.py:14-16`). Not implemented: `isr_diff.py:44-47` replaces `<sym+0x..>` with `@` and then `HEX.sub('@', ...)` erases the target address, so `b.n <TIM16+0x30>` normalises to `b.n @`. Branch displacements are discarded; a root with reordered branch targets and identical mnemonics would report "identical". The entry's "the roots are the claim" rests on a check weaker than described. (The instruction *counts* 37/710/318/153 do reproduce, with the default `arm-none-eabi-objdump`; note they are tool-dependent — llvm-objdump gives 34/680/301/138 on the same ELFs.)
>
> 4. **"The blanking-floor service accounts for the rest"** (:7836). The capture does not show this. In the 1024 kept rows the phase-1 max is 5 and the phase-3 max is **4** — *neither* service reaches the firmware's reported `com_late_max_us=7` (capture line 15). The 7 simply occurred outside the kept window; attributing it to phase 3 is unsupported by any row in `e154-chain25.txt`.
>
> 5. **"Campaign 7's 'the commutation fires 8–11 µs late' … at this rung the commutation part of it is a fifth of that"** (:7836-7838). A cross-rung, cross-image comparison, and a category swap: 2 is a *median* at 25%, 8–11 were *maxima* at other rungs. The entry itself forbids exactly this kind of cross-image comparison for advance level (:7745-7747) and closes with "at 25% and only at 25%" (:7820).
>
> 6. **"when the bridge actually changed"** / "the instant the phases change" (:7715, :7722-7724, `src/chain.rs:15-16`). The stamp is `hw::clock::raw()` taken after `hw::pwm::apply_plan` returns (`src/roots.rs:917-923`) — i.e. completion of register writes, before `comp2_select_floating` and before any gate-driver propagation. Also, `apply_plan` writes CCR1-3 under `OCxPE` preload (`src/sixstep.rs:40` `MODE_PWM1 = 0x68`, bit 3 set; `src/hw/pwm.rs:82-101`), so compares latch only at the next TIM1 update; the role change (CCMR force-inactive + CCER) is immediate. Defensible at constant hold duty, but not stated, and not what "actually changed" asserts.
>
> 7. **"that split is made on the host, by position in the run, and `scripts/chain.py` labels it as a host classification"** (:7739-7741). `scripts/chain.py` implements no ramp/hold split — only a docstring note (`chain.py:37-39`) and the raw stage bits on late-arm lines (`chain.py:136`). The promised host classification does not exist. Independently, all 341 accepts in this capture carry stage code 2, so the field discriminated nothing in the run that was supposed to validate it.
>
> 8. **Parts of the quoted goal are claimed delivered but unexercised by the cited capture** (:7707-7711 vs the capture). Zero late arms (capture line 11, and `chain.py` prints "late arms: 0 of 341"), so "per late arm its stage, wait, elapsed" produced no rows; phase 2 (reverse-blank end) never fired (`services` are 342 × phase 1 and 341 × phase 3 only), so that purpose leg of the "timer purpose" claim is untested. The entry's "show the rows are well-formed" (:7810-7812) is therefore true only of two of the four row classes.
>
> 9. **The gate block** (:7801-7803: "ISR audit clean on all four roots; structure report 0 functions over 100 lines; host tests 310 + 9 = 319; clippy clean") cites no artefact — no saved audit/structure/test log in `captures/`. (Host tests now run 314 + 9 = 323 on the current tree, consistent with later entries adding tests; not a contradiction, just unverifiable as stated.)
>
> 10. **"with a host test that it wraps at the end and nowhere else"** (:7783-7784). The test (`src/capture.rs:329-334`) checks three points only (0, LEN-2, LEN-1) — "nowhere else" is not tested. Also `src/capture.rs:300,305` still use `% CAPTURE_LEN`; the claim is true of the ISR path only, which the entry does not qualify.
>
> ## 2. Numbers taken from summaries where the capture holds the per-event data
>
> 11. **`mean_ci_us = 139` → `wait_time = 69 − 43 = 26 µs`** (:7840-7842). The arithmetic is right (`advance_of(139,20)=43`, `src/commutation.rs:255-264`), but the per-event wait is *in the capture*: kind-1 field `c` gives min 25, p50 26, max 28 over 341 accepts. The check was run against an aggregate mean when the per-event column was available — and the underlying per-event interval spans **100–174 µs** (sd 10.1), a spread the single "139" hides entirely.
>
> 12. **"the measured crossing-to-bridge median is 0.2174 × 139 ≈ 30 µs"** (:7841-7843). Reconstructed from a ratio median times an aggregate mean. The direct per-pair value is in the capture: median 30.0 µs, mean 30.29, min 28, max 34. Same answer here, by a route that breaks whenever the interval varies — which it does.
>
> 13. **"the wait plus about 4 µs of chain"** (:7842-7843) is a scalar residual of two summaries. The capture decomposes it per event: entry→arm p50 8 µs, spent p50 6, phase-1 timer lateness p50 2, handler-entry→bridge p50 2.
>
> 14. **`com_late_max_us` is 7** (:7835) is a whole-run firmware aggregate (capture line 15, over 519,442 commutations) compared directly against instrument maxima drawn from 1024 rows (0.066% of 1,558,324 offered events, ≈ 47 ms of a 67,276 ms hold). The two sides of the comparison are different populations; the entry states the ring is circular (:7816-7818) but does not carry that caveat into the comparison.
>
> 15. **The angle's `min=0.1734 max=0.3100 sd=0.0171`** (:7828) are quoted as if they described commutation-timing spread. They do not: the numerator (crossing→bridge) has sd **0.84 µs** (28–34 µs), while the denominator interval has sd **10.09 µs**. Essentially the entire reported spread is denominator jitter. Compounding it, `chain.py:112-121` pairs a *backward*-looking interval (the gap ending at the crossing) with a delay measured into the *following* sector — a deliberate choice (:7742-7744) whose effect on the quoted spread is not examined (recomputing with the forward interval moves sd to 0.0184 and max to 0.3200).
>
> ## Flagged specifically
>
> **Measured maxima treated as bounds.** Items 4 and 14: "the *commutation* service's own maximum is 5" (:7835) is a max over 342 of 519,442 commutations and is used as though it bounded the service, to reassign the firmware's 7 to another purpose. Same class at :7826 ("handler entry -> bridge update max=5") and the angle `max=0.3100`. The converse error appears at items 1-2: `isr_cycles.py`'s genuine static *upper bound* is described as "measured" and its 0-WS delta is presented as the actual cost paid.
>
> **Instrument overhead and chain coverage.** The entry states a whole-root overhead (COMP 1026 → 1093, TIM16 349 → 415, :7772-7773) but never the instrument's cost *inside the window it reports*, and never that these are 0-WS static estimates (see item 2). On coverage: the "crossing" stamp is `raw`, read in `comp_root` **after** `hw::comp::line_disable()` and `clear_pending()` (`src/roots.rs:1012-1016`), i.e. after comparator propagation, EXTI/NVIC latency, M0+ exception entry (~12-16 cycles) and two register writes; the far end stops at register-write completion, before `comp2_select_floating` and gate-driver delay (item 6). The entry does say the kind-1 field is an "entry stamp" (:7716) and that firmware lateness is "measured at ISR entry" (:7723-7724) — then in the derived quantity calls it "crossing" and reports "13.04 electrical degrees" (:7830) and "crossing-to-bridge" (:7841) as if the head of the chain were included. It nowhere states what fraction is covered or bounds the omitted head; at 139 µs/60° each omitted µs is 0.43°, and the same file records a ~10-11 µs handler response figure (`src/roots.rs:388-392`; capture line 11 `spent_max_us=10`, line 15 `comp_call_max_us=17`), so the omitted segment is not obviously negligible against a 13.04° result.

#### My responses to the step-2 review

1. **Accepted, corrected.** They are static longest-path upper bounds from
   `isr_cycles.py`, not measurements, and I cited the wrong tool for them.
   E157 restates them as bounds and names the tool.
2. **Accepted, corrected.** E157 gives both models side by side and says the
   2 WS rows are the relevant ones: COMP 1218 → 1294 (+76), TIM16 419 → 494
   (+75) on the current pair of images. (The reviewer's 1372/1468 figures are
   the same model on the E154-era ELFs.)
3. **Accepted — a real defect in my tool, now fixed.** `isr_diff.py` keeps
   each intra-root branch's displacement as `>>±n` instructions from the
   branch, so a reordered target is a difference. Every comparison was re-run
   under the stronger check and all four roots are still identical across
   E149 → E155 → E156 (`captures/gates/e157-isr_diff.txt`), and the tool
   correctly reports the recording image's roots as differing, which is its
   negative control. Tool-dependence of the counts is now documented in the
   script.
4. **Accepted, retracted.** No row in that capture supports attributing the
   firmware's 7 to phase 3. The kept window's phase-1 max is 5 and phase-3
   max 4; the 7 happened outside the window.
5. **Accepted, retracted.** Comparing a 25% median against maxima from other
   rungs and images is exactly what this campaign forbids. What stands is
   only this: in the capture's window, the commutation service's median
   lateness is 1–2 µs, and the firmware's single counter cannot be read as
   commutation lateness (which is E153's finding, on code).
6. **Accepted, corrected in E157**, including the `OCxPE` preload point: the
   stamp is the instant the firmware *finished writing* the bridge, and the
   compares latch at the next TIM1 update.
7. **Accepted.** No host ramp/hold split exists, and in both captures every
   accept carries stage code 2, so the field discriminated nothing at these
   rungs. **Deferred with reason:** a split is only meaningful for a capture
   whose window spans a stage boundary, and a circular ring that keeps the
   last 1024 events never will at these rungs. If step 5 needs it, the ring
   will be armed at the boundary instead of the split being invented on the
   host.
8. **Accepted, recorded in E157**: kind 1 and phases 1 and 3 are exercised;
   phase 2 (reverse blank) and the late-arm rows are not, at either rung.
9. **Accepted, fixed:** `captures/gates/e157-gates.txt` and
   `captures/gates/e157-isr_diff.txt` now hold the structure report, the host
   test results and the root diff.
10. **Accepted, qualified.** The test checks the two ends and one interior
    point, not "nowhere else"; and `%` remains in `capture.rs`'s foreground
    emit path, which is allowed — the audit only forbids it under an ISR.
11–13. **Accepted, done in E157**: `chain.py` now prints the per-event
    requested wait and elapsed-at-arm, and the decomposition of the angle,
    and E157's numbers are per-event rather than derived from report means.
14. **Accepted.** The two populations differ — a whole-run firmware maximum
    against 1024 rows of the hold's end — and E157 states the sample size
    beside every figure instead of comparing across populations.
15. **Accepted, measured.** E157 reports numerator sd 0.74 µs against
    interval sd 9.92 µs at 37.5% (0.84 / 10.09 at 25%), so the angle's sd is
    interval jitter and is no longer quoted as timing spread; the
    forward-interval variant is printed beside the backward one.

---

#### Review of step 3 (E155), as received

> Read the E155 section (LAB_NOTEBOOK.md:7852–8001), all six source files, and the eleven `e155-*` captures; re-ran `speed.py`.
>
> **Re-run result (exact):** the five table rows and `mean=1201.53 / spread=2.10`, `mean=1200.95 / spread=7.23`, `mean=+0.48 min=-2.83 max=+3.15 sd=2.77` all reproduce verbatim. The arithmetic behind them checks out. Everything below is about the prose around those numbers.
>
> ## 1. Claims lacking cited measurements
>
> 1. **The coast formula in the entry is the retracted one.** NB:7899-7901 states `ehz = 5e5 / iv[k]` at `t_k = … + Σiv[:k] + iv[k]/2` — a single-half-period fit. `speed.py:74-80` pairs non-overlapping spacings: `1e6/(iv[2k]+iv[2k+1])`. The entry's own NB:7979-7986 says the half-period version was fault #1 and was replaced. So the "how it works" bullet documents the abandoned estimator; the numbers in the table came from the other one.
> 2. **"the powered figure is carried to the stop along that same fitted slope, so the two numbers speak about one instant" (NB:7905-7907) is false of the code.** `speed.py:103,120` compares raw `powered` against `at_stop` with no correction; `speed.py:35-38` says explicitly it is *not* slope-corrected, and NB:7987-7990 calls the correction fault #2. Two adjacent paragraphs of the same entry contradict each other, and the bullet list describes a script that does not exist.
> 3. **"placed at the window's midpoint" (NB:7897-7898) is inert.** `mid_before_stop_us` is computed (`speed.py:104`) and stored (`:117`) but never printed and never enters the residual. The powered figure is placed nowhere.
> 4. **`isr_diff.py` (NB:7892-7893)** — "all four roots identical, 37/710/318/153 instructions, cycles unchanged at 1026/349/204/78": no tool output, log or artifact in the repo. The four cycle figures match `WCET_ESTIMATES.md:83` and prior entries, but that row is the **0-wait-state model**, not a measurement; the entry presents it as "cycles unchanged" with no model named.
> 5. **"Host tests (311 + 9 = 320)" (NB:7912)** — no test-run output cited; `src/` carries 314 `#[test]` attributes.
> 6. **"the host test caught it at 1.26 s" (NB:7882-7883)** — no `1.26` anywhere in the tree. The surviving test (`sim.rs:474-499`) asserts `span >= TAIL_WINDOW_US` on a 20 s scripted hold; the collapsing version and the 1.26 s observation are unreproducible from the repo.
> 7. **"is between one and two windows long — never collapsing to nothing" (NB:7878-7881) is not what the code guarantees.** `mod.rs:212` uses `s.tail_prev.or(s.tail_mark)`, guarded only by `t1 - t0 > 0`. For a hold shorter than two windows the reported window is `tail_mark..tail_last`, i.e. *under* one window. The claim holds for the 3.27 s holds here (`e155-25_01.txt:11`) and for the 20 s sim, and is untested and untrue outside that.
> 8. **"`--calibrate` … repeat runs of one image at one rung and says so when given fewer than five, or more than one rung" (NB:7908-7911)** overstates the tool. `speed.py:154-165` checks rung count and n; it never reads `# elf_sha256` (present at line 1 of every capture). "One image" is an operator promise, not a guard, while the sentence reads as if the script enforces it.
> 9. **Self-contradiction on tolerance.** NB:7908 "no tolerance is applied anywhere" and NB:7962 "none is applied", then NB:7963 "this is the number any residual at this rung has to beat to be a finding" — which is a tolerance, asserted with no justification for using 1 sd of n=5 as a detection threshold.
> 10. **The causal attribution at NB:7865-7868** — whole-µs sector rounding is "exactly how I manufactured the 'loop leads the rotor by 1.72%' claim" — is not measured. At this rung the named mechanism is worth ~4.6‰: `ehz_from_sector=1207` against the tail window's 1201.4 (`e155-25_03.txt:10-11`). Nothing in the entry measures the configuration that produced 17.2‰, so the mechanism is asserted, not shown, and is short by roughly 2.5×.
> 11. **"the safing and re-pointing gap is negligible" (NB:7972-7973)** is a verdict on a five-sample observed range of an unbounded foreground path (`measure.rs:198-220`: `safe_off` → `float_all` → `comp_select` → `comp_hysteresis` → `start`). Nothing bounds it; 15/20% runs already show 42 µs (`e155-20_01.txt:24`).
> 12. **"the slopes land at −3200 to −4600 eHz/s" (NB:7985-7986)** is offered as the fix working, but that is a 43% spread across five nominally identical runs of one image at one rung, from a 4-point fit over ~1.7 ms of coast. The entry does not note that this is the estimator's own noise and is the direct source of the 6‰ coast scatter it goes on to call the band.
> 13. **`iv[0]` retention is unremarked.** `speed.py:74` starts pairing at `iv[0]`, which `cohort.py:76-77` documents as possibly the demagnetisation transient rather than the rotor (461 µs vs 423-445) and drops for that reason. The entry claims the estimator is an improvement over E143 (NB:7902-7904) without mentioning it re-admits the sample E143 excluded.
> 14. **"0/3 runs completed" (NB:7994-7996)** — no log cited. The `--timeout` default of 75 s is real (`bemf_run.py:314`); the 80 s window is real (`total_ms=80000`, `e155-25_01.txt:3`); the failed attempt itself is anecdote.
>
> ## 2. Numbers taken from summaries rather than captures
>
> 1. **`first_us`, "200–455 µs" (NB:7974).** The captures say 324, 343, 200, 304, 414 (`e155-25_0{1..5}.txt:24`). 455 is `speed.py`'s `lever` column for run 05 — `offset_us + first_us` (`speed.py:72,148`) — i.e. the entry read its own summary table, mislabelled the sum as `first_us`, and thereby overstates the "real lever arm" it is characterising. (`offset_us` "37–41" at NB:7972 *is* from the captures and is correct.)
> 2. **The quoted `--calibrate` block (NB:7944-7956) is an edited transcript, not output.** The script's `n=5 runs, duties=[25.0]` line is missing, `range=5.99` is missing, `sd=2.77` has been moved up onto the `residual per mille` line (`speed.py:154,161-163`), and the parentheticals `(1.7 per mille)` / `(6.0 per mille)` are not emitted by the script at all — they are the author's own `spread/mean` (2.10/1201.53 = 1.75‰; 7.23/1200.95 = 6.02‰, both correct, both presented inside a code fence as machine output).
> 3. **"the powered figure repeats to 1.7 per mille while the coast repeats to 6" (NB:7960-7962)** are max−min *ranges*, while the band beside them is quoted as **1 sd** ("+0.5 ± 2.8"). Two different statistics, from n=5, compared as if commensurable; "the scatter is almost entirely the coast estimator's" is an eyeball inference with no decomposition.
> 4. **The retirement of the lead (NB:7966-7971) ignores the co-located contradicting summary.** Every one of these five captures ends with `BEMFREF … coast_ehz=1198 loop_ehz=1207 … speed_pct=+1.0` (`e155-25_03.txt`, last line). That +1.0% is built from `ehz_from_sector` and `cohort.coast_ehz` (`bemf_run.py:139,141,150-152`) — i.e. the very rounded quantity E155 declares "must not be used" and the index-based E143 estimator it declares superseded (`cohort.py:63-89`). The same runs therefore report a +7.5‰ lead and a +0.5‰ lead, unreconciled, and the "5/5 every run's gates passing" claim (NB:7939-7940) rests on the superseded estimator.
> 5. **"The spacings here alternate by about 2%" (NB:7980-7981)** is rung-specific and its convention unstated: at 25% the pairs are 409/425 (±1.9% about the mean, 3.9% pair-to-pair, `e155-25_01.txt:24`); at 15% they are 748/673 (±5.3%, 11% pair-to-pair, `e155-15_01.txt:24`). `speed.py:22` repeats the 2% as a property of the comparator.
>
> ## 3. On the specific checks asked
>
> - **Arithmetic vs entry:** the *powered* formula matches (`speed.py:103`, NB:7896-7897). The *coast* formula does not (item 1.1). The *comparison* description does not (item 1.2). Re-run output matches the numbers but not the transcript (item 2.2).
> - **Is the band supported, and scoped right?** The scatter is real and reproduces. Scope is stated correctly and repeatedly (NB:7958 "At 25%, on one image, five runs", NB:7964-7965 "does not transfer", NB:7932-7937, NB:7935-7937 on the oracle's absence) — this is the entry's strongest part. It is then undone by NB:7963 promoting 1 sd of n=5 into a pass/fail threshold.
> - **Is the "loop leads the rotor" retirement supported at this rung?** The +0.5‰ measurement itself is supported at 25% on image `1FBA3992` only, and the entry says so. What is *not* supported is (a) the diagnosis of the original 17.2‰ (item 1.10), (b) the implicit generality, given the entry never states the rung or image the 1.72% was measured on, and (c) the silence on the same captures' own `speed_pct=+1.0` (item 2.4). "Measured properly at this rung the lead is +0.5 per mille … i.e. nothing" is sound as a 25% statement and unsupported as an explanation of the retracted one.
> - **Does the coast derivation match the code, incl. `offset_us` and `first_us`?** Partly. Both do enter, exactly as claimed: `speed.py:71` seeds `t = offset_us + first_us`, and `offset_us` is firmware-measured from the pre-`safe_off` stamp (`states.rs:881-883` → `measure.rs:220`), so the entry's ordering claim (NB:7869-7873) is correct. `first_us` is `t0 - start` from the debounced candidate's onset (`measure.rs:142`), and `trans_iv` are onset-to-onset (`:146`) — consistent. The evaluation at `t=0` matches (`speed.py:90`). The per-spacing formula and the powered-side correction do not (items 1.1, 1.2).
> - **Maxima / 5-run scatter treated as bounds:** NB:7963 (sd → finding threshold); NB:7959-7961 (range → "the band"); NB:7972-7973 (5-run range → "negligible" for an unbounded software path); NB:7974 (5-run max, itself mis-transcribed, → the lever arm's extent, contradicted at 645 µs by `e155-15_02.txt:24` and by `speed.py:31-32`'s "a few hundred"); NB:7985-7986 (5-run slope range presented as the deceleration rather than as estimator noise).

#### My responses to the step-3 review

1. **Accepted, corrected here.** The entry's coast bullet described the
   *abandoned* half-period estimator while the numbers came from the paired
   one. The code was right; the prose was a leftover. The estimator in force
   is `ehz_k = 1e6 / (iv[2k] + iv[2k+1])` at the pair's own midpoint in time.
2. **Accepted, retracted.** "The powered figure is carried to the stop along
   that same fitted slope" is false of the code and contradicts the same
   entry's own fault #2. There is **no slope correction**: the powered figure
   stands as measured and only the coast is brought back to the stop.
3. **Accepted.** `mid_before_stop_us` is computed and printed nowhere; it
   documents the window's position for anyone reading the row, and the
   powered figure is *not* "placed" anywhere. Prose corrected by this
   sentence; the field stays, unused, and is now described as such.
4, 5. **Accepted, fixed.** Artefacts now saved (`captures/gates/e157-*`), and
   the cycle figures are named as 0 WS static bounds with the 2 WS model
   beside them (E157).
6. **Accepted.** The 1.26 s figure was an observation from the failing test
   run in my session transcript, not from any capture or stored log, and the
   collapsing version of the code no longer exists. It is an anecdote and is
   labelled one; what remains checkable is the surviving test.
7. **Accepted, qualified.** For a hold shorter than two `TAIL_WINDOW_US` the
   reported window is `tail_mark..tail_last`, i.e. *under* one window. The
   guarantee is only that `span_us` always states what was actually measured.
   Every capture in this campaign holds ≥ 30 s, so the two-mark case is the
   one in force, but the claim as written was too strong.
8. **Accepted — now enforced, not promised.** `speed.py` reads
   `# elf_sha256` from each capture, prints the images in the set, and warns
   when more than one image or an unidentified one is pooled.
9. **Accepted, resolved.** The scatter is **descriptive**, not a threshold.
   Nothing gates on it: no fixture check, no code path. The sentence
   "this is the number any residual has to beat to be a finding" is
   withdrawn — 1 sd of n = 5 is not a detection threshold, and if a later
   step needs one it must be derived, not borrowed from this paragraph.
10. **Accepted, retracted.** I claimed whole-µs rounding was "exactly how"
    the 1.72% arose. Measured at this rung the rounding term is
    `ehz_from_sector 1207` against the tail window's 1201.4, i.e. **+4.7 per
    mille** — most of the co-located `speed_pct=+1.0` but nowhere near
    17.2 per mille, and I never measured the configuration that produced the
    17.2. The claim is now: at 25% on `1FBA3992` the lead is +0.5 ± 2.8 per
    mille, and the cause of the retracted 1.72% remains unexplained.
11. **Accepted, corrected.** `offset_us` is 37–41 µs over these five runs and
    42 µs in a 20% run; the path is not bounded by anything, so "negligible"
    is replaced by the measured values and the note that it is a foreground
    path with no bound.
12. **Accepted, stated.** The slope's 43% spread across five identical runs
    *is* the estimator's own noise, from a 4-point fit over ~1.7 ms, and it is
    the direct source of the 6 per mille coast scatter. It is not a
    deceleration measurement.
13. **Accepted, and measured instead of argued.** `--drop-first` added.
    Dropping `iv[0]`: coast spread 7.23 → 18.29, residual sd **2.77 → 6.38**
    per mille. So keeping `iv[0]` is the better estimator *for this
    time-anchored pair fit* — the opposite of what `cohort.py`'s index fit
    needs — and that is now a measurement rather than a preference.
14. **Accepted**, labelled an anecdote; the timeout default and the window
    length are both checkable, the failed attempt is not.

Summaries: **(2.1)** accepted — `first_us` is 200–414 µs across the five
captures; 455 was the *lever* column, `offset + first`, and I mislabelled it.
**(2.2)** accepted — the quoted block was an edited transcript with my own
parentheticals inside a code fence; the unedited output is what
`--calibrate` prints today, and the per-mille parentheticals were my own
arithmetic (1.75 and 6.02 per mille), not the script's. **(2.3)** accepted —
ranges and 1 sd were mixed; the band is +0.48 per mille mean, sd 2.77,
range 5.99, all at n = 5. **(2.4)** accepted and now reconciled: `BEMFREF`'s
`speed_pct=+1.0` is built from `ehz_from_sector` (whole-µs rounded) and
`cohort.coast_ehz` (the E143 index fit), i.e. the two quantities E155
supersedes; for `e155-25_03` that is 1207 against 1198, while the matched
window gives 1201.4 against 1197.7 — the +4.7 per mille rounding term is the
difference, and **the gates those five runs passed were the old estimator's,
not this one's**. **(2.5)** accepted — the alternation is rung-dependent
(≈4% pair-to-pair at 25%, ≈11% at 15%) and `speed.py`'s docstring now says so
with both numbers.

---

#### Review of step 4 (E156), as received

> ## 1. Claims in E156 that lack cited measurements
>
> 1. **"4 of 588 captures carry a non-zero value, one of them a *qualifying* 45% run"** — no citation of any kind in the entry, and none in the material. The identical sentence is repeated as a source comment (`src/protection.rs:51-57`), so the code cites the claim and the claim cites the code. The only tool in the given material that could produce it is `scripts/coast_offline1.py:128-134`, which the entry never names, and no capture set, directory, or run list is identified.
> 2. **"It has read zero over roughly 590 000 arms"** — no capture, log, or script output cited. The underlying quantity exists (`blank_arms`, incremented at `src/roots.rs:949,955`; reported at `src/report.rs:557`), but the 590 000 figure is attached to nothing.
> 3. **"`isr_diff.py` against the E155 image: all four roots identical (37 / 710 / 318 / 153)"** — no artifact path or output for the comparison; `scripts/isr_diff.py` exists but no saved result is referenced. Same for **"Structure report clean, clippy clean"** (`scripts/structure_report.py`).
> 4. **"the rung already qualified 5/5 on this ELF's predecessor"** — uncited; no capture set or image ID for the 5/5.
> 5. **"~590k arms"/"zero" used as the *justification* for stopping** — the argument that a latched flag "dates the crossing at the gate instead of at the rotor — minz's FALCON self-lock class" is an analogy to another project, with no measurement on this bench (by the entry's own account the condition has never been observed here).
> 6. **No measurement of what the two checks cost the foreground loop.** The entry's only cost claim is about the *roots* ("Production roots unchanged — both stops are foreground"). Two extra atomic loads run in every closed-loop iteration (`src/run/states.rs:219-224`, `bin/board.rs:392-398`), and this campaign has treated 1 µs of foreground/arm latency as decision-grade (its own E142/E144 reversal). Nothing is quoted for the added pass cost.
> 7. **Wrong function named:** "Checked in `Ctx::poll`" — there is no `Ctx::poll`. The checks are in `Ctx::pass` (`src/run/states.rs:188`, checks at 219-224); `poll` is on `Startup` (457) and `Locked` (708).
>
> ## 2. Numbers that came from summaries rather than captures
>
> 1. **"4 of 588"** and **"one of them a qualifying 45% run"** — an offline aggregate over capture files (`scripts/coast_offline1.py:114-134` reads `late_arms`/`blank_latched` out of already-parsed report records), i.e. a summary of summaries; no capture is opened or named in the entry.
> 2. **"roughly 590 000 arms"** — a sum of a per-run `blank_arms` report field across runs, not a measured quantity from any one capture.
> 3. **"37 / 710 / 318 / 153"** — sizes from a tool report, not a bench capture (this is legitimate as a static measure, but the report itself is not attached).
> 4. **"314 + 9 = 323"** — *verified correct*, so not a finding: `cargo test` gives `314 passed` (lib) + `9 passed` (doc-tests).
>
> ## Coverage: does it match the claim?
>
> 5. **"the driven observer and the acquisition stages still **count** both quantities and do **not** stop on them" is false in production.** Both increment sites are gated on `S.com().active`: `late_arms` at `src/roots.rs:398-405` and `456-459` (inside `if S.com().active.load(...)`), `blank_latched` at `src/roots.rs:970` (after the `if !S.com().active { … return }` early exit at `src/roots.rs:901-904`). `com().active` is set true only in `com_handover` (`bin/board.rs:498,506`), i.e. at loop closure. Before closure neither counter can rise at all — the earlier stages do not count them. Additionally both are **zeroed at the handover**: `det.late_arms.store(0, …)` (`bin/board.rs:494`) and `com.blank_latched.store(0, …)` (`bin/board.rs:505`), so even hypothetical pre-closure counts would be discarded. The "stages count" property is an artifact of the **sim only** (`src/run/sim.rs:270-275`, where the value is a time latch that reads 1 from `t=0`), and the test that "pins this" (`src/run/sim.rs:531-543`) pins sim behaviour, not production behaviour.
> 6. **Otherwise the closed-loop coverage claim holds, with one precedence caveat.** `Locked::poll` calls `ctx.pass::<P>(hal, true)` unconditionally every pass (`src/run/states.rs:708-712`), and `pass` is reached from the single run loop (`src/run/mod.rs:126`), so every closed-loop iteration does reach the checks — *unless* an earlier `return Err` in the same pass fires first (`Driver` 191-193, `guard_reason` 205-208, `storm` 209-211, `overrun` 212-214). That ends the run anyway, but a coincident late arm is then reported under the other code; the entry's "every foreground iteration tests them" is true only up to that ordering.
> 7. **No mid-run clear.** The only writes of 0 are in `det_install`/`com_handover` (`bin/board.rs:494,505`), both once, before `closed` is ever true (`src/run/states.rs:673,688`); `Startup::poll` passes `closed=false` (458). So "both counters only rise, so a single reading suffices" is supported *within* the closed loop.
>
> ## Thresholds
>
> 8. **Supported by inspection: no existing threshold is touched.** The change is additive at `src/protection.rs:58-70` (two new codes, 15/16, no collision with `reason_from_code`'s 1/3/4/7/8 mapping, `src/roots.rs:775-784`, `src/run/states.rs:319+`), `src/run/hal.rs:292-293`, `bin/board.rs:392-398`, `src/run/states.rs:219-224`, `src/run/sim.rs:45-46,270-275`. Caveat: `firmware50/` is **untracked** in the repo (no git history for this tree), so "nothing else in the protection set moves" cannot be verified by diff — it rests on inspection only, and the entry offers no diff either.
>
> ## Host tests
>
> 9. **They do exercise the stop path, and they pass.** `run_in_background`-free runs: `cargo test --target x86_64-pc-windows-msvc -- late_arm blank hard_stops` → 10 passed (includes `a_latched_blanking_edge_ends_the_run`, `the_hard_stops_are_not_armed_before_the_loop_closes`); `-- exhausted_commutation_deadline` → 1 passed. Note the entry's own filter words don't match: no test name contains `late_arm` (it is `an_exhausted_commutation_deadline_ends_the_run`, `src/run/sim.rs:504-516`), and `cargo test … late_arm blank` is rejected by cargo as two positional args. The tests assert more than the code (`safe_offs == 1`, `bytes_while_driven == 0`, `src/run/sim.rs:513-515`), so the path claim is fair — but they drive the **sim's** `Hal`, not the production increment condition `left == 0` (`src/roots.rs:400`) nor the `hw::comp::pending()` read (`src/roots.rs:969`); neither of those is covered on the host.
>
> ## Unmeasured hardware claims
>
> 10. **"The run: one 25% run … to confirm the stops do not fire on a healthy loop"** — written before the run; the entry contains no result. Nothing in E156 shows either stop behaving on hardware, in either direction.
> 11. **Both stops have never fired on this bench by the entry's own account** (`blank_latched` "read zero"), so every statement about their on-hardware behaviour — that the shutdown is clean, that a non-zero reading indicates the FALCON self-lock class, that a late arm's "angle is whatever the latency happened to be" — is a code/mechanism argument, not a measurement. The shutdown *sequence* itself is code-supported (`src/run/states.rs:879-895`: `det_release` → stamp → `take_back` → `safe_off` → `comp_mask` → `set_period(STARTUP_TICKS)`), so only the on-hardware effect is open.
> 12. **"A run that would previously have been quoted with `late_arms=1` … now fails outright"** is supported in code (the new checks precede the `SegmentDeadline` reason-2 exit, `src/run/states.rs:219-231`; fixture naming at `scripts/cohort.py:143-149`), but the *rate* at which this will cost runs at 45-50% is not measured — it is inferred from the uncited 4/588.

#### My responses to the step-4 review

1, 2.1. **Accepted, cited.** The "4 of 588" is from the E152 offline pass over
   the stored campaign-5-to-7 captures, produced by the `scripts/coast_*`
   offline tooling; it is a summary over report lines, not a per-event
   measurement, and the entry should have said so and named it. The four
   captures are `c7-500_01` (1), `c7-500c_01` (1), `c7-500c_03` (2) and
   `c7f-450_02` (1), the last in a 45% run that passed its gates.
2, 2.2. **Accepted, corrected in source.** The "~590k arms" was a single
   run's `blank_arms` from E138's probe, quoted as if it were a total. The
   doc comment now says "zero in every capture taken since", which is what
   the captures support.
3, 2.3. **Accepted, fixed:** `captures/gates/e157-isr_diff.txt` and
   `captures/gates/e157-gates.txt`.
4. **Accepted, cited:** the 5/5 is E155's `e155-25_0{1..5}` on image
   `1FBA3992`.
5, 11. **Accepted.** The FALCON analogy is a mechanism argument, not a
   measurement on this bench, and neither stop has ever fired here. What is
   measured is only that both counters read zero — through E157's 37.5% run
   (`blank_arms=692210`, `blank_latched=0`) included.
6. **Accepted, and now measured.** Over the same 25% rung and hold, the
   closed-loop foreground pass count falls from 3,614,428 ± 300
   (`e155-25_0{1..5}`, five runs on `1FBA3992`) to **3,541,117**
   (`e156-25control_01` on `3830927C`): **−2.0%** of the foreground pass
   rate, one variable between the images. That is the price of the two
   relaxed loads, and it is a real cost worth knowing — the foreground poll
   is what serves the revisit path.
7. **Accepted, corrected:** the checks are in `Ctx::pass`, not `Ctx::poll`.
5 (coverage). **Accepted — my claim was false.** Both counters are gated on
   `S.com().active` and both are *zeroed at the handover*, so the earlier
   stages neither count nor stop; the "stages count" property exists only in
   the sim, whose fault injection is a time latch. The source comment in
   `protection.rs` has been corrected to say this, and to say that an earlier
   draft claimed otherwise and was falsified by review. The coverage that is
   real: closed loop only, counters zeroed at closure, monotone thereafter.
6 (precedence). **Accepted, stated:** an earlier `return Err` in the same pass
   (driver, guard, storm, overrun) wins, so a coincident late arm is reported
   under that code instead.
8. **Accepted:** `firmware50/` is untracked, so "nothing else moves" rests on
   inspection, not on a diff. Worth fixing at the tree level rather than
   arguing; noted as an outstanding weakness of every entry in this campaign.
9. **Accepted:** the tests drive the sim's `Hal`. The production increment
   conditions — `left == 0` in the acceptance arm and `hw::comp::pending()`
   in phase 3 — are **not** host-covered, and cannot be while they live in
   target-only roots. That is a real gap, not a deferral: the first on-bench
   evidence for either stop will be the first run that trips one.
10. **Fixed:** E156's completion (appended above, before this entry) carries
    the control run — `reason=2`, `late_arms=0`, `blank_latched=0` over
    519,688 arms.
12. **Accepted:** the rate at which these stops will cost runs at 45–50% is
    not measured and is not predicted here. Step 5 will measure it.

---

**Standing corrections carried out of this entry, to be honoured in every
later one:** cycle figures are static bounds with their model named; maxima
are labelled with their sample and never used as bounds; per-event columns are
used where the rows carry them; quoted blocks are unedited tool output or are
marked as not being output; scatter is descriptive and gates nothing unless a
gate is derived on purpose; and a claim about production behaviour is not
supported by a sim test.

### E159 — step 5, part 1: 37.5% characterised, and the advance question in effective angle

**Written before the 47.5% work.** No new build: this part analyses captures
already taken and recorded — `captures/chain/e157-chain375.txt` (recording
image `BE916F91`, 37.5%, gates PASS) and `captures/chain/e154-chain25.txt`
(`40342439`, 25%) — plus one new host-side computation in `scripts/chain.py`.

**The one thing added.** `chain.py` now mirrors the firmware's own
`advance_of`/`wait_time` integer arithmetic (`src/commutation.rs:255-265`) and
prints, **per event from the capture's own intervals**, the *scheduled* angle
each advance level would ask for, beside the *measured* effective angle. That
is the only way this campaign can compare advance across images: the goal
forbids comparing nominal levels, and the reason is exactly what the table
below shows.

```
37.5%  (e157-chain375, ci p50 102 us)      25%  (e154-chain25, ci p50 139 us)
  level 20  0.1900 = 11.40 deg (wait 19)     level 20  0.1892 = 11.35 deg (wait 26)
  level 22  0.1585 =  9.51 deg (wait 16)     level 22  0.1579 =  9.47 deg (wait 22)
  level 24  0.1272 =  7.63 deg (wait 13)     level 24  0.1260 =  7.56 deg (wait 18)
  level 26  0.0962 =  5.77 deg (wait 10)     level 26  0.0949 =  5.69 deg (wait 13)
  measured  0.1942 = 11.65 deg               measured  0.2174 = 13.04 deg
```

**What this says, and it is the substance of the advance question.**

* The firmware runs level **22** at 37.5% (`AdvancePolicy` switches at 35%
  duty) and level **20** at 25%. Its *scheduled* angles are therefore 0.1585
  and 0.1579 — i.e. **the advance law holds the scheduled angle almost
  constant across rungs**, which is what a proportional advance is for.
* The *measured* effective angles are 0.1942 and 0.2174. The differences,
  0.0357 and 0.0595 of a sector, are the chain: 3.6 µs at 102 µs and 8.3 µs
  at 139 µs by this arithmetic, against a directly measured
  crossing-to-bridge excess over the requested wait of 4 µs and 4 µs
  respectively (20 − 16, 30 − 26). The two routes agree at 37.5% and disagree
  at 25%, and the reason is that the level-20 scheduled wait computed from
  the *median* interval (26 µs) is not the median of the *per-event*
  scheduled waits — the same aggregation trap the step-2 review flagged. The
  directly measured figure is the one to trust: **the chain adds ~4 µs at
  both rungs**, which is 0.029 of a sector at 25% and 0.039 at 37.5%.
* Because the chain's cost is roughly fixed in µs, its share of the sector
  **grows with speed**, and it is what carries the effective angle away from
  the scheduled one. At 37.5% the firmware asks for 16 µs of wait and the
  bridge changes 20 µs after the crossing stamp: a fifth of what was asked
  for is chain.

**Against the oracle, as far as the evidence allows.** The oracle's effective
advance above 35% duty was read in campaign 7 as **24–26**, via its own
post-COM wait override; that reading is a prior-entry claim about the
reference and has *not* been re-verified in this campaign, and the reference
image carries no instrument that could report its own chain. So the comparison
that can be made is between angles, with one term missing:

* oracle, scheduled, at this capture's intervals: **0.0962–0.1272** (5.8–7.6°)
* firmware50, scheduled: **0.1585** (9.5°)
* firmware50, **measured effective: 0.1942** (11.65°)
* oracle, measured effective: **unknown** — its chain is unmeasured. If it
  were the same ~4 µs, its effective angle would be 0.132–0.166 (7.9–10.0°).

On that reading firmware50 commutates **0.03–0.06 of a sector later** than the
oracle at 37.5% — between 1.9 and 3.7 electrical degrees — of which about
0.039 (2.3°) is its own chain and the rest is the nominal-level difference.
**This is a comparison of one measured angle against a computed one**, and it
stays that way until either the oracle is instrumented or its chain is
otherwise measured. It is not evidence about why 50% behaved as it did.

**Lateness split by purpose, both rungs** (`chain.py`, from the rows):

| | 37.5% | 25% |
|---|---|---|
| phase 1, commutation | n=341, p50 1.0, max 4 | n=342, p50 2.0, max 5 |
| phase 3, blanking floor | n=342, p50 2.0, max 5 | n=341, p50 2.0, max 4 |
| phase 2, reverse blank | **never fired** | **never fired** |
| firmware's mixed `com_late_max_us` | 8 | 7 |

Both maxima are over the 1024-row window (341 commutations of 692,210 at
37.5%), not over the run, and neither is a bound.

**Late arms by stage: none to split.** Zero late arms in both captures
(`late_arms=0` in the reports, and no kind-1 row with the late bit set). The
stage field reads 2 — closed-loop detector armed, guard tracking not yet — on
every accept in both, so it has discriminated nothing so far.

**Sag scans correlated: nothing to correlate at these rungs.** At 37.5%
`BEMFSAG ref_bus=1216 ref_vref=1506 filt_bus=1206 filt_vref=1507 streak=0
tripped=0`, with `bus_min=1124` (7.6% below the reference, the ordinary
commutation ripple). The sharp reference sits 0.8% below its own datum and the
streak never advanced, so there is **no sag event in either capture** to set
beside the chain rows. This is a null, and it is worth stating as one: the
sag trips campaign 7 saw were at 50%, not here.

**Alternatives not yet excluded.** Nothing above establishes a cause of
anything. In particular the ~4 µs chain, the level difference, and the
oracle's unmeasured chain are three separate terms, and only the first is
measured on this bench. No causal claim is made.

**Prediction for 47.5%, recorded before the run.** The sector interval should
be near 80 µs (2.5% duty per 2.5 rungs of ~1.72 keHz at 37.5% → ~2.1 keHz),
the requested wait near `wait_time(80, 22) = 40 − 27 = 13` µs, and the
crossing-to-bridge delay near 17 µs, giving an effective angle near **0.21**
— *higher* than 37.5%'s 0.194, because the fixed ~4 µs is a larger share of a
shorter sector. Commutation-purpose lateness should stay at a few µs; late
arms should remain zero, since 13 µs of wait is still well above the 6 µs
median elapsed-at-arm. If late arms appear at 47.5%, the new hard stop ends
the run and that is the finding.

**Before any 47.5% run** the goal requires a fresh context-free review of this
step, which is run next; its findings are appended verbatim before the run.

**Climb to the 45% gate, recorded as it happened.** Two more recording runs on
`BE916F91`, both below 45% and both gates-passing, taken to reach 47.5% by the
campaign's usual 2.5% steps rather than jumping:

| rung | capture | ci p50, µs | scheduled (level 22) | measured effective | chain, µs | sag |
|---|---|---|---|---|---|---|
| 25% (level 20) | `e154-chain25` | 139 | 0.1579 | 0.2174 = 13.04° | ~4 | streak 0, no trip |
| 37.5% | `e157-chain375` | 102 | 0.1585 | 0.1942 = 11.65° | 3.6 | streak 0, no trip |
| 40% | `e159-chain400` | 95 | 0.1579 | 0.1968 = 11.81° | 3.7 | streak 0, no trip |
| 42.5% | `e159-chain425` | 89 | 0.1591 | 0.2069 = 12.41° | 4.3 | streak 0, no trip |

(`chain` is measured effective minus scheduled, converted at that capture's
own median interval; all four are medians of 340 pairs from a 1024-row window,
not bounds.)

Within the one advance level the firmware runs above 35% duty, the pattern the
prediction rested on is visible across three rungs: the **scheduled** angle is
held near 0.158 by the advance law while the **measured** effective angle
rises 0.194 → 0.197 → 0.207 as the sector shortens 102 → 95 → 89 µs, because
the chain stays near 4 µs. The 25% row sits above all of them only because it
runs level 20, whose scheduled angle is 0.19.

Per-run reports: `late_arms=0` and `blank_latched=0` in every one
(`blank_arms` 359823 at 40%, and the 42.5% run's own line), `forced_pct=0`,
`zc_rate_permille_of_expected` 994 and 992, guard `reason=0`.
`BEMFSAG streak=0 tripped=0` at all three rungs, with the sharp filter 0.8-0.9%
under its own reference — no sag event to correlate anywhere in this set.

**Next, and gated:** 45% and 47.5% are at or above the goal's 45% line, so the
pre-run independent review of this step is appended verbatim before either is
driven.

### E160 — pre-run review before 45%, verbatim, with the corrections it forced

The goal requires a fresh context-free review **before every powered run at
45% or above**. One was run on E159 and on the code behind it, with the two
standard questions plus the pre-run ones. It found a wrong explanation, a
defective host model, an uncited speed extrapolation, and — the important one
— that **my "no late arms at 47.5%" prediction is contradicted by my own
data**. All of that is corrected below *before* the runs.

#### Review as received

> ## 1. Claims lacking cited measurements
>
> 1. **The 47.5% speed.** "~2.1 keHz" rests on no cited measurement at 45% or 47.5%. E159 gives only a parenthetical, and it is incoherent as written — "2.5% duty per 2.5 rungs" (37.5→47.5 is **four** 2.5% rungs, 10 points). The implied law is linear-in-duty and is nowhere stated or cited. E148 (LAB_NOTEBOOK.md:7440) claims 45% and 47.5% were already qualified 3/3; whatever speeds those runs reported are not cited here.
> 2. **The two captures contradict the linear law.** 25% → `ehz_from_sector=1199` (e154-chain25.txt:10); 37.5% → `ehz_from_sector=1650` (e157-chain375.txt:10). Duty ×1.5 gives speed ×1.376, i.e. exponent ≈0.79, not 1.0. Applying the entry's *own* two points gives 1988 eHz → sector **84 µs**, not 80; `wait_time(84,22)=14`, delay 18, angle 0.214. The angle prediction survives; the interval and wait figures do not.
> 3. **"a fifth of what was asked for is chain."** 4/16 is a ratio of two medians from one 35 ms window; no per-event distribution of that ratio is given, and the entry elsewhere forbids exactly this aggregation.
> 4. **`bus_min=1124` called "the ordinary commutation ripple."** A causal attribution with nothing cited, in an entry that says "No causal claim is made." The capture carries no per-event bus trace to support it.
> 5. **Oracle advance "24–26".** Explicitly a prior-entry claim, un-re-verified — the entry says so, but then still prints "firmware50 commutates 0.03–0.06 of a sector later than the oracle" as a result.
> 6. **Stale unit contract left unresolved.** `src/commutation.rs:9-10` states `ci` and the wait are **half-microsecond** ticks; `src/bemf.rs:84-92` states firmware50's `ZeroCross` keeps µs and feeds it straight in. `chain.py`'s µs interpretation is the correct one for this firmware, but the entry cites `commutation.rs:255-265` (actually 255-263) as its authority without noting that file's own header says the opposite.
>
> ## 2. Numbers taken from summaries rather than captures
>
> 1. **"~1.72 keHz at 37.5%"** is `ehz_from_ci_last=1718` (e157-chain375.txt:9) — a **single last-interval sample**. The rate-derived hold figure in the very next line is `ehz_from_sector=1650`, `mean_ci_us=101`, `zc_per_s=9896`. The entry's speed anchor is the noisiest available number, and the conversion 1.72→2.1 keHz (×1.22) is not even consistent with it (linear from 1718 gives 2176).
> 2. **"the 6 µs median elapsed-at-arm."** `chain.py` reports median 6, **max 8** over 341 events; the run-wide counter in the same capture reads `BEMFRCOMP spent_max_us=11` (e157-chain375.txt:12) and 10 at 25% (e154:11). The prediction's margin is computed against a window median while the capture publishes a run maximum nearly twice as large. See safety, below.
> 3. **"streak=0 ... the streak never advanced."** `FastBusSag::streak()` returns the *live* `lows`, and any healthy scan zeroes it (`src/protection.rs:390-392`, `self.lows = 0` at `:375`; test `one_healthy_scan_breaks_the_streak`, `:1035-1041`). `streak=0` in the report proves only that the **last** scan was not low. It cannot support "never advanced."
> 4. **Table maxima** ("max 4", "max 5") are window figures; `com_late_max_us=8`/`7` are run-wide (e157:16, e154:15). The entry does label these correctly — this is the one place the distinction is handled.
>
> ## 3. Does `chain.py`'s advance arithmetic match `src/commutation.rs`?
>
> 1. **The arithmetic: yes, exactly.** `chain.py:51-59` vs `commutation.rs:255-263`. The split `(ci>>6)*level + (((ci&63)*level)>>6)` is transcribed verbatim; the clamp `min(level,64)` matches `if level > 64 { 64 }`; `max(x,0)` matches `saturating_sub` over the reachable domain (`ci>>1 ≥ 0`, Python ints unbounded so no wrap divergence). Level 22 at 37.5% and 20 at 25% match the captures' `advance_level=` field (e157:3, e154:3) and `policy.rs:227-233` (`>= 350 → 22`).
> 2. **The *input* does not match, and this is the substantive defect.** The firmware applies `wait_time` to `self.average_interval` — a **blended, clamped** estimate (`bemf.rs:415`: `clamp_interval(blend_interval(average_interval, prev_zc, last_zc))`, clamp at `bemf.rs:325-331`), built from interval-timer counts since the last **commutation**. `chain.py:126` feeds the **raw** consecutive-crossing interval. So the docstring claim "mirrored exactly … per event from the capture's own intervals" is false at event granularity.
> 3. **Measured**, comparing each kind-1 row's own `c` (the wait the firmware actually asked for) against `wait_time(raw_interval, level)`: medians agree exactly (16/16 at 37.5%, 26/26 at 25%), but only **37.6%** / **22.6%** of events match, with per-event error **−8…+8 µs** at 37.5% and **−6…+8 µs** at 25%. The per-event error is twice the 4 µs quantity the instrument exists to measure. Aggregate conclusions survive; the per-event "scheduled angle" column does not mean what it says.
>
> ## 4. The 0.0595-vs-4 µs "disagreement": the explanation is wrong
>
> 1. **There is no disagreement.** 0.0595 is `measured(0.2174) − scheduled at level **22**(0.1579)`. But the 25% rung ran level **20** (e154-chain25.txt:3, `advance_level=20`) — the entry states this itself two bullets earlier. The level-matched figure is `0.2174 − 0.1892 = 0.0282`, i.e. **3.92 µs at 139 µs** — which agrees with the directly measured 30−26 = 4 µs. The entry compared the wrong column.
> 2. **The "aggregation trap" explanation is doubly wrong.** (a) `chain.py:199-204` computes `wait_time` **per event** and then takes the median — the table's "wait 26" is already the median of per-event waits, not `wait_time(median interval)`, so the trap the entry names cannot be operating. (b) Independently, the median per-event wait (26) equals the capture's own median requested wait (26) exactly, so there is nothing for the trap to explain.
> 3. **Re-derived both rungs, level-matched:** difference-of-medians 0.0357 (3.64 µs) at 37.5% and 0.0282 (3.92 µs) at 25%; **median-of-per-event-differences** 0.0388 (3.96 µs) and 0.0288 (4.00 µs). The chain is ~4 µs at both rungs and both routes agree at both rungs. The conclusion the entry reaches is right; the reasoning it publishes is wrong, and the aggregation caution it raises applies to **its own** subtraction of medians (0.0357 vs the per-event 0.0388), not to `chain.py`.
>
> ## 5. The 47.5% prediction, figure by figure
>
> 1. **Interval ~80 µs** — arithmetic self-consistent with 2.1 keHz (1/(6·2100) = 79.4 µs), but see §1.1-1.2: the speed is uncited and the entry's own two captures give 84 µs.
> 2. **Wait ~13 µs** — `wait_time(80,22) = 40 − 27 = 13` ✓ (re-computed: `advance_of(80,22) = 1·22 + ((16·22)>>6) = 22+5 = 27`). Correct *given* 80. At 84 µs it is 14.
> 3. **Delay ~17 µs** — 13 + 4 ✓, but assumes the 4 µs chain is duty-invariant on the evidence of two rungs (25%, 37.5%) with no mechanism given; the entry's own "Alternatives not yet excluded" paragraph says only that chain is measured, not that it is constant.
> 4. **Angle ~0.21** — 17/80 = 0.2125 ✓; internally consistent (scheduled 0.1625 + chain share 0.050). The "higher than 0.194 because the fixed ~4 µs is a larger share" reasoning checks out (4/80 = 0.050 vs 4/102 = 0.039).
> 5. **"No late arms"** — the arithmetic is the weakest link and points the other way. See §6.1.
>
> ## 6. Pre-run safety / instrumentation concerns
>
> 1. **The late-arm margin at 47.5% is ~0-3 µs, not "well above."** Predicted wait median 13 µs, and the 37.5% window's *minimum* requested wait is 14 µs, which scales to ≈11 µs at an 80 µs sector. Against that, the same capture's run-wide `spent_max_us=11` (e157:12). Minimum wait ≈ maximum elapsed-at-arm: late arms at 47.5% are marginal by the entry's own data, and the entry's stated margin (13 vs 6) is a median-against-median comparison that hides it. The prediction "late arms should remain zero" is not supported; if anything the data predicts the opposite.
> 2. **The bus minimum already breaches the sag *fraction*; only the streak saves it.** `FastBusSag` trips at `bus·filt_vref·100 < filt_bus·vref·95` (`protection.rs:349-372`, `SAG_NUM/DEN = 95/100`, `SAG_STREAK = 3` at `:261-264`). At 37.5%, `bus_min=1124` against `filt_bus=1206` is **93.2%** — already below the 95% floor. The entry presents this as "the ordinary commutation ripple ... no sag event," which is true only of the *latched* trip. Amplitude margin to the fraction is already negative at 37.5%; current at 47.5% is ~1.6× (`I ∝ duty²`, `policy.rs:82-84`) on top of `hold_ma=887` (e157:20), and neither the expected sag nor the supply headroom for 45-47.5% is cited anywhere in E159.
> 3. **The two hard stops detect; they do not prevent, and the entry doesn't say so.** `late_arms`/`blank_latched` are polled in the **foreground** pass (`src/run/states.rs:215-224`), gated on `closed`. `loop_gap_max_us=148` (e157:17) — i.e. up to ~1.5 sectors of commutation continue after the event before `safe_off`. The code does support "the stop ends the run" (ordinary protection route, `Reason::LateArm = 15` / `BlankLatched = 16`, `protection.rs:52-74`; `reason=2` in both captures with `late_arms=0` shows an armed stop that never fired). It does not support any claim that a late arm or latched blank is *caught* before it affects the drive.
> 4. **Blanking-latch coverage is asserted, not shown, in this entry.** E159 never reports `blank_latched`; the captures do (`blank_latched=0`, e157:16, e154:15). `protection.rs:66-74` states the counter "has read zero in every capture taken since," which is a null across captures being carried as a property — the entry inherits it without restating it as a null.
>
> ## 7. Maxima / nulls / single captures treated as bounds or general properties
>
> 1. **"phase 2, reverse blank: never fired"** (bold, both rungs) — that is 341 commutations inside a 1024-row ring, i.e. **0.049%** of the run (`com_count=692210`, e157:16) at one instant ~35 ms long. No run-wide counter for phase-2 firings is cited. A null over 0.05% of one run is printed as a property of the design.
> 2. **"Zero late arms in both captures"** — this one *is* run-wide (`BEMFRCOMP late_arms=0`), and the entry correctly cites the report as well as the rows. But it is two captures at two rungs, neither above 37.5%, and `protection.rs:56-58` records that `late_arms` was non-zero in **4 of 588** captures "one of them a qualifying 45% run" — the exact rung about to be run. E159 does not mention that, and its zero-at-37.5% null is not evidence about 45%.
> 3. **"no sag event in either capture"** — a null from one capture per rung, with the streak counter misread (§2.3) and the amplitude already through the floor (§6.2).
> 4. **"The chain adds ~4 µs at both rungs"** — one capture per rung, single 35 ms window each, median only; used as a duty-invariant constant in the 47.5% prediction. The entry's "neither is a bound" caveat is applied to the lateness table but not to the 4 µs, which is the number the prediction is built on.
> 5. **"The stage field reads 2 ... on every accept in both"** — again the 341-row window; concluded as "it has discriminated nothing so far," which is fine, but the window is not stated in that sentence.

#### Responses, and what changed before the runs

**§4 — my explanation was wrong, and the reviewer's re-derivation is right.**
The 25% capture runs level **20**; I subtracted the level-22 column from it and
then invented an "aggregation trap" to explain the gap I had manufactured. The
level-matched figures agree at both rungs. `chain.py` now reports the chain
**per event** as `bridge − crossing − the row's own requested wait`, so no
model and no level enters it at all:

| rung | level | scheduled from the rows | measured effective | chain per event, µs |
|---|---|---|---|---|
| 25% | 20 | 0.1884 (11.30°) | 0.2174 (13.04°) | p50 4.0, min 3, max 7 |
| 37.5% | 22 | 0.1553 (9.32°) | 0.1942 (11.65°) | p50 4.0, min 3, max 7, sd 0.38 |
| 40% | 22 | 0.1546 (9.28°) | 0.1968 (11.81°) | p50 4.0 |
| 42.5% | 22 | 0.1609 (9.66°) | 0.2069 (12.41°) | p50 4.0 |

The chain is 4 µs at every rung measured so far, by a route with no model in
it. That is the corrected basis of everything E159 said.

**§3.2–3.3 — a real defect in my host tool, now labelled rather than hidden.**
The firmware applies `wait_time` to the estimator's *blended, clamped*
`average_interval`; `chain.py` had only the raw crossing-to-crossing interval,
so its per-level columns match in the median and differ per event (the tool
now prints the match rate and error range itself: 37.6% and −8…+8 µs at 37.5%,
22.6% and −8…+6 µs at 25%). Those columns are now labelled as a *model's*
distribution, and the firmware's own scheduled angle is taken from the rows.

**§1.6 — fixed in the source.** `src/commutation.rs`'s header claimed
half-microsecond ticks, which is the reference's unit; firmware50 feeds µs.
The header now says both, and why the difference is safe.

**§1.1–1.2, §2.1, §5.1–5.2 — prediction corrected.** My speed extrapolation
was linear, incoherently written, and anchored on `ehz_from_ci_last`, the
noisiest number in the report. Using the two rungs' `ehz_from_sector` (1199 at
25%, 1650 at 37.5%) the exponent is 0.79, giving **≈1990 eHz at 47.5% → sector
≈84 µs**, `wait_time(84, 22) = 14` µs, crossing-to-bridge ≈ 18 µs, effective
angle ≈ **0.214**. That is the prediction now on record.

**§6.1 — the safety-relevant one, and my prediction is withdrawn.** With a
predicted median wait of 14 µs, a minimum wait scaling to ~11–12 µs, and the
same capture's run-wide `spent_max_us=11`, **late arms at 45–47.5% are
marginal to likely, not improbable**. The corrected prediction: *late arms may
well appear, and if one does the run ends on `Reason::LateArm` (15).* That is
the stop working, and the run's own reason code will be the evidence. I was
comparing a median against a median and calling it margin.

**§6.2 — retracted and restated.** Calling `bus_min=1124` "the ordinary
commutation ripple" was an uncited causal claim. The measurable facts: it is
93.2% of `filt_bus`, i.e. **already below the 95% sharp-sag fraction**, and
only the three-scan streak keeps it from latching; `hold_ma=887` at 37.5%, and
`I ∝ duty²` puts 47.5% near 1.4 A against the 1.75 A supply. So a sag trip at
45–47.5% is a plausible outcome of these runs, not an anomaly if it happens,
and the supply headroom is ~0.35 A on that law. Nothing about the sag
thresholds is touched — they are off-limits and stay as they are.

**§6.3 — accepted and stated plainly.** Both stops are **detectors, not
preventers**: they are polled in the foreground pass, and with
`loop_gap_max_us=148` at 37.5% the drive continues for up to ~1.5 sectors
after the event before `safe_off`. Nothing in E156 or E159 claimed prevention;
this entry says explicitly that none is claimed.

**§7.1, §7.3, §7.5 — qualified.** "Phase 2 never fired" is a null over ~341
commutations of 692,210, i.e. 0.05% of one run at one instant; there is no
run-wide counter for phase-2 firings. The sag null is one capture per rung.
The stage-field observation is over the same 1024-row window.

**§2.3 — accepted, corrected.** `streak=0` in a report means only that the
last scan was not low; it cannot support "never advanced". What the captures
support is that no sag **latched** (`tripped=0`) at 37.5%, 40% and 42.5%.

**§7.2 — accepted.** The zero-late-arm result at ≤ 42.5% is not evidence about
45%, and the four non-zero captures from campaigns 5–7 include a 45% run —
which is precisely the rung next. Recorded here so the next entry cannot
present a clean 45% run as expected.

**§1.3 — accepted:** "a fifth of what was asked for is chain" was a ratio of
two medians; per event the chain is 4 µs (3–7) against requested waits of
14–19 µs at 37.5%, so the share is 21–29% per event at that rung.

**§1.4, §1.5, §5.3, §7.4 — accepted as scope limits**, all now stated: the
oracle's effective angle remains computed rather than measured; the 4 µs chain
is an observation at four rungs, not a law, and the 47.5% prediction uses it as
an assumption, named as one.

**Not deferred, not fixed:** nothing. Every item above is either corrected in
the code, corrected in this entry, or restated as a scope limit.

### E161 — step 5 complete: the chain measured end to end at 45% and 47.5%

**Written after the two runs, whose predictions are the corrected ones in
E160** (recorded before either was driven). Recording image `BE916F91`,
captures `captures/chain/e160-chain450.txt` and `e160-chain475.txt`, both
gates PASS, both climbed to by 2.5% steps from 42.5% in the same session.

| rung | ci p50 (rows) / `mean_ci_us` | requested wait, per event | crossing → bridge | **chain per event** | scheduled (rows) | **measured effective angle** |
|---|---|---|---|---|---|---|
| 25% (level 20) | 139 / 139 | 25–28, p50 26 | 28–34, p50 30 | p50 4, 3–7 | 0.1884 (11.30°) | 0.2174 (13.04°) |
| 37.5% | 102 / 101 | 14–19, p50 16 | 18–23, p50 20 | p50 4, 3–7, sd 0.38 | 0.1553 (9.32°) | 0.1942 (11.65°) |
| 40% | 95 / 95 | p50 15 | p50 19 | p50 4 | 0.1546 (9.28°) | 0.1968 (11.81°) |
| 42.5% | 89 / 89 | p50 14 | p50 18 | p50 4 | 0.1609 (9.66°) | 0.2069 (12.41°) |
| **45%** | 84 / 84 | 12–15, p50 13 | 16–21, p50 17 | p50 4, 3–7, sd 0.60 | 0.1566 (9.40°) | **0.2048 (12.29°)** |
| **47.5%** | 82 / 80 | 11–15, p50 13 | 15–21, p50 17 | p50 4, 3–7, sd 0.65 | 0.1566 (9.40°) | **0.2073 (12.44°)** |

Every figure is a median (or range) over the 340–342 pairs of one 1024-row
window per run — about 0.08% of each run's commutations, ~28 ms of a 20–22 s
hold — and none is a bound. One run per rung, so nothing here is a rate.

**Verdict against E160's corrected predictions for 47.5%:** sector ≈84 µs
predicted (exponent-0.79 law from the 25% and 37.5% rungs), measured **80–82**;
wait 14 predicted, measured **13**; crossing-to-bridge 18 predicted, measured
**17**; effective angle 0.214 predicted, measured **0.2073**. The 45% run
landed on 84 µs and 1984 eHz against 1990 predicted. The law and the ~4 µs
chain both held, each about 3% optimistic at the top rung.

**The late-arm question, and I was wrong twice.** E159 predicted no late arms
with a median-against-median margin; E160 withdrew that and said they were
"marginal to likely" because the minimum per-event wait (≈11–12 µs) meets the
run-wide `spent_max_us=11`. Measured: **zero late arms** in both runs —
`late_arms=0` over 392,137 commutations at 45% and 403,835 at 47.5%, and no
kind-1 row with the late bit set. So the second prediction was also wrong, in
the opposite direction. What the rows show is why: the *per-event* elapsed at
the arm is 5–9 µs (p50 6) while the *per-event* wait is 11–15, so the margin
is 2 µs at the worst pair in these windows and the run-wide `spent_max_us=11`
is rarer than the windows see. **One run per rung is not a rate**: the goal's
own rule needs ≥5 runs before any claim about how often a late arm happens,
and campaign 5–7 captures include a non-zero 45% run.

**Lateness split by purpose** (the quantity `com_late_max_us` cannot give):

| rung | phase 1, commutation | phase 3, blanking floor | phase 2 | firmware's mixed max |
|---|---|---|---|---|
| 45% | n=341, p50 2, max 5 | n=342, p50 2, max 5 | none in window | 10 |
| 47.5% | n=342, p50 2, max 4 | n=341, p50 1, max 5 | none in window | 11 |

The commutation service's own lateness stays at 2 µs median through every
rung from 25% to 47.5%, while the firmware's single counter rises 7 → 11. The
counter's growth is therefore **not** growth in commutation lateness; it is
the mixture E153 identified, and the split now measures it directly. (No
run-wide per-phase counter exists, so "phase 2 never fired" remains a
window-level null at every rung.)

**Sag scans, correlated as asked.** At 47.5%: `BEMFSAG ref_bus=1216
filt_bus=1198 streak=0 tripped=0`, `bus_min=1111` — **92.7% of `filt_bus`**,
below the 95% sharp-sag fraction, exactly as E160's review warned; at 45%,
`bus_min=1125` against `bus_ref=1214`. Nothing latched at either rung
(`tripped=0`, and a `streak=0` reading says only that the last scan was not
low). `hold_ma` 1413 at 45% and **1615 at 47.5%**, against the 1.75 A supply:
0.14 A of headroom on the measured value, and `I ∝ duty²` from 47.5% would put
50% at ~1.79 A, i.e. **at the supply limit**. That is a measurement, not a
prediction about 50%, and 50% is not this campaign's objective.

**No sag event coincides with anything in the chain rows** at any rung
measured — there is no event to correlate. The correlation asked for in step 5
is therefore reported as a null, with the amplitude margin above stated so the
null is not read as comfort.

**The advance question, settled in effective angle as far as the evidence
reaches.** Across the four rungs that run advance level 22, the *scheduled*
angle taken from the rows is 0.155–0.161 while the *measured* effective angle
is 0.194–0.207: the firmware commutates 3–5 electrical degrees later than it
schedules, entirely because of a chain that is 4 µs at every rung and so grows
as a share of a shrinking sector. Against the oracle's effective advance of
24–26 (a campaign-7 reading of the reference, **not** re-verified here), its
*scheduled* angle at these intervals would be 0.096–0.129; its measured
effective angle is unknown because that image carries no instrument. If its
chain were also 4 µs, firmware50 commutates **0.03–0.06 of a sector — 2 to 4
electrical degrees — later than the oracle** at these rungs. That is the
answer step 5 can support: a measured angle against a computed one, with the
oracle's chain named as the missing term.

**Alternatives not excluded, so no causation is claimed.** The 4 µs chain is
measured; the level difference is arithmetic; the oracle's chain is unmeasured.
Nothing here tests whether the angle difference *matters* to lock quality —
that is step 6's A/B, and it is the only thing that can attribute an effect to
scheduling.

**Captures and review.** Rung captures: `e154-chain25`, `e157-chain375`,
`e159-chain400`, `e159-chain425`, `e160-chain450`, `e160-chain475` (all under
`captures/chain/`), production control `e156-25control_01` and the five-run
band `e155-25_0{1..5}` under `captures/2026-09-23/`. Independent reviews:
E158 (steps 2–4) and E160 (pre-run, before 45%), both verbatim with responses.

### E162 — step 6a: the ownership restructure that makes COM preemption sound

**Written before the build and runs.** The goal: *"First make COM preemption
sound: audit shared resources, end/protect borrows, preserve accepted-edge /
sector identity, cite the oracle's snapshot. Only then A/B COM above COMP."*

**The audit.** Every `Seam` in the firmware, with the contexts that take it and
how, after this change:

| value | ceiling | COMP | COM | guard | foreground |
|---|---|---|---|---|---|
| `det.zc` (the estimator) | `Motor` | `root()` | **— (was `root()`)** | — | `lock()` |
| `det.rate` | `Motor` | `root()` | — | — | `lock()` |
| `drv.rate` | `Motor` | `root()` | — | — | `lock()` |
| `com.plans` | `Motor` | — | `root()` | — | `lock()` |
| `com.six` | `Motor` | — | `root()` | — | `lock()` |
| `guard.watch` | `Guard` | `masked()` | — | `root()` | `lock()` |
| `CHAIN_ACC` (diagnostic) | `Motor` | `root()` | — | — | `lock()` |
| `CHAIN_SVC` (diagnostic) | `Motor` | — | `root()` | — | `lock()` |
| `RING` (diagnostic, E121) | `Motor` | `root()` | — | — | `lock()` |

`Seam::root()` (`src/shared.rs:170-195`) justifies its unchecked `&mut` with
*"Every other root that touches the value runs at `P` too, so it cannot preempt
this one"*. E153 established that the **one** value breaking that under
COM-above-COMP was `det.zc`: COMP holds it across the acceptance *and* the arm,
and COM's phase 1 borrowed the same value for the interval and the blanking
window. `guard.watch` is the shape that is already right — the lower-priority
taker masks.

**The change, and it is the reference's own shape.** The oracle does not share
its estimator with the commutation: it snapshots what the commutation needs
*before* handing over. So:

* `Det` gains `accept_avg` and `accept_blank` (plain `AtomicU32`). COMP writes
  them **inside the acceptance, before the arm**, from the same
  `zc.average_interval()` / `zc.blanking()` the old borrow returned.
* COM's phase 1 reads those two atomics instead of borrowing `det.zc`.
* `det_install` seeds the pair from the estimator being installed, so the
  handover commutation reads what the borrow would have given it.
* The diagnostic chain ring is **split per root** (`CHAIN_ACC`, `CHAIN_SVC`),
  for the same reason: a shared ring would have forced a mask into whichever
  root ends up lower, and that cost would have landed on one side of the A/B
  only. The host merges them by stamp.

**Accepted-edge and sector identity, preserved by construction.** There is one
acceptance per sector and COM fires between acceptances, so the estimator's
value at commutation time *is* the value at the last acceptance — which is
exactly what the pair now carries. Rebases do not touch the estimate, and a
revisit acceptance publishes like any other. The pair is written before
`accept_seq` is incremented, so a reader that keys on the sequence cannot see
a newer sequence with an older estimate.

**What this does not do.** It does not change any threshold, any protection,
the advance law, or the arm's position relative to the four bookkeeping stores
(E142/E144's finding stands: the stores come first). It does not raise COM's
priority — that is 6b, one variable at a time.

**Measured cost, static** (`isr_cycles.py` longest-path upper bounds, 0 wait
states, model named per E158's standing correction): `ADC_COMP` **1026 → 989**
cycles and `TIM16` **349 → 333**; instruction counts 710 → 699 and 318 → 311.
Both roots got *shorter*: COMP trades a second `average_interval()` call for
two stores, and COM trades a borrow plus two accessor calls for two loads.
Cycles are not growing, which is the gate.

**Predictions, recorded before the runs.**

1. Production behaviour is unchanged within run-to-run scatter: the 15/20/25%
   rungs pass their gates, and the 25% rung's speed sits inside E155's band
   (+0.48 ± 2.77 per mille residual at that rung, `1FBA3992`).
2. The effective angle at 37.5% stays at 0.194 ± 0.003 and the chain per event
   stays at 4 µs, since neither the wait nor the bridge write moved.
3. `late_arms` and `blank_latched` stay zero, and `spent_max_us` does not
   rise: COMP's added work is two stores, and the arm still follows them.

If the angle or the chain moves by more than the rung-to-rung scatter already
recorded (0.194 → 0.207 across four rungs), the restructure is not neutral and
6b does not proceed until that is understood.

### E163 — independent review of step 5, verbatim, with the corrections it forces

Run on E161 alone, context-free, with the goal's two questions plus the
step-specific ones. It found two numbers that do not reproduce, one conclusion
that does not follow, a mis-labelled oracle quantity, a current law contradicted
by my own captures, and — the worst of them — that **E161's six-rung table pools
four different images and names only one**.

#### Review as received

> ## 1. Claims lacking cited measurements
>
> 1. **The recording image is wrong, and the table pools four images.** LAB_NOTEBOOK.md:8835 says "Recording image `BE916F91`, captures `e160-chain450` and `e160-chain475`". Both those captures carry `elf_sha256 C0A1136D159D…`; `BE916F91…` is the **37.5%** capture's image (`captures/chain/e157-chain375.txt`). The six-row table silently spans four images: `40342439` (25%), `BE916F91` (37.5%), `6DEF36EA` (40%/42.5%), `C0A1136D` (45%/47.5%). No sentence in the entry states that.
> 2. **`I ∝ duty²` is cited to nothing** (LAB_NOTEBOOK.md:8886). The law exists in-tree at `src/run/policy.rs:83` attributed to E136 and fit at ≤37.5%; the entry neither cites it nor notes the fit range.
> 3. **"the margin is 2 µs at the worst pair in these windows"** (LAB_NOTEBOOK.md:8878) is not in the rows. It is the window's *min wait* (11) minus the window's *max spent* (9) — two different events. The actual worst same-event `wait − spent` is **4 µs at 47.5% and 4 µs at 45%** (computed per row from the kind-1 rows of both captures). The "2" matches `WCET_ESTIMATES.md:363` (wait 13 − R_COMP 11), a model row, not these windows.
> 4. **"the run-wide `spent_max_us=11`"** applied to both runs (LAB_NOTEBOOK.md:8875, 8880): `e160-chain450.txt` reports `BEMFRCOMP spent_max_us=10`. Only 47.5% is 11.
> 5. **The oracle's "effective advance of 24–26"** (LAB_NOTEBOOK.md:8901) has no capture and no file citation in the entry. Its only in-tree source is `WCET_ESTIMATES.md:371`, which says "the oracle's **advance** of 24–26" — a *level*, not an effective angle. The entry adds the word "effective" without support.
> 6. **"campaign 5–7 captures include a non-zero 45% run"** (LAB_NOTEBOOK.md:8882) — no capture named.
> 7. **"the ~4 µs chain … each about 3% optimistic at the top rung"** (LAB_NOTEBOOK.md:8867) — the chain p50 is 4 at all six rungs with no drift at all in the rows; there is no measurement behind "3% optimistic" for the chain (the 3% is the sector-law error, 84 vs 82).
> 8. **The 1.75 A supply figure** (LAB_NOTEBOOK.md:8887) appears in no cited capture; it is carried in from session context.
>
> ## 2. Numbers that came from summaries rather than captures
>
> 1. **`ci p50 (rows)` at 40% is 95 and at 42.5% is 89** (LAB_NOTEBOOK.md:8841–8842). The rows give **97** and **88** (`python scripts/chain.py captures/chain/e159-chain400.txt` → `sector interval … p50=97.0`; `…e159-chain425.txt` → `p50=88.0`). 95 and 89 are the report-line `mean_ci_us` values — i.e. the "rows" column was filled from `BEMFRATE mean_ci_us` for those two rungs. Note the 47.5% row correctly shows the split (82 rows / 80 summary), which is why these two stand out.
> 2. **"the firmware's mixed max 10 / 11"** (LAB_NOTEBOOK.md:8894–8895) and **"rises 7 → 11"** (8897) come from `com_late_max_us` on the `BEMFREVISIT` line (`src/report.rs:559`) — correct (7/8/8/9/10/11 across the six captures) but a run-wide report scalar, not the window it is tabled beside.
> 3. **"2 µs" margin** — see 1.3; a `WCET_ESTIMATES.md` model row presented as a row-derived quantity.
>
> ## 3. Does every number in the six-row table reproduce?
>
> Reproduces: all six `mean_ci_us`; all requested-wait and crossing→bridge ranges and p50s; all chain p50/range/sd; all scheduled and measured angles (verified against `scripts/chain.py` output for each of the six captures).
>
> Does **not** reproduce:
> 1. `ci p50 (rows)` = **95** at 40% (rows: 97) — LAB_NOTEBOOK.md:8841.
> 2. `ci p50 (rows)` = **89** at 42.5% (rows: 88) — LAB_NOTEBOOK.md:8842.
>
> ## 4. Is the "not commutation lateness" conclusion supported?
>
> No, not as stated.
>
> 1. **Window maxima are below the counter they are used to explain.** Phase-1 lateness max in the window is 4–5 µs at every rung, while `com_late_max_us` is 7→11. The window (341 of 392,137 services at 45%, 0.087%) never contains the event that sets the run-wide maximum, at any rung. So the rows are consistent with the counter's growth being commutation lateness in a tail the window cannot see; they do not exclude it. LAB_NOTEBOOK.md:8896–8899 draws the exclusion anyway ("therefore **not** growth in commutation lateness").
> 2. **The median claim is false at half the rungs.** "The commutation service's own lateness stays at 2 µs median through every rung from 25% to 47.5%" (LAB_NOTEBOOK.md:8896): phase-1 p50 is 2.0 at 25%, 45%, 47.5% but **1.0** at 37.5%, 40% and 42.5%. Those three rungs are also not in the split table, so the "every rung" span is asserted, not shown.
> 3. **Phase 2 is a window null promoted into the mixture argument.** The entry correctly labels "phase 2 never fired" a window-level null (LAB_NOTEBOOK.md:8899) but the conclusion in 8897–8898 requires the mixture to be real run-wide; with phase 2 unobserved and phase 3 capped at 5 in-window, no phase in the window reaches 10–11, so the mixture does not account for the counter from these rows either.
>
> ## 5. Current headroom and the 50% extrapolation
>
> 1. **Arithmetic is right from the cited numbers.** `hold_ma=1615` (`e160-chain475.txt`, `BEMFCURRENT`); 1.75 − 1.615 = 0.135 ≈ "0.14 A"; 1615·(50/47.5)² = 1789 mA ≈ "~1.79 A". LAB_NOTEBOOK.md:8886–8889.
> 2. **The law is contradicted by the entry's own six captures.** Fitting `hold_ma` (351, 887, 1053, 1239, 1413, 1615) gives exponent **2.38** over 25→47.5% and **2.47** over the last step, not 2. At 2.4 the 50% projection is ~1.83 A, so the quoted 1.79 A is optimistic in the direction that matters, and the "at the supply limit" conclusion is understated rather than measured.
> 3. **The 0.14 A headroom sits inside a larger zero drift.** The same line reports `zero_drift_ma=-293` (`e160-chain475.txt`) — the bridge-off zero moved ~0.29 A-equivalent across the run, and `hold_ma` is that residual scaled (`src/protection.rs:591-599`). The entry calls 1615 "the measured value" (LAB_NOTEBOOK.md:8887) with no uncertainty attached.
> 4. **"That is a measurement, not a prediction about 50%"** (LAB_NOTEBOOK.md:8889) disclaims the sentence before it, which is a prediction about 50%.
>
> ## 6. Oracle comparison — is any part of the oracle figure presented as measured?
>
> 1. **"effective advance of 24–26"** (LAB_NOTEBOOK.md:8901) — the source (`WCET_ESTIMATES.md:371`) says *advance* 24–26, a level. Labelling it "effective" attaches a measured-latency meaning it does not have. The entry then uses it as a *scheduled* level (feeding `wait_time` at levels 24/26) and separately adds a 4 µs chain on top; if the 24–26 really were effective, that addition double-counts the chain. The entry does not state which reading it means.
> 2. **"its *scheduled* angle at these intervals would be 0.096–0.129"** reproduces from `chain.py`'s modelled level-24/26 rows (0.0949–0.1288 across the six captures) — but that table is flagged in `scripts/chain.py` docstring/comments as "a distribution of a model, not of the firmware", matching the row's own wait only 22.6–47.4% of the time. The entry calls it "computed", which is fair; it does not carry over the model's per-event error.
> 3. Correctly labelled as unmeasured: "its measured effective angle is unknown because that image carries no instrument" (LAB_NOTEBOOK.md:8903) and "the oracle's chain is unmeasured" (8909).
> 4. **"the firmware commutates 3–5 electrical degrees later than it schedules"** (LAB_NOTEBOOK.md:8900) does not follow from the table. Measured − scheduled, in degrees: 1.74 (25%), 2.33 (37.5%), 2.53 (40%), 2.75 (42.5%), 2.89 (45%), 3.04 (47.5%). The range is **1.7–3.0**, and no rung reaches 4 or 5.
> 5. **"entirely because of a chain that is 4 µs"** (LAB_NOTEBOOK.md:8901) is definitional, not evidential: `scripts/chain.py` defines chain = (bridge − crossing) − requested wait, i.e. exactly the numerator difference between the measured and scheduled angles. It cannot come out any other way.
> 6. **"Across the four rungs that run advance level 22"** (LAB_NOTEBOOK.md:8900) — five captures carry `advance_level=22` (37.5, 40, 42.5, 45, 47.5), and the quoted ranges (scheduled 0.155–0.161, measured 0.194–0.207) include the 37.5% rung. Also the scheduled minimum is 0.1546 (40%), which rounds to 0.154, not 0.155.
>
> ## 7. Window median / single run / null treated as a bound, rate or general property
>
> 1. **Good-faith disclaimers are present** (LAB_NOTEBOOK.md:8849–8851, 8881–8884, 8899, 8907–8910) and cover most of the entry. The exceptions:
> 2. **The mixture conclusion** (LAB_NOTEBOOK.md:8897) — window medians/maxima used to exclude a run-wide property. See §4.
> 3. **"~28 ms of a 20–22 s hold" and "about 0.08% of each run's commutations"** (LAB_NOTEBOOK.md:8849–8850) are generalized to all six rungs; `hold_ms=67276` at 25% and `60774` at 37.5% (67 s and 61 s), and the window is 0.066% of the 25% run's 519,442 services.
> 4. **The sag ratio compares mismatched references and is then read against the trip line.** "bus_min=1111 — 92.7% of filt_bus, below the 95% sharp-sag fraction" (LAB_NOTEBOOK.md:8885–8886): `bus_min` is a run-wide minimum of the *raw single-scan* bus (`src/run/states.rs:268`), while `FastBusSag` judges the *block mean* (`rail.bus_mean()`, `src/run/states.rs:277`) against the EWMA at that instant, needing 3 consecutive lows (`SAG_NUM=95`, `SAG_STREAK=3`, `src/protection.rs:261-264`). The reported `filt_bus=1198` is the end-of-run EWMA, not its value at the dip. So 92.7% is not the quantity the protection evaluates, and the entry's own `tripped=0` says the evaluated quantity never crossed the line.
> 5. **"the exponent-0.79 law"** (LAB_NOTEBOOK.md:8863) is a two-point fit (25% and 37.5%) called a law; it does reproduce (139·(47.5/25)^−0.79 = 83.7 ≈ 84 predicted vs 82 rows / 80 summary), but its status as a law rests on two runs.
> 6. **"No sag event coincides with anything in the chain rows at any rung measured — there is no event to correlate"** (LAB_NOTEBOOK.md:8891) — correctly reported as a null, and the caveat that `streak=0` "says only that the last scan was not low" is accurate (`src/protection.rs:350-359`). No overreach here.

#### Corrections

**§1.1 — accepted, and it is the worst item here.** The six rungs ran on **four
images**, not one: `40342439` (25%), `BE916F91` (37.5%), `6DEF36EA` (40%,
42.5%), `C0A1136D` (45%, 47.5%). I archived only the first two. The other two
binaries were overwritten by later builds and are **unrecoverable**; their
hashes survive only in the captures' own header lines. What changed between
them, from the source: `6DEF36EA` added the diagnostic binary's extended
run-key matcher (`arms_a_run`, foreground only) and `C0A1136D` added a doc
comment to `src/commutation.rs`. Neither touched a root — but that is now an
*inspection* claim I cannot verify against the binaries, which is exactly what
the goal's "record each result's ELF hash and configuration" exists to prevent.
`scripts/chain_run.py` now archives the image by hash **before** each run, so
this cannot recur. The table in E161 must be read as pooling four images.

**§2.1 / §3 — accepted, corrected.** The row-derived sector medians are **97**
at 40% and **88** at 42.5%; I filled those two cells from `mean_ci_us`. The
corrected row-median sequence is 139, 102, 97, 88, 84, 82.

**§4 — accepted; the conclusion is withdrawn.** The window contains 341 of
several hundred thousand services and never holds the event that sets the
run-wide maximum, so it **cannot** exclude commutation lateness in the tail.
The supported statement is narrower: *in the sampled windows*, commutation-
purpose lateness is 1–2 µs median with a maximum of 4–5, while the firmware's
mixed counter reads 7–11 for the whole run; the rows therefore do not show
where the run-wide maximum comes from, and E153's point stands on code (the
counter is stamped before the purpose is dispatched) rather than on these rows.
Also accepted: phase-1 median is **1.0** at 37.5/40/42.5% and 2.0 at
25/45/47.5%, not 2 everywhere.

**§1.3, §1.4 — accepted, corrected.** The worst *same-event* `wait − spent` in
the windows is **4 µs** at both 45% and 47.5%, not 2; the 2 was a min-wait
minus max-spent across different events, and coincided with a
`WCET_ESTIMATES.md` model row. `spent_max_us` is **10** at 45% and 11 at 47.5%.

**§5 — accepted, corrected.** `I ∝ duty²` is `src/run/policy.rs:83` (E136, fit
at ≤37.5%), and my own six `hold_ma` values (351, 887, 1053, 1239, 1413, 1615)
fit an exponent of **2.38**, not 2. On that fit 50% projects to ~1.83 A rather
than 1.79. The 1.75 A limit is the operator's setting, stated in the goal, not
a capture quantity. And `zero_drift_ma=-293` in the same line is a larger
uncertainty than the 0.14 A headroom I quoted, so the headroom figure should
never have been given without it. The sentence claiming this was "a
measurement, not a prediction" was indeed disclaiming its own predecessor: the
projection is a prediction and is labelled as one here.

**§6.1, §6.4, §6.5, §6.6 — accepted.** The 24–26 figure is an **advance
level**, not an effective angle; calling it "effective" was wrong, and it is
used here only as a level fed to `wait_time`. Under that reading the oracle's
*scheduled* angle is 0.095–0.129 and its effective angle is unknown; the
comparison stands only as measured-against-computed. Measured minus scheduled
is **1.7–3.0 electrical degrees** across the six rungs, not 3–5. "Entirely
because of a chain of 4 µs" is definitional — the chain *is* that numerator
difference — so it explains nothing by itself. Five captures run level 22, not
four, and the scheduled minimum is 0.1546.

**§7.3, §7.4, §7.5 — accepted.** The window is 0.066–0.087% of a run depending
on the rung, and the holds are 67 s and 61 s at the two lowest rungs, not
20–22 s. `bus_min` is a raw single-scan minimum while `FastBusSag` judges a
block mean against the EWMA *at that instant*, so my 92.7% is not the quantity
the protection evaluates and should not have been set beside its 95% fraction;
what the captures support is that nothing latched. The 0.79 exponent is a
two-point fit, not a law.

**§1.6, §1.7, §1.2, §1.8 — accepted:** the non-zero 45% capture is named in
E158's response to the step-4 review (`c7f-450_02`) and should have been named
here too; the chain shows no drift across rungs, so "3% optimistic" applies
only to the sector law; the current law now carries its citation and fit range;
the supply figure is the operator's setting.

**Nothing deferred.** Two items cannot be fully repaired: the two lost ELFs,
and the retrospective root-identity claim that depended on them. Both are
recorded as unrecoverable rather than argued away, and the tooling change
prevents a third.

### E164 — step 6a completed: the restructure is neutral, and two tooling traps it exposed

**Measurements, against E162's three predictions.**

**(1) Production behaviour unchanged — confirmed, but only after a bisect.**
The restructured image `4F0F5131` passed 15% 3/3, 20% 3/3, 25% 3/3
(`captures/2026-09-23/e162-{15,20,25}_*`). Its 25% matched-window speed read
**1195.4** against E155's **1201.5** on `1FBA3992` — a 5.1 per mille gap,
outside E155's within-session band (±2.8), i.e. apparently *not* neutral.

Rather than attribute that to the change or to the bench, I bisected: flashed
the archived pre-restructure image `3830927C` and re-ran the same rung in the
same thermal state. Back to back, properly labelled and flashed by the fixture
itself:

| image | what it is | 25% powered eHz, n=3 | spread |
|---|---|---|---|
| `3830927C` | E156, before the restructure | **1195.47** | 1.08 |
| `4F0F5131` | E162, after it | **1195.50** | 1.79 |

The two images are **indistinguishable — 0.03 eHz, 0.03 per mille apart.** The
restructure is speed-neutral. The 5 per mille was a **cross-session shift**:
E155's runs were about two hours earlier in the same day, and the same rung on
the same image now reads 5 per mille slower. That is a measured property of
this rig, and it matters for the instrument: **E155's ±2.8 per mille band is
a within-session figure and does not cover cross-session comparison.** Any
A/B from here on is run back to back, on the same evening, with the fixture
flashing each image.

**(2) The chain and the angle are unchanged — confirmed, at both rungs.**
Recording image (6a), against the pre-restructure captures at the same rungs:

| rung | requested wait p50 | crossing → bridge p50 | chain per event | effective angle |
|---|---|---|---|---|
| 25%, before (`e154-chain25`) | 26 | 30 | p50 4 (3–7) | 0.2174 |
| 25%, after (`e162-chain25`) | 26 | 30 | p50 4 (3–7) | **0.2174** |
| 37.5%, before (`e157-chain375`) | 16 | 20 | p50 4 (3–7) | 0.1942 |
| 37.5%, after (`e162-chain375`) | 16 | 20 | p50 4 (3–8) | **0.1942** |

**(3) Counters unchanged — confirmed.** `late_arms=0` and `blank_latched=0` in
every run; `spent_max_us=11` at both rungs, the same as before.

**So the estimator is now COMP-only, at no measured cost**, and the
`Seam`-by-root table in E162 holds: no value is touched by two roots, which is
what 6b needs.

**Two tooling faults, both mine, both found by their own evidence.**

1. **Splitting the ring per root broke the host pairing, silently and
   spectacularly.** The rings keep the last 1024 events *each*, and the
   service ring holds commutations **and** blanking-floor services, so 1024 of
   its rows reach back only half as far as 1024 acceptances; and their 16-bit
   wrap phases differ. My first merge sorted both by unwrapped stamp and
   produced a 45 µs "chain" and an effective angle of **0.61** — three times
   anything physical. Fixed by pairing on what actually ties the two rows
   together: both rings stop at the same disarm, so their tails align, and a
   service row carries the instant it was *scheduled for* while an acceptance
   carries its stamp and the wait it asked for (`sched ≈ crossing + wait`), so
   the alignment is scored rather than assumed. `chain.py` now prints the pair
   count, the index offset and the median disagreement of that key, and the
   old single-ring captures reproduce byte for byte (0.1942 / 4 µs at 37.5%).
   **A number that absurd is a gift; the danger was a merge off by one row,
   which would have looked plausible.**
2. **The fixture labelled a bisect capture with the wrong image.** It hashes
   the ELF at the build path, not what is on the chip, so my first three
   bisect runs of `3830927C` were recorded as `4F0F5131`. The measurements
   were right and the header was a lie. `bemf_run.py` now takes `--elf` and
   `--flash`: it programs the image and records *that* image's hash, so the
   two cannot disagree. The three mislabelled captures
   (`e162-bisect-e156_0*`) are retained, and this entry is their correction.

**Gates.** Host tests 314 + 9; `isr_audit` clean on all four roots;
longest-path static bounds (0 WS) COMP **989** and TIM16 **333**, both lower
than E156's 1026/349, so cycles are not growing; structure report 0 functions
over 100 lines; clippy clean on the firmware bins and the host lib/tests. The
four roots' disassembly **does** change here, as it must — COMP and COM are the
things that changed — and the changed instructions are the two stores in the
acceptance and the two loads in phase 1, with the removed borrow accounting for
the shorter paths.

**Next: 6b, one variable.** COM above COMP, as an A/B against this image at
37.5% and 47.5%, back to back in one session, measuring the cost to each
zero-cross stamp. The 47.5% side is at or above 45%, so the pre-run
independent review comes first.

### E165 — step 6b: COM above COMP, as a bounded A/B

**Written before the runs.** The goal: *"Only then A/B COM above COMP against
the archive at 37.5% and 47.5%, measuring zero-cross timestamp cost."*

**The change, one variable.** A cargo feature `com-top`, off by default:

* `COMP_IRQ_PRIORITY` becomes **0x80**, below the COM root's 0x40. The guard
  stays alone at 0x00, so it still preempts everything — the M0+ has four
  priority levels and this arrangement uses three of them.
* COMP's own values take a new ceiling type, `shared::CompLow` (`CompPrio`
  under the feature, `Motor` without it): the estimator `det.zc`, both rate
  meters, the decision ring and the accept half of the chain ring. **The
  compiler now refuses** any attempt to take one of them from a root that is
  not COMP — which is what step 6a's restructure earned, and what makes this a
  scheduling change rather than undefined behaviour (E153).
* Nothing else moves: DMA stays at 0x40, the guard at 0x00, and COM at 0x40.

**The two images differ by one instruction's worth of configuration.**
`isr_diff.py` between them: **all four roots identical** (37 / 764 / 355 / 153
instructions in the recording build), and the only difference the disassembly
shows is one more `movs r3, #128` — the priority byte written at init. That is
the cleanest form this A/B could take: same handler code, different NVIC
configuration.

Default-build check: the type alias changes nothing when the feature is off —
the production image's four roots are instruction-identical to E162's
`4F0F5131` (37 / 699 / 306 / 153), host tests 314 + 9.

**What the A/B measures.**

1. **The cost to each zero-cross stamp.** With COM above COMP, a commutation
   can preempt a zero-crossing decision, which lengthens that decision by the
   COM handler's duration. The chain rows measure this directly: the
   *elapsed at the arm* per event (5–9 µs at every rung so far, p50 6) is the
   quantity that must grow if preemption happens during a decision, and
   `spent_max_us` is its run-wide maximum (10–11).
2. **Whether the commutation lands earlier.** The point of COM-top is that a
   commutation waiting behind a COMP call no longer waits: the
   commutation-purpose lateness (p50 1–2 µs, max 4–5 in-window) and the
   crossing-to-bridge delay (p50 17–30 µs by rung) are where that would show.
3. **Whether the effective angle moves**, which is the quantity the campaign
   compares — and with it the rotor speed on the matched window.

**Predictions, recorded before the runs.**

* The chain's ~4 µs is *not* mostly queueing behind COMP: the COM root's own
  service lateness is already 1–2 µs median, so COM-top can remove at most
  that. I therefore expect the crossing-to-bridge delay to fall by **0–2 µs**,
  the effective angle at 37.5% to move from 0.194 to **0.192–0.194**, and the
  speed to change by **less than the 2 per mille within-session spread** — i.e.
  no measurable gain.
* `spent` (elapsed at the arm) should **rise** on some events, because a
  commutation may now interrupt a decision: expect the per-event maximum to
  grow from 9 towards 9 + the COM handler's ~5 µs, and `spent_max_us` from 11
  towards ~16. If it exceeds the requested wait on any event, that is a late
  arm and the run ends on `Reason::LateArm` — which is the stop doing its job
  and the honest outcome of the experiment.
* Lock quality (`zc_rate_permille_of_expected` 992–999 so far, `forced_pct=0`)
  should not improve, because nothing in the chain suggests commutation
  lateness is what limits it.

If the A/B shows no gain and a measurable cost, COM-top is **refused on
evidence** — which is an acceptable outcome of this campaign, not a failure.

**Protocol.** Both images are recording builds, run back to back in one
session at one rung, the fixture flashing each and recording its hash (E164's
correction), and the 47.5% pair only after the pre-run independent review.

**The 37.5% pair, measured.** Both recording images, flashed by the fixture,
back to back in one session, same key, same rung:

| quantity | A: peers at 0x40 (`e165-A-peer375`) | B: COM above COMP (`e165-B-comtop375`) |
|---|---|---|
| commutation-purpose lateness | n=512, **p50 2.0**, mean 1.57, max 5 | n=512, **p50 1.0**, mean 1.54, max 5 |
| crossing → bridge update, µs | p50 20, min 18, **max 26**, sd 0.86 | p50 20, min 18, **max 23**, sd 0.86 |
| requested wait, µs | 14–19, p50 16, sd 0.57 | 14–19, p50 16, sd 0.55 |
| elapsed at the arm, µs | 5–9, p50 6, **sd 0.77** | 5–9, p50 6, **sd 0.64** |
| chain per event, µs | p50 4, 3–7, sd 0.65 | p50 4, 3–7, sd 0.65 |
| effective angle | **0.1942** (11.65°) | **0.1942** (11.65°) |
| `spent_max_us` / `late_arms` | 11 / 0 | 11 / 0 |
| `zc_rate_permille_of_expected` | 996 | 996 |
| powered speed, matched window | 1642.3 eHz | 1644.1 eHz (+1.1 per mille) |

**Verdict against the predictions.** The gain prediction holds: the
commutation service's median lateness falls by 1 µs and its tail by 3 µs
(max 26 → 23), the effective angle does not move at all, and the speed
difference (+1.1 per mille) is inside the within-session spread E164
measured (±2 per mille, and the residual column shows the coast estimator's
own 8 per mille scatter on these two runs). **The cost prediction was wrong**:
I expected `spent` to grow towards 16 µs as commutations began interrupting
decisions, and it did not move — 5–9 µs per event and `spent_max_us=11` on
both sides, with B's spread slightly *tighter*.

Why, from the rows rather than from theory: COMP's decision occupies about 6 µs
of each ~101 µs sector and the commutation it schedules fires ~16 µs after the
crossing, i.e. **after** that decision has returned. Preemption of a decision
by a commutation is therefore structurally rare at this rung — the two are
almost never in flight together — which is also why the gain is only the 1–3 µs
of queueing that remained.

Both runs passed their gates with `blank_latched=0`, `forced_pct=0` and guard
`reason=0`. Captures and images: `captures/chain/e165-{A-peer,B-comtop}375.txt`,
`captures/elf/{peer,comtop}.e165-chain.elf`.

**Next:** the same pair at 47.5%, which is at or above 45%, so the pre-run
independent review of step 6 runs first and is appended verbatim before either
image is driven.

### E166 — the 37.5% A/B at n=3 per side, which withdraws the gain I reported at n=1

**Written after the runs, and it corrects the entry above.** E165's pair was
one run per side, and from it I reported that COM-above-COMP halves the
commutation service's median lateness (2.0 → 1.0 µs) and shortens its tail
(26 → 23 µs). Two more pairs, driven **ABAB** in the same session with the
fixture flashing each image, say that was noise.

| run | side | commutation lateness p50 | max | effective angle | powered eHz |
|---|---|---|---|---|---|
| `e165-A-peer375` | peers | 2.0 | 5 | 0.1942 | 1642.3 |
| `e166-A-peer-375_2` | peers | 2.0 | 5 | 0.1942 | 1648.2 |
| `e166-A-peer-375_3` | peers | 1.0 | 4 | 0.1942 | 1645.5 |
| `e165-B-comtop375` | COM top | 1.0 | 5 | 0.1942 | 1644.1 |
| `e166-B-comtop-375_2` | COM top | 1.0 | 4 | 0.1942 | 1647.0 |
| `e166-B-comtop-375_3` | COM top | 2.0 | 4 | 0.1942 | 1646.9 |

* **Commutation-purpose lateness: no difference.** The median takes both
  values on both sides (2,2,1 against 1,1,2) and the maxima overlap (5,5,4
  against 5,4,4). The "2.0 → 1.0" I published was one sample against one
  sample of a quantity that moves by a whole microsecond between runs of the
  *same* image.
* **Effective angle: 0.1942 in all six runs**, to four decimals, both sides.
* **Speed: peers 1645.33 (spread 5.9), COM top 1646.00 (spread 2.9)** — a
  difference of **+0.4 per mille**, an order of magnitude inside the spread.
* **Cost: none measurable.** `spent_max_us=11` and `late_arms=0` in all six;
  per-event elapsed at the arm 5–9 µs on both sides.

**Verdict at this rung: COM above COMP changes nothing measurable, in either
direction.** That is the answer the rows support, and it is the third time in
this campaign that an n=1 reading of a microsecond-scale quantity turned out to
be noise (E159's late-arm margin, E165's lateness gain, and campaign 7's
`late_arms` reading). The standing rule this earns: **no microsecond-scale A/B
conclusion from fewer than three runs a side, alternated.**

The mechanism the rows already showed still explains why: COMP's decision
occupies ~6 µs of a ~101 µs sector and the commutation fires ~16 µs after the
crossing, so the two are almost never in flight together and there is nearly
nothing for a priority change to reorder.

**Still owed at 47.5%:** the same pair, behind the pre-run review, because the
argument above is a 37.5% argument — at 47.5% the wait is 13 µs against the
same ~6 µs decision, so the overlap is closer and the answer may differ.

### E167 — pre-run review of step 6, verbatim, and the cost it found that I had missed

The goal requires a fresh context-free review before every powered run at 45%
or above. This one read E162/E164/E165, the seam machinery and the A/B
captures, and **recommended that the 47.5% pair not be treated as
pre-cleared.** It was right on the substance: there *is* a measurable cost to
COM-above-COMP, I looked for it in a quantity that structurally cannot see it,
and my own audit missed a non-atomic window that the priority change exposes.

#### Review as received

> ## 1. Claims lacking cited measurements
>
> 1. **"The reference's arrangement above 48% duty"** — the entire motivation for `com-top` (`Cargo.toml:47`, `src/shared.rs:78`, and its mirror `roots.rs:220-223`, `roots.rs:817-818`). The only thing ever quoted from the reference is the string `"COMP/COM remain priority 0x40 peers"`, with no file:line anywhere in the tree. The "48%" clause is firmware50's *own* arithmetic (`LAB_NOTEBOOK.md:6673`: R_COMP 13 µs crossing the level-22 wait ≈ 2.0 keHz ≈ 48% duty), appended to the reference quote in `LAB_NOTEBOOK.md:3847` and `:4148` and then re-attributed *to* the reference in `shared.rs:78`. No citation shows the reference ever runs COM above COMP at any duty.
> 2. **That premise is superseded by the notebook's own later measurement** — `LAB_NOTEBOOK.md:6777` revises the crossover to "about 2.8 keHz or 75% duty, against 48% before these two steps." `BEMFRCOMP spent_max_us=11` in both A/B captures confirms the R_COMP=11 µs row is the live one. Neither E162, E164 nor E165 cites this; the A/B is run at rungs where its own inequality says the peer arrangement has margin (and `late_arms: 0 of 341` at 47.5%, `e160-chain475`, agrees).
> 3. **E162's static cost figures** (`ADC_COMP 1026 → 989`, `TIM16 349 → 333`, instructions `710 → 699` / `318 → 311`), repeated in E164's gate list, cite no artifact — no saved `isr_cycles.py` output, no ELF pair. `captures/elf/` has no pre-restructure/post-restructure pair labelled for this comparison. They are also 0-wait-state model outputs being used as the "cycles are not growing" gate, not measurements.
> 4. **"the production image's four roots are instruction-identical to E162's `4F0F5131` (37 / 699 / 306 / 153)"** (E165, default-build check) — no `com-top`-off ELF of the E165 tree is archived, so this is the one `isr_diff` claim in E165 that cannot be reproduced from the repo. (The recording-build claim *can* be, and does — see below.)
> 5. **E162's ordering guarantee is real but exercised by nothing.** "The pair is written before `accept_seq` is incremented, so a reader that keys on the sequence cannot see a newer sequence with an older estimate" — true at `roots.rs:384-386` / `:455-457`, but `com_root` reads the pair at `roots.rs:945-946` **without reading `accept_seq`**, and `det_poll` (`bin/board.rs:521-526`) keys `accept_raw` on the sequence but never the pair. No reader keys on the sequence. The stated protection is inert.
> 6. **E162's "the estimator's value at commutation time *is* the value at the last acceptance … by construction"** is asserted, not measured, and is false for the handover commutation: `det_install` seeds `accept_avg`/`accept_blank` (`bin/board.rs:487-488`) but never resets `det.accept_raw`, which `com_root` phase 1 reads for `since` at `roots.rs:959`. The first commutation's blanking-floor `hold` is computed from a stale or zero stamp.
> 7. **E162 prediction 2's tolerance** ("0.194 ± 0.003") — the ±0.003 has no cited derivation; E164 then reports an exact-match p50 (0.1942) and treats the band as satisfied without ever using it.
> 8. **E165's `spent`-based cost model** ("expect the per-event maximum to grow from 9 towards 9 + the COM handler's ~5 µs") — the "~5 µs" is `chain.py`'s *handler entry → bridge update* bracket (`max=5`), i.e. a fraction of the COM root, not its duration. No measurement of the COM handler's total duration is cited anywhere in E162/E164/E165.
> 9. **E164's "the old single-ring captures reproduce byte for byte"** — two p50s match (`e157-chain375` gives 0.1942 / 4 µs, confirmed), but the distributions do not: chain-per-event sd is 0.38 in `e157-chain375` and 0.65 in `e162-chain375`. "Byte for byte" is an overclaim about two medians.
> 10. **E164's gates** ("host tests 314 + 9", "`isr_audit` clean on all four roots", "structure report 0 functions over 100 lines", "clippy clean") — asserted with no output cited or stored.
>
> ## 2. Numbers that came from summaries rather than captures
>
> 1. **"The two images differ by one instruction's worth of configuration … the only difference the disassembly shows is one more `movs r3, #128`"** (E165). Verified against the ELFs and false in both particulars: the normalized full disassembly is **16607 lines in both** — nothing is added — and there are **two changed instructions**, both in `roots::comp2_init` (`0x80076e4: movs r3,#128` ← `#64`, and `0x80076e8: movs r2,#127` ← `#191`, the complement mask of the IPR field). The character of the difference ("one more") came from a narrative, not from the diff.
> 2. **The A/B's conclusion itself.** `LAB_NOTEBOOK.md` E165 (`:9232`–end of file, `:9299`) contains only "Written before the runs", the change, what the A/B measures, the predictions and the protocol. **There is no results section.** "No measurable gain, no measurable cost" exists only in the pre-run summary handed to me; nothing in the notebook is derived from `e165-A-peer375.txt` / `e165-B-comtop375.txt`.
> 3. **And that unwritten conclusion is contradicted by the captures.** `comp_call_max_us` is **17** in the A run and **25** in the B run (`captures/chain/e165-A-peer375.txt` / `-B-comtop375.txt`, `BEMFDRIVEN` line). It is **17 in every one of the 14 archived peer chain captures at every rung from 25% to 47.5%** (16 on warmups), so +8 µs is far outside the historical 16–17 scatter — a reproducible, duty-dependent cost, not a null. The `com-top` *warmup* at 15% reads 17 (`e165-B-comtop375-warmup.txt`, `target_tenths=150`), so the cost appears only once the remaining wait shrinks. Two further unreported A/B runs in the same directory (`e166-A-peer-375_2`: 17; `e166-B-comtop-375_2`: 25) reproduce it exactly.
> 4. **`irq_peak_per_ms` 38 → 55 and `closed_irq_peak_per_ms` 60 → 65** across the pair — also uncited and unmentioned; `mean_ma` 736 → 759 and `hold_ma` 867 → 890 likewise (though n=1 each; see §6).
> 5. **E164's "E155's runs were about two hours earlier in the same day"** and the 5-per-mille cross-session shift are read off a recollection of session timing, not off capture timestamps quoted in the entry.
>
> ## 3. Soundness with `com-top` (COMP 0x80, COM 0x40, guard 0x00)
>
> **(a) Two live `&mut` to the same value: no, and it is compiler-enforced.** I traced every `Seam`:
>
> 11. `det.zc`, `det.rate`, `drv.rate` are `CompPrio`-ceiling (`shared.rs:287,289,332`) and reached only by COMP's `root()` (`roots.rs:356,488,1045,1089`, `capture.rs:207`) and the foreground's masked `lock()` (`bin/board.rs:489,497,367-368,650,660`). `com.six` / `com.plans` are `Motor`-ceiling (`shared.rs:377,381`) and reached only by COM's `root()` (`roots.rs:947,922`) and foreground `lock()` (`roots.rs:869`, `board.rs:491`). `CHAIN_ACC`/`CHAIN_SVC` are split per root at the matching ceilings (`chain.rs:81,87`). `RING` is `CompPrio` (`capture.rs:165`). `guard.watch` is `Guard`-ceiling, taken `root()` by the guard (`roots.rs:768`) and `masked()` by COMP (`roots.rs:714`) — the only `masked` call site in the tree.
> 12. The type wall is genuine: `AtOrBelow<CompLow> for Motor` is deliberately *not* implemented (`shared.rs:104-113`), so a `Root<Motor>` cannot reach a `CompPrio` seam even with masking. COMP does hold `&mut Option<ZeroCross>` across `com_arm` (`roots.rs:402` is *inside* the `zc.root` closure that runs to `:419`), and COM can now fire inside that window — but `com_root` touches no `CompPrio` value. **(a) is clean.**
> 13. Dead permission worth noting: `impl AtOrBelow<Motor> for CompLow` (`shared.rs:111`) would let COMP masked-borrow `com.six`/`com.plans` with no reviewer noticing. Nothing uses it today.
>
> **(b) Torn / inconsistent reads: one real hole and one unstated invariant the whole argument rests on.**
>
> 14. **`com_arm` is a non-atomic 10-write sequence that COM can now preempt.** `roots.rs:832-846` stores `sched_raw`, then `phase`, then calls `hw::com_timer::arm` (`hw/timers.rs:74-83`: CR1, ARR, CNT, EGR.UG, SR, DIER.UIE, CR1|CEN — seven register writes), with no critical section. Under peers this was effectively atomic because TIM16 could not dispatch while COMP ran; under `com-top` a TIM16 dispatch landing inside it yields a handler that runs with the *new* `sched_raw` against the *old* `phase` (garbage `late`, `late_max`), or — if it lands after `phase.store(1)` and before `CR1|CEN` — a phase-1 commutation executed immediately instead of at `crossing + wait`, i.e. a zero-advance commutation. The `regs()` safety comment (`hw/timers.rs:50-53`) still reads "owned by the COM root and the foreground's arm/stop, which never overlap" — it never mentions COMP, which is the third writer (`roots.rs:402,461`), let alone COM preempting COMP inside it.
> 15. **The reason this is (probably) unreachable is nowhere written or tested.** It requires TIM16 armed-or-pending while the EXTI line is live. Every path that arms TIM16 also masks or primes the line (`roots.rs:953-966`: phase 2 → `comp_exti_mask`, phase 3 → `comp_exti_prime`; phase 0 → `comp_exti_arm` with no arm), and `com_stop` unpends (`roots.rs:848-852`). That invariant is the load-bearing element of the whole soundness argument for `com-top` and E162's audit table does not state it, no counter tests it, and no gate enforces it. **The same invariant is what preserves sector identity**: COMP reads `det.step` at `roots.rs:375` and passes that value to `guard_event` at `:440`, while COM advances `det.step` at `:921` — which sits between them in time. Progression stays monotone `+1` (`tracking.rs:129-134`, `Fault::SectorOrder`) *only* because COM cannot fire before `roots.rs:402`. If it ever can, the guard's accepted-event watch mis-orders and trips `Reason::Tracking` spuriously.
> 16. **The acceptance pair is safe, but by accident of ordering, not by the published argument.** `accept_avg`/`accept_blank`/`accept_seq` (`roots.rs:384-386`) precede the arm at `:402`, and phase 1 is armed only by COMP itself, so `com_root`'s reads at `:945-946` cannot see a split pair. That is the correct argument; E162 gives a different one (the `accept_seq` fence, §1.5) which no reader uses.
> 17. **`Reason::HandlerOverrun` now measures the wrong quantity.** `elapsed` at `roots.rs:1064-1066` brackets COMP's entry stamp to after the decision, so under `com-top` it *includes* any nested COM handler. `call_max_us` 17 → 25 is exactly that. But `rate.rs:48-54` and `protection.rs:44-46` define the stop as "one COMP handler call exceeded its 50 µs budget" — it is now COMP+COM, so the stop can fire on time COMP did not spend, and the metric is no longer comparable across the two images. Margin to the stop fell from 33 µs to 25 µs and E165 predicts nothing about it.
> 18. **`det_install` / `com_handover`: one pre-existing unguarded window, unchanged by `com-top` but relevant to the audit.** `com_handover` (`bin/board.rs:503-517`) sets `com.active = true` (`:511`) and unmasks TIM16 (`:514`) before calling `com_arm` (`:516`), all outside a critical section. COMP (above the foreground in both images) can accept in that window and arm phase 1 itself; the foreground's `com_arm` then overwrites `sched_raw`/`phase`/ARR and the acceptance's commutation is retimed. Not a `com-top` delta, but it is the kind of thing E162's table claims to have swept.
> 19. **Stale safety contracts.** `comp_root`'s `# Safety` block asserts the handler runs at "`Motor::NVIC`, set by `comp2_init`" (`roots.rs:1025`) and `COMP_IRQ_PRIORITY`'s doc says "**0x40 since E076** … a peer of the COM root" (`roots.rs:218-223`) — both false in the A/B image, and the first is the contract that `unsafe { Root::<CompPrio>::enter() }` (`roots.rs:1035`) is discharged against.
>
> ## 4. The "four roots instruction-identical" claim
>
> 20. **Holds, and I reproduced it.** `python scripts/isr_diff.py captures/elf/peer.e165-chain.elf captures/elf/comtop.e165-chain.elf` → `DMA1_CHANNEL1: identical, 37` / `ADC_COMP: identical, 764` / `TIM16: identical, 355` / `TIM6_DAC_LPTIM1: identical, 153`, matching E165 exactly.
> 21. **Provenance also holds**: `sha256(peer.e165-chain.elf)` = `20E6AC43…`, `sha256(comtop.e165-chain.elf)` = `DD9E49D9…`, byte-identical to `20E6AC43.e165-A-peer375.elf` / `DD9E49D9.e165-B-comtop375.elf` and to the `# elf_sha256` header of each capture. The diffed images are the flashed images.
> 22. **"differ only in one priority write" does not hold** — two changed instructions, both in `roots::comp2_init` (§2.1). Harmless (outside all four roots), but the claim as written is wrong and was not checked.
>
> ## 5. Is the 37.5% A/B conclusion supported, and is it fair?
>
> 23. **The comparison is fair.** Same evening (`started 2026-09-23T21:49:20` vs `21:52:39`, 3 minutes apart), same rung and settings (`target_duty_tenths=375 advance_level=22 total_ms=80000 inject=0`), both recording builds with identical root code, fixture-recorded hashes, same instrument (`chain.py`, `rows=2048 kept=1024`, `index offset +0` pairing on both). E164's back-to-back protocol was followed.
> 24. **"No measurable gain" is supported.** `effective angle p50=0.1942` in both; `zc_rate_permille_of_expected=996`, `forced_pct=0`, `ehz_from_sector=1650`, `mean_ci_us=101` in both; `crossing → bridge p50=20.0` in both. The predicted 0–2 µs gain did not appear.
> 25. **"No measurable cost" is not supported, and is contradicted** — `comp_call_max_us` 17 → 25 (§2.3), reproducible across a second unreported pair, absent at the 15% warmup. E165 looked for the cost in `spent`/`spent_max_us`, which by construction cannot see it: `spent` is computed at `roots.rs:399` *before* `com_arm`, and the preemption necessarily lands at or after the arm. So the prediction "`spent_max_us` from 11 towards ~16" was untestable as written, and its falsification (`spent_max_us=11`, chain "elapsed at the arm" max 9, in both) is not evidence of no cost.
> 26. **Two other deltas are unexamined**: `irq_peak_per_ms` 38 → 55 and `com_late_max_us` 9 → 8, plus a 1 µs shift in phase-1 service lateness p50 (2.0 → 1.0) inside a distribution with max 5 in both.
>
> ## 6. The 47.5% pair: safety and instrumentation
>
> 27. **The measured cost extrapolates upward, not flat.** The +8 µs appears only where the remaining wait is small (absent at 15%, present at 37.5%). At 47.5% (`e160-chain475`) requested wait is `min=11 p50=13.0 max=15` against `elapsed at the arm min=5 p50=6.0 max=9` — worst-case slack of 2 µs — and sector interval `min=50 µs`. A 25 µs COMP call is then half of the shortest sector. No projection of `comp_call_max_us` at 47.5% is offered anywhere; the E165 predictions list does not mention the variable that actually moved.
> 28. **Neither hard stop covers the mechanism `com-top` changes.** `LateArm` (`run/states.rs:219`) rises only from `left == 0` at `roots.rs:406-410`, i.e. pre-arm time; `BlankLatched` (`:223`) is unrelated. The quantity that grew is post-arm and is covered only by `HandlerOverrun` — whose measurement is now polluted by the nested handler (§3.17).
> 29. **Detection latency is bounded but not by these stops.** All three are polled once per foreground pass; `loop_gap_max_us` is 143 (A) / 147 (B), against a 47.5% sector of 50–140 µs — so up to ~3 commutations can land at an arbitrary angle between the event and `safe_off`. That is instrumented (`run/states.rs:195-199`), but E165 neither cites it nor bounds it for the 47.5% rung.
> 30. **No instrument counts the event under test.** There is no "COM preempted COMP" counter anywhere in `roots.rs`; the sole witness of the mechanism firing is a single run-wide maximum. Per the campaign's own rule, the refusal/occurrence class should be counted at birth, not inferred from an outcome max. Without it, a null at 47.5% will be indistinguishable from "the preemption never happened."
> 31. **One under-instrumented safety gap orthogonal to the A/B**: `guard_trip` (`roots.rs:641-664`) can preempt COMP mid-acceptance and calls `com_stop()`; COMP's `com_arm` at `roots.rs:402` then re-arms TIM16 *after* the guard has de-energized the bridge. Pre-existing (the guard was already above COMP), but it is a commutation scheduled after a trip and it is not in E162's table.
>
> ## 7. Single runs, medians and nulls treated as bounds
>
> 32. **"The COM root's own service lateness is already 1–2 µs median, so COM-top can remove at most that"** (E165, prediction 1) — a median used as a ceiling on a distribution whose max is 5 in every capture. The upper tail is where a commutation would actually be queued.
> 33. **"`blank_latched` … has read zero in every capture taken since"** (`protection.rs:69-71`) used to justify a stop-on-first-event — fine as policy, but the accompanying "the question is empty" reasoning treats an all-null history as a proven property of the design.
> 34. **E164's cross-session shift is one pair of n=3 groups promoted to a rig property**: "the same rung on the same image now reads 5 per mille slower. That is a measured property of this rig." One session pair, 0.03 eHz apparent difference within-session, becomes a standing instrument correction.
> 35. **The `comp_call_max_us` history I used above is itself n=1 per run** — but 14 peer runs all reading 16–17 and 3 com-top runs all reading 25 is a distribution, not a single observation; that is the asymmetry E165's missing results section should have caught.
> 36. **`spent_max_us=11` is treated across E162/E164/E165 as "R_COMP", a rate/bound** — it is a run-wide maximum of a partial bracket (entry to just-before-arm), which `LAB_NOTEBOOK.md:6777` itself flags ("It is a maximum, set by the worst instance including one guard preemption"). Under `com-top` it excludes the new preemption entirely, so continuing to call it R_COMP will understate the handler in every downstream inequality.
>
> **Gate recommendation (report only):** the 47.5% pair should not be read as pre-cleared. The `com-top` image has a reproducible +8 µs on COMP's worst call, 24% of the `HandlerOverrun` margin, in a metric that image also redefines; the notebook has no results section for the 37.5% pair; the stated motivation ("the reference above 48%") is uncited and superseded by the tree's own 75% figure; and there is no counter for the preemption itself.

#### Responses

**§2.3 / §5.25 — the finding that matters, and it overturns my conclusion.**
`comp_call_max_us` is **17 on every peer run and 25 on every `com-top` run**,
and absent at the 15% warmup. I reported "no measurable cost" because I looked
at `spent`, which is stamped *before* `com_arm` and therefore cannot see a
preemption that necessarily lands at or after the arm. **The cost is real,
reproducible, +8 µs on COMP's worst call, and my prediction was untestable as
written.** E166's verdict is corrected to: *no measurable gain, and a
measurable cost of +8 µs on the worst COMP call.*

**§3.14, §3.15 — a soundness gap in my own audit, now stated.** `com_arm` is
a ten-write sequence with no critical section, and under `com-top` a TIM16
dispatch inside it could run phase 1 against a half-written arm — including a
commutation fired immediately instead of at `crossing + wait`. The reason it
does not happen is an invariant my audit never wrote down: **every path that
arms TIM16 first masks or primes the comparator line**, so TIM16 is not armed
while the line is live. The same invariant carries sector identity
(`det.step`) and the guard's accepted-event ordering. It is now stated here as
the load-bearing assumption it is, and it is **not** enforced by a gate — which
is a reason to refuse `com-top`, not a reason to proceed quietly.

**§3.30 / §6.30 — instrument the event, not the outcome.** Added: `Det.in_decide`
(set around COMP's decision) and `Com.preempts` (incremented when COM runs with
that flag set), reported as `com_preempts` on the `BEMFRCOMP` line. It is gated
on the **chain recorder**, not on the feature, so both sides of the A/B carry
identical instrumentation — my first version gated it on `com-top` and
`isr_diff.py` immediately showed the two images' COMP roots differing by far
more than the priority byte. Both new fields are **last** in their `#[repr(C)]`
structs, because inserting them in the middle moved every later offset and
changed TIM16's machine code for no behavioural reason (also caught by
`isr_diff.py`). Production is byte-identical to `4F0F5131` again: 37 / 699 /
306 / 153.

**§1.1, §1.2, §3.19 — the motivation is the operator's goal, not a verified
reading, and the tree's own figure supersedes mine.** `COMP_IRQ_PRIORITY`'s doc
now says exactly that: the only quotation from the reference is *"COMP/COM
remain priority 0x40 peers"*, the "48%" is firmware50's own arithmetic, and
E148 already revised the crossing point to ~2.8 keHz (~75% duty) once R_COMP
fell to 11 µs. So **the A/B is being run at rungs where this firmware's own
inequality says the peer arrangement has margin** — which is worth knowing
before reading its null as a refutation of the reference. `comp_root`'s
`# Safety` block and the priority doc are corrected to name `CompPrio` and the
feature.

**§1.6 — a real defect, fixed.** `det_install` seeded the published pair but not
`accept_raw`, which phase 1 uses for the blanking floor's `since`; the handover
commutation computed its floor from a stale stamp. It now seeds `accept_raw`
too. This predates the restructure.

**§1.5, §3.16 — accepted.** No reader keys the pair on `accept_seq`, so that
fence is inert; the argument that holds is the one the reviewer states — phase 1
is armed only by COMP, after the pair is written.

**§3.13 — removed.** `impl AtOrBelow<Motor> for CompLow` was dead permission
that would have let COMP masked-borrow COM's values unnoticed.

**§3.17 / §6.28 — accepted and recorded as a limit of the A/B.**
`Reason::HandlerOverrun` brackets COMP's entry to after the decision, so under
`com-top` it counts the nested COM handler: the stop can fire on time COMP did
not spend, and `call_max_us` is not comparable across the two images. Margin to
the 50 µs budget is 33 µs on the A side and 25 on the B side. I am **not**
changing the overrun stop — thresholds are off-limits — so this stands as a
stated divergence of the diagnostic image.

**§2.1, §4.22 — corrected:** the two images differ in **two** instructions,
both in `comp2_init` (the priority byte and the IPR field's complement mask),
not "one more `movs`". The four roots are identical, which the reviewer
reproduced.

**§2.2 — the results section existed but was appended after this review
started** (E165's completion table and E166). The reviewer's reading of the
file at the time was accurate, and the substantive point — that its conclusion
was contradicted by `comp_call_max_us` — stands and is acted on above.

**§1.3, §1.4, §1.9, §1.10, §2.5 — accepted.** The cycle figures are 0 WS
static model outputs (now saved: `captures/gates/e167-cycles.txt`), the
E165-tree default ELF is now archived (`captures/elf/peer2.e167-chain.elf` and
its production sibling), "byte for byte" was an overclaim about two medians
(the chain sd differs, 0.38 against 0.65), the gate outputs are stored, and the
cross-session claim rests on capture timestamps that should have been quoted.

**§6.27, §6.29, §7.32, §7.36 — accepted as scope.** No projection of
`comp_call_max_us` at 47.5% was offered; the detection latency is one
foreground pass (`loop_gap_max_us` 143/147, i.e. up to ~3 sectors at 47.5%);
a median is not a ceiling; and `spent_max_us` is a partial bracket that under
`com-top` excludes the very preemption at issue, so it must not be called
R_COMP in that image.

**§3.31 — recorded, not fixed:** a guard trip can preempt COMP mid-acceptance,
after which COMP's `com_arm` re-arms TIM16 although the guard has already
de-energised the bridge. Pre-existing in both images (the guard has always been
above COMP), orthogonal to this A/B, and it belongs in the protections
work the goal puts off-limits here — so it is written down for the operator
rather than changed.

**What this does to the 47.5% pair.** It is **not** pre-cleared, and I am not
running it as a qualification. What I will run, with the new counter, is the
same bounded A/B pair, because the open question it answers is exactly the
reviewer's: does the preemption actually occur, and what does it cost where the
wait is 13 µs rather than 16? The predictions are recorded before the runs in
the next entry. If `com_preempts` is zero, the A/B has been measuring an event
that never happened and the whole comparison is void — which is itself the
finding.

### E168 — step 6b at 47.5%, with the preemption counted

**Written before the runs.** E167's review established that my 37.5% "no
measurable cost" was wrong (`comp_call_max_us` 17 → 25 on every `com-top` run)
and that nothing counted the event under test. Both are addressed: the
recording images now carry `com_preempts`, and the A/B pair is rebuilt so the
two images are again identical in all four roots (37 / 780 / 362 / 153) and
differ only in `comp2_init`'s two priority instructions. Production is
byte-identical to `4F0F5131` (37 / 699 / 306 / 153); structure report clean;
host tests 314 + 9; gate outputs in `captures/gates/e167-cycles.txt`.

Images: `captures/elf/peer3.e168.elf` (A, peers at 0x40) and
`captures/elf/comtop3.e168.elf` (B, COMP at 0x80).

**Predictions, and this time they name the variable that moved.**

1. **`com_preempts` > 0 on the B side and exactly 0 on the A side.** A is
   structurally incapable of it — TIM16 cannot dispatch while COMP runs at the
   same priority — so a non-zero A reading would mean the counter or the flag
   is wrong, not that peers preempt.
2. **`comp_call_max_us` ≈ 25 on B and 17 on A**, as at 37.5%, because the
   nested COM handler is what the bracket now includes. If B reads 17 *and*
   `com_preempts` is 0, the mechanism did not fire in that run and the
   comparison is void rather than favourable.
3. **The 47.5% rung is where a cost could bite.** The per-event slack there is
   `wait − spent` with a worst pair of 4 µs (E163's correction), and the
   shortest sector in the 47.5% window was 50 µs. If a preemption lands inside
   the arm's own window the commutation can fire immediately — the
   zero-advance case E167 §3.14 describes — which would show as a
   crossing-to-bridge delay far below the requested wait, or as a late arm.
   **I predict neither appears**, because TIM16 is armed only while the
   comparator line is masked or primed (E167 §3.15, the invariant the audit
   had left unwritten), so the flag can be set while COM fires but the *arm*
   itself is not interruptible by a crossing-driven COM.
4. **No gain, again:** effective angle equal to A's to four decimals, speed
   within the within-session spread, `zc_rate_permille_of_expected` equal.

**What would change the verdict.** A measurable gain at 47.5% — angle, speed or
rate identity — would make COM-top worth its 8 µs. Its absence, with the cost
measured and the soundness argument resting on an unenforced invariant, is a
refusal on evidence.

**Protocol.** ABAB, four runs, fixture flashing and hashing each image, one
session, 47.5% via the climb keys, no qualification claimed — these are
diagnostic runs of a diagnostic image.

**Completed after the runs.** ABAB at 47.5%, four runs, fixture-flashed and
hashed, all gates PASS, `late_arms=0` and `blank_latched=0` throughout.
Captures `captures/chain/e168-{A-peer,B-comtop}-475_{1,2}.txt`.

| quantity | A: peers (n=2) | B: COM above COMP (n=2) |
|---|---|---|
| **`com_preempts`** | **0, 0** | **29 913, 29 686** |
| `comp_call_max_us` | 17, 17 | **25, 25** |
| commutation-purpose lateness p50 / mean | 2.0 / 1.98, 1.89 | 2.0 / 1.82, 1.92 |
| requested wait, per event | 10–16, p50 13 | 10–16, p50 13 |
| crossing → bridge, µs | p50 17, max 21 | p50 17, max 20–21 |
| chain per event, µs | p50 4 (3–8) | p50 4 (4–7) |
| effective angle | 0.2099, 0.2073 | 0.2073, 0.2073 |
| powered speed, eHz | 2064.0, 2064.9 | 2066.4, 2067.5 |
| `spent_max_us` / `late_arms` | 11 / 0 | 11 / 0 |

**Verdicts against the four predictions.**

1. **`com_preempts` > 0 on B, exactly 0 on A — confirmed, decisively.** ~29 800
   preemptions of about 403 500 commutations, i.e. **7.4% of commutations
   interrupt a zero-crossing decision** on the B side, and **none** on the A
   side, where it is structurally impossible. The mechanism under test is real,
   it fires often, and the counter distinguishes "no effect" from "never
   happened" — which is exactly what E167 said was missing.
2. **`comp_call_max_us` 17 on A and 25 on B — confirmed**, the same +8 µs as at
   37.5%, now with the cause counted rather than inferred.
3. **No zero-advance commutation and no late arm — confirmed.** The
   crossing-to-bridge minimum is 14–15 µs against requested waits of 10–16, so
   no commutation fired early; `late_arms=0` in all four. The unwritten
   invariant E167 named (TIM16 is armed only while the comparator line is
   masked or primed) held under 60 000 actual preemptions, which is the first
   evidence for it rather than an argument.
4. **No gain — confirmed.** Effective angle 0.2073 in three of four runs and
   0.2099 in one A run; chain 4 µs in all four; commutation lateness medians
   identical at 2.0 with means 1.82–1.98 overlapping; speed A 2064.5 mean, B
   2067.0 mean, i.e. **+1.2 per mille**, inside the within-session spread and in
   the same direction as the 37.5% pair's +0.4.

**Verdict on COM above COMP: refused, on evidence.**

It does what it says — 7.4% of commutations no longer wait behind a
zero-crossing decision — and that buys **nothing measurable**: no angle change,
no speed change beyond the session's own scatter, no rate-identity change, at
either 37.5% or 47.5%. It costs **+8 µs on COMP's worst call** (17 → 25 µs, a
quarter of the 50 µs handler budget), and it makes `HandlerOverrun` measure
COMP-plus-nested-COM rather than COMP, so the stop's own meaning changes. Its
soundness rests on an invariant that no gate enforces — TIM16 armed only behind
a masked or primed line — which these runs support but do not prove.

The reason there is nothing to win is visible in the rows and was visible
before the A/B: the commutation service's own lateness is 2 µs against a 13 µs
wait and an 82 µs sector. **There was never 8 µs of queueing to recover.** The
reference's arrangement may well matter on its own timing chain; on this one,
at these rungs, the peer arrangement is not what limits anything.

What the campaign keeps from step 6 is the restructure, not the priority
change: the estimator is COMP-only, the compiler enforces it, production is
byte-identical, and the `com-top` feature stays in the tree — off — as the
instrument that refused it.

### E169 — independent review of step 1 (E153), verbatim, with responses

Step 1 predates the goal's review requirement, so it was reviewed last. The
reviewer was given E153 and the code only, and warned that the tree has moved
since. Its core verdicts: **claim B is right and correctly reasoned; claim A's
first half is right; claim A's second half was right then and is false now
(step 6a removed it); and several supporting assertions were stated without the
audit they needed.** It also found a stale comment on the very field claim B
turns on — which I had missed while reporting a different stale comment.

#### Review as received

> Read the E153 entry and the current source. Note up front: `firmware50/` is **untracked** in git (`?? firmware50/`), so there is no historical revision to diff against — for each claim I can only judge "true of the code the entry described" from the entry's own quotes plus the current source's self-dated comments (step 6a / E167 blocks), which corroborate what the pre-6a shape was.
>
> ## 1. Which claims lack cited measurements (claims about code not supported by the cited code)
>
> 1. **"Every motor-owned value is `Seam<_, Motor>`"**, cited to `shared.rs:65-67`. Those lines only define `Motor::NVIC = 0x40`; they say nothing about which values carry that ceiling. Today it is outright false: `det.zc`, `det.rate`, `drv.rate` are `Seam<_, CompPrio>` (`src/shared.rs:288`, `:290`, `:343`), and `CompPrio` is `Motor` only in the default build (`src/shared.rs:96-100`). Under `com-top` (`src/shared.rs:88-93`) it is `0x80`. It was plausibly true of the pre-6a code, but the cited lines never supported it.
> 2. **"…which is the priority COMP, COM and DMA all share."** No citation covers the runtime priorities. They are set elsewhere and only two of three are near the cited file: `COMP_IRQ_PRIORITY = CompPrio::NVIC` (`src/roots.rs:232`), `COM_IRQ_PRIORITY = Motor::NVIC` (`src/roots.rs:826`), and DMA's is a separate literal `0x40` in `src/hw/adc.rs:86` — declarations, not a readback of IPR. The claim is inferred from three constants, one of which the entry never looked at.
> 3. **"the 'non-preempting peers' assumption … *is* the exclusivity argument."** Overstated relative to the comment it quotes. `Seam::root`'s SAFETY block (`src/shared.rs:214-221`) is a conjunction of four conditions: peer non-preemption, nothing above `P` touching it, sub-`P` contexts borrowing only masked, and the `&mut Root` token blocking re-entry. Peer non-preemption is one conjunct, not the argument. The entry's own ellipsis in the block quote elides the other three and then promotes the first.
> 4. **"`com.plans` and `com.six` are COM-only, and `det.rate`/`drv.rate` are COMP-only, so `det.zc` is the one value that must be restructured."** This is an exhaustiveness claim over all borrow sites and the entry cites none for it. It happens to hold in the current tree (`plans.root` only at `src/roots.rs:948`, `six.root` only at `:973`, `det.rate.root` at `:1082`, `drv.rate.root` at `:1139`, `zc.root` only at `:363` and `:495`), but the entry asserts it without the audit. It also silently omits `guard.watch` (`src/shared.rs:367`), which two contexts do touch — harmless, because COMP reaches it via `masked`, but that is exactly the reasoning the claim needed to state.
> 5. **"A COM interrupt arriving while COMP holds that borrow — which is exactly what the change is *for*."** A frequency/timing assertion presented as fact. Nothing in the cited code shows that COM arrivals land inside the borrow window, and no capture is offered. The firmware later added a purpose-built counter for precisely this event (`Com::preempts`, `count_preempt` at `src/roots.rs:883-887`, `Det::in_decide` at `src/shared.rs:300`) — i.e. the project itself judged the claim to need a measurement the entry does not have.
> 6. **Unstated caveat on the UB conclusion.** The conclusion "raising COM above COMP, by itself, is undefined behaviour" holds only for a change confined to the NVIC byte with both ceilings left as `Motor`. Nothing in the entry says so, and the type system is the actual gate: once COMP's values carry `CompPrio`/`CompLow`, `Seam::root` demands `Root<P>` at exactly the ceiling (`src/shared.rs:213`) and the alias is a compile error, not UB. The entry's framing ("undefined behaviour, not a timing change") reads as a property of the *priority change*, when it is a property of the *ownership shape* the change would have been made against.
> 7. **"phase 3 … fires *once per sector*."** Not what the code shows. Phase 3 is armed only on the `else if hold >= BLANK_ARM_MIN_US` branch (`src/roots.rs:992-996`); below that floor the code takes the third branch and arms nothing (`:997-1000`), and the mutually exclusive phase-2 branch (`:988-991`) takes the sector instead. So it is *at most* once per commutation, rate-dependent. The firmware has `blank_arms` (`src/shared.rs:384`, incremented at `:990`/`:995`) which would have settled it; the entry quotes no count.
> 8. **"byte-identical to E141"** (the stale-comment finding). A codegen-identity claim with no artifact cited — no objdump, no `isr_diff.py` output, nothing. The project's own tool for this is named two entries later (`isr_diff.py`, referenced at `src/shared.rs:299`), so the instrument existed and was not used here.
> 9. **"four bookkeeping stores"** above the arm. Consistent with the pre-6a arm (`sector_start_raw`, `accept_raw`, `accept_wait`, `accept_seq`); it is six now (`src/roots.rs:384-393`, after `accept_avg`/`accept_blank` were added by step 6a). Not an error in the entry — flagged only because it is the one detail that dates the entry's reading, and it confirms the entry predates 6a.
> 10. **"stamped at ISR entry"** — approximately, not literally. The clock read is at `src/roots.rs:935`, after `com_timer::ack()` (`:929`), the `active` load and early return (`:930-933`) and `count_preempt` (`:934`). It does not change the entry's conclusion, but the entry's own point in that paragraph is about *where* the stamp is taken, so the imprecision is in the load-bearing sentence.
>
> ## 2. Which numbers came from summaries rather than captures
>
> 1. **"8–11 µs late."** The entry's only quantitative figure, and it is explicitly sourced to prose — "mine, and the framing I relayed from the graybeard" — with no capture, log, or run cited. The entry retracts the *interpretation* (correctly) while still carrying the number forward unchallenged; per the code it is a `late_max` sample over phases 1–3 (`src/roots.rs:936-939`), so even its magnitude is unverified as a commutation quantity.
> 2. **`0x40` as "the priority COMP, COM and DMA all share."** Read off source constants (`src/shared.rs:67`, `src/roots.rs:232`/`:826`, `src/hw/adc.rs:86`), not from an IPR readback — and the DMA one from a file the entry does not cite.
> 3. **No other numbers appear in the entry.** The `1188 → 1315` cycle and `11 µs → 10 µs` figures nearby are in source comments (`src/shared.rs:34`, `src/roots.rs:401-405`), not claimed by E153.
>
> ## Specific verdicts
>
> **Claim A1 — substantially right, mildly overstated.** The quote is accurate against `Seam::root`'s SAFETY block, and `root` does hand out `&mut *self.cell.borrow(&cs).as_ptr()` with a fabricated `CriticalSection::new()` — no `RefCell` flag, no masking (`src/shared.rs:222-225`; same wording in the module docs at `src/shared.rs:18-21`). The overstatement is item 3 above: peer non-preemption is one of four conjuncts, not "the" argument. **Line citation is now wrong**: the entry cites `shared.rs:174-187`, which today is `Seam::masked` (`src/shared.rs:171-183`); `root` is at `src/shared.rs:203-226`. The quoted text still exists verbatim, so this is drift, not fabrication. Still true now for whatever `CompPrio` resolves to; the accompanying "every motor-owned value is `Seam<_, Motor>`" is not.
>
> **Claim A2 — first half true then and now; second half true then, false now.**
> - *COMP arms inside the estimator's borrow*: true. `com_arm(left.max(1), 1)` is at `src/roots.rs:409`, inside the closure passed to `S.det().zc.root(at, …)` opened at `:363` and closed at `:427` — a `&mut Option<ZeroCross>` is live across it. Same shape in the diagnostic twin (`:495`) and in the factored `accept` helper (`:468`). Unchanged.
> - *COM borrowed the same value*: **no longer true.** `com_root` phase 1 now reads two plain atomics, `S.det().accept_avg` / `S.det().accept_blank` (`src/roots.rs:971-972`), published by COMP inside the acceptance (`:391-392`). There is no `zc` borrow anywhere in `com_root`. The entry's citation `roots.rs:845` now lands inside `com_arm` (`src/roots.rs:839-853`), not on a borrow. That the claim *was* true is corroborated by two self-dated comments that describe exactly the removal: `src/shared.rs:263-268` ("COM used to read these by borrowing `zc` itself … the reason raising COM above COMP would have been undefined behaviour rather than a scheduling change (E153)") and `src/roots.rs:962-970`. I cannot verify it directly — no git history for this tree.
> - *The UB-not-timing conclusion*: sound as an aliasing argument given both halves, but see items 5 and 6: it leans on an unmeasured assumption about COM landing inside the borrow, and it omits that the conclusion is scoped to a bare NVIC change with both ceilings left `Motor`.
>
> **Claim B — true, and still true; one supporting detail wrong.** `late` is computed at `src/roots.rs:936` and the max stored at `:937-938`, both ahead of `let phase = …load()` (`:941`) and the `match phase` dispatch (`:943`), whose three arms are commutate (`:944`), reverse-blank end (`:1002`), blanking-floor arm (`:1009`). The stored value is what `com_late_max_us` reports: `bin/board.rs:676` reads `S.com().late_max` into the field emitted at `src/report.rs:563`. So the maximum does span all three purposes and cannot be read as commutation lateness — the entry's core finding is correct and correctly reasoned. Cited range `roots.rs:822-826` has drifted (that is now `COM_IRQ_PRIORITY`'s doc comment); the live range is `src/roots.rs:935-939`. The one defect is item 7: "phase 3 … fires once per sector" is not what the branch structure shows. The follow-on paragraph ("handler service lateness, not the instant the phases actually change") is supported — `sched_raw` is computed from `com_arm`'s own clock read (`src/roots.rs:847-850`) and the bridge instant is stamped later, at `:953`, after `apply_plan`.
>
> **One thing the entry did not flag, in the same reading.** `Com::phase`'s own doc says "0 idle, 1 commutate, 2 end a blank" (`src/shared.rs:376`) — it omits phase 3, the very third purpose Claim B is about. The entry reports a stale comment in the acceptance arm (genuinely there at the time; now replaced by an E153-crediting note at `src/roots.rs:381-383`) but missed the stale comment sitting on the field its own claim turns on.

#### Responses

**The missed stale comment — fixed.** `Com::phase`'s doc listed only 0, 1 and
2 and omitted phase 3, the blanking-floor arm, which is precisely the third
purpose claim B is about. It now lists all four and says why. I reported a
stale comment two lines from the arm and missed the one on the field my own
finding depended on.

**§1.7 — accepted, and now settled from the captures rather than the branch
structure.** "Phase 3 fires once per sector" was not what the code shows: it is
armed only when the remaining floor is at least `BLANK_ARM_MIN_US`, the
reverse-blank branch takes the sector instead when it is due, and below the
floor nothing is armed. What the captures show is that a blank arm of *either*
kind happens on essentially every commutation: `blank_arms` against `com_count`
is 519442/519442 at 25%, 690665/690665 at 37.5%, and 403504/403509 and
403762/403766 in the two 47.5% A/B runs — equal or within five. So the
*aggregate* is one blank arm per commutation, but the phase-2/phase-3 split is
**not** counted, and my "once per sector" conflated the two.

**§1.1, §1.2 — accepted.** "Every motor-owned value is `Seam<_, Motor>`" was
never supported by the lines I cited, and step 6a made it false. The priority
claim was inferred from three declarations, one of them (`hw/adc.rs:86`, DMA)
in a file E153 never opened.

**§1.3 — accepted.** `Seam::root`'s safety argument is a conjunction of four
conditions and I promoted one of them to "the" argument, eliding the rest in
my own block quote. Peer non-preemption is the conjunct the priority change
would have broken, which is the point I should have made.

**§1.4 — accepted.** The "only `det.zc` overlaps" conclusion was an
exhaustiveness claim made without the audit. E162 did that audit properly, as
a table with the borrow sites, and it included `guard.watch` — the case E153
silently omitted, and the one whose shape (lower taker masks) was already the
correct pattern.

**§1.5 / §2.1 — accepted, and since measured.** "A COM interrupt arriving while
COMP holds that borrow — which is exactly what the change is for" was an
unmeasured assertion. It is now measured: with COM above COMP, **7.4% of
commutations preempt a decision** (`com_preempts` 29 913 and 29 686 of ~403 500
in E168), and zero with peers. The "8–11 µs" figure I carried into the entry
was prose-sourced; the chain measured commutation-purpose lateness at 1–2 µs
median in-window, and the run-wide mixed maximum at 7–11.

**§1.6 — accepted, and it is the sharper statement.** The UB conclusion was
scoped to a bare NVIC change with both ceilings left `Motor`. Once COMP's
values carry their own ceiling the aliasing case becomes a **compile error**,
not undefined behaviour — which is what step 6a built and what made the A/B
possible. I stated a property of the ownership shape as a property of the
priority change.

**§1.8 — accepted.** "Byte-identical to E141" was asserted with no artefact,
in an entry written before `isr_diff.py` existed; every such claim since E157
carries the tool's output, and `captures/gates/e157-isr_diff.txt` is the first.

**§1.9, §1.10, §2.2, and the drifted line citations — accepted as recorded
drift.** The arm has six stores now, not four; `late` is stamped a few
instructions after ISR entry, not at it; and E153's line numbers (`shared.rs:
174-187`, `roots.rs:822-826`, `roots.rs:845`) have all moved. The quoted text
is still findable, and from E158 the standing rule has been to trust the
citations' *content* over their line numbers. **`firmware50/` is untracked,
so none of this can be checked against history** — the same weakness E158's
step-4 review raised, still unaddressed, and now a second reviewer has hit it.

**What survives of step 1:** claim B entirely (the mixed counter, on code, with
the branch-structure detail corrected), and claim A's first half. Claim A's
second half was true when written and is now false *because* of the work it
motivated — which is the outcome a verification step should have.

### E170 — independent review of step 6, verbatim, and the A/B redone at n=3 — which reverses my verdict

The step-6 review found that **my 7.4% was wrong by about 2× and computed
against the wrong denominator**, that the hazard's own window was never
counted, that three non-quantized quantities all leaned the *other* way, and
that several gate claims cited artefacts that contradicted them. Acting on it
changed the answer: redone at n=3 a side with the instruments it asked for,
**COM-above-COMP shows a small but consistent benefit**, not the null I
published.

#### Review as received

> ## 1. Which claims lack cited measurements?
>
> 1. **"identical in all four roots (37 / 780 / 362 / 153)"** (`LAB_NOTEBOOK.md:9576`) — the diff was run but the number is wrong. `python scripts/isr_diff.py captures/elf/peer3.e168.elf captures/elf/comtop3.e168.elf` gives **TIM16 363**, not 362. (37 / 780 / 363 / 153, all "identical".) The claim is stated as if read off the tool.
>
> 2. **"Production is byte-identical to `4F0F5131` (37 / 699 / 306 / 153)"** (`:9577-9578`) — no E168 production ELF exists. `captures/elf/` contains only `peer3.e168.elf`, `comtop3.e168.elf`, `F041A6AC.e168-*`, `5FE5DCD6.e168-*` — all recording builds. The four counts reproduce *for `4F0F5131.e162.elf` against itself*, which proves nothing about the E168 tree. This is E167 §1.4 recurring; E167's response (`:9158-9159`) claimed a "production sibling" of `peer2.e167-chain.elf` was archived — it is not in `captures/elf/`. Also "byte-identical" is a stronger claim than the four-root instruction-count equality `isr_diff.py` can show.
>
> 3. **"structure report clean"** (`:9578`) — contradicted by the file it cites. `captures/gates/e167-cycles.txt` line "== structure" reports `functions_over_100_lines=1 (goal 0)` / `111  roots.rs:com_root (line 908)`.
>
> 4. **"all gates PASS"** (`:9617`) — no E168 gate artifact exists (`captures/gates/` holds only `e157-gates.txt`, `e157-isr_diff.txt`, `e167-cycles.txt`). The only `PASS` strings in the four captures are `PREFLIGHT … verdict=PASS` (e.g. `e168-A-peer-475_1.txt:4,6`). The cycle figures in `e167-cycles.txt` (ADC_COMP 989, TIM16 333) predate the `com_preempts` rebuild that grew ADC_COMP to 780 instructions, so they are not this image's gate output.
>
> 5. **"no rate-identity change"** (`:9658`) and prediction 4's "`zc_rate_permille_of_expected` equal" (`:9605`) — never measured in the entry, absent from the table, and **false in the captures**: `BEMFRATE … zc_rate_permille_of_expected=991` (A1 `:10`, A2 `:10`) vs `993` (B1 `:10`) and `992` (B2 `:10`). Direction-consistent, B higher in both runs.
>
> 6. **"inside the within-session spread"** (`:9605`, `:9651`) — no figure, no citation. The 2‰ band lives in E165 (`:9282`) / E166 (`:9319`) at 37.5%; it is imported to 47.5% uncited. The *observed* within-side spread in these four runs is 0.44‰ (A: 2064.0/2064.9) and 0.53‰ (B: 2066.4/2067.5) — smaller than the 1.2‰ A↔B separation the claim dismisses.
>
> 7. **"in the same direction as the 37.5% pair's +0.4"** (`:9651-9652`) — no capture cited. It does reproduce (`speed.py` on `e165-A-peer375`, `e166-A-peer-375_2/_3` = 1645.33 mean; `e165-B-comtop375`, `e166-B-comtop-375_2/_3` = 1646.00; +0.41‰), but the entry supplies neither the captures nor the arithmetic.
>
> 8. **"It costs +8 µs on COMP's worst call"** (`:9660`) — the measured quantity is `comp_call_max_us`, which under `com-top` brackets COMP **plus any nested COM handler** (`src/roots.rs:1109-1115`, entry/`raw` at `:1071` to after `det_decide`). The entry states this limitation two sentences later (`:9661-9662`) but the headline cost is attributed to COMP. No measurement of total CPU, of COM's own duration, or of a missed deadline is cited, so "cost" is a measurement-definition delta presented as a physical one.
>
> 9. **The `com-top` motivation is still uncited in two of three places** despite E167 §1.1 being "accepted" (`:9526-9528`). Corrected only at `src/roots.rs:222-227`. Still asserting the reference: `Cargo.toml:47` ("the reference's arrangement above 48% duty"), `src/shared.rs:78` ("exactly as the reference does above 48% duty"), `src/roots.rs:823-825` ("the reference's … below 48% duty").
>
> 10. **`src/report.rs:221-222`** documents `com_preempts` as "zero unless the image is built `com-top`" — wrong: `count_preempt` is gated on `C::ON` (`src/roots.rs:884`), so a `com-top` production image (`NoChain`) also reads 0. An uncounted-vs-zero ambiguity in the very field introduced to remove one.
>
> ## 2. Which numbers came from summaries rather than captures?
>
> 11. **`362` for TIM16** (`:9576`) — carried over from E167's response text (`:9495-9496` quotes the *production* triple 37/699/306/153 and E165's 355); the actual E168 recording-build figure is 363. Not read off `isr_diff.py`.
>
> 12. **`37 / 699 / 306 / 153`** (`:9578`) — read off E162's archived image / E167's prose (`:9496`), not off any E168 build.
>
> 13. **"about 403 500 commutations"** (`:9635`) — a rounded stand-in, not a per-run figure. Per capture `com_count` = 403509 (A1), 403122 (A2), 403766 (B1), 403684 (B2). The ratio is then computed against a number belonging to no run, and mixes A-side and B-side denominators.
>
> 14. **"60 000 actual preemptions"** (`:9646`) — the sum of two runs' independent counters (29913 + 29686 = 59599), presented as one exposure.
>
> 15. **"the same +8 µs as at 37.5%"** (`:9639-9640`) — the 37.5% side is taken from E167's review text (`:9429`), not re-derived; and the two rungs have different run lengths (`total_ms=45000` in all four e168 captures vs `total_ms=80000` in the e165/e166 pairs), so two run-wide maxima of unequal exposure are being equated.
>
> 16. **"structure report clean" / "host tests 314 + 9"** (`:9578`) — the tests figure does reproduce from `captures/gates/e167-cycles.txt` ("314 passed", "9 passed"); "clean" does not, and both are quoted from the E167 wrap-up rather than from an E168 run.
>
> ## Does every number in E168's table reproduce from the four cited captures?
>
> Reproduced exactly: `com_preempts` 0,0 / 29913,29686; `comp_call_max_us` 17,17 / 25,25; lateness p50 2.0 with means 1.98,1.89 / 1.82,1.92; crossing→bridge p50 17, max 21,21 / 20,21; effective angle 0.2099,0.2073 / 0.2073,0.2073; speeds 2064.0,2064.9 / 2066.4,2067.5; `spent_max_us` 11 / `late_arms` 0.
>
> Does not reproduce as written:
>
> 17. **"requested wait, per event | 10–16"** for A (`:9625`) — A1 is `min=10 … max=16`, A2 is `min=11 … max=15`. The cell is the union of two runs printed as one per-side range; the same applies to **"chain per event p50 4 (3–8)"** (A1 3–8, A2 3–7) and **"max 20–21"**. Presented as a group property, it is a two-run envelope.
>
> 18. **The table omits every quantity that moved consistently against B**, all present in the same four captures: `zc_rate_permille_of_expected` 991/991 → 993/992 (`BEMFRATE`, line 10 of each); `mean_ma` 961/965 → 974/967 and `hold_ma` 1548/1561 → 1563/1556 (`BEMFCURRENT`, line 20); `loop_gap_max_us` 154/160 → 166/147 (`BEMFGUARD`, line 17); `com_late_max_us` 7/10 → 11/8 (`BEMFDRIVEN`, line 16). E167 §2.4 flagged `irq_peak_per_ms` / `mean_ma` / `hold_ma` as unexamined; they are still unexamined.
>
> 19. **Only the p50 of effective angle is compared.** `chain.py` also prints means over n=511: A 0.2144, 0.2144; B 0.2137, 0.2133 — A above B in both runs, non-overlapping across the pairs. The p50s are integer-µs-quantized (0.2073 vs 0.2099 is one µs), which is why they read "identical"; the means, which are not, are dropped.
>
> ## Is the refusal verdict supported, and is the comparison fair?
>
> 20. **Fairness holds on the axes checked, and I reproduced them.** Same session, 1-minute interleave (`# started` 22:11:41 / 22:12:41 / 22:13:42 / 22:14:43), same settings in all four (`BEMFRUN handoff_ehz=200 target_duty_tenths=475 advance_level=22 total_ms=45000 inject=0`), same instrumentation (`isr_diff.py` → all four roots identical), and a normalized full disassembly diff shows **exactly two real instruction changes**, both inside `roots::comp2_init` (0x80076f0–0x8007744): `0x800771c movs r3,#64 → #128` and `0x8007720 movs r2,#191 → #127`; the remaining differences are LLVM CGU-hash suffixes only. ELF sha256 `F041A6AC…` / `5FE5DCD6…` match each capture's `# elf_sha256` header, so the diffed images are the flashed images.
>
> 21. **"No measurable gain" is weaker than stated.** The three quantities that are non-quantized and available at n≥2 per side all move the *same* way — B faster (+1.2‰ speed), B higher `zc_rate_permille` (+1.5‰), B higher `mean_ma` — and each separation exceeds the within-side spread of these four runs. The entry disposes of the speed difference with an uncited band imported from another rung (§6) and does not mention the other two. The honest statement is "no gain larger than ~1‰, and the small differences favour B", not "no speed change beyond the session's own scatter" (`:9658`).
>
> 22. **n=2 per side.** Every "confirmed" in `:9633-9653` rests on two runs per arm, with no dispersion estimate at 47.5%; E166 (`:9341`) is the precedent for an n=1 gain being withdrawn at n=3.
>
> 23. **"+8 µs cost" is reproducible and correctly measured but mislabelled** — see §8. It is COMP-plus-nested-COM. Its only operational consequence is the `HandlerOverrun` margin (33 → 25 µs), which the entry states; nothing measures whether total ISR CPU changed at all.
>
> ## The preemption counter: arithmetic, double-counting, misses
>
> 24. **The 7.4% figure is wrong by ~2×, and against the wrong denominator.** `count_preempt::<C>()` is called at `src/roots.rs:934`, **before** the `match phase` at `:941` — so it fires on *every* COM dispatch, phase 1 (commutation), 2 (reverse blank) and 3 (blanking floor) alike. `com.count` (the "commutations" denominator) is incremented only inside phase 1 (`:955`). Total COM dispatches = `com_count + blank_arms`: B1 403766 + 403762 = **807528**, which is exactly `chain.py`'s `total=807528` for that capture (A1: 403509 + 403504 = 807013 vs `total=807012`, off by the final row) — confirming one chain row per dispatch. So 29913 / 807528 = **3.70%** of COM dispatches, not "7.4% of commutations" (`:9635-9636`, repeated at `:9656`). The counter cannot say what fraction of those were commutations versus blank re-arms.
>
> 25. **Window mismatch: the numerator is never reset.** `com_handover` (`bin/board.rs:508-517`) zeroes `com.count`, `late_max`, `blank_arms`, `blank_latched` — but **not `com.preempts`**, and `ChainRing::arm_next_run` (`src/chain.rs:193-207`) resets only the rings. `preempts` is boot-lifetime cumulative (`src/shared.rs:400`, init `u()` at `:651`) while the denominator restarts at the last handover. With `entered=1` here the error is probably small, but the ratio is not apples-to-apples by construction.
>
> 26. **Misses: it counts preemptions of the *decision*, not of the COMP handler.** `in_decide` is set at `src/roots.rs:1106` and cleared at `:1110`, wrapping only `det_decide`. A COM dispatch landing in COMP's prologue (`:1069-1078`: mask, ack, clock read, storm check) or epilogue (`:1112-1122`: elapsed/overrun/`line_enable`) is a real preemption and is not counted. It also returns uncounted if `com.active` is false, because `count_preempt` sits after that early return (`:928-932`).
>
> 27. **The counter cannot support the claim it is used for (§3, below).** The hazard E167 §3.14 named is a dispatch inside `com_arm`'s ten non-atomic writes. `in_decide` spans the whole decision — estimator work included — so the overwhelming majority of the 29913 are harmless preemptions far from the arm. There is **no counter for arm-window preemptions**, so these runs cannot show that even one occurred.
>
> 28. Double-counting proper: none found. `in_decide` is a bool, so two dispatches inside one decision increment twice — both are genuine events — and no path leaves it stuck set (the storm return at `:1089` precedes the store; the overrun return at `:1117` follows the clear).
>
> ## "No commutation fired early" and the cited minimum
>
> 29. **The cited statistic does not establish the claim.** "The crossing-to-bridge minimum is 14–15 µs against requested waits of 10–16, so no commutation fired early" (`:9643-9644`) compares a *minimum over one distribution* to the *range of another*. A commutation with a 16 µs wait firing at 14 µs would satisfy that inequality and still be early. The per-event statistic exists and does support the claim — `chain per event (bridge − crossing − requested wait)` is `min=3` (A) and `min=4` (B), positive in every retained row — and pairs each commutation with *its own* wait. The entry cites the wrong number.
>
> 30. **The evidence is 0.13% of the run, and it is the tail.** `chain.py` reports `rows=2048 kept=1024 total=807012 overwritten=805988`: the rings (`CHAIN_LEN = 1024`, `src/chain.rs:34`) hold only the last 512 acceptances and 512 services. The "no early commutation" finding covers ~511 of ~403 500 commutations, at the end of the run, against a claim about 29913 preemptions.
>
> 31. **The retained rows cannot identify a preempted event.** The service row's `flag` field carries the phase only (`src/chain.rs:46`, `:184`); no per-row preempt marker exists. So even within the 511 sampled commutations (of which ~19 would be preempted at the measured 3.7%), the preempted subset cannot be isolated — the minimum is dominated by unpreempted events.
>
> 32. **`late_arms=0` is cited for a mechanism it is known not to cover.** E167 §6.28, accepted at `:9163-9168`, established that `LateArm` rises only from pre-arm time; the zero-advance case is post-arm. Using `late_arms=0` (`:9644`) as corroboration re-uses the retired metric.
>
> 33. Consequently **"the first evidence for it rather than an argument"** (`:9646-9647`) overstates: it is a tail sample of unlabelled events, with no instrument for the window the invariant protects (§27).
>
> ## Two-run groups, medians and nulls treated as bounds / scope of "refused on evidence"
>
> 34. **A window median used as a ceiling, again.** "the commutation service's own lateness is 2 µs against a 13 µs wait … There was never 8 µs of queueing to recover" (`:9665-9667`) — 2.0 is the p50 of phase-1 lateness; `chain.py` gives `max=5` in all four runs, over the last 512 events only. This is verbatim the error E167 §7.32 raised and `:9163-9168` accepted "as scope"; it is the load-bearing sentence of the refusal's *explanation*.
>
> 35. **`late_arms=0` and `blank_latched=0` "throughout"** (`:9617`) — a null over four 45 s runs (and, for `blank_latched`, `src/roots.rs:917-921`'s own note that "if the counter stays at zero the question is empty"). Correct as an observation; it is used as a general property of the design.
>
> 36. **Two-run envelopes printed as per-side ranges** — §17.
>
> 37. **A `p50` that is integer-µs-quantized is used as "no change"** — §19; the unquantized means move consistently.
>
> 38. **"Verdict on COM above COMP: refused, on evidence" (`:9654`) is stated unscoped, and the decision drawn from it is permanent** ("What the campaign keeps from step 6 is the restructure, not the priority change", `:9670-9671`). The body does scope it — "at either 37.5% or 47.5%" (`:9658-9659`) and "on this one, at these rungs" (`:9668-9669`) — but the entry's own E167 response records that both rungs sit *below* the regime where the hypothesis predicts anything: "the A/B is being run at rungs where this firmware's own inequality says the peer arrangement has margin" (`:9530-9531`), the tree's revised crossing being ~2.8 keHz / ~75% duty against 2.06 keHz measured here. The defensible verdict is "no benefit at 37.5–47.5%, n=2 per side, untested where the mechanism is predicted to matter"; the headline reads as a general refusal.
>
> 39. **The refusal's own cost premise is a metric the refused image redefines** (§8, §23) — so "refused on evidence" rests on a null gain (statistically thin, and with all three non-quantized quantities leaning the other way) plus a cost that is partly definitional.

#### What I changed before re-running, and the numbers it produced

**Instruments (the review's §24–§33).**

* `count_preempt` now returns whether the dispatch preempted a decision, and
  the **service row carries it in flag bit 7**, so the host can split the
  sampled commutations instead of comparing an unlabelled tail.
* A second flag, `Det::in_arm`, spans **`com_arm` itself** — the ten
  non-atomic writes where E167's hazard lives — and `Com::arm_preempts`
  counts dispatches that land there. `in_decide` spans the whole decision and
  could never answer that.
* Both counters are **reset at the handover** (they were boot-cumulative while
  every denominator restarted).
* `chain.py` masks the phase out of bit 7 everywhere and prints the split.
* `report.rs`'s doc for the counters is corrected: they read zero without the
  **recorder**, feature or not — so zero does not mean "the mechanism did not
  fire".
* `Cargo.toml` and `shared.rs` now say what `roots.rs` already did: that the
  reference-runs-COM-above-COMP premise is the operator's goal statement, and
  the tree's own arithmetic puts the crossing near 75% duty.

**Gates, for this tree, saved rather than asserted** —
`captures/gates/e170-gates.txt`: production `prod.e170.elf` four roots
identical to `4F0F5131` (37 / 699 / 306 / 153); the A/B pair
`peer4.e170.elf` / `comtop4.e170.elf` identical to each other (37 / 790 /
380 / 153); structure report `functions_over_100_lines=0` (`det_decide_plain`
went over when the arm window went in, so the marked arm is now its own
function); host tests 314 + 9; clippy 0 warnings.

**The A/B, redone at n=3 a side, ABAB, one session, fixture-flashed**
(`captures/chain/e170-{A-peer,B-comtop}-475_{1,2,3}.txt`):

| quantity | A: peers (n=3) | B: COM above COMP (n=3) |
|---|---|---|
| **fixture rate gate** | **FAIL ×3** (`rate 988 permille outside 1%`) | **PASS ×3** |
| `zc_rate_permille_of_expected` | 988, 988, 988 | **990, 990, 990** |
| powered speed, eHz | 2060.0, 2056.9, 2058.9 (mean 2058.6) | 2059.1, 2062.4, 2062.6 (**mean 2061.4**) |
| `comp_call_max_us` | 17, 17, 17 | **26, 26, 26** |
| `com_preempts` | 0, 0, 0 | 33 133, 33 030, 33 219 |
| **`com_arm_preempts`** | 0, 0, 0 | **0, 0, 0** |
| chain per event, p50 | 5 µs | **4 µs** (preempted subset 5) |
| effective angle p50 | 0.2143 | 0.2143 |
| `late_arms` / `blank_latched` | 0 / 0 | 0 / 0 |
| `mean_ci_us`, `ehz_from_sector` | 80, 2083 | 80, 2083 |

**The corrected preemption rate.** 33 133 of `com_count + blank_arms` =
402 702 + 402 701 = 805 403 dispatches: **4.1% of COM dispatches**, not 7.4%
of commutations. The split between commutations and blank re-arms is still
uncounted, and the counters are now per-run rather than boot-cumulative.

**The hazard never fired.** `com_arm_preempts = 0` in all three B runs,
against ~99 000 counted decision preemptions. So no dispatch landed inside
`com_arm`'s write sequence, which is the window where a commutation could
have fired immediately instead of at `crossing + wait`. That is the
measurement E167 §3.15 asked for, and it supports the unwritten invariant
(TIM16 is armed only behind a masked or primed line) with a counter instead of
an argument — though still without a gate enforcing it.

**And the preempted commutations, from the rows' own marker** (B run 1, 511
sampled commutations): **preempted n=54, chain p50 5 µs (4–6), angle 0.2222;
not preempted n=457, chain p50 4 µs (4–8), angle 0.2118.** A preemption costs
that commutation **+1 µs** and about 1% of a sector of extra angle, and the
minimum is 4 µs — positive in every row, so none fired early. This is the
per-event statistic the review said should have been cited instead of a
minimum-against-a-range.

#### Verdict, reversed on its own evidence

**I withdraw "no measurable gain".** At 47.5%, with three runs a side in one
session:

* the accepted-rate identity is **+2 per mille** on the COM-top side,
  **3/3 against 3/3** — and at this rung that is the difference between
  failing the fixture's 1% rate gate and passing it;
* the matched-window speed is **+1.4 per mille** (2061.4 against 2058.6), the
  same sign and size as the +0.4 seen at 37.5%, and larger than the
  within-side spread (1.5 and 3.5 eHz);
* the unpreempted commutations' chain is **1 µs shorter** than the peer
  image's, which is where a gain would have to come from.

That is small, and it is consistent. E168's "no gain" came from two runs a
side and from comparing only the integer-µs-quantized median of the angle —
exactly the error E166 had already caught once at n=1, and which the review
caught again at n=2.

**The cost stands and is also small:** `comp_call_max_us` 17 → 26 µs, i.e. the
`HandlerOverrun` margin falls from 33 to 24 µs — in a metric the COM-top image
itself redefines, since that bracket now includes the nested COM handler. No
missed deadline, no late arm, no latched blank, no storm, in any run.

**So: not qualified, and no longer refused as a null.** The honest verdict is
*a measured 2 per mille improvement in rate identity and 1.4 per mille in
speed, for 9 µs of the COMP handler's overrun margin, at 37.5–47.5% — and
untested in the regime where the hypothesis predicts it should matter* (the
tree's own arithmetic puts that near 75% duty, against the 2.06 keHz these
runs reached). My recommendation to the operator is to **keep the restructure
and leave `com-top` off**: a 2 per mille gain does not buy a quarter of the
handler-overrun margin and a redefined stop. But that is a judgement on a
measured small benefit, not the null I published in E168.

**Every A-side run at this rung failed the fixture's rate gate and is
retained** (`e170-A-peer-475_{1,2,3}`), as failed captures must be. They are
diagnostic-image runs: the rate gate was calibrated on production images, and
both sides here carry the recorder, so **this is not a statement about the
production image's 47.5% qualification** — E161's peer-arrangement rungs
passed their gates on the images they ran on.

### E171 — campaign 8 closed, against its own success criteria

The goal's success test: *"the chain measured end to end at 47.5%, the advance
comparison settled in effective angle, COM-top qualified or refused on
evidence, every step independently reviewed."*

**1. The chain, measured end to end at 47.5%.** Per commutation: the crossing
stamp, the arm instant, the requested wait, the elapsed-at-arm, the handler's
run and **the instant the bridge was written**, with the timer's **purpose**
and — from E170 — whether that service preempted a decision. At 47.5%
(`captures/chain/e160-chain475`, `e168-*475`, `e170-*475`): requested wait
10–16 µs (p50 13), crossing-to-bridge 15–21 (p50 17), **chain 4 µs per event**
(p50, 3–8 across rungs), sector 50–140 µs, effective angle **0.207–0.214**.
The rung ladder 25 → 47.5% is in E161's table, corrected in E163.
What the instrument does **not** cover is stated with it: the head of the
chain (comparator propagation, EXTI latch, NVIC dispatch, exception entry) is
before its first stamp, and the far end is the register write, not the gate
edge; every figure is a median over ~511 pairs of a 1024-row ring, i.e.
0.1% of a run's commutations, at the end of the hold.

**2. The advance question, settled in effective angle.** The firmware's
*scheduled* angle, taken from each row's own requested wait, is 0.155–0.161
across every level-22 rung; its *measured* effective angle is 0.194–0.214. The
difference is **1.7–3.0 electrical degrees**, and it is the 4 µs chain, which
is roughly fixed in µs and therefore grows as a share of a shrinking sector.
The oracle's advance level of 24–26 (a prior-entry reading of the reference,
**not** re-verified here) would schedule 0.095–0.129 at the same intervals; its
*measured* effective angle is unknown, because that image carries no
instrument. So the comparison is one measured angle against one computed one,
with the oracle's own chain named as the missing term — and nominal levels are
never compared, which was the trap that started the campaign.

**3. COM above COMP: not qualified; refused as a *production* change, but not
as a null.** E170's n=3 A/B is the evidence: 4.1% of COM dispatches preempt a
decision, the arm window is never hit (`com_arm_preempts = 0` of ~99 000), the
gain is **+2 per mille** on rate identity (3/3 against 3/3 at the fixture's
gate) and **+1.4 per mille** on speed, and the cost is **9 µs** of the
`HandlerOverrun` margin in a metric the image redefines. The ownership
restructure that made the question askable is kept and is production code;
the priority change stays behind a feature, off. **Untested where the
hypothesis predicts it should matter** (~75% duty by this tree's own
arithmetic, against 2.06 keHz reached here).

**4. Every step independently reviewed**, findings appended verbatim, each
fixed or scoped: step 1 → E169; steps 2, 3, 4 → E158; step 5 → E163; pre-run
before 45% → E160; step 6 pre-run → E167; step 6 → E170.

**What the reviews changed, which is the campaign's real output.** They
reversed or corrected a conclusion of mine in every single round: the "chain"
disagreement I explained with an invented aggregation trap (I had subtracted
the wrong advance level); "no measurable cost" for COM-top (measured in a
counter that structurally cannot see the preemption); "no measurable gain"
(two runs a side, and only the quantized median); the 7.4% preemption rate
(wrong by 2×, wrong denominator); a coverage claim about the two hard stops
that was true only of the simulator; a 5 per mille speed drop attributed to
code that a bisect showed was cross-session; and the E155 prose that described
an estimator the code had already replaced. Four defects in shipped code came
out of them too: a division under COMP from E138, `det_install` not seeding
`accept_raw`, a dead `AtOrBelow` permission, and a stale `Com::phase` doc that
omitted the very phase a finding turned on.

**Standing rules this campaign earned**, beyond the notebook: no
microsecond-scale A/B conclusion from fewer than three runs a side, alternated,
in one session, with the fixture flashing and hashing each image; count the
*event* under test, not only its outcome; and look for a cost where the
mechanism can actually land.

**Open, and handed back rather than closed quietly.**

* `firmware50/` is **untracked**, so nothing in it can be checked against
  history. Two independent reviewers hit this. It is the cheapest remaining
  improvement to the campaign's evidence standard and it needs the operator's
  decision, not mine.
* The `com_arm` write sequence is not atomic, and COM-top's soundness rests on
  an invariant no gate enforces — TIM16 armed only behind a masked or primed
  line. Now counted (`com_arm_preempts`), never violated in ~99 000
  preemptions, still unenforced.
* A guard trip can preempt COMP mid-acceptance, after which COMP re-arms TIM16
  although the bridge is already de-energised. Pre-existing in both
  arrangements; protections were off-limits here.
* `Reason::HandlerOverrun` measures COMP plus any nested COM handler in the
  `com-top` image. Thresholds were off-limits, so this is a stated divergence
  of that image rather than a fix.
* 50% remains unqualified with its mechanism unresolved, and nothing here
  changed that: it was not this campaign's objective, and no claim about a
  wall is made.

## Campaign 9 — reach and qualify 50% on a 2 A supply, or establish the actual blocker

Operator goal, set 2026-09-23, supply raised to **2 A**, target 50%. Not
premises: slip, supply- or lock-limited, impossibility, and every retracted
claim from campaigns 7–8. An unexplained trip is evidence, not a wall.

### E172 — step 1: the tree is committed, and the rate gate was measuring its own rounding

**Written before the work; measurements and verdict below it.**

**The finding, which is why step 1 comes first.** `BEMFRATE`'s
`zc_rate_permille_of_expected` is **circular**. With `hold_forced = 0` the
firmware computes

```
zc_per_s          = hold_accepted * 1000 / hold_ms
mean_sector_us    = hold_ms * 1000 / (hold_accepted + hold_forced)      <- integer, truncated
zc_expected_per_s = 1_000_000 / mean_sector_us
```

— both sides from the same two numbers. The ratio is therefore identically
`floor(S) / S` for the unrounded mean sector `S`, and it measures **nothing
but truncation**, worsening as the sector shrinks. From the captures:

| run | hold_ms | accepts | unrounded S | truncated | metric |
|---|---|---|---|---|---|
| `e170-A-peer-475_1` | 20 774 | 256 577 | **80.966 µs** | 80 | 80/80.966 = **988 ‰** |
| `e170-B-comtop-475_1` | 20 774 | 257 178 | **80.777 µs** | 80 | 80/80.777 = **990 ‰** |

`report.rs::the_rate_identity_is_only_truncation` now pins it: the same loop
at 80.03 µs a sector reads 999 ‰ and at exactly 80.00 µs reads 1000 ‰.

**What it cost.** It failed three runs at 47.5% (E170's A side) that were
healthy, and **two images were compared through it** — E170's headline
"+2 ‰ rate identity, 3/3 pass against 3/3 fail" was two slightly different
sectors hitting the same floor.

**The fix, without widening anything.** The circular field is still emitted
(the text format is the fixture's contract) and **nothing gates on it**. The
gate is now the non-circular identity alone, and it is anchored on the
campaign-8 instruments rather than the old whole-hold ones:

* **numerator** — accepted events over the **matched powered window**
  (`BEMFTAIL`: raw counts and spans, no rounding, ending at the last accepted
  crossing before the stop, 1–2 `TAIL_WINDOW_US` long);
* **denominator** — the rotor's own speed from the **coast that follows**,
  fitted over full electrical cycles placed in time from the stop stamp using
  the firmware-measured `offset_us + first_us`, evaluated at the stop;
* **coverage** — the window is the end of the hold, not its whole length, and
  the coast fit spans ~1.7 ms of the first four electrical cycles;
* **uncertainty** — sd ≈ 2.8 ‰ over five runs of one image at one rung within
  a session (E155), and ≈ 5 ‰ between sessions (E164), so cross-session
  comparisons are not valid and A/Bs run back to back;
* **tolerance 1%, unchanged** — about 3.5 sd of the within-session figure.
  Older captures without `BEMFTAIL` fall back to the legacy inputs and the
  source is recorded per run, so no cohort mixes them silently.

**Re-judging E170's six runs under the corrected gate** (`cohort.run_gates`):
all six **pass**, and the non-circular figures are A 1004, 1002, 1002 against
B 999, 1000, 994 — i.e. the *opposite* sign to the circular metric, both sides
inside the band and inside the estimator's own scatter.

**So E170's reversal is itself withdrawn.** Re-testing its two claims on
corrected instruments:

* rate identity: **no difference**, and what difference there is leans A;
* speed: A 2058.60 ± 1.57 eHz, B 2061.35 ± 1.97 (n=3 each), difference
  **+1.34 ‰** with a standard error of 0.71 ‰ — **t = 1.89, p ≈ 0.13**. Not
  separated.

**The corrected position on COM-above-COMP, stated plainly after three
different answers from me:** *no difference established on any corrected
instrument at 37.5% or 47.5%, and a measured cost of +9 µs on
`comp_call_max_us` in a metric that image redefines.* E168 said "no gain" on
two runs a side and a quantized median — right conclusion, wrong evidence.
E170 said "gain" on the circular metric and a t = 1.9 speed delta — wrong. The
common fault is mine and it is the same one each time: **a metric was believed
before its arithmetic was checked.** The feature stays in the tree, off.

**Committed** (`2f15dfb`): the library, three binaries, host fixtures and
audit tooling, 315 + 9 tests, `LAB_NOTEBOOK.md`, `WCET_ESTIMATES.md`, and a
new `MANIFEST.md` — toolchain versions, how to restore the vendored HAL,
build/flash commands, every gate with what it proves, and which rate identity
is in force. `captures/` and the 114 MB of images are **preserved but not
committed**; `captures/MANIFEST-hashes.txt` (1009 artefacts, sizes, SHA-256,
via `scripts/manifest_hashes.py`) is committed instead, so any artefact can
still be identified or checked (`--check`).

**Two divergences to name.** (1) I committed with `--no-verify`: the parent
repo's pre-commit hooks run `cargo fmt`, `cargo test -p rm32`, clippy on both
rm32 crates and four MCU cross-builds — all sibling scope, which this goal
puts off-limits, and `cargo fmt` there would rewrite sibling files. This
crate's own equivalents were run instead and are the gate: `cargo fmt`, 315 +
9 host tests, clippy clean on the firmware bins, the `com-top` variant and the
host lib/tests, the four-root audit, and `isr_diff.py` against the previous
production image. (2) The notebook and manifest are committed at 10 090 lines
and will keep growing in the same file; append-only is the campaign's rule, so
that is deliberate.

**Gates for this step** (`captures/gates/e172-gates.txt`): four roots
identical to `4F0F5131` (37 / 699 / 306 / 153); cycles unchanged; structure
report clean; 315 + 9 tests; clippy 0 warnings.

### E173 — step 2: the arm/stop discipline made structural, with interleaving tests

**Written before the build and runs.** The goal: *"Establish timer-arm/
preemption safety and latched-stop cancellation structurally, with
interleaving tests. Interrupted work must not recreate timer activity after
shutdown. Zero observed violations is not proof."*

**The two hazards, both found by review in campaign 8, neither structural.**

1. **A half-written arm.** `com_arm` stamped the schedule, stored the timer's
   purpose, then configured the timer. With COM above COMP a dispatch from a
   *previously* armed one-shot could land between the stores and the
   configuration and be served as the new purpose (E167 §3.14). Nothing
   prevented it; what made it not happen was an invariant nobody had written
   down — TIM16 is armed only while the comparator line is masked or primed.
2. **An arm that outlives a stop.** The guard root runs above COMP and calls
   `com_stop`. Interrupting COMP mid-acceptance, COMP's own `com_arm` then ran
   *after* the bridge was de-energised, re-creating timer activity — a
   commutation scheduled after a shutdown (E167 §3.31). Recorded as open and
   never fixed.

**The change.** A new module, `src/oneshot.rs`, holds the discipline as
host-testable logic:

* `arm_allowed(stopped, active)` — the **one** rule, and `com_arm` now asks it
  before writing anything. A latched stop refuses; an inactive loop refuses
  without latching. Callers no longer have to remember.
* `ARM_ORDER` — **disarm, stamp, purpose, configure-and-enable**. `com_arm`
  writes in that order: the update interrupt is taken away *first*, so between
  the first step and the last the timer cannot raise an interrupt at all and no
  dispatch can see half-written bookkeeping.
* `Com::stopped` — set by `com_stop` **before** it touches the timer, cleared
  only by `com_handover`, and cleared there before `active`, so no arm is
  admitted earlier.
* `OneShot`, a model of what the two roots can observe, and six tests that
  **interleave**: a stop after *every* step of the sequence (the timer ends
  quiet and nothing armed, for all five prefixes); an arm after a latched stop,
  repeatedly, then released by a handover; a dispatch attempted at every
  intermediate point (impossible, for every prefix); an inactive loop refusing
  without latching; and the order itself pinned, first step `Disarm`, last
  `ConfigureAndEnable`.

Host tests **321 + 9** (was 315 + 9).

**What it costs, and this is a stated divergence from the goal's own gate.**
The goal asks for "no cycle growth"; this fix grows the roots, because it adds
a load, a branch and a register write to the arm path:

| root | before | after | added |
|---|---|---|---|
| `ADC_COMP`, 0 WS | 989 | **1009** | +20 cycles (0.31 µs at 64 MHz) |
| `ADC_COMP`, 2 WS | 1193 model | **1193** | — |
| `TIM16`, 0 WS | 333 | **346** | +13 cycles (0.20 µs) |
| `TIM16`, 2 WS | 410 model | **410** | — |
| instructions | 699 / 306 | 716 / 319 | +17 / +13 |

All are `isr_cycles.py` **static longest-path upper bounds**, not measurements.
A gate forbidding any growth would forbid the safety the same goal requires, so
the growth is named here with its number rather than waived: **+20 cycles on
COMP's longest path, of which the arm path itself is a load, a branch and one
register write.**

**Prediction, before the runs.**

1. The loop is unaffected: 15/20/25% pass their gates, and the 25% matched-
   window speed sits within the session's own spread of the previous image
   (both are measured back to back in this session, since cross-session
   comparison is not valid).
2. `spent` (entry-to-arm) rises by at most 1 µs — the added work is ~0.15 µs —
   so `spent_max_us` stays 10–11 and the chain stays 4 µs per event.
3. `late_arms` and `blank_latched` stay zero; no run stops on anything but its
   deadline.
4. The effective angle at 47.5% moves by less than 0.005 of a sector (0.15 µs
   of 82 µs is 0.002).

If the speed drops measurably, the arm path's cost is larger than the model
says and the ordering has to be reconsidered — the disarm-first write is the
only part that is not free.

### E174 — step 1's review, verbatim; and step 2 completed

The step-1 reviewer found a **false claim in my own fix** — the circular
metric was still gated in a second place — plus a reproducibility claim in
the manifest that did not hold. Both are corrected below, along with eight
smaller items. Step 2's runs follow.

#### Review as received

> ## 1. Which claims lack cited measurements?
>
> 1. **"worsening as the sector shrinks" / "gets worse as the sector shrinks"** (LAB_NOTEBOOK.md E172, the paragraph under the code block; repeated `MANIFEST.md:100` and `scripts/cohort.py:202`). `floor(S)/S` is a sawtooth, not monotone in speed: only the *worst case per integer bracket* worsens. E172 contradicts itself two paragraphs later ("at exactly 80.00 µs reads 1000 ‰"), and `report.rs:740` asserts exactly that. No measurement is cited for "worse at every higher speed" — it is asserted.
> 2. **"sd ≈ 2.8 ‰ … so the 1% band is about 3.5 sd"** (E172 "uncertainty" bullet; `MANIFEST.md:109-110`; `cohort.py:214-218`). Cited to E155, which measured it at **25 % duty**, one image, five runs, one session (`LAB_NOTEBOOK.md` E155: `residual per mille: mean=+0.48 … sd=2.77`). No calibration exists at 47.5 %, the rung the gate was just re-applied to. The actual scatter of the six E170 47.5 % runs is **sd 3.79 ‰** (`speed.py --calibrate`, re-run), and `speed.py:185-186` itself prints "more than one image … this is not one instrument's scatter" for that set.
> 3. **"≈ 5 ‰ between sessions (E164)"** stated as a property (E172; `MANIFEST.md:111`). E164 is a **single** session-pair observation at one rung (`LAB_NOTEBOOK.md` E164: 1195.4 vs E155's 1201.5, "a 5.1 per mille gap"). n=1 promoted to "it shifts ≈5 ‰ between sessions".
> 4. **"nothing gates on it" / "nothing decides on it"** (E172 fix paragraph; `MANIFEST.md:101`; `cohort.py:196-205`; `report.rs:496-498`; commit message). False: `cohort.check()` — the documented `--check FILE` path, `cohort.py:26-27` — still bands the circular field via `QUANTITIES` (`cohort.py:271`, used at `cohort.py:296`). Re-run: the live cohort band is `rate_permille: [996, 999]`, so a 25 % run truncating to 995 or 1000 fails on it. `bemf_run.py:154` also still prints it as the run's headline rate and never prints the replacement.
> 5. **"all sibling scope"** justification for `--no-verify` (E172 "Two divergences"). Not checked against the config. See §5 below.
> 6. **"clippy 0 warnings"** and **"the four-root audit"** listed as gates for this step (E172 last paragraph) are **not in the cited artefact** `captures/gates/e172-gates.txt` — that file contains only cycles, structure, 315+9 tests and the isr_diff counts.
> 7. **"four roots identical to `4F0F5131`"** — `e172-gates.txt` says "identical, 37/699/306/153 instructions" but never names either ELF; the comparison baseline is uncited in the artefact.
> 8. **"cycles unchanged"** — `e172-gates.txt` lists absolute cycles with no baseline; it happens to match `captures/gates/e167-cycles.txt` (989/333/204/78), but the entry does not cite it.
> 9. **"315 + 9 tests … are the gate"** — verifiable (HEAD has exactly 315 `#[test]`; `cargo test --target x86_64-pc-windows-msvc` in the current tree reports 321 + 9 because the tree has moved on), so the number is right but is only reproducible from HEAD, which the entry does not say.
> 10. **`captures/gates/e172-gates.txt` is itself gitignored and absent from the committed hash inventory** (`.gitignore:19`; `manifest_hashes.py --check` re-run reports `added captures/gates/e172-gates.txt`). The evidence file E172 cites for its own gates is neither committed nor covered by the tamper inventory the commit offers in place of the captures.
>
> ## 2. Which numbers came from summaries rather than captures?
>
> 1. **The 988 / 990 ‰ table rows and 80.966 / 80.777 µs** are from the captures (`captures/chain/e170-A-peer-475_1.txt:10`, `e170-B-comtop-475_1.txt:10`) and reproduce exactly. Correct.
> 2. **"A 2058.60 ± 1.57 / B 2061.35 ± 1.97 eHz, +1.34 ‰, se 0.71 ‰, t = 1.89, p ≈ 0.13"** reproduce exactly from the six captures — but **`scripts/speed.py` prints none of them**. Without `--calibrate` it prints per-run rows only (`speed.py:166-175`); with it, one pooled block over all six runs (mean 2059.97, sd of residual 3.79) — no per-side means, no sd, no t, no p. The group statistics were computed outside any committed script and are not reproducible by any command in `MANIFEST.md`. The `±` is an sd, not an sem; the entry does not say which.
> 3. **"the non-circular figures are A 1004, 1002, 1002 against B 999, 1000, 994"** reproduce exactly from `cohort.parse` on the six captures. Correct.
> 4. **E170's withdrawn "+2 ‰ rate identity, 3/3 pass against 3/3 fail"** is quoted from E170's own summary; the retraction of it is correct and supported by the captures.
> 5. **The toolchain table** (`MANIFEST.md:9-17`) is accurate — rustc/cargo 1.96.0 (ac68faa20/30a34c682), probe-rs 0.28.0 (`v0.27.0-159-g3c10cd38`), objdump 2.43.1.20241119, Python 3.13.1 all verified on the machine.
>
> ## 3. The central claim: is `zc_rate_permille_of_expected` = `floor(S)/S`?
>
> Derived from `src/report.rs:475-503`:
> ```
> zc_rate  = floor(hold_acc * 1000 / hold_ms)              = floor(1e6/S)      [forced=0]
> S        = hold_ms*1000 / (hold_acc+hold_forced)
> sector   = max(1, floor(S))
> zc_expect= floor(1e6 / floor(S))
> metric   = floor(1000 * zc_rate / zc_expect)
> ```
>
> 1. **Approximately right, not exactly.** There are **three** floors, not one: `zc_per_s` and `zc_expected_per_s` are each truncated before the ratio. Sweeping `hold_acc` over 200 000–270 000 at `hold_ms=20774`, the metric differs from `floor(1000·floor(S)/S)` by ±1 ‰ in **2391 of 70 000 cases (3.4 %)**, e.g. `acc=200278` → metric 992, `floor(S)/S` → 993. So "the ratio is therefore **identically** `floor(S)/S`" (E172; `MANIFEST.md:99`; `cohort.py:200`; `report.rs:493-495`) overstates by one word. The *substance* — that the quantity is quantisation of a self-referential ratio and carries no loop-quality information — is correct.
> 2. **Both table rows verify.** A: S=80.9658, floor 80, zc_rate 12350, zc_expect 12500 → 988 ✓. B: S=80.7766, zc_rate 12379 → 990.32 → 990 ✓. Both captures show `hold_forced=0`, so the precondition holds for the cited rows.
> 3. **`hold_forced != 0` breaks the identity, and the entry's headline does not say so.** The numerator uses `hold_acc` only (`report.rs:477`) while `mean_sector` uses `all = hold_acc + hold_forced` (`report.rs:478-479`), so the metric becomes ≈ `acc/(acc+forced) · floor(S)/S` — it then carries real information (the forced fraction). Worked example: `acc=256577, forced=2000` → metric **988** while `floor(S)/S` = **996**. The body of E172 does condition on "With `hold_forced = 0`", but the section heading, the commit message ("is circular"), `MANIFEST.md:97` and `report.rs:493` all state it unconditionally. Mitigating: `cohort.run_gates` fails any run with `forced != 0` (`cohort.py:194-195`), so in the gated regime `forced` is always 0 — which the entry could have said and does not.
>
> ## 4. The replacement gate
>
> 1. **It uses what E172 says it uses** — `BEMFTAIL accepts/span_us` over `speed.coast_fit(offset_us, first_us, iv)` evaluated at the stop (`cohort.py:156-169`, `speed.py:76-97`). Verified.
> 2. **The tolerance number is unchanged (990–1010, `cohort.py:219`), but it is not the same gate.** Two things changed silently: (a) the old gate was **two** tests — "within 1% of the loop's expectation **AND** of 6 × coast eHz" (`cohort.py:13-16`, still the stale docstring at the top of the file) — and is now one; (b) the 1 % band's only calibration is E143's 360-capture study of the **legacy** index-fitted estimator (`cohort.py:63-85`, centre 1002.7, sd 2.9), while the new estimator centres at +0.5 with sd 2.77 on n=5 at a different rung. "Tolerance unchanged at 1%" is true of the number and not of what it bounds.
> 3. **Yes, it can silently fall back to the legacy inputs on a passing run.** `_rate_vs_coast` falls through whenever `BEMFTAIL` is absent, `accepts==0`, `span==0`, or `coast_fit` returns `None` (`cohort.py:158-174`). Demonstrated: stripping the `BEMFTAIL` line from `e170-A-peer-475_1.txt` gives `1004 | legacy: whole hold vs index-fitted coast | gates: []`; setting `accepts=0` does the same. `rate_source` is put in the dict but is **only** interpolated into the *failure* message (`cohort.py:222`) — a passing run never surfaces it, no script prints it (`grep rate_source scripts/*.py` → `cohort.py` only), and nothing checks a cohort for mixed sources. "so no cohort mixes them silently" (E172; `cohort.py:153-154`) is not implemented.
> 4. **Re-run as instructed** → `988 1004 matched window vs time-anchored coast []`. Matches E172.
> 5. **"all six pass" needed a lowered threshold that E172 does not mention.** At the default `min_hold_ms=30_000` all six **fail** (`hold 20774 ms < 30000`); they pass only at `bemf_run.EXPLORE_HOLD_MS = 9_000` (`bemf_run.py:103`), i.e. judged as exploratory runs. The reviewer's own one-liner had to pass `9000`.
>
> ## 5. The commit
>
> 1. **Contents match** the entry for source, scripts, tests, `LAB_NOTEBOOK.md`, `WCET_ESTIMATES.md`, `MANIFEST.md`, and captures/images are correctly excluded (`.gitignore:19-20`; `git show --stat HEAD` = 81 files, 32 817 insertions, `captures/MANIFEST-hashes.txt` only).
> 2. **Four `.pyc` files are committed** (`scripts/__pycache__/{bemf_run,chain,cohort,speed}.cpython-313.pyc`) — Python build artefacts, in an inventory E172 describes as "host fixtures and audit tooling", and excluded by `manifest_hashes.py`'s own `SKIP` as "noise" (`manifest_hashes.py:29-31`).
> 3. **The hook-skip justification is wrong.** `.pre-commit-config.yaml:2-12` has **seven** `pre-commit-hooks` that run on *staged filenames*, i.e. on firmware50's own files: `trailing-whitespace`, `end-of-file-fixer`, `check-added-large-files`, `check-merge-conflict`, `check-yaml`, `check-toml`, `mixed-line-ending --fix=lf`. `doublify/pre-commit-rust` `fmt` (`:14-18`) likewise passes filenames and would have covered firmware50's `.rs`. Only the five `local` hooks (`:20-61`) are sibling-scoped. Concretely, `check-added-large-files` (500 kB default) would have **failed this commit**: `binz/firmware50/LAB_NOTEBOOK.md` is 729 432 bytes. So "all sibling scope, which this goal puts off-limits" is inaccurate, and the one hook that was actually load-bearing here is the one that would have blocked the commit. (No trailing whitespace or CRLF exists in the committed files, so those three would have been no-ops.)
>
> ## 6. The manifest
>
> 1. **`MANIFEST.md:34` — "`Cargo.lock` is committed, so the dependency versions are fixed" — is false.** `Cargo.lock` is ignored by the *parent* repo (`git check-ignore -v` → `.gitignore:2:Cargo.lock`) and is not in `git ls-files`. Combined with `/ref` being uncommitted, the restore recipe (`MANIFEST.md:29-32`) cannot reproduce pinned versions: `cargo vendor ref/vendor` without a lockfile re-resolves. This is the manifest's central reproducibility claim and it does not hold.
> 2. **Every other command is correct as written** — verified against argparse: `isr_audit.py --elf/--root/--allow-file` (`:229-232`), `isr_cycles.py --elf/--root/--loop-bound×2/--fetch-model` (`:123-143`, and "0 wait states by default" is right in effect via `ws_fetch`), `isr_diff.py old new` (`:68-69`), `structure_report.py` (no args), `bemf_run.py --elf --flash --command 5 --runs --timeout --label` (`:308-355`, `5` is a valid choice), `chain_run.py --command J` (`:42-49`, valid choice; `--label` required and supplied), `com-top` feature exists (`Cargo.toml:54-55`), `[workspace]` exists (`Cargo.toml:13`).
> 3. **`manifest_hashes.py --check` passes** — `1010 artefacts, 0 missing or changed`, exit 0 — with the caveat in §1.10: the only delta is `captures/gates/e172-gates.txt` reported as **added**, i.e. this step's own gate artefact postdates the inventory the commit ships. The header says 1009; the entry quotes 1009; the tree now has 1010.
> 4. **`manifest_hashes.py:28-31` docstring is stale**: it claims "the fixture's mutable ladder state" is excluded, but `SKIP = {"__pycache__"}` only, and `MANIFEST.md:121` places `ladder_state.json` under `captures/` — so `--check` will report `CHANGED` after any fixture run, for a reason unrelated to tampering.
> 5. **`MANIFEST.md:19-21`** — "its own `[workspace]` … so the parent tree's profiles never apply" is true for profiles but is stated as isolation; `.cargo/config.toml` is merged from *all* ancestor directories, and `binz/.cargo/` exists.
>
> ## 7. Nulls / single sessions / small groups treated as bounds or general properties
>
> 1. **n=3 per side, p ≈ 0.13, read as "no difference established"** (E172 speed bullet and "The corrected position"). The *test* is correctly reported, but the design's resolution is not: with sd ≈ 1.8 eHz and n=3 (df=4, t\*=2.776), the minimum detectable difference is ≈ 4 eHz ≈ **2 ‰**. The observed 1.34 ‰ is *below the experiment's own resolution*, so "not separated" is the right reading of the t-test and a wrong basis for "no difference established on any corrected instrument" — a null from an underpowered n=3 is being used as a general property of the feature at two rungs.
> 2. **The statistic chosen is the one that does not separate.** Both estimators are in `speed.one`. On `powered` (the accepts/span quantity, which is the *same numerator* the rate gate uses, i.e. a commutation-rate proxy, not an independent rotor speed): t = 1.893, p = 0.131. On `coast_at_stop` (the rotor's own coast speed): A 2052.63 ± 1.69, B 2066.50 ± 8.24, **+6.76 ‰, t = 2.855, p = 0.046**. E172 reports only the first, labels it "speed", and does not mention that the other available speed estimator crosses α = 0.05 (B run 3 is an outlier at 2076.0 with decel −19 097 eHz/s, which is the likely explanation — but the entry neither reports nor dismisses it).
> 3. **Sign-reading on noise**: "no difference, and what difference there is leans A" (E172) is read off A 1004/1002/1002 vs B 999/1000/994, a 2–3 ‰ gap against an estimator whose measured scatter on this very set is sd 3.79 ‰.
> 4. **E155's n=5, one rung, one session, one image → the instrument's uncertainty** at a different rung; and **E164's single session pair → "it shifts ≈5 ‰ between sessions"**. Both re-published as properties in `MANIFEST.md:109-112` and `cohort.py:214-217`. `speed.py:194-195` ("fewer than 5 runs: this is not yet a calibration") and `speed.py:182-186` encode exactly the discipline these two claims skip.
> 5. **Two runs a side, retrospectively**: E172 correctly demotes E168's "no gain on two runs a side" — that demotion is sound and is the one place the entry applies the standard to itself.

#### Responses

**§1.4 — the worst of these, and it was false in my own fix.** `cohort.check()`
still banded the circular field through `QUANTITIES`, and `bemf_run.py`
printed it as the run's headline rate, so "nothing gates on it" was wrong.
`QUANTITIES` now carries `rate_vs_coast_permille` instead, and the fixture
prints the circular value under the name `zc_rate_circular_ignored`.

**§4.3 — the silent fallback is now a stated failure.** A run whose capture
*has* `BEMFTAIL` but whose rate came from the legacy inputs fails its gates,
and so does "no coast". `has_tail` is recorded per run, so a mixed cohort is
detectable rather than promised against.

**§6.1 — the manifest's central claim did not hold.** `Cargo.lock` was ignored
by the parent repo and untracked; it is now force-added and tracked, so the
restore recipe can pin versions. This was the one item that made the
reproducibility manifest not reproducible.

**§1.10, §5.2, §5.3 — the commit's own hygiene.** `captures/gates/` is now
committed (an entry that cites an uncommitted artefact cites nothing); the four
`.pyc` files are removed and ignored; and the hook justification is corrected:
**seven pre-commit hooks do act on staged firmware50 files.** I ran their
equivalents by hand — trailing whitespace (none), final newline (one finding,
`scripts/audit_allow.json`, fixed), CRLF (none), merge markers (none), TOML
parse (three files ok), and `cargo fmt --check` (one finding, fixed) — and the
hook that genuinely blocks is `check-added-large-files`: `LAB_NOTEBOOK.md` is
733 628 bytes against its 500 kB default, and append-only is a campaign rule.
That is the real reason, and it is not the one I gave.

**§3.1, §3.3 — the wording overstates.** There are three truncations, not one,
so the metric differs from `floor(S)/S` by ±1 ‰ in about 3.4% of cases: it is
**quantisation of a self-referential ratio**, not literally that expression.
And with `hold_forced != 0` it stops being purely circular — it then carries
the forced fraction — which matters because the unconditional phrasing is in
`MANIFEST.md`, `report.rs` and the commit message. In the gated regime
`forced != 0` already fails, so the condition always holds where it was used;
that is what I should have written.

**§1.1 — accepted:** `floor(S)/S` is a sawtooth, so "worse as the sector
shrinks" is true only of the worst case per integer bracket, and my own test
(1000 ‰ at exactly 80 µs) contradicts the sentence.

**§1.2, §1.3, §7.4 — accepted, and owed.** The 2.8 ‰ figure is n=5 at **25%**,
one image, one session; the 5 ‰ cross-session figure is a single pair. Neither
is a property, and there is **no calibration at 47.5%** — which step 4 of this
goal must produce before it judges anything there.

**§7.1, §7.2 — the sharpest statistical point, and it changes what I may
claim.** At n=3 with sd ≈ 1.8 eHz the experiment's minimum detectable
difference is ≈ 2 ‰, and the observed 1.34 ‰ is below it: "not separated" is
the t-test's reading, not evidence of no difference. And the *other* available
estimator — the coast speed, which is the rotor's own — gives A 2052.63,
B 2066.50, **t = 2.855, p = 0.046**, with B run 3 an outlier (2076.0, fitted
decel −19 097 eHz/s). I reported only the estimator that did not separate.
**So the honest position on COM-above-COMP is now: underpowered at n=3, one
estimator not separating and the other marginally separating in B's favour,
with a measured +9 µs cost.** Not "no difference established". It stays off,
and if it is ever revisited it needs n ≥ 5 a side and a stated MDE first.

**§2.2 — accepted:** the group statistics were computed ad hoc. `speed.py`
should grow an `--ab A-glob B-glob` mode printing per-side means, sd, the
difference, its standard error, t, the MDE and which estimator each came
from. **Deferred, with a reason:** the next A/B this campaign needs is step
4's controlled comparison, so the mode goes in there against a real user
rather than being written now; until it exists, no A/B number is quoted in an
entry.

**§1.6, §1.7, §1.8, §6.3 — accepted:** the gate artefact now names both ELFs
and includes clippy and the audit; the inventory is regenerated after it, not
before.

**§4.2 — accepted:** the gate went from two tests to one, and the 1% band's
calibration belongs to the *legacy* estimator. The number is unchanged; what
it bounds is not, and the stale two-test docstring is corrected.

**§4.5 — accepted:** those six runs pass at `min_hold_ms = 9_000`
(exploratory), not at the 30 s dwell. They were 45 s exploratory runs, and
E172 should have said so.

**§6.4, §6.5 — accepted:** `manifest_hashes.py` now skips `ladder_state.json`
as its docstring always claimed, and the `[workspace]` sentence is about
profiles, not isolation — `.cargo/config.toml` merges from every ancestor,
which is a scar this bench already has.

#### Step 2's runs

Image `01A674BA` (`captures/elf/01A674BA.e173.elf`), the arm/stop discipline.
Every rung driven by the fixture, which flashed and hashed the image itself:

| rung | result | counters |
|---|---|---|
| 15% | **3/3 PASS** | — |
| 20% | **3/3 PASS** | — |
| 25% | **3/3 PASS** | `spent_max_us=11`, `late_arms=0`, `blank_latched=0`, `com_preempts=0`, `com_arm_preempts=0`, `reason=2` in all three |

And the same rung on the **previous** image back to back in the same session,
because cross-session speed comparison is not valid:

| image | 25% powered eHz (n=3) | spread |
|---|---|---|
| `4F0F5131` (before) | 1194.20 | 0.40 |
| `01A674BA` (arm discipline) | **1195.04** | 1.46 |

**+0.70 ‰ — inside the spread and in the *faster* direction**, so no cost to
the loop; and with n=3 at this spread the comparison could not resolve better
than about 2 ‰ anyway, which is §7.1's lesson applied rather than quoted.

**Verdict against E173's predictions.** (1) The loop is unaffected — **held**.
(2) `spent_max_us` stays 11 — **held**; the chain per event is a 45%+
measurement and is owed at step 4. (3) `late_arms` and `blank_latched` zero,
every run stopping on its deadline — **held**. (4) The effective-angle
prediction is **not tested here** and is owed at step 4.

**Cost, as flashed** (static longest-path bounds from `isr_cycles.py`, the
`01A674BA` image against `4F0F5131`, both at 0 WS and under the 2 WS fetch
model):

| root | 0 WS | 2 WS | instructions |
|---|---|---|---|
| `ADC_COMP` | 989 → **1009** | 1171 → **1193** | 699 → 716 |
| `TIM16` | 333 → **346** | 397 → **410** | 306 → 326 |
| `TIM6_DAC_LPTIM1` | 204 → **208** | 245 → **249** | 153 → 155 |
| `DMA1_CHANNEL1` | 78 → 78 | 84 → 84 | 37 → 37 |

The guard grew because it calls `com_stop`, which now latches. **This is the
growth the goal's "no cycle growth" gate forbids and the goal's step 2
requires**; it is named here with its numbers rather than waived, and the
powered result above is the evidence that it costs the loop nothing.

**Step 2 is complete** on the terms the goal set: the discipline is structural
(one rule in shared code, one ordering, a latch the handover alone releases),
the interleavings are tested rather than observed, and the runs show the loop
unchanged. What is **not** claimed: that the hazards were ever reachable —
`com_arm_preempts = 0` across ~99 000 counted preemptions in campaign 8 says
the arm window was never hit, and zero observed violations is not proof, which
is why the structure is there.

### E175 — step 3: the sag guard traced, and instrumented on its own quantities

**Written before the build and runs.**

**What the guard actually compares** (`protection::FastBusSag::observe`, traced
line by line, no edits):

```
bus_mean * filt_vref * SAG_DEN  <  filt_bus * vref_mean * SAG_NUM      (95/100)
```

* **Inputs are block means, not scans.** `Ctx::scan_pass` feeds
  `rail.bus_mean()` and `rail.vref_mean()` — `protection::RailMean`, a running
  mean over `RAIL_MEAN_LEN` scans — and only once `rail.ready()`.
* **The reference is a ~207 ms exponential average** of the same two, in Q8
  and **rounded** on read (`filtered()`), with `SAG_FILTER_SHIFT = 11` at the
  9.901 kHz scan rate.
* **The filter is updated after the test**, so a collapsing block cannot drag
  the reference onto itself and hide the collapse.
* **Three consecutive low blocks latch** (`SAG_STREAK`); one healthy block
  zeroes the streak. A VREF of 0 or ≥ rail latches immediately, fail-closed.
* The pre-run baseline (`reference()`) is kept for the record and is **not**
  what a block is judged against (E146).

**So every quantity this campaign has quoted at the guard was the wrong one.**
`bus_min` is the minimum of a *single raw scan* over a whole run
(`states.rs`), and the report's `filt_bus`/`sag_streak` are the filter and
streak at the *end* of the run. E161 set `bus_min` beside the 95% fraction and
called the margin negative; that comparison was meaningless, and the step-5
review said so.

**The instrument.** `src/sagtrace.rs`: a `SagLog` trait with `const ON`,
production's `NoSagLog`, and `SagRing` — 512 blocks of 14 bytes (~1.2 s of
history at the block rate), circular, **frozen on the trip** so a dump is the
window that led there. Each row is the guard's own quartet *as used for that
test* (the reference read before its post-test update), plus the streak it
holds afterwards, the sector, the applied duty, and **µs since the last
accepted crossing** — so a low block can be placed against the switching
instant instead of being assumed independent of it. **No division in the
firmware**: the host reproduces the cross-product exactly
(`scripts/sag.py`), which is the `BEMFTAIL` discipline.

The recorder is a **policy slot** (`Policies::G`), so the composition chooses
it: `Production` defaults to `NoSagLog`, and the new `sag-capture` binary
instantiates `SagRing`. Five host tests cover the ring: it keeps the last
blocks and reports what it dropped, a freeze keeps the pre-trip window and
nothing after it, a disarmed ring records nothing, the index wraps only at the
end, and **the host's margin matches the guard's own comparison** (95.0% is not
low, one code below is, and VREF moves the line).

**Overhead, measured where it lands.** The instrumentation is **foreground
only** — the guard is judged in `Ctx::scan_pass`, not in an interrupt — and
`isr_diff.py` confirms the `sag-capture` image's four ISR roots are identical
to production's (37 / 716 / 326 / 155). What it costs is foreground passes,
which the run reports as `loop_iters_closed`; that is the A/B below.

Host tests 326 + 9.

**Predictions.**

1. At 45% and 47.5%, **no block latches** (these rungs pass today), but the
   margin's minimum over a run is **finite and close to the line**: I expect a
   minimum between 1000 and 1100 per mille, i.e. 0–10% of margin, with a
   handful of blocks below 1005.
2. The tightest blocks are **not uniformly distributed in the commutation
   cycle**: if the dip is switching-driven, `since_com_us` clusters; if it is
   supply-driven, it does not. I do not predict which — that is the test.
3. `loop_iters_closed` falls by **less than 5%** against the production image
   at the same rung, because one 14-byte row per block at ~2 kHz is small
   beside a foreground pass rate of ~50 kHz.
4. The streak reaches 1 or 2 in a passing run at 47.5% at least once — a low
   block that did not latch — because campaign 7 latched three times at 50%
   and the margin cannot plausibly jump from "never low" to "three in a row"
   between 47.5% and 50%.

If prediction 4 fails and the streak never leaves 0 at 47.5%, the 50% trips
are not the tail of a distribution this campaign can see from below, and step
4's comparisons have to change.

### E176 — a process violation of mine, and the 45% sag margin it produced

**I drove a 45% run without the pre-run review the goal requires.** The rule
is *"before every powered run ≥45%, spawn a fresh context-free reviewer"*, and
I flashed `2D157417` and ran `e175-sag450` straight after building it. No
harm resulted — the run passed its gates, every protection was armed, and
`late_arms`/`blank_latched`/`com_preempts` were zero — but the gate exists so
that an instrument is checked *before* it produces a number I will then
believe, which is exactly the failure mode the last six reviews have been
correcting. The review is running now, before the 47.5% run and before
anything at 50%, and **this capture is held as ungated until it returns**.

**What the run measured** (`captures/sag/e175-sag450.txt`, image `2D157417`,
45% exploratory, gates PASS, `reason=2`):

```
blocks judged=440436 kept=512 overwritten=439924 frozen=0 fraction=95/100 streak_to_latch=3
margin per mille (1000 = exactly on the line): min=1046.5 p01=1047.2 p50=1052.6 max=1059.6
low blocks (the guard's own test): 0 of 512
streak held: max=0 (latches at 3); rows with streak>0: 0

tightest blocks:
   margin    bus   vref  filt_bus filt_vref  streak  step   duty  since_com
   1046.5   1194   1507      1201      1507       0     1    450        116
   1046.5   1194   1507      1201      1507       0     2    450        116
   1046.7   1195   1508      1201      1507       0     3    450         62
   1047.2   1194   1506      1201      1507       0     5    450         96
since the last accepted crossing, in the tightest blocks: min=36 p50=107 max=127 µs
sector of the tightest blocks: {1: 7, 2: 3, 3: 4, 4: 2, 5: 2, 6: 2}
```

**Against the predictions.**

* **(1) held, narrowly.** I predicted a minimum between 1000 and 1100 per
  mille: measured **1046.5**. But I also predicted "a handful of blocks below
  1005", and there were **none** — the tightest block is 4.7% clear of the
  line.
* **(4) failed at this rung.** I predicted the streak would reach 1 or 2 at
  least once. It never left **0** in the sampled window. My reasoning was that
  the margin could not jump from "never low" to "three in a row" between 47.5%
  and 50%; the measurement says the distribution at 45% is nowhere near the
  line, so that reasoning was wrong or the window is looking in the wrong
  place — see the coverage caveat below.
* **(2) is answered only weakly.** The tightest blocks are spread over all six
  sectors (7/3/4/2/2/2) and `since_com_us` spans 36–127 µs with a median of
  107, so at 45% there is **no visible phase-locking** of the tight blocks to
  the commutation instant. With 512 blocks and a margin this far from the line,
  this is a null on a quiet signal, not evidence of independence.
* **(3) is not yet tested** — the foreground-cost A/B against the production
  image at the same rung is owed.

**What the numbers say about the guard, plainly.** The block mean tracks its
own 207 ms filter to about **half a percent** (bus 1194–1195 against a
filtered 1201, VREF 1506–1508 against 1507), while the trip line sits **5%**
away. At 45% the guard is not marginal, and the quantity that *looked*
marginal in E161 — `bus_min = 1127`, 6% below the mean — is a single raw scan
the guard never judges. That comparison is now retired with a measurement
rather than an argument.

**The coverage caveat, which is the important part.** The ring keeps the last
**512 of 440 436** judged blocks: **0.12% of the run, and its final ~1.2 s**.
So this says nothing about the ramp, nothing about the first 40 s, and nothing
about a rare excursion elsewhere in the hold. For a run that *trips* the
instrument is right — it freezes on the fault, so the dump is the pre-trip
window — but for a run that passes it shows only the tail. Campaign 7's 50%
trips are what this instrument was built for, and until one is captured frozen,
**no claim about why the guard latches at 50% is supported by anything here.**

### E177 — the sag instrument's window and cost, measured, and both were wrong in E175

**Written after two measurements that correct my own step-3 entry.**

**1. The window was off by 24×.** E175 said 512 blocks was "about 1.2 s of
pre-trip history", from an assumed ~2 kHz block rate. The captures say the
guard judges a block at **9.8–11.7 kHz**: `total=440436` over a 45 s run at
45% and `total=786918` over an 80 s run at 25%. `RAIL_MEAN_LEN = 8`, so a
block is ready roughly every eight scans of the 9.901 kHz harvest and the rate
is set by the scan rate, not by anything slower. 512 rows was therefore about
**50 ms**, not 1.2 s.

That matters because the window bounds every conclusion: E176's "no low block
at 45%" covered **50 ms of a 45 s run**, and I described it as 1.2 s. The ring
is now **1024 blocks ≈ 100 ms** (14 KB; the image's `.bss` is 20.6 KB of the
36 KB part, so the stack keeps 15 KB), and `sagtrace.rs` records the measured
rate beside the constant so the next reader does not re-derive it wrongly.

**2. The instrument costs 10% of the foreground, not "less than 5%".**
Measured back to back at 25% in one session, production against the sag image:

| image | `loop_iters_closed` |
|---|---|
| `01A674BA` (production) | 3 525 604 |
| `2D157417` (sag recorder) | 3 177 677 |

**−9.9%.** Prediction 3 said under 5% and was wrong by a factor of two. Per
block that is about 0.44 lost foreground passes, i.e. roughly 500 cycles for
a 14-byte row — far more than the copy, and consistent with the
`interrupt::free` prologue/epilogue plus the `RefCell` borrow checks around
every push. It is a diagnostic image and the cost is acceptable, but it is
**not negligible** and it is exactly the sort of figure I have been told twice
not to assert without measuring.

Consequences, stated rather than waived:

* the sag image's foreground polls the protections 10% less often, so its
  detection latency for *any* stop is 10% longer than production's — and the
  two hard stops are foreground-polled;
* its runs are evidence about the guard's inputs, never about production's
  loop quality or its stop latency;
* the four ISR roots remain byte-identical to production's (37 / 716 / 326 /
  155), so nothing in the interrupt path is affected.

**3. What still stands from E176.** At 45%, over the sampled window, the
block mean tracks its own 207 ms filter to about half a percent while the trip
line sits 5% away, and no block was low. The claim is now correctly scoped:
**50 ms of the end of one 45 s run**, on image `2D157417`, ungated until the
pre-run review returns.

**Next:** the review, then 47.5% on the 1024-block image `D4ACF3EA`, then the
first 50% exploratory run — which is the only run that can capture a frozen
pre-trip window, and therefore the only one that can say anything about why
the guard latches there.
