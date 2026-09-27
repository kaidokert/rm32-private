Ordinal joining gives **127 pairs**, with unmatched COM420212 at the left boundary and acceptance420340 at the right. All supplied COM brackets span two 8-MHz ticks: **0.25 µs**, not 2 µs.

The terminal sequence is:

| Acceptance | Interval | Updated average | Wait |
|---|---:|---:|---:|
| 420336 | 49 | 48 | 12 |
| 420337 | 35 | 45 | 12 |
| 420338 | 28 | 40 | 11 |
| 420339 | 49 | 40 | 10 |
| 420340 | 63 | 48 | 10 |

All times are microseconds. COM420339 completes at 27274; acceptance420340 enters **48 µs later**, at 27322. Its interval and estimate are recovering, while its wait retains the previous estimate’s 10 µs. That supports a **lagged scheduling vulnerability**, not proof that a false crossing caused the contraction.

`spent_at_late=10` is consistent with exhausting that wait. Attribution to acceptance420340 remains inferred: the late-arm record contains no ordinal. Its missing COM alone proves neither a lost commutation nor correct shutdown.

**Highest-value next test:** interleave otherwise identical diagnostic builds selecting existing `PreviousEstimate` versus `FreshEstimate`, retaining every guard/threshold, propless, ≤80 V and 3 A PSU limit. At this recorded terminal state, fresh scheduling would request 12 µs; actual trajectories may differ. Compare contraction and stop outcomes across repetitions. This tests wait-policy contribution, without transferring causality to lean E421 or establishing physical crossing validity.
