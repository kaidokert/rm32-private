**Yes—this bounds a useful control experiment, without establishing carrier causality.**

- Terminal watch: `4718548 − 4717512 = 1036 µs`, exceeding the 1000 µs limit by 36 µs. Post-clock age is `4718551 − 4717512 = 1039 µs`.
- Raw-clock minus raw-accept is `23287 − 23273 = 14 ticks`. If these are 1 µs ticks on the same clock, the acceptance was only 14 µs old: a 1025 µs discrepancy requiring explanation. Clock units and publication semantics are absent; this does not yet prove a stale-watch bug.
- Target 150 means 15%; applied `133/1333 ≈ 9.98%`. With 4 ms closed and zero target hold, this is entry-failure evidence, not 15% operation.

Identity verification supports the reported image. Its first motor run after reset failed; prior 0F15-pass/25-entry-fail history cannot isolate same-image causality, reset state, or the stop/write fix.

Tests first, then one typed 24 kHz-entry/48 kHz-at-15% screen with unchanged seed/guards is discriminating. Review entry survival and transition separately; permit 25% only after clean reviewed evidence. Retain failures; no retry. Success supports the combined schedule, not carrier causality alone.
