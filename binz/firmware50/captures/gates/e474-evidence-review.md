Snapshot bits are stopped=1, active=2, timer-running=4, TIM16-pending=8, off=16. Thus the assertions require:

| Case | Counters (accepted, early, unstable) | Reason | Before → after |
|---|---|---|---|
| 0 | (1,0,0) | 0 | 22 → 22 |
| 1 | (1,0,0) | 3 | 22 → 17 |
| 2 | (0,0,1) | 3 | 18 → 17 |
| 3 | (1,0,0) | 15 | 17 → 17 |
| 4 | (1,0,0) | 0 | 22 → 22 |

22 means active/running/off; 18 active/off; 17 stopped/off. These decode correctly, but missing transaction/arming bodies prevent independently deriving those outcomes, including case 4’s pending-bit clearance.

INMSEL=3 matches HAL VRefint; selection precedes a 640-cycle delay. Analog settling sufficiency is unproven.

`prepare` explicitly sets guard priority, clears pending, and unmasks it. Priority **0** is unverified without the constant definition; ADC_COMP/TIM16 priorities require omitted board initialization. Driver fault precedes TickGap, making nFAULT-high necessary.

The old e473 capture fails and uses different output fields. Its detector readback decodes to **(0,0,1)**. The e474 audit does not establish runtime results or image identity. Exact flash verification and a fresh five-case capture remain required.
