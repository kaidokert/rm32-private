**Do not flash this as a passing `u` suite: case 8 contradicts the supplied helpers.**

- `guard_trip()` calls `com_stop()`, which stops TIM16, clears UIF, sets phase 0, and **unpends TIM16**. The refused `com_arm_crossing()` returns without changing that. Therefore `cleared` is true: flag bit 2 must be set, making expected `flags == 9` impossible. If COMP pending is also cleared, flags are **13**. `comp_exti_mask()` is omitted, so COMP clearing cannot independently be verified.
- No TIM16 event remains to invoke case 8’s handler. Expected behavior is **zero calls and timeout/failure**, not one stale service. To test stale dispatch refusal, explicitly pend TIM16 *after* stopping; test cancellation separately by observing no expiry through the former deadline.
- Case 6’s nominal observation ages are **501, 1501, 2001, 2501, 3001 µs**. Five pre-ack UIF samples witness real updates, but timestamps are only reported; spacing is not asserted. Late service can skip slots.
- Case 7 meaningfully checks replacement: stale TIM16 pending clears, then one UIF-backed callback arrives at 1000–1100 µs. It does **not** check survival past the obsolete 4000 µs deadline; cleanup stops observation early.

No execution evidence was supplied.
