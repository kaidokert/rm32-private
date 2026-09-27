No concrete blocker to the proposed single 25% exploratory screen is established here; safety at 25% remains unproven.

E480 sustained 20.278 s at 15%, with zero forced commutations, no reported LateArm, and coast agreement. That supports this operating point. One run per binary cannot attribute improvement to narrower commit: mailbox coalescing increased, loop-gap maximum rose 282→346 µs, and the earlier binary’s 25% LateArm remains relevant.

Interval-contraction protection is **not verified still live by this excerpt**. Guard ticks demonstrate activity, not that specific predicate; `oneshot.rs` shows exhausted-wait handling and source-checking tests, but neither the contraction implementation nor executed test results. This is an evidence limitation, not evidence of removal or a new gate.

Current proxy 249 versus 211 mA cannot establish increased physical current or 3 A headroom: calibration is absent, and reported zero drift (−304/−273 mA) exceeds those hold estimates. Zero margin/witness counters do not prove margin.

The same-binary, propless, ≥120 s OFF, command-8/28 s screen is supportable only as the stated exploration with existing protections unchanged. A clean outcome would not establish qualification or unique causality.
