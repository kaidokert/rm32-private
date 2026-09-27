# Propless 80% campaign — progress, not qualification

Updated through E411 results/dual review and E413 offline candidate, 2026-09-26 PDT.
Authoritative details, predictions, reviews and dispositions: LAB_NOTEBOOK.md.

## Scope

Firmware50 only. FlyFishRC Flash 1404 4500KV, 9N12P (six pole pairs), reverse
direction, prop removed. Previous loaded qualification does not transfer.
PSU limit remains operator-set 3 A. No motor-temperature sensor or verified
manufacturer maximum mechanical RPM is available. KV is not that limit.

Goal remains one reproducible image qualified at 60/70/80%, with restart and
protection coverage. **It is not complete.** Current evidence is exploration.
Existing guards remain active; only the authorized command ceiling rose to 80%.
Current exploratory exposure is at most 45 s powered, with at least 120 s OFF
between attempts. This is a precaution, not measured thermal recovery.

## Recent evidence

| Entry / image SHA prefix | Requested | Actual target dwell | Outcome |
|---|---:|---:|---|
| E393 / F971AACE, full diagnostic | 50% | 19.778 s | Deadline; coast 2491 eHz |
| E394 / F971AACE | 60% | 0 | Fast sag at applied 59% |
| E396 / 44DC8FF0, lean staged carrier | 50% | 19.778 s | Deadline; coast 2497 eHz |
| E397 / 44DC8FF0 | 60% | 14.778 s | Deadline; coast 2926 eHz |
| E399 / 58383038, same control, cap raised | 65% | 12.278 s | Deadline; time-anchored coast 3139 eHz |
| E400 / 58383038 | 70% | 9.025 s | Fast sag; tail accepted-rate 3292 eHz, coast 3219 eHz |
| E402 / 3E1B036E, sag-only diagnostic | 50% | 19.778 s | Deadline; coast 2495 eHz |
| E403 / 3E1B036E | 70% | 0 | Fast sag at applied 57%; raw guard operands retained |
| E406 / BF725A29, lean entry-relative arm, 48 kHz | 50% | 19.778 s | Deadline; matched tail/coast about 2529/2528 eHz |
| E407 / BF725A29 | 60% | 0 | Fast sag at applied 55%; all outputs verified off |
| E410 / EFFD1B1C, prior-estimate wait, 48 kHz | 50% | 19.778 s | Deadline; tail/coast 2522/2527 eHz |
| E411 / EFFD1B1C | 60% | 0 | Fast sag at applied 56%; all outputs verified off |

Every listed attempt has verified final bridge-off readback. None is a
three-run, 30-second target-dwell qualification. Mechanical RPM = eHz × 10.
The count-versus-coast mismatch at E400 compares a three-second powered average
with a near-stop coast estimate: it does not establish sustained slip.

## What changed / what did not

- Typed `FasterAbove<350,1000>` carrier: 48 kHz entry, 64 kHz from 35% duty,
  retained through foldback; ordinary restart begins afresh. Native update
  transaction stages ARR/CCRs under UDIS without UG/CNT/MOE/ENABLE changes.
  **Retired prospectively at E404:** the board's CSD88584Q5DC datasheet lists
  5–50 kHz recommended switching operation. The proposed 80 kHz experiment was
  cancelled before implementation. This is a component constraint, not proof
  that 64 kHz damaged anything or caused the sag. Preserve historical captures;
  do not qualify or reuse these 64 kHz images for further powered exploration.
- Lean `staged-pwm` removes full event recorders, not guards. E397 suggests
  useful observer sensitivity, but one pass versus one failure is not causality.
  Earlier lean 48 kHz E375 also failed at 60%: diagnostics were not necessary
  for the original failure.
- Near-arm counts rose from 2 at E397 to 133 at E399 and 926 at E400. These are
  whole-run observations of computed remaining time ≤2 µs; late-arm counts
  remained zero. Stage and clustering are unknown. They are not physical-edge
  timing margins or proof of the sag cause.
- Current readings are signed proxies with substantial zero drift. Their
  averages do not bound phase peaks; a sag-deciding scan bypasses accumulation.
- The sharp guard compares an eight-scan sliding mean with its filtered
  reference, requiring three low judgements. Three judgements are not three
  independent raw samples. No threshold, streak or latch is being relaxed.

## Current timing candidate

E417-E419: diode-mode candidate `DC00CCB4.e417-diode48.elf` retains the
prior-estimate/latch48 baseline but disables the source complementary output
in closed-loop sectors. Its actual-PWM driver-disabled pad check passed all
six sectors. One 15% screen completed 20.278s at target, tail2206.902eHz,
coast2207, matched ratio1000, no late/thin/sag/storm/tracking stop, finaloff.
This is exploration only. Duty-to-speed is materially different; old
synchronous current/speed curves do not transfer. Proxy174mA with -295mA zero
drift is not efficiency or thermal evidence. No25% or higher diode run yet.
The next-step review is unresolved: complete packet needed for host admission
and prior transition analysis; physical temperature/speed-rating gaps remain.
Current board is this image, last verified OFF by E419, COM41 closed.
Release audits: COM388 instructions (+6 vs latch baseline); COMP780, DMA37,
guard155 unchanged instruction streams, four-root soft-arithmetic clean.
No new qualified propless ceiling.

E409 changed the wait's estimator age through a typed policy: use the prior
estimate rather than the newly blended one, matching that particular AM32
dependency (not claiming full reference parity). E410 passed its 50% screen;
E411 sag-stopped at about56% on the 60% climb. This single screen did not
remove the failure. It cannot isolate sensitivity against unmatched runs.

E413/E414/E415: `8F977353.e413-latch48.elf` stages TIM1 roles and
transfers them together using COMG, without UG or counter reset. A finite
stop-checked critical section prevents resumed compare writes undoing safing;
UDIS and preload prevent partial new compare generations at native updates.
Host tests pass; COMP/DMA/guard code is unchanged, COM gains50 instructions
and one mask. This is a reviewed candidate cost, not WCET qualification.
Its disabled ENABLE-low pad witness passed. The protected 50% screen then
fast-sag-stopped after 3.662 seconds at target; final outputs-off passed.
Both result reviews reject escalation. The candidate is retired from powered
exploration, not proven causal. No physical glitch or sag cause has been
established from the register model. Next work is offline feasibility of a
separate source-low-off policy, not another test of this failed candidate.

### Earlier arm change retained in both candidates

E405 `BF725A29.e405-deadline48.elf` uses the lean controller at fixed 48 kHz.
The crossing arm prepares TIM16 before its final elapsed-time subtraction,
inside the same atomic stop/arm region. Requested-deadline bookkeeping no
longer restarts the clock after that subtraction. Exhausted waits invoke the
existing LateArm shutdown immediately; thresholds remain unchanged.

Normal poststamp-to-CEN code falls from 50 to 30 instructions, but total COMP
code grows from 742 to 778; neither count is measured WCET. A named candidate
ratchet records the reviewed change without overwriting production baselines.
This is software-entry-relative scheduling, not a hardware crossing timestamp
or absolute compare. Both `spent` and COM lateness have broader/different
coverage, so old numeric maxima cannot be directly compared.

The native timer passed its outputs-off boot smoke check. E406 passed the
50% screening run. E407 stopped during the 60% climb at actual55%, no target
hold. Both fresh reviews support that narrow outcome, not a cause or matched
improvement/worsening verdict. No 65–80% admission on this image.
Fresh reviews and raw gate artifacts are under `captures/gates/`; findings and
dispositions are appended verbatim in the notebook.

E403's sag-only recorder correctly captured means 1134/1123/1112 against
reference 1212 at equal VREF, forming the terminal streak. Phase channels
clipped before the sampled threshold crossing; sequential ADC timing and
masked recorder work prevent physical-cause attribution. The diagnostic's
earlier failure does not establish a 57% lean limit.
