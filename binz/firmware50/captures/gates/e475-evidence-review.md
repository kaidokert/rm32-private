Raw evidence, recomputed (each tick = 0.125 µs):

| Case | Body µs | Resume µs | Reason | Before→after | Accepted/early/unstable |
|---|---:|---:|---:|---|---|
| 0 | 7.250 | 0 | 0 | 22→22 | 1/0/0 |
| 1 | 8.250 | 14.875 | 3 | 22→17 | 1/0/0 |
| 2 | 3.125 | 8.500 | 3 | 18→17 | 0/0/1 |
| 3 | 29.625 | 32.750 | 15 | 17→17 | 1/0/0 |
| 4 | 7.750 | 0 | 0 | 22→22 | 1/0/0 |

Snapshot bits 0–4: stopped, active, timer-running, TIM16-pending, off. Thus 22 means active/running/off; 18 active/off; 17 stopped/off. All sampled pending bits are clear.

Five distinct cases match the predicates; totals are **4 accepted, 0 early, 1 unstable**, zero COM calls. All report masked/off/gates checks=1 and priorities guard/COM/COMP=0/64/128. Both idle readbacks show enable/MOE/CCRs=0, gates_low/nfault=1. Download/verify/reset report success.

**Yes: this completes the five-case disabled integration check within the supplied evidence.** The digest’s source/build linkage is not independently established.

It cannot establish energized motor timing, physical zero-cross accuracy, commutation phase/deadline correctness, switching latency, or worst-case interrupt blocking. COM uses a stopping stub; comparator stimulus and guard requests are synthetic. Body measurements include probe overhead; resume measures request-to-post-unmask observation, not isolated guard latency. Zero resume means no request. These observations do not qualify the motor candidate or bound production timing.
