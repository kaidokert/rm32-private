Raw recomputation: each tick is 0.125 µs.

| Run | Case results | Durations, case order |
|---|---|---|
| E458 | 0 passes; 1 fails | 9.000, 10.125 µs |
| E459 | 0–5 pass reported request/phase/armed expectations | 9.875, 11.500, 10.500, 4.125, 2.500, 2.500 µs |

E458 case1 requested zero versus one expected; its cause remains unknown.

E459 bits11 means present, line live, post level, **not admitted**; bits27 adds admission. Pending is clear in both. Case0 is early: age8 versus first due501. Case1 age67 exceeds due41; the model predicts another check after54 µs. Case2 age407 permits one late request, then exhausts slots—not a catch-up burst. Those delays were not measured. Cases3–5 bits0 mean no captured input; their age/average values are stale. `pass` does not assert observations or retirements.

This supports a driver-disabled peripheral experiment, not motor qualification. ADC_COMP is masked, schedules reset between cases, and callback timing includes Capture but excludes full COM service. The static audit establishes neither WCET nor deadlines.

**I would defer the proposed motor screen.** Missing admission evidence: original AC4ADA59 artifact correspondence; actual `admit`/budget rules; pending, wrong-level and dead-line rejection; autonomous five-slot progression; real COMP acceptance/publication/arm handoff and stop cancellation. Next test should exercise those paths disabled, recording timer delays and terminal state. A subsequent 15% screen would remain exploratory, without establishing Tracking improvement or qualifying60/80.
