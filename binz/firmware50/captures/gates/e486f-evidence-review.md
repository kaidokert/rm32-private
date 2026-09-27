The corrected logic supports a conditional, single-run narrow screen. Clearing UIF under UDIS, requiring a subsequent update, observing a successor COM, and waiting 43 µs address the premature-hold concern.

Evidence limits remain:

- The log reports 418 passing tests for e486e; it does not establish testing of final51007338. The e486f audit passes; ISR instructions match, with some encoded bytes differing.
- The shown regression uses Production, substitutes a COM snapshot, and injects update availability. It tests hold gating, not the actual SlowEntry transition.
- Hold accounting still has a boundary leak: `hold_start` is set before `det_poll()`, so `consume()` can credit pre-hold or coalesced acceptances to `hold_acc`. Target elapsed dwell and acceptance counts need separate claims.
- Disabled boot checks establish timer behavior, not energized transition quality.

Proceed only after the stated exact-flash verification/off PASS and bench conditions. Require measured ≥19 s target dwell; 28 s total alone proves neither completion nor qualification. Any fault ends the batch.