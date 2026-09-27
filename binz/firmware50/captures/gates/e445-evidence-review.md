Raw source/assembly supports first-decision correctness: strict `age > limit`, wrapping subtraction, origin tagging, and captured limit survive later tightening. `last` remains frozen because subsequent events return on the latched fault. No concrete defect found there.

Both ISR paths bypass the three evidence stores when healthy; those stores precede shutdown when stale. This supports fault-only evidence writes, not zero execution/layout cost or bounded shutdown latency. Baseline instruction-count deltas and clippy results are author assertions; supplied output shows 384 passing tests and the ISR audit passing.

Reporting prints outside the mask; `post_*` correctly distinguishes later snapshots from decision evidence. `u32::MAX` avoids reporting an unavailable watch as healthy, provided consumers recognize it; zero fallback metrics must also remain unavailable. The supplied “RAW STOP” is an unrelated test, so actual foreground safe-off ordering is not established here.

The single 25%/28s screen is conditionally reasonable only within independently established E442 limits, with >120s off, verified candidate flash, disabled boot/all-off checks, unchanged trips, and no retry/escalation. It can reveal software stale origin/age, not physical cause, shutdown timing, or full qualification.
