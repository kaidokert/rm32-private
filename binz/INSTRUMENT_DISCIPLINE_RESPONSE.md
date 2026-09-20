# Instrument discipline: retained decisions and evidence corrections

2026-09-14, E534. Operator requested reading and remembering
GRAYBEARD_INSTRUMENT_DISCIPLINE.md; all183 lines read, plus the new E478
falsification banner in GRAYBEARD_ENVELOPE_AND_SENSE.md. Advice is not an
operator authorization to weaken guards or a substitute for measurement.

## Adopt as working direction

- Prefer a small, question-specific observable over repeated layout work on a
  detailed recorder that cannot meet its budget. State the question, measured
  cost, and retirement condition before implementation.
- Separate diagnostic and qualification binaries, hashes and evidence. Audit
  actual linked features; a bench-* name alone does not mean instrumentation.
  Keep safety, required feedback and outcome evidence in qualification builds.
- Prefer bounded aggregation and post-stop output. Foreground analysis and
  archived minz/krabilorean work are candidates, not yet M0-qualified solutions.
  Retain numeric outputs against DCE and inspect emitted arithmetic.
- Use same-rig AM32 comparison to prioritize software investigation. E530
  provides a known-settings short response to741eHz, not reliable startup,
  sustained lock or proof of hardware behavior under every condition.
- Do not revive E478's falsified mid-persistence-preemption mechanism. Avoid
  hardware changes unsupported by evidence. No cap/resistor work proposed.

## Corrections required before choosing experiments

1. **bi_direction is not a one-effect switch.** Current AM32/Src/main.c
   lines819-823 halves polling_mode_changeover, but lines2346-2356 still change
   startup min_bemf_counts (TARGET+1 vs TARGET*2 below5 events), and line2457
   changes whether desync clears running. UART decoding bypass does not bypass
   these branches. Hold those behaviors constant if isolating changeover.
   E527 zc2/~21eHz does not demonstrate reaching the167eHz IRQ handoff.
   Later bidirection1 failures E529-E533 also prohibit the claim that1000ticks
   by itself establishes reliable startup. The lead remains testable, not proven.
2. **RAM arithmetic:**340*6*30=61,200 commutations. One u16 each needs122,400B;
   one u8 each still needs61,200B, not under16kB. Quantization alone does not
   reduce sample count. About16kB requires decimation, a shorter tail or actual
   compression with overflow/escape handling. Eight seconds of u16 is32,640B,
   before firmware RAM/stack; current binz does not have30kB free. Budget actual
   linked RAM, not nominal device size.
3. **Observer effects do not automatically cancel.** Matching source counters
   may compile/place/preempt differently in AM32 and Rust. Measure each emitted
   path and baseline effect separately. Equal duty also need not give equal
   speed or equal effective PWM (E530 input25 produced26.519%compare).
4. **Cycle estimates are hypotheses.** Loads/stores, binning, bounds, branches,
   ring indexing and overflow handling cost more than a lone store. Non-power-
   of-two carrier binning is not generally one shift. A64MHz cycle is15.625ns;
   ~150ns is about9.6cycles, not a guaranteed10instructions. Cost fraction is a
   useful principle, not a replacement for measured perturbation bounds.
5. **Foreground sampling can miss the rare event.** Reading the newest interval
   at1kHz undersamples2040commutations/s at340eHz; preserve missed/overwritten
   accounting. Kurtosis from a sampled stream is not guaranteed to see every
   excursion. Residual preprocessing and block precision need actual replay.
6. **Record-only CycleTiming is not adopted.** User goal retains safeguards;
   the memo is advisory. Distinguish qualified-speed enforcement from timing/
   tracking protection and specify evidence before any experimental change.

No new instrumentation or controller change follows automatically from this
memo. Next experiment should test one concrete software difference, not build
all proposed counters/analytics together.

## Bench handoff at this pause

Before memo arrived, restored frozen binz BC876BF0919EDC8B3FF00019A8D44DB8D3DE7142EB72C84151103EB19689B630.
AM32 stopped first; probe download/OpenOCD reset. Disabled guard3/18, full
five preflights at3200ticks, ADC route3checks, direct semantic/wire/raw-cycle
gate all pass. CPU maxima2/6/9us; direct maxaccept236cycles<=256, no subtraction.
Raw captures restorebinz_534_*.txt. Every final output-off check passed.
No motor command this entry. Board is BINZ, not AM32; COM41 commands off/p/i
apply again. Workspace root build is not this frozen image. AM3202592720
source/obj remains available separately. These are disabled checks, not a
fresh startup/lock result.

## E535 — powered baseline restored, 2026-09-14

One actual run on frozenBC876 after E534's exact-build disabled preflights:
`python scripts/drv_driven_handoff.py --out captures/restorebinz_535_start61_hold69_10s_01.txt --ms 10000 --drive-duty 61 --bemf-duty 69 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --qualification-direct`

Motor completed normal segment deadline, then host rejected the missing
--dma-peer expectation (actual captured COMP/COM/DMA64, guard0). No rerun.
Revalidation: same command with --infile instead of --out, plus --dma-peer:
PASS full acquisition/handoff/window, QD85 epoch/count, timeline and finaloff.
The initial host invocation remains exit1; this is a flag correction against
actual build provenance, not erasing a motor failure or changing a verifier.

Capture SHA25629753B70A577B6B7C17D6BD17CC8D2618BCCF8D472EFFC48D6FFAD0FD3AD420C.
Requested10,000,000us; guard stop10,000,007us, observation10,000,034us.
19998COM/19997accepted, order_bad0; estimated333.29896eHz, aggregate cycle
sigma42.458us includes acceleration. Retained tail cycles2923..3048us,
median3000.5us (overlapping observations, not independent rotor measurements).
Current peak310 raw counts, minimum bus11140mV; startup rawpeak476,bus11414.
ADC49751 samples, period3200, histogram CRC/sum/provenance validated. Not
calibrated amps or an unbiased-current certificate. Stack untouched2972.
CPU IRQ partition5,473,555/10,000,008us=54.7355%; foreground not all idle,
observer cost not subtracted, not total CPU utilization or WCET.
Guard max20us,commitmax21us, no veto. No injected loss/recovery this run.

Conclusion: the existing binz driven-start/handoff currently succeeds, while
the AM32 adapter's recent startup cohort did not. Do not call either a hardware
failure or a completed parity result. Stop prolonging AM32 startup work merely
to reach30%; its earlier known-config741eHz response serves the comparison.
Return to binz control-path differences with one bounded experiment and the
same guards. Actual frozenBC876 remains disabled; no source/build changes.
