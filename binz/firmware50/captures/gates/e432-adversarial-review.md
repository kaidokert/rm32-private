RAW: terminal accepts are 20620 → 21655 µs: 1035 µs apart. With `last=20620` and a 1000 µs ceiling, 21620 is legal; 21621 is stale. `event()` polls before updating `last`, so feeding 21655 necessarily latches Stale, assuming those are the watch timestamps.

Six matched accept→COM pairs give applied-minus-crossing-minus-wait errors of 5, 6, 6, 5, 6, 5 µs. Terminal acceptance requests 169 µs, nominally reaching 21824. Detector code arms **before** `guard_event`; an acceptance recorded after that call does not prove watchdog acceptance. Missing terminal COM is therefore consistent with shutdown canceling an already-armed timer, not evidence of timer failure.

Proceed offline with the separate NoChain binary, subject to falsifiable limits:

- Verify emitted roots remove logging while preserving detector decisions, publication-before-arm, guard invocation, and shutdown behavior.
- Test 1000/1001 µs boundaries and arm-before-stale cancellation.
- Demonstrate instruction/cycle reductions; code-size reduction alone is insufficient.
- Attribute neither the 35 µs excess nor startup failure to diagnostics without comparative evidence.

NoChain removes diagnostic work; it does not establish progress toward 80% operation.
