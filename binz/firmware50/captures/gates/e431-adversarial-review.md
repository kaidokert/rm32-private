The supplied evidence does not establish a new defect that bars the single bounded 25% attempt; it does not establish safety either.

One concrete code mismatch: `tighten_max_interval` rejects requests below `min_interval`, even with report-only fast events. Thus the advertised three-period deadline can stop tightening; the screen reports 240 µs against 76 µs mean sectors. This weakens that timing claim, but does not demonstrate a blocker for this attempt.

Coverage includes latched ISR shutdown, feedback/timing/event guards, absolute bus floor, filtered sag, and averaged-current protection. Sag uses eight-scan means; current uses signed 100-scan sums. Neither guarantees peak-current protection; ordered false crossings can satisfy tracking. The current verdict implementation is truncated here.

The evidence is a 15% run, not a 25% result. Thermal behavior and actual PSU limiting remain unmeasured; operator-set 3 A is not verified protection.

Scope: one 28-second, capped, propless reverse attempt after 120 seconds OFF, no retry or threshold changes. Success would not qualify 80%/12 V operation.
