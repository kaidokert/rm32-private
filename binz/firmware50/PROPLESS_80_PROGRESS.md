# Propless 80% campaign — progress, not qualification

Updated through E400 results / E401 diagnostic build, 2026-09-26 PDT.
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

Every listed attempt has verified final bridge-off readback. None is a
three-run, 30-second target-dwell qualification. Mechanical RPM = eHz × 10.
The count-versus-coast mismatch at E400 compares a three-second powered average
with a near-stop coast estimate: it does not establish sustained slip.

## What changed / what did not

- Typed `FasterAbove<350,1000>` carrier: 48 kHz entry, 64 kHz from 35% duty,
  retained through foldback; ordinary restart begins afresh. Native update
  transaction stages ARR/CCRs under UDIS without UG/CNT/MOE/ENABLE changes.
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

## Current next test

E401 `3E1B036E.e401-staged-sag.elf` is diagnostic only: existing foreground
SagRing, no COMP/COM recorder, identical motor-root instruction sequences to
the lean 58383038 image. Its masked recorder still perturbs scheduling.
One 50% control, then conditional 70% only if the control is clean, will retain
the actual guard operands and shunt codes around shutdown. This can distinguish
sampled reference evolution from a terminal notch, not physical crossing order.
No 75% test is admitted by the E400 result.

Fresh evidence/adversarial reviews of E399 and E400 are retained under
`captures/gates/` and appended verbatim with dispositions to the notebook.
