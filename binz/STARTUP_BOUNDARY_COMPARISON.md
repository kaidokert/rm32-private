# Local timing at7.1% refusal — E432

No firmware/profile change or hardware action. Actual14FE/offE431 unchanged.
New `scripts/drv_local_cycles.py` validates capture CRC/chronology/finaloff via
the existing decoder/reporter and reports prefix and tail separately. It never
bridges omitted events, includes a rejected event as accepted, or promotes
overlapping cycle samples to independent observations. Two host tests pass.

| Retained capture | Window | Median cycle us | Reciprocal median eHz |
| --- | --- | ---: | ---: |
|range340_start61_hold71_10s_01 failed|first0.47–20.31ms|3867|258.60|
|same|last154.31–169.89ms|3016|331.56|
|range340_start61_hold70_10s_01 pass|last~16ms of10s|3064.5|326.32|
|range340_start61_reentry70_30s_03 pass|last~16ms ofrecovery|3073|325.41|

Each window has32 retained events and26 OVERLAPPING full-cycle differences.
These are accepted-recorder timestamps, not independently measured rotor
periods. In the failed71capture,257 intervening events were omitted. The
table does not reconstruct the acceleration curve or establish an equilibrium.

The previous315.355eHz whole-run mean is diluted by startup. Before refusal,
the local median is already331.56eHz, with accepted tail cycles2981..3078us.
The rejected GUARD cycle2930us is86us (~2.85%) shorter than the tail median;
that comparison is contextual, not subtraction of identical clock brackets.
Reference snapshot measured2934us for the rejected cycle, so the distinction
is not explained solely by the final recorder callback. No physical speed,
preemption attribution or noise cause is proved.

This supports treating the next step as an envelope experiment near332eHz,
not fixing a mythical steady315eHz failure. It also does NOT prove that7.1%
would settle there. The present2942us floor legitimately refused the attempt.

Next candidate for review is345profile, only after carrying the original
electrical/tracking/deadline and late-arm rules forward. Reference integer math
at966 seedticks yields242 waitticks=121us; at85/87us age it leaves36/34us,
4/2us above the32us floor. At90us it refuses. Those are narrow nominal margins,
not WCET. Existing lean_seed_age tests cover the arithmetic, but345 is not an
enabled profile yet. A safely refused recovery would remain a failed attempt.
No higher duty, unchanged71 retry, new profile, or performance claim inE432.
