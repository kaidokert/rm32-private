- E485 stopped on Tracking after **4 ms**, seven accepts, **zero target hold**: stale age 1036 µs against 1000 µs. Final CCR 133/1333 indicates ~10% applied duty. This is entry-failure evidence, not a failed 15% hold.
- The supplied logs report 417 passing tests and unchanged ISR instructions. Neither establishes energized retiming behavior.

**Blockers:**

1. **Hold can start prematurely.** After retiming, `c.period=1333` makes settling 22 µs. A subsequent COM can occur before the pending native update, while the old 2666-tick cycle still governs overflow—up to ~42 µs. Observing COM plus 22 µs therefore does not guarantee target CCR/ARR activation. Bound settling by the old period or observe transfer completion.
2. **Race exclusion remains unproved.** `plans.lock` must exclude both COM and the stopping guard throughout owner/MOE checks, staging, publication and UDIS release. Its implementation is absent. Instruction identity does not establish that exclusion or bound added guard latency.
3. **Shadow proof is incomplete.** The model changes PWM roles immediately; the candidate selects `DiodeLatched`. It does not establish actual COM-latched mode/preload interactions. Disabled zero-compare testing cannot prove nonzero energized pulse geometry.

**Experiment limit:** After resolving these blockers, the proposed single `9` screen is bounded: exact-image preflight, reverse/propless, confirmed 3 A limit, ≥120 s OFF, faults terminate without retry. Require ≥19 s actual target hold. Success supports this schedule only; it neither isolates carrier causality nor qualifies operation.