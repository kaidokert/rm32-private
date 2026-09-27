The ordinal join yields 127 pairs (420213–420339). COM420212 is outside the acceptance tail; acceptance420340 has no matching COM. Every paired sector advances correctly. TIM2 brackets are 2 ticks = 0.25 µs; these do not establish physical crossing time.

Recomputed terminal sequence:

| Acceptance | Interval | Published average | Wait |
|---|---:|---:|---:|
| 420337 | 35 | 45 | 12 |
| 420338 | 28 | 40 | 11 |
| 420339 | 49 | 40 | 10 |
| 420340 | 63 | 48 | 10 |

All values are µs. The terminal acceptance occurs **48 µs after the preceding postapply timestamp**, following interval recovery—not at the shortest interval. Its wait remains consistent with the previous estimate, despite the published estimate rebounding to 48.

Paired service lateness is 1–4 µs; entry-to-postapply excess over requested wait reaches 8 µs. This excess is distinct from pre-arm spend. Reported terminal spend=10 exhausts wait=10; the missing terminal COM is consistent with stopping, not proof of an earlier lost commutation.

Neither valid physical crossings nor the initiating disturbance is established. Aggregate coast agreement cannot resolve individual events; lean-image causality cannot transfer.

**Highest-value test:** matched, repeated PreviousEstimate/FreshEstimate runs using existing code, identical remaining configuration and logging. Fresh timing would request 12 µs at the terminal recorded state, but replay cannot predict its altered trajectory. Preserve all guards/thresholds, propless operation, and PSU ≤80 V/3 A.
